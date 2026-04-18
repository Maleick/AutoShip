//! Admin API endpoints for session lifecycle control.
//!
//! Provides REST endpoints to start, stop, and restart sessions:
//! - POST /api/admin/sessions/{id}/start
//! - POST /api/admin/sessions/{id}/stop
//! - POST /api/admin/sessions/{id}/restart

use axum::{
    Json,
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

// ─── Request/Response Types ────────────────────────────────────────────────────

/// Request body for session lifecycle operations (currently unused but reserved
/// for future per-operation configuration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLifecycleRequest {
    /// Optional timeout in seconds for the operation.
    pub timeout_secs: Option<u32>,
}

/// Response for successful session lifecycle operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLifecycleResponse {
    /// The numeric session ID.
    pub session_id: u32,
    /// The operation that was executed (e.g., "start", "stop", "restart").
    pub operation: String,
    /// Human-readable status message.
    pub message: String,
}

/// Error response for failed operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

fn json_error(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}

// ─── Session Lifecycle Handlers ────────────────────────────────────────────────

/// Start a session by ID.
///
/// Emits a "session:start:{id}" event on the broadcast channel that the
/// orchestrator subscribes to. Returns 404 if the session ID does not exist
/// (based on live session snapshot).
pub async fn start_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return json_error(
            StatusCode::FORBIDDEN,
            "Forbidden: untrusted origin for session lifecycle mutation",
        )
        .into_response();
    }

    // Validate that the session exists by reading live sessions
    if let Err(e) = validate_session_exists(&state, id).await {
        return e.into_response();
    }

    // Emit event for orchestrator to handle
    let event = format!("session:start:{}", id);
    let _ = state.event_tx.send(event);

    let response = SessionLifecycleResponse {
        session_id: id,
        operation: "start".to_string(),
        message: format!("Start request queued for session {}", id),
    };

    (StatusCode::ACCEPTED, Json(response)).into_response()
}

/// Stop a session by ID.
///
/// Emits a "session:stop:{id}" event on the broadcast channel that the
/// orchestrator subscribes to. Returns 404 if the session ID does not exist.
pub async fn stop_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
) -> impl IntoResponse {
    if let Err(e) = validate_session_exists(&state, id).await {
        return e.into_response();
    }

    let event = format!("session:stop:{}", id);
    let _ = state.event_tx.send(event);

    let response = SessionLifecycleResponse {
        session_id: id,
        operation: "stop".to_string(),
        message: format!("Stop request queued for session {}", id),
    };

    (StatusCode::ACCEPTED, Json(response)).into_response()
}

/// Restart a session by ID.
///
/// Emits a "session:restart:{id}" event on the broadcast channel that the
/// orchestrator subscribes to. Returns 404 if the session ID does not exist.
pub async fn restart_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
) -> impl IntoResponse {
    if let Err(e) = validate_session_exists(&state, id).await {
        return e.into_response();
    }

    let event = format!("session:restart:{}", id);
    let _ = state.event_tx.send(event);

    let response = SessionLifecycleResponse {
        session_id: id,
        operation: "restart".to_string(),
        message: format!("Restart request queued for session {}", id),
    };

    (StatusCode::ACCEPTED, Json(response)).into_response()
}

// ─── Helper Functions ──────────────────────────────────────────────────────────

/// Validates that a session with the given ID exists.
///
/// Returns Ok(()) if the session exists, or Err(response) if it doesn't.
/// A session exists if:
/// 1. Live session snapshot contains a session with that client_id, OR
/// 2. Character configs contain an entry (fallback for demo mode)
async fn validate_session_exists(
    state: &Arc<AppState>,
    session_id: u32,
) -> Result<(), impl IntoResponse> {
    // Try to read live sessions from snapshot
    match read_live_sessions(&state.live_session_snapshot_path) {
        Ok(sessions) => {
            if sessions.iter().any(|s| s.client_id == session_id) {
                return Ok(());
            }
        }
        Err(err) => {
            return Err(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read live sessions: {}", err),
            ));
        }
    }

    // Fallback: check character configs for demo mode
    let configs = state.character_configs.read().await;
    // In demo mode, session IDs are derived from character config position (1-indexed)
    let num_configs = configs.len();
    if (session_id as usize) <= num_configs && session_id > 0 {
        return Ok(());
    }

    Err(json_error(
        StatusCode::NOT_FOUND,
        format!("Session {} not found", session_id),
    ))
}

/// Reads the live session snapshot from disk.
fn read_live_sessions(
    path: &std::path::Path,
) -> anyhow::Result<Vec<textquest_common::shared_client_state::SharedClientState>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let payload = std::fs::read(path)?;
    let sessions = serde_json::from_slice::<Vec<textquest_common::shared_client_state::SharedClientState>>(&payload)?;
    Ok(sessions)
}

// ─── Router ────────────────────────────────────────────────────────────────────

/// Build the admin sessions sub-router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/:id/start", post(start_session))
        .route("/:id/stop", post(stop_session))
        .route("/:id/restart", post(restart_session))
}
