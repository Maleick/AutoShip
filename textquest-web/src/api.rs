//! REST API handlers for the web dashboard.

#![allow(dead_code)] // Demo shapes and placeholder handlers stay in this module before router wiring.

pub mod dashboard;
pub mod economy;
pub mod loot;
pub mod soul;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use textquest_common::box_chat::BoxChatConfig;
use textquest_common::ipc::AutoRezConfig;
use toml_edit::{DocumentMut, Item, Table, value};

use crate::AppState;

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
    pub status: String,
}

/// List active sessions.
pub async fn list_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // Build a list of sessions from character configs.
    // In a production system, this would read from IPC shared memory or a session
    // registry.
    let configs = state.character_configs.read().await;
    let sessions: Vec<SessionInfo> = configs
        .values()
        .enumerate()
        .map(|(idx, cfg)| SessionInfo {
            client_id: idx as u32 + 1,
            character_name: cfg.character_name.clone(),
            zone: "Unknown".to_string(), // Would come from game state
            level: 50,                   // Would come from game state
            hp_pct: 100.0,               // Would come from game state
            mana_pct: 100.0,             // Would come from game state
            status: "idle".to_string(),  // Would come from game state
        })
        .collect();

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
pub struct AutoCampOnDeathConfig {
    pub enabled: bool,
    pub camp_delay_secs: u64,
    pub relog_wait_secs: u64,
}

impl Default for AutoCampOnDeathConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            camp_delay_secs: 30,
            relog_wait_secs: 900,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TributeAlertState {
    Ok,
    Expiring,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    #[serde(default)]
    pub auto_camp_on_death: AutoCampOnDeathConfig,
    pub tribute_preferences: TributePreferences,
    pub tribute_status: TributeStatus,
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
    #[serde(default)]
    pub auto_rez: AutoRezConfig,
    pub group_override: bool,
    pub group_name: Option<String>,
    #[serde(default)]
    pub auto_camp_on_death: AutoCampOnDeathConfig,
    pub tribute_preferences: TributePreferences,
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
            auto_camp_on_death: AutoCampOnDeathConfig {
                enabled: true,
                camp_delay_secs: 30,
                relog_wait_secs: 900,
            },
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
            auto_camp_on_death: AutoCampOnDeathConfig {
                enabled: false,
                camp_delay_secs: 30,
                relog_wait_secs: 900,
            },
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
            auto_camp_on_death: AutoCampOnDeathConfig {
                enabled: true,
                camp_delay_secs: 45,
                relog_wait_secs: 1200,
            },
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
            auto_camp_on_death: AutoCampOnDeathConfig {
                enabled: false,
                camp_delay_secs: 30,
                relog_wait_secs: 900,
            },
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
                min_xp_pct: 90,
                trusted_casters: vec!["Frostreaver".into(), "Highclerk".into()],
                decline_if_untrusted: true,
                delay_ms: 3_500,
            },
            group_override: false,
            group_name: Some("Group 3".into()),
            auto_camp_on_death: AutoCampOnDeathConfig {
                enabled: false,
                camp_delay_secs: 30,
                relog_wait_secs: 900,
            },
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
) -> Result<Json<CharacterConfig>, StatusCode> {
    if name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut configs_map = state.character_configs.write().await;
    let tribute_status = configs_map
        .get(&name)
        .map(|existing| existing.tribute_status.clone())
        .unwrap_or_default();
    let saved = CharacterConfig {
        character_name: name,
        class: config.class,
        role: config.role,
        heal_at_pct: config.heal_at_pct,
        mana_sit_pct: config.mana_sit_pct,
        nuke_at_pct: config.nuke_at_pct,
        rotation: config.rotation,
        class_params: config.class_params,
        auto_rez: config.auto_rez,
        group_override: config.group_override,
        group_name: config.group_name,
        auto_camp_on_death: config.auto_camp_on_death,
        tribute_preferences: config.tribute_preferences,
        tribute_status,
    };
    configs_map.insert(saved.character_name.clone(), saved.clone());
    Ok(Json(saved))
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
    use tempfile::tempdir;

    struct ConfigPathGuard {
        original: Option<String>,
    }

    impl ConfigPathGuard {
        fn set(path: &std::path::Path) -> Self {
            let original = std::env::var("TEXTQUEST_CONFIG_PATH").ok();
            unsafe {
                std::env::set_var("TEXTQUEST_CONFIG_PATH", path);
            }
            Self { original }
        }
    }

    impl Drop for ConfigPathGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.original {
                    Some(value) => std::env::set_var("TEXTQUEST_CONFIG_PATH", value),
                    None => std::env::remove_var("TEXTQUEST_CONFIG_PATH"),
                }
            }
        }
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
    async fn box_chat_settings_default_when_config_missing() {
        let dir = tempdir().expect("tempdir should exist");
        let config_path = dir.path().join("textquest.toml");
        let _guard = ConfigPathGuard::set(&config_path);

        let response = get_box_chat_settings().await.into_response();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should collect")
            .to_bytes();
        let settings: BoxChatConfig =
            serde_json::from_slice(&body).expect("settings response should parse");

        assert_eq!(settings, BoxChatConfig::default());
    }

    #[tokio::test]
    async fn put_box_chat_settings_writes_config_section() {
        let dir = tempdir().expect("tempdir should exist");
        let config_path = dir.path().join("textquest.toml");
        std::fs::write(
            &config_path,
            "process_name = \"eqgame.exe\"\n[launch]\neq_path = \"C:/EQ\"\n",
        )
        .expect("seed config should write");
        let _guard = ConfigPathGuard::set(&config_path);

        let settings = BoxChatConfig {
            enabled: true,
            host: "192.168.1.25".to_string(),
            port: 3002,
            auto_connect: true,
        };

        let response = put_box_chat_settings(Json(settings.clone()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let written = std::fs::read_to_string(&config_path).expect("config should exist");
        assert!(written.contains("process_name = \"eqgame.exe\""));
        assert!(written.contains("[box_chat]"));
        assert!(written.contains("host = \"192.168.1.25\""));

        let response = get_box_chat_settings().await.into_response();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should collect")
            .to_bytes();
        let reloaded: BoxChatConfig =
            serde_json::from_slice(&body).expect("settings response should parse");
        assert_eq!(reloaded, settings);
    }

    #[tokio::test]
    async fn put_box_chat_settings_rejects_zero_port() {
        let dir = tempdir().expect("tempdir should exist");
        let config_path = dir.path().join("textquest.toml");
        let _guard = ConfigPathGuard::set(&config_path);

        let response = put_box_chat_settings(Json(BoxChatConfig {
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 0,
            auto_connect: true,
        }))
        .await
        .into_response();

        let (status, body) = error_response_json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            body,
            serde_json::json!({ "error": "Box chat port must be between 1 and 65535" })
        );
    }

    #[tokio::test]
    async fn sessions_returns_ok() {
        let state = Arc::new(AppState {
            event_tx: tokio::sync::broadcast::channel::<String>(8).0,
            account_store: std::sync::Mutex::new(crate::accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(demo_character_configs()),
            loot_state: crate::api::loot::LootState::new_demo(),
            economy_state: crate::api::economy::EconomyState::new_demo(),
            dashboard_state: crate::api::dashboard::DashboardState::new_demo(),
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            api_token: None,
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
            loot_state: crate::api::loot::LootState::new_demo(),
            economy_state: crate::api::economy::EconomyState::new_demo(),
            dashboard_state: crate::api::dashboard::DashboardState::new_demo(),
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            api_token: None,
        });
        let Json(configs) = list_character_configs(State(state)).await;
        assert!(!configs.is_empty());
        assert!(configs.iter().any(|c| c.character_name == "Frostreaver"));
        assert!(
            configs
                .iter()
                .any(|c| c.auto_camp_on_death.enabled && c.auto_camp_on_death.camp_delay_secs == 30)
        );
        let frostreaver = configs
            .into_iter()
            .find(|c| c.character_name == "Frostreaver")
            .expect("demo config should exist");
        let json = serde_json::to_value(frostreaver).expect("config should serialize");
        assert_eq!(json["tribute_preferences"]["auto_activate"], true);
        assert_eq!(
            json["tribute_status"]["point_balance"],
            serde_json::json!(3_200)
        );
        assert_eq!(
            json["tribute_status"]["alert_state"],
            serde_json::json!("expiring")
        );
    }

    #[tokio::test]
    async fn put_character_config_upserts() {
        let state = Arc::new(AppState {
            event_tx: tokio::sync::broadcast::channel::<String>(8).0,
            account_store: std::sync::Mutex::new(crate::accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(demo_character_configs()),
            loot_state: crate::api::loot::LootState::new_demo(),
            economy_state: crate::api::economy::EconomyState::new_demo(),
            dashboard_state: crate::api::dashboard::DashboardState::new_demo(),
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            api_token: None,
        });
        let input = CharacterConfigUpdate {
            character_name: "IgnoredName".into(),
            class: "Wizard".into(),
            role: "DPS".into(),
            heal_at_pct: 50,
            mana_sit_pct: 15,
            nuke_at_pct: 70,
            rotation: vec![],
            class_params: ClassParams::default(),
            auto_rez: AutoRezConfig {
                enabled: true,
                min_xp_pct: 96,
                trusted_casters: vec!["Frostreaver".into()],
                decline_if_untrusted: true,
                delay_ms: 5_100,
            },
            group_override: false,
            group_name: None,
            auto_camp_on_death: AutoCampOnDeathConfig {
                enabled: true,
                camp_delay_secs: 75,
                relog_wait_secs: 1800,
            },
            tribute_preferences: tribute_preferences(&["Arcane Fury", "Hero's Fortitude"], 180),
        };
        let Json(saved) =
            put_character_config(State(state.clone()), Path("Aelrindel".into()), Json(input))
                .await
                .expect("put character config should succeed");
        assert_eq!(saved.character_name, "Aelrindel");
        assert_eq!(
            saved.auto_camp_on_death,
            AutoCampOnDeathConfig {
                enabled: true,
                camp_delay_secs: 75,
                relog_wait_secs: 1800,
            }
        );
        let saved_json = serde_json::to_value(&saved).expect("saved config should serialize");
        assert_eq!(
            saved_json["tribute_preferences"]["preferred_tributes"],
            serde_json::json!(["Arcane Fury", "Hero's Fortitude"])
        );
        assert_eq!(
            saved_json["tribute_status"]["alert_state"],
            serde_json::json!("expired")
        );
        assert_eq!(saved_json["tribute_status"]["point_balance"], serde_json::json!(875));

        let Json(configs) = list_character_configs(State(state)).await;
        let updated = configs
            .into_iter()
            .find(|c| c.character_name == "Aelrindel")
            .expect("updated config should exist");
        assert_eq!(updated.heal_at_pct, 50);
        assert_eq!(updated.auto_rez.min_xp_pct, 96);
        assert_eq!(updated.auto_rez.trusted_casters, vec!["Frostreaver"]);
        assert!(updated.auto_camp_on_death.enabled);
    }

    #[test]
    fn character_config_defaults_missing_death_config() {
        let config: CharacterConfig = serde_json::from_value(serde_json::json!({
            "character_name": "Aelrindel",
            "class": "Wizard",
            "role": "DPS",
            "heal_at_pct": 45,
            "mana_sit_pct": 20,
            "nuke_at_pct": 80,
            "rotation": [],
            "class_params": {},
            "auto_rez": {
                "enabled": true,
                "min_xp_pct": 96,
                "trusted_casters": ["Frostreaver"],
                "decline_if_untrusted": true,
                "delay_ms": 5100
            },
            "group_override": false,
            "group_name": null
        }))
        .expect("legacy payload should deserialize");

        assert_eq!(config.auto_camp_on_death, AutoCampOnDeathConfig::default());
    }
}
