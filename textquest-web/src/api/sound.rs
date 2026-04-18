//! Sound alert configuration API handlers.
#![allow(dead_code)]

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GameEventType {
    LowHp,
    Death,
    NamedSpawn,
    GmEnter,
    TellReceived,
    #[default]
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SoundType {
    File {
        path: String,
    },
    Beep,
    #[default]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundTrigger {
    pub id: String,
    pub name: String,
    pub event_pattern: String,
    #[serde(default)]
    pub sound: SoundType,
    pub enabled: bool,
    pub priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundConfig {
    pub enabled: bool,
    pub volume: f32,
    pub triggers: Vec<SoundTrigger>,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 0.75,
            triggers: vec![
                SoundTrigger {
                    id: "trigger_low_hp".into(),
                    name: "Low HP".into(),
                    event_pattern: "hp_low".into(),
                    sound: SoundType::File {
                        path: "sounds/hp_low.wav".into(),
                    },
                    enabled: true,
                    priority: 100,
                },
                SoundTrigger {
                    id: "trigger_death".into(),
                    name: "Death".into(),
                    event_pattern: "you_have_died".into(),
                    sound: SoundType::File {
                        path: "sounds/death.wav".into(),
                    },
                    enabled: true,
                    priority: 200,
                },
                SoundTrigger {
                    id: "trigger_named_spawn".into(),
                    name: "Named Spawn".into(),
                    event_pattern: "named_spawn".into(),
                    sound: SoundType::Beep,
                    enabled: true,
                    priority: 150,
                },
                SoundTrigger {
                    id: "trigger_gm_detected".into(),
                    name: "GM Detected".into(),
                    event_pattern: "gm_detected".into(),
                    sound: SoundType::File {
                        path: "sounds/gm_alert.wav".into(),
                    },
                    enabled: true,
                    priority: 255,
                },
                SoundTrigger {
                    id: "trigger_tell_received".into(),
                    name: "Tell Received".into(),
                    event_pattern: "tell:".into(),
                    sound: SoundType::File {
                        path: "sounds/tell.wav".into(),
                    },
                    enabled: true,
                    priority: 180,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundTriggerUpdate {
    pub name: String,
    pub event_pattern: String,
    #[serde(default)]
    pub sound: SoundType,
    pub enabled: bool,
    pub priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundConfigUpdate {
    pub enabled: Option<bool>,
    pub volume: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundTriggerCreate {
    pub name: String,
    pub event_pattern: String,
    #[serde(default)]
    pub sound: SoundType,
    pub enabled: bool,
    pub priority: u8,
}

/// GET /api/sound/config — return full sound configuration.
pub async fn get_sound_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = {
        let sound_state = state.sound_config.read().await;
        sound_state.clone()
    };
    (StatusCode::OK, Json(config)).into_response()
}

/// PUT /api/sound/config — update sound global settings (enabled, volume).
pub async fn put_sound_config(
    State(state): State<Arc<AppState>>,
    Json(update): Json<SoundConfigUpdate>,
) -> impl IntoResponse {
    let mut sound_state = state.sound_config.write().await;
    if let Some(enabled) = update.enabled {
        sound_state.enabled = enabled;
    }
    if let Some(volume) = update.volume {
        sound_state.volume = volume.clamp(0.0, 1.0);
    }
    (StatusCode::OK, Json(sound_state.clone())).into_response()
}

/// GET /api/sound/triggers — list all sound triggers.
pub async fn list_sound_triggers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let triggers = {
        let sound_state = state.sound_config.read().await;
        sound_state.triggers.clone()
    };
    (StatusCode::OK, Json(triggers)).into_response()
}

/// POST /api/sound/triggers — create a new sound trigger.
pub async fn create_sound_trigger(
    State(state): State<Arc<AppState>>,
    Json(trigger): Json<SoundTriggerCreate>,
) -> impl IntoResponse {
    let id = format!("trigger_{}", trigger.name.to_lowercase().replace(' ', "_"));
    let new_trigger = SoundTrigger {
        id,
        name: trigger.name,
        event_pattern: trigger.event_pattern,
        sound: trigger.sound,
        enabled: trigger.enabled,
        priority: trigger.priority,
    };

    let mut sound_state = state.sound_config.write().await;
    sound_state.triggers.push(new_trigger.clone());

    (StatusCode::CREATED, Json(new_trigger)).into_response()
}

/// GET /api/sound/triggers/:id — get a specific trigger.
pub async fn get_sound_trigger(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let sound_state = state.sound_config.read().await;
    if let Some(trigger) = sound_state.triggers.iter().find(|t| t.id == id) {
        (StatusCode::OK, Json(trigger.clone())).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Trigger '{}' not found", id) })),
        )
            .into_response()
    }
}

/// PUT /api/sound/triggers/:id — update a sound trigger.
pub async fn update_sound_trigger(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(update): Json<SoundTriggerUpdate>,
) -> impl IntoResponse {
    let mut sound_state = state.sound_config.write().await;
    if let Some(trigger) = sound_state.triggers.iter_mut().find(|t| t.id == id) {
        trigger.name = update.name;
        trigger.event_pattern = update.event_pattern;
        trigger.sound = update.sound;
        trigger.enabled = update.enabled;
        trigger.priority = update.priority;
        (StatusCode::OK, Json(trigger.clone())).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Trigger '{}' not found", id) })),
        )
            .into_response()
    }
}

/// DELETE /api/sound/triggers/:id — delete a sound trigger.
pub async fn delete_sound_trigger(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut sound_state = state.sound_config.write().await;
    let original_len = sound_state.triggers.len();
    sound_state.triggers.retain(|t| t.id != id);

    if sound_state.triggers.len() < original_len {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Trigger '{}' not found", id) })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_config_default_has_preset_triggers() {
        let config = SoundConfig::default();
        assert!(config.enabled);
        assert!((config.volume - 0.75).abs() < f32::EPSILON);
        assert_eq!(config.triggers.len(), 5);
        let names: Vec<_> = config.triggers.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"Low HP"));
        assert!(names.contains(&"Death"));
        assert!(names.contains(&"Named Spawn"));
        assert!(names.contains(&"GM Detected"));
        assert!(names.contains(&"Tell Received"));
    }

    #[test]
    fn sound_trigger_create_id_is_derived() {
        let create = SoundTriggerCreate {
            name: "My Custom Alert".into(),
            event_pattern: "custom_event".into(),
            sound: SoundType::Beep,
            enabled: true,
            priority: 50,
        };
        let id = format!("trigger_{}", create.name.to_lowercase().replace(' ', "_"));
        assert_eq!(id, "trigger_my_custom_alert");
    }

    #[test]
    fn sound_config_update_respects_volume_bounds() {
        let update = SoundConfigUpdate {
            enabled: None,
            volume: Some(1.5), // should be clamped
        };
        let new_volume = update.volume.unwrap().clamp(0.0, 1.0);
        assert!((new_volume - 1.0).abs() < f32::EPSILON);
    }
}
