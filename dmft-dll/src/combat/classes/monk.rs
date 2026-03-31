use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// Monk strategy: melee DPS + puller, flying kick/round kick priority, feign death escape.
/// EQ class ID: 7
pub struct MonkStrategy {
    class_id: u8,
}

impl MonkStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for MonkStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::assist_target(ctx)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        // Monks primarily use melee skills (flying kick, etc.) via UseSkill.
        // If config has spells (e.g., discs), use highest priority.
        strategy::best_spell_by_mana(ctx)
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Monk");
    }

    fn on_action_complete(&mut self, ctx: &CombatContext) {
        if !ctx.in_combat {
            strategy::melee_on_disengage();
        }
    }

    fn aoe_threshold(&self) -> u8 {
        255 // Monks don't AoE
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsMelee
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dmft_common::combat::CombatConfig;

    fn test_config() -> CombatConfig {
        CombatConfig::default()
    }

    #[test]
    fn monk_class_id() {
        let monk = MonkStrategy::new(7);
        assert_eq!(monk.class_id(), 7);
    }

    #[test]
    fn monk_role_is_melee_dps() {
        let monk = MonkStrategy::new(7);
        assert_eq!(monk.role(), CombatRole::DpsMelee);
    }

    #[test]
    fn monk_should_assist() {
        let monk = MonkStrategy::new(7);
        let config = test_config();
        let player = dmft_common::types::SpawnData::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
        };
        assert!(monk.should_assist(&ctx));
    }
}
