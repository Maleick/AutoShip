//! Sound alert system — configurable audio event triggers for fleet
//! notifications.

use serde::{Deserialize, Serialize};

// ─── Trigger ───────────────────────────────────────────────────────────────

/// A single sound alert trigger that matches game events by pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundTrigger {
    /// Human-readable name for this trigger (e.g. "Low HP Warning").
    pub name: String,
    /// Pattern to match against event strings (substring match).
    pub event_pattern: String,
    /// Optional path to a sound file to play when triggered.
    pub sound_file: Option<String>,
    /// Whether this trigger is active.
    pub enabled: bool,
    /// Priority level (0 = lowest, 255 = highest). Higher priority triggers
    /// should pre-empt lower ones when multiple fire simultaneously.
    pub priority: u8,
}

impl SoundTrigger {
    /// Creates a new enabled trigger with the given name and pattern.
    #[must_use]
    pub fn new(name: impl Into<String>, event_pattern: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            event_pattern: event_pattern.into(),
            sound_file: None,
            enabled: true,
            priority: 0,
        }
    }

    /// Sets the sound file path for this trigger.
    #[must_use]
    pub fn with_sound_file(mut self, path: impl Into<String>) -> Self {
        self.sound_file = Some(path.into());
        self
    }

    /// Sets the priority for this trigger.
    #[must_use]
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Sets the enabled state for this trigger.
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Whether this trigger matches the given event string (case-insensitive
    /// substring).
    #[must_use]
    pub fn matches(&self, event: &str) -> bool {
        if !self.enabled {
            return false;
        }
        let event_lower = event.to_lowercase();
        let pattern_lower = self.event_pattern.to_lowercase();
        event_lower.contains(&pattern_lower)
    }
}

// ─── Config ────────────────────────────────────────────────────────────────

/// Top-level sound configuration — global enable and volume plus trigger list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundConfig {
    /// Master switch for all sound alerts.
    pub enabled: bool,
    /// Master volume (0.0 = silent, 1.0 = full).
    pub volume: f32,
    /// Registered triggers.
    pub triggers: Vec<SoundTrigger>,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 0.75,
            triggers: Vec::new(),
        }
    }
}

// ─── Manager ───────────────────────────────────────────────────────────────

/// Manages sound alert triggers and matches incoming events against them.
#[derive(Debug, Clone, Default)]
pub struct SoundAlertManager {
    config: SoundConfig,
}

impl SoundAlertManager {
    /// Creates a new manager with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a manager from an existing configuration.
    #[must_use]
    pub fn from_config(config: SoundConfig) -> Self {
        Self { config }
    }

    /// Returns a reference to the current configuration.
    #[must_use]
    pub fn config(&self) -> &SoundConfig {
        &self.config
    }

    /// Adds a trigger to the manager.
    pub fn add_trigger(&mut self, trigger: SoundTrigger) {
        self.config.triggers.push(trigger);
    }

    /// Removes the first trigger with the given name. Returns `true` if found.
    pub fn remove_trigger(&mut self, name: &str) -> bool {
        let before = self.config.triggers.len();
        self.config.triggers.retain(|t| t.name != name);
        self.config.triggers.len() < before
    }

    /// Returns all enabled triggers.
    #[must_use]
    pub fn enabled_triggers(&self) -> Vec<&SoundTrigger> {
        self.config.triggers.iter().filter(|t| t.enabled).collect()
    }

    /// Checks an event string against all triggers. Returns matching triggers
    /// sorted by priority descending (highest priority first).
    ///
    /// Returns an empty vec if the manager is globally disabled.
    #[must_use]
    pub fn check_event(&self, event: &str) -> Vec<&SoundTrigger> {
        if !self.config.enabled {
            return Vec::new();
        }
        let mut matches: Vec<&SoundTrigger> = self
            .config
            .triggers
            .iter()
            .filter(|t| t.matches(event))
            .collect();
        matches.sort_by_key(|trigger| std::cmp::Reverse(trigger.priority));
        matches
    }

    /// Sets the global enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// Sets the master volume, clamped to 0.0..=1.0.
    pub fn set_volume(&mut self, volume: f32) {
        self.config.volume = volume.clamp(0.0, 1.0);
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_matches_substring() {
        let trigger = SoundTrigger::new("HP Low", "hp_low");
        assert!(trigger.matches("player_hp_low_warning"));
        assert!(!trigger.matches("mana_low"));
    }

    #[test]
    fn trigger_match_is_case_insensitive() {
        let trigger = SoundTrigger::new("Ding", "LEVEL_UP");
        assert!(trigger.matches("level_up"));
        assert!(trigger.matches("Level_Up"));
        assert!(trigger.matches("LEVEL_UP"));
    }

    #[test]
    fn disabled_trigger_never_matches() {
        let trigger = SoundTrigger::new("Disabled", "spawn").with_enabled(false);
        assert!(!trigger.matches("new_spawn_detected"));
    }

    #[test]
    fn check_event_returns_matches_sorted_by_priority() {
        let mut mgr = SoundAlertManager::new();
        mgr.add_trigger(SoundTrigger::new("Low", "alert").with_priority(10));
        mgr.add_trigger(SoundTrigger::new("High", "alert").with_priority(50));
        mgr.add_trigger(SoundTrigger::new("Med", "alert").with_priority(25));

        let matches = mgr.check_event("alert_fired");
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].name, "High");
        assert_eq!(matches[1].name, "Med");
        assert_eq!(matches[2].name, "Low");
    }

    #[test]
    fn check_event_skips_non_matching() {
        let mut mgr = SoundAlertManager::new();
        mgr.add_trigger(SoundTrigger::new("HP", "hp_low"));
        mgr.add_trigger(SoundTrigger::new("Mana", "mana_low"));

        let matches = mgr.check_event("hp_low_critical");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "HP");
    }

    #[test]
    fn globally_disabled_returns_empty() {
        let mut mgr = SoundAlertManager::new();
        mgr.add_trigger(SoundTrigger::new("Test", "test"));
        mgr.set_enabled(false);

        assert!(mgr.check_event("test_event").is_empty());
    }

    #[test]
    fn add_and_remove_trigger() {
        let mut mgr = SoundAlertManager::new();
        mgr.add_trigger(SoundTrigger::new("Alpha", "alpha"));
        mgr.add_trigger(SoundTrigger::new("Beta", "beta"));

        assert!(mgr.remove_trigger("Alpha"));
        assert!(!mgr.remove_trigger("Alpha")); // already removed
        assert_eq!(mgr.config().triggers.len(), 1);
        assert_eq!(mgr.config().triggers[0].name, "Beta");
    }

    #[test]
    fn enabled_triggers_filters_disabled() {
        let mut mgr = SoundAlertManager::new();
        mgr.add_trigger(SoundTrigger::new("On", "on"));
        mgr.add_trigger(SoundTrigger::new("Off", "off").with_enabled(false));
        mgr.add_trigger(SoundTrigger::new("Also On", "also"));

        let enabled = mgr.enabled_triggers();
        assert_eq!(enabled.len(), 2);
        assert!(enabled.iter().all(|t| t.enabled));
    }

    #[test]
    fn default_config_values() {
        let config = SoundConfig::default();
        assert!(config.enabled);
        assert!((config.volume - 0.75).abs() < f32::EPSILON);
        assert!(config.triggers.is_empty());
    }

    #[test]
    fn volume_clamped() {
        let mut mgr = SoundAlertManager::new();
        mgr.set_volume(2.0);
        assert!((mgr.config().volume - 1.0).abs() < f32::EPSILON);

        mgr.set_volume(-0.5);
        assert!(mgr.config().volume.abs() < f32::EPSILON);
    }

    #[test]
    fn trigger_builder_methods() {
        let trigger = SoundTrigger::new("Raid", "raid_target")
            .with_sound_file("/sounds/raid.wav")
            .with_priority(100)
            .with_enabled(true);

        assert_eq!(trigger.name, "Raid");
        assert_eq!(trigger.event_pattern, "raid_target");
        assert_eq!(trigger.sound_file.as_deref(), Some("/sounds/raid.wav"));
        assert_eq!(trigger.priority, 100);
        assert!(trigger.enabled);
    }

    #[test]
    fn from_config_roundtrip() {
        let config = SoundConfig {
            enabled: false,
            volume: 0.5,
            triggers: vec![SoundTrigger::new("Test", "test").with_priority(42)],
        };
        let mgr = SoundAlertManager::from_config(config);
        assert!(!mgr.config().enabled);
        assert!((mgr.config().volume - 0.5).abs() < f32::EPSILON);
        assert_eq!(mgr.config().triggers.len(), 1);
        assert_eq!(mgr.config().triggers[0].priority, 42);
    }
}
