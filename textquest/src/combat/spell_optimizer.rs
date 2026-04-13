//! Advanced spell database and casting optimization.
//!
//! Provides `SpellOptimizer` for ranking and filtering spell candidates by
//! mana efficiency, cast time, and mana cost, and `CastingPredictor` for
//! computing haste-adjusted cast times.

/// A spell candidate with all attributes needed for casting optimization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellCandidate {
    /// Unique spell identifier.
    pub spell_id: u32,
    /// Human-readable spell name.
    pub name: String,
    /// Mana cost to cast the spell.
    pub mana_cost: u32,
    /// Base cast time in milliseconds.
    pub cast_time_ms: u32,
    /// Effect duration in seconds.
    pub duration_secs: u32,
    /// Damage dealt by the spell.
    pub damage: u32,
    /// Resist modifier applied to the target (negative = easier to resist).
    pub resist_mod: i8,
}

/// Optimizer for selecting the best spell from a set of candidates.
///
/// Spells are stored in insertion order; ranking methods return references
/// sorted on demand so add_candidate remains O(1).
#[derive(Debug, Default)]
pub struct SpellOptimizer {
    candidates: Vec<SpellCandidate>,
}

impl SpellOptimizer {
    /// Create a new, empty optimizer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a spell candidate to the pool.
    pub fn add_candidate(&mut self, spell: SpellCandidate) {
        self.candidates.push(spell);
    }

    /// Compute the mana efficiency (damage per mana) of a spell.
    ///
    /// Returns `0.0` when `mana_cost` is zero to avoid division by zero.
    #[must_use]
    pub fn mana_efficiency(&self, spell: &SpellCandidate) -> f32 {
        if spell.mana_cost == 0 {
            0.0
        } else {
            spell.damage as f32 / spell.mana_cost as f32
        }
    }

    /// Return the spell with the highest mana efficiency, or `None` if the
    /// candidate pool is empty.
    #[must_use]
    pub fn best_efficiency(&self) -> Option<&SpellCandidate> {
        self.candidates.iter().max_by(|a, b| {
            self.mana_efficiency(a)
                .partial_cmp(&self.mana_efficiency(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Return all spells whose cast time is at or below `max_ms`.
    #[must_use]
    pub fn filter_by_cast_time(&self, max_ms: u32) -> Vec<&SpellCandidate> {
        self.candidates
            .iter()
            .filter(|s| s.cast_time_ms <= max_ms)
            .collect()
    }

    /// Return all spells whose mana cost is at or below `max_mana`.
    #[must_use]
    pub fn filter_by_mana(&self, max_mana: u32) -> Vec<&SpellCandidate> {
        self.candidates
            .iter()
            .filter(|s| s.mana_cost <= max_mana)
            .collect()
    }

    /// Return all spells sorted by mana efficiency, highest first.
    #[must_use]
    pub fn ranked_by_efficiency(&self) -> Vec<&SpellCandidate> {
        let mut ranked: Vec<&SpellCandidate> = self.candidates.iter().collect();
        ranked.sort_by(|a, b| {
            self.mana_efficiency(b)
                .partial_cmp(&self.mana_efficiency(a))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        ranked
    }
}

/// Predicts actual cast time after applying haste.
#[derive(Debug, Clone, PartialEq)]
pub struct CastingPredictor {
    /// Base cast time in milliseconds (before haste).
    pub base_cast_time_ms: u32,
    /// Haste percentage (e.g., `25.0` means 25% faster).
    pub haste_pct: f32,
}

impl CastingPredictor {
    /// Minimum enforced cast time (milliseconds).
    pub const MIN_CAST_MS: u32 = 200;

    /// Create a new predictor.
    #[must_use]
    pub fn new(base_cast_time_ms: u32, haste_pct: f32) -> Self {
        Self {
            base_cast_time_ms,
            haste_pct,
        }
    }

    /// Compute the haste-adjusted cast time, clamped to a minimum of 200 ms.
    ///
    /// Formula: `base * (1 - haste_pct / 100)`, rounded to the nearest
    /// millisecond, with a floor of [`Self::MIN_CAST_MS`].
    ///
    /// Non-finite haste values fall back to the base cast time. Finite haste
    /// values are clamped to `0.0..=100.0` before applying the formula.
    #[must_use]
    pub fn predicted_cast_ms(&self) -> u32 {
        let haste_pct = if self.haste_pct.is_finite() {
            self.haste_pct.clamp(0.0, 100.0)
        } else {
            0.0
        };

        let adjusted = self.base_cast_time_ms as f32 * (1.0 - haste_pct / 100.0);
        let result = adjusted.round() as u32;
        result.max(Self::MIN_CAST_MS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_spell(
        spell_id: u32,
        name: &str,
        mana_cost: u32,
        cast_time_ms: u32,
        damage: u32,
    ) -> SpellCandidate {
        SpellCandidate {
            spell_id,
            name: name.to_string(),
            mana_cost,
            cast_time_ms,
            duration_secs: 0,
            damage,
            resist_mod: 0,
        }
    }

    // --- mana_efficiency ---

    #[test]
    fn test_mana_efficiency_normal() {
        let opt = SpellOptimizer::new();
        let spell = make_spell(1, "Fireball", 100, 2500, 300);
        let eff = opt.mana_efficiency(&spell);
        assert!(
            (eff - 3.0_f32).abs() < f32::EPSILON,
            "expected 3.0, got {eff}"
        );
    }

    #[test]
    fn test_mana_efficiency_zero_mana() {
        let opt = SpellOptimizer::new();
        let spell = make_spell(2, "FreeSpell", 0, 1000, 200);
        assert_eq!(opt.mana_efficiency(&spell), 0.0);
    }

    #[test]
    fn test_mana_efficiency_zero_damage() {
        let opt = SpellOptimizer::new();
        let spell = make_spell(3, "Buff", 50, 3000, 0);
        assert_eq!(opt.mana_efficiency(&spell), 0.0);
    }

    // --- best_efficiency ---

    #[test]
    fn test_best_efficiency_empty() {
        let opt = SpellOptimizer::new();
        assert!(opt.best_efficiency().is_none());
    }

    #[test]
    fn test_best_efficiency_single() {
        let mut opt = SpellOptimizer::new();
        opt.add_candidate(make_spell(1, "Bolt", 100, 2000, 500));
        let best = opt.best_efficiency().expect("should have a best spell");
        assert_eq!(best.spell_id, 1);
    }

    #[test]
    fn test_best_efficiency_selects_highest() {
        let mut opt = SpellOptimizer::new();
        // efficiency: 2.0
        opt.add_candidate(make_spell(1, "Weak", 100, 2000, 200));
        // efficiency: 5.0
        opt.add_candidate(make_spell(2, "Strong", 100, 2000, 500));
        // efficiency: 3.0
        opt.add_candidate(make_spell(3, "Middle", 100, 2000, 300));
        let best = opt.best_efficiency().expect("should have a best spell");
        assert_eq!(best.spell_id, 2, "Strong should be best");
    }

    // --- filter_by_cast_time ---

    #[test]
    fn test_filter_by_cast_time_all_pass() {
        let mut opt = SpellOptimizer::new();
        opt.add_candidate(make_spell(1, "Fast", 50, 500, 100));
        opt.add_candidate(make_spell(2, "Medium", 80, 1500, 200));
        let result = opt.filter_by_cast_time(2000);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_by_cast_time_none_pass() {
        let mut opt = SpellOptimizer::new();
        opt.add_candidate(make_spell(1, "Slow", 100, 3000, 300));
        let result = opt.filter_by_cast_time(500);
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_by_cast_time_exact_boundary() {
        let mut opt = SpellOptimizer::new();
        opt.add_candidate(make_spell(1, "Exact", 100, 1000, 200));
        let result = opt.filter_by_cast_time(1000);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filter_by_cast_time_partial() {
        let mut opt = SpellOptimizer::new();
        opt.add_candidate(make_spell(1, "Instant", 30, 200, 80));
        opt.add_candidate(make_spell(2, "Long", 120, 4000, 400));
        let result = opt.filter_by_cast_time(1000);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].spell_id, 1);
    }

    // --- filter_by_mana ---

    #[test]
    fn test_filter_by_mana_all_pass() {
        let mut opt = SpellOptimizer::new();
        opt.add_candidate(make_spell(1, "Cheap", 20, 1000, 100));
        opt.add_candidate(make_spell(2, "Medium", 50, 1500, 200));
        let result = opt.filter_by_mana(100);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_by_mana_none_pass() {
        let mut opt = SpellOptimizer::new();
        opt.add_candidate(make_spell(1, "Pricey", 300, 2000, 500));
        let result = opt.filter_by_mana(100);
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_by_mana_exact_boundary() {
        let mut opt = SpellOptimizer::new();
        opt.add_candidate(make_spell(1, "Exact", 100, 2000, 200));
        let result = opt.filter_by_mana(100);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filter_by_mana_partial() {
        let mut opt = SpellOptimizer::new();
        opt.add_candidate(make_spell(1, "Cheap", 40, 1000, 100));
        opt.add_candidate(make_spell(2, "Expensive", 500, 2000, 1000));
        let result = opt.filter_by_mana(100);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].spell_id, 1);
    }

    // --- ranked_by_efficiency ---

    #[test]
    fn test_ranked_by_efficiency_order() {
        let mut opt = SpellOptimizer::new();
        opt.add_candidate(make_spell(1, "Low", 100, 2000, 100)); // eff 1.0
        opt.add_candidate(make_spell(2, "High", 100, 2000, 500)); // eff 5.0
        opt.add_candidate(make_spell(3, "Mid", 100, 2000, 300)); // eff 3.0
        let ranked = opt.ranked_by_efficiency();
        assert_eq!(ranked[0].spell_id, 2, "highest first");
        assert_eq!(ranked[1].spell_id, 3);
        assert_eq!(ranked[2].spell_id, 1, "lowest last");
    }

    #[test]
    fn test_ranked_by_efficiency_empty() {
        let opt = SpellOptimizer::new();
        assert!(opt.ranked_by_efficiency().is_empty());
    }

    // --- predicted_cast_ms ---

    #[test]
    fn test_predicted_cast_ms_no_haste() {
        let p = CastingPredictor::new(2000, 0.0);
        assert_eq!(p.predicted_cast_ms(), 2000);
    }

    #[test]
    fn test_predicted_cast_ms_with_haste() {
        // 2000 * (1 - 0.25) = 1500
        let p = CastingPredictor::new(2000, 25.0);
        assert_eq!(p.predicted_cast_ms(), 1500);
    }

    #[test]
    fn test_predicted_cast_ms_minimum_enforced() {
        // 500 * (1 - 0.99) = 5ms → clamped to 200
        let p = CastingPredictor::new(500, 99.0);
        assert_eq!(p.predicted_cast_ms(), CastingPredictor::MIN_CAST_MS);
    }

    #[test]
    fn test_predicted_cast_ms_100_pct_haste() {
        // 100% haste → 0ms → clamped to 200
        let p = CastingPredictor::new(3000, 100.0);
        assert_eq!(p.predicted_cast_ms(), 200);
    }

    #[test]
    fn test_predicted_cast_ms_50_pct_haste() {
        // 1000 * (1 - 0.5) = 500
        let p = CastingPredictor::new(1000, 50.0);
        assert_eq!(p.predicted_cast_ms(), 500);
    }

    // --- additional CastingPredictor edge cases ---

    #[test]
    fn test_predicted_cast_ms_negative_haste_clamped() {
        // Negative haste is clamped to 0 → no effect
        let p = CastingPredictor::new(2000, -50.0);
        assert_eq!(p.predicted_cast_ms(), 2000);
    }

    #[test]
    fn test_predicted_cast_ms_over_100_haste_clamped() {
        // >100% haste clamped to 100% → 0 → MIN_CAST_MS
        let p = CastingPredictor::new(2000, 150.0);
        assert_eq!(p.predicted_cast_ms(), CastingPredictor::MIN_CAST_MS);
    }

    #[test]
    fn test_predicted_cast_ms_nan_haste_fallback() {
        let p = CastingPredictor::new(2000, f32::NAN);
        assert_eq!(p.predicted_cast_ms(), 2000, "NaN haste should fall back to base");
    }

    #[test]
    fn test_predicted_cast_ms_infinity_haste_fallback() {
        let p = CastingPredictor::new(2000, f32::INFINITY);
        assert_eq!(p.predicted_cast_ms(), 2000, "Infinity haste should fall back to base");
    }

    #[test]
    fn test_predicted_cast_ms_neg_infinity_haste_fallback() {
        let p = CastingPredictor::new(2000, f32::NEG_INFINITY);
        assert_eq!(p.predicted_cast_ms(), 2000, "-Infinity haste should fall back to base");
    }

    #[test]
    fn test_predicted_cast_ms_zero_base() {
        let p = CastingPredictor::new(0, 25.0);
        assert_eq!(p.predicted_cast_ms(), CastingPredictor::MIN_CAST_MS);
    }

    // --- additional SpellOptimizer edge cases ---

    #[test]
    fn test_best_efficiency_with_zero_mana_spells() {
        let mut opt = SpellOptimizer::new();
        // Zero mana spells have 0.0 efficiency
        opt.add_candidate(make_spell(1, "Free", 0, 1000, 500));
        opt.add_candidate(make_spell(2, "Paid", 100, 1000, 200));
        let best = opt.best_efficiency().expect("should have a best");
        assert_eq!(best.spell_id, 2, "Paid spell with 2.0 eff beats free with 0.0");
    }

    #[test]
    fn test_ranked_by_efficiency_includes_all_ties() {
        let mut opt = SpellOptimizer::new();
        // Two spells with same efficiency (1.0)
        opt.add_candidate(make_spell(1, "Alpha", 100, 1000, 100));
        opt.add_candidate(make_spell(2, "Beta", 200, 2000, 200));
        let ranked = opt.ranked_by_efficiency();
        assert_eq!(ranked.len(), 2);
        assert!(ranked.iter().any(|spell| spell.spell_id == 1));
        assert!(ranked.iter().any(|spell| spell.spell_id == 2));
    }

    #[test]
    fn test_filter_by_cast_time_zero() {
        let mut opt = SpellOptimizer::new();
        opt.add_candidate(make_spell(1, "Instant", 50, 0, 100));
        opt.add_candidate(make_spell(2, "Slow", 100, 5000, 300));
        let result = opt.filter_by_cast_time(0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].spell_id, 1);
    }

    #[test]
    fn test_filter_by_mana_zero() {
        let mut opt = SpellOptimizer::new();
        opt.add_candidate(make_spell(1, "Free", 0, 1000, 100));
        opt.add_candidate(make_spell(2, "Costly", 100, 1000, 200));
        let result = opt.filter_by_mana(0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].spell_id, 1);
    }

    #[test]
    fn test_spell_candidate_fields() {
        let spell = SpellCandidate {
            spell_id: 42,
            name: "Greater Heal".to_string(),
            mana_cost: 200,
            cast_time_ms: 4000,
            duration_secs: 0,
            damage: 0,
            resist_mod: -10,
        };
        assert_eq!(spell.resist_mod, -10);
        assert_eq!(spell.duration_secs, 0);
    }

    #[test]
    fn test_predictor_clone_and_eq() {
        let p1 = CastingPredictor::new(2000, 25.0);
        let p2 = p1.clone();
        assert_eq!(p1, p2);
    }
}
