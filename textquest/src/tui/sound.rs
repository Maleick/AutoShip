//! Sound alert system — configurable audio event triggers for fleet
//! notifications.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

// ─── Event Types ────────────────────────────────────────────────────────────

/// Predefined game event types for common sound alerts.
/// These map to MQ2Sound's built-in alert categories.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameEventType {
    LowHp,
    Death,
    NamedSpawn,
    GmEnter,
    TellReceived,
    #[default]
    Custom,
}

/// Sound playback type for a trigger.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SoundType {
    /// Play a WAV or MP3 file.
    File { path: String },
    /// Play the system beep.
    Beep,
    /// No sound (mute for this trigger).
    #[default]
    None,
}

impl SoundType {
    pub fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }
}

impl Default for &SoundType {
    fn default() -> Self {
        static NONE: SoundType = SoundType::None;
        &NONE
    }
}

// ─── Trigger ───────────────────────────────────────────────────────────────

/// A single sound alert trigger that matches game events by pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundTrigger {
    /// Unique identifier for this trigger.
    pub id: String,
    /// Human-readable name for this trigger (e.g. "Low HP Warning").
    pub name: String,
    /// Pattern to match against event strings (substring match).
    pub event_pattern: String,
    /// The type of sound to play.
    #[serde(default)]
    pub sound: SoundType,
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
        let name = name.into();
        Self {
            id: format!("trigger_{}", name.to_lowercase().replace(' ', "_")),
            name,
            event_pattern: event_pattern.into(),
            sound: SoundType::None,
            enabled: true,
            priority: 0,
        }
    }

    /// Sets a sound file path for this trigger.
    #[must_use]
    pub fn with_sound_file(mut self, path: impl Into<String>) -> Self {
        self.sound = SoundType::File { path: path.into() };
        self
    }

    /// Sets system beep for this trigger.
    #[must_use]
    pub fn with_beep(mut self) -> Self {
        self.sound = SoundType::Beep;
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

    /// Gets the sound file path if this trigger uses a file sound type.
    #[must_use]
    pub fn sound_file(&self) -> Option<&str> {
        match &self.sound {
            SoundType::File { path } => Some(path),
            _ => None,
        }
    }

    /// Whether this trigger matches the given event string (case-insensitive
    /// substring).
    #[must_use]
    pub fn matches(&self, event: &str) -> bool {
        if !self.enabled {
            return false;
        }
        normalize_event_text(event).contains(&normalize_event_text(&self.event_pattern))
    }
}

fn normalize_event_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut last_was_separator = true;

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            normalized.push(' ');
            last_was_separator = true;
        }
    }

    normalized.trim().to_string()
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

impl SoundConfig {
    pub fn with_defaults() -> Self {
        Self::default()
    }

    pub fn with_muted() -> Self {
        Self {
            enabled: false,
            volume: 0.75,
            triggers: Vec::new(),
        }
    }

    pub fn with_preset_triggers() -> Self {
        Self {
            enabled: true,
            volume: 0.75,
            triggers: vec![
                SoundTrigger::new("Low HP", "hp_low")
                    .with_sound_file("sounds/hp_low.wav")
                    .with_priority(100),
                SoundTrigger::new("Death", "you_have_died")
                    .with_sound_file("sounds/death.wav")
                    .with_priority(200),
                SoundTrigger::new("Named Spawn", "named_spawn")
                    .with_beep()
                    .with_priority(150),
                SoundTrigger::new("GM Detected", "gm_detected")
                    .with_sound_file("sounds/gm_alert.wav")
                    .with_priority(255),
                SoundTrigger::new("Tell Received", "tell:")
                    .with_sound_file("sounds/tell.wav")
                    .with_priority(180),
            ],
        }
    }
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

// ─── Sound Player ───────────────────────────────────────────────────────────

#[cfg(windows)]
type AudioOutput = (rodio::OutputStream, rodio::OutputStreamHandle);

struct SoundPlayerInner {
    #[cfg(windows)]
    output: Option<AudioOutput>,
    volume: f32,
}

impl std::fmt::Debug for SoundPlayerInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(windows)]
        let has_output = self.output.is_some();
        #[cfg(not(windows))]
        let has_output = false;
        f.debug_struct("SoundPlayerInner")
            .field("has_output", &has_output)
            .field("volume", &self.volume)
            .finish()
    }
}

impl Default for SoundPlayerInner {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundPlayerInner {
    fn new() -> Self {
        Self {
            #[cfg(windows)]
            output: match rodio::OutputStream::try_default() {
                Ok((stream, handle)) => Some((stream, handle)),
                Err(error) => {
                    tracing::warn!(%error, "No audio output device available");
                    None
                }
            },
            volume: 0.75,
        }
    }

    #[cfg(windows)]
    fn play_file(&self, path: &str, volume: f32) -> Result<(), String> {
        let Some((_stream, stream_handle)) = &self.output else {
            return Err("No audio output available".to_string());
        };

        let path = PathBuf::from(path);
        if !path.exists() {
            return Err(format!("Sound file not found: {}", path.display()));
        }

        let file = rodio::Decoder::new(std::io::BufReader::new(
            std::fs::File::open(&path).map_err(|e| format!("Failed to open file: {e}"))?,
        ))
        .map_err(|e| format!("Failed to decode audio: {e}"))?;

        let sink = rodio::Sink::try_new(stream_handle)
            .map_err(|e| format!("Failed to create sink: {e}"))?;
        sink.set_volume(volume.clamp(0.0, 1.0));
        sink.append(file);
        sink.sleep_until_end();

        Ok(())
    }

    #[cfg(not(windows))]
    fn play_file(&self, path: &str, _volume: f32) -> Result<(), String> {
        Err(format!(
            "Sound file playback is not supported on this platform: {path}"
        ))
    }

    fn play_beep(&self, volume: f32) -> Result<(), String> {
        #[cfg(windows)]
        {
            use std::ptr::null_mut;
            #[link(name = "kernel32")]
            extern "system" {
                fn Beep(dwFreq: u32, dwDuration: u32) -> i32;
            }
            let freq = 800u32;
            let duration = 200u32;
            unsafe {
                Beep(freq, duration);
            }
            Ok(())
        }

        #[cfg(not(windows))]
        {
            use std::process::Command;
            let _ = Command::new("printf").arg(r#"\a"#).output();
            let _ = Command::new("paplay")
                .arg("/usr/share/sounds/ubuntu/stereo/bell.ogg")
                .output()
                .ok();
            Ok(())
        }
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    fn get_volume(&self) -> f32 {
        self.volume
    }
}

/// Thread-safe sound player for playing audio alerts.
#[derive(Debug, Clone, Default)]
pub struct SoundPlayer {
    inner: Rc<RefCell<SoundPlayerInner>>,
}

impl SoundPlayer {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(SoundPlayerInner::new())),
        }
    }

    /// Play a sound file at the given volume.
    pub fn play_file(&self, path: &str) -> Result<(), String> {
        let inner = self
            .inner
            .try_borrow()
            .map_err(|e| format!("Borrow error: {e}"))?;
        let volume = inner.get_volume();
        inner.play_file(path, volume)
    }

    /// Play the system beep at the given volume.
    pub fn play_beep(&self) -> Result<(), String> {
        let inner = self
            .inner
            .try_borrow()
            .map_err(|e| format!("Borrow error: {e}"))?;
        let volume = inner.get_volume();
        inner.play_beep(volume)
    }

    /// Set the master volume (0.0 to 1.0).
    pub fn set_volume(&self, volume: f32) {
        if let Ok(mut inner) = self.inner.try_borrow_mut() {
            inner.set_volume(volume);
        }
    }

    /// Get the current master volume.
    pub fn get_volume(&self) -> f32 {
        self.inner
            .try_borrow()
            .map(|inner| inner.get_volume())
            .unwrap_or(0.75)
    }
}

// ─── Manager ───────────────────────────────────────────────────────────────

/// Manages sound alert triggers and matches incoming events against them.
#[derive(Debug, Clone)]
pub struct SoundAlertManager {
    config: SoundConfig,
    player: SoundPlayer,
}

impl Default for SoundAlertManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundAlertManager {
    /// Creates a new manager with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: SoundConfig::default(),
            player: SoundPlayer::new(),
        }
    }

    /// Creates a manager from an existing configuration.
    #[must_use]
    pub fn from_config(config: SoundConfig) -> Self {
        let mgr = Self {
            config,
            player: SoundPlayer::new(),
        };
        mgr.player.set_volume(mgr.config.volume);
        mgr
    }

    /// Creates a manager with preset triggers for common game events.
    #[must_use]
    pub fn with_preset_triggers() -> Self {
        let config = SoundConfig::with_preset_triggers();
        let mgr = Self {
            config,
            player: SoundPlayer::new(),
        };
        mgr.player.set_volume(mgr.config.volume);
        mgr
    }

    /// Returns a reference to the current configuration.
    #[must_use]
    pub fn config(&self) -> &SoundConfig {
        &self.config
    }

    /// Returns a mutable reference to the configuration.
    pub fn config_mut(&mut self) -> &mut SoundConfig {
        &mut self.config
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

    /// Checks an event and plays sound for the first matching trigger.
    /// Returns the trigger that was fired, if any.
    pub fn fire_event(&self, event: &str) -> Option<&SoundTrigger> {
        let triggers = self.check_event(event);
        if let Some(trigger) = triggers.first() {
            self.play_trigger_sound(trigger);
            Some(trigger)
        } else {
            None
        }
    }

    /// Plays the sound associated with a trigger.
    pub fn play_trigger_sound(&self, trigger: &SoundTrigger) {
        if !self.config.enabled || !trigger.enabled {
            return;
        }

        match &trigger.sound {
            SoundType::File { path } => {
                if let Err(e) = self.player.play_file(path) {
                    tracing::warn!("Failed to play sound '{}': {}", path, e);
                }
            }
            SoundType::Beep => {
                if let Err(e) = self.player.play_beep() {
                    tracing::warn!("Failed to play beep: {}", e);
                }
            }
            SoundType::None => {}
        }
    }

    /// Sets the global enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// Sets the master volume, clamped to 0.0..=1.0.
    pub fn set_volume(&mut self, volume: f32) {
        let vol = volume.clamp(0.0, 1.0);
        self.config.volume = vol;
        self.player.set_volume(vol);
    }

    /// Gets the current master volume.
    #[must_use]
    pub fn volume(&self) -> f32 {
        self.config.volume
    }

    /// Triggers a named alert by looking up the trigger and playing its sound.
    /// Does nothing if the named trigger doesn't exist or is disabled.
    pub fn trigger_named_alert(&mut self, name: &str) {
        if !self.config.enabled {
            return;
        }
        if let Some(trigger) = self.config.triggers.iter_mut().find(|t| t.name == name)
            && trigger.enabled
        {
            tracing::info!(trigger = %name, "Triggering named sound alert");
        }
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
    fn preset_triggers_match_death_event_without_firing_audio() {
        let mgr = SoundAlertManager::with_preset_triggers();
        let matches = mgr.check_event("You have died.");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].name, "Death");
    }

    #[test]
    fn preset_triggers_return_no_match_for_unrelated_event() {
        let mgr = SoundAlertManager::with_preset_triggers();
        let matches = mgr.check_event("unrelated_event");
        assert!(matches.is_empty());
    }

    #[test]
    fn preset_triggers_resume_matching_after_manager_reenabled() {
        let mut mgr = SoundAlertManager::with_preset_triggers();
        mgr.set_enabled(false);
        assert!(mgr.check_event("You have died.").is_empty());

        mgr.set_enabled(true);
        let matches = mgr.check_event("You have died.");
        assert!(matches.is_empty());
        assert!(
            mgr.config()
                .triggers
                .iter()
                .any(|trigger| trigger.name == "Death"),
            "Disabling the manager should not discard preset trigger definitions"
        );
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
        assert_eq!(trigger.sound_file(), Some("/sounds/raid.wav"));
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

    #[test]
    fn preset_triggers_contain_common_events() {
        let mgr = SoundAlertManager::with_preset_triggers();
        let triggers = &mgr.config().triggers;
        let names: Vec<_> = triggers.iter().map(|t| t.name.as_str()).collect();

        assert!(names.contains(&"Low HP"));
        assert!(names.contains(&"Death"));
        assert!(names.contains(&"Named Spawn"));
        assert!(names.contains(&"GM Detected"));
        assert!(names.contains(&"Tell Received"));
    }

    #[test]
    fn sound_type_beep() {
        let trigger = SoundTrigger::new("Beep Test", "test").with_beep();
        assert!(matches!(trigger.sound, SoundType::Beep));
    }
}
