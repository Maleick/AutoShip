use textquest_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{self, ClassStrategy, CombatContext, PetAction};

/// Magician strategy: pet-based DPS + nukes. Pet management via /pet commands.
/// EQ class ID: 13
pub struct MagicianStrategy {
    class_id: u8,
}

impl MagicianStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for MagicianStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        ctx.target.map(|t| t.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
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

    fn pet_action(&self, ctx: &CombatContext) -> Option<PetAction> {
        strategy::pet_attack_action(ctx)
    }

    fn aoe_threshold(&self) -> u8 {
        3
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsRanged
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use textquest_common::combat::CombatConfig;
    use textquest_common::types::SpawnData;

    fn make_ctx<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        config: &'a CombatConfig,
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: &[],
            group_members: &[],
            config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        }
    }

    #[test]
    fn mage_class_id() {
        let mage = MagicianStrategy::new(13);
        assert_eq!(mage.class_id(), 13);
    }

    #[test]
    fn mage_role_is_ranged_dps() {
        let mage = MagicianStrategy::new(13);
        assert_eq!(mage.role(), CombatRole::DpsRanged);
    }

    #[test]
    fn mage_aoe_threshold() {
        let mage = MagicianStrategy::new(13);
        assert_eq!(mage.aoe_threshold(), 3);
    }

    #[test]
    fn mage_should_assist() {
        let mage = MagicianStrategy::new(13);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config);
        assert!(mage.should_assist(&ctx));
    }

    #[test]
    fn select_target_returns_target_id() {
        let mage = MagicianStrategy::new(13);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 88,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &config);
        assert_eq!(mage.select_target(&ctx), Some(88));
    }

    #[test]
    fn select_target_none_without_target() {
        let mage = MagicianStrategy::new(13);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config);
        assert!(mage.select_target(&ctx).is_none());
    }

    #[test]
    fn select_spell_filters_by_mana() {
        let mage = MagicianStrategy::new(13);
        let mut player = SpawnData::default();
        player.mana_current = 3000;
        player.mana_max = 10000; // 30%
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "BoltOfFire".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 5,
                    min_mana_pct: 10.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "ManaBlaze".into(),
                    slot: 2,
                    spell_id: 2,
                    priority: 15,
                    min_mana_pct: 50.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, None, &config);
        let spell = mage.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "BoltOfFire"); // ManaBlaze too expensive at 30%
    }

    #[test]
    fn select_spell_none_when_empty() {
        let mage = MagicianStrategy::new(13);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config);
        assert!(mage.select_spell(&ctx).is_none());
    }
}
