//! REST API handlers for the web dashboard.

pub mod admin;
pub mod admin_config;
pub mod admin_diagnostics;
pub mod admin_logs;
pub mod admin_sessions;
pub mod alerts;
pub mod auto_group;
pub mod chat_log;
pub mod chat_pattern_rules;
pub mod control;
pub mod dashboard;
pub mod discord;
pub mod economy;
pub mod extensions;
pub mod gm_alerts;
pub mod inventory_utility_parity;
pub mod kill_tracker;
pub mod loot;
pub mod player_watch;
pub mod say_detection;
pub mod session_control;
pub mod soul;
pub mod spawn_alerts;
pub mod vendor_watch;
pub mod xassist;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path as StdPath, PathBuf},
    sync::{Arc, OnceLock},
};
use textquest::chat_log::{
    ChatChannel, ChatLogConfig as CoreChatLogConfig, LogLevel, RotationStrategy,
};
use textquest_common::box_chat::BoxChatConfig;
use textquest_common::character_config::{self as shared_character_config};
use textquest_common::ipc::{AutoAcceptSettings, AutoRezConfig};
use textquest_common::protocol::{
    ConfigCopyRequest, ConfigCopyResult, ConfigCopyStatus, ConfigCopySubset,
};
use textquest_common::tradeskill_trophy::TradeskillTrophySettings;
use toml_edit::{Array, DocumentMut, Item, Table, value};

use crate::AppState;
use textquest_common::shared_client_state::SharedClientState;

pub fn mount_admin_sessions(
    router: axum::Router<std::sync::Arc<AppState>>,
) -> axum::Router<std::sync::Arc<AppState>> {
    router.nest("/admin/sessions", admin_sessions::router())
}

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

/// Returns the active config path.
///
/// In tests, respects `TEST_CONFIG_OVERRIDE` (set via `ConfigPathGuard`) so
/// parallel tests never mutate the process environment.  In production the
/// override is always `None` and the env-var / default are used as before.
pub(crate) fn textquest_config_path() -> PathBuf {
    #[cfg(test)]
    {
        if let Ok(guard) = test_config_override().read()
            && let Some(ref p) = *guard
        {
            return p.clone();
        }
    }
    std::env::var("TEXTQUEST_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/textquest.toml"))
}

/// Returns the global in-process test-override lock.  Only compiled in `#[cfg(test)]`.
#[cfg(test)]
pub(crate) fn test_config_override() -> &'static std::sync::RwLock<Option<PathBuf>> {
    static OVERRIDE: OnceLock<std::sync::RwLock<Option<PathBuf>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| std::sync::RwLock::new(None))
}

pub(crate) fn textquest_config_write_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

fn config_sidecar_path(file_name: &str) -> PathBuf {
    textquest_config_path().with_file_name(file_name)
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
    let existing_permissions = std::fs::metadata(&path).ok().map(|meta| meta.permissions());
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

    if let Some(permissions) = existing_permissions {
        std::fs::set_permissions(&temp_path, permissions).map_err(|error| {
            format!(
                "Failed to copy permissions from {} to {}: {error}",
                path.display(),
                temp_path.display()
            )
        })?;
    }

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
pub async fn put_box_chat_settings(
    State(_state): State<Arc<AppState>>,
    Json(settings): Json<BoxChatConfig>,
) -> impl IntoResponse {
    let _config_write_guard = textquest_config_write_lock().lock().await;
    match write_box_chat_settings_to_disk(&settings) {
        Ok(()) => (StatusCode::OK, Json(settings)).into_response(),
        Err(error) => json_error(StatusCode::BAD_REQUEST, error).into_response(),
    }
}
// ─── Chat Log Settings ──────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatLogRotationPayload {
    Daily { daily: Option<()> },
    Size { size: u64 },
    None(String),
}

impl Default for ChatLogRotationPayload {
    fn default() -> Self {
        Self::Size {
            size: 10 * 1024 * 1024,
        }
    }
}

impl From<RotationStrategy> for ChatLogRotationPayload {
    fn from(value: RotationStrategy) -> Self {
        match value {
            RotationStrategy::Daily => Self::Daily { daily: None },
            RotationStrategy::Size(size) => Self::Size { size },
            RotationStrategy::None => Self::None("none".to_string()),
        }
    }
}

impl TryFrom<ChatLogRotationPayload> for RotationStrategy {
    type Error = String;

    fn try_from(value: ChatLogRotationPayload) -> Result<Self, Self::Error> {
        match value {
            ChatLogRotationPayload::Daily { .. } => Ok(Self::Daily),
            ChatLogRotationPayload::Size { size } => Ok(Self::Size(size)),
            ChatLogRotationPayload::None(value) if value.eq_ignore_ascii_case("none") => {
                Ok(Self::None)
            }
            ChatLogRotationPayload::None(value) => Err(format!(
                "Invalid chat log rotation strategy literal: {value}"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatLogSettings {
    pub enabled: bool,
    pub channels: Vec<ChatChannel>,
    pub rotation_strategy: ChatLogRotationPayload,
    pub max_file_size_bytes: u64,
    pub min_level: LogLevel,
    pub log_eq_chat: bool,
}

impl Default for ChatLogSettings {
    fn default() -> Self {
        CoreChatLogConfig::default().into()
    }
}

impl From<CoreChatLogConfig> for ChatLogSettings {
    fn from(value: CoreChatLogConfig) -> Self {
        Self {
            enabled: value.enabled,
            channels: value.channels,
            rotation_strategy: value.rotation_strategy.into(),
            max_file_size_bytes: value.max_file_size_bytes,
            min_level: value.min_level,
            log_eq_chat: value.log_eq_chat,
        }
    }
}

impl TryFrom<ChatLogSettings> for CoreChatLogConfig {
    type Error = String;

    fn try_from(value: ChatLogSettings) -> Result<Self, Self::Error> {
        Ok(Self {
            enabled: value.enabled,
            channels: value.channels,
            rotation_strategy: value.rotation_strategy.try_into()?,
            max_file_size_bytes: value.max_file_size_bytes,
            min_level: value.min_level,
            log_eq_chat: value.log_eq_chat,
        })
    }
}

#[derive(Debug, Deserialize)]
struct ChatLogSettingsFile {
    #[serde(default)]
    chat_log: CoreChatLogConfig,
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

    let mut wrapped = DocumentMut::new();
    wrapped["chat_log"] = item.clone();

    let config = toml::from_str::<ChatLogSettingsFile>(&wrapped.to_string())
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;

    Ok(config.chat_log.into())
}

fn write_chat_log_settings_to_disk(settings: &ChatLogSettings) -> Result<(), String> {
    let settings: CoreChatLogConfig = settings.clone().try_into()?;
    let max_file_size_bytes = i64::try_from(settings.max_file_size_bytes)
        .map_err(|_| "max_file_size_bytes exceeds TOML integer range".to_string())?;
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
    let channels = settings
        .channels
        .iter()
        .map(|channel| toml_edit::Value::from(channel.to_string()))
        .collect::<Array>();
    table["channels"] = Item::Value(channels.into());
    match settings.rotation_strategy {
        RotationStrategy::Daily => {
            table["rotation_strategy"] = value("daily");
        }
        RotationStrategy::Size(size) => {
            let size = i64::try_from(size)
                .map_err(|_| "rotation_strategy.size exceeds TOML integer range".to_string())?;
            let mut rotation = toml_edit::InlineTable::new();
            rotation.insert("size", toml_edit::Value::from(size));
            table["rotation_strategy"] = Item::Value(toml_edit::Value::InlineTable(rotation));
        }
        RotationStrategy::None => {
            table["rotation_strategy"] = value("none");
        }
    }
    table["max_file_size_bytes"] = value(max_file_size_bytes);
    table["min_level"] = value(match settings.min_level {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
    });
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
pub async fn put_chat_log_settings(
    State(_state): State<Arc<AppState>>,
    Json(settings): Json<ChatLogSettings>,
) -> impl IntoResponse {
    let _config_write_guard = textquest_config_write_lock().lock().await;
    match write_chat_log_settings_to_disk(&settings) {
        Ok(()) => (StatusCode::OK, Json(settings)).into_response(),
        Err(error) => json_error(StatusCode::BAD_REQUEST, error).into_response(),
    }
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

impl From<textquest_common::character_config::RotationEntry> for RotationEntry {
    fn from(value: textquest_common::character_config::RotationEntry) -> Self {
        Self {
            id: value.id,
            name: value.name,
            priority: value.priority,
            enabled: value.enabled,
        }
    }
}

impl From<RotationEntry> for textquest_common::character_config::RotationEntry {
    fn from(value: RotationEntry) -> Self {
        Self {
            id: value.id,
            name: value.name,
            priority: value.priority,
            enabled: value.enabled,
        }
    }
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
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

fn copy_subset_requires_rotation(subset: &ConfigCopySubset) -> bool {
    matches!(subset, ConfigCopySubset::Rotation | ConfigCopySubset::Both)
}

fn copy_subset_includes_class_params(subset: &ConfigCopySubset) -> bool {
    matches!(
        subset,
        ConfigCopySubset::ClassParams | ConfigCopySubset::Both
    )
}

fn build_config_copy_diff_summary(
    before: &CharacterConfig,
    after: &CharacterConfig,
    subset: &ConfigCopySubset,
) -> String {
    let mut changed = Vec::new();
    let class_params_changed = serde_json::to_value(&before.class_params)
        .expect("serialize class params")
        != serde_json::to_value(&after.class_params).expect("serialize class params");
    let rotation_changed = serde_json::to_value(&before.rotation).expect("serialize rotation")
        != serde_json::to_value(&after.rotation).expect("serialize rotation");

    if copy_subset_includes_class_params(subset) && class_params_changed {
        changed.push("class_params");
    }
    if copy_subset_requires_rotation(subset) && rotation_changed {
        changed.push("rotation");
    }

    if changed.is_empty() {
        "no changes".to_string()
    } else {
        format!("copied {}", changed.join(", "))
    }
}

fn apply_config_copy(
    source: &CharacterConfig,
    target: &CharacterConfig,
    subset: &ConfigCopySubset,
) -> CharacterConfig {
    let mut result = target.clone();
    if copy_subset_includes_class_params(subset) {
        result.class_params = source.class_params.clone();
    }
    if copy_subset_requires_rotation(subset) {
        result.rotation = source.rotation.clone();
    }
    result
}

/// POST /api/config/copy — copy settings between character tuning configs.
pub async fn post_config_copy(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ConfigCopyRequest>,
) -> Result<Json<Vec<ConfigCopyResult>>, (StatusCode, Json<ErrorResponse>)> {
    let from_char = request.from_char.trim().to_string();
    if from_char.is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "from_char must not be blank",
        ));
    }
    if request.to_chars.is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "to_chars must not be empty",
        ));
    }

    let _write_guard = state.character_config_write_lock.lock().await;
    let mut configs = state.character_configs.write().await;
    let source = match configs.get(&from_char) {
        Some(config) => config.clone(),
        None => {
            return Err(json_error(
                StatusCode::BAD_REQUEST,
                format!("Source character '{from_char}' was not found"),
            ));
        }
    };

    let mut results = Vec::new();
    let mut next_configs = configs.clone();

    for raw_target in request.to_chars {
        let target_name = raw_target.trim().to_string();

        if target_name.is_empty() {
            results.push(ConfigCopyResult {
                r#char: raw_target,
                status: ConfigCopyStatus::Error,
                diff_summary: "target character name must not be blank".to_string(),
            });
            continue;
        }

        let target = match next_configs.get(&target_name) {
            Some(config) => config.clone(),
            None => {
                results.push(ConfigCopyResult {
                    r#char: target_name,
                    status: ConfigCopyStatus::Error,
                    diff_summary: "target character not found".to_string(),
                });
                continue;
            }
        };

        if copy_subset_requires_rotation(&request.subset)
            && !source.class.eq_ignore_ascii_case(&target.class)
        {
            results.push(ConfigCopyResult {
                r#char: target_name,
                status: ConfigCopyStatus::Error,
                diff_summary: "target class must match source class for rotation copy".to_string(),
            });
            continue;
        }

        let copied = apply_config_copy(&source, &target, &request.subset);
        let diff_summary = build_config_copy_diff_summary(&target, &copied, &request.subset);

        if let Some(entry) = next_configs.get_mut(&target_name) {
            *entry = copied.clone();
        }

        results.push(ConfigCopyResult {
            r#char: target_name,
            status: ConfigCopyStatus::Success,
            diff_summary,
        });
    }

    if results.iter().any(|result| {
        matches!(
            result,
            ConfigCopyResult {
                status: ConfigCopyStatus::Success,
                ..
            }
        )
    }) {
        if let Err(error) =
            write_character_configs_to_path(&state.character_config_path, &next_configs)
        {
            return Err(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to persist character config copy: {error}"),
            ));
        }
        *configs = next_configs;
    }

    Ok(Json(results))
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
pub async fn put_character_config(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(config): Json<CharacterConfigUpdate>,
) -> Result<Json<CharacterConfig>, (StatusCode, Json<ErrorResponse>)> {
    if name.trim().is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "Character name must not be blank",
        ));
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
        return Err(json_error(StatusCode::INTERNAL_SERVER_ERROR, error));
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

/// GET /api/config/tradeskill-trophy — return the current trophy automation
/// settings.
pub async fn get_tradeskill_trophy_settings(
    State(state): State<Arc<AppState>>,
) -> Json<TradeskillTrophySettings> {
    Json(state.tradeskill_trophy_settings.read().await.clone())
}

/// PUT /api/config/tradeskill-trophy — replace the current trophy automation
/// policy.
pub async fn put_tradeskill_trophy_settings(
    State(state): State<Arc<AppState>>,
    Json(settings): Json<TradeskillTrophySettings>,
) -> Result<Json<TradeskillTrophySettings>, (StatusCode, Json<ErrorResponse>)> {
    let settings = settings.sanitized();
    if settings.enabled && settings.trophy_item_name.is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "Trophy item name must not be blank when tradeskill trophy automation is enabled",
        ));
    }

    *state.tradeskill_trophy_settings.write().await = settings.clone();
    let applied = crate::live_ipc::apply_tradeskill_trophy_settings(&settings);
    tracing::info!(
        attempted_clients = applied.attempted,
        applied_clients = applied.applied,
        failed_clients = applied.failures.len(),
        "Updated tradeskill trophy settings"
    );
    for (pid, error) in applied.failures {
        tracing::warn!(
            pid,
            %error,
            "Failed to apply tradeskill trophy settings to live client"
        );
    }
    Ok(Json(settings))
}

/// GET /api/tradeskill-trophy/status — return live trophy status for connected
/// clients.
pub async fn get_tradeskill_trophy_statuses()
-> Json<Vec<crate::live_ipc::LiveTradeskillTrophyStatusResult>> {
    Json(crate::live_ipc::query_tradeskill_trophy_statuses())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerFilterMode {
    #[default]
    All,
    StrangersOnly,
    FriendsOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerWatchConfig {
    pub filter_mode: PlayerFilterMode,
    pub sound_on_zone_in: bool,
    pub friends: Vec<String>,
}

impl Default for PlayerWatchConfig {
    fn default() -> Self {
        Self {
            filter_mode: PlayerFilterMode::All,
            sound_on_zone_in: false,
            friends: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlayerWatchConfigPatch {
    pub filter_mode: Option<PlayerFilterMode>,
    pub sound_on_zone_in: Option<bool>,
    pub friends: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlayerWatchConfigFile {
    #[serde(default)]
    player_filter_mode: PlayerFilterMode,
    #[serde(default)]
    sound_on_player_zone_in: bool,
    #[serde(default)]
    friends: Vec<String>,
}

impl Default for PlayerWatchConfigFile {
    fn default() -> Self {
        Self {
            player_filter_mode: PlayerFilterMode::All,
            sound_on_player_zone_in: false,
            friends: Vec::new(),
        }
    }
}

impl From<PlayerWatchConfigFile> for PlayerWatchConfig {
    fn from(value: PlayerWatchConfigFile) -> Self {
        Self {
            filter_mode: value.player_filter_mode,
            sound_on_zone_in: value.sound_on_player_zone_in,
            friends: value.friends,
        }
    }
}

pub(crate) fn read_player_watch_config_from_disk() -> Result<PlayerWatchConfig, String> {
    let path = textquest_config_path();
    if !path.exists() {
        return Ok(PlayerWatchConfig::default());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    let doc = content
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;

    let Some(item) = doc.get("spawn_watch") else {
        return Ok(PlayerWatchConfig::default());
    };

    let config = toml_edit::de::from_str::<PlayerWatchConfigFile>(&item.to_string())
        .map_err(|error| format!("Failed to decode [spawn_watch]: {error}"))?;
    Ok(config.into())
}

fn write_player_watch_config_to_disk(config: &PlayerWatchConfig) -> Result<(), String> {
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

    let mut table = match doc.get("spawn_watch") {
        Some(Item::Table(existing)) => existing.clone(),
        _ => Table::new(),
    };
    let filter_mode = match config.filter_mode {
        PlayerFilterMode::All => "all",
        PlayerFilterMode::StrangersOnly => "strangers_only",
        PlayerFilterMode::FriendsOnly => "friends_only",
    };
    table["player_filter_mode"] = value(filter_mode);
    table["sound_on_player_zone_in"] = value(config.sound_on_zone_in);
    let friends = config
        .friends
        .iter()
        .cloned()
        .map(toml_edit::Value::from)
        .collect::<Array>();
    table["friends"] = Item::Value(friends.into());
    doc["spawn_watch"] = Item::Table(table);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    std::fs::write(&path, doc.to_string())
        .map_err(|error| format!("Failed to write {}: {error}", path.display()))
}

fn timestamp_config_path() -> PathBuf {
    config_sidecar_path("timestamp.toml")
}

pub(crate) fn load_timestamp_configs_from_path(path: &StdPath) -> HashMap<String, TimestampConfig> {
    match std::fs::read_to_string(path) {
        Ok(content) => match toml::from_str::<HashMap<String, TimestampConfig>>(&content) {
            Ok(configs) => configs,
            Err(error) => {
                tracing::warn!(
                    %error,
                    path = %path.display(),
                    "Failed to parse timestamp config; using defaults"
                );
                HashMap::new()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
        Err(error) => {
            tracing::warn!(
                %error,
                path = %path.display(),
                "Failed to read timestamp config; using defaults"
            );
            HashMap::new()
        }
    }
}

pub(crate) fn load_timestamp_configs_from_disk() -> Result<HashMap<String, TimestampConfig>, String>
{
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
    write_timestamp_configs_to_path(&timestamp_config_path(), configs)
}

fn write_timestamp_configs_to_path(
    path: &StdPath,
    configs: &HashMap<String, TimestampConfig>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    let content = toml::to_string_pretty(configs)
        .map_err(|error| format!("Failed to serialize timestamp configs: {error}"))?;
    let temp_path = path.with_extension("toml.tmp");
    std::fs::write(&temp_path, content)
        .map_err(|error| format!("Failed to write temp file {}: {error}", temp_path.display()))?;
    replace_timestamp_file_with_overwrite_fallback(
        &temp_path,
        path,
        |from: &StdPath, to: &StdPath| std::fs::rename(from, to),
        |target: &StdPath| std::fs::remove_file(target),
    )?;
    Ok(())
}

fn replace_timestamp_file_with_overwrite_fallback<Rename, Remove>(
    temp_path: &StdPath,
    path: &StdPath,
    mut rename: Rename,
    mut remove_file: Remove,
) -> Result<(), String>
where
    Rename: FnMut(&StdPath, &StdPath) -> std::io::Result<()>,
    Remove: FnMut(&StdPath) -> std::io::Result<()>,
{
    match rename(temp_path, path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            remove_file(path).map_err(|remove_error| {
                let _ = remove_file(temp_path);
                format!(
                    "Failed to replace {} with {} after destination already existed: {remove_error}",
                    path.display(),
                    temp_path.display()
                )
            })?;

            rename(temp_path, path).map_err(|rename_error| {
                let _ = remove_file(temp_path);
                format!(
                    "Failed to replace {} with {} after removing the existing destination: {rename_error}",
                    path.display(),
                    temp_path.display()
                )
            })
        }
        Err(error) => {
            let _ = remove_file(temp_path);
            Err(format!(
                "Failed to replace {} with {}: {error}",
                path.display(),
                temp_path.display()
            ))
        }
    }
}

/// GET /api/config/player-watch — return the current player watch filter config.
pub async fn get_player_watch_config(
    State(state): State<Arc<AppState>>,
) -> Json<PlayerWatchConfig> {
    Json(state.player_watch_config.read().await.clone())
}

/// PUT /api/config/player-watch — update the current player watch filter config.
pub async fn put_player_watch_config(
    State(state): State<Arc<AppState>>,
    Json(patch): Json<PlayerWatchConfigPatch>,
) -> Result<Json<PlayerWatchConfig>, (StatusCode, Json<ErrorResponse>)> {
    let _config_write_guard = textquest_config_write_lock().lock().await;
    let _write_guard = state.player_watch_write_lock.lock().await;

    let updated = {
        let current = state.player_watch_config.read().await;
        let mut updated = current.clone();
        if let Some(filter_mode) = patch.filter_mode {
            updated.filter_mode = filter_mode;
        }
        if let Some(sound_on_zone_in) = patch.sound_on_zone_in {
            updated.sound_on_zone_in = sound_on_zone_in;
        }
        if let Some(friends) = patch.friends {
            let mut normalized = Vec::new();
            for friend in friends {
                let trimmed = friend.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if normalized
                    .iter()
                    .all(|existing: &String| !existing.eq_ignore_ascii_case(trimmed))
                {
                    normalized.push(trimmed.to_string());
                }
            }
            updated.friends = normalized;
        }
        updated
    };

    write_player_watch_config_to_disk(&updated)
        .map_err(|error| json_error(StatusCode::INTERNAL_SERVER_ERROR, error))?;

    *state.player_watch_config.write().await = updated.clone();

    Ok(Json(updated))
}

/// GET /api/timestamp-config — return all known per-character timestamp configs.
pub async fn list_timestamp_configs(
    State(state): State<Arc<AppState>>,
) -> Json<HashMap<String, TimestampConfig>> {
    Json(state.timestamp_configs.read().await.clone())
}

/// GET /api/timestamp-config/:character — return the stored config for one character.
pub async fn get_timestamp_config(
    State(state): State<Arc<AppState>>,
    Path(character): Path<String>,
) -> Json<TimestampConfig> {
    let configs = state.timestamp_configs.read().await;
    Json(configs.get(&character).cloned().unwrap_or_default())
}

/// PUT /api/timestamp-config/:character — replace one character's timestamp config.
pub async fn put_timestamp_config(
    State(state): State<Arc<AppState>>,
    Path(character): Path<String>,
    Json(config): Json<TimestampConfig>,
) -> Result<Json<TimestampConfig>, (StatusCode, Json<ErrorResponse>)> {
    if character.trim().is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "Character name must not be blank",
        ));
    }

    let _write_guard = state.timestamp_config_write_lock.lock().await;

    let snapshot = {
        let configs = state.timestamp_configs.read().await;
        let mut snapshot = configs.clone();
        snapshot.insert(character.clone(), config.clone());
        snapshot
    };

    write_timestamp_configs_to_disk(&snapshot).map_err(|error| {
        json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to persist timestamp config: {error}"),
        )
    })?;

    *state.timestamp_configs.write().await = snapshot;

    Ok(Json(config))
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;
    use serde_json::Value;
    use std::collections::HashMap;
    use tempfile::tempdir;
    use textquest_common::ipc::{AutoAcceptSettings, AutoAcceptTrustMode};

    /// Sets the in-process test config-path override and restores the previous
    /// value on drop.  Panic-safe: if the test panics before drop runs, the
    /// override is still restored when the stack unwinds — no env var is ever
    /// mutated.
    struct ConfigPathGuard {
        previous: Option<PathBuf>,
    }

    impl ConfigPathGuard {
        fn set(path: &std::path::Path) -> Self {
            let mut lock = crate::api::test_config_override()
                .write()
                .expect("test_config_override lock poisoned");
            let previous = lock.take();
            *lock = Some(path.to_path_buf());
            Self { previous }
        }
    }

    impl Drop for ConfigPathGuard {
        fn drop(&mut self) {
            let mut lock = crate::api::test_config_override()
                .write()
                .expect("test_config_override lock poisoned");
            *lock = self.previous.take();
        }
    }

    // Shim so existing test call sites compile unchanged. The RwLock used by
    // ConfigPathGuard only protects the individual set()/drop() mutations of
    // the override; it is not held for the full guard lifetime. Tests that
    // need the override to remain stable across a whole scope must still use
    // this mutex (or another lifetime-long guard) for correctness.
    fn config_env_lock() -> &'static tokio::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    fn temp_config_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "textquest-web-api-{name}-{}.toml",
            uuid::Uuid::new_v4()
        ))
    }

    fn test_state(snapshot_name: &str) -> AppState {
        let mut state = crate::test_app_state();
        state.live_session_snapshot_path =
            crate::test_support::test_live_session_snapshot_path(snapshot_name);
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

        let Json(configs) = list_character_configs(State(state.clone())).await;
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
    async fn config_copy_multi_target_class_params() {
        let dir = tempdir().expect("tempdir should exist");
        let mut state = test_state("api-config-copy-class-params.json");
        state.character_config_path = dir.path().join("character-configs.json");
        let state = Arc::new(state);

        {
            let mut configs = state.character_configs.write().await;
            let source = configs
                .get_mut("Aelrindel")
                .expect("demo config should exist");
            source.class_params.burn_at_hp_pct = Some(42);
        }

        let request = ConfigCopyRequest {
            from_char: "Aelrindel".into(),
            to_chars: vec!["Noxus".into(), "Grok".into()],
            subset: ConfigCopySubset::ClassParams,
        };

        let Json(results) = post_config_copy(State(state.clone()), Json(request))
            .await
            .expect("copy should succeed");
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|result| {
            result.status == ConfigCopyStatus::Success
                && matches!(result.diff_summary.as_str(), "copied class_params")
        }));

        let configs = state.character_configs.read().await;
        let source = configs
            .get("Aelrindel")
            .expect("source character should still exist");
        let noxus = configs.get("Noxus").expect("target character should exist");
        let grok = configs.get("Grok").expect("target character should exist");

        assert_eq!(
            serde_json::to_value(&source.class_params).expect("serialize class params"),
            serde_json::to_value(&noxus.class_params).expect("serialize class params"),
        );
        assert_eq!(
            serde_json::to_value(&source.class_params).expect("serialize class params"),
            serde_json::to_value(&grok.class_params).expect("serialize class params"),
        );
        assert_eq!(noxus.rotation.len(), 1);
        assert_eq!(noxus.rotation[0].id, "taunt");
        assert_eq!(noxus.rotation[0].name, "Taunt");
        assert_eq!(noxus.rotation[0].priority, 1);
        assert!(noxus.rotation[0].enabled);
        assert_eq!(grok.rotation.len(), 1);
        assert_eq!(grok.rotation[0].id, "turgurs_insects");
        assert_eq!(grok.rotation[0].name, "Turgur's Insects");
        assert_eq!(grok.rotation[0].priority, 1);
        assert!(grok.rotation[0].enabled);
    }

    #[tokio::test]
    async fn config_copy_missing_target_reports_error() {
        let dir = tempdir().expect("tempdir should exist");
        let mut state = test_state("api-config-copy-missing-target.json");
        state.character_config_path = dir.path().join("character-configs.json");
        let state = Arc::new(state);

        let request = ConfigCopyRequest {
            from_char: "Aelrindel".into(),
            to_chars: vec!["NoSuchCharacter".into()],
            subset: ConfigCopySubset::Both,
        };

        let Json(results) = post_config_copy(State(state), Json(request))
            .await
            .expect("missing target should return structured error result");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].r#char, "NoSuchCharacter");
        assert_eq!(results[0].status, ConfigCopyStatus::Error);
        assert_eq!(results[0].diff_summary, "target character not found");
    }

    #[tokio::test]
    async fn config_copy_rotation_rejects_class_mismatch() {
        let dir = tempdir().expect("tempdir should exist");
        let mut state = test_state("api-config-copy-class-mismatch.json");
        state.character_config_path = dir.path().join("character-configs.json");
        let state = Arc::new(state);

        let request = ConfigCopyRequest {
            from_char: "Aelrindel".into(),
            to_chars: vec!["Noxus".into()],
            subset: ConfigCopySubset::Rotation,
        };

        let Json(results) = post_config_copy(State(state.clone()), Json(request))
            .await
            .expect("mismatched class should return error result");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].r#char, "Noxus");
        assert_eq!(results[0].status, ConfigCopyStatus::Error);
        assert_eq!(
            results[0].diff_summary,
            "target class must match source class for rotation copy"
        );

        let configs = state.character_configs.read().await;
        let noxus_rotation: Vec<_> = configs
            .get("Noxus")
            .expect("target should exist")
            .rotation
            .iter()
            .map(|entry| {
                (
                    entry.id.as_str(),
                    entry.name.as_str(),
                    entry.priority,
                    entry.enabled,
                )
            })
            .collect();
        assert_eq!(noxus_rotation, vec![("taunt", "Taunt", 1, true)]);
    }

    #[tokio::test]
    async fn config_copy_empty_to_chars_errors() {
        let dir = tempdir().expect("tempdir should exist");
        let mut state = test_state("api-config-copy-empty-targets.json");
        state.character_config_path = dir.path().join("character-configs.json");
        let state = Arc::new(state);

        let request = ConfigCopyRequest {
            from_char: "Aelrindel".into(),
            to_chars: vec![],
            subset: ConfigCopySubset::ClassParams,
        };

        let Err((status, error)) = post_config_copy(State(state), Json(request)).await else {
            panic!("empty to_chars should fail");
        };

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error.0.error, "to_chars must not be empty");
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

    #[test]
    fn load_timestamp_configs_from_path_round_trips_saved_data() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("timestamp.toml");
        let expected = HashMap::from([(
            "Frostreaver".to_string(),
            TimestampConfig {
                enabled: true,
                format: TimestampFormat::Time24,
            },
        )]);

        write_timestamp_configs_to_path(&path, &expected).expect("write timestamp configs");
        let loaded = load_timestamp_configs_from_path(&path);
        assert_eq!(loaded, expected);
    }

    #[test]
    fn replace_timestamp_config_retries_after_destination_exists_error() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let path = temp_dir.path().join("timestamp.toml");
        let temp_path = temp_dir.path().join("timestamp.toml.tmp");
        std::fs::write(&path, "old").expect("write destination");
        std::fs::write(&temp_path, "new").expect("write temp");

        let rename_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let remove_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let rename_counter = Arc::clone(&rename_calls);
        let remove_counter = Arc::clone(&remove_calls);
        replace_timestamp_file_with_overwrite_fallback(
            &temp_path,
            &path,
            move |from: &StdPath, to: &StdPath| {
                let attempt = rename_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if attempt == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        format!("{} already exists", to.display()),
                    ));
                }
                std::fs::rename(from, to)
            },
            move |target: &StdPath| {
                remove_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                std::fs::remove_file(target)
            },
        )
        .expect("replace timestamp file");

        assert_eq!(rename_calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(remove_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(
            std::fs::read_to_string(&path).expect("read destination"),
            "new"
        );
        assert!(!temp_path.exists());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn put_timestamp_config_returns_error_when_persist_fails() {
        let _lock = config_env_lock().lock().await;
        let state = Arc::new(test_state("api-timestamp-persist-error.json"));
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let blocker = temp_dir.path().join("not-a-directory");
        std::fs::write(&blocker, "blocker").expect("write blocker");
        let _guard = ConfigPathGuard::set(&blocker.join("textquest.toml"));

        let mut configs = state.timestamp_configs.write().await;
        configs.insert(
            "Existing".into(),
            TimestampConfig {
                enabled: true,
                format: TimestampFormat::DateTime12,
            },
        );
        drop(configs);

        let response = put_timestamp_config(
            State(state.clone()),
            Path("Frostreaver".into()),
            Json(TimestampConfig {
                enabled: true,
                format: TimestampFormat::Time12,
            }),
        )
        .await
        .expect_err("persist failure should return an error");

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            response
                .1
                .0
                .error
                .contains("Failed to persist timestamp config: Failed to create")
        );
        assert!(
            !state
                .timestamp_configs
                .read()
                .await
                .contains_key("Frostreaver")
        );
    }

    #[tokio::test]
    async fn tradeskill_trophy_settings_round_trip() {
        let state = Arc::new(test_state("api-tradeskill-trophy-round-trip.json"));
        let update = TradeskillTrophySettings {
            enabled: true,
            trophy_item_name: "  Geerlok Automated Hammer  ".into(),
        };

        let Json(saved) = put_tradeskill_trophy_settings(State(state.clone()), Json(update))
            .await
            .expect("put tradeskill trophy settings should succeed");
        assert_eq!(
            saved,
            TradeskillTrophySettings {
                enabled: true,
                trophy_item_name: "Geerlok Automated Hammer".into(),
            }
        );

        let Json(loaded) = get_tradeskill_trophy_settings(State(state)).await;
        assert_eq!(loaded, saved);
    }

    #[tokio::test]
    async fn put_tradeskill_trophy_settings_rejects_blank_name_when_enabled() {
        let state = Arc::new(test_state("api-put-tradeskill-trophy-blank-reject.json"));
        let invalid = TradeskillTrophySettings {
            enabled: true,
            trophy_item_name: "   ".into(),
        };

        let response = put_tradeskill_trophy_settings(State(state), Json(invalid))
            .await
            .expect_err("blank trophy item should be rejected")
            .into_response();
        let (status, body) = error_response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            body,
            serde_json::json!({
                "error": "Trophy item name must not be blank when tradeskill trophy automation is enabled"
            })
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn write_box_chat_settings_preserves_existing_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = config_env_lock().lock().await;
        let path = temp_config_path("box-chat-permissions");
        let _config_guard = ConfigPathGuard::set(&path);

        std::fs::write(
            &path,
            "[box_chat]\nenabled = false\nhost = \"127.0.0.1\"\nport = 1\n",
        )
        .expect("seed config should be writable");
        let restrictive_mode = 0o600;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(restrictive_mode))
            .expect("seed config permissions should be adjustable");

        let settings = BoxChatConfig {
            enabled: true,
            host: "127.0.0.1".into(),
            port: 2112,
            auto_connect: true,
        };
        write_box_chat_settings_to_disk(&settings)
            .expect("box chat settings write should preserve permissions");

        let written_mode = std::fs::metadata(&path)
            .expect("written config should have metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(written_mode, restrictive_mode);

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn read_chat_log_settings_returns_typed_rotation_payload() {
        let _lock = config_env_lock().lock().await;
        let config_path = temp_config_path("chat-log-read");
        let _guard = ConfigPathGuard::set(&config_path);

        std::fs::write(
            &config_path,
            r#"[chat_log]
enabled = true
channels = ["say", "group"]
rotation_strategy = { size = 5242880 }
max_file_size_bytes = 5242880
min_level = "debug"
log_eq_chat = true
"#,
        )
        .expect("seed chat log config");

        let settings = read_chat_log_settings_from_disk().expect("chat log settings");

        assert!(settings.enabled);
        assert_eq!(
            settings.channels,
            vec![ChatChannel::Say, ChatChannel::Group]
        );
        assert_eq!(
            settings.rotation_strategy,
            ChatLogRotationPayload::Size {
                size: 5 * 1024 * 1024
            }
        );
        assert_eq!(settings.min_level, LogLevel::Debug);
        assert!(settings.log_eq_chat);

        std::fs::remove_file(&config_path).ok();
    }

    #[tokio::test]
    async fn write_chat_log_settings_round_trips_rotation_shape() {
        let _lock = config_env_lock().lock().await;
        let config_path = temp_config_path("chat-log-write");
        let _guard = ConfigPathGuard::set(&config_path);

        let settings = ChatLogSettings {
            enabled: true,
            channels: vec![ChatChannel::MQ2, ChatChannel::Guild],
            rotation_strategy: ChatLogRotationPayload::Daily { daily: None },
            max_file_size_bytes: 10 * 1024 * 1024,
            min_level: LogLevel::Warn,
            log_eq_chat: true,
        };

        write_chat_log_settings_to_disk(&settings).expect("write chat log config");
        let reloaded = read_chat_log_settings_from_disk().expect("reload chat log config");

        assert_eq!(reloaded, settings);

        std::fs::remove_file(&config_path).ok();
    }

    #[tokio::test]
    async fn write_chat_log_settings_rejects_values_outside_toml_integer_range() {
        let _lock = config_env_lock().lock().await;
        let config_path = temp_config_path("chat-log-overflow");
        let _guard = ConfigPathGuard::set(&config_path);

        let oversized_rotation = ChatLogSettings {
            enabled: true,
            channels: vec![ChatChannel::MQ2],
            rotation_strategy: ChatLogRotationPayload::Size {
                size: i64::MAX as u64 + 1,
            },
            max_file_size_bytes: 1024,
            min_level: LogLevel::Info,
            log_eq_chat: false,
        };
        let error = write_chat_log_settings_to_disk(&oversized_rotation)
            .expect_err("oversized rotation should fail");
        assert!(error.contains("rotation_strategy.size"));

        let oversized_max = ChatLogSettings {
            enabled: true,
            channels: vec![ChatChannel::MQ2],
            rotation_strategy: ChatLogRotationPayload::None("none".to_string()),
            max_file_size_bytes: i64::MAX as u64 + 1,
            min_level: LogLevel::Info,
            log_eq_chat: false,
        };
        let error = write_chat_log_settings_to_disk(&oversized_max)
            .expect_err("oversized max file size should fail");
        assert!(error.contains("max_file_size_bytes"));

        std::fs::remove_file(&config_path).ok();
    }

    #[tokio::test]
    async fn write_player_watch_config_preserves_existing_spawn_watch_fields() {
        let _lock = config_env_lock().lock().await;
        let config_path = temp_config_path("player-watch-preserve");
        let _guard = ConfigPathGuard::set(&config_path);

        std::fs::write(
            &config_path,
            r#"[spawn_watch]
enabled = true
watch_names = ["Quillmane"]
alert_named = true
max_feed_entries = 99
player_filter_mode = "all"
sound_on_player_zone_in = false
friends = ["OldFriend"]
"#,
        )
        .expect("seed player watch config");

        write_player_watch_config_to_disk(&PlayerWatchConfig {
            filter_mode: PlayerFilterMode::FriendsOnly,
            sound_on_zone_in: true,
            friends: vec!["NewFriend".into()],
        })
        .expect("write player watch config");

        let contents = std::fs::read_to_string(&config_path).expect("updated config");
        let doc = contents
            .parse::<DocumentMut>()
            .expect("updated config should parse");

        assert_eq!(doc["spawn_watch"]["enabled"].as_bool(), Some(true));
        assert_eq!(doc["spawn_watch"]["alert_named"].as_bool(), Some(true));
        assert_eq!(
            doc["spawn_watch"]["max_feed_entries"].as_integer(),
            Some(99)
        );
        assert_eq!(
            doc["spawn_watch"]["player_filter_mode"].as_str(),
            Some("friends_only")
        );
        assert_eq!(
            doc["spawn_watch"]["sound_on_player_zone_in"].as_bool(),
            Some(true)
        );
        assert_eq!(
            doc["spawn_watch"]["watch_names"]
                .as_array()
                .expect("watch_names array")
                .iter()
                .filter_map(|value| value.as_str())
                .collect::<Vec<_>>(),
            vec!["Quillmane"]
        );
        assert_eq!(
            doc["spawn_watch"]["friends"]
                .as_array()
                .expect("friends array")
                .iter()
                .filter_map(|value| value.as_str())
                .collect::<Vec<_>>(),
            vec!["NewFriend"]
        );

        std::fs::remove_file(&config_path).ok();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn put_player_watch_config_does_not_update_state_when_disk_write_fails() {
        use std::os::unix::fs::PermissionsExt;

        let _lock = config_env_lock().lock().await;
        let temp_root = std::env::temp_dir().join(format!(
            "textquest-web-api-player-watch-readonly-{}",
            uuid::Uuid::new_v4()
        ));
        let read_only_dir = temp_root.join("readonly");
        std::fs::create_dir_all(&read_only_dir).expect("create readonly dir");
        std::fs::set_permissions(&read_only_dir, std::fs::Permissions::from_mode(0o555))
            .expect("mark readonly");

        let config_path = read_only_dir.join("textquest.toml");
        let _guard = ConfigPathGuard::set(&config_path);
        let state =
            crate::test_support::demo_app_state_with_snapshot("api-player-watch-failure.json");
        let original = PlayerWatchConfig {
            filter_mode: PlayerFilterMode::All,
            sound_on_zone_in: false,
            friends: vec!["ExistingFriend".into()],
        };
        *state.player_watch_config.write().await = original.clone();

        let response = put_player_watch_config(
            State(state.clone()),
            Json(PlayerWatchConfigPatch {
                filter_mode: Some(PlayerFilterMode::FriendsOnly),
                sound_on_zone_in: Some(true),
                friends: Some(vec!["NewFriend".into()]),
            }),
        )
        .await
        .expect_err("readonly target should fail")
        .into_response();
        let (status, _) = error_response_json(response).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

        let saved = state.player_watch_config.read().await.clone();
        assert_eq!(saved, original);

        std::fs::set_permissions(&read_only_dir, std::fs::Permissions::from_mode(0o755))
            .expect("restore dir perms");
        std::fs::remove_dir_all(&temp_root).ok();
    }

    #[tokio::test]
    async fn write_timestamp_configs_uses_textquest_config_path_base_name() {
        let _lock = config_env_lock().lock().await;
        let config_path = temp_config_path("timestamp-sidecar");
        let _guard = ConfigPathGuard::set(&config_path);

        let expected_timestamp_path = config_path.with_file_name("timestamp.toml");
        let configs = HashMap::from([(
            "Frostreaver".to_string(),
            TimestampConfig {
                enabled: true,
                format: TimestampFormat::DateTime12,
            },
        )]);

        write_timestamp_configs_to_disk(&configs).expect("write timestamp config");

        let contents =
            std::fs::read_to_string(&expected_timestamp_path).expect("timestamp config file");
        let parsed: HashMap<String, TimestampConfig> =
            toml::from_str(&contents).expect("timestamp config should parse");
        assert_eq!(parsed, configs);

        std::fs::remove_file(&expected_timestamp_path).ok();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn put_timestamp_config_does_not_update_state_when_disk_write_fails() {
        use std::os::unix::fs::PermissionsExt;

        let _lock = config_env_lock().lock().await;
        let temp_root = std::env::temp_dir().join(format!(
            "textquest-web-api-timestamp-readonly-{}",
            uuid::Uuid::new_v4()
        ));
        let read_only_dir = temp_root.join("readonly");
        std::fs::create_dir_all(&read_only_dir).expect("create readonly dir");
        std::fs::set_permissions(&read_only_dir, std::fs::Permissions::from_mode(0o555))
            .expect("mark readonly");

        let config_path = read_only_dir.join("textquest.toml");
        let _guard = ConfigPathGuard::set(&config_path);
        let state = crate::test_support::demo_app_state_with_snapshot("api-timestamp-failure.json");
        let original = TimestampConfig {
            enabled: false,
            format: TimestampFormat::Time24,
        };
        state
            .timestamp_configs
            .write()
            .await
            .insert("Frostreaver".into(), original.clone());

        let response = put_timestamp_config(
            State(state.clone()),
            Path("Frostreaver".into()),
            Json(TimestampConfig {
                enabled: true,
                format: TimestampFormat::DateTime12,
            }),
        )
        .await
        .expect_err("readonly target should fail")
        .into_response();
        let (status, _) = error_response_json(response).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

        let saved = state
            .timestamp_configs
            .read()
            .await
            .get("Frostreaver")
            .cloned()
            .expect("original config still present");
        assert_eq!(saved, original);

        std::fs::set_permissions(&read_only_dir, std::fs::Permissions::from_mode(0o755))
            .expect("restore dir perms");
        std::fs::remove_dir_all(&temp_root).ok();
    }

    /// Validates that `ConfigPathGuard` is panic-safe: if a test panics while a
    /// guard is active, the override is restored on unwind so sibling tests are
    /// never poisoned.
    #[test]
    fn config_path_guard_cleans_up_on_panic() {
        use tempfile::tempdir;

        let dir = tempdir().expect("tempdir");
        let sentinel = dir.path().join("sentinel.toml");

        // Simulate a test that panics while holding a ConfigPathGuard.
        let result = std::panic::catch_unwind(|| {
            let _guard = ConfigPathGuard::set(&sentinel);
            // The override is visible inside the panicking closure.
            assert_eq!(
                crate::api::textquest_config_path(),
                sentinel,
                "override should be active before panic"
            );
            panic!("intentional panic to test guard cleanup");
        });
        assert!(result.is_err(), "closure should have panicked");

        // After unwind, the override must be gone.
        let after_panic = crate::api::textquest_config_path();
        assert_ne!(
            after_panic,
            sentinel,
            "override must be cleared after panic unwind — found: {}",
            after_panic.display()
        );

        // A subsequent guard must work correctly, proving no lock poisoning.
        let dir2 = tempdir().expect("tempdir2");
        let sentinel2 = dir2.path().join("sentinel2.toml");
        {
            let _guard = ConfigPathGuard::set(&sentinel2);
            assert_eq!(crate::api::textquest_config_path(), sentinel2);
        }
        // Restored after normal drop.
        assert_ne!(crate::api::textquest_config_path(), sentinel2);
    }
}
