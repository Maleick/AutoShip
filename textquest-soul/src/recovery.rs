//! Soul Engine error handling and recovery — retry, fallback, reload, restart
//! strategies.

use std::time::Duration;

use super::{config::SoulConfig, llm::LlmResponse};

/// Errors that can occur within the Soul Engine subsystems.
#[derive(Debug)]
pub enum SoulError {
    /// SQLite database connection lost or query failed.
    DbConnectionLoss(String),
    /// LLM provider timed out or returned no response.
    LlmTimeout(String),
    /// In-memory state appears corrupted (out-of-range values, invalid IDs,
    /// etc.).
    MemoryCorruption(String),
    /// The coordinator task panicked or entered an unrecoverable state.
    CoordinatorPanic(String),
}

impl std::fmt::Display for SoulError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DbConnectionLoss(msg) => write!(f, "Soul DB connection loss: {msg}"),
            Self::LlmTimeout(msg) => write!(f, "Soul LLM timeout: {msg}"),
            Self::MemoryCorruption(msg) => write!(f, "Soul memory corruption: {msg}"),
            Self::CoordinatorPanic(msg) => write!(f, "Soul coordinator panic: {msg}"),
        }
    }
}

impl std::error::Error for SoulError {}

/// The state a `SoulCoordinator` can be restored to after a hard reset.
///
/// Carries only the configuration needed to re-initialise from scratch; runtime
/// data (registered characters, in-flight LLM requests, social graph) must be
/// re-populated by the caller after the restart.
#[derive(Debug, Clone)]
pub struct SoulState {
    /// Configuration used to reconstruct the coordinator.
    pub config: SoulConfig,
    /// Human-readable label for logging/diagnostics.
    pub label: String,
}

impl SoulState {
    /// Create a default `SoulState` from a `SoulConfig`.
    pub fn from_config(config: SoulConfig) -> Self {
        Self {
            config,
            label: String::from("default"),
        }
    }
}

/// What the caller should do in response to a `SoulError`.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum RecoveryAction {
    /// Retry the failed operation after the given delay (exponential backoff).
    Retry(Duration),
    /// Use a cached LLM response as a stand-in while the real provider is
    /// unavailable.
    Fallback(LlmResponse),
    /// Discard in-memory state and reload from the database.
    Reload,
    /// Tear down the coordinator entirely and restart with the given default
    /// state.
    Restart(SoulState),
}

/// Manages error recovery for the Soul Engine.
///
/// `RecoveryManager` is stateful: it tracks consecutive failure counts so it
/// can apply exponential backoff for transient errors (e.g., DB blips) and
/// escalate to harder recovery actions when a subsystem is persistently
/// failing.
pub struct RecoveryManager {
    /// Consecutive DB errors seen without a successful operation in between.
    db_failures: u32,
    /// Consecutive LLM timeout errors seen.
    llm_failures: u32,
    /// Default state used when a full `Restart` is required.
    default_state: SoulState,
    /// Cached LLM response returned during `LlmTimeout` recovery.
    cached_response: LlmResponse,
}

/// Base delay for the first retry attempt (doubles each subsequent attempt).
const BASE_RETRY_MS: u64 = 250;
/// Maximum backoff delay cap.
const MAX_RETRY_MS: u64 = 30_000;
/// Number of consecutive failures before escalating to a harder action.
const ESCALATION_THRESHOLD: u32 = 5;

impl RecoveryManager {
    /// Construct a new `RecoveryManager`.
    ///
    /// `default_state` is the `SoulState` used when a `Restart` action is
    /// issued. `cached_response` is the `LlmResponse` returned during
    /// `Fallback` actions.
    pub fn new(default_state: SoulState, cached_response: LlmResponse) -> Self {
        Self {
            db_failures: 0,
            llm_failures: 0,
            default_state,
            cached_response,
        }
    }

    /// Decide how to handle a `SoulError`.
    ///
    /// The returned `RecoveryAction` tells the caller what to do next:
    /// - `Retry(duration)` — wait then retry the failing operation.
    /// - `Fallback(response)` — use a cached response in place of the LLM.
    /// - `Reload` — discard in-memory state and reload from the database.
    /// - `Restart(state)` — full restart with the default state.
    pub fn handle_error(&mut self, error: SoulError) -> RecoveryAction {
        match error {
            SoulError::DbConnectionLoss(_) => {
                self.db_failures += 1;
                let delay = self.backoff_delay(self.db_failures);
                RecoveryAction::Retry(delay)
            }
            SoulError::LlmTimeout(_) => {
                self.llm_failures += 1;
                if self.llm_failures >= ESCALATION_THRESHOLD {
                    // Persistent LLM failure: fall back to cached response
                    RecoveryAction::Fallback(self.cached_response.clone())
                } else {
                    let delay = self.backoff_delay(self.llm_failures);
                    RecoveryAction::Retry(delay)
                }
            }
            SoulError::MemoryCorruption(_) => {
                // Reload from DB to restore clean state
                RecoveryAction::Reload
            }
            SoulError::CoordinatorPanic(_) => {
                // Full restart with default state
                RecoveryAction::Restart(self.default_state.clone())
            }
        }
    }

    /// Notify the manager that a DB operation succeeded, resetting the failure
    /// counter.
    pub fn reset_db_failures(&mut self) {
        self.db_failures = 0;
    }

    /// Notify the manager that an LLM call succeeded, resetting the failure
    /// counter.
    pub fn reset_llm_failures(&mut self) {
        self.llm_failures = 0;
    }

    /// Compute exponential backoff capped at `MAX_RETRY_MS`.
    fn backoff_delay(&self, failures: u32) -> Duration {
        // 2^(failures-1) * BASE_RETRY_MS, saturating at MAX_RETRY_MS
        let exp = failures.saturating_sub(1);
        let shift = exp.min(63);
        let multiplier = 1u64.checked_shl(shift).unwrap_or(u64::MAX);
        let ms = BASE_RETRY_MS.saturating_mul(multiplier).min(MAX_RETRY_MS);
        Duration::from_millis(ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager() -> RecoveryManager {
        let config = SoulConfig::default();
        let state = SoulState::from_config(config);
        let cached = LlmResponse {
            text: String::from("I'm here."),
            from_llm: false,
            tokens_used: 0,
        };
        RecoveryManager::new(state, cached)
    }

    // --- DB connection loss ---

    #[test]
    fn test_db_connection_loss_returns_retry() {
        let mut mgr = make_manager();
        let action = mgr.handle_error(SoulError::DbConnectionLoss("timeout".into()));
        assert!(matches!(action, RecoveryAction::Retry(_)));
    }

    #[test]
    fn test_db_connection_loss_backoff_increases() {
        let mut mgr = make_manager();
        let a1 = mgr.handle_error(SoulError::DbConnectionLoss("e1".into()));
        let a2 = mgr.handle_error(SoulError::DbConnectionLoss("e2".into()));
        let (RecoveryAction::Retry(d1), RecoveryAction::Retry(d2)) = (a1, a2) else {
            panic!("expected Retry actions");
        };
        assert!(d2 >= d1, "backoff should increase: d1={d1:?} d2={d2:?}");
    }

    #[test]
    fn test_db_reset_clears_failure_count() {
        let mut mgr = make_manager();
        // Build up failures
        for _ in 0..3 {
            mgr.handle_error(SoulError::DbConnectionLoss("e".into()));
        }
        assert_eq!(mgr.db_failures, 3);
        mgr.reset_db_failures();
        assert_eq!(mgr.db_failures, 0);
        // After reset, backoff should be back at base
        let action = mgr.handle_error(SoulError::DbConnectionLoss("e".into()));
        let RecoveryAction::Retry(d) = action else {
            panic!("expected Retry");
        };
        assert_eq!(d, Duration::from_millis(BASE_RETRY_MS));
    }

    // --- LLM timeout ---

    #[test]
    fn test_llm_timeout_returns_retry_below_threshold() {
        let mut mgr = make_manager();
        let action = mgr.handle_error(SoulError::LlmTimeout("no response".into()));
        assert!(matches!(action, RecoveryAction::Retry(_)));
    }

    #[test]
    fn test_llm_timeout_escalates_to_fallback_at_threshold() {
        let mut mgr = make_manager();
        // Drive failures up to and past the escalation threshold
        let mut last_action = None;
        for _ in 0..ESCALATION_THRESHOLD {
            last_action = Some(mgr.handle_error(SoulError::LlmTimeout("no response".into())));
        }
        assert!(
            matches!(last_action, Some(RecoveryAction::Fallback(_))),
            "expected Fallback after {ESCALATION_THRESHOLD} consecutive LLM failures"
        );
    }

    #[test]
    fn test_llm_fallback_response_matches_cached() {
        let mut mgr = make_manager();
        for _ in 0..ESCALATION_THRESHOLD {
            mgr.handle_error(SoulError::LlmTimeout("t".into()));
        }
        // One more to confirm fallback is stable
        let action = mgr.handle_error(SoulError::LlmTimeout("t".into()));
        let RecoveryAction::Fallback(resp) = action else {
            panic!("expected Fallback");
        };
        assert_eq!(resp.text, "I'm here.");
        assert!(!resp.from_llm);
    }

    // --- Memory corruption ---

    #[test]
    fn test_memory_corruption_returns_reload() {
        let mut mgr = make_manager();
        let action = mgr.handle_error(SoulError::MemoryCorruption("bad ptr".into()));
        assert!(matches!(action, RecoveryAction::Reload));
    }

    // --- Coordinator panic ---

    #[test]
    fn test_coordinator_panic_returns_restart() {
        let mut mgr = make_manager();
        let action = mgr.handle_error(SoulError::CoordinatorPanic("thread panicked".into()));
        assert!(matches!(action, RecoveryAction::Restart(_)));
    }

    #[test]
    fn test_coordinator_panic_restart_contains_default_state() {
        let mut mgr = make_manager();
        let action = mgr.handle_error(SoulError::CoordinatorPanic("gone".into()));
        let RecoveryAction::Restart(state) = action else {
            panic!("expected Restart");
        };
        assert_eq!(state.label, "default");
    }
}
