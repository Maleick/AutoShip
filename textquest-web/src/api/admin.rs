//! Admin-only API handlers backed by orchestrator-managed session snapshots.

use std::{path::Path, sync::Arc};

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse, routing::get, Router};
use serde::Serialize;

use crate::AppState;

use super::json_error;

/// Stable lifecycle label persisted by the orchestrator for admin inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum AdminSessionLifecycle {
    Active,
    Paused,
    Error,
}

/// Stable routing-scope kind persisted by the orchestrator for admin
/// inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum AdminRoutingScopeKind {
    OneToon,
    Group,
    AllSession,
}

/// Admin-safe routing summary for one managed session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct AdminRoutingScopeSnapshot {
    kind: AdminRoutingScopeKind,
    label: String,
    group_id: Option<u8>,
    toon_name: Option<String>,
}

/// Stable admin snapshot for one orchestrator-managed session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct AdminSessionSnapshot {
    session_id: u32,
    character_name: Option<String>,
    class_name: Option<String>,
    group_id: u8,
    routing_scope: AdminRoutingScopeSnapshot,
    lifecycle_state: AdminSessionLifecycle,
}

fn read_admin_sessions(path: &Path) -> anyhow::Result<Vec<AdminSessionSnapshot>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let payload = std::fs::read(path)?;
    let sessions = serde_json::from_slice::<Vec<AdminSessionSnapshot>>(&payload)?;
    Ok(sessions)
}

/// List orchestrator-managed session inventory for the admin dashboard.
pub async fn list_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match read_admin_sessions(&state.admin_session_snapshot_path) {
        Ok(sessions) => (StatusCode::OK, Json(sessions)).into_response(),
        Err(error) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read admin session inventory: {error}"),
        )
        .into_response(),
    }
}

/// Admin session record for inventory and monitoring.
#[derive(Debug, Clone, Serialize)]
pub struct AdminSessionRecord {
    pub session_id: String,
    pub character_name: String,
    pub profile: Option<String>,
    pub group_id: Option<String>,
    pub routing_scope: Option<String>,
    pub lifecycle: Option<String>,
    pub status: Option<String>,
    pub zone: Option<String>,
    pub level: Option<u32>,
    pub class_name: Option<String>,
    pub last_heartbeat: Option<String>,
}

/// Get all managed sessions for the admin dashboard.
///
/// Returns session inventory from the live snapshot, enriched with
/// configuration and profile metadata. Falls back to configured sessions
/// if no live snapshot exists yet.
pub async fn list_admin_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Try to read live session snapshot first
    if let Ok(live_sessions) = super::read_live_sessions(&state.live_session_snapshot_path) {
        if !live_sessions.is_empty() {
            let sessions: Vec<AdminSessionRecord> = live_sessions
                .into_iter()
                .map(|session| AdminSessionRecord {
                    session_id: format!("session-{}", session.client_id),
                    character_name: session.character_name,
                    profile: None, // TODO: enrich from profile registry
                    group_id: None, // TODO: enrich from group assignments
                    routing_scope: None, // TODO: enrich from routing config
                    lifecycle: Some(session.status.clone()),
                    status: Some(session.status),
                    zone: if session.zone_long_name.is_empty() {
                        Some(session.zone_short_name)
                    } else {
                        Some(session.zone_long_name)
                    },
                    level: Some(session.level as u32),
                    class_name: None, // TODO: enrich from character config
                    last_heartbeat: None, // TODO: track heartbeat timestamps
                })
                .collect();
            return Json(sessions).into_response();
        }
    }

    // Fallback: return configured sessions as placeholders
    let configs = state.character_configs.read().await;
    let sessions: Vec<AdminSessionRecord> = configs
        .iter()
        .enumerate()
        .map(|(idx, (_name, cfg))| AdminSessionRecord {
            session_id: format!("session-{}", idx + 1),
            character_name: cfg.character_name.clone(),
            profile: None,
            group_id: None,
            routing_scope: None,
            lifecycle: Some("idle".to_string()),
            status: Some("offline".to_string()),
            zone: None,
            level: None,
            class_name: None,
            last_heartbeat: None,
        })
        .collect();

    Json(sessions).into_response()
}

/// Build the admin sub-router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/sessions", get(list_admin_sessions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_admin_sessions_returns_empty_inventory_when_file_is_missing() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let missing = tempdir.path().join("missing-admin-sessions.json");

        let sessions = read_admin_sessions(&missing).expect("missing file should be empty");

        assert!(sessions.is_empty());
    }

    #[test]
    fn read_admin_sessions_parses_persisted_inventory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let snapshot_path = tempdir.path().join("admin-sessions.json");
        std::fs::write(
            &snapshot_path,
            serde_json::to_vec(&vec![serde_json::json!({
                "session_id": 4242,
                "character_name": "Cleric42",
                "class_name": "Cleric",
                "group_id": 2,
                "routing_scope": {
                    "kind": "group",
                    "label": "G2",
                    "group_id": 2,
                    "toon_name": null
                },
                "lifecycle_state": "paused"
            })])
            .expect("admin snapshot json"),
        )
        .expect("write snapshot");

        let sessions = read_admin_sessions(&snapshot_path).expect("snapshot should parse");

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, 4242);
        assert_eq!(sessions[0].character_name.as_deref(), Some("Cleric42"));
        assert_eq!(sessions[0].class_name.as_deref(), Some("Cleric"));
        assert_eq!(sessions[0].group_id, 2);
        assert_eq!(sessions[0].routing_scope.kind, AdminRoutingScopeKind::Group);
    }
}
