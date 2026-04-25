//! Rez handler — MQ2Rez parity.
//!
//! Evaluates incoming resurrection offer events against per-character
//! `AutoRezConfig` rules: minimum XP-loss percentage, trusted caster list,
//! and an optional manual-override delay window before accepting.
//!
//! Config lives in `textquest_common::ipc::AutoRezConfig` and is stored on
//! `CharacterConfig::auto_rez`. This module provides the decision logic.
//!
//! # rgmercs compatibility note
//!
//! rgmercs requires a rez handler to be active. This module satisfies that
//! requirement — instantiate a `RezHandler` per character and pass rez offer
//! events through `evaluate()`.

use std::collections::VecDeque;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use textquest_common::ipc::AutoRezConfig;

/// A rez offer received from a caster.
#[derive(Debug, Clone)]
pub struct RezOffer {
    /// Name of the caster offering the rez.
    pub caster_name: String,
    /// Experience percentage that would be restored (0–100).
    /// Some rez spells restore less than 96 % XP.
    pub xp_restore_pct: u8,
    /// Spell name if available (for logging).
    pub spell_name: Option<String>,
    /// When the offer was received.
    pub received_at: SystemTime,
}

impl RezOffer {
    /// Construct a rez offer.
    #[must_use]
    pub fn new(caster_name: String, xp_restore_pct: u8, spell_name: Option<String>) -> Self {
        Self {
            caster_name,
            xp_restore_pct,
            spell_name,
            received_at: SystemTime::now(),
        }
    }
}

/// Decision made by the rez handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RezDecision {
    /// Accept the rez offer.
    Accept,
    /// Decline the rez offer.
    Decline,
    /// Handler is disabled; do nothing.
    Ignore,
}

/// Rez offer evaluation event for telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RezEvent {
    pub caster_name: String,
    pub xp_restore_pct: u8,
    pub decision: RezDecision,
    pub reason: RezDeclineReason,
    pub timestamp: SystemTime,
}

/// Why a rez offer was declined (or N/A when accepted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RezDeclineReason {
    /// Offer was accepted — no decline reason.
    Accepted,
    /// Handler disabled.
    Disabled,
    /// Caster is not on the trusted casters list.
    UntrustedCaster,
    /// XP restore percentage is below the configured minimum.
    BelowMinXp,
}

/// Pending rez offer being held during the manual-override delay window.
struct PendingOffer {
    offer: RezOffer,
    accept_after: SystemTime,
}

/// Rez handler for a single character.
///
/// Call `on_rez_offer()` when a rez window appears, then `tick()` each pulse.
/// Drain `pending_events()` to process accept/decline commands.
pub struct RezHandler {
    config: AutoRezConfig,
    pending: Option<PendingOffer>,
    events: VecDeque<RezEvent>,
}

impl RezHandler {
    /// Create a new handler with the given configuration.
    #[must_use]
    pub fn new(config: AutoRezConfig) -> Self {
        Self {
            config,
            pending: None,
            events: VecDeque::new(),
        }
    }

    /// Update configuration at runtime.
    pub fn set_config(&mut self, config: AutoRezConfig) {
        self.config = config;
    }

    /// Current configuration.
    #[must_use]
    pub fn config(&self) -> &AutoRezConfig {
        &self.config
    }

    /// Notify the handler of an incoming rez offer.
    ///
    /// Performs policy checks immediately. If the offer passes, queues it for
    /// acceptance after `delay_ms`. If it fails, emits a decline event now.
    pub fn on_rez_offer(&mut self, offer: RezOffer) {
        if !self.config.enabled {
            self.emit(
                &offer.caster_name,
                offer.xp_restore_pct,
                RezDecision::Ignore,
                RezDeclineReason::Disabled,
            );
            return;
        }

        // XP check
        if offer.xp_restore_pct < self.config.min_xp_pct {
            tracing::info!(
                caster = %offer.caster_name,
                xp_pct = offer.xp_restore_pct,
                min_xp_pct = self.config.min_xp_pct,
                "Rez declined: below min XP threshold"
            );
            if self.config.decline_if_untrusted {
                self.emit(
                    &offer.caster_name,
                    offer.xp_restore_pct,
                    RezDecision::Decline,
                    RezDeclineReason::BelowMinXp,
                );
            }
            return;
        }

        // Trust list check (only when list is non-empty)
        if !self.config.trusted_casters.is_empty() {
            let trusted = self.config.trusted_casters.iter().any(|t| {
                t.to_ascii_lowercase() == offer.caster_name.to_ascii_lowercase()
            });
            if !trusted {
                tracing::info!(
                    caster = %offer.caster_name,
                    "Rez declined: caster not in trust list"
                );
                if self.config.decline_if_untrusted {
                    self.emit(
                        &offer.caster_name,
                        offer.xp_restore_pct,
                        RezDecision::Decline,
                        RezDeclineReason::UntrustedCaster,
                    );
                }
                return;
            }
        }

        // Offer passes — queue with delay
        tracing::info!(
            caster = %offer.caster_name,
            xp_pct = offer.xp_restore_pct,
            delay_ms = self.config.delay_ms,
            "Rez offer queued for acceptance after delay"
        );
        let accept_after = SystemTime::now()
            + Duration::from_millis(u64::from(self.config.delay_ms));
        self.pending = Some(PendingOffer { offer, accept_after });
    }

    /// Drive the handler. Call once per game pulse.
    ///
    /// Emits an Accept event when the delay window has elapsed.
    pub fn tick(&mut self) {
        let should_accept = self
            .pending
            .as_ref()
            .map(|p| SystemTime::now() >= p.accept_after)
            .unwrap_or(false);

        if should_accept {
            if let Some(pending) = self.pending.take() {
                tracing::info!(
                    caster = %pending.offer.caster_name,
                    xp_pct = pending.offer.xp_restore_pct,
                    "Rez auto-accepted"
                );
                self.emit(
                    &pending.offer.caster_name,
                    pending.offer.xp_restore_pct,
                    RezDecision::Accept,
                    RezDeclineReason::Accepted,
                );
            }
        }
    }

    /// Returns true if a rez offer is currently pending acceptance.
    #[must_use]
    pub fn has_pending_offer(&self) -> bool {
        self.pending.is_some()
    }

    /// Cancel any pending rez offer (e.g. manual operator override).
    pub fn cancel_pending(&mut self) {
        self.pending = None;
    }

    /// Drain and return all pending rez decision events.
    #[must_use]
    pub fn pending_events(&mut self) -> Vec<RezEvent> {
        self.events.drain(..).collect()
    }

    fn emit(
        &mut self,
        caster_name: &str,
        xp_restore_pct: u8,
        decision: RezDecision,
        reason: RezDeclineReason,
    ) {
        self.events.push_back(RezEvent {
            caster_name: caster_name.to_string(),
            xp_restore_pct,
            decision,
            reason,
            timestamp: SystemTime::now(),
        });
    }
}

impl Default for RezHandler {
    fn default() -> Self {
        Self::new(AutoRezConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::ipc::AutoRezConfig;

    fn enabled_config() -> AutoRezConfig {
        AutoRezConfig {
            enabled: true,
            min_xp_pct: 90,
            trusted_casters: Vec::new(),
            decline_if_untrusted: true,
            delay_ms: 0,
        }
    }

    #[test]
    fn accepts_rez_above_min_xp_with_empty_trust_list() {
        let mut handler = RezHandler::new(enabled_config());
        handler.on_rez_offer(RezOffer::new("Cleric".into(), 96, None));
        handler.tick();
        let events = handler.pending_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].decision, RezDecision::Accept);
    }

    #[test]
    fn declines_rez_below_min_xp() {
        let mut handler = RezHandler::new(enabled_config());
        handler.on_rez_offer(RezOffer::new("Cleric".into(), 50, None));
        let events = handler.pending_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].decision, RezDecision::Decline);
        assert_eq!(events[0].reason, RezDeclineReason::BelowMinXp);
    }

    #[test]
    fn declines_untrusted_caster_when_list_set() {
        let config = AutoRezConfig {
            trusted_casters: vec!["TrustedCleric".into()],
            ..enabled_config()
        };
        let mut handler = RezHandler::new(config);
        handler.on_rez_offer(RezOffer::new("RandomCleric".into(), 96, None));
        let events = handler.pending_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].reason, RezDeclineReason::UntrustedCaster);
    }

    #[test]
    fn accepts_trusted_caster() {
        let config = AutoRezConfig {
            trusted_casters: vec!["TrustedCleric".into()],
            ..enabled_config()
        };
        let mut handler = RezHandler::new(config);
        handler.on_rez_offer(RezOffer::new("trustedcleric".into(), 96, None));
        handler.tick();
        let events = handler.pending_events();
        assert_eq!(events[0].decision, RezDecision::Accept);
    }

    #[test]
    fn ignores_when_disabled() {
        let config = AutoRezConfig {
            enabled: false,
            ..enabled_config()
        };
        let mut handler = RezHandler::new(config);
        handler.on_rez_offer(RezOffer::new("Cleric".into(), 96, None));
        let events = handler.pending_events();
        assert_eq!(events[0].decision, RezDecision::Ignore);
    }

    #[test]
    fn cancel_pending_prevents_accept() {
        let mut handler = RezHandler::new(enabled_config());
        handler.on_rez_offer(RezOffer::new("Cleric".into(), 96, None));
        assert!(handler.has_pending_offer());
        handler.cancel_pending();
        handler.tick();
        assert!(handler.pending_events().is_empty());
    }

    #[test]
    fn no_decline_emitted_when_decline_if_untrusted_false() {
        let config = AutoRezConfig {
            trusted_casters: vec!["Other".into()],
            decline_if_untrusted: false,
            ..enabled_config()
        };
        let mut handler = RezHandler::new(config);
        handler.on_rez_offer(RezOffer::new("Unknown".into(), 96, None));
        assert!(handler.pending_events().is_empty());
    }
}
