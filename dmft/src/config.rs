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
        self.accounts
            .iter()
            .filter(|a| a.group == group_id)
            .collect()
    }

    /// Find a single account by name (case-insensitive).
    pub fn find_account(&self, name: &str) -> Option<&AccountEntry> {
        let lower = name.to_lowercase();
        self.accounts
            .iter()
            .find(|a| a.name.to_lowercase() == lower)
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
    #[allow(dead_code)]
    pub group: Vec<GroupConfig>,

    /// Launch configuration for starting EQ clients
    #[serde(default)]
    #[allow(dead_code)]
    pub launch: LaunchConfig,

    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Retry / backoff configuration
    #[serde(default)]
    #[allow(dead_code)]
    pub retry: RetryConfig,

    /// Soul Engine configuration
    #[serde(default)]
    pub soul: SoulConfig,

    /// Discord integration configuration
    #[serde(default)]
    pub discord: DiscordConfig,
}

/// Discord webhook and bot configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DiscordConfig {
    /// Discord webhook URL for outbound alerts. Empty = disabled.
    pub webhook_url: String,
    /// Whether to send alerts for HVT (high-value target) detections.
    pub alert_hvt: bool,
    /// Whether to send alerts for client crashes/disconnects.
    pub alert_crashes: bool,
    /// Whether to send alerts for mass login failures.
    pub alert_mass_failures: bool,
    /// Whether to send status updates (camp started, login complete).
    pub alert_status: bool,
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self {
            webhook_url: String::new(),
            alert_hvt: true,
            alert_crashes: true,
            alert_mass_failures: true,
            alert_status: false,
        }
    }
}

#[allow(dead_code)] // Deserialized from config, consumed in later milestones
#[derive(Debug, Deserialize, Clone)]
pub struct GroupConfig {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub toon: Vec<ToonConfig>,
}

#[allow(dead_code)] // Deserialized from config, consumed in later milestones
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
            discord: DiscordConfig::default(),
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
    fn app_config_defaults() {
        let cfg = AppConfig::default_config();
        assert_eq!(cfg.process_name, "eqgame.exe");
        assert_eq!(cfg.max_spawns, 2048);
        assert!(cfg.group.is_empty());
    }

    #[test]
    fn launch_config_defaults() {
        let cfg = LaunchConfig::default();
        assert_eq!(cfg.stagger_min_secs, 3);
        assert_eq!(cfg.stagger_max_secs, 15);
        assert_eq!(cfg.max_concurrent_launches, 3);
        assert_eq!(cfg.max_working_set_mb, 800);
        assert!(cfg.launch_args.is_empty());
    }

    #[test]
    fn server_config_defaults() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.name, "Firiona Vie");
        assert!(cfg.status_url.is_none());
        assert_eq!(cfg.status_check_timeout_secs, 10);
    }

    #[test]
    fn retry_config_defaults() {
        let cfg = RetryConfig::default();
        assert_eq!(cfg.max_retries, 3);
        assert_eq!(cfg.base_backoff_secs, 30);
        assert_eq!(cfg.mass_failure_threshold, 5);
        assert_eq!(cfg.mass_failure_window_secs, 60);
    }

    #[test]
    fn app_config_minimal_toml() {
        let toml_str = r#"
            process_name = "test.exe"
            max_spawns = 100
        "#;
        let cfg: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.process_name, "test.exe");
        assert_eq!(cfg.max_spawns, 100);
        // Defaults for nested configs
        assert!(cfg.group.is_empty());
        assert_eq!(cfg.server.name, "Firiona Vie");
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

    #[test]
    fn discord_config_defaults() {
        let cfg = DiscordConfig::default();
        assert!(cfg.webhook_url.is_empty());
        assert!(cfg.alert_hvt);
        assert!(cfg.alert_crashes);
        assert!(cfg.alert_mass_failures);
        assert!(!cfg.alert_status);
    }

    #[test]
    fn accounts_for_group_returns_empty_for_no_match() {
        let cfg = parse_sample();
        let g0 = cfg.accounts_for_group(0);
        assert!(g0.is_empty());
    }

    #[test]
    fn find_account_returns_correct_entry() {
        let cfg = parse_sample();
        let acct = cfg.find_account("frostreaver02").unwrap();
        assert_eq!(acct.character, "Zisdarenu");
        assert_eq!(acct.class, "SHM");
        assert_eq!(acct.group, 1);
    }

    #[test]
    fn to_account_info_level_defaults_to_one() {
        let cfg = parse_sample();
        let info = AccountsConfig::to_account_info(&cfg.accounts[0]);
        assert_eq!(info.level, 1);
    }

    #[test]
    fn app_config_default_has_empty_eq_path() {
        let cfg = AppConfig::default_config();
        assert!(cfg.launch.eq_path.is_empty());
    }

    #[test]
    fn app_config_full_toml_parse() {
        let toml_str = r#"
            process_name = "custom.exe"
            max_spawns = 500

            [server]
            name = "TestServer"
            status_check_timeout_secs = 30

            [retry]
            max_retries = 5
            base_backoff_secs = 10
            mass_failure_threshold = 3
            mass_failure_window_secs = 120

            [launch]
            eq_path = "C:/EQ"
            stagger_min_secs = 5
            stagger_max_secs = 20
            max_concurrent_launches = 6
            max_working_set_mb = 1024

            [discord]
            webhook_url = "https://example.com/webhook"
            alert_hvt = false
            alert_crashes = true
            alert_mass_failures = false
            alert_status = true
        "#;
        let cfg: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.process_name, "custom.exe");
        assert_eq!(cfg.max_spawns, 500);
        assert_eq!(cfg.server.name, "TestServer");
        assert_eq!(cfg.server.status_check_timeout_secs, 30);
        assert_eq!(cfg.retry.max_retries, 5);
        assert_eq!(cfg.retry.base_backoff_secs, 10);
        assert_eq!(cfg.launch.eq_path, "C:/EQ");
        assert_eq!(cfg.launch.max_concurrent_launches, 6);
        assert_eq!(cfg.launch.max_working_set_mb, 1024);
        assert_eq!(cfg.discord.webhook_url, "https://example.com/webhook");
        assert!(!cfg.discord.alert_hvt);
        assert!(cfg.discord.alert_crashes);
        assert!(!cfg.discord.alert_mass_failures);
        assert!(cfg.discord.alert_status);
    }

    #[test]
    fn app_config_with_groups() {
        let toml_str = r#"
            [[group]]
            id = 1
            name = "Group Alpha"

            [[group.toon]]
            name = "Tank"
            class = "WAR"
            role = "tank"

            [[group.toon]]
            name = "Healer"
            class = "CLR"
            role = "healer"
        "#;
        let cfg: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.group.len(), 1);
        assert_eq!(cfg.group[0].id, 1);
        assert_eq!(cfg.group[0].name, "Group Alpha");
        assert_eq!(cfg.group[0].toon.len(), 2);
        assert_eq!(cfg.group[0].toon[0].name, "Tank");
        assert_eq!(cfg.group[0].toon[0].role, "tank");
    }

    #[test]
    fn toon_config_optional_fields_default() {
        let toml_str = r#"
            [[group]]
            id = 1
            name = "Test"

            [[group.toon]]
            name = "Foo"
            class = "WAR"
            role = "dps"
        "#;
        let cfg: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.group[0].toon[0].eq_window_title.is_empty());
        assert!(cfg.group[0].toon[0].account.is_none());
    }

    #[test]
    fn default_process_name_fn() {
        assert_eq!(default_process_name(), "eqgame.exe");
    }

    #[test]
    fn default_max_spawns_fn() {
        assert_eq!(default_max_spawns(), 2048);
    }
}
