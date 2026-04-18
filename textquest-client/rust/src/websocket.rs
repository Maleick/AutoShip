//! WebSocket client implementation for real-time session events

use crate::{Error, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub enum Event {
    SessionUpdate(serde_json::Value),
    Command(String),
    Error(String),
}

pub struct WebSocketClient {
    sender: mpsc::Sender<Message>,
}

impl WebSocketClient {
    pub async fn connect(url: &str, api_token: Option<&str>) -> Result<(Self, mpsc::Receiver<Event>)> {
        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| Error::WebSocket(e.to_string()))?;

        let (mut write, mut read) = ws_stream.split();

        if let Some(token) = api_token {
            let auth_msg = serde_json::json!({ "type": "auth", "token": token });
            write.send(Message::Text(auth_msg.to_string()))
                .await
                .map_err(|e| Error::WebSocket(e.to_string()))?;
        }

        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        let event: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|_| {
                            serde_json::json!({ "type": "unknown", "data": text })
                        });
                        let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let event = match event_type {
                            "session" | "session_update" => Event::SessionUpdate(event),
                            "command" => {
                                let cmd = event.get("command").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                Event::Command(cmd)
                            }
                            _ => Event::Error(format!("Unknown event type: {}", event_type)),
                        };
                        let _ = tx.send(event).await;
                    }
                    Ok(Message::Close(_)) => break,
                    Err(e) => {
                        let _ = tx.send(Event::Error(e.to_string())).await;
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok((
            Self {
                sender: mpsc::Sender::from_channel(0),
            },
            rx,
        ))
    }

    pub async fn send_command(&mut self, command: &str, target: Option<&str>) -> Result<()> {
        let body = serde_json::json!({
            "type": "command",
            "command": command,
            "target": target,
        });
        let msg = Message::Text(body.to_string());
        self.sender
            .send(msg)
            .await
            .map_err(|e| Error::WebSocket(e.to_string()))?;
        Ok(())
    }
}