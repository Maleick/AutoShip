use textquest_common::{
    combat::{
        AbilityCandidate, AbilitySet, EQExpansion, ActionType, CombatRole, CombatStateReq, ConditionExpr,
        SpellEntry, TargetSelector,
    },
    nav::Waypoint,
    types::SpawnData,
};

use crate::combat::{
    rotation::{self, RotationGroup},
    strategy::{self, ClassStrategy, CombatContext},
};

const RANGER_MELEE_RANGE: f32 = 30.0;
const RANGER_DISC_LOCKOUT_TICKS: u32 = 86_400;

/// Ranger strategy: ranged/melee hybrid DPS with tracking and bow pulling.
///
/// Rangers operate in two stances:
/// - **Ranged**: Use bow attacks and `DoT` spells from distance (default when
///   pulling)
/// - **Melee**: Switch to melee when target is close, use kicks and
///   backstab-style abilities
///
/// Rangers also provide: tracking (find mobs), snare (Snare/Ensnare),
/// and at higher levels, Headshot AA for trivial kills.
pub struct RangerStrategy {
    class_id: u8,
    /// Distance threshold to switch from ranged to melee (in EQ units).
    melee_range: f32,
}

impl RangerStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            melee_range: RANGER_MELEE_RANGE,
        }
    }

    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "SelfBuff".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Natureskin".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Strength of Tunare".into(),
                        min_level: 62,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Call of the Predator".into(),
                        min_level: 60,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Strength of Nature".into(),
                        min_level: 51,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "ProcBuff".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Call of the Rathe".into(),
                        min_level: 62,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Call of Fire".into(),
                        min_level: 55,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Call of Sky".into(),
                        min_level: 39,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "PrimaryNuke".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Sylvan Burn".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Calefaction".into(),
                        min_level: 59,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Firestrike".into(),
                        min_level: 52,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Call of Flame".into(),
                        min_level: 49,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "ColdNuke".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Circle of Winter".into(),
                        min_level: 61,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Chill of the Wildtide".into(),
                        min_level: 56,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Dot".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Drifting Death".into(),
                        min_level: 62,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Drones of Doom".into(),
                        min_level: 54,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Immolate".into(),
                        min_level: 49,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Stinging Swarm".into(),
                        min_level: 18,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Debuff".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Nature's Rebuke".into(),
                        min_level: 64,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Ensnare".into(),
                        min_level: 51,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "EmergencyHeal".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Chloroblast".into(),
                        min_level: 62,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Greater Healing".into(),
                        min_level: 57,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Healing".into(),
                        min_level: 39,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "BurnDisc".into(),
                candidates: vec![AbilityCandidate {
                    name: "Trueshot Discipline".into(),
                    min_level: 55,
                    spell_id: -1,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "DefenseDisc".into(),
                candidates: vec![AbilityCandidate {
                    name: "Weapon Shield Discipline".into(),
                    min_level: 60,
                    spell_id: -1,
                }],
                min_expansion: EQExpansion::Classic,
            },
        ]
    }

    fn build_rotations() -> Vec<RotationGroup> {
        vec![
            {
                let mut group = rotation::group(
                    "Downtime",
                    TargetSelector::SelfOnly,
                    CombatStateReq::Downtime,
                );
                group.steps_per_frame = 2;
                group.entries = vec![
                    rotation::RotationEntry {
                        cooldown_ticks: Some(7_200),
                        ..rotation::entry_if(
                            "SelfBuff",
                            ActionType::Spell("SelfBuff".into()),
                            ConditionExpr::ManaAbove(35.0),
                        )
                    },
                    rotation::RotationEntry {
                        cooldown_ticks: Some(7_200),
                        ..rotation::entry_if(
                            "ProcBuff",
                            ActionType::Spell("ProcBuff".into()),
                            ConditionExpr::ManaAbove(40.0),
                        )
                    },
                ];
                group
            },
            {
                let mut group = rotation::group(
                    "Emergency",
                    TargetSelector::SelfOnly,
                    CombatStateReq::Combat,
                );
                group.hp_threshold = Some(35.0);
                group.full_rotation = true;
                group.entries = vec![
                    rotation::entry_if(
                        "EmergencyHeal",
                        ActionType::Spell("EmergencyHeal".into()),
                        ConditionExpr::ManaAbove(30.0),
                    ),
                    rotation::RotationEntry {
                        cooldown_ticks: Some(RANGER_DISC_LOCKOUT_TICKS),
                        cooldown_key: Some("ranger-discipline-lockout".into()),
                        ..rotation::entry_if(
                            "DefenseDisc",
                            ActionType::Disc("DefenseDisc".into()),
                            ConditionExpr::EnduranceAbove(15.0),
                        )
                    },
                ];
                group
            },
            {
                // Headshot: ranged AA that instant-kills trivial mobs on a
                // ranged auto-attack hit. Requires bow in ranged slot + target
                // must be low-level (below 20). This rotation entry queues the
                // AA activation; the actual kill proc fires on the next ranged
                // auto-attack.
                let mut group = rotation::group(
                    "Headshot",
                    TargetSelector::AutoTarget,
                    CombatStateReq::Combat,
                );
                group.steps_per_frame = 1;
                group.entries = vec![rotation::entry_if(
                    "Headshot",
                    ActionType::AA("Headshot".into()),
                    ConditionExpr::And(vec![
                        ConditionExpr::RangedWeaponEquipped,
                        ConditionExpr::TargetLevelBelow(20),
                    ]),
                )];
                group
            },
            {
                let mut group =
                    rotation::group("Debuff", TargetSelector::AutoTarget, CombatStateReq::Combat);
                group.entries = vec![rotation::entry_if(
                    "Debuff",
                    ActionType::Spell("Debuff".into()),
                    ConditionExpr::And(vec![
                        ConditionExpr::TargetHpAbove(35.0),
                        ConditionExpr::ManaAbove(25.0),
                    ]),
                )];
                group
            },
            {
                let mut group =
                    rotation::group("Burn", TargetSelector::AutoTarget, CombatStateReq::Combat);
                group.full_rotation = true;
                group.entries = vec![
                    rotation::RotationEntry {
                        cooldown_ticks: Some(RANGER_DISC_LOCKOUT_TICKS),
                        cooldown_key: Some("ranger-discipline-lockout".into()),
                        ..rotation::entry_if(
                            "BurnDisc",
                            ActionType::Disc("BurnDisc".into()),
                            ConditionExpr::And(vec![
                                ConditionExpr::TargetHpAbove(70.0),
                                ConditionExpr::EnduranceAbove(20.0),
                            ]),
                        )
                    },
                    rotation::entry_if(
                        "ProcBuff",
                        ActionType::Spell("ProcBuff".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::TargetHpAbove(45.0),
                            ConditionExpr::ManaAbove(40.0),
                        ]),
                    ),
                ];
                group
            },
            {
                let mut group =
                    rotation::group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
                group.entries = vec![
                    rotation::entry_if(
                        "Dot",
                        ActionType::Spell("Dot".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::TargetHpAbove(60.0),
                            ConditionExpr::ManaAbove(55.0),
                        ]),
                    ),
                    rotation::entry_if(
                        "PrimaryNuke",
                        ActionType::Spell("PrimaryNuke".into()),
                        ConditionExpr::ManaAbove(45.0),
                    ),
                    rotation::entry_if(
                        "ColdNuke",
                        ActionType::Spell("ColdNuke".into()),
                        ConditionExpr::ManaAbove(35.0),
                    ),
                    rotation::entry_if(
                        "Kick",
                        ActionType::Ability("Kick".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::TargetDistanceBelow(RANGER_MELEE_RANGE),
                            ConditionExpr::EnduranceAbove(15.0),
                        ]),
                    ),
                ];
                group
            },
        ]
    }

    /// Calculate distance to target.
    fn distance_to(&self, player: &SpawnData, target: &SpawnData) -> f32 {
        let p = Waypoint::new(player.x, player.y, player.z);
        let t = Waypoint::new(target.x, target.y, target.z);
        p.distance_2d(&t)
    }

    /// Whether we're in melee range of the target.
    fn in_melee_range(&self, ctx: &CombatContext) -> bool {
        ctx.target
            .is_some_and(|t| self.distance_to(ctx.player, t) <= self.melee_range)
    }
}

impl ClassStrategy for RangerStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        // Assist MA when in combat, otherwise target nearest.
        if ctx.in_combat {
            ctx.target.map(|t| t.spawn_id)
        } else {
            strategy::nearest_enemy(ctx.player, ctx.nearby_enemies).map(|s| s.spawn_id)
        }
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let spells = &ctx.config.spells;
        if spells.is_empty() {
            return None;
        }

        if self.in_melee_range(ctx) {
            // In melee range: prefer melee abilities (lower spell IDs or higher priority)
            spells
                .iter()
                .filter(|s| s.priority >= 5) // High priority = melee abilities
                .max_by_key(|s| s.priority)
                .or_else(|| spells.iter().max_by_key(|s| s.priority))
                .cloned()
        } else {
            // At range: prefer ranged spells (DoTs, snare, bow)
            spells
                .iter()
                .filter(|s| s.priority < 5) // Lower priority = ranged
                .max_by_key(|s| s.priority)
                .or_else(|| spells.iter().max_by_key(|s| s.priority))
                .cloned()
        }
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true // Rangers assist the MA
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        if let Some(target) = ctx.target {
            let dist = self.distance_to(ctx.player, target);
            let stance = if dist <= self.melee_range {
                "melee"
            } else {
                "ranged"
            };
            tracing::info!(
                target_id = target.spawn_id,
                target_name = %target.name,
                distance = format!("{:.0}", dist),
                stance,
                "Ranger engaging target"
            );
        }
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
    use textquest_common::combat::{CombatConfig, PositionalContext, SpellEntry};

    fn make_ctx<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        enemies: &'a [SpawnData],
        config: &'a CombatConfig,
        in_combat: bool,
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: enemies,
            group_members: &[],
            config,
            tick: 0,
            in_combat,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
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
    fn ranger_class_id() {
        let r = RangerStrategy::new(4);
        assert_eq!(r.class_id(), 4);
    }

    #[test]
    fn ranger_role_is_dps() {
        let ranger = RangerStrategy::new(4);
        assert!(matches!(ranger.role(), CombatRole::DpsRanged));
    }

    #[test]
    fn ranger_aoe_threshold() {
        let r = RangerStrategy::new(4);
        assert_eq!(r.aoe_threshold(), 3);
    }

    #[test]
    fn ranger_assists_ma() {
        let ranger = RangerStrategy::new(4);
        let player = SpawnData::default();
        let config = textquest_common::combat::CombatConfig::default();
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
            positional: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
        };
        assert!(ranger.should_assist(&ctx));
    }

    #[test]
    fn select_target_in_combat_returns_assist_target() {
        let r = RangerStrategy::new(4);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 33,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &[], &config, true);
        assert_eq!(r.select_target(&ctx), Some(33));
    }

    #[test]
    fn select_target_out_of_combat_nearest_enemy() {
        let r = RangerStrategy::new(4);
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
                x: 15.0,
                ..SpawnData::default()
            },
        ];
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &enemies, &config, false);
        assert_eq!(r.select_target(&ctx), Some(2));
    }

    #[test]
    fn in_melee_range_true_when_close() {
        let r = RangerStrategy::new(4);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 1,
            x: 20.0, // within 30 unit melee range
            y: 0.0,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &[], &config, true);
        assert!(r.in_melee_range(&ctx));
    }

    #[test]
    fn in_melee_range_false_when_far() {
        let r = RangerStrategy::new(4);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 1,
            x: 100.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &[], &config, true);
        assert!(!r.in_melee_range(&ctx));
    }

    #[test]
    fn in_melee_range_false_no_target() {
        let r = RangerStrategy::new(4);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &[], &config, true);
        assert!(!r.in_melee_range(&ctx));
    }

    #[test]
    fn select_spell_melee_range_prefers_high_priority() {
        let r = RangerStrategy::new(4);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 1,
            x: 10.0, // in melee range
            ..SpawnData::default()
        };
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "RangedDoT".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 3,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Kick".into(),
                    slot: 2,
                    spell_id: 2,
                    priority: 7,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, Some(&target), &[], &config, true);
        let spell = r.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Kick"); // priority >= 5, melee ability
    }

    #[test]
    fn select_spell_ranged_prefers_low_priority() {
        let r = RangerStrategy::new(4);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 1,
            x: 100.0, // at range
            ..SpawnData::default()
        };
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "Snare".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 4,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Kick".into(),
                    slot: 2,
                    spell_id: 2,
                    priority: 7,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, Some(&target), &[], &config, true);
        let spell = r.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Snare"); // priority < 5, ranged ability
    }

    #[test]
    fn select_spell_empty_returns_none() {
        let r = RangerStrategy::new(4);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &[], &config, false);
        assert!(r.select_spell(&ctx).is_none());
    }

    #[test]
    fn nearest_enemy_finds_closest() {
        let player = SpawnData {
            x: 50.0,
            y: 50.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 200.0,
                y: 200.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 55.0,
                y: 55.0,
                ..SpawnData::default()
            },
        ];
        let nearest = strategy::nearest_enemy(&player, &enemies).unwrap();
        assert_eq!(nearest.spawn_id, 2);
    }

    #[test]
    fn ranger_has_rotation_groups_in_priority_order() {
        let ranger = RangerStrategy::new(4);
        let groups = ranger
            .rotation_groups()
            .expect("ranger should use rotations");
        let names: Vec<&str> = groups.iter().map(|group| group.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Downtime", "Emergency", "Debuff", "Burn", "Combat"]
        );
    }

    #[test]
    fn ranger_level_overrides_resolve_expected_lines() {
        let ranger = RangerStrategy::new(4);
        let sets = ranger.ability_sets();
        let known: Vec<textquest_common::combat::KnownAbility> = sets
            .iter()
            .flat_map(|set| set.candidates.iter())
            .enumerate()
            .map(
                |(index, candidate)| textquest_common::combat::KnownAbility {
                    name: candidate.name.clone(),
                    spell_id: 10_000 + index as i32,
                    level: candidate.min_level,
                },
            )
            .collect();

        let at_60 = textquest_common::combat::resolve_abilities(&sets, &known, 60);
        assert_eq!(
            at_60
                .get("SelfBuff")
                .map(|ability| ability.ability_name.as_str()),
            Some("Call of the Predator")
        );
        assert_eq!(
            at_60
                .get("DefenseDisc")
                .map(|ability| ability.ability_name.as_str()),
            Some("Weapon Shield Discipline")
        );

        let at_61 = textquest_common::combat::resolve_abilities(&sets, &known, 61);
        assert_eq!(
            at_61
                .get("ColdNuke")
                .map(|ability| ability.ability_name.as_str()),
            Some("Circle of Winter")
        );

        let at_62 = textquest_common::combat::resolve_abilities(&sets, &known, 62);
        assert_eq!(
            at_62
                .get("Dot")
                .map(|ability| ability.ability_name.as_str()),
            Some("Drifting Death")
        );
        assert_eq!(
            at_62
                .get("ProcBuff")
                .map(|ability| ability.ability_name.as_str()),
            Some("Call of the Rathe")
        );
        assert_eq!(
            textquest_common::combat::resolve_abilities(&sets, &known, 64)
                .get("Debuff")
                .map(|ability| ability.ability_name.as_str()),
            Some("Nature's Rebuke")
        );

        let at_65 = textquest_common::combat::resolve_abilities(&sets, &known, 65);
        assert_eq!(
            at_65
                .get("PrimaryNuke")
                .map(|ability| ability.ability_name.as_str()),
            Some("Sylvan Burn")
        );
        assert_eq!(
            at_65
                .get("SelfBuff")
                .map(|ability| ability.ability_name.as_str()),
            Some("Natureskin")
        );
    }

    #[test]
    fn ranger_low_hp_rotation_prefers_emergency_heal() {
        let ranger = RangerStrategy::new(4);
        let mut groups = ranger
            .rotation_groups()
            .expect("ranger should use rotations");
        let player = SpawnData {
            hp_current: 2_000,
            hp_max: 10_000,
            mana_current: 8_000,
            mana_max: 10_000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 77,
            hp_current: 9_000,
            hp_max: 10_000,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &[], &config, true);

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx)
            .expect("expected an emergency rotation action");
        assert_eq!(action.entry_name, "EmergencyHeal");
    }

    // -----------------------------------------------------------------------
    // Headshot gate tests
    // -----------------------------------------------------------------------

    #[test]
    fn headshot_fires_with_ranged_weapon_and_low_level_target() {
        let player = SpawnData {
            spawn_id: 1,
            hp_current: 10_000,
            hp_max: 10_000,
            mana_current: 10_000,
            mana_max: 10_000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 42,
            level: 10,
            hp_current: 1_000,
            hp_max: 1_000,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let pos = PositionalContext {
            ranged_weapon_equipped: true,
            ..PositionalContext::default()
        };
        let ctx = make_ctx_positional(&player, Some(&target), &config, &pos);
        let mut groups = RangerStrategy::build_rotations();

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx).unwrap();
        assert_eq!(action.entry_name, "Headshot");
    }

    #[test]
    fn headshot_blocked_when_target_too_high_level() {
        let player = SpawnData {
            spawn_id: 1,
            hp_current: 10_000,
            hp_max: 10_000,
            mana_current: 10_000,
            mana_max: 10_000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 42,
            level: 50,
            hp_current: 1_000,
            hp_max: 1_000,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let pos = PositionalContext {
            ranged_weapon_equipped: true,
            ..PositionalContext::default()
        };
        let ctx = make_ctx_positional(&player, Some(&target), &config, &pos);
        let mut groups = RangerStrategy::build_rotations();
        // Keep only headshot group to isolate the gate
        groups.retain(|g| g.name == "Headshot");

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx);
        assert!(
            action.is_none(),
            "Headshot must not fire against level-50 target"
        );
    }

    #[test]
    fn headshot_blocked_without_ranged_weapon() {
        let player = SpawnData {
            spawn_id: 1,
            hp_current: 10_000,
            hp_max: 10_000,
            mana_current: 10_000,
            mana_max: 10_000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 42,
            level: 5,
            hp_current: 1_000,
            hp_max: 1_000,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let pos = PositionalContext {
            ranged_weapon_equipped: false,
            ..PositionalContext::default()
        };
        let ctx = make_ctx_positional(&player, Some(&target), &config, &pos);
        let mut groups = RangerStrategy::build_rotations();
        groups.retain(|g| g.name == "Headshot");

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx);
        assert!(action.is_none(), "Headshot must not fire without bow");
    }
}
