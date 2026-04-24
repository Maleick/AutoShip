//! Authenticated WebSocket endpoint for DLL control sessions.

use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use textquest_common::ipc::{IpcCommand, IpcResponse};
use tokio::sync::{OnceCell, RwLock};

use crate::AppState;
use textquest_common::crypto::cmp::constant_time_eq;

type SessionRegistry = Arc<RwLock<HashMap<String, DllSession>>>;

static DLL_SESSIONS: OnceCell<SessionRegistry> = OnceCell::const_new();

#[derive(Debug, Clone)]
struct DllSession {
    instance_id: String,
    connection_id: uuid::Uuid,
    connected_at: DateTime<Utc>,
    last_seen_heartbeat: Option<DateTime<Utc>>,
    pid: Option<u32>,
    session_id: Option<u64>,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DllWsQuery {
    instance_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
enum DllWsInboundMessage {
    Hello(DllHello),
    Heartbeat(DllHeartbeat),
    IpcResponse(IpcResponse),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
enum DllWsOutboundMessage {
    HelloAck(DllHelloAck),
    HeartbeatAck(DllHeartbeatAck),
    IpcCommand(IpcCommand),
    Error(DllError),
}

#[derive(Debug, Clone, Deserialize)]
struct DllHello {
    instance_id: Option<String>,
    pid: Option<u32>,
    session_id: Option<u64>,
    version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DllHeartbeat {
    instance_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct DllHelloAck {
    instance_id: String,
}

#[derive(Debug, Serialize)]
struct DllHeartbeatAck {
    instance_id: String,
    server_time_ms: i64,
}

#[derive(Debug, Serialize)]
struct DllError {
    message: String,
}

/// Upgrade a DLL control connection to WebSocket after shared-secret auth.
///
/// DLL clients can set HTTP headers during the upgrade, so this endpoint uses
/// the same `X-API-Token` shared secret as REST API routes. `Authorization:
/// Bearer <token>` is also accepted for non-browser clients that standardize on
/// bearer tokens.
pub async fn dll_ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<DllWsQuery>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    if !is_authorized(&state, &headers, "/ws/dll") {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let instance_id = query
        .instance_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| format!("dll-{}", uuid::Uuid::new_v4()));

    ws.on_upgrade(move |socket| handle_socket(socket, instance_id))
        .into_response()
}

fn is_authorized(state: &AppState, headers: &HeaderMap, path: &'static str) -> bool {
    if state.auth_disabled {
        tracing::warn!(
            path,
            "Auth disabled (TEXTQUEST_DISABLE_AUTH=1) - DLL WebSocket allowed without token"
        );
        return true;
    }

    let Some(expected_token) = state.api_token.as_deref() else {
        tracing::error!(
            path,
            "DLL WebSocket rejected: TEXTQUEST_API_TOKEN is not set and auth is not disabled"
        );
        return false;
    };

    match provided_token(headers) {
        Some(token) if constant_time_eq(token.as_bytes(), expected_token.as_bytes()) => true,
        _ => {
            tracing::warn!(
                path,
                "DLL WebSocket rejected: missing or invalid auth token"
            );
            false
        }
    }
}

fn provided_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-api-token")
        .and_then(|value| value.to_str().ok())
        .or_else(|| bearer_token(headers))
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
}

async fn handle_socket(mut socket: WebSocket, instance_id: String) {
    let connection_id = uuid::Uuid::new_v4();
    register_session(instance_id.clone(), connection_id).await;
    if let Err(error) = run_socket(&mut socket, &instance_id).await {
        tracing::warn!(instance_id, error = %error, "DLL WebSocket closing after protocol error");
    }
    unregister_session(&instance_id, connection_id).await;
}

async fn run_socket(socket: &mut WebSocket, instance_id: &str) -> anyhow::Result<()> {
    while let Some(message) = socket.recv().await {
        match message? {
            Message::Text(text) => handle_text(socket, instance_id, text.as_str()).await?,
            Message::Binary(_) => {
                send_error(socket, "binary DLL WebSocket messages are not supported").await?;
            }
            Message::Ping(data) => socket.send(Message::Pong(data)).await?,
            Message::Pong(_) => mark_heartbeat(instance_id, None).await,
            Message::Close(_) => break,
        }
    }

    Ok(())
}

async fn handle_text(socket: &mut WebSocket, instance_id: &str, text: &str) -> anyhow::Result<()> {
    match serde_json::from_str::<DllWsInboundMessage>(text) {
        Ok(DllWsInboundMessage::Hello(hello)) => {
            merge_hello(instance_id, hello).await;
            send_json(
                socket,
                &DllWsOutboundMessage::HelloAck(DllHelloAck {
                    instance_id: instance_id.to_string(),
                }),
            )
            .await?;
        }
        Ok(DllWsInboundMessage::Heartbeat(heartbeat)) => {
            mark_heartbeat(instance_id, heartbeat.instance_id).await;
            send_json(
                socket,
                &DllWsOutboundMessage::HeartbeatAck(DllHeartbeatAck {
                    instance_id: instance_id.to_string(),
                    server_time_ms: Utc::now().timestamp_millis(),
                }),
            )
            .await?;
        }
        Ok(DllWsInboundMessage::IpcResponse(response)) => {
            mark_heartbeat(instance_id, None).await;
            tracing::debug!(
                instance_id,
                correlation_id = ?response.correlation_id,
                response = ?response.response,
                "Received DLL IPC response over WebSocket"
            );
        }
        Err(error) => {
            tracing::warn!(instance_id, error = %error, "Invalid DLL WebSocket message");
            send_error(socket, "invalid DLL WebSocket message").await?;
        }
    }

    Ok(())
}

async fn send_error(socket: &mut WebSocket, message: &str) -> anyhow::Result<()> {
    send_json(
        socket,
        &DllWsOutboundMessage::Error(DllError {
            message: message.to_string(),
        }),
    )
    .await
}

async fn send_json(socket: &mut WebSocket, message: &DllWsOutboundMessage) -> anyhow::Result<()> {
    socket
        .send(Message::Text(serde_json::to_string(message)?.into()))
        .await?;
    Ok(())
}

async fn registry() -> &'static SessionRegistry {
    DLL_SESSIONS
        .get_or_init(|| async { Arc::new(RwLock::new(HashMap::new())) })
        .await
}

async fn register_session(instance_id: String, connection_id: uuid::Uuid) {
    let connected_at = Utc::now();
    let sessions = registry().await;
    let mut sessions = sessions.write().await;
    sessions.insert(
        instance_id.clone(),
        DllSession {
            instance_id: instance_id.clone(),
            connection_id,
            connected_at,
            last_seen_heartbeat: None,
            pid: None,
            session_id: None,
            version: None,
        },
    );
    tracing::info!(
        instance_id,
        %connection_id,
        connected_at = %connected_at,
        active_sessions = sessions.len(),
        "DLL WebSocket session connected"
    );
}

async fn merge_hello(instance_id: &str, hello: DllHello) {
    let sessions = registry().await;
    let mut sessions = sessions.write().await;
    let active_sessions = sessions.len();
    if let Some(session) = sessions.get_mut(instance_id) {
        session.pid = hello.pid;
        session.session_id = hello.session_id;
        session.version = hello.version;
        session.last_seen_heartbeat = Some(Utc::now());
        tracing::info!(
            instance_id = %session.instance_id,
            hello_instance_id = ?hello.instance_id,
            pid = ?session.pid,
            session_id = ?session.session_id,
            version = ?session.version,
            active_sessions,
            "DLL WebSocket session identified"
        );
    }
}

async fn mark_heartbeat(instance_id: &str, heartbeat_instance_id: Option<String>) {
    let now = Utc::now();
    let sessions = registry().await;
    let mut sessions = sessions.write().await;
    if let Some(session) = sessions.get_mut(instance_id) {
        session.last_seen_heartbeat = Some(now);
        tracing::debug!(
            instance_id = %session.instance_id,
            heartbeat_instance_id,
            last_seen_heartbeat = %now,
            "DLL WebSocket heartbeat"
        );
    }
}

async fn unregister_session(instance_id: &str, connection_id: uuid::Uuid) {
    let sessions = registry().await;
    let mut sessions = sessions.write().await;
    let should_remove = sessions
        .get(instance_id)
        .is_some_and(|session| session.connection_id == connection_id);
    let removed = if should_remove {
        sessions.remove(instance_id)
    } else {
        None
    };
    let connected_at = removed.as_ref().map(|session| session.connected_at);
    let last_seen_heartbeat = removed
        .as_ref()
        .and_then(|session| session.last_seen_heartbeat);
    tracing::info!(
        instance_id,
        %connection_id,
        connected_at = ?connected_at,
        last_seen_heartbeat = ?last_seen_heartbeat,
        active_sessions = sessions.len(),
        "DLL WebSocket session disconnected"
    );
}
