//! Shared audio alert configuration and event routing.
//!
//! The registry in this module is intentionally independent from any one UI or
//! runtime loop: producers emit typed alert events, the registry maps them to
//! configured playback requests, and a backend performs the platform-specific
//! audio work.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::thread;

/// Built-in audio alert event kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioAlertKind {
    LowMana,
    CharacterDead,
    LootDrop,
    GroupWipe,
    NamedNpcSpawn,
    StuckDetection,
    CampModeChange,
    CharmBreak,
    SlowCast,
    Custom,
}

impl AudioAlertKind {
    /// Return the default bundled sound file for this alert kind.
    #[must_use]
    pub fn default_sound_file(self) -> &'static str {
        match self {
            Self::LowMana => "low_mana.wav",
            Self::CharacterDead => "character_dead.wav",
            Self::LootDrop => "loot_drop.wav",
            Self::GroupWipe => "group_wipe.wav",
            Self::NamedNpcSpawn => "named_npc_spawn.wav",
            Self::StuckDetection => "stuck_detection.wav",
            Self::CampModeChange => "camp_mode_change.wav",
            Self::CharmBreak => "charm_break.wav",
            Self::SlowCast => "slow_cast.wav",
            Self::Custom => "custom_alert.wav",
        }
    }
}

/// Alert priority for dashboards and future channel arbitration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AudioAlertPriority {
    Low,
    #[default]
    Medium,
    High,
}

/// Configured rule for one audio alert binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioAlertRule {
    /// Built-in event kind or `custom` for a named custom event.
    pub event: AudioAlertKind,
    /// Optional custom event name. When omitted for `custom`, all custom events
    /// match this rule.
    pub custom_event: Option<String>,
    /// Whether this alert binding is active.
    pub enabled: bool,
    /// Priority exposed to dashboards and dispatchers.
    pub priority: AudioAlertPriority,
    /// Per-event playback volume, clamped to `0..=100` when routed. Omitted
    /// values use `AudioAlertConfig::default_volume_percent`; explicit `0`
    /// mutes the alert request.
    pub volume_percent: Option<u8>,
    /// Optional percent threshold for events that carry a percent value.
    pub threshold_percent: Option<u8>,
    /// Logical audio channels. Each channel becomes a separate playback
    /// request so backends can play multiple alerts concurrently.
    pub channels: Vec<String>,
    /// Optional custom sound file. Relative paths are resolved below
    /// `AudioAlertConfig::sound_root`.
    pub sound_file: Option<String>,
}

impl AudioAlertRule {
    /// Construct a default rule for a built-in event kind.
    #[must_use]
    pub fn for_event(event: AudioAlertKind) -> Self {
        let priority = match event {
            AudioAlertKind::CharacterDead
            | AudioAlertKind::GroupWipe
            | AudioAlertKind::CharmBreak => AudioAlertPriority::High,
            AudioAlertKind::LootDrop
            | AudioAlertKind::CampModeChange
            | AudioAlertKind::SlowCast => AudioAlertPriority::Low,
            AudioAlertKind::LowMana
            | AudioAlertKind::NamedNpcSpawn
            | AudioAlertKind::StuckDetection
            | AudioAlertKind::Custom => AudioAlertPriority::Medium,
        };

        Self {
            event,
            priority,
            threshold_percent: (event == AudioAlertKind::LowMana).then_some(20),
            ..Self::default()
        }
    }

    fn matches(&self, event: &AudioAlertEvent) -> bool {
        if self.event != event.kind {
            return false;
        }

        if self.event != AudioAlertKind::Custom {
            return true;
        }

        match (&self.custom_event, &event.custom_event) {
            (Some(expected), Some(actual)) => expected.eq_ignore_ascii_case(actual),
            (Some(_), None) => false,
            (None, _) => true,
        }
    }

    fn threshold_allows(&self, event: &AudioAlertEvent) -> bool {
        match (self.threshold_percent, event.value_percent) {
            (Some(threshold), Some(value)) => value <= threshold,
            (Some(_), None) => false,
            (None, _) => true,
        }
    }

    fn normalized_volume(&self, default_volume_percent: u8) -> u8 {
        self.volume_percent
            .unwrap_or(default_volume_percent)
            .min(100)
    }
}

impl Default for AudioAlertRule {
    fn default() -> Self {
        Self {
            event: AudioAlertKind::Custom,
            custom_event: None,
            enabled: true,
            priority: AudioAlertPriority::Medium,
            volume_percent: None,
            threshold_percent: None,
            channels: default_channels(),
            sound_file: None,
        }
    }
}

/// Top-level TOML-backed audio alert configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioAlertConfig {
    /// Master switch for event-triggered audio.
    pub enabled: bool,
    /// Directory containing bundled or user-provided alert sounds.
    pub sound_root: String,
    /// Whether the backend should fall back to a system beep when the sound
    /// file is missing or unavailable.
    pub fallback_beep: bool,
    /// Default event volume when a rule does not specify one.
    pub default_volume_percent: u8,
    /// Event-to-sound routing rules.
    pub rules: Vec<AudioAlertRule>,
}

impl Default for AudioAlertConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sound_root: default_sound_root(),
            fallback_beep: true,
            default_volume_percent: default_volume_percent(),
            rules: default_audio_alert_rules(),
        }
    }
}

/// Runtime alert event emitted by game-state detectors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioAlertEvent {
    pub kind: AudioAlertKind,
    pub custom_event: Option<String>,
    pub value_percent: Option<u8>,
    pub label: Option<String>,
}

impl AudioAlertEvent {
    /// Create a built-in alert event without extra values.
    #[must_use]
    pub fn new(kind: AudioAlertKind) -> Self {
        Self {
            kind,
            custom_event: None,
            value_percent: None,
            label: None,
        }
    }

    /// Create a low-mana alert event with the current mana percentage.
    #[must_use]
    pub fn low_mana(value_percent: u8) -> Self {
        Self {
            kind: AudioAlertKind::LowMana,
            custom_event: None,
            value_percent: Some(value_percent.min(100)),
            label: None,
        }
    }

    /// Create a named custom alert event.
    #[must_use]
    pub fn custom(name: impl Into<String>) -> Self {
        Self {
            kind: AudioAlertKind::Custom,
            custom_event: Some(name.into()),
            value_percent: None,
            label: None,
        }
    }
}

/// Concrete playback request produced by the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioPlaybackRequest {
    pub kind: AudioAlertKind,
    pub custom_event: Option<String>,
    pub priority: AudioAlertPriority,
    pub volume_percent: u8,
    pub channel: String,
    pub sound_path: PathBuf,
    pub fallback_beep: bool,
    pub label: Option<String>,
}

/// Backend contract for platform-specific audio playback.
pub trait AudioAlertBackend: Clone + Send + Sync + 'static {
    /// Play one alert request. Implementations should return promptly.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the platform playback API rejects the request.
    fn play(&self, request: AudioPlaybackRequest) -> std::io::Result<()>;
}

/// Registry that binds typed events to configured audio playback requests.
#[derive(Debug, Clone)]
pub struct AudioAlertRegistry {
    config: AudioAlertConfig,
}

impl AudioAlertRegistry {
    /// Create a registry from TOML-backed configuration.
    #[must_use]
    pub fn new(config: AudioAlertConfig) -> Self {
        Self { config }
    }

    /// Borrow the active audio alert configuration.
    #[must_use]
    pub fn config(&self) -> &AudioAlertConfig {
        &self.config
    }

    /// Route an event into zero or more playback requests.
    #[must_use]
    pub fn route_event(&self, event: &AudioAlertEvent) -> Vec<AudioPlaybackRequest> {
        if !self.config.enabled {
            return Vec::new();
        }

        self.config
            .rules
            .iter()
            .filter(|rule| rule.enabled)
            .filter(|rule| rule.matches(event))
            .filter(|rule| rule.threshold_allows(event))
            .flat_map(|rule| self.requests_for_rule(rule, event))
            .collect()
    }

    /// Dispatch an event on detached worker threads so playback does not block
    /// the game-state loop.
    pub fn dispatch_event<B>(&self, event: &AudioAlertEvent, backend: B) -> usize
    where
        B: AudioAlertBackend,
    {
        let mut spawned = 0;

        for request in self.route_event(event) {
            let worker = backend.clone();
            let name = format!("textquest-audio-alert-{:?}", request.kind);
            match thread::Builder::new().name(name).spawn(move || {
                if let Err(error) = worker.play(request) {
                    tracing::warn!(?error, "audio alert playback failed");
                }
            }) {
                Ok(_handle) => {
                    spawned += 1;
                }
                Err(error) => {
                    tracing::warn!(?error, "failed to spawn audio alert worker");
                }
            }
        }

        spawned
    }

    fn requests_for_rule(
        &self,
        rule: &AudioAlertRule,
        event: &AudioAlertEvent,
    ) -> Vec<AudioPlaybackRequest> {
        let sound_path = self.resolve_sound_path(rule);
        let channels = if rule.channels.is_empty() {
            default_channels()
        } else {
            rule.channels.clone()
        };

        channels
            .into_iter()
            .map(|channel| AudioPlaybackRequest {
                kind: event.kind,
                custom_event: event.custom_event.clone(),
                priority: rule.priority,
                volume_percent: rule.normalized_volume(self.config.default_volume_percent),
                channel,
                sound_path: sound_path.clone(),
                fallback_beep: self.config.fallback_beep,
                label: event.label.clone(),
            })
            .collect()
    }

    fn resolve_sound_path(&self, rule: &AudioAlertRule) -> PathBuf {
        let sound_file = rule
            .sound_file
            .as_deref()
            .unwrap_or_else(|| rule.event.default_sound_file());
        let path = PathBuf::from(sound_file);
        if path.is_absolute() {
            path
        } else {
            PathBuf::from(&self.config.sound_root).join(path)
        }
    }
}

/// Platform audio backend using Windows `PlaySoundW` when available.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemAudioBackend;

impl AudioAlertBackend for SystemAudioBackend {
    fn play(&self, request: AudioPlaybackRequest) -> std::io::Result<()> {
        play_system_audio(&request)
    }
}

#[cfg(windows)]
fn play_system_audio(request: &AudioPlaybackRequest) -> std::io::Result<()> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;

    const SND_ASYNC: u32 = 0x0001;
    const SND_FILENAME: u32 = 0x0002_0000;
    const SND_NODEFAULT: u32 = 0x0002;
    const MB_ICONEXCLAMATION: u32 = 0x0000_0030;

    #[link(name = "winmm")]
    unsafe extern "system" {
        fn PlaySoundW(psz_sound: *const u16, hmod: *mut c_void, fdw_sound: u32) -> i32;
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBeep(beep_type: u32) -> i32;
    }

    if request.sound_path.exists() {
        let wide_path: Vec<u16> = request
            .sound_path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let ok = unsafe {
            PlaySoundW(
                wide_path.as_ptr(),
                null_mut(),
                SND_ASYNC | SND_FILENAME | SND_NODEFAULT,
            )
        };

        if ok != 0 {
            return Ok(());
        }
    }

    if request.fallback_beep {
        unsafe {
            MessageBeep(MB_ICONEXCLAMATION);
        }
    }

    Ok(())
}

#[cfg(not(windows))]
fn play_system_audio(request: &AudioPlaybackRequest) -> std::io::Result<()> {
    tracing::debug!(
        sound_path = %request.sound_path.display(),
        volume_percent = request.volume_percent,
        channel = %request.channel,
        "audio alerts require a platform playback backend on this OS"
    );
    Ok(())
}

fn default_sound_root() -> String {
    "assets/audio/alerts".to_string()
}

fn default_volume_percent() -> u8 {
    80
}

fn default_channels() -> Vec<String> {
    vec!["alerts".to_string()]
}

fn default_audio_alert_rules() -> Vec<AudioAlertRule> {
    [
        AudioAlertKind::LowMana,
        AudioAlertKind::CharacterDead,
        AudioAlertKind::LootDrop,
        AudioAlertKind::GroupWipe,
        AudioAlertKind::NamedNpcSpawn,
        AudioAlertKind::StuckDetection,
        AudioAlertKind::CampModeChange,
        AudioAlertKind::CharmBreak,
        AudioAlertKind::SlowCast,
    ]
    .into_iter()
    .map(AudioAlertRule::for_event)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_low_mana_only_below_configured_threshold() {
        let registry = AudioAlertRegistry::new(AudioAlertConfig {
            enabled: true,
            sound_root: "sounds".to_string(),
            ..AudioAlertConfig::default()
        });

        assert!(
            registry
                .route_event(&AudioAlertEvent::low_mana(35))
                .is_empty()
        );

        let requests = registry.route_event(&AudioAlertEvent::low_mana(15));
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].kind, AudioAlertKind::LowMana);
        assert_eq!(requests[0].volume_percent, 80);
        assert_eq!(requests[0].channel, "alerts");
        assert_eq!(requests[0].sound_path, PathBuf::from("sounds/low_mana.wav"));
    }

    #[test]
    fn custom_alerts_fan_out_channels_and_clamp_volume() {
        let registry = AudioAlertRegistry::new(AudioAlertConfig {
            enabled: true,
            sound_root: "sounds".to_string(),
            rules: vec![AudioAlertRule {
                event: AudioAlertKind::Custom,
                custom_event: Some("raid_pull".to_string()),
                volume_percent: Some(140),
                channels: vec!["main".to_string(), "secondary".to_string()],
                sound_file: Some("custom/raid_pull.flac".to_string()),
                ..AudioAlertRule::default()
            }],
            ..AudioAlertConfig::default()
        });

        let requests = registry.route_event(&AudioAlertEvent::custom("RAID_PULL"));
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|request| request.volume_percent == 100));
        assert_eq!(
            requests[0].sound_path,
            PathBuf::from("sounds/custom/raid_pull.flac")
        );
        assert_eq!(requests[0].channel, "main");
        assert_eq!(requests[1].channel, "secondary");
    }
}
