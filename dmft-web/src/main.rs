//! DMFT Web Dashboard — Axum backend for the M6 configuration & monitoring UI.
//!
//! Serves a React SPA and provides:
//! - REST API for credentials, group config, loot tables
//! - WebSocket endpoint for live session monitoring
//! - Shared types via dmft-common

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

mod api;
mod ws;

/// Shared application state accessible from all handlers.
pub struct AppState {
    /// Broadcast channel for real-time session events.
    pub event_tx: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("dmft_web=debug,tower_http=debug")
        .init();

    let (event_tx, _) = broadcast::channel::<String>(256);

    let state = Arc::new(AppState { event_tx });

    let app = Router::new()
        .route("/api/health", get(api::health))
        .route("/api/sessions", get(api::list_sessions))
        .route("/ws", get(ws::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    tracing::info!("DMFT web dashboard listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
