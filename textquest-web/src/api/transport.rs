//! REST API for cross-client transport configuration.
//!
//! `GET  /api/transport/config`   — read current transport config
//! `PUT  /api/transport/config`   — update transport config (persisted)
//! `GET  /api/transport/peers`    — list currently connected EQBC + DanNet peers
//! `GET  /api/transport/vitals`   — NetBots-style vitals snapshot for all peers
//! `POST /api/transport/bc`       — issue a /bc broadcast command
//! `POST /api/transport/bct`      — issue a /bct targeted command
//! `POST /api/transport/dgae`     — issue a /dgae group-execute command

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

/// Minimal transport state exposed to the web layer.  The full runtime handle
/// lives in the orchestrator; this module expects a thin `Arc<TransportState>`
/// in the Axum router state.
///
/// Wire types are defined here for the web layer; the cross-client
/// transport implementation (EQBC/DanNet/NetBots) lives outside this crate.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TransportConfigView {
    /// Active transport: "eqbc" | "dannet" | "both"
    pub kind: String,
    /// EQBC host (e.g. "192.168.1.10")
    pub eqbc_host: String,
    /// EQBC port (default 2112)
    pub eqbc_port: u16,
    /// Whether the EQBC client auto-connects on startup
    pub eqbc_auto_connect: bool,
    /// Whether this instance acts as an EQBC hub server
    pub eqbc_serve: bool,
    /// DanNet UDP port (default 2114)
    pub dannet_port: u16,
    /// DanNet server group name
    pub dannet_group: String,
    /// Static DanNet peer addresses ("host:port")
    pub dannet_peers: Vec<String>,
    /// Whether NetBots vitals broadcast is enabled
    pub netbots_enabled: bool,
    /// NetBots publish interval cap in milliseconds (max 250)
    pub netbots_interval_ms: u64,
}

/// Peer entry returned by `GET /api/transport/peers`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerView {
    pub name: String,
    pub transport: String,
    pub address: String,
    pub online: bool,
}

/// NetBots vitals entry returned by `GET /api/transport/vitals`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VitalsView {
    pub name: String,
    pub class: String,
    pub level: u8,
    pub zone: String,
    pub hp_pct: f32,
    pub mana_pct: f32,
    pub end_pct: f32,
    pub target_name: String,
    pub target_hp_pct: f32,
    pub published_at_ms: u64,
}

/// Body for `POST /api/transport/bc`.
#[derive(Debug, Deserialize)]
pub struct BcRequest {
    pub command: String,
}

/// Body for `POST /api/transport/bct`.
#[derive(Debug, Deserialize)]
pub struct BctRequest {
    pub target: String,
    pub command: String,
}

/// Body for `POST /api/transport/dgae`.
#[derive(Debug, Deserialize)]
pub struct DgaeRequest {
    pub command: String,
    /// Optional override group; defaults to the node's configured group.
    #[serde(default)]
    pub group: Option<String>,
}

/// Stub app state — replace with the real `Arc<TransportHandle>` once the
/// orchestrator wires up the cross-client transport layer.
#[derive(Debug, Clone, Default)]
pub struct TransportAppState {
    pub config: TransportConfigView,
}

async fn get_config(State(state): State<TransportAppState>) -> Json<TransportConfigView> {
    Json(state.config)
}

async fn put_config(
    State(_state): State<TransportAppState>,
    Json(body): Json<TransportConfigView>,
) -> impl IntoResponse {
    // Validate interval cap
    if body.netbots_interval_ms > 250 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({
                "error": "netbots_interval_ms must be ≤ 250 to meet latency requirements"
            })),
        )
            .into_response();
    }
    // TODO: persist via the orchestrator config layer and restart transport
    (StatusCode::OK, Json(body)).into_response()
}

async fn get_peers(_state: State<TransportAppState>) -> Json<Vec<PeerView>> {
    // TODO: wire to live EQBC peer list + DanNet peer registry
    Json(vec![])
}

async fn get_vitals(_state: State<TransportAppState>) -> Json<Vec<VitalsView>> {
    // TODO: wire to VitalsRegistry snapshot
    Json(vec![])
}

async fn post_bc(
    _state: State<TransportAppState>,
    Json(body): Json<BcRequest>,
) -> impl IntoResponse {
    // TODO: forward to EqbcClient::bc()
    tracing::info!("API /bc: {}", body.command);
    StatusCode::ACCEPTED
}

async fn post_bct(
    _state: State<TransportAppState>,
    Json(body): Json<BctRequest>,
) -> impl IntoResponse {
    // TODO: forward to EqbcClient::bct()
    tracing::info!("API /bct {} -> {}", body.target, body.command);
    StatusCode::ACCEPTED
}

async fn post_dgae(
    _state: State<TransportAppState>,
    Json(body): Json<DgaeRequest>,
) -> impl IntoResponse {
    // TODO: forward to DanNetNode::dgae() or dggaexecute()
    tracing::info!("API /dgae {:?}: {}", body.group, body.command);
    StatusCode::ACCEPTED
}

/// Build the transport API sub-router.  Mount at `/api/transport` in the main
/// Axum router.
pub fn router(state: TransportAppState) -> Router {
    Router::new()
        .route("/config", get(get_config).put(put_config))
        .route("/peers", get(get_peers))
        .route("/vitals", get(get_vitals))
        .route("/bc", post(post_bc))
        .route("/bct", post(post_bct))
        .route("/dgae", post(post_dgae))
        .with_state(state)
}
