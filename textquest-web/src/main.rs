//! TextQuest Web Dashboard — Axum backend for the M6 configuration & monitoring UI.
//!
//! Serves a React SPA and provides:
//! - REST API for credentials, group config, loot tables, raid configuration
//! - WebSocket endpoint for live session monitoring
//! - DZ lockout timers, reset queue, raid instance tracking
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

// Re-export DZ types used in api.rs so tests can reference `crate::AppState`.
use api::{DzHistoryEntry, DzLockout, RaidInstance};

/// A queued DZ reset request (not yet dispatched to the game client).
#[derive(Debug, Clone)]
pub struct DzResetEntry {
    pub expedition: String,
    pub character: String,
}

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

impl AppState {
    /// Build demo-seeded state reusing an existing broadcast sender.
    fn with_demo_data_from(event_tx: broadcast::Sender<String>) -> Self {
        let now = chrono::Utc::now();

        let lockouts = vec![
            DzLockout {
                id: 1,
                character: "Frostreaver".into(),
                expedition: "Plane of Time".into(),
                expires_at: (now + chrono::Duration::hours(42)).to_rfc3339(),
                lockout_type: "48h replay".into(),
                seconds_remaining: 42 * 3600,
            },
            DzLockout {
                id: 2,
                character: "Noxus".into(),
                expedition: "Anguish".into(),
                expires_at: (now + chrono::Duration::hours(130)).to_rfc3339(),
                lockout_type: "6.5d full".into(),
                seconds_remaining: 130 * 3600,
            },
            DzLockout {
                id: 3,
                character: "Aelrindel".into(),
                expedition: "Plane of Time".into(),
                expires_at: (now + chrono::Duration::hours(6)).to_rfc3339(),
                lockout_type: "48h replay".into(),
                seconds_remaining: 6 * 3600,
            },
            DzLockout {
                id: 4,
                character: "Bloodfury".into(),
                expedition: "Vex Thal".into(),
                expires_at: (now + chrono::Duration::hours(20)).to_rfc3339(),
                lockout_type: "48h replay".into(),
                seconds_remaining: 20 * 3600,
            },
        ];

        let instances = vec![
            RaidInstance {
                id: 1,
                expedition: "Plane of Time".into(),
                zone: "potimeb".into(),
                group: "Group Alpha".into(),
                members: vec![
                    "Frostreaver".into(),
                    "Noxus".into(),
                    "Grok".into(),
                    "Valerius".into(),
                ],
                entered_at: (now - chrono::Duration::minutes(14)).to_rfc3339(),
            },
            RaidInstance {
                id: 2,
                expedition: "Vex Thal".into(),
                zone: "vexthal".into(),
                group: "Group Beta".into(),
                members: vec!["Bloodfury".into(), "Aelrindel".into()],
                entered_at: (now - chrono::Duration::minutes(3)).to_rfc3339(),
            },
        ];

        let history = vec![
            DzHistoryEntry {
                id: 1,
                expedition: "Anguish".into(),
                zone: "anguish".into(),
                participants: vec![
                    "Frostreaver".into(),
                    "Noxus".into(),
                    "Bloodfury".into(),
                    "Grok".into(),
                ],
                completed_at: (now - chrono::Duration::hours(3)).to_rfc3339(),
                duration_secs: 4523,
                loot: vec![
                    "Muramite Plate Chest Armor".into(),
                    "Wristguard of the Crimson Slayer".into(),
                ],
            },
            DzHistoryEntry {
                id: 2,
                expedition: "Plane of Time".into(),
                zone: "potimeb".into(),
                participants: vec!["Frostreaver".into(), "Aelrindel".into(), "Valerius".into()],
                completed_at: (now - chrono::Duration::hours(30)).to_rfc3339(),
                duration_secs: 9812,
                loot: vec!["Amulet of Necropotence".into()],
            },
        ];

        Self {
            event_tx,
            dz_lockouts: RwLock::new(lockouts),
            dz_instances: RwLock::new(instances),
            dz_history: RwLock::new(history),
            dz_reset_queue: RwLock::new(Vec::new()),
        }
    }
}
