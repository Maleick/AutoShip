//! Default alert sound configuration.
//!
//! Provides pre-configured alert sounds for common game events.
//! Sounds are mapped to standard WAV files with sensible defaults for
//! volume and priority.

use super::sound::{SoundAlertManager, SoundTrigger, SoundType};

/// Creates the default alert sound manager with preset triggers.
/// This configuration includes:
/// - Death alerts (high priority)
/// - Loot notifications (medium priority)
/// - Named spawn detection (high priority)
/// - Tell received (medium priority)
/// - Group wipe (critical priority)
pub fn create_default_alert_manager() -> SoundAlertManager {
    let mut manager = SoundAlertManager::new();

    // Critical events — highest priority
    manager.add_trigger(
        SoundTrigger::new("Character Death", "death")
            .with_sound_file("assets/audio/character_death.wav")
            .with_priority(200),
    );

    manager.add_trigger(
        SoundTrigger::new("Group Wipe", "group_wipe")
            .with_sound_file("assets/audio/group_wipe.wav")
            .with_priority(255), // Absolute highest
    );

    // High priority — important notifications
    manager.add_trigger(
        SoundTrigger::new("Named Spawn", "named")
            .with_sound_file("assets/audio/named_spawn.wav")
            .with_priority(180),
    );

    manager.add_trigger(
        SoundTrigger::new("Low Mana", "mana_low")
            .with_sound_file("assets/audio/low_mana.wav")
            .with_priority(150),
    );

    manager.add_trigger(
        SoundTrigger::new("Low HP", "hp_low")
            .with_sound_file("assets/audio/low_hp.wav")
            .with_priority(160),
    );

    // Medium priority — standard notifications
    manager.add_trigger(
        SoundTrigger::new("Loot Drop", "loot")
            .with_sound_file("assets/audio/loot_drop.wav")
            .with_priority(100),
    );

    manager.add_trigger(
        SoundTrigger::new("Tell Received", "tell")
            .with_sound_file("assets/audio/tell.wav")
            .with_priority(120),
    );

    manager.add_trigger(
        SoundTrigger::new("Zone Change", "zone")
            .with_sound_file("assets/audio/zone_change.wav")
            .with_priority(90),
    );

    // Low priority — informational
    manager.add_trigger(
        SoundTrigger::new("Level Up", "level_up")
            .with_sound_file("assets/audio/level_up.wav")
            .with_priority(70),
    );

    manager.add_trigger(
        SoundTrigger::new("Combat Start", "combat")
            .with_sound_file("assets/audio/combat_start.wav")
            .with_priority(80)
            .with_enabled(false), // Disabled by default (too frequent)
    );

    manager
}

/// Creates a minimal alert manager with only critical alerts enabled.
pub fn create_minimal_alert_manager() -> SoundAlertManager {
    let mut manager = SoundAlertManager::new();

    manager.add_trigger(
        SoundTrigger::new("Character Death", "death")
            .with_sound_file("assets/audio/character_death.wav")
            .with_priority(200),
    );

    manager.add_trigger(
        SoundTrigger::new("Group Wipe", "group_wipe")
            .with_sound_file("assets/audio/group_wipe.wav")
            .with_priority(255),
    );

    manager.add_trigger(
        SoundTrigger::new("Named Spawn", "named")
            .with_sound_file("assets/audio/named_spawn.wav")
            .with_priority(180),
    );

    manager
}

/// Test helper — creates a manager with mock sounds (beeps only).
#[cfg(test)]
pub fn create_test_alert_manager() -> SoundAlertManager {
    let mut manager = SoundAlertManager::new();

    manager.add_trigger(
        SoundTrigger::new("Test Death", "death")
            .with_beep()
            .with_priority(200),
    );

    manager.add_trigger(
        SoundTrigger::new("Test Loot", "loot")
            .with_beep()
            .with_priority(100),
    );

    manager
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_manager_has_triggers() {
        let mgr = create_default_alert_manager();
        let triggers = mgr.enabled_triggers();
        assert!(!triggers.is_empty());
    }

    #[test]
    fn default_manager_has_death_trigger() {
        let mgr = create_default_alert_manager();
        let matches = mgr.check_event("death");
        assert!(!matches.is_empty());
    }

    #[test]
    fn minimal_manager_has_fewer_triggers() {
        let default = create_default_alert_manager();
        let minimal = create_minimal_alert_manager();

        let default_count = default.enabled_triggers().len();
        let minimal_count = minimal.enabled_triggers().len();

        assert!(minimal_count < default_count);
    }

    #[test]
    fn triggers_have_proper_priority() {
        let mgr = create_default_alert_manager();
        let matches = mgr.check_event("death group_wipe");
        // Group wipe (255) should come before death (200)
        assert!(
            matches[0].priority > matches.get(1).map(|t| t.priority).unwrap_or(0),
            "Triggers should be sorted by priority descending"
        );
    }
}
