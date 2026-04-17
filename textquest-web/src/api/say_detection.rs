//! Say detection and alerting API.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use crate::AppState;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SayPatternType {
    #[default]
    Substring,
    Exact,
    Regex,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SayRuleAction {
    #[default]
    Alert,
    Broadcast,
    Command,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SayDetectionRule {
    pub name: String,
    pub pattern: String,
    #[serde(default)]
    pub pattern_type: SayPatternType,
    #[serde(default)]
    pub action_type: SayRuleAction,
    #[serde(default)]
    pub action_value: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SayDetectionConfig {
    pub enabled: bool,
    pub sound_enabled: bool,
    pub sound_file: Option<String>,
    pub toast_enabled: bool,
    pub discord_webhook_url: Option<String>,
    pub broadcast_all_clients: bool,
    pub rules: Vec<SayDetectionRule>,
}

impl Default for SayDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sound_enabled: true,
            sound_file: Some("say_alert.wav".to_string()),
            toast_enabled: true,
            discord_webhook_url: None,
            broadcast_all_clients: false,
            rules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SayDetectionMatchSummary {
    pub rule_name: String,
    pub sender: String,
    pub message: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SayDetectionStatus {
    pub config: SayDetectionConfig,
    pub total_matches: u64,
    pub last_match: Option<SayDetectionMatchSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SayDetectionSyncPayload {
    pub rule_name: String,
    pub pattern: String,
    pub sender: String,
    pub message: String,
    pub timestamp: u64,
}

pub struct SayDetectionState {
    pub config: RwLock<SayDetectionConfig>,
    pub total_matches: AtomicU64,
    pub last_match: RwLock<Option<SayDetectionMatchSummary>>,
}

impl Default for SayDetectionState {
    fn default() -> Self {
        Self {
            config: RwLock::new(SayDetectionConfig::default()),
            total_matches: AtomicU64::new(0),
            last_match: RwLock::new(None),
        }
    }
}

impl SayDetectionState {
    pub fn new_demo() -> Self {
        Self::default()
    }

    pub async fn record_match(&self, payload: SayDetectionSyncPayload) {
        self.total_matches.fetch_add(1, Ordering::Relaxed);
        *self.last_match.write().await = Some(SayDetectionMatchSummary {
            rule_name: payload.rule_name,
            sender: payload.sender,
            message: payload.message,
            timestamp: payload.timestamp,
        });
    }

    pub async fn get_status(&self) -> SayDetectionStatus {
        SayDetectionStatus {
            config: self.config.read().await.clone(),
            total_matches: self.total_matches.load(Ordering::Relaxed),
            last_match: self.last_match.read().await.clone(),
        }
    }
}

fn textquest_config_path() -> std::path::PathBuf {
    std::env::var("TEXTQUEST_CONFIG_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("config/textquest.toml"))
}

fn read_say_detection_config_from_disk() -> Result<SayDetectionConfig, String> {
    let path = textquest_config_path();
    if !path.exists() {
        return Ok(SayDetectionConfig::default());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    let doc = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;

    let Some(item) = doc.get("say_detection") else {
        return Ok(SayDetectionConfig::default());
    };

    toml_edit::de::from_str::<SayDetectionConfig>(&item.to_string())
        .map_err(|error| format!("Failed to decode [say_detection]: {error}"))
}

fn write_say_detection_config_to_disk(config: &SayDetectionConfig) -> Result<(), String> {
    let path = textquest_config_path();
    let mut doc = if path.exists() {
        let content = std::fs::read_to_string(&path)
            .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
        content
            .parse::<DocumentMut>()
            .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?
    } else {
        DocumentMut::new()
    };

    let mut table = Table::new();
    table["enabled"] = value(config.enabled);
    table["sound_enabled"] = value(config.sound_enabled);
    if let Some(sound_file) = &config.sound_file {
        table["sound_file"] = value(sound_file.clone());
    }
    table["toast_enabled"] = value(config.toast_enabled);
    if let Some(webhook) = &config.discord_webhook_url {
        table["discord_webhook_url"] = value(webhook.clone());
    }
    table["broadcast_all_clients"] = value(config.broadcast_all_clients);

    let mut rules = ArrayOfTables::new();
    for rule in &config.rules {
        let mut rule_table = Table::new();
        rule_table["name"] = value(rule.name.clone());
        rule_table["pattern"] = value(rule.pattern.clone());
        rule_table["pattern_type"] = value(
            serde_json::to_string(&rule.pattern_type)
                .map_err(|error| format!("Failed to encode rule pattern type: {error}"))?
                .trim_matches('"')
                .to_string(),
        );
        rule_table["action_type"] = value(
            serde_json::to_string(&rule.action_type)
                .map_err(|error| format!("Failed to encode rule action type: {error}"))?
                .trim_matches('"')
                .to_string(),
        );
        if let Some(action_value) = &rule.action_value
            && !action_value.trim().is_empty()
        {
            rule_table["action_value"] = value(action_value.clone());
        }
        rule_table["enabled"] = value(rule.enabled);
        rules.push(rule_table);
    }
    table["rules"] = Item::ArrayOfTables(rules);
    doc["say_detection"] = Item::Table(table);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("Failed to determine file name for {}", path.display()))?;

    let temp_path = path.with_file_name(format!(
        ".{file_name}.tmp.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("Failed to build temp path: {error}"))?
            .as_nanos()
    ));

    let mut temp_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|error| format!("Failed to create temp file: {error}"))?;
    std::io::Write::write_all(&mut temp_file, doc.to_string().as_bytes())
        .map_err(|error| format!("Failed to write temp file: {error}"))?;
    temp_file
        .sync_all()
        .map_err(|error| format!("Failed to sync temp file: {error}"))?;
    drop(temp_file);

    std::fs::rename(&temp_path, &path).map_err(|error| {
        let _ = std::fs::remove_file(&temp_path);
        format!(
            "Failed to replace {} with {}: {error}",
            path.display(),
            temp_path.display()
        )
    })?;

    Ok(())
}

async fn get_say_detection_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = read_say_detection_config_from_disk().unwrap_or_default();
    if let Some(say_detection) = &state.say_detection {
        *say_detection.config.write().await = config;
        return (StatusCode::OK, Json(say_detection.get_status().await)).into_response();
    }

    (
        StatusCode::OK,
        Json(SayDetectionStatus {
            config,
            total_matches: 0,
            last_match: None,
        }),
    )
        .into_response()
}

async fn get_say_detection_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match read_say_detection_config_from_disk() {
        Ok(config) => {
            if let Some(say_detection) = &state.say_detection {
                *say_detection.config.write().await = config.clone();
            }
            (StatusCode::OK, Json(config)).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": error })),
        )
            .into_response(),
    }
}

async fn put_say_detection_config(
    State(state): State<Arc<AppState>>,
    Json(config): Json<SayDetectionConfig>,
) -> impl IntoResponse {
    match write_say_detection_config_to_disk(&config) {
        Ok(()) => {
            if let Some(say_detection) = &state.say_detection {
                *say_detection.config.write().await = config.clone();
            }
            (StatusCode::OK, Json(config)).into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": error })),
        )
            .into_response(),
    }
}

async fn get_say_detection_rules(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match read_say_detection_config_from_disk() {
        Ok(config) => {
            if let Some(say_detection) = &state.say_detection {
                *say_detection.config.write().await = config.clone();
            }
            (StatusCode::OK, Json(config.rules)).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": error })),
        )
            .into_response(),
    }
}

async fn post_say_detection_rule(
    State(state): State<Arc<AppState>>,
    Json(rule): Json<SayDetectionRule>,
) -> impl IntoResponse {
    let mut config = match read_say_detection_config_from_disk() {
        Ok(config) => config,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": error })),
            )
                .into_response();
        }
    };
    config.rules.push(rule);

    match write_say_detection_config_to_disk(&config) {
        Ok(()) => {
            if let Some(say_detection) = &state.say_detection {
                *say_detection.config.write().await = config;
            }
            StatusCode::CREATED.into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": error })),
        )
            .into_response(),
    }
}

async fn delete_say_detection_rule(
    State(state): State<Arc<AppState>>,
    Path(rule_name): Path<String>,
) -> impl IntoResponse {
    let mut config = match read_say_detection_config_from_disk() {
        Ok(config) => config,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": error })),
            )
                .into_response();
        }
    };
    config.rules.retain(|rule| rule.name != rule_name);

    match write_say_detection_config_to_disk(&config) {
        Ok(()) => {
            if let Some(say_detection) = &state.say_detection {
                *say_detection.config.write().await = config;
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": error })),
        )
            .into_response(),
    }
}

async fn post_say_detection_sync(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SayDetectionSyncPayload>,
) -> impl IntoResponse {
    if let Some(say_detection) = &state.say_detection {
        say_detection.record_match(payload).await;
    }

    (StatusCode::OK, Json(serde_json::json!({ "synced": true }))).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/status", get(get_say_detection_status))
        .route(
            "/config",
            get(get_say_detection_config).put(put_say_detection_config),
        )
        .route(
            "/rules",
            get(get_say_detection_rules).post(post_say_detection_rule),
        )
        .route("/rules/{rule_name}", delete(delete_say_detection_rule))
        .route("/sync", post(post_say_detection_sync))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn say_detection_config_default() {
        let config = SayDetectionConfig::default();
        assert!(!config.enabled);
        assert!(config.sound_enabled);
        assert!(config.toast_enabled);
        assert_eq!(config.sound_file.as_deref(), Some("say_alert.wav"));
        assert!(config.rules.is_empty());
    }

    #[test]
    fn say_detection_status_default() {
        let status = SayDetectionStatus::default();
        assert_eq!(status.total_matches, 0);
        assert!(status.last_match.is_none());
        assert!(!status.config.enabled);
    }
}
