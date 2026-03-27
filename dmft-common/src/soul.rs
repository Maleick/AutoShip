// dmft-common/src/soul.rs — Shared types for the Soul Engine (M5)
use serde::{Deserialize, Serialize};

/// Big Five personality traits + EQ-themed traits.
/// All values are 0.0..1.0 (normalized).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityTraits {
    // Big Five
    pub openness: f32,
    pub conscientiousness: f32,
    pub extraversion: f32,
    pub agreeableness: f32,
    pub neuroticism: f32,
    // EQ-themed traits
    pub battle_hunger: f32,
    pub piety: f32,
    pub greed: f32,
    pub wanderlust: f32,
    pub loyalty: f32,
    pub mischief: f32,
}

impl Default for PersonalityTraits {
    fn default() -> Self {
        Self {
            openness: 0.5,
            conscientiousness: 0.5,
            extraversion: 0.5,
            agreeableness: 0.5,
            neuroticism: 0.5,
            battle_hunger: 0.5,
            piety: 0.5,
            greed: 0.5,
            wanderlust: 0.5,
            loyalty: 0.5,
            mischief: 0.5,
        }
    }
}

/// Current mood of a character. Affects combat style, social behavior, idle choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoodState {
    Neutral,
    Happy,
    Angry,
    Anxious,
    Bored,
    Excited,
    Melancholy,
    Focused,
    Playful,
    Exhausted,
}

impl Default for MoodState {
    fn default() -> Self {
        Self::Neutral
    }
}

/// Types of idle behavior a character can perform when not in combat or traveling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdleBehaviorType {
    Sit,
    Wander,
    Emote,
    Fish,
    Craft,
    VendorBrowse,
    LoreChatter,
    BioBrk,
    LogOffToSleep,
    RandomJump,
    Inspect,
}

/// Tags for social relationships between characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SocialTag {
    Friend,
    Rival,
    Mentor,
    Mentee,
    Sibling,
    Acquaintance,
    Nemesis,
    Crush,
}

/// Per-character speech style that evolves over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechStyle {
    /// Vocabulary tier: 0.0 = simple, 1.0 = elaborate
    #[serde(default = "default_half")]
    pub vocabulary_level: f32,
    /// How often the character uses emotes vs text
    #[serde(default = "default_half")]
    pub emote_frequency: f32,
    /// Typing speed factor (affects simulated typing delay)
    #[serde(default = "default_one")]
    pub typing_speed: f32,
    /// Catchphrases this character has picked up
    #[serde(default)]
    pub catchphrases: Vec<String>,
    /// Slang or inside jokes adopted from others
    #[serde(default)]
    pub adopted_slang: Vec<String>,
}

fn default_half() -> f32 {
    0.5
}

fn default_one() -> f32 {
    1.0
}

impl Default for SpeechStyle {
    fn default() -> Self {
        Self {
            vocabulary_level: 0.5,
            emote_frequency: 0.5,
            typing_speed: 1.0,
            catchphrases: Vec::new(),
            adopted_slang: Vec::new(),
        }
    }
}

/// Events that the Soul Engine tracks for character memory and mood evolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SoulEvent {
    /// Character died
    Death { zone: String, killer: Option<String> },
    /// Character got a notable kill
    Kill { target: String, zone: String },
    /// Received loot
    Loot { item: String, zone: String },
    /// Had a conversation with a real player
    PlayerChat { player_name: String, sentiment: f32 },
    /// Had a conversation with another bot
    BotChat { character_name: String },
    /// Witnessed something notable
    Witnessed { description: String },
    /// Mood shifted
    MoodShift { from: MoodState, to: MoodState, reason: String },
    /// Entered a new zone
    ZoneEnter { zone: String },
    /// Level gained
    LevelUp { new_level: u8 },
    /// Group wipe
    GroupWipe { zone: String },
    /// Relationship changed with another character
    RelationshipChange { character: String, delta: f32 },
}

/// EQ chat channels for Say/Emote commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SayChannel {
    Say,
    Shout,
    Ooc,
    Guild,
    Group,
    Tell,
    Auction,
}

/// Actions the Soul Engine can request the orchestrator to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SoulAction {
    /// Send a chat message on a channel
    Say {
        channel: SayChannel,
        message: String,
        /// For tells — the target player name
        target: Option<String>,
    },
    /// Perform an emote
    Emote { emote: String },
    /// Start an idle behavior
    StartIdle { behavior: IdleBehaviorType },
    /// Stop current idle behavior and return to normal
    StopIdle,
}
