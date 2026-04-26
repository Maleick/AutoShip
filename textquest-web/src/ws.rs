//! WebSocket handler for real-time session monitoring.

use std::{
    path::Path,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::AppState;
use textquest_common::shared_client_state::SharedClientState;

/// Query parameters for WebSocket upgrade.
#[derive(Deserialize)]
pub struct WsQuery {
    /// Optional authentication token passed via query parameter.
    /// Used for WebSocket authentication since browser WebSocket API
    /// does not allow setting custom headers.
    token: Option<String>,
    /// Optional live dashboard stream selector. Supported values:
    /// `stream=dashboard` or `stream=sessions`.
    stream: Option<String>,
    /// Optional boolean-like dashboard stream toggle:
    /// `dashboard=1`, `dashboard=true`, `dashboard=yes`, or `dashboard=on`.
    dashboard: Option<String>,
    /// Optional group filter for dashboard session snapshots.
    group: Option<String>,
}

const DASHBOARD_REFRESH_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone)]
struct DashboardStreamOptions {
    enabled: bool,
    group: Option<String>,
}

impl DashboardStreamOptions {
    fn from_query(query: &WsQuery) -> Self {
        let stream_enabled = query
            .stream
            .as_deref()
            .is_some_and(matches_dashboard_stream);
        let dashboard_enabled = query
            .dashboard
            .as_deref()
            .is_some_and(matches_dashboard_toggle);

        Self {
            enabled: stream_enabled || dashboard_enabled,
            group: query
                .group
                .as_ref()
                .map(|group| group.trim().to_string())
                .filter(|group| !group.is_empty()),
        }
    }
}

fn matches_dashboard_stream(value: &str) -> bool {
    value.eq_ignore_ascii_case("dashboard") || value.eq_ignore_ascii_case("sessions")
}

fn matches_dashboard_toggle(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardSnapshotEvent {
    #[serde(rename = "type")]
    event_type: &'static str,
    generated_at_ms: u64,
    group_filter: Option<String>,
    metrics: DashboardSummaryMetrics,
    clients: Vec<DashboardClientStatus>,
    alerts: Vec<DashboardAlertFeedItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardSummaryMetrics {
    total_clients: usize,
    online_clients: usize,
    offline_clients: usize,
    stuck_clients: usize,
    average_hp_pct: f32,
    average_mana_pct: f32,
    total_dps: Option<f32>,
    heal_coverage: Option<f32>,
}

impl DashboardSummaryMetrics {
    fn from_clients(clients: &[DashboardClientStatus]) -> Self {
        let total_clients = clients.len();
        let online_clients = clients
            .iter()
            .filter(|client| client.login_status != "offline")
            .count();
        let offline_clients = clients
            .iter()
            .filter(|client| client.login_status == "offline")
            .count();
        let stuck_clients = clients.iter().filter(|client| client.stuck).count();
        let average_hp_pct = average_percent(clients.iter().map(|client| client.hp_pct));
        let average_mana_pct = average_percent(clients.iter().map(|client| client.mana_pct));

        Self {
            total_clients,
            online_clients,
            offline_clients,
            stuck_clients,
            average_hp_pct,
            average_mana_pct,
            total_dps: None,
            heal_coverage: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardClientStatus {
    client_id: u32,
    account_name: Option<String>,
    character_name: String,
    role: Option<String>,
    group_name: Option<String>,
    login_status: &'static str,
    zone: String,
    level: u8,
    hp_pct: f32,
    mana_pct: f32,
    endurance_pct: f32,
    status: String,
    camp_phase: &'static str,
    stuck: bool,
    last_action_at_ms: Option<u64>,
    buff_count: usize,
    target_name: Option<String>,
    target_hp_pct: Option<f32>,
    pet_name: Option<String>,
}

impl DashboardClientStatus {
    fn from_live(
        session: SharedClientState,
        config: Option<&crate::api::CharacterConfig>,
        _generated_at_ms: u64,
    ) -> Self {
        let stuck = status_is_stuck(&session.status);
        Self {
            client_id: session.client_id,
            account_name: None,
            character_name: session.character_name,
            role: config.map(|cfg| cfg.role.clone()),
            group_name: config.and_then(|cfg| cfg.group_name.clone()),
            login_status: if stuck { "stuck" } else { "online" },
            zone: if session.zone_long_name.is_empty() {
                session.zone_short_name
            } else {
                session.zone_long_name
            },
            level: session.level,
            hp_pct: session.hp_pct,
            mana_pct: session.mana_pct,
            endurance_pct: session.endurance_pct,
            camp_phase: camp_phase_from_status(&session.status),
            status: session.status,
            stuck,
            last_action_at_ms: None,
            buff_count: session.buffs.len(),
            target_name: session.target.as_ref().map(|target| target.name.clone()),
            target_hp_pct: session.target.as_ref().map(|target| target.hp_pct),
            pet_name: session.pet.as_ref().map(|pet| pet.name.clone()),
        }
    }

    fn from_config(index: usize, config: &crate::api::CharacterConfig) -> Self {
        Self {
            client_id: index as u32 + 1,
            account_name: None,
            character_name: config.character_name.clone(),
            role: Some(config.role.clone()),
            group_name: config.group_name.clone(),
            login_status: "offline",
            zone: "Unknown".to_string(),
            level: 0,
            hp_pct: 0.0,
            mana_pct: 0.0,
            endurance_pct: 0.0,
            status: "offline".to_string(),
            camp_phase: "offline",
            stuck: false,
            last_action_at_ms: None,
            buff_count: 0,
            target_name: None,
            target_hp_pct: None,
            pet_name: None,
        }
    }

    fn matches_group(&self, group: &str) -> bool {
        self.group_name
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(group))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardAlertFeedItem {
    severity: &'static str,
    message: String,
    generated_at_ms: u64,
}

// Re-use the canonical constant-time comparison from the crate root so there
// is exactly one implementation to audit and maintain.
use textquest_common::crypto::cmp::constant_time_eq;

/// Upgrade HTTP connection to WebSocket for live session events.
///
/// Authentication is **on by default** (matching the REST API).  If
/// `TEXTQUEST_DISABLE_AUTH=1` is set, the upgrade is allowed without a token
/// but a `WARN` is emitted.  Otherwise a matching `?token=` query parameter is
/// required; the upgrade is rejected with `401 Unauthorized` when:
/// - the token is missing or incorrect, or
/// - no token is configured and auth is not explicitly disabled.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    // Explicit dev opt-out.
    if state.auth_disabled {
        tracing::warn!(
            "Auth disabled (TEXTQUEST_DISABLE_AUTH=1) — WebSocket allowed without token"
        );
        let dashboard_options = DashboardStreamOptions::from_query(&query);
        return ws
            .on_upgrade(move |socket| handle_socket(socket, state, dashboard_options))
            .into_response();
    }

    match state.api_token {
        Some(ref expected_token) => match &query.token {
            Some(provided_token)
                if constant_time_eq(provided_token.as_bytes(), expected_token.as_bytes()) => {}
            _ => {
                tracing::warn!("WebSocket connection rejected: missing or invalid token");
                return StatusCode::UNAUTHORIZED.into_response();
            }
        },
        None => {
            // No token configured and auth not explicitly disabled — reject.
            tracing::error!(
                "WebSocket connection rejected: TEXTQUEST_API_TOKEN is not set and auth is not \
                 disabled. Set TEXTQUEST_API_TOKEN or TEXTQUEST_DISABLE_AUTH=1."
            );
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }

    let dashboard_options = DashboardStreamOptions::from_query(&query);
    ws.on_upgrade(move |socket| handle_socket(socket, state, dashboard_options))
        .into_response()
}

async fn handle_socket(
    mut socket: WebSocket,
    state: Arc<AppState>,
    dashboard_options: DashboardStreamOptions,
) {
    let mut rx = state.event_tx.subscribe();
    let mut dashboard_tick = tokio::time::interval(DASHBOARD_REFRESH_INTERVAL);
    dashboard_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    dashboard_tick.tick().await;

    tracing::info!("WebSocket client connected");

    if dashboard_options.enabled
        && send_dashboard_snapshot(&mut socket, &state, &dashboard_options)
            .await
            .is_err()
    {
        tracing::info!("WebSocket client disconnected before dashboard snapshot");
        return;
    }

    loop {
        tokio::select! {
            // Forward broadcast events to the WebSocket client.
            Ok(event) = rx.recv() => {
                if socket.send(Message::Text(event.into())).await.is_err() {
                    break;
                }
            }
            _ = dashboard_tick.tick(), if dashboard_options.enabled => {
                if send_dashboard_snapshot(&mut socket, &state, &dashboard_options).await.is_err() {
                    break;
                }
            }
            // Handle incoming messages from the client.
            Some(msg) = receive_message(&mut socket) => {
                match msg {
                    Message::Close(_) => break,
                    Message::Ping(data)
                        if socket.send(Message::Pong(data.clone())).await.is_err() =>
                    {
                        break;
                    }
                    _ => {}
                }
            }
            else => break,
        }
    }

    tracing::info!("WebSocket client disconnected");
}

async fn send_dashboard_snapshot(
    socket: &mut WebSocket,
    state: &AppState,
    options: &DashboardStreamOptions,
) -> Result<(), axum::Error> {
    match build_dashboard_snapshot(state, options).await {
        Ok(event) => {
            let payload = serde_json::to_string(&event).unwrap_or_else(|error| {
                tracing::error!(%error, "Failed to encode dashboard snapshot");
                dashboard_error_payload("Failed to encode dashboard snapshot")
            });
            socket.send(Message::Text(payload.into())).await
        }
        Err(error) => {
            tracing::warn!(%error, "Failed to build dashboard session snapshot");
            socket
                .send(Message::Text(
                    dashboard_error_payload("Failed to read live session snapshot").into(),
                ))
                .await
        }
    }
}

async fn build_dashboard_snapshot(
    state: &AppState,
    options: &DashboardStreamOptions,
) -> anyhow::Result<DashboardSnapshotEvent> {
    let generated_at_ms = unix_timestamp_millis();
    let live_sessions = read_live_sessions(&state.live_session_snapshot_path)?;
    let configs = state.character_configs.read().await;

    let mut clients = if live_sessions.is_empty() {
        configs
            .values()
            .enumerate()
            .map(|(index, config)| DashboardClientStatus::from_config(index, config))
            .collect::<Vec<_>>()
    } else {
        live_sessions
            .into_iter()
            .map(|session| {
                let config = configs.values().find(|config| {
                    config
                        .character_name
                        .eq_ignore_ascii_case(&session.character_name)
                });
                DashboardClientStatus::from_live(session, config, generated_at_ms)
            })
            .collect::<Vec<_>>()
    };

    if let Some(group) = options.group.as_deref() {
        clients.retain(|client| client.matches_group(group));
    }

    let metrics = DashboardSummaryMetrics::from_clients(&clients);
    Ok(DashboardSnapshotEvent {
        event_type: "session.dashboard.snapshot",
        generated_at_ms,
        group_filter: options.group.clone(),
        metrics,
        clients,
        alerts: Vec::new(),
    })
}

fn read_live_sessions(path: &Path) -> anyhow::Result<Vec<SharedClientState>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let payload = std::fs::read(path)?;
    Ok(serde_json::from_slice::<Vec<SharedClientState>>(&payload)?)
}

fn average_percent(values: impl Iterator<Item = f32>) -> f32 {
    let (sum, count) = values.fold((0.0, 0usize), |(sum, count), value| {
        (sum + value, count + 1)
    });
    if count == 0 { 0.0 } else { sum / count as f32 }
}

fn camp_phase_from_status(status: &str) -> &'static str {
    let normalized = status.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "active" | "engaging" | "pulling" | "casting" => "hunting",
        "recovering" | "idle" => "resting",
        "buffing" => "buffing",
        "stuck" => "stuck",
        "dead" => "dead",
        "offline" => "offline",
        _ => "unknown",
    }
}

fn status_is_stuck(status: &str) -> bool {
    status.trim().to_ascii_lowercase().contains("stuck")
}

fn unix_timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn dashboard_error_payload(message: &str) -> String {
    serde_json::json!({
        "type": "session.dashboard.error",
        "message": message,
        "generatedAtMs": unix_timestamp_millis(),
    })
    .to_string()
}

async fn receive_message(socket: &mut WebSocket) -> Option<Message> {
    match socket.recv().await {
        Some(Ok(msg)) => Some(msg),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::{AppState, build_app};
    use axum::http::StatusCode;
    use axum::{
        Json,
        extract::{Path as AxumPath, State},
    };
    use futures_util::{SinkExt, StreamExt};
    use std::{sync::Arc, time::Duration};
    use textquest_common::shared_client_state::{SharedClientState, SharedTargetState};
    use tokio::{net::TcpListener, task::JoinHandle, time::timeout};
    use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

    fn test_state() -> Arc<AppState> {
        let mut state = crate::test_app_state();
        state.live_session_snapshot_path =
            crate::data_dir().join("data/runtime/ws-test-live-sessions.json");
        Arc::new(state)
    }

    async fn spawn_test_server(state: Arc<AppState>) -> (Arc<AppState>, JoinHandle<()>, String) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("listener address");
        let app = build_app(state.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server should run");
        });

        let url = match state.api_token.as_ref() {
            Some(token) => {
                // Include token in query if authentication is enabled.
                format!("ws://{addr}/ws?token={token}")
            }
            None => format!("ws://{addr}/ws"),
        };

        (state, server, url)
    }

    async fn connect_test_socket() -> (
        Arc<AppState>,
        JoinHandle<()>,
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) {
        let (state, server, url) = spawn_test_server(test_state()).await;
        let (socket, _) = connect_async(url)
            .await
            .expect("websocket handshake should succeed");
        wait_for_receiver_count(&state, 1).await;
        (state, server, socket)
    }

    fn websocket_test_timeout() -> Duration {
        // Coverage instrumentation slows websocket handshakes and broadcast
        // delivery enough that a 1s test budget becomes flaky in CI.
        Duration::from_secs(10)
    }

    fn append_ws_query(url: String, query: &str) -> String {
        if url.contains('?') {
            format!("{url}&{query}")
        } else {
            format!("{url}?{query}")
        }
    }

    fn live_session(client_id: u32, character_name: &str, status: &str) -> SharedClientState {
        SharedClientState {
            client_id,
            spawn_id: 10_000 + client_id,
            character_name: character_name.to_string(),
            class_id: 2,
            level: 65,
            zone_short_name: "poknowledge".to_string(),
            zone_long_name: "Plane of Knowledge".to_string(),
            hp_pct: 62.5,
            mana_pct: 41.0,
            endurance_pct: 88.0,
            is_dead: false,
            status: status.to_string(),
            target: Some(SharedTargetState {
                spawn_id: 20_000 + client_id,
                name: "a test target".to_string(),
                hp_pct: 73.0,
            }),
            buffs: Vec::new(),
            pet: None,
        }
    }

    fn write_live_sessions(state: &AppState, sessions: &[SharedClientState]) {
        if let Some(parent) = state.live_session_snapshot_path.parent() {
            std::fs::create_dir_all(parent).expect("snapshot parent should exist");
        }
        std::fs::write(
            &state.live_session_snapshot_path,
            serde_json::to_vec(sessions).expect("live sessions should encode"),
        )
        .expect("live sessions snapshot should write");
    }

    async fn wait_for_receiver_count(state: &AppState, expected: usize) {
        timeout(websocket_test_timeout(), async {
            while state.event_tx.receiver_count() != expected {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("websocket receiver count should settle");
    }

    async fn next_message(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> WsMessage {
        timeout(websocket_test_timeout(), socket.next())
            .await
            .expect("message should arrive in time")
            .expect("socket should stay open")
            .expect("message should decode")
    }

    #[tokio::test]
    async fn websocket_forwards_broadcast_events() {
        let (state, server, mut socket) = connect_test_socket().await;

        state
            .event_tx
            .send("session:update".to_string())
            .expect("broadcast should reach websocket subscriber");

        assert_eq!(
            next_message(&mut socket).await,
            WsMessage::Text("session:update".into())
        );

        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn websocket_dashboard_stream_sends_live_session_snapshot() {
        let (state, server, url) = spawn_test_server(test_state()).await;
        write_live_sessions(&state, &[live_session(7, "Ariane", "active")]);

        let (mut socket, _) = connect_async(append_ws_query(url, "stream=dashboard"))
            .await
            .expect("dashboard websocket handshake should succeed");
        wait_for_receiver_count(&state, 1).await;

        let message = next_message(&mut socket).await;
        let WsMessage::Text(payload) = message else {
            panic!("expected dashboard snapshot text frame");
        };
        let event: serde_json::Value =
            serde_json::from_str(payload.as_ref()).expect("dashboard snapshot json");

        assert_eq!(event["type"], "session.dashboard.snapshot");
        assert_eq!(event["metrics"]["totalClients"], 1);
        assert_eq!(event["metrics"]["onlineClients"], 1);
        assert_eq!(event["clients"][0]["clientId"], 7);
        assert_eq!(event["clients"][0]["characterName"], "Ariane");
        assert_eq!(event["clients"][0]["loginStatus"], "online");
        assert_eq!(event["clients"][0]["campPhase"], "hunting");
        assert_eq!(event["clients"][0]["zone"], "Plane of Knowledge");
        assert_eq!(event["clients"][0]["targetName"], "a test target");

        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn websocket_replies_to_ping_frames() {
        let (_state, server, mut socket) = connect_test_socket().await;

        socket
            .send(WsMessage::Ping(vec![1, 2, 3, 4].into()))
            .await
            .expect("ping should send");

        assert_eq!(
            next_message(&mut socket).await,
            WsMessage::Pong(vec![1, 2, 3, 4].into())
        );

        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn websocket_ignores_text_messages_and_stays_connected() {
        let (state, server, mut socket) = connect_test_socket().await;

        socket
            .send(WsMessage::Text("noop".into()))
            .await
            .expect("text message should send");
        state
            .event_tx
            .send("after-text".to_string())
            .expect("broadcast should still reach websocket subscriber");

        assert_eq!(
            next_message(&mut socket).await,
            WsMessage::Text("after-text".into())
        );

        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn websocket_receives_extension_runtime_events() {
        let (state, server, mut socket) = connect_test_socket().await;

        let mut trusted_h = axum::http::HeaderMap::new();
        trusted_h.insert(
            axum::http::header::ORIGIN,
            crate::api::loot::TRUSTED_ORIGINS[0].parse().unwrap(),
        );
        let response = crate::api::extensions::put_runtime_status(
            State(state.clone()),
            AxumPath("mq2eqbc".to_string()),
            trusted_h,
            Json(crate::api::extensions::RuntimeUpdateRequest { enabled: true }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);

        let message = next_message(&mut socket).await;
        let WsMessage::Text(payload) = message else {
            panic!("expected websocket text frame");
        };
        let event: serde_json::Value =
            serde_json::from_str(payload.as_ref()).expect("runtime event json");
        assert_eq!(event["type"], "extension.runtime");
        assert_eq!(event["entry"]["id"], "mq2eqbc");
        assert_eq!(event["entry"]["runtime"]["enabled"], true);
        assert_eq!(event["entry"]["runtime"]["adapterHealth"], "healthy");

        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn websocket_close_frame_drops_broadcast_subscription() {
        let (state, server, mut socket) = connect_test_socket().await;

        socket
            .close(None)
            .await
            .expect("client should close websocket");
        wait_for_receiver_count(&state, 0).await;

        assert!(
            state.event_tx.receiver_count() == 0,
            "websocket close should drop the broadcast receiver"
        );

        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn websocket_requires_valid_token_when_api_token_is_set() {
        // Create state with a token set and auth enforcement enabled.
        // auth_disabled must be false here so the ws_handler actually rejects
        // unauthenticated connections — test_app_state() defaults auth_disabled to true.
        let mut state = crate::test_app_state();
        state.api_token = Some("secret-token".to_string());
        state.auth_disabled = false; // enforce auth for this test
        state.live_session_snapshot_path =
            crate::data_dir().join("data/runtime/ws-auth-test-live-sessions.json");
        let state_with_token = Arc::new(state);

        // Spawn server with authenticated state
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("listener address");
        let app = build_app(state_with_token.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server should run");
        });

        // Test 1: Connection without token should fail
        let url_no_token = format!("ws://{addr}/ws");
        let result = connect_async(url_no_token).await;
        assert!(
            result.is_err(),
            "WebSocket should reject connection without token"
        );

        // Test 2: Connection with wrong token should fail
        let url_wrong_token = format!("ws://{addr}/ws?token=wrong-token");
        let result = connect_async(url_wrong_token).await;
        assert!(
            result.is_err(),
            "WebSocket should reject connection with wrong token"
        );

        // Test 3: Connection with correct token should succeed
        let url_correct_token = format!("ws://{addr}/ws?token=secret-token");
        let (socket, _) = connect_async(url_correct_token)
            .await
            .expect("WebSocket should accept connection with correct token");
        wait_for_receiver_count(&state_with_token, 1).await;

        // Verify the socket can receive messages
        state_with_token
            .event_tx
            .send("test:authenticated".to_string())
            .expect("broadcast should reach authenticated subscriber");

        let mut socket = socket;
        assert_eq!(
            timeout(websocket_test_timeout(), socket.next())
                .await
                .expect("message should arrive in time")
                .expect("socket should stay open")
                .expect("message should decode"),
            WsMessage::Text("test:authenticated".into())
        );

        server.abort();
        let _ = server.await;
    }

    #[tokio::test]
    async fn dll_websocket_requires_header_token_when_api_token_is_set() {
        let mut state = crate::test_app_state();
        state.api_token = Some("secret-token".to_string());
        state.auth_disabled = false;
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("listener address");
        let app = build_app(Arc::new(state));
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("server should run");
        });

        let result = connect_async(format!("ws://{addr}/ws/dll?instance_id=test-dll")).await;

        match result {
            Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            }
            other => panic!("expected 401 websocket rejection, got {other:?}"),
        }

        server.abort();
        let _ = server.await;
    }
}
