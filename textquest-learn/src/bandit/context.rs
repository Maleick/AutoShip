//! Fixed-width context vector schema for in-rotation bandit decisions.
//!
//! All 32 slots are always transmitted; unused slots are `0.0`.  The
//! `ContextSpec` describes which slots are populated for a given scope and
//! its hash guards against schema drift at model-load time.

use serde::{Deserialize, Serialize};
use siphasher::sip::SipHasher13;
use std::hash::{Hash, Hasher};

/// Fixed context-vector dimension.  Changing this is a breaking model format change.
pub const CTX_DIM: usize = 32;

/// A context observation fed to a bandit arm.
pub type ContextVec = [f32; CTX_DIM];

/// Slot assignments (indices into [`ContextVec`]).
pub mod slots {
    pub const OWN_HP: usize = 0;
    pub const OWN_MANA: usize = 1;
    pub const OWN_END: usize = 2;
    pub const PARTY_HP_MIN: usize = 3;
    pub const PARTY_HP_MEAN: usize = 4;
    pub const PARTY_MANA_MIN: usize = 5;
    pub const PARTY_MANA_MEAN: usize = 6;
    pub const ADDS_COUNT_NORM: usize = 7;
    pub const DEBUFF_COUNT_NORM: usize = 8;
    pub const GCD_REMAINING_NORM: usize = 9;
    pub const HOLYSHIT_READY: usize = 10;
    pub const CAMP_PHASE_NORM: usize = 11;
    /// Slots 12–31 are per-ability time-since-last-cast (up to 20 abilities).
    pub const ABILITY_TSCAST_BASE: usize = 12;
    pub const ABILITY_TSCAST_LEN: usize = 20;
}

/// Human-readable description of which context slots are active for a scope.
/// Its SipHash-derived fingerprint is stored in [`ModelFile`] and checked on load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSpec {
    /// Number of meaningful dimensions (must be ≤ [`CTX_DIM`]).
    pub dim: u32,
    /// Ordered field names, one per slot (length == dim).
    pub field_names: Vec<String>,
}

impl ContextSpec {
    /// Standard 12-field base spec (no per-ability slots).
    pub fn base() -> Self {
        Self {
            dim: 12,
            field_names: vec![
                "own_hp".into(),
                "own_mana".into(),
                "own_end".into(),
                "party_hp_min".into(),
                "party_hp_mean".into(),
                "party_mana_min".into(),
                "party_mana_mean".into(),
                "adds_count_norm".into(),
                "debuff_count_norm".into(),
                "gcd_remaining_norm".into(),
                "holyshit_ready".into(),
                "camp_phase_norm".into(),
            ],
        }
    }

    /// Spec with per-ability time-since-last-cast slots appended.
    pub fn with_abilities(ability_names: &[&str]) -> Self {
        assert!(
            ability_names.len() <= slots::ABILITY_TSCAST_LEN,
            "at most {} ability slots",
            slots::ABILITY_TSCAST_LEN
        );
        let mut spec = Self::base();
        spec.dim += ability_names.len() as u32;
        for name in ability_names {
            spec.field_names.push(format!("tscast_{name}"));
        }
        spec
    }

    /// SipHash-13 fingerprint of this spec — stored in the model file as a schema guard.
    pub fn hash_fingerprint(&self) -> u64 {
        let mut h = SipHasher13::new();
        self.dim.hash(&mut h);
        self.field_names.hash(&mut h);
        h.finish()
    }
}

/// Builder for constructing a [`ContextVec`] from live game state.
#[derive(Debug, Default)]
pub struct ContextBuilder {
    ctx: ContextVec,
}

impl ContextBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn own_hp(mut self, frac: f32) -> Self {
        self.ctx[slots::OWN_HP] = frac.clamp(0.0, 1.0);
        self
    }
    pub fn own_mana(mut self, frac: f32) -> Self {
        self.ctx[slots::OWN_MANA] = frac.clamp(0.0, 1.0);
        self
    }
    pub fn own_end(mut self, frac: f32) -> Self {
        self.ctx[slots::OWN_END] = frac.clamp(0.0, 1.0);
        self
    }
    pub fn party_hp(mut self, min: f32, mean: f32) -> Self {
        self.ctx[slots::PARTY_HP_MIN] = min.clamp(0.0, 1.0);
        self.ctx[slots::PARTY_HP_MEAN] = mean.clamp(0.0, 1.0);
        self
    }
    pub fn party_mana(mut self, min: f32, mean: f32) -> Self {
        self.ctx[slots::PARTY_MANA_MIN] = min.clamp(0.0, 1.0);
        self.ctx[slots::PARTY_MANA_MEAN] = mean.clamp(0.0, 1.0);
        self
    }
    /// `count` raw adds count; normalized as `count / 8.0` capped at 1.
    pub fn adds_count(mut self, count: usize) -> Self {
        self.ctx[slots::ADDS_COUNT_NORM] = (count as f32 / 8.0).min(1.0);
        self
    }
    pub fn debuff_count(mut self, count: usize) -> Self {
        self.ctx[slots::DEBUFF_COUNT_NORM] = (count as f32 / 16.0).min(1.0);
        self
    }
    /// `remaining` GCD ticks; `max_gcd` is the full GCD window in ticks (e.g. 30).
    pub fn gcd_remaining(mut self, remaining: u32, max_gcd: u32) -> Self {
        let denom = max_gcd.max(1) as f32;
        self.ctx[slots::GCD_REMAINING_NORM] = (remaining as f32 / denom).clamp(0.0, 1.0);
        self
    }
    pub fn holyshit_ready(mut self, ready: bool) -> Self {
        self.ctx[slots::HOLYSHIT_READY] = if ready { 1.0 } else { 0.0 };
        self
    }
    /// `phase` in [0.0, 1.0].
    pub fn camp_phase(mut self, phase: f32) -> Self {
        self.ctx[slots::CAMP_PHASE_NORM] = phase.clamp(0.0, 1.0);
        self
    }
    /// Set a per-ability time-since-last-cast slot (index 0..ABILITY_TSCAST_LEN).
    /// `elapsed_ticks` normalized to [0,1] over `max_window` ticks.
    pub fn ability_tscast(mut self, slot: usize, elapsed_ticks: u32, max_window: u32) -> Self {
        assert!(slot < slots::ABILITY_TSCAST_LEN);
        let denom = max_window.max(1) as f32;
        self.ctx[slots::ABILITY_TSCAST_BASE + slot] =
            (elapsed_ticks as f32 / denom).clamp(0.0, 1.0);
        self
    }

    pub fn build(self) -> ContextVec {
        self.ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctx_dim_is_32() {
        let v: ContextVec = [0.0; CTX_DIM];
        assert_eq!(v.len(), 32);
    }

    #[test]
    fn spec_hash_stable() {
        let s = ContextSpec::base();
        let h1 = s.hash_fingerprint();
        let h2 = s.hash_fingerprint();
        assert_eq!(h1, h2);
    }

    #[test]
    fn spec_hash_differs_on_change() {
        let s1 = ContextSpec::base();
        let s2 = ContextSpec::with_abilities(&["heal", "cure"]);
        assert_ne!(s1.hash_fingerprint(), s2.hash_fingerprint());
    }

    #[test]
    fn builder_clamps_and_sets() {
        let ctx = ContextBuilder::new()
            .own_hp(0.75)
            .own_mana(1.5) // should clamp to 1.0
            .adds_count(4)
            .build();
        assert_eq!(ctx[slots::OWN_HP], 0.75);
        assert_eq!(ctx[slots::OWN_MANA], 1.0);
        assert!((ctx[slots::ADDS_COUNT_NORM] - 0.5).abs() < 1e-6);
    }
}
