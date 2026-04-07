//! TextQuest Web Dashboard — Axum backend for the M6 configuration & monitoring UI.
//!
//! Serves a React SPA and provides:
//! - REST API for credentials, group config, loot tables, raid configuration
//! - WebSocket endpoint for live session monitoring
//! - Shared types via textquest-common

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::routing::{get, put};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

mod api;
mod ws;

/// Shared application state accessible from all handlers.
pub struct AppState {
    /// Broadcast channel for real-time session events.
    pub event_tx: broadcast::Sender<String>,
    /// In-memory per-character config store (mirrors TUI config panel values).
    pub config_store: RwLock<HashMap<String, api::CharacterConfig>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("textquest_web=debug,tower_http=debug")
        .init();

    let (event_tx, _) = broadcast::channel::<String>(256);

    let state = Arc::new(AppState {
        event_tx,
        config_store: RwLock::new(api::default_configs()),
    });

    // Serve the pre-built React SPA from web/dist/.
    // The fallback sends index.html for any unmatched path (SPA client-side routing).
    let spa_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/dist");
    let serve_spa =
        ServeDir::new(&spa_dir).not_found_service(ServeFile::new(spa_dir.join("index.html")));

    let app = Router::new()
        .route("/api/health", get(api::health))
        .route("/api/sessions", get(api::list_sessions))
        // Economy endpoints
        .route("/api/economy/settings", get(api::get_economy_settings).put(api::put_economy_settings))
        .route("/api/economy/vendor-routes", get(api::list_vendor_routes).post(api::create_vendor_route))
        .route("/api/economy/vendor-routes/{id}", put(api::update_vendor_route).delete(api::delete_vendor_route))
        .route("/api/economy/wealth", get(api::get_wealth))
        .route("/ws", get(ws::ws_handler))
        .fallback_service(serve_spa)
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    tracing::info!("TextQuest web dashboard listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
