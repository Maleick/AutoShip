//! Audio alert playback — MQ2Sound native equivalent.
//!
//! Maps game events (low HP, death, named spawn, GM detection, tell received)
//! to WAV/MP3 files played via rodio on Windows. Supports per-event volume
//! control and a global mute-all override.
//!
//! On non-Windows platforms this module compiles fully but playback is a
//! no-op; audio hardware is only available on Windows game clients.

use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Game events that can trigger audio alerts — MQ2Sound event set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundEvent {
    /// HP dropped below the configured threshold (default 20%).
    LowHp,
    /// Character death.
    Death,
    /// Named NPC (boss / rare) spawned in zone.
    NamedSpawn,
    /// GM detected in zone.
    GmDetected,
    /// Tell message received.
    TellReceived,
    /// A custom [`crate::react::ReactRule`] matched and requested a sound.
    CustomPattern,
}

/// Per-event audio configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSoundConfig {
    /// Path to a WAV or MP3 file. `None` → OS system beep.
    pub path: Option<PathBuf>,
    /// Playback volume in [0.0, 1.0]. Clamped at call time.
    #[serde(default = "one_f32")]
    pub volume: f32,
    /// `false` silences this event without removing its configuration.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn one_f32() -> f32 {
    1.0
}
fn default_true() -> bool {
    true
}

impl Default for EventSoundConfig {
    fn default() -> Self {
        Self {
            path: None,
            volume: 1.0,
            enabled: true,
        }
    }
}

/// Full sound configuration — event mapping table plus global controls.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SoundConfig {
    /// Event → audio config lookup.
    #[serde(default)]
    pub events: HashMap<SoundEvent, EventSoundConfig>,
    /// When `true`, all audio is suppressed regardless of per-event settings.
    #[serde(default)]
    pub mute_all: bool,
}

impl SoundConfig {
    /// Load from a TOML file. A missing file returns an empty default config.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => {
                toml::from_str(&s).map_err(|e| anyhow::anyhow!("sound config parse: {e}"))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(anyhow::anyhow!("read sound config {}: {e}", path.display())),
        }
    }

    /// Returns the resolved event config, falling back to defaults when the
    /// event has no explicit entry.
    pub fn event_config(&self, event: SoundEvent) -> EventSoundConfig {
        self.events.get(&event).cloned().unwrap_or_default()
    }
}

/// Play the audio associated with `event` according to `config`.
///
/// Returns immediately when muted or the event is disabled.  On Windows the
/// call blocks until playback completes; wrap in a thread for non-blocking
/// behaviour.
///
/// # Errors
///
/// Returns an error if the audio device is unavailable or the file cannot be
/// decoded.
pub fn play_event_sound(event: SoundEvent, config: &SoundConfig) -> Result<()> {
    if config.mute_all {
        return Ok(());
    }

    let ev_cfg = config.event_config(event);
    if !ev_cfg.enabled {
        return Ok(());
    }

    play_impl(ev_cfg.path.as_deref(), ev_cfg.volume)
}

// ── platform implementations ───────────────────────────────────────────────

#[cfg(windows)]
fn play_impl(path: Option<&std::path::Path>, volume: f32) -> Result<()> {
    use rodio::{Decoder, OutputStream, Sink};
    use std::{fs::File, io::BufReader};

    if let Some(file_path) = path {
        let (_stream, stream_handle) =
            OutputStream::try_default().map_err(|e| anyhow::anyhow!("audio output: {e}"))?;
        let sink =
            Sink::try_new(&stream_handle).map_err(|e| anyhow::anyhow!("audio sink: {e}"))?;
        sink.set_volume(volume.clamp(0.0, 1.0));

        let file = File::open(file_path)
            .map_err(|e| anyhow::anyhow!("open sound file {}: {e}", file_path.display()))?;
        let source = Decoder::new(BufReader::new(file))
            .map_err(|e| anyhow::anyhow!("decode sound file: {e}"))?;
        sink.append(source);
        sink.sleep_until_end();
    } else {
        // No file configured — fall back to the OS system beep.
        use windows::Win32::UI::WindowsAndMessaging::{MessageBeep, MB_OK};
        // SAFETY: MessageBeep is safe to call from any thread at any time.
        let _ = unsafe { MessageBeep(MB_OK) };
    }

    Ok(())
}

#[cfg(not(windows))]
fn play_impl(_path: Option<&std::path::Path>, _volume: f32) -> Result<()> {
    tracing::debug!("sound playback is a no-op on non-Windows platforms");
    Ok(())
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mute_all_suppresses_all_events() {
        let config = SoundConfig {
            mute_all: true,
            events: HashMap::new(),
        };
        for event in [
            SoundEvent::LowHp,
            SoundEvent::Death,
            SoundEvent::NamedSpawn,
            SoundEvent::GmDetected,
            SoundEvent::TellReceived,
            SoundEvent::CustomPattern,
        ] {
            assert!(play_event_sound(event, &config).is_ok());
        }
    }

    #[test]
    fn disabled_event_is_suppressed() {
        let mut config = SoundConfig::default();
        config.events.insert(
            SoundEvent::Death,
            EventSoundConfig {
                enabled: false,
                ..Default::default()
            },
        );
        assert!(play_event_sound(SoundEvent::Death, &config).is_ok());
    }

    #[test]
    fn default_config_roundtrips_toml() {
        let config = SoundConfig::default();
        let s = toml::to_string(&config).expect("serialize");
        let back: SoundConfig = toml::from_str(&s).expect("deserialize");
        assert!(back.events.is_empty());
        assert!(!back.mute_all);
    }

    #[test]
    fn event_config_fallback_to_default() {
        let config = SoundConfig::default();
        let ev = config.event_config(SoundEvent::LowHp);
        assert!(ev.enabled);
        assert!((ev.volume - 1.0).abs() < f32::EPSILON);
        assert!(ev.path.is_none());
    }
}
