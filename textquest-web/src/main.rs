//! TextQuest Web Dashboard — Axum backend for the monitoring UI.
//!
//! The backend serves:
//! - live health plus explicit `501` responses for unsupported session and
//!   economy endpoints
//! - loot APIs
//! - account-management APIs backed by an in-memory registry plus optional
//!   credential storage
//! - explicit `501` placeholders for not-yet-implemented raid and character
//!   configuration APIs
//! - a WebSocket endpoint for live session monitoring

use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    extract::Request,
    http::{HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, put},
};
use tokio::sync::broadcast;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};

mod accounts;
mod api;
mod ws;

/// Shared application state accessible from all handlers.
pub struct AppState {
    /// Broadcast channel for real-time session events.
    pub event_tx: broadcast::Sender<String>,
    /// In-memory account registry.
    pub account_store: Mutex<accounts::AccountStore>,
    /// Optional encrypted password store, enabled by
    /// `TEXTQUEST_MASTER_PASSWORD`.
    pub credential_store: Option<accounts::CredentialStore>,
    /// In-memory character tuning config store for the strategy tuning panel.
    pub character_configs: tokio::sync::RwLock<HashMap<String, api::CharacterConfig>>,
    /// In-memory loot configuration state.
    pub loot_state: Arc<api::loot::LootState>,
    /// In-memory economy cycle state.
    pub economy_state: Arc<api::economy::EconomyState>,
    /// In-memory operator dashboard snapshot and action state.
    pub dashboard_state: Arc<api::dashboard::DashboardState>,
    /// In-memory soul audit log.
    pub soul_audit: Arc<api::soul::SoulAuditState>,
    /// Optional static API token for protecting all `/api` endpoints.
    /// Set via `TEXTQUEST_API_TOKEN` environment variable.
    /// When `None`, API endpoints are unauthenticated (localhost-only
    /// deployment).
    pub api_token: Option<String>,
}

/// Axum middleware: enforce `X-API-Token` header when `TEXTQUEST_API_TOKEN` is
/// set.
///
/// If the env var is unset, all requests pass through (backward-compatible
/// default). When set, requests without a matching token receive `401
/// Unauthorized`.
async fn api_token_auth(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(ref expected_token) = state.api_token {
        let provided = req
            .headers()
            .get("x-api-token")
            .and_then(|v| v.to_str().ok());

        match provided {
            Some(token) if constant_time_eq_str(token, expected_token) => {}
            _ => {
                tracing::warn!(
                    path = req.uri().path(),
                    "API request rejected: missing or invalid X-API-Token"
                );
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    }
    Ok(next.run(req).await)
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

fn credentials_db_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/credentials.db")
}

/// Build the initial application state for production use.
///
/// - `event_tx` and `account_store` are always initialised empty.
/// - `credential_store` is populated only when `TEXTQUEST_MASTER_PASSWORD` is
///   set in the environment; otherwise password routes return `501`.
/// - `character_configs`, `loot_state`, `economy_state`, and `soul_audit` are
///   seeded with in-memory state; character-config routes still return `501`
///   until a supported backing store is wired.
fn build_state() -> Arc<AppState> {
    let (event_tx, _) = broadcast::channel::<String>(256);
    let credential_store = std::env::var("TEXTQUEST_MASTER_PASSWORD")
        .ok()
        .filter(|password| !password.trim().is_empty())
        .and_then(
            |password| match accounts::CredentialStore::open(&credentials_db_path(), &password) {
                Ok(store) => Some(store),
                Err(error) => {
                    tracing::error!(%error, "Failed to initialize credential store; password routes will return 501");
                    None
                }
            },
        );

    let api_token = std::env::var("TEXTQUEST_API_TOKEN")
        .ok()
        .filter(|t| !t.trim().is_empty());

    if api_token.is_none() {
        tracing::warn!(
            "TEXTQUEST_API_TOKEN is not set — API endpoints are unauthenticated. Set this env var \
             to enable token-based authentication."
        );
    }

    Arc::new(AppState {
        event_tx,
        account_store: Mutex::new(accounts::AccountStore::default()),
        credential_store,
        character_configs: tokio::sync::RwLock::new(api::demo_character_configs()),
        loot_state: api::loot::LootState::new_demo(),
        economy_state: api::economy::EconomyState::new_demo(),
        dashboard_state: api::dashboard::DashboardState::new_demo(),
        soul_audit: api::soul::SoulAuditState::new_demo(),
        api_token,
    })
}

/// Build the soul audit sub-router.
fn build_soul_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/audit", get(api::soul::get_all_audit))
        .route("/audit/export.csv", get(api::soul::export_all_audit_csv))
        .route("/audit/{character_id}", get(api::soul::get_character_audit))
        .route(
            "/audit/{character_id}/export.csv",
            get(api::soul::export_character_audit_csv),
        )
}

/// Build the loot sub-router.  Loot handlers extract `State<Arc<AppState>>`
/// and access `state.loot_state`, so this router shares the same state type
/// as the rest of the API — eliminating the type mismatch that arose when
/// `Arc<LootState>` was provided as its own separate state.
fn build_loot_router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/rules",
            get(api::loot::get_rules).put(api::loot::put_rules),
        )
        .route("/filters", get(api::loot::get_filters))
        .route("/filters/{character}", put(api::loot::put_filter))
        .route(
            "/master-looter",
            get(api::loot::get_master_looter).put(api::loot::put_master_looter),
        )
        .route(
            "/distribution",
            get(api::loot::get_distribution).put(api::loot::put_distribution),
        )
        .route("/history", get(api::loot::get_history))
}

fn build_api_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(api::health))
        .route("/sessions", get(api::list_sessions))
        .nest("/dashboard", api::dashboard::router())
        .nest("/accounts", accounts::router())
        .route(
            "/economy/settings",
            get(api::get_economy_settings).put(api::put_economy_settings),
        )
        .route(
            "/economy/vendor-routes",
            get(api::list_vendor_routes).post(api::create_vendor_route),
        )
        .route(
            "/economy/vendor-routes/{id}",
            put(api::update_vendor_route).delete(api::delete_vendor_route),
        )
        .route("/economy/wealth", get(api::get_wealth))
        .route("/soul", get(api::soul::list_soul_states))
        .route("/soul/{character_id}", get(api::soul::get_soul_state))
        .route(
            "/raid/config",
            get(api::raid_config_unavailable).put(api::raid_config_unavailable),
        )
        .route(
            "/config/characters",
            get(api::character_configs_unavailable),
        )
        .route(
            "/config/characters/{character}",
            put(api::character_config_unavailable),
        )
        .nest("/loot", build_loot_router())
        .nest("/soul", build_soul_router())
        .fallback(api::api_not_found)
}

fn build_app(state: Arc<AppState>) -> Router {
    let spa_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/dist");
    let serve_spa =
        ServeDir::new(&spa_dir).not_found_service(ServeFile::new(spa_dir.join("index.html")));

    // Restrict CORS to trusted local dashboard origins so cross-site pages
    // cannot issue authenticated-like write requests against localhost APIs.
    // Derived from `api::loot::TRUSTED_ORIGINS` so CORS middleware and the
    // per-handler origin guard always use the same allowlist.
    let cors = CorsLayer::new()
        .allow_origin(
            api::loot::TRUSTED_ORIGINS
                .iter()
                .map(|&o| HeaderValue::from_static(o))
                .collect::<Vec<_>>(),
        )
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    Router::new()
        // Apply API token authentication to all /api routes before routing.
        // The middleware is a no-op when TEXTQUEST_API_TOKEN is unset (backward compatible).
        .nest(
            "/api",
            build_api_router().layer(middleware::from_fn_with_state(
                state.clone(),
                api_token_auth,
            )),
        )
        .route("/ws", get(ws::ws_handler))
        .fallback_service(serve_spa)
        .layer(cors)
        .with_state(state)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("textquest_web=debug,tower_http=debug")
        .init();

    let state = build_state();
    api::dashboard::spawn_dashboard_tick_loop(state.clone());
    let app = build_app(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    tracing::info!("TextQuest web dashboard listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    async fn json_response(app: Router, request: Request<Body>) -> (StatusCode, Value) {
        let response = app.oneshot(request).await.expect("request should succeed");
        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should collect")
            .to_bytes();
        let value = serde_json::from_slice(&body).expect("body should be valid json");
        (status, value)
    }

    fn test_state_with_credentials(path: &std::path::Path) -> Arc<AppState> {
        let (event_tx, _) = broadcast::channel::<String>(8);
        Arc::new(AppState {
            event_tx,
            account_store: Mutex::new(accounts::AccountStore::default()),
            credential_store: Some(
                accounts::CredentialStore::open(path, "test_master_pw").expect("credential store"),
            ),
            character_configs: tokio::sync::RwLock::new(api::demo_character_configs()),
            loot_state: api::loot::LootState::new_demo(),
            economy_state: api::economy::EconomyState::new_demo(),
            dashboard_state: api::dashboard::DashboardState::new_demo(),
            soul_audit: api::soul::SoulAuditState::new_demo(),
            api_token: None, // No auth in tests — auth middleware is a no-op when None
        })
    }

    #[tokio::test]
    async fn unknown_api_route_returns_json_404() {
        let app = build_app(build_state());
        let (status, body) = json_response(
            app,
            Request::builder()
                .uri("/api/not-real")
                .body(Body::empty())
                .expect("request"),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "API route not found");
    }

    #[tokio::test]
    async fn known_unimplemented_routes_return_json_501() {
        let app = build_app(build_state());
        let (status, body) = json_response(
            app.clone(),
            Request::builder()
                .uri("/api/raid/config")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert!(
            body["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not implemented")
        );

        let (status, body) = json_response(
            app,
            Request::builder()
                .uri("/api/config/characters")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert!(
            body["error"]
                .as_str()
                .unwrap_or_default()
                .contains("not implemented")
        );
    }

    #[tokio::test]
    async fn dashboard_routes_are_mounted() {
        let app = build_app(build_state());

        let (status, body) = json_response(
            app.clone(),
            Request::builder()
                .uri("/api/dashboard")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.get("sessions").is_some());

        let (status, body) = json_response(
            app,
            Request::builder()
                .method("POST")
                .uri("/api/dashboard/action")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "type": "create_session",
                        "profile": "Loot Crew",
                        "character_name": "Newpuller"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body["sessions"]["items"]
                .as_array()
                .expect("sessions array")
                .iter()
                .any(|session| session["characterName"] == "Newpuller")
        );
    }

    #[tokio::test]
    async fn account_routes_are_mounted_and_roundtrip() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let state = test_state_with_credentials(&tempdir.path().join("creds.db"));
        let app = build_app(state);

        let (status, created) = json_response(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "wizard1",
                        "server": "Teek",
                        "character": "Starfire",
                        "class": "WIZ",
                        "group": 2,
                        "status": "active",
                        "password": "hunter2"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(created["name"], "wizard1");
        assert_eq!(created["has_password"], true);

        let (status, listed) = json_response(
            app.clone(),
            Request::builder()
                .uri("/api/accounts")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(listed.as_array().expect("array").len(), 1);

        let (status, updated) = json_response(
            app.clone(),
            Request::builder()
                .method("PUT")
                .uri("/api/accounts/wizard1")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "server": "Rizlona",
                        "group": 3
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["server"], "Rizlona");
        assert_eq!(updated["group"], 3);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/accounts/wizard1/password")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "password": "newpass" }).to_string()))
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let (status, exported) = json_response(
            app.clone(),
            Request::builder()
                .uri("/api/accounts/export")
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(exported["accounts"].as_array().expect("array").len(), 1);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/accounts/wizard1")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("request should succeed");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let (status, imported) = json_response(
            app,
            Request::builder()
                .method("POST")
                .uri("/api/accounts/import")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "accounts": exported["accounts"]
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(imported["imported"], 1);
    }

    #[tokio::test]
    async fn password_route_returns_501_without_credential_store() {
        let app = build_app(build_state());

        let _ = json_response(
            app.clone(),
            Request::builder()
                .method("POST")
                .uri("/api/accounts")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "cleric1",
                        "server": "Teek",
                        "character": "Mercy",
                        "class": "CLR",
                        "group": 1,
                        "status": "active"
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await;

        let (status, body) = json_response(
            app,
            Request::builder()
                .method("PUT")
                .uri("/api/accounts/cleric1/password")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "password": "secret" }).to_string()))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert!(
            body["error"]
                .as_str()
                .unwrap_or_default()
                .contains("TEXTQUEST_MASTER_PASSWORD")
        );
    }
}
