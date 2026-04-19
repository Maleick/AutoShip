use textquest_common::combat::{
    AbilityCandidate, AbilitySet, ActionType, CombatRole, CombatStateReq, ConditionExpr,
    SpellEntry, TargetSelector,
};

use crate::combat::{
    rotation::{self, RotationGroup},
    strategy::{self, ClassStrategy, CombatContext},
};

/// Berserker strategy: pure melee DPS centered on endurance-aware activated
/// abilities.
///
/// Rotation order follows the live-safe priorities validated for the 60-65
/// tuning window:
/// 1. Burn — battle cry, burn discs, and late cleave disc when endurance is high
/// 2. Combat — rage volley on cooldown, then Frenzy as the always-on melee core
pub struct BerserkerStrategy {
    class_id: u8,
}

impl BerserkerStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "PrimaryBurn".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Burning Rage Discipline".into(),
                        min_level: 60,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Blind Rage Discipline".into(),
                        min_level: 58,
                        spell_id: -1,
                    },
                ],
            },
            AbilitySet {
                name: "Volley".into(),
                candidates: vec![AbilityCandidate {
                    name: "Rage Volley".into(),
                    min_level: 61,
                    spell_id: -1,
                }],
            },
            AbilitySet {
                name: "BattleCry".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Ancient: Cry of Chaos".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Battle Cry of the Mastruq".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "War Cry of Dravel".into(),
                        min_level: 64,
                        spell_id: -1,
                    },
                ],
            },
            AbilitySet {
                name: "Cleave".into(),
                candidates: vec![AbilityCandidate {
                    name: "Cleaving Anger Discipline".into(),
                    min_level: 65,
                    spell_id: -1,
                }],
            },
        ]
    }

    fn build_rotations() -> Vec<RotationGroup> {
        vec![
            {
                let mut g =
                    rotation::group("Burn", TargetSelector::AutoTarget, CombatStateReq::Combat);
                g.steps_per_frame = 1;
                g.entries = vec![
                    rotation::entry_if(
                        "BattleCry",
                        ActionType::Disc("BattleCry".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::ManaAbove(70.0),
                            ConditionExpr::TargetHpAbove(60.0),
                        ]),
                    ),
                    rotation::entry_if(
                        "PrimaryBurn",
                        ActionType::Disc("PrimaryBurn".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::ManaAbove(60.0),
                            ConditionExpr::TargetHpAbove(40.0),
                        ]),
                    ),
                    rotation::entry_if(
                        "Cleave",
                        ActionType::Disc("Cleave".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::ManaAbove(45.0),
                            ConditionExpr::TargetHpAbove(40.0),
                        ]),
                    ),
                ];
                g
            },
            {
                let mut g =
                    rotation::group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
                g.steps_per_frame = 1;
                g.entries = vec![
                    rotation::entry_if(
                        "Volley",
                        ActionType::Disc("Volley".into()),
                        ConditionExpr::EnduranceAbove(25.0),
                    ),
                    rotation::entry_if(
                        "Frenzy",
                        ActionType::Ability("Frenzy".into()),
                        ConditionExpr::EnduranceAbove(15.0),
                    ),
                ];
                g
            },
        ]
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
        // Legacy fallback — only used if rotation_groups() is bypassed.
        ctx.config.spells.iter().max_by_key(|s| s.priority).cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Berserker");
    }

    fn aoe_threshold(&self) -> u8 {
        2
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

    fn uses_builtin_combat_drivers(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use textquest_common::{
        combat::{CombatConfig, KnownAbility},
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

    fn known_abilities() -> Vec<KnownAbility> {
        vec![
            KnownAbility {
                name: "Blind Rage Discipline".into(),
                spell_id: 5041,
                level: 58,
            },
            KnownAbility {
                name: "Burning Rage Discipline".into(),
                spell_id: 5034,
                level: 60,
            },
            KnownAbility {
                name: "Rage Volley".into(),
                spell_id: 6754,
                level: 61,
            },
            KnownAbility {
                name: "Battle Cry of the Mastruq".into(),
                spell_id: 5031,
                level: 65,
            },
            KnownAbility {
                name: "Ancient: Cry of Chaos".into(),
                spell_id: 5032,
                level: 65,
            },
            KnownAbility {
                name: "Cleaving Anger Discipline".into(),
                spell_id: 5043,
                level: 65,
            },
        ]
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
    fn berserker_uses_rotation_owned_combat_drivers() {
        let ber = BerserkerStrategy::new(16);
        assert!(!ber.uses_builtin_combat_drivers());
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
                    name: "Volley".into(),
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
        let spell = ber.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Frenzy");
    }

    #[test]
    fn berserker_has_rotation_groups() {
        let ber = BerserkerStrategy::new(16);
        let groups = ber.rotation_groups().unwrap();
        let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["Burn", "Combat"]);
    }

    #[test]
    fn berserker_rotation_prefers_burn_group_with_high_endurance() {
        let ber = BerserkerStrategy::new(16);
        let mut groups = ber.rotation_groups().unwrap();
        let player = SpawnData {
            mana_current: 90,
            mana_max: 100,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 42,
            hp_current: 10_000,
            hp_max: 10_000,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &[], true);

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx).unwrap();
        assert_eq!(action.entry_name, "BattleCry");
    }

    #[test]
    fn berserker_rotation_conserves_burns_when_endurance_is_low() {
        let ber = BerserkerStrategy::new(16);
        let mut groups = ber.rotation_groups().unwrap();
        let player = SpawnData {
            mana_current: 20,
            mana_max: 100,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 42,
            hp_current: 10_000,
            hp_max: 10_000,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &[], true);

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx).unwrap();
        assert_eq!(action.entry_name, "Frenzy");
    }

    #[test]
    fn berserker_rotation_stops_when_endurance_is_too_low() {
        let ber = BerserkerStrategy::new(16);
        let mut groups = ber.rotation_groups().unwrap();
        let player = SpawnData {
            mana_current: 10,
            mana_max: 100,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 42,
            hp_current: 10_000,
            hp_max: 10_000,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &[], true);

        assert!(crate::combat::rotation::execute_rotations(&mut groups, &ctx).is_none());
    }

    #[test]
    fn berserker_has_ability_sets() {
        let ber = BerserkerStrategy::new(16);
        let sets = ber.ability_sets();
        let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"PrimaryBurn"));
        assert!(names.contains(&"Volley"));
        assert!(names.contains(&"BattleCry"));
        assert!(names.contains(&"Cleave"));
    }

    #[test]
    fn berserker_ability_resolution_at_60() {
        let sets = BerserkerStrategy::build_ability_sets();
        let resolved = textquest_common::combat::resolve_abilities(&sets, &known_abilities(), 60);

        let primary_burn = resolved
            .get("PrimaryBurn")
            .expect("level 60 should resolve primary burn");
        assert_eq!(primary_burn.ability_name, "Burning Rage Discipline");
        assert_eq!(primary_burn.spell_id, 5034);
        assert!(!resolved.contains_key("Volley"));
        assert!(!resolved.contains_key("BattleCry"));
        assert!(!resolved.contains_key("Cleave"));
    }

    #[test]
    fn berserker_ability_resolution_at_61_and_62() {
        let sets = BerserkerStrategy::build_ability_sets();
        let resolved_61 =
            textquest_common::combat::resolve_abilities(&sets, &known_abilities(), 61);
        let resolved_62 =
            textquest_common::combat::resolve_abilities(&sets, &known_abilities(), 62);

        let volley_61 = resolved_61
            .get("Volley")
            .expect("level 61 should resolve volley");
        let volley_62 = resolved_62
            .get("Volley")
            .expect("level 62 should keep volley");
        assert_eq!(volley_61.ability_name, "Rage Volley");
        assert_eq!(volley_62.ability_name, "Rage Volley");
        assert!(!resolved_61.contains_key("BattleCry"));
        assert!(!resolved_62.contains_key("BattleCry"));
    }

    #[test]
    fn berserker_ability_resolution_at_65_prefers_ancient_cry() {
        let sets = BerserkerStrategy::build_ability_sets();
        let resolved = textquest_common::combat::resolve_abilities(&sets, &known_abilities(), 65);

        let battle_cry = resolved
            .get("BattleCry")
            .expect("level 65 should resolve battle cry");
        assert_eq!(battle_cry.ability_name, "Ancient: Cry of Chaos");
        assert_eq!(battle_cry.spell_id, 5032);

        let cleave = resolved
            .get("Cleave")
            .expect("level 65 should resolve cleave");
        assert_eq!(cleave.ability_name, "Cleaving Anger Discipline");
        assert_eq!(cleave.spell_id, 5043);
    }
}
