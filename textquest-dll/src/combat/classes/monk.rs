use textquest_common::combat::{
    AbilityCandidate, AbilitySet, ActionType, CombatRole, CombatStateReq, ConditionExpr,
    EQExpansion, SpellEntry, TargetSelector,
};

use crate::combat::{
    rotation::{self, RotationGroup},
    strategy::{self, ClassStrategy, CombatContext},
};

const MONK_TIMER_2_IDS: &[i32] = &[4502, 4509, 4510, 4690];
const MONK_TIMER_4_IDS: &[i32] = &[4507, 4508, 4511];
const MONK_TIMER_5_IDS: &[i32] = &[4692];
const MONK_TIMER_11_IDS: &[i32] = &[4691];

const TICKS_PER_SECOND: u32 = 20;
const MONK_CD_3M54S: u32 = 234 * TICKS_PER_SECOND;
const MONK_CD_10M: u32 = 600 * TICKS_PER_SECOND;
const MONK_CD_22M: u32 = 1320 * TICKS_PER_SECOND;
const MONK_CD_30M: u32 = 1800 * TICKS_PER_SECOND;
const MONK_CD_40M: u32 = 2400 * TICKS_PER_SECOND;

/// Monk strategy: melee DPS + puller, flying kick/round kick priority, feign
/// death escape. EQ class ID: 7
pub struct MonkStrategy {
    class_id: u8,
}

impl MonkStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "KickFocus".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Ashenhand Discipline".into(),
                        min_level: 60,
                        spell_id: 4508,
                    },
                    AbilityCandidate {
                        name: "Thunderkick Discipline".into(),
                        min_level: 52,
                        spell_id: 4511,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "PrecisionStrikes".into(),
                candidates: vec![AbilityCandidate {
                    name: "Silentfist Discipline".into(),
                    min_level: 59,
                    spell_id: 4507,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "DamageBoost".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Speed Focus Discipline".into(),
                        min_level: 63,
                        spell_id: 4691,
                    },
                    AbilityCandidate {
                        name: "Hundred Fists Discipline".into(),
                        min_level: 57,
                        spell_id: 4513,
                    },
                    AbilityCandidate {
                        name: "Innerflame Discipline".into(),
                        min_level: 56,
                        spell_id: 4512,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "DefenseDisc".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Earthwalk Discipline".into(),
                        min_level: 65,
                        spell_id: 4690,
                    },
                    AbilityCandidate {
                        name: "Voiddance Discipline".into(),
                        min_level: 54,
                        spell_id: 4502,
                    },
                    AbilityCandidate {
                        name: "Stonestance Discipline".into(),
                        min_level: 51,
                        spell_id: 4510,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "MobilityDisc".into(),
                candidates: vec![AbilityCandidate {
                    name: "Planeswalk Discipline".into(),
                    min_level: 61,
                    spell_id: 4692,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "AoeRiposte".into(),
                candidates: vec![AbilityCandidate {
                    name: "Whirlwind Discipline".into(),
                    min_level: 53,
                    spell_id: 4509,
                }],
                min_expansion: EQExpansion::Classic,
            },
        ]
    }

    fn build_rotations() -> Vec<RotationGroup> {
        vec![
            {
                // Flying Kick Priority: when positioned behind or to the side
                // of the target, Flying Kick deals bonus damage. This rotation
                // group pre-empts the standard kick in the Combat group so the
                // engine issues Flying Kick first when the positional is set.
                let mut g = rotation::group(
                    "FlyingKickPriority",
                    TargetSelector::AutoTarget,
                    CombatStateReq::Combat,
                );
                g.steps_per_frame = 1;
                g.entries = vec![rotation::entry_if(
                    "Flying Kick",
                    ActionType::Ability("Flying Kick".into()),
                    ConditionExpr::And(vec![
                        ConditionExpr::BehindTarget,
                        ConditionExpr::EnduranceAbove(15.0),
                    ]),
                )];
                g
            },
            {
                let mut g = rotation::group(
                    "Emergency",
                    TargetSelector::AutoTarget,
                    CombatStateReq::Combat,
                );
                g.steps_per_frame = 1;
                g.full_rotation = true;
                g.entries = vec![
                    rotation::entry_if(
                        "Feign Death",
                        ActionType::Ability("Feign Death".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::HpBelow(12.0),
                            ConditionExpr::AggroOnMe,
                        ]),
                    ),
                    rotation::entry_if(
                        "DefenseDisc",
                        ActionType::Disc("DefenseDisc".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::HpBelow(35.0),
                            ConditionExpr::EnduranceAbove(25.0),
                        ]),
                    ),
                    rotation::entry_if(
                        "Mend",
                        ActionType::Ability("Mend".into()),
                        ConditionExpr::HpBelow(50.0),
                    ),
                ];
                g
            },
            {
                let mut g =
                    rotation::group("Burn", TargetSelector::AutoTarget, CombatStateReq::Combat);
                g.steps_per_frame = 1;
                g.entries = vec![
                    rotation::entry_if(
                        "AoeRiposte",
                        ActionType::Disc("AoeRiposte".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::EnemyCountAbove(3),
                            ConditionExpr::EnduranceAbove(60.0),
                        ]),
                    ),
                    rotation::entry_if(
                        "DamageBoost",
                        ActionType::Disc("DamageBoost".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::EnduranceAbove(50.0),
                            ConditionExpr::TargetHpAbove(40.0),
                        ]),
                    ),
                    rotation::entry_if(
                        "KickFocus",
                        ActionType::Disc("KickFocus".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::EnduranceAbove(35.0),
                            ConditionExpr::TargetHpAbove(20.0),
                        ]),
                    ),
                    rotation::entry_if(
                        "PrecisionStrikes",
                        ActionType::Disc("PrecisionStrikes".into()),
                        ConditionExpr::EnduranceAbove(30.0),
                    ),
                ];
                g
            },
            {
                let mut g =
                    rotation::group("Utility", TargetSelector::SelfOnly, CombatStateReq::Combat);
                g.steps_per_frame = 1;
                g.entries = vec![rotation::entry_if(
                    "MobilityDisc",
                    ActionType::Disc("MobilityDisc".into()),
                    ConditionExpr::And(vec![
                        ConditionExpr::EnduranceAbove(70.0),
                        ConditionExpr::TargetHpAbove(90.0),
                    ]),
                )];
                g
            },
        ]
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

    fn rotation_groups(&self) -> Option<Vec<RotationGroup>> {
        Some(Self::build_rotations())
    }

    fn ability_sets(&self) -> Vec<AbilitySet> {
        Self::build_ability_sets()
    }

    fn activated_ability_cooldown_ticks(&self, spell_id: i32) -> Option<u32> {
        match spell_id {
            4502 | 4509 => Some(MONK_CD_40M),
            4510 | 4690 => Some(MONK_CD_3M54S),
            4507 | 4511 => Some(MONK_CD_10M),
            4508 | 4692 => Some(MONK_CD_30M),
            4512 | 4513 | 4691 => Some(MONK_CD_22M),
            _ => None,
        }
    }

    fn shared_activated_ability_ids(&self, spell_id: i32) -> &'static [i32] {
        if MONK_TIMER_2_IDS.contains(&spell_id) {
            MONK_TIMER_2_IDS
        } else if MONK_TIMER_4_IDS.contains(&spell_id) {
            MONK_TIMER_4_IDS
        } else if MONK_TIMER_5_IDS.contains(&spell_id) {
            MONK_TIMER_5_IDS
        } else if MONK_TIMER_11_IDS.contains(&spell_id) {
            MONK_TIMER_11_IDS
        } else {
            &[]
        }
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::combat::rotation;
    use textquest_common::{
        combat::{CombatConfig, KnownAbility, PositionalContext, SpellEntry, resolve_abilities},
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
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        }
    }

    fn make_ctx_positional<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        config: &'a CombatConfig,
        positional: &'a PositionalContext,
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: &[],
            group_members: &[],
            config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: Some(positional),
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
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
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
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

    #[test]
    fn monk_has_rotation_groups() {
        let monk = MonkStrategy::new(7);
        let groups = monk.rotation_groups().expect("monk rotation groups");
        let names: Vec<&str> = groups.iter().map(|group| group.name.as_str()).collect();
        assert!(names.contains(&"Emergency"));
        assert!(names.contains(&"Utility"));
        assert!(names.contains(&"Burn"));
    }

    #[test]
    fn monk_rotation_prefers_emergency_before_burn() {
        let monk = MonkStrategy::new(7);
        let mut groups = monk.rotation_groups().expect("monk rotation groups");
        let config = CombatConfig::default();
        let mut player = SpawnData::default();
        player.hp_current = 4500;
        player.hp_max = 10000;
        player.endurance_current = 800;
        player.endurance_max = 1000;
        let target = SpawnData {
            spawn_id: 42,
            hp_current: 9000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &config, true);

        let action = rotation::execute_rotations(&mut groups, &ctx).expect("monk action");
        assert_eq!(action.entry_name, "Mend");
    }

    #[test]
    fn monk_rotation_uses_burn_disc_when_stable() {
        let monk = MonkStrategy::new(7);
        let mut groups = monk.rotation_groups().expect("monk rotation groups");
        let config = CombatConfig::default();
        let mut player = SpawnData::default();
        player.hp_current = 10000;
        player.hp_max = 10000;
        player.endurance_current = 900;
        player.endurance_max = 1000;
        let target = SpawnData {
            spawn_id: 42,
            hp_current: 9000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &config, true);

        let action = rotation::execute_rotations(&mut groups, &ctx).expect("monk action");
        assert_eq!(action.entry_name, "DamageBoost");
    }

    #[test]
    fn monk_rotation_keeps_burn_openers_ahead_of_utility_discs() {
        let monk = MonkStrategy::new(7);
        let mut groups = monk.rotation_groups().expect("monk rotation groups");
        let config = CombatConfig::default();
        let mut player = SpawnData::default();
        player.hp_current = 10000;
        player.hp_max = 10000;
        player.endurance_current = 900;
        player.endurance_max = 1000;
        let target = SpawnData {
            spawn_id: 42,
            hp_current: 9500,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &config, true);

        let action = rotation::execute_rotations(&mut groups, &ctx).expect("monk action");
        assert_eq!(action.entry_name, "DamageBoost");
    }

    #[test]
    fn monk_rotation_skips_burn_when_endurance_is_low() {
        let monk = MonkStrategy::new(7);
        let mut groups = monk.rotation_groups().expect("monk rotation groups");
        let config = CombatConfig::default();
        let mut player = SpawnData::default();
        player.hp_current = 10000;
        player.hp_max = 10000;
        player.endurance_current = 200;
        player.endurance_max = 1000;
        let target = SpawnData {
            spawn_id: 42,
            hp_current: 9000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &config, true);

        assert!(
            rotation::execute_rotations(&mut groups, &ctx).is_none(),
            "monk should conserve endurance for higher-value abilities"
        );
    }

    #[test]
    fn monk_rotation_prefers_defense_disc_before_mend_when_hp_is_critical() {
        let monk = MonkStrategy::new(7);
        let mut groups = monk.rotation_groups().expect("monk rotation groups");
        let config = CombatConfig::default();
        let mut player = SpawnData::default();
        player.hp_current = 3000;
        player.hp_max = 10000;
        player.endurance_current = 800;
        player.endurance_max = 1000;
        let target = SpawnData {
            spawn_id: 42,
            hp_current: 9000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &config, true);

        let action = rotation::execute_rotations(&mut groups, &ctx).expect("monk action");
        assert_eq!(action.entry_name, "DefenseDisc");
    }

    #[test]
    fn monk_rotation_prefers_feign_death_when_dying_with_aggro() {
        let monk = MonkStrategy::new(7);
        let mut groups = monk.rotation_groups().expect("monk rotation groups");
        let config = CombatConfig::default();
        let mut player = SpawnData::default();
        player.hp_current = 1000;
        player.hp_max = 10000;
        player.endurance_current = 800;
        player.endurance_max = 1000;
        let target = SpawnData {
            spawn_id: 42,
            spawn_type: 1,
            hp_current: 9000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &config, true);

        let action = rotation::execute_rotations(&mut groups, &ctx).expect("monk action");
        assert_eq!(action.entry_name, "Feign Death");
    }

    fn all_known_from_sets() -> Vec<KnownAbility> {
        MonkStrategy::new(7)
            .ability_sets()
            .into_iter()
            .flat_map(|set| set.candidates)
            .enumerate()
            .map(|(index, candidate)| KnownAbility {
                name: candidate.name,
                spell_id: if candidate.spell_id >= 0 {
                    candidate.spell_id
                } else {
                    9000 + index as i32
                },
                level: candidate.min_level,
            })
            .collect()
    }

    #[test]
    fn monk_ability_resolution_at_60() {
        let sets = MonkStrategy::new(7).ability_sets();
        let resolved = resolve_abilities(&sets, &all_known_from_sets(), 60);
        assert_eq!(
            resolved.get("KickFocus").expect("kick focus").ability_name,
            "Ashenhand Discipline"
        );
        assert_eq!(
            resolved
                .get("DamageBoost")
                .expect("damage boost")
                .ability_name,
            "Hundred Fists Discipline"
        );
        assert_eq!(
            resolved
                .get("DefenseDisc")
                .expect("defense disc")
                .ability_name,
            "Voiddance Discipline"
        );
        assert!(
            !resolved.contains_key("MobilityDisc"),
            "Planeswalk should not resolve before level 61"
        );
    }

    #[test]
    fn monk_ability_resolution_at_61() {
        let sets = MonkStrategy::new(7).ability_sets();
        let resolved = resolve_abilities(&sets, &all_known_from_sets(), 61);
        assert_eq!(
            resolved
                .get("MobilityDisc")
                .expect("mobility disc")
                .ability_name,
            "Planeswalk Discipline"
        );
    }

    #[test]
    fn monk_ability_resolution_at_62_keeps_same_lines() {
        let sets = MonkStrategy::new(7).ability_sets();
        let resolved = resolve_abilities(&sets, &all_known_from_sets(), 62);
        assert_eq!(
            resolved.get("KickFocus").expect("kick focus").ability_name,
            "Ashenhand Discipline"
        );
        assert_eq!(
            resolved
                .get("DamageBoost")
                .expect("damage boost")
                .ability_name,
            "Hundred Fists Discipline"
        );
        assert_eq!(
            resolved
                .get("MobilityDisc")
                .expect("mobility disc")
                .ability_name,
            "Planeswalk Discipline"
        );
    }

    #[test]
    fn monk_ability_resolution_at_65() {
        let sets = MonkStrategy::new(7).ability_sets();
        let resolved = resolve_abilities(&sets, &all_known_from_sets(), 65);
        assert_eq!(
            resolved
                .get("DamageBoost")
                .expect("damage boost")
                .ability_name,
            "Speed Focus Discipline"
        );
        assert_eq!(
            resolved
                .get("DefenseDisc")
                .expect("defense disc")
                .ability_name,
            "Earthwalk Discipline"
        );
    }

    // -----------------------------------------------------------------------
    // Flying Kick positional priority tests
    // -----------------------------------------------------------------------

    #[test]
    fn flying_kick_fires_when_behind_target_with_endurance() {
        let mut player = SpawnData::default();
        player.spawn_id = 1;
        player.endurance_current = 800;
        player.endurance_max = 1000;
        let target = SpawnData {
            spawn_id: 42,
            ..SpawnData::default()
        };
        let config = test_config();
        let pos = PositionalContext {
            is_behind_target: true,
            ..PositionalContext::default()
        };
        let ctx = make_ctx_positional(&player, Some(&target), &config, &pos);
        let mut groups = MonkStrategy::new(7).rotation_groups().unwrap();
        // Isolate to FlyingKickPriority group
        groups.retain(|g| g.name == "FlyingKickPriority");

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx).unwrap();
        assert_eq!(action.entry_name, "Flying Kick");
    }

    #[test]
    fn flying_kick_blocked_when_not_behind_target() {
        let mut player = SpawnData::default();
        player.spawn_id = 1;
        player.endurance_current = 800;
        player.endurance_max = 1000;
        let target = SpawnData {
            spawn_id: 42,
            ..SpawnData::default()
        };
        let config = test_config();
        let pos = PositionalContext {
            is_behind_target: false,
            ..PositionalContext::default()
        };
        let ctx = make_ctx_positional(&player, Some(&target), &config, &pos);
        let mut groups = MonkStrategy::new(7).rotation_groups().unwrap();
        groups.retain(|g| g.name == "FlyingKickPriority");

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx);
        assert!(
            action.is_none(),
            "Flying Kick priority should not fire from front"
        );
    }

    #[test]
    fn flying_kick_preempts_other_groups_when_behind() {
        let mut player = SpawnData::default();
        player.spawn_id = 1;
        player.endurance_current = 800;
        player.endurance_max = 1000;
        let target = SpawnData {
            spawn_id: 42,
            hp_current: 5000,
            hp_max: 10_000,
            ..SpawnData::default()
        };
        let config = test_config();
        let pos = PositionalContext {
            is_behind_target: true,
            ..PositionalContext::default()
        };
        let ctx = make_ctx_positional(&player, Some(&target), &config, &pos);
        let mut groups = MonkStrategy::new(7).rotation_groups().unwrap();

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx).unwrap();
        assert_eq!(
            action.entry_name, "Flying Kick",
            "FlyingKickPriority group should preempt Burn group"
        );
    }
}
