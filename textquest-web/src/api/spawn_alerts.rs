//! REST API handlers for Rare Spawn Alert System.
#![allow(dead_code)]

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAlertEntry {
    pub id: u64,
    pub spawn_name: String,
    pub zone: String,
    pub is_up: bool,
    pub timestamp: String,
    pub time_since_last_pop_ms: Option<u64>,
    pub match_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchPattern {
    pub pattern: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAlertConfig {
    pub watch_named_enabled: bool,
    pub watch_patterns: Vec<WatchPattern>,
    pub broadcast_to_web: bool,
    pub broadcast_to_clients: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnAlertStats {
    pub total_alerts: u64,
    pub spawns_up: u64,
    pub spawns_down: u64,
}

pub struct SpawnAlertState {
    pub entries: RwLock<Vec<SpawnAlertEntry>>,
    pub watch_patterns: RwLock<Vec<WatchPattern>>,
    pub watch_named_enabled: RwLock<bool>,
    pub broadcast_to_web: RwLock<bool>,
    pub broadcast_to_clients: RwLock<bool>,
    pub next_id: std::sync::atomic::AtomicU64,
}

impl SpawnAlertState {
    pub fn new_demo() -> Arc<Self> {
        let demo_patterns = vec![
            WatchPattern {
                pattern: "*Maestro*".into(),
                enabled: true,
            },
            WatchPattern {
                pattern: "Emperor Crush".into(),
                enabled: true,
            },
            WatchPattern {
                pattern: "*Rancor*".into(),
                enabled: false,
            },
        ];

        let demo_entries = vec![
            SpawnAlertEntry {
                id: 1,
                spawn_name: "Maestro of Rancor".into(),
                zone: "The Plane of Time".into(),
                is_up: true,
                timestamp: "2026-04-15T10:30:00Z".into(),
                time_since_last_pop_ms: Some(3600000),
                match_source: "*Maestro*".into(),
            },
            SpawnAlertEntry {
                id: 2,
                spawn_name: "Maestro of Rancor".into(),
                zone: "The Plane of Time".into(),
                is_up: false,
                timestamp: "2026-04-15T09:30:00Z".into(),
                time_since_last_pop_ms: None,
                match_source: "*Maestro*".into(),
            },
            SpawnAlertEntry {
                id: 3,
                spawn_name: "Emperor Crush".into(),
                zone: "Durius".into(),
                is_up: true,
                timestamp: "2026-04-15T08:00:00Z".into(),
                time_since_last_pop_ms: Some(7200000),
                match_source: "Named".into(),
            },
        ];

        Arc::new(Self {
            entries: RwLock::new(demo_entries),
            watch_patterns: RwLock::new(demo_patterns),
            watch_named_enabled: RwLock::new(true),
            broadcast_to_web: RwLock::new(true),
            broadcast_to_clients: RwLock::new(false),
            next_id: std::sync::atomic::AtomicU64::new(4),
        })
    }

    pub async fn add_entry(&self, entry: SpawnAlertEntry) -> u64 {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut entries = self.entries.write().await;
        entries.push(SpawnAlertEntry { id, ..entry });
        id
    }

    pub async fn get_stats(&self) -> SpawnAlertStats {
        let entries = self.entries.read().await;
        let total_alerts = entries.len() as u64;
        let spawns_up = entries.iter().filter(|e| e.is_up).count() as u64;
        let spawns_down = entries.iter().filter(|e| !e.is_up).count() as u64;
        SpawnAlertStats {
            total_alerts,
            spawns_up,
            spawns_down,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AlertQuery {
    #[serde(default)]
    pub offset: usize,
    pub limit: Option<usize>,
    pub zone: Option<String>,
    pub spawn_name: Option<String>,
}

#[derive(Serialize)]
pub struct AlertPage {
    total: usize,
    offset: usize,
    limit: usize,
    entries: Vec<SpawnAlertEntry>,
}

pub async fn list_alerts(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AlertQuery>,
) -> impl IntoResponse {
    let entries = state.spawn_alerts.entries.read().await;

    let mut filtered: Vec<&SpawnAlertEntry> = entries
        .iter()
        .filter(|e| {
            params
                .zone
                .as_deref()
                .is_none_or(|z| e.zone.to_lowercase().contains(&z.to_lowercase()))
        })
        .filter(|e| {
            params
                .spawn_name
                .as_deref()
                .is_none_or(|n| e.spawn_name.to_lowercase().contains(&n.to_lowercase()))
        })
        .collect();

    filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let total = filtered.len();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset;

    let page: Vec<SpawnAlertEntry> = filtered
        .into_iter()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect();

    (
        StatusCode::OK,
        Json(AlertPage {
            total,
            offset,
            limit,
            entries: page,
        }),
    )
}

pub async fn get_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let stats = state.spawn_alerts.get_stats().await;
    Json(stats)
}

pub async fn get_watch_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let patterns = state.spawn_alerts.watch_patterns.read().await;
    let watch_named = *state.spawn_alerts.watch_named_enabled.read().await;
    Json(serde_json::json!({
        "watch_named_enabled": watch_named,
        "patterns": *patterns
    }))
}

pub async fn put_watch_pattern(
    State(state): State<Arc<AppState>>,
    Json(pattern): Json<WatchPattern>,
) -> impl IntoResponse {
    let mut patterns = state.spawn_alerts.watch_patterns.write().await;
    if let Some(existing) = patterns.iter_mut().find(|p| p.pattern == pattern.pattern) {
        existing.enabled = pattern.enabled;
    } else {
        patterns.push(pattern);
    }
    StatusCode::NO_CONTENT
}

pub async fn delete_watch_pattern(
    State(state): State<Arc<AppState>>,
    Path(pattern): Path<String>,
) -> impl IntoResponse {
    let mut patterns = state.spawn_alerts.watch_patterns.write().await;
    patterns.retain(|p| p.pattern != pattern);
    StatusCode::NO_CONTENT
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let watch_named = *state.spawn_alerts.watch_named_enabled.read().await;
    let patterns = state.spawn_alerts.watch_patterns.read().await;
    let broadcast_to_web = *state.spawn_alerts.broadcast_to_web.read().await;
    let broadcast_to_clients = *state.spawn_alerts.broadcast_to_clients.read().await;

    Json(SpawnAlertConfig {
        watch_named_enabled: watch_named,
        watch_patterns: patterns.clone(),
        broadcast_to_web,
        broadcast_to_clients,
    })
}

pub async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(config): Json<SpawnAlertConfig>,
) -> impl IntoResponse {
    {
        let mut watch_named = state.spawn_alerts.watch_named_enabled.write().await;
        *watch_named = config.watch_named_enabled;
    }
    {
        let mut patterns = state.spawn_alerts.watch_patterns.write().await;
        *patterns = config.watch_patterns;
    }
    {
        let mut broadcast_to_web = state.spawn_alerts.broadcast_to_web.write().await;
        *broadcast_to_web = config.broadcast_to_web;
    }
    {
        let mut broadcast_to_clients = state.spawn_alerts.broadcast_to_clients.write().await;
        *broadcast_to_clients = config.broadcast_to_clients;
    }

    tracing::debug!("Spawn alert config updated");
    StatusCode::NO_CONTENT
}

pub async fn clear_alerts(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut entries = state.spawn_alerts.entries.write().await;
    entries.clear();
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_pattern_serialization() {
        let pattern = WatchPattern {
            pattern: "*test*".into(),
            enabled: true,
        };
        let json = serde_json::to_string(&pattern).unwrap();
        assert!(json.contains("\"pattern\":\"*test*\""));
        assert!(json.contains("\"enabled\":true"));
    }

    #[test]
    fn spawn_alert_entry_serialization() {
        let entry = SpawnAlertEntry {
            id: 1,
            spawn_name: "Test Mob".into(),
            zone: "Test Zone".into(),
            is_up: true,
            timestamp: "2026-04-15T10:00:00Z".into(),
            time_since_last_pop_ms: Some(3600000),
            match_source: "Named".into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"spawn_name\":\"Test Mob\""));
        assert!(json.contains("\"is_up\":true"));
    }
}
