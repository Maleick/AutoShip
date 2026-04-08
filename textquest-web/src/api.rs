//! REST API handlers for the web dashboard.

pub mod loot;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::AppState;

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

/// List active sessions (placeholder — will connect to IPC).
pub async fn list_sessions() -> Json<Vec<SessionInfo>> {
    // TODO: Read real session data from IPC / shared memory
    Json(vec![SessionInfo {
        client_id: 1,
        character_name: "Frostreaver".into(),
        zone: "East Commonlands".into(),
        level: 50,
        hp_pct: 100.0,
        mana_pct: 85.0,
        status: "idle".into(),
    }])
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
pub async fn list_character_configs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<CharacterConfig>>, StatusCode> {
    let configs_map = state.character_configs.read().map_err(|_|
        // It's good practice to log this error for observability.
        StatusCode::INTERNAL_SERVER_ERROR
    )?;
    let mut configs = configs_map
        .values()
        .cloned()
        .collect::<Vec<_>>();
    configs.sort_by(|a, b| a.character_name.cmp(&b.character_name));
    Ok(Json(configs))
}

/// PUT /api/config/characters/:name — upsert per-character tuning config.
pub async fn put_character_config(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
    Json(mut config): Json<CharacterConfig>,
) -> Result<Json<CharacterConfig>, StatusCode> {
    if name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    config.character_name = name;
    let mut configs_map = state
        .character_configs
        .write()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    configs_map.insert(config.character_name.clone(), config.clone());
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
pub async fn get_economy_settings() -> Json<EconomySettings> {
    // TODO: Load from persistent config (TOML / SQLite)
    Json(EconomySettings {
        krono: KronoSettings {
            target_rate_per_day: 3,
            min_sell_price: 800,
            max_buy_price: 750,
            restock_threshold: 5,
            enabled: true,
        },
        banking_rules: vec![
            BankingRule {
                id: "br-1".into(),
                item_category: "Plat".into(),
                deposit_threshold: 5000,
                keep_on_hand: 500,
                auto_deposit: true,
            },
            BankingRule {
                id: "br-2".into(),
                item_category: "Krono".into(),
                deposit_threshold: 10,
                keep_on_hand: 2,
                auto_deposit: true,
            },
            BankingRule {
                id: "br-3".into(),
                item_category: "Tradeskill Mats".into(),
                deposit_threshold: 200,
                keep_on_hand: 20,
                auto_deposit: false,
            },
        ],
        tradeskill_supplies: vec![
            TradeskillSupply {
                id: "ts-1".into(),
                skill: "Tailoring".into(),
                materials: vec![
                    "Silk Threads".into(),
                    "Animal Pelts".into(),
                    "Spiderling Silk".into(),
                ],
                restock_quantity: 100,
                source_zone: "Lower Guk".into(),
                enabled: true,
            },
            TradeskillSupply {
                id: "ts-2".into(),
                skill: "Smithing".into(),
                materials: vec!["Iron Ore".into(), "Coal".into(), "High Quality Ore".into()],
                restock_quantity: 50,
                source_zone: "Kaladim".into(),
                enabled: true,
            },
        ],
    })
}

/// PUT /api/economy/settings — update full economy configuration.
pub async fn put_economy_settings(Json(settings): Json<EconomySettings>) -> Json<EconomySettings> {
    // TODO: Persist to config file / SQLite
    Json(settings)
}

/// GET /api/economy/vendor-routes — list all vendor routes.
pub async fn list_vendor_routes() -> Json<Vec<VendorRoute>> {
    // TODO: Load from persistent storage
    Json(vec![
        VendorRoute {
            id: "vr-1".into(),
            zone: "East Commonlands".into(),
            npc_name: "Merchant Ooldi".into(),
            path_notes: "Near zone-in from West Commonlands, sells food/drink.".into(),
            item_categories: vec!["Food".into(), "Drink".into(), "Reagents".into()],
            enabled: true,
        },
        VendorRoute {
            id: "vr-2".into(),
            zone: "Neriak Commons".into(),
            npc_name: "Vira S`Lex".into(),
            path_notes: "Inside the Neriak armory building, sells weapons.".into(),
            item_categories: vec!["Weapons".into(), "Armor".into()],
            enabled: true,
        },
    ])
}

/// POST /api/economy/vendor-routes — create a vendor route.
pub async fn create_vendor_route(Json(route): Json<VendorRoute>) -> Json<VendorRoute> {
    // TODO: Persist to storage
    Json(route)
}

/// PUT /api/economy/vendor-routes/:id — update a vendor route.
pub async fn update_vendor_route(
    Path(id): Path<String>,
    Json(mut route): Json<VendorRoute>,
) -> Json<VendorRoute> {
    // TODO: Persist to storage
    route.id = id;
    Json(route)
}

/// DELETE /api/economy/vendor-routes/:id — delete a vendor route.
pub async fn delete_vendor_route(Path(_id): Path<String>) -> axum::http::StatusCode {
    // TODO: Remove from storage
    axum::http::StatusCode::NO_CONTENT
}

/// GET /api/economy/wealth — wealth history and current snapshot.
pub async fn get_wealth() -> Json<WealthHistory> {
    // TODO: Load from metrics SQLite DB
    let snapshots = vec![
        WealthSnapshot {
            timestamp: "2026-04-01T00:00:00Z".into(),
            plat: 120_000,
            krono: 28,
            item_value_estimate: 210_000,
        },
        WealthSnapshot {
            timestamp: "2026-04-02T00:00:00Z".into(),
            plat: 134_500,
            krono: 30,
            item_value_estimate: 230_000,
        },
        WealthSnapshot {
            timestamp: "2026-04-03T00:00:00Z".into(),
            plat: 148_200,
            krono: 33,
            item_value_estimate: 255_000,
        },
        WealthSnapshot {
            timestamp: "2026-04-04T00:00:00Z".into(),
            plat: 155_900,
            krono: 35,
            item_value_estimate: 270_000,
        },
        WealthSnapshot {
            timestamp: "2026-04-05T00:00:00Z".into(),
            plat: 164_100,
            krono: 38,
            item_value_estimate: 285_000,
        },
        WealthSnapshot {
            timestamp: "2026-04-06T00:00:00Z".into(),
            plat: 172_800,
            krono: 40,
            item_value_estimate: 298_000,
        },
        WealthSnapshot {
            timestamp: "2026-04-07T00:00:00Z".into(),
            plat: 187_430,
            krono: 42,
            item_value_estimate: 312_000,
        },
    ];
    let current = snapshots.last().unwrap().clone();
    Json(WealthHistory { current, snapshots })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_returns_ok() {
        let Json(resp) = health().await;
        assert_eq!(resp.status, "ok");
    }

    #[tokio::test]
    async fn sessions_returns_placeholder() {
        let Json(sessions) = list_sessions().await;
        assert!(!sessions.is_empty());
        assert_eq!(sessions[0].character_name, "Frostreaver");
    }

    #[tokio::test]
    async fn economy_settings_returns_krono_config() {
        let Json(settings) = get_economy_settings().await;
        assert!(settings.krono.enabled);
        assert_eq!(settings.krono.target_rate_per_day, 3);
        assert!(!settings.banking_rules.is_empty());
        assert!(!settings.tradeskill_supplies.is_empty());
    }

    #[tokio::test]
    async fn vendor_routes_returns_list() {
        let Json(routes) = list_vendor_routes().await;
        assert!(!routes.is_empty());
        assert!(routes.iter().all(|r| !r.zone.is_empty()));
    }

    #[tokio::test]
    async fn wealth_history_has_snapshots() {
        let Json(history) = get_wealth().await;
        assert!(!history.snapshots.is_empty());
        assert_eq!(history.current.plat, history.snapshots.last().unwrap().plat);
    }

    #[tokio::test]
    async fn put_economy_settings_roundtrips() {
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
        let Json(returned) = put_economy_settings(Json(settings)).await;
        assert_eq!(returned.krono.target_rate_per_day, 5);
        assert!(!returned.krono.enabled);
    }

    #[tokio::test]
    async fn create_vendor_route_roundtrips() {
        let route = VendorRoute {
            id: "vr-test".into(),
            zone: "Test Zone".into(),
            npc_name: "Test NPC".into(),
            path_notes: "Test notes".into(),
            item_categories: vec!["Test".into()],
            enabled: true,
        };
        let Json(returned) = create_vendor_route(Json(route)).await;
        assert_eq!(returned.id, "vr-test");
        assert_eq!(returned.zone, "Test Zone");
    }

    #[tokio::test]
    async fn character_configs_returns_demo_data() {
        let state = Arc::new(AppState {
            event_tx: tokio::sync::broadcast::channel::<String>(8).0,
            character_configs: tokio::sync::RwLock::new(demo_character_configs()),
        });
        let Json(configs) = list_character_configs(State(state)).await;
        assert!(!configs.is_empty());
        assert!(configs.iter().any(|c| c.character_name == "Frostreaver"));
    }

    #[tokio::test]
    async fn put_character_config_upserts() {
        let state = Arc::new(AppState {
            event_tx: tokio::sync::broadcast::channel::<String>(8).0,
            character_configs: tokio::sync::RwLock::new(demo_character_configs()),
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
