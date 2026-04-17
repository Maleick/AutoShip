//! REST API handlers for the web dashboard.

pub mod alerts;
pub mod chat_log;
pub mod chat_pattern_rules;
pub mod dashboard;
pub mod discord;
pub mod economy;
pub mod gm_alerts;
pub mod kill_tracker;
pub mod loot;
pub mod player_watch;
pub mod say_detection;
pub mod soul;
pub mod spawn_alerts;
pub mod xassist;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
pub use player_watch::PlayerWatchConfig;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path as StdPath, PathBuf},
    sync::Arc,
};
use textquest_common::box_chat::BoxChatConfig;
use textquest_common::character_config as shared_character_config;
use textquest_common::ipc::{AutoAcceptSettings, AutoRezConfig};
use toml_edit::{DocumentMut, Item, Table, value};

use crate::AppState;
use textquest_common::shared_client_state::SharedClientState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub error: String,
}

fn json_error(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}

fn live_state_unavailable(message: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    json_error(StatusCode::NOT_IMPLEMENTED, message)
}

/// Catch-all for unknown API routes so they do not fall through to the SPA.
pub async fn api_not_found() -> impl IntoResponse {
    json_error(StatusCode::NOT_FOUND, "API route not found")
}

/// Placeholder response for known raid-config endpoints that are not
/// implemented on this build.
pub async fn raid_config_unavailable() -> impl IntoResponse {
    json_error(
        StatusCode::NOT_IMPLEMENTED,
        "Raid configuration API is not implemented in this build",
    )
}

/// Placeholder response for known character-config list endpoint.
pub async fn character_configs_unavailable() -> impl IntoResponse {
    json_error(
        StatusCode::NOT_IMPLEMENTED,
        "Character configuration API is not implemented in this build",
    )
}

/// Placeholder response for known per-character config mutation endpoint.
pub async fn character_config_unavailable(Path(character): Path<String>) -> impl IntoResponse {
    json_error(
        StatusCode::NOT_IMPLEMENTED,
        format!("Character configuration API is not implemented for '{character}'"),
    )
}

// ─── Health
// ───────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

/// Health check endpoint.
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

// ─── Box Chat Settings ──────────────────────────────────────────────────────

fn textquest_config_path() -> PathBuf {
    std::env::var("TEXTQUEST_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/textquest.toml"))
}

fn read_box_chat_settings_from_disk() -> Result<BoxChatConfig, String> {
    let path = textquest_config_path();
    if !path.exists() {
        return Ok(BoxChatConfig::default());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    let doc = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;

    let Some(item) = doc.get("box_chat") else {
        return Ok(BoxChatConfig::default());
    };
    let settings = toml_edit::de::from_str::<BoxChatConfig>(&item.to_string())
        .map_err(|error| format!("Failed to decode [box_chat]: {error}"))?;
    Ok(settings)
}

fn write_box_chat_settings_to_disk(settings: &BoxChatConfig) -> Result<(), String> {
    if settings.host.trim().is_empty() {
        return Err("Box chat host must not be empty".to_string());
    }
    if settings.port == 0 {
        return Err("Box chat port must be between 1 and 65535".to_string());
    }

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
    table["enabled"] = value(settings.enabled);
    table["host"] = value(settings.host.clone());
    table["port"] = value(i64::from(settings.port));
    table["auto_connect"] = value(settings.auto_connect);
    doc["box_chat"] = Item::Table(table);

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
            .map_err(|error| format!("Failed to build temp path for {}: {error}", path.display()))?
            .as_nanos()
    ));

    let mut temp_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|error| {
            format!(
                "Failed to create temp file {}: {error}",
                temp_path.display()
            )
        })?;
    std::io::Write::write_all(&mut temp_file, doc.to_string().as_bytes())
        .map_err(|error| format!("Failed to write temp file {}: {error}", temp_path.display()))?;
    temp_file
        .sync_all()
        .map_err(|error| format!("Failed to sync temp file {}: {error}", temp_path.display()))?;
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

/// GET /api/box-chat/settings — read persisted EQBC-style relay settings.
pub async fn get_box_chat_settings() -> impl IntoResponse {
    match read_box_chat_settings_from_disk() {
        Ok(settings) => (StatusCode::OK, Json(settings)).into_response(),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error).into_response(),
    }
}

/// PUT /api/box-chat/settings — persist EQBC-style relay settings.
pub async fn put_box_chat_settings(Json(settings): Json<BoxChatConfig>) -> impl IntoResponse {
    match write_box_chat_settings_to_disk(&settings) {
        Ok(()) => (StatusCode::OK, Json(settings)).into_response(),
        Err(error) => json_error(StatusCode::BAD_REQUEST, error).into_response(),
    }
}

// ─── Chat Log Settings ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatLogSettings {
    pub enabled: bool,
    pub channels: Vec<String>,
    pub rotation_strategy: String,
    pub max_file_size_bytes: u64,
    pub min_level: String,
    pub log_eq_chat: bool,
}

impl Default for ChatLogSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            channels: vec!["mq2".to_string()],
            rotation_strategy: "size:10".to_string(),
            max_file_size_bytes: 10 * 1024 * 1024,
            min_level: "info".to_string(),
            log_eq_chat: false,
        }
    }
}

fn read_chat_log_settings_from_disk() -> Result<ChatLogSettings, String> {
    let path = textquest_config_path();
    if !path.exists() {
        return Ok(ChatLogSettings::default());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    let doc = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;

    let Some(item) = doc.get("chat_log") else {
        return Ok(ChatLogSettings::default());
    };

    let mut settings = ChatLogSettings::default();

    if let Some(enabled) = item.get("enabled")
        && let Some(val) = enabled.as_bool()
    {
        settings.enabled = val;
    }

    if let Some(channels) = item.get("channels")
        && let Ok(ch) = toml_edit::de::from_str::<Vec<String>>(&channels.to_string())
    {
        settings.channels = ch;
    }

    if let Some(rotation) = item.get("rotation_strategy") {
        settings.rotation_strategy = rotation.to_string().trim_matches('"').to_string();
    }

    if let Some(size) = item.get("max_file_size_bytes")
        && let Some(val) = size.as_integer()
    {
        settings.max_file_size_bytes = val as u64;
    }

    if let Some(level) = item.get("min_level") {
        settings.min_level = level.to_string().trim_matches('"').to_string();
    }

    if let Some(eq_chat) = item.get("log_eq_chat")
        && let Some(val) = eq_chat.as_bool()
    {
        settings.log_eq_chat = val;
    }

    Ok(settings)
}

fn write_chat_log_settings_to_disk(settings: &ChatLogSettings) -> Result<(), String> {
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
    table["enabled"] = value(settings.enabled);
    let mut channels = toml_edit::Array::new();
    for channel in &settings.channels {
        channels.push(channel.as_str());
    }
    table["channels"] = Item::Value(toml_edit::Value::Array(channels));
    table["rotation_strategy"] = value(settings.rotation_strategy.clone());
    table["max_file_size_bytes"] = value(settings.max_file_size_bytes as i64);
    table["min_level"] = value(settings.min_level.clone());
    table["log_eq_chat"] = value(settings.log_eq_chat);
    doc["chat_log"] = Item::Table(table);

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
            .map_err(|error| format!("Failed to build temp path for {}: {error}", path.display()))?
            .as_nanos()
    ));

    let mut temp_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|error| {
            format!(
                "Failed to create temp file {}: {error}",
                temp_path.display()
            )
        })?;
    std::io::Write::write_all(&mut temp_file, doc.to_string().as_bytes())
        .map_err(|error| format!("Failed to write temp file {}: {error}", temp_path.display()))?;
    temp_file
        .sync_all()
        .map_err(|error| format!("Failed to sync temp file {}: {error}", temp_path.display()))?;
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

/// GET /api/chat-log/settings — read persisted chat log settings.
pub async fn get_chat_log_settings() -> impl IntoResponse {
    match read_chat_log_settings_from_disk() {
        Ok(settings) => (StatusCode::OK, Json(settings)).into_response(),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error).into_response(),
    }
}

/// PUT /api/chat-log/settings — persist chat log settings.
pub async fn put_chat_log_settings(Json(settings): Json<ChatLogSettings>) -> impl IntoResponse {
    match write_chat_log_settings_to_disk(&settings) {
        Ok(()) => (StatusCode::OK, Json(settings)).into_response(),
        Err(error) => json_error(StatusCode::BAD_REQUEST, error).into_response(),
    }
}

// ─── Player Watch API ───────────────────────────────────────────────────────────

/// GET /api/config/player-watch — get player watch configuration.
pub async fn get_player_watch_config(
    State(state): State<Arc<AppState>>,
) -> Json<PlayerWatchConfig> {
    Json(state.player_watch_config.read().await.clone())
}

/// PUT /api/config/player-watch — update player watch configuration.
pub async fn put_player_watch_config(
    State(state): State<Arc<AppState>>,
    Json(settings): Json<PlayerWatchConfig>,
) -> Json<PlayerWatchConfig> {
    let response = settings.clone();
    *state.player_watch_config.write().await = settings;
    Json(response)
}
// ─── Sessions
// ─────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SessionInfo {
    pub client_id: u32,
    pub character_name: String,
    pub zone: String,
    pub level: u8,
    pub hp_pct: f32,
    pub mana_pct: f32,
    pub endurance_pct: f32,
    pub status: String,
    pub buff_count: usize,
    pub target_name: Option<String>,
    pub target_hp_pct: Option<f32>,
    pub pet_name: Option<String>,
}

fn read_live_sessions(path: &StdPath) -> anyhow::Result<Vec<SharedClientState>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let payload = std::fs::read(path)?;
    let sessions = serde_json::from_slice::<Vec<SharedClientState>>(&payload)?;
    Ok(sessions)
}

/// List active sessions.
pub async fn list_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let live_sessions = match read_live_sessions(&state.live_session_snapshot_path) {
        Ok(sessions) => sessions,
        Err(error) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read live session snapshot: {error}"),
            )
            .into_response();
        }
    };

    let sessions: Vec<SessionInfo> = if live_sessions.is_empty() {
        let configs = state.character_configs.read().await;
        configs
            .values()
            .enumerate()
            .map(|(idx, cfg)| SessionInfo {
                client_id: idx as u32 + 1,
                character_name: cfg.character_name.clone(),
                zone: "Unknown".to_string(),
                level: 50,
                hp_pct: 100.0,
                mana_pct: 100.0,
                endurance_pct: 100.0,
                status: "idle".to_string(),
                buff_count: 0,
                target_name: None,
                target_hp_pct: None,
                pet_name: None,
            })
            .collect()
    } else {
        live_sessions
            .into_iter()
            .map(|session| SessionInfo {
                client_id: session.client_id,
                character_name: session.character_name,
                zone: if session.zone_long_name.is_empty() {
                    session.zone_short_name
                } else {
                    session.zone_long_name
                },
                level: session.level,
                hp_pct: session.hp_pct,
                mana_pct: session.mana_pct,
                endurance_pct: session.endurance_pct,
                status: session.status,
                buff_count: session.buffs.len(),
                target_name: session.target.as_ref().map(|target| target.name.clone()),
                target_hp_pct: session.target.as_ref().map(|target| target.hp_pct),
                pet_name: session.pet.as_ref().map(|pet| pet.name.clone()),
            })
            .collect()
    };

    (StatusCode::OK, Json(sessions)).into_response()
}

// ─── Strategy tuning ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationEntry {
    pub id: String,
    pub name: String,
    pub priority: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClassParams {
    pub ch_chain_timing_ms: Option<u32>,
    pub dot_overlap_pct: Option<u8>,
    pub burn_at_hp_pct: Option<u8>,
    pub slow_at_hp_pct: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TributeAlertState {
    Ok,
    Expiring,
    Expired,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TributePreferences {
    pub auto_activate: bool,
    pub warning_threshold_secs: u64,
    pub preferred_tributes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TributeStatus {
    pub active: bool,
    pub remaining_secs: u64,
    pub point_balance: u32,
    pub active_tributes: Vec<String>,
    pub alert_state: TributeAlertState,
}

impl Default for TributeStatus {
    fn default() -> Self {
        Self {
            active: false,
            remaining_secs: 0,
            point_balance: 0,
            active_tributes: Vec::new(),
            alert_state: TributeAlertState::Expired,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterConfig {
    pub character_name: String,
    pub class: String,
    pub role: String,
    pub heal_at_pct: u8,
    pub mana_sit_pct: u8,
    pub nuke_at_pct: u8,
    pub rotation: Vec<RotationEntry>,
    pub class_params: ClassParams,
    #[serde(default)]
    pub auto_rez: AutoRezConfig,
    pub group_override: bool,
    pub group_name: Option<String>,
    #[serde(default = "textquest_common::window_title::default_window_title_format")]
    pub window_title_format: String,
    #[serde(default)]
    pub reward_automation: shared_character_config::RewardAutomationConfig,
    #[serde(default)]
    pub tribute_preferences: TributePreferences,
    #[serde(default)]
    pub tribute_status: TributeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampFormat {
    #[default]
    DateTime24,
    Time24,
    DateTime12,
    Time12,
}

impl From<textquest_common::chat::TimestampFormat> for TimestampFormat {
    fn from(f: textquest_common::chat::TimestampFormat) -> Self {
        match f {
            textquest_common::chat::TimestampFormat::DateTime24 => TimestampFormat::DateTime24,
            textquest_common::chat::TimestampFormat::Time24 => TimestampFormat::Time24,
            textquest_common::chat::TimestampFormat::DateTime12 => TimestampFormat::DateTime12,
            textquest_common::chat::TimestampFormat::Time12 => TimestampFormat::Time12,
        }
    }
}

impl From<TimestampFormat> for textquest_common::chat::TimestampFormat {
    fn from(f: TimestampFormat) -> Self {
        match f {
            TimestampFormat::DateTime24 => textquest_common::chat::TimestampFormat::DateTime24,
            TimestampFormat::Time24 => textquest_common::chat::TimestampFormat::Time24,
            TimestampFormat::DateTime12 => textquest_common::chat::TimestampFormat::DateTime12,
            TimestampFormat::Time12 => textquest_common::chat::TimestampFormat::Time12,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimestampConfig {
    pub enabled: bool,
    pub format: TimestampFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterConfigUpdate {
    pub character_name: String,
    pub class: String,
    pub role: String,
    pub heal_at_pct: u8,
    pub mana_sit_pct: u8,
    pub nuke_at_pct: u8,
    pub rotation: Vec<RotationEntry>,
    pub class_params: ClassParams,
    pub auto_rez: Option<AutoRezConfig>,
    pub group_override: bool,
    pub group_name: Option<String>,
    pub window_title_format: Option<String>,
    pub reward_automation: Option<shared_character_config::RewardAutomationConfig>,
    pub tribute_preferences: Option<TributePreferences>,
}

fn tribute_preferences(
    preferred_tributes: &[&str],
    warning_threshold_secs: u64,
) -> TributePreferences {
    TributePreferences {
        auto_activate: true,
        warning_threshold_secs,
        preferred_tributes: preferred_tributes
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
    }
}

fn tribute_status(
    active: bool,
    remaining_secs: u64,
    point_balance: u32,
    active_tributes: &[&str],
    alert_state: TributeAlertState,
) -> TributeStatus {
    TributeStatus {
        active,
        remaining_secs,
        point_balance,
        active_tributes: active_tributes
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
        alert_state,
    }
}

fn character_configs_path() -> PathBuf {
    textquest_config_path()
        .parent()
        .map(|parent| parent.join("character-configs.json"))
        .unwrap_or_else(|| PathBuf::from("config/character-configs.json"))
}

fn from_shared_tribute_alert_state(
    value: shared_character_config::TributeAlertState,
) -> TributeAlertState {
    match value {
        shared_character_config::TributeAlertState::Ok => TributeAlertState::Ok,
        shared_character_config::TributeAlertState::Expiring => TributeAlertState::Expiring,
        shared_character_config::TributeAlertState::Expired => TributeAlertState::Expired,
    }
}

fn to_shared_tribute_alert_state(
    value: TributeAlertState,
) -> shared_character_config::TributeAlertState {
    match value {
        TributeAlertState::Ok => shared_character_config::TributeAlertState::Ok,
        TributeAlertState::Expiring => shared_character_config::TributeAlertState::Expiring,
        TributeAlertState::Expired => shared_character_config::TributeAlertState::Expired,
    }
}

fn from_shared_character_config(
    config: shared_character_config::CharacterConfig,
) -> CharacterConfig {
    CharacterConfig {
        character_name: config.character_name,
        class: config.class,
        role: config.role,
        heal_at_pct: config.heal_at_pct,
        mana_sit_pct: config.mana_sit_pct,
        nuke_at_pct: config.nuke_at_pct,
        rotation: config
            .rotation
            .into_iter()
            .map(|entry| RotationEntry {
                id: entry.id,
                name: entry.name,
                priority: entry.priority,
                enabled: entry.enabled,
            })
            .collect(),
        class_params: ClassParams {
            ch_chain_timing_ms: config.class_params.ch_chain_timing_ms,
            dot_overlap_pct: config.class_params.dot_overlap_pct,
            burn_at_hp_pct: config.class_params.burn_at_hp_pct,
            slow_at_hp_pct: config.class_params.slow_at_hp_pct,
        },
        auto_rez: config.auto_rez,
        group_override: config.group_override,
        group_name: config.group_name,
        window_title_format: config.window_title_format,
        reward_automation: config.reward_automation,
        tribute_preferences: TributePreferences {
            auto_activate: config.tribute_preferences.auto_activate,
            warning_threshold_secs: config.tribute_preferences.warning_threshold_secs,
            preferred_tributes: config.tribute_preferences.preferred_tributes,
        },
        tribute_status: TributeStatus {
            active: config.tribute_status.active,
            remaining_secs: config.tribute_status.remaining_secs,
            point_balance: config.tribute_status.point_balance,
            active_tributes: config.tribute_status.active_tributes,
            alert_state: from_shared_tribute_alert_state(config.tribute_status.alert_state),
        },
    }
}

fn to_shared_character_config(config: CharacterConfig) -> shared_character_config::CharacterConfig {
    shared_character_config::CharacterConfig {
        character_name: config.character_name,
        class: config.class,
        role: config.role,
        heal_at_pct: config.heal_at_pct,
        mana_sit_pct: config.mana_sit_pct,
        nuke_at_pct: config.nuke_at_pct,
        rotation: config
            .rotation
            .into_iter()
            .map(|entry| shared_character_config::RotationEntry {
                id: entry.id,
                name: entry.name,
                priority: entry.priority,
                enabled: entry.enabled,
            })
            .collect(),
        class_params: shared_character_config::ClassParams {
            ch_chain_timing_ms: config.class_params.ch_chain_timing_ms,
            dot_overlap_pct: config.class_params.dot_overlap_pct,
            burn_at_hp_pct: config.class_params.burn_at_hp_pct,
            slow_at_hp_pct: config.class_params.slow_at_hp_pct,
        },
        auto_rez: config.auto_rez,
        group_override: config.group_override,
        group_name: config.group_name,
        window_title_format: config.window_title_format,
        reward_automation: config.reward_automation,
        tribute_preferences: shared_character_config::TributePreferences {
            auto_activate: config.tribute_preferences.auto_activate,
            warning_threshold_secs: config.tribute_preferences.warning_threshold_secs,
            preferred_tributes: config.tribute_preferences.preferred_tributes,
        },
        tribute_status: shared_character_config::TributeStatus {
            active: config.tribute_status.active,
            remaining_secs: config.tribute_status.remaining_secs,
            point_balance: config.tribute_status.point_balance,
            active_tributes: config.tribute_status.active_tributes,
            alert_state: to_shared_tribute_alert_state(config.tribute_status.alert_state),
        },
    }
}

pub(crate) fn load_character_configs_from_path(
    path: &std::path::Path,
) -> Result<HashMap<String, CharacterConfig>, String> {
    shared_character_config::load_character_configs(path)
        .map(|configs| {
            configs
                .into_iter()
                .map(|(name, config)| (name, from_shared_character_config(config)))
                .collect()
        })
        .map_err(|error| format!("Failed to load {}: {error}", path.display()))
}

fn load_character_configs_from_disk() -> Result<HashMap<String, CharacterConfig>, String> {
    load_character_configs_from_path(&character_configs_path())
}

fn write_character_configs_to_path(
    path: &std::path::Path,
    configs: &HashMap<String, CharacterConfig>,
) -> Result<(), String> {
    let shared = configs
        .iter()
        .map(|(name, config)| (name.clone(), to_shared_character_config(config.clone())))
        .collect::<HashMap<_, _>>();
    shared_character_config::save_character_configs(path, &shared)
        .map_err(|error| format!("Failed to save {}: {error}", path.display()))
}

fn write_character_configs_to_disk(
    configs: &HashMap<String, CharacterConfig>,
) -> Result<(), String> {
    write_character_configs_to_path(&character_configs_path(), configs)
}

pub fn initial_character_configs() -> HashMap<String, CharacterConfig> {
    let mut configs = demo_character_configs();
    match load_character_configs_from_disk() {
        Ok(persisted) => {
            for (name, config) in persisted {
                let demo_status = configs
                    .get(&name)
                    .map(|existing| existing.tribute_status.clone());
                let mut merged = config;
                if merged.tribute_status == TributeStatus::default()
                    && let Some(status) = demo_status
                {
                    merged.tribute_status = status;
                }
                configs.insert(name, merged);
            }
        }
        Err(error) => {
            tracing::warn!(%error, "Failed to load character configs from disk");
        }
    }
    configs
}

pub fn demo_character_configs() -> HashMap<String, CharacterConfig> {
    let mut configs = HashMap::new();
    for cfg in [
        CharacterConfig {
            character_name: "Frostreaver".into(),
            class: "Cleric".into(),
            role: "Healer".into(),
            heal_at_pct: 70,
            mana_sit_pct: 25,
            nuke_at_pct: 90,
            rotation: vec![
                RotationEntry {
                    id: "complete_heal".into(),
                    name: "Complete Heal".into(),
                    priority: 1,
                    enabled: true,
                },
                RotationEntry {
                    id: "celestial_healing".into(),
                    name: "Celestial Healing".into(),
                    priority: 2,
                    enabled: true,
                },
            ],
            class_params: ClassParams {
                ch_chain_timing_ms: Some(2500),
                ..ClassParams::default()
            },
            auto_rez: AutoRezConfig {
                enabled: true,
                min_xp_pct: 96,
                trusted_casters: vec!["Highclerk".into(), "Leafbinder".into()],
                decline_if_untrusted: true,
                delay_ms: 5_000,
            },
            group_override: false,
            group_name: Some("Group 1".into()),
            window_title_format: textquest_common::window_title::default_window_title_format(),
            reward_automation: shared_character_config::RewardAutomationConfig::default(),
            tribute_preferences: tribute_preferences(&["Marr's Gift", "Champion's Aura"], 300),
            tribute_status: tribute_status(
                true,
                240,
                3_200,
                &["Marr's Gift"],
                TributeAlertState::Expiring,
            ),
        },
        CharacterConfig {
            character_name: "Noxus".into(),
            class: "Warrior".into(),
            role: "Tank".into(),
            heal_at_pct: 35,
            mana_sit_pct: 0,
            nuke_at_pct: 100,
            rotation: vec![RotationEntry {
                id: "taunt".into(),
                name: "Taunt".into(),
                priority: 1,
                enabled: true,
            }],
            class_params: ClassParams::default(),
            auto_rez: AutoRezConfig {
                enabled: false,
                min_xp_pct: 90,
                trusted_casters: vec!["Frostreaver".into()],
                decline_if_untrusted: false,
                delay_ms: 3_000,
            },
            group_override: false,
            group_name: Some("Group 1".into()),
            window_title_format: textquest_common::window_title::default_window_title_format(),
            reward_automation: shared_character_config::RewardAutomationConfig::default(),
            tribute_preferences: tribute_preferences(&["Stalwart Ward", "Champion's Aura"], 420),
            tribute_status: tribute_status(
                true,
                3_600,
                1_950,
                &["Stalwart Ward", "Champion's Aura"],
                TributeAlertState::Ok,
            ),
        },
        CharacterConfig {
            character_name: "Aelrindel".into(),
            class: "Wizard".into(),
            role: "DPS".into(),
            heal_at_pct: 45,
            mana_sit_pct: 20,
            nuke_at_pct: 80,
            rotation: vec![RotationEntry {
                id: "ice_comet".into(),
                name: "Ice Comet".into(),
                priority: 1,
                enabled: true,
            }],
            class_params: ClassParams {
                burn_at_hp_pct: Some(30),
                ..ClassParams::default()
            },
            auto_rez: AutoRezConfig {
                enabled: true,
                min_xp_pct: 90,
                trusted_casters: vec!["Frostreaver".into(), "Oakmantle".into()],
                decline_if_untrusted: false,
                delay_ms: 2_500,
            },
            group_override: false,
            group_name: Some("Group 2".into()),
            window_title_format: textquest_common::window_title::default_window_title_format(),
            reward_automation: shared_character_config::RewardAutomationConfig::default(),
            tribute_preferences: tribute_preferences(&["Arcane Fury", "Hero's Fortitude"], 180),
            tribute_status: tribute_status(false, 0, 875, &[], TributeAlertState::Expired),
        },
        CharacterConfig {
            character_name: "Grok".into(),
            class: "Shaman".into(),
            role: "Support".into(),
            heal_at_pct: 60,
            mana_sit_pct: 30,
            nuke_at_pct: 85,
            rotation: vec![RotationEntry {
                id: "turgurs_insects".into(),
                name: "Turgur's Insects".into(),
                priority: 1,
                enabled: true,
            }],
            class_params: ClassParams {
                slow_at_hp_pct: Some(95),
                ..ClassParams::default()
            },
            auto_rez: AutoRezConfig {
                enabled: true,
                min_xp_pct: 96,
                trusted_casters: vec!["Frostreaver".into()],
                decline_if_untrusted: true,
                delay_ms: 4_000,
            },
            group_override: false,
            group_name: Some("Group 2".into()),
            window_title_format: textquest_common::window_title::default_window_title_format(),
            reward_automation: shared_character_config::RewardAutomationConfig::default(),
            tribute_preferences: tribute_preferences(&["Ancient Bulwark", "Spirit's Resolve"], 300),
            tribute_status: tribute_status(
                true,
                1_020,
                1_480,
                &["Ancient Bulwark"],
                TributeAlertState::Ok,
            ),
        },
        CharacterConfig {
            character_name: "Valerius".into(),
            class: "Necromancer".into(),
            role: "DPS".into(),
            heal_at_pct: 40,
            mana_sit_pct: 15,
            nuke_at_pct: 75,
            rotation: vec![RotationEntry {
                id: "ignite_blood".into(),
                name: "Ignite Blood".into(),
                priority: 1,
                enabled: true,
            }],
            class_params: ClassParams {
                dot_overlap_pct: Some(10),
                ..ClassParams::default()
            },
            auto_rez: AutoRezConfig {
                enabled: true,
                min_xp_pct: 96,
                trusted_casters: vec!["Frostreaver".into()],
                decline_if_untrusted: true,
                delay_ms: 4_000,
            },
            group_override: false,
            group_name: Some("Group 3".into()),
            window_title_format: textquest_common::window_title::default_window_title_format(),
            reward_automation: shared_character_config::RewardAutomationConfig::default(),
            tribute_preferences: tribute_preferences(
                &["Fervor of Shadows", "Hero's Vitality"],
                240,
            ),
            tribute_status: tribute_status(
                true,
                150,
                2_250,
                &["Fervor of Shadows"],
                TributeAlertState::Expiring,
            ),
        },
    ] {
        configs.insert(cfg.character_name.clone(), cfg);
    }
    configs
}

/// GET /api/config/characters — list all character tuning configs.
/// Not yet mounted in the live API router (returns 501 via placeholder); kept
/// for future use.
#[allow(dead_code)]
pub async fn list_character_configs(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<CharacterConfig>> {
    let mut configs = {
        let configs_map = state.character_configs.read().await;
        configs_map.values().cloned().collect::<Vec<_>>()
    };
    configs.sort_by(|a, b| a.character_name.cmp(&b.character_name));
    Json(configs)
}

/// PUT /api/config/characters/:name — upsert per-character tuning config.
/// Not yet mounted in the live API router (returns 501 via placeholder); kept
/// for future use.
#[allow(dead_code)]
pub async fn put_character_config(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(config): Json<CharacterConfigUpdate>,
) -> Result<Json<CharacterConfig>, StatusCode> {
    if name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let _write_guard = state.character_config_write_lock.lock().await;
    let mut configs_map = state.character_configs.write().await;
    let existing = configs_map.get(&name).cloned();
    let saved = CharacterConfig {
        character_name: name,
        class: config.class,
        role: config.role,
        heal_at_pct: config.heal_at_pct,
        mana_sit_pct: config.mana_sit_pct,
        nuke_at_pct: config.nuke_at_pct,
        rotation: config.rotation,
        class_params: config.class_params,
        auto_rez: config.auto_rez.unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|cfg| cfg.auto_rez.clone())
                .unwrap_or_default()
        }),
        group_override: config.group_override,
        group_name: config.group_name,
        window_title_format: config
            .window_title_format
            .filter(|value| !value.trim().is_empty())
            .or_else(|| existing.as_ref().map(|cfg| cfg.window_title_format.clone()))
            .unwrap_or_else(textquest_common::window_title::default_window_title_format),
        reward_automation: config
            .reward_automation
            .or_else(|| existing.as_ref().map(|cfg| cfg.reward_automation.clone()))
            .unwrap_or_default(),
        tribute_preferences: config
            .tribute_preferences
            .or_else(|| existing.as_ref().map(|cfg| cfg.tribute_preferences.clone()))
            .unwrap_or_default(),
        tribute_status: existing
            .as_ref()
            .map(|cfg| cfg.tribute_status.clone())
            .unwrap_or_default(),
    };
    let previous = configs_map.insert(saved.character_name.clone(), saved.clone());
    if let Err(error) = write_character_configs_to_path(&state.character_config_path, &configs_map)
    {
        if let Some(previous) = previous {
            configs_map.insert(saved.character_name.clone(), previous);
        } else {
            configs_map.remove(&saved.character_name);
        }
        tracing::error!(%error, "Failed to persist character config update");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    Ok(Json(saved))
}

/// GET /api/config/auto-accept — return the current auto-accept policy.
pub async fn get_auto_accept_settings(
    State(state): State<Arc<AppState>>,
) -> Json<AutoAcceptSettings> {
    Json(state.auto_accept_settings.read().await.clone())
}

/// PUT /api/config/auto-accept — replace the current auto-accept policy.
pub async fn put_auto_accept_settings(
    State(state): State<Arc<AppState>>,
    Json(mut settings): Json<AutoAcceptSettings>,
) -> Result<Json<AutoAcceptSettings>, (StatusCode, Json<ErrorResponse>)> {
    let mut trusted_players = Vec::new();
    for player in settings.trusted_players {
        let trimmed = player.trim();
        if trimmed.is_empty() {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                "Trusted player names must not be blank",
            ));
        }
        if trusted_players
            .iter()
            .all(|existing: &String| !existing.eq_ignore_ascii_case(trimmed))
        {
            trusted_players.push(trimmed.to_string());
        }
    }
    settings.trusted_players = trusted_players;

    *state.auto_accept_settings.write().await = settings.clone();
    let applied = crate::live_ipc::apply_auto_accept_settings(&settings);
    tracing::info!(
        attempted_clients = applied.attempted,
        applied_clients = applied.applied,
        failed_clients = applied.failures.len(),
        "Updated auto-accept settings"
    );
    for (pid, error) in applied.failures {
        tracing::warn!(pid, %error, "Failed to apply auto-accept settings to live client");
    }
    Ok(Json(settings))
}
// ── Economy types
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KronoSettings {
    pub target_rate_per_day: u32,
    pub min_sell_price: u32,
    pub max_buy_price: u32,
    pub restock_threshold: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorRoute {
    pub id: String,
    pub zone: String,
    pub npc_name: String,
    pub path_notes: String,
    pub item_categories: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankingRule {
    pub id: String,
    pub item_category: String,
    pub deposit_threshold: u32,
    pub keep_on_hand: u32,
    pub auto_deposit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeskillSupply {
    pub id: String,
    pub skill: String,
    pub materials: Vec<String>,
    pub restock_quantity: u32,
    pub source_zone: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WealthSnapshot {
    pub timestamp: String,
    pub plat: u64,
    pub krono: u32,
    pub item_value_estimate: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WealthHistory {
    pub current: WealthSnapshot,
    pub snapshots: Vec<WealthSnapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EconomySettings {
    pub krono: KronoSettings,
    pub banking_rules: Vec<BankingRule>,
    pub tradeskill_supplies: Vec<TradeskillSupply>,
}

// ── Economy handlers
// ──────────────────────────────────────────────────────────

/// GET /api/economy/settings — return full economy configuration.
pub async fn get_economy_settings() -> impl IntoResponse {
    // Return default demo economy settings.
    // In production, this would load from a persisted config file or database.
    let settings = EconomySettings {
        krono: KronoSettings {
            target_rate_per_day: 5,
            min_sell_price: 900,
            max_buy_price: 850,
            restock_threshold: 3,
            enabled: true,
        },
        banking_rules: vec![BankingRule {
            id: "default-bank".into(),
            item_category: "Tradeskill".into(),
            deposit_threshold: 100,
            keep_on_hand: 20,
            auto_deposit: true,
        }],
        tradeskill_supplies: vec![TradeskillSupply {
            id: "pottery".into(),
            skill: "Pottery".into(),
            materials: vec!["Clay".into(), "Water".into()],
            restock_quantity: 50,
            source_zone: "South Karana".into(),
            enabled: true,
        }],
    };
    (StatusCode::OK, Json(settings)).into_response()
}

/// PUT /api/economy/settings — update full economy configuration.
pub async fn put_economy_settings(Json(settings): Json<EconomySettings>) -> impl IntoResponse {
    // In production, persist the settings to a config file or database.
    // For now, just acknowledge the update with a success status.
    tracing::debug!(
        "Economy settings updated: krono enabled = {}",
        settings.krono.enabled
    );
    StatusCode::NO_CONTENT.into_response()
}

/// GET /api/economy/vendor-routes — list all vendor routes.
pub async fn list_vendor_routes() -> impl IntoResponse {
    // Return demo vendor routes.
    // In production, this would load from a persisted config or database.
    let routes = vec![
        VendorRoute {
            id: "vr-qey".into(),
            zone: "Queynos Hills".into(),
            npc_name: "Merchant".into(),
            path_notes: "Near the stone".into(),
            item_categories: vec!["Armor".into(), "Weapons".into()],
            enabled: true,
        },
        VendorRoute {
            id: "vr-pok".into(),
            zone: "Plane of Knowledge".into(),
            npc_name: "Tradeskill Master".into(),
            path_notes: "Central tower".into(),
            item_categories: vec!["Tradeskill".into()],
            enabled: true,
        },
    ];
    (StatusCode::OK, Json(routes)).into_response()
}

/// POST /api/economy/vendor-routes — create a vendor route.
pub async fn create_vendor_route(Json(route): Json<VendorRoute>) -> impl IntoResponse {
    // In production, persist to a config file or database.
    tracing::debug!("Created vendor route: {}", route.id);
    (StatusCode::CREATED, Json(route)).into_response()
}

/// PUT /api/economy/vendor-routes/:id — update a vendor route.
pub async fn update_vendor_route(
    Path(id): Path<String>,
    Json(mut route): Json<VendorRoute>,
) -> impl IntoResponse {
    // In production, persist to a config file or database.
    route.id = id;
    tracing::debug!("Updated vendor route: {}", route.id);
    (StatusCode::OK, Json(route)).into_response()
}

/// DELETE /api/economy/vendor-routes/:id — delete a vendor route.
pub async fn delete_vendor_route(Path(id): Path<String>) -> impl IntoResponse {
    // In production, delete from a config file or database.
    tracing::debug!("Deleted vendor route: {}", id);
    StatusCode::NO_CONTENT.into_response()
}

/// GET /api/economy/wealth — wealth history and current snapshot.
pub async fn get_wealth() -> impl IntoResponse {
    // Return demo wealth history.
    // In production, this would load from a SQLite metrics database.
    let current = WealthSnapshot {
        timestamp: "2024-12-20T12:00:00Z".into(),
        plat: 250_000,
        krono: 15,
        item_value_estimate: 500_000,
    };

    let history = WealthHistory {
        current,
        snapshots: vec![
            WealthSnapshot {
                timestamp: "2024-12-20T11:00:00Z".into(),
                plat: 245_000,
                krono: 15,
                item_value_estimate: 495_000,
            },
            WealthSnapshot {
                timestamp: "2024-12-20T10:00:00Z".into(),
                plat: 240_000,
                krono: 14,
                item_value_estimate: 490_000,
            },
        ],
    };

    (StatusCode::OK, Json(history)).into_response()
}

// ─── Timestamp Config ───────────────────────────────────────────────────────

pub async fn list_timestamp_configs(
    State(state): State<Arc<AppState>>,
) -> Json<HashMap<String, TimestampConfig>> {
    if let Ok(from_disk) = load_timestamp_configs_from_disk() {
        let mut configs = state.timestamp_configs.write().await;
        *configs = from_disk;
    }
    let configs = state.timestamp_configs.read().await;
    Json(configs.clone())
}

pub async fn get_timestamp_config(
    State(state): State<Arc<AppState>>,
    Path(character): Path<String>,
) -> impl IntoResponse {
    if let Ok(from_disk) = load_timestamp_configs_from_disk() {
        let mut configs = state.timestamp_configs.write().await;
        *configs = from_disk;
    }

    let configs = state.timestamp_configs.read().await;
    let config = configs.get(&character).cloned().unwrap_or_default();
    (StatusCode::OK, Json(config)).into_response()
}

fn timestamp_config_path() -> PathBuf {
    std::env::var("TEXTQUEST_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/textquest.toml"))
        .parent()
        .map(|parent| parent.join("timestamp.toml"))
        .unwrap_or_else(|| PathBuf::from("config/timestamp.toml"))
}

fn load_timestamp_configs_from_disk() -> Result<HashMap<String, TimestampConfig>, String> {
    let path = timestamp_config_path();
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    toml::from_str(&content).map_err(|error| format!("Failed to parse {}: {error}", path.display()))
}

fn write_timestamp_configs_to_disk(
    configs: &HashMap<String, TimestampConfig>,
) -> Result<(), String> {
    let path = timestamp_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    let content = toml::to_string_pretty(configs)
        .map_err(|error| format!("Failed to serialize timestamp configs: {error}"))?;
    let temp_path = path.with_extension("toml.tmp");
    std::fs::write(&temp_path, content)
        .map_err(|error| format!("Failed to write temp file {}: {error}", temp_path.display()))?;
    std::fs::rename(&temp_path, &path).map_err(|error| {
        format!(
            "Failed to replace {} with {}: {error}",
            path.display(),
            temp_path.display()
        )
    })
}

pub async fn put_timestamp_config(
    State(state): State<Arc<AppState>>,
    Path(character): Path<String>,
    Json(config): Json<TimestampConfig>,
) -> Result<Json<TimestampConfig>, (StatusCode, Json<ErrorResponse>)> {
    if character.trim().is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "Character name must not be empty",
        ));
    }

    let mut configs = state.timestamp_configs.write().await;
    configs.insert(character.clone(), config.clone());
    if let Err(error) = write_timestamp_configs_to_disk(&configs) {
        return Err(json_error(StatusCode::INTERNAL_SERVER_ERROR, error));
    }

    tracing::debug!(
        %character,
        enabled = config.enabled,
        format = ?config.format,
        "Updated timestamp config"
    );
    Ok(Json(config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;
    use serde_json::Value;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use textquest_common::ipc::{AutoAcceptSettings, AutoAcceptTrustMode};

    fn test_live_session_snapshot_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../data/runtime/{name}"))
    }

    fn test_state(snapshot_name: &str) -> AppState {
        let mut state = crate::test_app_state();
        state.live_session_snapshot_path = test_live_session_snapshot_path(snapshot_name);
        state.character_configs = tokio::sync::RwLock::new(demo_character_configs());
        state
    }

    async fn error_response_json(response: axum::response::Response) -> (StatusCode, Value) {
        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should collect")
            .to_bytes();
        let value = serde_json::from_slice(&body).expect("body should be valid json");
        (status, value)
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let Json(resp) = health().await;
        assert_eq!(resp.status, "ok");
    }

    #[tokio::test]
    async fn sessions_returns_ok() {
        let state = Arc::new(test_state("api-sessions-ok.json"));
        let response = list_sessions(State(state)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn economy_settings_returns_ok() {
        let response = get_economy_settings().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn vendor_routes_returns_ok() {
        let response = list_vendor_routes().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn wealth_history_returns_ok() {
        let response = get_wealth().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn put_economy_settings_returns_no_content() {
        let settings = EconomySettings {
            krono: KronoSettings {
                target_rate_per_day: 5,
                min_sell_price: 900,
                max_buy_price: 850,
                restock_threshold: 3,
                enabled: false,
            },
            banking_rules: vec![],
            tradeskill_supplies: vec![],
        };
        let response = put_economy_settings(Json(settings)).await.into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn create_vendor_route_returns_created() {
        let route = VendorRoute {
            id: "vr-test".into(),
            zone: "Test Zone".into(),
            npc_name: "Test NPC".into(),
            path_notes: "Test notes".into(),
            item_categories: vec!["Test".into()],
            enabled: true,
        };
        let response = create_vendor_route(Json(route)).await.into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn update_vendor_route_returns_ok() {
        let route = VendorRoute {
            id: "ignored".into(),
            zone: "Test Zone".into(),
            npc_name: "Test NPC".into(),
            path_notes: "Test notes".into(),
            item_categories: vec!["Test".into()],
            enabled: true,
        };
        let response = update_vendor_route(Path("vr-test".into()), Json(route))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn delete_vendor_route_returns_no_content() {
        let response = delete_vendor_route(Path("vr-test".into()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn character_configs_returns_demo_data() {
        let state = Arc::new(test_state("api-character-configs-demo.json"));
        let Json(configs) = list_character_configs(State(state)).await;
        assert!(!configs.is_empty());
        assert!(configs.iter().any(|c| c.character_name == "Frostreaver"));
    }

    #[tokio::test]
    async fn put_character_config_upserts() {
        let dir = tempdir().expect("tempdir should exist");
        let mut state = test_state("api-put-character-config.json");
        state.character_config_path = dir.path().join("character-configs.json");
        let state = Arc::new(state);
        {
            let mut configs = state.character_configs.write().await;
            configs
                .get_mut("Aelrindel")
                .expect("demo config should exist")
                .reward_automation = shared_character_config::RewardAutomationConfig {
                rules: vec![shared_character_config::TaskRewardPreference {
                    task_matcher: "orc mission".into(),
                    preference: shared_character_config::RewardPreference::ByPosition {
                        reward_position: 2,
                    },
                }],
            };
        }
        let input = CharacterConfigUpdate {
            character_name: "IgnoredName".into(),
            class: "Wizard".into(),
            role: "DPS".into(),
            heal_at_pct: 50,
            mana_sit_pct: 15,
            nuke_at_pct: 70,
            rotation: vec![],
            class_params: ClassParams::default(),
            auto_rez: Some(AutoRezConfig {
                enabled: true,
                min_xp_pct: 96,
                trusted_casters: vec!["Frostreaver".into()],
                decline_if_untrusted: true,
                delay_ms: 5_100,
            }),
            group_override: false,
            group_name: None,
            window_title_format: Some("[{server}] {character} ({level} {class_short})".into()),
            reward_automation: None,
            tribute_preferences: Some(tribute_preferences(
                &["Arcane Fury", "Hero's Fortitude"],
                180,
            )),
        };
        let Json(saved) =
            put_character_config(State(state.clone()), Path("Aelrindel".into()), Json(input))
                .await
                .expect("put character config should succeed");
        assert_eq!(saved.character_name, "Aelrindel");
        assert!(saved.auto_rez.enabled);
        let saved_json = serde_json::to_value(&saved).expect("saved config should serialize");
        assert_eq!(
            saved_json["tribute_preferences"]["preferred_tributes"],
            serde_json::json!(["Arcane Fury", "Hero's Fortitude"])
        );
        assert_eq!(
            saved_json["tribute_status"]["alert_state"],
            serde_json::json!("expired")
        );
        assert_eq!(
            saved_json["tribute_status"]["point_balance"],
            serde_json::json!(875)
        );
        assert_eq!(
            saved_json["window_title_format"],
            serde_json::json!("[{server}] {character} ({level} {class_short})")
        );
        assert_eq!(
            saved_json["reward_automation"]["rules"][0]["task_matcher"],
            serde_json::json!("orc mission")
        );

        let persisted_path = dir.path().join("character-configs.json");
        let persisted = std::fs::read_to_string(&persisted_path)
            .expect("character config snapshot should be written");
        let persisted_json: serde_json::Value =
            serde_json::from_str(&persisted).expect("snapshot should parse");
        assert_eq!(
            persisted_json["Aelrindel"]["window_title_format"],
            serde_json::json!("[{server}] {character} ({level} {class_short})")
        );
        assert_eq!(
            persisted_json["Aelrindel"]["reward_automation"]["rules"][0]["task_matcher"],
            serde_json::json!("orc mission")
        );

        let Json(configs) = list_character_configs(State(state)).await;
        let updated = configs
            .into_iter()
            .find(|c| c.character_name == "Aelrindel")
            .expect("updated config should exist");
        assert_eq!(updated.heal_at_pct, 50);
        assert_eq!(updated.auto_rez.min_xp_pct, 96);
        assert_eq!(updated.auto_rez.trusted_casters, vec!["Frostreaver"]);
        assert_eq!(updated.reward_automation.rules.len(), 1);
        assert_eq!(
            updated.window_title_format,
            "[{server}] {character} ({level} {class_short})"
        );
    }

    #[tokio::test]
    async fn auto_accept_settings_round_trip() {
        let state = Arc::new(test_state("api-auto-accept-round-trip.json"));
        let update = AutoAcceptSettings {
            enabled: true,
            accept_trades: false,
            trust_mode: AutoAcceptTrustMode::TrustList,
            trusted_players: vec!["Leaderone".into(), "Clericone".into()],
            ..AutoAcceptSettings::default()
        };

        let Json(saved) = put_auto_accept_settings(State(state.clone()), Json(update.clone()))
            .await
            .expect("put auto-accept settings should succeed");
        assert_eq!(saved, update);

        let Json(loaded) = get_auto_accept_settings(State(state)).await;
        assert_eq!(loaded, update);
    }

    #[tokio::test]
    async fn put_auto_accept_settings_rejects_blank_trusted_names() {
        let state = Arc::new(test_state("api-put-auto-accept-blank-reject.json"));
        let invalid = AutoAcceptSettings {
            enabled: true,
            trust_mode: AutoAcceptTrustMode::TrustList,
            trusted_players: vec!["".into(), "   ".into()],
            ..AutoAcceptSettings::default()
        };

        let response = put_auto_accept_settings(State(state), Json(invalid))
            .await
            .expect_err("blank names should be rejected")
            .into_response();
        let (status, body) = error_response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            body,
            serde_json::json!({ "error": "Trusted player names must not be blank" })
        );
    }
}
