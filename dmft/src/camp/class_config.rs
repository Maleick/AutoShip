//! Class-specific ability configurations — loadable from TOML files in `config/classes/`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single ability a class can use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassAbility {
    /// Ability name for display/logging.
    pub name: String,
    /// Slash command to execute this ability (e.g., "/kick").
    pub command: String,
    /// Recast cooldown in seconds.
    pub cooldown_secs: f32,
    /// Priority ordering (lower = higher priority).
    pub priority: u8,
    /// Optional condition expression for when to use this ability.
    #[serde(default)]
    pub condition: Option<String>,
    /// Buff duration in seconds. For `buff_abilities`, this is how long the buff
    /// lasts on the target (NOT the recast cooldown). Defaults to `cooldown_secs`
    /// if not specified, which is correct for abilities where cooldown ≈ duration.
    #[serde(default)]
    pub duration_secs: Option<f32>,
}

impl ClassAbility {
    /// Effective buff duration — uses explicit `duration_secs` if set, else `cooldown_secs`.
    #[must_use]
    pub fn effective_duration_secs(&self) -> f32 {
        self.duration_secs.unwrap_or(self.cooldown_secs)
    }
}

/// A crowd control ability (mez, stun, charm, snare, root).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CcAbilityConfig {
    /// Ability name for display/logging.
    pub name: String,
    /// CC type string (e.g., "mez", "stun", "charm").
    pub cc_type: String,
    /// Slash command to cast the CC.
    pub command: String,
    /// Recast cooldown in seconds.
    pub cooldown_secs: f32,
    /// How long the CC effect lasts in seconds.
    pub duration_secs: f32,
    /// Priority ordering (lower = higher priority).
    pub priority: u8,
}

/// A resist debuff ability (Tash, Malo) that lands before CC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebuffAbilityConfig {
    /// Debuff name (e.g., "Tash", "Malo").
    pub name: String,
    /// Slash command to cast the debuff.
    pub command: String,
    /// Recast cooldown in seconds.
    pub cooldown_secs: f32,
    /// Cast order (lower = cast first).
    pub order: u8,
}

/// Full ability configuration for one EQ class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassConfig {
    /// EQ class name (e.g., "warrior", "cleric").
    pub class_name: String,
    /// Role this class fills (e.g., "tank", "healer", "cc").
    pub role: String,
    /// Abilities to use during active combat.
    #[serde(default)]
    pub combat_abilities: Vec<ClassAbility>,
    /// Buff spells to maintain on group members.
    #[serde(default)]
    pub buff_abilities: Vec<ClassAbility>,
    /// Emergency abilities (heal, defensive cooldowns).
    #[serde(default)]
    pub emergency_abilities: Vec<ClassAbility>,
    /// Crowd control abilities (mez, stun, charm).
    #[serde(default)]
    pub cc_abilities: Vec<CcAbilityConfig>,
    /// Resist debuffs to land before CC (Tash, Malo).
    #[serde(default)]
    pub debuff_abilities: Vec<DebuffAbilityConfig>,
    /// Command to enter rest mode between pulls.
    #[serde(default = "default_rest_command")]
    pub rest_command: String,
    /// Bard twist interval in seconds (only meaningful for bards).
    #[serde(default)]
    pub twist_interval_secs: Option<f32>,
}

fn default_rest_command() -> String {
    "/sit".into()
}

impl ClassConfig {
    /// Load a class config from the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn load(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read class config: {}", path.display()))?;
        let config: Self = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse class config: {}", path.display()))?;
        Ok(config)
    }

    /// Save this class config to the given path (creates parent dirs).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        let toml_str = toml::to_string_pretty(self).context("Failed to serialize class config")?;
        std::fs::write(path, toml_str)
            .with_context(|| format!("Failed to write class config: {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_warrior() -> ClassConfig {
        ClassConfig {
            class_name: "warrior".into(),
            role: "tank".into(),
            combat_abilities: vec![
                ClassAbility {
                    name: "Taunt".into(),
                    command: "/taunt".into(),
                    cooldown_secs: 8.0,
                    priority: 1,
                    condition: None,
                    duration_secs: None,
                },
                ClassAbility {
                    name: "Kick".into(),
                    command: "/kick".into(),
                    cooldown_secs: 6.0,
                    priority: 2,
                    condition: None,
                    duration_secs: None,
                },
            ],
            buff_abilities: vec![],
            emergency_abilities: vec![],
            cc_abilities: vec![],
            debuff_abilities: vec![],
            rest_command: "/sit".into(),
            twist_interval_secs: None,
        }
    }

    #[test]
    fn test_roundtrip_toml() {
        let config = sample_warrior();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let loaded: ClassConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(loaded.class_name, "warrior");
        assert_eq!(loaded.role, "tank");
        assert_eq!(loaded.combat_abilities.len(), 2);
        assert_eq!(loaded.combat_abilities[0].cooldown_secs, 8.0);
        assert!(loaded.emergency_abilities.is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("warrior.toml");

        let config = sample_warrior();
        config.save(&path).unwrap();

        let loaded = ClassConfig::load(&path).unwrap();
        assert_eq!(loaded.class_name, "warrior");
        assert_eq!(loaded.combat_abilities.len(), 2);
        assert_eq!(loaded.rest_command, "/sit");
    }

    #[test]
    fn test_default_rest_command() {
        let toml_str = r#"
            class_name = "monk"
            role = "dps"
        "#;
        let config: ClassConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.rest_command, "/sit");
        assert!(config.combat_abilities.is_empty());
    }

    #[test]
    fn test_condition_field() {
        let toml_str = r#"
            class_name = "cleric"
            role = "healer"
            rest_command = "/sit"

            [[emergency_abilities]]
            name = "Divine Arb"
            command = "/cast 3"
            cooldown_secs = 30.0
            priority = 0
            condition = "target_hp_below_20"
        "#;
        let config: ClassConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.emergency_abilities[0].condition.as_deref(),
            Some("target_hp_below_20")
        );
    }

    #[test]
    fn effective_duration_explicit() {
        let ability = ClassAbility {
            name: "Haste".into(),
            command: "/cast 1".into(),
            cooldown_secs: 10.0,
            priority: 1,
            condition: None,
            duration_secs: Some(300.0),
        };
        assert!((ability.effective_duration_secs() - 300.0).abs() < f32::EPSILON);
    }

    #[test]
    fn effective_duration_falls_back_to_cooldown() {
        let ability = ClassAbility {
            name: "Taunt".into(),
            command: "/taunt".into(),
            cooldown_secs: 8.0,
            priority: 1,
            condition: None,
            duration_secs: None,
        };
        assert!((ability.effective_duration_secs() - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn cc_ability_roundtrip() {
        let cc = CcAbilityConfig {
            name: "Mesmerize".into(),
            cc_type: "mez".into(),
            command: "/cast 4".into(),
            cooldown_secs: 3.0,
            duration_secs: 18.0,
            priority: 1,
        };
        let toml_str = toml::to_string(&cc).unwrap();
        let loaded: CcAbilityConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(loaded.name, "Mesmerize");
        assert_eq!(loaded.cc_type, "mez");
        assert!((loaded.duration_secs - 18.0).abs() < f32::EPSILON);
    }

    #[test]
    fn debuff_ability_roundtrip() {
        let debuff = DebuffAbilityConfig {
            name: "Tashani".into(),
            command: "/cast 5".into(),
            cooldown_secs: 2.5,
            order: 1,
        };
        let toml_str = toml::to_string(&debuff).unwrap();
        let loaded: DebuffAbilityConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(loaded.name, "Tashani");
        assert_eq!(loaded.order, 1);
    }

    #[test]
    fn class_config_with_all_fields() {
        let toml_str = r#"
            class_name = "enchanter"
            role = "cc"
            rest_command = "/sit"
            twist_interval_secs = 3.0

            [[combat_abilities]]
            name = "Nuke"
            command = "/cast 1"
            cooldown_secs = 5.0
            priority = 2

            [[cc_abilities]]
            name = "Mez"
            cc_type = "mez"
            command = "/cast 4"
            cooldown_secs = 3.0
            duration_secs = 18.0
            priority = 1

            [[debuff_abilities]]
            name = "Tash"
            command = "/cast 5"
            cooldown_secs = 2.5
            order = 1
        "#;
        let config: ClassConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.class_name, "enchanter");
        assert_eq!(config.combat_abilities.len(), 1);
        assert_eq!(config.cc_abilities.len(), 1);
        assert_eq!(config.debuff_abilities.len(), 1);
        assert!((config.twist_interval_secs.unwrap() - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn load_nonexistent_file_returns_error() {
        let result = ClassConfig::load(Path::new("/nonexistent/path.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn class_ability_clone() {
        let ability = ClassAbility {
            name: "Taunt".into(),
            command: "/taunt".into(),
            cooldown_secs: 8.0,
            priority: 1,
            condition: Some("always".into()),
            duration_secs: Some(0.0),
        };
        let cloned = ability.clone();
        assert_eq!(cloned, ability);
    }

    #[test]
    fn class_ability_debug_format() {
        let ability = ClassAbility {
            name: "Kick".into(),
            command: "/kick".into(),
            cooldown_secs: 6.0,
            priority: 2,
            condition: None,
            duration_secs: None,
        };
        let dbg = format!("{:?}", ability);
        assert!(dbg.contains("Kick"));
        assert!(dbg.contains("6.0"));
    }

    #[test]
    fn cc_ability_config_debug_clone() {
        let cc = CcAbilityConfig {
            name: "Stun".into(),
            cc_type: "stun".into(),
            command: "/cast 2".into(),
            cooldown_secs: 6.0,
            duration_secs: 12.0,
            priority: 0,
        };
        let cloned = cc.clone();
        assert_eq!(cloned, cc);
        let dbg = format!("{:?}", cc);
        assert!(dbg.contains("stun"));
    }

    #[test]
    fn debuff_ability_config_debug_clone() {
        let debuff = DebuffAbilityConfig {
            name: "Malo".into(),
            command: "/cast 6".into(),
            cooldown_secs: 3.0,
            order: 2,
        };
        let cloned = debuff.clone();
        assert_eq!(cloned, debuff);
        let dbg = format!("{:?}", debuff);
        assert!(dbg.contains("Malo"));
    }

    #[test]
    fn class_config_clone_and_debug() {
        let config = sample_warrior();
        let cloned = config.clone();
        assert_eq!(cloned, config);
        let dbg = format!("{:?}", config);
        assert!(dbg.contains("warrior"));
    }

    #[test]
    fn class_config_serialization_roundtrip() {
        let config = sample_warrior();
        let json = serde_json::to_string(&config).unwrap();
        let loaded: ClassConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn default_rest_command_fn() {
        assert_eq!(default_rest_command(), "/sit");
    }

    #[test]
    fn effective_duration_zero_explicit() {
        let ability = ClassAbility {
            name: "Instant".into(),
            command: "/cast 1".into(),
            cooldown_secs: 5.0,
            priority: 1,
            condition: None,
            duration_secs: Some(0.0),
        };
        assert!((ability.effective_duration_secs() - 0.0).abs() < f32::EPSILON);
    }

    /// Validate all shipped class TOML files parse correctly.
    #[test]
    fn test_all_shipped_configs_parse() {
        let classes_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("config/classes");

        if !classes_dir.exists() {
            panic!(
                "config/classes directory not found at {}",
                classes_dir.display()
            );
        }

        let mut count = 0;
        for entry in std::fs::read_dir(&classes_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toml") {
                let config = ClassConfig::load(&path)
                    .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", path.display()));
                assert!(
                    !config.class_name.is_empty(),
                    "class_name empty in {}",
                    path.display()
                );
                assert!(!config.role.is_empty(), "role empty in {}", path.display());
                count += 1;
            }
        }
        assert!(
            count >= 7,
            "Expected at least 7 class configs, found {count}"
        );
    }
}
