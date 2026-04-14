//! Zone transition state machine — orchestrator-level tracking of InGame → LoadingZone → InGame flow.
//!
//! This module provides the orchestrator-level state machine that tracks zone transition
//! progress at a high level. It complements the nav-level `ZoneTransitionFsm` which handles
//! low-level movement and safe-coordinate validation. This state machine is platform-independent
//! and runs entirely in the external orchestrator process.
//!
//! States: Idle, Validating, Loading, InGame, Failed(code), Recovering
//! Transitions: request_zone → Validating, validation_ok → Loading, loaded → InGame,
//!             failure → Failed, retry → Recovering

use std::time::{Duration, Instant};
use textquest_common::types::ClientId;
use crate::zoning::failure_codes::{ZoneFailureCode, ZoneFailureState};

/// Maximum number of retry attempts before giving up on a zone transition.
const MAX_ZONE_RETRIES: u32 = 3;

/// Maximum duration allowed for the entire zone transition (Validating → Loading → InGame).
const ZONE_TRANSITION_TIMEOUT: Duration = Duration::from_secs(60);

/// Orchestrator-level zone transition states.
///
/// This FSM tracks the high-level progression of a zone transition.
/// It is separate from the nav-level `ZoneTransitionFsm` which handles movement details.
#[derive(Debug, Clone, PartialEq)]
pub enum ZoneTransitionState {
    /// No zone transition in progress.
    Idle,

    /// Validating that the zone transition is allowed (level requirements, lockouts, etc.).
    Validating {
        /// Target zone name.
        zone_name: String,
        /// When validation started.
        started_at: Instant,
    },

    /// Zone is loading — character is crossing zone boundary or landing.
    Loading {
        /// Target zone name.
        zone_name: String,
        /// When the loading phase started.
        started_at: Instant,
    },

    /// Zone transition completed successfully — character is in-game in the new zone.
    InGame {
        /// Current zone name.
        zone_name: String,
        /// When the character landed in the new zone.
        landed_at: Instant,
    },

    /// Zone transition failed with a specific failure code.
    Failed {
        /// The failure code.
        code: ZoneFailureCode,
        /// Human-readable failure reason.
        reason: String,
    },

    /// Recovering from a failed transition — will retry up to MAX_ZONE_RETRIES times.
    Recovering {
        /// The original failure code that triggered recovery.
        code: ZoneFailureCode,
        /// Current retry attempt number (1-indexed).
        retry_count: u32,
        /// When recovery started.
        started_at: Instant,
    },
}

/// Orchestrator-level zone transition state machine.
///
/// Tracks the overall progression of zone transitions for a single client.
/// Manages failure code tracking, retry counting, and timeout detection.
///
/// This FSM is primarily for orchestration-level bookkeeping. The actual movement
/// and coordinate validation is handled by the nav-level `ZoneTransitionFsm`.
///
/// # Example
///
/// ```rust
/// use textquest::zoning::state::ZoneTransitionStateMachine;
/// use textquest_common::types::ClientId;
///
/// let mut sm = ZoneTransitionStateMachine::new(1);
/// sm.request_zone("qeynos".to_string());
/// ```
pub struct ZoneTransitionStateMachine {
    /// The client this state machine belongs to.
    pub client_id: ClientId,

    /// Current state of the zone transition.
    state: ZoneTransitionState,

    /// Optional failure state (if in Failed or Recovering states).
    failure_state: Option<ZoneFailureState>,
}

impl ZoneTransitionStateMachine {
    /// Create a new idle zone transition state machine for the given client.
    pub fn new(client_id: ClientId) -> Self {
        Self {
            client_id,
            state: ZoneTransitionState::Idle,
            failure_state: None,
        }
    }

    /// Current state of the state machine.
    pub fn state(&self) -> &ZoneTransitionState {
        &self.state
    }

    /// Current failure state (if in Failed or Recovering states).
    pub fn failure_state(&self) -> Option<&ZoneFailureState> {
        self.failure_state.as_ref()
    }

    /// Whether the state machine is currently idle.
    pub fn is_idle(&self) -> bool {
        matches!(self.state, ZoneTransitionState::Idle)
    }

    /// Whether the state machine is in a terminal failed state (not recovering).
    pub fn is_failed(&self) -> bool {
        matches!(self.state, ZoneTransitionState::Failed { .. })
    }

    /// Whether the state machine is currently in-game (zone transition complete).
    pub fn is_in_game(&self) -> bool {
        matches!(self.state, ZoneTransitionState::InGame { .. })
    }

    /// Request a zone transition to the specified zone.
    ///
    /// Transitions from Idle → Validating.
    /// Panics if not currently idle.
    pub fn request_zone(&mut self, zone_name: String) {
        assert!(self.is_idle(), "Cannot request zone while in state: {:?}", self.state);
        tracing::info!(
            client_id = self.client_id,
            zone = %zone_name,
            "Requesting zone transition"
        );
        self.state = ZoneTransitionState::Validating {
            zone_name,
            started_at: Instant::now(),
        };
        self.failure_state = None;
    }

    /// Validate that the zone transition is allowed.
    ///
    /// Transitions from Validating → Loading.
    pub fn validation_ok(&mut self) {
        let zone_name = match &self.state {
            ZoneTransitionState::Validating { zone_name, .. } => zone_name.clone(),
            _ => {
                tracing::warn!(
                    client_id = self.client_id,
                    state = ?self.state,
                    "validation_ok called in wrong state"
                );
                return;
            }
        };

        tracing::info!(
            client_id = self.client_id,
            zone = %zone_name,
            "Zone validation succeeded — entering Loading"
        );
        self.state = ZoneTransitionState::Loading {
            zone_name,
            started_at: Instant::now(),
        };
    }

    /// Confirm that the zone has loaded and the character is in-game.
    ///
    /// Transitions from Loading → InGame.
    pub fn loaded(&mut self) {
        let zone_name = match &self.state {
            ZoneTransitionState::Loading { zone_name, .. } => zone_name.clone(),
            _ => {
                tracing::warn!(
                    client_id = self.client_id,
                    state = ?self.state,
                    "loaded called in wrong state"
                );
                return;
            }
        };

        tracing::info!(
            client_id = self.client_id,
            zone = %zone_name,
            "Zone loaded — character is in-game"
        );
        self.state = ZoneTransitionState::InGame {
            zone_name,
            landed_at: Instant::now(),
        };
        self.failure_state = None;
    }

    /// Signal a zone transition failure with the given failure code.
    ///
    /// If max retries have not been exceeded, transitions to Recovering.
    /// If max retries are exhausted, transitions to Failed (terminal).
    pub fn failure(&mut self, code: ZoneFailureCode) {
        tracing::warn!(
            client_id = self.client_id,
            code = ?code,
            description = %code.description(),
            "Zone transition failed"
        );

        let mut failure_state = ZoneFailureState::new(code, MAX_ZONE_RETRIES);
        if failure_state.can_retry() {
            tracing::info!(
                client_id = self.client_id,
                code = ?code,
                "Entering recovery (retries available)"
            );
            self.state = ZoneTransitionState::Recovering {
                code,
                retry_count: 1,
                started_at: Instant::now(),
            };
            self.failure_state = Some(failure_state);
        } else {
            tracing::error!(
                client_id = self.client_id,
                code = ?code,
                "No retries available — entering Failed state"
            );
            self.state = ZoneTransitionState::Failed {
                code,
                reason: code.description().to_string(),
            };
            self.failure_state = Some(failure_state);
        }
    }

    /// Attempt a retry after a failure.
    ///
    /// Transitions from Recovering → Validating.
    /// Only valid when in Recovering state.
    pub fn retry(&mut self) {
        let (code, zone_name) = match &self.state {
            ZoneTransitionState::Recovering { code, retry_count, .. } => {
                if let Some(fs) = &mut self.failure_state {
                    if *retry_count >= MAX_ZONE_RETRIES || !fs.can_retry() {
                        tracing::error!(
                            client_id = self.client_id,
                            code = ?code,
                            retry_count,
                            "Retry exhausted — cannot retry further"
                        );
                        self.state = ZoneTransitionState::Failed {
                            code: *code,
                            reason: code.description().to_string(),
                        };
                        return;
                    }
                }
                (*code, "unknown_zone".to_string())
            }
            _ => {
                tracing::warn!(
                    client_id = self.client_id,
                    state = ?self.state,
                    "retry called in wrong state"
                );
                return;
            }
        };

        let zone_name = zone_name.to_string();

        tracing::info!(
            client_id = self.client_id,
            code = ?code,
            zone = %zone_name,
            "Retrying zone transition"
        );
        self.state = ZoneTransitionState::Validating {
            zone_name,
            started_at: Instant::now(),
        };
    }

    /// Reset to idle state (e.g., on logout or manual cancel).
    pub fn reset(&mut self) {
        tracing::info!(client_id = self.client_id, "Resetting zone state machine");
        self.state = ZoneTransitionState::Idle;
        self.failure_state = None;
    }

    /// Get the elapsed time since the current state transition began.
    pub fn state_elapsed(&self) -> Duration {
        match &self.state {
            ZoneTransitionState::Idle => Duration::from_secs(0),
            ZoneTransitionState::Validating { started_at, .. }
            | ZoneTransitionState::Loading { started_at, .. }
            | ZoneTransitionState::Recovering { started_at, .. } => started_at.elapsed(),
            ZoneTransitionState::InGame { landed_at, .. } => landed_at.elapsed(),
            ZoneTransitionState::Failed { .. } => Duration::from_secs(0),
        }
    }

    /// Check if the current zone transition has exceeded the timeout.
    ///
    /// Returns `true` if in Validating, Loading, or Recovering state and elapsed time
    /// exceeds ZONE_TRANSITION_TIMEOUT.
    pub fn has_timed_out(&self) -> bool {
        self.state_elapsed() >= ZONE_TRANSITION_TIMEOUT
    }

    /// Retry count in Recovering state, or 0 if not recovering.
    pub fn retry_count(&self) -> u32 {
        match &self.state {
            ZoneTransitionState::Recovering { retry_count, .. } => *retry_count,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_machine_is_idle() {
        let sm = ZoneTransitionStateMachine::new(1);
        assert!(sm.is_idle());
        assert_eq!(sm.client_id, 1);
    }

    #[test]
    fn new_machine_has_no_failure_state() {
        let sm = ZoneTransitionStateMachine::new(1);
        assert!(sm.failure_state().is_none());
    }

    #[test]
    fn new_machine_retry_count_is_zero() {
        let sm = ZoneTransitionStateMachine::new(1);
        assert_eq!(sm.retry_count(), 0);
    }

    #[test]
    fn is_idle_on_idle_state() {
        let sm = ZoneTransitionStateMachine::new(1);
        assert!(sm.is_idle());
        assert!(!sm.is_failed());
        assert!(!sm.is_in_game());
    }

    #[test]
    fn request_zone_from_idle() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("qeynos".to_string());
        assert!(!sm.is_idle());
        assert!(matches!(
            sm.state(),
            ZoneTransitionState::Validating { zone_name, .. } if zone_name == "qeynos"
        ));
    }

    #[test]
    fn validation_ok_advances_to_loading() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("freeport".to_string());
        sm.validation_ok();
        assert!(matches!(
            sm.state(),
            ZoneTransitionState::Loading { zone_name, .. } if zone_name == "freeport"
        ));
    }

    #[test]
    fn loaded_advances_to_in_game() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("gfay".to_string());
        sm.validation_ok();
        sm.loaded();
        assert!(sm.is_in_game());
        assert!(matches!(
            sm.state(),
            ZoneTransitionState::InGame { zone_name, .. } if zone_name == "gfay"
        ));
    }

    #[test]
    fn normal_flow_clears_failure_state() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("commons".to_string());
        sm.validation_ok();
        sm.loaded();
        assert!(sm.failure_state().is_none());
    }

    #[test]
    fn failure_with_retries_available_enters_recovering() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("nektulos".to_string());
        sm.failure(ZoneFailureCode::PlayerInCombat);
        assert!(matches!(
            sm.state(),
            ZoneTransitionState::Recovering { code, retry_count: 1, .. }
                if code == ZoneFailureCode::PlayerInCombat
        ));
        assert!(sm.failure_state().is_some());
    }

    #[test]
    fn failure_with_no_retries_enters_failed_state() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("highpass".to_string());
        sm.failure(ZoneFailureCode::ExpansionNotUnlocked);
        assert!(sm.is_failed());
        assert!(matches!(
            sm.state(),
            ZoneTransitionState::Failed { code, .. }
                if code == ZoneFailureCode::ExpansionNotUnlocked
        ));
    }

    #[test]
    fn retry_from_recovering_goes_to_validating() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("sebilus".to_string());
        sm.failure(ZoneFailureCode::TooFar);
        sm.retry();
        assert!(matches!(
            sm.state(),
            ZoneTransitionState::Validating { zone_name, .. } if zone_name == "unknown_zone"
        ));
    }

    #[test]
    fn validation_ok_in_wrong_state_is_noop() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.validation_ok();
        assert!(sm.is_idle());
    }

    #[test]
    fn loaded_in_wrong_state_is_noop() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.loaded();
        assert!(sm.is_idle());
    }

    #[test]
    fn retry_in_wrong_state_is_noop() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.retry();
        assert!(sm.is_idle());
    }

    #[test]
    #[should_panic]
    fn request_zone_not_from_idle_panics() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("qeynos".to_string());
        sm.request_zone("freeport".to_string());
    }

    #[test]
    fn reset_from_validating_returns_to_idle() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("halas".to_string());
        sm.reset();
        assert!(sm.is_idle());
    }

    #[test]
    fn reset_from_loading_returns_to_idle() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("karana".to_string());
        sm.validation_ok();
        sm.reset();
        assert!(sm.is_idle());
    }

    #[test]
    fn reset_from_recovering_returns_to_idle() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("nektulos".to_string());
        sm.failure(ZoneFailureCode::TooFar);
        sm.reset();
        assert!(sm.is_idle());
    }

    #[test]
    fn reset_clears_failure_state() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("nektulos".to_string());
        sm.failure(ZoneFailureCode::TooFar);
        sm.reset();
        assert!(sm.failure_state().is_none());
    }

    #[test]
    fn idle_has_zero_elapsed() {
        let sm = ZoneTransitionStateMachine::new(1);
        assert_eq!(sm.state_elapsed(), Duration::from_secs(0));
    }

    #[test]
    fn validating_state_tracks_elapsed_time() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("qeynos".to_string());
        let elapsed = sm.state_elapsed();
        assert!(elapsed >= Duration::from_millis(0));
        assert!(elapsed < Duration::from_millis(100));
    }

    #[test]
    fn has_timed_out_is_false_initially() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("freeport".to_string());
        assert!(!sm.has_timed_out());
    }

    #[test]
    fn has_timed_out_true_after_timeout_duration() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("highpass".to_string());
        sm.state = ZoneTransitionState::Validating {
            zone_name: "highpass".to_string(),
            started_at: Instant::now() - ZONE_TRANSITION_TIMEOUT - Duration::from_secs(1),
        };
        assert!(sm.has_timed_out());
    }

    #[test]
    fn in_game_timeout_resets() {
        let mut sm = ZoneTransitionStateMachine::new(1);
        sm.request_zone("commons".to_string());
        sm.validation_ok();
        sm.loaded();
        assert_eq!(sm.state_elapsed(), Duration::from_secs(0));
    }

    #[test]
    fn multiple_clients_are_independent() {
        let mut sm1 = ZoneTransitionStateMachine::new(1);
        let mut sm2 = ZoneTransitionStateMachine::new(2);

        sm1.request_zone("qeynos".to_string());
        sm2.request_zone("freeport".to_string());

        assert!(matches!(
            sm1.state(),
            ZoneTransitionState::Validating { zone_name, .. } if zone_name == "qeynos"
        ));
        assert!(matches!(
            sm2.state(),
            ZoneTransitionState::Validating { zone_name, .. } if zone_name == "freeport"
        ));
    }

    #[test]
    fn non_retryable_codes_go_directly_to_failed() {
        let non_retryable = vec![
            ZoneFailureCode::WrongType,
            ZoneFailureCode::LevelTooLow,
            ZoneFailureCode::LevelTooHigh,
            ZoneFailureCode::RaidLockoutActive,
            ZoneFailureCode::ZoneRestricted,
            ZoneFailureCode::ExpansionNotUnlocked,
        ];

        for code in non_retryable {
            let mut sm = ZoneTransitionStateMachine::new(1);
            sm.request_zone("test".to_string());
            sm.failure(code);
            assert!(sm.is_failed(), "Expected {} to result in Failed state", code.description());
        }
    }

    #[test]
    fn retryable_codes_enter_recovering() {
        let retryable = vec![
            ZoneFailureCode::GeneralFailure,
            ZoneFailureCode::TooFar,
            ZoneFailureCode::InvalidCoordinates,
            ZoneFailureCode::NetworkTimeout,
            ZoneFailureCode::ZoneLoadTimeout,
        ];

        for code in retryable {
            let mut sm = ZoneTransitionStateMachine::new(1);
            sm.request_zone("test".to_string());
            sm.failure(code);
            assert!(
                matches!(sm.state(), ZoneTransitionState::Recovering { .. }),
                "Expected {} to enter Recovering state",
                code.description()
            );
        }
    }
}
