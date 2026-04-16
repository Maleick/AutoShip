//! Timestamp config runtime for orchestrator-side IPC command dispatch.
//!
//! Loads per-character timestamp configuration from disk and dispatches
//! `SetChatTimestampConfig` IPC commands to active DLL clients.

use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};

use textquest_common::chat::TimestampFormat as CommonTimestampFormat;
use textquest_common::ipc::Command;

const CONFIG_RELOAD_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimestampConfig {
    pub enabled: bool,
    pub format: CommonTimestampFormat,
}

impl Default for TimestampConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            format: CommonTimestampFormat::default(),
        }
    }
}

pub fn default_config_path() -> PathBuf {
    PathBuf::from("config/timestamp.toml")
}

pub struct TimestampRuntime {
    config_path: PathBuf,
    last_check: Option<Instant>,
    last_mtime: Option<std::time::SystemTime>,
    active_configs: HashMap<String, TimestampConfig>,
}

impl Default for TimestampRuntime {
    fn default() -> Self {
        Self {
            config_path: default_config_path(),
            last_check: None,
            last_mtime: None,
            active_configs: HashMap::new(),
        }
    }
}

impl TimestampRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    fn should_check(&self) -> bool {
        match self.last_check {
            None => true,
            Some(last) => Instant::now().duration_since(last) >= CONFIG_RELOAD_INTERVAL,
        }
    }

    pub fn tick(&mut self) -> Option<HashMap<String, TimestampConfig>> {
        if !self.should_check() {
            return None;
        }
        self.last_check = Some(Instant::now());

        let current_mtime = match std::fs::metadata(&self.config_path) {
            Ok(meta) => meta.modified().ok(),
            Err(_) => None,
        };

        if current_mtime == self.last_mtime {
            return None;
        }

        self.last_mtime = current_mtime;

        match load_from_disk(&self.config_path) {
            Ok(configs) => {
                self.active_configs = configs.clone();
                Some(configs)
            }
            Err(e) => {
                tracing::debug!(error = %e, "Failed to load timestamp config");
                None
            }
        }
    }

    pub fn get_config(&self, character: &str) -> TimestampConfig {
        self.active_configs
            .get(character)
            .cloned()
            .unwrap_or_default()
    }

    pub fn apply_to_client(
        &self,
        orchestrator: &mut crate::orchestrator::Orchestrator,
        pid: u32,
        character_name: &str,
    ) {
        let config = self.get_config(character_name);
        orchestrator.send_ipc_command(
            pid,
            Command::SetChatTimestampConfig {
                enabled: config.enabled,
                format: config.format,
            },
        );
        tracing::debug!(
            pid,
            character = character_name,
            enabled = config.enabled,
            format = ?config.format,
            "Applied timestamp config to client"
        );
    }
}

fn load_from_disk(path: &PathBuf) -> Result<HashMap<String, TimestampConfig>, String> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let configs: HashMap<String, TimestampConfigFile> =
        toml::from_str(&content).map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;

    let mut result = HashMap::new();
    for (name, cfg) in configs {
        result.insert(name, TimestampConfig::from(cfg));
    }
    Ok(result)
}

#[derive(Debug, Clone, serde::Deserialize)]
struct TimestampConfigFile {
    enabled: bool,
    format: String,
}

impl From<TimestampConfigFile> for TimestampConfig {
    fn from(f: TimestampConfigFile) -> Self {
        let format = match f.format.as_str() {
            "date_time_24" => CommonTimestampFormat::DateTime24,
            "time_24" => CommonTimestampFormat::Time24,
            "date_time_12" => CommonTimestampFormat::DateTime12,
            "time_12" => CommonTimestampFormat::Time12,
            _ => CommonTimestampFormat::default(),
        };
        Self {
            enabled: f.enabled,
            format,
        }
    }
}
