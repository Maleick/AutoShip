use std::{collections::HashMap, sync::Arc};

use axum::{
    Json,
    extract::{Path, State},
    routing::get,
};
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
) -> Json<KillTrackerSettings> {
    state
        .kill_tracker_state
        .update_settings(settings.clone())
        .await;
    Json(settings)
}

async fn get_sessions(State(state): State<Arc<AppState>>) -> Json<Vec<SessionStats>> {
    let sessions = state.kill_tracker_state.sessions.read().await;
    let mut result: Vec<SessionStats> = Vec::new();
    for (_character, char_sessions) in sessions.iter() {
        if let Some(latest) = char_sessions.last() {
            result.push(latest.clone());
        }
    }
    Json(result)
}

async fn get_character_sessions(
    State(state): State<Arc<AppState>>,
    Path(character): Path<String>,
) -> Json<Vec<SessionStats>> {
    let sessions = state.kill_tracker_state.sessions.read().await;
    match sessions.get(&character) {
        Some(char_sessions) => Json(char_sessions.clone()),
        None => Json(Vec::<SessionStats>::new()),
    }
}

async fn get_history(State(state): State<Arc<AppState>>) -> Json<Vec<CharacterHistory>> {
    let sessions = state.kill_tracker_state.sessions.read().await;
    let history: Vec<CharacterHistory> = sessions
        .iter()
        .map(|(character, char_sessions)| CharacterHistory {
            character: character.clone(),
            sessions: char_sessions.clone(),
        })
        .collect();
    Json(history)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppState;
    use std::sync::Arc;

    fn test_state() -> Arc<AppState> {
        let mut state = crate::test_app_state();
        state.kill_tracker_state = KillTrackerState::new_demo();
        Arc::new(state)
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

        let Json(saved) = put_settings(State(state.clone()), Json(new_settings.clone())).await;
        assert_eq!(saved.auto_report_interval_minutes, 5);

        let Json(updated) = get_settings(State(state)).await;
        assert!(!updated.enabled);
        assert_eq!(updated.auto_report_interval_minutes, 5);
    }

    #[tokio::test]
    async fn get_sessions_returns_empty_when_no_sessions() {
        let state = test_state();
        let Json(sessions) = get_sessions(State(state)).await;
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn get_character_sessions_returns_empty_for_unknown_character() {
        let state = test_state();
        let Json(sessions) =
            get_character_sessions(State(state.clone()), Path("UnknownChar".to_string())).await;
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn get_history_returns_empty_when_no_sessions() {
        let state = test_state();
        let Json(history) = get_history(State(state)).await;
        assert!(history.is_empty());
    }
}
