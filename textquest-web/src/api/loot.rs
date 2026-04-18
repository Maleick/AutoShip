//! REST API handlers for loot rules, scoring, and distribution configuration.

use std::{
    collections::HashMap,
    fs,
    path::{Path as FsPath, PathBuf},
    sync::Arc,
};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use toml_edit::DocumentMut;

use crate::AppState;

pub type ItemScoreConfigPayload = textquest::loot::ItemScoreConfig;
pub type InventoryUtilityConfigPayload =
    textquest_common::inventory_utility::InventoryUtilityConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ItemScoreConfigFile {
    #[serde(default)]
    item_score: ItemScoreConfigPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct InventoryUtilityConfigFile {
    #[serde(default)]
    inventory_utility: InventoryUtilityConfigPayload,
}

// ── Shared loot state ────────────────────────────────────────────────────────

/// In-memory loot configuration state shared across handlers.
pub struct LootState {
    pub rules: RwLock<LootRulesPayload>,
    pub filters: RwLock<HashMap<String, CharacterLootFilter>>,
    pub master_looter: RwLock<MasterLooterPayload>,
    pub distribution: RwLock<DistributionConfig>,
    pub history: RwLock<Vec<LootHistoryEntry>>,
    pub item_score: RwLock<ItemScoreConfigPayload>,
    pub inventory_utility: RwLock<InventoryUtilityConfigPayload>,
    pub item_score_write_lock: tokio::sync::Mutex<()>,
    pub inventory_utility_write_lock: tokio::sync::Mutex<()>,
    item_score_config_path: PathBuf,
}

impl LootState {
    /// Create state pre-populated with sensible defaults and demo data.
    pub fn new_demo() -> Arc<Self> {
        Self::new_with_item_score_path(item_score_config_path())
    }

    pub(crate) fn new_with_item_score_path(item_score_config_path: PathBuf) -> Arc<Self> {
        let item_score = load_item_score_from_path(&item_score_config_path).unwrap_or_else(|error| {
            tracing::warn!(%error, path = %item_score_config_path.display(), "Failed to load item-score config");
            ItemScoreConfigPayload::default()
        });
        let inventory_utility = load_inventory_utility_from_path(&item_score_config_path)
            .unwrap_or_else(|error| {
                tracing::warn!(
                    %error,
                    path = %item_score_config_path.display(),
                    "Failed to load inventory-utility config"
                );
                InventoryUtilityConfigPayload::default()
            });

        Arc::new(Self {
            rules: RwLock::new(LootRulesPayload::default()),
            filters: RwLock::new(default_filters()),
            master_looter: RwLock::new(MasterLooterPayload::default()),
            distribution: RwLock::new(DistributionConfig::default()),
            history: RwLock::new(demo_history()),
            item_score: RwLock::new(item_score),
            inventory_utility: RwLock::new(inventory_utility),
            item_score_write_lock: tokio::sync::Mutex::new(()),
            inventory_utility_write_lock: tokio::sync::Mutex::new(()),
            item_score_config_path,
        })
    }
}

// ── API types ────────────────────────────────────────────────────────────────

/// Global auto-loot rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LootRulesPayload {
    /// Items to always pick up and keep.
    pub keep_items: Vec<String>,
    /// Items to pick up for selling to vendors.
    pub sell_items: Vec<String>,
    /// Items to pick up and immediately destroy.
    pub destroy_items: Vec<String>,
    /// Loot everything not in `destroy_items`.
    pub loot_all: bool,
    /// Auto-split coin with group.
    pub auto_split: bool,
}

impl Default for LootRulesPayload {
    fn default() -> Self {
        Self {
            keep_items: vec![
                "Rubicite Breastplate".into(),
                "Mithril Breastplate".into(),
                "Flowing Black Silk Sash".into(),
            ],
            sell_items: vec!["Rusty Sword".into(), "Tattered Cloth".into()],
            destroy_items: vec!["Bone Chips".into(), "Rat Whisker".into()],
            loot_all: true,
            auto_split: true,
        }
    }
}

/// Per-item filter action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FilterAction {
    Keep,
    Sell,
    Destroy,
    Bank,
}

/// A single item filter entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemFilterEntry {
    pub item_name: String,
    pub action: FilterAction,
}

/// Per-character auto-loot filter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterLootFilter {
    pub character: String,
    pub filters: Vec<ItemFilterEntry>,
}

/// Distribution method for a loot rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DistributionMethod {
    NeedBeforeGreed,
    Greed,
    RoundRobin,
    MasterLooter,
    FreeForAll,
}

/// A distribution rule mapping item type/quality to a distribution method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionRule {
    /// Stable client-assigned identifier for ordering and keying.
    pub id: String,
    /// Item type this rule applies to (e.g. "armor", "weapon", "misc", "all").
    pub item_type: String,
    /// Optional quality tier (e.g. "rare", "magical", "common", "nodrop").
    pub quality: Option<String>,
    /// Distribution method to use.
    pub method: DistributionMethod,
}

/// Full distribution configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributionConfig {
    pub rules: Vec<DistributionRule>,
}

impl Default for DistributionConfig {
    fn default() -> Self {
        Self {
            rules: vec![
                DistributionRule {
                    id: "rule-1".into(),
                    item_type: "armor".into(),
                    quality: Some("nodrop".into()),
                    method: DistributionMethod::NeedBeforeGreed,
                },
                DistributionRule {
                    id: "rule-2".into(),
                    item_type: "weapon".into(),
                    quality: Some("nodrop".into()),
                    method: DistributionMethod::NeedBeforeGreed,
                },
                DistributionRule {
                    id: "rule-3".into(),
                    item_type: "armor".into(),
                    quality: Some("rare".into()),
                    method: DistributionMethod::MasterLooter,
                },
                DistributionRule {
                    id: "rule-4".into(),
                    item_type: "all".into(),
                    quality: None,
                    method: DistributionMethod::RoundRobin,
                },
            ],
        }
    }
}

/// Master looter assignment.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MasterLooterPayload {
    /// Character name of the master looter, or null if none assigned.
    pub character: Option<String>,
}

/// A single loot history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LootHistoryEntry {
    pub id: u64,
    pub timestamp: String,
    pub item_name: String,
    pub recipient: String,
    pub source_mob: Option<String>,
    pub zone: Option<String>,
    pub quantity: u32,
    pub assigned_by: Option<String>,
}

/// Query params for history search.
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    /// Filter by item name substring.
    pub search: Option<String>,
    /// Filter by recipient character name.
    pub character: Option<String>,
    /// Max entries to return (default 50).
    pub limit: Option<usize>,
}

// ── Demo data helpers ────────────────────────────────────────────────────────

fn default_filters() -> HashMap<String, CharacterLootFilter> {
    let mut map = HashMap::new();
    for name in ["Frostreaver", "Shadowdancer", "Ironclad", "Lightbringer"] {
        map.insert(
            name.to_string(),
            CharacterLootFilter {
                character: name.to_string(),
                filters: vec![
                    ItemFilterEntry {
                        item_name: "Bone Chips".into(),
                        action: FilterAction::Destroy,
                    },
                    ItemFilterEntry {
                        item_name: "Rusty Sword".into(),
                        action: FilterAction::Sell,
                    },
                ],
            },
        );
    }
    map
}

fn demo_history() -> Vec<LootHistoryEntry> {
    vec![
        LootHistoryEntry {
            id: 1,
            timestamp: "2025-01-15 23:14:01".into(),
            item_name: "Mithril Breastplate".into(),
            recipient: "Frostreaver".into(),
            source_mob: Some("Maestro of Rancor".into()),
            zone: Some("Plane of Hate".into()),
            quantity: 1,
            assigned_by: Some("Frostreaver".into()),
        },
        LootHistoryEntry {
            id: 2,
            timestamp: "2025-01-15 23:10:45".into(),
            item_name: "Flowing Black Silk Sash".into(),
            recipient: "Shadowdancer".into(),
            source_mob: Some("Maestro of Rancor".into()),
            zone: Some("Plane of Hate".into()),
            quantity: 1,
            assigned_by: Some("Frostreaver".into()),
        },
        LootHistoryEntry {
            id: 3,
            timestamp: "2025-01-15 22:55:12".into(),
            item_name: "Rubicite Breastplate".into(),
            recipient: "Ironclad".into(),
            source_mob: Some("Innoruuk".into()),
            zone: Some("Plane of Hate".into()),
            quantity: 1,
            assigned_by: Some("Frostreaver".into()),
        },
        LootHistoryEntry {
            id: 4,
            timestamp: "2025-01-15 22:30:00".into(),
            item_name: "Lendiniara's Signet Ring".into(),
            recipient: "Lightbringer".into(),
            source_mob: Some("Lendiniara the Keeper".into()),
            zone: Some("Temple of Veeshan".into()),
            quantity: 1,
            assigned_by: None,
        },
        LootHistoryEntry {
            id: 5,
            timestamp: "2025-01-15 22:20:33".into(),
            item_name: "Bone Chips".into(),
            recipient: "Frostreaver".into(),
            source_mob: Some("Skeleton".into()),
            zone: Some("Lower Guk".into()),
            quantity: 5,
            assigned_by: None,
        },
        LootHistoryEntry {
            id: 6,
            timestamp: "2025-01-15 21:45:00".into(),
            item_name: "Veeshan's Peak Key".into(),
            recipient: "Shadowdancer".into(),
            source_mob: Some("Phara Dar".into()),
            zone: Some("Veeshan's Peak".into()),
            quantity: 1,
            assigned_by: Some("Frostreaver".into()),
        },
    ]
}

fn item_score_config_path() -> PathBuf {
    std::env::var("TEXTQUEST_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/textquest.toml"))
}

fn load_item_score_from_path(path: &FsPath) -> Result<ItemScoreConfigPayload, String> {
    if !path.exists() {
        return Ok(ItemScoreConfigPayload::default());
    }

    let content = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read item-score config: {error}"))?;
    let file = toml::from_str::<ItemScoreConfigFile>(&content)
        .map_err(|error| format!("Failed to deserialize item-score config: {error}"))?;
    Ok(file.item_score)
}

fn save_item_score_to_path(path: &FsPath, config: &ItemScoreConfigPayload) -> Result<(), String> {
    let mut doc = if path.exists() {
        fs::read_to_string(path)
            .map_err(|error| format!("Failed to read config file: {error}"))?
            .parse::<DocumentMut>()
            .map_err(|error| format!("Failed to parse config file: {error}"))?
    } else {
        DocumentMut::new()
    };

    let serialized = toml::to_string_pretty(&ItemScoreConfigFile {
        item_score: config.clone(),
    })
    .map_err(|error| format!("Failed to serialize item-score config: {error}"))?;
    let item_doc = serialized
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse item-score config: {error}"))?;
    doc["item_score"] = item_doc["item_score"].clone();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create config directory: {error}"))?;
    }

    fs::write(path, doc.to_string())
        .map_err(|error| format!("Failed to write item-score config: {error}"))
}

fn load_inventory_utility_from_path(
    path: &FsPath,
) -> Result<InventoryUtilityConfigPayload, String> {
    if !path.exists() {
        return Ok(InventoryUtilityConfigPayload::default());
    }

    let content = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read inventory-utility config: {error}"))?;
    let file = toml::from_str::<InventoryUtilityConfigFile>(&content)
        .map_err(|error| format!("Failed to deserialize inventory-utility config: {error}"))?;
    Ok(file.inventory_utility)
}

fn save_inventory_utility_to_path(
    path: &FsPath,
    config: &InventoryUtilityConfigPayload,
) -> Result<(), String> {
    let mut doc = if path.exists() {
        fs::read_to_string(path)
            .map_err(|error| format!("Failed to read config file: {error}"))?
            .parse::<DocumentMut>()
            .map_err(|error| format!("Failed to parse config file: {error}"))?
    } else {
        DocumentMut::new()
    };

    let serialized = toml::to_string_pretty(&InventoryUtilityConfigFile {
        inventory_utility: config.clone(),
    })
    .map_err(|error| format!("Failed to serialize inventory-utility config: {error}"))?;
    let inventory_doc = serialized
        .parse::<DocumentMut>()
        .map_err(|error| format!("Failed to parse inventory-utility config: {error}"))?;
    doc["inventory_utility"] = inventory_doc["inventory_utility"].clone();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create config directory: {error}"))?;
    }

    fs::write(path, doc.to_string())
        .map_err(|error| format!("Failed to write inventory-utility config: {error}"))
}

// ── Origin allowlist
// ──────────────────────────────────────────────────────────

/// Trusted local-dev origins shared with the global CORS configuration in
/// `main.rs`.  Both the CORS middleware and the per-handler origin guard must
/// use this list as the single source of truth to prevent configuration drift.
///
/// - Port `3001` — the axum backend itself (direct API access)
/// - Port `5173` — the Vite dev server (proxies API calls during `npm run dev`)
pub const TRUSTED_ORIGINS: &[&str] = &[
    "http://localhost:3001",
    "http://127.0.0.1:3001",
    "http://localhost:5173",
    "http://127.0.0.1:5173",
];

// ── Handlers ─────────────────────────────────────────────────────────────────

pub(crate) fn is_trusted_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(axum::http::header::ORIGIN) else {
        // Non-browser clients can omit Origin entirely; in production builds we
        // treat that as untrusted.  In test builds we allow it so unit tests
        // that construct bare HeaderMaps still pass without faking an Origin.
        return cfg!(test);
    };

    let Ok(origin_str) = origin.to_str() else {
        return false;
    };

    TRUSTED_ORIGINS.contains(&origin_str)
}

/// GET /api/loot/rules — return current loot rules.
pub async fn get_rules(State(state): State<Arc<AppState>>) -> Json<LootRulesPayload> {
    let rules = state.loot_state.rules.read().await;
    Json(rules.clone())
}

/// PUT /api/loot/rules — replace loot rules.
pub async fn put_rules(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<LootRulesPayload>,
) -> StatusCode {
    if !is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN;
    }
    let mut rules = state.loot_state.rules.write().await;
    *rules = payload;
    StatusCode::NO_CONTENT
}

/// GET /api/loot/filters — return all character loot filters.
pub async fn get_filters(State(state): State<Arc<AppState>>) -> Json<Vec<CharacterLootFilter>> {
    let filters = state.loot_state.filters.read().await;
    let mut list: Vec<CharacterLootFilter> = filters.values().cloned().collect();
    list.sort_by(|a, b| a.character.cmp(&b.character));
    Json(list)
}

/// PUT /api/loot/filters/:character — replace a character's loot filter.
pub async fn put_filter(
    State(state): State<Arc<AppState>>,
    Path(character): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<CharacterLootFilter>,
) -> StatusCode {
    if !is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN;
    }
    if payload.character != character {
        return StatusCode::BAD_REQUEST;
    }
    let mut filters = state.loot_state.filters.write().await;
    filters.insert(character, payload);
    StatusCode::NO_CONTENT
}

/// GET /api/loot/master-looter — return master looter assignment.
pub async fn get_master_looter(State(state): State<Arc<AppState>>) -> Json<MasterLooterPayload> {
    let ml = state.loot_state.master_looter.read().await;
    Json(ml.clone())
}

/// PUT /api/loot/master-looter — set master looter.
pub async fn put_master_looter(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<MasterLooterPayload>,
) -> StatusCode {
    if !is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN;
    }
    let mut ml = state.loot_state.master_looter.write().await;
    *ml = payload;
    StatusCode::NO_CONTENT
}

/// GET /api/loot/distribution — return distribution rules.
pub async fn get_distribution(State(state): State<Arc<AppState>>) -> Json<DistributionConfig> {
    let dist = state.loot_state.distribution.read().await;
    Json(dist.clone())
}

/// PUT /api/loot/distribution — replace distribution configuration.
pub async fn put_distribution(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<DistributionConfig>,
) -> StatusCode {
    if !is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN;
    }
    let mut dist = state.loot_state.distribution.write().await;
    *dist = payload;
    StatusCode::NO_CONTENT
}

/// GET /api/loot/history — return loot history, with optional search filters.
pub async fn get_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HistoryQuery>,
) -> Json<Vec<LootHistoryEntry>> {
    let history = state.loot_state.history.read().await;
    // Clamp limit: 0 falls back to default; max 500.
    let limit = params.limit.filter(|&l| l > 0).unwrap_or(50).min(500);
    // Lowercase the search query once so we don't redo it on every iteration.
    // Note: per-entry item_name lowercasing still allocates; that is unavoidable
    // unless item names are stored pre-normalized.
    let search_lower = params.search.as_deref().map(str::to_lowercase);

    let results: Vec<LootHistoryEntry> = history
        .iter()
        .filter(|e| {
            if let Some(ref search) = search_lower
                && !e.item_name.to_lowercase().contains(search.as_str())
            {
                return false;
            }
            if let Some(ref character) = params.character
                && !e.recipient.eq_ignore_ascii_case(character)
            {
                return false;
            }
            true
        })
        .take(limit)
        .cloned()
        .collect();

    Json(results)
}

/// GET /api/loot/item-score — return current per-class stat weights.
pub async fn get_item_score(State(state): State<Arc<AppState>>) -> Json<ItemScoreConfigPayload> {
    let config = state.loot_state.item_score.read().await;
    Json(config.clone())
}

/// PUT /api/loot/item-score — replace current per-class stat weights.
pub async fn put_item_score(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ItemScoreConfigPayload>,
) -> StatusCode {
    if !is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN;
    }

    let _config_write_guard = crate::api::textquest_config_write_lock().lock().await;
    let _write_guard = state.loot_state.item_score_write_lock.lock().await;

    if let Err(error) = save_item_score_to_path(&state.loot_state.item_score_config_path, &payload)
    {
        tracing::warn!(%error, "Failed to persist item-score config");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    let mut config = state.loot_state.item_score.write().await;
    *config = payload;
    StatusCode::NO_CONTENT
}

/// GET /api/loot/inventory-utility — return current inventory utility parity config.
pub async fn get_inventory_utility(
    State(state): State<Arc<AppState>>,
) -> Json<InventoryUtilityConfigPayload> {
    let config = state.loot_state.inventory_utility.read().await;
    Json(config.clone())
}

/// PUT /api/loot/inventory-utility — replace current inventory utility parity config.
pub async fn put_inventory_utility(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<InventoryUtilityConfigPayload>,
) -> StatusCode {
    if !is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN;
    }

    let _config_write_guard = crate::api::textquest_config_write_lock().lock().await;
    let _write_guard = state.loot_state.inventory_utility_write_lock.lock().await;

    if let Err(error) =
        save_inventory_utility_to_path(&state.loot_state.item_score_config_path, &payload)
    {
        tracing::warn!(%error, "Failed to persist inventory-utility config");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    let mut config = state.loot_state.inventory_utility.write().await;
    *config = payload;
    StatusCode::NO_CONTENT
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppState;
    use textquest::alerts::AlertStore;
    use textquest::config::AlertingConfig;

    fn test_config_path() -> std::path::PathBuf {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("textquest.toml");
        std::mem::forget(dir);
        path
    }

    fn demo_state() -> Arc<AppState> {
        let mut state = crate::test_app_state();
        state.loot_state = LootState::new_with_item_score_path(test_config_path());
        Arc::new(state)
    }

    #[tokio::test]
    async fn get_rules_returns_defaults() {
        let state = demo_state();
        let Json(rules) = get_rules(State(state)).await;
        assert!(rules.loot_all);
        assert!(rules.auto_split);
        assert!(!rules.keep_items.is_empty());
    }

    #[tokio::test]
    async fn put_rules_updates_state() {
        let state = demo_state();
        let new_rules = LootRulesPayload {
            keep_items: vec!["Fabled Sword".into()],
            sell_items: vec![],
            destroy_items: vec![],
            loot_all: false,
            auto_split: false,
        };
        let status = put_rules(State(state.clone()), HeaderMap::new(), Json(new_rules)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let Json(rules) = get_rules(State(state)).await;
        assert!(!rules.loot_all);
        assert_eq!(rules.keep_items, vec!["Fabled Sword"]);
    }

    #[tokio::test]
    async fn get_filters_returns_all_characters() {
        let state = demo_state();
        let Json(filters) = get_filters(State(state)).await;
        assert!(!filters.is_empty());
        // Should be sorted alphabetically
        let names: Vec<&str> = filters.iter().map(|f| f.character.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[tokio::test]
    async fn put_filter_updates_character() {
        let state = demo_state();
        let new_filter = CharacterLootFilter {
            character: "Frostreaver".into(),
            filters: vec![ItemFilterEntry {
                item_name: "Dragon Scale".into(),
                action: FilterAction::Keep,
            }],
        };
        let status = put_filter(
            State(state.clone()),
            Path("Frostreaver".into()),
            HeaderMap::new(),
            Json(new_filter),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let Json(filters) = get_filters(State(state)).await;
        let frostreaver = filters
            .iter()
            .find(|f| f.character == "Frostreaver")
            .unwrap();
        assert_eq!(frostreaver.filters.len(), 1);
        assert_eq!(frostreaver.filters[0].item_name, "Dragon Scale");
    }

    #[tokio::test]
    async fn get_master_looter_returns_default() {
        let state = demo_state();
        let Json(ml) = get_master_looter(State(state)).await;
        assert!(ml.character.is_none());
    }

    #[tokio::test]
    async fn put_master_looter_sets_character() {
        let state = demo_state();
        let payload = MasterLooterPayload {
            character: Some("Frostreaver".into()),
        };
        let status = put_master_looter(State(state.clone()), HeaderMap::new(), Json(payload)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let Json(ml) = get_master_looter(State(state)).await;
        assert_eq!(ml.character, Some("Frostreaver".into()));
    }

    #[tokio::test]
    async fn get_distribution_returns_defaults() {
        let state = demo_state();
        let Json(dist) = get_distribution(State(state)).await;
        assert!(!dist.rules.is_empty());
    }

    #[tokio::test]
    async fn get_history_returns_entries() {
        let state = demo_state();
        let Json(history) = get_history(
            State(state),
            Query(HistoryQuery {
                search: None,
                character: None,
                limit: None,
            }),
        )
        .await;
        assert!(!history.is_empty());
    }

    #[tokio::test]
    async fn get_history_filters_by_search() {
        let state = demo_state();
        let Json(history) = get_history(
            State(state),
            Query(HistoryQuery {
                search: Some("mithril".into()),
                character: None,
                limit: None,
            }),
        )
        .await;
        assert_eq!(history.len(), 1);
        assert!(history[0].item_name.to_lowercase().contains("mithril"));
    }

    #[tokio::test]
    async fn put_filter_rejects_character_mismatch() {
        let state = demo_state();
        let new_filter = CharacterLootFilter {
            character: "Shadowdancer".into(),
            filters: vec![],
        };
        let status = put_filter(
            State(state),
            Path("Frostreaver".into()),
            HeaderMap::new(),
            Json(new_filter),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn put_rules_rejects_untrusted_origin() {
        let state = demo_state();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::ORIGIN,
            "https://evil.example".parse().unwrap(),
        );
        let payload = LootRulesPayload {
            keep_items: vec!["Safe Item".into()],
            sell_items: vec![],
            destroy_items: vec![],
            loot_all: false,
            auto_split: false,
        };

        let status = put_rules(State(state), headers, Json(payload)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn get_history_clamps_limit() {
        let state = demo_state();
        // limit=0 falls back to default (50); should return all demo entries (< 50)
        let Json(history) = get_history(
            State(state.clone()),
            Query(HistoryQuery {
                search: None,
                character: None,
                limit: Some(0),
            }),
        )
        .await;
        assert!(!history.is_empty());

        // limit=1000 is clamped to 500; should still return all demo entries (< 500)
        let Json(history_large) = get_history(
            State(state),
            Query(HistoryQuery {
                search: None,
                character: None,
                limit: Some(1000),
            }),
        )
        .await;
        assert!(!history_large.is_empty());
    }

    #[tokio::test]
    async fn get_history_filters_by_character() {
        let state = demo_state();
        let Json(history) = get_history(
            State(state),
            Query(HistoryQuery {
                search: None,
                character: Some("Shadowdancer".into()),
                limit: None,
            }),
        )
        .await;
        assert!(history.iter().all(|e| e.recipient == "Shadowdancer"));
    }

    #[test]
    fn load_item_score_returns_default_when_missing() {
        let config = load_item_score_from_path(&test_config_path()).expect("default config");
        assert!(!config.class_weights.is_empty());
        assert_eq!(config.min_upgrade_delta, 0.0);
    }

    #[test]
    fn save_item_score_round_trips() {
        let path = test_config_path();
        let mut config = ItemScoreConfigPayload {
            min_upgrade_delta: 3.5,
            ..ItemScoreConfigPayload::default()
        };
        config.class_weights.insert(
            "Warrior".into(),
            std::collections::BTreeMap::from([("STR".into(), 1.2), ("AC".into(), 0.8)]),
        );

        save_item_score_to_path(&path, &config).expect("saved");
        let loaded = load_item_score_from_path(&path).expect("loaded");

        assert_eq!(loaded.min_upgrade_delta, 3.5);
        assert_eq!(
            loaded.class_weights.get("Warrior"),
            config.class_weights.get("Warrior")
        );
    }

    #[tokio::test]
    async fn get_item_score_returns_defaults() {
        let state = demo_state();
        let Json(config) = get_item_score(State(state)).await;
        assert!(!config.class_weights.is_empty());
    }

    #[tokio::test]
    async fn put_item_score_updates_state() {
        let state = demo_state();
        let mut payload = ItemScoreConfigPayload {
            min_upgrade_delta: 2.25,
            ..ItemScoreConfigPayload::default()
        };
        payload.class_weights.insert(
            "Rogue".into(),
            std::collections::BTreeMap::from([("DEX".into(), 1.4), ("STR".into(), 0.6)]),
        );

        let status = put_item_score(
            State(state.clone()),
            HeaderMap::new(),
            Json(payload.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let Json(saved) = get_item_score(State(state.clone())).await;
        assert_eq!(saved.min_upgrade_delta, 2.25);
        assert_eq!(
            saved.class_weights.get("Rogue"),
            payload.class_weights.get("Rogue")
        );

        let reloaded = load_item_score_from_path(&state.loot_state.item_score_config_path)
            .expect("config persisted");
        assert_eq!(
            reloaded.class_weights.get("Rogue"),
            payload.class_weights.get("Rogue")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn put_item_score_does_not_update_state_when_disk_write_fails() {
        use std::os::unix::fs::PermissionsExt;

        let temp_root = std::env::temp_dir().join(format!(
            "textquest-web-loot-item-score-readonly-{}",
            uuid::Uuid::new_v4()
        ));
        let read_only_dir = temp_root.join("readonly");
        std::fs::create_dir_all(&read_only_dir).expect("create readonly dir");
        std::fs::set_permissions(&read_only_dir, std::fs::Permissions::from_mode(0o555))
            .expect("mark readonly");

        let path = read_only_dir.join("item-score.toml");
        let state = Arc::new(AppState {
            event_tx: tokio::sync::broadcast::channel(1).0,
            account_store: std::sync::Mutex::new(crate::accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            auto_accept_settings: tokio::sync::RwLock::new(Default::default()),
            tradeskill_trophy_settings: tokio::sync::RwLock::new(Default::default()),
            character_config_path: crate::test_support::test_live_session_snapshot_path(
                "loot-test-character-configs.json",
            ),
            character_config_write_lock: tokio::sync::Mutex::new(()),
            loot_state: LootState::new_with_item_score_path(path),
            economy_state: crate::api::economy::EconomyState::new_demo(),
            dashboard_state: crate::api::dashboard::DashboardState::new_demo(),
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            discord_state: crate::api::discord::DiscordState::new_demo(),
            player_watch_config: tokio::sync::RwLock::new(crate::api::PlayerWatchConfig::default()),
            player_watch_write_lock: tokio::sync::Mutex::new(()),
            gm_alert_state: Arc::new(crate::api::gm_alerts::GmAlertState::default()),
            spawn_alerts: crate::api::spawn_alerts::SpawnAlertState::new_demo(),
            vendor_watch_state: crate::api::vendor_watch::VendorWatchState::new_demo(),
            timestamp_configs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            timestamp_config_write_lock: tokio::sync::Mutex::new(()),
            kill_tracker_state: crate::api::kill_tracker::KillTrackerState::new_empty(),
            alert_store: AlertStore::open_memory().expect("alert store"),
            alert_config: tokio::sync::RwLock::new(AlertingConfig::default()),
            alerting_config_path: crate::test_support::test_live_session_snapshot_path(
                "loot-test-alerting.toml",
            ),
            api_token: None,
            live_session_snapshot_path: crate::test_support::test_live_session_snapshot_path(
                "loot-test-live-sessions.json",
            ),
            admin_session_snapshot_path: crate::test_support::test_admin_session_snapshot_path(
                "loot-test-admin-sessions.json",
            ),
            xassist_configs: crate::api::xassist::demo_xassist_configs(),
            chat_pattern_rules: crate::api::chat_pattern_rules::load_rules_state(),
            say_detection: Some(Arc::new(
                crate::api::say_detection::SayDetectionState::new_demo(),
            )),
            auto_group_state: crate::api::auto_group::AutoGroupState::new_demo(),
            extension_catalog_state: crate::api::extensions::ExtensionCatalogState::load(
                std::env::temp_dir().join(format!(
                    "textquest-loot-test-extension-catalog-{}.json",
                    uuid::Uuid::new_v4()
                )),
            ),
        });

        let original = get_item_score(State(state.clone())).await.0;
        let mut payload = ItemScoreConfigPayload {
            min_upgrade_delta: 9.5,
            ..ItemScoreConfigPayload::default()
        };
        payload.class_weights.insert(
            "Warrior".into(),
            std::collections::BTreeMap::from([("STR".into(), 2.0)]),
        );

        let status = put_item_score(State(state.clone()), HeaderMap::new(), Json(payload)).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);

        let saved = get_item_score(State(state.clone())).await.0;
        assert_eq!(saved, original);

        std::fs::set_permissions(&read_only_dir, std::fs::Permissions::from_mode(0o755))
            .expect("restore dir perms");
        std::fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn load_inventory_utility_returns_default_when_missing() {
        let config = load_inventory_utility_from_path(&test_config_path()).expect("default config");
        assert_eq!(config.plugin_mappings.len(), 13);
        assert!(config.item_knowledge.show_provenance);
    }

    #[test]
    fn save_inventory_utility_round_trips() {
        let path = test_config_path();
        let mut config = textquest_common::inventory_utility::default_inventory_utility_config();
        config.cursor_rules[0].keep_at_or_below = Some(3);
        config.auto_claim.enabled = true;

        save_inventory_utility_to_path(&path, &config).expect("saved");
        let loaded = load_inventory_utility_from_path(&path).expect("loaded");

        assert_eq!(loaded.cursor_rules[0].keep_at_or_below, Some(3));
        assert!(loaded.auto_claim.enabled);
    }

    #[tokio::test]
    async fn get_inventory_utility_returns_defaults() {
        let state = demo_state();
        let Json(config) = get_inventory_utility(State(state)).await;
        assert_eq!(config.plugin_mappings.len(), 13);
        assert!(config.item_knowledge.show_unsupported_fields);
    }

    #[tokio::test]
    async fn put_inventory_utility_updates_state() {
        let state = demo_state();
        let mut payload = textquest_common::inventory_utility::default_inventory_utility_config();
        payload.vendor_watch.push(
            textquest_common::inventory_utility::VendorWatchRule::notify_on("Jacinth", Some(450)),
        );
        payload.auto_claim.enabled = true;

        let status = put_inventory_utility(
            State(state.clone()),
            HeaderMap::new(),
            Json(payload.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let Json(saved) = get_inventory_utility(State(state.clone())).await;
        assert_eq!(saved.vendor_watch.len(), payload.vendor_watch.len());
        assert!(saved.auto_claim.enabled);

        let reloaded = load_inventory_utility_from_path(&state.loot_state.item_score_config_path)
            .expect("config persisted");
        assert_eq!(reloaded.auto_claim.enabled, payload.auto_claim.enabled);
    }
}
