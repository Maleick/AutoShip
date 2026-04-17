//! Cross-group assist API handlers.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response as AxumResponse},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct XAssistConfig {
    pub ma_name: Option<String>,
    pub enabled: bool,
}

impl Default for XAssistConfig {
    fn default() -> Self {
        Self {
            ma_name: None,
            enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XAssistCharacterConfig {
    pub character_name: String,
    pub ma_name: Option<String>,
    pub enabled: bool,
}

pub type XAssistConfigs = Arc<RwLock<HashMap<String, XAssistConfig>>>;

pub fn demo_xassist_configs() -> XAssistConfigs {
    let mut configs = HashMap::new();
    configs.insert(
        "Frostreaver".into(),
        XAssistConfig {
            ma_name: Some("Noxus".into()),
            enabled: false,
        },
    );
    configs.insert(
        "Noxus".into(),
        XAssistConfig {
            ma_name: None,
            enabled: false,
        },
    );
    configs.insert(
        "Aelrindel".into(),
        XAssistConfig {
            ma_name: Some("Noxus".into()),
            enabled: true,
        },
    );
    configs.insert(
        "Grok".into(),
        XAssistConfig {
            ma_name: Some("Noxus".into()),
            enabled: false,
        },
    );
    configs.insert(
        "Valerius".into(),
        XAssistConfig {
            ma_name: Some("Noxus".into()),
            enabled: true,
        },
    );
    Arc::new(RwLock::new(configs))
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

fn json_error(status: StatusCode, message: impl Into<String>) -> AxumResponse {
    (
        status,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
        .into_response()
}

pub async fn get_xassist_config(
    State(state): State<Arc<AppState>>,
    Path(character): Path<String>,
) -> AxumResponse {
    let configs = state.xassist_configs.read().await;
    match configs.get(&character) {
        Some(cfg) => (
            StatusCode::OK,
            Json(XAssistCharacterConfig {
                character_name: character,
                ma_name: cfg.ma_name.clone(),
                enabled: cfg.enabled,
            }),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Config not found for '{}'", character),
            }),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct XAssistConfigUpdate {
    pub ma_name: Option<String>,
    pub enabled: bool,
}

pub async fn put_xassist_config(
    State(state): State<Arc<AppState>>,
    Path(character): Path<String>,
    Json(update): Json<XAssistConfigUpdate>,
) -> AxumResponse {
    if character.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "Character name cannot be empty");
    }
    let ma_name = update.ma_name.clone();
    let mut configs = state.xassist_configs.write().await;
    configs.insert(
        character.clone(),
        XAssistConfig {
            ma_name: update.ma_name,
            enabled: update.enabled,
        },
    );
    (
        StatusCode::OK,
        Json(XAssistCharacterConfig {
            character_name: character,
            ma_name,
            enabled: update.enabled,
        }),
    )
        .into_response()
}

pub async fn delete_xassist_config(
    State(state): State<Arc<AppState>>,
    Path(character): Path<String>,
) -> AxumResponse {
    let mut configs = state.xassist_configs.write().await;
    if configs.remove(&character).is_some() {
        StatusCode::NO_CONTENT.into_response()
    } else {
        json_error(
            StatusCode::NOT_FOUND,
            format!("Config not found for '{}'", character),
        )
    }
}

pub async fn list_xassist_configs(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<XAssistCharacterConfig>> {
    let configs = state.xassist_configs.read().await;
    let mut result: Vec<XAssistCharacterConfig> = configs
        .iter()
        .map(|(name, cfg)| XAssistCharacterConfig {
            character_name: name.clone(),
            ma_name: cfg.ma_name.clone(),
            enabled: cfg.enabled,
        })
        .collect();
    result.sort_by(|a, b| a.character_name.cmp(&b.character_name));
    Json(result)
}
