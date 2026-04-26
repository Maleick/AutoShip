//! Shared audio alert configuration and event routing.
//!
//! The registry in this module is intentionally independent from any one UI or
//! runtime loop: producers emit typed alert events, the registry maps them to
//! configured playback requests, and a backend performs the platform-specific
//! audio work.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

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

const BUILTIN_VOICE_COMMAND_COUNT: usize = 22;
const OPERATOR_VOICE_COMMAND_SLOTS: usize = 8;
const DEFAULT_CANCEL_WINDOW_MS: u64 = 1_500;
const DEFAULT_ECHO_PROMPT_MS: u64 = 1_000;
const DEFAULT_VOICE_MODEL: &str = "ggml-base.en-q5_1.bin";
const DEFAULT_PUSH_TO_TALK_HOTKEY: &str = "Ctrl+Alt+Space";

fn default_operator_voice_slots() -> Vec<Option<VoiceOperatorPhrase>> {
    vec![None; OPERATOR_VOICE_COMMAND_SLOTS]
}

fn canonicalize_voice_text(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '\'' {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn humanize_voice_text(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Persisted configuration for opt-in, push-to-talk voice commands.
///
/// The defaults intentionally encode the privacy contract from the issue:
/// disabled by default, local-only processing, push-to-talk only, and no audio
/// persistence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceCommandConfig {
    /// Master switch for the voice command subsystem.
    pub enabled: bool,
    /// Global push-to-talk hotkey that arms capture while held.
    pub push_to_talk_hotkey: String,
    /// Voice commands are always processed locally.
    pub local_only: bool,
    /// Audio is never persisted to disk.
    pub persist_audio: bool,
    /// Whisper model file used by the local transcriber.
    pub whisper_model: PathBuf,
    /// Pending side-effecting commands remain cancellable for this long.
    pub cancel_window_ms: u64,
    /// Echo prompt duration before the command can commit.
    pub echo_prompt_ms: u64,
    /// Operator-defined phrases that extend the fixed 22-command grammar to
    /// the 30-phrase issue target.
    #[serde(default = "default_operator_voice_slots")]
    pub operator_phrases: Vec<Option<VoiceOperatorPhrase>>,
}

impl Default for VoiceCommandConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            push_to_talk_hotkey: DEFAULT_PUSH_TO_TALK_HOTKEY.to_string(),
            local_only: true,
            persist_audio: false,
            whisper_model: PathBuf::from(DEFAULT_VOICE_MODEL),
            cancel_window_ms: DEFAULT_CANCEL_WINDOW_MS,
            echo_prompt_ms: DEFAULT_ECHO_PROMPT_MS,
            operator_phrases: default_operator_voice_slots(),
        }
    }
}

impl VoiceCommandConfig {
    /// Return a normalized copy with exactly eight operator slots.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.operator_phrases.truncate(OPERATOR_VOICE_COMMAND_SLOTS);
        self.operator_phrases
            .resize(OPERATOR_VOICE_COMMAND_SLOTS, None);
        self
    }

    /// Validate the privacy and lifecycle invariants before enabling voice.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.push_to_talk_hotkey.trim().is_empty() {
            return Err(anyhow::anyhow!("voice command hotkey must not be empty"));
        }

        if self.whisper_model.as_os_str().is_empty() {
            return Err(anyhow::anyhow!(
                "voice command model path must not be empty"
            ));
        }

        if self.cancel_window_ms == 0 {
            return Err(anyhow::anyhow!(
                "voice command cancel window must be positive"
            ));
        }

        if self.echo_prompt_ms == 0 {
            return Err(anyhow::anyhow!(
                "voice command echo prompt duration must be positive"
            ));
        }

        if !self.local_only {
            return Err(anyhow::anyhow!("voice commands must remain local-only"));
        }

        if self.persist_audio {
            return Err(anyhow::anyhow!(
                "voice commands must not persist audio to disk"
            ));
        }

        if self.operator_phrases.len() > OPERATOR_VOICE_COMMAND_SLOTS {
            return Err(anyhow::anyhow!(
                "voice command grammar only supports {OPERATOR_VOICE_COMMAND_SLOTS} operator-defined phrases"
            ));
        }

        Ok(())
    }

    /// Cancel window as a [`Duration`].
    #[must_use]
    pub fn cancel_window(&self) -> Duration {
        Duration::from_millis(self.cancel_window_ms.max(1))
    }

    /// Echo duration as a [`Duration`].
    #[must_use]
    pub fn echo_prompt_duration(&self) -> Duration {
        Duration::from_millis(self.echo_prompt_ms.max(1))
    }
}

/// One operator-defined phrase in the 30-phrase grammar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct VoiceOperatorPhrase {
    /// The exact phrase to match after transcript normalization.
    pub phrase: String,
    /// Whether the phrase should go through the echo + cancel path.
    pub side_effecting: bool,
    /// Optional operator-facing label used in confirmations.
    pub label: Option<String>,
}

/// Built-in voice command recognized from a Whisper transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceCommand {
    PauseCharacter {
        character: String,
    },
    ResumeCharacter {
        character: String,
    },
    PauseAll,
    ResumeAll,
    GoToCamp {
        camp: u8,
    },
    Assist {
        character: String,
    },
    MuteAlerts {
        character: String,
    },
    MuteNarrator,
    UnmuteNarrator,
    MuteAllVoices,
    UnmuteAllVoices,
    WhoIsLowOnMana,
    WhoIsLowOnHealth,
    NextPull,
    RepeatLastAlert,
    BookmarkThis,
    TakeScreenshot,
    Stop,
    SeverityPZeroOnly,
    SeverityAll,
    ReadSessionSummary,
    Cancel,
    OperatorDefined {
        slot: usize,
        phrase: String,
        label: Option<String>,
        side_effecting: bool,
    },
}

impl VoiceCommand {
    /// Returns `true` when the command must be shown in the echo+cancel flow.
    #[must_use]
    pub fn requires_confirmation(&self) -> bool {
        match self {
            Self::PauseCharacter { .. }
            | Self::ResumeCharacter { .. }
            | Self::PauseAll
            | Self::ResumeAll
            | Self::GoToCamp { .. }
            | Self::Assist { .. }
            | Self::NextPull
            | Self::Stop => true,
            Self::OperatorDefined { side_effecting, .. } => *side_effecting,
            Self::MuteAlerts { .. }
            | Self::MuteNarrator
            | Self::UnmuteNarrator
            | Self::MuteAllVoices
            | Self::UnmuteAllVoices
            | Self::WhoIsLowOnMana
            | Self::WhoIsLowOnHealth
            | Self::RepeatLastAlert
            | Self::BookmarkThis
            | Self::TakeScreenshot
            | Self::SeverityPZeroOnly
            | Self::SeverityAll
            | Self::ReadSessionSummary
            | Self::Cancel => false,
        }
    }

    /// Human-readable label used for the TTS echo prompt.
    #[must_use]
    pub fn echo_label(&self) -> String {
        match self {
            Self::PauseCharacter { character } => format!("Pausing {character}"),
            Self::ResumeCharacter { character } => format!("Resuming {character}"),
            Self::PauseAll => "Pausing all characters".to_string(),
            Self::ResumeAll => "Resuming all characters".to_string(),
            Self::GoToCamp { camp } => format!("Switching to camp {camp}"),
            Self::Assist { character } => format!("Assisting {character}"),
            Self::MuteAlerts { character } => format!("Muting alerts for {character}"),
            Self::MuteNarrator => "Muting narrator".to_string(),
            Self::UnmuteNarrator => "Unmuting narrator".to_string(),
            Self::MuteAllVoices => "Muting all voices".to_string(),
            Self::UnmuteAllVoices => "Unmuting all voices".to_string(),
            Self::WhoIsLowOnMana => "Reading mana states".to_string(),
            Self::WhoIsLowOnHealth => "Reading health states".to_string(),
            Self::NextPull => "Triggering next pull".to_string(),
            Self::RepeatLastAlert => "Repeating last alert".to_string(),
            Self::BookmarkThis => "Bookmarking this".to_string(),
            Self::TakeScreenshot => "Taking screenshot".to_string(),
            Self::Stop => "Stopping all characters".to_string(),
            Self::SeverityPZeroOnly => "Setting severity floor to P zero only".to_string(),
            Self::SeverityAll => "Setting severity to all".to_string(),
            Self::ReadSessionSummary => "Reading session summary".to_string(),
            Self::Cancel => "Cancelling pending voice command".to_string(),
            Self::OperatorDefined { phrase, label, .. } => {
                label.clone().unwrap_or_else(|| humanize_voice_text(phrase))
            }
        }
    }

    /// Returns `true` for the `cancel` transcript.
    #[must_use]
    pub fn is_cancel(&self) -> bool {
        matches!(self, Self::Cancel)
    }
}

/// Transcript parser and pending-command state for the 30-phrase grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceCommandGrammar {
    operator_phrases: Vec<Option<VoiceOperatorPhrase>>,
}

impl VoiceCommandGrammar {
    /// Build a grammar from a voice command config.
    #[must_use]
    pub fn from_config(config: &VoiceCommandConfig) -> Self {
        Self {
            operator_phrases: config.clone().normalized().operator_phrases,
        }
    }

    /// Return the 30-phrase capacity (22 fixed phrases + 8 operator slots).
    #[must_use]
    pub const fn phrase_capacity() -> usize {
        BUILTIN_VOICE_COMMAND_COUNT + OPERATOR_VOICE_COMMAND_SLOTS
    }

    /// Parse a normalized transcript. Returns `None` when the transcript does
    /// not exactly match one of the constrained grammar phrases.
    #[must_use]
    pub fn parse(&self, transcript: &str) -> Option<VoiceCommand> {
        let normalized = canonicalize_voice_text(transcript);
        if normalized.is_empty() {
            return None;
        }

        let tokens: Vec<&str> = normalized.split_whitespace().collect();
        match tokens.as_slice() {
            ["cancel"] => Some(VoiceCommand::Cancel),
            ["pause", "all"] => Some(VoiceCommand::PauseAll),
            ["resume", "all"] => Some(VoiceCommand::ResumeAll),
            ["mute", "narrator"] => Some(VoiceCommand::MuteNarrator),
            ["unmute", "narrator"] => Some(VoiceCommand::UnmuteNarrator),
            ["mute", "all", "voices"] => Some(VoiceCommand::MuteAllVoices),
            ["unmute", "all", "voices"] => Some(VoiceCommand::UnmuteAllVoices),
            ["who", "is", "low", "on", "mana"] => Some(VoiceCommand::WhoIsLowOnMana),
            ["who", "is", "low", "on", "health"] => Some(VoiceCommand::WhoIsLowOnHealth),
            ["next", "pull"] => Some(VoiceCommand::NextPull),
            ["repeat", "last", "alert"] => Some(VoiceCommand::RepeatLastAlert),
            ["bookmark", "this"] => Some(VoiceCommand::BookmarkThis),
            ["take", "screenshot"] => Some(VoiceCommand::TakeScreenshot),
            ["stop"] => Some(VoiceCommand::Stop),
            ["severity", "p", "zero", "only"] => Some(VoiceCommand::SeverityPZeroOnly),
            ["severity", "all"] => Some(VoiceCommand::SeverityAll),
            ["read", "session", "summary"] => Some(VoiceCommand::ReadSessionSummary),
            ["go", "to", "camp", camp] => camp
                .parse::<u8>()
                .ok()
                .map(|camp| VoiceCommand::GoToCamp { camp }),
            ["pause", rest @ ..] if !rest.is_empty() => Some(VoiceCommand::PauseCharacter {
                character: humanize_voice_text(&rest.join(" ")),
            }),
            ["resume", rest @ ..] if !rest.is_empty() => Some(VoiceCommand::ResumeCharacter {
                character: humanize_voice_text(&rest.join(" ")),
            }),
            ["assist", rest @ ..] if !rest.is_empty() => Some(VoiceCommand::Assist {
                character: humanize_voice_text(&rest.join(" ")),
            }),
            ["mute", "alerts", rest @ ..] if !rest.is_empty() => Some(VoiceCommand::MuteAlerts {
                character: humanize_voice_text(&rest.join(" ")),
            }),
            _ => self.match_operator_phrase(&normalized),
        }
    }

    fn match_operator_phrase(&self, normalized: &str) -> Option<VoiceCommand> {
        self.operator_phrases
            .iter()
            .enumerate()
            .find_map(|(slot, phrase)| {
                let phrase = phrase.as_ref()?;
                if canonicalize_voice_text(&phrase.phrase) == normalized {
                    Some(VoiceCommand::OperatorDefined {
                        slot,
                        phrase: phrase.phrase.clone(),
                        label: phrase.label.clone(),
                        side_effecting: phrase.side_effecting,
                    })
                } else {
                    None
                }
            })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PendingVoiceCommand {
    command: VoiceCommand,
    deadline: Instant,
    echo_prompt: String,
}

/// Outcome of feeding one transcript into the push-to-talk voice pipeline.
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceCommandOutcome {
    Rejected,
    Executed(VoiceCommand),
    Pending {
        command: VoiceCommand,
        cancel_deadline: Instant,
        echo_prompt: String,
        echo_prompt_ms: u64,
    },
    Cancelled,
}

/// Session state for PTT voice commands.
///
/// This keeps all audio handling in-memory and only tracks the transcript,
/// pending confirmation window, and cancel timing. It does not persist audio.
#[derive(Debug, Clone)]
pub struct VoiceCommandSession {
    grammar: VoiceCommandGrammar,
    cancel_window: Duration,
    echo_prompt_ms: u64,
    pending: Option<PendingVoiceCommand>,
}

impl VoiceCommandSession {
    /// Create a session from the persisted voice-command config.
    #[must_use]
    pub fn from_config(config: &VoiceCommandConfig) -> Self {
        Self {
            grammar: VoiceCommandGrammar::from_config(config),
            cancel_window: config.cancel_window(),
            echo_prompt_ms: config.echo_prompt_ms.max(1),
            pending: None,
        }
    }

    /// Feed one transcript into the grammar and pending-command state.
    pub fn submit_transcript(&mut self, transcript: &str, now: Instant) -> VoiceCommandOutcome {
        let Some(command) = self.grammar.parse(transcript) else {
            return VoiceCommandOutcome::Rejected;
        };

        if command.is_cancel() {
            return if self.cancel_pending() {
                VoiceCommandOutcome::Cancelled
            } else {
                VoiceCommandOutcome::Rejected
            };
        }

        if command.requires_confirmation() {
            let echo_prompt = format!("{} - say cancel.", command.echo_label());
            let deadline = now + self.cancel_window;
            self.pending = Some(PendingVoiceCommand {
                command: command.clone(),
                deadline,
                echo_prompt: echo_prompt.clone(),
            });

            return VoiceCommandOutcome::Pending {
                command,
                cancel_deadline: deadline,
                echo_prompt,
                echo_prompt_ms: self.echo_prompt_ms,
            };
        }

        VoiceCommandOutcome::Executed(command)
    }

    /// Cancel the current pending command, if any.
    pub fn cancel_pending(&mut self) -> bool {
        self.pending.take().is_some()
    }

    /// Commit a pending command once its cancel window elapses.
    pub fn tick(&mut self, now: Instant) -> Option<VoiceCommand> {
        let should_fire = self
            .pending
            .as_ref()
            .is_some_and(|pending| now >= pending.deadline);
        if should_fire {
            return self.pending.take().map(|pending| pending.command);
        }

        None
    }

    /// Returns the pending echo prompt when confirmation is active.
    #[must_use]
    pub fn pending_echo_prompt(&self) -> Option<&str> {
        self.pending
            .as_ref()
            .map(|pending| pending.echo_prompt.as_str())
    }
}

#[cfg(test)]
mod voice_command_tests {
    use super::*;

    #[test]
    fn voice_command_config_defaults_are_opt_in_and_local_only() {
        let config = VoiceCommandConfig::default();

        assert!(!config.enabled);
        assert_eq!(config.push_to_talk_hotkey, DEFAULT_PUSH_TO_TALK_HOTKEY);
        assert!(config.local_only);
        assert!(!config.persist_audio);
        assert_eq!(config.cancel_window_ms, DEFAULT_CANCEL_WINDOW_MS);
        assert_eq!(config.echo_prompt_ms, DEFAULT_ECHO_PROMPT_MS);
        assert_eq!(config.operator_phrases.len(), OPERATOR_VOICE_COMMAND_SLOTS);
        assert_eq!(VoiceCommandGrammar::phrase_capacity(), 30);

        config.validate().expect("default config should be valid");
    }

    #[test]
    fn voice_command_grammar_recognizes_constrained_phrases() {
        let mut config = VoiceCommandConfig::default();
        config.operator_phrases[0] = Some(VoiceOperatorPhrase {
            phrase: "call evac".to_string(),
            side_effecting: true,
            label: Some("Emergency evac".to_string()),
        });

        let grammar = VoiceCommandGrammar::from_config(&config);
        assert_eq!(
            grammar.parse("pause Cleric"),
            Some(VoiceCommand::PauseCharacter {
                character: "Cleric".to_string(),
            })
        );
        assert_eq!(
            grammar.parse("go to camp 3"),
            Some(VoiceCommand::GoToCamp { camp: 3 })
        );
        assert_eq!(
            grammar.parse("mute narrator"),
            Some(VoiceCommand::MuteNarrator)
        );
        assert_eq!(
            grammar.parse("call evac"),
            Some(VoiceCommand::OperatorDefined {
                slot: 0,
                phrase: "call evac".to_string(),
                label: Some("Emergency evac".to_string()),
                side_effecting: true,
            })
        );
        assert_eq!(grammar.parse("totally unrelated chatter"), None);
    }

    #[test]
    fn voice_command_session_echoes_and_honors_cancel_window() {
        let config = VoiceCommandConfig::default();
        let mut session = VoiceCommandSession::from_config(&config);
        let now = Instant::now();

        let pending = session.submit_transcript("pause cleric", now);
        let (command, cancel_deadline, echo_prompt_ms) = match pending {
            VoiceCommandOutcome::Pending {
                command,
                cancel_deadline,
                echo_prompt,
                echo_prompt_ms,
            } => {
                assert_eq!(echo_prompt, "Pausing Cleric - say cancel.");
                (command, cancel_deadline, echo_prompt_ms)
            }
            other => panic!("expected pending confirmation, got {other:?}"),
        };

        assert_eq!(
            command,
            VoiceCommand::PauseCharacter {
                character: "Cleric".to_string(),
            }
        );
        assert_eq!(echo_prompt_ms, DEFAULT_ECHO_PROMPT_MS);
        assert!(cancel_deadline > now);
        assert_eq!(
            session.pending_echo_prompt(),
            Some("Pausing Cleric - say cancel.")
        );

        assert_eq!(
            session.submit_transcript("cancel", now + Duration::from_millis(200)),
            VoiceCommandOutcome::Cancelled
        );
        assert!(session.tick(now + Duration::from_secs(2)).is_none());
    }

    #[test]
    fn voice_command_session_executes_after_window_without_cancel() {
        let config = VoiceCommandConfig::default();
        let mut session = VoiceCommandSession::from_config(&config);
        let now = Instant::now();

        assert!(matches!(
            session.submit_transcript("stop", now),
            VoiceCommandOutcome::Pending { .. }
        ));

        assert_eq!(
            session.tick(now + Duration::from_secs(2)),
            Some(VoiceCommand::Stop)
        );
    }
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
