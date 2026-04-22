//! Personality trait drift — gradual changes to `PersonalityTraits` driven by
//! in-game experiences.
//!
//! # Design
//!
//! Each `SoulEvent` nudges one or more traits by a small per-event delta
//! (default 0.01 for major events, 0.005 for minor). The engine enforces:
//!
//! 1. **Per-event clamping** — each trait stays in `[0.0, 1.0]`.
//! 2. **Per-session cap** — each trait can move at most `MAX_SESSION_DELTA`
//!    (0.15) from its baseline during a single session, preventing rapid
//!    whiplash from repeated identical events.
//!
//! `DriftEngine` is cheap to clone and requires no I/O; persistence is handled
//! by the caller (coordinator checkpoints to SQLite every N ticks).

use textquest_common::soul::{PersonalityTraits, SoulEvent};

/// Maximum absolute change a trait is allowed to accumulate in one session.
pub const MAX_SESSION_DELTA: f32 = 0.15;

/// Tracks how much each trait has drifted from its session-start baseline,
/// enforcing the per-session cap.
#[derive(Debug, Clone, Default)]
pub struct SessionDriftAccumulator {
    pub openness: f32,
    pub conscientiousness: f32,
    pub extraversion: f32,
    pub agreeableness: f32,
    pub neuroticism: f32,
    pub battle_hunger: f32,
    pub greed: f32,
    pub wanderlust: f32,
    pub loyalty: f32,
    pub mischief: f32,
}

/// Stateless drift engine — apply per-event deltas while respecting caps.
#[derive(Debug, Clone, Default)]
pub struct DriftEngine {
    /// Accumulated per-session drift.
    pub session: SessionDriftAccumulator,
    /// Multiplier applied to every delta (from `SoulConfig::trait_drift_multiplier`).
    pub multiplier: f32,
}

impl DriftEngine {
    /// Create a new engine with the given per-event multiplier (1.0 = default).
    #[must_use]
    pub fn new(multiplier: f32) -> Self {
        Self {
            session: SessionDriftAccumulator::default(),
            multiplier,
        }
    }

    /// Apply trait drift for a single `SoulEvent`.
    ///
    /// Mutates `traits` in place; updates the session accumulator so the
    /// per-session cap is tracked correctly.
    pub fn apply(&mut self, traits: &mut PersonalityTraits, event: &SoulEvent) {
        match event {
            // Death → +neuroticism (anxiety from being killed)
            SoulEvent::Death { .. } => {
                self.nudge(traits, TraitField::Neuroticism, 0.01);
            }

            // Kill → +battle_hunger (combat participation)
            SoulEvent::Kill { .. } => {
                self.nudge(traits, TraitField::BattleHunger, 0.005);
            }

            // Loot → +greed (consistent looting)
            SoulEvent::Loot { .. } => {
                self.nudge(traits, TraitField::Greed, 0.01);
            }

            // Zone enter → +openness, +wanderlust (exploration)
            SoulEvent::ZoneEnter { .. } => {
                self.nudge(traits, TraitField::Openness, 0.01);
                self.nudge(traits, TraitField::Wanderlust, 0.01);
            }

            // Positive player chat → +extraversion (social reward)
            // Negative player chat → -agreeableness (hostility wears down trust)
            SoulEvent::PlayerChat { sentiment, .. } => {
                if *sentiment > 0.3 {
                    self.nudge(traits, TraitField::Extraversion, 0.01);
                } else if *sentiment < -0.3 {
                    self.nudge(traits, TraitField::Agreeableness, -0.01);
                }
            }

            // Group wipe → +neuroticism (trauma from wiping)
            SoulEvent::GroupWipe { .. } => {
                self.nudge(traits, TraitField::Neuroticism, 0.01);
            }

            // Relationship growth → +loyalty (bonds deepen)
            SoulEvent::RelationshipChange { delta, .. } if *delta > 0.0 => {
                self.nudge(traits, TraitField::Loyalty, 0.01);
            }

            // Bot interaction → +agreeableness, +conscientiousness
            // (proxy for mentoring / cooperative bot behaviour)
            SoulEvent::BotChat { .. } => {
                self.nudge(traits, TraitField::Agreeableness, 0.005);
                self.nudge(traits, TraitField::Conscientiousness, 0.005);
            }

            // Level up → +conscientiousness (effort pays off)
            SoulEvent::LevelUp { .. } => {
                self.nudge(traits, TraitField::Conscientiousness, 0.01);
            }

            // MoodShift / other events — no trait drift
            _ => {}
        }
    }

    /// Apply one decay tick — nudge all traits 1% toward 0.5 (baseline).
    ///
    /// This is the "per-day" decay described in the issue spec. Callers
    /// should invoke this once per simulated day (or on a configurable
    /// tick interval).
    pub fn decay_toward_baseline(&mut self, traits: &mut PersonalityTraits, rate: f32) {
        decay_field(&mut traits.openness, rate);
        decay_field(&mut traits.conscientiousness, rate);
        decay_field(&mut traits.extraversion, rate);
        decay_field(&mut traits.agreeableness, rate);
        decay_field(&mut traits.neuroticism, rate);
        decay_field(&mut traits.battle_hunger, rate);
        decay_field(&mut traits.greed, rate);
        decay_field(&mut traits.wanderlust, rate);
        decay_field(&mut traits.loyalty, rate);
        decay_field(&mut traits.mischief, rate);
    }

    // --- internals -------------------------------------------------------

    fn nudge(&mut self, traits: &mut PersonalityTraits, field: TraitField, raw_delta: f32) {
        let delta = raw_delta * self.multiplier;
        match field {
            TraitField::Openness => apply_with_cap(
                &mut traits.openness,
                &mut self.session.openness,
                delta,
            ),
            TraitField::Conscientiousness => apply_with_cap(
                &mut traits.conscientiousness,
                &mut self.session.conscientiousness,
                delta,
            ),
            TraitField::Extraversion => apply_with_cap(
                &mut traits.extraversion,
                &mut self.session.extraversion,
                delta,
            ),
            TraitField::Agreeableness => apply_with_cap(
                &mut traits.agreeableness,
                &mut self.session.agreeableness,
                delta,
            ),
            TraitField::Neuroticism => apply_with_cap(
                &mut traits.neuroticism,
                &mut self.session.neuroticism,
                delta,
            ),
            TraitField::BattleHunger => apply_with_cap(
                &mut traits.battle_hunger,
                &mut self.session.battle_hunger,
                delta,
            ),
            TraitField::Greed => apply_with_cap(
                &mut traits.greed,
                &mut self.session.greed,
                delta,
            ),
            TraitField::Wanderlust => apply_with_cap(
                &mut traits.wanderlust,
                &mut self.session.wanderlust,
                delta,
            ),
            TraitField::Loyalty => apply_with_cap(
                &mut traits.loyalty,
                &mut self.session.loyalty,
                delta,
            ),
            TraitField::Mischief => apply_with_cap(
                &mut traits.mischief,
                &mut self.session.mischief,
                delta,
            ),
        }
    }
}

/// Apply `delta` to `value`, clamped to `[0.0, 1.0]` and bounded by the
/// per-session cap tracked in `accumulated`.
fn apply_with_cap(value: &mut f32, accumulated: &mut f32, delta: f32) {
    // How much cap is still available in the direction of this delta?
    let remaining_cap = if delta >= 0.0 {
        (MAX_SESSION_DELTA - *accumulated).max(0.0)
    } else {
        (-MAX_SESSION_DELTA - *accumulated).min(0.0)
    };

    let effective = if delta >= 0.0 {
        delta.min(remaining_cap)
    } else {
        delta.max(remaining_cap)
    };

    *value = (*value + effective).clamp(0.0, 1.0);
    *accumulated += effective;
}

/// Nudge `value` 1% toward 0.5 at the given `rate` (0.0–1.0).
fn decay_field(value: &mut f32, rate: f32) {
    const BASELINE: f32 = 0.5;
    *value += (BASELINE - *value) * rate;
}

/// Enumeration of driftable trait fields (avoids string-keyed maps).
/// `Mischief` is included for future event types; suppress dead-code lint.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum TraitField {
    Openness,
    Conscientiousness,
    Extraversion,
    Agreeableness,
    Neuroticism,
    BattleHunger,
    Greed,
    Wanderlust,
    Loyalty,
    Mischief,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::soul::{PersonalityTraits, SoulEvent};

    fn default_traits() -> PersonalityTraits {
        PersonalityTraits::default()
    }

    // --- drift triggers ----------------------------------------------------

    #[test]
    fn death_increases_neuroticism() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = default_traits();
        let before = traits.neuroticism;
        engine.apply(
            &mut traits,
            &SoulEvent::Death {
                zone: "guk".into(),
                killer: None,
            },
        );
        assert!(
            traits.neuroticism > before,
            "death should raise neuroticism"
        );
        assert!((traits.neuroticism - before - 0.01).abs() < f32::EPSILON);
    }

    #[test]
    fn kill_increases_battle_hunger() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = default_traits();
        let before = traits.battle_hunger;
        engine.apply(
            &mut traits,
            &SoulEvent::Kill {
                target: "gnoll".into(),
                zone: "blackburrow".into(),
            },
        );
        assert!(traits.battle_hunger > before);
        assert!((traits.battle_hunger - before - 0.005).abs() < f32::EPSILON);
    }

    #[test]
    fn loot_increases_greed() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = default_traits();
        let before = traits.greed;
        engine.apply(
            &mut traits,
            &SoulEvent::Loot {
                item: "Fungi Tunic".into(),
                zone: "sebilis".into(),
            },
        );
        assert!(traits.greed > before);
    }

    #[test]
    fn zone_enter_increases_openness_and_wanderlust() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = default_traits();
        let before_open = traits.openness;
        let before_wander = traits.wanderlust;
        engine.apply(
            &mut traits,
            &SoulEvent::ZoneEnter {
                zone: "everfrost".into(),
            },
        );
        assert!(traits.openness > before_open);
        assert!(traits.wanderlust > before_wander);
    }

    #[test]
    fn positive_chat_increases_extraversion() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = default_traits();
        let before = traits.extraversion;
        engine.apply(
            &mut traits,
            &SoulEvent::PlayerChat {
                player_name: "Friend".into(),
                sentiment: 0.8,
            },
        );
        assert!(traits.extraversion > before);
    }

    #[test]
    fn negative_chat_decreases_agreeableness() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = default_traits();
        let before = traits.agreeableness;
        engine.apply(
            &mut traits,
            &SoulEvent::PlayerChat {
                player_name: "Troll".into(),
                sentiment: -0.9,
            },
        );
        assert!(traits.agreeableness < before);
    }

    #[test]
    fn neutral_chat_no_drift() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = default_traits();
        let snap = traits.clone();
        engine.apply(
            &mut traits,
            &SoulEvent::PlayerChat {
                player_name: "Passerby".into(),
                sentiment: 0.1,
            },
        );
        // Neutral sentiment (|s| <= 0.3) → no change
        assert_eq!(traits.agreeableness, snap.agreeableness);
        assert_eq!(traits.extraversion, snap.extraversion);
    }

    #[test]
    fn relationship_positive_increases_loyalty() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = default_traits();
        let before = traits.loyalty;
        engine.apply(
            &mut traits,
            &SoulEvent::RelationshipChange {
                character: "Alice".into(),
                delta: 0.5,
            },
        );
        assert!(traits.loyalty > before);
    }

    #[test]
    fn level_up_increases_conscientiousness() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = default_traits();
        let before = traits.conscientiousness;
        engine.apply(&mut traits, &SoulEvent::LevelUp { new_level: 20 });
        assert!(traits.conscientiousness > before);
    }

    // --- magnitude constraints -------------------------------------------

    #[test]
    fn traits_clamped_to_unit_range() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = PersonalityTraits {
            neuroticism: 0.999,
            ..Default::default()
        };
        // Apply many deaths — trait must not exceed 1.0
        for _ in 0..200 {
            engine.apply(
                &mut traits,
                &SoulEvent::Death {
                    zone: "guk".into(),
                    killer: None,
                },
            );
        }
        assert!(traits.neuroticism <= 1.0, "trait must not exceed 1.0");
    }

    #[test]
    fn session_cap_enforced() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = PersonalityTraits {
            neuroticism: 0.1,
            ..Default::default()
        };
        let baseline = traits.neuroticism;

        // Apply many deaths; session cap is 0.15
        for _ in 0..100 {
            engine.apply(
                &mut traits,
                &SoulEvent::Death {
                    zone: "guk".into(),
                    killer: None,
                },
            );
        }

        let total_drift = traits.neuroticism - baseline;
        assert!(
            total_drift <= MAX_SESSION_DELTA + f32::EPSILON,
            "session drift {total_drift} exceeded cap {MAX_SESSION_DELTA}"
        );
    }

    // --- decay -----------------------------------------------------------

    #[test]
    fn decay_toward_baseline_moves_high_trait_down() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = PersonalityTraits {
            neuroticism: 0.9,
            ..Default::default()
        };
        engine.decay_toward_baseline(&mut traits, 0.01);
        assert!(traits.neuroticism < 0.9, "high trait should decay toward 0.5");
        assert!(traits.neuroticism > 0.5);
    }

    #[test]
    fn decay_toward_baseline_moves_low_trait_up() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = PersonalityTraits {
            neuroticism: 0.1,
            ..Default::default()
        };
        engine.decay_toward_baseline(&mut traits, 0.01);
        assert!(traits.neuroticism > 0.1, "low trait should rise toward 0.5");
        assert!(traits.neuroticism < 0.5);
    }

    #[test]
    fn decay_at_baseline_is_stable() {
        let mut engine = DriftEngine::new(1.0);
        let mut traits = PersonalityTraits {
            neuroticism: 0.5,
            ..Default::default()
        };
        let before = traits.neuroticism;
        engine.decay_toward_baseline(&mut traits, 0.01);
        assert!((traits.neuroticism - before).abs() < f32::EPSILON);
    }

    // --- multiplier ------------------------------------------------------

    #[test]
    fn multiplier_zero_disables_drift() {
        let mut engine = DriftEngine::new(0.0);
        let mut traits = default_traits();
        let snap = traits.clone();
        engine.apply(
            &mut traits,
            &SoulEvent::Death {
                zone: "guk".into(),
                killer: None,
            },
        );
        assert_eq!(traits.neuroticism, snap.neuroticism);
    }

    #[test]
    fn multiplier_two_doubles_delta() {
        let mut engine_1x = DriftEngine::new(1.0);
        let mut engine_2x = DriftEngine::new(2.0);
        let mut traits_1x = default_traits();
        let mut traits_2x = default_traits();
        let event = SoulEvent::Kill {
            target: "gnoll".into(),
            zone: "blackburrow".into(),
        };
        engine_1x.apply(&mut traits_1x, &event);
        engine_2x.apply(&mut traits_2x, &event);
        let diff = traits_2x.battle_hunger - traits_1x.battle_hunger;
        assert!(
            (diff - 0.005).abs() < f32::EPSILON,
            "2x multiplier should double the 0.005 delta; diff={diff}"
        );
    }
}
