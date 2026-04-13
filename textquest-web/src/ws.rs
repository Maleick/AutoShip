//! WebSocket handler for real-time session monitoring.

use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;

use crate::AppState;

/// Upgrade HTTP connection to WebSocket for live session events.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
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
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;
    use tokio::time::timeout;
    use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

    fn test_state() -> Arc<AppState> {
        let (event_tx, _) = tokio::sync::broadcast::channel::<String>(8);
        Arc::new(AppState {
            event_tx,
            account_store: Mutex::new(accounts::AccountStore::default()),
            credential_store: None,
            character_configs: tokio::sync::RwLock::new(api::demo_character_configs()),
            loot_state: api::loot::LootState::new_demo(),
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

        (state, server, format!("ws://{addr}/ws"))
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
}
