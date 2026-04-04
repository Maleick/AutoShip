use dmft_common::combat::{
    ActionType, CombatRole, CombatStateReq, ConditionExpr, SpellEntry, TargetSelector,
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
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use dmft_common::combat::CombatConfig;
    use dmft_common::types::SpawnData;

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
            target_is_mezzed: false,
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
            target_is_mezzed: false,
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
            target_is_mezzed: false,
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
            target_is_mezzed: false,
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
            target_is_mezzed: false,
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
            target_is_mezzed: false,
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
            target_is_mezzed: false,
        };

        // Execute full rotation — should return an action from HateTools (first combat group)
        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx);
        assert!(action.is_some(), "Should produce an action during combat");
        assert_eq!(action.unwrap().entry_name, "Taunt");
    }
}
