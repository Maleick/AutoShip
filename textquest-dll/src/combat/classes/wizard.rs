use textquest_common::combat::{AbilityCandidate, AbilitySet, CombatRole, SpellEntry};

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

    /// Build wizard ability sets — nuke lines tiered by level.
    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "FireNuke".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Fire of Tallon".into(),
                        min_level: 65,
                        spell_id: 5006,
                    },
                    AbilityCandidate {
                        name: "Sunstrike".into(),
                        min_level: 49,
                        spell_id: 1398,
                    },
                    AbilityCandidate {
                        name: "Conflagration".into(),
                        min_level: 20,
                        spell_id: 1397,
                    },
                    AbilityCandidate {
                        name: "Fireball".into(),
                        min_level: 4,
                        spell_id: 68,
                    },
                ],
            },
            AbilitySet {
                name: "IceNuke".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Draught of E`ci".into(),
                        min_level: 65,
                        spell_id: 5007,
                    },
                    AbilityCandidate {
                        name: "Ice Comet".into(),
                        min_level: 60,
                        spell_id: 1500,
                    },
                    AbilityCandidate {
                        name: "Frost".into(),
                        min_level: 52,
                        spell_id: 1200,
                    },
                    AbilityCandidate {
                        name: "Chill Sight".into(),
                        min_level: 44,
                        spell_id: 900,
                    },
                    AbilityCandidate {
                        name: "Frost Bolt".into(),
                        min_level: 1,
                        spell_id: 66,
                    },
                ],
            },
            AbilitySet {
                name: "MagicNuke".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Lure of Thunder".into(),
                        min_level: 60,
                        spell_id: 3347,
                    },
                    AbilityCandidate {
                        name: "Thunder Strike".into(),
                        min_level: 52,
                        spell_id: 1201,
                    },
                    AbilityCandidate {
                        name: "Shock of Lightning".into(),
                        min_level: 1,
                        spell_id: 69,
                    },
                ],
            },
            AbilitySet {
                name: "AoENuke".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Jyll's Wave of Heat".into(),
                        min_level: 60,
                        spell_id: 3580,
                    },
                    AbilityCandidate {
                        name: "Pillar of Frost".into(),
                        min_level: 51,
                        spell_id: 1399,
                    },
                    AbilityCandidate {
                        name: "Ice Rain".into(),
                        min_level: 24,
                        spell_id: 1396,
                    },
                ],
            },
            AbilitySet {
                name: "Root".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Paralyzing Earth".into(),
                        min_level: 55,
                        spell_id: 2164,
                    },
                    AbilityCandidate {
                        name: "Root".into(),
                        min_level: 8,
                        spell_id: 230,
                    },
                ],
            },
            AbilitySet {
                name: "Evac".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Succor".into(),
                        min_level: 52,
                        spell_id: 2160,
                    },
                    AbilityCandidate {
                        name: "Evacuate".into(),
                        min_level: 24,
                        spell_id: 2161,
                    },
                ],
            },
        ]
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

    fn ability_sets(&self) -> Vec<AbilitySet> {
        Self::build_ability_sets()
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

    // ── AbilitySet tests ────────────────────────────────────────

    fn wizard_known_spells() -> Vec<textquest_common::combat::KnownAbility> {
        WizardStrategy::build_ability_sets()
            .iter()
            .flat_map(|s| &s.candidates)
            .map(|c| textquest_common::combat::KnownAbility {
                name: c.name.clone(),
                spell_id: c.spell_id,
                level: c.min_level,
            })
            .collect()
    }

    #[test]
    fn wizard_has_ability_sets() {
        let wiz = WizardStrategy::new(12);
        let sets = wiz.ability_sets();
        assert!(!sets.is_empty());
        let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"FireNuke"));
        assert!(names.contains(&"IceNuke"));
        assert!(names.contains(&"MagicNuke"));
        assert!(names.contains(&"AoENuke"));
        assert!(names.contains(&"Root"));
        assert!(names.contains(&"Evac"));
    }

    #[test]
    fn wizard_fire_nuke_resolution_at_65() {
        let sets = WizardStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &wizard_known_spells(), 65);
        let fire = resolved.get("FireNuke").expect("should resolve FireNuke");
        assert_eq!(fire.ability_name, "Fire of Tallon");
    }

    #[test]
    fn wizard_fire_nuke_resolution_at_30() {
        let sets = WizardStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &wizard_known_spells(), 30);
        let fire = resolved.get("FireNuke").expect("should resolve FireNuke");
        assert_eq!(fire.ability_name, "Conflagration");
    }

    #[test]
    fn wizard_ice_nuke_resolution_at_55() {
        let sets = WizardStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &wizard_known_spells(), 55);
        let ice = resolved.get("IceNuke").expect("should resolve IceNuke");
        assert_eq!(ice.ability_name, "Frost");
    }

    #[test]
    fn wizard_evac_not_available_at_low_level() {
        let sets = WizardStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &wizard_known_spells(), 20);
        assert!(!resolved.contains_key("Evac"));
    }

    #[test]
    fn wizard_all_lines_resolve_at_65() {
        let sets = WizardStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &wizard_known_spells(), 65);
        assert_eq!(
            resolved.len(),
            6,
            "All 6 wizard ability lines should resolve at 65"
        );
    }

    #[test]
    fn wizard_level_1_has_basics() {
        let sets = WizardStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &wizard_known_spells(), 1);
        assert!(resolved.contains_key("IceNuke"), "Frost Bolt at level 1");
        assert!(resolved.contains_key("MagicNuke"), "Shock at level 1");
        assert!(!resolved.contains_key("AoENuke"), "No AoE at level 1");
    }
}
