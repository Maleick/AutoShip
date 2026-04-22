//! REST API handlers for Discord webhook and alert routing configuration.

use std::{collections::HashMap, sync::Arc};

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::AppState;

use textquest_common::integrations::{DiscordMentionPolicy, DiscordRouteConfig, Severity};

/// In-memory Discord settings state shared across handlers.
pub struct DiscordState {
    pub settings: RwLock<DiscordSettings>,
}

impl DiscordState {
    /// Create state pre-populated with the dashboard's default Discord
    /// settings.
    pub fn new_demo() -> Arc<Self> {
        Arc::new(Self {
            settings: RwLock::new(DiscordSettings::default()),
        })
    }
}

/// Full Discord dashboard settings payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscordSettings {
    /// Default webhook URL used when a route-specific URL is blank.
    pub webhook_url: String,
    /// Per-category webhook routing for the existing metrics relay.
    pub channels: HashMap<String, String>,
    /// Per-alert-type routing overrides for operational alerts.
    pub notification_routes: HashMap<String, DiscordRouteConfig>,
}

impl Default for DiscordSettings {
    fn default() -> Self {
        let mut notification_routes = HashMap::new();
        notification_routes.insert(
            "death".into(),
            DiscordRouteConfig {
                level: Severity::Critical,
                mention_policy: DiscordMentionPolicy::Everyone,
                ..DiscordRouteConfig::default()
            },
        );
        notification_routes.insert(
            "status".into(),
            DiscordRouteConfig {
                level: Severity::Info,
                ..DiscordRouteConfig::default()
            },
        );
        notification_routes.insert(
            "hvt".into(),
            DiscordRouteConfig {
                level: Severity::Critical,
                ..DiscordRouteConfig::default()
            },
        );
        notification_routes.insert(
            "crash".into(),
            DiscordRouteConfig {
                level: Severity::Critical,
                ..DiscordRouteConfig::default()
            },
        );
        notification_routes.insert(
            "mass_failure".into(),
            DiscordRouteConfig {
                level: Severity::Critical,
                ..DiscordRouteConfig::default()
            },
        );

        Self {
            webhook_url: String::new(),
            channels: HashMap::from([
                ("kills".into(), String::new()),
                ("loot".into(), String::new()),
                ("timers".into(), String::new()),
                ("feats".into(), String::new()),
                ("status".into(), String::new()),
            ]),
            notification_routes,
        }
    }
}

/// GET /api/config/discord — return current Discord routing settings.
pub async fn get_settings(State(state): State<Arc<AppState>>) -> Json<DiscordSettings> {
    Json(state.discord_state.settings.read().await.clone())
}

/// PUT /api/config/discord — update Discord routing settings.
pub async fn put_settings(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(mut settings): Json<DiscordSettings>,
) -> impl axum::response::IntoResponse {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    settings.webhook_url = settings.webhook_url.trim().to_string();
    settings.channels = settings
        .channels
        .into_iter()
        .map(|(key, value)| (key, value.trim().to_string()))
        .collect();
    settings.notification_routes = settings
        .notification_routes
        .into_iter()
        .map(|(key, mut route)| {
            route.webhook_url = route.webhook_url.trim().to_string();
            (key, route)
        })
        .collect();

    *state.discord_state.settings.write().await = settings.clone();
    (StatusCode::OK, Json(settings)).into_response()
}

use axum::response::IntoResponse;

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::integrations::DiscordMessageMode;

    fn demo_state() -> Arc<AppState> {
        crate::test_support::demo_app_state()
    }

    /// Build a HeaderMap with a trusted local-dev Origin for mutation tests.
    fn trusted_headers() -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            axum::http::header::ORIGIN,
            crate::api::loot::TRUSTED_ORIGINS[0].parse().unwrap(),
        );
        h
    }

    #[tokio::test]
    async fn get_settings_returns_defaults() {
        let state = demo_state();
        let Json(settings) = get_settings(State(state)).await;
        assert!(settings.channels.contains_key("kills"));
        assert_eq!(
            settings
                .notification_routes
                .get("death")
                .expect("death route")
                .mention_policy,
            DiscordMentionPolicy::Everyone
        );
    }

    #[tokio::test]
    async fn put_settings_updates_state_and_trims_urls() {
        let state = demo_state();
        let settings = DiscordSettings {
            webhook_url: " https://discord.example.com/default ".into(),
            channels: HashMap::from([(
                "kills".into(),
                " https://discord.example.com/kills ".into(),
            )]),
            notification_routes: HashMap::from([(
                "death".into(),
                DiscordRouteConfig {
                    enabled: true,
                    webhook_url: " https://discord.example.com/death ".into(),
                    level: Severity::Critical,
                    message_mode: DiscordMessageMode::PlainText,
                    mention_policy: DiscordMentionPolicy::Everyone,
                },
            )]),
        };

        let status = put_settings(
            State(state.clone()),
            trusted_headers(),
            Json(settings.clone()),
        )
        .await
        .into_response()
        .status();
        assert_eq!(status, StatusCode::OK);

        let stored = state.discord_state.settings.read().await.clone();
        assert_eq!(stored.webhook_url, "https://discord.example.com/default");
        assert_eq!(
            stored.channels.get("kills").map(String::as_str),
            Some("https://discord.example.com/kills")
        );
        assert_eq!(
            stored
                .notification_routes
                .get("death")
                .expect("death route")
                .webhook_url,
            "https://discord.example.com/death"
        );
    }

    #[tokio::test]
    async fn put_settings_rejects_untrusted_origin() {
        let state = demo_state();
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::ORIGIN,
            "https://evil.example".parse().unwrap(),
        );

        let status = put_settings(
            State(state.clone()),
            headers,
            Json(DiscordSettings::default()),
        )
        .await
        .into_response()
        .status();
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            *state.discord_state.settings.read().await,
            DiscordSettings::default()
        );
    }
}
