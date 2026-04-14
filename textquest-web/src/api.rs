//! REST API handlers for the web dashboard.

#![allow(dead_code)] // Demo shapes and placeholder handlers stay in this module before router wiring.

pub mod economy;
pub mod loot;
pub mod soul;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

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

/// Placeholder response for known raid-config endpoints that are not implemented on this build.
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

// ─── Health ───────────────────────────────────────────────────────────────────

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

// ─── Sessions ─────────────────────────────────────────────────────────────────

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
pub async fn list_sessions() -> impl IntoResponse {
    live_state_unavailable("Session monitoring API is not backed by live state in this build")
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
/// Not yet mounted in the live API router (returns 501 via placeholder); kept for future use.
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
/// Not yet mounted in the live API router (returns 501 via placeholder); kept for future use.
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

// ── Economy types ─────────────────────────────────────────────────────────────

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

// ── Economy handlers ──────────────────────────────────────────────────────────

/// GET /api/economy/settings — return full economy configuration.
pub async fn get_economy_settings() -> impl IntoResponse {
    live_state_unavailable("Economy settings API is not backed by live state in this build")
}

/// PUT /api/economy/settings — update full economy configuration.
pub async fn put_economy_settings(Json(_settings): Json<EconomySettings>) -> impl IntoResponse {
    live_state_unavailable("Economy settings API does not persist changes in this build")
}

/// GET /api/economy/vendor-routes — list all vendor routes.
pub async fn list_vendor_routes() -> impl IntoResponse {
    live_state_unavailable("Vendor route API is not backed by live state in this build")
}

/// POST /api/economy/vendor-routes — create a vendor route.
pub async fn create_vendor_route(Json(_route): Json<VendorRoute>) -> impl IntoResponse {
    live_state_unavailable("Vendor route API does not persist changes in this build")
}

/// PUT /api/economy/vendor-routes/:id — update a vendor route.
pub async fn update_vendor_route(
    Path(_id): Path<String>,
    Json(_route): Json<VendorRoute>,
) -> impl IntoResponse {
    live_state_unavailable("Vendor route API does not persist changes in this build")
}

/// DELETE /api/economy/vendor-routes/:id — delete a vendor route.
pub async fn delete_vendor_route(Path(_id): Path<String>) -> impl IntoResponse {
    live_state_unavailable("Vendor route API does not persist changes in this build")
}

/// GET /api/economy/wealth — wealth history and current snapshot.
pub async fn get_wealth() -> impl IntoResponse {
    live_state_unavailable("Wealth API is not backed by live state in this build")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;
    use serde_json::Value;

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
    async fn sessions_returns_not_implemented() {
        let (status, body) = error_response_json(list_sessions().await.into_response()).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(
            body["error"],
            "Session monitoring API is not backed by live state in this build"
        );
    }

    #[tokio::test]
    async fn economy_settings_returns_not_implemented() {
        let (status, body) =
            error_response_json(get_economy_settings().await.into_response()).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(
            body["error"],
            "Economy settings API is not backed by live state in this build"
        );
    }

    #[tokio::test]
    async fn vendor_routes_returns_not_implemented() {
        let (status, body) = error_response_json(list_vendor_routes().await.into_response()).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(
            body["error"],
            "Vendor route API is not backed by live state in this build"
        );
    }

    #[tokio::test]
    async fn wealth_history_returns_not_implemented() {
        let (status, body) = error_response_json(get_wealth().await.into_response()).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(
            body["error"],
            "Wealth API is not backed by live state in this build"
        );
    }

    #[tokio::test]
    async fn put_economy_settings_returns_not_implemented() {
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
        let (status, body) =
            error_response_json(put_economy_settings(Json(settings)).await.into_response()).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(
            body["error"],
            "Economy settings API does not persist changes in this build"
        );
    }

    #[tokio::test]
    async fn create_vendor_route_returns_not_implemented() {
        let route = VendorRoute {
            id: "vr-test".into(),
            zone: "Test Zone".into(),
            npc_name: "Test NPC".into(),
            path_notes: "Test notes".into(),
            item_categories: vec!["Test".into()],
            enabled: true,
        };
        let (status, body) =
            error_response_json(create_vendor_route(Json(route)).await.into_response()).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(
            body["error"],
            "Vendor route API does not persist changes in this build"
        );
    }

    #[tokio::test]
    async fn update_vendor_route_returns_not_implemented() {
        let route = VendorRoute {
            id: "ignored".into(),
            zone: "Test Zone".into(),
            npc_name: "Test NPC".into(),
            path_notes: "Test notes".into(),
            item_categories: vec!["Test".into()],
            enabled: true,
        };
        let (status, body) = error_response_json(
            update_vendor_route(Path("vr-test".into()), Json(route))
                .await
                .into_response(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(
            body["error"],
            "Vendor route API does not persist changes in this build"
        );
    }

    #[tokio::test]
    async fn delete_vendor_route_returns_not_implemented() {
        let (status, body) = error_response_json(
            delete_vendor_route(Path("vr-test".into()))
                .await
                .into_response(),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(
            body["error"],
            "Vendor route API does not persist changes in this build"
        );
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
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            api_token: None,
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
            loot_state: crate::api::loot::LootState::new_demo(),
            economy_state: crate::api::economy::EconomyState::new_demo(),
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            api_token: None,
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
}
