//! Session and group control endpoints for the web API.
//!
//! These routes expose the same lifecycle and routing-scope transitions that
//! the TUI operator panel and internal IPC layer use, so external SDKs can
//! drive the orchestrator over HTTP.
//!
//! ## Command execution model
//!
//! All mutating endpoints are **fire-and-forget**: the handler updates the
//! in-memory control record and returns `200 OK` immediately.  There is no
//! live IPC connection to running EQ clients in this build — the state here
//! acts as the authoritative staging area that a future orchestrator watcher
//! will read from.  This behavior is documented explicitly so SDK authors know
//! not to expect synchronous confirmation from the game client.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};

use crate::AppState;

use super::json_error;

// ─── State ───────────────────────────────────────────────────────────────────

/// Operational state of a managed EQ session, mirroring
/// `textquest::orchestrator::session_control::SessionState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Active,
    Paused,
    Error,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Routing scope for a session's outbound commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RoutingScope {
    /// Commands broadcast to all sessions.
    AllSession,
    /// Commands scoped to a specific group.
    Group { group_id: u8, label: String },
}

/// Per-session control record maintained by the web API layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionControlRecord {
    /// Session identifier (typically the EQ client process ID).
    pub session_id: u32,
    /// Group assignment (1-based; 0 = ungrouped / AllSession scope).
    pub group_id: u8,
    /// Current routing scope.
    pub routing_scope: RoutingScope,
    /// Operational state.
    pub state: SessionState,
}

impl SessionControlRecord {
    fn new(session_id: u32) -> Self {
        Self {
            session_id,
            group_id: 0,
            routing_scope: RoutingScope::AllSession,
            state: SessionState::Active,
        }
    }
}

/// In-memory store of per-session control records.
pub struct SessionControlState {
    pub records: Mutex<HashMap<u32, SessionControlRecord>>,
    max_records: usize,
}

impl SessionControlState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::with_max_records(10_000))
    }

    fn with_max_records(max_records: usize) -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            max_records,
        }
    }
}

fn get_or_create_record(
    records: &mut HashMap<u32, SessionControlRecord>,
    session_id: u32,
    max_records: usize,
) -> Result<&mut SessionControlRecord, &'static str> {
    if records.contains_key(&session_id) {
        return records
            .get_mut(&session_id)
            .ok_or("failed to load existing session control record");
    }
    if records.len() >= max_records {
        return Err("session control capacity reached");
    }
    Ok(records
        .entry(session_id)
        .or_insert_with(|| SessionControlRecord::new(session_id)))
}

// ─── Request / response types ─────────────────────────────────────────────────

/// Response body for session control queries and mutations.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionControlResponse {
    pub session_id: u32,
    pub state: SessionState,
    pub group_id: u8,
    pub routing_scope: RoutingScope,
    /// Human-readable description of the operation outcome.
    pub message: String,
}

/// Request body for `PUT /api/sessions/:id/group`.
#[derive(Debug, Deserialize)]
pub struct SetGroupRequest {
    /// Target group ID (1-based; 0 = ungrouped / AllSession scope).
    pub group_id: u8,
}

/// Request body for `POST /api/sessions/:id/command`.
#[derive(Debug, Deserialize)]
pub struct SlashCommandRequest {
    /// Slash command text without the leading `/`.  Must not be empty.
    ///
    /// Example: `"who all"`, `"loc"`, `"say hello"`
    pub command: String,
    /// Optional routing scope override for this single command.
    /// When omitted the session's current routing scope is used.
    pub scope: Option<CommandScope>,
}

/// Scope override for a single slash-command relay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandScope {
    /// Relay only to the addressed session.
    Single,
    /// Relay to all sessions in the same group.
    Group,
    /// Relay to every active session.
    All,
}

/// Response for `POST /api/sessions/:id/command`.
#[derive(Debug, Serialize)]
pub struct SlashCommandResponse {
    pub session_id: u32,
    pub command: String,
    pub scope_used: String,
    /// Whether the command was accepted for dispatch.
    ///
    /// `false` when the session is paused and the command was dropped.
    pub accepted: bool,
    /// Human-readable note on acceptance or rejection reason.
    pub message: String,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn scope_label(scope: &RoutingScope) -> String {
    match scope {
        RoutingScope::AllSession => "all_session".to_string(),
        RoutingScope::Group { label, .. } => label.clone(),
    }
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// `GET /api/sessions/:id/control` — fetch the current control record for a
/// session, creating a default `Active` record on first access.
pub async fn get_session_control(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<u32>,
) -> impl IntoResponse {
    if session_id == 0 {
        return json_error(StatusCode::BAD_REQUEST, "session_id must be non-zero").into_response();
    }

    let record = {
        let mut records = state
            .session_control_state
            .records
            .lock()
            .expect("session_control_state lock poisoned");
        match get_or_create_record(
            &mut records,
            session_id,
            state.session_control_state.max_records,
        ) {
            Ok(record) => record.clone(),
            Err(_) => {
                return json_error(
                    StatusCode::INSUFFICIENT_STORAGE,
                    "session control capacity reached",
                )
                .into_response();
            }
        }
    };

    (
        StatusCode::OK,
        Json(SessionControlResponse {
            session_id: record.session_id,
            state: record.state,
            group_id: record.group_id,
            routing_scope: record.routing_scope,
            message: "Session control record retrieved".to_string(),
        }),
    )
        .into_response()
}

/// `PUT /api/sessions/:id/pause` — pause an active session.
///
/// Returns `200 OK` even when the session was already paused (idempotent).
///
/// **Fire-and-forget**: the in-memory record is updated immediately.  Live EQ
/// clients are not notified in this build.
pub async fn pause_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<u32>,
) -> impl IntoResponse {
    if session_id == 0 {
        return json_error(StatusCode::BAD_REQUEST, "session_id must be non-zero").into_response();
    }

    let (record, was_active) = {
        let mut records = state
            .session_control_state
            .records
            .lock()
            .expect("session_control_state lock poisoned");
        let record = match get_or_create_record(
            &mut records,
            session_id,
            state.session_control_state.max_records,
        ) {
            Ok(record) => record,
            Err(_) => {
                return json_error(
                    StatusCode::INSUFFICIENT_STORAGE,
                    "session control capacity reached",
                )
                .into_response();
            }
        };
        let was_active = record.state == SessionState::Active;
        if was_active {
            record.state = SessionState::Paused;
        }
        (record.clone(), was_active)
    };

    let message = if was_active {
        "Session paused"
    } else {
        "Session was already paused"
    };

    (
        StatusCode::OK,
        Json(SessionControlResponse {
            session_id: record.session_id,
            state: record.state,
            group_id: record.group_id,
            routing_scope: record.routing_scope,
            message: message.to_string(),
        }),
    )
        .into_response()
}

/// `PUT /api/sessions/:id/resume` — resume a paused session.
///
/// Returns `200 OK` even when the session was already active (idempotent).
///
/// **Fire-and-forget**: the in-memory record is updated immediately.  Live EQ
/// clients are not notified in this build.
pub async fn resume_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<u32>,
) -> impl IntoResponse {
    if session_id == 0 {
        return json_error(StatusCode::BAD_REQUEST, "session_id must be non-zero").into_response();
    }

    let (record, was_paused) = {
        let mut records = state
            .session_control_state
            .records
            .lock()
            .expect("session_control_state lock poisoned");
        let record = match get_or_create_record(
            &mut records,
            session_id,
            state.session_control_state.max_records,
        ) {
            Ok(record) => record,
            Err(_) => {
                return json_error(
                    StatusCode::INSUFFICIENT_STORAGE,
                    "session control capacity reached",
                )
                .into_response();
            }
        };
        let was_paused = record.state == SessionState::Paused;
        if was_paused {
            record.state = SessionState::Active;
        }
        (record.clone(), was_paused)
    };

    let message = if was_paused {
        "Session resumed"
    } else {
        "Session was already active"
    };

    (
        StatusCode::OK,
        Json(SessionControlResponse {
            session_id: record.session_id,
            state: record.state,
            group_id: record.group_id,
            routing_scope: record.routing_scope,
            message: message.to_string(),
        }),
    )
        .into_response()
}

/// `PUT /api/sessions/:id/group` — assign a session to a group or remove it
/// from group routing.
///
/// A `group_id` of `0` removes the session from any group and resets its
/// routing scope to `AllSession`.
///
/// **Fire-and-forget**: the in-memory record is updated immediately.  Live EQ
/// clients are not notified in this build.
pub async fn set_session_group(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<u32>,
    Json(body): Json<SetGroupRequest>,
) -> impl IntoResponse {
    if session_id == 0 {
        return json_error(StatusCode::BAD_REQUEST, "session_id must be non-zero").into_response();
    }

    let record = {
        let mut records = state
            .session_control_state
            .records
            .lock()
            .expect("session_control_state lock poisoned");
        let record = match get_or_create_record(
            &mut records,
            session_id,
            state.session_control_state.max_records,
        ) {
            Ok(record) => record,
            Err(_) => {
                return json_error(
                    StatusCode::INSUFFICIENT_STORAGE,
                    "session control capacity reached",
                )
                .into_response();
            }
        };
        record.group_id = body.group_id;
        record.routing_scope = if body.group_id == 0 {
            RoutingScope::AllSession
        } else {
            RoutingScope::Group {
                group_id: body.group_id,
                label: format!("G{}", body.group_id),
            }
        };
        record.clone()
    };

    let message = if body.group_id == 0 {
        "Session removed from group; routing scope reset to AllSession".to_string()
    } else {
        format!("Session assigned to group {}", body.group_id)
    };

    (
        StatusCode::OK,
        Json(SessionControlResponse {
            session_id: record.session_id,
            state: record.state,
            group_id: record.group_id,
            routing_scope: record.routing_scope,
            message,
        }),
    )
        .into_response()
}

/// `PUT /api/sessions/:id/broadcast-all` — set a session's routing scope to
/// `AllSession` so it receives every broadcast command.
///
/// **Fire-and-forget**: the in-memory record is updated immediately.  Live EQ
/// clients are not notified in this build.
pub async fn set_broadcast_all(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<u32>,
) -> impl IntoResponse {
    if session_id == 0 {
        return json_error(StatusCode::BAD_REQUEST, "session_id must be non-zero").into_response();
    }

    let record = {
        let mut records = state
            .session_control_state
            .records
            .lock()
            .expect("session_control_state lock poisoned");
        let record = match get_or_create_record(
            &mut records,
            session_id,
            state.session_control_state.max_records,
        ) {
            Ok(record) => record,
            Err(_) => {
                return json_error(
                    StatusCode::INSUFFICIENT_STORAGE,
                    "session control capacity reached",
                )
                .into_response();
            }
        };
        record.routing_scope = RoutingScope::AllSession;
        record.clone()
    };

    (
        StatusCode::OK,
        Json(SessionControlResponse {
            session_id: record.session_id,
            state: record.state,
            group_id: record.group_id,
            routing_scope: record.routing_scope,
            message: "Routing scope set to AllSession".to_string(),
        }),
    )
        .into_response()
}

/// `POST /api/sessions/:id/command` — relay a slash command to an EQ session.
///
/// ## Semantics
///
/// This endpoint is **fire-and-forget**.  The handler validates the command,
/// checks the session's operational state, and returns immediately.
///
/// - If the session is `Paused`, the command is **dropped** and
///   `accepted: false` is returned with `200 OK`.  The caller must not retry
///   until the session is resumed.
/// - If the session is `Active`, `accepted: true` is returned.  In a live
///   deployment the command would be forwarded to the game client over IPC; in
///   this build it is logged only.
///
/// The `scope` field in the request overrides the session's current routing
/// scope for this single command.  When omitted the session's stored scope is
/// used.
pub async fn relay_command(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<u32>,
    Json(body): Json<SlashCommandRequest>,
) -> impl IntoResponse {
    if session_id == 0 {
        return json_error(StatusCode::BAD_REQUEST, "session_id must be non-zero").into_response();
    }

    let command = body.command.trim().to_string();
    if command.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "command must not be empty").into_response();
    }

    let record = {
        let mut records = state
            .session_control_state
            .records
            .lock()
            .expect("session_control_state lock poisoned");
        match get_or_create_record(
            &mut records,
            session_id,
            state.session_control_state.max_records,
        ) {
            Ok(record) => record.clone(),
            Err(_) => {
                return json_error(
                    StatusCode::INSUFFICIENT_STORAGE,
                    "session control capacity reached",
                )
                .into_response();
            }
        }
    };

    let scope_used = match &body.scope {
        Some(CommandScope::Single) => "single".to_string(),
        Some(CommandScope::Group) => "group".to_string(),
        Some(CommandScope::All) => "all".to_string(),
        None => scope_label(&record.routing_scope),
    };

    let (accepted, message) = match record.state {
        SessionState::Active => {
            tracing::info!(
                session_id,
                command = %command,
                scope = %scope_used,
                "Slash command relay accepted"
            );
            (true, "Command accepted for dispatch".to_string())
        }
        SessionState::Paused => {
            tracing::debug!(
                session_id,
                command = %command,
                "Slash command dropped — session is paused"
            );
            (false, "Command dropped: session is paused".to_string())
        }
        SessionState::Error => {
            tracing::warn!(
                session_id,
                command = %command,
                "Slash command dropped — session is in error state"
            );
            (
                false,
                "Command dropped: session is in error state".to_string(),
            )
        }
    };

    (
        StatusCode::OK,
        Json(SlashCommandResponse {
            session_id,
            command,
            scope_used,
            accepted,
            message,
        }),
    )
        .into_response()
}

/// `GET /api/sessions/control` — list all session control records.
pub async fn list_session_controls(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let records: Vec<SessionControlResponse> = {
        let records = state
            .session_control_state
            .records
            .lock()
            .expect("session_control_state lock poisoned");
        let mut list: Vec<_> = records
            .values()
            .map(|r| SessionControlResponse {
                session_id: r.session_id,
                state: r.state,
                group_id: r.group_id,
                routing_scope: r.routing_scope.clone(),
                message: String::new(),
            })
            .collect();
        list.sort_by_key(|r| r.session_id);
        list
    };

    (StatusCode::OK, Json(records)).into_response()
}

// ─── Router ──────────────────────────────────────────────────────────────────

pub fn router() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/control", get(list_session_controls))
        .route("/{id}/control", get(get_session_control))
        .route("/{id}/pause", put(pause_session))
        .route("/{id}/resume", put(resume_session))
        .route("/{id}/group", put(set_session_group))
        .route("/{id}/broadcast-all", put(set_broadcast_all))
        .route("/{id}/command", post(relay_command))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::{get, post, put},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::{AppState, accounts::AccountStore, api};

    fn test_state() -> Arc<AppState> {
        test_state_with_session_control_limit(10_000)
    }

    fn test_state_with_session_control_limit(limit: usize) -> Arc<AppState> {
        use textquest::{alerts::AlertStore, config::AlertingConfig};
        let (event_tx, _) = tokio::sync::broadcast::channel::<String>(8);
        Arc::new(AppState {
            event_tx,
            account_store: std::sync::Mutex::new(AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(api::demo_character_configs()),
            auto_accept_settings: tokio::sync::RwLock::new(
                textquest_common::ipc::AutoAcceptSettings::default(),
            ),
            character_config_path: std::path::PathBuf::from("/tmp/test-char-configs.json"),
            character_config_write_lock: tokio::sync::Mutex::new(()),
            chat_log_write_lock: tokio::sync::Mutex::new(()),
            loot_state: api::loot::LootState::new_demo(),
            economy_state: api::economy::EconomyState::new_demo(),
            dashboard_state: api::dashboard::DashboardState::new_demo(),
            soul_audit: api::soul::SoulAuditState::new_demo(),
            discord_state: api::discord::DiscordState::new_demo(),
            player_watch_config: tokio::sync::RwLock::new(api::PlayerWatchConfig::default()),
            player_watch_write_lock: tokio::sync::Mutex::new(()),
            gm_alert_state: Arc::new(api::gm_alerts::GmAlertState::default()),
            spawn_alerts: api::spawn_alerts::SpawnAlertState::new_demo(),
            timestamp_configs: tokio::sync::RwLock::new(Default::default()),
            timestamp_config_write_lock: tokio::sync::Mutex::new(()),
            kill_tracker_state: api::kill_tracker::KillTrackerState::new_demo(),
            alert_store: AlertStore::open_memory().expect("in-memory alert store"),
            alert_config: tokio::sync::RwLock::new(AlertingConfig::default()),
            alerting_config_path: std::path::PathBuf::from("/tmp/test-alerting.toml"),
            api_token: None,
            auth_disabled: true, // Tests bypass auth
            live_session_snapshot_path: std::path::PathBuf::from("/tmp/test-live-sessions.json"),
            admin_session_snapshot_path: std::path::PathBuf::from("/tmp/test-admin-sessions.json"),
            xassist_configs: api::xassist::demo_xassist_configs(),
            chat_pattern_rules: api::chat_pattern_rules::load_rules_state(),
            say_detection: Some(Arc::new(api::say_detection::SayDetectionState::new_demo())),
            session_controls: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            tradeskill_trophy_settings: tokio::sync::RwLock::new(Default::default()),
            auto_group_settings: tokio::sync::RwLock::new(
                textquest_common::auto_group::AutoGroupSettings::default(),
            ),
            auto_group_config_path: std::path::PathBuf::from("/tmp/test-auto-group.json"),
            auto_group_state: api::auto_group::AutoGroupState::new_demo(),
            inventory_utility_parity: tokio::sync::RwLock::new(
                textquest_common::inventory_utility::InventoryUtilityConfig::default(),
            ),
            inventory_utility_parity_path: std::path::PathBuf::from(
                "/tmp/test-inventory-utility.json",
            ),
            inventory_utility_parity_write_lock: tokio::sync::Mutex::new(()),
            vendor_watch_state: api::vendor_watch::VendorWatchState::new_demo(),
            extension_catalog_state: api::extensions::ExtensionCatalogState::load(
                std::path::PathBuf::from("/tmp/test-extension-catalog.json"),
            ),
            session_logs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            session_logs_owner: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            session_control_state: Arc::new(SessionControlState::with_max_records(limit)),
            raid_config: tokio::sync::RwLock::new(api::RaidConfig::default()),
            raid_config_path: std::path::PathBuf::from("/tmp/test-raid-config.toml"),
            raid_config_write_lock: tokio::sync::Mutex::new(()),
            sound_config: tokio::sync::RwLock::new(api::sound::SoundConfig::default()),
            text_to_speech_state: api::text_to_speech::TextToSpeechState::new(),
            self_improvement_state: Arc::new(api::self_improvement::SelfImprovementState::new()),
            suggestion_state: api::suggestions::SuggestionState::new(),
            config_change_history: tokio::sync::RwLock::new(Vec::new()),
            last_config_change: tokio::sync::RwLock::new(None),
        })
    }

    fn test_app(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/api/sessions/control", get(list_session_controls))
            .route("/api/sessions/{id}/control", get(get_session_control))
            .route("/api/sessions/{id}/pause", put(pause_session))
            .route("/api/sessions/{id}/resume", put(resume_session))
            .route("/api/sessions/{id}/group", put(set_session_group))
            .route("/api/sessions/{id}/broadcast-all", put(set_broadcast_all))
            .route("/api/sessions/{id}/command", post(relay_command))
            .with_state(state)
    }

    async fn json_response(app: Router, req: Request<Body>) -> (StatusCode, Value) {
        let response = app.oneshot(req).await.expect("request should succeed");
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

    #[tokio::test]
    async fn get_control_creates_default_active_record() {
        let app = test_app(test_state());
        let (status, body) = json_response(
            app,
            Request::builder()
                .uri("/api/sessions/42/control")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["session_id"], 42);
        assert_eq!(body["state"], "active");
        assert_eq!(body["group_id"], 0);
    }

    #[tokio::test]
    async fn pause_transitions_active_to_paused() {
        let state = test_state();
        let app = test_app(state.clone());
        let (status, body) = json_response(
            app,
            Request::builder()
                .method("PUT")
                .uri("/api/sessions/1/pause")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["state"], "paused");
        assert_eq!(body["message"], "Session paused");
    }

    #[tokio::test]
    async fn pause_is_idempotent() {
        let state = test_state();
        // Pre-populate paused record
        {
            let mut records = state.session_control_state.records.lock().unwrap();
            let mut r = SessionControlRecord::new(7);
            r.state = SessionState::Paused;
            records.insert(7, r);
        }
        let app = test_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .method("PUT")
                .uri("/api/sessions/7/pause")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["state"], "paused");
        assert_eq!(body["message"], "Session was already paused");
    }

    #[tokio::test]
    async fn resume_transitions_paused_to_active() {
        let state = test_state();
        // First pause the session
        {
            let mut records = state.session_control_state.records.lock().unwrap();
            let mut r = SessionControlRecord::new(2);
            r.state = SessionState::Paused;
            records.insert(2, r);
        }
        let app = test_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .method("PUT")
                .uri("/api/sessions/2/resume")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["state"], "active");
        assert_eq!(body["message"], "Session resumed");
    }

    #[tokio::test]
    async fn resume_is_idempotent_when_already_active() {
        let state = test_state();
        let app = test_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .method("PUT")
                .uri("/api/sessions/3/resume")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["state"], "active");
        assert_eq!(body["message"], "Session was already active");
    }

    #[tokio::test]
    async fn set_group_assigns_group_and_scope() {
        let state = test_state();
        let app = test_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .method("PUT")
                .uri("/api/sessions/5/group")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "group_id": 3 }).to_string()))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["group_id"], 3);
        assert_eq!(body["routing_scope"]["kind"], "group");
        assert_eq!(body["routing_scope"]["group_id"], 3);
        assert!(body["message"].as_str().unwrap().contains("group 3"));
    }

    #[tokio::test]
    async fn set_group_zero_resets_to_all_session() {
        let state = test_state();
        // Put session in group 2 first
        {
            let mut records = state.session_control_state.records.lock().unwrap();
            let mut r = SessionControlRecord::new(10);
            r.group_id = 2;
            r.routing_scope = RoutingScope::Group {
                group_id: 2,
                label: "G2".to_string(),
            };
            records.insert(10, r);
        }
        let app = test_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .method("PUT")
                .uri("/api/sessions/10/group")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "group_id": 0 }).to_string()))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["group_id"], 0);
        assert_eq!(body["routing_scope"]["kind"], "all_session");
    }

    #[tokio::test]
    async fn broadcast_all_sets_all_session_scope() {
        let state = test_state();
        // Start in group scope
        {
            let mut records = state.session_control_state.records.lock().unwrap();
            let mut r = SessionControlRecord::new(4);
            r.group_id = 1;
            r.routing_scope = RoutingScope::Group {
                group_id: 1,
                label: "G1".to_string(),
            };
            records.insert(4, r);
        }
        let app = test_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .method("PUT")
                .uri("/api/sessions/4/broadcast-all")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["routing_scope"]["kind"], "all_session");
        assert!(body["message"].as_str().unwrap().contains("AllSession"));
    }

    #[tokio::test]
    async fn command_relay_accepted_for_active_session() {
        let state = test_state();
        let app = test_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .method("POST")
                .uri("/api/sessions/6/command")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "command": "who all" }).to_string()))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["command"], "who all");
        assert_eq!(body["accepted"], true);
    }

    #[tokio::test]
    async fn command_relay_dropped_when_session_paused() {
        let state = test_state();
        {
            let mut records = state.session_control_state.records.lock().unwrap();
            let mut r = SessionControlRecord::new(8);
            r.state = SessionState::Paused;
            records.insert(8, r);
        }
        let app = test_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .method("POST")
                .uri("/api/sessions/8/command")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "command": "say hello" }).to_string()))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["accepted"], false);
        assert!(body["message"].as_str().unwrap().contains("paused"));
    }

    #[tokio::test]
    async fn command_relay_rejects_empty_command() {
        let state = test_state();
        let app = test_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .method("POST")
                .uri("/api/sessions/9/command")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "command": "   " }).to_string()))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().contains("empty"));
    }

    #[tokio::test]
    async fn session_id_zero_returns_bad_request() {
        let state = test_state();
        let app = test_app(state);
        for uri in [
            "/api/sessions/0/control",
            "/api/sessions/0/pause",
            "/api/sessions/0/resume",
            "/api/sessions/0/broadcast-all",
        ] {
            let method = if uri.ends_with("control") {
                "GET"
            } else {
                "PUT"
            };
            let (status, _) = json_response(
                app.clone(),
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "expected 400 for {uri}");
        }
    }

    #[tokio::test]
    async fn list_session_controls_returns_all_records() {
        let state = test_state();
        {
            let mut records = state.session_control_state.records.lock().unwrap();
            records.insert(1, SessionControlRecord::new(1));
            records.insert(2, SessionControlRecord::new(2));
        }
        let app = test_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .uri("/api/sessions/control")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let arr = body.as_array().expect("expected array");
        assert_eq!(arr.len(), 2);
        // Sorted by session_id
        assert_eq!(arr[0]["session_id"], 1);
        assert_eq!(arr[1]["session_id"], 2);
    }

    #[tokio::test]
    async fn command_relay_uses_scope_override() {
        let state = test_state();
        let app = test_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .method("POST")
                .uri("/api/sessions/11/command")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "command": "loc", "scope": "group" }).to_string(),
                ))
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["scope_used"], "group");
        assert_eq!(body["accepted"], true);
    }

    #[tokio::test]
    async fn new_session_record_rejected_when_capacity_reached() {
        let state = test_state_with_session_control_limit(1);
        {
            let mut records = state.session_control_state.records.lock().unwrap();
            records.insert(1, SessionControlRecord::new(1));
        }

        let app = test_app(state);
        let (status, body) = json_response(
            app,
            Request::builder()
                .uri("/api/sessions/2/control")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(status, StatusCode::INSUFFICIENT_STORAGE);
        assert!(body["error"].as_str().unwrap().contains("capacity reached"));
    }
}
