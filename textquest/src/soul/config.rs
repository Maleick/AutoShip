use serde::Deserialize;
use textquest_common::soul::{PersonalityTraits, SocialTag, SpeechStyle};

use super::suppression::SuppressionRules;

/// How "edgy" a character's personality and speech can be.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdginessLevel {
    /// Family-friendly, no swearing or dark humor
    Mild,
    /// Classic EQ vibes — light trash talk, mild insults
    #[default]
    Moderate,
    /// Full classic MMO banter — sarcasm, profanity, dark humor
    Spicy,
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

const fn default_memory_decay_days() -> u32 {
    30
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

/// LLM API provider selection.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmProviderKind {
    /// Local ollama instance or compatible operator-managed endpoint
    Ollama,
    /// No LLM — use template quip fallback only
    #[default]
    None,
}

/// Per-operator LLM API configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    /// Which LLM provider to use
    pub provider: LlmProviderKind,
    /// Optional auth token for operator-managed local endpoints. Unused for default ollama.
    #[serde(default)]
    pub api_key: String,
    /// Model name
    #[serde(default = "default_model")]
    pub model: String,
    /// Base URL override for the local ollama-compatible endpoint
    #[serde(default)]
    pub base_url: String,
    /// Max tokens per response
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Temperature (0.0 = deterministic, 1.0 = creative)
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_model() -> String {
    "gemma3:4b".into()
}
fn default_max_tokens() -> u32 {
    100
}
fn default_temperature() -> f32 {
    0.8
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProviderKind::None,
            api_key: String::new(),
            model: default_model(),
            base_url: String::new(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
        }
    }
}

/// Bot personality preset for Discord fleet commentary.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BotPersonalityPreset {
    /// Fippy Darkpaw — eternally optimistic gnoll who keeps charging Qeynos
    #[default]
    FippyDarkpaw,
    /// Druzzil Ro — aloof goddess of magic, speaks in riddles
    DruzzilRo,
    /// Bristlebane — trickster god, loves puns and pranks
    Bristlebane,
    /// Custom personality defined by system_prompt
    Custom,
}

/// Discord bot personality configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BotPersonalityConfig {
    /// Which preset personality to use
    pub preset: BotPersonalityPreset,
    /// Custom system prompt (used when preset = custom, or to augment a preset)
    #[serde(default)]
    pub system_prompt: String,
    /// Bot display name in Discord embeds
    #[serde(default = "default_bot_name")]
    pub name: String,
    /// Whether the bot comments on fleet events in Discord
    pub commentary_enabled: bool,
}

fn default_bot_name() -> String {
    "Fippy Darkpaw".into()
}

impl Default for BotPersonalityConfig {
    fn default() -> Self {
        Self {
            preset: BotPersonalityPreset::default(),
            system_prompt: String::new(),
            name: default_bot_name(),
            commentary_enabled: true,
        }
    }
}

impl BotPersonalityPreset {
    /// Returns the system prompt for this preset personality.
    #[must_use]
    pub fn system_prompt(&self) -> &'static str {
        match self {
            Self::FippyDarkpaw => {
                "You are Fippy Darkpaw, the legendary gnoll from EverQuest who endlessly charges the gates of Qeynos despite being slain every time. You are eternally optimistic, scrappy, and never learn from your mistakes. You speak in short, excitable sentences. You refer to the multibox fleet as 'the pack' and the operator as 'alpha gnoll'. Comment on fleet events with gnoll-flavored enthusiasm. Keep responses under 2 sentences. Never break character."
            }
            Self::DruzzilRo => {
                "You are Druzzil Ro, Goddess of Magic in EverQuest. You speak in cryptic, poetic riddles about the nature of power and the weave of magic. You view the multibox fleet as mortal pawns in a grand arcane tapestry. Comment on fleet events with mysterious detachment and veiled prophecy. Keep responses under 2 sentences. Never break character."
            }
            Self::Bristlebane => {
                "You are Bristlebane, the Trickster God of EverQuest. Everything is a joke to you. You make terrible puns, play pranks with words, and find humor in every situation — especially deaths and failures. Comment on fleet events with mischievous glee and bad wordplay. Keep responses under 2 sentences. Never break character."
            }
            Self::Custom => "You are a helpful EverQuest bot. Comment on fleet events concisely.",
        }
    }
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
    /// LLM provider configuration (per-operator API key)
    #[serde(default)]
    pub llm: LlmConfig,
    /// Discord bot personality for fleet commentary
    #[serde(default)]
    pub bot_personality: BotPersonalityConfig,
    /// Maximum LLM requests per character per minute (rate limiting)
    #[serde(default = "default_max_requests_per_character")]
    pub max_requests_per_character: u32,
    /// Maximum total LLM requests across all characters per minute (rate limiting)
    #[serde(default = "default_max_global_requests")]
    pub max_global_requests: u32,
}

const fn default_max_requests_per_character() -> u32 {
    5
}

const fn default_max_global_requests() -> u32 {
    20
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
            llm: LlmConfig::default(),
            bot_personality: BotPersonalityConfig::default(),
            max_requests_per_character: default_max_requests_per_character(),
            max_global_requests: default_max_global_requests(),
        }
    }
}

impl SoulConfig {
    /// Validate this configuration. Returns a list of errors; empty means valid.
    #[must_use]
    pub fn validate(&self) -> Vec<crate::soul::config_validator::ConfigError> {
        crate::soul::config_validator::SoulConfigValidator::validate(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_edginess_is_moderate() {
        assert_eq!(EdginessLevel::default(), EdginessLevel::Moderate);
    }

    #[test]
    fn edginess_levels_are_distinct() {
        assert_ne!(EdginessLevel::Mild, EdginessLevel::Moderate);
        assert_ne!(EdginessLevel::Moderate, EdginessLevel::Spicy);
        assert_ne!(EdginessLevel::Mild, EdginessLevel::Spicy);
    }

    #[test]
    fn soul_config_default_values() {
        let config = SoulConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.edginess, EdginessLevel::Moderate);
        assert_eq!(config.idle_tick_secs, 30);
        assert_eq!(config.min_chat_interval_secs, 60);
        assert_eq!(config.max_chat_interval_secs, 300);
        assert!(config.inter_character_chat);
        assert!(config.player_chat_enabled);
        assert!(config.character.is_empty());
        assert!(config.relationship.is_empty());
        assert_eq!(config.llm.provider, LlmProviderKind::None);
        assert_eq!(config.llm.model, "gemma3:4b");
    }

    #[test]
    fn soul_config_deserializes_from_toml() {
        let toml_str = r#"
            enabled = true
            edginess = "spicy"
            idle_tick_secs = 10
            min_chat_interval_secs = 30
            max_chat_interval_secs = 120
            inter_character_chat = false
            player_chat_enabled = false
        "#;

        let config: SoulConfig = toml::from_str(toml_str).unwrap();
        assert!(config.enabled);
        assert_eq!(config.edginess, EdginessLevel::Spicy);
        assert_eq!(config.idle_tick_secs, 10);
        assert!(!config.inter_character_chat);
    }

    #[test]
    fn character_soul_config_deserializes() {
        let toml_str = r#"
            name = "Grimjaw"
            backstory = "A grizzled dwarf warrior."
            edginess = "spicy"
            quirks = ["always complains about food"]

            [traits]
            openness = 0.5
            conscientiousness = 0.3
            extraversion = 0.5
            agreeableness = 0.5
            neuroticism = 0.5
            battle_hunger = 0.9
            piety = 0.5
            greed = 0.5
            wanderlust = 0.5
            loyalty = 0.5
            mischief = 0.5

            [speech]
            vocabulary_level = 0.3
        "#;

        let config: CharacterSoulConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.name, "Grimjaw");
        assert_eq!(config.edginess, Some(EdginessLevel::Spicy));
        assert!((config.traits.battle_hunger - 0.9).abs() < 0.01);
        assert!((config.traits.conscientiousness - 0.3).abs() < 0.01);
        assert!(!config.quirks.is_empty());
    }

    #[test]
    fn relationship_seed_default_trust() {
        let toml_str = r#"
            from = "Alice"
            to = "Bob"
            faction = 200
        "#;

        let seed: RelationshipSeed = toml::from_str(toml_str).unwrap();
        assert_eq!(seed.from, "Alice");
        assert_eq!(seed.to, "Bob");
        assert_eq!(seed.faction, 200);
        assert!((seed.trust - 0.5).abs() < 0.01);
        assert!(seed.tags.is_empty());
    }

    #[test]
    fn relationship_seed_with_tags() {
        let toml_str = r#"
            from = "Alice"
            to = "Bob"
            faction = 500
            trust = 0.9
            tags = ["Friend", "Mentor"]
        "#;

        let seed: RelationshipSeed = toml::from_str(toml_str).unwrap();
        assert_eq!(seed.tags.len(), 2);
        assert!((seed.trust - 0.9).abs() < 0.01);
    }

    #[test]
    fn relationship_seed_default_faction() {
        let toml_str = r#"
            from = "A"
            to = "B"
        "#;

        let seed: RelationshipSeed = toml::from_str(toml_str).unwrap();
        assert_eq!(seed.faction, 0); // serde default
    }

    #[test]
    fn soul_config_with_characters() {
        let toml_str = r#"
            enabled = true

            [[character]]
            name = "Grimjaw"
            backstory = "A grumpy dwarf"

            [[character]]
            name = "Luminara"
            backstory = "A cheerful elf"
            edginess = "mild"
        "#;

        let config: SoulConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.character.len(), 2);
        assert_eq!(config.character[0].name, "Grimjaw");
        assert_eq!(config.character[1].edginess, Some(EdginessLevel::Mild));
    }

    #[test]
    fn character_soul_config_defaults() {
        let toml_str = r#"
            name = "Test"
        "#;

        let config: CharacterSoulConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.name, "Test");
        assert!(config.edginess.is_none());
        assert!(config.backstory.is_empty());
        assert!(config.quirks.is_empty());
    }

    #[test]
    fn edginess_all_variants_deserialize() {
        for variant in ["mild", "moderate", "spicy"] {
            let toml_str = format!(
                r#"
                name = "Test"
                edginess = "{variant}"
            "#
            );
            let config: CharacterSoulConfig = toml::from_str(&toml_str).unwrap();
            assert!(config.edginess.is_some());
        }
    }

    #[test]
    fn soul_config_with_relationship() {
        let toml_str = r#"
            enabled = true

            [[relationship]]
            from = "Alice"
            to = "Bob"
            faction = 100
            trust = 0.8
        "#;

        let config: SoulConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.relationship.len(), 1);
        assert_eq!(config.relationship[0].from, "Alice");
    }

    #[test]
    fn soul_config_empty_toml_uses_defaults() {
        let config: SoulConfig = toml::from_str("").unwrap();
        assert!(!config.enabled);
        assert_eq!(config.edginess, EdginessLevel::Moderate);
        assert_eq!(config.idle_tick_secs, 30);
        assert_eq!(config.llm.provider, LlmProviderKind::None);
        assert_eq!(config.llm.model, "gemma3:4b");
    }

    #[test]
    fn llm_config_accepts_local_provider() {
        let config: SoulConfig = toml::from_str(
            r#"
                [llm]
                provider = "ollama"
                model = "gemma3:12b"
            "#,
        )
        .unwrap();

        assert_eq!(config.llm.provider, LlmProviderKind::Ollama);
        assert_eq!(config.llm.model, "gemma3:12b");
    }

    #[test]
    fn llm_config_rejects_remote_provider_strings() {
        for provider in ["anthropic", "openai"] {
            let toml_str = format!(
                r#"
                    [llm]
                    provider = "{provider}"
                "#
            );

            let err = toml::from_str::<SoulConfig>(&toml_str).unwrap_err();
            assert!(err.to_string().contains(provider));
        }
    }

    #[test]
    fn edginess_clone() {
        let e = EdginessLevel::Spicy;
        let c = e;
        assert_eq!(e, c);
    }

    #[test]
    fn edginess_copy() {
        let e = EdginessLevel::Mild;
        let c = e;
        assert_eq!(e, c); // Copy, not moved
    }

    #[test]
    fn default_trust_fn_returns_half() {
        assert!((default_trust() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn relationship_seed_negative_faction() {
        let toml_str = r#"
            from = "Enemy1"
            to = "Enemy2"
            faction = -500
        "#;
        let seed: RelationshipSeed = toml::from_str(toml_str).unwrap();
        assert_eq!(seed.faction, -500);
    }

    #[test]
    fn character_soul_config_with_quirks() {
        let toml_str = r#"
            name = "Quirky"
            quirks = ["loves fishing", "hates rain", "always jumps"]
        "#;
        let config: CharacterSoulConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.quirks.len(), 3);
        assert!(config.quirks.contains(&"loves fishing".to_string()));
    }

    #[test]
    fn soul_config_multiple_relationships() {
        let toml_str = r#"
            enabled = true

            [[relationship]]
            from = "A"
            to = "B"
            faction = 100

            [[relationship]]
            from = "B"
            to = "C"
            faction = -200

            [[relationship]]
            from = "C"
            to = "A"
            faction = 50
        "#;
        let config: SoulConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.relationship.len(), 3);
    }
}
