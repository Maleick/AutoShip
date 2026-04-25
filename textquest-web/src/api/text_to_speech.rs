//! Text-to-speech configuration and alerting API handlers.
//!
//! Mirrors MQTextToSpeech plugin functionality: reads configurable chat
//! channels aloud using platform TTS engines.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::AppState;

use super::json_error;

/// Text-to-speech engine selection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TtsEngine {
    /// System default TTS (platform-native: SAPI5 on Windows, AVFoundation on macOS, festival on Linux)
    System,
    /// SAPI5 (Windows-only)
    Sapi5,
    /// Apple AVFoundation (macOS-only)
    AvFoundation,
    /// Festival/eSpeak (Linux)
    Festival,
}

impl std::fmt::Display for TtsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TtsEngine::System => write!(f, "system"),
            TtsEngine::Sapi5 => write!(f, "sapi5"),
            TtsEngine::AvFoundation => write!(f, "avfoundation"),
            TtsEngine::Festival => write!(f, "festival"),
        }
    }
}

impl Default for TtsEngine {
    fn default() -> Self {
        #[cfg(target_os = "windows")]
        return TtsEngine::Sapi5;
        #[cfg(target_os = "macos")]
        return TtsEngine::AvFoundation;
        #[cfg(target_os = "linux")]
        return TtsEngine::Festival;
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        return TtsEngine::System;
    }
}

/// Chat channels eligible for TTS reading.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TtsChatChannel {
    Say,
    Tell,
    TellOut,
    Group,
    Guild,
    Raid,
    Shout,
    Ooc,
    Auction,
}

impl std::fmt::Display for TtsChatChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TtsChatChannel::Say => write!(f, "say"),
            TtsChatChannel::Tell => write!(f, "tell"),
            TtsChatChannel::TellOut => write!(f, "tell_out"),
            TtsChatChannel::Group => write!(f, "group"),
            TtsChatChannel::Guild => write!(f, "guild"),
            TtsChatChannel::Raid => write!(f, "raid"),
            TtsChatChannel::Shout => write!(f, "shout"),
            TtsChatChannel::Ooc => write!(f, "ooc"),
            TtsChatChannel::Auction => write!(f, "auction"),
        }
    }
}

/// TTS voice settings per-channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsChannelConfig {
    /// Whether this channel should be read aloud.
    pub enabled: bool,
    /// Voice name (e.g., "David" on SAPI5, "Samantha" on AVFoundation).
    /// If empty, uses default voice.
    pub voice_name: Option<String>,
    /// Speech rate multiplier (0.5–2.0, default 1.0).
    pub rate: f32,
    /// Volume multiplier (0.0–1.0, default 1.0).
    pub volume: f32,
}

impl Default for TtsChannelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            voice_name: None,
            rate: 1.0,
            volume: 1.0,
        }
    }
}

/// Global text-to-speech configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextToSpeechConfig {
    /// Master enable/disable.
    pub enabled: bool,
    /// TTS engine to use.
    pub engine: TtsEngine,
    /// Per-channel TTS settings.
    pub channels: Vec<(TtsChatChannel, TtsChannelConfig)>,
}

impl Default for TextToSpeechConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            engine: TtsEngine::default(),
            channels: vec![
                (TtsChatChannel::Tell, TtsChannelConfig::default()),
                (TtsChatChannel::Guild, TtsChannelConfig::default()),
                (TtsChatChannel::Raid, TtsChannelConfig::default()),
                (TtsChatChannel::Group, TtsChannelConfig::default()),
            ],
        }
    }
}

/// In-memory TTS state.
pub struct TextToSpeechState {
    pub config: RwLock<TextToSpeechConfig>,
}

impl TextToSpeechState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            config: RwLock::new(TextToSpeechConfig::default()),
        })
    }
}

impl Default for TextToSpeechState {
    fn default() -> Self {
        Self {
            config: RwLock::new(TextToSpeechConfig::default()),
        }
    }
}

/// GET /api/tts/config
pub async fn get_tts_config(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let config = state.text_to_speech_state.config.read().await;
    (StatusCode::OK, Json(config.clone())).into_response()
}

/// PUT /api/tts/config
pub async fn put_tts_config(
    State(state): State<Arc<AppState>>,
    Json(config): Json<TextToSpeechConfig>,
) -> impl IntoResponse {
    *state.text_to_speech_state.config.write().await = config.clone();
    (StatusCode::OK, Json(config)).into_response()
}

/// GET /api/tts/engines — list available TTS engines on this system.
pub async fn list_tts_engines() -> impl IntoResponse {
    let available = vec![
        #[cfg(target_os = "windows")]
        ("sapi5", "SAPI5 (Windows)"),
        #[cfg(target_os = "macos")]
        ("avfoundation", "Apple AVFoundation (macOS)"),
        #[cfg(target_os = "linux")]
        ("festival", "Festival/eSpeak (Linux)"),
    ];

    let engines = serde_json::json!({
        "available": available,
        "default": TtsEngine::default().to_string(),
    });

    (StatusCode::OK, Json(engines)).into_response()
}

/// GET /api/tts/status — check if TTS system is ready.
pub async fn get_tts_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.text_to_speech_state.config.read().await;

    let status = serde_json::json!({
        "enabled": config.enabled,
        "engine": config.engine.to_string(),
        "active_channels": config.channels
            .iter()
            .filter(|(_, cfg)| cfg.enabled)
            .map(|(ch, _)| ch.to_string())
            .collect::<Vec<_>>(),
    });

    (StatusCode::OK, Json(status)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tts_config_is_disabled() {
        let config = TextToSpeechConfig::default();
        assert!(!config.enabled, "TTS should be disabled by default");
    }

    #[test]
    fn default_tts_config_has_channels() {
        let config = TextToSpeechConfig::default();
        assert!(!config.channels.is_empty(), "default config should have channels");
        // At least Tell, Guild, Raid, Group should be present
        assert!(config.channels.len() >= 4, "should have at least 4 channels");
    }

    #[test]
    fn tts_channel_config_has_sensible_defaults() {
        let cfg = TtsChannelConfig::default();
        assert_eq!(cfg.rate, 1.0, "default rate should be 1.0");
        assert_eq!(cfg.volume, 1.0, "default volume should be 1.0");
        assert!(!cfg.enabled, "channels disabled by default");
    }

    #[test]
    fn tts_engines_are_platform_aware() {
        let engine = TtsEngine::default();
        #[cfg(target_os = "windows")]
        assert_eq!(engine, TtsEngine::Sapi5);
        #[cfg(target_os = "macos")]
        assert_eq!(engine, TtsEngine::AvFoundation);
        #[cfg(target_os = "linux")]
        assert_eq!(engine, TtsEngine::Festival);
    }
}
