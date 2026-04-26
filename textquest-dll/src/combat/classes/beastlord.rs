use textquest_common::combat::{
    AbilityCandidate, EQExpansion, AbilitySet, CombatRole, CombatStateReq, ConditionExpr, SpellEntry,
    TargetSelector,
};

use crate::combat::{
    rotation::{self, RotationGroup},
    strategy::{self, ClassStrategy, CombatContext, PetAction},
};

/// Beastlord strategy: melee DPS with warder support, opener slow, and
/// level-gated burn/nuke tools.
///
/// The runtime strategy stays inside combat-friendly surfaces:
/// - send the warder on the current target
/// - open with the highest slow line at the start of a fight
/// - use the best available burn discipline on healthy targets
/// - spend mana on the current spell DPS line only after opener duties are covered
/// - fall back to kick when spell DPS is gated
///
/// Pet healing and pet-buff maintenance remain out of the active rotation for
/// now because `CombatContext` does not expose pet HP or pet buff state.
pub struct BeastlordStrategy {
    class_id: u8,
}

impl BeastlordStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "Slow".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Sha's Revenge".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Sha's Advantage".into(),
                        min_level: 60,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "BestialFury".into(),
                candidates: vec![AbilityCandidate {
                    name: "Bestial Fury Discipline".into(),
                    min_level: 60,
                    spell_id: -1,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Nuke".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Trushar's Frost".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Scorpion Venom".into(),
                        min_level: 61,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "SelfHeal".into(),
                candidates: vec![AbilityCandidate {
                    name: "Trushar's Mending".into(),
                    min_level: 65,
                    spell_id: -1,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "PetHeal".into(),
                candidates: vec![AbilityCandidate {
                    name: "Healing of Sorsha".into(),
                    min_level: 61,
                    spell_id: 3455,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "HpAttackBuff".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Spiritual Vigor".into(),
                        min_level: 62,
                        spell_id: 3456,
                    },
                    AbilityCandidate {
                        name: "Spiritual Strength".into(),
                        min_level: 60,
                        spell_id: 2630,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "DirectHpBuff".into(),
                candidates: vec![AbilityCandidate {
                    name: "Talisman of Kragg".into(),
                    min_level: 62,
                    spell_id: -1,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "StatBuff".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Ferocity".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Savagery".into(),
                        min_level: 60,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "PetBuff".into(),
                candidates: vec![AbilityCandidate {
                    name: "Arag's Celerity".into(),
                    min_level: 63,
                    spell_id: 3458,
                }],
                min_expansion: EQExpansion::Classic,
            },
        ]
    }

    fn build_rotations() -> Vec<RotationGroup> {
        vec![
            {
                let mut g = rotation::group(
                    "Emergency",
                    TargetSelector::SelfOnly,
                    CombatStateReq::Combat,
                );
                g.hp_threshold = Some(40.0);
                g.steps_per_frame = 1;
                g.full_rotation = true;
                g.entries = vec![rotation::entry_if(
                    "SelfHeal",
                    textquest_common::combat::ActionType::Spell("SelfHeal".into()),
                    ConditionExpr::ManaAbove(25.0),
                )];
                g
            },
            {
                let mut g =
                    rotation::group("Debuff", TargetSelector::AutoTarget, CombatStateReq::Combat);
                g.steps_per_frame = 1;
                g.entries = vec![rotation::entry_if(
                    "Slow",
                    textquest_common::combat::ActionType::Spell("Slow".into()),
                    ConditionExpr::And(vec![
                        ConditionExpr::ManaAbove(20.0),
                        // Without target-debuff visibility, treat slow as an opener.
                        ConditionExpr::TargetHpAbove(98.0),
                    ]),
                )];
                g
            },
            {
                let mut g =
                    rotation::group("Burn", TargetSelector::AutoTarget, CombatStateReq::Combat);
                g.steps_per_frame = 1;
                g.entries = vec![rotation::entry_if(
                    "BestialFury",
                    textquest_common::combat::ActionType::Disc("BestialFury".into()),
                    ConditionExpr::TargetHpAbove(60.0),
                )];
                g
            },
            {
                let mut g = rotation::group(
                    "CombatSpells",
                    TargetSelector::AutoTarget,
                    CombatStateReq::Combat,
                );
                g.steps_per_frame = 1;
                g.entries = vec![rotation::entry_if(
                    "Nuke",
                    textquest_common::combat::ActionType::Spell("Nuke".into()),
                    ConditionExpr::And(vec![
                        ConditionExpr::ManaAbove(45.0),
                        ConditionExpr::TargetHpAbove(25.0),
                    ]),
                )];
                g
            },
            {
                let mut g =
                    rotation::group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
                g.steps_per_frame = 1;
                g.entries = vec![rotation::entry(
                    "Kick",
                    textquest_common::combat::ActionType::Ability("Kick".into()),
                )];
                g
            },
        ]
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

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Beastlord");
    }

    fn aoe_threshold(&self) -> u8 {
        3
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsMelee
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
    use textquest_common::{
        combat::{
            CombatConfig, ExtendedTargetList, ExtendedTargetSlot, KnownAbility, XTargetSlotStatus,
            XTargetType,
        },
        types::SpawnData,
    };

    static DEFAULT_CONFIG: std::sync::LazyLock<CombatConfig> =
        std::sync::LazyLock::new(CombatConfig::default);

    fn make_ctx<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        enemies: &'a [SpawnData],
        in_combat: bool,
        xtargets: Option<&'a ExtendedTargetList>,
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
            extended_targets: xtargets,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        }
    }

    fn beastlord_known_spells() -> Vec<KnownAbility> {
        let mut synthetic_id = 900_000;
        BeastlordStrategy::build_ability_sets()
            .iter()
            .flat_map(|set| &set.candidates)
            .map(|candidate| {
                let spell_id = if candidate.spell_id >= 0 {
                    candidate.spell_id
                } else {
                    synthetic_id += 1;
                    synthetic_id
                };
                KnownAbility {
                    name: candidate.name.clone(),
                    spell_id,
                    level: candidate.min_level,
                }
            })
            .collect()
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
        let ctx = make_ctx(&player, None, &[], false, None);
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
        let ctx = make_ctx(&player, Some(&target), &[], true, None);
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
        let ctx = make_ctx(&player, None, &enemies, false, None);
        assert_eq!(bl.select_target(&ctx), Some(2));
    }

    #[test]
    fn select_target_out_of_combat_no_enemies() {
        let bl = BeastlordStrategy::new(15);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[], false, None);
        assert!(bl.select_target(&ctx).is_none());
    }

    #[test]
    fn select_spell_filters_by_mana() {
        let bl = BeastlordStrategy::new(15);
        let player = SpawnData {
            mana_current: 3000,
            mana_max: 10000,
            ..SpawnData::default()
        };
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "HighCost".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 10,
                    min_mana_pct: 60.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Affordable".into(),
                    slot: 2,
                    spell_id: 2,
                    priority: 5,
                    min_mana_pct: 20.0,
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
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        };
        let spell = bl.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Affordable");
    }

    #[test]
    fn beastlord_has_rotation_groups() {
        let bl = BeastlordStrategy::new(15);
        let groups = bl.rotation_groups().expect("rotation groups should exist");
        let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Emergency", "Debuff", "Burn", "CombatSpells", "Combat"]
        );
    }

    #[test]
    fn beastlord_has_ability_sets() {
        let bl = BeastlordStrategy::new(15);
        let sets = bl.ability_sets();
        let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Slow"));
        assert!(names.contains(&"BestialFury"));
        assert!(names.contains(&"Nuke"));
        assert!(names.contains(&"SelfHeal"));
        assert!(names.contains(&"PetHeal"));
        assert!(names.contains(&"HpAttackBuff"));
        assert!(names.contains(&"DirectHpBuff"));
        assert!(names.contains(&"StatBuff"));
        assert!(names.contains(&"PetBuff"));
    }

    #[test]
    fn beastlord_level_60_resolves_opening_tools() {
        let sets = BeastlordStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &beastlord_known_spells(), 60);
        assert_eq!(
            resolved.get("Slow").unwrap().ability_name,
            "Sha's Advantage"
        );
        assert_eq!(
            resolved.get("BestialFury").unwrap().ability_name,
            "Bestial Fury Discipline"
        );
        assert_eq!(
            resolved.get("HpAttackBuff").unwrap().ability_name,
            "Spiritual Strength"
        );
        assert_eq!(resolved.get("StatBuff").unwrap().ability_name, "Savagery");
        assert!(!resolved.contains_key("PetHeal"));
    }

    #[test]
    fn beastlord_level_61_resolves_pet_heal() {
        let sets = BeastlordStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &beastlord_known_spells(), 61);
        assert_eq!(
            resolved.get("PetHeal").unwrap().ability_name,
            "Healing of Sorsha"
        );
        assert_eq!(
            resolved.get("Slow").unwrap().ability_name,
            "Sha's Advantage"
        );
        assert_eq!(resolved.get("Nuke").unwrap().ability_name, "Scorpion Venom");
    }

    #[test]
    fn beastlord_level_62_resolves_group_buff_upgrades() {
        let sets = BeastlordStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &beastlord_known_spells(), 62);
        assert_eq!(resolved.get("Nuke").unwrap().ability_name, "Scorpion Venom");
        assert_eq!(
            resolved.get("HpAttackBuff").unwrap().ability_name,
            "Spiritual Vigor"
        );
        assert_eq!(
            resolved.get("DirectHpBuff").unwrap().ability_name,
            "Talisman of Kragg"
        );
    }

    #[test]
    fn beastlord_level_65_resolves_final_lines() {
        let sets = BeastlordStrategy::build_ability_sets();
        let resolved =
            textquest_common::combat::resolve_abilities(&sets, &beastlord_known_spells(), 65);
        assert_eq!(resolved.get("Slow").unwrap().ability_name, "Sha's Revenge");
        assert_eq!(
            resolved.get("Nuke").unwrap().ability_name,
            "Trushar's Frost"
        );
        assert_eq!(
            resolved.get("SelfHeal").unwrap().ability_name,
            "Trushar's Mending"
        );
        assert_eq!(resolved.get("StatBuff").unwrap().ability_name, "Ferocity");
    }

    #[test]
    fn beastlord_rotation_opens_with_slow() {
        let mut groups = BeastlordStrategy::build_rotations();
        let player = SpawnData {
            spawn_id: 1,
            mana_current: 8000,
            mana_max: 10000,
            hp_current: 9000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 77,
            hp_current: 10000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &[], true, None);
        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx)
            .expect("rotation should pick an action");
        assert_eq!(action.entry_name, "Slow");
    }

    #[test]
    fn beastlord_emergency_heal_preempts_other_actions() {
        let mut groups = BeastlordStrategy::build_rotations();
        let player = SpawnData {
            spawn_id: 1,
            mana_current: 8000,
            mana_max: 10000,
            hp_current: 2500,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 77,
            hp_current: 4000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &[], true, None);
        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx)
            .expect("rotation should pick an action");
        assert_eq!(action.entry_name, "SelfHeal");
        assert_eq!(action.target_id, player.spawn_id);
    }

    #[test]
    fn beastlord_low_mana_falls_back_to_kick() {
        let mut groups = BeastlordStrategy::build_rotations();
        let player = SpawnData {
            spawn_id: 1,
            mana_current: 1500,
            mana_max: 10000,
            hp_current: 9000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 77,
            hp_current: 4000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &[], true, None);
        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx)
            .expect("rotation should pick an action");
        assert_eq!(action.entry_name, "Kick");
    }

    #[test]
    fn beastlord_pet_action_attacks_when_warder_is_idle() {
        let bl = BeastlordStrategy::new(15);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 99,
            ..SpawnData::default()
        };
        let xtargets = ExtendedTargetList {
            slots: vec![ExtendedTargetSlot {
                slot_type: XTargetType::MyPet,
                status: XTargetSlotStatus::CurrentZone,
                spawn_id: 77,
                name: "Warder".into(),
                aggro_pct: 0,
            }],
            auto_add_haters: false,
        };
        let ctx = make_ctx(&player, Some(&target), &[], true, Some(&xtargets));
        assert_eq!(bl.pet_action(&ctx), Some(PetAction::Attack));
    }

    #[test]
    fn beastlord_pet_action_skips_when_warder_already_has_target() {
        let bl = BeastlordStrategy::new(15);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 99,
            ..SpawnData::default()
        };
        let xtargets = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::MyPet,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 77,
                    name: "Warder".into(),
                    aggro_pct: 0,
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::MyPetTarget,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 99,
                    name: "a rigid skeleton".into(),
                    aggro_pct: 0,
                },
            ],
            auto_add_haters: false,
        };
        let ctx = make_ctx(&player, Some(&target), &[], true, Some(&xtargets));
        assert_eq!(bl.pet_action(&ctx), None);
    }
}
