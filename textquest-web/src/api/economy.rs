//! REST API handlers for economy state and operator controls.

#![allow(dead_code)] // Demo responses and placeholder handlers are exercised by tests, not the live router.
#![allow(
    clippy::assigning_clones,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    clippy::items_after_statements,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::needless_pass_by_value,
    clippy::no_effect_underscore_binding,
    clippy::option_if_let_else,
    clippy::return_self_not_must_use,
    clippy::significant_drop_in_scrutinee,
    clippy::significant_drop_tightening,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_long_first_doc_paragraph,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unreadable_literal,
    clippy::unused_self,
    clippy::while_float,
    clippy::used_underscore_binding,
    clippy::trivially_copy_pass_by_ref,
    clippy::ref_option,
    clippy::or_fun_call,
    clippy::needless_pass_by_ref_mut,
    clippy::match_wildcard_for_single_variants,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::branches_sharing_code,
    clippy::wildcard_imports,
    clippy::unused_async,
    clippy::unnecessary_debug_formatting,
    clippy::single_option_map,
    clippy::needless_collect,
    clippy::map_unwrap_or,
    clippy::many_single_char_names,
    clippy::missing_const_for_fn,
    clippy::cast_ptr_alignment,
    clippy::default_trait_access,
    clippy::format_collect,
    clippy::format_push_string,
    clippy::implicit_hasher,
    clippy::iter_on_single_items,
    clippy::redundant_field_names
)]
use std::sync::Arc;

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::AppState;

// ── Shared economy state
// ──────────────────────────────────────────────────────

/// In-memory economy cycle state shared across handlers.
pub struct EconomyState {
    pub is_paused: RwLock<bool>,
    pub active_cycles: RwLock<Vec<String>>,
    pub ledger: RwLock<EconomyLedgerResponse>,
    pub queues: RwLock<EconomyQueuesResponse>,
}

impl EconomyState {
    /// Create state pre-populated with sensible demo defaults.
    pub fn new_demo() -> Arc<Self> {
        Arc::new(Self {
            is_paused: RwLock::new(false),
            active_cycles: RwLock::new(vec!["loot".into(), "vendor".into(), "banking".into()]),
            ledger: RwLock::new(EconomyLedgerResponse {
                plat_per_hour: 1_450.5,
                items_distributed: 312,
                vendor_sales: 87,
            }),
            queues: RwLock::new(EconomyQueuesResponse {
                loot_queue_len: 4,
                vendor_backlog_len: 11,
            }),
        })
    }
}

// ── Response types
// ────────────────────────────────────────────────────────────

/// Response for GET /api/economy/status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomyStatusResponse {
    /// Names of currently active economy cycles (e.g. "loot", "vendor",
    /// "banking").
    pub active_cycles: Vec<String>,
    /// Whether the economy engine is globally paused.
    pub is_paused: bool,
}

/// Response for GET /api/economy/ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomyLedgerResponse {
    /// Rolling plat-per-hour rate based on recent activity.
    pub plat_per_hour: f64,
    /// Total items distributed to characters this session.
    pub items_distributed: u64,
    /// Total items sold to vendors this session.
    pub vendor_sales: u64,
}

/// Response for GET /api/economy/queues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomyQueuesResponse {
    /// Number of items currently queued for looting.
    pub loot_queue_len: u32,
    /// Number of items in the vendor sell backlog.
    pub vendor_backlog_len: u32,
}

// ── Handlers
// ──────────────────────────────────────────────────────────────────

/// GET /api/economy/status — current active cycles and pause state.
pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<EconomyStatusResponse> {
    let is_paused = *state.economy_state.is_paused.read().await;
    let active_cycles = state.economy_state.active_cycles.read().await.clone();
    Json(EconomyStatusResponse {
        active_cycles,
        is_paused,
    })
}

/// GET /api/economy/ledger — summary stats (plat/hour, items distributed,
/// vendor sales).
pub async fn get_ledger(State(state): State<Arc<AppState>>) -> Json<EconomyLedgerResponse> {
    let ledger = state.economy_state.ledger.read().await.clone();
    Json(ledger)
}

/// GET /api/economy/queues — loot queue and vendor backlog sizes.
pub async fn get_queues(State(state): State<Arc<AppState>>) -> Json<EconomyQueuesResponse> {
    let queues = state.economy_state.queues.read().await.clone();
    Json(queues)
}

/// POST /api/economy/pause — pause all economy cycles.
pub async fn pause_economy(State(state): State<Arc<AppState>>, headers: HeaderMap) -> StatusCode {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN;
    }
    let mut paused = state.economy_state.is_paused.write().await;
    *paused = true;
    StatusCode::NO_CONTENT
}

/// POST /api/economy/resume — resume all economy cycles.
pub async fn resume_economy(State(state): State<Arc<AppState>>, headers: HeaderMap) -> StatusCode {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN;
    }
    let mut paused = state.economy_state.is_paused.write().await;
    *paused = false;
    StatusCode::NO_CONTENT
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppState;

    fn demo_state() -> Arc<AppState> {
        let mut state = crate::test_app_state();
        state.economy_state = EconomyState::new_demo();
        Arc::new(state)
    }

    #[tokio::test]
    async fn get_status_returns_demo_cycles() {
        let state = demo_state();
        let Json(status) = get_status(State(state)).await;
        assert!(!status.active_cycles.is_empty());
        assert!(status.active_cycles.contains(&"loot".to_string()));
        assert!(status.active_cycles.contains(&"vendor".to_string()));
        assert!(status.active_cycles.contains(&"banking".to_string()));
        assert!(!status.is_paused);
    }

    #[tokio::test]
    async fn get_ledger_returns_nonzero_stats() {
        let state = demo_state();
        let Json(ledger) = get_ledger(State(state)).await;
        assert!(ledger.plat_per_hour > 0.0);
        assert!(ledger.items_distributed > 0);
        assert!(ledger.vendor_sales > 0);
    }

    #[tokio::test]
    async fn get_queues_returns_queue_sizes() {
        let state = demo_state();
        let Json(queues) = get_queues(State(state)).await;
        // Demo data has non-zero queue sizes
        assert!(queues.loot_queue_len > 0 || queues.vendor_backlog_len > 0);
    }

    #[tokio::test]
    async fn pause_sets_is_paused_true() {
        let state = demo_state();
        let status = pause_economy(State(state.clone()), HeaderMap::new()).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let Json(economy_status) = get_status(State(state)).await;
        assert!(economy_status.is_paused);
    }

    #[tokio::test]
    async fn resume_clears_is_paused() {
        let state = demo_state();
        // First pause
        pause_economy(State(state.clone()), HeaderMap::new()).await;
        // Then resume
        let status = resume_economy(State(state.clone()), HeaderMap::new()).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let Json(economy_status) = get_status(State(state)).await;
        assert!(!economy_status.is_paused);
    }

    #[tokio::test]
    async fn pause_rejects_untrusted_origin() {
        let state = demo_state();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::ORIGIN,
            "https://evil.example".parse().unwrap(),
        );

        let status = pause_economy(State(state.clone()), headers).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let Json(economy_status) = get_status(State(state)).await;
        assert!(!economy_status.is_paused);
    }
}
