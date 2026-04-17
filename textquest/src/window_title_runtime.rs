//! Window title config runtime for orchestrator-side IPC command dispatch.
//!
//! Loads per-character window title configuration from disk and dispatches
//! `SetWindowTitleConfig` IPC commands to active DLL clients.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

#[cfg(windows)]
use textquest_common::ipc::Command;
use textquest_common::{
    character_config::load_character_configs, window_title::default_window_title_format,
};

const CONFIG_RELOAD_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowTitleConfig {
    pub format: String,
}

impl Default for WindowTitleConfig {
    fn default() -> Self {
        Self {
            format: default_window_title_format(),
        }
    }
}

pub fn default_config_path() -> PathBuf {
    std::env::var("TEXTQUEST_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/textquest.toml"))
        .parent()
        .map(|parent| parent.join("character-configs.json"))
        .unwrap_or_else(|| PathBuf::from("config/character-configs.json"))
}

pub struct WindowTitleRuntime {
    config_path: PathBuf,
    last_check: Option<Instant>,
    last_mtime: Option<SystemTime>,
    active_configs: HashMap<String, WindowTitleConfig>,
}

impl Default for WindowTitleRuntime {
    fn default() -> Self {
        Self {
            config_path: default_config_path(),
            last_check: None,
            last_mtime: None,
            active_configs: HashMap::new(),
        }
    }
}

impl WindowTitleRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config_path(config_path: PathBuf) -> Self {
        Self {
            config_path,
            ..Self::default()
        }
    }

    fn should_check(&self) -> bool {
        match self.last_check {
            None => true,
            Some(last) => Instant::now().duration_since(last) >= CONFIG_RELOAD_INTERVAL,
        }
    }

    pub fn tick(&mut self) -> Option<HashMap<String, WindowTitleConfig>> {
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
            Err(error) => {
                tracing::debug!(%error, "Failed to load window title config");
                None
            }
        }
    }

    pub fn get_config(&self, character: &str) -> WindowTitleConfig {
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
        server_name: &str,
    ) {
        let config = self.get_config(character_name);
        orchestrator.send_ipc_command(
            pid,
            Command::SetWindowTitleConfig {
                format: config.format.clone(),
                server_name: server_name.to_string(),
            },
        );
        tracing::debug!(
            pid,
            character = character_name,
            server = server_name,
            format = config.format,
            "Applied window title config to client"
        );
    }
}

fn load_from_disk(path: &Path) -> Result<HashMap<String, WindowTitleConfig>, String> {
    let configs = load_character_configs(path)
        .map_err(|error| format!("Failed to load {}: {error}", path.display()))?;

    Ok(configs
        .into_iter()
        .map(|(name, cfg)| {
            (
                name,
                WindowTitleConfig {
                    format: cfg.window_title_format,
                },
            )
        })
        .collect())
}
