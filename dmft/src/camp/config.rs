//! Camp configuration — loadable from TOML files in `config/camps/`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Configuration for a single XP camp location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampConfig {
    pub name: String,
    pub zone: String,
    pub camp_center: [f32; 3],
    pub pull_point: [f32; 3],
    pub pull_radius: f32,
    pub camp_radius: f32,
    pub leash_radius: f32,
    pub rest_mana_pct: u8,
    pub pull_mana_pct: u8,
    pub level_range: [u8; 2],
    /// Mob names to pull. Empty means pull anything in range.
    #[serde(default)]
    pub pull_mob_names: Vec<String>,
}

impl CampConfig {
    /// Directory where camp configs are stored.
    fn camps_dir() -> PathBuf {
        Path::new("config/camps").to_path_buf()
    }

    /// Save this camp config to `config/camps/{name}.toml`.
    pub fn save(&self) -> Result<()> {
        let dir = Self::camps_dir();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create camps directory: {}", dir.display()))?;

        let path = dir.join(format!("{}.toml", self.name));
        let toml_str = toml::to_string_pretty(self)
            .context("Failed to serialize camp config")?;

        std::fs::write(&path, toml_str)
            .with_context(|| format!("Failed to write camp config: {}", path.display()))?;

        Ok(())
    }

    /// Load a camp config from `config/camps/{name}.toml`.
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

        let loaded: CampConfig = toml::from_str(
            &std::fs::read_to_string(&path).unwrap(),
        )
        .unwrap();
        assert_eq!(loaded.name, "crushbone_entrance");
        assert_eq!(loaded.camp_center, [100.0, 200.0, 0.0]);
        assert_eq!(loaded.rest_mana_pct, 60);
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
}
