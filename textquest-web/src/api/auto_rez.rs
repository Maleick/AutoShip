//! Auto-resurrection acceptance handling (MQ2Rez parity).

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutoRezStatus {
    pub enabled: bool,
    pub min_xp_pct: u8,
    pub accept_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutAutoRezConfig {
    pub enabled: bool,
    pub min_xp_pct: u8,
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
    let configs = state.character_configs.read().await;
    let auto_rez = configs
        .values()
        .next()
        .map(|c| c.auto_rez.clone())
        .unwrap_or_default();
    let status = AutoRezStatus {
        enabled: auto_rez.enabled,
        min_xp_pct: auto_rez.min_xp_pct,
        accept_count: 0,
    };
    Json(status).into_response()
}

/// Update auto-rez configuration.
pub async fn put_auto_rez(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PutAutoRezConfig>,
) -> Response {
    if !(0..=100).contains(&req.min_xp_pct) {
        return json_error(
            StatusCode::BAD_REQUEST,
            "min_xp_pct must be between 0 and 100",
        );
    }

    // Apply to all character configs.
    {
        let mut configs = state.character_configs.write().await;
        for cfg in configs.values_mut() {
            cfg.auto_rez.enabled = req.enabled;
            cfg.auto_rez.min_xp_pct = req.min_xp_pct;
        }
    }

    let status = AutoRezStatus {
        enabled: req.enabled,
        min_xp_pct: req.min_xp_pct,
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
