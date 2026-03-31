use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// Beastlord strategy: pet class with melee DPS and slow.
///
/// Beastlords combine melee DPS with pet management and debuffs:
/// - Keep pet attacking current target
/// - Apply slow to targets (priority debuff)
/// - Melee DPS alongside pet
/// - Pet heals when pet HP is low
pub struct BeastlordStrategy {
    class_id: u8,
}

impl BeastlordStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for BeastlordStrategy {
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
        // Priority: slow > pet heal > DPS spells
        ctx.config.spells.iter().max_by_key(|s| s.priority).cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Beastlord");
    }

    fn aoe_threshold(&self) -> u8 {
        3
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsMelee
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beastlord_role_is_melee_dps() {
        let bl = BeastlordStrategy::new(15);
        assert!(matches!(bl.role(), CombatRole::DpsMelee));
    }
}
