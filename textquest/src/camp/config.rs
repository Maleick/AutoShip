//! Camp configuration — loadable from TOML files in `config/camps/`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use textquest_common::combat::PullMode;

/// Configuration for a single XP camp location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampConfig {
    /// Camp name used as the file stem (e.g., `crushbone_entrance`).
    pub name: String,
    /// Zone short name (e.g., `crushbone`).
    pub zone: String,
    /// XYZ coordinates of the camp anchor point.
    pub camp_center: [f32; 3],
    /// XYZ coordinates where the puller brings mobs.
    pub pull_point: [f32; 3],
    /// Maximum distance from pull point to engage mobs.
    pub pull_radius: f32,
    /// Radius around camp center that members should stay within.
    pub camp_radius: f32,
    /// Maximum distance a pulled mob can go before being abandoned.
    pub leash_radius: f32,
    /// Mana percentage threshold to sit and med between pulls.
    pub rest_mana_pct: u8,
    /// Minimum mana percentage required before pulling the next mob.
    pub pull_mana_pct: u8,
    /// Min and max level range for this camp (used for progression).
    pub level_range: [u8; 2],
    /// Mob names to pull. Empty means pull anything in range.
    #[serde(default)]
    pub pull_mob_names: Vec<String>,
    /// Mob names to never pull (named mobs, quest NPCs, etc.).
    #[serde(default)]
    pub ignore_mob_names: Vec<String>,
    /// Mob names to burn down immediately when spotted (named/rare spawns).
    #[serde(default)]
    pub burn_mob_names: Vec<String>,
    /// Suppress return-to-camp movement while the character has aggro.
    /// MQ2MoveUtils equivalent: `/makecamp returnnoaggro`.
    #[serde(default)]
    pub return_no_aggro: bool,
    /// Camp file name to progress to when the group outlevels this camp.
    #[serde(default)]
    pub next_camp: Option<String>,
    /// Camp file name to fall back to (reverse progression).
    #[serde(default)]
    pub prev_camp: Option<String>,
    /// Pull strategy mode — controls macro-level pull behavior (gap #2).
    /// Switchable at runtime via web UI or `set_peer`.
    #[serde(default)]
    pub pull_mode: PullMode,
}

impl CampConfig {
    /// Directory where camp configs are stored.
    fn camps_dir() -> PathBuf {
        Path::new("config/camps").to_path_buf()
    }

    /// Save this camp config to `config/camps/{name}.toml`.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn save(&self) -> Result<()> {
        let dir = Self::camps_dir();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create camps directory: {}", dir.display()))?;

        let path = dir.join(format!("{}.toml", self.name));
        let toml_str = toml::to_string_pretty(self).context("Failed to serialize camp config")?;

        std::fs::write(&path, toml_str)
            .with_context(|| format!("Failed to write camp config: {}", path.display()))?;

        Ok(())
    }

    /// Load a camp config from `config/camps/{name}.toml`.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn load(name: &str) -> Result<Self> {
        let path = Self::camps_dir().join(format!("{name}.toml"));
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read camp config: {}", path.display()))?;

        let config: Self = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse camp config: {}", path.display()))?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> CampConfig {
        CampConfig {
            name: "crushbone_entrance".into(),
            zone: "crushbone".into(),
            camp_center: [100.0, 200.0, 0.0],
            pull_point: [150.0, 250.0, 0.0],
            pull_radius: 200.0,
            camp_radius: 30.0,
            leash_radius: 100.0,
            rest_mana_pct: 60,
            pull_mana_pct: 30,
            level_range: [5, 12],
            pull_mob_names: vec!["an orc pawn".into(), "an orc centurion".into()],
            ignore_mob_names: vec!["Ambassador DVinn".into()],
            burn_mob_names: vec!["Emperor Crush".into()],
            return_no_aggro: false,
            next_camp: Some("crushbone_throne".into()),
            prev_camp: None,
        }
    }

    #[test]
    fn test_roundtrip_toml() {
        let config = sample_config();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let loaded: CampConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(loaded.name, config.name);
        assert_eq!(loaded.zone, config.zone);
        assert_eq!(loaded.pull_radius, config.pull_radius);
        assert_eq!(loaded.level_range, config.level_range);
        assert_eq!(loaded.pull_mob_names.len(), 2);
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let camps_dir = dir.path().join("config/camps");
        std::fs::create_dir_all(&camps_dir).unwrap();

        let config = sample_config();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let path = camps_dir.join("crushbone_entrance.toml");
        std::fs::write(&path, &toml_str).unwrap();

        let loaded: CampConfig = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.name, "crushbone_entrance");
        assert_eq!(loaded.camp_center, [100.0, 200.0, 0.0]);
        assert_eq!(loaded.rest_mana_pct, 60);
    }

    #[test]
    fn test_ignore_and_burn_defaults_empty() {
        let toml_str = r#"
            name = "test"
            zone = "gfay"
            camp_center = [0.0, 0.0, 0.0]
            pull_point = [10.0, 10.0, 0.0]
            pull_radius = 100.0
            camp_radius = 20.0
            leash_radius = 80.0
            rest_mana_pct = 50
            pull_mana_pct = 20
            level_range = [1, 5]
        "#;
        let config: CampConfig = toml::from_str(toml_str).unwrap();
        assert!(config.ignore_mob_names.is_empty());
        assert!(config.burn_mob_names.is_empty());
        assert!(config.next_camp.is_none());
        assert!(config.prev_camp.is_none());
    }

    #[test]
    fn test_next_prev_camp_links() {
        let config = sample_config();
        assert_eq!(config.next_camp.as_deref(), Some("crushbone_throne"));
        assert!(config.prev_camp.is_none());
    }

    #[test]
    fn test_camp_center_and_pull_point() {
        let config = sample_config();
        assert_eq!(config.camp_center, [100.0, 200.0, 0.0]);
        assert_eq!(config.pull_point, [150.0, 250.0, 0.0]);
    }

    #[test]
    fn test_radiuses() {
        let config = sample_config();
        assert!(config.pull_radius > config.camp_radius);
        assert!(config.leash_radius > config.camp_radius);
    }

    #[test]
    fn test_empty_pull_mob_names_default() {
        let toml_str = r#"
            name = "test"
            zone = "gfay"
            camp_center = [0.0, 0.0, 0.0]
            pull_point = [10.0, 10.0, 0.0]
            pull_radius = 100.0
            camp_radius = 20.0
            leash_radius = 80.0
            rest_mana_pct = 50
            pull_mana_pct = 20
            level_range = [1, 5]
        "#;
        let config: CampConfig = toml::from_str(toml_str).unwrap();
        assert!(config.pull_mob_names.is_empty());
    }

    #[test]
    fn test_serialize_deserialize_burn_mobs() {
        let mut config = sample_config();
        config.burn_mob_names = vec!["Emperor Crush".into(), "King Doric".into()];
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let loaded: CampConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(loaded.burn_mob_names.len(), 2);
        assert!(loaded.burn_mob_names.contains(&"Emperor Crush".to_string()));
        assert!(loaded.burn_mob_names.contains(&"King Doric".to_string()));
    }

    #[test]
    fn test_mana_pct_values() {
        let config = sample_config();
        assert_eq!(config.rest_mana_pct, 60);
        assert_eq!(config.pull_mana_pct, 30);
        assert!(config.rest_mana_pct > config.pull_mana_pct);
    }

    #[test]
    fn test_level_range_ordering() {
        let config = sample_config();
        assert!(config.level_range[0] <= config.level_range[1]);
    }

    #[test]
    fn test_prev_camp_chain() {
        let mut config = sample_config();
        config.prev_camp = Some("crushbone_entrance".into());
        config.next_camp = Some("crushbone_inner".into());
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let loaded: CampConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(loaded.prev_camp.as_deref(), Some("crushbone_entrance"));
        assert_eq!(loaded.next_camp.as_deref(), Some("crushbone_inner"));
    }

    #[test]
    fn test_ignore_mob_names_roundtrip() {
        let config = sample_config();
        assert_eq!(config.ignore_mob_names, vec!["Ambassador DVinn"]);
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let loaded: CampConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn test_camps_dir_path() {
        let dir = CampConfig::camps_dir();
        assert!(dir.ends_with("config/camps"));
    }

    #[test]
    fn test_zero_radius_config() {
        let toml_str = r#"
            name = "tiny_camp"
            zone = "arena"
            camp_center = [0.0, 0.0, 0.0]
            pull_point = [0.0, 0.0, 0.0]
            pull_radius = 0.0
            camp_radius = 0.0
            leash_radius = 0.0
            rest_mana_pct = 0
            pull_mana_pct = 0
            level_range = [1, 1]
        "#;
        let config: CampConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.pull_radius, 0.0);
        assert_eq!(config.camp_radius, 0.0);
        assert_eq!(config.rest_mana_pct, 0);
    }

    #[test]
    fn test_checked_in_sebilis_disco_config_parses() {
        let path = crate::paths::data_dir().join("config/camps/sebilis_disco.toml");
        let contents = std::fs::read_to_string(&path).unwrap();
        let config: CampConfig = toml::from_str(&contents).unwrap();

        assert_eq!(config.name, "sebilis_disco");
        assert_eq!(config.zone, "sebilis");
        assert!(config.return_no_aggro);
        assert!(config.pull_radius > config.camp_radius);
        assert!(!config.pull_mob_names.is_empty());
        assert!(!config.burn_mob_names.is_empty());
        assert_eq!(config.prev_camp.as_deref(), Some("lguk_dead_side"));
    }
}
