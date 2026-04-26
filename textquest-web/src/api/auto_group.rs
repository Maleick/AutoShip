//! Auto-group formation API — MQ2AutoGroup parity.
//!
//! Endpoints:
//!   GET  /auto-group          — return current config + phase status
//!   PUT  /auto-group          — update config (rejected while active)
//!   POST /auto-group/start    — begin group formation
//!   POST /auto-group/reset    — abort and reset to idle

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::AppState;

// ── Shared types ─────────────────────────────────────────────────────────────

/// EQ group role numbers as used by `/grouprole set <name> <role>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GroupRole {
    #[default]
    None,
    MainTank,
    MainAssist,
    Puller,
    MarkNpc,
    MasterLooter,
}

impl GroupRole {
    pub fn role_id(self) -> u8 {
        match self {
            GroupRole::None => 0,
            GroupRole::MainTank => 1,
            GroupRole::MainAssist => 2,
            GroupRole::Puller => 3,
            GroupRole::MarkNpc => 4,
            GroupRole::MasterLooter => 5,
        }
    }
}

impl std::fmt::Display for GroupRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GroupRole::None => write!(f, "None"),
            GroupRole::MainTank => write!(f, "MainTank"),
            GroupRole::MainAssist => write!(f, "MainAssist"),
            GroupRole::Puller => write!(f, "Puller"),
            GroupRole::MarkNpc => write!(f, "MarkNPC"),
            GroupRole::MasterLooter => write!(f, "MasterLooter"),
        }
    }
}

/// A single member slot in an auto-group configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoGroupMember {
    pub name: String,
    #[serde(default)]
    pub role: GroupRole,
}

/// Full auto-group configuration stored by the web API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutoGroupConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub members: Vec<AutoGroupMember>,
    #[serde(default)]
    pub completion_command: Option<String>,
    #[serde(default = "default_max_retries")]
    pub max_retries: u8,
    #[serde(default = "default_invite_interval_ticks")]
    pub invite_interval_ticks: u64,
    #[serde(default = "default_member_wait_ticks")]
    pub member_wait_ticks: u64,
}

fn default_enabled() -> bool {
    true
}
fn default_max_retries() -> u8 {
    3
}
fn default_invite_interval_ticks() -> u64 {
    10
}
fn default_member_wait_ticks() -> u64 {
    300
}

impl Default for AutoGroupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            members: Vec::new(),
            completion_command: None,
            max_retries: default_max_retries(),
            invite_interval_ticks: default_invite_interval_ticks(),
            member_wait_ticks: default_member_wait_ticks(),
        }
    }
}

/// Status snapshot returned by the GET endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoGroupStatus {
    pub phase: String,
    pub config: AutoGroupConfig,
}

/// Thread-safe shared state owned by `AppState`.
pub struct AutoGroupState {
    pub config: RwLock<AutoGroupConfig>,
    pub phase_label: std::sync::Mutex<String>,
}

impl AutoGroupState {
    #[must_use]
    pub fn new_demo() -> Arc<Self> {
        Arc::new(Self {
            config: RwLock::new(AutoGroupConfig {
                enabled: false,
                members: vec![
                    AutoGroupMember {
                        name: "Frostreaver".into(),
                        role: GroupRole::MainTank,
                    },
                    AutoGroupMember {
                        name: "Aelrindel".into(),
                        role: GroupRole::None,
                    },
                    AutoGroupMember {
                        name: "Grok".into(),
                        role: GroupRole::None,
                    },
                ],
                completion_command: None,
                max_retries: default_max_retries(),
                invite_interval_ticks: default_invite_interval_ticks(),
                member_wait_ticks: default_member_wait_ticks(),
            }),
            phase_label: std::sync::Mutex::new("idle".into()),
        })
    }

    pub async fn snapshot(&self) -> AutoGroupStatus {
        AutoGroupStatus {
            phase: self
                .phase_label
                .lock()
                .map(|g| g.clone())
                .unwrap_or_else(|_| "unknown".into()),
            config: self.config.read().await.clone(),
        }
    }
}

impl Default for AutoGroupState {
    fn default() -> Self {
        Self {
            config: RwLock::new(AutoGroupConfig::default()),
            phase_label: std::sync::Mutex::new("idle".into()),
        }
    }
}

// ── Error helpers ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
        .into_response()
}

// ── Request / response types ──────────────────────────────────────────────────

/// PUT body for updating the auto-group configuration.
#[derive(Debug, Deserialize)]
pub struct PutAutoGroupConfig {
    pub enabled: Option<bool>,
    pub members: Option<Vec<PutAutoGroupMember>>,
    pub completion_command: Option<Option<String>>,
    pub max_retries: Option<u8>,
    pub invite_interval_ticks: Option<u64>,
    pub member_wait_ticks: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct PutAutoGroupMember {
    pub name: String,
    #[serde(default)]
    pub role: GroupRole,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /auto-group — return current config and phase.
pub async fn get_auto_group(State(state): State<Arc<AppState>>) -> Response {
    let snapshot = state.auto_group_state.snapshot().await;
    (StatusCode::OK, Json(snapshot)).into_response()
}

/// PUT /auto-group — replace config (only allowed while idle/done).
pub async fn put_auto_group(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<PutAutoGroupConfig>,
) -> Response {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let phase = state
        .auto_group_state
        .phase_label
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| "unknown".into());

    if phase != "idle" && phase != "done" && phase != "unknown" {
        return json_error(
            StatusCode::CONFLICT,
            format!(
                "Cannot update config while auto-group is active (phase: {phase}). \
                 Reset first."
            ),
        );
    }

    let mut cfg = state.auto_group_state.config.write().await;

    if let Some(enabled) = body.enabled {
        cfg.enabled = enabled;
    }
    if let Some(members) = body.members {
        cfg.members = members
            .into_iter()
            .map(|m| AutoGroupMember {
                name: m.name,
                role: m.role,
            })
            .collect();
    }
    if let Some(cc) = body.completion_command {
        cfg.completion_command = cc;
    }
    if let Some(v) = body.max_retries {
        cfg.max_retries = v;
    }
    if let Some(v) = body.invite_interval_ticks {
        cfg.invite_interval_ticks = v;
    }
    if let Some(v) = body.member_wait_ticks {
        cfg.member_wait_ticks = v;
    }

    let snapshot = AutoGroupStatus {
        phase,
        config: cfg.clone(),
    };
    (StatusCode::OK, Json(snapshot)).into_response()
}

/// POST /auto-group/start — begin group formation (triggers phase → inviting).
///
/// In the web-only (no live orchestrator) context this just updates the phase
/// label so the dashboard can display the intent. When the real orchestrator
/// is connected it polls the phase label and initiates formation.
pub async fn post_auto_group_start(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let phase = state
        .auto_group_state
        .phase_label
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| "unknown".into());

    if phase != "idle" && phase != "done" && phase != "unknown" {
        return json_error(
            StatusCode::CONFLICT,
            format!("Auto-group already active (phase: {phase}). Reset first."),
        );
    }

    {
        let cfg = state.auto_group_state.config.read().await;
        if !cfg.enabled {
            return json_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Auto-group is disabled. Enable it in the config first.",
            );
        }
        if cfg.members.is_empty() {
            return json_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "No members configured. Add at least one member before starting.",
            );
        }
    }

    if let Ok(mut label) = state.auto_group_state.phase_label.lock() {
        *label = "inviting".into();
    }

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "status": "started", "phase": "inviting" })),
    )
        .into_response()
}

/// POST /auto-group/reset — abort formation and return to idle.
pub async fn post_auto_group_reset(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    if let Ok(mut label) = state.auto_group_state.phase_label.lock() {
        *label = "idle".into();
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "reset", "phase": "idle" })),
    )
        .into_response()
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_auto_group).put(put_auto_group))
        .route("/start", post(post_auto_group_start))
        .route("/reset", post(post_auto_group_reset))
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
    use std::sync::Mutex;
    use tokio::sync::{RwLock, broadcast};
    use tower::ServiceExt;

    fn make_test_state() -> Arc<AppState> {
        let (event_tx, _) = broadcast::channel::<String>(8);
        Arc::new(AppState {
            event_tx,
            account_store: Mutex::new(crate::accounts::AccountStore::default()),
            credential_store: None,
            character_configs: RwLock::new(crate::api::demo_character_configs()),
            character_config_path: std::env::temp_dir().join("tq-test-cfg.json"),
            character_config_write_lock: tokio::sync::Mutex::new(()),
            chat_log_write_lock: tokio::sync::Mutex::new(()),
            loot_state: crate::api::loot::LootState::new_demo(),
            economy_state: crate::api::economy::EconomyState::new_demo(),
            dashboard_state: crate::api::dashboard::DashboardState::new_demo(),
            soul_audit: crate::api::soul::SoulAuditState::new_demo(),
            discord_state: crate::api::discord::DiscordState::new_demo(),
            player_watch_config: RwLock::new(crate::api::PlayerWatchConfig::default()),
            player_watch_write_lock: tokio::sync::Mutex::new(()),
            gm_alert_state: Arc::new(crate::api::gm_alerts::GmAlertState::default()),
            spawn_alerts: crate::api::spawn_alerts::SpawnAlertState::new_demo(),
            vendor_watch_state: crate::api::vendor_watch::VendorWatchState::new_demo(),
            timestamp_configs: RwLock::new(std::collections::HashMap::new()),
            timestamp_config_write_lock: tokio::sync::Mutex::new(()),
            kill_tracker_state: crate::api::kill_tracker::KillTrackerState::new_demo(),
            alert_store: textquest::alerts::AlertStore::open_memory().expect("alert store"),
            alert_config: RwLock::new(textquest::config::AlertingConfig::default()),
            alerting_config_path: std::env::temp_dir()
                .join(format!("tq-test-alerting-{}.toml", uuid::Uuid::new_v4())),
            api_token: None,
            auth_disabled: true, // Tests bypass auth
            live_session_snapshot_path: std::env::temp_dir().join("tq-test-sessions.json"),
            admin_session_snapshot_path: std::env::temp_dir().join("tq-test-admin-sessions.json"),
            xassist_configs: crate::api::xassist::demo_xassist_configs(),
            chat_pattern_rules: crate::api::chat_pattern_rules::load_rules_state(),
            say_detection: Some(Arc::new(
                crate::api::say_detection::SayDetectionState::new_demo(),
            )),
            session_controls: RwLock::new(std::collections::HashMap::new()),
            auto_accept_settings: tokio::sync::RwLock::new(Default::default()),
            tradeskill_trophy_settings: tokio::sync::RwLock::new(Default::default()),
            auto_group_settings: tokio::sync::RwLock::new(
                textquest_common::auto_group::AutoGroupSettings::default(),
            ),
            auto_group_config_path: std::env::temp_dir().join("tq-test-auto-group.json"),
            auto_group_state: AutoGroupState::new_demo(),
            inventory_utility_parity: tokio::sync::RwLock::new(
                textquest_common::inventory_utility::InventoryUtilityConfig::default(),
            ),
            inventory_utility_parity_path: std::env::temp_dir()
                .join("tq-test-inventory-utility.json"),
            inventory_utility_parity_write_lock: tokio::sync::Mutex::new(()),
            extension_catalog_state: crate::api::extensions::ExtensionCatalogState::load(
                std::env::temp_dir().join(format!(
                    "textquest-auto-group-test-extension-catalog-{}.json",
                    uuid::Uuid::new_v4()
                )),
            ),
            session_logs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            session_control_state: crate::api::session_control::SessionControlState::new(),
        })
    }

    async fn json_response(router: Router, req: Request<Body>) -> (StatusCode, Value) {
        let response = router.oneshot(req).await.expect("request failed");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body collect")
            .to_bytes();
        let value = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).expect("valid json")
        };
        (status, value)
    }

    fn make_router(state: Arc<AppState>) -> Router {
        router().with_state(state)
    }

    #[tokio::test]
    async fn get_returns_config_and_phase() {
        let state = make_test_state();
        let app = make_router(state);

        let (status, body) = json_response(
            app,
            Request::builder().uri("/").body(Body::empty()).unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["config"].is_object());
        assert!(body["phase"].is_string());
    }

    #[tokio::test]
    async fn put_updates_config_while_idle() {
        let state = make_test_state();
        let app = make_router(state);

        let payload = json!({
            "enabled": true,
            "members": [
                { "name": "Warrior1", "role": "main_tank" },
                { "name": "Cleric1", "role": "none" }
            ],
            "completion_command": "/say formed",
            "max_retries": 5
        });

        let (status, body) = json_response(
            app,
            Request::builder()
                .method("PUT")
                .uri("/")
                .header("content-type", "application/json")
                .header(
                    axum::http::header::ORIGIN,
                    crate::api::loot::TRUSTED_ORIGINS[0],
                )
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::OK, "body: {body}");
        assert_eq!(body["config"]["members"][0]["name"], "Warrior1");
        assert_eq!(body["config"]["completion_command"], "/say formed");
        assert_eq!(body["config"]["max_retries"], 5);
    }

    #[tokio::test]
    async fn start_transitions_to_inviting() {
        let state = make_test_state();
        // Set enabled + members first
        {
            let mut cfg = state.auto_group_state.config.write().await;
            cfg.enabled = true;
            cfg.members = vec![AutoGroupMember {
                name: "Alice".into(),
                role: GroupRole::None,
            }];
        }
        let app = make_router(state);

        let (status, body) = json_response(
            app,
            Request::builder()
                .method("POST")
                .uri("/start")
                .header(
                    axum::http::header::ORIGIN,
                    crate::api::loot::TRUSTED_ORIGINS[0],
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::ACCEPTED, "body: {body}");
        assert_eq!(body["phase"], "inviting");
    }

    #[tokio::test]
    async fn start_rejected_when_disabled() {
        let state = make_test_state();
        {
            let mut cfg = state.auto_group_state.config.write().await;
            cfg.enabled = false;
        }
        let app = make_router(state);

        let (status, _) = json_response(
            app,
            Request::builder()
                .method("POST")
                .uri("/start")
                .header(
                    axum::http::header::ORIGIN,
                    crate::api::loot::TRUSTED_ORIGINS[0],
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn start_rejected_when_no_members() {
        let state = make_test_state();
        {
            let mut cfg = state.auto_group_state.config.write().await;
            cfg.enabled = true;
            cfg.members = vec![];
        }
        let app = make_router(state);

        let (status, _) = json_response(
            app,
            Request::builder()
                .method("POST")
                .uri("/start")
                .header(
                    axum::http::header::ORIGIN,
                    crate::api::loot::TRUSTED_ORIGINS[0],
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn reset_returns_idle() {
        let state = make_test_state();
        // Manually set to active phase
        {
            let mut label = state.auto_group_state.phase_label.lock().unwrap();
            *label = "inviting".into();
        }
        let app = make_router(state);

        let (status, body) = json_response(
            app,
            Request::builder()
                .method("POST")
                .uri("/reset")
                .header(
                    axum::http::header::ORIGIN,
                    crate::api::loot::TRUSTED_ORIGINS[0],
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["phase"], "idle");
    }

    #[tokio::test]
    async fn put_rejected_while_active() {
        let state = make_test_state();
        {
            let mut label = state.auto_group_state.phase_label.lock().unwrap();
            *label = "inviting".into();
        }
        let app = make_router(state);

        let (status, _) = json_response(
            app,
            Request::builder()
                .method("PUT")
                .uri("/")
                .header("content-type", "application/json")
                .header(
                    axum::http::header::ORIGIN,
                    crate::api::loot::TRUSTED_ORIGINS[0],
                )
                .body(Body::from(json!({ "enabled": false }).to_string()))
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn put_rejected_for_untrusted_origin() {
        let state = make_test_state();
        let app = make_router(state);

        let (status, _) = json_response(
            app,
            Request::builder()
                .method("PUT")
                .uri("/")
                .header("content-type", "application/json")
                .header(axum::http::header::ORIGIN, "https://evil.example")
                .body(Body::from(json!({ "enabled": false }).to_string()))
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn start_rejected_for_untrusted_origin() {
        let state = make_test_state();
        let app = make_router(state);

        let (status, _) = json_response(
            app,
            Request::builder()
                .method("POST")
                .uri("/start")
                .header(axum::http::header::ORIGIN, "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn reset_rejected_for_untrusted_origin() {
        let state = make_test_state();
        let app = make_router(state);

        let (status, _) = json_response(
            app,
            Request::builder()
                .method("POST")
                .uri("/reset")
                .header(axum::http::header::ORIGIN, "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::FORBIDDEN);
    }
}
