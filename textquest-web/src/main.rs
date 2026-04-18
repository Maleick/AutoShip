//! TextQuest Web Dashboard — Axum backend for the monitoring UI.
//!
//! The backend serves:
//! - live health, session, economy, dashboard, and per-character tuning
//!   endpoints
//! - loot APIs
//! - account-management APIs backed by an in-memory registry plus optional
//!   credential storage
//! - raid configuration placeholders plus live character configuration APIs
//! - a WebSocket endpoint for live session monitoring

#![allow(dead_code)]

use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use textquest_common::auto_group::AutoGroupSettings;
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

use textquest::{alerts::AlertStore, config::AlertingConfig};

mod accounts;
mod api;
mod live_ipc;
#[cfg(test)]
mod test_support;
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
    /// Auto-accept trade settings exposed through the web API.
    pub auto_accept_settings: tokio::sync::RwLock<textquest_common::ipc::AutoAcceptSettings>,
    /// Tradeskill trophy automation settings exposed through the web API.
    pub tradeskill_trophy_settings:
        tokio::sync::RwLock<textquest_common::tradeskill_trophy::TradeskillTrophySettings>,
    /// On-disk JSON store for character tuning and reward automation settings.
    pub character_config_path: PathBuf,
    /// Serializes PUT-driven writes to [`character_config_path`] so concurrent
    /// updates can't interleave snapshot writes and drop acknowledged edits.
    /// Reads are unaffected — they still go through the `character_configs`
    /// RwLock.
    pub character_config_write_lock: tokio::sync::Mutex<()>,
    /// Persisted auto-group profiles consumed by the orchestrator runtime.
    pub auto_group_settings: tokio::sync::RwLock<AutoGroupSettings>,
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
    /// Serializes PUT-driven writes to the player-watch config sidecar so disk
    /// and in-memory state stay in the same order under concurrent requests.
    pub player_watch_write_lock: tokio::sync::Mutex<()>,
    /// GM alert state — zone-wide GM detection status for web dashboard.
    pub gm_alert_state: Arc<api::gm_alerts::GmAlertState>,
    /// In-memory spawn alert state for rare spawn monitoring.
    pub spawn_alerts: Arc<api::spawn_alerts::SpawnAlertState>,
    /// In-memory vendor item watch configuration and alert history.
    pub vendor_watch_state: Arc<api::vendor_watch::VendorWatchState>,
    /// Inventory-utility parity pack config for RedGuides extension mappings,
    /// rule editing, and legacy provenance reporting.
    pub inventory_utility_parity:
        tokio::sync::RwLock<textquest_common::inventory_utility::InventoryUtilityConfig>,
    /// Disk location where the inventory-utility parity config persists.
    pub inventory_utility_parity_path: PathBuf,
    /// Serializes writes to [`inventory_utility_parity_path`].
    pub inventory_utility_parity_write_lock: tokio::sync::Mutex<()>,
    /// In-memory timestamp config store per character.
    pub timestamp_configs: tokio::sync::RwLock<HashMap<String, api::TimestampConfig>>,
    /// Serializes timestamp sidecar writes so acknowledged edits persist in the
    /// same order they become visible through the API.
    pub timestamp_config_write_lock: tokio::sync::Mutex<()>,
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
    /// Side-car file where dashboard-edited auto-group profiles persist.
    pub auto_group_config_path: PathBuf,
    /// Optional static API token for protecting all `/api` endpoints.
    /// Set via `TEXTQUEST_API_TOKEN` environment variable.
    /// When `None`, API endpoints are unauthenticated (localhost-only
    /// deployment).
    pub api_token: Option<String>,
    /// Mutable runtime snapshot written by the orchestrator for live session
    /// monitoring.
    pub live_session_snapshot_path: PathBuf,
    /// Mutable runtime snapshot written by the orchestrator for admin session
    /// inventory.
    pub admin_session_snapshot_path: PathBuf,
    /// In-memory XAssist configuration per character.
    pub xassist_configs: api::xassist::XAssistConfigs,
    /// In-memory chat pattern rules engine for MQ2Events/MQ2React parity.
    pub chat_pattern_rules: tokio::sync::RwLock<ChatPatternRuleEngine>,
    /// Say detection state for /say channel pattern matching.
    pub say_detection: Option<Arc<api::say_detection::SayDetectionState>>,
    /// In-memory session control state for external SDK control endpoints.
    pub session_controls: tokio::sync::RwLock<HashMap<u32, api::control::SessionControlState>>,
    /// Auto-group formation state — MQ2AutoGroup parity.
    pub auto_group_state: Arc<api::auto_group::AutoGroupState>,
    /// Persisted extension catalog metadata, overrides, and runtime status.
    pub extension_catalog_state: Arc<api::extensions::ExtensionCatalogState>,
    /// In-memory session and group control records for the web API control
    /// surface.  Mirrors the orchestrator's per-session state and acts as the
    /// authoritative staging area for external SDK commands.
    pub session_control_state: Arc<api::session_control::SessionControlState>,
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

fn admin_session_snapshot_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/runtime/admin_sessions.json")
}

fn inventory_utility_parity_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../config/inventory-utility-parity.json")
}

fn character_config_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../config/character-configs.json")
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
    if let Ok(override_path) = std::env::var("TEXTQUEST_ALERTING_CONFIG_PATH")
        && !override_path.is_empty()
    {
        return PathBuf::from(override_path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../config/alerting.toml")
}

fn auto_group_config_path() -> PathBuf {
    textquest::auto_group::default_config_path()
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
    let contents = toml::to_string_pretty(config)
        .map_err(|error| anyhow::anyhow!("Failed to serialize alerting config: {error}"))?;
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

    let character_config_path = character_config_path();
    let auto_group_config_path = auto_group_config_path();
    // Demo-default seeding only runs when the file is absent. A present-but-
    // empty file ({}) is an explicit operator choice — restoring demo entries
    // would pollute their config on the next save.
    let character_configs = if character_config_path.exists() {
        match api::load_character_configs_from_path(&character_config_path) {
            Ok(configs) => configs,
            Err(error) => {
                tracing::error!(
                    %error,
                    path = %character_config_path.display(),
                    "Failed to load persisted character configs; falling back to demo defaults"
                );
                api::demo_character_configs()
            }
        }
    } else {
        tracing::info!(
            path = %character_config_path.display(),
            "No persisted character configs found; seeding demo defaults"
        );
        api::demo_character_configs()
    };
    let auto_group_settings = textquest::auto_group::load_settings_from_path(
        &auto_group_config_path,
    )
    .unwrap_or_else(|error| {
        tracing::warn!(
            %error,
            path = %auto_group_config_path.display(),
            "Failed to load persisted auto-group config; using defaults"
        );
        AutoGroupSettings::default()
    });

    Arc::new(AppState {
        event_tx,
        account_store: Mutex::new(accounts::AccountStore::default()),
        credential_store,
        character_configs: tokio::sync::RwLock::new(character_configs),
        auto_accept_settings: tokio::sync::RwLock::new(Default::default()),
        tradeskill_trophy_settings: tokio::sync::RwLock::new(Default::default()),
        character_config_path,
        character_config_write_lock: tokio::sync::Mutex::new(()),
        auto_group_settings: tokio::sync::RwLock::new(auto_group_settings),
        loot_state: api::loot::LootState::new_demo(),
        economy_state: api::economy::EconomyState::new_demo(),
        dashboard_state: api::dashboard::DashboardState::new_demo(),
        soul_audit: api::soul::SoulAuditState::new_demo(),
        discord_state: api::discord::DiscordState::new_demo(),
        player_watch_config: tokio::sync::RwLock::new(
            api::read_player_watch_config_from_disk().unwrap_or_else(|error| {
                tracing::warn!(%error, "Failed to load player-watch config");
                api::PlayerWatchConfig::default()
            }),
        ),
        player_watch_write_lock: tokio::sync::Mutex::new(()),
        gm_alert_state: Arc::new(api::gm_alerts::GmAlertState::default()),
        spawn_alerts: api::spawn_alerts::SpawnAlertState::new_demo(),
        vendor_watch_state: api::vendor_watch::VendorWatchState::new_from_disk_or_default(),
        inventory_utility_parity: tokio::sync::RwLock::new(
            api::inventory_utility_parity::load_config_from_disk().unwrap_or_else(|error| {
                tracing::warn!(%error, "Failed to load inventory utility parity config");
                textquest_common::inventory_utility::InventoryUtilityConfig::default()
            }),
        ),
        inventory_utility_parity_path: inventory_utility_parity_path(),
        inventory_utility_parity_write_lock: tokio::sync::Mutex::new(()),
        timestamp_configs: tokio::sync::RwLock::new(
            api::load_timestamp_configs_from_disk().unwrap_or_else(|error| {
                tracing::warn!(%error, "Failed to load timestamp configs");
                HashMap::new()
            }),
        ),
        timestamp_config_write_lock: tokio::sync::Mutex::new(()),
        kill_tracker_state: api::kill_tracker::KillTrackerState::new_empty(),
        alert_store: open_alert_store(),
        alert_config: tokio::sync::RwLock::new(load_alerting_config_from(&alerting_config_path())),
        alerting_config_path: alerting_config_path(),
        auto_group_config_path,
        api_token,
        live_session_snapshot_path: live_session_snapshot_path(),
        admin_session_snapshot_path: admin_session_snapshot_path(),
        xassist_configs: api::xassist::demo_xassist_configs(),
        chat_pattern_rules: api::chat_pattern_rules::load_rules_state(),
        say_detection: Some(Arc::new(api::say_detection::SayDetectionState::new_demo())),
        session_controls: tokio::sync::RwLock::new(HashMap::new()),
        auto_group_state: api::auto_group::AutoGroupState::new_demo(),
        extension_catalog_state: api::extensions::ExtensionCatalogState::load(
            api::extensions::extension_catalog_path(),
        ),
        session_control_state: api::session_control::SessionControlState::new(),
    })
}

#[cfg(test)]
pub(crate) fn test_app_state() -> AppState {
    let (event_tx, _) = broadcast::channel::<String>(8);
    AppState {
        event_tx,
        account_store: Mutex::new(accounts::AccountStore::default()),
        credential_store: None,
        character_configs: tokio::sync::RwLock::new(api::demo_character_configs()),
        character_config_path: std::env::temp_dir().join(format!(
            "textquest-web-test-character-configs-{}.json",
            uuid::Uuid::new_v4()
        )),
        character_config_write_lock: tokio::sync::Mutex::new(()),
        auto_accept_settings: tokio::sync::RwLock::new(Default::default()),
        tradeskill_trophy_settings: tokio::sync::RwLock::new(Default::default()),
        loot_state: api::loot::LootState::new_demo(),
        economy_state: api::economy::EconomyState::new_demo(),
        dashboard_state: api::dashboard::DashboardState::new_demo(),
        soul_audit: api::soul::SoulAuditState::new_demo(),
        discord_state: api::discord::DiscordState::new_demo(),
        player_watch_config: tokio::sync::RwLock::new(api::PlayerWatchConfig::default()),
        player_watch_write_lock: tokio::sync::Mutex::new(()),
        gm_alert_state: Arc::new(api::gm_alerts::GmAlertState::default()),
        spawn_alerts: api::spawn_alerts::SpawnAlertState::new_demo(),
        vendor_watch_state: api::vendor_watch::VendorWatchState::new_demo(),
        inventory_utility_parity: tokio::sync::RwLock::new(
            textquest_common::inventory_utility::InventoryUtilityConfig::default(),
        ),
        inventory_utility_parity_path: std::env::temp_dir().join(format!(
            "textquest-web-test-inventory-utility-parity-{}.json",
            uuid::Uuid::new_v4()
        )),
        inventory_utility_parity_write_lock: tokio::sync::Mutex::new(()),
        timestamp_configs: tokio::sync::RwLock::new(HashMap::new()),
        timestamp_config_write_lock: tokio::sync::Mutex::new(()),
        kill_tracker_state: api::kill_tracker::KillTrackerState::new_empty(),
        alert_store: AlertStore::open_memory().expect("alert store"),
        alert_config: tokio::sync::RwLock::new(AlertingConfig::default()),
        alerting_config_path: std::env::temp_dir().join(format!(
            "textquest-web-test-alerting-{}.toml",
            uuid::Uuid::new_v4()
        )),
        auto_group_config_path: std::env::temp_dir().join(format!(
            "textquest-web-test-auto-group-{}.toml",
            uuid::Uuid::new_v4()
        )),
        api_token: None,
        live_session_snapshot_path: std::env::temp_dir().join(format!(
            "textquest-web-test-live-sessions-{}.json",
            uuid::Uuid::new_v4()
        )),
        admin_session_snapshot_path: std::env::temp_dir().join(format!(
            "textquest-web-test-admin-sessions-{}.json",
            uuid::Uuid::new_v4()
        )),
        xassist_configs: api::xassist::demo_xassist_configs(),
        chat_pattern_rules: api::chat_pattern_rules::load_rules_state(),
        say_detection: Some(Arc::new(api::say_detection::SayDetectionState::new_demo())),
        session_controls: tokio::sync::RwLock::new(HashMap::new()),
        auto_group_state: api::auto_group::AutoGroupState::new_demo(),
        extension_catalog_state: api::extensions::ExtensionCatalogState::load(
            std::env::temp_dir().join(format!(
                "textquest-web-test-extension-catalog-{}.json",
                uuid::Uuid::new_v4()
            )),
        ),
        session_control_state: api::session_control::SessionControlState::new(),
    }
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
        .route(
            "/item-score",
            get(api::loot::get_item_score).put(api::loot::put_item_score),
        )
        .route(
            "/inventory-utility",
            get(api::loot::get_inventory_utility).put(api::loot::put_inventory_utility),
        )
}

fn build_api_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(api::health))
        .route("/admin/sessions", get(api::admin::list_sessions))
        .route(
            "/admin/diagnostics/{session_id}",
            get(api::admin_diagnostics::get_diagnostics),
        )
        .route("/admin/logs/{session_id}", get(api::admin_logs::tail_logs))
        .route(
            "/admin/config/audit/{session_id}",
            get(api::admin_config::audit_config),
        )
        .route("/sessions", get(api::list_sessions))
        // Session and group control — SDK-facing control surface
        .nest("/sessions", api::session_control::router())
        .nest("/accounts", accounts::router())
        .nest("/admin", api::admin::router())
        .nest("/dashboard", api::dashboard::router())
        .nest("/extensions", api::extensions::router())
        .route(
            "/box-chat/settings",
            get(api::get_box_chat_settings).put(api::put_box_chat_settings),
        )
        .route(
            "/chat-log/settings",
            get(api::chat_log::get_chat_log_settings).put(api::chat_log::put_chat_log_settings),
        )
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
            "/config/characters/{character}",
            put(api::put_character_config),
        )
        .route(
            "/config/auto-accept",
            get(api::get_auto_accept_settings).put(api::put_auto_accept_settings),
        )
        .route(
            "/config/tradeskill-trophy",
            get(api::get_tradeskill_trophy_settings).put(api::put_tradeskill_trophy_settings),
        )
        .route(
            "/tradeskill-trophy/status",
            get(api::get_tradeskill_trophy_statuses),
        )
        .route(
            "/config/discord",
            get(api::discord::get_settings).put(api::discord::put_settings),
        )
        .route(
            "/config/player-watch",
            get(api::get_player_watch_config).put(api::put_player_watch_config),
        )
        .route(
            "/config/inventory-utility-parity",
            get(api::inventory_utility_parity::get_config)
                .put(api::inventory_utility_parity::put_config),
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
        .route(
            "/vendor-watch/alerts",
            get(api::vendor_watch::list_alerts).delete(api::vendor_watch::clear_alerts),
        )
        .route("/vendor-watch/stats", get(api::vendor_watch::get_stats))
        .route(
            "/vendor-watch/config",
            get(api::vendor_watch::get_config).put(api::vendor_watch::put_config),
        )
        .route(
            "/vendor-watch/watch-list",
            get(api::vendor_watch::get_watch_list).put(api::vendor_watch::put_watch_item),
        )
        .route(
            "/vendor-watch/watch-list/{item_name}",
            delete(api::vendor_watch::delete_watch_item),
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
        .nest("/say-detection", api::say_detection::router())
        .nest("/auto-group", api::auto_group::router())
        .nest("/control", api::control::router())
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
            get(api::chat_pattern_rules::list_rules).post(api::chat_pattern_rules::create_rule),
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
            get(api::chat_pattern_rules::get_rule)
                .put(api::chat_pattern_rules::update_rule)
                .delete(api::chat_pattern_rules::delete_rule),
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
        // Admin Sessions API
        .nest("/admin/sessions", api::admin_sessions::router())
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
    api::vendor_watch::spawn_vendor_watch_loop(state.clone());
    api::dashboard::spawn_dashboard_tick_loop(state.clone());
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
            tradeskill_trophy_settings: tokio::sync::RwLock::new(Default::default()),
            character_config_path: path.with_file_name("character-configs.json"),
            character_config_write_lock: tokio::sync::Mutex::new(()),
            auto_group_settings: tokio::sync::RwLock::new(AutoGroupSettings::default()),
            loot_state: api::loot::LootState::new_demo(),
            economy_state: api::economy::EconomyState::new_demo(),
            dashboard_state: api::dashboard::DashboardState::new_demo(),
            soul_audit: api::soul::SoulAuditState::new_demo(),
            discord_state: api::discord::DiscordState::new_demo(),
            player_watch_config: tokio::sync::RwLock::new(api::PlayerWatchConfig::default()),
            player_watch_write_lock: tokio::sync::Mutex::new(()),
            gm_alert_state: Arc::new(api::gm_alerts::GmAlertState::default()),
            spawn_alerts: api::spawn_alerts::SpawnAlertState::new_demo(),
            vendor_watch_state: api::vendor_watch::VendorWatchState::new_demo(),
            inventory_utility_parity: tokio::sync::RwLock::new(
                textquest_common::inventory_utility::InventoryUtilityConfig::default(),
            ),
            inventory_utility_parity_path: path.with_file_name("inventory-utility-parity.json"),
            inventory_utility_parity_write_lock: tokio::sync::Mutex::new(()),
            timestamp_configs: tokio::sync::RwLock::new(HashMap::new()),
            timestamp_config_write_lock: tokio::sync::Mutex::new(()),
            kill_tracker_state: api::kill_tracker::KillTrackerState::new_empty(),
            alert_store: AlertStore::open_memory().expect("alert store"),
            alert_config: tokio::sync::RwLock::new(AlertingConfig::default()),
            alerting_config_path: std::env::temp_dir().join(format!(
                "textquest-main-test-alerting-{}.toml",
                uuid::Uuid::new_v4()
            )),
            auto_group_config_path: path.with_file_name("auto-group.toml"),
            api_token: None, // No auth in tests — auth middleware is a no-op when None
            live_session_snapshot_path: path.with_file_name("live_sessions.json"),
            admin_session_snapshot_path: path.with_file_name("admin_sessions.json"),
            xassist_configs: api::xassist::demo_xassist_configs(),
            chat_pattern_rules: api::chat_pattern_rules::load_rules_state(),
            say_detection: Some(Arc::new(api::say_detection::SayDetectionState::new_demo())),
            session_controls: tokio::sync::RwLock::new(HashMap::new()),
            auto_group_state: api::auto_group::AutoGroupState::new_demo(),
            extension_catalog_state: api::extensions::ExtensionCatalogState::load(
                std::env::temp_dir().join(format!(
                    "textquest-main-test-extension-catalog-{}.json",
                    uuid::Uuid::new_v4()
                )),
            ),
            session_control_state: api::session_control::SessionControlState::new(),
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
    async fn known_unimplemented_and_live_config_routes_return_expected_statuses() {
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
            app.clone(),
            Request::builder()
                .uri("/api/config/characters")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.is_array());

        let (status, _body) = json_response(
            app.clone(),
            Request::builder()
                .uri("/api/dashboard")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, _body) = json_response(
            app.clone(),
            Request::builder()
                .uri("/api/config/discord")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, _body) = json_response(
            app.clone(),
            Request::builder()
                .uri("/api/box-chat/settings")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, _body) = json_response(
            app,
            Request::builder()
                .uri("/api/chat-log/settings")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn admin_sessions_endpoint_returns_empty_array_when_inventory_missing() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let app = build_app(test_state_with_credentials(
            &tempdir.path().join("creds.db"),
        ));

        let (status, body) = json_response(
            app,
            Request::builder()
                .uri("/api/admin/sessions")
                .body(Body::empty())
                .expect("request"),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!([]));
    }

    #[tokio::test]
    async fn admin_sessions_endpoint_returns_persisted_inventory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let state = test_state_with_credentials(&tempdir.path().join("creds.db"));
        std::fs::write(
            &state.admin_session_snapshot_path,
            serde_json::to_vec(&vec![json!({
                "session_id": 4242,
                "character_name": "Cleric42",
                "class_name": "Cleric",
                "group_id": 2,
                "routing_scope": {
                    "kind": "group",
                    "label": "G2",
                    "group_id": 2,
                    "toon_name": null
                },
                "lifecycle_state": "paused"
            })])
            .expect("admin snapshot json"),
        )
        .expect("write admin snapshot");

        let app = build_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .uri("/api/admin/sessions")
                .body(Body::empty())
                .expect("request"),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let sessions = body.as_array().expect("sessions array");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0]["session_id"], 4242);
        assert_eq!(sessions[0]["character_name"], "Cleric42");
        assert_eq!(sessions[0]["class_name"], "Cleric");
        assert_eq!(sessions[0]["group_id"], 2);
        assert_eq!(sessions[0]["routing_scope"]["kind"], "group");
        assert_eq!(sessions[0]["lifecycle_state"], "paused");
    }

    #[tokio::test]
    async fn extension_catalog_routes_expose_supported_entries() {
        let app = build_app(Arc::new(test_app_state()));
        let (status, body) = json_response(
            app,
            Request::builder()
                .uri("/api/extensions/catalog")
                .body(Body::empty())
                .expect("request"),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        let entries = body.as_array().expect("catalog array");
        assert!(
            entries.iter().any(|entry| entry["id"] == "mq2eqbc"),
            "expected box-chat parity entry in extension catalog"
        );
        assert!(
            entries.iter().any(|entry| entry["id"] == "mq2autoaccept"),
            "expected auto-accept parity entry in extension catalog"
        );
    }

    #[tokio::test]
    async fn extension_catalog_scope_overrides_round_trip() {
        let app = build_app(Arc::new(test_app_state()));

        let (status, saved) = json_response(
            app.clone(),
            Request::builder()
                .method("PUT")
                .uri("/api/extensions/catalog/mq2autoaccept/scopes/character/Frostreaver")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "settings": {
                            "enabled": true,
                            "acceptGroupInvites": true,
                            "acceptTrades": true,
                            "acceptTaskAdds": true,
                            "acceptDzAdds": true,
                            "acceptTranslocates": true,
                            "acceptAnchors": true,
                            "trustMode": "trust_list",
                            "trustedPlayers": ["Noxus"]
                        }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(saved["scope"]["kind"], "character");
        assert_eq!(saved["scope"]["id"], "Frostreaver");

        let (status, entry) = json_response(
            app.clone(),
            Request::builder()
                .uri("/api/extensions/catalog/mq2autoaccept")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(entry["id"], "mq2autoaccept");
        assert_eq!(
            entry["overrides"].as_array().expect("override array").len(),
            1
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/extensions/catalog/mq2autoaccept/scopes/character/Frostreaver")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn extension_catalog_rejects_invalid_settings_payloads() {
        let app = build_app(Arc::new(test_app_state()));

        let (status, body) = json_response(
            app,
            Request::builder()
                .method("PUT")
                .uri("/api/extensions/catalog/mq2eqbc/settings")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "settings": {
                            "enabled": true,
                            "host": "127.0.0.1",
                            "port": 2112,
                            "autoConnect": false,
                            "bogusField": "unexpected"
                        }
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "Unsupported setting `bogusField`");
    }

    #[tokio::test]
    async fn extension_catalog_write_routes_reject_untrusted_origin() {
        let app = build_app(Arc::new(test_app_state()));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/extensions/catalog/mq2eqbc/settings")
                    .header("origin", "https://evil.invalid")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "settings": {
                                "enabled": true,
                                "host": "127.0.0.1",
                                "port": 2112,
                                "autoConnect": true
                            }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/extensions/catalog/mq2autoaccept/scopes/character/Frostreaver")
                    .header("origin", "https://evil.invalid")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "settings": {
                                "enabled": true,
                                "acceptGroupInvites": true,
                                "acceptTrades": true,
                                "acceptTaskAdds": true,
                                "acceptDzAdds": true,
                                "acceptTranslocates": true,
                                "acceptAnchors": true,
                                "trustMode": "trust_all",
                                "trustedPlayers": []
                            }
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/extensions/catalog/mq2autoaccept/scopes/character/Frostreaver")
                    .header("origin", "https://evil.invalid")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/extensions/catalog/mq2eqbc/runtime")
                    .header("origin", "https://evil.invalid")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "enabled": true
                        })
                        .to_string(),
                    ))
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
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
