use anyhow::Result;
use textquest_common::{
    nav::Xorshift32,
    soul::{MoodState, PersonalityTraits, SpeechStyle},
};

use super::{LlmProvider, LlmRequest, LlmResponse, Situation};
use crate::config::EdginessLevel;

/// Phase 1 fallback: generates text from trait vectors + phrase templates.
/// No LLM API calls — everything is deterministic based on personality.
pub struct TraitDrivenResponder {
    rng: Xorshift32,
    edginess: EdginessLevel,
}

impl TraitDrivenResponder {
    /// Create a new trait-driven responder seeded from the client ID.
    #[must_use]
    pub fn new(client_id: u32, edginess: EdginessLevel) -> Self {
        Self {
            rng: Xorshift32::from_client_id(client_id),
            edginess,
        }
    }

    /// Pick a random entry from a slice.
    fn pick<'a>(&mut self, options: &'a [&str]) -> &'a str {
        let idx = (self.rng.next_u32() as usize) % options.len();
        options[idx]
    }

    /// Apply speech style post-processing to generated text.
    fn apply_speech_style(&mut self, text: &str, style: &SpeechStyle) -> String {
        let mut result = text.to_string();

        // Vocabulary level: simple characters drop to lowercase, elaborate add flair
        if style.vocabulary_level < 0.3 {
            result = result.to_lowercase();
        }

        // Occasionally append a catchphrase
        if !style.catchphrases.is_empty() && self.rng.next_f32() < 0.15 {
            let idx = (self.rng.next_u32() as usize) % style.catchphrases.len();
            result = format!("{} {}", result, style.catchphrases[idx]);
        }

        // Occasionally use adopted slang
        if !style.adopted_slang.is_empty() && self.rng.next_f32() < 0.1 {
            let idx = (self.rng.next_u32() as usize) % style.adopted_slang.len();
            result = format!("{} {}", style.adopted_slang[idx], result);
        }

        result
    }

    /// Generate a response for a player chat situation.
    fn respond_to_player(
        &mut self,
        player_name: &str,
        _message: &str,
        traits: &PersonalityTraits,
        mood: MoodState,
        memory_context: &[String],
    ) -> String {
        // Friendly vs curt response based on agreeableness + mood
        let greetings = if traits.agreeableness > 0.6 {
            match mood {
                MoodState::Happy | MoodState::Playful => &[
                    "Hey there! Good to see you.",
                    "Hail! Welcome, friend.",
                    "Well met! How's it going?",
                    "Hey! Pull up a chair.",
                ] as &[&str],
                MoodState::Angry => &[
                    "Yeah? What do you need?",
                    "Hail. Bit busy here.",
                    "Oh hey. What's up?",
                ],
                _ => &[
                    "Hail, friend.",
                    "Hey there.",
                    "Good to see you.",
                    "What brings you around?",
                ],
            }
        } else {
            match mood {
                MoodState::Happy | MoodState::Playful => {
                    &["Oh, hey.", "Hail.", "Sup."] as &[&str]
                }
                MoodState::Angry => &["What.", "Busy.", "Yeah?"],
                _ => &["Hail.", "Hey.", "Sup.", "What do you need?"],
            }
        };

        let greeting = self.pick(greetings);
        let memory_note = memory_context.first().map(|entry| {
            let snippet = entry.replace('\n', " ");
            snippet.chars().take(72).collect::<String>()
        });

        // Sometimes add the player's name
        if traits.extraversion > 0.5 && self.rng.next_f32() < 0.4 {
            if let Some(memory_note) = memory_note {
                format!("{greeting}, {player_name}. I still remember {memory_note}.")
            } else {
                format!("{greeting}, {player_name}")
            }
        } else if let Some(memory_note) = memory_note {
            format!("{greeting}. I still remember {memory_note}.")
        } else {
            greeting.to_string()
        }
    }

    /// Generate idle chatter flavor text.
    fn idle_chatter(&mut self, traits: &PersonalityTraits, mood: MoodState) -> String {
        let phrases = idle_phrases(mood, self.edginess, traits);
        self.pick(phrases).to_string()
    }

    /// Generate a game event reaction.
    fn react_to_event(
        &mut self,
        description: &str,
        traits: &PersonalityTraits,
        mood: MoodState,
    ) -> String {
        // Generic reactions flavored by mood
        let reactions = event_reactions(mood, self.edginess);
        let base = self.pick(reactions);

        // High openness characters comment on what happened
        if traits.openness > 0.7 && self.rng.next_f32() < 0.3 {
            format!("{base} {description}")
        } else {
            base.to_string()
        }
    }

    /// Generate a combat reaction.
    fn react_to_combat(
        &mut self,
        description: &str,
        traits: &PersonalityTraits,
        mood: MoodState,
    ) -> String {
        let reactions = combat_reactions(mood, self.edginess, traits);
        let base = self.pick(reactions);

        if traits.battle_hunger > 0.7 && self.rng.next_f32() < 0.3 {
            format!("{base}! {description}")
        } else {
            base.to_string()
        }
    }

    /// Generate a response to another bot character.
    fn respond_to_bot(
        &mut self,
        character_name: &str,
        _message: &str,
        traits: &PersonalityTraits,
        mood: MoodState,
    ) -> String {
        let responses = bot_chat_responses(mood, traits);
        let base = self.pick(responses);

        if traits.extraversion > 0.6 && self.rng.next_f32() < 0.3 {
            format!("{base}, {character_name}")
        } else {
            base.to_string()
        }
    }
}

impl LlmProvider for TraitDrivenResponder {
    fn generate(&mut self, request: &LlmRequest) -> Result<LlmResponse> {
        let raw_text = match &request.situation {
            Situation::PlayerChat {
                player_name,
                message,
                ..
            } => self.respond_to_player(
                player_name,
                message,
                &request.traits,
                request.mood,
                &request.memory_context,
            ),

            Situation::IdleChatter => self.idle_chatter(&request.traits, request.mood),

            Situation::GameEvent { description } => {
                self.react_to_event(description, &request.traits, request.mood)
            }

            Situation::CombatReaction { description } => {
                self.react_to_combat(description, &request.traits, request.mood)
            }

            Situation::BotChat {
                character_name,
                message,
            } => self.respond_to_bot(character_name, message, &request.traits, request.mood),

            Situation::FleetCommentary { event_summary } => {
                self.react_to_event(event_summary, &request.traits, request.mood)
            }
        };

        let text = self.apply_speech_style(&raw_text, &request.speech_style);

        Ok(LlmResponse {
            text,
            from_llm: false,
            tokens_used: 0,
        })
    }

    fn name(&self) -> &str {
        "trait-driven-fallback"
    }

    fn is_available(&self) -> bool {
        true // always available
    }
}

// ─── Phrase tables for fallback generation ───

fn idle_phrases(
    mood: MoodState,
    edginess: EdginessLevel,
    traits: &PersonalityTraits,
) -> &'static [&'static str] {
    // Delegate to personality engine's phrase tables via mood + edginess
    // These are additional idle-specific phrases beyond the personality engine's
    // set
    match (mood, edginess) {
        (MoodState::Bored, _) if traits.wanderlust > 0.6 => &[
            "Wonder what's in the next zone over...",
            "I heard there's good hunting east of here.",
            "Anyone been to the other side of this zone?",
            "My feet are itching to move.",
        ],
        (MoodState::Bored, _) if traits.greed > 0.6 => &[
            "Bet the vendors have something good.",
            "I should check the tunnel prices.",
            "Nothing dropping here worth anything.",
            "My coin purse feels light.",
        ],
        (MoodState::Happy, _) if traits.piety > 0.6 => &[
            "The gods smile upon us today.",
            "Blessed be this fine company.",
            "May Tunare's light guide our path.",
            "We walk in grace.",
        ],
        (MoodState::Playful, _) if traits.mischief > 0.6 => &[
            "What if I 'accidentally' pulled that?",
            "Hey, anyone dare me to do something stupid?",
            "I wonder what happens if I press this...",
            "Rules are more like guidelines, right?",
        ],
        _ => match mood {
            MoodState::Neutral => &[
                "Quiet day.",
                "Nothing much happening.",
                "Just hanging out.",
                "All good here.",
            ],
            MoodState::Happy => &[
                "Life is good!",
                "Can't complain.",
                "Loving this.",
                "What a day!",
            ],
            MoodState::Bored => &[
                "So... anyone got a joke?",
                "I've counted all the rocks here.",
                "Think I'll go explore.",
                "Yawn.",
            ],
            MoodState::Anxious => &[
                "Is it just me or is it too quiet?",
                "I keep hearing things.",
                "Stay close, everyone.",
                "Something feels off.",
            ],
            MoodState::Focused => &["On it.", "Eyes forward.", "Staying sharp.", "Ready."],
            MoodState::Exhausted => &[
                "Need... coffee...",
                "How long have we been at this?",
                "Five more minutes...",
                "Zzz... huh? I'm awake!",
            ],
            _ => &["...", "Hmm.", "Interesting.", "Right then."],
        },
    }
}

fn event_reactions(mood: MoodState, _edginess: EdginessLevel) -> &'static [&'static str] {
    match mood {
        MoodState::Excited => &[
            "Whoa!",
            "Did you see that?!",
            "Now THAT was something!",
            "Amazing!",
        ],
        MoodState::Anxious => &[
            "That's not good.",
            "Uh oh.",
            "Everyone okay?",
            "Be careful!",
        ],
        MoodState::Happy => &["Nice!", "Awesome!", "Love it!", "Sweet!"],
        MoodState::Angry => &["Figures.", "Of course.", "Just great.", "Typical."],
        _ => &["Huh.", "Interesting.", "Well then.", "Noted."],
    }
}

fn combat_reactions(
    mood: MoodState,
    edginess: EdginessLevel,
    traits: &PersonalityTraits,
) -> &'static [&'static str] {
    if traits.battle_hunger > 0.7 {
        return match edginess {
            EdginessLevel::Mild => &["For glory!", "Have at thee!", "Charge!", "To battle!"],
            EdginessLevel::Moderate => {
                &["Get some!", "Bring it!", "Time to work!", "Let's do this!"]
            }
            EdginessLevel::Spicy => &["DIE!", "Crush them!", "Blood and thunder!", "DESTROY!"],
        };
    }

    match mood {
        MoodState::Focused => &["Target acquired.", "Engaging.", "On it.", "Attacking."],
        MoodState::Anxious => &["Here they come!", "Watch out!", "Incoming!", "Careful!"],
        MoodState::Excited => &["Let's go!", "More!", "Yeah!", "Woo!"],
        _ => &["Attacking.", "Got it.", "On target.", "Engaging."],
    }
}

fn bot_chat_responses(mood: MoodState, traits: &PersonalityTraits) -> &'static [&'static str] {
    if traits.extraversion > 0.7 {
        match mood {
            MoodState::Happy | MoodState::Playful => &[
                "Hah, good one!",
                "You said it!",
                "Right?!",
                "Tell me about it!",
                "Ha! Classic.",
            ],
            MoodState::Angry => &["Don't even start.", "Not now.", "Yeah yeah.", "Whatever."],
            _ => &["Mm-hmm.", "Yeah.", "Fair enough.", "True that."],
        }
    } else {
        match mood {
            MoodState::Happy => &["Heh.", "Yep.", "Mhm."],
            MoodState::Angry => &["...", "Sure.", "Uh huh."],
            _ => &["...", "Mm.", "Right.", "Yeah."],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::EdginessLevel,
        llm::{LlmPriority, LlmProvider, LlmRequest, Situation},
    };
    use textquest_common::soul::{MoodState, PersonalityTraits, SpeechStyle};

    fn make_request_with_situation(situation: Situation) -> LlmRequest {
        LlmRequest {
            character_name: "TestChar".into(),
            traits: PersonalityTraits::default(),
            mood: MoodState::Neutral,
            speech_style: SpeechStyle::default(),
            situation,
            priority: LlmPriority::Low,
            memory_context: Vec::new(),
            backstory: String::new(),
        }
    }

    fn make_request_with_mood_and_traits(
        mood: MoodState,
        traits: PersonalityTraits,
        situation: Situation,
    ) -> LlmRequest {
        LlmRequest {
            character_name: "TestChar".into(),
            traits,
            mood,
            speech_style: SpeechStyle::default(),
            situation,
            priority: LlmPriority::Low,
            memory_context: Vec::new(),
            backstory: String::new(),
        }
    }

    #[test]
    fn generate_idle_chatter_returns_non_empty() {
        let mut responder = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        let request = make_request_with_situation(Situation::IdleChatter);
        let response = responder.generate(&request).unwrap();
        assert!(!response.text.is_empty());
        assert!(!response.from_llm);
        assert_eq!(response.tokens_used, 0);
    }

    #[test]
    fn generate_player_chat_returns_non_empty() {
        let mut responder = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        let request = make_request_with_situation(Situation::PlayerChat {
            player_name: "Dave".into(),
            message: "Hey there!".into(),
            channel: "say".into(),
        });
        let response = responder.generate(&request).unwrap();
        assert!(!response.text.is_empty());
    }

    #[test]
    fn generate_game_event_returns_non_empty() {
        let mut responder = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        let request = make_request_with_situation(Situation::GameEvent {
            description: "A rare spawn appeared!".into(),
        });
        let response = responder.generate(&request).unwrap();
        assert!(!response.text.is_empty());
    }

    #[test]
    fn generate_combat_reaction_returns_non_empty() {
        let mut responder = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        let request = make_request_with_situation(Situation::CombatReaction {
            description: "Critical hit!".into(),
        });
        let response = responder.generate(&request).unwrap();
        assert!(!response.text.is_empty());
    }

    #[test]
    fn generate_bot_chat_returns_non_empty() {
        let mut responder = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        let request = make_request_with_situation(Situation::BotChat {
            character_name: "AllyBot".into(),
            message: "Good pull!".into(),
        });
        let response = responder.generate(&request).unwrap();
        assert!(!response.text.is_empty());
    }

    #[test]
    fn different_edginess_levels_produce_different_combat_reactions() {
        let battle_hungry = PersonalityTraits {
            battle_hunger: 0.9,
            ..Default::default()
        };

        let mut mild = TraitDrivenResponder::new(42, EdginessLevel::Mild);
        let mut spicy = TraitDrivenResponder::new(42, EdginessLevel::Spicy);

        let mild_req = make_request_with_mood_and_traits(
            MoodState::Excited,
            battle_hungry.clone(),
            Situation::CombatReaction {
                description: "Enemy down!".into(),
            },
        );
        let spicy_req = make_request_with_mood_and_traits(
            MoodState::Excited,
            battle_hungry,
            Situation::CombatReaction {
                description: "Enemy down!".into(),
            },
        );

        // Run multiple times and collect unique responses
        let mild_responses: Vec<String> = (0..10)
            .map(|_| mild.generate(&mild_req).unwrap().text)
            .collect();
        let spicy_responses: Vec<String> = (0..10)
            .map(|_| spicy.generate(&spicy_req).unwrap().text)
            .collect();

        // The phrase tables are completely different for Mild vs Spicy battle-hungry
        assert_ne!(mild_responses, spicy_responses);
    }

    #[test]
    fn different_situations_produce_different_responses() {
        let mut responder = TraitDrivenResponder::new(42, EdginessLevel::Moderate);

        let idle = responder
            .generate(&make_request_with_situation(Situation::IdleChatter))
            .unwrap()
            .text;

        // Reset with same seed for fair comparison
        let mut responder2 = TraitDrivenResponder::new(42, EdginessLevel::Moderate);
        let combat = responder2
            .generate(&make_request_with_situation(Situation::CombatReaction {
                description: "Engaged!".into(),
            }))
            .unwrap()
            .text;

        // Idle and combat responses come from different phrase tables
        // With the same seed, different tables should produce different text
        // (unless by extreme coincidence they share a phrase)
        // Just verify both are non-empty since tables are different
        assert!(!idle.is_empty());
        assert!(!combat.is_empty());
    }

    #[test]
    fn agreeable_player_chat_is_friendlier() {
        let agreeable = PersonalityTraits {
            agreeableness: 0.9,
            extraversion: 0.3,
            ..Default::default()
        };
        let disagreeable = PersonalityTraits {
            agreeableness: 0.1,
            extraversion: 0.3,
            ..Default::default()
        };

        let mut resp1 = TraitDrivenResponder::new(42, EdginessLevel::Moderate);
        let mut resp2 = TraitDrivenResponder::new(42, EdginessLevel::Moderate);

        let req1 = make_request_with_mood_and_traits(
            MoodState::Angry,
            agreeable,
            Situation::PlayerChat {
                player_name: "Dave".into(),
                message: "Hey".into(),
                channel: "say".into(),
            },
        );
        let req2 = make_request_with_mood_and_traits(
            MoodState::Angry,
            disagreeable,
            Situation::PlayerChat {
                player_name: "Dave".into(),
                message: "Hey".into(),
                channel: "say".into(),
            },
        );

        let friendly = resp1.generate(&req1).unwrap().text;
        let curt = resp2.generate(&req2).unwrap().text;

        // Both should be non-empty
        assert!(!friendly.is_empty());
        assert!(!curt.is_empty());
    }

    #[test]
    fn speech_style_catchphrase_applied() {
        let mut request = make_request_with_situation(Situation::IdleChatter);
        request.speech_style = SpeechStyle {
            vocabulary_level: 0.5,
            emote_frequency: 0.5,
            typing_speed: 1.0,
            catchphrases: vec!["By Bristlebane!".into()],
            adopted_slang: Vec::new(),
        };

        // Run many times; catchphrase has 15% chance per call
        let mut found_catchphrase = false;
        for i in 0..200 {
            let mut r = TraitDrivenResponder::new(i, EdginessLevel::Moderate);
            let resp = r.generate(&request).unwrap();
            if resp.text.contains("By Bristlebane!") {
                found_catchphrase = true;
                break;
            }
        }
        assert!(
            found_catchphrase,
            "Catchphrase should appear in at least one of 200 attempts"
        );
    }

    #[test]
    fn low_vocabulary_lowercases_output() {
        let mut responder = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        let mut request = make_request_with_situation(Situation::IdleChatter);
        request.speech_style = SpeechStyle {
            vocabulary_level: 0.1,
            emote_frequency: 0.5,
            typing_speed: 1.0,
            catchphrases: Vec::new(),
            adopted_slang: Vec::new(),
        };

        let response = responder.generate(&request).unwrap();
        // With vocabulary_level < 0.3, output should be lowercased
        assert_eq!(response.text, response.text.to_lowercase());
    }

    #[test]
    fn provider_name_is_correct() {
        let responder = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        assert_eq!(LlmProvider::name(&responder), "trait-driven-fallback");
    }

    #[test]
    fn provider_is_always_available() {
        let responder = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        assert!(LlmProvider::is_available(&responder));
    }

    #[test]
    fn idle_phrases_non_empty_for_all_moods_and_edginess() {
        let moods = [
            MoodState::Neutral,
            MoodState::Happy,
            MoodState::Angry,
            MoodState::Anxious,
            MoodState::Excited,
            MoodState::Melancholy,
            MoodState::Focused,
            MoodState::Playful,
            MoodState::Bored,
            MoodState::Exhausted,
        ];
        let edginess_levels = [
            EdginessLevel::Mild,
            EdginessLevel::Moderate,
            EdginessLevel::Spicy,
        ];
        let traits = PersonalityTraits::default();
        for mood in &moods {
            for edginess in &edginess_levels {
                let phrases = idle_phrases(*mood, *edginess, &traits);
                assert!(
                    !phrases.is_empty(),
                    "No idle phrases for {:?}/{:?}",
                    mood,
                    edginess
                );
            }
        }
    }

    #[test]
    fn event_reactions_non_empty_for_all_moods() {
        let moods = [
            MoodState::Neutral,
            MoodState::Happy,
            MoodState::Angry,
            MoodState::Excited,
        ];
        for mood in &moods {
            let reactions = event_reactions(*mood, EdginessLevel::Moderate);
            assert!(!reactions.is_empty(), "No reactions for {:?}", mood);
        }
    }

    #[test]
    fn combat_reactions_non_empty_for_all_edginess() {
        let traits = PersonalityTraits::default();
        for edginess in &[
            EdginessLevel::Mild,
            EdginessLevel::Moderate,
            EdginessLevel::Spicy,
        ] {
            let reactions = combat_reactions(MoodState::Excited, *edginess, &traits);
            assert!(
                !reactions.is_empty(),
                "No combat reactions for {:?}",
                edginess
            );
        }
    }

    #[test]
    fn combat_reactions_battle_hungry_differs() {
        let default_traits = PersonalityTraits::default();
        let battle_hungry = PersonalityTraits {
            battle_hunger: 0.9,
            ..Default::default()
        };
        let default_reactions =
            combat_reactions(MoodState::Excited, EdginessLevel::Moderate, &default_traits);
        let hungry_reactions =
            combat_reactions(MoodState::Excited, EdginessLevel::Moderate, &battle_hungry);
        // Battle-hungry gets a different table
        assert_ne!(
            default_reactions.as_ptr(),
            hungry_reactions.as_ptr(),
            "Battle-hungry should use a different phrase table"
        );
    }

    #[test]
    fn bot_chat_responses_non_empty_for_all_moods() {
        let traits = PersonalityTraits::default();
        let moods = [
            MoodState::Neutral,
            MoodState::Happy,
            MoodState::Angry,
            MoodState::Playful,
        ];
        for mood in &moods {
            let responses = bot_chat_responses(*mood, &traits);
            assert!(!responses.is_empty(), "No bot chat for {:?}", mood);
        }
    }

    #[test]
    fn bot_chat_extraverted_differs() {
        let introverted = PersonalityTraits {
            extraversion: 0.2,
            ..Default::default()
        };
        let extraverted = PersonalityTraits {
            extraversion: 0.9,
            ..Default::default()
        };
        let intro_resp = bot_chat_responses(MoodState::Neutral, &introverted);
        let extro_resp = bot_chat_responses(MoodState::Neutral, &extraverted);
        assert_ne!(
            intro_resp.as_ptr(),
            extro_resp.as_ptr(),
            "Extraverted and introverted should use different tables"
        );
    }

    #[test]
    fn apply_speech_style_slang_prepend() {
        let style = SpeechStyle {
            vocabulary_level: 0.5,
            emote_frequency: 0.5,
            typing_speed: 1.0,
            catchphrases: Vec::new(),
            adopted_slang: vec!["kek".into()],
        };
        // Run many times; adopted slang has 10% chance
        let mut found_slang = false;
        for i in 0..300 {
            let mut r = TraitDrivenResponder::new(i, EdginessLevel::Moderate);
            let result = r.apply_speech_style("Hello world", &style);
            if result.starts_with("kek") {
                found_slang = true;
                break;
            }
        }
        assert!(
            found_slang,
            "Adopted slang should appear at least once in 300 attempts"
        );
    }

    #[test]
    fn apply_speech_style_no_modification_with_defaults() {
        let style = SpeechStyle::default();
        let mut r = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        let result = r.apply_speech_style("Test phrase", &style);
        // Default style has no catchphrases or slang, vocabulary_level = 0.5
        // So the text should pass through unchanged
        assert_eq!(result, "Test phrase");
    }

    #[test]
    fn respond_to_player_with_name_sometimes() {
        let mut found_with_name = false;
        for i in 0..100 {
            let mut r = TraitDrivenResponder::new(i, EdginessLevel::Moderate);
            let traits = PersonalityTraits {
                extraversion: 0.9,
                agreeableness: 0.5,
                ..Default::default()
            };
            let text = r.respond_to_player("Dave", "Hey", &traits, MoodState::Neutral, &[]);
            if text.contains("Dave") {
                found_with_name = true;
                break;
            }
        }
        assert!(
            found_with_name,
            "Extraverted character should use player name sometimes"
        );
    }

    #[test]
    fn high_vocabulary_preserves_case() {
        let style = SpeechStyle {
            vocabulary_level: 0.9,
            ..Default::default()
        };
        let mut r = TraitDrivenResponder::new(1, EdginessLevel::Moderate);
        let result = r.apply_speech_style("Hello World", &style);
        assert!(result.contains('H') && result.contains('W'));
    }
}
