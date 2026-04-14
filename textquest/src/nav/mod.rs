//! Orchestrator-side navigation — routing, recording, camp management, navmesh pathfinding.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::ipc;

pub mod camp;
pub mod mesh;
pub mod recorder;
pub mod router;
pub mod stuck_detection;
pub mod zone_transition;

const NAV_RELOAD_SHARED_STATE_TIMEOUT: Duration = Duration::from_millis(1200);

fn read_shared_state_with_retry(
    reader: &mut ipc::shared::SharedStateReader,
    timeout: Duration,
) -> Option<textquest_common::types::GameState> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(state) = reader.read() {
            return Some(state);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn navmesh_zone_for_pid(pid: u32) -> Result<String> {
    let token = ipc::load_session_token(pid).ok_or_else(|| {
        anyhow::anyhow!(
            "No session token for PID {pid}. Inject the DLL first to create authenticated IPC state."
        )
    })?;
    let session_id = textquest_common::ipc::session_id_from_token(&token);
    let mut reader = ipc::shared::SharedStateReader::new(pid, session_id).with_context(|| {
        format!("Cannot open shared memory for PID {pid} — is the DLL injected?")
    })?;
    let state = read_shared_state_with_retry(&mut reader, NAV_RELOAD_SHARED_STATE_TIMEOUT)
        .ok_or_else(|| anyhow::anyhow!("No shared memory data available for PID {pid}"))?;
    if state.zone_short_name.is_empty() {
        anyhow::bail!("PID {pid} is not in a zone yet; no zone short name is available");
    }
    Ok(state.zone_short_name)
}

fn is_nav_reload_command(command: &str) -> bool {
    let mut tokens = command.split_ascii_whitespace();
    matches!(
        (tokens.next(), tokens.next(), tokens.next()),
        (Some(first), Some(second), None)
            if first.eq_ignore_ascii_case("/nav") && second.eq_ignore_ascii_case("reload")
    )
}

pub(crate) fn try_handle_local_slash_command(pid: u32, command: &str) -> Result<Option<String>> {
    if !is_nav_reload_command(command) {
        return Ok(None);
    }

    let zone = navmesh_zone_for_pid(pid)?;
    let reload = mesh::reload_zone_mesh(&zone)
        .with_context(|| format!("Failed to reload navmesh for zone '{zone}'"))?;
    let cache_status = if reload.replaced_cached_file {
        "replaced cached file"
    } else {
        "downloaded fresh cache"
    };

    Ok(Some(format!(
        "Reloaded navmesh for '{}' ({cache_status}, {} bytes, {} overlay segments)",
        reload.zone_short_name, reload.cache_bytes, reload.overlay_segment_count
    )))
}

#[cfg(test)]
mod tests {
    use super::is_nav_reload_command;

    #[test]
    fn nav_reload_command_matcher_accepts_exact_command() {
        assert!(is_nav_reload_command("/nav reload"));
        assert!(is_nav_reload_command("  /NAV   reload  "));
    }

    #[test]
    fn nav_reload_command_matcher_rejects_other_commands() {
        assert!(!is_nav_reload_command("/nav target"));
        assert!(!is_nav_reload_command("/nav reload now"));
        assert!(!is_nav_reload_command("/say /nav reload"));
    }
}
