//! Safe-coordinate recovery orchestration for failed zone transitions.
//!
//! Handles the recovery flow when a zone transition fails due to invalid landing
//! coordinates or zone denial. Manages requesting safe coordinates from the DLL,
//! moving the player to the safe position, and retrying the original transition.

use std::time::{Duration, Instant};
use textquest_common::safe_coords::{
    is_safe_coordinate_tuple, positions_differ, SafeCoordRequest, SafeCoordResponse,
};
use textquest_common::types::ClientId;

/// Maximum number of recovery retries before giving up.
pub const MAX_RECOVERY_RETRIES: u32 = 3;

/// Backoff time between recovery attempts (exponential, with cap).
pub const MAX_RECOVERY_BACKOFF: Duration = Duration::from_secs(30);

/// Recovery state for a single client's failed zone transition.
#[derive(Debug, Clone)]
pub struct ZoneRecoveryState {
    /// Client this recovery belongs to.
    pub client_id: ClientId,

    /// Zone that was being entered.
    pub target_zone: String,

    /// The invalid landing position that triggered recovery.
    pub invalid_pos: (f32, f32, f32),

    /// Preferred fallback position (if known).
    pub fallback_pos: Option<(f32, f32, f32)>,

    /// Number of recovery attempts made so far.
    pub attempts: u32,

    /// When this recovery started (for backoff tracking).
    pub started_at: Instant,

    /// Current backoff deadline (when next retry is permitted).
    pub backoff_until: Instant,

    /// Whether we've requested safe coordinates from the DLL yet.
    pub request_sent: bool,

    /// Safe coordinates received from the DLL (if any).
    pub safe_coords: Option<(f32, f32, f32)>,

    /// Whether we've confirmed the player has moved to the safe position.
    pub moved_to_safe_pos: bool,
}

impl ZoneRecoveryState {
    /// Create a new recovery state for a zone transition failure.
    pub fn new(client_id: ClientId, target_zone: String, invalid_pos: (f32, f32, f32)) -> Self {
        let now = Instant::now();
        Self {
            client_id,
            target_zone,
            invalid_pos,
            fallback_pos: None,
            attempts: 0,
            started_at: now,
            backoff_until: now,
            request_sent: false,
            safe_coords: None,
            moved_to_safe_pos: false,
        }
    }

    /// Set a preferred fallback position.
    pub fn with_fallback(mut self, fallback: (f32, f32, f32)) -> Self {
        self.fallback_pos = Some(fallback);
        self
    }

    /// Check if retries are still available.
    pub fn can_retry(&self) -> bool {
        self.attempts < MAX_RECOVERY_RETRIES
    }

    /// Check if the backoff period has expired.
    pub fn backoff_expired(&self) -> bool {
        Instant::now() >= self.backoff_until
    }

    /// Advance to the next retry attempt (increment counter and set backoff deadline).
    ///
    /// Uses exponential backoff: 2^attempt seconds, capped at `MAX_RECOVERY_BACKOFF`.
    /// Returns `false` if maximum retries have been exhausted.
    pub fn advance_retry(&mut self) -> bool {
        if !self.can_retry() {
            tracing::warn!(
                client_id = self.client_id,
                target_zone = %self.target_zone,
                attempts = self.attempts,
                "Recovery attempts exhausted"
            );
            return false;
        }

        self.attempts += 1;
        let backoff_secs = 2_u64.pow(self.attempts.saturating_sub(1).min(10));
        let backoff = Duration::from_secs(backoff_secs).min(MAX_RECOVERY_BACKOFF);
        self.backoff_until = Instant::now() + backoff;

        tracing::debug!(
            client_id = self.client_id,
            target_zone = %self.target_zone,
            attempt = self.attempts,
            backoff_secs = backoff.as_secs(),
            "Zone recovery backoff scheduled"
        );

        true
    }

    /// Create a SafeCoordRequest to send to the DLL.
    pub fn create_request(&mut self) -> SafeCoordRequest {
        let mut req = SafeCoordRequest::new(self.target_zone.clone(), self.invalid_pos);
        if let Some(fallback) = self.fallback_pos {
            req = req.with_fallback(fallback);
        }
        self.request_sent = true;
        req
    }

    /// Handle a response from the DLL with safe coordinates.
    pub fn on_safe_coords_received(&mut self, response: SafeCoordResponse) {
        self.safe_coords = Some(response.safe_pos);
        tracing::info!(
            client_id = self.client_id,
            target_zone = %self.target_zone,
            safe_pos = ?response.safe_pos,
            reason = %response.reason,
            is_validated = response.is_validated,
            "Safe coordinates received for recovery"
        );
    }

    /// Notify that the player has moved to the safe position.
    pub fn on_moved_to_safe_pos(&mut self, current_pos: (f32, f32, f32)) {
        if let Some(safe) = self.safe_coords {
            if !positions_differ(current_pos, safe) {
                self.moved_to_safe_pos = true;
                tracing::info!(
                    client_id = self.client_id,
                    current_pos = ?current_pos,
                    safe_pos = ?safe,
                    "Player reached safe position"
                );
            }
        }
    }

    /// Check if recovery is ready to retry the original transition.
    pub fn ready_to_retry(&self) -> bool {
        self.backoff_expired() && self.safe_coords.is_some() && self.moved_to_safe_pos
    }

    /// Total elapsed time since recovery started.
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    /// Time remaining until the next retry is permitted.
    pub fn time_until_retry(&self) -> Duration {
        if self.backoff_until > Instant::now() {
            self.backoff_until - Instant::now()
        } else {
            Duration::from_secs(0)
        }
    }
}

/// Recovery flow validator — checks positions and coordinates during recovery.
pub struct RecoveryValidator;

impl RecoveryValidator {
    /// Validate that the requested safe position is actually safe.
    pub fn is_safe_for_recovery(pos: (f32, f32, f32)) -> bool {
        is_safe_coordinate_tuple(pos)
    }

    /// Validate that the DLL's response is sensible (position is safe, has reason).
    pub fn validate_response(response: &SafeCoordResponse) -> bool {
        !response.reason.is_empty() && Self::is_safe_for_recovery(response.safe_pos)
    }

    /// Validate that the player has sufficiently moved to the safe position.
    ///
    /// Uses position epsilon to avoid false positives from floating-point noise.
    pub fn has_moved_far_enough(from: (f32, f32, f32), to: (f32, f32, f32)) -> bool {
        positions_differ(from, to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── ZoneRecoveryState creation and initialization ───────────────────

    #[test]
    fn recovery_state_new() {
        let state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (100.0, 200.0, 50.0));
        assert_eq!(state.client_id, 1);
        assert_eq!(state.target_zone, "qey2hh1");
        assert_eq!(state.invalid_pos, (100.0, 200.0, 50.0));
        assert_eq!(state.attempts, 0);
        assert!(!state.request_sent);
        assert!(state.safe_coords.is_none());
        assert!(!state.moved_to_safe_pos);
    }

    #[test]
    fn recovery_state_with_fallback() {
        let state = ZoneRecoveryState::new(1, "gfay".to_string(), (0.0, 0.0, 0.0))
            .with_fallback((10.0, 20.0, 5.0));
        assert_eq!(state.fallback_pos, Some((10.0, 20.0, 5.0)));
    }

    // ─── Retry management tests ──────────────────────────────────────────

    #[test]
    fn can_retry_initially() {
        let state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (0.0, 0.0, 0.0));
        assert!(state.can_retry());
    }

    #[test]
    fn can_retry_after_attempts() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (0.0, 0.0, 0.0));
        state.attempts = 1;
        assert!(state.can_retry());
        state.attempts = 2;
        assert!(state.can_retry());
    }

    #[test]
    fn cannot_retry_when_exhausted() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (0.0, 0.0, 0.0));
        state.attempts = MAX_RECOVERY_RETRIES;
        assert!(!state.can_retry());
    }

    #[test]
    fn backoff_expired_initially() {
        let state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (0.0, 0.0, 0.0));
        assert!(state.backoff_expired());
    }

    #[test]
    fn advance_retry_increments_counter() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (0.0, 0.0, 0.0));
        assert_eq!(state.attempts, 0);
        assert!(state.advance_retry());
        assert_eq!(state.attempts, 1);
        assert!(state.advance_retry());
        assert_eq!(state.attempts, 2);
    }

    #[test]
    fn advance_retry_sets_backoff() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (0.0, 0.0, 0.0));
        let now = Instant::now();
        state.advance_retry();
        assert!(state.backoff_until > now);
    }

    #[test]
    fn advance_retry_returns_false_when_exhausted() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (0.0, 0.0, 0.0));
        for _ in 0..MAX_RECOVERY_RETRIES {
            assert!(state.advance_retry());
        }
        assert!(!state.advance_retry());
    }

    #[test]
    fn advance_retry_exponential_backoff() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (0.0, 0.0, 0.0));

        // 1st retry: 2^0 = 1 second
        state.advance_retry();
        let backoff_1 = state.backoff_until - Instant::now();

        // 2nd retry: 2^1 = 2 seconds
        state.backoff_until = Instant::now(); // Simulate backoff expiry
        state.advance_retry();
        let backoff_2 = state.backoff_until - Instant::now();

        // Backoff should roughly double
        assert!(backoff_2 > backoff_1);
    }

    // ─── Request/Response handling tests ─────────────────────────────────

    #[test]
    fn create_request_marks_sent() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (100.0, 200.0, 0.0));
        assert!(!state.request_sent);
        let _req = state.create_request();
        assert!(state.request_sent);
    }

    #[test]
    fn create_request_includes_zone_and_pos() {
        let mut state = ZoneRecoveryState::new(1, "gfay".to_string(), (50.0, 50.0, 0.0));
        let req = state.create_request();
        assert_eq!(req.zone, "gfay");
        assert_eq!(req.invalid_pos, (50.0, 50.0, 0.0));
    }

    #[test]
    fn create_request_includes_fallback() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (0.0, 0.0, 0.0))
            .with_fallback((10.0, 20.0, 5.0));
        let req = state.create_request();
        assert_eq!(req.fallback_pos, Some((10.0, 20.0, 5.0)));
    }

    #[test]
    fn on_safe_coords_received() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (100.0, 200.0, 0.0));
        let resp = SafeCoordResponse::origin("zone origin");
        state.on_safe_coords_received(resp);
        assert_eq!(state.safe_coords, Some((0.0, 0.0, 0.0)));
    }

    // ─── Movement tracking tests ─────────────────────────────────────────

    #[test]
    fn on_moved_to_safe_pos_sets_flag() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (100.0, 200.0, 0.0));
        let resp = SafeCoordResponse::origin("zone origin");
        state.on_safe_coords_received(resp);
        state.on_moved_to_safe_pos((0.0, 0.0, 0.0));
        assert!(state.moved_to_safe_pos);
    }

    #[test]
    fn on_moved_to_safe_pos_ignores_without_coords() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (100.0, 200.0, 0.0));
        state.on_moved_to_safe_pos((0.0, 0.0, 0.0));
        assert!(!state.moved_to_safe_pos); // No coords yet, so flag not set
    }

    #[test]
    fn on_moved_to_safe_pos_requires_proximity() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (100.0, 200.0, 0.0));
        let resp = SafeCoordResponse::custom((10.0, 20.0, 5.0), true, "test");
        state.on_safe_coords_received(resp);
        // Move to a different position (far away)
        state.on_moved_to_safe_pos((100.0, 100.0, 100.0));
        assert!(!state.moved_to_safe_pos); // Not close enough
    }

    // ─── Readiness checks ────────────────────────────────────────────────

    #[test]
    fn ready_to_retry_requires_all_conditions() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (100.0, 200.0, 0.0));

        // Not ready initially
        assert!(!state.ready_to_retry());

        // Add safe coords
        let resp = SafeCoordResponse::origin("zone origin");
        state.on_safe_coords_received(resp);
        assert!(!state.ready_to_retry()); // Still need to move

        // Mark as moved
        state.on_moved_to_safe_pos((0.0, 0.0, 0.0));
        assert!(state.ready_to_retry()); // Now ready!
    }

    #[test]
    fn ready_to_retry_respects_backoff() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (100.0, 200.0, 0.0));
        let resp = SafeCoordResponse::origin("zone origin");
        state.on_safe_coords_received(resp);
        state.on_moved_to_safe_pos((0.0, 0.0, 0.0));

        // Set backoff in the future
        state.backoff_until = Instant::now() + Duration::from_secs(10);
        assert!(!state.ready_to_retry()); // Still in backoff

        // Expire backoff
        state.backoff_until = Instant::now() - Duration::from_secs(1);
        assert!(state.ready_to_retry()); // Now ready
    }

    // ─── Time tracking tests ────────────────────────────────────────────

    #[test]
    fn elapsed_accumulates() {
        let state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (0.0, 0.0, 0.0));
        let elapsed1 = state.elapsed();
        std::thread::sleep(Duration::from_millis(10));
        let elapsed2 = state.elapsed();
        assert!(elapsed2 > elapsed1);
    }

    #[test]
    fn time_until_retry_future_backoff() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (0.0, 0.0, 0.0));
        state.backoff_until = Instant::now() + Duration::from_secs(5);
        let time_until = state.time_until_retry();
        assert!(time_until > Duration::from_secs(4));
        assert!(time_until <= Duration::from_secs(5));
    }

    #[test]
    fn time_until_retry_expired() {
        let mut state = ZoneRecoveryState::new(1, "qey2hh1".to_string(), (0.0, 0.0, 0.0));
        state.backoff_until = Instant::now() - Duration::from_secs(1);
        assert_eq!(state.time_until_retry(), Duration::from_secs(0));
    }

    // ─── RecoveryValidator tests ─────────────────────────────────────────

    #[test]
    fn validator_is_safe_for_recovery_valid() {
        assert!(RecoveryValidator::is_safe_for_recovery((100.0, 200.0, 50.0)));
    }

    #[test]
    fn validator_is_safe_for_recovery_oob() {
        assert!(!RecoveryValidator::is_safe_for_recovery((20_000.0, 0.0, 0.0)));
    }

    #[test]
    fn validator_validate_response_valid() {
        let resp = SafeCoordResponse::origin("test");
        assert!(RecoveryValidator::validate_response(&resp));
    }

    #[test]
    fn validator_validate_response_oob() {
        let resp = SafeCoordResponse::custom((99_999.0, 0.0, 0.0), true, "bad");
        assert!(!RecoveryValidator::validate_response(&resp));
    }

    #[test]
    fn validator_validate_response_empty_reason() {
        let resp = SafeCoordResponse::custom((0.0, 0.0, 0.0), true, "");
        assert!(!RecoveryValidator::validate_response(&resp));
    }

    #[test]
    fn validator_has_moved_far_enough_yes() {
        assert!(RecoveryValidator::has_moved_far_enough(
            (0.0, 0.0, 0.0),
            (100.0, 0.0, 0.0)
        ));
    }

    #[test]
    fn validator_has_moved_far_enough_no() {
        assert!(!RecoveryValidator::has_moved_far_enough(
            (0.0, 0.0, 0.0),
            (0.01, 0.0, 0.0)
        ));
    }

    // ─── Integration tests ──────────────────────────────────────────────

    #[test]
    fn recovery_flow_complete_scenario() {
        // 1. Zone denied entry with OOB coords
        let mut state = ZoneRecoveryState::new(2, "nektulos".to_string(), (99_999.0, 0.0, 0.0));

        // 2. Check we can retry
        assert!(state.can_retry());

        // 3. Request safe coords
        let req = state.create_request();
        assert_eq!(req.zone, "nektulos");
        assert!(state.request_sent);

        // 4. DLL responds
        let resp = SafeCoordResponse::origin("zone origin");
        state.on_safe_coords_received(resp);
        assert_eq!(state.safe_coords, Some((0.0, 0.0, 0.0)));

        // 5. Player moves to safe pos
        state.on_moved_to_safe_pos((0.0, 0.0, 0.0));
        assert!(state.moved_to_safe_pos);

        // 6. Check ready to retry
        assert!(state.ready_to_retry());
    }

    #[test]
    fn recovery_flow_with_retries_and_backoff() {
        let mut state = ZoneRecoveryState::new(3, "qeynos".to_string(), (100.0, 100.0, 0.0));

        // Attempt 1
        assert!(state.advance_retry());
        assert_eq!(state.attempts, 1);
        assert!(!state.backoff_expired());

        // Attempt 2
        state.backoff_until = Instant::now() - Duration::from_millis(1);
        assert!(state.advance_retry());
        assert_eq!(state.attempts, 2);

        // Attempt 3
        state.backoff_until = Instant::now() - Duration::from_millis(1);
        assert!(state.advance_retry());
        assert_eq!(state.attempts, 3);

        // No more attempts
        assert!(!state.advance_retry());
        assert!(!state.can_retry());
    }
}
