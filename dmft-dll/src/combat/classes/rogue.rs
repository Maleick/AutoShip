use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// Rogue strategy: melee DPS, backstab priority, uses configured spells + UseSkill for backstab.
/// EQ class ID: 9
pub struct RogueStrategy {
    class_id: u8,
}

impl RogueStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for RogueStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::assist_target(ctx)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        // Rogues primarily use melee skills (backstab via UseSkill), not spells.
        // If config has spells (e.g., poison proc discs), use highest priority.
        strategy::best_spell_by_mana(ctx)
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Rogue");
    }

    fn on_action_complete(&mut self, _ctx: &CombatContext) {
        strategy::melee_on_disengage();
    }

    fn aoe_threshold(&self) -> u8 {
        255 // Rogues don't AoE
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
    fn rogue_class_id() {
        let rogue = RogueStrategy::new(9);
        assert_eq!(rogue.class_id(), 9);
    }

    #[test]
    fn rogue_role_is_melee_dps() {
        let rogue = RogueStrategy::new(9);
        assert_eq!(rogue.role(), CombatRole::DpsMelee);
    }

    #[test]
    fn rogue_should_assist() {
        let rogue = RogueStrategy::new(9);
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
        assert!(rogue.should_assist(&ctx));
    }

    #[test]
    fn rogue_no_aoe() {
        let rogue = RogueStrategy::new(9);
        assert_eq!(rogue.aoe_threshold(), 255);
    }
}
