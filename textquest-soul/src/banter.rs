//! Inter-character banter — proximity and relationship-based dialogue
//! triggering.
//!
//! Each tick the `BanterEngine` is given the full map of client states and
//! soul names. It finds pairs of characters that are co-located (same zone),
//! applies a relationship-weighted probability, enforces a per-pair cooldown,
//! and — when all gates pass — emits an `LlmRequest` for the *initiating*
//! character using `Situation::BotChat`.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use textquest_common::soul::SocialTag;

use crate::{
    config::SoulConfig,
    llm::{LlmPriority, LlmRequest, Situation},
    social::SocialGraph,
};
use textquest_common::{soul::SpeechStyle, types::ClientId};

// ---------------------------------------------------------------------------
// Configuration defaults
// ---------------------------------------------------------------------------

/// Banter trigger probabilities per relationship class (per tick).
///
/// Fraction of ticks where a pair should consider initiating banter, given
/// their social standing.
struct BanterChances {
    /// Friend / positive relationship probability
    friend: f32,
    /// Neutral / acquaintance probability
    neutral: f32,
    /// Rival / competitive probability
    rival: f32,
    /// Nemesis / hostile probability
    nemesis: f32,
}

impl BanterChances {
    fn from_config(config: &SoulConfig) -> Self {
        Self {
            friend: config.banter_chance_friend,
            neutral: 0.05,
            rival: 0.10,
            nemesis: 0.01,
        }
    }
}

// ---------------------------------------------------------------------------
// Per-pair state
// ---------------------------------------------------------------------------

/// Cooldown state for a single character pair.
#[derive(Debug)]
pub struct BanterState {
    /// When the last banter was initiated for this pair.
    pub last_banter: Option<Instant>,
}

impl BanterState {
    fn new() -> Self {
        Self { last_banter: None }
    }

    /// Returns `true` if enough time has elapsed since the last banter.
    fn cooldown_elapsed(&self, cooldown: Duration) -> bool {
        match self.last_banter {
            None => true,
            Some(t) => t.elapsed() >= cooldown,
        }
    }

    /// Mark a banter as just fired.
    fn mark_fired(&mut self) {
        self.last_banter = Some(Instant::now());
    }
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Manages per-pair cooldowns and probability gating for inter-character
/// banter.
///
/// Key ordering: pairs are stored as `(min(a,b), max(a,b))` so that `(A,B)`
/// and `(B,A)` share one cooldown slot.
#[derive(Debug, Default)]
pub struct BanterEngine {
    /// Per-pair cooldown state. Key is `(initiator_id, responder_id)` with
    /// `initiator_id < responder_id` for deduplication.
    cooldowns: HashMap<(ClientId, ClientId), BanterState>,
}

impl BanterEngine {
    /// Create a new engine with no active cooldowns.
    pub fn new() -> Self {
        Self::default()
    }

    /// Main tick entry-point.
    ///
    /// Returns a list of LLM requests that should be enqueued — one per pair
    /// that passes all gates.
    ///
    /// # Parameters
    ///
    /// - `client_names` — mapping from `ClientId` to character name.
    /// - `client_zones` — mapping from `ClientId` to zone short name.
    /// - `social` — the social graph for relationship lookup.
    /// - `soul_traits` — personality traits for each client (for LLM context).
    /// - `soul_moods` — mood for each client.
    /// - `soul_backstories` — backstory for each client.
    /// - `config` — soul config for probability / cooldown values.
    /// - `rng` — caller-provided random value in `[0.0, 1.0)`.
    ///
    /// The caller supplies `rng` so that the engine is deterministic in tests.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        client_names: &HashMap<ClientId, String>,
        client_zones: &HashMap<ClientId, String>,
        social: &SocialGraph,
        soul_traits: &HashMap<ClientId, textquest_common::soul::PersonalityTraits>,
        soul_moods: &HashMap<ClientId, textquest_common::soul::MoodState>,
        soul_backstories: &HashMap<ClientId, String>,
        config: &SoulConfig,
        rng_fn: &mut dyn FnMut() -> f32,
    ) -> Vec<(ClientId, LlmRequest)> {
        if !config.inter_character_chat {
            return Vec::new();
        }

        let cooldown = Duration::from_secs(config.banter_cooldown_secs);
        let chances = BanterChances::from_config(config);

        let mut results: Vec<(ClientId, LlmRequest)> = Vec::new();

        // Collect sorted client ID pairs to avoid duplicates
        let mut ids: Vec<ClientId> = client_names.keys().copied().collect();
        ids.sort_unstable();

        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let id_a = ids[i];
                let id_b = ids[j];

                // Same zone check (proximity gate)
                let zone_a = client_zones.get(&id_a).map(String::as_str).unwrap_or("");
                let zone_b = client_zones.get(&id_b).map(String::as_str).unwrap_or("");
                if zone_a.is_empty() || zone_a != zone_b {
                    continue;
                }

                // Cooldown gate (shared for A→B and B→A)
                let pair_key = (id_a.min(id_b), id_a.max(id_b));
                let state = self
                    .cooldowns
                    .entry(pair_key)
                    .or_insert_with(BanterState::new);
                if !state.cooldown_elapsed(cooldown) {
                    continue;
                }

                // Relationship gate — compute trigger probability
                let name_a = match client_names.get(&id_a) {
                    Some(n) => n.as_str(),
                    None => continue,
                };
                let name_b = match client_names.get(&id_b) {
                    Some(n) => n.as_str(),
                    None => continue,
                };

                let threshold = banter_threshold(name_a, name_b, social, &chances);

                // Probability gate
                if rng_fn() >= threshold {
                    continue;
                }

                // All gates passed — emit an LLM request for id_a (initiator)
                state.mark_fired();

                let traits = soul_traits.get(&id_a).cloned().unwrap_or_default();
                let mood = soul_moods.get(&id_a).copied().unwrap_or_default();
                let backstory = soul_backstories.get(&id_a).cloned().unwrap_or_default();

                let request = LlmRequest {
                    character_name: name_a.to_string(),
                    traits,
                    mood,
                    speech_style: SpeechStyle::default(),
                    situation: Situation::BotChat {
                        character_name: name_b.to_string(),
                        message: String::new(), // initiator opens the conversation
                    },
                    priority: LlmPriority::Low,
                    memory_context: Vec::new(),
                    backstory,
                };

                results.push((id_a, request));
            }
        }

        results
    }

    /// Return the number of active (not-yet-expired) pair cooldowns.
    #[cfg(test)]
    pub fn active_cooldown_count(&self) -> usize {
        self.cooldowns.len()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Determine the banter trigger probability for a pair from their tags.
///
/// Uses the strongest applicable tag. Falls back to neutral.
fn banter_threshold(
    name_a: &str,
    name_b: &str,
    social: &SocialGraph,
    chances: &BanterChances,
) -> f32 {
    // Check A→B relationship tags
    let tags = social
        .get(name_a, name_b)
        .map(|r| r.tags.as_slice())
        .unwrap_or(&[]);

    if tags.contains(&SocialTag::Friend)
        || tags.contains(&SocialTag::Sibling)
        || tags.contains(&SocialTag::Crush)
    {
        chances.friend
    } else if tags.contains(&SocialTag::Nemesis) {
        chances.nemesis
    } else if tags.contains(&SocialTag::Rival) {
        chances.rival
    } else {
        chances.neutral
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::SoulConfig, social::SocialGraph};
    use textquest_common::soul::{MoodState, PersonalityTraits, SocialTag};

    fn default_config() -> SoulConfig {
        SoulConfig {
            enabled: true,
            inter_character_chat: true,
            banter_cooldown_secs: 300,
            banter_chance_friend: 0.2,
            ..SoulConfig::default()
        }
    }

    fn names(pairs: &[(ClientId, &str)]) -> HashMap<ClientId, String> {
        pairs.iter().map(|(id, n)| (*id, n.to_string())).collect()
    }

    fn zones(pairs: &[(ClientId, &str)]) -> HashMap<ClientId, String> {
        pairs.iter().map(|(id, z)| (*id, z.to_string())).collect()
    }

    fn always_trigger() -> impl FnMut() -> f32 {
        || 0.0 // 0.0 < any positive threshold → always triggers
    }

    fn never_trigger() -> impl FnMut() -> f32 {
        || 1.0 // 1.0 >= any threshold → never triggers
    }

    // ------------------------------------------------------------------
    // Proximity filter tests
    // ------------------------------------------------------------------

    #[test]
    fn different_zones_no_banter() {
        let mut engine = BanterEngine::new();
        let config = default_config();
        let client_names = names(&[(1, "Alice"), (2, "Bob")]);
        let client_zones = zones(&[(1, "pok"), (2, "qeynos")]);
        let social = SocialGraph::new();
        let traits: HashMap<ClientId, PersonalityTraits> = HashMap::new();
        let moods: HashMap<ClientId, MoodState> = HashMap::new();
        let backstories: HashMap<ClientId, String> = HashMap::new();

        let results = engine.tick(
            &client_names,
            &client_zones,
            &social,
            &traits,
            &moods,
            &backstories,
            &config,
            &mut always_trigger(),
        );

        assert!(
            results.is_empty(),
            "different zones should produce no banter"
        );
    }

    #[test]
    fn same_zone_produces_banter_when_trigger_fires() {
        let mut engine = BanterEngine::new();
        let config = default_config();
        let client_names = names(&[(1, "Alice"), (2, "Bob")]);
        let client_zones = zones(&[(1, "pok"), (2, "pok")]);
        let social = SocialGraph::new();
        let traits: HashMap<ClientId, PersonalityTraits> = HashMap::new();
        let moods: HashMap<ClientId, MoodState> = HashMap::new();
        let backstories: HashMap<ClientId, String> = HashMap::new();

        let results = engine.tick(
            &client_names,
            &client_zones,
            &social,
            &traits,
            &moods,
            &backstories,
            &config,
            &mut always_trigger(),
        );

        assert_eq!(
            results.len(),
            1,
            "same zone with trigger should produce 1 banter"
        );
        assert_eq!(results[0].0, 1); // initiator is id_a (lower ID)
        assert_eq!(results[0].1.character_name, "Alice");
        if let Situation::BotChat { character_name, .. } = &results[0].1.situation {
            assert_eq!(character_name, "Bob");
        } else {
            panic!("expected BotChat situation");
        }
    }

    #[test]
    fn never_trigger_rng_blocks_banter() {
        let mut engine = BanterEngine::new();
        let config = default_config();
        let client_names = names(&[(1, "Alice"), (2, "Bob")]);
        let client_zones = zones(&[(1, "pok"), (2, "pok")]);
        let social = SocialGraph::new();
        let traits: HashMap<ClientId, PersonalityTraits> = HashMap::new();
        let moods: HashMap<ClientId, MoodState> = HashMap::new();
        let backstories: HashMap<ClientId, String> = HashMap::new();

        let results = engine.tick(
            &client_names,
            &client_zones,
            &social,
            &traits,
            &moods,
            &backstories,
            &config,
            &mut never_trigger(),
        );

        assert!(results.is_empty(), "rng=1.0 should never trigger");
    }

    // ------------------------------------------------------------------
    // Relationship threshold tests
    // ------------------------------------------------------------------

    #[test]
    fn friend_tag_uses_friend_threshold() {
        let mut social = SocialGraph::new();
        social
            .get_or_create("Alice", "Bob")
            .tags
            .push(SocialTag::Friend);

        let chances = BanterChances {
            friend: 0.9,
            neutral: 0.01,
            rival: 0.1,
            nemesis: 0.001,
        };

        let threshold = banter_threshold("Alice", "Bob", &social, &chances);
        assert!(
            (threshold - 0.9).abs() < f32::EPSILON,
            "friend should use friend threshold"
        );
    }

    #[test]
    fn rival_tag_uses_rival_threshold() {
        let mut social = SocialGraph::new();
        social
            .get_or_create("Alice", "Bob")
            .tags
            .push(SocialTag::Rival);

        let chances = BanterChances {
            friend: 0.9,
            neutral: 0.01,
            rival: 0.5,
            nemesis: 0.001,
        };

        let threshold = banter_threshold("Alice", "Bob", &social, &chances);
        assert!(
            (threshold - 0.5).abs() < f32::EPSILON,
            "rival should use rival threshold"
        );
    }

    #[test]
    fn nemesis_tag_uses_nemesis_threshold() {
        let mut social = SocialGraph::new();
        social
            .get_or_create("Alice", "Bob")
            .tags
            .push(SocialTag::Nemesis);

        let chances = BanterChances {
            friend: 0.9,
            neutral: 0.05,
            rival: 0.1,
            nemesis: 0.001,
        };

        let threshold = banter_threshold("Alice", "Bob", &social, &chances);
        assert!(
            (threshold - 0.001).abs() < f32::EPSILON,
            "nemesis should use nemesis threshold"
        );
    }

    #[test]
    fn no_tags_uses_neutral_threshold() {
        let social = SocialGraph::new();
        let chances = BanterChances {
            friend: 0.9,
            neutral: 0.05,
            rival: 0.1,
            nemesis: 0.001,
        };

        let threshold = banter_threshold("Alice", "Bob", &social, &chances);
        assert!(
            (threshold - 0.05).abs() < f32::EPSILON,
            "no tags should use neutral threshold"
        );
    }

    // ------------------------------------------------------------------
    // Cooldown enforcement
    // ------------------------------------------------------------------

    #[test]
    fn cooldown_prevents_immediate_repeat() {
        let mut engine = BanterEngine::new();
        let config = default_config();
        let client_names = names(&[(1, "Alice"), (2, "Bob")]);
        let client_zones = zones(&[(1, "pok"), (2, "pok")]);
        let social = SocialGraph::new();
        let traits: HashMap<ClientId, PersonalityTraits> = HashMap::new();
        let moods: HashMap<ClientId, MoodState> = HashMap::new();
        let backstories: HashMap<ClientId, String> = HashMap::new();

        // First tick fires
        let first = engine.tick(
            &client_names,
            &client_zones,
            &social,
            &traits,
            &moods,
            &backstories,
            &config,
            &mut always_trigger(),
        );
        assert_eq!(first.len(), 1, "first tick should fire");

        // Second tick immediately after should be blocked by cooldown
        let second = engine.tick(
            &client_names,
            &client_zones,
            &social,
            &traits,
            &moods,
            &backstories,
            &config,
            &mut always_trigger(),
        );
        assert!(second.is_empty(), "cooldown should block second tick");
        assert_eq!(engine.active_cooldown_count(), 1);
    }

    #[test]
    fn expired_cooldown_allows_new_banter() {
        let mut engine = BanterEngine::new();
        let config = SoulConfig {
            enabled: true,
            inter_character_chat: true,
            banter_cooldown_secs: 0, // zero cooldown for testing
            banter_chance_friend: 0.2,
            ..SoulConfig::default()
        };
        let client_names = names(&[(1, "Alice"), (2, "Bob")]);
        let client_zones = zones(&[(1, "pok"), (2, "pok")]);
        let social = SocialGraph::new();
        let traits: HashMap<ClientId, PersonalityTraits> = HashMap::new();
        let moods: HashMap<ClientId, MoodState> = HashMap::new();
        let backstories: HashMap<ClientId, String> = HashMap::new();

        let first = engine.tick(
            &client_names,
            &client_zones,
            &social,
            &traits,
            &moods,
            &backstories,
            &config,
            &mut always_trigger(),
        );
        assert_eq!(first.len(), 1, "first tick should fire");

        // With 0-second cooldown, next tick should also fire
        let second = engine.tick(
            &client_names,
            &client_zones,
            &social,
            &traits,
            &moods,
            &backstories,
            &config,
            &mut always_trigger(),
        );
        assert_eq!(
            second.len(),
            1,
            "zero cooldown should allow immediate repeat"
        );
    }

    // ------------------------------------------------------------------
    // LLM request correctness
    // ------------------------------------------------------------------

    #[test]
    fn llm_request_has_correct_situation_fields() {
        let mut engine = BanterEngine::new();
        let config = default_config();
        let client_names = names(&[(10, "Grimjaw"), (20, "Luminara")]);
        let client_zones = zones(&[(10, "sebilis"), (20, "sebilis")]);
        let social = SocialGraph::new();
        let traits: HashMap<ClientId, PersonalityTraits> = HashMap::new();
        let moods: HashMap<ClientId, MoodState> = HashMap::new();
        let backstories: HashMap<ClientId, String> = HashMap::new();

        let results = engine.tick(
            &client_names,
            &client_zones,
            &social,
            &traits,
            &moods,
            &backstories,
            &config,
            &mut always_trigger(),
        );

        assert_eq!(results.len(), 1);
        let (id, req) = &results[0];
        assert_eq!(*id, 10); // lower ID initiates
        assert_eq!(req.character_name, "Grimjaw");
        assert_eq!(req.priority, LlmPriority::Low);
        match &req.situation {
            Situation::BotChat { character_name, .. } => {
                assert_eq!(character_name, "Luminara");
            }
            _ => panic!("expected BotChat"),
        }
    }

    #[test]
    fn inter_character_chat_disabled_suppresses_all() {
        let mut engine = BanterEngine::new();
        let config = SoulConfig {
            enabled: true,
            inter_character_chat: false,
            banter_cooldown_secs: 0,
            banter_chance_friend: 1.0,
            ..SoulConfig::default()
        };
        let client_names = names(&[(1, "Alice"), (2, "Bob")]);
        let client_zones = zones(&[(1, "pok"), (2, "pok")]);
        let social = SocialGraph::new();
        let traits: HashMap<ClientId, PersonalityTraits> = HashMap::new();
        let moods: HashMap<ClientId, MoodState> = HashMap::new();
        let backstories: HashMap<ClientId, String> = HashMap::new();

        let results = engine.tick(
            &client_names,
            &client_zones,
            &social,
            &traits,
            &moods,
            &backstories,
            &config,
            &mut always_trigger(),
        );
        assert!(
            results.is_empty(),
            "disabled inter_character_chat should suppress all banter"
        );
    }
}
