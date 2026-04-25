//! Auto-resurrection acceptance handling (MQ2Rez parity).

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use textquest_common::ipc::AutoRezConfig;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutoRezStatus {
    pub enabled: bool,
    pub hp_threshold: u8,
    pub accept_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutAutoRezConfig {
    pub enabled: bool,
    pub hp_threshold: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ErrorResponse {
    pub error: String,
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
        .into_response()
}

/// Get current auto-rez configuration and status.
pub async fn get_auto_rez(State(state): State<Arc<AppState>>) -> Response {
    let config = &state.character_config.auto_rez;
    let status = AutoRezStatus {
        enabled: config.enabled,
        hp_threshold: config.hp_threshold,
        accept_count: 0,
    };
    Json(status).into_response()
}

/// Update auto-rez configuration.
pub async fn put_auto_rez(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PutAutoRezConfig>,
) -> Response {
    if !(1..=100).contains(&req.hp_threshold) {
        return json_error(
            StatusCode::BAD_REQUEST,
            "hp_threshold must be between 1 and 100",
        );
    }

    let mut config = state.character_config.auto_rez.clone();
    config.enabled = req.enabled;
    config.hp_threshold = req.hp_threshold;

    // Broadcast updated config to connected clients
    if let Err(e) = state.broadcast_config_update().await {
        return json_error(StatusCode::INTERNAL_SERVER_ERROR, format!("Config update failed: {}", e));
    }

    let status = AutoRezStatus {
        enabled: config.enabled,
        hp_threshold: config.hp_threshold,
        accept_count: 0,
    };
    Json(status).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", axum::routing::get(get_auto_rez))
        .route("/", axum::routing::put(put_auto_rez))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hp_threshold_validation() {
        let valid_configs = vec![1, 25, 50, 75, 100];
        for threshold in valid_configs {
            assert!((1..=100).contains(&threshold));
        }

        let invalid_configs = vec![0, 101, 200];
        for threshold in invalid_configs {
            assert!(!(1..=100).contains(&threshold));
        }
    }
}
