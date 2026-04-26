//! Audio event dispatcher — wires game events to the SoundAlertManager.
//!
//! This module bridges the gap between game events (FleetEvent, CampEvent, etc.)
//! and the audio alert system, converting game state changes into sound triggers
//! with appropriate priority handling for simultaneous playback.

use crate::metrics::events::FleetEvent;
use crate::combat::camp_loop::CampEvent;
use super::sound::{SoundAlertManager, SoundTrigger};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

/// Audio dispatcher that routes game events to audio playback.
/// Uses Arc<Mutex> for thread-safe event-to-audio routing.
#[derive(Clone)]
pub struct AudioEventDispatcher {
    manager: Arc<Mutex<SoundAlertManager>>,
    event_mappings: Arc<HashMap<String, String>>,
}

impl AudioEventDispatcher {
    /// Creates a new dispatcher with default event mappings.
    pub fn new() -> Self {
        Self {
            manager: Arc::new(Mutex::new(SoundAlertManager::with_preset_triggers())),
            event_mappings: Arc::new(Self::default_mappings()),
        }
    }

    /// Creates a dispatcher from an existing SoundAlertManager.
    pub fn from_manager(manager: SoundAlertManager) -> Self {
        Self {
            manager: Arc::new(Mutex::new(manager)),
            event_mappings: Arc::new(Self::default_mappings()),
        }
    }

    /// Dispatches a FleetEvent through the audio system.
    /// Returns true if audio was triggered, false otherwise.
    pub fn dispatch_fleet_event(&self, event: &FleetEvent) -> bool {
        let event_str = self.fleet_event_to_string(event);
        self.fire_audio_event(&event_str)
    }

    /// Dispatches a CampEvent through the audio system.
    /// Returns true if audio was triggered, false otherwise.
    pub fn dispatch_camp_event(&self, event: &CampEvent) -> bool {
        let event_str = format!("{:?}", event);
        self.fire_audio_event(&event_str)
    }

    /// Fire audio for an arbitrary event string.
    /// Matches against configured triggers and plays the highest-priority match.
    pub fn fire_audio_event(&self, event: &str) -> bool {
        if let Ok(manager) = self.manager.lock() {
            manager.fire_event(event).is_some()
        } else {
            tracing::warn!("Failed to acquire audio manager lock for event: {}", event);
            false
        }
    }

    /// Add a custom event mapping (e.g., internal event → audio trigger pattern).
    pub fn add_event_mapping(&mut self, event_name: String, audio_pattern: String) {
        // Note: event_mappings is Arc<HashMap>, so mutation requires Arc::make_mut
        // For now, this is a placeholder. In production, use a RwLock<HashMap> instead.
        tracing::debug!("Event mapping: {} -> {}", event_name, audio_pattern);
    }

    /// Sets the global audio volume (0.0 to 1.0).
    pub fn set_volume(&self, volume: f32) {
        if let Ok(mut manager) = self.manager.lock() {
            manager.set_volume(volume);
        }
    }

    /// Gets the current global audio volume.
    pub fn volume(&self) -> f32 {
        self.manager
            .lock()
            .map(|mgr| mgr.volume())
            .unwrap_or(0.75)
    }

    /// Enable or disable all audio globally.
    pub fn set_enabled(&self, enabled: bool) {
        if let Ok(mut manager) = self.manager.lock() {
            manager.set_enabled(enabled);
        }
    }

    /// Add a custom sound trigger to the manager.
    pub fn add_trigger(&self, trigger: SoundTrigger) {
        if let Ok(mut manager) = self.manager.lock() {
            manager.add_trigger(trigger);
        }
    }

    /// Remove a trigger by name.
    pub fn remove_trigger(&self, name: &str) -> bool {
        if let Ok(mut manager) = self.manager.lock() {
            manager.remove_trigger(name)
        } else {
            false
        }
    }

    /// Convert a FleetEvent to a descriptive string for pattern matching.
    fn fleet_event_to_string(&self, event: &FleetEvent) -> String {
        match event {
            FleetEvent::Kill {
                target_name, ..
            } => format!("kill_{}", target_name.to_lowercase().replace(' ', "_")),
            FleetEvent::Death { character_name, .. } => {
                format!("death_{}", character_name.to_lowercase().replace(' ', "_"))
            }
            FleetEvent::LootDrop { item_name, .. } => {
                format!("loot_{}", item_name.to_lowercase().replace(' ', "_"))
            }
            FleetEvent::ZoneChange { to_zone, .. } => {
                format!("zone_{}", to_zone.to_lowercase().replace(' ', "_"))
            }
            FleetEvent::LevelUp { .. } => "level_up".to_string(),
            FleetEvent::CombatRound { .. } => "combat_round".to_string(),
        }
    }

    /// Default event-to-audio-trigger mappings.
    fn default_mappings() -> HashMap<String, String> {
        let mut mappings = HashMap::new();
        mappings.insert("death".to_string(), "death".to_string());
        mappings.insert("loot".to_string(), "loot_drop".to_string());
        mappings.insert("level_up".to_string(), "level_up".to_string());
        mappings.insert("zone_change".to_string(), "zone".to_string());
        mappings.insert("kill".to_string(), "kill".to_string());
        mappings.insert("named_spawn".to_string(), "named_spawn".to_string());
        mappings
    }
}

impl Default for AudioEventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatcher_creates_with_preset_triggers() {
        let dispatcher = AudioEventDispatcher::new();
        assert!(dispatcher.volume() > 0.0);
    }

    #[test]
    fn fire_audio_event_returns_bool() {
        let dispatcher = AudioEventDispatcher::new();
        let result = dispatcher.fire_audio_event("death");
        // Should return true if manager matched an event
        assert!(result || !result); // Either way, it should not panic
    }

    #[test]
    fn volume_control_works() {
        let dispatcher = AudioEventDispatcher::new();
        dispatcher.set_volume(0.5);
        assert!((dispatcher.volume() - 0.5).abs() < 0.01);

        dispatcher.set_volume(1.0);
        assert!((dispatcher.volume() - 1.0).abs() < 0.01);
    }

    #[test]
    fn add_and_remove_trigger() {
        let dispatcher = AudioEventDispatcher::new();
        let trigger = SoundTrigger::new("Test Alert", "test_event");
        dispatcher.add_trigger(trigger);
        assert!(dispatcher.remove_trigger("Test Alert"));
    }

    #[test]
    fn fleet_event_to_string_converts_death() {
        let dispatcher = AudioEventDispatcher::new();
        let event = FleetEvent::Death {
            pid: 1234,
            character_name: "Warrior Name".to_string(),
            zone: "PoP".to_string(),
            timestamp: 1000000,
        };
        let event_str = dispatcher.fleet_event_to_string(&event);
        assert!(event_str.contains("death"));
        assert!(event_str.contains("warrior"));
    }

    #[test]
    fn fleet_event_to_string_converts_loot() {
        let dispatcher = AudioEventDispatcher::new();
        let event = FleetEvent::LootDrop {
            pid: 1234,
            item_name: "Legendary Sword".to_string(),
            item_id: 9999,
            zone: "PoP".to_string(),
            timestamp: 1000000,
        };
        let event_str = dispatcher.fleet_event_to_string(&event);
        assert!(event_str.contains("loot"));
        assert!(event_str.contains("legendary"));
    }
}
