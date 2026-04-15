use textquest_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext, PetAction};

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

    fn pet_action(&self, ctx: &CombatContext) -> Option<PetAction> {
        strategy::pet_attack_action(ctx)
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
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use textquest_common::{
        combat::{CombatConfig, SpellEntry},
        types::SpawnData,
    };

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
            extended_targets: None,
        }
    }

    #[test]
    fn beastlord_class_id() {
        let bl = BeastlordStrategy::new(15);
        assert_eq!(bl.class_id(), 15);
    }

    #[test]
    fn beastlord_role_is_melee_dps() {
        let bl = BeastlordStrategy::new(15);
        assert!(matches!(bl.role(), CombatRole::DpsMelee));
    }

    #[test]
    fn beastlord_aoe_threshold() {
        let bl = BeastlordStrategy::new(15);
        assert_eq!(bl.aoe_threshold(), 3);
    }

    #[test]
    fn beastlord_should_assist() {
        let bl = BeastlordStrategy::new(15);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[], false);
        assert!(bl.should_assist(&ctx));
    }

    #[test]
    fn select_target_in_combat_uses_assist() {
        let bl = BeastlordStrategy::new(15);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 77,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &[], true);
        assert_eq!(bl.select_target(&ctx), Some(77));
    }

    #[test]
    fn select_target_out_of_combat_uses_nearest() {
        let bl = BeastlordStrategy::new(15);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 200.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 25.0,
                ..SpawnData::default()
            },
        ];
        let ctx = make_ctx(&player, None, &enemies, false);
        assert_eq!(bl.select_target(&ctx), Some(2));
    }

    #[test]
    fn select_target_out_of_combat_no_enemies() {
        let bl = BeastlordStrategy::new(15);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[], false);
        assert!(bl.select_target(&ctx).is_none());
    }

    #[test]
    fn select_spell_highest_priority() {
        let bl = BeastlordStrategy::new(15);
        let player = SpawnData::default();
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "Slow".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 10,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "PetHeal".into(),
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
            extended_targets: None,
        };
        let spell = bl.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Slow");
    }

    #[test]
    fn select_spell_none_when_empty() {
        let bl = BeastlordStrategy::new(15);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[], false);
        assert!(bl.select_spell(&ctx).is_none());
    }
}
