use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, PoisonError},
};

use textquest_common::combat::{
    AbilityCandidate, EQExpansion, AbilitySet, ActionType, CastResult, CombatRole, CombatStateReq,
    ConditionExpr, ResolvedAbility, SpellEntry, TargetSelector,
};

use crate::combat::{
    rotation::{self, RotationGroup},
    strategy::{self, ClassStrategy, CombatContext, PetAction},
};

const MOD_ROD_MANA_THRESHOLD: f32 = 40.0;
const PET_BUFF_MIN_MANA: f32 = 30.0;
const DEBUFF_MIN_MANA: f32 = 70.0;
const DEBUFF_MIN_TARGET_HP: f32 = 80.0;
const PRIMARY_NUKE_MIN_MANA: f32 = 55.0;
const SECONDARY_NUKE_MIN_MANA: f32 = 40.0;
const AOE_NUKE_MIN_MANA: f32 = 75.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingCast {
    Debuff { target_id: u32 },
    PetBuff { pet_id: u32 },
}

#[derive(Default)]
struct MagicianRuntimeState {
    resolved_abilities: HashMap<String, ResolvedAbility>,
    debuffed_targets: HashSet<u32>,
    buffed_pet_id: Option<u32>,
    pending_cast: Option<PendingCast>,
}

/// Magician strategy: pet-led ranged DPS with explicit debuff, AoE, nuke, and
/// downtime utility priorities.
///
/// Priority order:
/// 1. Send pet on engage.
/// 2. Out of combat, buff a newly summoned pet once with the best Burnout line.
/// 3. During combat, cast Mala on healthy targets once per target.
/// 4. Use the strongest available AoE nuke when the enemy count threshold and
///    mana threshold are both met.
/// 5. Use the primary single-target nuke line.
/// 6. Fall back to a cheaper secondary nuke line.
/// 7. Stop direct damage casting below the secondary mana threshold and let the
///    pet carry DPS while recovering mana.
///
/// EQ class ID: 13
pub struct MagicianStrategy {
    class_id: u8,
    runtime: Mutex<MagicianRuntimeState>,
}

impl MagicianStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            runtime: Mutex::new(MagicianRuntimeState::default()),
        }
    }

    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: "PetBuff".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Burnout V".into(),
                        min_level: 62,
                        spell_id: 5988,
                    },
                    AbilityCandidate {
                        name: "Burnout IV".into(),
                        min_level: 61,
                        spell_id: 5987,
                    },
                    AbilityCandidate {
                        name: "Greater Burnout".into(),
                        min_level: 44,
                        spell_id: 1748,
                    },
                    AbilityCandidate {
                        name: "Burnout".into(),
                        min_level: 16,
                        spell_id: 1033,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "PrimaryNuke".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Seeking Flame of Seukor".into(),
                        min_level: 59,
                        spell_id: 1715,
                    },
                    AbilityCandidate {
                        name: "Shock of Swords".into(),
                        min_level: 44,
                        spell_id: 1713,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "SecondaryNuke".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Shock of Steel".into(),
                        min_level: 60,
                        spell_id: 1716,
                    },
                    AbilityCandidate {
                        name: "Shock of Swords".into(),
                        min_level: 44,
                        spell_id: 1713,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "AoeNuke".into(),
                candidates: vec![AbilityCandidate {
                    name: "Sun Storm".into(),
                    min_level: 62,
                    spell_id: 5989,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "Debuff".into(),
                candidates: vec![AbilityCandidate {
                    name: "Mala".into(),
                    min_level: 60,
                    spell_id: 1717,
                }],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "UtilitySummon".into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Call of the Arch Mage".into(),
                        min_level: 65,
                        spell_id: 5990,
                    },
                    AbilityCandidate {
                        name: "Call of the Hero".into(),
                        min_level: 52,
                        spell_id: 1719,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: "PetSummon".into(),
                candidates: vec![AbilityCandidate {
                    name: "Greater Vocaration: Water".into(),
                    min_level: 60,
                    spell_id: 1718,
                }],
                min_expansion: EQExpansion::Classic,
            },
        ]
    }

    fn fallback_select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        let mana_pct = ctx.player.mana_pct();
        ctx.config
            .spells
            .iter()
            .filter(|spell| mana_pct >= spell.min_mana_pct)
            .max_by_key(|spell| spell.priority)
            .cloned()
    }

    fn resolved_spell_entry(
        ctx: &CombatContext,
        state: &MagicianRuntimeState,
        set_name: &str,
        min_mana_pct: f32,
        is_aoe: bool,
    ) -> Option<SpellEntry> {
        let resolved = state.resolved_abilities.get(set_name)?;
        let slot = ctx
            .config
            .spells
            .iter()
            .find(|spell| {
                spell.spell_id == resolved.spell_id
                    || spell.name.eq_ignore_ascii_case(&resolved.ability_name)
            })
            .map(|spell| spell.slot)
            .unwrap_or(0);
        Some(SpellEntry {
            name: resolved.ability_name.clone(),
            slot,
            spell_id: resolved.spell_id,
            priority: 0,
            min_mana_pct,
            is_aoe,
        })
    }

    fn update_resolved_abilities(&self, resolved: HashMap<String, ResolvedAbility>) {
        let mut runtime = self.runtime.lock().unwrap_or_else(PoisonError::into_inner);
        runtime.resolved_abilities = resolved;
        runtime.debuffed_targets.clear();
        runtime.buffed_pet_id = None;
        runtime.pending_cast = None;
    }

    fn handle_cast_outcome(&self, result: CastResult) {
        let mut runtime = self.runtime.lock().unwrap_or_else(PoisonError::into_inner);
        let pending_cast = runtime.pending_cast.take();

        if !matches!(result, CastResult::Success) {
            return;
        }

        match pending_cast {
            Some(PendingCast::Debuff { target_id }) => {
                runtime.debuffed_targets.insert(target_id);
            }
            Some(PendingCast::PetBuff { pet_id }) => {
                runtime.buffed_pet_id = Some(pet_id);
            }
            None => {}
        }
    }

    #[cfg(test)]
    fn on_abilities_resolved_for_tests(&self, resolved: HashMap<String, ResolvedAbility>) {
        self.update_resolved_abilities(resolved);
    }

    #[cfg(test)]
    fn on_cast_outcome_for_tests(&self, _ctx: &CombatContext, result: CastResult) {
        self.handle_cast_outcome(result);
    }
}

impl ClassStrategy for MagicianStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        ctx.target.map(|target| target.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        if ctx.target_is_mezzed {
            return None;
        }

        let Some(target) = ctx.target else {
            return self.fallback_select_spell(ctx);
        };
        let mana_pct = ctx.player.mana_pct();

        let mut runtime = self.runtime.lock().unwrap_or_else(PoisonError::into_inner);
        if runtime.resolved_abilities.is_empty() {
            drop(runtime);
            return self.fallback_select_spell(ctx);
        }

        if !runtime.debuffed_targets.contains(&target.spawn_id)
            && mana_pct >= DEBUFF_MIN_MANA
            && target.hp_pct() >= DEBUFF_MIN_TARGET_HP
            && let Some(spell) =
                Self::resolved_spell_entry(ctx, &runtime, "Debuff", DEBUFF_MIN_MANA, false)
        {
            runtime.pending_cast = Some(PendingCast::Debuff {
                target_id: target.spawn_id,
            });
            return Some(spell);
        }

        runtime.pending_cast = None;

        if ctx.nearby_enemies.len() as u8 >= self.aoe_threshold()
            && mana_pct >= AOE_NUKE_MIN_MANA
            && let Some(spell) =
                Self::resolved_spell_entry(ctx, &runtime, "AoeNuke", AOE_NUKE_MIN_MANA, true)
        {
            return Some(spell);
        }

        if mana_pct >= PRIMARY_NUKE_MIN_MANA
            && let Some(spell) = Self::resolved_spell_entry(
                ctx,
                &runtime,
                "PrimaryNuke",
                PRIMARY_NUKE_MIN_MANA,
                false,
            )
        {
            return Some(spell);
        }

        if mana_pct >= SECONDARY_NUKE_MIN_MANA
            && let Some(spell) = Self::resolved_spell_entry(
                ctx,
                &runtime,
                "SecondaryNuke",
                SECONDARY_NUKE_MIN_MANA,
                false,
            )
        {
            return Some(spell);
        }

        None
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        true
    }

    fn pet_action(&self, ctx: &CombatContext) -> Option<PetAction> {
        if ctx.in_combat {
            return strategy::pet_attack_action(ctx);
        }

        let pet_id = ctx.pet_spawn_id()?;
        let mana_pct = ctx.player.mana_pct();
        if mana_pct < PET_BUFF_MIN_MANA {
            return None;
        }

        let mut runtime = self.runtime.lock().unwrap_or_else(PoisonError::into_inner);
        if runtime.buffed_pet_id != Some(pet_id) {
            runtime.buffed_pet_id = None;
        }
        if runtime.buffed_pet_id == Some(pet_id) {
            return None;
        }

        let spell = Self::resolved_spell_entry(ctx, &runtime, "PetBuff", PET_BUFF_MIN_MANA, false)?;
        runtime.pending_cast = Some(PendingCast::PetBuff { pet_id });
        Some(PetAction::Buff { spell })
    }

    fn on_cast_outcome(&mut self, _ctx: &CombatContext, _gem: u8, result: CastResult) {
        self.handle_cast_outcome(result);
    }

    fn on_abilities_resolved(&mut self, resolved: &HashMap<String, ResolvedAbility>) {
        self.update_resolved_abilities(resolved.clone());
    }

    fn on_action_complete(&mut self, ctx: &CombatContext) {
        if !ctx.in_combat {
            let mut runtime = self.runtime.lock().unwrap_or_else(PoisonError::into_inner);
            runtime.debuffed_targets.clear();
            runtime.pending_cast = None;
        }
    }

    fn aoe_threshold(&self) -> u8 {
        3
    }

    fn role(&self) -> CombatRole {
        CombatRole::DpsRanged
    }

    fn rotation_groups(&self) -> Option<Vec<RotationGroup>> {
        let mut downtime = rotation::group(
            "DowntimeUtility",
            TargetSelector::SelfOnly,
            CombatStateReq::Downtime,
        );
        downtime.entries.push(rotation::entry_if(
            "RodOfMysticalTransvergence",
            ActionType::Item("Rod of Mystical Transvergence".into()),
            ConditionExpr::ManaBelow(MOD_ROD_MANA_THRESHOLD),
        ));
        Some(vec![downtime])
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
        combat::{CombatConfig, KnownAbility, SpellEntry},
        types::SpawnData,
    };

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
            positional: None,
        }
    }

    fn known_magician_spells() -> Vec<KnownAbility> {
        vec![
            KnownAbility {
                name: "Burnout".into(),
                spell_id: 1033,
                level: 16,
            },
            KnownAbility {
                name: "Greater Burnout".into(),
                spell_id: 1748,
                level: 44,
            },
            KnownAbility {
                name: "Burnout IV".into(),
                spell_id: 5987,
                level: 61,
            },
            KnownAbility {
                name: "Burnout V".into(),
                spell_id: 5988,
                level: 62,
            },
            KnownAbility {
                name: "Shock of Swords".into(),
                spell_id: 1713,
                level: 44,
            },
            KnownAbility {
                name: "Seeking Flame of Seukor".into(),
                spell_id: 1715,
                level: 59,
            },
            KnownAbility {
                name: "Shock of Steel".into(),
                spell_id: 1716,
                level: 60,
            },
            KnownAbility {
                name: "Sun Storm".into(),
                spell_id: 5989,
                level: 62,
            },
            KnownAbility {
                name: "Mala".into(),
                spell_id: 1717,
                level: 60,
            },
            KnownAbility {
                name: "Greater Vocaration: Water".into(),
                spell_id: 1718,
                level: 60,
            },
            KnownAbility {
                name: "Call of the Hero".into(),
                spell_id: 1719,
                level: 52,
            },
            KnownAbility {
                name: "Call of the Arch Mage".into(),
                spell_id: 5990,
                level: 65,
            },
        ]
    }

    #[test]
    fn mage_class_id() {
        let mage = MagicianStrategy::new(13);
        assert_eq!(mage.class_id(), 13);
    }

    #[test]
    fn mage_role_is_ranged_dps() {
        let mage = MagicianStrategy::new(13);
        assert_eq!(mage.role(), CombatRole::DpsRanged);
    }

    #[test]
    fn mage_aoe_threshold() {
        let mage = MagicianStrategy::new(13);
        assert_eq!(mage.aoe_threshold(), 3);
    }

    #[test]
    fn mage_should_assist() {
        let mage = MagicianStrategy::new(13);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config);
        assert!(mage.should_assist(&ctx));
    }

    #[test]
    fn select_target_returns_target_id() {
        let mage = MagicianStrategy::new(13);
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 88,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &config);
        assert_eq!(mage.select_target(&ctx), Some(88));
    }

    #[test]
    fn select_target_none_without_target() {
        let mage = MagicianStrategy::new(13);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config);
        assert!(mage.select_target(&ctx).is_none());
    }

    #[test]
    fn select_spell_filters_by_mana() {
        let mage = MagicianStrategy::new(13);
        let mut player = SpawnData::default();
        player.mana_current = 3000;
        player.mana_max = 10000;
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "BoltOfFire".into(),
                    slot: 1,
                    spell_id: 1,
                    priority: 5,
                    min_mana_pct: 10.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "ManaBlaze".into(),
                    slot: 2,
                    spell_id: 2,
                    priority: 15,
                    min_mana_pct: 50.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx(&player, None, &config);
        let spell = mage.select_spell(&ctx).expect("fallback spell");
        assert_eq!(spell.name, "BoltOfFire");
    }

    #[test]
    fn select_spell_none_when_empty() {
        let mage = MagicianStrategy::new(13);
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx(&player, None, &config);
        assert!(mage.select_spell(&ctx).is_none());
    }

    #[test]
    fn magician_has_rotation_groups_for_downtime_utility() {
        let mage = MagicianStrategy::new(13);
        let groups = mage
            .rotation_groups()
            .expect("Magician should expose utility rotation groups");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "DowntimeUtility");
        assert_eq!(groups[0].entries[0].name, "RodOfMysticalTransvergence");
    }

    #[test]
    fn magician_has_expected_ability_sets() {
        let mage = MagicianStrategy::new(13);
        let sets = mage.ability_sets();
        let names: Vec<&str> = sets.iter().map(|set| set.name.as_str()).collect();
        assert!(names.contains(&"PetBuff"));
        assert!(names.contains(&"PrimaryNuke"));
        assert!(names.contains(&"SecondaryNuke"));
        assert!(names.contains(&"AoeNuke"));
        assert!(names.contains(&"Debuff"));
        assert!(names.contains(&"UtilitySummon"));
    }

    #[test]
    fn magician_resolves_expected_spell_lines_by_level() {
        let mage = MagicianStrategy::new(13);
        let known = known_magician_spells();
        let sets = mage.ability_sets();

        let resolved_60 = textquest_common::combat::resolve_abilities(&sets, &known, 60);
        assert_eq!(
            resolved_60
                .get("PrimaryNuke")
                .expect("60 primary nuke")
                .ability_name,
            "Seeking Flame of Seukor"
        );
        assert_eq!(
            resolved_60.get("Debuff").expect("60 debuff").ability_name,
            "Mala"
        );

        let resolved_61 = textquest_common::combat::resolve_abilities(&sets, &known, 61);
        assert_eq!(
            resolved_61
                .get("PetBuff")
                .expect("61 pet buff")
                .ability_name,
            "Burnout IV"
        );

        let resolved_62 = textquest_common::combat::resolve_abilities(&sets, &known, 62);
        assert_eq!(
            resolved_62
                .get("PetBuff")
                .expect("62 pet buff")
                .ability_name,
            "Burnout V"
        );
        assert_eq!(
            resolved_62
                .get("AoeNuke")
                .expect("62 aoe nuke")
                .ability_name,
            "Sun Storm"
        );

        let resolved_65 = textquest_common::combat::resolve_abilities(&sets, &known, 65);
        assert_eq!(
            resolved_65
                .get("UtilitySummon")
                .expect("65 utility summon")
                .ability_name,
            "Call of the Arch Mage"
        );
    }

    #[test]
    fn magician_prioritizes_malo_then_primary_then_secondary_by_mana() {
        let mut player = SpawnData::default();
        player.level = 60;
        player.mana_current = 8500;
        player.mana_max = 10000;
        let target = SpawnData {
            spawn_id: 44,
            hp_current: 9900,
            hp_max: 10000,
            ..SpawnData::default()
        };

        let mage = MagicianStrategy::new(13);
        mage.on_abilities_resolved_for_tests(textquest_common::combat::resolve_abilities(
            &mage.ability_sets(),
            &known_magician_spells(),
            60,
        ));

        let config = CombatConfig::default();
        let ctx = make_ctx(&player, Some(&target), &config);
        let first = mage.select_spell(&ctx).expect("opening debuff");
        assert_eq!(first.name, "Mala");

        mage.on_cast_outcome_for_tests(&ctx, textquest_common::combat::CastResult::Success);
        let second = mage.select_spell(&ctx).expect("primary nuke");
        assert_eq!(second.name, "Seeking Flame of Seukor");

        player.mana_current = 4500;
        let ctx = make_ctx(&player, Some(&target), &config);
        let fallback = mage.select_spell(&ctx).expect("secondary nuke");
        assert_eq!(fallback.name, "Shock of Steel");
    }

    #[test]
    fn magician_remembers_debuffed_targets_across_assist_swaps() {
        let mut player = SpawnData::default();
        player.level = 60;
        player.mana_current = 8500;
        player.mana_max = 10000;
        let first_target = SpawnData {
            spawn_id: 44,
            hp_current: 9900,
            hp_max: 10000,
            ..SpawnData::default()
        };
        let second_target = SpawnData {
            spawn_id: 55,
            hp_current: 9800,
            hp_max: 10000,
            ..SpawnData::default()
        };

        let mage = MagicianStrategy::new(13);
        mage.on_abilities_resolved_for_tests(textquest_common::combat::resolve_abilities(
            &mage.ability_sets(),
            &known_magician_spells(),
            60,
        ));

        let config = CombatConfig::default();
        let first_ctx = make_ctx(&player, Some(&first_target), &config);
        let second_ctx = make_ctx(&player, Some(&second_target), &config);

        let first_debuff = mage.select_spell(&first_ctx).expect("first target debuff");
        assert_eq!(first_debuff.name, "Mala");
        mage.on_cast_outcome_for_tests(&first_ctx, textquest_common::combat::CastResult::Success);

        let second_debuff = mage
            .select_spell(&second_ctx)
            .expect("second target debuff");
        assert_eq!(second_debuff.name, "Mala");
        mage.on_cast_outcome_for_tests(&second_ctx, textquest_common::combat::CastResult::Success);

        let revisited_target = mage
            .select_spell(&first_ctx)
            .expect("returning to first target should nuke");
        assert_eq!(revisited_target.name, "Seeking Flame of Seukor");
    }

    #[test]
    fn magician_clears_debuff_tracking_after_disengage() {
        let mut player = SpawnData::default();
        player.level = 60;
        player.mana_current = 8500;
        player.mana_max = 10000;
        let target = SpawnData {
            spawn_id: 44,
            hp_current: 9900,
            hp_max: 10000,
            ..SpawnData::default()
        };

        let mut mage = MagicianStrategy::new(13);
        mage.on_abilities_resolved_for_tests(textquest_common::combat::resolve_abilities(
            &mage.ability_sets(),
            &known_magician_spells(),
            60,
        ));

        let config = CombatConfig::default();
        let target_ctx = make_ctx(&player, Some(&target), &config);

        let first_debuff = mage.select_spell(&target_ctx).expect("opening debuff");
        assert_eq!(first_debuff.name, "Mala");
        mage.on_cast_outcome_for_tests(&target_ctx, textquest_common::combat::CastResult::Success);

        let idle_ctx = make_ctx(&player, None, &config);
        mage.on_action_complete(&idle_ctx);

        let recast = mage
            .select_spell(&target_ctx)
            .expect("disengage should clear debuff tracking");
        assert_eq!(recast.name, "Mala");
    }

    #[test]
    fn magician_buffs_new_pet_out_of_combat_once() {
        let mut player = SpawnData::default();
        player.level = 62;
        player.mana_current = 9000;
        player.mana_max = 10000;
        let mage = MagicianStrategy::new(13);
        mage.on_abilities_resolved_for_tests(textquest_common::combat::resolve_abilities(
            &mage.ability_sets(),
            &known_magician_spells(),
            62,
        ));

        let xtargets = textquest_common::combat::ExtendedTargetList {
            slots: vec![textquest_common::combat::ExtendedTargetSlot {
                slot_type: textquest_common::combat::XTargetType::MyPet,
                status: textquest_common::combat::XTargetSlotStatus::CurrentZone,
                spawn_id: 77,
                name: "Water pet".into(),
                aggro_pct: 0,
            }],
            auto_add_haters: false,
        };
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &CombatConfig::default(),
            tick: 200,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: Some(&xtargets),
            positional: None,
        };

        let first = mage.pet_action(&ctx);
        assert!(matches!(
            first,
            Some(PetAction::Buff { ref spell }) if spell.name == "Burnout V"
        ));
        mage.on_cast_outcome_for_tests(&ctx, textquest_common::combat::CastResult::Success);
        assert_eq!(mage.pet_action(&ctx), None);
    }

    #[test]
    fn magician_uses_configured_slots_for_resolved_combat_spells() {
        let mut player = SpawnData::default();
        player.level = 60;
        player.mana_current = 9000;
        player.mana_max = 10000;
        let target = SpawnData {
            spawn_id: 55,
            hp_current: 9900,
            hp_max: 10000,
            ..SpawnData::default()
        };

        let mage = MagicianStrategy::new(13);
        mage.on_abilities_resolved_for_tests(textquest_common::combat::resolve_abilities(
            &mage.ability_sets(),
            &known_magician_spells(),
            60,
        ));

        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "Mala".into(),
                    slot: 5,
                    spell_id: 1717,
                    priority: 1,
                    min_mana_pct: 70.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Seeking Flame of Seukor".into(),
                    slot: 4,
                    spell_id: 1715,
                    priority: 2,
                    min_mana_pct: 55.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "Shock of Steel".into(),
                    slot: 3,
                    spell_id: 1716,
                    priority: 3,
                    min_mana_pct: 40.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };

        let ctx = make_ctx(&player, Some(&target), &config);
        let debuff = mage.select_spell(&ctx).expect("opening debuff");
        assert_eq!(debuff.name, "Mala");
        assert_eq!(debuff.slot, 5);

        mage.on_cast_outcome_for_tests(&ctx, textquest_common::combat::CastResult::Success);
        let nuke = mage.select_spell(&ctx).expect("primary nuke");
        assert_eq!(nuke.name, "Seeking Flame of Seukor");
        assert_eq!(nuke.slot, 4);
    }
}
