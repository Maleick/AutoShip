use dmft_common::nav::Xorshift32;
use dmft_common::soul::{
    IdleBehaviorType, MoodState, PersonalityTraits, SoulAction, SoulEvent, SayChannel,
};

use crate::soul::config::{EdginessLevel, SoulConfig};

/// Snapshot of a character's current soul state, passed to engine methods.
pub struct SoulContext<'a> {
    pub character_name: &'a str,
    pub traits: &'a PersonalityTraits,
    pub mood: MoodState,
    pub edginess: EdginessLevel,
    pub zone: &'a str,
    pub level: u8,
    pub in_combat: bool,
    pub group_members: &'a [String],
}

/// Deterministic personality engine (Phase 1 — no LLM).
/// Uses trait vectors + mood state to drive emotes, chat, and idle behavior.
pub struct PersonalityEngine {
    rng: Xorshift32,
}

impl PersonalityEngine {
    pub fn new(client_id: u32) -> Self {
        Self {
            rng: Xorshift32::from_client_id(client_id),
        }
    }

    /// Process a soul event and return the new mood state.
    pub fn process_event(
        &mut self,
        current_mood: MoodState,
        event: &SoulEvent,
        traits: &PersonalityTraits,
    ) -> MoodState {
        match event {
            // Death → anxious (high neuroticism) or angry (low neuroticism)
            SoulEvent::Death { .. } => {
                if traits.neuroticism > 0.6 {
                    MoodState::Anxious
                } else if traits.battle_hunger > 0.6 {
                    MoodState::Angry
                } else {
                    MoodState::Melancholy
                }
            }
            // Kill → excited (battle hungry) or focused
            SoulEvent::Kill { .. } => {
                if traits.battle_hunger > 0.7 {
                    MoodState::Excited
                } else if traits.conscientiousness > 0.6 {
                    MoodState::Focused
                } else {
                    current_mood
                }
            }
            // Loot → happy (greedy) or playful
            SoulEvent::Loot { .. } => {
                if traits.greed > 0.6 {
                    MoodState::Happy
                } else if traits.mischief > 0.5 {
                    MoodState::Playful
                } else {
                    current_mood
                }
            }
            // Player chat → mood based on sentiment
            SoulEvent::PlayerChat { sentiment, .. } => {
                if *sentiment > 0.5 {
                    if traits.extraversion > 0.6 {
                        MoodState::Excited
                    } else {
                        MoodState::Happy
                    }
                } else if *sentiment < -0.3 {
                    if traits.agreeableness < 0.4 {
                        MoodState::Angry
                    } else if traits.neuroticism > 0.6 {
                        MoodState::Anxious
                    } else {
                        MoodState::Melancholy
                    }
                } else {
                    current_mood
                }
            }
            // Bot chat → playful (extraverted) or no change
            SoulEvent::BotChat { .. } => {
                if traits.extraversion > 0.7 {
                    MoodState::Playful
                } else {
                    current_mood
                }
            }
            // Zone enter → excited (wanderlust) or anxious (neurotic)
            SoulEvent::ZoneEnter { .. } => {
                if traits.wanderlust > 0.7 {
                    MoodState::Excited
                } else if traits.neuroticism > 0.7 && traits.openness < 0.4 {
                    MoodState::Anxious
                } else {
                    current_mood
                }
            }
            // Level up → excited or happy
            SoulEvent::LevelUp { .. } => {
                if traits.battle_hunger > 0.5 || traits.conscientiousness > 0.5 {
                    MoodState::Excited
                } else {
                    MoodState::Happy
                }
            }
            // Group wipe → anxious, angry, or melancholy
            SoulEvent::GroupWipe { .. } => {
                if traits.neuroticism > 0.7 {
                    MoodState::Anxious
                } else if traits.battle_hunger > 0.6 {
                    MoodState::Angry
                } else if traits.loyalty > 0.6 {
                    MoodState::Melancholy
                } else {
                    MoodState::Angry
                }
            }
            // Mood shift is informational — pass through
            SoulEvent::MoodShift { to, .. } => *to,
            // Witnessed → excited if open, else no change
            SoulEvent::Witnessed { .. } => {
                if traits.openness > 0.7 {
                    MoodState::Excited
                } else {
                    current_mood
                }
            }
            // Relationship change → mood based on delta magnitude
            SoulEvent::RelationshipChange { delta, .. } => {
                if *delta > 50.0 {
                    MoodState::Happy
                } else if *delta < -50.0 {
                    if traits.agreeableness > 0.6 {
                        MoodState::Melancholy
                    } else {
                        MoodState::Angry
                    }
                } else {
                    current_mood
                }
            }
        }
    }

    /// Generate an emote action appropriate for the current mood and personality.
    pub fn generate_emote(&mut self, ctx: &SoulContext<'_>) -> SoulAction {
        let emotes = emotes_for_mood(ctx.mood);
        let idx = (self.rng.next_u32() as usize) % emotes.len();
        SoulAction::Emote {
            emote: emotes[idx].to_string(),
        }
    }

    /// Generate a chat message (say/group) appropriate for mood and personality.
    pub fn generate_chat(&mut self, ctx: &SoulContext<'_>) -> SoulAction {
        let phrases = phrases_for_mood(ctx.mood, ctx.edginess);
        let idx = (self.rng.next_u32() as usize) % phrases.len();
        let message = phrases[idx].to_string();

        // Extraverted characters use /say, introverted use /group
        let channel = if ctx.traits.extraversion > 0.6 {
            SayChannel::Say
        } else {
            SayChannel::Group
        };

        SoulAction::Say {
            channel,
            message,
            target: None,
        }
    }

    /// Derive idle behavior weights from personality traits + mood.
    /// Returns (behavior, weight) pairs — caller picks based on weighted random.
    pub fn idle_weights(&self, ctx: &SoulContext<'_>) -> Vec<(IdleBehaviorType, f32)> {
        let t = ctx.traits;
        let mood = ctx.mood;

        let mut weights = vec![
            (IdleBehaviorType::Sit, 1.0 - t.extraversion * 0.5),
            (IdleBehaviorType::Wander, t.wanderlust * 0.8 + t.extraversion * 0.2),
            (IdleBehaviorType::Emote, t.extraversion * 0.6 + t.mischief * 0.4),
            (IdleBehaviorType::Fish, t.conscientiousness * 0.7 * (1.0 - t.battle_hunger * 0.5)),
            (IdleBehaviorType::Craft, t.conscientiousness * 0.5 + t.openness * 0.3),
            (IdleBehaviorType::VendorBrowse, t.greed * 0.8),
            (IdleBehaviorType::LoreChatter, t.openness * 0.5 + t.piety * 0.3),
            (IdleBehaviorType::BioBrk, 0.1 + t.conscientiousness * 0.1),
            (
                IdleBehaviorType::LogOffToSleep,
                if mood == MoodState::Exhausted { 0.8 } else { 0.02 },
            ),
            (IdleBehaviorType::RandomJump, t.mischief * 0.6),
            (IdleBehaviorType::Inspect, t.openness * 0.4 + t.extraversion * 0.3),
        ];

        // Mood modifiers
        match mood {
            MoodState::Bored => {
                adjust_weight(&mut weights, &IdleBehaviorType::Wander, 1.5);
                adjust_weight(&mut weights, &IdleBehaviorType::RandomJump, 2.0);
                adjust_weight(&mut weights, &IdleBehaviorType::Fish, 1.3);
            }
            MoodState::Happy | MoodState::Playful => {
                adjust_weight(&mut weights, &IdleBehaviorType::Emote, 1.8);
                adjust_weight(&mut weights, &IdleBehaviorType::LoreChatter, 1.5);
            }
            MoodState::Anxious => {
                adjust_weight(&mut weights, &IdleBehaviorType::Sit, 1.5);
                adjust_weight(&mut weights, &IdleBehaviorType::Wander, 0.3);
            }
            MoodState::Melancholy => {
                adjust_weight(&mut weights, &IdleBehaviorType::Sit, 2.0);
                adjust_weight(&mut weights, &IdleBehaviorType::Fish, 1.5);
                adjust_weight(&mut weights, &IdleBehaviorType::Emote, 0.5);
            }
            MoodState::Focused => {
                adjust_weight(&mut weights, &IdleBehaviorType::Sit, 1.5);
                adjust_weight(&mut weights, &IdleBehaviorType::Wander, 0.2);
                adjust_weight(&mut weights, &IdleBehaviorType::RandomJump, 0.1);
            }
            MoodState::Exhausted => {
                adjust_weight(&mut weights, &IdleBehaviorType::Sit, 3.0);
                adjust_weight(&mut weights, &IdleBehaviorType::LogOffToSleep, 3.0);
                adjust_weight(&mut weights, &IdleBehaviorType::Wander, 0.1);
            }
            _ => {}
        }

        weights
    }

    /// Pick an idle behavior using weighted random selection.
    pub fn pick_idle(&mut self, ctx: &SoulContext<'_>) -> IdleBehaviorType {
        let weights = self.idle_weights(ctx);
        let total: f32 = weights.iter().map(|(_, w)| w).sum();
        let mut roll = self.rng.next_f32() * total;
        for (behavior, weight) in &weights {
            roll -= weight;
            if roll <= 0.0 {
                return behavior.clone();
            }
        }
        IdleBehaviorType::Sit
    }

    /// Calculate chat interval in seconds based on personality.
    /// More extraverted/playful characters chat more often.
    pub fn chat_interval_secs(&mut self, config: &SoulConfig, traits: &PersonalityTraits) -> u64 {
        let chattiness = traits.extraversion * 0.5 + traits.mischief * 0.3 + traits.openness * 0.2;
        let range = config.max_chat_interval_secs - config.min_chat_interval_secs;
        // Higher chattiness → shorter interval
        let interval = config.max_chat_interval_secs as f32 - (chattiness * range as f32);
        // Add jitter
        let jitter = (self.rng.next_f32() * 0.3 - 0.15) * interval;
        (interval + jitter).max(config.min_chat_interval_secs as f32) as u64
    }
}

fn adjust_weight(weights: &mut [(IdleBehaviorType, f32)], target: &IdleBehaviorType, multiplier: f32) {
    for (behavior, weight) in weights.iter_mut() {
        if behavior == target {
            *weight *= multiplier;
            return;
        }
    }
}

// ─── Emote tables ───

fn emotes_for_mood(mood: MoodState) -> &'static [&'static str] {
    match mood {
        MoodState::Neutral => &[
            "stretches",
            "looks around",
            "adjusts equipment",
            "yawns",
            "cracks knuckles",
        ],
        MoodState::Happy => &[
            "smiles",
            "grins broadly",
            "chuckles to themselves",
            "whistles a tune",
            "does a little dance",
        ],
        MoodState::Angry => &[
            "scowls",
            "clenches fists",
            "kicks the dirt",
            "mutters angrily",
            "glares",
        ],
        MoodState::Anxious => &[
            "fidgets nervously",
            "glances around warily",
            "wrings hands",
            "shifts weight uneasily",
            "checks behind them",
        ],
        MoodState::Bored => &[
            "yawns loudly",
            "taps foot impatiently",
            "sighs",
            "picks at fingernails",
            "stares blankly",
        ],
        MoodState::Excited => &[
            "bounces on their heels",
            "rubs hands together eagerly",
            "grins from ear to ear",
            "pumps a fist",
            "can barely contain their excitement",
        ],
        MoodState::Melancholy => &[
            "stares off into the distance",
            "sighs softly",
            "looks wistful",
            "gazes at the ground",
            "sits quietly",
        ],
        MoodState::Focused => &[
            "narrows their eyes",
            "studies the surroundings intently",
            "nods to themselves",
            "takes a deep breath",
            "adjusts their stance",
        ],
        MoodState::Playful => &[
            "winks",
            "does a little jig",
            "pretends to juggle",
            "makes a funny face",
            "nudges a nearby ally",
        ],
        MoodState::Exhausted => &[
            "slumps",
            "rubs tired eyes",
            "yawns deeply",
            "leans against a wall",
            "struggles to stay awake",
        ],
    }
}

// ─── Phrase tables ───

fn phrases_for_mood(mood: MoodState, edginess: EdginessLevel) -> &'static [&'static str] {
    match (mood, edginess) {
        // ── Neutral ──
        (MoodState::Neutral, EdginessLevel::Mild) => &[
            "Nice day for adventuring.",
            "Wonder what's around the next corner.",
            "Everyone doing alright?",
            "Keeping an eye out.",
            "Steady as she goes.",
        ],
        (MoodState::Neutral, EdginessLevel::Moderate) => &[
            "Another day, another plat.",
            "Let's keep moving.",
            "Anything good drop yet?",
            "I've seen worse camps.",
            "Stay sharp, people.",
        ],
        (MoodState::Neutral, EdginessLevel::Spicy) => &[
            "This camp is deader than my last group.",
            "Somebody wake me when something drops.",
            "I didn't roll on this server to sit around.",
            "Buff check, who's slacking?",
            "At least the company's tolerable. Barely.",
        ],
        // ── Happy ──
        (MoodState::Happy, EdginessLevel::Mild) => &[
            "What a great group!",
            "This is going really well!",
            "I love this zone.",
            "Good times, good times.",
            "Couldn't ask for better company.",
        ],
        (MoodState::Happy, EdginessLevel::Moderate) => &[
            "Now we're cooking!",
            "This is more like it!",
            "Hah, that was a good pull.",
            "Ding! Oh wait, not yet.",
            "Loving this XP flow.",
        ],
        (MoodState::Happy, EdginessLevel::Spicy) => &[
            "This is what I live for!",
            "Get wrecked, mobs!",
            "I could do this all day. And I will.",
            "Who needs sleep when the XP is this good?",
            "Whoever picked this camp deserves a raise.",
        ],
        // ── Angry ──
        (MoodState::Angry, EdginessLevel::Mild) => &[
            "That wasn't ideal.",
            "We can do better than this.",
            "Focus up, everyone.",
            "Let's not let that happen again.",
        ],
        (MoodState::Angry, EdginessLevel::Moderate) => &[
            "Come on, keep it together!",
            "That pull was sloppy.",
            "We're better than this.",
            "Who pulled that? Seriously.",
        ],
        (MoodState::Angry, EdginessLevel::Spicy) => &[
            "What in Cazic's name was THAT?",
            "I swear, if we wipe again...",
            "Did the tank fall asleep?",
            "My grandma could tank better than this.",
            "I'm about two bad pulls from camping.",
        ],
        // ── Anxious ──
        (MoodState::Anxious, EdginessLevel::Mild) => &[
            "Is everyone buffed?",
            "Maybe we should pull slower.",
            "I have a bad feeling about this.",
            "Check your health, please.",
        ],
        (MoodState::Anxious, EdginessLevel::Moderate) => &[
            "Mana check before the next pull.",
            "Anyone else feeling jumpy?",
            "Last time I was here it didn't go well.",
            "Let's not get cocky.",
        ],
        (MoodState::Anxious, EdginessLevel::Spicy) => &[
            "I'm not dying here. Not today.",
            "If we wipe I'm blaming whoever pulled.",
            "My corpse is NOT staying in this zone.",
            "Please tell me the cleric is paying attention.",
        ],
        // ── Bored ──
        (MoodState::Bored, EdginessLevel::Mild) => &[
            "How much longer until we move?",
            "Anything else we could be doing?",
            "Maybe I'll go fishing.",
            "Getting a bit restless here.",
        ],
        (MoodState::Bored, EdginessLevel::Moderate) => &[
            "This camp is dead.",
            "Think I'll go check vendors.",
            "Need a bio, back in a few.",
            "We've been here forever.",
            "Can we pull faster?",
        ],
        (MoodState::Bored, EdginessLevel::Spicy) => &[
            "I'm going to die of boredom before anything kills me.",
            "ZzZzZz... oh sorry, still here.",
            "My butt is numb from sitting.",
            "Entertainment value of this camp: zero.",
            "I've had more excitement waiting for the boat.",
        ],
        // ── Excited ──
        (MoodState::Excited, EdginessLevel::Mild) => &[
            "This is wonderful!",
            "What an adventure!",
            "I can't wait to see what's next!",
            "This is why I became an adventurer!",
        ],
        (MoodState::Excited, EdginessLevel::Moderate) => &[
            "Let's GO!",
            "Oh man, this is going to be good!",
            "Best camp on the server right here!",
            "More! Pull more!",
        ],
        (MoodState::Excited, EdginessLevel::Spicy) => &[
            "SEND IT!",
            "Let's burn this place to the ground!",
            "I am FEELING it right now!",
            "Chain pull! CHAIN PULL!",
            "Who needs mana? Just keep pulling!",
        ],
        // ── Melancholy ──
        (MoodState::Melancholy, EdginessLevel::Mild) => &[
            "I miss the old days.",
            "Things were simpler back in Qeynos.",
            "Does anyone remember when...",
            "Just thinking about things.",
        ],
        (MoodState::Melancholy, EdginessLevel::Moderate) => &[
            "Remember when this zone used to be packed?",
            "Lost a good group member last week.",
            "Sometimes I wonder why we keep at this.",
            "The loot tables mock me.",
        ],
        (MoodState::Melancholy, EdginessLevel::Spicy) => &[
            "This game's gonna outlive us all.",
            "Remember when dying meant something?",
            "Spent more time here than with my family. Worth it.",
            "I've seen things... terrible wipes in Lower Guk...",
        ],
        // ── Focused ──
        (MoodState::Focused, EdginessLevel::Mild) => &[
            "Stay alert.",
            "Let's keep the pace.",
            "Good work, everyone.",
            "Eyes on the camp.",
        ],
        (MoodState::Focused, EdginessLevel::Moderate) => &[
            "On task. Let's go.",
            "Save the chat for after the named.",
            "Mana's good, keep pulling.",
            "Tight pulls, tight heals.",
        ],
        (MoodState::Focused, EdginessLevel::Spicy) => &[
            "Shut up and DPS.",
            "Less talking, more killing.",
            "Save the life story for after we ding.",
            "Focus or wipe. Your call.",
        ],
        // ── Playful ──
        (MoodState::Playful, EdginessLevel::Mild) => &[
            "Hehe, watch this!",
            "Bet I can out-DPS the tank.",
            "Anyone want to race to the zone line?",
            "Duck duck goose?",
        ],
        (MoodState::Playful, EdginessLevel::Moderate) => &[
            "I dare someone to pull two.",
            "Last one to ding buys the port!",
            "Plot twist: I'm actually a bard.",
            "Hold my ale and watch this.",
        ],
        (MoodState::Playful, EdginessLevel::Spicy) => &[
            "I'm going to train the whole zone. Kidding. Maybe.",
            "What if I just... pulled everything?",
            "The floor is lava! Everyone levitate!",
            "Leroy Jenkins would be proud of that pull.",
            "Accidental AoE? That was ARTISTIC AoE.",
        ],
        // ── Exhausted ──
        (MoodState::Exhausted, EdginessLevel::Mild) => &[
            "Getting a bit tired.",
            "Maybe we should take a break.",
            "My eyes are getting heavy.",
            "One more pull, then rest?",
        ],
        (MoodState::Exhausted, EdginessLevel::Moderate) => &[
            "Running on fumes here.",
            "I need to crash soon.",
            "Bio and then maybe call it?",
            "My fingers are going numb.",
        ],
        (MoodState::Exhausted, EdginessLevel::Spicy) => &[
            "If I fall asleep at the keyboard, just rez me later.",
            "We've been at this so long I forgot what daylight looks like.",
            "My coffee gave up on me three hours ago.",
            "I'm one bad pull from logging to bed.",
        ],
    }
}
