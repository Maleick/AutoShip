//! Failure detection and recovery routing for the economy loop.
//!
//! Tracks failure states per actor, maps failure types to recovery actions,
//! and escalates repeated failures to operator attention.

use std::collections::HashMap;

use textquest_common::types::ClientId;

/// Categories of failure that can occur during economy loop execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FailureType {
    /// Character inventory is at or above carry weight limit.
    OverweightInventory,
    /// Bank slots are completely full — cannot deposit items.
    FullBank,
    /// Vendor NPC is not present in the zone or cannot be found.
    MissingVendor,
    /// Navigation could not plot a path to the target location.
    UnreachablePath,
    /// IPC or network communication timed out.
    NetworkTimeout,
    /// The EQ client process is not responding or has disconnected.
    ClientOffline,
}

/// Actions the recovery router can prescribe in response to a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Retry the failed operation after a short delay.
    Retry,
    /// Navigate back to the home camp position and resume from there.
    ReturnHome,
    /// Abandon the current economy cycle and wait for the next scheduled run.
    AbortCycle,
    /// Pause automation and notify the operator for manual intervention.
    EscalateToOperator,
}

/// A snapshot of a single failure event for one actor.
#[derive(Debug, Clone)]
pub struct FailureState {
    /// The category of failure that occurred.
    pub failure_type: FailureType,
    /// The client that experienced the failure.
    pub actor: ClientId,
    /// Unix timestamp (seconds) when the failure was recorded.
    pub timestamp: u64,
    /// The recovery action prescribed for this failure.
    pub recovery_action: RecoveryAction,
    /// How many times this same failure type has been attempted/retried.
    pub attempt_count: u32,
}

impl FailureState {
    /// Create a new `FailureState` with an initial attempt count of 1.
    pub fn new(
        failure_type: FailureType,
        actor: ClientId,
        timestamp: u64,
        recovery_action: RecoveryAction,
    ) -> Self {
        Self {
            failure_type,
            actor,
            timestamp,
            recovery_action,
            attempt_count: 1,
        }
    }
}

/// Maps `FailureType` variants to their default `RecoveryAction`.
///
/// The routing logic follows a simple priority model:
/// - Transient failures (timeout, path) → `Retry`
/// - Resource exhaustion (overweight, full bank) → `ReturnHome` to offload
/// - Missing dependency (vendor) → `AbortCycle` (cannot proceed without it)
/// - Hard failure (client offline) → `EscalateToOperator`
pub struct FailureRouter;

impl FailureRouter {
    /// Return the default recovery action for the given failure type.
    pub fn route(failure: &FailureType) -> RecoveryAction {
        match failure {
            FailureType::NetworkTimeout => RecoveryAction::Retry,
            FailureType::UnreachablePath => RecoveryAction::Retry,
            FailureType::OverweightInventory => RecoveryAction::ReturnHome,
            FailureType::FullBank => RecoveryAction::ReturnHome,
            FailureType::MissingVendor => RecoveryAction::AbortCycle,
            FailureType::ClientOffline => RecoveryAction::EscalateToOperator,
        }
    }
}

/// Tracks recent failures per actor and detects escalation patterns.
///
/// When the same failure type is recorded 3 or more times for a single actor,
/// the prescribed action is automatically upgraded to `EscalateToOperator`.
#[derive(Default)]
pub struct FailureHistory {
    /// `actor → (failure_type → count)` map.
    counts: HashMap<ClientId, HashMap<FailureType, u32>>,
}

impl FailureHistory {
    /// Create an empty `FailureHistory`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a failure and return the appropriate `RecoveryAction`.
    ///
    /// The base action comes from `FailureRouter::route`. If the same failure
    /// type has now been seen 3 or more times for this actor, the action is
    /// upgraded to `EscalateToOperator`.
    pub fn record(&mut self, actor: ClientId, failure: &FailureType) -> RecoveryAction {
        let count = self
            .counts
            .entry(actor)
            .or_default()
            .entry(failure.clone())
            .or_insert(0);
        *count += 1;

        if *count >= 3 {
            RecoveryAction::EscalateToOperator
        } else {
            FailureRouter::route(failure)
        }
    }

    /// Return the current failure count for a given actor and failure type.
    pub fn count(&self, actor: ClientId, failure: &FailureType) -> u32 {
        self.counts
            .get(&actor)
            .and_then(|m| m.get(failure))
            .copied()
            .unwrap_or(0)
    }

    /// Reset all failure counts for a specific actor (e.g., after a successful cycle).
    pub fn reset_actor(&mut self, actor: ClientId) {
        self.counts.remove(&actor);
    }

    /// Reset the count for a specific failure type for an actor.
    pub fn reset_failure(&mut self, actor: ClientId, failure: &FailureType) {
        if let Some(m) = self.counts.get_mut(&actor) {
            m.remove(failure);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── FailureRouter routing tests ──────────────────────────────────────────

    #[test]
    fn route_network_timeout_returns_retry() {
        assert_eq!(
            FailureRouter::route(&FailureType::NetworkTimeout),
            RecoveryAction::Retry
        );
    }

    #[test]
    fn route_unreachable_path_returns_retry() {
        assert_eq!(
            FailureRouter::route(&FailureType::UnreachablePath),
            RecoveryAction::Retry
        );
    }

    #[test]
    fn route_overweight_returns_return_home() {
        assert_eq!(
            FailureRouter::route(&FailureType::OverweightInventory),
            RecoveryAction::ReturnHome
        );
    }

    #[test]
    fn route_full_bank_returns_return_home() {
        assert_eq!(
            FailureRouter::route(&FailureType::FullBank),
            RecoveryAction::ReturnHome
        );
    }

    #[test]
    fn route_missing_vendor_returns_abort_cycle() {
        assert_eq!(
            FailureRouter::route(&FailureType::MissingVendor),
            RecoveryAction::AbortCycle
        );
    }

    #[test]
    fn route_client_offline_returns_escalate() {
        assert_eq!(
            FailureRouter::route(&FailureType::ClientOffline),
            RecoveryAction::EscalateToOperator
        );
    }

    // ── FailureHistory escalation tests ─────────────────────────────────────

    #[test]
    fn history_first_failure_uses_router_action() {
        let mut history = FailureHistory::new();
        let action = history.record(1, &FailureType::NetworkTimeout);
        assert_eq!(action, RecoveryAction::Retry);
    }

    #[test]
    fn history_second_failure_still_uses_router_action() {
        let mut history = FailureHistory::new();
        history.record(1, &FailureType::NetworkTimeout);
        let action = history.record(1, &FailureType::NetworkTimeout);
        assert_eq!(action, RecoveryAction::Retry);
    }

    #[test]
    fn history_third_failure_escalates() {
        let mut history = FailureHistory::new();
        history.record(1, &FailureType::NetworkTimeout);
        history.record(1, &FailureType::NetworkTimeout);
        let action = history.record(1, &FailureType::NetworkTimeout);
        assert_eq!(action, RecoveryAction::EscalateToOperator);
    }

    #[test]
    fn history_escalation_applies_to_any_base_action() {
        let mut history = FailureHistory::new();
        // OverweightInventory normally maps to ReturnHome
        history.record(2, &FailureType::OverweightInventory);
        history.record(2, &FailureType::OverweightInventory);
        let action = history.record(2, &FailureType::OverweightInventory);
        assert_eq!(action, RecoveryAction::EscalateToOperator);
    }

    #[test]
    fn history_different_actors_tracked_independently() {
        let mut history = FailureHistory::new();
        // Actor 1 hits 3 failures
        history.record(1, &FailureType::MissingVendor);
        history.record(1, &FailureType::MissingVendor);
        history.record(1, &FailureType::MissingVendor);
        // Actor 2 has only 1 — should not be escalated
        let action = history.record(2, &FailureType::MissingVendor);
        assert_eq!(action, RecoveryAction::AbortCycle);
    }

    #[test]
    fn history_different_failure_types_tracked_independently() {
        let mut history = FailureHistory::new();
        // Two NetworkTimeout failures for actor 1
        history.record(1, &FailureType::NetworkTimeout);
        history.record(1, &FailureType::NetworkTimeout);
        // A different failure type should not be escalated yet
        let action = history.record(1, &FailureType::FullBank);
        assert_eq!(action, RecoveryAction::ReturnHome);
    }

    #[test]
    fn history_count_reflects_recorded_failures() {
        let mut history = FailureHistory::new();
        assert_eq!(history.count(1, &FailureType::ClientOffline), 0);
        history.record(1, &FailureType::ClientOffline);
        history.record(1, &FailureType::ClientOffline);
        assert_eq!(history.count(1, &FailureType::ClientOffline), 2);
    }

    #[test]
    fn history_reset_actor_clears_all_counts() {
        let mut history = FailureHistory::new();
        history.record(1, &FailureType::NetworkTimeout);
        history.record(1, &FailureType::NetworkTimeout);
        history.reset_actor(1);
        assert_eq!(history.count(1, &FailureType::NetworkTimeout), 0);
        // After reset, next failure should use normal routing again
        let action = history.record(1, &FailureType::NetworkTimeout);
        assert_eq!(action, RecoveryAction::Retry);
    }

    #[test]
    fn history_reset_specific_failure_preserves_others() {
        let mut history = FailureHistory::new();
        history.record(1, &FailureType::NetworkTimeout);
        history.record(1, &FailureType::FullBank);
        history.reset_failure(1, &FailureType::NetworkTimeout);
        assert_eq!(history.count(1, &FailureType::NetworkTimeout), 0);
        assert_eq!(history.count(1, &FailureType::FullBank), 1);
    }

    // ── FailureState construction ────────────────────────────────────────────

    #[test]
    fn failure_state_new_sets_attempt_count_to_one() {
        let fs = FailureState::new(
            FailureType::UnreachablePath,
            42,
            1_700_000_000,
            RecoveryAction::Retry,
        );
        assert_eq!(fs.attempt_count, 1);
        assert_eq!(fs.actor, 42);
        assert_eq!(fs.failure_type, FailureType::UnreachablePath);
        assert_eq!(fs.recovery_action, RecoveryAction::Retry);
    }
}
