use std::{collections::HashMap, path::PathBuf, sync::LazyLock};

use serde::Deserialize;
use textquest_common::combat::{
    AbilityCandidate, AbilitySet, CombatRole, KnownAbility, SpellEntry,
};

use crate::combat::{
    rotation::RotationGroup,
    strategy::{self, AbilityResolution, ClassStrategy, CombatContext},
    toon_config::ToonRotationGroup,
};

const WARRIOR_CLASS_CONFIG_ENV_VAR: &str = "TEXTQUEST_CLASS_CONFIG_DIR";
const EMBEDDED_WARRIOR_CONFIG: &str = include_str!("../../../../config/classes/warrior.toml");

#[derive(Debug, Clone, Default, Deserialize)]
struct WarriorRuntimeConfig {
    #[serde(default)]
    ability_sets: Vec<WarriorAbilitySetConfig>,
    #[serde(default)]
    rotation_groups: Vec<ToonRotationGroup>,
}

#[derive(Debug, Clone, Deserialize)]
struct WarriorAbilitySetConfig {
    name: String,
    #[serde(default)]
    candidates: Vec<WarriorAbilityCandidate>,
}

#[derive(Debug, Clone, Deserialize)]
struct WarriorAbilityCandidate {
    name: String,
    min_level: u8,
    spell_id: i32,
    #[serde(default)]
    cooldown_ticks: Option<u32>,
    #[serde(default)]
    shared_cooldown_key: Option<String>,
    #[serde(default)]
    shared_cooldown_ticks: Option<u32>,
    #[serde(default)]
    requires_known: bool,
}

static WARRIOR_RUNTIME_CONFIG: LazyLock<WarriorRuntimeConfig> =
    LazyLock::new(load_warrior_runtime_config);

fn warrior_config_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(WARRIOR_CLASS_CONFIG_ENV_VAR) {
        return Some(PathBuf::from(dir).join("warrior.toml"));
    }

    std::env::current_dir()
        .ok()
        .map(|dir| dir.join("config/classes/warrior.toml"))
}

fn parse_runtime_config(contents: &str) -> Result<WarriorRuntimeConfig, toml::de::Error> {
    toml::from_str(contents)
}

fn embedded_runtime_config() -> WarriorRuntimeConfig {
    parse_runtime_config(EMBEDDED_WARRIOR_CONFIG).unwrap_or_else(|error| {
        tracing::warn!(
            error = %error,
            "Failed to parse embedded warrior runtime config; using empty defaults"
        );
        WarriorRuntimeConfig::default()
    })
}

fn load_runtime_config_from_contents(contents: &str, source: &str) -> WarriorRuntimeConfig {
    parse_runtime_config(contents).unwrap_or_else(|error| {
        tracing::warn!(
            source,
            error = %error,
            "Failed to parse warrior runtime config; using embedded defaults"
        );
        embedded_runtime_config()
    })
}

fn load_warrior_runtime_config() -> WarriorRuntimeConfig {
    if let Some(path) = warrior_config_path()
        && let Ok(contents) = std::fs::read_to_string(&path)
    {
        let source = path.display().to_string();
        return load_runtime_config_from_contents(&contents, &source);
    }

    embedded_runtime_config()
}

fn runtime_config() -> &'static WarriorRuntimeConfig {
    &WARRIOR_RUNTIME_CONFIG
}

fn resolve_candidate_spell_id(
    candidate: &WarriorAbilityCandidate,
    known: &[KnownAbility],
) -> Option<i32> {
    let matched_ability = known.iter().find(|known_ability| {
        (candidate.spell_id >= 0 && known_ability.spell_id == candidate.spell_id)
            || known_ability.name.eq_ignore_ascii_case(&candidate.name)
    });

    if candidate.requires_known || candidate.spell_id < 0 {
        matched_ability.map(|known_ability| {
            if candidate.spell_id >= 0 {
                candidate.spell_id
            } else {
                known_ability.spell_id
            }
        })
    } else if candidate.spell_id > 0 {
        Some(candidate.spell_id)
    } else {
        matched_ability.map(|known_ability| known_ability.spell_id)
    }
}

fn resolve_runtime_abilities(
    ability_sets: &[WarriorAbilitySetConfig],
    known: &[KnownAbility],
    character_level: u8,
) -> HashMap<String, AbilityResolution> {
    let mut resolved = HashMap::new();

    for ability_set in ability_sets {
        for candidate in &ability_set.candidates {
            if character_level < candidate.min_level {
                continue;
            }

            let Some(spell_id) = resolve_candidate_spell_id(candidate, known) else {
                continue;
            };

            resolved.insert(
                ability_set.name.clone(),
                AbilityResolution {
                    set_name: ability_set.name.clone(),
                    ability_name: candidate.name.clone(),
                    spell_id,
                    min_level: candidate.min_level,
                    cooldown_ticks: candidate.cooldown_ticks,
                    shared_cooldown_key: candidate.shared_cooldown_key.clone(),
                    shared_cooldown_ticks: candidate.shared_cooldown_ticks,
                },
            );
            break;
        }
    }

    resolved
}

impl From<&WarriorAbilitySetConfig> for AbilitySet {
    fn from(value: &WarriorAbilitySetConfig) -> Self {
        Self {
            name: value.name.clone(),
            candidates: value
                .candidates
                .iter()
                .map(|candidate| AbilityCandidate {
                    name: candidate.name.clone(),
                    min_level: candidate.min_level,
                    spell_id: candidate.spell_id,
                })
                .collect(),
            min_expansion: textquest_common::combat::EQExpansion::Classic,
        }
    }
}

/// Warrior strategy: main tank, selects nearest enemy, and executes a
/// config-backed Live-safe combat rotation.
pub struct WarriorStrategy {
    class_id: u8,
}

impl WarriorStrategy {
    pub fn new(class_id: u8) -> Self {
        Self { class_id }
    }

    fn build_ability_sets() -> Vec<AbilitySet> {
        runtime_config()
            .ability_sets
            .iter()
            .map(AbilitySet::from)
            .collect()
    }

    fn build_rotations() -> Vec<RotationGroup> {
        runtime_config()
            .rotation_groups
            .iter()
            .cloned()
            .map(Into::into)
            .collect()
    }
}

impl ClassStrategy for WarriorStrategy {
    fn class_id(&self) -> u8 {
        self.class_id
    }

    fn select_target(&self, ctx: &CombatContext) -> Option<u32> {
        strategy::nearest_enemy(ctx.player, ctx.nearby_enemies).map(|spawn| spawn.spawn_id)
    }

    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry> {
        ctx.config
            .spells
            .iter()
            .max_by_key(|spell| spell.priority)
            .cloned()
    }

    fn should_assist(&self, _ctx: &CombatContext) -> bool {
        false
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

    fn resolve_abilities_for_character(
        &self,
        known: &[KnownAbility],
        character_level: u8,
    ) -> HashMap<String, AbilityResolution> {
        resolve_runtime_abilities(&runtime_config().ability_sets, known, character_level)
    }

    fn manages_melee_skills_in_rotation(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use textquest_common::{
        combat::{ActionType, CombatConfig},
        types::SpawnData,
    };

    use crate::combat::rotation;

    static DEFAULT_CONFIG: LazyLock<CombatConfig> = LazyLock::new(CombatConfig::default);

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
            positional: None,
        }
    }

    fn make_player(hp_pct: f32, endurance_pct: f32) -> SpawnData {
        let mut player = SpawnData::default();
        player.spawn_id = 1;
        player.hp_max = 10_000;
        player.hp_current = ((hp_pct / 100.0) * player.hp_max as f32) as i64;
        player.endurance_max = 100;
        player.endurance_current = endurance_pct as i32;
        player
    }

    fn make_target(hp_pct: f32) -> SpawnData {
        let mut target = SpawnData::default();
        target.spawn_id = 42;
        target.hp_max = 10_000;
        target.hp_current = ((hp_pct / 100.0) * target.hp_max as f32) as i64;
        target.spawn_type = 1;
        target
    }

    #[test]
    fn warrior_class_id() {
        let warrior = WarriorStrategy::new(1);
        assert_eq!(warrior.class_id(), 1);
    }

    #[test]
    fn warrior_role_is_main_tank() {
        let warrior = WarriorStrategy::new(1);
        assert_eq!(warrior.role(), CombatRole::MainTank);
    }

    #[test]
    fn warrior_manages_melee_skills_in_rotation() {
        let warrior = WarriorStrategy::new(1);
        assert!(warrior.manages_melee_skills_in_rotation());
    }

    #[test]
    fn warrior_has_config_backed_rotation_groups() {
        let warrior = WarriorStrategy::new(1);
        let groups = warrior
            .rotation_groups()
            .expect("warrior groups should load");
        let names: Vec<_> = groups.iter().map(|group| group.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "Downtime",
                "HateTools",
                "Emergency",
                "Defenses",
                "Burn",
                "Combat"
            ]
        );
    }

    #[test]
    fn warrior_combat_priority_starts_with_taunt() {
        let warrior = WarriorStrategy::new(1);
        let mut groups = warrior
            .rotation_groups()
            .expect("warrior groups should load");
        let player = make_player(85.0, 80.0);
        let target = make_target(90.0);
        let ctx = make_ctx(&player, Some(&target), &[], true);

        let action = rotation::execute_rotations(&mut groups, &ctx)
            .expect("warrior combat rotation should produce an action");
        assert_eq!(action.entry_name, "Taunt");
        assert_eq!(action.action_type, ActionType::Ability("Taunt".into()));
    }

    #[test]
    fn warrior_burn_rotation_requires_endurance_budget() {
        let warrior = WarriorStrategy::new(1);
        let mut groups = warrior
            .rotation_groups()
            .expect("warrior groups should load");
        let burn = groups
            .iter_mut()
            .find(|group| group.name == "Burn")
            .expect("burn group should exist");

        let low_endurance_player = make_player(85.0, 30.0);
        let target = make_target(90.0);
        let low_ctx = make_ctx(&low_endurance_player, Some(&target), &[], true);
        let low_result = rotation::execute_group(burn, &low_ctx);
        assert!(low_result.actions.is_empty());

        let high_endurance_player = make_player(85.0, 80.0);
        let high_ctx = make_ctx(&high_endurance_player, Some(&target), &[], true);
        let high_result = rotation::execute_group(burn, &high_ctx);
        assert_eq!(high_result.actions[0].entry_name, "BurnPrimary");
    }

    #[test]
    fn warrior_runtime_ability_sets_are_config_backed() {
        let warrior = WarriorStrategy::new(1);
        let sets = warrior.ability_sets();
        let set_names: Vec<_> = sets.iter().map(|set| set.name.as_str()).collect();
        assert_eq!(
            set_names,
            vec![
                "EmergencyGuard",
                "DefensiveLine",
                "BurnPrimary",
                "PrecisionLine"
            ]
        );
    }

    #[test]
    fn warrior_resolves_live_level_breakpoints() {
        let warrior = WarriorStrategy::new(1);

        let resolved_60 = warrior.resolve_abilities_for_character(&[], 60);
        assert_eq!(
            resolved_60.get("EmergencyGuard").unwrap().ability_name,
            "Fortitude Discipline"
        );
        assert_eq!(
            resolved_60.get("BurnPrimary").unwrap().ability_name,
            "Aggressive Discipline"
        );

        let resolved_61 = warrior.resolve_abilities_for_character(&[], 61);
        assert_eq!(
            resolved_61.get("BurnPrimary").unwrap().ability_name,
            "Spirit of Rage Discipline"
        );

        let resolved_62 = warrior.resolve_abilities_for_character(&[], 62);
        assert_eq!(
            resolved_62.get("EmergencyGuard").unwrap().ability_name,
            "Deflection Discipline"
        );

        let resolved_65 = warrior.resolve_abilities_for_character(&[], 65);
        assert_eq!(
            resolved_65.get("EmergencyGuard").unwrap().ability_name,
            "Stonewall Discipline"
        );
        assert_eq!(
            resolved_65.get("BurnPrimary").unwrap().ability_name,
            "Fellstrike Discipline"
        );
    }

    #[test]
    fn warrior_resolved_burn_metadata_carries_shared_timer() {
        let warrior = WarriorStrategy::new(1);
        let resolved = warrior.resolve_abilities_for_character(&[], 65);
        let burn = resolved
            .get("BurnPrimary")
            .expect("burn primary should resolve at 65");

        assert_eq!(burn.cooldown_ticks, Some(36000));
        assert_eq!(
            burn.shared_cooldown_key.as_deref(),
            Some("warrior-offensive-disc")
        );
        assert_eq!(burn.shared_cooldown_ticks, Some(36000));
    }

    #[test]
    fn warrior_falls_back_to_embedded_defaults_on_malformed_toml() {
        let config = load_runtime_config_from_contents("ability_sets = [", "test warrior config");

        let set_names: Vec<_> = config
            .ability_sets
            .iter()
            .map(|set| set.name.as_str())
            .collect();
        assert_eq!(
            set_names,
            vec![
                "EmergencyGuard",
                "DefensiveLine",
                "BurnPrimary",
                "PrecisionLine"
            ]
        );

        let rotation_names: Vec<_> = config
            .rotation_groups
            .iter()
            .map(|group| group.name.as_str())
            .collect();
        assert_eq!(
            rotation_names,
            vec![
                "Downtime",
                "HateTools",
                "Emergency",
                "Defenses",
                "Burn",
                "Combat"
            ]
        );
    }

    #[test]
    fn select_target_prefers_nearest_enemy() {
        let warrior = WarriorStrategy::new(1);
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
        let ctx = make_ctx(&player, None, &enemies, true);
        assert_eq!(warrior.select_target(&ctx), Some(2));
    }
}
