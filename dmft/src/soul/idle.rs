use dmft_common::nav::Xorshift32;
use dmft_common::soul::{IdleBehaviorType, MoodState, PersonalityTraits, SpeechStyle};

use crate::soul::config::SoulConfig;
use crate::soul::llm::fallback::TraitDrivenResponder;
use crate::soul::llm::{LlmPriority, LlmProvider, LlmRequest, Situation};
use crate::soul::personality::SoulContext;

/// An active idle behavior with its remaining duration.
#[derive(Debug, Clone)]
pub struct ActiveBehavior {
    /// The type of idle behavior being performed.
    pub behavior: IdleBehaviorType,
    /// Ticks remaining before this behavior ends.
    pub ticks_remaining: u32,
    /// Flavor text generated when the behavior started.
    pub flavor_text: Option<String>,
}

/// A behavior with its computed weight for selection.
#[derive(Debug, Clone)]
pub struct PrioritizedBehavior {
    /// The idle behavior type.
    pub behavior: IdleBehaviorType,
    /// Computed selection weight based on personality and context.
    pub weight: f32,
}

/// Transition result from the idle scheduler.
#[derive(Debug)]
pub enum IdleTransition {
    /// Continue the current behavior.
    Continue,
    /// Start a new behavior, replacing the current one.
    Start(ActiveBehavior),
    /// Stop idling entirely (e.g., combat started).
    Stop,
    /// `LogOffToSleep`: character "logs off" — remove from active rotation.
    LogOff {
        /// Seconds until the character returns from the "logged off" state.
        return_after_secs: u64,
    },
}

/// Drives idle behavior selection and timing for a single character.
pub struct IdleScheduler {
    rng: Xorshift32,
    current: Option<ActiveBehavior>,
    /// Ticks since last behavior change — used to detect "stuck" idle.
    ticks_idle: u32,
    /// Minimum ticks for a behavior duration.
    min_duration_ticks: u32,
    /// Maximum ticks for a behavior duration.
    max_duration_ticks: u32,
}

impl IdleScheduler {
    /// Create a new idle scheduler seeded from the client ID.
    #[must_use]
    pub fn new(client_id: u32, config: &SoulConfig) -> Self {
        // Convert config seconds to ticks (soul tick = 5s by default)
        let tick_secs = config.idle_tick_secs.max(1);
        let min_ticks = (config.min_chat_interval_secs / tick_secs).max(1) as u32;
        let max_ticks =
            (config.max_chat_interval_secs / tick_secs).max(u64::from(min_ticks) + 1u64) as u32;

        Self {
            rng: Xorshift32::from_client_id(client_id.wrapping_mul(7919)),
            current: None,
            ticks_idle: 0,
            min_duration_ticks: min_ticks,
            max_duration_ticks: max_ticks,
        }
    }

    /// Called each soul tick. Returns what the scheduler wants to do.
    pub fn tick(
        &mut self,
        ctx: &SoulContext<'_>,
        responder: &mut TraitDrivenResponder,
    ) -> IdleTransition {
        // If in combat, stop idling
        if ctx.in_combat {
            self.current = None;
            self.ticks_idle = 0;
            return IdleTransition::Stop;
        }

        self.ticks_idle += 1;

        // If we have an active behavior, tick it down
        if let Some(ref mut active) = self.current
            && active.ticks_remaining > 0
        {
            active.ticks_remaining -= 1;
            return IdleTransition::Continue;
        }

        // Pick a new behavior
        let weights = self.compute_weights(ctx);
        let behavior = self.weighted_select(&weights);

        // Handle LogOffToSleep special case
        if behavior == IdleBehaviorType::LogOffToSleep {
            let sleep_duration = self.sleep_duration(ctx.traits);
            self.current = None;
            self.ticks_idle = 0;
            return IdleTransition::LogOff {
                return_after_secs: sleep_duration,
            };
        }

        let duration = self.random_duration(&behavior, ctx.traits);
        let flavor_text = self.generate_flavor_text(&behavior, ctx, responder);

        let active = ActiveBehavior {
            behavior,
            ticks_remaining: duration,
            flavor_text,
        };

        self.current = Some(active.clone());
        self.ticks_idle = 0;

        IdleTransition::Start(active)
    }

    /// Get the current active behavior, if any.
    #[must_use]
    pub fn current_behavior(&self) -> Option<&ActiveBehavior> {
        self.current.as_ref()
    }

    /// Force-stop the current behavior (e.g., combat started externally).
    pub fn interrupt(&mut self) {
        self.current = None;
        self.ticks_idle = 0;
    }

    /// Compute weighted behavior list using personality engine's `idle_weights`.
    fn compute_weights(&self, ctx: &SoulContext<'_>) -> Vec<PrioritizedBehavior> {
        let t = ctx.traits;
        let mood = ctx.mood;

        let mut weights = vec![
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Sit,
                weight: 1.0 - t.extraversion * 0.5,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Wander,
                weight: t.wanderlust * 0.8 + t.extraversion * 0.2,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Emote,
                weight: t.extraversion * 0.6 + t.mischief * 0.4,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Fish,
                weight: t.conscientiousness * 0.7 * (1.0 - t.battle_hunger * 0.5),
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Craft,
                weight: t.conscientiousness * 0.5 + t.openness * 0.3,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::VendorBrowse,
                weight: t.greed * 0.8,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::LoreChatter,
                weight: t.openness * 0.5 + t.piety * 0.3,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::BioBrk,
                weight: 0.1 + t.conscientiousness * 0.1,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::LogOffToSleep,
                weight: if mood == MoodState::Exhausted {
                    0.8
                } else {
                    0.02
                },
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::RandomJump,
                weight: t.mischief * 0.6,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Inspect,
                weight: t.openness * 0.4 + t.extraversion * 0.3,
            },
        ];

        // Mood modifiers
        match mood {
            MoodState::Bored => {
                adjust(&mut weights, &IdleBehaviorType::Wander, 1.5);
                adjust(&mut weights, &IdleBehaviorType::RandomJump, 2.0);
                adjust(&mut weights, &IdleBehaviorType::Fish, 1.3);
            }
            MoodState::Happy | MoodState::Playful => {
                adjust(&mut weights, &IdleBehaviorType::Emote, 1.8);
                adjust(&mut weights, &IdleBehaviorType::LoreChatter, 1.5);
            }
            MoodState::Anxious => {
                adjust(&mut weights, &IdleBehaviorType::Sit, 1.5);
                adjust(&mut weights, &IdleBehaviorType::Wander, 0.3);
            }
            MoodState::Melancholy => {
                adjust(&mut weights, &IdleBehaviorType::Sit, 2.0);
                adjust(&mut weights, &IdleBehaviorType::Fish, 1.5);
                adjust(&mut weights, &IdleBehaviorType::Emote, 0.5);
            }
            MoodState::Focused => {
                adjust(&mut weights, &IdleBehaviorType::Sit, 1.5);
                adjust(&mut weights, &IdleBehaviorType::Wander, 0.2);
                adjust(&mut weights, &IdleBehaviorType::RandomJump, 0.1);
            }
            MoodState::Exhausted => {
                adjust(&mut weights, &IdleBehaviorType::Sit, 3.0);
                adjust(&mut weights, &IdleBehaviorType::LogOffToSleep, 3.0);
                adjust(&mut weights, &IdleBehaviorType::Wander, 0.1);
            }
            _ => {}
        }

        // Increase variety pressure the longer we've been idle
        if self.ticks_idle > 10 {
            adjust(&mut weights, &IdleBehaviorType::Wander, 1.3);
            adjust(&mut weights, &IdleBehaviorType::Emote, 1.2);
        }

        weights
    }

    /// Weighted random selection using Xorshift32.
    fn weighted_select(&mut self, weights: &[PrioritizedBehavior]) -> IdleBehaviorType {
        let total: f32 = weights.iter().map(|w| w.weight).sum();
        let mut roll = self.rng.next_f32() * total;
        for pb in weights {
            roll -= pb.weight;
            if roll <= 0.0 {
                return pb.behavior.clone();
            }
        }
        IdleBehaviorType::Sit
    }

    /// Random duration in ticks, bounded by config min/max.
    /// Some behaviors have personality-dependent duration scaling.
    fn random_duration(&mut self, behavior: &IdleBehaviorType, traits: &PersonalityTraits) -> u32 {
        let range = self.max_duration_ticks - self.min_duration_ticks;
        let base = self.min_duration_ticks + (self.rng.next_u32() % range.max(1));

        // Scale duration by personality
        let scale = match behavior {
            IdleBehaviorType::Sit => 1.0 + (1.0 - traits.extraversion) * 0.5,
            IdleBehaviorType::Wander => 0.5 + traits.wanderlust * 0.5,
            IdleBehaviorType::Fish => 1.0 + traits.conscientiousness * 1.0,
            IdleBehaviorType::Craft => 0.8 + traits.conscientiousness * 0.8,
            IdleBehaviorType::BioBrk => 0.3 + self.rng.next_f32() * 0.4,
            IdleBehaviorType::RandomJump => 0.1 + self.rng.next_f32() * 0.2,
            IdleBehaviorType::Emote => 0.2 + self.rng.next_f32() * 0.3,
            IdleBehaviorType::Inspect => 0.3 + traits.openness * 0.4,
            IdleBehaviorType::VendorBrowse => 0.5 + traits.greed * 0.5,
            IdleBehaviorType::LoreChatter => 0.4 + traits.openness * 0.4,
            IdleBehaviorType::LogOffToSleep => 1.0, // handled by sleep_duration
        };

        ((base as f32 * scale) as u32).max(1)
    }

    /// How long a character sleeps (in seconds) based on personality.
    fn sleep_duration(&mut self, traits: &PersonalityTraits) -> u64 {
        // Base: 20-60 minutes
        // Conscientious characters sleep shorter, lazy ones sleep longer
        let base_secs = 1200.0 + (1.0 - traits.conscientiousness) * 2400.0;
        let jitter = (self.rng.next_f32() - 0.5) * 600.0;
        (base_secs + jitter).max(600.0) as u64
    }

    /// Generate flavor text for the behavior start using the `TraitDrivenResponder`.
    fn generate_flavor_text(
        &mut self,
        behavior: &IdleBehaviorType,
        ctx: &SoulContext<'_>,
        responder: &mut TraitDrivenResponder,
    ) -> Option<String> {
        // Not all behaviors need flavor text
        let needs_text = matches!(
            behavior,
            IdleBehaviorType::LoreChatter
                | IdleBehaviorType::Emote
                | IdleBehaviorType::BioBrk
                | IdleBehaviorType::Fish
                | IdleBehaviorType::VendorBrowse
        );

        if !needs_text {
            return None;
        }

        let request = LlmRequest {
            character_name: ctx.character_name.to_string(),
            traits: ctx.traits.clone(),
            mood: ctx.mood,
            speech_style: SpeechStyle::default(),
            situation: Situation::IdleChatter,
            priority: LlmPriority::Low,
            memory_context: Vec::new(),
            backstory: String::new(),
        };

        match responder.generate(&request) {
            Ok(response) => Some(response.text),
            Err(_) => None,
        }
    }
}

fn adjust(weights: &mut [PrioritizedBehavior], target: &IdleBehaviorType, multiplier: f32) {
    for pb in weights.iter_mut() {
        if &pb.behavior == target {
            pb.weight *= multiplier;
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soul::config::{EdginessLevel, SoulConfig};
    use crate::soul::llm::fallback::TraitDrivenResponder;
    use crate::soul::personality::SoulContext;
    use dmft_common::soul::{IdleBehaviorType, MoodState, PersonalityTraits};

    fn default_config() -> SoulConfig {
        SoulConfig::default()
    }

    fn make_ctx<'a>(
        traits: &'a PersonalityTraits,
        mood: MoodState,
        in_combat: bool,
    ) -> SoulContext<'a> {
        SoulContext {
            character_name: "TestChar",
            traits,
            mood,
            edginess: EdginessLevel::Moderate,
            zone: "freportn",
            level: 50,
            in_combat,
            group_members: &[],
        }
    }

    #[test]
    fn new_scheduler_has_no_active_behavior() {
        let config = default_config();
        let scheduler = IdleScheduler::new(1, &config);
        assert!(scheduler.current_behavior().is_none());
    }

    #[test]
    fn tick_selects_behavior_when_none_active() {
        let config = default_config();
        let mut scheduler = IdleScheduler::new(1, &config);
        let traits = PersonalityTraits::default();
        let ctx = make_ctx(&traits, MoodState::Neutral, false);
        let mut responder = TraitDrivenResponder::new(1, EdginessLevel::Moderate);

        let transition = scheduler.tick(&ctx, &mut responder);
        match transition {
            IdleTransition::Start(active) => {
                assert!(active.ticks_remaining > 0);
            }
            IdleTransition::LogOff { .. } => {
                // LogOffToSleep is valid but rare
            }
            other => panic!("Expected Start or LogOff, got {:?}", other),
        }
    }

    #[test]
    fn tick_in_combat_returns_stop() {
        let config = default_config();
        let mut scheduler = IdleScheduler::new(1, &config);
        let traits = PersonalityTraits::default();
        let ctx = make_ctx(&traits, MoodState::Neutral, true);
        let mut responder = TraitDrivenResponder::new(1, EdginessLevel::Moderate);

        let transition = scheduler.tick(&ctx, &mut responder);
        assert!(matches!(transition, IdleTransition::Stop));
    }

    #[test]
    fn active_behavior_decrements_ticks_remaining() {
        let config = default_config();
        let mut scheduler = IdleScheduler::new(1, &config);
        let traits = PersonalityTraits::default();
        let ctx = make_ctx(&traits, MoodState::Neutral, false);
        let mut responder = TraitDrivenResponder::new(1, EdginessLevel::Moderate);

        // First tick: start a behavior
        let first = scheduler.tick(&ctx, &mut responder);
        let initial_ticks = match &first {
            IdleTransition::Start(active) => active.ticks_remaining,
            IdleTransition::LogOff { .. } => return, // skip if LogOff was selected
            other => panic!("Expected Start, got {:?}", other),
        };

        if initial_ticks > 0 {
            // Second tick: should continue and decrement
            let second = scheduler.tick(&ctx, &mut responder);
            assert!(matches!(second, IdleTransition::Continue));

            let remaining = scheduler.current_behavior().unwrap().ticks_remaining;
            assert_eq!(remaining, initial_ticks - 1);
        }
    }

    #[test]
    fn interrupt_clears_current_behavior() {
        let config = default_config();
        let mut scheduler = IdleScheduler::new(1, &config);
        let traits = PersonalityTraits::default();
        let ctx = make_ctx(&traits, MoodState::Neutral, false);
        let mut responder = TraitDrivenResponder::new(1, EdginessLevel::Moderate);

        // Start a behavior
        scheduler.tick(&ctx, &mut responder);
        // Interrupt it
        scheduler.interrupt();
        assert!(scheduler.current_behavior().is_none());
    }

    #[test]
    fn exhausted_mood_heavily_weights_sit_and_logoff() {
        let config = default_config();
        let scheduler = IdleScheduler::new(1, &config);
        let traits = PersonalityTraits::default();
        let ctx = make_ctx(&traits, MoodState::Exhausted, false);

        let weights = scheduler.compute_weights(&ctx);

        let sit_weight = weights
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Sit)
            .unwrap()
            .weight;
        let logoff_weight = weights
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::LogOffToSleep)
            .unwrap()
            .weight;
        let wander_weight = weights
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Wander)
            .unwrap()
            .weight;

        // Exhausted should heavily favor sitting/sleeping over wandering
        assert!(sit_weight > wander_weight);
        assert!(logoff_weight > wander_weight);
    }

    #[test]
    fn bored_mood_boosts_wander_and_random_jump() {
        let config = default_config();
        let scheduler = IdleScheduler::new(1, &config);
        let traits = PersonalityTraits {
            wanderlust: 0.5,
            mischief: 0.5,
            ..Default::default()
        };
        let neutral_ctx = make_ctx(&traits, MoodState::Neutral, false);
        let bored_ctx = make_ctx(&traits, MoodState::Bored, false);

        let neutral_weights = scheduler.compute_weights(&neutral_ctx);
        let bored_weights = scheduler.compute_weights(&bored_ctx);

        let neutral_wander = neutral_weights
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Wander)
            .unwrap()
            .weight;
        let bored_wander = bored_weights
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Wander)
            .unwrap()
            .weight;

        assert!(bored_wander > neutral_wander);
    }

    #[test]
    fn wanderlust_trait_increases_wander_weight() {
        let config = default_config();
        let scheduler = IdleScheduler::new(1, &config);

        let low_wanderlust = PersonalityTraits {
            wanderlust: 0.1,
            ..Default::default()
        };
        let high_wanderlust = PersonalityTraits {
            wanderlust: 0.9,
            ..Default::default()
        };

        let low_ctx = make_ctx(&low_wanderlust, MoodState::Neutral, false);
        let high_ctx = make_ctx(&high_wanderlust, MoodState::Neutral, false);

        let low_weights = scheduler.compute_weights(&low_ctx);
        let high_weights = scheduler.compute_weights(&high_ctx);

        let low_wander = low_weights
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Wander)
            .unwrap()
            .weight;
        let high_wander = high_weights
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Wander)
            .unwrap()
            .weight;

        assert!(high_wander > low_wander);
    }

    #[test]
    fn weighted_select_always_returns_valid_behavior() {
        let config = default_config();
        let mut scheduler = IdleScheduler::new(42, &config);

        let weights = vec![
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Sit,
                weight: 1.0,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Wander,
                weight: 1.0,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Emote,
                weight: 1.0,
            },
        ];

        for _ in 0..20 {
            let selected = scheduler.weighted_select(&weights);
            assert!(
                selected == IdleBehaviorType::Sit
                    || selected == IdleBehaviorType::Wander
                    || selected == IdleBehaviorType::Emote
            );
        }
    }

    #[test]
    fn adjust_multiplies_target_weight() {
        let mut weights = vec![
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Sit,
                weight: 1.0,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Wander,
                weight: 2.0,
            },
        ];
        adjust(&mut weights, &IdleBehaviorType::Wander, 3.0);
        assert!((weights[1].weight - 6.0).abs() < f32::EPSILON);
        // Sit should be unchanged
        assert!((weights[0].weight - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn adjust_noop_for_missing_target() {
        let mut weights = vec![PrioritizedBehavior {
            behavior: IdleBehaviorType::Sit,
            weight: 1.0,
        }];
        adjust(&mut weights, &IdleBehaviorType::Wander, 5.0);
        assert!((weights[0].weight - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn happy_mood_boosts_emote_and_lore() {
        let config = default_config();
        let scheduler = IdleScheduler::new(1, &config);
        let traits = PersonalityTraits {
            extraversion: 0.5,
            openness: 0.5,
            piety: 0.5,
            ..Default::default()
        };
        let neutral_ctx = make_ctx(&traits, MoodState::Neutral, false);
        let happy_ctx = make_ctx(&traits, MoodState::Happy, false);

        let neutral_w = scheduler.compute_weights(&neutral_ctx);
        let happy_w = scheduler.compute_weights(&happy_ctx);

        let neutral_emote = neutral_w
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Emote)
            .unwrap()
            .weight;
        let happy_emote = happy_w
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Emote)
            .unwrap()
            .weight;
        assert!(happy_emote > neutral_emote);
    }

    #[test]
    fn anxious_mood_reduces_wander() {
        let config = default_config();
        let scheduler = IdleScheduler::new(1, &config);
        let traits = PersonalityTraits {
            wanderlust: 0.5,
            ..Default::default()
        };
        let neutral_ctx = make_ctx(&traits, MoodState::Neutral, false);
        let anxious_ctx = make_ctx(&traits, MoodState::Anxious, false);

        let neutral_w = scheduler.compute_weights(&neutral_ctx);
        let anxious_w = scheduler.compute_weights(&anxious_ctx);

        let neutral_wander = neutral_w
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Wander)
            .unwrap()
            .weight;
        let anxious_wander = anxious_w
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Wander)
            .unwrap()
            .weight;
        assert!(anxious_wander < neutral_wander);
    }

    #[test]
    fn melancholy_mood_boosts_sit_and_fish() {
        let config = default_config();
        let scheduler = IdleScheduler::new(1, &config);
        let traits = PersonalityTraits {
            conscientiousness: 0.5,
            ..Default::default()
        };
        let neutral_ctx = make_ctx(&traits, MoodState::Neutral, false);
        let mel_ctx = make_ctx(&traits, MoodState::Melancholy, false);

        let neutral_w = scheduler.compute_weights(&neutral_ctx);
        let mel_w = scheduler.compute_weights(&mel_ctx);

        let neutral_sit = neutral_w
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Sit)
            .unwrap()
            .weight;
        let mel_sit = mel_w
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Sit)
            .unwrap()
            .weight;
        assert!(mel_sit > neutral_sit);
    }

    #[test]
    fn focused_mood_reduces_wander_and_jump() {
        let config = default_config();
        let scheduler = IdleScheduler::new(1, &config);
        let traits = PersonalityTraits {
            wanderlust: 0.5,
            mischief: 0.5,
            ..Default::default()
        };
        let neutral_ctx = make_ctx(&traits, MoodState::Neutral, false);
        let focused_ctx = make_ctx(&traits, MoodState::Focused, false);

        let neutral_w = scheduler.compute_weights(&neutral_ctx);
        let focused_w = scheduler.compute_weights(&focused_ctx);

        let neutral_wander = neutral_w
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Wander)
            .unwrap()
            .weight;
        let focused_wander = focused_w
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Wander)
            .unwrap()
            .weight;
        assert!(focused_wander < neutral_wander);

        let neutral_jump = neutral_w
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::RandomJump)
            .unwrap()
            .weight;
        let focused_jump = focused_w
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::RandomJump)
            .unwrap()
            .weight;
        assert!(focused_jump < neutral_jump);
    }

    #[test]
    fn long_idle_boosts_variety() {
        let config = default_config();
        let mut scheduler = IdleScheduler::new(1, &config);
        let traits = PersonalityTraits::default();
        let ctx = make_ctx(&traits, MoodState::Neutral, false);

        let fresh_weights = scheduler.compute_weights(&ctx);
        let fresh_wander = fresh_weights
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Wander)
            .unwrap()
            .weight;

        // Simulate being idle for >10 ticks
        scheduler.ticks_idle = 15;
        let stale_weights = scheduler.compute_weights(&ctx);
        let stale_wander = stale_weights
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::Wander)
            .unwrap()
            .weight;

        assert!(stale_wander > fresh_wander);
    }

    #[test]
    fn random_duration_returns_at_least_one() {
        let config = default_config();
        let mut scheduler = IdleScheduler::new(1, &config);
        let traits = PersonalityTraits::default();
        for behavior in &[
            IdleBehaviorType::Sit,
            IdleBehaviorType::Wander,
            IdleBehaviorType::Fish,
            IdleBehaviorType::Craft,
            IdleBehaviorType::BioBrk,
            IdleBehaviorType::RandomJump,
            IdleBehaviorType::Emote,
            IdleBehaviorType::Inspect,
            IdleBehaviorType::VendorBrowse,
            IdleBehaviorType::LoreChatter,
            IdleBehaviorType::LogOffToSleep,
        ] {
            let d = scheduler.random_duration(behavior, &traits);
            assert!(d >= 1, "duration for {:?} was {}", behavior, d);
        }
    }

    #[test]
    fn sleep_duration_minimum_600_seconds() {
        let config = default_config();
        let mut scheduler = IdleScheduler::new(1, &config);
        // Even with extreme traits, sleep should be at least 600s
        for i in 0..20 {
            let traits = PersonalityTraits {
                conscientiousness: if i % 2 == 0 { 1.0 } else { 0.0 },
                ..Default::default()
            };
            let dur = scheduler.sleep_duration(&traits);
            assert!(dur >= 600, "sleep_duration was {} (< 600)", dur);
        }
    }

    #[test]
    fn weighted_select_single_weight() {
        let config = default_config();
        let mut scheduler = IdleScheduler::new(1, &config);
        let weights = vec![PrioritizedBehavior {
            behavior: IdleBehaviorType::Fish,
            weight: 1.0,
        }];
        for _ in 0..10 {
            assert_eq!(scheduler.weighted_select(&weights), IdleBehaviorType::Fish);
        }
    }

    #[test]
    fn active_behavior_debug_and_clone() {
        let active = ActiveBehavior {
            behavior: IdleBehaviorType::Wander,
            ticks_remaining: 5,
            flavor_text: Some("Walking around...".into()),
        };
        let cloned = active.clone();
        assert_eq!(cloned.ticks_remaining, 5);
        assert_eq!(cloned.flavor_text.as_deref(), Some("Walking around..."));
        let _ = format!("{:?}", cloned);
    }

    #[test]
    fn idle_transition_debug() {
        let transitions = [
            IdleTransition::Continue,
            IdleTransition::Stop,
            IdleTransition::LogOff {
                return_after_secs: 1200,
            },
            IdleTransition::Start(ActiveBehavior {
                behavior: IdleBehaviorType::Sit,
                ticks_remaining: 3,
                flavor_text: None,
            }),
        ];
        for t in &transitions {
            let _ = format!("{:?}", t);
        }
    }

    #[test]
    fn prioritized_behavior_debug_and_clone() {
        let pb = PrioritizedBehavior {
            behavior: IdleBehaviorType::Craft,
            weight: 0.42,
        };
        let c = pb.clone();
        assert!((c.weight - 0.42).abs() < f32::EPSILON);
        let _ = format!("{:?}", c);
    }

    #[test]
    fn scheduler_config_tick_conversion() {
        // With 10s ticks, 60s min / 300s max → 6 / 30 ticks
        let config = SoulConfig {
            idle_tick_secs: 10,
            min_chat_interval_secs: 60,
            max_chat_interval_secs: 300,
            ..Default::default()
        };
        let scheduler = IdleScheduler::new(1, &config);
        assert_eq!(scheduler.min_duration_ticks, 6);
        assert_eq!(scheduler.max_duration_ticks, 30);
    }

    #[test]
    fn scheduler_config_prevents_zero_tick_secs() {
        // idle_tick_secs = 0 should be clamped to 1
        let config = SoulConfig {
            idle_tick_secs: 0,
            min_chat_interval_secs: 10,
            max_chat_interval_secs: 20,
            ..Default::default()
        };
        let scheduler = IdleScheduler::new(1, &config);
        assert!(scheduler.min_duration_ticks >= 1);
        assert!(scheduler.max_duration_ticks > scheduler.min_duration_ticks);
    }

    #[test]
    fn greed_trait_boosts_vendor_browse() {
        let config = default_config();
        let scheduler = IdleScheduler::new(1, &config);

        let low_greed = PersonalityTraits {
            greed: 0.1,
            ..Default::default()
        };
        let high_greed = PersonalityTraits {
            greed: 0.9,
            ..Default::default()
        };

        let low_ctx = make_ctx(&low_greed, MoodState::Neutral, false);
        let high_ctx = make_ctx(&high_greed, MoodState::Neutral, false);

        let low_w = scheduler.compute_weights(&low_ctx);
        let high_w = scheduler.compute_weights(&high_ctx);

        let low_vendor = low_w
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::VendorBrowse)
            .unwrap()
            .weight;
        let high_vendor = high_w
            .iter()
            .find(|w| w.behavior == IdleBehaviorType::VendorBrowse)
            .unwrap()
            .weight;
        assert!(high_vendor > low_vendor);
    }
}
