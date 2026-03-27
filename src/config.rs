use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    /// Name of the EQ process to attach to (default: "eqgame.exe")
    #[serde(default = "default_process_name")]
    pub process_name: String,

    /// Maximum spawns to read from the linked list (safety limit)
    #[serde(default = "default_max_spawns")]
    pub max_spawns: usize,

    /// Group definitions (optional for M1, needed for later milestones)
    #[serde(default)]
    pub group: Vec<GroupConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GroupConfig {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub toon: Vec<ToonConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ToonConfig {
    pub name: String,
    pub class: String,
    pub role: String,
    #[serde(default)]
    pub eq_window_title: String,
}

fn default_process_name() -> String {
    "eqgame.exe".to_string()
}

fn default_max_spawns() -> usize {
    2048
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        Ok(config)
    }

    pub fn default_config() -> Self {
        Self {
            process_name: default_process_name(),
            max_spawns: default_max_spawns(),
            group: Vec::new(),
        }
    }
}
