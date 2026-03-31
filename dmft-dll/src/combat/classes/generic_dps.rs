use dmft_common::combat::{CombatConfig, CombatRole, SpellEntry};

use crate::combat::strategy::{ClassStrategy, CombatContext};

/// Generic DPS strategy: works for any DPS class (melee or ranged).
/// Assists main assist, uses highest priority spell that mana allows.
pub struct GenericDpsStrategy {
    class_id: u8,
    role: CombatRole,
    aoe_threshold: u8,
}

impl GenericDpsStrategy {
    pub fn new(class_id: u8, config: &CombatConfig) -> Self {
        Self {
            class_id,
            role: config.role,
            aoe_threshold: config.aoe_threshold,
        }
    }
}

impl ClassStrategy for GenericDpsStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        // Pass through current target (assist target).
        ctx.target.map(|t| t.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();

        // Highest priority spell from config where mana is sufficient.
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
        self.aoe_threshold
    }

    fn role(&self) -> CombatRole {
        self.role
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::combat::strategy::CombatContext;
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
            ch_chain_slot: None,
        }
    }

    #[test]
    fn class_id_matches_construction() {
        let config = CombatConfig::default();
        let strat = GenericDpsStrategy::new(99, &config);
        assert_eq!(strat.class_id(), 99);
    }

    #[test]
    fn role_matches_config() {
        let config = CombatConfig {
            role: CombatRole::DpsRanged,
            ..CombatConfig::default()
        };
        let strat = GenericDpsStrategy::new(99, &config);
        assert_eq!(strat.role(), CombatRole::DpsRanged);
    }

    #[test]
    fn aoe_threshold_matches_config() {
        let config = CombatConfig {
            aoe_threshold: 5,
            ..CombatConfig::default()
        };
        let strat = GenericDpsStrategy::new(99, &config);
        assert_eq!(strat.aoe_threshold(), 5);
    }

    #[test]
    fn should_assist_always_true() {
        let config = CombatConfig::default();
        let strat = GenericDpsStrategy::new(99, &config);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &config);
        assert!(strat.should_assist(&ctx));
    }

    #[test]
    fn select_target_returns_target_spawn_id() {
        let config = CombatConfig::default();
        let strat = GenericDpsStrategy::new(99, &config);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 42,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &config);
        assert_eq!(strat.select_target(&ctx), Some(42));
    }

    #[test]
    fn select_target_none_without_target() {
        let config = CombatConfig::default();
        let strat = GenericDpsStrategy::new(99, &config);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &config);
        assert!(strat.select_target(&ctx).is_none());
    }

    #[test]
    fn select_spell_highest_priority_affordable() {
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "Cheap".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 1,
                    min_mana_pct: 10.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Best".into(),
                    slot: 2,
                    spell_id: 2,
                    priority: 10,
                    min_mana_pct: 20.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "TooExpensive".into(),
                    slot: 3,
                    spell_id: 3,
                    priority: 100,
                    min_mana_pct: 90.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let strat = GenericDpsStrategy::new(99, &config);
        let mut player = SpawnData::default();
        player.mana_current = 5000;
        player.mana_max = 10000; // 50% mana
        let ctx = make_ctx(&player, None, &config);
        let spell = strat.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Best");
    }

    #[test]
    fn select_spell_none_when_no_spells() {
        let config = CombatConfig::default(); // empty spells
        let strat = GenericDpsStrategy::new(99, &config);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &config);
        assert!(strat.select_spell(&ctx).is_none());
    }

    #[test]
    fn select_spell_none_when_all_too_expensive() {
        let config = CombatConfig {
            spells: vec![SpellEntry {
                name: "Expensive".into(),
                slot: 1,
                spell_id: 1,
                priority: 10,
                min_mana_pct: 90.0,
                is_aoe: false,
            }],
            ..CombatConfig::default()
        };
        let strat = GenericDpsStrategy::new(99, &config);
        let mut player = SpawnData::default();
        player.mana_current = 100;
        player.mana_max = 10000; // 1% mana
        let ctx = make_ctx(&player, None, &config);
        assert!(strat.select_spell(&ctx).is_none());
    }
}
