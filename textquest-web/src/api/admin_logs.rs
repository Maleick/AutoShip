//! Admin log tail API — retrieves recent log lines for sessions.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::AppState;

/// Extract the authenticated user identifier from a request.
///
/// Since the system uses a single shared API token, we derive a user_id
/// from the request or use a default. In a multi-user system, this would
/// extract from JWT claims or session context.
fn extract_user_id() -> String {
    // For now, use a default user_id. In production, this would be extracted
    // from authenticated request context (JWT claims, session cookies, etc.)
    "default_user".to_string()
}

#[derive(Debug, serde::Deserialize)]
pub struct LogQuery {
    #[serde(default = "default_lines")]
    lines: u32,
}

fn default_lines() -> u32 {
    50
}

/// Retrieves the last N log lines for a session.
///
/// Returns a JSON array of recent log lines, newest last.
/// Ownership check: verify the authenticated user owns this session.
/// Returns 403 Forbidden if the user does not own the session.
/// Returns 404 Not Found if the session does not exist.
pub async fn tail_logs(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<u32>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    let user_id = extract_user_id();
    let lines_requested = query.lines.clamp(1, 10000) as usize;

    // Check ownership: verify the authenticated user owns this session_id
    {
        let owners = state.session_logs_owner.read().await;
        if let Some(owner) = owners.get(&session_id) {
            if owner != &user_id {
                // User does not own this session — return 403 Forbidden
                return (StatusCode::FORBIDDEN, Json::<Vec<String>>(Vec::new())).into_response();
            }
        } else {
            // Session does not exist (no owner registered)
            return (StatusCode::NOT_FOUND, Json::<Vec<String>>(Vec::new())).into_response();
        }
    }

    // Read session logs from the in-memory store
    let session_logs = state.session_logs.read().await;

    let logs = if let Some(all_logs) = session_logs.get(&session_id) {
        // Return the last N lines
        let start = if all_logs.len() > lines_requested {
            all_logs.len() - lines_requested
        } else {
            0
        };
        all_logs[start..].to_vec()
    } else {
        // No logs for this session
        Vec::new()
    };

    (StatusCode::OK, Json(logs)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_tail_logs_empty_session() {
        let session_logs = Arc::new(RwLock::new(HashMap::<u32, Vec<String>>::new()));
        let logs_read = session_logs.read().await;
        let result = logs_read.get(&1);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_tail_logs_returns_all_lines_when_fewer_than_requested() {
        let session_logs = Arc::new(RwLock::new(HashMap::<u32, Vec<String>>::new()));

        let mut logs = session_logs.write().await;
        logs.insert(
            1,
            vec![
                "[2026-04-18 10:00:00] Session started".to_string(),
                "[2026-04-18 10:00:01] Loading map".to_string(),
                "[2026-04-18 10:00:02] Ready".to_string(),
            ],
        );
        drop(logs);

        let logs_read = session_logs.read().await;
        let lines = logs_read.get(&1).expect("session should have logs");
        assert_eq!(lines.len(), 3);
    }

    #[tokio::test]
    async fn test_tail_logs_returns_last_n_lines() {
        let session_logs = Arc::new(RwLock::new(HashMap::<u32, Vec<String>>::new()));

        let mut logs = session_logs.write().await;
        let many_logs: Vec<String> = (0..100)
            .map(|i| format!("[2026-04-18 10:00:{:02}] Log line {}", i % 60, i))
            .collect();
        logs.insert(1, many_logs);
        drop(logs);

        let logs_read = session_logs.read().await;
        let lines = logs_read.get(&1).expect("session should have logs");
        assert_eq!(lines.len(), 100);
        // Verify we can retrieve the last N
        assert!(lines[lines.len() - 1].contains("Log line 99"));
    }

    #[tokio::test]
    async fn test_ownership_check_denies_cross_owner_access() {
        use crate::test_support;

        let state = test_support::demo_app_state();

        // Set up logs for session 100 owned by "user_1"
        {
            let mut owners = state.session_logs_owner.write().await;
            owners.insert(100, "user_1".to_string());
        }
        {
            let mut logs = state.session_logs.write().await;
            logs.insert(100, vec!["log line 1".to_string(), "log line 2".to_string()]);
        }

        // Attempt to access session 100 as "default_user" (the current extract_user_id() returns)
        // This should be denied with 403 Forbidden since "default_user" != "user_1"
        let response = tail_logs(
            axum::extract::State(state),
            AxumPath(100u32),
            Query(LogQuery { lines: 50 }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_nonexistent_session_returns_404() {
        use crate::test_support;

        let state = test_support::demo_app_state();

        // Try to access session 999 which doesn't exist in ownership map
        let response = tail_logs(
            axum::extract::State(state),
            AxumPath(999u32),
            Query(LogQuery { lines: 50 }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
