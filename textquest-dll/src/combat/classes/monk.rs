use textquest_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// Monk strategy: melee DPS + puller, flying kick/round kick priority, feign
/// death escape. EQ class ID: 7
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
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use textquest_common::{
        combat::{CombatConfig, SpellEntry},
        types::SpawnData,
    };

    fn test_config() -> CombatConfig {
        CombatConfig::default()
    }

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
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        }
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
    fn monk_no_aoe() {
        let monk = MonkStrategy::new(7);
        assert_eq!(monk.aoe_threshold(), 255);
    }

    #[test]
    fn monk_should_assist() {
        let monk = MonkStrategy::new(7);
        let config = test_config();
        let player = textquest_common::types::SpawnData::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        assert!(monk.should_assist(&ctx));
    }

    #[test]
    fn select_target_returns_assist_target() {
        let monk = MonkStrategy::new(7);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 99,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &config, true);
        assert_eq!(monk.select_target(&ctx), Some(99));
    }

    #[test]
    fn select_target_none_without_target() {
        let monk = MonkStrategy::new(7);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config, true);
        assert!(monk.select_target(&ctx).is_none());
    }

    #[test]
    fn select_spell_uses_best_by_mana() {
        let monk = MonkStrategy::new(7);
        let mut player = SpawnData::default();
        player.mana_current = 500;
        player.mana_max = 1000;
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "FlyingKick".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 10,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Thunderfoot".into(),
                    slot: 2,
                    spell_id: 2,
                    priority: 20,
                    min_mana_pct: 80.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, None, &config, true);
        let spell = monk.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "FlyingKick");
    }

    #[test]
    fn select_spell_empty_returns_none() {
        let monk = MonkStrategy::new(7);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config, false);
        assert!(monk.select_spell(&ctx).is_none());
    }
}
