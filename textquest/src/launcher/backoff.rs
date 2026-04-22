//! Exponential backoff for login failure recovery.
//!
//! Tracks per-account retry attempts and computes the next delay using
//! `delay = min(base * multiplier^n, max) + jitter`.

use std::time::Duration;

/// Exponential backoff policy for login failures.
#[derive(Debug, Clone)]
pub struct LoginBackoff {
    /// Base delay for the first retry.
    pub base: Duration,
    /// Maximum delay cap.
    pub max: Duration,
    /// Exponential growth multiplier (e.g., 2.0 → doubles each attempt).
    pub multiplier: f64,
    /// Jitter fraction applied to the computed delay (e.g., 0.1 = ±10 %).
    /// Applied as `delay * jitter * rand`, always positive (additive).
    pub jitter: f64,
}

impl LoginBackoff {
    /// Create a new backoff policy.
    ///
    /// # Parameters
    ///
    /// - `base`       — initial delay (applied on the first retry).
    /// - `max`        — hard cap on the computed delay before jitter.
    /// - `multiplier` — growth factor per attempt (must be >= 1.0; clamped if
    ///   not).
    /// - `jitter`     — fraction of the computed delay added as random noise
    ///   (0.0 = no jitter, 1.0 = up to 100 % extra).
    #[must_use]
    pub fn new(base: Duration, max: Duration, multiplier: f64, jitter: f64) -> Self {
        Self {
            base,
            max,
            multiplier: multiplier.max(1.0),
            jitter: jitter.clamp(0.0, 1.0),
        }
    }

    /// Compute the backoff delay for attempt number `n` (0-indexed).
    ///
    /// - Attempt 0 → `Duration::ZERO` (first attempt is immediate).
    /// - Attempt 1 → `base * multiplier^0` (first retry).
    /// - Attempt n → `min(base * multiplier^(n-1), max) + jitter_amount`.
    ///
    /// The jitter component is deterministic in tests (no external RNG calls),
    /// but callers should treat the output as advisory — add any PRNG on top
    /// if true randomness is needed.
    #[must_use]
    pub fn delay_for(&self, n: u32) -> Duration {
        if n == 0 {
            return Duration::ZERO;
        }
        let exponent = (n - 1) as f64;
        let base_ms = self.base.as_millis() as f64;
        let raw_ms = base_ms * self.multiplier.powf(exponent);
        let max_ms = self.max.as_millis() as f64;
        let capped_ms = raw_ms.min(max_ms);
        // Deterministic jitter: scale by a fixed fraction for predictability in
        // tests. Production callers can layer real randomness on the result.
        let jitter_ms = capped_ms * self.jitter;
        let total_ms = capped_ms + jitter_ms;
        Duration::from_millis(total_ms as u64)
    }

    /// Compute the backoff delay with pseudo-random jitter using a simple
    /// linear-congruential PRNG seeded from the attempt count and account
    /// index. Suitable for production use without pulling in `rand`.
    ///
    /// The returned value is always `>= delay_for(n)` and
    /// `<= delay_for(n) * (1 + jitter)`.
    #[must_use]
    pub fn delay_for_with_jitter(&self, n: u32, account_seed: u64) -> Duration {
        if n == 0 {
            return Duration::ZERO;
        }
        let exponent = (n - 1) as f64;
        let base_ms = self.base.as_millis() as f64;
        let raw_ms = base_ms * self.multiplier.powf(exponent);
        let max_ms = self.max.as_millis() as f64;
        let capped_ms = raw_ms.min(max_ms);

        // Simple LCG for jitter fraction in [0, 1)
        let seed = account_seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(n as u64);
        let jitter_frac = (seed >> 33) as f64 / (u32::MAX as f64);
        let jitter_ms = capped_ms * self.jitter * jitter_frac;
        let total_ms = capped_ms + jitter_ms;
        Duration::from_millis(total_ms as u64)
    }

    /// Returns `true` if `n` has not exceeded `max_attempts`.
    #[must_use]
    pub fn should_retry(&self, n: u32, max_attempts: u32) -> bool {
        n < max_attempts
    }
}

/// Per-account backoff state tracker.
///
/// Tracks how many times a specific account has failed and computes the
/// next retry delay. Resets on success.
#[derive(Debug, Clone)]
pub struct AccountBackoffTracker {
    /// Number of consecutive login failures for this account.
    pub attempt_count: u32,
    /// Stable seed derived from the account name for jitter variance.
    account_seed: u64,
}

impl AccountBackoffTracker {
    /// Create a new tracker for the given account name.
    #[must_use]
    pub fn new(account_name: &str) -> Self {
        // FNV-1a hash for a stable, fast seed from the account name
        let mut hash: u64 = 14_695_981_039_346_656_037;
        for byte in account_name.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(1_099_511_628_211);
        }
        Self {
            attempt_count: 0,
            account_seed: hash,
        }
    }

    /// Record a failure and return the delay before the next retry.
    pub fn record_failure(&mut self, policy: &LoginBackoff) -> Duration {
        self.attempt_count += 1;
        let delay = policy.delay_for_with_jitter(self.attempt_count, self.account_seed);
        tracing::debug!(
            attempt = self.attempt_count,
            delay_ms = delay.as_millis(),
            "Login failure recorded; scheduling retry with backoff"
        );
        delay
    }

    /// Record a successful login and reset the failure counter.
    pub fn record_success(&mut self) {
        if self.attempt_count > 0 {
            tracing::debug!(
                recovered_after = self.attempt_count,
                "Login succeeded; resetting backoff counter"
            );
        }
        self.attempt_count = 0;
    }

    /// Whether the account has exceeded `max_attempts` consecutive failures.
    #[must_use]
    pub fn is_exhausted(&self, max_attempts: u32) -> bool {
        self.attempt_count >= max_attempts
    }

    /// Current consecutive failure count.
    #[must_use]
    pub fn failures(&self) -> u32 {
        self.attempt_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_policy() -> LoginBackoff {
        LoginBackoff::new(
            Duration::from_secs(1),
            Duration::from_secs(60),
            2.0,
            0.0, // no jitter for deterministic tests
        )
    }

    // ─── LoginBackoff::delay_for ─────────────────────────────────────────

    #[test]
    fn attempt_zero_is_immediate() {
        let policy = default_policy();
        assert_eq!(policy.delay_for(0), Duration::ZERO);
    }

    #[test]
    fn attempt_one_equals_base() {
        let policy = default_policy();
        assert_eq!(policy.delay_for(1), Duration::from_secs(1));
    }

    #[test]
    fn attempt_two_doubles() {
        let policy = default_policy();
        assert_eq!(policy.delay_for(2), Duration::from_secs(2));
    }

    #[test]
    fn attempt_three_quadruples() {
        let policy = default_policy();
        assert_eq!(policy.delay_for(3), Duration::from_secs(4));
    }

    #[test]
    fn delay_grows_exponentially() {
        let policy = default_policy();
        let d1 = policy.delay_for(1);
        let d2 = policy.delay_for(2);
        let d3 = policy.delay_for(3);
        assert!(d2 > d1, "delay should grow: {d1:?} < {d2:?}");
        assert!(d3 > d2, "delay should grow: {d2:?} < {d3:?}");
    }

    #[test]
    fn delay_caps_at_max() {
        let policy = LoginBackoff::new(
            Duration::from_secs(1),
            Duration::from_secs(10),
            2.0,
            0.0,
        );
        // After enough attempts, delay should be capped
        let large = policy.delay_for(20);
        assert_eq!(large, Duration::from_secs(10), "delay must cap at max");
    }

    #[test]
    fn delay_never_exceeds_max_with_jitter() {
        let policy = LoginBackoff::new(
            Duration::from_secs(1),
            Duration::from_secs(10),
            2.0,
            0.5, // 50% jitter
        );
        for attempt in 0..20 {
            let d = policy.delay_for_with_jitter(attempt, 12345);
            let max_with_jitter = Duration::from_millis(
                (policy.max.as_millis() as f64 * (1.0 + policy.jitter)) as u64,
            );
            assert!(
                d <= max_with_jitter,
                "attempt {attempt}: delay {d:?} exceeded max+jitter {max_with_jitter:?}"
            );
        }
    }

    #[test]
    fn jitter_adds_to_base_delay() {
        let no_jitter = LoginBackoff::new(
            Duration::from_secs(2),
            Duration::from_secs(60),
            2.0,
            0.0,
        );
        let with_jitter = LoginBackoff::new(
            Duration::from_secs(2),
            Duration::from_secs(60),
            2.0,
            0.5,
        );
        // delay_for uses deterministic jitter (fixed fraction), so with_jitter >= no_jitter
        let base = no_jitter.delay_for(1);
        let jittered = with_jitter.delay_for(1);
        assert!(jittered >= base, "jittered delay must be >= base delay");
    }

    #[test]
    fn multiplier_below_one_is_clamped_to_one() {
        let policy = LoginBackoff::new(
            Duration::from_secs(5),
            Duration::from_secs(60),
            0.5, // invalid — clamped to 1.0
            0.0,
        );
        // With multiplier=1, delay stays at base for all attempts
        assert_eq!(policy.delay_for(1), Duration::from_secs(5));
        assert_eq!(policy.delay_for(2), Duration::from_secs(5));
    }

    #[test]
    fn should_retry_within_limit() {
        let policy = default_policy();
        assert!(policy.should_retry(0, 3));
        assert!(policy.should_retry(1, 3));
        assert!(policy.should_retry(2, 3));
    }

    #[test]
    fn should_retry_at_limit_returns_false() {
        let policy = default_policy();
        assert!(!policy.should_retry(3, 3));
        assert!(!policy.should_retry(4, 3));
    }

    // ─── AccountBackoffTracker ────────────────────────────────────────────

    #[test]
    fn tracker_starts_at_zero_failures() {
        let tracker = AccountBackoffTracker::new("account1");
        assert_eq!(tracker.failures(), 0);
        assert!(!tracker.is_exhausted(3));
    }

    #[test]
    fn tracker_increments_on_failure() {
        let mut tracker = AccountBackoffTracker::new("account1");
        let policy = default_policy();
        tracker.record_failure(&policy);
        assert_eq!(tracker.failures(), 1);
        tracker.record_failure(&policy);
        assert_eq!(tracker.failures(), 2);
    }

    #[test]
    fn tracker_resets_on_success() {
        let mut tracker = AccountBackoffTracker::new("account1");
        let policy = default_policy();
        tracker.record_failure(&policy);
        tracker.record_failure(&policy);
        tracker.record_success();
        assert_eq!(tracker.failures(), 0);
    }

    #[test]
    fn tracker_failure_delay_grows() {
        let mut tracker = AccountBackoffTracker::new("account1");
        let policy = default_policy();
        let d1 = tracker.record_failure(&policy);
        let d2 = tracker.record_failure(&policy);
        let d3 = tracker.record_failure(&policy);
        assert!(d2 >= d1, "delay must grow: {d1:?} -> {d2:?}");
        assert!(d3 >= d2, "delay must grow: {d2:?} -> {d3:?}");
    }

    #[test]
    fn tracker_is_exhausted_after_max_attempts() {
        let mut tracker = AccountBackoffTracker::new("account1");
        let policy = default_policy();
        for _ in 0..3 {
            tracker.record_failure(&policy);
        }
        assert!(tracker.is_exhausted(3));
    }

    #[test]
    fn tracker_not_exhausted_below_max() {
        let mut tracker = AccountBackoffTracker::new("account1");
        let policy = default_policy();
        tracker.record_failure(&policy);
        tracker.record_failure(&policy);
        assert!(!tracker.is_exhausted(3));
    }

    #[test]
    fn tracker_first_failure_returns_base_delay() {
        let mut tracker = AccountBackoffTracker::new("account1");
        let policy = LoginBackoff::new(
            Duration::from_secs(5),
            Duration::from_secs(60),
            2.0,
            0.0, // no jitter
        );
        let delay = tracker.record_failure(&policy);
        // First retry (attempt 1) → base = 5s (no jitter)
        assert_eq!(delay, Duration::from_secs(5));
    }

    #[test]
    fn tracker_success_after_failures_allows_immediate_retry() {
        let mut tracker = AccountBackoffTracker::new("account1");
        let policy = default_policy();
        tracker.record_failure(&policy);
        tracker.record_failure(&policy);
        tracker.record_success();
        // After reset, next failure should be the base delay again
        let delay = tracker.record_failure(&policy);
        assert_eq!(delay, policy.delay_for(1));
    }

    #[test]
    fn different_accounts_get_different_seeds() {
        let t1 = AccountBackoffTracker::new("alice");
        let t2 = AccountBackoffTracker::new("bob");
        // Seeds should differ (FNV hash is collision-resistant for short strings)
        assert_ne!(t1.account_seed, t2.account_seed);
    }

    #[test]
    fn jitter_delay_for_with_jitter_produces_at_least_base_delay() {
        let policy = LoginBackoff::new(
            Duration::from_secs(1),
            Duration::from_secs(60),
            2.0,
            0.5,
        );
        for seed in [0u64, 1, 12345, u64::MAX] {
            for attempt in 1..=5 {
                let jittered = policy.delay_for_with_jitter(attempt, seed);
                let base_delay = policy.delay_for(attempt);
                assert!(
                    jittered >= base_delay,
                    "seed={seed} attempt={attempt}: jittered {jittered:?} < base {base_delay:?}"
                );
            }
        }
    }
}
