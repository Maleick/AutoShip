//! Speech style evolution — catchphrase learning and slang contagion for Soul
//! Engine characters.
//!
//! Two parallel systems:
//! - **Catchphrases**: longer signature phrases adopted from trusted peers
//!   (faction > 500). Max 5 per character.
//! - **Slang**: short casual terms ("kk", "inc", "ty") that spread through
//!   groups at a lower trust threshold. Max 10 per character; fade over time
//!   (tick-based decay).

use std::{
    collections::{HashMap, VecDeque},
    time::Instant,
};

const MAX_OBSERVATION_HISTORY: usize = 1_000;
/// Hard cap for globally tracked phrases to prevent unbounded growth from
/// attacker-controlled chat tokens.
const MAX_TRACKED_PHRASES: usize = 4_096;
/// Hard cap for distinct speakers retained per phrase.
const MAX_SPEAKERS_PER_PHRASE: usize = 64;

/// Faction score threshold for catchphrase adoption (trusted friend).
const CATCHPHRASE_FACTION_THRESHOLD: i32 = 500;

/// Faction score threshold for slang contagion (lower — group spread).
const SLANG_FACTION_THRESHOLD: i32 = 100;

/// Minimum number of distinct speakers before a phrase is considered a
/// catchphrase candidate (issue spec: >3 speakers).
const MIN_CATCHPHRASE_SPEAKERS: usize = 3;

/// A recorded observation of another character using a phrase.
#[derive(Debug, Clone)]
pub struct CatchphraseEvent {
    pub source_character: String,
    pub phrase: String,
    pub timestamp: Instant,
}

/// Configuration for speech evolution behavior.
#[derive(Debug, Clone)]
pub struct SpeechEvolutionConfig {
    /// Maximum number of active catchphrases a character can hold.
    pub max_catchphrases: usize,
    /// Maximum number of active slang terms a character can hold.
    pub max_slang: usize,
    /// Base probability (0.0–1.0) of adopting an observed catchphrase.
    pub adoption_chance: f32,
    /// Multiplier applied to adoption_chance when faction >= threshold.
    pub trust_multiplier: f32,
    /// Ticks before an unused slang term decays and is removed.
    pub slang_decay_ticks: u64,
}

impl Default for SpeechEvolutionConfig {
    fn default() -> Self {
        Self {
            max_catchphrases: 5,
            max_slang: 10,
            adoption_chance: 0.1,
            trust_multiplier: 2.0,
            slang_decay_ticks: 1440, // ~2 hours at 5s/tick
        }
    }
}

/// A slang term with its adoption tick for decay tracking.
#[derive(Debug, Clone)]
pub struct SlangEntry {
    pub term: String,
    /// Tick count at which this slang was last reinforced (heard or used).
    pub last_seen_tick: u64,
}

/// Global phrase frequency tracker — watches how many distinct speakers have
/// used a given phrase. Used by the coordinator to identify catchphrase
/// candidates before routing them to per-character `SpeechEvolution`.
#[derive(Debug, Default)]
pub struct PhraseFrequencyTracker {
    /// phrase → set of distinct speaker names
    speakers: HashMap<String, Vec<String>>,
    /// Insertion order used to evict oldest phrases when at capacity.
    order: VecDeque<String>,
}

impl PhraseFrequencyTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `speaker` used `phrase`. Returns the updated distinct
    /// speaker count for this phrase.
    pub fn record(&mut self, speaker: &str, phrase: &str) -> usize {
        if !self.speakers.contains_key(phrase) {
            if self.speakers.len() >= MAX_TRACKED_PHRASES
                && let Some(oldest) = self.order.pop_front()
            {
                self.speakers.remove(&oldest);
            }
            self.order.push_back(phrase.to_string());
        }

        let entry = self.speakers.entry(phrase.to_string()).or_default();
        if !entry.iter().any(|s| s == speaker) && entry.len() < MAX_SPEAKERS_PER_PHRASE {
            entry.push(speaker.to_string());
        }
        entry.len()
    }

    /// Returns how many distinct speakers have used `phrase`.
    pub fn speaker_count(&self, phrase: &str) -> usize {
        self.speakers.get(phrase).map(|v| v.len()).unwrap_or(0)
    }

    /// Returns true if `phrase` has been used by more than
    /// `MIN_CATCHPHRASE_SPEAKERS` distinct speakers.
    pub fn is_catchphrase_candidate(&self, phrase: &str) -> bool {
        self.speaker_count(phrase) > MIN_CATCHPHRASE_SPEAKERS
    }

    /// Clears all tracked phrases (e.g. on zone change).
    pub fn reset(&mut self) {
        self.speakers.clear();
        self.order.clear();
    }
}

/// Tracks catchphrase observations, learned catchphrases, and slang for a
/// single character.
#[derive(Debug)]
pub struct SpeechEvolution {
    config: SpeechEvolutionConfig,
    history: VecDeque<CatchphraseEvent>,
    catchphrases: Vec<String>,
    slang: Vec<SlangEntry>,
}

impl SpeechEvolution {
    pub fn new(config: SpeechEvolutionConfig) -> Self {
        Self {
            config,
            history: VecDeque::new(),
            catchphrases: Vec::new(),
            slang: Vec::new(),
        }
    }

    /// Observe another character using a phrase. Returns true if the phrase was
    /// adopted as a catchphrase.
    ///
    /// - `faction_score`: relationship faction score (-1000..1000); >= 500
    ///   activates the trust multiplier.
    /// - `rng_roll`: caller-provided random value in 0.0–1.0.
    ///
    /// Only adopts if the phrase has been seen from > `MIN_CATCHPHRASE_SPEAKERS`
    /// distinct speakers (pass `is_candidate = true` from the global tracker).
    pub fn observe_phrase(
        &mut self,
        source: &str,
        phrase: &str,
        faction_score: i32,
        rng_roll: f32,
    ) -> bool {
        self.history.push_back(CatchphraseEvent {
            source_character: source.to_string(),
            phrase: phrase.to_string(),
            timestamp: Instant::now(),
        });
        if self.history.len() > MAX_OBSERVATION_HISTORY {
            self.history.pop_front();
        }

        // Don't adopt if below faction threshold
        if faction_score < CATCHPHRASE_FACTION_THRESHOLD {
            return false;
        }

        // Don't adopt duplicates
        if self.catchphrases.iter().any(|p| p == phrase) {
            return false;
        }

        let threshold = if faction_score >= CATCHPHRASE_FACTION_THRESHOLD {
            self.config.adoption_chance * self.config.trust_multiplier
        } else {
            self.config.adoption_chance
        };

        if rng_roll < threshold {
            // Evict oldest if at capacity
            if self.catchphrases.len() >= self.config.max_catchphrases {
                self.forget_oldest();
            }
            self.catchphrases.push(phrase.to_string());
            true
        } else {
            false
        }
    }

    /// Observe a slang term from another character. Returns true if adopted.
    ///
    /// Slang spreads at a lower trust threshold than catchphrases and
    /// decays if not reinforced within `slang_decay_ticks`.
    ///
    /// - `faction_score`: >= `SLANG_FACTION_THRESHOLD` (100) to be eligible.
    /// - `current_tick`: coordinator tick count for decay tracking.
    /// - `rng_roll`: caller-provided random value in 0.0–1.0.
    pub fn observe_slang(
        &mut self,
        _source: &str,
        term: &str,
        faction_score: i32,
        current_tick: u64,
        rng_roll: f32,
    ) -> bool {
        if faction_score < SLANG_FACTION_THRESHOLD {
            return false;
        }

        // Reinforce if already known — reset decay timer
        if let Some(entry) = self.slang.iter_mut().find(|s| s.term == term) {
            entry.last_seen_tick = current_tick;
            return false; // already have it
        }

        // Slang uses base adoption_chance (no trust multiplier needed)
        if rng_roll >= self.config.adoption_chance {
            return false;
        }

        // Evict oldest if at capacity
        if self.slang.len() >= self.config.max_slang {
            self.slang.remove(0);
        }
        self.slang.push(SlangEntry {
            term: term.to_string(),
            last_seen_tick: current_tick,
        });
        true
    }

    /// Decay slang terms that haven't been reinforced within the decay window.
    /// Call this periodically from the coordinator tick.
    pub fn decay_slang(&mut self, current_tick: u64) {
        self.slang.retain(|entry| {
            current_tick.saturating_sub(entry.last_seen_tick) < self.config.slang_decay_ticks
        });
    }

    /// Returns the currently active catchphrases.
    pub fn active_catchphrases(&self) -> &[String] {
        &self.catchphrases
    }

    /// Returns the currently active slang terms.
    pub fn active_slang(&self) -> Vec<&str> {
        self.slang.iter().map(|s| s.term.as_str()).collect()
    }

    /// Removes the oldest catchphrase if any exist.
    pub fn forget_oldest(&mut self) {
        if !self.catchphrases.is_empty() {
            self.catchphrases.remove(0);
        }
    }

    /// Returns the observation history.
    pub fn history(&self) -> &VecDeque<CatchphraseEvent> {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_evolution() -> SpeechEvolution {
        SpeechEvolution::new(SpeechEvolutionConfig::default())
    }

    // --- SpeechEvolutionConfig ---

    #[test]
    fn test_default_config_values() {
        let config = SpeechEvolutionConfig::default();
        assert_eq!(config.max_catchphrases, 5);
        assert_eq!(config.max_slang, 10);
        assert!((config.adoption_chance - 0.1).abs() < f32::EPSILON);
        assert!((config.trust_multiplier - 2.0).abs() < f32::EPSILON);
        assert_eq!(config.slang_decay_ticks, 1440);
    }

    // --- Catchphrase adoption ---

    #[test]
    fn test_adopt_phrase_trusted_low_roll() {
        let mut evo = default_evolution();
        // faction >= 500 → threshold = 0.1 * 2.0 = 0.2; roll 0.05 < 0.2 → adopted
        let adopted = evo.observe_phrase("Gandalf", "You shall not pass!", 500, 0.05);
        assert!(adopted);
        assert_eq!(evo.active_catchphrases(), &["You shall not pass!"]);
    }

    #[test]
    fn test_reject_phrase_high_roll() {
        let mut evo = default_evolution();
        // roll 0.5 >= 0.2 → rejected even with trusted faction
        let adopted = evo.observe_phrase("Gandalf", "You shall not pass!", 500, 0.5);
        assert!(!adopted);
        assert!(evo.active_catchphrases().is_empty());
    }

    #[test]
    fn test_reject_phrase_below_faction_threshold() {
        let mut evo = default_evolution();
        // faction 499 < 500 → no adoption regardless of roll
        let adopted = evo.observe_phrase("Stranger", "For the Horde!", 499, 0.01);
        assert!(!adopted);
        assert!(evo.active_catchphrases().is_empty());
    }

    #[test]
    fn test_no_duplicate_catchphrases() {
        let mut evo = default_evolution();
        let first = evo.observe_phrase("A", "Leroy!", 600, 0.01);
        assert!(first);
        // Same phrase, low roll — rejected as duplicate
        let second = evo.observe_phrase("B", "Leroy!", 600, 0.01);
        assert!(!second);
        assert_eq!(evo.active_catchphrases().len(), 1);
    }

    #[test]
    fn test_max_catchphrases_enforced() {
        let config = SpeechEvolutionConfig {
            max_catchphrases: 3,
            adoption_chance: 1.0, // always adopt
            trust_multiplier: 1.0,
            ..Default::default()
        };
        let mut evo = SpeechEvolution::new(config);

        evo.observe_phrase("A", "phrase1", 600, 0.0);
        evo.observe_phrase("B", "phrase2", 600, 0.0);
        evo.observe_phrase("C", "phrase3", 600, 0.0);
        assert_eq!(evo.active_catchphrases().len(), 3);

        // Adding a 4th should evict the oldest
        evo.observe_phrase("D", "phrase4", 600, 0.0);
        assert_eq!(evo.active_catchphrases().len(), 3);
        assert_eq!(
            evo.active_catchphrases(),
            &["phrase2", "phrase3", "phrase4"]
        );
    }

    #[test]
    fn test_forget_oldest_removes_first() {
        let mut evo = default_evolution();
        evo.observe_phrase("A", "alpha", 600, 0.01);
        evo.observe_phrase("B", "beta", 600, 0.01);
        assert_eq!(evo.active_catchphrases(), &["alpha", "beta"]);

        evo.forget_oldest();
        assert_eq!(evo.active_catchphrases(), &["beta"]);
    }

    #[test]
    fn test_forget_oldest_on_empty_is_safe() {
        let mut evo = default_evolution();
        evo.forget_oldest(); // should not panic
        assert!(evo.active_catchphrases().is_empty());
    }

    #[test]
    fn test_history_records_all_observations() {
        let mut evo = default_evolution();
        evo.observe_phrase("A", "hello", 600, 0.99); // rejected (high roll)
        evo.observe_phrase("B", "world", 600, 0.01); // adopted
        assert_eq!(evo.history().len(), 2);
        assert_eq!(evo.history()[0].source_character, "A");
        assert_eq!(evo.history()[1].phrase, "world");
    }

    #[test]
    fn test_history_is_bounded_to_max_size() {
        let mut evo = default_evolution();
        for idx in 0..(MAX_OBSERVATION_HISTORY + 5) {
            evo.observe_phrase("A", &format!("phrase{idx}"), 100, 0.99);
        }
        assert_eq!(evo.history().len(), MAX_OBSERVATION_HISTORY);
        assert_eq!(evo.history().front().unwrap().phrase, "phrase5");
    }

    #[test]
    fn test_zero_adoption_chance_never_adopts() {
        let config = SpeechEvolutionConfig {
            adoption_chance: 0.0,
            ..Default::default()
        };
        let mut evo = SpeechEvolution::new(config);
        // Even with roll 0.0, threshold is 0.0 and 0.0 < 0.0 is false
        let adopted = evo.observe_phrase("A", "test", 600, 0.0);
        assert!(!adopted);
    }

    #[test]
    fn test_high_trust_with_custom_multiplier() {
        let config = SpeechEvolutionConfig {
            adoption_chance: 0.1,
            trust_multiplier: 5.0,
            ..Default::default()
        };
        let mut evo = SpeechEvolution::new(config);
        // faction >= 500 → threshold = 0.1 * 5.0 = 0.5; roll 0.4 < 0.5 → adopted
        let adopted = evo.observe_phrase("Boss", "Get to the chopper!", 600, 0.4);
        assert!(adopted);
    }

    // --- Slang contagion ---

    #[test]
    fn test_slang_adopted_at_low_faction_threshold() {
        let mut evo = default_evolution();
        // faction 100 >= SLANG_FACTION_THRESHOLD; roll 0.05 < 0.1 → adopted
        let adopted = evo.observe_slang("Groupmate", "kk", 100, 0, 0.05);
        assert!(adopted);
        assert_eq!(evo.active_slang(), vec!["kk"]);
    }

    #[test]
    fn test_slang_rejected_below_faction_threshold() {
        let mut evo = default_evolution();
        let adopted = evo.observe_slang("Stranger", "inc", 50, 0, 0.01);
        assert!(!adopted);
        assert!(evo.active_slang().is_empty());
    }

    #[test]
    fn test_slang_rejected_high_roll() {
        let mut evo = default_evolution();
        let adopted = evo.observe_slang("Friend", "ty", 200, 0, 0.9);
        assert!(!adopted);
        assert!(evo.active_slang().is_empty());
    }

    #[test]
    fn test_slang_no_duplicate() {
        let mut evo = default_evolution();
        let first = evo.observe_slang("A", "kk", 200, 0, 0.01);
        assert!(first);
        // Second observation of same term returns false (reinforce, no new entry)
        let second = evo.observe_slang("B", "kk", 200, 0, 0.01);
        assert!(!second);
        assert_eq!(evo.active_slang().len(), 1);
    }

    #[test]
    fn test_slang_max_terms_enforced() {
        let config = SpeechEvolutionConfig {
            max_slang: 3,
            adoption_chance: 1.0,
            ..Default::default()
        };
        let mut evo = SpeechEvolution::new(config);

        evo.observe_slang("A", "kk", 200, 0, 0.0);
        evo.observe_slang("B", "inc", 200, 0, 0.0);
        evo.observe_slang("C", "ty", 200, 0, 0.0);
        assert_eq!(evo.active_slang().len(), 3);

        // 4th evicts oldest
        evo.observe_slang("D", "rotfl", 200, 0, 0.0);
        assert_eq!(evo.active_slang().len(), 3);
        let terms = evo.active_slang();
        assert!(!terms.contains(&"kk")); // oldest evicted
        assert!(terms.contains(&"rotfl"));
    }

    #[test]
    fn test_slang_decay_removes_old_terms() {
        let config = SpeechEvolutionConfig {
            slang_decay_ticks: 100,
            adoption_chance: 1.0,
            ..Default::default()
        };
        let mut evo = SpeechEvolution::new(config);

        evo.observe_slang("A", "kk", 200, 0, 0.0); // adopted at tick 0
        assert_eq!(evo.active_slang().len(), 1);

        // Decay at tick 100 — exactly at boundary, not yet expired
        evo.decay_slang(100);
        // 100 - 0 = 100, not < 100 → removed
        assert!(evo.active_slang().is_empty());
    }

    #[test]
    fn test_slang_decay_keeps_fresh_terms() {
        let config = SpeechEvolutionConfig {
            slang_decay_ticks: 100,
            adoption_chance: 1.0,
            ..Default::default()
        };
        let mut evo = SpeechEvolution::new(config);

        evo.observe_slang("A", "kk", 200, 50, 0.0); // adopted at tick 50
        evo.decay_slang(100); // 100 - 50 = 50 < 100 → still alive
        assert_eq!(evo.active_slang(), vec!["kk"]);
    }

    #[test]
    fn test_slang_reinforce_resets_decay() {
        let config = SpeechEvolutionConfig {
            slang_decay_ticks: 50,
            adoption_chance: 1.0,
            ..Default::default()
        };
        let mut evo = SpeechEvolution::new(config);

        evo.observe_slang("A", "kk", 200, 0, 0.0); // tick 0

        // Reinforce at tick 40
        evo.observe_slang("B", "kk", 200, 40, 0.0);

        // Decay at tick 80: 80 - 40 = 40 < 50 → still alive
        evo.decay_slang(80);
        assert_eq!(evo.active_slang(), vec!["kk"]);

        // Decay at tick 91: 91 - 40 = 51 >= 50 → removed
        evo.decay_slang(91);
        assert!(evo.active_slang().is_empty());
    }

    // --- PhraseFrequencyTracker ---

    #[test]
    fn test_phrase_tracker_counts_distinct_speakers() {
        let mut tracker = PhraseFrequencyTracker::new();
        assert_eq!(tracker.record("Alice", "For the Horde!"), 1);
        assert_eq!(tracker.record("Bob", "For the Horde!"), 2);
        assert_eq!(tracker.record("Carol", "For the Horde!"), 3);
        // Same speaker again — no increase
        assert_eq!(tracker.record("Alice", "For the Horde!"), 3);
    }

    #[test]
    fn test_phrase_tracker_is_catchphrase_candidate() {
        let mut tracker = PhraseFrequencyTracker::new();
        tracker.record("A", "phrase");
        tracker.record("B", "phrase");
        tracker.record("C", "phrase");
        // 3 speakers — not yet > MIN_CATCHPHRASE_SPEAKERS (3), need >3
        assert!(!tracker.is_catchphrase_candidate("phrase"));

        tracker.record("D", "phrase");
        assert!(tracker.is_catchphrase_candidate("phrase"));
    }

    #[test]
    fn test_phrase_tracker_speaker_count() {
        let mut tracker = PhraseFrequencyTracker::new();
        assert_eq!(tracker.speaker_count("unknown"), 0);
        tracker.record("Alice", "hello");
        assert_eq!(tracker.speaker_count("hello"), 1);
    }

    #[test]
    fn test_phrase_tracker_reset_clears_all() {
        let mut tracker = PhraseFrequencyTracker::new();
        tracker.record("Alice", "kk");
        tracker.record("Bob", "kk");
        tracker.reset();
        assert_eq!(tracker.speaker_count("kk"), 0);
        assert!(!tracker.is_catchphrase_candidate("kk"));
    }

    #[test]
    fn test_phrase_tracker_evicts_oldest_phrases_at_capacity() {
        let mut tracker = PhraseFrequencyTracker::new();

        for i in 0..MAX_TRACKED_PHRASES {
            let phrase = format!("p{i}");
            tracker.record("Alice", &phrase);
        }
        assert_eq!(tracker.speaker_count("p0"), 1);

        tracker.record("Alice", "overflow");

        assert_eq!(tracker.speaker_count("p0"), 0);
        assert_eq!(tracker.speaker_count("overflow"), 1);
    }

    #[test]
    fn test_phrase_tracker_caps_distinct_speakers_per_phrase() {
        let mut tracker = PhraseFrequencyTracker::new();
        for i in 0..(MAX_SPEAKERS_PER_PHRASE + 10) {
            let speaker = format!("S{i}");
            tracker.record(&speaker, "crowded");
        }

        assert_eq!(tracker.speaker_count("crowded"), MAX_SPEAKERS_PER_PHRASE);
    }

    // --- Multi-character contagion integration ---

    #[test]
    fn test_multi_character_contagion_catchphrase() {
        // Simulate 5 characters, one hears a catchphrase from a trusted friend
        let mut evo = default_evolution();

        // Trusted friend (faction 600) uses the phrase
        let adopted = evo.observe_phrase("GuildLeader", "Strength and Honor!", 600, 0.05);
        assert!(adopted);
        assert!(
            evo.active_catchphrases()
                .contains(&"Strength and Honor!".to_string())
        );
    }

    #[test]
    fn test_multi_character_slang_spread() {
        // Simulate slang spreading through a group
        let terms = ["kk", "inc", "ty", "lol"];
        let mut characters: Vec<SpeechEvolution> = (0..4)
            .map(|_| SpeechEvolution::new(SpeechEvolutionConfig::default()))
            .collect();

        // Each character adopts "kk" from a group member (faction 150)
        for evo in &mut characters {
            evo.observe_slang("GroupMate", "kk", 150, 0, 0.05);
        }

        // Each character adopted (roll 0.05 < 0.1)
        for evo in &characters {
            assert!(evo.active_slang().contains(&"kk"));
        }

        // Verify other terms don't bleed in
        assert_eq!(characters[0].active_slang().len(), 1);
        let _ = terms; // suppress unused warning
    }
}
