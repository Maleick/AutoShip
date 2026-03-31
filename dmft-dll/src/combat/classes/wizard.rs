use dmft_common::combat::{CombatRole, SpellEntry};

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Wizard strategy: pure nuke DPS. Highest priority spell available, mana-aware.
/// EQ class ID: 12
pub struct WizardStrategy {
    class_id: u8,
}

impl WizardStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }
}

impl ClassStrategy for WizardStrategy {
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
    use dmft_common::combat::CombatConfig;
    use dmft_common::types::SpawnData;

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
        }
    }

    #[test]
    fn wizard_class_id() {
        let wiz = WizardStrategy::new(12);
        assert_eq!(wiz.class_id(), 12);
    }

    #[test]
    fn wizard_role_is_ranged_dps() {
        let wiz = WizardStrategy::new(12);
        assert_eq!(wiz.role(), CombatRole::DpsRanged);
    }

    #[test]
    fn wizard_aoe_threshold() {
        let wiz = WizardStrategy::new(12);
        assert_eq!(wiz.aoe_threshold(), 3);
    }

    #[test]
    fn wizard_should_assist() {
        let wiz = WizardStrategy::new(12);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config);
        assert!(wiz.should_assist(&ctx));
    }

    #[test]
    fn select_target_returns_target_id() {
        let wiz = WizardStrategy::new(12);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 55,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &config);
        assert_eq!(wiz.select_target(&ctx), Some(55));
    }

    #[test]
    fn select_target_none_without_target() {
        let wiz = WizardStrategy::new(12);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config);
        assert!(wiz.select_target(&ctx).is_none());
    }

    #[test]
    fn select_spell_highest_priority_affordable() {
        let wiz = WizardStrategy::new(12);
        let mut player = SpawnData::default();
        player.mana_current = 5000;
        player.mana_max = 10000;
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "IceComet".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 10,
                    min_mana_pct: 40.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Sunstrike".into(),
                    slot: 2,
                    spell_id: 2,
                    priority: 20,
                    min_mana_pct: 80.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, None, &config);
        let spell = wiz.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "IceComet");
    }

    #[test]
    fn select_spell_none_when_oom() {
        let wiz = WizardStrategy::new(12);
        let mut player = SpawnData::default();
        player.mana_current = 10;
        player.mana_max = 10000;
        let config = CombatConfig {
            spells: vec![SpellEntry {
                name: "Nuke".into(),
                slot: 1,
                spell_id: 1,
                priority: 10,
                min_mana_pct: 20.0,
                is_aoe: false,
            }],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, None, &config);
        assert!(wiz.select_spell(&ctx).is_none());
    }

    #[test]
    fn select_spell_none_when_empty() {
        let wiz = WizardStrategy::new(12);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config);
        assert!(wiz.select_spell(&ctx).is_none());
    }
}
