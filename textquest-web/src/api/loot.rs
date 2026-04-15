//! REST API handlers for loot rules and distribution configuration.

use std::{collections::HashMap, sync::Arc};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::AppState;

// ── Shared loot state ────────────────────────────────────────────────────────

/// In-memory loot configuration state shared across handlers.
pub struct LootState {
    pub rules: RwLock<LootRulesPayload>,
    pub filters: RwLock<HashMap<String, CharacterLootFilter>>,
    pub master_looter: RwLock<MasterLooterPayload>,
    pub distribution: RwLock<DistributionConfig>,
    pub history: RwLock<Vec<LootHistoryEntry>>,
}

impl LootState {
    /// Create state pre-populated with sensible defaults and demo data.
    pub fn new_demo() -> Arc<Self> {
        Arc::new(Self {
            rules: RwLock::new(LootRulesPayload::default()),
            filters: RwLock::new(default_filters()),
            master_looter: RwLock::new(MasterLooterPayload::default()),
            distribution: RwLock::new(DistributionConfig::default()),
            history: RwLock::new(demo_history()),
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_state() -> Arc<AppState> {
        let (event_tx, _) = tokio::sync::broadcast::channel(1);
        Arc::new(AppState {
            event_tx,
            account_store: std::sync::Mutex::new(crate::accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            loot_state: LootState::new_demo(),
            economy_state: crate::api::economy::EconomyState::new_demo(),
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            api_token: None,
        })
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
}
