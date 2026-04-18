//! Auto-group config persistence and orchestrator runtime.
//!
//! The web dashboard persists operator-edited profiles to a sidecar TOML file,
//! while the Windows orchestrator watches the same file and translates desired
//! group membership into live `/invite`, `/accept`, and `/grouproles` commands.

use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use textquest_common::auto_group::AutoGroupSettings;

pub fn default_config_path() -> PathBuf {
    std::env::var("TEXTQUEST_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/textquest.toml"))
        .parent()
        .map(|parent| parent.join("auto-group.toml"))
        .unwrap_or_else(|| PathBuf::from("config/auto-group.toml"))
}

/// Load persisted auto-group settings from the supplied TOML file.
///
/// Missing files are treated as an empty settings document.
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed.
pub fn load_settings_from_path(path: &Path) -> Result<AutoGroupSettings> {
    if !path.exists() {
        return Ok(AutoGroupSettings::default());
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read auto-group config {}", path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("Failed to parse auto-group config {}", path.display()))
}

/// Persist auto-group settings to the supplied TOML file atomically.
///
/// # Errors
///
/// Returns an error if the config directory cannot be created, the settings
/// cannot be serialized, or the file cannot be replaced.
pub fn write_settings_to_path(path: &Path, settings: &AutoGroupSettings) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create auto-group config directory {}",
                parent.display()
            )
        })?;
    }

    let contents = toml::to_string_pretty(settings)
        .context("Failed to serialize auto-group settings to TOML")?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to derive temp file name for {}", path.display()))?;
    let temp_path = path.with_file_name(format!(
        ".{file_name}.tmp.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    let mut temp_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .with_context(|| format!("Failed to create temp config {}", temp_path.display()))?;
    temp_file
        .write_all(contents.as_bytes())
        .with_context(|| format!("Failed to write temp config {}", temp_path.display()))?;
    temp_file
        .sync_all()
        .with_context(|| format!("Failed to sync temp config {}", temp_path.display()))?;
    drop(temp_file);

    replace_config_file(&temp_path, path)?;

    Ok(())
}

fn replace_config_file(temp_path: &Path, path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        replace_config_file_windows(temp_path, path)
    }

    #[cfg(not(windows))]
    {
        std::fs::rename(temp_path, path).with_context(|| {
            format!(
                "Failed to replace auto-group config {} with {}",
                path.display(),
                temp_path.display()
            )
        })
    }
}

#[cfg(windows)]
fn replace_config_file_windows(temp_path: &Path, path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows::{
        Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        },
        core::PCWSTR,
    };

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    let source = wide(temp_path);
    let destination = wide(path);

    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .with_context(|| {
        format!(
            "Failed to replace auto-group config {} with {}",
            path.display(),
            temp_path.display()
        )
    })
}

#[cfg(windows)]
mod windows_runtime {
    use std::{
        collections::{BTreeMap, BTreeSet},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use textquest_common::{
        auto_group::{
            AutoGroupCommand, AutoGroupCommandPhase, AutoGroupController, AutoGroupObservation,
        },
        ipc::Command,
    };

    use super::{default_config_path, load_settings_from_path};
    use crate::{
        client::{manager::ClientManager, session::EqSession},
        orchestrator::Orchestrator,
        process::memory::ProcessHandle,
    };

    const CONFIG_RELOAD_INTERVAL: Duration = Duration::from_secs(2);

    pub struct AutoGroupRuntime {
        config_path: std::path::PathBuf,
        last_check: Option<Instant>,
        last_mtime: Option<SystemTime>,
        settings: textquest_common::auto_group::AutoGroupSettings,
        controller: AutoGroupController,
    }

    impl Default for AutoGroupRuntime {
        fn default() -> Self {
            Self {
                config_path: default_config_path(),
                last_check: None,
                last_mtime: None,
                settings: textquest_common::auto_group::AutoGroupSettings::default(),
                controller: AutoGroupController::default(),
            }
        }
    }

    impl AutoGroupRuntime {
        pub fn new() -> Self {
            Self::default()
        }

        fn should_check(&self) -> bool {
            match self.last_check {
                None => true,
                Some(last) => Instant::now().duration_since(last) >= CONFIG_RELOAD_INTERVAL,
            }
        }

        fn refresh_settings_if_needed(&mut self) {
            if !self.should_check() {
                return;
            }
            self.last_check = Some(Instant::now());

            let current_mtime = std::fs::metadata(&self.config_path)
                .ok()
                .and_then(|metadata| metadata.modified().ok());
            if current_mtime == self.last_mtime {
                return;
            }
            self.last_mtime = current_mtime;

            match load_settings_from_path(&self.config_path) {
                Ok(settings) => {
                    self.settings = settings;
                    self.controller = AutoGroupController::default();
                    tracing::debug!(
                        path = %self.config_path.display(),
                        groups = self.settings.groups.len(),
                        "Reloaded auto-group config"
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        %error,
                        path = %self.config_path.display(),
                        "Failed to reload auto-group config"
                    );
                }
            }
        }

        pub fn tick(&mut self, client_manager: &ClientManager, orchestrator: &mut Orchestrator) {
            self.refresh_settings_if_needed();
            if self.settings.groups.is_empty() {
                return;
            }

            let active_sessions = active_sessions(client_manager);
            if active_sessions.is_empty() {
                return;
            }

            let observation = build_observation(&active_sessions);
            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            let commands = self.controller.tick(&self.settings, &observation, now_secs);
            if commands.is_empty() {
                return;
            }

            let pids_by_character = active_sessions
                .iter()
                .map(|session| (normalize_name(&session.character_name), session.pid))
                .collect::<BTreeMap<_, _>>();

            for command in commands {
                dispatch_command(orchestrator, &pids_by_character, command);
            }
        }
    }

    #[derive(Debug, Clone)]
    struct ActiveSession {
        pid: u32,
        character_name: String,
    }

    fn active_sessions(client_manager: &ClientManager) -> Vec<ActiveSession> {
        client_manager
            .all_sessions()
            .filter(|session| session.is_active())
            .filter_map(|session| {
                active_character_name(session).map(|character_name| ActiveSession {
                    pid: session.pid,
                    character_name,
                })
            })
            .collect()
    }

    fn active_character_name(session: &EqSession) -> Option<String> {
        session.character_name.clone().or_else(|| {
            session
                .bound_toon
                .as_ref()
                .map(|toon| toon.character_name.clone())
        })
    }

    fn build_observation(active_sessions: &[ActiveSession]) -> AutoGroupObservation {
        let mut observation = AutoGroupObservation {
            live_characters: active_sessions
                .iter()
                .map(|session| session.character_name.clone())
                .collect::<BTreeSet<_>>(),
            leader_group_members: BTreeMap::new(),
        };

        for session in active_sessions {
            let Ok(process) = ProcessHandle::open(session.pid) else {
                tracing::debug!(pid = session.pid, "Failed to open process for auto-group");
                continue;
            };
            let Ok(eq_base) = crate::get_module_base(&process) else {
                tracing::debug!(
                    pid = session.pid,
                    "Failed to resolve eq base for auto-group"
                );
                continue;
            };
            let Ok(group_info) = crate::eq::spawn::read_group_info(&process, eq_base) else {
                tracing::debug!(
                    pid = session.pid,
                    "Failed to read group info for auto-group"
                );
                continue;
            };
            let Some(group_info) = group_info else {
                continue;
            };
            let crate::eq::structs::GroupInfo {
                leader_name,
                members,
                ..
            } = group_info;
            if leader_name.trim().is_empty() || members.is_empty() {
                continue;
            }

            observation
                .leader_group_members
                .entry(leader_name)
                .or_insert(members);
        }

        observation
    }

    fn dispatch_command(
        orchestrator: &mut Orchestrator,
        pids_by_character: &BTreeMap<String, u32>,
        command: AutoGroupCommand,
    ) {
        let Some(&pid) = pids_by_character.get(&normalize_name(&command.character_name)) else {
            tracing::warn!(
                character = %command.character_name,
                phase = ?command.phase,
                command = %command.command,
                "Auto-group skipped command because target character is not active"
            );
            return;
        };

        let ipc_command = Command::SlashCommand {
            command: command.command.clone(),
        };
        let dispatched = orchestrator.send_ipc_command(pid, ipc_command);
        if dispatched {
            tracing::debug!(
                pid,
                character = %command.character_name,
                phase = ?command.phase,
                command = %command.command,
                "Auto-group command dispatched"
            );
        } else {
            let phase = match command.phase {
                AutoGroupCommandPhase::Invite => "invite",
                AutoGroupCommandPhase::AcceptInvite => "accept_invite",
                AutoGroupCommandPhase::AssignRole => "assign_role",
                AutoGroupCommandPhase::CompletionCommand => "completion",
            };
            tracing::warn!(
                pid,
                character = %command.character_name,
                phase,
                command = %command.command,
                "Auto-group command dispatch failed"
            );
        }
    }

    fn normalize_name(name: &str) -> String {
        name.trim().to_ascii_lowercase()
    }
}

#[cfg(windows)]
pub use windows_runtime::AutoGroupRuntime;

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use textquest_common::auto_group::{
        AutoGroupMember, AutoGroupProfile, AutoGroupRole, AutoGroupSettings,
    };

    use super::{load_settings_from_path, write_settings_to_path};

    fn sample_settings() -> AutoGroupSettings {
        AutoGroupSettings {
            groups: vec![AutoGroupProfile {
                name: "Fire Team".into(),
                leader_name: "Alpha".into(),
                enabled: true,
                invite_retry_interval_secs: 5,
                max_invite_retries: 4,
                completion_command: Some("/say group ready".into()),
                members: vec![
                    AutoGroupMember {
                        name: "Alpha".into(),
                        role: AutoGroupRole::None,
                    },
                    AutoGroupMember {
                        name: "Bravo".into(),
                        role: AutoGroupRole::MainTank,
                    },
                ],
            }],
        }
    }

    #[test]
    fn load_settings_defaults_when_file_is_missing() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("auto-group.toml");

        let settings = load_settings_from_path(&path).expect("missing config should default");

        assert_eq!(settings, AutoGroupSettings::default());
    }

    #[test]
    fn write_settings_round_trips() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("auto-group.toml");
        let settings = sample_settings();

        write_settings_to_path(&path, &settings).expect("write settings");
        let loaded = load_settings_from_path(&path).expect("reload settings");

        assert_eq!(loaded, settings);
    }

    #[test]
    fn write_settings_replaces_existing_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("auto-group.toml");
        let initial = sample_settings();
        let mut updated = sample_settings();
        updated.groups[0].completion_command = Some("/say updated".into());
        updated.groups[0].members.push(AutoGroupMember {
            name: "Charlie".into(),
            role: AutoGroupRole::MainAssist,
        });

        write_settings_to_path(&path, &initial).expect("write initial settings");
        write_settings_to_path(&path, &updated).expect("replace settings");

        let loaded = load_settings_from_path(&path).expect("reload replaced settings");
        assert_eq!(loaded, updated);
    }
}
