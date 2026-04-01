// dmft-common/src/soul.rs — Shared types for the Soul Engine (M5)
use serde::{Deserialize, Serialize};

/// Big Five personality traits + EQ-themed traits.
/// All values are 0.0..1.0 (normalized).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonalityTraits {
    // Big Five
    /// Openness to experience (curiosity, creativity).
    pub openness: f32,
    /// Conscientiousness (discipline, organization).
    pub conscientiousness: f32,
    /// Extraversion (sociability, talkativeness).
    pub extraversion: f32,
    /// Agreeableness (cooperativeness, empathy).
    pub agreeableness: f32,
    /// Neuroticism (emotional instability, anxiety).
    pub neuroticism: f32,
    // EQ-themed traits
    /// Eagerness for combat encounters.
    pub battle_hunger: f32,
    /// Devotion to deity and role-play religiosity.
    pub piety: f32,
    /// Desire for loot and wealth accumulation.
    pub greed: f32,
    /// Urge to explore new zones and wander.
    pub wanderlust: f32,
    /// Devotion to groupmates and guild.
    pub loyalty: f32,
    /// Tendency toward pranks and playful behavior.
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoodState {
    /// Baseline emotional state.
    #[default]
    Neutral,
    /// Positive mood — more social, generous behavior.
    Happy,
    /// Aggressive mood — more combat-hungry, less patient.
    Angry,
    /// Nervous mood — cautious pulls, avoids risk.
    Anxious,
    /// Bored — triggers idle behaviors and wandering.
    Bored,
    /// High energy — fast actions, more emotes.
    Excited,
    /// Sad mood — quieter, introspective behavior.
    Melancholy,
    /// Concentrated — efficient combat, minimal chat.
    Focused,
    /// Lighthearted — jokes, pranks, random emotes.
    Playful,
    /// Fatigued — slower actions, may AFK or log off.
    Exhausted,
}

/// Types of idle behavior a character can perform when not in combat or traveling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdleBehaviorType {
    /// Sit down to regen mana/HP.
    Sit,
    /// Wander randomly near camp.
    Wander,
    /// Perform a random emote.
    Emote,
    /// Go fishing at a nearby water source.
    Fish,
    /// Craft items (tradeskills).
    Craft,
    /// Browse a nearby vendor's inventory.
    VendorBrowse,
    /// Chat about zone lore or tell stories.
    LoreChatter,
    /// Announce a bio break (AFK message).
    BioBrk,
    /// Log off the character to simulate sleep.
    LogOffToSleep,
    /// Jump randomly in place (fidget behavior).
    RandomJump,
    /// Inspect a nearby player's gear.
    Inspect,
}

/// Tags for social relationships between characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SocialTag {
    /// Positive relationship — groupmate, ally.
    Friend,
    /// Competitive relationship — contested camps, loot rivalry.
    Rival,
    /// Teaches or guides this character.
    Mentor,
    /// Learns from this character.
    Mentee,
    /// Family bond (shared account/player lore).
    Sibling,
    /// Casual contact — met once, no strong bond.
    Acquaintance,
    /// Hostile relationship — KOS, grief history.
    Nemesis,
    /// Romantic interest (RP flavor).
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
    /// Character died.
    Death {
        /// Zone short name where death occurred.
        zone: String,
        /// Name of the mob/player that killed us, if known.
        killer: Option<String>,
    },
    /// Character got a notable kill.
    Kill {
        /// Name of the killed mob.
        target: String,
        /// Zone short name.
        zone: String,
    },
    /// Received loot.
    Loot {
        /// Item name that was looted.
        item: String,
        /// Zone short name.
        zone: String,
    },
    /// Had a conversation with a real player.
    PlayerChat {
        /// Name of the real player.
        player_name: String,
        /// Sentiment score (-1.0 hostile .. 1.0 friendly).
        sentiment: f32,
    },
    /// Had a conversation with another bot.
    BotChat {
        /// Character name of the other bot.
        character_name: String,
    },
    /// Witnessed something notable.
    Witnessed {
        /// Free-text description of the event.
        description: String,
    },
    /// Mood shifted.
    MoodShift {
        /// Previous mood state.
        from: MoodState,
        /// New mood state.
        to: MoodState,
        /// Why the mood changed.
        reason: String,
    },
    /// Entered a new zone.
    ZoneEnter {
        /// Zone short name.
        zone: String,
    },
    /// Level gained.
    LevelUp {
        /// The new level reached.
        new_level: u8,
    },
    /// Group wipe.
    GroupWipe {
        /// Zone short name where the wipe occurred.
        zone: String,
    },
    /// Relationship changed with another character.
    RelationshipChange {
        /// Name of the other character.
        character: String,
        /// Change in relationship score (-1.0 .. 1.0).
        delta: f32,
    },
}

/// EQ chat channels for Say/Emote commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SayChannel {
    /// /say — local area chat.
    Say,
    /// /shout — zone-wide broadcast.
    Shout,
    /// /ooc — out-of-character zone chat.
    Ooc,
    /// /gu — guild chat channel.
    Guild,
    /// /g — group chat channel.
    Group,
    /// /tell — private message to a player.
    Tell,
    /// /auction — zone-wide trade channel.
    Auction,
}

/// Actions the Soul Engine can request the orchestrator to execute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SoulAction {
    /// Send a chat message on a channel.
    Say {
        /// Chat channel to use.
        channel: SayChannel,
        /// Message text to send.
        message: String,
        /// For tells — the target player name.
        target: Option<String>,
    },
    /// Perform an emote.
    Emote {
        /// Emote name (e.g. "dance", "bow").
        emote: String,
    },
    /// Start an idle behavior.
    StartIdle {
        /// The idle behavior to perform.
        behavior: IdleBehaviorType,
    },
    /// Stop current idle behavior and return to normal
    StopIdle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mood_state_default_is_neutral() {
        assert_eq!(MoodState::default(), MoodState::Neutral);
    }

    #[test]
    fn mood_state_all_variants() {
        let variants = [
            MoodState::Neutral,
            MoodState::Happy,
            MoodState::Angry,
            MoodState::Anxious,
            MoodState::Bored,
            MoodState::Excited,
            MoodState::Melancholy,
            MoodState::Focused,
            MoodState::Playful,
            MoodState::Exhausted,
        ];
        // Verify they are all distinct
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn personality_traits_default_all_half() {
        let traits = PersonalityTraits::default();
        assert!((traits.openness - 0.5).abs() < f32::EPSILON);
        assert!((traits.conscientiousness - 0.5).abs() < f32::EPSILON);
        assert!((traits.extraversion - 0.5).abs() < f32::EPSILON);
        assert!((traits.agreeableness - 0.5).abs() < f32::EPSILON);
        assert!((traits.neuroticism - 0.5).abs() < f32::EPSILON);
        assert!((traits.battle_hunger - 0.5).abs() < f32::EPSILON);
        assert!((traits.loyalty - 0.5).abs() < f32::EPSILON);
        assert!((traits.mischief - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn speech_style_default() {
        let style = SpeechStyle::default();
        assert!((style.vocabulary_level - 0.5).abs() < f32::EPSILON);
        assert!((style.typing_speed - 1.0).abs() < f32::EPSILON);
        assert!(style.catchphrases.is_empty());
        assert!(style.adopted_slang.is_empty());
    }

    #[test]
    fn social_tag_equality() {
        assert_eq!(SocialTag::Friend, SocialTag::Friend);
        assert_ne!(SocialTag::Friend, SocialTag::Rival);
    }

    #[test]
    fn say_channel_equality() {
        assert_eq!(SayChannel::Say, SayChannel::Say);
        assert_ne!(SayChannel::Say, SayChannel::Group);
    }

    #[test]
    fn default_half_returns_half() {
        assert!((default_half() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn default_one_returns_one() {
        assert!((default_one() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn idle_behavior_type_all_variants() {
        let variants = [
            IdleBehaviorType::Sit,
            IdleBehaviorType::Wander,
            IdleBehaviorType::Emote,
            IdleBehaviorType::Fish,
            IdleBehaviorType::Craft,
            IdleBehaviorType::VendorBrowse,
            IdleBehaviorType::LoreChatter,
            IdleBehaviorType::BioBrk,
            IdleBehaviorType::LogOffToSleep,
            IdleBehaviorType::RandomJump,
            IdleBehaviorType::Inspect,
        ];
        assert_eq!(variants.len(), 11);
        // Verify distinctness
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn say_channel_all_variants() {
        let channels = [
            SayChannel::Say,
            SayChannel::Shout,
            SayChannel::Ooc,
            SayChannel::Guild,
            SayChannel::Group,
            SayChannel::Tell,
            SayChannel::Auction,
        ];
        assert_eq!(channels.len(), 7);
    }

    #[test]
    fn soul_event_death_construction() {
        let event = SoulEvent::Death {
            zone: "nektulos".into(),
            killer: Some("a_shadow_wolf".into()),
        };
        if let SoulEvent::Death { zone, killer } = event {
            assert_eq!(zone, "nektulos");
            assert_eq!(killer, Some("a_shadow_wolf".into()));
        } else {
            panic!("expected Death");
        }
    }

    #[test]
    fn soul_event_death_no_killer() {
        let event = SoulEvent::Death {
            zone: "lava".into(),
            killer: None,
        };
        if let SoulEvent::Death { killer, .. } = event {
            assert!(killer.is_none());
        }
    }

    #[test]
    fn soul_event_mood_shift() {
        let event = SoulEvent::MoodShift {
            from: MoodState::Neutral,
            to: MoodState::Angry,
            reason: "got ganked".into(),
        };
        if let SoulEvent::MoodShift { from, to, reason } = event {
            assert_eq!(from, MoodState::Neutral);
            assert_eq!(to, MoodState::Angry);
            assert_eq!(reason, "got ganked");
        }
    }

    #[test]
    fn soul_event_level_up() {
        let event = SoulEvent::LevelUp { new_level: 60 };
        if let SoulEvent::LevelUp { new_level } = event {
            assert_eq!(new_level, 60);
        }
    }

    #[test]
    fn soul_action_say_construction() {
        let action = SoulAction::Say {
            channel: SayChannel::Group,
            message: "inc!".into(),
            target: None,
        };
        if let SoulAction::Say {
            channel,
            message,
            target,
        } = action
        {
            assert_eq!(channel, SayChannel::Group);
            assert_eq!(message, "inc!");
            assert!(target.is_none());
        }
    }

    #[test]
    fn soul_action_tell_has_target() {
        let action = SoulAction::Say {
            channel: SayChannel::Tell,
            message: "hello".into(),
            target: Some("Legolas".into()),
        };
        if let SoulAction::Say { target, .. } = action {
            assert_eq!(target, Some("Legolas".into()));
        }
    }

    #[test]
    fn soul_action_idle_behavior() {
        let action = SoulAction::StartIdle {
            behavior: IdleBehaviorType::Fish,
        };
        if let SoulAction::StartIdle { behavior } = action {
            assert_eq!(behavior, IdleBehaviorType::Fish);
        }
    }

    #[test]
    fn speech_style_serde_defaults() {
        // Deserialize with missing optional fields to test serde defaults
        let json = r#"{"vocabulary_level": 0.8}"#;
        let style: SpeechStyle = serde_json::from_str(json).expect("deserialize");
        assert!((style.vocabulary_level - 0.8).abs() < f32::EPSILON);
        // Missing fields should get defaults
        assert!((style.emote_frequency - 0.5).abs() < f32::EPSILON);
        assert!((style.typing_speed - 1.0).abs() < f32::EPSILON);
        assert!(style.catchphrases.is_empty());
    }

    #[test]
    fn personality_traits_serialization_roundtrip() {
        let traits = PersonalityTraits {
            openness: 0.9,
            conscientiousness: 0.1,
            extraversion: 0.7,
            agreeableness: 0.3,
            neuroticism: 0.5,
            battle_hunger: 0.8,
            piety: 0.2,
            greed: 0.6,
            wanderlust: 0.4,
            loyalty: 0.95,
            mischief: 0.05,
        };
        let json = serde_json::to_string(&traits).expect("serialize");
        let restored: PersonalityTraits = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, traits);
    }
}
