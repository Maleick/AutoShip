use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::soul::config::SoulConfig;

// ─── Account Configuration ───────────────────────────────────────────────

/// A single account entry from config/accounts.toml.
#[derive(Debug, Deserialize, Clone)]
pub struct AccountEntry {
    pub name: String,
    pub server: String,
    pub character: String,
    #[serde(default = "default_class")]
    pub class: String,
    #[serde(default = "default_group")]
    pub group: u32,
}

fn default_class() -> String {
    "UNK".to_string()
}

fn default_group() -> u32 {
    0
}

/// Top-level wrapper for config/accounts.toml.
#[derive(Debug, Deserialize, Clone)]
pub struct AccountsConfig {
    #[serde(default)]
    pub accounts: Vec<AccountEntry>,
}

impl AccountsConfig {
    /// Load account definitions from a TOML file.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read accounts config: {}", path.display()))?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse accounts config: {}", path.display()))?;
        Ok(config)
    }

    /// Return accounts belonging to a specific group.
    pub fn accounts_for_group(&self, group_id: u32) -> Vec<&AccountEntry> {
        self.accounts.iter().filter(|a| a.group == group_id).collect()
    }

    /// Find a single account by name (case-insensitive).
    pub fn find_account(&self, name: &str) -> Option<&AccountEntry> {
        let lower = name.to_lowercase();
        self.accounts.iter().find(|a| a.name.to_lowercase() == lower)
    }

    /// Convert an AccountEntry into the AccountInfo used by the launch system.
    pub fn to_account_info(entry: &AccountEntry) -> dmft_common::login::AccountInfo {
        dmft_common::login::AccountInfo {
            account_name: entry.name.clone(),
            character_name: entry.character.clone(),
            class_name: entry.class.clone(),
            level: 1,
            group_id: entry.group,
            server_name: entry.server.clone(),
        }
    }
}

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
    /// Maximum physical RAM (working set) per EQ client in MB. 0 = unlimited.
    pub max_working_set_mb: u32,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            eq_path: String::new(),
            stagger_min_secs: 3,
            stagger_max_secs: 15,
            max_concurrent_launches: 3,
            launch_args: Vec::new(),
            max_working_set_mb: 800,
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ACCOUNTS_TOML: &str = r#"
[[accounts]]
name = "frostreaver01"
server = "Firiona Vie"
character = "Camrene"
class = "WAR"
group = 1

[[accounts]]
name = "frostreaver02"
server = "Firiona Vie"
character = "Zisdarenu"
class = "SHM"
group = 1

[[accounts]]
name = "frostreaver07"
server = "Firiona Vie"
character = "Paladin"
class = "PAL"
group = 2
"#;

    fn parse_sample() -> AccountsConfig {
        toml::from_str(SAMPLE_ACCOUNTS_TOML).unwrap()
    }

    #[test]
    fn parse_accounts_toml() {
        let cfg = parse_sample();
        assert_eq!(cfg.accounts.len(), 3);
        assert_eq!(cfg.accounts[0].name, "frostreaver01");
        assert_eq!(cfg.accounts[0].character, "Camrene");
        assert_eq!(cfg.accounts[0].class, "WAR");
        assert_eq!(cfg.accounts[0].group, 1);
        assert_eq!(cfg.accounts[0].server, "Firiona Vie");
    }

    #[test]
    fn accounts_for_group_filters_correctly() {
        let cfg = parse_sample();
        let g1 = cfg.accounts_for_group(1);
        assert_eq!(g1.len(), 2);
        assert!(g1.iter().all(|a| a.group == 1));

        let g2 = cfg.accounts_for_group(2);
        assert_eq!(g2.len(), 1);
        assert_eq!(g2[0].name, "frostreaver07");

        let g99 = cfg.accounts_for_group(99);
        assert!(g99.is_empty());
    }

    #[test]
    fn find_account_case_insensitive() {
        let cfg = parse_sample();
        assert!(cfg.find_account("frostreaver01").is_some());
        assert!(cfg.find_account("FROSTREAVER01").is_some());
        assert!(cfg.find_account("Frostreaver01").is_some());
        assert!(cfg.find_account("nonexistent").is_none());
    }

    #[test]
    fn to_account_info_conversion() {
        let cfg = parse_sample();
        let info = AccountsConfig::to_account_info(&cfg.accounts[0]);
        assert_eq!(info.account_name, "frostreaver01");
        assert_eq!(info.character_name, "Camrene");
        assert_eq!(info.class_name, "WAR");
        assert_eq!(info.group_id, 1);
        assert_eq!(info.server_name, "Firiona Vie");
    }

    #[test]
    fn empty_accounts_toml() {
        let cfg: AccountsConfig = toml::from_str("").unwrap();
        assert!(cfg.accounts.is_empty());
    }

    #[test]
    fn defaults_for_optional_fields() {
        let cfg: AccountsConfig = toml::from_str(
            r#"
[[accounts]]
name = "test"
server = "Test"
character = "Foo"
"#,
        )
        .unwrap();
        assert_eq!(cfg.accounts[0].class, "UNK");
        assert_eq!(cfg.accounts[0].group, 0);
    }

    #[test]
    fn load_real_accounts_toml() {
        let path = std::path::Path::new("config/accounts.toml");
        if path.exists() {
            let cfg = AccountsConfig::load(path).unwrap();
            assert!(!cfg.accounts.is_empty());
            // Every account should have a non-empty name
            for acct in &cfg.accounts {
                assert!(!acct.name.is_empty());
                assert!(!acct.server.is_empty());
            }
        }
    }
}
