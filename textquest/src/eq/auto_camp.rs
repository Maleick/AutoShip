//! AutoCamp — MQ2AutoCamp parity.
//!
//! On character death, waits a configurable delay then issues `/camp desktop`,
//! waits another delay, and hands off to the existing AutoLogin re-login flow.
//! Optionally sends a Discord alert before camping.
//!
//! # State Machine
//!
//! ```text
//! Idle ──(death)──► PendingCamp ──(delay)──► Camping ──(camp ack)──►
//!    PendingRelogin ──(delay)──► Relogging
//! ```

use std::collections::VecDeque;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use textquest_common::safety_features::{AutoCampConfig, AutoCampState};

/// Events emitted by the AutoCamp state machine for the orchestrator to act on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutoCampEvent {
    /// Death detected; AutoCamp countdown started.
    DeathDetected {
        character_name: String,
        camp_in_secs: u32,
    },
    /// `/camp desktop` command should be issued now.
    IssueCampCommand { character_name: String },
    /// Camp confirmed; re-login countdown started.
    CampConfirmed {
        character_name: String,
        relogin_in_secs: u32,
    },
    /// AutoLogin re-login should be triggered now.
    TriggerRelogin { character_name: String },
    /// Discord alert should be sent with this message.
    DiscordAlert { character_name: String, message: String },
}

/// Internal tracking state with wall-clock timestamp.
#[derive(Debug, Clone)]
struct PhaseTimer {
    entered_at: SystemTime,
    duration: Duration,
}

impl PhaseTimer {
    fn new(duration: Duration) -> Self {
        Self {
            entered_at: SystemTime::now(),
            duration,
        }
    }

    fn elapsed(&self) -> bool {
        self.entered_at
            .elapsed()
            .map(|e| e >= self.duration)
            .unwrap_or(false)
    }
}

/// AutoCamp controller for a single character.
///
/// Call `on_death()` when a character death is detected, then call `tick()`
/// each game pulse. Drain `pending_events()` to process orchestrator actions.
pub struct AutoCampController {
    character_name: String,
    config: AutoCampConfig,
    state: AutoCampState,
    timer: Option<PhaseTimer>,
    events: VecDeque<AutoCampEvent>,
}

impl AutoCampController {
    /// Create a new controller for the named character.
    #[must_use]
    pub fn new(character_name: String, config: AutoCampConfig) -> Self {
        Self {
            character_name,
            config,
            state: AutoCampState::Idle,
            timer: None,
            events: VecDeque::new(),
        }
    }

    /// Update configuration at runtime.
    pub fn set_config(&mut self, config: AutoCampConfig) {
        self.config = config;
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> AutoCampState {
        self.state
    }

    /// Notify the controller that the character has died.
    ///
    /// No-op if AutoCamp is disabled or already in a non-Idle state.
    pub fn on_death(&mut self) {
        if !self.config.enabled || self.state != AutoCampState::Idle {
            return;
        }

        tracing::info!(
            character = %self.character_name,
            camp_delay = self.config.camp_delay_secs,
            "Death detected, AutoCamp countdown started"
        );

        if self.config.discord_alert_enabled {
            self.events.push_back(AutoCampEvent::DiscordAlert {
                character_name: self.character_name.clone(),
                message: format!(
                    "{} died — camping in {}s",
                    self.character_name, self.config.camp_delay_secs
                ),
            });
        }

        self.events.push_back(AutoCampEvent::DeathDetected {
            character_name: self.character_name.clone(),
            camp_in_secs: self.config.camp_delay_secs,
        });

        self.state = AutoCampState::PendingCamp;
        self.timer = Some(PhaseTimer::new(Duration::from_secs(
            u64::from(self.config.camp_delay_secs),
        )));
    }

    /// Notify the controller that the camp command has been acknowledged.
    ///
    /// Call this after the `/camp desktop` command is confirmed sent.
    pub fn on_camp_issued(&mut self) {
        if self.state != AutoCampState::Camping {
            return;
        }

        tracing::info!(
            character = %self.character_name,
            relogin_delay = self.config.relogin_delay_secs,
            "Camp issued, waiting for re-login"
        );

        self.events.push_back(AutoCampEvent::CampConfirmed {
            character_name: self.character_name.clone(),
            relogin_in_secs: self.config.relogin_delay_secs,
        });

        self.state = AutoCampState::PendingRelogin;
        self.timer = Some(PhaseTimer::new(Duration::from_secs(
            u64::from(self.config.relogin_delay_secs),
        )));
    }

    /// Notify the controller that the re-login sequence has completed and the
    /// character is back in game. Resets to Idle.
    pub fn on_relogin_complete(&mut self) {
        tracing::info!(
            character = %self.character_name,
            "Re-login complete, AutoCamp reset to Idle"
        );
        self.state = AutoCampState::Idle;
        self.timer = None;
    }

    /// Drive the state machine. Call once per game pulse.
    ///
    /// Generates events when phase timers expire.
    pub fn tick(&mut self) {
        let timer_elapsed = self.timer.as_ref().map(PhaseTimer::elapsed).unwrap_or(false);

        match self.state {
            AutoCampState::PendingCamp if timer_elapsed => {
                tracing::info!(
                    character = %self.character_name,
                    "AutoCamp delay expired, issuing /camp desktop"
                );
                self.events.push_back(AutoCampEvent::IssueCampCommand {
                    character_name: self.character_name.clone(),
                });
                self.state = AutoCampState::Camping;
                self.timer = None;
            }
            AutoCampState::PendingRelogin if timer_elapsed => {
                tracing::info!(
                    character = %self.character_name,
                    "Re-login delay expired, triggering AutoLogin"
                );
                self.events.push_back(AutoCampEvent::TriggerRelogin {
                    character_name: self.character_name.clone(),
                });
                self.state = AutoCampState::Relogging;
                self.timer = None;
            }
            _ => {}
        }
    }

    /// Drain and return all pending events.
    #[must_use]
    pub fn pending_events(&mut self) -> Vec<AutoCampEvent> {
        self.events.drain(..).collect()
    }

    /// Reset to Idle regardless of current state.
    ///
    /// Use when the character is manually resurrected or revived out-of-band.
    pub fn reset(&mut self) {
        self.state = AutoCampState::Idle;
        self.timer = None;
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::safety_features::AutoCampConfig;

    fn enabled_config() -> AutoCampConfig {
        AutoCampConfig {
            enabled: true,
            camp_delay_secs: 0,
            relogin_delay_secs: 0,
            discord_alert_enabled: false,
            discord_webhook_url: None,
        }
    }

    #[test]
    fn death_transitions_to_pending_camp() {
        let mut ctrl = AutoCampController::new("Mage".into(), enabled_config());
        assert_eq!(ctrl.state(), AutoCampState::Idle);
        ctrl.on_death();
        assert_eq!(ctrl.state(), AutoCampState::PendingCamp);
    }

    #[test]
    fn tick_with_zero_delay_issues_camp_command() {
        let mut ctrl = AutoCampController::new("Mage".into(), enabled_config());
        ctrl.on_death();
        // Drain death event
        ctrl.pending_events();
        // Tick — delay is 0 so timer should fire
        ctrl.tick();
        let events = ctrl.pending_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            AutoCampEvent::IssueCampCommand { .. }
        ));
        assert_eq!(ctrl.state(), AutoCampState::Camping);
    }

    #[test]
    fn on_camp_issued_transitions_to_pending_relogin() {
        let mut ctrl = AutoCampController::new("Mage".into(), enabled_config());
        ctrl.on_death();
        ctrl.tick(); // PendingCamp → Camping
        ctrl.pending_events();
        ctrl.on_camp_issued();
        assert_eq!(ctrl.state(), AutoCampState::PendingRelogin);
    }

    #[test]
    fn relogin_triggered_after_delay() {
        let mut ctrl = AutoCampController::new("Mage".into(), enabled_config());
        ctrl.on_death();
        ctrl.tick();
        ctrl.pending_events();
        ctrl.on_camp_issued();
        ctrl.pending_events();
        ctrl.tick(); // relogin delay 0 → fires
        let events = ctrl.pending_events();
        assert!(matches!(events[0], AutoCampEvent::TriggerRelogin { .. }));
        assert_eq!(ctrl.state(), AutoCampState::Relogging);
    }

    #[test]
    fn disabled_config_ignores_death() {
        let cfg = AutoCampConfig {
            enabled: false,
            ..enabled_config()
        };
        let mut ctrl = AutoCampController::new("Mage".into(), cfg);
        ctrl.on_death();
        assert_eq!(ctrl.state(), AutoCampState::Idle);
        assert_eq!(ctrl.pending_events().len(), 0);
    }

    #[test]
    fn discord_alert_emitted_on_death() {
        let cfg = AutoCampConfig {
            discord_alert_enabled: true,
            ..enabled_config()
        };
        let mut ctrl = AutoCampController::new("Mage".into(), cfg);
        ctrl.on_death();
        let events = ctrl.pending_events();
        // First event is DiscordAlert, second is DeathDetected
        assert!(matches!(events[0], AutoCampEvent::DiscordAlert { .. }));
        assert!(matches!(events[1], AutoCampEvent::DeathDetected { .. }));
    }

    #[test]
    fn reset_clears_state() {
        let mut ctrl = AutoCampController::new("Mage".into(), enabled_config());
        ctrl.on_death();
        ctrl.reset();
        assert_eq!(ctrl.state(), AutoCampState::Idle);
        assert!(ctrl.pending_events().is_empty());
    }

    #[test]
    fn second_death_ignored_while_camping() {
        let mut ctrl = AutoCampController::new("Mage".into(), enabled_config());
        ctrl.on_death();
        let count_before = ctrl.pending_events().len();
        ctrl.on_death(); // should be ignored — already in PendingCamp
        assert_eq!(ctrl.pending_events().len(), 0);
        let _ = count_before;
    }
}
