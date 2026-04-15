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

/// Upgrade HTTP connection to WebSocket for live session events.
///
/// If `TEXTQUEST_API_TOKEN` is configured, requires a matching `?token=` query
/// parameter. Rejects the upgrade with `401 Unauthorized` if token validation
/// fails.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    // Validate token if authentication is enabled
    if let Some(ref expected_token) = state.api_token {
        match &query.token {
            Some(provided_token) if constant_time_eq_str(provided_token, expected_token) => {}
            _ => {
                tracing::warn!("WebSocket connection rejected: missing or invalid token");
                return StatusCode::UNAUTHORIZED.into_response();
            }
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
    use crate::{AppState, accounts, api, build_app};
    use futures_util::{SinkExt, StreamExt};
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };
    use tokio::{net::TcpListener, task::JoinHandle, time::timeout};
    use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

    fn test_state() -> Arc<AppState> {
        let (event_tx, _) = tokio::sync::broadcast::channel::<String>(8);
        Arc::new(AppState {
            event_tx,
            account_store: Mutex::new(accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(api::demo_character_configs()),
            loot_state: api::loot::LootState::new_demo(),
            economy_state: api::economy::EconomyState::new_demo(),
            dashboard_state: api::dashboard::DashboardState::new_demo(),
            soul_audit: api::soul::SoulAuditState::new_demo(),
            api_token: None,
        })
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

    async fn wait_for_receiver_count(state: &AppState, expected: usize) {
        timeout(Duration::from_secs(1), async {
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
        timeout(Duration::from_secs(1), socket.next())
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
        // Create state with a token set
        let (event_tx, _) = tokio::sync::broadcast::channel::<String>(8);
        let state_with_token = Arc::new(AppState {
            event_tx,
            account_store: Mutex::new(accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(api::demo_character_configs()),
            loot_state: api::loot::LootState::new_demo(),
            economy_state: api::economy::EconomyState::new_demo(),
            dashboard_state: api::dashboard::DashboardState::new_demo(),
            soul_audit: api::soul::SoulAuditState::new_demo(),
            api_token: Some("secret-token".to_string()),
        });

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
            timeout(Duration::from_secs(1), socket.next())
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
