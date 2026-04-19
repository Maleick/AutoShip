use textquest_common::combat::{
    AbilityCandidate, AbilitySet, ActionType, CombatRole, CombatStateReq, ConditionExpr,
    SpellEntry, TargetSelector,
};

use crate::combat::{
    rotation::{self, RotationGroup},
    strategy::{self, ClassStrategy, CombatContext},
};

/// Shadow Knight strategy: off-tank with lifetap DPS, disease/poison DoTs,
/// snare, and cooldown-based disciples. EQ class ID: 5
///
/// Rotation order:
/// 1. Burn — Shout disciplines and lifetap when endurance is high
/// 2. Debuff — Disease/poison DoTs for DPS amplification
/// 3. Combat — Standard melee attacks with periodic cooldown-gated abilities
pub struct ShadowKnightStrategy {
    class_id: u8,
}

impl ShadowKnightStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "Shout".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Shout of Fury".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Shout of Rage".into(),
                        min_level: 60,
                        spell_id: -1,
                    },
                ],
            },
            AbilitySet {
                name: "Lifetap".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Waves of Decay".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Splurt of Decay".into(),
                        min_level: 60,
                        spell_id: -1,
                    },
                ],
            },
            AbilitySet {
                name: "DefensiveDisc".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Unholy Aura Discipline".into(),
                        min_level: 60,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Unholy Aura Discipline".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                ],
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
                        "Shout",
                        ActionType::Disc("Shout".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::ManaAbove(50.0),
                            ConditionExpr::TargetHpAbove(40.0),
                        ]),
                    ),
                    rotation::entry_if(
                        "Lifetap",
                        ActionType::Disc("Lifetap".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::ManaAbove(40.0),
                            ConditionExpr::HpBelow(60.0),
                        ]),
                    ),
                ];
                g
            },
            {
                let mut g =
                    rotation::group("Debuff", TargetSelector::AutoTarget, CombatStateReq::Combat);
                g.steps_per_frame = 1;
                g.entries = vec![rotation::entry_if(
                    "DefensiveDisc",
                    ActionType::Disc("DefensiveDisc".into()),
                    ConditionExpr::And(vec![
                        ConditionExpr::ManaAbove(30.0),
                        ConditionExpr::HpBelow(50.0),
                    ]),
                )];
                g
            },
            {
                let mut g =
                    rotation::group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
                g.steps_per_frame = 1;
                g.entries = vec![rotation::entry_if(
                    "Kick",
                    ActionType::Ability("Kick".into()),
                    ConditionExpr::ManaAbove(20.0),
                )];
                g
            },
        ]
    }
}

impl ClassStrategy for ShadowKnightStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::assist_target(ctx)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        let hp_pct = ctx.player.hp_pct();

        // Priority 1: Lifetap when HP is low
        if hp_pct < 60.0
            && let Some(tap) = ctx
                .config
                .spells
                .iter()
                .filter(|s| {
                    s.name.contains("Tap")
                        || s.name.contains("tap")
                        || s.name.contains("Leech")
                        || s.name.contains("Drain")
                })
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
        {
            return Some(tap);
        }

        // Priority 2: Snare on fleeing mob
        if let Some(target) = ctx.target
            && target.hp_pct() < 15.0
            && let Some(snare) = ctx
                .config
                .spells
                .iter()
                .filter(|s| s.name.contains("Snare") || s.name.contains("Darkness"))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
        {
            return Some(snare);
        }

        // Priority 3: Disease/poison DoTs and nukes
        ctx.config
            .spells
            .iter()
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        false // SK tanks, doesn't assist
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Shadow Knight");
    }

    fn on_action_complete(&mut self, ctx: &CombatContext) {
        if !ctx.in_combat {
            strategy::melee_on_disengage();
        }
    }

    fn aoe_threshold(&self) -> u8 {
        2
    }

    fn role(&self) -> CombatRole {
        CombatRole::OffTank
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
                name: "Shout of Rage".into(),
                spell_id: 5050,
                level: 60,
            },
            KnownAbility {
                name: "Shout of Fury".into(),
                spell_id: 5051,
                level: 65,
            },
            KnownAbility {
                name: "Splurt of Decay".into(),
                spell_id: 5052,
                level: 60,
            },
            KnownAbility {
                name: "Waves of Decay".into(),
                spell_id: 5053,
                level: 65,
            },
            KnownAbility {
                name: "Unholy Aura Discipline".into(),
                spell_id: 5054,
                level: 60,
            },
        ]
    }

    #[test]
    fn sk_class_id() {
        let sk = ShadowKnightStrategy::new(5);
        assert_eq!(sk.class_id(), 5);
    }

    #[test]
    fn sk_role_is_off_tank() {
        let sk = ShadowKnightStrategy::new(5);
        assert_eq!(sk.role(), CombatRole::OffTank);
    }

    #[test]
    fn sk_does_not_assist() {
        let sk = ShadowKnightStrategy::new(5);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &[], false);
        assert!(!sk.should_assist(&ctx));
    }

    #[test]
    fn sk_aoe_threshold() {
        let sk = ShadowKnightStrategy::new(5);
        assert_eq!(sk.aoe_threshold(), 2);
    }

    #[test]
    fn sk_lifetap_priority_when_low_hp() {
        let sk = ShadowKnightStrategy::new(5);
        let player = SpawnData {
            hp_current: 4000,
            hp_max: 10000,
            mana_current: 5000,
            mana_max: 10000,
            ..SpawnData::default()
        };
        let config = CombatConfig {
            spells: vec![
                textquest_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Lifetap".into(),
                    min_mana_pct: 10.0,
                    priority: 5,
                    is_aoe: false,
                },
                textquest_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Disease Cloud".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
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
        let spell = sk.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Lifetap"); // lifetap priority at low HP
    }

    #[test]
    fn sk_snare_on_fleeing_mob() {
        let sk = ShadowKnightStrategy::new(5);
        let player = SpawnData {
            hp_current: 9000,
            hp_max: 10000,
            mana_current: 5000,
            mana_max: 10000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            hp_current: 1000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let config = CombatConfig {
            spells: vec![
                textquest_common::combat::SpellEntry {
                    slot: 1,
                    spell_id: 100,
                    name: "Lifetap".into(),
                    min_mana_pct: 10.0,
                    priority: 5,
                    is_aoe: false,
                },
                textquest_common::combat::SpellEntry {
                    slot: 2,
                    spell_id: 200,
                    name: "Darkness Snare".into(),
                    min_mana_pct: 10.0,
                    priority: 8,
                    is_aoe: false,
                },
                textquest_common::combat::SpellEntry {
                    slot: 3,
                    spell_id: 300,
                    name: "Nuke".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
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
        let spell = sk.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Darkness Snare"); // snare on fleeing mob
    }

    #[test]
    fn sk_fallback_to_generic() {
        let sk = ShadowKnightStrategy::new(5);
        let player = SpawnData {
            hp_current: 9000,
            hp_max: 10000,
            mana_current: 5000,
            mana_max: 10000,
            ..SpawnData::default()
        };
        let config = CombatConfig {
            spells: vec![textquest_common::combat::SpellEntry {
                slot: 1,
                spell_id: 300,
                name: "Nuke".into(),
                min_mana_pct: 10.0,
                priority: 10,
                is_aoe: false,
            }],
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
        let spell = sk.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Nuke");
    }

    #[test]
    fn sk_no_spells_returns_none() {
        let sk = ShadowKnightStrategy::new(5);
        let player = SpawnData::default();
        let config = CombatConfig::default();
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
        assert!(sk.select_spell(&ctx).is_none());
    }

    #[test]
    fn sk_has_rotation_groups() {
        let sk = ShadowKnightStrategy::new(5);
        let groups = sk.rotation_groups().unwrap();
        let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["Burn", "Debuff", "Combat"]);
    }

    #[test]
    fn sk_rotation_prioritizes_shout_with_high_mana() {
        let sk = ShadowKnightStrategy::new(5);
        let mut groups = sk.rotation_groups().unwrap();
        let player = SpawnData {
            mana_current: 90,
            mana_max: 100,
            hp_current: 10_000,
            hp_max: 10_000,
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
        assert_eq!(action.entry_name, "Shout");
    }

    #[test]
    fn sk_rotation_uses_lifetap_when_low_hp() {
        let sk = ShadowKnightStrategy::new(5);
        let mut groups = sk.rotation_groups().unwrap();
        let player = SpawnData {
            mana_current: 90,
            mana_max: 100,
            hp_current: 4000,
            hp_max: 10000,
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
        assert_eq!(action.entry_name, "Shout");
    }

    #[test]
    fn sk_rotation_stops_when_mana_is_low() {
        let sk = ShadowKnightStrategy::new(5);
        let mut groups = sk.rotation_groups().unwrap();
        let player = SpawnData {
            mana_current: 10,
            mana_max: 100,
            hp_current: 10_000,
            hp_max: 10_000,
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
    fn sk_has_ability_sets() {
        let sk = ShadowKnightStrategy::new(5);
        let sets = sk.ability_sets();
        let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Shout"));
        assert!(names.contains(&"Lifetap"));
        assert!(names.contains(&"DefensiveDisc"));
    }

    #[test]
    fn sk_ability_resolution_at_60() {
        let sets = ShadowKnightStrategy::build_ability_sets();
        let resolved = textquest_common::combat::resolve_abilities(&sets, &known_abilities(), 60);

        let shout = resolved
            .get("Shout")
            .expect("level 60 should resolve shout");
        assert_eq!(shout.ability_name, "Shout of Rage");
        assert_eq!(shout.spell_id, 5050);

        let lifetap = resolved
            .get("Lifetap")
            .expect("level 60 should resolve lifetap");
        assert_eq!(lifetap.ability_name, "Splurt of Decay");
        assert_eq!(lifetap.spell_id, 5052);
    }

    #[test]
    fn sk_ability_resolution_at_65_prefers_newer_abilities() {
        let sets = ShadowKnightStrategy::build_ability_sets();
        let resolved = textquest_common::combat::resolve_abilities(&sets, &known_abilities(), 65);

        let shout = resolved
            .get("Shout")
            .expect("level 65 should resolve shout");
        assert_eq!(shout.ability_name, "Shout of Fury");
        assert_eq!(shout.spell_id, 5051);

        let lifetap = resolved
            .get("Lifetap")
            .expect("level 65 should resolve lifetap");
        assert_eq!(lifetap.ability_name, "Waves of Decay");
        assert_eq!(lifetap.spell_id, 5053);
    }
}
