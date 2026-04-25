use std::collections::HashMap;

use textquest_common::combat::{
    AbilityCandidate, EQExpansion, AbilitySet, ActionType, CombatRole, CombatStateReq, ConditionExpr,
    KnownAbility, SpellEntry, TargetSelector,
};

use crate::combat::{
    rotation::{self, RotationGroup},
    strategy::{self, AbilityResolution, ClassStrategy, CombatContext},
};

const TICKS_PER_SECOND: u32 = 20;
const ROGUE_TIMER_BURN: &str = "rogue-burn";
const ROGUE_TIMER_PRECISION: &str = "rogue-precision";
const ROGUE_TIMER_UTILITY: &str = "rogue-utility";

const fn seconds_to_ticks(seconds: u32) -> u32 {
    seconds * TICKS_PER_SECOND
}

const fn minutes_to_ticks(minutes: u32) -> u32 {
    seconds_to_ticks(minutes * 60)
}

/// Rogue strategy: melee DPS with backstab on the melee-skill path and
/// discipline burns on the activated rotation path. EQ class ID: 9.
pub struct RogueStrategy {
    class_id: u8,
}

impl RogueStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "Precision".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Twisted Chance Discipline".into(),
                        min_level: 65,
                        spell_id: 4695,
                    },
                    AbilityCandidate {
                        name: "Deadly Precision Discipline".into(),
                        min_level: 63,
                        spell_id: 4694,
                    },
                    AbilityCandidate {
                        name: "Blinding Speed Discipline".into(),
                        min_level: 58,
                        spell_id: 4677,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Duelist".into(),
                candidates: vec![AbilityCandidate {
                    name: "Duelist Discipline".into(),
                    min_level: 59,
                    spell_id: 4676,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "WeaponAffinity".into(),
                candidates: vec![AbilityCandidate {
                    name: "Weapon Affinity Discipline".into(),
                    min_level: 61,
                    spell_id: 4696,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "CrowdControl".into(),
                candidates: vec![AbilityCandidate {
                    name: "Rogue's Ploy".into(),
                    min_level: 61,
                    spell_id: 6751,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "StealthStrike".into(),
                candidates: vec![AbilityCandidate {
                    name: "Kyv Strike".into(),
                    min_level: 65,
                    spell_id: 5017,
                }],
                min_expansion: EQExpansion::Classic,
            },
        ]
    }

    fn build_rotations() -> Vec<RotationGroup> {
        let mut burn = rotation::group("Burn", TargetSelector::AutoTarget, CombatStateReq::Combat);
        burn.steps_per_frame = 1;
        burn.entries = vec![
            rotation::entry_if(
                "Precision",
                ActionType::Disc("Precision".into()),
                ConditionExpr::EnduranceAbove(55.0),
            ),
            rotation::entry_if(
                "Duelist",
                ActionType::Disc("Duelist".into()),
                ConditionExpr::EnduranceAbove(45.0),
            ),
            rotation::entry_if(
                "WeaponAffinity",
                ActionType::Disc("WeaponAffinity".into()),
                ConditionExpr::EnduranceAbove(35.0),
            ),
        ];

        vec![burn]
    }

    fn metadata_for_resolved_name(
        resolved_name: &str,
    ) -> (Option<u32>, Option<&'static str>, Option<u32>) {
        match resolved_name {
            "Twisted Chance Discipline" => {
                (Some(minutes_to_ticks(22)), Some(ROGUE_TIMER_BURN), None)
            }
            "Duelist Discipline" => (Some(minutes_to_ticks(22)), Some(ROGUE_TIMER_BURN), None),
            "Deadly Precision Discipline" => {
                (Some(minutes_to_ticks(5)), Some(ROGUE_TIMER_PRECISION), None)
            }
            "Blinding Speed Discipline" => (
                Some(minutes_to_ticks(20)),
                Some(ROGUE_TIMER_PRECISION),
                None,
            ),
            "Weapon Affinity Discipline" => (Some(minutes_to_ticks(30)), None, None),
            "Rogue's Ploy" => (Some(seconds_to_ticks(30)), Some(ROGUE_TIMER_UTILITY), None),
            "Kyv Strike" => (Some(seconds_to_ticks(30)), Some(ROGUE_TIMER_UTILITY), None),
            _ => (None, None, None),
        }
    }
}

impl ClassStrategy for RogueStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::assist_target(ctx)
    }

    fn select_spell(&self, _ctx: &CombatContext) -> Option<SpellEntry> {
        None
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Rogue");
    }

    fn on_action_complete(&mut self, ctx: &CombatContext) {
        if !ctx.in_combat {
            strategy::melee_on_disengage();
        }
    }

    fn aoe_threshold(&self) -> u8 {
        255
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsMelee
    }

    fn rotation_groups(&self) -> Option<Vec<RotationGroup>> {
        // Backstab stays on the melee-skill path, while the activated rotation
        // only handles long-reuse disciplines that are safe to automate.
        // Rogue's Ploy and Kyv Strike are mapped via ability sets but omitted
        // from the automatic rotation because they are situational control and
        // stealth tools rather than sustain actions.
        Some(Self::build_rotations())
    }

    fn ability_sets(&self) -> Vec<AbilitySet> {
        Self::build_ability_sets()
    }

    fn resolve_abilities_for_character(
        &self,
        known: &[KnownAbility],
        character_level: u8,
    ) -> HashMap<String, AbilityResolution> {
        textquest_common::combat::resolve_abilities(
            &Self::build_ability_sets(),
            known,
            character_level,
        )
        .into_iter()
        .map(|(set_name, resolved)| {
            let (cooldown_ticks, shared_cooldown_key, shared_cooldown_ticks) =
                Self::metadata_for_resolved_name(&resolved.ability_name);
            (
                set_name,
                AbilityResolution {
                    set_name: resolved.set_name,
                    ability_name: resolved.ability_name,
                    spell_id: resolved.spell_id,
                    min_level: resolved.min_level,
                    cooldown_ticks,
                    shared_cooldown_key: shared_cooldown_key.map(str::to_string),
                    shared_cooldown_ticks,
                },
            )
        })
        .collect()
    }

    fn uses_mana_for_combat(&self) -> bool {
        false
    }

    fn sync_resolved_rotation_groups(
        &self,
        groups: &mut [RotationGroup],
        resolved_abilities: &HashMap<String, AbilityResolution>,
    ) {
        let template_groups = Self::build_rotations();
        for group in groups {
            let Some(template_group) = template_groups
                .iter()
                .find(|candidate| candidate.name == group.name)
            else {
                group.entries.retain(|entry| {
                    matches!(
                        entry.action_type,
                        ActionType::Ability(_) | ActionType::Item(_)
                    ) || resolved_abilities.contains_key(&entry.name)
                });
                continue;
            };

            let preserved_step = group.current_step;
            group.entries = template_group
                .entries
                .iter()
                .filter(|entry| {
                    matches!(
                        entry.action_type,
                        ActionType::Ability(_) | ActionType::Item(_)
                    ) || resolved_abilities.contains_key(&entry.name)
                })
                .cloned()
                .collect();
            group.current_step = if group.entries.is_empty() {
                0
            } else {
                preserved_step.min(group.entries.len().saturating_sub(1))
            };
        }
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
        }
    }

    fn known_abilities() -> Vec<KnownAbility> {
        vec![
            KnownAbility {
                name: "Blinding Speed Discipline".into(),
                spell_id: 4677,
                level: 58,
            },
            KnownAbility {
                name: "Duelist Discipline".into(),
                spell_id: 4676,
                level: 59,
            },
            KnownAbility {
                name: "Weapon Affinity Discipline".into(),
                spell_id: 4696,
                level: 61,
            },
            KnownAbility {
                name: "Rogue's Ploy".into(),
                spell_id: 6751,
                level: 61,
            },
            KnownAbility {
                name: "Deadly Precision Discipline".into(),
                spell_id: 4694,
                level: 63,
            },
            KnownAbility {
                name: "Twisted Chance Discipline".into(),
                spell_id: 4695,
                level: 65,
            },
            KnownAbility {
                name: "Kyv Strike".into(),
                spell_id: 5017,
                level: 65,
            },
        ]
    }

    #[test]
    fn rogue_class_id() {
        let rogue = RogueStrategy::new(9);
        assert_eq!(rogue.class_id(), 9);
    }

    #[test]
    fn rogue_role_is_melee_dps() {
        let rogue = RogueStrategy::new(9);
        assert_eq!(rogue.role(), CombatRole::DpsMelee);
    }

    #[test]
    fn rogue_should_assist() {
        let rogue = RogueStrategy::new(9);
        let player = SpawnData::default();
        let ctx = make_ctx(&player, None, &DEFAULT_CONFIG, false);
        assert!(rogue.should_assist(&ctx));
    }

    #[test]
    fn rogue_uses_endurance_not_mana_for_combat() {
        let rogue = RogueStrategy::new(9);
        assert!(!rogue.uses_mana_for_combat());
    }

    #[test]
    fn rogue_no_aoe() {
        let rogue = RogueStrategy::new(9);
        assert_eq!(rogue.aoe_threshold(), 255);
    }

    #[test]
    fn select_target_returns_assist_target() {
        let rogue = RogueStrategy::new(9);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 66,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &DEFAULT_CONFIG, true);
        assert_eq!(rogue.select_target(&ctx), Some(66));
    }

    #[test]
    fn build_ability_sets_cover_level_tuned_lines() {
        let sets = RogueStrategy::build_ability_sets();
        let names = sets.iter().map(|set| set.name.as_str()).collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "Precision",
                "Duelist",
                "WeaponAffinity",
                "CrowdControl",
                "StealthStrike"
            ]
        );
    }

    #[test]
    fn resolve_abilities_for_level_60() {
        let rogue = RogueStrategy::new(9);
        let resolved = rogue.resolve_abilities_for_character(&known_abilities(), 60);

        assert_eq!(
            resolved
                .get("Precision")
                .map(|ability| ability.ability_name.as_str()),
            Some("Blinding Speed Discipline")
        );
        assert_eq!(
            resolved.get("Duelist").map(|ability| ability.spell_id),
            Some(4676)
        );
        assert!(!resolved.contains_key("WeaponAffinity"));
        assert!(!resolved.contains_key("CrowdControl"));
        assert!(!resolved.contains_key("StealthStrike"));
    }

    #[test]
    fn resolve_abilities_for_level_61_and_62() {
        let rogue = RogueStrategy::new(9);
        for level in [61, 62] {
            let resolved = rogue.resolve_abilities_for_character(&known_abilities(), level);

            assert_eq!(
                resolved
                    .get("Precision")
                    .map(|ability| ability.ability_name.as_str()),
                Some("Blinding Speed Discipline")
            );
            assert_eq!(
                resolved
                    .get("WeaponAffinity")
                    .map(|ability| ability.ability_name.as_str()),
                Some("Weapon Affinity Discipline")
            );
            assert_eq!(
                resolved
                    .get("CrowdControl")
                    .map(|ability| ability.ability_name.as_str()),
                Some("Rogue's Ploy")
            );
        }
    }

    #[test]
    fn resolve_abilities_for_level_65_prefers_twisted_chance() {
        let rogue = RogueStrategy::new(9);
        let resolved = rogue.resolve_abilities_for_character(&known_abilities(), 65);

        assert_eq!(
            resolved
                .get("Precision")
                .map(|ability| ability.ability_name.as_str()),
            Some("Twisted Chance Discipline")
        );
        assert_eq!(
            resolved
                .get("StealthStrike")
                .map(|ability| ability.ability_name.as_str()),
            Some("Kyv Strike")
        );
    }

    #[test]
    fn burn_rotation_prioritizes_precision_then_duelist_then_weapon_affinity() {
        let groups = RogueStrategy::build_rotations();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "Burn");
        assert_eq!(
            groups[0]
                .entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Precision", "Duelist", "WeaponAffinity"]
        );
    }

    #[test]
    fn level_60_rotation_omits_weapon_affinity_until_it_resolves() {
        let rogue = RogueStrategy::new(9);
        let resolved = rogue.resolve_abilities_for_character(&known_abilities(), 60);
        let mut groups = RogueStrategy::build_rotations();

        rogue.sync_resolved_rotation_groups(&mut groups, &resolved);

        assert_eq!(
            groups[0]
                .entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Precision", "Duelist"]
        );
    }

    #[test]
    fn level_61_rotation_restores_weapon_affinity_after_level_60_prune() {
        let rogue = RogueStrategy::new(9);
        let resolved_60 = rogue.resolve_abilities_for_character(&known_abilities(), 60);
        let resolved_61 = rogue.resolve_abilities_for_character(&known_abilities(), 61);
        let mut groups = RogueStrategy::build_rotations();

        rogue.sync_resolved_rotation_groups(&mut groups, &resolved_60);
        rogue.sync_resolved_rotation_groups(&mut groups, &resolved_61);

        assert_eq!(
            groups[0]
                .entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Precision", "Duelist", "WeaponAffinity"]
        );
    }

    #[test]
    fn rogue_combat_rotation_prefers_precision_when_endurance_is_high() {
        let mut player = SpawnData::default();
        player.spawn_id = 1;
        player.endurance_current = 900;
        player.endurance_max = 1000;
        let target = SpawnData {
            spawn_id: 42,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &DEFAULT_CONFIG, true);
        let mut groups = RogueStrategy::build_rotations();

        let selected = rotation::execute_rotations(&mut groups, &ctx).unwrap();
        assert_eq!(selected.entry_name, "Precision");
    }

    #[test]
    fn rogue_combat_rotation_holds_burns_when_endurance_is_low() {
        let mut player = SpawnData::default();
        player.spawn_id = 1;
        player.endurance_current = 200;
        player.endurance_max = 1000;
        let target = SpawnData {
            spawn_id: 42,
            ..SpawnData::default()
        };
        let ctx = make_ctx(&player, Some(&target), &DEFAULT_CONFIG, true);
        let mut groups = RogueStrategy::build_rotations();

        assert!(rotation::execute_rotations(&mut groups, &ctx).is_none());
    }

    #[test]
    fn rogue_resolution_exposes_cooldowns_and_shared_timers() {
        let rogue = RogueStrategy::new(9);
        let resolved = rogue.resolve_abilities_for_character(&known_abilities(), 65);

        let precision = resolved.get("Precision").expect("precision should resolve");
        assert_eq!(precision.ability_name, "Twisted Chance Discipline");
        assert_eq!(precision.cooldown_ticks, Some(minutes_to_ticks(22)));
        assert_eq!(
            precision.shared_cooldown_key.as_deref(),
            Some(ROGUE_TIMER_BURN)
        );

        let utility = resolved
            .get("CrowdControl")
            .expect("crowd control should resolve");
        assert_eq!(utility.ability_name, "Rogue's Ploy");
        assert_eq!(utility.cooldown_ticks, Some(seconds_to_ticks(30)));
        assert_eq!(
            utility.shared_cooldown_key.as_deref(),
            Some(ROGUE_TIMER_UTILITY)
        );
    }
}
