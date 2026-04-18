//! Admin diagnostics API — performance metrics and IPC latency for sessions.

use std::{path::Path, sync::Arc};

use axum::{
    Json, extract::Path as AxumPath, extract::State, http::StatusCode, response::IntoResponse,
};

use crate::AppState;

use super::json_error;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Diagnostics {
    pub session_id: u32,
    pub memory_mb: u64,
    pub cpu_percent: f32,
    pub ipc_latency_p50: f64,
    pub ipc_latency_p95: f64,
    pub ipc_latency_p99: f64,
    pub status: String,
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

pub async fn get_diagnostics(
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

    let _session = match sessions.iter().find(|s| s.client_id == session_id) {
        Some(s) => s,
        None => {
            return json_error(
                StatusCode::NOT_FOUND,
                format!("Session {} not found", session_id),
            )
            .into_response();
        }
    };

    let diagnostics = Diagnostics {
        session_id,
        memory_mb: 0,
        cpu_percent: 0.0,
        ipc_latency_p50: 0.0,
        ipc_latency_p95: 0.0,
        ipc_latency_p99: 0.0,
        status: "active".to_string(),
    };

    (StatusCode::OK, Json(diagnostics)).into_response()
}
