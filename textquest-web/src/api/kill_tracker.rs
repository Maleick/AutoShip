use std::{collections::HashMap, sync::Arc};

use axum::{
    routing::get,
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillTrackerSettings {
    pub enabled: bool,
    pub auto_report_interval_minutes: u32,
    pub auto_report_channel: String,
    pub auto_report_include_mobs: bool,
    pub auto_report_include_kph: bool,
    pub track_per_character: bool,
    pub max_session_history: usize,
}

impl Default for KillTrackerSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_report_interval_minutes: 10,
            auto_report_channel: "group".to_string(),
            auto_report_include_mobs: true,
            auto_report_include_kph: true,
            track_per_character: true,
            max_session_history: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillRecord {
    pub mob_name: String,
    pub mob_level: u8,
    pub zone: String,
    pub timestamp: String,
    pub kill_time_ms: u64,
    pub total_damage: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MobStats {
    pub mob_name: String,
    pub kill_count: u32,
    pub best_time_ms: u64,
    pub avg_time_ms: u64,
    pub avg_dps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EfficiencyScore {
    pub kills_per_hour: f64,
    pub avg_kill_time_secs: f64,
    pub death_ratio: f64,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub character: String,
    pub session_start: String,
    pub total_kills: u32,
    pub total_deaths: u32,
    pub kills_per_hour: f64,
    pub efficiency: EfficiencyScore,
    pub mob_stats: Vec<MobStats>,
    pub top_mobs: Vec<(String, u32)>,
    pub zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterHistory {
    pub character: String,
    pub sessions: Vec<SessionStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillTrackerDashboard {
    pub current_session: Option<SessionStats>,
    pub character_history: Vec<CharacterHistory>,
    pub settings: KillTrackerSettings,
}

pub struct KillTrackerState {
    pub settings: RwLock<KillTrackerSettings>,
    pub sessions: RwLock<HashMap<String, Vec<SessionStats>>>,
}

impl KillTrackerState {
    pub fn new_demo() -> Arc<Self> {
        Arc::new(Self {
            settings: RwLock::new(KillTrackerSettings::default()),
            sessions: RwLock::new(HashMap::new()),
        })
    }

    pub async fn update_settings(&self, settings: KillTrackerSettings) {
        let mut current = self.settings.write().await;
        *current = settings;
    }

    pub async fn add_session(&self, character: String, session: SessionStats) {
        let mut sessions = self.sessions.write().await;
        sessions
            .entry(character)
            .or_insert_with(Vec::new)
            .push(session);
    }
}

pub fn router() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/settings", get(get_settings).put(put_settings))
        .route("/sessions", get(get_sessions))
        .route("/sessions/{character}", get(get_character_sessions))
        .route("/history", get(get_history))
}

async fn get_settings(State(state): State<Arc<AppState>>) -> Json<KillTrackerSettings> {
    let settings = state.kill_tracker_state.settings.read().await;
    Json(settings.clone())
}

async fn put_settings(
    State(state): State<Arc<AppState>>,
    Json(settings): Json<KillTrackerSettings>,
) -> impl IntoResponse {
    state.kill_tracker_state.update_settings(settings.clone()).await;
    (StatusCode::OK, Json(settings)).into_response()
}

async fn get_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let sessions = state.kill_tracker_state.sessions.read().await;
    let mut result: Vec<SessionStats> = Vec::new();
    for (_character, char_sessions) in sessions.iter() {
        if let Some(latest) = char_sessions.last() {
            result.push(latest.clone());
        }
    }
    (StatusCode::OK, Json(result)).into_response()
}

async fn get_character_sessions(
    State(state): State<Arc<AppState>>,
    Path(character): Path<String>,
) -> impl IntoResponse {
    let sessions = state.kill_tracker_state.sessions.read().await;
    match sessions.get(&character) {
        Some(char_sessions) => {
            (StatusCode::OK, Json(char_sessions.clone())).into_response()
        }
        None => (StatusCode::OK, Json(Vec::<SessionStats>::new())).into_response(),
    }
}

async fn get_history(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let sessions = state.kill_tracker_state.sessions.read().await;
    let history: Vec<CharacterHistory> = sessions
        .iter()
        .map(|(character, char_sessions)| CharacterHistory {
            character: character.clone(),
            sessions: char_sessions.clone(),
        })
        .collect();
    (StatusCode::OK, Json(history)).into_response()
}

pub fn demo_session() -> SessionStats {
    SessionStats {
        character: "Frostreaver".to_string(),
        session_start: Utc::now().to_rfc3339(),
        total_kills: 47,
        total_deaths: 2,
        kills_per_hour: 42.5,
        efficiency: EfficiencyScore {
            kills_per_hour: 42.5,
            avg_kill_time_secs: 28.3,
            death_ratio: 0.04,
            score: 78.5,
        },
        mob_stats: vec![
            MobStats {
                mob_name: "Fennin Ro".to_string(),
                kill_count: 15,
                best_time_ms: 22000,
                avg_time_ms: 28000,
                avg_dps: 1850.0,
            },
            MobStats {
                mob_name: "Fire Elemental".to_string(),
                kill_count: 12,
                best_time_ms: 18000,
                avg_time_ms: 24000,
                avg_dps: 2100.0,
            },
            MobStats {
                mob_name: "Flame Spirit".to_string(),
                kill_count: 20,
                best_time_ms: 12000,
                avg_time_ms: 16000,
                avg_dps: 2400.0,
            },
        ],
        top_mobs: vec![
            ("Flame Spirit".to_string(), 20),
            ("Fennin Ro".to_string(), 15),
            ("Fire Elemental".to_string(), 12),
        ],
        zone: "Plane of Fire".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState {
            event_tx: tokio::sync::broadcast::channel::<String>(8).0,
            account_store: std::sync::Mutex::new(crate::accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(HashMap::new()),
            loot_state: crate::api::loot::LootState::new_demo(),
            economy_state: crate::api::economy::EconomyState::new_demo(),
            dashboard_state: crate::api::dashboard::DashboardState::new_demo(),
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            api_token: None,
            kill_tracker_state: KillTrackerState::new_demo(),
        })
    }

    #[tokio::test]
    async fn get_settings_returns_default() {
        let state = test_state();
        let response = get_settings(State(state)).await;
        let Json(settings) = response;
        assert!(settings.enabled);
        assert_eq!(settings.auto_report_interval_minutes, 10);
    }

    #[tokio::test]
    async fn put_settings_updates_state() {
        let state = test_state();
        let new_settings = KillTrackerSettings {
            enabled: false,
            auto_report_interval_minutes: 5,
            auto_report_channel: "raid".to_string(),
            auto_report_include_mobs: false,
            auto_report_include_kph: true,
            track_per_character: true,
            max_session_history: 50,
        };

        let response = put_settings(State(state.clone()), Json(new_settings.clone())).await;
        assert_eq!(response.status(), StatusCode::OK);

        let Json(updated) = get_settings(State(state)).await;
        assert!(!updated.enabled);
        assert_eq!(updated.auto_report_interval_minutes, 5);
    }

    #[tokio::test]
    async fn get_sessions_returns_empty_when_no_sessions() {
        let state = test_state();
        let response = get_sessions(State(state)).await;
        let Json(sessions) = response;
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn get_character_sessions_returns_empty_for_unknown_character() {
        let state = test_state();
        let response = get_character_sessions(State(state.clone()), Path("UnknownChar".to_string())).await;
        let Json(sessions) = response;
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn get_history_returns_empty_when_no_sessions() {
        let state = test_state();
        let response = get_history(State(state)).await;
        let Json(history) = response;
        assert!(history.is_empty());
    }
}
