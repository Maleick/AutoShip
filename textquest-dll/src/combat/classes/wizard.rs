use textquest_common::combat::{
    AbilityCandidate, EQExpansion, AbilitySet, ActionType, CombatRole, CombatStateReq, ConditionExpr,
    SpellEntry, TargetSelector,
};

use crate::combat::{
    rotation::{self, RotationGroup},
    strategy::{ClassStrategy, CombatContext},
};

/// Wizard strategy: pure nuke DPS. Highest priority spell available,
/// mana-aware. EQ class ID: 12
pub struct WizardStrategy {
    class_id: u8,
}

/// Reserve this much mana before continuing the standard DPS cycle.
const NUKING_MANA_FLOOR: f32 = 20.0;
/// Trigger Harvest once mana drops low enough that sustained nuking would
/// starve the primary lines.
const HARVEST_MANA_THRESHOLD: f32 = 35.0;
/// Require a healthier mana pool before spending casts on AoE.
const AOE_MANA_THRESHOLD: f32 = 45.0;
/// Emergency threshold for self-preservation tools.
const EMERGENCY_HP_THRESHOLD: f32 = 20.0;
/// Approximate spell reuse window for Harvest of Druzzil / Harvest.
const HARVEST_COOLDOWN_TICKS: u32 = 1200;
/// Conservative retry window for evac spells so low-HP checks do not spam.
const EVAC_COOLDOWN_TICKS: u32 = 600;

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
                        name: "White Fire".into(),
                        min_level: 62,
                        spell_id: -1,
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
                min_expansion: EQExpansion::Classic,
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
                        name: "Ancient: Destruction of Ice".into(),
                        min_level: 62,
                        spell_id: -1,
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
                min_expansion: EQExpansion::Classic,
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
                min_expansion: EQExpansion::Classic,
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
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Harvest".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Harvest of Druzzil".into(),
                        min_level: 61,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Harvest".into(),
                        min_level: 51,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
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
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Evac".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Greater Decession".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
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
                min_expansion: EQExpansion::Classic,
            },
        ]
    }

    fn build_rotations() -> Vec<RotationGroup> {
        vec![
            {
                let mut group = rotation::group(
                    "Emergency",
                    TargetSelector::SelfOnly,
                    CombatStateReq::Combat,
                );
                group.hp_threshold = Some(EMERGENCY_HP_THRESHOLD);
                group.full_rotation = true;
                group.entries = vec![rotation::entry_with_cooldown(
                    "Evac",
                    ActionType::Spell("Evac".into()),
                    "wizard-evac",
                    EVAC_COOLDOWN_TICKS,
                )];
                group
            },
            {
                let mut group =
                    rotation::group("Recovery", TargetSelector::SelfOnly, CombatStateReq::Combat);
                group.full_rotation = true;
                group.entries = vec![rotation::entry_if_with_cooldown(
                    "Harvest",
                    ActionType::Spell("Harvest".into()),
                    ConditionExpr::ManaBelow(HARVEST_MANA_THRESHOLD),
                    "wizard-harvest",
                    HARVEST_COOLDOWN_TICKS,
                )];
                group
            },
            {
                let mut group =
                    rotation::group("AoE", TargetSelector::AutoTarget, CombatStateReq::Combat);
                group.entries = vec![rotation::entry_if(
                    "AoENuke",
                    ActionType::Spell("AoENuke".into()),
                    ConditionExpr::And(vec![
                        ConditionExpr::EnemyCountAbove(3),
                        ConditionExpr::ManaAbove(AOE_MANA_THRESHOLD),
                    ]),
                )];
                group
            },
            {
                let mut group =
                    rotation::group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
                group.entries = vec![
                    rotation::entry_if(
                        "FireNuke",
                        ActionType::Spell("FireNuke".into()),
                        ConditionExpr::ManaAbove(NUKING_MANA_FLOOR),
                    ),
                    rotation::entry_if(
                        "IceNuke",
                        ActionType::Spell("IceNuke".into()),
                        ConditionExpr::ManaAbove(HARVEST_MANA_THRESHOLD),
                    ),
                    rotation::entry_if(
                        "MagicNuke",
                        ActionType::Spell("MagicNuke".into()),
                        ConditionExpr::ManaAbove(NUKING_MANA_FLOOR),
                    ),
                ];
                group
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

    fn rotation_groups(&self) -> Option<Vec<RotationGroup>> {
        Some(Self::build_rotations())
    }

    fn ability_sets(&self) -> Vec<AbilitySet> {
        Self::build_ability_sets()
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use textquest_common::{combat::CombatConfig, types::SpawnData};

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
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
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
        assert!(names.contains(&"Harvest"));
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
            7,
            "All 7 wizard ability lines should resolve at 65"
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

    #[test]
    fn wizard_has_rotation_groups() {
        let wiz = WizardStrategy::new(12);
        let groups = wiz
            .rotation_groups()
            .expect("wizard should define rotations");
        let names: Vec<_> = groups.iter().map(|group| group.name.as_str()).collect();
        assert_eq!(names, vec!["Emergency", "Recovery", "AoE", "Combat"]);
    }

    #[test]
    fn wizard_rotation_emergency_evacuates_before_any_other_action() {
        let wiz = WizardStrategy::new(12);
        let mut player = SpawnData {
            spawn_id: 1,
            hp_current: 10,
            hp_max: 100,
            mana_current: 8000,
            mana_max: 10000,
            ..SpawnData::default()
        };
        player.level = 65;
        let target = SpawnData {
            spawn_id: 77,
            spawn_type: 1,
            hp_current: 100,
            hp_max: 100,
            ..SpawnData::default()
        };
        let mut groups = wiz.rotation_groups().expect("wizard rotations");
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &CombatConfig::default(),
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx)
            .expect("wizard should pick an emergency action");
        assert_eq!(action.entry_name, "Evac");
        assert_eq!(action.action_type, ActionType::Spell("Evac".into()));
        assert_eq!(action.target_id, player.spawn_id);
    }

    #[test]
    fn wizard_rotation_uses_harvest_before_dps_when_mana_is_low() {
        let wiz = WizardStrategy::new(12);
        let mut player = SpawnData {
            spawn_id: 1,
            hp_current: 100,
            hp_max: 100,
            mana_current: 2500,
            mana_max: 10000,
            ..SpawnData::default()
        };
        player.level = 65;
        let target = SpawnData {
            spawn_id: 77,
            spawn_type: 1,
            hp_current: 100,
            hp_max: 100,
            ..SpawnData::default()
        };
        let mut groups = wiz.rotation_groups().expect("wizard rotations");
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &CombatConfig::default(),
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx)
            .expect("wizard should recover mana before nuking");
        assert_eq!(action.entry_name, "Harvest");
        assert_eq!(action.action_type, ActionType::Spell("Harvest".into()));
        assert_eq!(action.target_id, player.spawn_id);
    }

    #[test]
    fn wizard_rotation_prefers_aoe_when_multiple_enemies_are_present() {
        let wiz = WizardStrategy::new(12);
        let mut player = SpawnData {
            spawn_id: 1,
            hp_current: 100,
            hp_max: 100,
            mana_current: 8000,
            mana_max: 10000,
            ..SpawnData::default()
        };
        player.level = 65;
        let target = SpawnData {
            spawn_id: 77,
            spawn_type: 1,
            hp_current: 100,
            hp_max: 100,
            ..SpawnData::default()
        };
        let nearby = vec![
            target.clone(),
            SpawnData {
                spawn_id: 78,
                spawn_type: 1,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 79,
                spawn_type: 1,
                ..SpawnData::default()
            },
        ];
        let mut groups = wiz.rotation_groups().expect("wizard rotations");
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &nearby,
            group_members: &[],
            config: &CombatConfig::default(),
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx)
            .expect("wizard should AoE when three enemies are present");
        assert_eq!(action.entry_name, "AoENuke");
        assert_eq!(action.action_type, ActionType::Spell("AoENuke".into()));
        assert_eq!(action.target_id, target.spawn_id);
    }

    #[test]
    fn wizard_rotation_uses_fire_nuke_for_single_target_sustain() {
        let wiz = WizardStrategy::new(12);
        let mut player = SpawnData {
            spawn_id: 1,
            hp_current: 100,
            hp_max: 100,
            mana_current: 8000,
            mana_max: 10000,
            ..SpawnData::default()
        };
        player.level = 65;
        let target = SpawnData {
            spawn_id: 77,
            spawn_type: 1,
            hp_current: 100,
            hp_max: 100,
            ..SpawnData::default()
        };
        let mut groups = wiz.rotation_groups().expect("wizard rotations");
        let nearby_enemies = std::slice::from_ref(&target);
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies,
            group_members: &[],
            config: &CombatConfig::default(),
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx)
            .expect("wizard should cast a single-target nuke");
        assert_eq!(action.entry_name, "FireNuke");
        assert_eq!(action.action_type, ActionType::Spell("FireNuke".into()));
        assert_eq!(action.target_id, target.spawn_id);
    }

    #[test]
    fn wizard_level_60_resolves_group_dps_lines() {
        let sets = WizardStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &wizard_known_spells(), 60);
        assert_eq!(
            resolved
                .get("FireNuke")
                .expect("should resolve FireNuke at 60")
                .ability_name,
            "Sunstrike"
        );
        assert_eq!(
            resolved
                .get("IceNuke")
                .expect("should resolve IceNuke at 60")
                .ability_name,
            "Ice Comet"
        );
        assert_eq!(
            resolved
                .get("AoENuke")
                .expect("should resolve AoENuke at 60")
                .ability_name,
            "Jyll's Wave of Heat"
        );
    }

    #[test]
    fn wizard_level_61_unlocks_harvest_upgrade() {
        let sets = WizardStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &wizard_known_spells(), 61);
        assert_eq!(
            resolved
                .get("Harvest")
                .expect("should resolve Harvest at 61")
                .ability_name,
            "Harvest of Druzzil"
        );
    }

    #[test]
    fn wizard_level_62_unlocks_ancient_burst_line() {
        let sets = WizardStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &wizard_known_spells(), 62);
        assert_eq!(
            resolved
                .get("FireNuke")
                .expect("should resolve FireNuke at 62")
                .ability_name,
            "White Fire"
        );
        assert_eq!(
            resolved
                .get("IceNuke")
                .expect("should resolve IceNuke at 62")
                .ability_name,
            "Ancient: Destruction of Ice"
        );
    }
}
