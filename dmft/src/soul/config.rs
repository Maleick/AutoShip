use dmft_common::soul::{PersonalityTraits, SocialTag, SpeechStyle};
use serde::Deserialize;

/// How "edgy" a character's personality and speech can be.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdginessLevel {
    /// Family-friendly, no swearing or dark humor
    Mild,
    /// Classic EQ vibes — light trash talk, mild insults
    Moderate,
    /// Full classic MMO banter — sarcasm, profanity, dark humor
    Spicy,
}

impl Default for EdginessLevel {
    fn default() -> Self {
        Self::Moderate
    }
}

/// Seed data for a pre-defined relationship between two characters.
#[derive(Debug, Clone, Deserialize)]
pub struct RelationshipSeed {
    /// First character name
    pub from: String,
    /// Second character name
    pub to: String,
    /// Initial faction score (-1000..1000)
    #[serde(default)]
    pub faction: i32,
    /// Relationship tags (friend, rival, mentor, etc.)
    #[serde(default)]
    pub tags: Vec<SocialTag>,
    /// Initial trust level (0.0..1.0)
    #[serde(default = "default_trust")]
    pub trust: f32,
}

fn default_trust() -> f32 {
    0.5
}

/// Per-character soul configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct CharacterSoulConfig {
    /// Character name (must match ToonConfig.name)
    pub name: String,
    /// Override personality traits (merged with defaults)
    #[serde(default)]
    pub traits: PersonalityTraits,
    /// Override speech style
    #[serde(default)]
    pub speech: SpeechStyle,
    /// Per-character edginess override
    pub edginess: Option<EdginessLevel>,
    /// Backstory snippet (used as LLM context seed)
    #[serde(default)]
    pub backstory: String,
    /// Unique quirks or idle behaviors for this character
    #[serde(default)]
    pub quirks: Vec<String>,
}

/// Top-level Soul Engine configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SoulConfig {
    /// Master enable/disable for the Soul Engine
    pub enabled: bool,
    /// Global edginess level (per-character can override)
    pub edginess: EdginessLevel,
    /// How often (in seconds) the idle scheduler ticks
    pub idle_tick_secs: u64,
    /// Minimum seconds between emotes/chat for a single character
    pub min_chat_interval_secs: u64,
    /// Maximum seconds between emotes/chat for a single character
    pub max_chat_interval_secs: u64,
    /// Whether characters talk to each other
    pub inter_character_chat: bool,
    /// Whether characters respond to real player tells
    pub player_chat_enabled: bool,
    /// Per-character soul configs
    pub character: Vec<CharacterSoulConfig>,
    /// Pre-defined relationships between characters
    pub relationship: Vec<RelationshipSeed>,
}

impl Default for SoulConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            edginess: EdginessLevel::default(),
            idle_tick_secs: 30,
            min_chat_interval_secs: 60,
            max_chat_interval_secs: 300,
            inter_character_chat: true,
            player_chat_enabled: true,
            character: Vec::new(),
            relationship: Vec::new(),
        }
    }
}
