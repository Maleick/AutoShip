use textquest_common::combat::{CombatRole, SpellEntry};

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
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use textquest_common::combat::{CombatConfig, SpellEntry};
    use textquest_common::types::SpawnData;

    static DEFAULT_CONFIG: std::sync::LazyLock<CombatConfig> =
        std::sync::LazyLock::new(CombatConfig::default);

    fn make_ctx<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        enemies: &'a [SpawnData],
        in_combat: bool,
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: enemies,
            group_members: &[],
            config: &DEFAULT_CONFIG,
            tick: 0,
            in_combat,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
        }
    }

    #[test]
    fn berserker_class_id() {
        let ber = BerserkerStrategy::new(16);
        assert_eq!(ber.class_id(), 16);
    }

    #[test]
    fn berserker_role_is_melee_dps() {
        let ber = BerserkerStrategy::new(16);
        assert!(matches!(ber.role(), CombatRole::DpsMelee));
    }

    #[test]
    fn berserker_aoe_threshold() {
        let ber = BerserkerStrategy::new(16);
        assert_eq!(ber.aoe_threshold(), 2);
    }

    #[test]
    fn berserker_should_assist() {
        let ber = BerserkerStrategy::new(16);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[], false);
        assert!(ber.should_assist(&ctx));
    }

    #[test]
    fn select_target_in_combat_uses_assist() {
        let ber = BerserkerStrategy::new(16);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 42,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &[], true);
        assert_eq!(ber.select_target(&ctx), Some(42));
    }

    #[test]
    fn select_target_out_of_combat_uses_nearest_enemy() {
        let ber = BerserkerStrategy::new(16);
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
                x: 10.0,
                ..SpawnData::default()
            },
        ];
        let ctx = make_ctx(&player, None, &enemies, false);
        assert_eq!(ber.select_target(&ctx), Some(2));
    }

    #[test]
    fn select_target_out_of_combat_no_enemies() {
        let ber = BerserkerStrategy::new(16);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[], false);
        assert!(ber.select_target(&ctx).is_none());
    }

    #[test]
    fn select_spell_highest_priority() {
        let ber = BerserkerStrategy::new(16);
        let player = SpawnData::default();
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "Frenzy".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 10,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Rage".into(),
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
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
        };
        let spell = ber.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Frenzy");
    }

    #[test]
    fn select_spell_none_when_empty() {
        let ber = BerserkerStrategy::new(16);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[], false);
        assert!(ber.select_spell(&ctx).is_none());
    }
}
