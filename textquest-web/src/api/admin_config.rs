//! Admin config audit API — verifies configuration consistency for sessions.

use std::{path::Path, sync::Arc};

use axum::{
    Json,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::AppState;

use super::json_error;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigAuditItem {
    pub name: String,
    pub consistent: bool,
    pub detail: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigAudit {
    pub session_id: u32,
    pub character_name: String,
    pub class_name: String,
    pub group_id: u8,
    pub items: Vec<ConfigAuditItem>,
    pub issues: Vec<String>,
}

fn read_live_sessions(
    path: &Path,
) -> anyhow::Result<Vec<textquest_common::shared_client_state::SharedClientState>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let payload = std::fs::read(path)?;
    serde_json::from_slice::<Vec<textquest_common::shared_client_state::SharedClientState>>(
        &payload,
    )
    .map_err(|e| anyhow::anyhow!("Failed to parse sessions: {}", e))
}

pub async fn audit_config(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<u32>,
) -> impl IntoResponse {
    let sessions = match read_live_sessions(&state.live_session_snapshot_path) {
        Ok(s) => s,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read session data: {}", e),
            )
            .into_response();
        }
    };

    let session = match sessions.iter().find(|s| s.client_id == session_id) {
        Some(s) => s,
        None => {
            return json_error(
                StatusCode::NOT_FOUND,
                format!("Session {} not found", session_id),
            )
            .into_response();
        }
    };

    let mut items = Vec::new();
    let issues = Vec::new();

    let char_name = session.character_name.clone();
    let class_name = format!("Class{}", session.class_id);

    items.push(ConfigAuditItem {
        name: "Character Name".to_string(),
        consistent: !char_name.is_empty(),
        detail: char_name.clone(),
    });

    items.push(ConfigAuditItem {
        name: "Class".to_string(),
        consistent: session.class_id > 0,
        detail: class_name.clone(),
    });

    items.push(ConfigAuditItem {
        name: "Group ID".to_string(),
        consistent: true,
        detail: format!("Group {}", session.client_id),
    });

    items.push(ConfigAuditItem {
        name: "Routing Scope".to_string(),
        consistent: true,
        detail: format!("{} ({})", session.status, session.zone_short_name),
    });

    let config = ConfigAudit {
        session_id,
        character_name: char_name,
        class_name,
        group_id: session.client_id as u8,
        items,
        issues,
    };

    (StatusCode::OK, Json(config)).into_response()
}
