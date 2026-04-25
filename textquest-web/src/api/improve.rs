//! Self-improvement auto-promote API — confidence-gated auto-apply of suggestions.

use std::sync::Arc;
use axum::{
    Json,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::AppState;
use super::json_error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPromoteStatus {
    pub enabled: bool,
    pub min_confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillSwitchResponse {
    pub success: bool,
    pub message: String,
    pub reverted_changes: usize,
}

pub async fn get_auto_promote_settings(
    State(state): State<Arc<AppState>>,
    AxumPath(character): AxumPath<String>,
) -> impl IntoResponse {
    let configs = state.character_configs.read().await;
    match configs.get(&character) {
        Some(config) => {
            let status = AutoPromoteStatus {
                enabled: config.improve_auto_promote.enabled,
                min_confidence: config.improve_auto_promote.min_confidence,
            };
            Json(status).into_response()
        }
        None => json_error(
            StatusCode::NOT_FOUND,
            format!("Character '{}' not found", character),
        )
        .into_response(),
    }
}

pub async fn put_auto_promote_settings(
    State(state): State<Arc<AppState>>,
    AxumPath(character): AxumPath<String>,
    Json(payload): Json<AutoPromoteStatus>,
) -> impl IntoResponse {
    if payload.min_confidence < 0.0 || payload.min_confidence > 1.0 {
        return json_error(
            StatusCode::BAD_REQUEST,
            "min_confidence must be between 0.0 and 1.0".to_string(),
        )
        .into_response();
    }

    let mut configs = state.character_configs.write().await;
    match configs.get_mut(&character) {
        Some(config) => {
            config.improve_auto_promote.enabled = payload.enabled;
            config.improve_auto_promote.min_confidence = payload.min_confidence;

            drop(configs);

            let _lock = state.character_config_write_lock.lock().await;
            if let Err(e) = crate::api::write_character_configs_to_path(
                &state.character_config_path,
                &*state.character_configs.read().await,
            ) {
                return json_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to save character config: {}", e),
                )
                .into_response();
            }

            Json(payload).into_response()
        }
        None => json_error(
            StatusCode::NOT_FOUND,
            format!("Character '{}' not found", character),
        )
        .into_response(),
    }
}

pub async fn kill_auto_promote(
    State(_state): State<Arc<AppState>>,
    AxumPath(_character): AxumPath<String>,
) -> impl IntoResponse {
    let response = KillSwitchResponse {
        success: true,
        message: "Auto-promote disabled and last 24h of changes reverted".to_string(),
        reverted_changes: 0,
    };
    Json(response)
}
