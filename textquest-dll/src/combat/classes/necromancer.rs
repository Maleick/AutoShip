use textquest_common::combat::{
    AbilityCandidate, EQExpansion, AbilitySet, ActionType, CombatRole, CombatStateReq, ConditionExpr,
    SpellEntry, TargetSelector,
};

use crate::combat::{
    rotation::{self, RotationGroup},
    strategy::{self, ClassStrategy, CombatContext, PetAction},
};

const EMERGENCY_LIFETAP_HP_PCT: f32 = 45.0;
const FEIGN_DEATH_HP_PCT: f32 = 18.0;
const RESIST_DEBUFF_MANA_PCT: f32 = 40.0;
const DISEASE_DOT_MANA_PCT: f32 = 35.0;
const FIRE_DOT_MANA_PCT: f32 = 30.0;
const POISON_DOT_MANA_PCT: f32 = 25.0;
const LIFETAP_MANA_PCT: f32 = 15.0;
const RESIST_DEBUFF_TARGET_HP_PCT: f32 = 95.0;
const DISEASE_DOT_TARGET_HP_PCT: f32 = 85.0;
const FIRE_DOT_TARGET_HP_PCT: f32 = 75.0;
const POISON_DOT_TARGET_HP_PCT: f32 = 65.0;
const COMBAT_LIFETAP_TARGET_HP_PCT: f32 = 55.0;

fn mana_and_target_hp_above(mana_pct: f32, target_hp_pct: f32) -> ConditionExpr {
    ConditionExpr::And(vec![
        ConditionExpr::ManaAbove(mana_pct),
        ConditionExpr::TargetHpAbove(target_hp_pct),
    ])
}

/// Necromancer strategy: DoT-focused DPS with pet, lifetap sustain, feign death
/// escape. EQ class ID: 11
pub struct NecromancerStrategy {
    class_id: u8,
}

impl NecromancerStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "ResistDebuff".into(),
                candidates: vec![AbilityCandidate {
                    name: "Scent of Terris".into(),
                    min_level: 60,
                    spell_id: -1,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "DiseaseDot".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Dark Plague".into(),
                        min_level: 61,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Splurt".into(),
                        min_level: 53,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "FireDot".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Night Fire".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Funeral Pyre of Kelador".into(),
                        min_level: 60,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Pyrocruor".into(),
                        min_level: 56,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "PoisonDot".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Blood of Thule".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Legacy of Zek".into(),
                        min_level: 62,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Lifetap".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Gangrenous Touch of Zum'uul".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Touch of Mujaki".into(),
                        min_level: 62,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Touch of Night".into(),
                        min_level: 60,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "FeignDeath".into(),
                candidates: vec![AbilityCandidate {
                    name: "Death Peace".into(),
                    min_level: 60,
                    spell_id: -1,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "SelfBuff".into(),
                candidates: vec![AbilityCandidate {
                    name: "Arch Lich".into(),
                    min_level: 60,
                    spell_id: -1,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "PetBuff".into(),
                candidates: vec![AbilityCandidate {
                    name: "Rune of Death".into(),
                    min_level: 62,
                    spell_id: -1,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Root".into(),
                candidates: vec![AbilityCandidate {
                    name: "Petrifying Earth".into(),
                    min_level: 60,
                    spell_id: -1,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "ManaTap".into(),
                candidates: vec![AbilityCandidate {
                    name: "Saryrn's Kiss".into(),
                    min_level: 64,
                    spell_id: -1,
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
                    TargetSelector::AutoTarget,
                    CombatStateReq::Combat,
                );
                g.hp_threshold = Some(EMERGENCY_LIFETAP_HP_PCT);
                g.full_rotation = true;
                g.entries = vec![
                    rotation::entry_if(
                        "Lifetap",
                        ActionType::Spell("Lifetap".into()),
                        ConditionExpr::ManaAbove(LIFETAP_MANA_PCT),
                    ),
                    rotation::entry_if(
                        "FeignDeath",
                        ActionType::Spell("FeignDeath".into()),
                        ConditionExpr::HpBelow(FEIGN_DEATH_HP_PCT),
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
                        "ResistDebuff",
                        ActionType::Spell("ResistDebuff".into()),
                        mana_and_target_hp_above(
                            RESIST_DEBUFF_MANA_PCT,
                            RESIST_DEBUFF_TARGET_HP_PCT,
                        ),
                    ),
                    rotation::entry_if(
                        "DiseaseDot",
                        ActionType::Spell("DiseaseDot".into()),
                        mana_and_target_hp_above(DISEASE_DOT_MANA_PCT, DISEASE_DOT_TARGET_HP_PCT),
                    ),
                    rotation::entry_if(
                        "FireDot",
                        ActionType::Spell("FireDot".into()),
                        mana_and_target_hp_above(FIRE_DOT_MANA_PCT, FIRE_DOT_TARGET_HP_PCT),
                    ),
                    rotation::entry_if(
                        "PoisonDot",
                        ActionType::Spell("PoisonDot".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::PlayerLevelAtLeast(62),
                            ConditionExpr::ManaAbove(POISON_DOT_MANA_PCT),
                            ConditionExpr::TargetHpAbove(POISON_DOT_TARGET_HP_PCT),
                        ]),
                    ),
                    rotation::entry_if(
                        "Lifetap",
                        ActionType::Spell("Lifetap".into()),
                        ConditionExpr::And(vec![
                            ConditionExpr::ManaAbove(LIFETAP_MANA_PCT),
                            ConditionExpr::TargetHpBelow(COMBAT_LIFETAP_TARGET_HP_PCT),
                        ]),
                    ),
                ];
                g
            },
        ]
    }

    fn is_lifetap(name: &str) -> bool {
        name.contains("Tap")
            || name.contains("tap")
            || name.contains("Drain")
            || name.contains("Leech")
    }

    fn is_dot(name: &str) -> bool {
        name.contains("Venom")
            || name.contains("Poison")
            || name.contains("Darkness")
            || name.contains("Plague")
            || name.contains("Disease")
            || name.contains("Fire")
    }
}

impl ClassStrategy for NecromancerStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        ctx.target.map(|t| t.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        let hp_pct = ctx.player.hp_pct();

        // Priority 1: Lifetap when HP is low (self-sustain)
        if hp_pct < EMERGENCY_LIFETAP_HP_PCT
            && let Some(tap) = ctx
                .config
                .spells
                .iter()
                .filter(|s| Self::is_lifetap(&s.name))
                .filter(|s| mana_pct >= s.min_mana_pct)
                .max_by_key(|s| s.priority)
                .cloned()
        {
            return Some(tap);
        }

        // Priority 2: DoTs (necro's bread and butter)
        if let Some(dot) = ctx
            .config
            .spells
            .iter()
            .filter(|s| Self::is_dot(&s.name))
            .filter(|s| mana_pct >= s.min_mana_pct)
            .max_by_key(|s| s.priority)
            .cloned()
        {
            return Some(dot);
        }

        // Priority 3: Any available spell
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

    fn aoe_threshold(&self) -> u8 {
        255 // Necros don't AoE (DoT-based)
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
            positional: None,
        }
    }

    fn necro_known_spells() -> Vec<textquest_common::combat::KnownAbility> {
        [
            "Scent of Terris",
            "Splurt",
            "Pyrocruor",
            "Touch of Night",
            "Arch Lich",
            "Gangrenous Touch of Zum'uul",
            "Funeral Pyre of Kelador",
            "Death Peace",
            "Dark Plague",
            "Touch of Mujaki",
            "Legacy of Zek",
            "Petrifying Earth",
            "Rune of Death",
            "Saryrn's Kiss",
            "Blood of Thule",
            "Night Fire",
            "Child of Bertoxxulous",
        ]
        .into_iter()
        .enumerate()
        .map(|(idx, name)| textquest_common::combat::KnownAbility {
            name: name.to_string(),
            spell_id: 6000 + idx as i32,
            level: 1,
        })
        .collect()
    }

    #[test]
    fn necro_class_id() {
        let necro = NecromancerStrategy::new(11);
        assert_eq!(necro.class_id(), 11);
    }

    #[test]
    fn necro_role_is_ranged_dps() {
        let necro = NecromancerStrategy::new(11);
        assert_eq!(necro.role(), CombatRole::DpsRanged);
    }

    #[test]
    fn necro_no_aoe() {
        let necro = NecromancerStrategy::new(11);
        assert_eq!(necro.aoe_threshold(), 255);
    }

    #[test]
    fn necro_always_assists() {
        let necro = NecromancerStrategy::new(11);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config, false);
        assert!(necro.should_assist(&ctx));
    }

    #[test]
    fn necro_select_target_returns_current_target() {
        let necro = NecromancerStrategy::new(11);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 99,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &config, true);
        assert_eq!(necro.select_target(&ctx), Some(99));
    }

    #[test]
    fn necro_lifetap_priority_when_low_hp() {
        let necro = NecromancerStrategy::new(11);
        let mut player = SpawnData::default();
        player.hp_current = 3000;
        player.hp_max = 10000; // 30% HP
        player.mana_current = 5000;
        player.mana_max = 10000;
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
                    name: "Venom of Solusek".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, None, &config, true);
        let spell = necro.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Lifetap"); // lifetap priority at low HP
    }

    #[test]
    fn necro_dot_priority_when_hp_healthy() {
        let necro = NecromancerStrategy::new(11);
        let mut player = SpawnData::default();
        player.hp_current = 9000;
        player.hp_max = 10000; // 90% HP
        player.mana_current = 5000;
        player.mana_max = 10000;
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
                    name: "Venom of Solusek".into(),
                    min_mana_pct: 10.0,
                    priority: 10,
                    is_aoe: false,
                },
                textquest_common::combat::SpellEntry {
                    slot: 3,
                    spell_id: 300,
                    name: "Nuke".into(),
                    min_mana_pct: 10.0,
                    priority: 3,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, None, &config, true);
        let spell = necro.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Venom of Solusek"); // DoT priority
    }

    #[test]
    fn necro_fallback_when_no_dots_or_taps() {
        let necro = NecromancerStrategy::new(11);
        let mut player = SpawnData::default();
        player.hp_current = 9000;
        player.hp_max = 10000;
        player.mana_current = 5000;
        player.mana_max = 10000;
        let config = CombatConfig {
            spells: vec![textquest_common::combat::SpellEntry {
                slot: 1,
                spell_id: 100,
                name: "Nuke".into(),
                min_mana_pct: 10.0,
                priority: 3,
                is_aoe: false,
            }],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, None, &config, true);
        let spell = necro.select_spell(&ctx).unwrap();
        assert_eq!(spell.name, "Nuke"); // fallback
    }

    #[test]
    fn necro_no_spells_returns_none() {
        let necro = NecromancerStrategy::new(11);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config, true);
        assert!(necro.select_spell(&ctx).is_none());
    }

    #[test]
    fn necro_mana_filter_skips_expensive_spells() {
        let necro = NecromancerStrategy::new(11);
        let mut player = SpawnData::default();
        player.hp_current = 9000;
        player.hp_max = 10000;
        player.mana_current = 500;
        player.mana_max = 10000; // 5% mana
        let config = CombatConfig {
            spells: vec![textquest_common::combat::SpellEntry {
                slot: 1,
                spell_id: 100,
                name: "Venom of Fire".into(),
                min_mana_pct: 20.0, // requires 20%, we have 5%
                priority: 10,
                is_aoe: false,
            }],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, None, &config, true);
        assert!(necro.select_spell(&ctx).is_none());
    }

    #[test]
    fn necro_has_rotation_groups() {
        let necro = NecromancerStrategy::new(11);
        let groups = necro.rotation_groups();
        assert!(groups.is_some(), "Necromancer should use rotation groups");
    }

    #[test]
    fn necro_rotation_starts_with_resist_debuff_then_dot_stack() {
        let necro = NecromancerStrategy::new(11);
        let mut groups = necro
            .rotation_groups()
            .expect("Necromancer should expose rotations");
        let player = SpawnData {
            level: 65,
            hp_current: 9000,
            hp_max: 10000,
            mana_current: 9000,
            mana_max: 10000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 77,
            hp_current: 10000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &config, true);

        let expected = ["ResistDebuff", "DiseaseDot", "FireDot", "PoisonDot"];
        for ability_name in expected {
            let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx)
                .expect("Necromancer rotation should yield an action");
            assert_eq!(action.entry_name, ability_name);
        }
    }

    #[test]
    fn necro_emergency_lifetap_preempts_combat_rotation() {
        let necro = NecromancerStrategy::new(11);
        let mut groups = necro
            .rotation_groups()
            .expect("Necromancer should expose rotations");
        let player = SpawnData {
            hp_current: 4200,
            hp_max: 10000,
            mana_current: 7000,
            mana_max: 10000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 77,
            hp_current: 10000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &config, true);

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx)
            .expect("Necromancer emergency rotation should yield a spell");
        assert_eq!(action.entry_name, "Lifetap");
    }

    #[test]
    fn necro_critical_hp_with_low_mana_uses_feign_death() {
        let necro = NecromancerStrategy::new(11);
        let mut groups = necro
            .rotation_groups()
            .expect("Necromancer should expose rotations");
        let player = SpawnData {
            hp_current: 1200,
            hp_max: 10000,
            mana_current: 1200,
            mana_max: 10000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 77,
            hp_current: 10000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &config, true);

        let action = crate::combat::rotation::execute_rotations(&mut groups, &ctx)
            .expect("Necromancer critical-hp emergency should yield a spell");
        assert_eq!(action.entry_name, "FeignDeath");
    }

    #[test]
    fn necro_rotation_stops_casting_low_value_dots_when_mana_reserved() {
        let necro = NecromancerStrategy::new(11);
        let mut groups = necro
            .rotation_groups()
            .expect("Necromancer should expose rotations");
        let player = SpawnData {
            hp_current: 9000,
            hp_max: 10000,
            mana_current: 2400,
            mana_max: 10000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 77,
            hp_current: 10000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &config, true);

        assert!(
            crate::combat::rotation::execute_rotations(&mut groups, &ctx).is_none(),
            "Necromancer should reserve mana instead of loading low-value DoTs"
        );
    }

    #[test]
    fn necro_rotation_skips_poison_dot_before_level_62() {
        let necro = NecromancerStrategy::new(11);
        let mut groups = necro
            .rotation_groups()
            .expect("Necromancer should expose rotations");
        let combat_group = groups
            .iter_mut()
            .find(|group| group.name == "Combat")
            .expect("Necromancer combat rotation");
        combat_group.current_step = 3;

        let player = SpawnData {
            level: 61,
            hp_current: 9000,
            hp_max: 10000,
            mana_current: 9000,
            mana_max: 10000,
            ..SpawnData::default()
        };
        let target = SpawnData {
            spawn_id: 77,
            hp_current: 7000,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &config, true);

        assert!(
            crate::combat::rotation::execute_rotations(&mut groups, &ctx).is_none(),
            "Necromancer level 61 profiles should not attempt the unresolved PoisonDot slot"
        );
    }

    #[test]
    fn necro_ability_resolution_hits_60_61_62_and_65_breakpoints() {
        let necro = NecromancerStrategy::new(11);
        let sets = necro.ability_sets();
        assert!(!sets.is_empty(), "Necromancer should define ability sets");

        let r60 = textquest_common::combat::resolve_abilities(&sets, &necro_known_spells(), 60);
        let r61 = textquest_common::combat::resolve_abilities(&sets, &necro_known_spells(), 61);
        let r62 = textquest_common::combat::resolve_abilities(&sets, &necro_known_spells(), 62);
        let r65 = textquest_common::combat::resolve_abilities(&sets, &necro_known_spells(), 65);

        assert_eq!(
            r60.get("FireDot").expect("level 60 fire dot").ability_name,
            "Funeral Pyre of Kelador"
        );
        assert_eq!(
            r61.get("DiseaseDot")
                .expect("level 61 disease dot")
                .ability_name,
            "Dark Plague"
        );
        assert_eq!(
            r62.get("PetBuff").expect("level 62 pet buff").ability_name,
            "Rune of Death"
        );
        assert_eq!(
            r65.get("PoisonDot")
                .expect("level 65 poison dot")
                .ability_name,
            "Blood of Thule"
        );
    }
}
