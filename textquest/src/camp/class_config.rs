//! Class-specific ability configurations — loadable from TOML files in
//! `config/classes/`.

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
    /// Buff duration in seconds. For `buff_abilities`, this is how long the
    /// buff lasts on the target (NOT the recast cooldown). Defaults to
    /// `cooldown_secs` if not specified, which is correct for abilities
    /// where cooldown ≈ duration.
    #[serde(default)]
    pub duration_secs: Option<f32>,
}

impl ClassAbility {
    /// Effective buff duration — uses explicit `duration_secs` if set, else
    /// `cooldown_secs`.
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
    /// Level-gated ability profile overrides for this class.
    #[serde(default)]
    pub level_overrides: Vec<AbilityProfileOverride>,
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

/// Resolved ability profile after applying any level-based overrides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbilityProfile {
    /// Abilities to use during active combat.
    pub combat_abilities: Vec<ClassAbility>,
    /// Buff spells to maintain on group members.
    pub buff_abilities: Vec<ClassAbility>,
    /// Emergency abilities (heal, defensive cooldowns).
    pub emergency_abilities: Vec<ClassAbility>,
    /// Crowd control abilities (mez, stun, charm).
    pub cc_abilities: Vec<CcAbilityConfig>,
    /// Resist debuffs to land before CC (Tash, Malo).
    pub debuff_abilities: Vec<DebuffAbilityConfig>,
}

impl AbilityProfile {
    fn from_config(config: &ClassConfig) -> Self {
        Self {
            combat_abilities: config.combat_abilities.clone(),
            buff_abilities: config.buff_abilities.clone(),
            emergency_abilities: config.emergency_abilities.clone(),
            cc_abilities: config.cc_abilities.clone(),
            debuff_abilities: config.debuff_abilities.clone(),
        }
    }
}

/// A level-gated override that can replace portions of the base ability
/// profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AbilityProfileOverride {
    /// Human-friendly name for this override profile.
    pub name: String,
    /// Inclusive minimum level where this override applies.
    #[serde(default)]
    pub min_level: Option<u8>,
    /// Inclusive maximum level where this override applies.
    #[serde(default)]
    pub max_level: Option<u8>,
    /// Combat abilities specific to this override.
    #[serde(default)]
    pub combat_abilities: Option<Vec<ClassAbility>>,
    /// Buff abilities specific to this override.
    #[serde(default)]
    pub buff_abilities: Option<Vec<ClassAbility>>,
    /// Emergency abilities specific to this override.
    #[serde(default)]
    pub emergency_abilities: Option<Vec<ClassAbility>>,
    /// CC abilities specific to this override.
    #[serde(default)]
    pub cc_abilities: Option<Vec<CcAbilityConfig>>,
    /// Debuff abilities specific to this override.
    #[serde(default)]
    pub debuff_abilities: Option<Vec<DebuffAbilityConfig>>,
}

impl AbilityProfileOverride {
    /// Whether this override applies to the provided level.
    #[must_use]
    pub fn applies_to(&self, level: u8) -> bool {
        let min = self.min_level.unwrap_or(0);
        let max = self.max_level.unwrap_or(u8::MAX);
        level >= min && level <= max
    }

    fn apply(&self, base: &AbilityProfile) -> AbilityProfile {
        AbilityProfile {
            combat_abilities: self
                .combat_abilities
                .clone()
                .unwrap_or_else(|| base.combat_abilities.clone()),
            buff_abilities: self
                .buff_abilities
                .clone()
                .unwrap_or_else(|| base.buff_abilities.clone()),
            emergency_abilities: self
                .emergency_abilities
                .clone()
                .unwrap_or_else(|| base.emergency_abilities.clone()),
            cc_abilities: self
                .cc_abilities
                .clone()
                .unwrap_or_else(|| base.cc_abilities.clone()),
            debuff_abilities: self
                .debuff_abilities
                .clone()
                .unwrap_or_else(|| base.debuff_abilities.clone()),
        }
    }
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

    /// Resolve the effective ability profile for a given level, applying the
    /// most specific matching override when available.
    #[must_use]
    pub fn profile_for_level(&self, level: Option<u8>) -> AbilityProfile {
        let base = AbilityProfile::from_config(self);
        let level = match level {
            Some(level) => level,
            None => return base,
        };

        let selected_override = self
            .level_overrides
            .iter()
            .filter(|o| o.applies_to(level))
            .max_by_key(|o| (o.min_level.unwrap_or(0), o.max_level.unwrap_or(u8::MAX)));

        selected_override
            .map(|ovr| ovr.apply(&base))
            .unwrap_or(base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_warrior() -> ClassConfig {
        ClassConfig {
            class_name: "warrior".into(),
            role: "tank".into(),
            level_overrides: Vec::new(),
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
    fn profile_for_level_applies_override_and_falls_back() {
        let base_combat = ClassAbility {
            name: "Kick".into(),
            command: "/kick".into(),
            cooldown_secs: 6.0,
            priority: 2,
            condition: None,
            duration_secs: None,
        };
        let base_buff = ClassAbility {
            name: "Base Buff".into(),
            command: "/cast 4".into(),
            cooldown_secs: 200.0,
            priority: 1,
            condition: None,
            duration_secs: None,
        };
        let override_combat = ClassAbility {
            name: "Flying Kick".into(),
            command: "/doability 1".into(),
            cooldown_secs: 5.0,
            priority: 1,
            condition: None,
            duration_secs: None,
        };

        let config = ClassConfig {
            class_name: "monk".into(),
            role: "dps".into(),
            level_overrides: vec![AbilityProfileOverride {
                name: "low-level".into(),
                min_level: Some(1),
                max_level: Some(20),
                combat_abilities: Some(vec![override_combat.clone()]),
                ..AbilityProfileOverride::default()
            }],
            combat_abilities: vec![base_combat.clone()],
            buff_abilities: vec![base_buff.clone()],
            emergency_abilities: vec![],
            cc_abilities: vec![],
            debuff_abilities: vec![],
            rest_command: "/sit".into(),
            twist_interval_secs: None,
        };

        let low_profile = config.profile_for_level(Some(10));
        assert_eq!(
            low_profile.combat_abilities[0].name, "Flying Kick",
            "Override should replace combat abilities when level matches"
        );
        assert_eq!(
            low_profile.buff_abilities[0].name, "Base Buff",
            "Override should fall back to base for missing categories"
        );

        let high_profile = config.profile_for_level(Some(50));
        assert_eq!(
            high_profile.combat_abilities[0].name, "Kick",
            "Base profile should be used when no override applies"
        );
    }

    #[test]
    fn level_overrides_parse_from_toml() {
        let toml_str = r#"
            class_name = "cleric"
            role = "healer"

            [[level_overrides]]
            name = "low"
            min_level = 1
            max_level = 20

            [[level_overrides.buff_abilities]]
            name = "Minor HP Buff"
            command = "/cast 1"
            cooldown_secs = 30.0
            priority = 1
        "#;
        let config: ClassConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.level_overrides.len(), 1);
        let override_profile = &config.level_overrides[0];
        assert_eq!(override_profile.name, "low");
        assert_eq!(override_profile.min_level, Some(1));
        assert_eq!(override_profile.max_level, Some(20));
        let buff = override_profile
            .buff_abilities
            .as_ref()
            .expect("buff overrides should parse");
        assert_eq!(buff[0].name, "Minor HP Buff");
        assert_eq!(config.rest_command, "/sit");
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

    #[test]
    fn beastlord_config_breakpoints_are_explicit_and_group_safe() {
        let classes_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("config/classes");
        let path = classes_dir.join("beastlord.toml");
        let config = ClassConfig::load(&path)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", path.display()));

        let level_60 = config.profile_for_level(Some(60));
        assert_eq!(level_60.combat_abilities[2].name, "Sha's Advantage");
        assert_eq!(level_60.buff_abilities[0].name, "Savagery");
        assert_eq!(level_60.buff_abilities[1].name, "Spiritual Strength");
        assert!(level_60.emergency_abilities.is_empty());

        let level_61 = config.profile_for_level(Some(61));
        assert!(
            level_61
                .combat_abilities
                .iter()
                .any(|ability| ability.name == "Scorpion Venom"),
            "61 profile should add the poison DPS line"
        );
        assert!(
            level_61
                .buff_abilities
                .iter()
                .any(|ability| ability.name == "Infusion of Spirit"),
            "61 profile should add the single-target melee buff"
        );

        let level_62 = config.profile_for_level(Some(62));
        let level_62_buffs: Vec<&str> = level_62
            .buff_abilities
            .iter()
            .map(|ability| ability.name.as_str())
            .collect();
        assert_eq!(level_62_buffs, vec!["Spiritual Vigor", "Talisman of Kragg"]);

        let level_65 = config.profile_for_level(Some(65));
        assert_eq!(level_65.combat_abilities[2].name, "Sha's Revenge");
        assert!(
            level_65
                .combat_abilities
                .iter()
                .any(|ability| ability.name == "Trushar's Frost"),
            "65 profile should upgrade the direct damage spell"
        );
        assert_eq!(level_65.emergency_abilities[0].name, "Trushar's Mending");
        let level_65_buffs: Vec<&str> = level_65
            .buff_abilities
            .iter()
            .map(|ability| ability.name.as_str())
            .collect();
        assert_eq!(
            level_65_buffs,
            vec!["Ferocity", "Spiritual Vigor", "Talisman of Kragg"]
        );

        for profile in [level_60, level_61, level_62, level_65] {
            assert!(
                profile
                    .buff_abilities
                    .iter()
                    .all(|ability| !ability.name.to_ascii_lowercase().contains("warder")),
                "beastlord camp buffs must stay group-target safe"
            );
        }
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

    #[test]
    fn shipped_shaman_config_covers_live_breakpoints() {
        let config: ClassConfig =
            toml::from_str(include_str!("../../../config/classes/shaman.toml")).unwrap();

        let level_60 = config.profile_for_level(Some(60));
        let level_61 = config.profile_for_level(Some(61));
        let level_62 = config.profile_for_level(Some(62));
        let level_65 = config.profile_for_level(Some(65));

        let level_60_names: Vec<&str> = level_60
            .combat_abilities
            .iter()
            .map(|ability| ability.name.as_str())
            .collect();
        assert!(level_60_names.contains(&"Turgur's Insects"));
        assert!(level_60_names.contains(&"Malo"));
        assert!(level_60_names.contains(&"Torpor"));
        assert!(level_60_names.contains(&"Chloroblast"));
        assert!(level_60_names.contains(&"Cannibalize IV"));
        assert!(level_60_names.contains(&"Ancient: Scourge of Nife"));

        let level_61_names: Vec<&str> = level_61
            .combat_abilities
            .iter()
            .map(|ability| ability.name.as_str())
            .collect();
        assert!(level_61_names.contains(&"Cloud of Grummus"));

        let level_62_names: Vec<&str> = level_62
            .combat_abilities
            .iter()
            .map(|ability| ability.name.as_str())
            .collect();
        assert!(level_62_names.contains(&"Tnarg's Mending"));

        let level_62_buff_names: Vec<&str> = level_62
            .buff_abilities
            .iter()
            .map(|ability| ability.name.as_str())
            .collect();
        assert!(level_62_buff_names.contains(&"Focus of Soul"));

        let level_65_names: Vec<&str> = level_65
            .combat_abilities
            .iter()
            .map(|ability| ability.name.as_str())
            .collect();
        assert!(level_65_names.contains(&"Malos"));
        assert!(level_65_names.contains(&"Quiescence"));

        let level_65_buff_names: Vec<&str> = level_65
            .buff_abilities
            .iter()
            .map(|ability| ability.name.as_str())
            .collect();
        assert!(level_65_buff_names.contains(&"Focus of the Seventh"));

        let level_60_malo = level_60
            .debuff_abilities
            .iter()
            .find(|ability| ability.name == "Malo")
            .unwrap();
        assert_eq!(level_60_malo.order, 2);

        let level_65_malos = level_65
            .debuff_abilities
            .iter()
            .find(|ability| ability.name == "Malos")
            .unwrap();
        assert_eq!(level_65_malos.order, 2);
    fn shipped_ranger_config_tracks_level_rotation_overrides() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("config/classes/ranger.toml");
        let config = ClassConfig::load(&path).expect("ranger config should parse");

        let at_60 = config.profile_for_level(Some(60));
        assert_eq!(at_60.buff_abilities[0].name, "Call of the Predator");
        assert_eq!(at_60.combat_abilities[0].name, "Trueshot Discipline");
        assert_eq!(
            at_60.emergency_abilities[1].name,
            "Weapon Shield Discipline"
        );

        let at_61 = config.profile_for_level(Some(61));
        assert!(
            at_61
                .combat_abilities
                .iter()
                .any(|ability| ability.name == "Circle of Winter")
        );

        let at_62 = config.profile_for_level(Some(62));
        assert_eq!(at_62.buff_abilities[1].name, "Call of the Rathe");
        assert!(
            at_62
                .combat_abilities
                .iter()
                .any(|ability| ability.name == "Drifting Death")
        );
        assert_eq!(at_62.debuff_abilities[0].name, "Ensnare");

        let at_64 = config.profile_for_level(Some(64));
        assert_eq!(at_64.debuff_abilities[0].name, "Nature's Rebuke");

        let at_65 = config.profile_for_level(Some(65));
        assert_eq!(at_65.buff_abilities[0].name, "Natureskin");
        assert_eq!(at_65.combat_abilities[1].name, "Sylvan Burn");
    }

    #[test]
    fn shipped_necromancer_config_matches_live_breakpoints() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("config/classes/necromancer.toml");
        let config = ClassConfig::load(&path)
            .unwrap_or_else(|error| panic!("Failed to parse {}: {error}", path.display()));

        let override_ranges: Vec<(Option<u8>, Option<u8>)> = config
            .level_overrides
            .iter()
            .map(|profile| (profile.min_level, profile.max_level))
            .collect();
        assert_eq!(
            override_ranges,
            vec![
                (Some(60), Some(60)),
                (Some(61), Some(61)),
                (Some(62), Some(64)),
                (Some(65), Some(65)),
            ]
        );

        let base = config.profile_for_level(None);
        assert_eq!(base.combat_abilities[0].name, "Resist Debuff");
        assert_eq!(base.combat_abilities[1].name, "Disease DoT");
        assert_eq!(base.emergency_abilities[1].name, "Feign Death");

        let level_60 = config.profile_for_level(Some(60));
        assert_eq!(level_60.combat_abilities[0].name, "Scent of Terris");
        assert_eq!(level_60.combat_abilities[1].name, "Splurt");
        assert_eq!(level_60.combat_abilities[2].name, "Funeral Pyre of Kelador");
        assert_eq!(level_60.combat_abilities[4].name, "Touch of Night");
        assert_eq!(level_60.buff_abilities[0].name, "Arch Lich");

        let level_61 = config.profile_for_level(Some(61));
        assert_eq!(level_61.combat_abilities[1].name, "Dark Plague");

        let level_62 = config.profile_for_level(Some(62));
        assert_eq!(level_62.combat_abilities[3].name, "Legacy of Zek");
        assert_eq!(level_62.combat_abilities[5].name, "Touch of Mujaki");
        assert_eq!(level_62.buff_abilities[1].name, "Rune of Death");

        let level_65 = config.profile_for_level(Some(65));
        assert_eq!(level_65.combat_abilities[2].name, "Night Fire");
        assert_eq!(level_65.combat_abilities[3].name, "Blood of Thule");
        assert_eq!(
            level_65.combat_abilities[5].name,
            "Gangrenous Touch of Zum'uul"
        );
        assert_eq!(level_65.emergency_abilities[1].name, "Death Peace");
    }

    #[test]
    fn shipped_ranger_config_tracks_level_rotation_overrides() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("config/classes/ranger.toml");
        let config = ClassConfig::load(&path).expect("ranger config should parse");

        let at_60 = config.profile_for_level(Some(60));
        assert_eq!(at_60.buff_abilities[0].name, "Call of the Predator");
        assert_eq!(at_60.combat_abilities[0].name, "Trueshot Discipline");
        assert_eq!(
            at_60.emergency_abilities[1].name,
            "Weapon Shield Discipline"
        );

        let at_61 = config.profile_for_level(Some(61));
        assert!(
            at_61
                .combat_abilities
                .iter()
                .any(|ability| ability.name == "Circle of Winter")
        );

        let at_62 = config.profile_for_level(Some(62));
        assert_eq!(at_62.buff_abilities[1].name, "Call of the Rathe");
        assert!(
            at_62
                .combat_abilities
                .iter()
                .any(|ability| ability.name == "Drifting Death")
        );
        assert_eq!(at_62.debuff_abilities[0].name, "Ensnare");

        let at_64 = config.profile_for_level(Some(64));
        assert_eq!(at_64.debuff_abilities[0].name, "Nature's Rebuke");

        let at_65 = config.profile_for_level(Some(65));
        assert_eq!(at_65.buff_abilities[0].name, "Natureskin");
        assert_eq!(at_65.combat_abilities[1].name, "Sylvan Burn");
    }

    #[test]
    fn cleric_live_config_profiles_fall_back_to_base_when_no_level_override_exists() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("config/classes/cleric.toml");
        let config = ClassConfig::load(&path).expect("load cleric class config");

        let base_profile = config.profile_for_level(None);
        assert!(
            !base_profile.combat_abilities.is_empty() || !base_profile.buff_abilities.is_empty(),
            "live cleric config should define at least one ability"
        );

        for level in [1_u8, 60, 61, 62, 65] {
            let profile = config.profile_for_level(Some(level));
            assert_eq!(
                profile.combat_abilities, base_profile.combat_abilities,
                "cleric combat profile at level {level} should fall back to base config"
            );
            assert_eq!(
                profile.buff_abilities, base_profile.buff_abilities,
                "cleric buff profile at level {level} should fall back to base config"
            );
        }
    }

}
