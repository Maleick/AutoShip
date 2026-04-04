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
                    Message::Ping(data) => {
                        if socket.send(Message::Pong(data)).await.is_err() {
                            break;
                        }
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
