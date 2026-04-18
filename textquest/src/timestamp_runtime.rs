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
#[cfg(windows)]
use textquest_common::ipc::Command;

const CONFIG_RELOAD_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimestampConfig {
    pub enabled: bool,
    pub format: CommonTimestampFormat,
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

    #[cfg(windows)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_config_file(enabled: bool, format: &str) -> TimestampConfigFile {
        TimestampConfigFile {
            enabled,
            format: format.to_string(),
        }
    }

    #[test]
    fn config_file_converts_date_time_24() {
        let cfg = TimestampConfig::from(make_config_file(true, "date_time_24"));
        assert!(cfg.enabled);
        assert_eq!(cfg.format, CommonTimestampFormat::DateTime24);
    }

    #[test]
    fn config_file_converts_time_24() {
        let cfg = TimestampConfig::from(make_config_file(false, "time_24"));
        assert!(!cfg.enabled);
        assert_eq!(cfg.format, CommonTimestampFormat::Time24);
    }

    #[test]
    fn config_file_converts_date_time_12() {
        let cfg = TimestampConfig::from(make_config_file(true, "date_time_12"));
        assert_eq!(cfg.format, CommonTimestampFormat::DateTime12);
    }

    #[test]
    fn config_file_converts_time_12() {
        let cfg = TimestampConfig::from(make_config_file(true, "time_12"));
        assert_eq!(cfg.format, CommonTimestampFormat::Time12);
    }

    #[test]
    fn config_file_unknown_format_falls_back_to_default() {
        let cfg = TimestampConfig::from(make_config_file(true, "not_a_real_format"));
        assert_eq!(cfg.format, CommonTimestampFormat::default());
    }

    #[test]
    fn get_config_returns_default_for_unknown_character() {
        let runtime = TimestampRuntime::new();
        let cfg = runtime.get_config("Aelrindel");
        assert_eq!(cfg, TimestampConfig::default());
    }

    #[test]
    fn load_from_disk_missing_file_returns_empty_map() {
        // Use a known-nonexistent path inside the system temp dir to avoid
        // platform differences between /tmp (Unix) and %TEMP% (Windows).
        let path = std::env::temp_dir().join("__tq_nonexistent_timestamp_config_file.toml");
        let result = load_from_disk(&path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn load_from_disk_valid_toml_parses_all_characters() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("timestamp.toml");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(
            file,
            r#"
[Aelrindel]
enabled = true
format = "time_24"

[Bryndas]
enabled = false
format = "date_time_12"
"#
        )
        .unwrap();

        let result = load_from_disk(&path).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result["Aelrindel"],
            TimestampConfig {
                enabled: true,
                format: CommonTimestampFormat::Time24,
            }
        );
        assert_eq!(
            result["Bryndas"],
            TimestampConfig {
                enabled: false,
                format: CommonTimestampFormat::DateTime12,
            }
        );
    }

    #[test]
    fn should_check_returns_true_when_never_run() {
        let runtime = TimestampRuntime::new();
        assert!(runtime.should_check());
    }

    #[test]
    fn tick_skips_reload_when_recently_checked() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("timestamp.toml");
        // Write initial config so tick has something to read on first call.
        std::fs::write(&path, "[Aelrindel]\nenabled = true\nformat = \"time_24\"\n").unwrap();

        let mut runtime = TimestampRuntime {
            config_path: path,
            last_check: None,
            last_mtime: None,
            active_configs: HashMap::new(),
        };

        // First tick loads config.
        let first = runtime.tick();
        assert!(first.is_some(), "first tick should load config");

        // Immediate second tick should be suppressed by the rate-limit guard.
        let second = runtime.tick();
        assert!(
            second.is_none(),
            "second tick within interval should be None"
        );
    }
}
