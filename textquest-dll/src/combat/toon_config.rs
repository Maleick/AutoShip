use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use textquest_common::combat::{
    ActionType, CombatConfig, CombatStateReq, ConditionExpr, DisciplineEntry, HolyShitCondition,
    TargetSelector,
};

use super::rotation::{RotationEntry, RotationGroup};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ToonCombatConfig {
    #[serde(default)]
    pub spells: Option<Vec<textquest_common::combat::SpellEntry>>,
    #[serde(default)]
    pub disciplines: Option<Vec<DisciplineEntry>>,
    #[serde(default)]
    pub holyshit_rules: Option<Vec<HolyShitCondition>>,
    #[serde(default)]
    pub rotation_groups: Option<Vec<ToonRotationGroup>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToonRotationGroup {
    pub name: String,
    pub target_selector: TargetSelector,
    pub combat_state_req: CombatStateReq,
    #[serde(default = "default_steps_per_frame")]
    pub steps_per_frame: u8,
    #[serde(default)]
    pub full_rotation: bool,
    #[serde(default)]
    pub hp_threshold: Option<f32>,
    #[serde(default)]
    pub entries: Vec<ToonRotationEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToonRotationEntry {
    pub name: String,
    pub action_type: ActionType,
    #[serde(default)]
    pub condition: Option<ConditionExpr>,
    #[serde(default)]
    pub active_condition: Option<ConditionExpr>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub cooldown_key: Option<String>,
}

const TOON_CONFIG_ENV_VAR: &str = "TEXTQUEST_TOON_CONFIG_DIR";

fn default_steps_per_frame() -> u8 {
    1
}

fn default_enabled() -> bool {
    true
}

fn sanitize_toon_name(toon_name: &str) -> String {
    toon_name
        .trim()
        .chars()
        .filter(|ch| {
            !ch.is_control() && !matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
        .collect()
}

fn configured_toon_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os(TOON_CONFIG_ENV_VAR) {
        return Ok(PathBuf::from(dir));
    }

    Ok(std::env::current_dir()
        .context("Failed to resolve current working directory for toon config lookup")?
        .join("config/toons"))
}

fn candidate_paths(base_dir: &Path, toon_name: &str) -> Vec<PathBuf> {
    let sanitized = sanitize_toon_name(toon_name);
    if sanitized.is_empty() {
        return Vec::new();
    }

    let lower = sanitized.to_ascii_lowercase();
    let mut names = vec![sanitized.clone()];
    if lower != sanitized {
        names.push(lower);
    }

    let mut paths = Vec::new();
    for name in names {
        paths.push(base_dir.join(format!("{name}.toml")));
    }
    paths
}

pub fn load_for_toon(toon_name: &str) -> Result<Option<(PathBuf, ToonCombatConfig)>> {
    let toon_dir = configured_toon_dir()?;
    load_from_dir(&toon_dir, toon_name)
}

pub fn load_from_dir(
    base_dir: &Path,
    toon_name: &str,
) -> Result<Option<(PathBuf, ToonCombatConfig)>> {
    for path in candidate_paths(base_dir, toon_name) {
        if !path.is_file() {
            continue;
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read toon combat config: {}", path.display()))?;
        let config = toml::from_str::<ToonCombatConfig>(&contents)
            .with_context(|| format!("Failed to parse toon combat config: {}", path.display()))?;
        return Ok(Some((path, config)));
    }

    Ok(None)
}

impl ToonCombatConfig {
    pub fn apply_to(
        &self,
        combat_config: &mut CombatConfig,
        rotation_groups: &mut Option<Vec<RotationGroup>>,
    ) {
        if let Some(spells) = &self.spells {
            combat_config.spells = spells.clone();
        }
        if let Some(disciplines) = &self.disciplines {
            combat_config.disciplines = disciplines.clone();
        }
        if let Some(holyshit_rules) = &self.holyshit_rules {
            combat_config.holyshit_rules = holyshit_rules.clone();
        }
        if let Some(groups) = &self.rotation_groups {
            *rotation_groups = Some(groups.iter().cloned().map(Into::into).collect());
        }
    }
}

impl From<ToonRotationGroup> for RotationGroup {
    fn from(value: ToonRotationGroup) -> Self {
        Self {
            name: value.name,
            target_selector: value.target_selector,
            combat_state_req: value.combat_state_req,
            steps_per_frame: value.steps_per_frame,
            full_rotation: value.full_rotation,
            hp_threshold: value.hp_threshold,
            burn_duration_ticks: None,
            burn_cooldown_duration_ticks: None,
            entries: value.entries.into_iter().map(Into::into).collect(),
            current_step: 0,
        }
    }
}

impl From<ToonRotationEntry> for RotationEntry {
    fn from(value: ToonRotationEntry) -> Self {
        Self {
            name: value.name,
            action_type: value.action_type,
            condition: value.condition,
            active_condition: value.active_condition,
            pre_activate: None,
            post_activate: None,
            enabled: value.enabled,
            cooldown_key: value.cooldown_key,
            cooldown_ticks: None,
            shared_cooldown_key: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::combat::{CombatConfig, HolyShitAction, SpellEntry};

    #[test]
    fn load_from_dir_reads_toon_override() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("Camrene.toml");
        std::fs::write(
            &path,
            r#"
                [[spells]]
                slot = 1
                spell_id = 200
                name = "Flash of Light"
                min_mana_pct = 0.0
                priority = 1
                is_aoe = false

                [[holyshit_rules]]
                priority = 1
                condition = "Always"
                action = "Flee"

                [[rotation_groups]]
                name = "Combat"
                target_selector = "AutoTarget"
                combat_state_req = "Combat"
                steps_per_frame = 1

                [[rotation_groups.entries]]
                name = "Kick"
                action_type = { Ability = "Kick" }
            "#,
        )
        .unwrap();

        let loaded = load_from_dir(temp_dir.path(), "Camrene").unwrap().unwrap();
        assert_eq!(loaded.0, path);
        assert_eq!(loaded.1.spells.as_ref().unwrap()[0].name, "Flash of Light");
        assert!(matches!(
            loaded.1.holyshit_rules.as_ref().unwrap()[0].action,
            HolyShitAction::Flee
        ));
        assert_eq!(
            loaded.1.rotation_groups.as_ref().unwrap()[0].entries[0].name,
            "Kick"
        );
    }

    #[test]
    fn load_from_dir_falls_back_to_lowercase_name() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("camrene.toml");
        std::fs::write(
            &path,
            r#"
                [[spells]]
                slot = 2
                spell_id = 201
                name = "Stun"
                min_mana_pct = 0.0
                priority = 2
                is_aoe = false
            "#,
        )
        .unwrap();

        let loaded = load_from_dir(temp_dir.path(), "Camrene").unwrap().unwrap();
        // On case-insensitive filesystems (macOS), the first candidate
        // ("Camrene.toml") matches the lowercase file, so compare
        // case-insensitively.
        assert_eq!(
            loaded
                .0
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_lowercase(),
            "camrene.toml"
        );
        assert_eq!(loaded.1.spells.unwrap()[0].slot, 2);
    }

    #[test]
    fn apply_to_replaces_combat_actions_and_rotations() {
        let override_config = ToonCombatConfig {
            spells: Some(vec![SpellEntry {
                slot: 3,
                spell_id: 302,
                name: "Cease".into(),
                min_mana_pct: 10.0,
                priority: 3,
                is_aoe: false,
            }]),
            disciplines: Some(vec![DisciplineEntry {
                name: "Deflection".into(),
                spell_id: 4694,
                priority: 1,
                cooldown_ticks: 1200,
                min_hp_pct: 0.0,
                max_hp_pct: 50.0,
                min_endurance_pct: 0.0,
            }]),
            holyshit_rules: Some(vec![HolyShitCondition {
                priority: 1,
                condition: ConditionExpr::Always,
                action: HolyShitAction::Flee,
            }]),
            rotation_groups: Some(vec![ToonRotationGroup {
                name: "Combat".into(),
                target_selector: TargetSelector::AutoTarget,
                combat_state_req: CombatStateReq::Combat,
                steps_per_frame: 2,
                full_rotation: true,
                hp_threshold: Some(35.0),
                entries: vec![ToonRotationEntry {
                    name: "Taunt".into(),
                    action_type: ActionType::Ability("Taunt".into()),
                    condition: Some(ConditionExpr::Always),
                    active_condition: None,
                    enabled: true,
                    cooldown_key: None,
                }],
            }]),
        };

        let mut combat_config = CombatConfig::default();
        let mut rotation_groups = None;
        override_config.apply_to(&mut combat_config, &mut rotation_groups);

        assert_eq!(combat_config.spells[0].name, "Cease");
        assert_eq!(combat_config.disciplines[0].name, "Deflection");
        assert!(matches!(
            combat_config.holyshit_rules[0].action,
            HolyShitAction::Flee
        ));
        let groups = rotation_groups.unwrap();
        assert_eq!(groups[0].name, "Combat");
        assert_eq!(groups[0].steps_per_frame, 2);
        assert!(groups[0].full_rotation);
        assert_eq!(groups[0].entries[0].name, "Taunt");
    }

    #[test]
    fn sanitize_toon_name_strips_path_separators() {
        let sanitized = sanitize_toon_name("../../etc/passwd");
        assert!(!sanitized.contains('/'));
        assert!(!sanitized.contains('\\'));
        // '.' is allowed; letters remain; separators are stripped
        assert_eq!(sanitized, "....etcpasswd");
    }

    #[test]
    fn sanitize_toon_name_removes_filesystem_special_chars() {
        let chars_to_strip = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
        for ch in chars_to_strip {
            let name = format!("toon{ch}name");
            let sanitized = sanitize_toon_name(&name);
            assert!(
                !sanitized.contains(ch),
                "sanitized name should not contain '{ch}'"
            );
        }
    }

    #[test]
    fn sanitize_toon_name_removes_control_characters() {
        let name = "toon\x00name\x1f";
        let sanitized = sanitize_toon_name(name);
        assert!(!sanitized.contains('\x00'));
        assert!(!sanitized.contains('\x1f'));
        assert_eq!(sanitized, "toonname");
    }

    #[test]
    fn sanitize_toon_name_trims_whitespace() {
        let sanitized = sanitize_toon_name("  Warrior  ");
        assert_eq!(sanitized, "Warrior");
    }

    #[test]
    fn sanitize_toon_name_empty_input_returns_empty() {
        assert_eq!(sanitize_toon_name(""), "");
        assert_eq!(sanitize_toon_name("   "), "");
    }

    #[test]
    fn sanitize_toon_name_preserves_normal_chars() {
        let name = "Lyralei1";
        assert_eq!(sanitize_toon_name(name), name);
    }

    #[test]
    fn candidate_paths_empty_after_sanitization() {
        let base = std::path::Path::new("/tmp");
        // All chars stripped → sanitized is empty → no candidates
        let paths = candidate_paths(base, "///");
        assert!(paths.is_empty());
    }

    #[test]
    fn candidate_paths_returns_exact_and_lowercase() {
        let base = std::path::Path::new("/tmp/toons");
        let paths = candidate_paths(base, "Warrior");
        // "Warrior" != "warrior", so both should be present
        assert_eq!(paths.len(), 2);
        let names: Vec<String> = paths
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert!(
            names.contains(&"Warrior.toml".to_string()),
            "should contain original-case filename"
        );
        assert!(
            names.contains(&"warrior.toml".to_string()),
            "should contain lowercase filename"
        );
    }

    #[test]
    fn candidate_paths_already_lowercase_returns_one() {
        let base = std::path::Path::new("/tmp/toons");
        let paths = candidate_paths(base, "warrior");
        // If already lowercase, no duplicate needed
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("warrior.toml"));
    }

    #[test]
    fn load_from_dir_returns_none_for_missing_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = load_from_dir(temp_dir.path(), "NonExistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn load_from_dir_returns_none_for_empty_toon_name() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = load_from_dir(temp_dir.path(), "///").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn apply_to_empty_config_leaves_defaults() {
        let override_config = ToonCombatConfig::default();
        let mut combat_config = CombatConfig::default();
        let mut rotation_groups: Option<Vec<super::super::rotation::RotationGroup>> = None;
        override_config.apply_to(&mut combat_config, &mut rotation_groups);
        // Nothing should change when override is empty
        assert!(combat_config.spells.is_empty());
        assert!(rotation_groups.is_none());
    }

    #[test]
    fn toon_rotation_entry_defaults_enabled_true() {
        let entry: ToonRotationEntry = toml::from_str(
            r#"
            name = "Kick"
            action_type = { Ability = "Kick" }
        "#,
        )
        .unwrap();
        assert!(entry.enabled);
    }

    #[test]
    fn toon_rotation_group_defaults_steps_per_frame_one() {
        let group: ToonRotationGroup = toml::from_str(
            r#"
            name = "Combat"
            target_selector = "AutoTarget"
            combat_state_req = "Combat"
        "#,
        )
        .unwrap();
        assert_eq!(group.steps_per_frame, 1);
    }
}
