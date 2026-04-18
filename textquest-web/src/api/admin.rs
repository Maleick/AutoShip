//! Admin-only API handlers backed by orchestrator-managed session snapshots.

use std::{path::Path, sync::Arc};

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

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
