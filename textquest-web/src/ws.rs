//! WebSocket handler for real-time session monitoring.

use std::sync::Arc;

use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::AppState;

/// Query parameters for WebSocket upgrade.
#[derive(Deserialize)]
pub struct WsQuery {
    /// Optional authentication token passed via query parameter.
    /// Used for WebSocket authentication since browser WebSocket API
    /// does not allow setting custom headers.
    token: Option<String>,
}

// Re-use the canonical constant-time comparison from the crate root so there
// is exactly one implementation to audit and maintain.
use super::constant_time_eq_str;

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
        tracing::warn!("Auth disabled (TEXTQUEST_DISABLE_AUTH=1) — WebSocket allowed without token");
        return ws.on_upgrade(move |socket| handle_socket(socket, state)).into_response();
    }

    match state.api_token {
        Some(ref expected_token) => {
            match &query.token {
                Some(provided_token) if constant_time_eq_str(provided_token, expected_token) => {}
                _ => {
                    tracing::warn!("WebSocket connection rejected: missing or invalid token");
                    return StatusCode::UNAUTHORIZED.into_response();
                }
            }
        }
        None => {
            // No token configured and auth not explicitly disabled — reject.
            tracing::error!(
                "WebSocket connection rejected: TEXTQUEST_API_TOKEN is not set and auth is not \
                 disabled. Set TEXTQUEST_API_TOKEN or TEXTQUEST_DISABLE_AUTH=1."
            );
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state))
        .into_response()
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.event_tx.subscribe();

    tracing::info!("WebSocket client connected");

    loop {
        tokio::select! {
            // Forward broadcast events to the WebSocket client.
            Ok(event) = rx.recv() => {
                if socket.send(Message::Text(event.into())).await.is_err() {
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
    use tokio::{net::TcpListener, task::JoinHandle, time::timeout};
    use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

    fn test_state() -> Arc<AppState> {
        let mut state = crate::test_app_state();
        state.live_session_snapshot_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../data/runtime/ws-test-live-sessions.json");
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
        state.live_session_snapshot_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../data/runtime/ws-auth-test-live-sessions.json");
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
}
