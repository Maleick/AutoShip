use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// Warrior strategy: main tank, selects nearest enemy, uses taunt/aggro abilities.
pub struct WarriorStrategy {
    class_id: u8,
}

impl WarriorStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for WarriorStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::nearest_enemy(ctx.player, ctx.nearby_enemies).map(|s| s.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        // Use highest-priority taunt/aggro ability from config spells list.
        ctx.config.spells.iter().max_by_key(|s| s.priority).cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        false // Tank leads, doesn't assist.
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Warrior");
    }

    fn aoe_threshold(&self) -> u8 {
        2
    }

    fn role(&self) -> CombatRole {
        CombatRole::MainTank
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use dmft_common::combat::{CombatConfig, SpellEntry};
    use dmft_common::types::SpawnData;

    static DEFAULT_CONFIG: std::sync::LazyLock<CombatConfig> =
        std::sync::LazyLock::new(CombatConfig::default);

    fn make_ctx<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        enemies: &'a [SpawnData],
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: enemies,
            group_members: &[],
            config: &DEFAULT_CONFIG,
            tick: 0,
            in_combat: true,
        }
    }

    #[test]
    fn warrior_class_id() {
        let w = WarriorStrategy::new(1);
        assert_eq!(w.class_id(), 1);
    }

    #[test]
    fn warrior_role_is_main_tank() {
        let w = WarriorStrategy::new(1);
        assert_eq!(w.role(), CombatRole::MainTank);
    }

    #[test]
    fn warrior_aoe_threshold() {
        let w = WarriorStrategy::new(1);
        assert_eq!(w.aoe_threshold(), 2);
    }

    #[test]
    fn warrior_does_not_assist() {
        let w = WarriorStrategy::new(1);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[]);
        assert!(!w.should_assist(&ctx));
    }

    #[test]
    fn select_target_nearest_enemy() {
        let w = WarriorStrategy::new(1);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 100.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 20.0,
                ..SpawnData::default()
            },
        ];
        let ctx = make_ctx(&player, None, &enemies);
        assert_eq!(w.select_target(&ctx), Some(2));
    }

    #[test]
    fn select_target_no_enemies() {
        let w = WarriorStrategy::new(1);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[]);
        assert!(w.select_target(&ctx).is_none());
    }

    #[test]
    fn select_spell_highest_priority() {
        let w = WarriorStrategy::new(1);
        let player = SpawnData::default();
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "Taunt".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 10,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Bash".into(),
                    slot: 2,
                    spell_id: 2,
                    priority: 5,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
        };
        let spell = w.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Taunt");
    }

    #[test]
    fn select_spell_none_when_empty() {
        let w = WarriorStrategy::new(1);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[]);
        assert!(w.select_spell(&ctx).is_none());
    }
}
