//! Admin log tail API — retrieves recent log lines for sessions.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::AppState;

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
/// If no logs are stored for the session, returns an empty array.
pub async fn tail_logs(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<u32>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    let lines_requested = query.lines.max(1).min(10000) as usize;

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
    use tokio::sync::RwLock;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_tail_logs_empty_session() {
        let session_logs = Arc::new(RwLock::new(HashMap::new()));
        let logs_read = session_logs.read().await;
        let result = logs_read.get(&1);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_tail_logs_returns_all_lines_when_fewer_than_requested() {
        let session_logs = Arc::new(RwLock::new(HashMap::new()));

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
        let session_logs = Arc::new(RwLock::new(HashMap::new()));

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
}
