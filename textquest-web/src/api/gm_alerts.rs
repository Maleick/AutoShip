//! GM Alert configuration and status API.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmSyncPayload {
    pub is_gm_in_zone: bool,
    pub gm_count: usize,
    pub gm_names: Vec<String>,
    pub automation_paused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmAlertConfig {
    pub enabled: bool,
    pub sound_enabled: bool,
    pub sound_file: Option<String>,
    pub toast_enabled: bool,
    pub auto_pause_enabled: bool,
    pub discord_webhook_url: Option<String>,
    pub broadcast_all_clients: bool,
}

impl Default for GmAlertConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sound_enabled: true,
            sound_file: Some("gm_alert.wav".to_string()),
            toast_enabled: true,
            auto_pause_enabled: false,
            discord_webhook_url: None,
            broadcast_all_clients: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmPresenceStatus {
    pub is_gm_in_zone: bool,
    pub gm_count: usize,
    pub gm_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmAlertStatus {
    pub config: GmAlertConfig,
    pub presence: GmPresenceStatus,
    pub automation_paused: bool,
}

pub struct GmAlertState {
    pub config: RwLock<GmAlertConfig>,
    pub is_gm_in_zone: AtomicBool,
    pub gm_names: RwLock<Vec<String>>,
    pub gm_count: RwLock<usize>,
    pub automation_paused: AtomicBool,
}

impl Default for GmAlertState {
    fn default() -> Self {
        Self {
            config: RwLock::new(GmAlertConfig::default()),
            is_gm_in_zone: AtomicBool::new(false),
            gm_names: RwLock::new(Vec::new()),
            gm_count: RwLock::new(0),
            automation_paused: AtomicBool::new(false),
        }
    }
}

impl GmAlertState {
    pub fn new_demo() -> Self {
        Self::default()
    }
}

fn read_gm_config_from_disk() -> Result<GmAlertConfig, String> {
    let path = std::env::var("TEXTQUEST_CONFIG_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("config/textquest.toml"));

    if !path.exists() {
        return Ok(GmAlertConfig::default());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;

    let doc: toml_edit::DocumentMut = content
        .parse()
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;

    let Some(item) = doc.get("gm_alert") else {
        return Ok(GmAlertConfig::default());
    };

    toml_edit::de::from_str::<GmAlertConfig>(&item.to_string())
        .map_err(|error| format!("Failed to decode [gm_alert]: {error}"))
}

fn write_gm_config_to_disk(config: &GmAlertConfig) -> Result<(), String> {
    let path = std::env::var("TEXTQUEST_CONFIG_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("config/textquest.toml"));

    let mut doc = if path.exists() {
        let content = std::fs::read_to_string(&path)
            .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
        content
            .parse()
            .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?
    } else {
        toml_edit::DocumentMut::new()
    };

    let mut table = toml_edit::Table::new();
    table["enabled"] = toml_edit::value(config.enabled);
    table["sound_enabled"] = toml_edit::value(config.sound_enabled);
    table["sound_file"] = toml_edit::value(config.sound_file.clone());
    table["toast_enabled"] = toml_edit::value(config.toast_enabled);
    table["auto_pause_enabled"] = toml_edit::value(config.auto_pause_enabled);
    table["discord_webhook_url"] = toml_edit::value(config.discord_webhook_url.clone());
    table["broadcast_all_clients"] = toml_edit::value(config.broadcast_all_clients);
    doc["gm_alert"] = toml_edit::Item::Table(table);

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

pub async fn get_gm_alert_config() -> impl IntoResponse {
    match read_gm_config_from_disk() {
        Ok(config) => (StatusCode::OK, Json(config)).into_response(),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": error }))).into_response(),
    }
}

pub async fn put_gm_alert_config(Json(config): Json<GmAlertConfig>) -> impl IntoResponse {
    match write_gm_config_to_disk(&config) {
        Ok(()) => (StatusCode::OK, Json(config)).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": error }))).into_response(),
    }
}

pub async fn get_gm_alert_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = read_gm_config_from_disk().unwrap_or_default();
    let presence = GmPresenceStatus {
        is_gm_in_zone: state.gm_alert_state.is_gm_in_zone.load(Ordering::Relaxed),
        gm_count: *state.gm_alert_state.gm_count.read().await,
        gm_names: state.gm_alert_state.gm_names.read().await.clone(),
    };
    let status = GmAlertStatus {
        config,
        presence,
        automation_paused: state.gm_alert_state.automation_paused.load(Ordering::Relaxed),
    };
    (StatusCode::OK, Json(status)).into_response()
}

pub async fn post_gm_alert_sync(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<GmSyncPayload>,
) -> impl IntoResponse {
    state.gm_alert_state.is_gm_in_zone.store(payload.is_gm_in_zone, Ordering::Relaxed);
    *state.gm_alert_state.gm_count.write().await = payload.gm_count;
    *state.gm_alert_state.gm_names.write().await = payload.gm_names;
    state.gm_alert_state.automation_paused.store(payload.automation_paused, Ordering::Relaxed);
    (StatusCode::OK, Json(serde_json::json!({ "synced": true }))).into_response()
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/config", get(get_gm_alert_config).put(put_gm_alert_config))
        .route("/status", get(get_gm_alert_status))
        .route("/sync", post(post_gm_alert_sync))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = GmAlertConfig::default();
        assert!(config.enabled);
        assert!(config.sound_enabled);
        assert!(config.toast_enabled);
        assert!(!config.auto_pause_enabled);
        assert!(config.broadcast_all_clients);
    }

    #[test]
    fn gm_alert_status_serialization() {
        let status = GmAlertStatus {
            config: GmAlertConfig::default(),
            presence: GmPresenceStatus {
                is_gm_in_zone: false,
                gm_count: 0,
                gm_names: vec![],
            },
            automation_paused: false,
        };

        let json = serde_json::to_value(&status).expect("should serialize");
        assert!(json.is_object());
        assert!(json.get("config").is_some());
        assert!(json.get("presence").is_some());
        assert!(json.get("automationPaused").is_some());
    }
}
