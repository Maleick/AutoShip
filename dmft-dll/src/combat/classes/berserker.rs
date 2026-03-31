use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// Berserker strategy: pure melee DPS with frenzy/rage abilities.
///
/// Berserkers are the highest sustained melee DPS class:
/// - Frenzy (primary attack ability)
/// - Rage/bloodlust buffs
/// - No spells — all disc/ability based
/// - Can throw axes at range
pub struct BerserkerStrategy {
    class_id: u8,
}

impl BerserkerStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for BerserkerStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        if ctx.in_combat {
            strategy::assist_target(ctx)
        } else {
            strategy::nearest_enemy(ctx.player, ctx.nearby_enemies).map(|s| s.spawn_id)
        }
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        // Berserkers use abilities (modeled as spells with high priority).
        ctx.config.spells.iter().max_by_key(|s| s.priority).cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Berserker");
    }

    fn aoe_threshold(&self) -> u8 {
        2 // Berserkers excel at AoE with frenzy
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsMelee
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn berserker_role_is_melee_dps() {
        let ber = BerserkerStrategy::new(16);
        assert!(matches!(ber.role(), CombatRole::DpsMelee));
    }
}
