use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::soul::config::SoulConfig;

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

    /// Launch configuration for starting EQ clients
    #[serde(default)]
    pub launch: LaunchConfig,

    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Retry / backoff configuration
    #[serde(default)]
    pub retry: RetryConfig,

    /// Soul Engine configuration
    #[serde(default)]
    pub soul: SoulConfig,
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
    #[serde(default)]
    pub account: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LaunchConfig {
    pub eq_path: String,
    pub stagger_min_secs: u64,
    pub stagger_max_secs: u64,
    pub max_concurrent_launches: usize,
    pub launch_args: Vec<String>,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            eq_path: String::new(),
            stagger_min_secs: 3,
            stagger_max_secs: 15,
            max_concurrent_launches: 3,
            launch_args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub name: String,
    pub status_url: Option<String>,
    pub status_check_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "Firiona Vie".to_string(),
            status_url: None,
            status_check_timeout_secs: 10,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_backoff_secs: u64,
    pub mass_failure_threshold: u32,
    pub mass_failure_window_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_backoff_secs: 30,
            mass_failure_threshold: 5,
            mass_failure_window_secs: 60,
        }
    }
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
            launch: LaunchConfig::default(),
            server: ServerConfig::default(),
            retry: RetryConfig::default(),
            soul: SoulConfig::default(),
        }
    }
}
