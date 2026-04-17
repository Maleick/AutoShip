//! REST API handlers for the web dashboard.

pub mod chat_log;
pub mod chat_pattern_rules;
pub mod dashboard;
pub mod discord;
pub mod economy;
pub mod loot;
pub mod say_detection;
pub mod soul;
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
    sync::Arc,
};
use textquest_common::box_chat::BoxChatConfig;
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

    if let Some(enabled) = item.get("enabled") {
        if let Some(val) = enabled.as_bool() {
            settings.enabled = val;
        }
    }

    if let Some(channels) = item.get("channels") {
        if let Ok(ch) = toml_edit::de::from_str::<Vec<String>>(&channels.to_string()) {
            settings.channels = ch;
        }
    }

    if let Some(rotation) = item.get("rotation_strategy") {
        settings.rotation_strategy = rotation.to_string().trim_matches('"').to_string();
    }

    if let Some(size) = item.get("max_file_size_bytes") {
        if let Some(val) = size.as_integer() {
            settings.max_file_size_bytes = val as u64;
        }
    }

    if let Some(level) = item.get("min_level") {
        settings.min_level = level.to_string().trim_matches('"').to_string();
    }

    if let Some(eq_chat) = item.get("log_eq_chat") {
        if let Some(val) = eq_chat.as_bool() {
            settings.log_eq_chat = val;
        }
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
    table["channels"] = value(settings.channels.clone());
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
    pub group_override: bool,
    pub group_name: Option<String>,
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
            group_override: false,
            group_name: Some("Group 1".into()),
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
            group_override: false,
            group_name: Some("Group 1".into()),
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
            group_override: false,
            group_name: Some("Group 2".into()),
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
            group_override: false,
            group_name: Some("Group 2".into()),
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
            group_override: false,
            group_name: Some("Group 3".into()),
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
    Json(mut config): Json<CharacterConfig>,
) -> Result<Json<CharacterConfig>, StatusCode> {
    if name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    config.character_name = name;
    {
        let mut configs_map = state.character_configs.write().await;
        configs_map.insert(config.character_name.clone(), config.clone());
    }
    Ok(Json(config))
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
        let state = Arc::new(AppState {
            event_tx: tokio::sync::broadcast::channel::<String>(8).0,
            account_store: std::sync::Mutex::new(crate::accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(demo_character_configs()),
            auto_accept_settings: tokio::sync::RwLock::new(AutoAcceptSettings::default()),
            loot_state: crate::api::loot::LootState::new_demo(),
            economy_state: crate::api::economy::EconomyState::new_demo(),
            dashboard_state: crate::api::dashboard::DashboardState::new_demo(),
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            discord_state: crate::api::discord::DiscordState::new_demo(),
            player_watch_config: tokio::sync::RwLock::new(PlayerWatchConfig::default()),
            gm_alert_state: std::sync::Arc::new(crate::api::gm_alerts::GmAlertState::default()),
            spawn_alerts: crate::api::spawn_alerts::SpawnAlertState::new_demo(),
            timestamp_configs: tokio::sync::RwLock::new(Default::default()),
            kill_tracker_state: crate::api::kill_tracker::KillTrackerState::new_demo(),
            api_token: None,
            live_session_snapshot_path: test_live_session_snapshot_path("api-sessions-ok.json"),
            xassist_configs: crate::api::xassist::demo_xassist_configs(),
            chat_pattern_rules: crate::api::chat_pattern_rules::load_rules_state(),
        });
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
        let state = Arc::new(AppState {
            event_tx: tokio::sync::broadcast::channel::<String>(8).0,
            account_store: std::sync::Mutex::new(crate::accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(demo_character_configs()),
            auto_accept_settings: tokio::sync::RwLock::new(AutoAcceptSettings::default()),
            loot_state: crate::api::loot::LootState::new_demo(),
            economy_state: crate::api::economy::EconomyState::new_demo(),
            dashboard_state: crate::api::dashboard::DashboardState::new_demo(),
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            discord_state: crate::api::discord::DiscordState::new_demo(),
            player_watch_config: tokio::sync::RwLock::new(PlayerWatchConfig::default()),
            gm_alert_state: Arc::new(crate::api::gm_alerts::GmAlertState::default()),
            spawn_alerts: crate::api::spawn_alerts::SpawnAlertState::new_demo(),
            timestamp_configs: tokio::sync::RwLock::new(Default::default()),
            kill_tracker_state: crate::api::kill_tracker::KillTrackerState::new_demo(),
            api_token: None,
            live_session_snapshot_path: test_live_session_snapshot_path(
                "api-character-configs-demo.json",
            ),
            xassist_configs: crate::api::xassist::demo_xassist_configs(),
            chat_pattern_rules: crate::api::chat_pattern_rules::load_rules_state(),
        });
        let Json(configs) = list_character_configs(State(state)).await;
        assert!(!configs.is_empty());
        assert!(configs.iter().any(|c| c.character_name == "Frostreaver"));
    }

    #[tokio::test]
    async fn put_character_config_upserts() {
        let state = Arc::new(AppState {
            event_tx: tokio::sync::broadcast::channel::<String>(8).0,
            account_store: std::sync::Mutex::new(crate::accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(demo_character_configs()),
            auto_accept_settings: tokio::sync::RwLock::new(AutoAcceptSettings::default()),
            loot_state: crate::api::loot::LootState::new_demo(),
            economy_state: crate::api::economy::EconomyState::new_demo(),
            dashboard_state: crate::api::dashboard::DashboardState::new_demo(),
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            discord_state: crate::api::discord::DiscordState::new_demo(),
            player_watch_config: tokio::sync::RwLock::new(PlayerWatchConfig::default()),
            gm_alert_state: Arc::new(crate::api::gm_alerts::GmAlertState::default()),
            spawn_alerts: crate::api::spawn_alerts::SpawnAlertState::new_demo(),
            timestamp_configs: tokio::sync::RwLock::new(Default::default()),
            kill_tracker_state: crate::api::kill_tracker::KillTrackerState::new_demo(),
            api_token: None,
            live_session_snapshot_path: test_live_session_snapshot_path(
                "api-put-character-config.json",
            ),
            xassist_configs: crate::api::xassist::demo_xassist_configs(),
            chat_pattern_rules: crate::api::chat_pattern_rules::load_rules_state(),
        });
        let input = CharacterConfig {
            character_name: "IgnoredName".into(),
            class: "Wizard".into(),
            role: "DPS".into(),
            heal_at_pct: 50,
            mana_sit_pct: 15,
            nuke_at_pct: 70,
            rotation: vec![],
            class_params: ClassParams::default(),
            group_override: false,
            group_name: None,
        };
        let Json(saved) =
            put_character_config(State(state.clone()), Path("Aelrindel".into()), Json(input))
                .await
                .expect("put character config should succeed");
        assert_eq!(saved.character_name, "Aelrindel");

        let Json(configs) = list_character_configs(State(state)).await;
        let updated = configs
            .into_iter()
            .find(|c| c.character_name == "Aelrindel")
            .expect("updated config should exist");
        assert_eq!(updated.heal_at_pct, 50);
    }

    #[tokio::test]
    async fn auto_accept_settings_round_trip() {
        let state = Arc::new(AppState {
            event_tx: tokio::sync::broadcast::channel::<String>(8).0,
            account_store: std::sync::Mutex::new(crate::accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(demo_character_configs()),
            auto_accept_settings: tokio::sync::RwLock::new(AutoAcceptSettings::default()),
            loot_state: crate::api::loot::LootState::new_demo(),
            economy_state: crate::api::economy::EconomyState::new_demo(),
            dashboard_state: crate::api::dashboard::DashboardState::new_demo(),
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            discord_state: crate::api::discord::DiscordState::new_demo(),
            player_watch_config: tokio::sync::RwLock::new(PlayerWatchConfig::default()),
            gm_alert_state: Arc::new(crate::api::gm_alerts::GmAlertState::default()),
            spawn_alerts: crate::api::spawn_alerts::SpawnAlertState::new_demo(),
            timestamp_configs: tokio::sync::RwLock::new(Default::default()),
            kill_tracker_state: crate::api::kill_tracker::KillTrackerState::new_demo(),
            api_token: None,
            live_session_snapshot_path: test_live_session_snapshot_path(
                "api-auto-accept-round-trip.json",
            ),
            xassist_configs: crate::api::xassist::demo_xassist_configs(),
            chat_pattern_rules: crate::api::chat_pattern_rules::load_rules_state(),
        });
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
        let state = Arc::new(AppState {
            event_tx: tokio::sync::broadcast::channel::<String>(8).0,
            account_store: std::sync::Mutex::new(crate::accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(demo_character_configs()),
            auto_accept_settings: tokio::sync::RwLock::new(AutoAcceptSettings::default()),
            loot_state: crate::api::loot::LootState::new_demo(),
            economy_state: crate::api::economy::EconomyState::new_demo(),
            dashboard_state: crate::api::dashboard::DashboardState::new_demo(),
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            discord_state: crate::api::discord::DiscordState::new_demo(),
            player_watch_config: tokio::sync::RwLock::new(PlayerWatchConfig::default()),
            gm_alert_state: Arc::new(crate::api::gm_alerts::GmAlertState::default()),
            spawn_alerts: crate::api::spawn_alerts::SpawnAlertState::new_demo(),
            timestamp_configs: tokio::sync::RwLock::new(Default::default()),
            kill_tracker_state: crate::api::kill_tracker::KillTrackerState::new_demo(),
            api_token: None,
            live_session_snapshot_path: test_live_session_snapshot_path(
                "api-put-auto-accept-blank-reject.json",
            ),
            xassist_configs: crate::api::xassist::demo_xassist_configs(),
            chat_pattern_rules: crate::api::chat_pattern_rules::load_rules_state(),
        });
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
