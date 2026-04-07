use textquest_common::combat::{
    AbilityCandidate, AbilitySet, ActionType, CombatRole, CombatStateReq, ConditionExpr,
    SpellEntry, TargetSelector,
};

use crate::combat::rotation::{self, RotationGroup};
use crate::combat::strategy::{self, ClassStrategy, CombatContext};

/// Warrior strategy: main tank, selects nearest enemy, uses taunt/aggro abilities.
///
/// Rotation order (modeled after rgmercs warrior):
/// 1. Downtime — self buffs when out of combat
/// 2. HateTools — maintain aggro on auto-target
/// 3. Emergency — defensive discs when HP is critically low
/// 4. Defenses — proactive defensive discs
/// 5. Burn — offensive burst abilities
/// 6. Combat — DPS discs and abilities
pub struct WarriorStrategy {
    class_id: u8,
}

impl WarriorStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    /// Build warrior ability sets — maps disc/ability line names to
    /// level-tiered candidates, strongest first.
    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "Deflection".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Deflection Discipline".into(),
                        min_level: 62,
                        spell_id: 4694,
                    },
                    AbilityCandidate {
                        name: "Evasive Discipline".into(),
                        min_level: 52,
                        spell_id: 4670,
                    },
                ],
            },
            AbilitySet {
                name: "LeechCurse".into(),
                candidates: vec![AbilityCandidate {
                    name: "Leechbane Discipline".into(),
                    min_level: 63,
                    spell_id: 4695,
                }],
            },
            AbilitySet {
                name: "Carapace".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Stonewall Discipline".into(),
                        min_level: 65,
                        spell_id: 8001,
                    },
                    AbilityCandidate {
                        name: "Defensive Discipline".into(),
                        min_level: 55,
                        spell_id: 4685,
                    },
                ],
            },
            AbilitySet {
                name: "Mantle".into(),
                candidates: vec![AbilityCandidate {
                    name: "Furious Discipline".into(),
                    min_level: 56,
                    spell_id: 4674,
                }],
            },
            AbilitySet {
                name: "MeleeMit".into(),
                candidates: vec![AbilityCandidate {
                    name: "Precision Discipline".into(),
                    min_level: 57,
                    spell_id: 4676,
                }],
            },
            AbilitySet {
                name: "Blade".into(),
                candidates: vec![AbilityCandidate {
                    name: "Mighty Strike Discipline".into(),
                    min_level: 54,
                    spell_id: 4672,
                }],
            },
            AbilitySet {
                name: "CombatEndRegen".into(),
                candidates: vec![AbilityCandidate {
                    name: "Second Wind Discipline".into(),
                    min_level: 57,
                    spell_id: 4675,
                }],
            },
            AbilitySet {
                name: "EndRegen".into(),
                candidates: vec![AbilityCandidate {
                    name: "Breather".into(),
                    min_level: 1,
                    spell_id: -1,
                }],
            },
        ]
    }

    /// Build the warrior's rotation groups.
    fn build_rotations() -> Vec<RotationGroup> {
        vec![
            // 1. Downtime: self-buffs when not in combat
            {
                let mut g = rotation::group(
                    "Downtime",
                    TargetSelector::SelfOnly,
                    CombatStateReq::Downtime,
                );
                g.steps_per_frame = 1;
                g.entries = vec![
                    rotation::entry_if(
                        "EndRegen",
                        ActionType::Disc("EndRegen".into()),
                        ConditionExpr::ManaBelow(15.0), // endurance treated as mana for warriors
                    ),
                    rotation::entry("AuraBuff", ActionType::Disc("AuraBuff".into())),
                    rotation::entry("DefenseACBuff", ActionType::Disc("DefenseACBuff".into())),
                ];
                g
            },
            // 2. HateTools: maintain aggro on main target (combat only)
            {
                let mut g = rotation::group(
                    "HateTools",
                    TargetSelector::AutoTarget,
                    CombatStateReq::Combat,
                );
                g.steps_per_frame = 1;
                g.entries = vec![
                    rotation::entry("Taunt", ActionType::Ability("Taunt".into())),
                    rotation::entry("BlastOfAnger", ActionType::AA("Blast of Anger".into())),
                    rotation::entry("Attention", ActionType::Disc("Attention".into())),
                ];
                g
            },
            // 3. Emergency: defensive discs when HP critically low
            {
                let mut g = rotation::group(
                    "Emergency",
                    TargetSelector::AutoTarget,
                    CombatStateReq::Combat,
                );
                g.hp_threshold = Some(30.0);
                g.steps_per_frame = 1;
                g.full_rotation = true; // always re-check from top
                g.entries = vec![
                    rotation::entry("Deflection", ActionType::Disc("Deflection".into())),
                    rotation::entry("LeechCurse", ActionType::Disc("LeechCurse".into())),
                    rotation::entry("Carapace", ActionType::Disc("Carapace".into())),
                ];
                g
            },
            // 4. Defenses: proactive mitigation
            {
                let mut g = rotation::group(
                    "Defenses",
                    TargetSelector::AutoTarget,
                    CombatStateReq::Combat,
                );
                g.steps_per_frame = 1;
                g.entries = vec![
                    rotation::entry("Mantle", ActionType::Disc("Mantle".into())),
                    rotation::entry("MeleeMit", ActionType::Disc("MeleeMit".into())),
                ];
                g
            },
            // 5. Burn: offensive burst abilities
            {
                let mut g =
                    rotation::group("Burn", TargetSelector::AutoTarget, CombatStateReq::Combat);
                g.steps_per_frame = 2;
                g.entries = vec![
                    rotation::entry("Blade", ActionType::Disc("Blade".into())),
                    rotation::entry("Crimson", ActionType::Disc("Crimson".into())),
                ];
                g
            },
            // 6. Combat: standard DPS rotation
            {
                let mut g =
                    rotation::group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
                g.steps_per_frame = 1;
                g.entries = vec![
                    rotation::entry("Kick", ActionType::Ability("Kick".into())),
                    rotation::entry("Bash", ActionType::Ability("Bash".into())),
                    rotation::entry("CombatEndRegen", ActionType::Disc("CombatEndRegen".into())),
                ];
                g
            },
        ]
    }
}

impl ClassStrategy for WarriorStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::nearest_enemy(ctx.player, ctx.nearby_enemies).map(|s| s.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        // Legacy fallback — only used if rotation_groups() returns None.
        ctx.config.spells.iter().max_by_key(|s| s.priority).cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        false // Tank leads, doesn't assist.
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Warrior");
    }

    fn aoe_threshold(&self) -> u8 {
        2
    }

    fn role(&self) -> CombatRole {
        CombatRole::MainTank
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
    use textquest_common::combat::CombatConfig;
    use textquest_common::types::SpawnData;

    static DEFAULT_CONFIG: std::sync::LazyLock<CombatConfig> =
        std::sync::LazyLock::new(CombatConfig::default);

    fn make_ctx<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        enemies: &'a [SpawnData],
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: enemies,
            group_members: &[],
            config: &DEFAULT_CONFIG,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        }
    }

    #[test]
    fn warrior_class_id() {
        let w = WarriorStrategy::new(1);
        assert_eq!(w.class_id(), 1);
    }

    #[test]
    fn warrior_role_is_main_tank() {
        let w = WarriorStrategy::new(1);
        assert_eq!(w.role(), CombatRole::MainTank);
    }

    #[test]
    fn warrior_aoe_threshold() {
        let w = WarriorStrategy::new(1);
        assert_eq!(w.aoe_threshold(), 2);
    }

    #[test]
    fn warrior_does_not_assist() {
        let w = WarriorStrategy::new(1);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[]);
        assert!(!w.should_assist(&ctx));
    }

    #[test]
    fn select_target_nearest_enemy() {
        let w = WarriorStrategy::new(1);
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
                x: 20.0,
                ..SpawnData::default()
            },
        ];
        let ctx = make_ctx(&player, None, &enemies);
        assert_eq!(w.select_target(&ctx), Some(2));
    }

    #[test]
    fn select_target_no_enemies() {
        let w = WarriorStrategy::new(1);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[]);
        assert!(w.select_target(&ctx).is_none());
    }

    #[test]
    fn select_spell_highest_priority() {
        let w = WarriorStrategy::new(1);
        let player = SpawnData::default();
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "Taunt".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 10,
                    min_mana_pct: 0.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Bash".into(),
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
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        let spell = w.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Taunt");
    }

    #[test]
    fn select_spell_none_when_empty() {
        let w = WarriorStrategy::new(1);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[]);
        assert!(w.select_spell(&ctx).is_none());
    }

    // --- Rotation-specific tests ---

    #[test]
    fn warrior_has_rotation_groups() {
        let w = WarriorStrategy::new(1);
        let groups = w.rotation_groups();
        assert!(groups.is_some());
        let groups = groups.unwrap();
        assert!(
            groups.len() >= 6,
            "Warrior should have at least 6 rotation groups"
        );
    }

    #[test]
    fn warrior_rotation_group_names() {
        let w = WarriorStrategy::new(1);
        let groups = w.rotation_groups().unwrap();
        let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        assert!(names.contains(&"Downtime"));
        assert!(names.contains(&"HateTools"));
        assert!(names.contains(&"Emergency"));
        assert!(names.contains(&"Defenses"));
        assert!(names.contains(&"Burn"));
        assert!(names.contains(&"Combat"));
    }

    #[test]
    fn warrior_downtime_runs_out_of_combat() {
        let w = WarriorStrategy::new(1);
        let mut groups = w.rotation_groups().unwrap();
        let player = SpawnData::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &DEFAULT_CONFIG,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        // Only the Downtime group should produce actions out of combat
        let downtime = &mut groups[0];
        assert_eq!(downtime.name, "Downtime");
        let result = crate::combat::rotation::execute_group(downtime, &ctx);
        // EndRegen has ManaBelow(15.0) condition — with default 0 mana, it should pass
        assert!(
            !result.actions.is_empty(),
            "Downtime should run out of combat"
        );
    }

    #[test]
    fn warrior_emergency_only_when_low_hp() {
        let w = WarriorStrategy::new(1);
        let mut groups = w.rotation_groups().unwrap();
        let target = SpawnData {
            spawn_id: 42,
            ..SpawnData::default()
        };

        // Find Emergency group
        let emergency = groups.iter_mut().find(|g| g.name == "Emergency").unwrap();

        // At 80% HP: should not fire
        let mut player = SpawnData::default();
        player.hp_current = 8000;
        player.hp_max = 10000;
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &DEFAULT_CONFIG,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        let result = crate::combat::rotation::execute_group(emergency, &ctx);
        assert!(
            result.actions.is_empty(),
            "Emergency should not fire at 80% HP"
        );

        // At 20% HP: should fire
        player.hp_current = 2000;
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &DEFAULT_CONFIG,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        let result = crate::combat::rotation::execute_group(emergency, &ctx);
        assert!(
            !result.actions.is_empty(),
            "Emergency should fire at 20% HP"
        );
        assert_eq!(result.actions[0].entry_name, "Deflection");
    }

    #[test]
    fn warrior_combat_rotations_skip_during_downtime() {
        let w = WarriorStrategy::new(1);
        let mut groups = w.rotation_groups().unwrap();
        let player = SpawnData::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &DEFAULT_CONFIG,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };

        // All combat groups should produce nothing during downtime
        for g in groups.iter_mut().filter(|g| g.name != "Downtime") {
            let result = crate::combat::rotation::execute_group(g, &ctx);
            assert!(
                result.actions.is_empty(),
                "Group '{}' should not run during downtime",
                g.name
            );
        }
    }

    #[test]
    fn warrior_full_rotation_execution() {
        let w = WarriorStrategy::new(1);
        let mut groups = w.rotation_groups().unwrap();
        let target = SpawnData {
            spawn_id: 42,
            ..SpawnData::default()
        };
        let mut player = SpawnData::default();
        player.hp_current = 8000;
        player.hp_max = 10000;
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &DEFAULT_CONFIG,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };

        // Execute full rotation — should return an action from HateTools (first combat group)
        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx);
        assert!(action.is_some(), "Should produce an action during combat");
        assert_eq!(action.unwrap().entry_name, "Taunt");
    }

    // --- AbilitySet tests ---

    #[test]
    fn warrior_has_ability_sets() {
        let w = WarriorStrategy::new(1);
        let sets = w.ability_sets();
        assert!(!sets.is_empty(), "Warrior should define ability sets");
        let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Deflection"));
        assert!(names.contains(&"Carapace"));
        assert!(names.contains(&"Blade"));
    }

    #[test]
    fn warrior_ability_resolution_at_65() {
        let w = WarriorStrategy::new(1);
        let sets = w.ability_sets();
        let known: Vec<textquest_common::combat::KnownAbility> = sets
            .iter()
            .flat_map(|s| &s.candidates)
            .map(|c| textquest_common::combat::KnownAbility {
                name: c.name.clone(),
                spell_id: c.spell_id,
                level: c.min_level,
            })
            .collect();
        let resolved = textquest_common::combat::resolve_abilities(&sets, &known, 65);
        let deflection = resolved
            .get("Deflection")
            .expect("should resolve Deflection");
        assert_eq!(deflection.ability_name, "Deflection Discipline");
        let carapace = resolved.get("Carapace").expect("should resolve Carapace");
        assert_eq!(carapace.ability_name, "Stonewall Discipline");
    }

    #[test]
    fn warrior_ability_resolution_at_55() {
        let w = WarriorStrategy::new(1);
        let sets = w.ability_sets();
        let known: Vec<textquest_common::combat::KnownAbility> = sets
            .iter()
            .flat_map(|s| &s.candidates)
            .map(|c| textquest_common::combat::KnownAbility {
                name: c.name.clone(),
                spell_id: c.spell_id,
                level: c.min_level,
            })
            .collect();
        let resolved = textquest_common::combat::resolve_abilities(&sets, &known, 55);
        // At level 55, Deflection (62) is too high — should pick Evasive (52)
        let deflection = resolved
            .get("Deflection")
            .expect("should resolve Deflection");
        assert_eq!(deflection.ability_name, "Evasive Discipline");
        // Carapace: Stonewall (65) too high, picks Defensive (55)
        let carapace = resolved.get("Carapace").expect("should resolve Carapace");
        assert_eq!(carapace.ability_name, "Defensive Discipline");
        // Blade: Mighty Strike (54) should resolve
        assert!(resolved.contains_key("Blade"));
    }
}
