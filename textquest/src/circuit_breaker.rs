//! Circuit breaker for mass failure protection.
//!
//! Implements the classic 3-state circuit breaker pattern:
//!
//! ```text
//! ┌────────┐  threshold reached  ┌──────┐
//! │ Closed │ ──────────────────► │ Open │
//! └────────┘                     └──────┘
//!     ▲                              │
//!     │ probe succeeds         reset_timeout
//!     │                              │
//!     │                              ▼
//! ┌──────────┐  probe fails    ┌──────────┐
//! │ HalfOpen │ ◄────────────── │ HalfOpen │
//! └──────────┘                 └──────────┘
//! ```
//!
//! - **Closed**: Normal operation. Failures are counted in a sliding window.
//!   When the count reaches `failure_threshold`, transitions to **Open**.
//! - **Open**: All operations are rejected immediately. After `reset_timeout`
//!   elapses, transitions to **HalfOpen**.
//! - **HalfOpen**: A limited number of probe operations are allowed
//!   (`half_open_max_probes`). On success, transitions back to **Closed**. On
//!   failure, transitions back to **Open**.
//!
//! The implementation uses `std::sync::Mutex` for interior mutability so the
//! breaker can be shared across threads via `Arc<CircuitBreaker>`.

use std::{
    collections::VecDeque,
    sync::Mutex,
    time::{Duration, Instant},
};

/// State of the circuit breaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — failures are being tracked.
    Closed,
    /// All operations rejected; waiting for reset timeout.
    Open {
        /// When the breaker is allowed to move to [`CircuitState::HalfOpen`].
        retry_after: Instant,
    },
    /// Limited probe operations allowed to test recovery.
    HalfOpen {
        /// Number of probes that have succeeded so far.
        successes: u32,
    },
}

/// Inner mutable state of the circuit breaker, guarded by a `Mutex`.
struct Inner {
    state: CircuitState,
    /// Timestamps of recent failures (sliding window).
    failure_window: VecDeque<Instant>,
}

/// A thread-safe 3-state circuit breaker.
///
/// Wrap in an `Arc` to share across tasks/threads:
///
/// ```rust
/// use std::sync::Arc;
/// use textquest::circuit_breaker::CircuitBreaker;
///
/// let cb = Arc::new(CircuitBreaker::new(3, 60, 30, 1));
/// ```
pub struct CircuitBreaker {
    /// Number of failures in the window to trip the breaker.
    failure_threshold: u32,
    /// Sliding window length for failure counting.
    time_window: Duration,
    /// How long the breaker stays Open before moving to HalfOpen.
    reset_timeout: Duration,
    /// Consecutive successes in HalfOpen needed to close.
    half_open_max_probes: u32,
    inner: Mutex<Inner>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    ///
    /// # Parameters
    ///
    /// - `failure_threshold`: Failures in `time_window_secs` to trip to Open.
    /// - `time_window_secs`: Sliding window duration in seconds.
    /// - `reset_timeout_secs`: Seconds to stay Open before attempting HalfOpen.
    /// - `half_open_max_probes`: Successful probes required to close from
    ///   HalfOpen.
    #[must_use]
    pub fn new(
        failure_threshold: u32,
        time_window_secs: u64,
        reset_timeout_secs: u64,
        half_open_max_probes: u32,
    ) -> Self {
        Self {
            failure_threshold,
            time_window: Duration::from_secs(time_window_secs),
            reset_timeout: Duration::from_secs(reset_timeout_secs),
            half_open_max_probes,
            inner: Mutex::new(Inner {
                state: CircuitState::Closed,
                failure_window: VecDeque::new(),
            }),
        }
    }

    /// Whether the breaker is currently open (all calls should be rejected).
    ///
    /// If the breaker is Open and the reset timeout has elapsed, this
    /// transparently transitions to HalfOpen before returning `false`.
    pub fn is_open(&self) -> bool {
        let mut guard = self.inner.lock().expect("circuit breaker mutex poisoned");
        self.check_timeout_transition(&mut guard);
        matches!(guard.state, CircuitState::Open { .. })
    }

    /// Record a failure event.
    ///
    /// - In **Closed**: increments the sliding-window failure count and trips
    ///   to Open if the threshold is reached.
    /// - In **HalfOpen**: immediately trips back to Open (probe failed).
    /// - In **Open**: records the timestamp but takes no additional action
    ///   (breaker is already open).
    pub fn record_failure(&self) {
        let mut guard = self.inner.lock().expect("circuit breaker mutex poisoned");
        self.check_timeout_transition(&mut guard);

        let now = Instant::now();

        match guard.state {
            CircuitState::Closed => {
                guard.failure_window.push_back(now);
                self.prune_window(&mut guard, now);
                let count = guard.failure_window.len() as u32;
                if count >= self.failure_threshold {
                    tracing::warn!(
                        threshold = self.failure_threshold,
                        window_secs = self.time_window.as_secs(),
                        "Circuit breaker tripped — pausing all launches for {}s",
                        self.reset_timeout.as_secs()
                    );
                    guard.state = CircuitState::Open {
                        retry_after: now + self.reset_timeout,
                    };
                    guard.failure_window.clear();
                }
            }
            CircuitState::HalfOpen { .. } => {
                tracing::warn!(
                    "Circuit breaker probe failed — returning to Open for {}s",
                    self.reset_timeout.as_secs()
                );
                guard.state = CircuitState::Open {
                    retry_after: now + self.reset_timeout,
                };
            }
            CircuitState::Open { .. } => {
                // Already open; no additional action needed.
            }
        }
    }

    /// Record a successful operation.
    ///
    /// - In **Closed**: clears the failure window (success resets streak).
    /// - In **HalfOpen**: increments the probe success counter. When
    ///   `half_open_max_probes` successes are reached, closes the breaker.
    /// - In **Open**: no-op.
    pub fn record_success(&self) {
        let mut guard = self.inner.lock().expect("circuit breaker mutex poisoned");
        self.check_timeout_transition(&mut guard);

        match guard.state {
            CircuitState::Closed => {
                guard.failure_window.clear();
            }
            CircuitState::HalfOpen { ref mut successes } => {
                *successes += 1;
                if *successes >= self.half_open_max_probes {
                    tracing::info!(
                        probes = self.half_open_max_probes,
                        "Circuit breaker probe succeeded — closing"
                    );
                    guard.state = CircuitState::Closed;
                    guard.failure_window.clear();
                }
            }
            CircuitState::Open { .. } => {
                // Success while Open is unexpected but harmless.
            }
        }
    }

    /// Return a clone of the current state (for inspection / metrics).
    pub fn state(&self) -> CircuitState {
        let mut guard = self.inner.lock().expect("circuit breaker mutex poisoned");
        self.check_timeout_transition(&mut guard);
        guard.state.clone()
    }

    /// Number of failures currently tracked in the sliding window.
    ///
    /// Always 0 when the breaker is Open or HalfOpen (the window is cleared on
    /// transition).
    pub fn failure_count(&self) -> usize {
        let mut guard = self.inner.lock().expect("circuit breaker mutex poisoned");
        let now = Instant::now();
        self.prune_window(&mut guard, now);
        guard.failure_window.len()
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    /// If the breaker is Open and the retry_after has elapsed, move to
    /// HalfOpen.
    fn check_timeout_transition(&self, guard: &mut Inner) {
        if let CircuitState::Open { retry_after } = guard.state
            && Instant::now() >= retry_after
        {
            tracing::info!(
                probes_needed = self.half_open_max_probes,
                "Circuit breaker reset timeout elapsed — entering HalfOpen"
            );
            guard.state = CircuitState::HalfOpen { successes: 0 };
        }
    }

    /// Remove failure timestamps that are outside the sliding window.
    fn prune_window(&self, guard: &mut Inner, now: Instant) {
        while let Some(&ts) = guard.failure_window.front() {
            if now.duration_since(ts) > self.time_window {
                guard.failure_window.pop_front();
            } else {
                break;
            }
        }
    }
}

impl std::fmt::Debug for CircuitBreaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state();
        f.debug_struct("CircuitBreaker")
            .field("failure_threshold", &self.failure_threshold)
            .field("time_window", &self.time_window)
            .field("reset_timeout", &self.reset_timeout)
            .field("half_open_max_probes", &self.half_open_max_probes)
            .field("state", &state)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn breaker(threshold: u32, window_secs: u64, reset_secs: u64, probes: u32) -> CircuitBreaker {
        CircuitBreaker::new(threshold, window_secs, reset_secs, probes)
    }

    // ── Closed → Open transition ─────────────────────────────────────────────

    #[test]
    fn starts_closed_not_open() {
        let cb = breaker(3, 60, 30, 1);
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(!cb.is_open());
    }

    #[test]
    fn below_threshold_stays_closed() {
        let cb = breaker(3, 60, 30, 1);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(!cb.is_open());
        assert_eq!(cb.failure_count(), 2);
    }

    #[test]
    fn at_threshold_trips_to_open() {
        let cb = breaker(3, 60, 30, 1);
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_open());
        assert!(matches!(cb.state(), CircuitState::Open { .. }));
    }

    #[test]
    fn open_clears_failure_window() {
        let cb = breaker(3, 60, 30, 1);
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        // Window should be cleared when tripped
        assert_eq!(cb.failure_count(), 0);
    }

    // ── Open → HalfOpen transition ───────────────────────────────────────────

    /// Build a breaker that goes Open with a very short reset timeout so tests
    /// can drive the Open → HalfOpen transition without sleeping long.
    fn tripped_open_then_half_open(probes: u32) -> CircuitBreaker {
        // threshold=1, window=60s, reset=1s (short but > 0)
        let cb = breaker(1, 60, 1, probes);
        cb.record_failure();
        // Immediately Open because threshold == 1
        assert!(cb.is_open(), "expected Open after threshold reached");

        // Force the retry_after into the past by directly manipulating state
        {
            let mut guard = cb.inner.lock().unwrap();
            guard.state = CircuitState::Open {
                retry_after: Instant::now() - Duration::from_millis(1),
            };
        }
        cb
    }

    #[test]
    fn open_to_half_open_after_timeout() {
        let cb = tripped_open_then_half_open(1);
        // is_open() should detect elapsed timeout and move to HalfOpen
        assert!(!cb.is_open(), "expected HalfOpen (not Open) after timeout");
        assert!(matches!(cb.state(), CircuitState::HalfOpen { .. }));
    }

    // ── HalfOpen → Closed transition ────────────────────────────────────────

    #[test]
    fn half_open_closes_on_probe_success() {
        let cb = tripped_open_then_half_open(1);
        // Drive into HalfOpen
        let _ = cb.is_open();
        assert!(matches!(cb.state(), CircuitState::HalfOpen { .. }));

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(!cb.is_open());
    }

    #[test]
    fn half_open_requires_all_probes_before_closing() {
        let cb = tripped_open_then_half_open(3);
        let _ = cb.is_open(); // → HalfOpen

        cb.record_success();
        assert!(matches!(cb.state(), CircuitState::HalfOpen { successes: 1 }));
        cb.record_success();
        assert!(matches!(cb.state(), CircuitState::HalfOpen { successes: 2 }));
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    // ── HalfOpen → Open transition (probe failure) ───────────────────────────

    #[test]
    fn half_open_failure_returns_to_open() {
        let cb = tripped_open_then_half_open(1);
        let _ = cb.is_open(); // → HalfOpen
        assert!(matches!(cb.state(), CircuitState::HalfOpen { .. }));

        cb.record_failure(); // probe failed → back to Open
        assert!(cb.is_open(), "expected Open after probe failure");
    }

    // ── Success in Closed state ──────────────────────────────────────────────

    #[test]
    fn success_in_closed_resets_failure_window() {
        let cb = breaker(3, 60, 30, 1);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);
        cb.record_success();
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    // ── Sliding window expiry ────────────────────────────────────────────────

    #[test]
    fn failures_outside_window_do_not_count() {
        // 0-second window — failures expire immediately
        let cb = breaker(3, 0, 30, 1);
        cb.record_failure();
        cb.record_failure();
        // Sleep to ensure the timestamps fall outside the window
        std::thread::sleep(Duration::from_millis(5));
        // failure_count() prunes the window
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    // ── Debug output ─────────────────────────────────────────────────────────

    #[test]
    fn debug_output_contains_state() {
        let cb = breaker(3, 60, 30, 1);
        let s = format!("{cb:?}");
        assert!(s.contains("CircuitBreaker"));
        assert!(s.contains("Closed"));
    }

    // ── Thread safety smoke test ─────────────────────────────────────────────

    #[test]
    fn thread_safe_concurrent_record_failure() {
        use std::sync::Arc;
        let cb = Arc::new(breaker(100, 60, 30, 1));
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let cb = Arc::clone(&cb);
                std::thread::spawn(move || {
                    for _ in 0..5 {
                        cb.record_failure();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        // 50 total failures, threshold 100 → still Closed
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 50);
    }
}
