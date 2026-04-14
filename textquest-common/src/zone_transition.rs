//! Zone transition retry logic with exponential backoff.
//!
//! Provides failure state tracking and retry FSM for zone transition operations
//! with configurable exponential backoff. Designed to handle transient failures
//! that can be recovered by waiting and retrying.

use std::time::{Duration, Instant};

/// Zone transition failure states.
///
/// Represents specific failure modes that can occur during zone transitions.
/// Each failure type has specific handling requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZoneTransitionFailureState {
    /// Character is blocked or stuck in terrain.
    ///
    /// Recovery: Move to nearby safe position, retry transition.
    Blocked,

    /// Zone transition exceeded time limit.
    ///
    /// Recovery: Wait for game to respond, retry the transition.
    Timeout,

    /// Zone animation is lagging or incomplete.
    ///
    /// Recovery: Wait for animation to complete, then retry.
    AnimationLag,

    /// Zone queue is at capacity.
    ///
    /// Recovery: Wait for zone queue to deplete, then retry.
    QueueOverflow,
}

impl ZoneTransitionFailureState {
    /// Get a human-readable description of this failure state.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Blocked => "Character is blocked or stuck in terrain",
            Self::Timeout => "Zone transition exceeded time limit",
            Self::AnimationLag => "Zone animation is lagging or incomplete",
            Self::QueueOverflow => "Zone queue is at capacity",
        }
    }
}

/// Blocker detection types for zone transition failures.
///
/// Used to identify what kind of obstruction or condition is preventing
/// the zone transition from completing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockerDetection {
    /// Character movement is blocked by collision.
    CollisionObstruction,

    /// Character is stuck in geometry.
    GeometryStuck,

    /// Zone server is not responding.
    ZoneServerUnresponsive,

    /// Game is processing zone animation.
    AnimationInProgress,

    /// Zone queue is full and rejecting new entries.
    ZoneQueueFull,

    /// Character is in a state that prevents zoning (e.g., combat, seated).
    InvalidCharacterState,

    /// Network communication is degraded or lost.
    NetworkDegraded,

    /// Unknown obstruction — unable to determine specific cause.
    Unknown,
}

impl BlockerDetection {
    /// Get a human-readable description of this blocker type.
    pub fn description(&self) -> &'static str {
        match self {
            Self::CollisionObstruction => "Movement blocked by collision",
            Self::GeometryStuck => "Stuck in geometry",
            Self::ZoneServerUnresponsive => "Zone server not responding",
            Self::AnimationInProgress => "Zone animation in progress",
            Self::ZoneQueueFull => "Zone queue full",
            Self::InvalidCharacterState => "Invalid character state for zoning",
            Self::NetworkDegraded => "Network communication degraded",
            Self::Unknown => "Unknown blocker",
        }
    }
}

/// Zone transition retry state tracker.
///
/// Manages exponential backoff retry logic for zone transitions.
/// Tracks failure, retry count, and backoff timing.
#[derive(Debug, Clone)]
pub struct ZoneTransitionRetryState {
    /// The current failure state (if any).
    pub failure: Option<ZoneTransitionFailureState>,

    /// The blocker detection (if any).
    pub blocker: Option<BlockerDetection>,

    /// Number of retry attempts made.
    pub attempt_count: u32,

    /// Maximum number of retry attempts before giving up (default: 3).
    pub max_attempts: u32,

    /// Base backoff delay in milliseconds (default: 500).
    pub base_backoff_ms: u64,

    /// Maximum backoff delay in milliseconds (default: 5000).
    pub max_backoff_ms: u64,

    /// When the current backoff period expires (when next retry is allowed).
    backoff_until: Instant,

    /// When the failure first occurred.
    failed_at: Instant,
}

impl ZoneTransitionRetryState {
    /// Create a new retry state with default backoff parameters.
    ///
    /// Default: base 500ms, max 5000ms, max 3 attempts.
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            failure: None,
            blocker: None,
            attempt_count: 0,
            max_attempts: 3,
            base_backoff_ms: 500,
            max_backoff_ms: 5000,
            backoff_until: now,
            failed_at: now,
        }
    }

    /// Create a new retry state with custom max attempts.
    pub fn with_max_attempts(max_attempts: u32) -> Self {
        let mut state = Self::new();
        state.max_attempts = max_attempts;
        state
    }

    /// Create a new retry state with custom backoff parameters.
    pub fn with_backoff(base_ms: u64, max_ms: u64, max_attempts: u32) -> Self {
        Self {
            failure: None,
            blocker: None,
            attempt_count: 0,
            max_attempts,
            base_backoff_ms: base_ms,
            max_backoff_ms: max_ms,
            backoff_until: Instant::now(),
            failed_at: Instant::now(),
        }
    }

    /// Record a failure and start backoff.
    pub fn record_failure(
        &mut self,
        failure: ZoneTransitionFailureState,
        blocker: Option<BlockerDetection>,
    ) {
        self.failure = Some(failure);
        self.blocker = blocker;
        self.attempt_count += 1;
        self.failed_at = Instant::now();
        self.schedule_backoff();

        tracing::debug!(
            attempt = self.attempt_count,
            failure = ?failure,
            blocker = ?blocker,
            "Zone transition failure recorded"
        );
    }

    /// Schedule the next backoff period using exponential backoff formula.
    ///
    /// Backoff time = `base_backoff_ms * 2^(attempt_count - 1)`, capped at `max_backoff_ms`.
    fn schedule_backoff(&mut self) {
        if self.attempt_count == 0 {
            self.backoff_until = Instant::now();
            return;
        }

        let exponent = (self.attempt_count - 1).min(10);
        let backoff_ms = self
            .base_backoff_ms
            .saturating_mul(2u64.pow(exponent))
            .min(self.max_backoff_ms);

        self.backoff_until = Instant::now() + Duration::from_millis(backoff_ms);

        tracing::debug!(
            attempt = self.attempt_count,
            backoff_ms,
            "Zone transition backoff scheduled"
        );
    }

    /// Check if another retry is available.
    pub fn can_retry(&self) -> bool {
        self.attempt_count < self.max_attempts && self.backoff_expired()
    }

    /// Check if the backoff period has expired.
    pub fn backoff_expired(&self) -> bool {
        Instant::now() >= self.backoff_until
    }

    /// Get the time remaining until the next retry is allowed.
    pub fn time_until_retry(&self) -> Duration {
        self.backoff_until.saturating_duration_since(Instant::now())
    }

    /// Get the total elapsed time since the failure occurred.
    pub fn elapsed_since_failure(&self) -> Duration {
        self.failed_at.elapsed()
    }

    /// Check if retries have been exhausted.
    pub fn exhausted(&self) -> bool {
        self.attempt_count >= self.max_attempts
    }

    /// Reset the retry state to initial condition.
    pub fn reset(&mut self) {
        self.failure = None;
        self.blocker = None;
        self.attempt_count = 0;
        self.backoff_until = Instant::now();
        self.failed_at = Instant::now();
    }

    /// Get the current backoff duration based on attempt count.
    pub fn current_backoff(&self) -> Duration {
        if self.attempt_count == 0 {
            Duration::from_millis(0)
        } else {
            let exponent = (self.attempt_count - 1).min(10);
            let backoff_ms = self
                .base_backoff_ms
                .saturating_mul(2u64.pow(exponent))
                .min(self.max_backoff_ms);
            Duration::from_millis(backoff_ms)
        }
    }
}

impl Default for ZoneTransitionRetryState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    // ─── ZoneTransitionFailureState tests ──────────────────────────────────

    #[test]
    fn failure_state_blocked_description() {
        let state = ZoneTransitionFailureState::Blocked;
        assert!(!state.description().is_empty());
        assert!(state.description().len() > 5);
    }

    #[test]
    fn failure_state_timeout_description() {
        let state = ZoneTransitionFailureState::Timeout;
        assert!(!state.description().is_empty());
    }

    #[test]
    fn failure_state_animation_lag_description() {
        let state = ZoneTransitionFailureState::AnimationLag;
        assert!(!state.description().is_empty());
    }

    #[test]
    fn failure_state_queue_overflow_description() {
        let state = ZoneTransitionFailureState::QueueOverflow;
        assert!(!state.description().is_empty());
    }

    // ─── BlockerDetection tests ────────────────────────────────────────────

    #[test]
    fn blocker_collision_obstruction() {
        let blocker = BlockerDetection::CollisionObstruction;
        assert_eq!(blocker, BlockerDetection::CollisionObstruction);
        assert!(!blocker.description().is_empty());
    }

    #[test]
    fn blocker_geometry_stuck() {
        let blocker = BlockerDetection::GeometryStuck;
        assert!(!blocker.description().is_empty());
    }

    #[test]
    fn blocker_zone_server_unresponsive() {
        let blocker = BlockerDetection::ZoneServerUnresponsive;
        assert!(!blocker.description().is_empty());
    }

    #[test]
    fn blocker_animation_in_progress() {
        let blocker = BlockerDetection::AnimationInProgress;
        assert!(!blocker.description().is_empty());
    }

    #[test]
    fn blocker_zone_queue_full() {
        let blocker = BlockerDetection::ZoneQueueFull;
        assert!(!blocker.description().is_empty());
    }

    #[test]
    fn blocker_invalid_character_state() {
        let blocker = BlockerDetection::InvalidCharacterState;
        assert!(!blocker.description().is_empty());
    }

    #[test]
    fn blocker_network_degraded() {
        let blocker = BlockerDetection::NetworkDegraded;
        assert!(!blocker.description().is_empty());
    }

    #[test]
    fn blocker_unknown() {
        let blocker = BlockerDetection::Unknown;
        assert!(!blocker.description().is_empty());
    }

    // ─── ZoneTransitionRetryState basic tests ──────────────────────────────

    #[test]
    fn new_retry_state_is_clean() {
        let state = ZoneTransitionRetryState::new();
        assert_eq!(state.attempt_count, 0);
        assert_eq!(state.max_attempts, 3);
        assert_eq!(state.base_backoff_ms, 500);
        assert_eq!(state.max_backoff_ms, 5000);
        assert!(state.failure.is_none());
        assert!(state.blocker.is_none());
        assert!(!state.exhausted());
    }

    #[test]
    fn with_max_attempts_sets_limit() {
        let state = ZoneTransitionRetryState::with_max_attempts(5);
        assert_eq!(state.max_attempts, 5);
        assert_eq!(state.attempt_count, 0);
    }

    #[test]
    fn with_backoff_sets_parameters() {
        let state = ZoneTransitionRetryState::with_backoff(100, 2000, 4);
        assert_eq!(state.base_backoff_ms, 100);
        assert_eq!(state.max_backoff_ms, 2000);
        assert_eq!(state.max_attempts, 4);
    }

    #[test]
    fn default_creates_new_state() {
        let state = ZoneTransitionRetryState::default();
        assert_eq!(state.attempt_count, 0);
        assert_eq!(state.max_attempts, 3);
    }

    // ─── Failure recording and backoff tests ───────────────────────────────

    #[test]
    fn record_failure_increments_attempt() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        assert_eq!(state.attempt_count, 1);
        assert_eq!(state.failure, Some(ZoneTransitionFailureState::Blocked));
    }

    #[test]
    fn record_failure_with_blocker() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(
            ZoneTransitionFailureState::Timeout,
            Some(BlockerDetection::ZoneServerUnresponsive),
        );
        assert_eq!(state.attempt_count, 1);
        assert_eq!(state.failure, Some(ZoneTransitionFailureState::Timeout));
        assert_eq!(
            state.blocker,
            Some(BlockerDetection::ZoneServerUnresponsive)
        );
    }

    #[test]
    fn multiple_failures_increment_attempts() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        assert_eq!(state.attempt_count, 1);

        state.backoff_until = Instant::now();
        state.record_failure(ZoneTransitionFailureState::Timeout, None);
        assert_eq!(state.attempt_count, 2);
    }

    // ─── Backoff expiration tests ──────────────────────────────────────────

    #[test]
    fn new_state_backoff_already_expired() {
        let state = ZoneTransitionRetryState::new();
        assert!(state.backoff_expired());
    }

    #[test]
    fn backoff_expired_after_recording_failure() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        // Backoff was just scheduled, so not expired yet
        assert!(!state.backoff_expired());
    }

    #[test]
    fn can_retry_requires_backoff_expired() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        assert!(!state.can_retry());

        // Simulate backoff expiry
        state.backoff_until = Instant::now() - Duration::from_millis(1);
        assert!(state.can_retry());
    }

    // ─── Exponential backoff formula tests ────────────────────────────────

    #[test]
    fn exponential_backoff_first_attempt() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        let backoff = state.current_backoff();
        // First attempt: 500ms * 2^0 = 500ms
        assert_eq!(backoff, Duration::from_millis(500));
    }

    #[test]
    fn exponential_backoff_second_attempt() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        state.backoff_until = Instant::now();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        let backoff = state.current_backoff();
        // Second attempt: 500ms * 2^1 = 1000ms
        assert_eq!(backoff, Duration::from_millis(1000));
    }

    #[test]
    fn exponential_backoff_third_attempt() {
        let mut state = ZoneTransitionRetryState::new();
        for _ in 0..3 {
            state.record_failure(ZoneTransitionFailureState::Blocked, None);
            state.backoff_until = Instant::now();
        }
        let backoff = state.current_backoff();
        // Third attempt: 500ms * 2^2 = 2000ms
        assert_eq!(backoff, Duration::from_millis(2000));
    }

    #[test]
    fn exponential_backoff_capped_at_max() {
        let mut state = ZoneTransitionRetryState::with_backoff(500, 5000, 10);
        // Simulate many attempts to exceed the max backoff
        for _ in 0..20 {
            state.record_failure(ZoneTransitionFailureState::Blocked, None);
            state.backoff_until = Instant::now();
        }
        let backoff = state.current_backoff();
        // Should be capped at max_backoff_ms = 5000ms
        assert!(backoff <= Duration::from_millis(5000));
    }

    // ─── Can retry tests ──────────────────────────────────────────────────

    #[test]
    fn can_retry_before_max_attempts() {
        let mut state = ZoneTransitionRetryState::with_max_attempts(3);
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        state.backoff_until = Instant::now() - Duration::from_millis(1);
        assert!(state.can_retry());
    }

    #[test]
    fn can_retry_exhausted_after_max_attempts() {
        let mut state = ZoneTransitionRetryState::with_max_attempts(2);
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        state.backoff_until = Instant::now();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        state.backoff_until = Instant::now();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        state.backoff_until = Instant::now();
        assert!(!state.can_retry());
    }

    #[test]
    fn exhausted_after_max_attempts() {
        let mut state = ZoneTransitionRetryState::with_max_attempts(3);
        assert!(!state.exhausted());

        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        assert!(!state.exhausted());

        state.backoff_until = Instant::now();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        assert!(!state.exhausted());

        state.backoff_until = Instant::now();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        assert!(state.exhausted());
    }

    // ─── Time tracking tests ──────────────────────────────────────────────

    #[test]
    fn time_until_retry_before_expiry() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        let time_until = state.time_until_retry();
        assert!(time_until > Duration::from_millis(490));
        assert!(time_until <= Duration::from_millis(510));
    }

    #[test]
    fn time_until_retry_after_expiry() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        state.backoff_until = Instant::now() - Duration::from_secs(1);
        let time_until = state.time_until_retry();
        assert_eq!(time_until, Duration::from_secs(0));
    }

    #[test]
    fn elapsed_since_failure_accumulates() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        let elapsed1 = state.elapsed_since_failure();
        thread::sleep(Duration::from_millis(10));
        let elapsed2 = state.elapsed_since_failure();
        assert!(elapsed2 > elapsed1);
        assert!(elapsed2 > Duration::from_millis(5));
    }

    // ─── Reset tests ──────────────────────────────────────────────────────

    #[test]
    fn reset_clears_state() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(
            ZoneTransitionFailureState::Timeout,
            Some(BlockerDetection::Unknown),
        );
        assert_eq!(state.attempt_count, 1);
        assert!(state.failure.is_some());

        state.reset();
        assert_eq!(state.attempt_count, 0);
        assert!(state.failure.is_none());
        assert!(state.blocker.is_none());
        assert!(state.backoff_expired());
    }

    // ─── Integration scenarios ────────────────────────────────────────────

    #[test]
    fn scenario_single_blocked_retry() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(
            ZoneTransitionFailureState::Blocked,
            Some(BlockerDetection::GeometryStuck),
        );

        assert_eq!(state.attempt_count, 1);
        assert!(!state.can_retry()); // Backoff not expired

        thread::sleep(Duration::from_millis(510));
        assert!(state.can_retry()); // Backoff expired
    }

    #[test]
    fn scenario_multiple_retries_with_backoff() {
        let mut state = ZoneTransitionRetryState::with_backoff(100, 2000, 3);

        // First failure
        state.record_failure(ZoneTransitionFailureState::Blocked, None);
        assert_eq!(state.current_backoff(), Duration::from_millis(100));

        // Simulate backoff expiry and second failure
        state.backoff_until = Instant::now();
        state.record_failure(ZoneTransitionFailureState::Timeout, None);
        assert_eq!(state.current_backoff(), Duration::from_millis(200));

        // Simulate backoff expiry and third failure
        state.backoff_until = Instant::now();
        state.record_failure(ZoneTransitionFailureState::AnimationLag, None);
        assert_eq!(state.current_backoff(), Duration::from_millis(400));

        // All retries exhausted
        assert!(state.exhausted());
        assert!(!state.can_retry());
    }

    #[test]
    fn scenario_queue_overflow_retry() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(
            ZoneTransitionFailureState::QueueOverflow,
            Some(BlockerDetection::ZoneQueueFull),
        );

        assert!(state.failure.is_some());
        assert!(state.blocker.is_some());
        assert!(!state.can_retry());
    }

    #[test]
    fn scenario_animation_lag_retry() {
        let mut state = ZoneTransitionRetryState::new();
        state.record_failure(
            ZoneTransitionFailureState::AnimationLag,
            Some(BlockerDetection::AnimationInProgress),
        );

        assert!(state.failure.is_some());
        assert_eq!(state.blocker, Some(BlockerDetection::AnimationInProgress));
        assert!(!state.can_retry());
    }
}
