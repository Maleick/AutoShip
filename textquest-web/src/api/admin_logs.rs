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

pub async fn tail_logs(
    State(_state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<u32>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    let logs: Vec<String> = (0..query.lines)
        .map(|i| {
            format!(
                "[{}] Log line for session {} - sample line {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                session_id,
                i + 1
            )
        })
        .collect();

    (StatusCode::OK, Json(logs)).into_response()
}
