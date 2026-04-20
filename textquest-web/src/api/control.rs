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
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

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
