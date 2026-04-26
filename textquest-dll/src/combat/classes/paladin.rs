use std::collections::HashMap;

use textquest_common::combat::{
    AbilityCandidate, AbilitySet, CombatRole, EQExpansion, ResolvedAbility, SpellEntry,
};

use crate::combat::strategy::{self, ClassStrategy, CombatContext};

const MAIN_HEAL_SET: &str = "MainHeal";
const GROUP_HEAL_SET: &str = "GroupHeal";
const CURE_SET: &str = "Cure";
const STUN_SET: &str = "Stun";
const NUKE_SET: &str = "Nuke";
const HP_BUFF_SET: &str = "HpBuff";
const SELF_BUFF_SET: &str = "SelfBuff";

const MAIN_HEAL_ALIASES: &[&str] = &[
    "Light of Nife",
    "Touch of Nife",
    "Superior Healing",
    "Healing",
    "Light Healing",
    "Minor Healing",
];
const GROUP_HEAL_ALIASES: &[&str] = &[
    "Wave of Marr",
    "Healing Wave of Prexus",
    "Wave of Healing",
    "Wave of Life",
];
const CURE_ALIASES: &[&str] = &[
    "Supernal Cleansing",
    "Crusader`s Touch",
    "Crusader's Touch",
    "Celestial Cleansing",
    "Counteract Disease",
    "Cure Disease",
    "Cure Poison",
];
const STUN_ALIASES: &[&str] = &[
    "Quellious' Word of Serenity",
    "Force of Akilae",
    "Force",
    "Stun",
];
const NUKE_ALIASES: &[&str] = &[
    "Pious Might",
    "Holy Might",
    "Hammer of Striking",
    "Hammer of Wrath",
];
const HP_BUFF_ALIASES: &[&str] = &[
    "Brell's Stalwart Shield",
    "Brell's Mountainous Barrier",
    "Shield of Words",
    "Valor of Marr",
];
const SELF_BUFF_ALIASES: &[&str] = &["Yaulp IV", "Yaulp III", "Yaulp II", "Yaulp"];

const PALADIN_EMERGENCY_HEAL_HP_PCT: f32 = 35.0;
const PALADIN_GROUP_HEAL_HP_PCT: f32 = 55.0;
const PALADIN_GROUP_HEAL_MEMBER_COUNT: usize = 2;
const PALADIN_SUPPORT_HEAL_HP_PCT: f32 = 60.0;
const PALADIN_CURE_SAFE_HP_PCT: f32 = 55.0;
const PALADIN_CURE_MIN_MANA_PCT: f32 = 15.0;
const PALADIN_STUN_MIN_MANA_PCT: f32 = 22.0;
const PALADIN_DPS_MIN_MANA_PCT: f32 = 40.0;
const PALADIN_BUFF_MIN_MANA_PCT: f32 = 30.0;
const PALADIN_HEAL_CANCEL_THRESHOLD: f32 = 82.0;

/// Paladin strategy: Live-safe hybrid tank that preserves mana and endurance
/// for pickup control, off-heals, cures, and controlled DPS.
///
/// Priority order:
/// 1. Emergency single-target heal (<35% HP)
/// 2. Group heal (2+ members <=55% HP)
/// 3. Cure when the group is stable (>55% HP floor)
/// 4. Support heal (<=60% HP)
/// 5. Stun (aggro / interrupt) when mana is healthy
/// 6. Holy nuke when no support action is needed
/// 7. Downtime self-buffs (Yaulp / HP buff)
pub struct PaladinStrategy {
    class_id: u8,
    resolved_abilities: HashMap<String, ResolvedAbility>,
}

impl PaladinStrategy {
    pub fn new(class_id: u8) -> Self {
        Self {
            class_id,
            resolved_abilities: HashMap::new(),
        }
    }

    fn build_ability_sets() -> Vec<AbilitySet> {
        vec![
            AbilitySet {
                name: MAIN_HEAL_SET.into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Light of Nife".into(),
                        min_level: 63,
                        spell_id: 3430,
                    },
                    AbilityCandidate {
                        name: "Touch of Nife".into(),
                        min_level: 61,
                        spell_id: 3429,
                    },
                    AbilityCandidate {
                        name: "Superior Healing".into(),
                        min_level: 57,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Healing".into(),
                        min_level: 30,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Light Healing".into(),
                        min_level: 15,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: GROUP_HEAL_SET.into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Wave of Marr".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Healing Wave of Prexus".into(),
                        min_level: 58,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Wave of Healing".into(),
                        min_level: 55,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Wave of Life".into(),
                        min_level: 39,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: CURE_SET.into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Supernal Cleansing".into(),
                        min_level: 64,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Crusader's Touch".into(),
                        min_level: 62,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Celestial Cleansing".into(),
                        min_level: 59,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Counteract Disease".into(),
                        min_level: 56,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Cure Disease".into(),
                        min_level: 15,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: STUN_SET.into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Quellious' Word of Serenity".into(),
                        min_level: 64,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Force of Akilae".into(),
                        min_level: 62,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Force".into(),
                        min_level: 52,
                        spell_id: 124,
                    },
                    AbilityCandidate {
                        name: "Stun".into(),
                        min_level: 5,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: NUKE_SET.into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Pious Might".into(),
                        min_level: 63,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Holy Might".into(),
                        min_level: 49,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Hammer of Striking".into(),
                        min_level: 30,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Hammer of Wrath".into(),
                        min_level: 15,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: HP_BUFF_SET.into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Brell's Stalwart Shield".into(),
                        min_level: 65,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Brell's Mountainous Barrier".into(),
                        min_level: 60,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Shield of Words".into(),
                        min_level: 60,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Valor of Marr".into(),
                        min_level: 49,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
            AbilitySet {
                name: SELF_BUFF_SET.into(),
                candidates: vec![
                    AbilityCandidate {
                        name: "Yaulp IV".into(),
                        min_level: 60,
                        spell_id: 1534,
                    },
                    AbilityCandidate {
                        name: "Yaulp III".into(),
                        min_level: 56,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Yaulp II".into(),
                        min_level: 39,
                        spell_id: -1,
                    },
                    AbilityCandidate {
                        name: "Yaulp".into(),
                        min_level: 9,
                        spell_id: -1,
                    },
                ],
                min_expansion: EQExpansion::Classic,
            },
        ]
    }

    fn mana_allows(ctx: &CombatContext, spell: &SpellEntry, required_mana_pct: f32) -> bool {
        ctx.player.mana_pct() >= spell.min_mana_pct.max(required_mana_pct)
    }

    fn matches_alias(spell_name: &str, aliases: &[&str]) -> bool {
        aliases
            .iter()
            .any(|alias| spell_name.eq_ignore_ascii_case(alias))
    }

    fn spell_from_resolved_set(
        &self,
        ctx: &CombatContext,
        set_name: &str,
        required_mana_pct: f32,
    ) -> Option<SpellEntry> {
        let resolved = self.resolved_abilities.get(set_name)?;
        ctx.config
            .spells
            .iter()
            .find(|spell| {
                (spell.spell_id > 0 && spell.spell_id == resolved.spell_id)
                    || spell.name.eq_ignore_ascii_case(&resolved.ability_name)
            })
            .filter(|spell| Self::mana_allows(ctx, spell, required_mana_pct))
            .cloned()
    }

    fn spell_from_aliases(
        &self,
        ctx: &CombatContext,
        aliases: &[&str],
        required_mana_pct: f32,
    ) -> Option<SpellEntry> {
        ctx.config
            .spells
            .iter()
            .filter(|spell| Self::matches_alias(&spell.name, aliases))
            .filter(|spell| Self::mana_allows(ctx, spell, required_mana_pct))
            .max_by_key(|spell| spell.priority)
            .cloned()
    }

    fn spell_for_set_or_aliases(
        &self,
        ctx: &CombatContext,
        set_name: &str,
        aliases: &[&str],
        required_mana_pct: f32,
    ) -> Option<SpellEntry> {
        self.spell_from_resolved_set(ctx, set_name, required_mana_pct)
            .or_else(|| self.spell_from_aliases(ctx, aliases, required_mana_pct))
    }

    fn lowest_hp_member(&self, ctx: &CombatContext) -> Option<(u32, f32)> {
        strategy::lowest_hp_member(ctx)
    }

    fn afflicted_member(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::prioritized_afflicted_member(ctx).map(|(spawn_id, _)| spawn_id)
    }

    fn low_hp_member_count(&self, ctx: &CombatContext, hp_threshold: f32) -> usize {
        ctx.group_members
            .iter()
            .filter(|member| {
                !member.is_dead && member.hp_pct > 0.0 && member.hp_pct <= hp_threshold
            })
            .count()
    }

    fn group_is_stable_for_cure(&self, ctx: &CombatContext) -> bool {
        self.lowest_hp_member(ctx)
            .is_none_or(|(_, hp_pct)| hp_pct > PALADIN_CURE_SAFE_HP_PCT)
    }

    fn should_use_emergency_heal(&self, ctx: &CombatContext) -> bool {
        self.lowest_hp_member(ctx)
            .is_some_and(|(_, hp_pct)| hp_pct <= PALADIN_EMERGENCY_HEAL_HP_PCT)
    }

    fn should_use_group_heal(&self, ctx: &CombatContext) -> bool {
        self.low_hp_member_count(ctx, PALADIN_GROUP_HEAL_HP_PCT) >= PALADIN_GROUP_HEAL_MEMBER_COUNT
            && self.find_group_heal_spell(ctx).is_some()
    }

    fn should_use_support_heal(&self, ctx: &CombatContext) -> bool {
        self.lowest_hp_member(ctx)
            .is_some_and(|(_, hp_pct)| hp_pct <= PALADIN_SUPPORT_HEAL_HP_PCT)
    }

    fn find_main_heal_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        self.spell_for_set_or_aliases(ctx, MAIN_HEAL_SET, MAIN_HEAL_ALIASES, 0.0)
    }

    fn find_group_heal_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        self.spell_for_set_or_aliases(ctx, GROUP_HEAL_SET, GROUP_HEAL_ALIASES, 0.0)
    }

    fn find_cure_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        self.spell_for_set_or_aliases(ctx, CURE_SET, CURE_ALIASES, PALADIN_CURE_MIN_MANA_PCT)
    }

    fn cure_target(&self, ctx: &CombatContext) -> Option<u32> {
        if strategy::afflicted_member_count(ctx) > 1
            && self
                .find_cure_spell(ctx)
                .is_some_and(|spell| strategy::is_group_cure_spell(&spell))
        {
            return Some(ctx.player.spawn_id);
        }
        self.afflicted_member(ctx)
    }

    fn find_stun_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        self.spell_for_set_or_aliases(ctx, STUN_SET, STUN_ALIASES, PALADIN_STUN_MIN_MANA_PCT)
    }

    fn find_nuke_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        self.spell_for_set_or_aliases(ctx, NUKE_SET, NUKE_ALIASES, PALADIN_DPS_MIN_MANA_PCT)
    }

    fn has_active_buff(&self, ctx: &CombatContext, spell: &SpellEntry) -> bool {
        spell.spell_id > 0 && ctx.active_buffs.contains(&spell.spell_id)
    }

    fn find_buff_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        if ctx.in_combat {
            return None;
        }

        let self_buff = self.spell_for_set_or_aliases(
            ctx,
            SELF_BUFF_SET,
            SELF_BUFF_ALIASES,
            PALADIN_BUFF_MIN_MANA_PCT,
        );
        if let Some(spell) = self_buff
            && !self.has_active_buff(ctx, &spell)
        {
            return Some(spell);
        }

        let hp_buff = self.spell_for_set_or_aliases(
            ctx,
            HP_BUFF_SET,
            HP_BUFF_ALIASES,
            PALADIN_BUFF_MIN_MANA_PCT,
        );
        hp_buff.filter(|spell| !self.has_active_buff(ctx, spell))
    }
}

impl ClassStrategy for PaladinStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        if self.should_use_emergency_heal(ctx) {
            return self.lowest_hp_member(ctx).map(|(spawn_id, _)| spawn_id);
        }

        if self.should_use_group_heal(ctx) {
            return Some(ctx.player.spawn_id);
        }

        if self.group_is_stable_for_cure(ctx)
            && self.find_cure_spell(ctx).is_some()
            && let Some(target_id) = self.cure_target(ctx)
        {
            return Some(target_id);
        }

        if self.should_use_support_heal(ctx) {
            return self.lowest_hp_member(ctx).map(|(spawn_id, _)| spawn_id);
        }

        if self.find_buff_spell(ctx).is_some() {
            return Some(ctx.player.spawn_id);
        }

        strategy::assist_target(ctx)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        if self.should_use_emergency_heal(ctx) {
            return self.find_main_heal_spell(ctx);
        }

        if self.should_use_group_heal(ctx) {
            return self.find_group_heal_spell(ctx);
        }

        if self.group_is_stable_for_cure(ctx) && self.cure_target(ctx).is_some() {
            if let Some(cure) = self.find_cure_spell(ctx) {
                return Some(cure);
            }
        }

        if self.should_use_support_heal(ctx) {
            if let Some(heal) = self.find_main_heal_spell(ctx) {
                return Some(heal);
            }
        }

        if ctx.in_combat && ctx.target.is_some() {
            if let Some(stun) = self.find_stun_spell(ctx) {
                return Some(stun);
            }
            if let Some(nuke) = self.find_nuke_spell(ctx) {
                return Some(nuke);
            }
        }

        self.find_buff_spell(ctx)
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        false
    }

    fn on_engage(&mut self, ctx: &CombatContext) {
        strategy::melee_on_engage(ctx, "Paladin");
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

    fn ability_sets(&self) -> Vec<AbilitySet> {
        Self::build_ability_sets()
    }

    fn on_abilities_resolved(&mut self, resolved: &HashMap<String, ResolvedAbility>) {
        self.resolved_abilities = resolved.clone();
    }

    fn heal_cancel_threshold(&self) -> Option<f32> {
        Some(PALADIN_HEAL_CANCEL_THRESHOLD)
    }

    fn is_heal_cast(&self, ctx: &CombatContext, spell_slot: u8, spell_id: i32) -> bool {
        [
            self.spell_for_set_or_aliases(ctx, MAIN_HEAL_SET, MAIN_HEAL_ALIASES, 0.0),
            self.spell_for_set_or_aliases(ctx, GROUP_HEAL_SET, GROUP_HEAL_ALIASES, 0.0),
        ]
        .into_iter()
        .flatten()
        .any(|spell| spell.slot == spell_slot || spell.spell_id == spell_id)
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use std::{collections::BTreeSet, path::PathBuf};

    use serde::Deserialize;
    use textquest_common::{
        combat::{CombatConfig, KnownAbility, resolve_abilities},
        types::SpawnData,
    };

    use super::*;
    use crate::combat::strategy::GroupMemberState;

    fn crate_manifest_dir() -> PathBuf {
        let mut dir = std::env::current_dir().expect("test working directory should be readable");

        loop {
            if dir.join("Cargo.toml").exists()
                && dir.file_name().and_then(|name| name.to_str()) == Some("textquest-dll")
            {
                return dir;
            }

            let nested = dir.join("textquest-dll");
            if nested.join("Cargo.toml").exists() {
                return nested;
            }

            if !dir.pop() {
                panic!("could not locate textquest-dll crate root from test working directory");
            }
        }
    }

    fn class_config_path(file_name: &str) -> PathBuf {
        crate_manifest_dir()
            .parent()
            .expect("textquest-dll crate should have a workspace parent")
            .join("config")
            .join("classes")
            .join(file_name)
    }

    fn make_ctx<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        members: &'a [GroupMemberState],
        config: &'a CombatConfig,
        active_buffs: &'a [i32],
        in_combat: bool,
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: &[],
            group_members: members,
            config,
            tick: 0,
            in_combat,
            ch_chain_slot: None,
            active_buffs,
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            burn_state: textquest_common::combat::BurnState::Ready,
            burnnow_triggered: false,
            burn_cooldown_ticks: 0,
            positional: None,
        }
    }

    fn spell(slot: u8, spell_id: i32, name: &str, priority: u8, is_aoe: bool) -> SpellEntry {
        SpellEntry {
            slot,
            spell_id,
            name: name.into(),
            min_mana_pct: 0.0,
            priority,
            is_aoe,
        }
    }

    fn test_config() -> CombatConfig {
        CombatConfig {
            role: CombatRole::OffTank,
            spells: vec![
                spell(1, 3429, "Touch of Nife", 80, false),
                spell(2, 3430, "Light of Nife", 90, false),
                spell(3, 6201, "Crusader's Touch", 70, false),
                spell(4, 6401, "Quellious' Word of Serenity", 60, false),
                spell(5, 6501, "Wave of Marr", 85, true),
                spell(6, 1534, "Yaulp IV", 40, false),
                spell(7, 6502, "Brell's Stalwart Shield", 35, false),
                spell(8, 6301, "Pious Might", 50, false),
                spell(9, 6202, "Force of Akilae", 55, false),
                spell(10, 5901, "Celestial Cleansing", 45, false),
            ],
            ..CombatConfig::default()
        }
    }

    fn test_player(mana_pct: f32) -> SpawnData {
        SpawnData {
            spawn_id: 1,
            name: "Paladin".into(),
            mana_current: (mana_pct * 100.0) as i32,
            mana_max: 10_000,
            ..SpawnData::default()
        }
    }

    fn test_target() -> SpawnData {
        SpawnData {
            spawn_id: 100,
            name: "Target".into(),
            ..SpawnData::default()
        }
    }

    fn member(spawn_id: u32, hp_pct: f32, has_detrimental: bool) -> GroupMemberState {
        GroupMemberState {
            spawn_id,
            hp_pct,
            mana_pct: 100.0,
            class_id: 3,
            is_dead: false,
            name: format!("Member{spawn_id}"),
            has_detrimental,
        }
    }

    fn seed_resolved(strategy: &mut PaladinStrategy) {
        let resolved = HashMap::from([
            (
                MAIN_HEAL_SET.into(),
                ResolvedAbility {
                    set_name: MAIN_HEAL_SET.into(),
                    ability_name: "Light of Nife".into(),
                    spell_id: 3430,
                    min_level: 63,
                },
            ),
            (
                GROUP_HEAL_SET.into(),
                ResolvedAbility {
                    set_name: GROUP_HEAL_SET.into(),
                    ability_name: "Wave of Marr".into(),
                    spell_id: 6501,
                    min_level: 65,
                },
            ),
            (
                CURE_SET.into(),
                ResolvedAbility {
                    set_name: CURE_SET.into(),
                    ability_name: "Crusader's Touch".into(),
                    spell_id: 6201,
                    min_level: 62,
                },
            ),
            (
                STUN_SET.into(),
                ResolvedAbility {
                    set_name: STUN_SET.into(),
                    ability_name: "Quellious' Word of Serenity".into(),
                    spell_id: 6401,
                    min_level: 64,
                },
            ),
            (
                NUKE_SET.into(),
                ResolvedAbility {
                    set_name: NUKE_SET.into(),
                    ability_name: "Pious Might".into(),
                    spell_id: 6301,
                    min_level: 63,
                },
            ),
            (
                SELF_BUFF_SET.into(),
                ResolvedAbility {
                    set_name: SELF_BUFF_SET.into(),
                    ability_name: "Yaulp IV".into(),
                    spell_id: 1534,
                    min_level: 60,
                },
            ),
            (
                HP_BUFF_SET.into(),
                ResolvedAbility {
                    set_name: HP_BUFF_SET.into(),
                    ability_name: "Brell's Stalwart Shield".into(),
                    spell_id: 6502,
                    min_level: 65,
                },
            ),
        ]);
        strategy.on_abilities_resolved(&resolved);
    }

    #[test]
    fn paladin_resolves_level_breakpoints() {
        let known = vec![
            KnownAbility {
                name: "Superior Healing".into(),
                spell_id: 5701,
                level: 57,
            },
            KnownAbility {
                name: "Touch of Nife".into(),
                spell_id: 3429,
                level: 61,
            },
            KnownAbility {
                name: "Crusader's Touch".into(),
                spell_id: 6201,
                level: 62,
            },
            KnownAbility {
                name: "Light of Nife".into(),
                spell_id: 3430,
                level: 63,
            },
            KnownAbility {
                name: "Quellious' Word of Serenity".into(),
                spell_id: 6401,
                level: 64,
            },
            KnownAbility {
                name: "Wave of Marr".into(),
                spell_id: 6501,
                level: 65,
            },
        ];

        let sets = PaladinStrategy::build_ability_sets();
        let level_60 = resolve_abilities(&sets, &known, 60);
        let level_61 = resolve_abilities(&sets, &known, 61);
        let level_62 = resolve_abilities(&sets, &known, 62);
        let level_65 = resolve_abilities(&sets, &known, 65);

        assert_eq!(
            level_60
                .get(MAIN_HEAL_SET)
                .map(|ability| ability.ability_name.as_str()),
            Some("Superior Healing")
        );
        assert_eq!(
            level_61
                .get(MAIN_HEAL_SET)
                .map(|ability| ability.ability_name.as_str()),
            Some("Touch of Nife")
        );
        assert_eq!(
            level_62
                .get(CURE_SET)
                .map(|ability| ability.ability_name.as_str()),
            Some("Crusader's Touch")
        );
        assert_eq!(
            level_65
                .get(GROUP_HEAL_SET)
                .map(|ability| ability.ability_name.as_str()),
            Some("Wave of Marr")
        );
    }

    #[test]
    fn paladin_emergency_heal_beats_cure_and_stun() {
        let mut paladin = PaladinStrategy::new(3);
        seed_resolved(&mut paladin);
        let config = test_config();
        let player = test_player(80.0);
        let target = test_target();
        let members = vec![member(10, 28.0, true), member(11, 88.0, false)];
        let ctx = make_ctx(&player, Some(&target), &members, &config, &[], true);

        assert_eq!(paladin.select_target(&ctx), Some(10));
        assert_eq!(
            paladin.select_spell(&ctx).map(|spell| spell.name),
            Some("Light of Nife".into())
        );
    }

    #[test]
    fn paladin_prefers_group_heal_when_multiple_members_are_low() {
        let mut paladin = PaladinStrategy::new(3);
        seed_resolved(&mut paladin);
        let config = test_config();
        let player = test_player(70.0);
        let target = test_target();
        let members = vec![member(10, 50.0, false), member(11, 54.0, false)];
        let ctx = make_ctx(&player, Some(&target), &members, &config, &[], true);

        assert_eq!(paladin.select_target(&ctx), Some(player.spawn_id));
        assert_eq!(
            paladin.select_spell(&ctx).map(|spell| spell.name),
            Some("Wave of Marr".into())
        );
    }

    #[test]
    fn paladin_cures_when_group_is_stable() {
        let mut paladin = PaladinStrategy::new(3);
        seed_resolved(&mut paladin);
        let config = test_config();
        let player = test_player(70.0);
        let target = test_target();
        let members = vec![member(10, 80.0, true), member(11, 95.0, false)];
        let ctx = make_ctx(&player, Some(&target), &members, &config, &[], true);

        assert_eq!(paladin.select_target(&ctx), Some(10));
        assert_eq!(
            paladin.select_spell(&ctx).map(|spell| spell.name),
            Some("Crusader's Touch".into())
        );
    }

    #[test]
    fn paladin_stuns_before_dps_when_support_is_not_needed() {
        let mut paladin = PaladinStrategy::new(3);
        seed_resolved(&mut paladin);
        let config = test_config();
        let player = test_player(65.0);
        let target = test_target();
        let members = vec![member(10, 90.0, false)];
        let ctx = make_ctx(&player, Some(&target), &members, &config, &[], true);

        assert_eq!(paladin.select_target(&ctx), Some(target.spawn_id));
        assert_eq!(
            paladin.select_spell(&ctx).map(|spell| spell.name),
            Some("Quellious' Word of Serenity".into())
        );
    }

    #[test]
    fn paladin_reserves_mana_for_support_actions() {
        let mut paladin = PaladinStrategy::new(3);
        seed_resolved(&mut paladin);
        let config = test_config();
        let player = test_player(20.0);
        let target = test_target();
        let ctx = make_ctx(&player, Some(&target), &[], &config, &[], true);

        assert_eq!(paladin.select_target(&ctx), Some(target.spawn_id));
        assert!(paladin.select_spell(&ctx).is_none());
    }

    #[test]
    fn paladin_rebuffs_self_out_of_combat_when_missing_yaulp() {
        let mut paladin = PaladinStrategy::new(3);
        seed_resolved(&mut paladin);
        let config = test_config();
        let player = test_player(60.0);
        let ctx = make_ctx(&player, None, &[], &config, &[], false);

        assert_eq!(paladin.select_target(&ctx), Some(player.spawn_id));
        assert_eq!(
            paladin.select_spell(&ctx).map(|spell| spell.name),
            Some("Yaulp IV".into())
        );
    }

    #[test]
    fn paladin_skips_self_buff_when_already_active() {
        let mut paladin = PaladinStrategy::new(3);
        seed_resolved(&mut paladin);
        let config = test_config();
        let player = test_player(60.0);
        let active_buffs = [1534];
        let ctx = make_ctx(&player, None, &[], &config, &active_buffs, false);

        assert_eq!(
            paladin.select_spell(&ctx).map(|spell| spell.name),
            Some("Brell's Stalwart Shield".into())
        );
    }

    #[test]
    fn paladin_heal_cancel_threshold_matches_live_safe_profile() {
        let paladin = PaladinStrategy::new(3);
        assert_eq!(
            paladin.heal_cancel_threshold(),
            Some(PALADIN_HEAL_CANCEL_THRESHOLD)
        );
    }

    #[test]
    fn paladin_only_targets_friend_for_cure_when_cure_is_available() {
        let strategy = PaladinStrategy::new(3);
        let player = test_player(70.0);
        let target = test_target();
        let afflicted_ally = member(10, 90.0, true);
        let config = CombatConfig {
            role: CombatRole::OffTank,
            spells: vec![spell(3, 1197, "Holy Might", 30, false)],
            ..CombatConfig::default()
        };
        let members = [afflicted_ally];
        let ctx = make_ctx(&player, Some(&target), &members, &config, &[], true);

        assert_eq!(strategy.select_target(&ctx), Some(target.spawn_id));
    }

    #[derive(Debug, Deserialize)]
    struct PaladinDocConfig {
        #[serde(default)]
        combat_abilities: Vec<DocAbility>,
        #[serde(default)]
        buff_abilities: Vec<DocAbility>,
        #[serde(default)]
        emergency_abilities: Vec<DocAbility>,
        #[serde(default)]
        cc_abilities: Vec<DocCcAbility>,
        #[serde(default)]
        level_overrides: Vec<DocLevelOverride>,
        resource_thresholds: DocThresholds,
    }

    #[derive(Debug, Deserialize)]
    struct DocThresholds {
        emergency_heal_hp_pct: f32,
        group_heal_hp_pct: f32,
        group_heal_member_count: usize,
        support_heal_hp_pct: f32,
        cure_min_mana_pct: f32,
        stun_min_mana_pct: f32,
        dps_min_mana_pct: f32,
        buff_min_mana_pct: f32,
        melee_endurance_floor_pct: f32,
        stop_cast_hp_pct: f32,
    }

    #[derive(Debug, Deserialize)]
    struct DocAbility {
        name: String,
        #[serde(default)]
        line: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct DocCcAbility {
        name: String,
        #[serde(default)]
        line: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct DocLevelOverride {
        min_level: u8,
        #[serde(default)]
        combat_abilities: Vec<DocAbility>,
        #[serde(default)]
        buff_abilities: Vec<DocAbility>,
        #[serde(default)]
        emergency_abilities: Vec<DocAbility>,
        #[serde(default)]
        cc_abilities: Vec<DocCcAbility>,
    }

    #[test]
    fn paladin_toml_matches_runtime_profile() {
        let paladin_config = class_config_path("paladin.toml");
        let doc_contents =
            std::fs::read_to_string(&paladin_config).expect("paladin.toml should be readable");
        let doc: PaladinDocConfig =
            toml::from_str(&doc_contents).expect("paladin.toml should parse");

        assert_eq!(
            doc.resource_thresholds.emergency_heal_hp_pct,
            PALADIN_EMERGENCY_HEAL_HP_PCT
        );
        assert_eq!(
            doc.resource_thresholds.group_heal_hp_pct,
            PALADIN_GROUP_HEAL_HP_PCT
        );
        assert_eq!(
            doc.resource_thresholds.group_heal_member_count,
            PALADIN_GROUP_HEAL_MEMBER_COUNT
        );
        assert_eq!(
            doc.resource_thresholds.support_heal_hp_pct,
            PALADIN_SUPPORT_HEAL_HP_PCT
        );
        assert_eq!(
            doc.resource_thresholds.cure_min_mana_pct,
            PALADIN_CURE_MIN_MANA_PCT
        );
        assert_eq!(
            doc.resource_thresholds.stun_min_mana_pct,
            PALADIN_STUN_MIN_MANA_PCT
        );
        assert_eq!(
            doc.resource_thresholds.dps_min_mana_pct,
            PALADIN_DPS_MIN_MANA_PCT
        );
        assert_eq!(
            doc.resource_thresholds.buff_min_mana_pct,
            PALADIN_BUFF_MIN_MANA_PCT
        );
        assert_eq!(doc.resource_thresholds.melee_endurance_floor_pct, 15.0);
        assert_eq!(
            doc.resource_thresholds.stop_cast_hp_pct,
            PALADIN_HEAL_CANCEL_THRESHOLD
        );

        let override_levels: BTreeSet<u8> = doc
            .level_overrides
            .iter()
            .map(|override_doc| override_doc.min_level)
            .collect();
        assert!(override_levels.contains(&60));
        assert!(override_levels.contains(&61));
        assert!(override_levels.contains(&62));
        assert!(override_levels.contains(&65));

        let configured_lines: BTreeSet<String> = doc
            .combat_abilities
            .iter()
            .chain(doc.buff_abilities.iter())
            .chain(doc.emergency_abilities.iter())
            .map(|ability| ability.line.clone().unwrap_or_else(|| ability.name.clone()))
            .chain(
                doc.cc_abilities
                    .iter()
                    .map(|ability| ability.line.clone().unwrap_or_else(|| ability.name.clone())),
            )
            .chain(doc.level_overrides.iter().flat_map(|override_doc| {
                override_doc
                    .combat_abilities
                    .iter()
                    .chain(override_doc.buff_abilities.iter())
                    .chain(override_doc.emergency_abilities.iter())
                    .map(|ability| ability.line.clone().unwrap_or_else(|| ability.name.clone()))
                    .chain(override_doc.cc_abilities.iter().map(|ability| {
                        ability.line.clone().unwrap_or_else(|| ability.name.clone())
                    }))
            }))
            .collect();

        let runtime_lines = BTreeSet::from([
            "Touch of Nife".to_string(),
            "Light of Nife".to_string(),
            "Wave of Marr".to_string(),
            "Crusader's Touch".to_string(),
            "Quellious' Word of Serenity".to_string(),
            "Pious Might".to_string(),
            "Brell's Stalwart Shield".to_string(),
            "Yaulp IV".to_string(),
        ]);

        for runtime_line in runtime_lines {
            assert!(
                configured_lines.contains(&runtime_line),
                "paladin.toml should document runtime line {runtime_line}"
            );
        }

        assert!(paladin_config.exists());
    }
}
