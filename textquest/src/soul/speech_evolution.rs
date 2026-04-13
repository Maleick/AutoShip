//! Speech style evolution — catchphrase learning mechanics for Soul Engine characters.

use std::collections::VecDeque;
use std::time::Instant;

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
    /// Base probability (0.0–1.0) of adopting an observed phrase.
    pub adoption_chance: f32,
    /// Multiplier applied to adoption_chance when trust_level >= 1.0.
    pub trust_multiplier: f32,
}

impl Default for SpeechEvolutionConfig {
    fn default() -> Self {
        Self {
            max_catchphrases: 5,
            adoption_chance: 0.1,
            trust_multiplier: 2.0,
        }
    }
}

/// Tracks catchphrase observations and learned phrases for a single character.
#[derive(Debug)]
pub struct SpeechEvolution {
    config: SpeechEvolutionConfig,
    history: VecDeque<CatchphraseEvent>,
    catchphrases: Vec<String>,
}

impl SpeechEvolution {
    pub fn new(config: SpeechEvolutionConfig) -> Self {
        Self {
            config,
            history: VecDeque::new(),
            catchphrases: Vec::new(),
        }
    }

    /// Observe another character using a phrase. Returns true if the phrase was adopted.
    ///
    /// - `trust_level`: 0.0–1.0+, where >= 1.0 activates the trust multiplier
    /// - `rng_roll`: caller-provided random value in 0.0–1.0
    pub fn observe_phrase(
        &mut self,
        source: &str,
        phrase: &str,
        trust_level: f32,
        rng_roll: f32,
    ) -> bool {
        self.history.push_back(CatchphraseEvent {
            source_character: source.to_string(),
            phrase: phrase.to_string(),
            timestamp: Instant::now(),
        });

        // Don't adopt duplicates
        if self.catchphrases.iter().any(|p| p == phrase) {
            return false;
        }

        let threshold = if trust_level >= 1.0 {
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

    /// Returns the currently active catchphrases.
    pub fn active_catchphrases(&self) -> &[String] {
        &self.catchphrases
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

    #[test]
    fn test_default_config_values() {
        let config = SpeechEvolutionConfig::default();
        assert_eq!(config.max_catchphrases, 5);
        assert!((config.adoption_chance - 0.1).abs() < f32::EPSILON);
        assert!((config.trust_multiplier - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_adopt_phrase_low_roll() {
        let mut evo = default_evolution();
        // roll 0.05 < adoption_chance 0.1 → adopted
        let adopted = evo.observe_phrase("Gandalf", "You shall not pass!", 0.5, 0.05);
        assert!(adopted);
        assert_eq!(evo.active_catchphrases(), &["You shall not pass!"]);
    }

    #[test]
    fn test_reject_phrase_high_roll() {
        let mut evo = default_evolution();
        // roll 0.5 >= adoption_chance 0.1 → rejected
        let adopted = evo.observe_phrase("Gandalf", "You shall not pass!", 0.5, 0.5);
        assert!(!adopted);
        assert!(evo.active_catchphrases().is_empty());
    }

    #[test]
    fn test_trust_multiplier_enables_adoption() {
        let mut evo = default_evolution();
        // trust >= 1.0 → threshold = 0.1 * 2.0 = 0.2
        // roll 0.15 < 0.2 → adopted
        let adopted = evo.observe_phrase("TrustedFriend", "For the Horde!", 1.0, 0.15);
        assert!(adopted);
        assert_eq!(evo.active_catchphrases(), &["For the Horde!"]);
    }

    #[test]
    fn test_trust_multiplier_not_applied_below_threshold() {
        let mut evo = default_evolution();
        // trust < 1.0 → threshold = 0.1 (no multiplier)
        // roll 0.15 >= 0.1 → rejected
        let adopted = evo.observe_phrase("Stranger", "For the Horde!", 0.9, 0.15);
        assert!(!adopted);
        assert!(evo.active_catchphrases().is_empty());
    }

    #[test]
    fn test_no_duplicate_catchphrases() {
        let mut evo = default_evolution();
        let first = evo.observe_phrase("A", "Leroy!", 0.5, 0.01);
        assert!(first);
        // Same phrase, low roll — should still be rejected as duplicate
        let second = evo.observe_phrase("B", "Leroy!", 0.5, 0.01);
        assert!(!second);
        assert_eq!(evo.active_catchphrases().len(), 1);
    }

    #[test]
    fn test_max_catchphrases_enforced() {
        let config = SpeechEvolutionConfig {
            max_catchphrases: 3,
            adoption_chance: 1.0, // always adopt
            ..Default::default()
        };
        let mut evo = SpeechEvolution::new(config);

        evo.observe_phrase("A", "phrase1", 0.0, 0.0);
        evo.observe_phrase("B", "phrase2", 0.0, 0.0);
        evo.observe_phrase("C", "phrase3", 0.0, 0.0);
        assert_eq!(evo.active_catchphrases().len(), 3);

        // Adding a 4th should evict the oldest
        evo.observe_phrase("D", "phrase4", 0.0, 0.0);
        assert_eq!(evo.active_catchphrases().len(), 3);
        assert_eq!(
            evo.active_catchphrases(),
            &["phrase2", "phrase3", "phrase4"]
        );
    }

    #[test]
    fn test_forget_oldest_removes_first() {
        let mut evo = default_evolution();
        evo.observe_phrase("A", "alpha", 0.5, 0.01);
        evo.observe_phrase("B", "beta", 0.5, 0.01);
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
        evo.observe_phrase("A", "hello", 0.5, 0.99); // rejected
        evo.observe_phrase("B", "world", 0.5, 0.01); // adopted
        assert_eq!(evo.history().len(), 2);
        assert_eq!(evo.history()[0].source_character, "A");
        assert_eq!(evo.history()[1].phrase, "world");
    }

    #[test]
    fn test_zero_adoption_chance_never_adopts() {
        let config = SpeechEvolutionConfig {
            adoption_chance: 0.0,
            ..Default::default()
        };
        let mut evo = SpeechEvolution::new(config);
        // Even with roll 0.0, threshold is 0.0 and 0.0 < 0.0 is false
        let adopted = evo.observe_phrase("A", "test", 2.0, 0.0);
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
        // trust >= 1.0 → threshold = 0.1 * 5.0 = 0.5
        // roll 0.4 < 0.5 → adopted
        let adopted = evo.observe_phrase("Boss", "Get to the chopper!", 1.5, 0.4);
        assert!(adopted);
    }
}
