//! Tracks unattended death handling for auto-camp and delayed relog.

use std::collections::HashMap;

/// Per-character unattended death settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoCampOnDeathSettings {
    pub enabled: bool,
    pub camp_delay_secs: u64,
    pub relog_wait_secs: u64,
}

impl Default for AutoCampOnDeathSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            camp_delay_secs: 30,
            relog_wait_secs: 900,
        }
    }
}

/// Scheduler output emitted when a death transition needs action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeathCampAction {
    SendAlert {
        character_name: String,
        camp_delay_secs: u64,
        relog_wait_secs: u64,
    },
    TriggerRelog {
        character_name: String,
        relog_wait_secs: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeathState {
    first_seen_at_secs: u64,
    alert_sent: bool,
    relog_handled: bool,
}

/// Tracks pending death-camp transitions across clients.
#[derive(Debug, Default)]
pub struct DeathCampTracker {
    states: HashMap<u32, DeathState>,
}

impl DeathCampTracker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe one client's latest death state and return any actions due.
    #[must_use]
    pub fn observe(
        &mut self,
        client_id: u32,
        character_name: &str,
        is_dead: bool,
        settings: &AutoCampOnDeathSettings,
        now_secs: u64,
    ) -> Vec<DeathCampAction> {
        if !settings.enabled {
            self.states.remove(&client_id);
            return Vec::new();
        }

        if !is_dead {
            self.states.remove(&client_id);
            return Vec::new();
        }

        let state = self.states.entry(client_id).or_insert(DeathState {
            first_seen_at_secs: now_secs,
            alert_sent: false,
            relog_handled: false,
        });

        let mut actions = Vec::new();
        if !state.alert_sent {
            state.alert_sent = true;
            actions.push(DeathCampAction::SendAlert {
                character_name: character_name.to_string(),
                camp_delay_secs: settings.camp_delay_secs,
                relog_wait_secs: settings.relog_wait_secs,
            });
        }

        let elapsed_secs = now_secs.saturating_sub(state.first_seen_at_secs);
        if !state.relog_handled && elapsed_secs >= settings.camp_delay_secs {
            actions.push(DeathCampAction::TriggerRelog {
                character_name: character_name.to_string(),
                relog_wait_secs: settings.relog_wait_secs,
            });
        }

        actions
    }

    pub fn mark_relog_handled(&mut self, client_id: u32) {
        if let Some(state) = self.states.get_mut(&client_id) {
            state.relog_handled = true;
        }
    }

    pub fn retain_clients(&mut self, active_client_ids: &std::collections::HashSet<u32>) {
        self.states
            .retain(|client_id, _| active_client_ids.contains(client_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_settings() -> AutoCampOnDeathSettings {
        AutoCampOnDeathSettings {
            enabled: true,
            camp_delay_secs: 30,
            relog_wait_secs: 900,
        }
    }

    #[test]
    fn detects_death_within_one_pulse_and_alerts_once() {
        let mut tracker = DeathCampTracker::new();
        let settings = enabled_settings();

        let actions = tracker.observe(7, "Aelrindel", true, &settings, 100);
        assert_eq!(
            actions,
            vec![DeathCampAction::SendAlert {
                character_name: "Aelrindel".into(),
                camp_delay_secs: 30,
                relog_wait_secs: 900,
            }]
        );

        let actions = tracker.observe(7, "Aelrindel", true, &settings, 101);
        assert!(actions.is_empty(), "alert should only fire once per death");
    }

    #[test]
    fn triggers_relog_once_after_camp_delay_expires() {
        let mut tracker = DeathCampTracker::new();
        let settings = enabled_settings();

        let _ = tracker.observe(7, "Aelrindel", true, &settings, 0);
        assert!(
            tracker
                .observe(7, "Aelrindel", true, &settings, 29)
                .is_empty()
        );
        assert_eq!(
            tracker.observe(7, "Aelrindel", true, &settings, 30),
            vec![DeathCampAction::TriggerRelog {
                character_name: "Aelrindel".into(),
                relog_wait_secs: 900,
            }]
        );
        tracker.mark_relog_handled(7);
        assert!(
            tracker
                .observe(7, "Aelrindel", true, &settings, 31)
                .is_empty(),
            "relog trigger should not repeat for the same death"
        );
    }

    #[test]
    fn retries_relog_until_it_is_marked_handled() {
        let mut tracker = DeathCampTracker::new();
        let settings = enabled_settings();

        let _ = tracker.observe(7, "Aelrindel", true, &settings, 0);
        assert_eq!(
            tracker.observe(7, "Aelrindel", true, &settings, 30),
            vec![DeathCampAction::TriggerRelog {
                character_name: "Aelrindel".into(),
                relog_wait_secs: 900,
            }]
        );
        assert_eq!(
            tracker.observe(7, "Aelrindel", true, &settings, 31),
            vec![DeathCampAction::TriggerRelog {
                character_name: "Aelrindel".into(),
                relog_wait_secs: 900,
            }]
        );
    }

    #[test]
    fn revival_clears_pending_death_timer() {
        let mut tracker = DeathCampTracker::new();
        let settings = enabled_settings();

        let _ = tracker.observe(7, "Aelrindel", true, &settings, 0);
        assert!(
            tracker
                .observe(7, "Aelrindel", false, &settings, 10)
                .is_empty()
        );
        let actions = tracker.observe(7, "Aelrindel", true, &settings, 11);
        assert_eq!(
            actions,
            vec![DeathCampAction::SendAlert {
                character_name: "Aelrindel".into(),
                camp_delay_secs: 30,
                relog_wait_secs: 900,
            }]
        );
        assert!(
            tracker
                .observe(7, "Aelrindel", true, &settings, 40)
                .is_empty(),
            "new death should use a fresh timer after revival"
        );
    }

    #[test]
    fn disabled_setting_never_emits_actions() {
        let mut tracker = DeathCampTracker::new();
        let settings = AutoCampOnDeathSettings {
            enabled: false,
            camp_delay_secs: 30,
            relog_wait_secs: 900,
        };

        assert!(
            tracker
                .observe(7, "Aelrindel", true, &settings, 0)
                .is_empty()
        );
        assert!(
            tracker
                .observe(7, "Aelrindel", true, &settings, 30)
                .is_empty()
        );
    }
}
