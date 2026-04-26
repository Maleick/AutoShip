//! DLL-side integration of the L-4 contextual-bandit runtime.
//!
//! Bridges [`textquest_learn::BanditRuntime`] to the DLL's [`CombatContext`]
//! and emits shadow-mode decision events to the orchestrator decision log.

use std::path::Path;

use textquest_learn::{
    BanditRuntime, PolicyMode,
    bandit::{
        context::{ContextBuilder, ContextSpec, ContextVec},
        model::BanditScope,
        shadow::ShadowLog,
    },
};

use super::strategy::{CombatContext, GroupMemberState};

// ─── Context-vector builder from CombatContext ────────────────────────────────

/// Build a [`ContextVec`] from the live DLL [`CombatContext`].
///
/// `ability_last_cast[i]` should contain the number of ticks since ability i
/// was last cast, or `u32::MAX` if it has never been cast.  Pass an empty slice
/// when per-ability timing is not available.
pub fn context_from_combat(
    ctx: &CombatContext<'_>,
    gcd_remaining: u32,
    gcd_max: u32,
    holyshit_ready: bool,
    camp_phase: f32,
    ability_last_cast: &[(u32, u32)], // (elapsed_ticks, max_window) per ability slot
) -> ContextVec {
    let own_hp = ctx.player.hp_pct() / 100.0;
    let own_mana = ctx.player.mana_pct() / 100.0;
    let own_end = ctx.player.endurance_pct() / 100.0;

    let (party_hp_min, party_hp_mean, party_mana_min, party_mana_mean) =
        party_stats(ctx.group_members);

    let adds_count = ctx.nearby_enemies.len();

    // Count detrimental debuffs on group members as a proxy for debuff pressure.
    let debuff_count = ctx
        .group_members
        .iter()
        .filter(|m| m.has_detrimental)
        .count();

    let mut builder = ContextBuilder::new()
        .own_hp(own_hp)
        .own_mana(own_mana)
        .own_end(own_end)
        .party_hp(party_hp_min, party_hp_mean)
        .party_mana(party_mana_min, party_mana_mean)
        .adds_count(adds_count)
        .debuff_count(debuff_count)
        .gcd_remaining(gcd_remaining, gcd_max)
        .holyshit_ready(holyshit_ready)
        .camp_phase(camp_phase);

    for (slot, &(elapsed, max_window)) in ability_last_cast.iter().enumerate() {
        builder = builder.ability_tscast(slot, elapsed, max_window);
    }

    builder.build()
}

fn party_stats(members: &[GroupMemberState]) -> (f32, f32, f32, f32) {
    if members.is_empty() {
        return (1.0, 1.0, 1.0, 1.0);
    }
    let alive: Vec<&GroupMemberState> = members.iter().filter(|m| !m.is_dead).collect();
    if alive.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let n = alive.len() as f32;
    let hp_min = alive.iter().map(|m| m.hp_pct).fold(f32::MAX, f32::min) / 100.0;
    let hp_mean = alive.iter().map(|m| m.hp_pct).sum::<f32>() / (n * 100.0);
    let mana_min = alive.iter().map(|m| m.mana_pct).fold(f32::MAX, f32::min) / 100.0;
    let mana_mean = alive.iter().map(|m| m.mana_pct).sum::<f32>() / (n * 100.0);
    (hp_min, hp_mean, mana_min, mana_mean)
}

// ─── DLL bandit decision point ────────────────────────────────────────────────

/// A loaded, ready-to-serve bandit decision point for one scope.
///
/// Holds a [`BanditRuntime`] and a file-backed [`ShadowLog`].  The combat loop
/// calls [`BanditDecisionPoint::serve`] once per decision; it runs in ≤50 μs.
pub struct BanditDecisionPoint {
    runtime: BanditRuntime,
    shadow_log: ShadowLog<std::fs::File>,
}

impl BanditDecisionPoint {
    /// Load a model from `model_path` and open `log_path` for shadow logging.
    pub fn load(
        model_path: &Path,
        log_path: &Path,
        expected_spec: &ContextSpec,
    ) -> Result<Self, textquest_learn::BanditError> {
        let runtime = BanditRuntime::load(model_path, expected_spec)?;
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(textquest_learn::BanditError::Io)?;
        let scope = BanditScope::new(
            runtime.scope_key().split('.').next().unwrap_or("unknown"),
            runtime.scope_key().split('.').nth(1).unwrap_or("unknown"),
        );
        let shadow_log = ShadowLog::new(log_file, scope);
        Ok(Self {
            runtime,
            shadow_log,
        })
    }

    /// Set the policy mode (shadow vs live).
    pub fn set_policy(&mut self, mode: PolicyMode) {
        self.runtime.policy_mode = mode;
    }

    /// Run a bandit decision.
    ///
    /// `rule_arm` is the arm chosen by the rule-based rotation.
    /// In shadow mode the rule arm executes and both are logged.
    /// In live mode the bandit arm executes.
    pub fn serve(&mut self, ctx: &ContextVec, rule_arm: usize) -> usize {
        self.runtime
            .serve_with_shadow(ctx, rule_arm, &mut self.shadow_log)
    }

    pub fn policy_mode(&self) -> &PolicyMode {
        &self.runtime.policy_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_learn::bandit::{
        context::{CTX_DIM, ContextSpec},
        linucb::LinUcbModel,
        model::{AlgorithmTag, BanditScope, MODEL_FILE_VERSION, ModelFile},
    };

    fn make_test_model(spec: &ContextSpec) -> Vec<u8> {
        let d = spec.dim as usize;
        let linucb = LinUcbModel::new(&["CH", "Heal"], d, 1.0);
        let file = ModelFile {
            version: MODEL_FILE_VERSION,
            context_spec_hash: spec.hash_fingerprint(),
            scope: BanditScope::new("cleric", "heal_picker"),
            algorithm: AlgorithmTag::LinUcb,
            exploration_param: 1.0,
            context_dim: spec.dim,
            arms: linucb.arms.iter().map(|a| a.to_serialized()).collect(),
        };
        file.to_bytes().unwrap()
    }

    #[test]
    fn context_vector_has_correct_dim() {
        let v: ContextVec = [0.0; CTX_DIM];
        assert_eq!(v.len(), 32);
    }

    #[test]
    fn serve_in_shadow_returns_rule_arm() {
        use textquest_learn::{
            BanditRuntime,
            bandit::{context::ContextSpec, shadow::ShadowLog},
        };

        let spec = ContextSpec::base();
        let bytes = make_test_model(&spec);
        let mut runtime = BanditRuntime::from_bytes(&bytes, &spec).unwrap();

        let ctx = [0.5f32; CTX_DIM];
        let rule_arm = 1usize;
        let buf: Vec<u8> = Vec::new();
        let scope = BanditScope::new("cleric", "heal_picker");
        let mut log = ShadowLog::new(buf, scope);
        let result = runtime.serve_with_shadow(&ctx, rule_arm, &mut log);
        assert_eq!(result, rule_arm, "shadow → rule arm must execute");
    }
}
