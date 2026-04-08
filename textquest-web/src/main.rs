//! TextQuest Web Dashboard — Axum backend for the M6 configuration & monitoring UI.
//!
//! Serves a React SPA and provides:
//! - REST API for credentials, group config, loot tables, raid configuration
//! - WebSocket endpoint for live session monitoring
//! - DZ lockout timers, reset queue, raid instance tracking
//! - Shared types via textquest-common
//!
//! ## Credential storage
//!
//! Set `TEXTQUEST_MASTER_PASSWORD` in the environment to enable password
//! management.  When set the server opens (or creates) `data/credentials.db`,
//! derives a master key with Argon2id and stores per-account passwords using
//! AES-256-GCM — the same schema used by the CLI orchestrator.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::http::{HeaderValue, Method};
use axum::routing::{get, put};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

mod accounts;
mod api;
mod ws;

/// Shared application state accessible from all handlers.
pub struct AppState {
    /// Broadcast channel for real-time session events.
    pub event_tx: broadcast::Sender<String>,
    /// In-memory character tuning config store for the strategy tuning panel.
    pub character_configs: tokio::sync::RwLock<HashMap<String, api::CharacterConfig>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("textquest_web=debug,tower_http=debug")
        .init();

    let (event_tx, _) = broadcast::channel::<String>(256);

    let state = Arc::new(AppState {
        event_tx,
        character_configs: tokio::sync::RwLock::new(api::demo_character_configs()),
    });

    // Serve the pre-built React SPA from web/dist/.
    // The fallback sends index.html for any unmatched path (SPA client-side routing).
    let spa_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/dist");
    let serve_spa =
        ServeDir::new(&spa_dir).not_found_service(ServeFile::new(spa_dir.join("index.html")));

    // Loot state is independent of the main AppState so it can be extracted
    // directly in each handler via its own Arc<LootState>.
    let loot_state = api::loot::LootState::new_demo();

    let loot_router = Router::new()
        .route(
            "/rules",
            get(api::loot::get_rules).put(api::loot::put_rules),
        )
        .route("/filters", get(api::loot::get_filters))
        .route("/filters/:character", put(api::loot::put_filter))
        .route(
            "/master-looter",
            get(api::loot::get_master_looter).put(api::loot::put_master_looter),
        )
        .route(
            "/distribution",
            get(api::loot::get_distribution).put(api::loot::put_distribution),
        )
        .route("/history", get(api::loot::get_history))
        .with_state(loot_state);

    // Restrict CORS to trusted local dashboard origins so cross-site pages
    // cannot issue authenticated-like write requests against localhost APIs.
    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://127.0.0.1:3001"),
            HeaderValue::from_static("http://localhost:3001"),
        ])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    let app = Router::new()
        // Health + sessions
        .route("/api/health", get(api::health))
        .route("/api/sessions", get(api::list_sessions))
        .route("/api/config/characters", get(api::list_character_configs))
        .route(
            "/api/config/characters/{name}",
            put(api::put_character_config),
        )
        // Economy endpoints
        .route(
            "/api/economy/settings",
            get(api::get_economy_settings).put(api::put_economy_settings),
        )
        .route(
            "/api/economy/vendor-routes",
            get(api::list_vendor_routes).post(api::create_vendor_route),
        )
        .route(
            "/api/economy/vendor-routes/{id}",
            put(api::update_vendor_route).delete(api::delete_vendor_route),
        )
        .route("/api/economy/wealth", get(api::get_wealth))
        .nest("/api/loot", loot_router)
        .route("/ws", get(ws::ws_handler))
        .fallback_service(serve_spa)
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    tracing::info!("TextQuest web dashboard listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
