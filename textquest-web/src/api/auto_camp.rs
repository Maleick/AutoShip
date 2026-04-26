//! Auto-camp handling for safety automation (MQ2AutoCamp parity).

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, Eq)]
pub enum CampTrigger {
    #[serde(rename = "gm_detected")]
    GmDetected,
    #[serde(rename = "death_threshold")]
    DeathThreshold,
    #[serde(rename = "afk_timeout")]
    AfkTimeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCampConfig {
    pub enabled: bool,
    pub triggers: Vec<CampTrigger>,
    pub death_threshold: u8,
    pub afk_timeout_minutes: u32,
}

impl Default for AutoCampConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            triggers: vec![CampTrigger::GmDetected],
            death_threshold: 20,
            afk_timeout_minutes: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCampStatus {
    pub enabled: bool,
    pub active_triggers: Vec<CampTrigger>,
    pub camp_count: u64,
    pub last_camp_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutAutoCampConfig {
    pub enabled: bool,
    pub triggers: Vec<CampTrigger>,
    pub death_threshold: u8,
    pub afk_timeout_minutes: u32,
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

/// Get current auto-camp configuration and status.
pub async fn get_auto_camp(State(_state): State<Arc<AppState>>) -> Response {
    let config = AutoCampConfig::default();
    let status = AutoCampStatus {
        enabled: config.enabled,
        active_triggers: config.triggers.clone(),
        camp_count: 0,
        last_camp_reason: None,
    };
    Json(status).into_response()
}

/// Update auto-camp configuration.
pub async fn put_auto_camp(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<PutAutoCampConfig>,
) -> Response {
    if req.triggers.is_empty() {
        return json_error(
            StatusCode::BAD_REQUEST,
            "At least one camp trigger must be enabled",
        );
    }

    if !(1..=100).contains(&req.death_threshold) {
        return json_error(
            StatusCode::BAD_REQUEST,
            "death_threshold must be between 1 and 100",
        );
    }

    if req.afk_timeout_minutes == 0 {
        return json_error(
            StatusCode::BAD_REQUEST,
            "afk_timeout_minutes must be greater than 0",
        );
    }

    let config = AutoCampConfig {
        enabled: req.enabled,
        triggers: req.triggers.clone(),
        death_threshold: req.death_threshold,
        afk_timeout_minutes: req.afk_timeout_minutes,
    };

    let status = AutoCampStatus {
        enabled: config.enabled,
        active_triggers: config.triggers,
        camp_count: 0,
        last_camp_reason: None,
    };

    Json(status).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", axum::routing::get(get_auto_camp))
        .route("/", axum::routing::put(put_auto_camp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_valid() {
        let config = AutoCampConfig::default();
        assert!(!config.triggers.is_empty());
        assert!((1..=100).contains(&config.death_threshold));
        assert!(config.afk_timeout_minutes > 0);
    }

    #[test]
    fn camp_trigger_serialization() {
        let trigger = CampTrigger::GmDetected;
        let json = serde_json::to_string(&trigger).unwrap();
        assert_eq!(json, r#""gm_detected""#);
    }
}
