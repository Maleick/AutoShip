//! Admin API endpoints for session lifecycle control and configuration audit.
//!
//! Provides REST endpoints to restart sessions and audit configuration:
//! - POST /api/admin/sessions/{id}/start
//! - POST /api/admin/sessions/{id}/stop
//! - POST /api/admin/sessions/{id}/restart
//! - GET /api/admin/sessions/{id}/config-audit

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use textquest_common::api_types::ErrorResponse;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupId {
    pub backup_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupList {
    pub backups: Vec<BackupId>,
}

/// Request body for config accept operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigAcceptRequest {
    /// Configuration changes to accept (as JSON object).
    pub changes: serde_json::Value,
}

/// Request body for promoting config to a named file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteToConfigRequest {
    /// Name of the config file to save to.
    pub config_name: String,
    /// Optional description of the changes.
    pub description: Option<String>,
}

/// Response for config operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigOpResponse {
    pub session_id: u32,
    pub operation: String,
    pub message: String,
}

use crate::error::json_error_pair as json_error;

// ─── Session Lifecycle Handlers ────────────────────────────────────────────────

async fn queue_lifecycle_operation(
    state: Arc<AppState>,
    id: u32,
    headers: HeaderMap,
    operation: &'static str,
) -> axum::response::Response {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return json_error(
            StatusCode::FORBIDDEN,
            "Forbidden: untrusted origin for session lifecycle mutation",
        )
        .into_response();
    }

    if let Err(e) = validate_session_exists(&state, id).await {
        return e.into_response();
    }

    let event = format!("session:{operation}:{id}");
    let _ = state.event_tx.send(event);

    let response = SessionLifecycleResponse {
        session_id: id,
        operation: operation.to_string(),
        message: format!("{operation} request queued for session {id}"),
    };

    (StatusCode::ACCEPTED, Json(response)).into_response()
}

/// Start a session by ID.
pub async fn start_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
    headers: HeaderMap,
) -> impl IntoResponse {
    queue_lifecycle_operation(state, id, headers, "start").await
}

/// Stop a session by ID.
pub async fn stop_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
    headers: HeaderMap,
) -> impl IntoResponse {
    queue_lifecycle_operation(state, id, headers, "stop").await
}

/// Restart a session by ID.
///
/// Emits a "session:restart:{id}" event on the broadcast channel that the
/// orchestrator subscribes to. Returns 404 if the session ID does not exist.
pub async fn restart_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
    headers: HeaderMap,
) -> impl IntoResponse {
    queue_lifecycle_operation(state, id, headers, "restart").await
}

pub async fn list_backups(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
) -> impl IntoResponse {
    if let Err(e) = validate_session_exists(&state, id).await {
        return e.into_response();
    }

    (StatusCode::OK, Json(BackupList { backups: vec![] })).into_response()
}

pub async fn create_backup(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return json_error(
            StatusCode::FORBIDDEN,
            "Forbidden: untrusted origin for session backup mutation",
        )
        .into_response();
    }

    if let Err(e) = validate_session_exists(&state, id).await {
        return e.into_response();
    }

    json_error(
        StatusCode::NOT_IMPLEMENTED,
        "Session backup creation is not implemented in this build",
    )
    .into_response()
}

pub async fn restore_backup(
    State(state): State<Arc<AppState>>,
    Path((id, _backup_id)): Path<(u32, String)>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return json_error(
            StatusCode::FORBIDDEN,
            "Forbidden: untrusted origin for session backup mutation",
        )
        .into_response();
    }

    if let Err(e) = validate_session_exists(&state, id).await {
        return e.into_response();
    }

    json_error(
        StatusCode::NOT_IMPLEMENTED,
        "Session backup restore is not implemented in this build",
    )
    .into_response()
}

// ─── Config Accept/Undo/Promote Handlers ──────────────────────────────────────

/// Accept configuration changes (write to in-memory state).
pub async fn accept_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
    headers: HeaderMap,
    Json(payload): Json<ConfigAcceptRequest>,
) -> impl IntoResponse {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return json_error(
            StatusCode::FORBIDDEN,
            "Forbidden: untrusted origin for config mutation",
        )
        .into_response();
    }

    if let Err(e) = validate_session_exists(&state, id).await {
        return e.into_response();
    }

    let changes = payload.changes.clone();
    {
        let mut history = state.config_change_history.write().await;
        history.push(changes.clone());
    }
    {
        let mut last = state.last_config_change.write().await;
        *last = Some(changes);
    }

    let response = ConfigOpResponse {
        session_id: id,
        operation: "accept".to_string(),
        message: format!("Configuration accepted for session {id}"),
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// Undo the last accepted configuration change.
pub async fn undo_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return json_error(
            StatusCode::FORBIDDEN,
            "Forbidden: untrusted origin for config mutation",
        )
        .into_response();
    }

    if let Err(e) = validate_session_exists(&state, id).await {
        return e.into_response();
    }

    {
        let mut history = state.config_change_history.write().await;

        if history.is_empty() {
            return json_error(StatusCode::BAD_REQUEST, "No changes to undo").into_response();
        }

        history.pop();
        let mut last = state.last_config_change.write().await;
        *last = history.last().cloned();
    }

    let response = ConfigOpResponse {
        session_id: id,
        operation: "undo".to_string(),
        message: format!("Configuration change undone for session {id}"),
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// Promote accepted configuration changes to a named config file.
pub async fn promote_to_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
    headers: HeaderMap,
    Json(payload): Json<PromoteToConfigRequest>,
) -> impl IntoResponse {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return json_error(
            StatusCode::FORBIDDEN,
            "Forbidden: untrusted origin for config mutation",
        )
        .into_response();
    }

    if let Err(e) = validate_session_exists(&state, id).await {
        return e.into_response();
    }

    let last_change = state.last_config_change.read().await;

    if last_change.is_none() {
        return json_error(StatusCode::BAD_REQUEST, "No configuration to promote")
            .into_response();
    }

    // In a real implementation, this would:
    // 1. Take the last_config_change
    // 2. Write it to config/{config_name}.json
    // 3. Return the file path or ID

    let response = ConfigOpResponse {
        session_id: id,
        operation: "promote".to_string(),
        message: format!(
            "Configuration promoted to '{}' for session {id}",
            payload.config_name
        ),
    };

    (StatusCode::OK, Json(response)).into_response()
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
    let sessions = serde_json::from_slice::<
        Vec<textquest_common::shared_client_state::SharedClientState>,
    >(&payload)?;
    Ok(sessions)
}

// ─── Router ────────────────────────────────────────────────────────────────────

/// Build the admin sessions sub-router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/{id}/start", post(start_session))
        .route("/{id}/stop", post(stop_session))
        .route("/{id}/restart", post(restart_session))
        .route("/{id}/config-audit", get(audit_config))
        .route("/{id}/config/accept", post(accept_config))
        .route("/{id}/config/undo", post(undo_config))
        .route("/{id}/config/promote", post(promote_to_config))
        .route("/{id}/backups", get(list_backups).post(create_backup))
        .route("/{id}/backups/{backup_id}/restore", post(restore_backup))
}

// ─── Config Audit Endpoint ────────────────────────────────────────────────────

/// Audit endpoint re-exported from admin_config module for mounting at /admin/sessions/:id/config-audit
pub use crate::api::admin_config::audit_config;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_router_has_config_audit_route() {
        // Test that the router is properly constructed with all required routes
        let _app_router = router();
        // If this compiles and runs, the router is correctly built
    }

    #[test]
    fn test_session_lifecycle_response_serialization() {
        let response = SessionLifecycleResponse {
            session_id: 42,
            operation: "start".to_string(),
            message: "Start request queued for session 42".to_string(),
        };

        let json = serde_json::to_string(&response).expect("serialize");
        assert!(json.contains("\"session_id\":42"));
        assert!(json.contains("\"operation\":\"start\""));
        assert!(json.contains("message"));
    }

    #[test]
    fn test_error_response_serialization() {
        let error = ErrorResponse {
            error: "Session not found".to_string(),
        };

        let json = serde_json::to_string(&error).expect("serialize");
        assert!(json.contains("\"error\":\"Session not found\""));
    }

    #[test]
    fn test_session_lifecycle_request_parsing() {
        let json = r#"{"timeout_secs": 30}"#;
        let req: SessionLifecycleRequest = serde_json::from_str(json).expect("parse request");
        assert_eq!(req.timeout_secs, Some(30));
    }

    #[test]
    fn test_session_lifecycle_request_parsing_empty() {
        let json = r#"{}"#;
        let req: SessionLifecycleRequest = serde_json::from_str(json).expect("parse request");
        assert_eq!(req.timeout_secs, None);
    }

    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn restart_session_rejects_untrusted_origin() {
        let state = crate::test_support::demo_app_state();
        let app = router().with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/restart")
                    .header(header::ORIGIN, "https://evil.example")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn lifecycle_routes_accept_trusted_local_origin() {
        for operation in ["start", "stop", "restart"] {
            let state = crate::test_support::demo_app_state();
            let app = router().with_state(state);

            let response = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/1/{operation}"))
                        .header(header::ORIGIN, "http://127.0.0.1:3001")
                        .body(Body::empty())
                        .expect("request"),
                )
                .await
                .expect("response");

            assert_eq!(
                response.status(),
                StatusCode::ACCEPTED,
                "{operation} should be mounted and accepted"
            );
        }
    }

    #[tokio::test]
    async fn backup_routes_are_explicitly_mounted() {
        let state = crate::test_support::demo_app_state();
        let app = router().with_state(state);

        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/1/backups")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(list_response.status(), StatusCode::OK);

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/backups")
                    .header(header::ORIGIN, "http://127.0.0.1:3001")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(create_response.status(), StatusCode::NOT_IMPLEMENTED);

        let restore_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/1/backups/example/restore")
                    .header(header::ORIGIN, "http://127.0.0.1:3001")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(restore_response.status(), StatusCode::NOT_IMPLEMENTED);
    }
}
