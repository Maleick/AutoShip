//! Session control and command relay endpoints for SDK parity.
//!
//! These endpoints provide the external control surface needed by SDK clients
//! for session/group control and slash-command relay.
//!
//! Security note:
//! The `/command` relay is intentionally narrow by design. Only read-only
//! observability commands are accepted to prevent HTTP callers from issuing
//! arbitrary fleet-control commands that bypass TUI-side authorization.

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum RelayCommandAllowed {
    ReadStatus,
    GetLog,
    GetMetrics,
}

impl RelayCommandAllowed {
    fn from_command(value: &str) -> Option<Self> {
        match value {
            "ReadStatus" => Some(Self::ReadStatus),
            "GetLog" => Some(Self::GetLog),
            "GetMetrics" => Some(Self::GetMetrics),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::ReadStatus => "ReadStatus",
            Self::GetLog => "GetLog",
            Self::GetMetrics => "GetMetrics",
        }
    }
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
    let command = req.command.trim();
    if command.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "command must not be empty").into_response();
    }

    let Some(allowed_command) = RelayCommandAllowed::from_command(command) else {
        tracing::warn!(
            command = req.command,
            target = ?req.target,
            "Relay command rejected by allowlist"
        );
        return json_error(
            StatusCode::FORBIDDEN,
            "command not allowed through web relay endpoint",
        )
        .into_response();
    };

    tracing::debug!(
        command = allowed_command.as_str(),
        target = ?req.target,
        "Command relay requested"
    );
    let event = serde_json::json!({
        "type": "command",
        "command": allowed_command.as_str(),
        "target": req.target,
    });
    if let Err(e) = state.event_tx.send(event.to_string()) {
        tracing::error!("Failed to broadcast command: {}", e);
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to relay command")
            .into_response();
    }
    tracing::info!(command = allowed_command.as_str(), "Command relayed via API");
    (
        StatusCode::OK,
        Json(CommandResponse {
            success: true,
            message: format!("Command '{}' relayed", allowed_command.as_str()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;
    use serde_json::Value;
    use std::sync::Arc;

    fn test_app() -> Arc<AppState> {
        Arc::new(crate::test_app_state())
    }

    async fn assert_status_and_body<T>(response: axum::response::Response, expected: StatusCode) -> T
    where
        T: for<'a> serde::Deserialize<'a>,
    {
        assert_eq!(response.status(), expected);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("response body should be readable")
            .to_bytes();
        serde_json::from_slice::<T>(&body).expect("response body should deserialize")
    }

    #[tokio::test]
    async fn control_relay_blocks_disallowed_command() {
        let state = test_app();
        let mut events = state.event_tx.subscribe();
        let response = relay_command(
            axum::extract::State(state.clone()),
            axum::Json(CommandRequest {
                command: "StartSession".to_string(),
                target: Some("evil".to_string()),
            }),
        )
        .await
        .into_response();
        let _: Value = assert_status_and_body::<Value>(response, StatusCode::FORBIDDEN).await;
        assert!(
            events.try_recv().is_err(),
            "disallowed command should not emit events"
        );
    }

    #[tokio::test]
    async fn control_relay_allows_readonly_commands() {
        let state = test_app();
        let mut events = state.event_tx.subscribe();
        let response = relay_command(
            axum::extract::State(state.clone()),
            axum::Json(CommandRequest {
                command: "GetLog".to_string(),
                target: Some("session-8".to_string()),
            }),
        )
        .await
        .into_response();
        let body: CommandResponse = assert_status_and_body::<CommandResponse>(response, StatusCode::OK).await;
        assert!(body.success);

        let event = events
            .try_recv()
            .expect("allowed command should emit one event")
            .replace(" ", "");
        let event_value: serde_json::Value = serde_json::from_str(&event).expect("event should be json");
        assert_eq!(event_value["command"], "GetLog");
        assert_eq!(event_value["type"], "command");
        assert_eq!(event_value["target"], "session-8");
    }
}
