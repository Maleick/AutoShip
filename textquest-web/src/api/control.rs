//! Session control and command relay endpoints for SDK parity.
//!
//! These endpoints provide the external control surface needed by SDK clients
//! for session/group control and slash-command relay.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionControlState {
    pub session_id: u32,
    pub group_id: u8,
    pub state: String,
    pub routing_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequest {
    pub command: String,
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupAssignment {
    pub group_id: u8,
}

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

pub async fn pause_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<u32>,
) -> impl IntoResponse {
    tracing::debug!(session_id, "Pause requested via API");
    let mut controls = state.session_controls.write().await;
    if let Some(control) = controls.get_mut(&session_id) {
        control.state = "Paused".to_string();
        tracing::info!(session_id, "Session paused via API");
        (StatusCode::OK, Json(())).into_response()
    } else {
        json_error(
            StatusCode::NOT_FOUND,
            format!("Session {session_id} not found"),
        )
        .into_response()
    }
}

pub async fn resume_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<u32>,
) -> impl IntoResponse {
    tracing::debug!(session_id, "Resume requested via API");
    let mut controls = state.session_controls.write().await;
    if let Some(control) = controls.get_mut(&session_id) {
        control.state = "Active".to_string();
        tracing::info!(session_id, "Session resumed via API");
        (StatusCode::OK, Json(())).into_response()
    } else {
        json_error(
            StatusCode::NOT_FOUND,
            format!("Session {session_id} not found"),
        )
        .into_response()
    }
}

pub async fn set_session_group(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<u32>,
    Json(assignment): Json<GroupAssignment>,
) -> impl IntoResponse {
    tracing::debug!(
        session_id,
        group_id = assignment.group_id,
        "SetGroup requested via API"
    );
    let mut controls = state.session_controls.write().await;
    if let Some(control) = controls.get_mut(&session_id) {
        control.group_id = assignment.group_id;
        control.routing_scope = if assignment.group_id == 0 {
            "AllSession".to_string()
        } else {
            format!("Group{}", assignment.group_id)
        };
        tracing::info!(
            session_id,
            group_id = assignment.group_id,
            "Session group set via API"
        );
        (StatusCode::OK, Json(())).into_response()
    } else {
        json_error(
            StatusCode::NOT_FOUND,
            format!("Session {session_id} not found"),
        )
        .into_response()
    }
}

pub async fn broadcast_all(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<u32>,
) -> impl IntoResponse {
    tracing::debug!(session_id, "BroadcastAll requested via API");
    let mut controls = state.session_controls.write().await;
    if let Some(control) = controls.get_mut(&session_id) {
        control.routing_scope = "AllSession".to_string();
        tracing::info!(session_id, "Session broadcast set to AllSession via API");
        (StatusCode::OK, Json(())).into_response()
    } else {
        json_error(
            StatusCode::NOT_FOUND,
            format!("Session {session_id} not found"),
        )
        .into_response()
    }
}

pub async fn relay_command(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CommandRequest>,
) -> impl IntoResponse {
    tracing::debug!(command = req.command, target = ?req.target, "Command relay requested");
    let event = serde_json::json!({
        "type": "command",
        "command": req.command,
        "target": req.target,
    });
    if let Err(e) = state.event_tx.send(event.to_string()) {
        tracing::error!("Failed to broadcast command: {}", e);
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to relay command")
            .into_response();
    }
    tracing::info!(command = req.command, "Command relayed via API");
    (
        StatusCode::OK,
        Json(CommandResponse {
            success: true,
            message: format!("Command '{}' relayed", req.command),
        }),
    )
        .into_response()
}

pub fn router() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/pause/{session_id}", axum::routing::put(pause_session))
        .route("/resume/{session_id}", axum::routing::put(resume_session))
        .route("/group/{session_id}", axum::routing::put(set_session_group))
        .route(
            "/broadcast-all/{session_id}",
            axum::routing::put(broadcast_all),
        )
}
