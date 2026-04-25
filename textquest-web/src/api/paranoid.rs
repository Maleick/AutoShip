//! Enhanced anti-GM detection with behavioral changes on suspicion (MQ2Paranoid parity).

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, Eq)]
pub enum ParanoidBehavior {
    #[serde(rename = "pause_automation")]
    PauseAutomation,
    #[serde(rename = "reduce_movement")]
    ReduceMovement,
    #[serde(rename = "go_idle")]
    GoIdle,
    #[serde(rename = "camp_immediately")]
    CampImmediately,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParanoidConfig {
    pub enabled: bool,
    pub suspicion_threshold: u8,
    pub behavior_on_suspicion: Vec<ParanoidBehavior>,
    pub revert_delay_seconds: u32,
    pub check_player_names: bool,
    pub check_spawn_patterns: bool,
}

impl Default for ParanoidConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            suspicion_threshold: 50,
            behavior_on_suspicion: vec![ParanoidBehavior::PauseAutomation],
            revert_delay_seconds: 300,
            check_player_names: true,
            check_spawn_patterns: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParanoidStatus {
    pub enabled: bool,
    pub current_suspicion_level: u8,
    pub is_suspicious: bool,
    pub active_behaviors: Vec<ParanoidBehavior>,
    pub last_suspicion_trigger: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutParanoidConfig {
    pub enabled: bool,
    pub suspicion_threshold: u8,
    pub behavior_on_suspicion: Vec<ParanoidBehavior>,
    pub revert_delay_seconds: u32,
    pub check_player_names: bool,
    pub check_spawn_patterns: bool,
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

/// Get current paranoid configuration and status.
pub async fn get_paranoid(State(_state): State<Arc<AppState>>) -> Response {
    let config = ParanoidConfig::default();
    let status = ParanoidStatus {
        enabled: config.enabled,
        current_suspicion_level: 0,
        is_suspicious: false,
        active_behaviors: config.behavior_on_suspicion.clone(),
        last_suspicion_trigger: None,
    };
    Json(status).into_response()
}

/// Update paranoid configuration.
pub async fn put_paranoid(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<PutParanoidConfig>,
) -> Response {
    if req.behavior_on_suspicion.is_empty() {
        return json_error(
            StatusCode::BAD_REQUEST,
            "At least one behavior must be configured",
        );
    }

    if !(1..=100).contains(&req.suspicion_threshold) {
        return json_error(
            StatusCode::BAD_REQUEST,
            "suspicion_threshold must be between 1 and 100",
        );
    }

    if req.revert_delay_seconds == 0 {
        return json_error(
            StatusCode::BAD_REQUEST,
            "revert_delay_seconds must be greater than 0",
        );
    }

    let config = ParanoidConfig {
        enabled: req.enabled,
        suspicion_threshold: req.suspicion_threshold,
        behavior_on_suspicion: req.behavior_on_suspicion.clone(),
        revert_delay_seconds: req.revert_delay_seconds,
        check_player_names: req.check_player_names,
        check_spawn_patterns: req.check_spawn_patterns,
    };

    let status = ParanoidStatus {
        enabled: config.enabled,
        current_suspicion_level: 0,
        is_suspicious: false,
        active_behaviors: config.behavior_on_suspicion,
        last_suspicion_trigger: None,
    };

    Json(status).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", axum::routing::get(get_paranoid))
        .route("/", axum::routing::put(put_paranoid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_valid() {
        let config = ParanoidConfig::default();
        assert!(!config.behavior_on_suspicion.is_empty());
        assert!((1..=100).contains(&config.suspicion_threshold));
        assert!(config.revert_delay_seconds > 0);
    }

    #[test]
    fn paranoid_behavior_serialization() {
        let behaviors = vec![
            ParanoidBehavior::PauseAutomation,
            ParanoidBehavior::ReduceMovement,
            ParanoidBehavior::GoIdle,
            ParanoidBehavior::CampImmediately,
        ];

        for behavior in behaviors {
            let json = serde_json::to_string(&behavior).unwrap();
            let deserialized: ParanoidBehavior = serde_json::from_str(&json).unwrap();
            assert_eq!(behavior, deserialized);
        }
    }
}
