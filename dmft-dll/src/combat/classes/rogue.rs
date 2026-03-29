use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Rogue strategy: melee DPS, backstab priority, uses configured spells + UseSkill for backstab.
/// EQ class ID: 9
pub struct RogueStrategy {
    class_id: u8,
    backstab_cooldown: u32,
}

impl RogueStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            backstab_cooldown: 0,
        }
    }
}

impl ClassStrategy for RogueStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        // Rogue assists the main assist target
        ctx.target.map(|t| t.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        // Rogues primarily use melee skills (backstab via UseSkill), not spells.
        // If config has spells (e.g., poison proc discs), use highest priority.
        // Otherwise return None — the combat tick handles melee skill usage.
        let mana_pct = ctx.player.mana_pct();
        ctx.config
            .spells
            .iter()
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        if let Some(target) = ctx.target {
            tracing::info!(
                target_id = target.spawn_id,
                target_name = %target.name,
                "Rogue engaging — backstab ready"
            );
            // Enable auto-attack on engage
            crate::eq::toggle_auto_attack(true);
        }
    }

    fn on_action_complete(&mut self, _ctx: &CombatContext) {
        crate::eq::toggle_auto_attack(false);
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
