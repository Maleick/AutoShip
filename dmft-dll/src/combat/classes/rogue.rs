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

    fn on_action_complete(&mut self, ctx: &CombatContext) {
        if !ctx.in_combat {
            strategy::melee_on_disengage();
        }
    }

    fn aoe_threshold(&self) -> u8 {
        255 // Rogues don't AoE
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsMelee
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use dmft_common::combat::{CombatConfig, SpellEntry};
    use dmft_common::types::SpawnData;

    fn make_ctx<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        config: &'a CombatConfig,
        in_combat: bool,
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: &[],
            group_members: &[],
            config,
            tick: 0,
            in_combat,
        }
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
        let config = CombatConfig::default();
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &config, false);
        assert!(rogue.should_assist(&ctx));
    }

    #[test]
    fn rogue_no_aoe() {
        let rogue = RogueStrategy::new(9);
        assert_eq!(rogue.aoe_threshold(), 255);
    }

    #[test]
    fn select_target_returns_assist_target() {
        let rogue = RogueStrategy::new(9);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 66,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &config, true);
        assert_eq!(rogue.select_target(&ctx), Some(66));
    }

    #[test]
    fn select_target_none_without_target() {
        let rogue = RogueStrategy::new(9);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config, false);
        assert!(rogue.select_target(&ctx).is_none());
    }

    #[test]
    fn select_spell_highest_priority() {
        let rogue = RogueStrategy::new(9);
        let mut player = SpawnData::default();
        player.mana_current = 1000;
        player.mana_max = 1000;
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "PoisonDisc".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 10,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Backstab".into(),
                    slot: 2,
                    spell_id: 2,
                    priority: 20,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, None, &config, true);
        let spell = rogue.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Backstab");
    }

    #[test]
    fn select_spell_empty_returns_none() {
        let rogue = RogueStrategy::new(9);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config, false);
        assert!(rogue.select_spell(&ctx).is_none());
    }
}
