//! TextQuest Web Dashboard — Axum backend for the monitoring UI.
//!
//! The backend serves:
//! - live health, session, economy, dashboard, and per-character tuning
//!   endpoints
//! - loot APIs
//! - account-management APIs backed by an in-memory registry plus optional
//!   credential storage
//! - explicit `501` placeholders for not-yet-implemented raid configuration
//!   APIs
//! - live character-configuration APIs backed by in-memory dashboard state
//! - a WebSocket endpoint for live session monitoring

use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use textquest_common::chat_pattern_rules::ChatPatternRuleEngine;

use axum::{
    Router,
    extract::Request,
    http::{HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{delete, get, post, put},
};
use tokio::sync::broadcast;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};

use textquest::{
    alerts::AlertStore,
    config::AlertingConfig,
};

mod accounts;
mod accounts;
mod api;
mod ws;

/// Shared application state accessible from all handlers.
pub struct AppState {
    /// Broadcast channel for real-time session events.
    pub event_tx: broadcast::Sender<String>,
    /// In-memory account registry.
    pub account_store: Mutex<accounts::AccountStore>,
    /// Optional encrypted password store, enabled by
    /// `TEXTQUEST_MASTER_PASSWORD`.
    pub credential_store: Option<accounts::CredentialStore>,
    /// In-memory character tuning config store for the strategy tuning panel.
    pub character_configs: tokio::sync::RwLock<HashMap<String, api::CharacterConfig>>,
    /// In-memory auto-accept policy store for the dashboard controls.
    pub auto_accept_settings: tokio::sync::RwLock<textquest_common::ipc::AutoAcceptSettings>,
    /// In-memory loot configuration state.
    pub loot_state: Arc<api::loot::LootState>,
    /// In-memory economy cycle state.
    pub economy_state: Arc<api::economy::EconomyState>,
    /// In-memory operator dashboard snapshot and action state.
    pub dashboard_state: Arc<api::dashboard::DashboardState>,
    /// In-memory soul audit log.
    pub soul_audit: Arc<api::soul::SoulAuditState>,
    /// In-memory Discord routing and webhook settings.
    pub discord_state: Arc<api::discord::DiscordState>,
    /// In-memory player watch (zone entry/exit) configuration.
    pub player_watch_config: tokio::sync::RwLock<api::PlayerWatchConfig>,
    /// GM alert state — zone-wide GM detection status for web dashboard.
    pub gm_alert_state: Arc<api::gm_alerts::GmAlertState>,
    /// In-memory spawn alert state for rare spawn monitoring.
    pub spawn_alerts: Arc<api::spawn_alerts::SpawnAlertState>,
    /// In-memory timestamp config store per character.
    pub timestamp_configs: tokio::sync::RwLock<HashMap<String, api::TimestampConfig>>,
    /// Kill tracker state for session tracking and auto-reporting.
    pub kill_tracker_state: Arc<api::kill_tracker::KillTrackerState>,
    /// Persistent operational alert history.
    pub alert_store: AlertStore,
    /// Runtime-editable alert delivery configuration for the web dashboard.
    pub alert_config: tokio::sync::RwLock<AlertingConfig>,
    /// Disk location where `PUT /api/alerts/config` persists the
    /// `AlertingConfig`. Tests point this at a tempfile via
    /// [`test_state`] so they never touch the checked-in repo.
    pub alerting_config_path: PathBuf,
    /// Optional static API token for protecting all `/api` endpoints.
    /// Set via `TEXTQUEST_API_TOKEN` environment variable.
    /// When `None`, API endpoints are unauthenticated (localhost-only
    /// deployment).
    pub api_token: Option<String>,
    /// Mutable runtime snapshot written by the orchestrator for live session
    /// monitoring.
    pub live_session_snapshot_path: PathBuf,
    /// In-memory XAssist configuration per character.
    pub xassist_configs: api::xassist::XAssistConfigs,
    /// In-memory chat pattern rules engine for MQ2Events/MQ2React parity.
    pub chat_pattern_rules: tokio::sync::RwLock<ChatPatternRuleEngine>,
}

/// Axum middleware: enforce `X-API-Token` header when `TEXTQUEST_API_TOKEN` is
/// set.
///
/// If the env var is unset, all requests pass through (backward-compatible
/// default). When set, requests without a matching token receive `401
/// Unauthorized`.
async fn api_token_auth(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(ref expected_token) = state.api_token {
        let provided = req
            .headers()
            .get("x-api-token")
            .and_then(|v| v.to_str().ok());

        match provided {
            Some(token) if constant_time_eq_str(token, expected_token) => {}
            _ => {
                tracing::warn!(
                    path = req.uri().path(),
                    "API request rejected: missing or invalid X-API-Token"
                );
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    }
    Ok(next.run(req).await)
}

/// Constant-time string comparison to prevent timing oracle attacks on the API
/// token.
fn constant_time_eq_str(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    ab.iter()
        .zip(bb.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn credentials_db_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/credentials.db")
}

fn live_session_snapshot_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/runtime/live_sessions.json")
}

fn alerts_db_path() -> PathBuf {
    // Delegate to the shared helper so the TUI and web processes always
    // resolve to the same SQLite file. Honors TEXTQUEST_ALERT_DB_PATH.
    textquest::alerts::resolve_alert_db_path()
}

/// Side-car file where dashboard-edited [`AlertingConfig`] values persist
/// across web restarts. Separate from `config/textquest.toml` so the web
/// dashboard can re-save alert config without touching operator-managed
/// TUI configuration.
fn alerting_config_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("TEXTQUEST_ALERTING_CONFIG_PATH") {
        if !override_path.is_empty() {
            return PathBuf::from(override_path);
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../config/alerting.toml")
}

fn load_alerting_config_from(path: &std::path::Path) -> AlertingConfig {
    match std::fs::read_to_string(path) {
        Ok(contents) => match toml::from_str::<AlertingConfig>(&contents) {
            Ok(config) => {
                tracing::info!(path = %path.display(), "Loaded persisted alerting config");
                config
            }
            Err(error) => {
                tracing::warn!(
                    %error,
                    path = %path.display(),
                    "Failed to parse alerting config; using defaults"
                );
                AlertingConfig::default()
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => AlertingConfig::default(),
        Err(error) => {
            tracing::warn!(
                %error,
                path = %path.display(),
                "Failed to read alerting config; using defaults"
            );
            AlertingConfig::default()
        }
    }
}

/// Persist the supplied [`AlertingConfig`] to `path` so subsequent web
/// restarts (and, once an alerting-config watcher is added to the TUI, the
/// live TUI process) pick up dashboard edits. Errors are logged and
/// surfaced to the caller.
pub fn persist_alerting_config(
    path: &std::path::Path,
    config: &AlertingConfig,
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            anyhow::anyhow!(
                "Failed to create alerting config directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let contents = toml::to_string_pretty(config).map_err(|error| {
        anyhow::anyhow!("Failed to serialize alerting config: {error}")
    })?;
    std::fs::write(path, contents).map_err(|error| {
        anyhow::anyhow!(
            "Failed to write alerting config to {}: {error}",
            path.display()
        )
    })?;
    Ok(())
}

fn open_alert_store() -> AlertStore {
    let path = alerts_db_path();
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        tracing::warn!(
            %error,
            path = %parent.display(),
            "Failed to create alert store directory"
        );
    }

    match AlertStore::open(&path) {
        Ok(store) => store,
        Err(error) => {
            tracing::error!(
                %error,
                path = %path.display(),
                "Falling back to in-memory alert store"
            );
            AlertStore::open_memory().expect("in-memory alert store")
        }
    }
}

/// Build the initial application state for production use.
///
/// - `event_tx` and `account_store` are always initialised empty.
/// - `credential_store` is populated only when `TEXTQUEST_MASTER_PASSWORD` is
///   set in the environment; otherwise password routes return `501`.
/// - `character_configs`, `loot_state`, `economy_state`, and `soul_audit` are
///   seeded with in-memory state; character-config routes still return `501`
///   until a supported backing store is wired.
fn build_state() -> Arc<AppState> {
    let (event_tx, _) = broadcast::channel::<String>(256);
    let credential_store = std::env::var("TEXTQUEST_MASTER_PASSWORD")
        .ok()
        .filter(|password| !password.trim().is_empty())
        .and_then(
            |password| match accounts::CredentialStore::open(&credentials_db_path(), &password) {
                Ok(store) => Some(store),
                Err(error) => {
                    tracing::error!(%error, "Failed to initialize credential store; password routes will return 501");
                    None
                }
            },
        );

    let api_token = std::env::var("TEXTQUEST_API_TOKEN")
        .ok()
        .filter(|t| !t.trim().is_empty());

    if api_token.is_none() {
        tracing::warn!(
            "TEXTQUEST_API_TOKEN is not set — API endpoints are unauthenticated. Set this env var \
             to enable token-based authentication."
        );
    }

    Arc::new(AppState {
        event_tx,
        account_store: Mutex::new(accounts::AccountStore::default()),
        credential_store,
        character_configs: tokio::sync::RwLock::new(api::demo_character_configs()),
        auto_accept_settings: tokio::sync::RwLock::new(Default::default()),
        loot_state: api::loot::LootState::new_demo(),
        economy_state: api::economy::EconomyState::new_demo(),
        dashboard_state: api::dashboard::DashboardState::new_demo(),
        soul_audit: api::soul::SoulAuditState::new_demo(),
        discord_state: api::discord::DiscordState::new_demo(),
        player_watch_config: tokio::sync::RwLock::new(api::PlayerWatchConfig::default()),
        gm_alert_state: Arc::new(api::gm_alerts::GmAlertState::default()),
        spawn_alerts: api::spawn_alerts::SpawnAlertState::new_demo(),
        timestamp_configs: tokio::sync::RwLock::new(HashMap::new()),
        kill_tracker_state: api::kill_tracker::KillTrackerState::new_demo(),
        alert_store: open_alert_store(),
        alert_config: tokio::sync::RwLock::new(load_alerting_config_from(&alerting_config_path())),
        alerting_config_path: alerting_config_path(),
        api_token,
        live_session_snapshot_path: live_session_snapshot_path(),
        xassist_configs: api::xassist::demo_xassist_configs(),
        chat_pattern_rules: api::chat_pattern_rules::load_rules_state(),
    })
}

/// Build the soul audit sub-router.
fn build_soul_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/audit", get(api::soul::get_all_audit))
        .route("/audit/export.csv", get(api::soul::export_all_audit_csv))
        .route("/audit/{character_id}", get(api::soul::get_character_audit))
        .route(
            "/audit/{character_id}/export.csv",
            get(api::soul::export_character_audit_csv),
        )
}

/// Build the loot sub-router.  Loot handlers extract `State<Arc<AppState>>`
/// and access `state.loot_state`, so this router shares the same state type
/// as the rest of the API — eliminating the type mismatch that arose when
/// `Arc<LootState>` was provided as its own separate state.
fn build_loot_router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/rules",
            get(api::loot::get_rules).put(api::loot::put_rules),
        )
        .route("/filters", get(api::loot::get_filters))
        .route("/filters/{character}", put(api::loot::put_filter))
        .route(
            "/master-looter",
            get(api::loot::get_master_looter).put(api::loot::put_master_looter),
        )
        .route(
            "/distribution",
            get(api::loot::get_distribution).put(api::loot::put_distribution),
        )
        .route("/history", get(api::loot::get_history))
}

fn build_api_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(api::health))
        .route("/sessions", get(api::list_sessions))
        .nest("/accounts", accounts::router())
        .route(
            "/economy/settings",
            get(api::get_economy_settings).put(api::put_economy_settings),
        )
        .route(
            "/economy/vendor-routes",
            get(api::list_vendor_routes).post(api::create_vendor_route),
        )
        .route(
            "/economy/vendor-routes/{id}",
            put(api::update_vendor_route).delete(api::delete_vendor_route),
        )
        .route("/economy/wealth", get(api::get_wealth))
        .route("/soul", get(api::soul::list_soul_states))
        .route("/soul/{character_id}", get(api::soul::get_soul_state))
        .nest("/alerts", api::alerts::router())
        .route(
            "/raid/config",
            get(api::raid_config_unavailable).put(api::raid_config_unavailable),
        )
        .route("/config/characters", get(api::list_character_configs))
        .route(
            "/config/discord",
            get(api::discord::get_settings).put(api::discord::put_settings),
        )
        .route(
            "/config/characters/{character}",
            put(api::put_character_config),
        )
        .route(
            "/config/auto-accept",
            get(api::get_auto_accept_settings).put(api::put_auto_accept_settings),
        )
        .route(
            "/config/player-watch",
            get(api::get_player_watch_config).put(api::put_player_watch_config),
        )
        // Spawn Alerts API
        .route(
            "/spawn-alerts",
            get(api::spawn_alerts::list_alerts).delete(api::spawn_alerts::clear_alerts),
        )
        .route("/spawn-alerts/stats", get(api::spawn_alerts::get_stats))
        .route(
            "/spawn-alerts/config",
            get(api::spawn_alerts::get_config).put(api::spawn_alerts::put_config),
        )
        .route(
            "/spawn-alerts/watch-list",
            get(api::spawn_alerts::get_watch_list),
        )
        .route(
            "/spawn-alerts/watch-list/{pattern}",
            put(api::spawn_alerts::put_watch_pattern)
                .delete(api::spawn_alerts::delete_watch_pattern),
        )
        // Timestamp Config API
        .route("/timestamp-config", get(api::list_timestamp_configs))
        .route(
            "/spawn-alerts/watch-list/{pattern}",
            put(api::spawn_alerts::put_watch_pattern)
                .delete(api::spawn_alerts::delete_watch_pattern),
        )
        // Timestamp Config API
        .route("/timestamp-config", get(api::list_timestamp_configs))
        .route(
            "/timestamp-config/{character}",
            get(api::get_timestamp_config).put(api::put_timestamp_config),
        )
        .nest("/kill-tracker", api::kill_tracker::router())
        .nest("/loot", build_loot_router())
        .nest("/soul", build_soul_router())
        .nest("/gm-alerts", api::gm_alerts::router())
        .route("/xassist/configs", get(api::xassist::list_xassist_configs))
        .route(
            "/xassist/config/{character}",
            get(api::xassist::get_xassist_config)
                .put(api::xassist::put_xassist_config)
                .delete(api::xassist::delete_xassist_config),
        )
        // Chat Pattern Rules API
        .route(
            "/chat-pattern-rules",
            get(api::chat_pattern_rules::list_rules),
        )
        .route(
            "/chat-pattern-rules/stats",
            get(api::chat_pattern_rules::get_stats),
        )
        .route(
            "/chat-pattern-rules/import",
            post(api::chat_pattern_rules::import_rules),
        )
        .route(
            "/chat-pattern-rules/{id}",
            get(api::chat_pattern_rules::get_rule),
        )
        .route(
            "/chat-pattern-rules/{id}",
            put(api::chat_pattern_rules::update_rule),
        )
        .route(
            "/chat-pattern-rules/{id}",
            delete(api::chat_pattern_rules::delete_rule),
        )
        .route(
            "/chat-pattern-rules/{id}/toggle",
            put(api::chat_pattern_rules::toggle_rule),
        )
        .route(
            "/chat-pattern-rules/{id}/reset-cooldown",
            put(api::chat_pattern_rules::reset_cooldown),
        )
        .route(
            "/chat-pattern-rules/cooldowns/reset",
            put(api::chat_pattern_rules::reset_all_cooldowns),
        )
        .fallback(api::api_not_found)
}

fn build_app(state: Arc<AppState>) -> Router {
    let spa_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/dist");
    let serve_spa =
        ServeDir::new(&spa_dir).not_found_service(ServeFile::new(spa_dir.join("index.html")));

    // Restrict CORS to trusted local dashboard origins so cross-site pages
    // cannot issue authenticated-like write requests against localhost APIs.
    // Derived from `api::loot::TRUSTED_ORIGINS` so CORS middleware and the
    // per-handler origin guard always use the same allowlist.
    let cors = CorsLayer::new()
        .allow_origin(
            api::loot::TRUSTED_ORIGINS
                .iter()
                .map(|&o| HeaderValue::from_static(o))
                .collect::<Vec<_>>(),
        )
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    Router::new()
        // Apply API token authentication to all /api routes before routing.
        // The middleware is a no-op when TEXTQUEST_API_TOKEN is unset (backward compatible).
        .nest(
            "/api",
            build_api_router().layer(middleware::from_fn_with_state(
                state.clone(),
                api_token_auth,
            )),
        )
        .route("/ws", get(ws::ws_handler))
        .fallback_service(serve_spa)
        .layer(cors)
        .with_state(state)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("textquest_web=debug,tower_http=debug")
        .init();

    let state = build_state();
    let app = build_app(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    tracing::info!("TextQuest web dashboard listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use textquest_common::shared_client_state::SharedClientState;
    use tower::ServiceExt;

    async fn json_response(app: Router, request: Request<Body>) -> (StatusCode, Value) {
        let response = app.oneshot(request).await.expect("request should succeed");
        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should collect")
            .to_bytes();
        let value = serde_json::from_slice(&body).expect("body should be valid json");
        (status, value)
    }

    fn test_state_with_credentials(path: &std::path::Path) -> Arc<AppState> {
        let (event_tx, _) = broadcast::channel::<String>(8);
        Arc::new(AppState {
            event_tx,
            account_store: Mutex::new(accounts::AccountStore::default()),
            credential_store: Some(
                accounts::CredentialStore::open(path, "test_master_pw").expect("credential store"),
            ),
            character_configs: tokio::sync::RwLock::new(api::demo_character_configs()),
            auto_accept_settings: tokio::sync::RwLock::new(Default::default()),
            loot_state: api::loot::LootState::new_demo(),
            economy_state: api::economy::EconomyState::new_demo(),
            dashboard_state: api::dashboard::DashboardState::new_demo(),
            soul_audit: api::soul::SoulAuditState::new_demo(),
            discord_state: api::discord::DiscordState::new_demo(),
            player_watch_config: tokio::sync::RwLock::new(api::PlayerWatchConfig::default()),
            gm_alert_state: Arc::new(api::gm_alerts::GmAlertState::default()),
            spawn_alerts: api::spawn_alerts::SpawnAlertState::new_demo(),
            timestamp_configs: tokio::sync::RwLock::new(HashMap::new()),
            kill_tracker_state: api::kill_tracker::KillTrackerState::new_demo(),
            alert_store: AlertStore::open_memory().expect("alert store"),
            alert_config: tokio::sync::RwLock::new(AlertingConfig::default()),
            alerting_config_path: std::env::temp_dir()
                .join(format!("textquest-main-test-alerting-{}.toml", uuid::Uuid::new_v4())),
            api_token: None, // No auth in tests — auth middleware is a no-op when None
            live_session_snapshot_path: path.with_file_name("live_sessions.json"),
            xassist_configs: api::xassist::demo_xassist_configs(),
            chat_pattern_rules: api::chat_pattern_rules::load_rules_state(),
        })
    }

    #[tokio::test]
    async fn unknown_api_route_returns_json_404() {
        let app = build_app(build_state());
        let (status, body) = json_response(
            app,
            Request::builder()
                .uri("/api/not-real")
                .body(Body::empty())
                .expect("request"),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "API route not found");
    }

    #[tokio::test]
    async fn known_unimplemented_routes_return_json_501() {
        let app = build_app(build_state());
        let (status, body) = json_response(
            app.clone(),
            Request::builder()
                .uri("/api/raid/config")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert!(
            body["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not implemented")
        );

        let (status, body) = json_response(
            app,
            Request::builder()
                .uri("/api/config/characters")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert!(
            body["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not implemented")
        );
    }

    #[tokio::test]
    async fn sessions_endpoint_prefers_live_snapshot_when_present() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let state = test_state_with_credentials(&tempdir.path().join("creds.db"));
        std::fs::write(
            &state.live_session_snapshot_path,
            serde_json::to_vec(&vec![SharedClientState {
                client_id: 77,
                spawn_id: 42,
                character_name: "Frostreaver".into(),
                class_id: 2,
                level: 60,
                zone_short_name: "kael".into(),
                zone_long_name: "Kael Drakkel".into(),
                hp_pct: 72.5,
                mana_pct: 81.0,
                endurance_pct: 49.0,
                is_dead: false,
                status: "active".into(),
                target: None,
                buffs: Vec::new(),
                pet: None,
            }])
            .expect("snapshot json"),
        )
        .expect("write snapshot");

        let app = build_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .uri("/api/sessions")
                .body(Body::empty())
                .expect("request"),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let sessions = body.as_array().expect("sessions array");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0]["character_name"], "Frostreaver");
        assert_eq!(sessions[0]["zone"], "Kael Drakkel");
        assert_eq!(sessions[0]["endurance_pct"], 49.0);
    }

    #[tokio::test]
    async fn account_routes_are_mounted_and_roundtrip() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let state = test_state_with_credentials(&tempdir.path().join("creds.db"));
        let app = build_app(state);

        let (status, created) = json_response(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "wizard1",
                        "server": "Teek",
                        "character": "Starfire",
                        "class": "WIZ",
                        "group": 2,
                        "status": "active",
                        "password": "hunter2"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(created["name"], "wizard1");
        assert_eq!(created["has_password"], true);

        let (status, listed) = json_response(
            app.clone(),
            Request::builder()
                .uri("/api/accounts")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(listed.as_array().expect("array").len(), 1);

        let (status, updated) = json_response(
            app.clone(),
            Request::builder()
                .method("PUT")
                .uri("/api/accounts/wizard1")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "server": "Rizlona",
                        "group": 3
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["server"], "Rizlona");
        assert_eq!(updated["group"], 3);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/accounts/wizard1/password")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "password": "newpass" }).to_string()))
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let (status, exported) = json_response(
            app.clone(),
            Request::builder()
                .uri("/api/accounts/export")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(exported["accounts"].as_array().expect("array").len(), 1);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/accounts/wizard1")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let (status, imported) = json_response(
            app,
            Request::builder()
                .method("POST")
                .uri("/api/accounts/import")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "accounts": exported["accounts"]
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(imported["imported"], 1);
    }

    #[tokio::test]
    async fn password_route_returns_501_without_credential_store() {
        let app = build_app(build_state());

        let _ = json_response(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "cleric1",
                        "server": "Teek",
                        "character": "Mercy",
                        "class": "CLR",
                        "group": 1,
                        "status": "active"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await;

        let (status, body) = json_response(
            app,
            Request::builder()
                .method("PUT")
                .uri("/api/accounts/cleric1/password")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "password": "secret" }).to_string()))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert!(
            body["error"]
                .as_str()
                .unwrap_or_default()
                .contains("TEXTQUEST_MASTER_PASSWORD")
        );
    }
}
