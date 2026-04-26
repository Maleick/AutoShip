//! Example integration of AudioEventDispatcher with game loop.
//!
//! This module demonstrates how to wire the audio dispatcher into
//! the main game loop to respond to FleetEvent and CampEvent changes.
//!
//! USAGE:
//! 1. Add audio_dispatcher to App struct
//! 2. Call dispatcher.dispatch_fleet_event() when metrics events fire
//! 3. Call dispatcher.dispatch_camp_event() during combat loop updates
//! 4. Handle settings changes to enable/disable/volume control

#![allow(dead_code)]

use crate::metrics::events::FleetEvent;
use crate::combat::camp_loop::CampEvent;
use super::audio_dispatcher::AudioEventDispatcher;

/// Example integration points for audio dispatch.
pub struct AudioIntegrationExample {
    dispatcher: AudioEventDispatcher,
}

impl AudioIntegrationExample {
    /// Initialize audio dispatcher at app startup.
    pub fn init() -> Self {
        let dispatcher = AudioEventDispatcher::new();
        tracing::info!("Audio dispatcher initialized");
        Self { dispatcher }
    }

    /// Example: Handle metric event from IPC pipeline.
    /// Call this when FleetEvent is received from metrics engine.
    pub fn on_fleet_event(&self, event: &FleetEvent) {
        let triggered = self.dispatcher.dispatch_fleet_event(event);
        if triggered {
            tracing::debug!("Audio triggered for event: {:?}", event);
        }
    }

    /// Example: Handle combat loop event.
    /// Call this when CampEvent state machine fires.
    pub fn on_camp_event(&self, event: &CampEvent) {
        let triggered = self.dispatcher.dispatch_camp_event(event);
        if triggered {
            tracing::debug!("Audio triggered for camp event: {:?}", event);
        }
    }

    /// Example: Handle volume slider change.
    pub fn on_volume_changed(&self, volume: f32) {
        self.dispatcher.set_volume(volume);
        tracing::info!("Audio volume set to {:.0}%", volume * 100.0);
    }

    /// Example: Handle audio enable/disable toggle.
    pub fn on_audio_enabled(&self, enabled: bool) {
        self.dispatcher.set_enabled(enabled);
        tracing::info!("Audio {}", if enabled { "enabled" } else { "disabled" });
    }

    /// Example: Load custom alert from user config.
    /// Called when settings are loaded from disk.
    pub fn on_load_custom_triggers(&self, triggers_json: &str) {
        match serde_json::from_str::<Vec<_>>(triggers_json) {
            Ok(triggers) => {
                for trigger in triggers {
                    self.dispatcher.add_trigger(trigger);
                    tracing::debug!("Loaded custom audio trigger: {}", trigger.name);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to parse custom audio triggers: {}", e);
            }
        }
    }

    /// Example metrics integration:
    /// In your metrics/session_aggregator.rs or similar:
    ///
    /// ```ignore
    /// impl Aggregator {
    ///     fn emit_event(&mut self, event: FleetEvent) {
    ///         // ... existing code ...
    ///         self.audio_dispatcher.dispatch_fleet_event(&event);
    ///     }
    /// }
    /// ```
    ///
    /// Example combat loop integration:
    /// In your combat/camp_loop.rs:
    ///
    /// ```ignore
    /// fn handle_camp_event(
    ///     &mut self,
    ///     event: CampEvent,
    ///     audio_dispatcher: &AudioEventDispatcher,
    /// ) {
    ///     // ... existing state machine code ...
    ///     audio_dispatcher.dispatch_camp_event(&event);
    /// }
    /// ```
    ///
    /// Example App struct integration:
    /// ```ignore
    /// pub struct App {
    ///     // ... existing fields ...
    ///     #[serde(skip)]
    ///     audio_dispatcher: AudioEventDispatcher,
    /// }
    ///
    /// impl App {
    ///     pub fn new() -> Self {
    ///         Self {
    ///             // ... other fields ...
    ///             audio_dispatcher: AudioEventDispatcher::new(),
    ///         }
    ///     }
    /// }
    /// ```
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::events::FleetEvent;

    #[test]
    fn integration_example_initializes() {
        let _example = AudioIntegrationExample::init();
    }

    #[test]
    fn on_fleet_event_does_not_panic() {
        let example = AudioIntegrationExample::init();
        let event = FleetEvent::Death {
            pid: 1,
            character_name: "Warrior".to_string(),
            zone: "PoP".to_string(),
            timestamp: 1000,
        };
        example.on_fleet_event(&event);
    }

    #[test]
    fn volume_control_works() {
        let example = AudioIntegrationExample::init();
        example.on_volume_changed(0.5);
        example.on_volume_changed(1.0);
        example.on_volume_changed(0.0);
    }

    #[test]
    fn enable_disable_works() {
        let example = AudioIntegrationExample::init();
        example.on_audio_enabled(false);
        example.on_audio_enabled(true);
    }
}
