//! Auto-accept handler — MQ2AutoAccept parity.
//!
//! Detects pending EQ prompt types (group invite, trade, task add, DZ add,
//! translocate, anchor) and accepts or declines them based on per-type enable
//! flags and an optional trust list.
//!
//! # Usage
//!
//! ```
//! use textquest::eq::auto_accept::AutoAcceptHandler;
//! use textquest_common::safety_features::AutoAcceptConfig;
//!
//! let mut handler = AutoAcceptHandler::new(AutoAcceptConfig::default());
//!
//! // Check a pending group invite from "Healer"
//! use textquest_common::safety_features::AcceptablePromptType;
//! let decision = handler.evaluate("Healer", AcceptablePromptType::GroupInvite);
//! ```

use std::collections::VecDeque;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use textquest_common::safety_features::{AcceptablePromptType, AutoAcceptConfig};

/// Decision made by the auto-accept handler for a given prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptDecision {
    /// Accept the prompt.
    Accept,
    /// Decline the prompt (only emitted when `require_trust_list` is set and
    /// the sender is not trusted).
    Decline,
    /// Ignore — feature disabled or prompt type disabled; do nothing.
    Ignore,
}

/// Record of an auto-accept decision for telemetry / TUI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoAcceptEvent {
    /// The type of prompt that was evaluated.
    pub prompt_type: AcceptablePromptType,
    /// Name of the player who initiated the prompt.
    pub from_name: String,
    /// Decision reached.
    pub decision: AcceptDecision,
    /// When the decision was made.
    pub timestamp: SystemTime,
}

/// AutoAccept handler — evaluates pending EQ prompts and emits accept/decline
/// decisions.
pub struct AutoAcceptHandler {
    config: AutoAcceptConfig,
    events: VecDeque<AutoAcceptEvent>,
}

impl AutoAcceptHandler {
    /// Create a new handler with the given configuration.
    #[must_use]
    pub fn new(config: AutoAcceptConfig) -> Self {
        Self {
            config,
            events: VecDeque::new(),
        }
    }

    /// Update configuration at runtime.
    pub fn set_config(&mut self, config: AutoAcceptConfig) {
        self.config = config;
    }

    /// Get the current configuration.
    #[must_use]
    pub fn config(&self) -> &AutoAcceptConfig {
        &self.config
    }

    /// Evaluate a pending prompt from `from_name` of `prompt_type`.
    ///
    /// Returns the decision and records an event for the audit trail.
    pub fn evaluate(&mut self, from_name: &str, prompt_type: AcceptablePromptType) -> AcceptDecision {
        if !self.config.enabled {
            return AcceptDecision::Ignore;
        }

        if !self.config.prompt_types.is_enabled(prompt_type) {
            return AcceptDecision::Ignore;
        }

        let trusted = self.config.trust_list.contains(from_name);

        let decision = if self.config.require_trust_list && !trusted {
            AcceptDecision::Decline
        } else {
            AcceptDecision::Accept
        };

        tracing::info!(
            from = %from_name,
            prompt_type = ?prompt_type,
            ?decision,
            trusted,
            "AutoAccept evaluated prompt"
        );

        self.events.push_back(AutoAcceptEvent {
            prompt_type,
            from_name: from_name.to_string(),
            decision,
            timestamp: SystemTime::now(),
        });

        decision
    }

    /// Drain and return all pending decision events.
    #[must_use]
    pub fn pending_events(&mut self) -> Vec<AutoAcceptEvent> {
        self.events.drain(..).collect()
    }

    /// Number of pending events.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.events.len()
    }
}

impl Default for AutoAcceptHandler {
    fn default() -> Self {
        Self::new(AutoAcceptConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::safety_features::{AutoAcceptConfig, AutoAcceptTypeFlags};
    use textquest_common::trust_list::TrustList;

    fn trusted_config() -> AutoAcceptConfig {
        AutoAcceptConfig {
            enabled: true,
            require_trust_list: true,
            trust_list: TrustList::new(vec!["Healer".into(), "Tank".into()]),
            prompt_types: AutoAcceptTypeFlags::default(),
        }
    }

    #[test]
    fn accepts_group_invite_from_trusted() {
        let mut handler = AutoAcceptHandler::new(trusted_config());
        let d = handler.evaluate("healer", AcceptablePromptType::GroupInvite);
        assert_eq!(d, AcceptDecision::Accept);
    }

    #[test]
    fn declines_group_invite_from_stranger() {
        let mut handler = AutoAcceptHandler::new(trusted_config());
        let d = handler.evaluate("Stranger", AcceptablePromptType::GroupInvite);
        assert_eq!(d, AcceptDecision::Decline);
    }

    #[test]
    fn ignores_disabled_prompt_type() {
        let mut handler = AutoAcceptHandler::new(trusted_config());
        // Trade is disabled by default
        let d = handler.evaluate("Healer", AcceptablePromptType::Trade);
        assert_eq!(d, AcceptDecision::Ignore);
    }

    #[test]
    fn ignores_all_when_disabled() {
        let cfg = AutoAcceptConfig {
            enabled: false,
            ..trusted_config()
        };
        let mut handler = AutoAcceptHandler::new(cfg);
        let d = handler.evaluate("Healer", AcceptablePromptType::GroupInvite);
        assert_eq!(d, AcceptDecision::Ignore);
    }

    #[test]
    fn accepts_without_trust_list_requirement() {
        let cfg = AutoAcceptConfig {
            require_trust_list: false,
            ..trusted_config()
        };
        let mut handler = AutoAcceptHandler::new(cfg);
        let d = handler.evaluate("RandomPerson", AcceptablePromptType::GroupInvite);
        assert_eq!(d, AcceptDecision::Accept);
    }

    #[test]
    fn events_recorded_and_drained() {
        let mut handler = AutoAcceptHandler::new(trusted_config());
        handler.evaluate("Healer", AcceptablePromptType::GroupInvite);
        handler.evaluate("Stranger", AcceptablePromptType::TaskAdd);
        assert_eq!(handler.pending_count(), 2);
        let events = handler.pending_events();
        assert_eq!(events.len(), 2);
        assert_eq!(handler.pending_count(), 0);
    }

    #[test]
    fn all_prompt_types_covered() {
        let cfg = AutoAcceptConfig {
            require_trust_list: false,
            prompt_types: AutoAcceptTypeFlags {
                group_invites: true,
                trades: true,
                task_adds: true,
                dz_adds: true,
                translocates: true,
                anchors: true,
            },
            ..trusted_config()
        };
        let mut handler = AutoAcceptHandler::new(cfg);

        let types = [
            AcceptablePromptType::GroupInvite,
            AcceptablePromptType::Trade,
            AcceptablePromptType::TaskAdd,
            AcceptablePromptType::DzAdd,
            AcceptablePromptType::Translocate,
            AcceptablePromptType::Anchor,
        ];
        for pt in types {
            let d = handler.evaluate("Anyone", pt);
            assert_eq!(d, AcceptDecision::Accept, "Expected Accept for {pt:?}");
        }
    }
}
