//! Admin-only API handlers backed by orchestrator-managed session snapshots.

use std::{path::Path, sync::Arc};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use serde::Serialize;

use crate::AppState;

use super::json_error;

/// Checks admin authentication independently of the global auth_disabled flag.
/// Admin endpoints should always require valid credentials, even in dev mode.
///
/// Returns Ok(()) if auth passes, or Err(Response) if it fails.
fn check_admin_auth(state: &Arc<AppState>, headers: &HeaderMap) -> Result<(), impl IntoResponse> {
    match state.api_token {
        Some(ref expected_token) => {
            let provided = headers.get("x-api-token").and_then(|v| v.to_str().ok());

            match provided {
                Some(token)
                    if textquest_common::crypto::cmp::constant_time_eq(
                        token.as_bytes(),
                        expected_token.as_bytes(),
                    ) =>
                {
                    Ok(())
                }
                _ => {
                    tracing::warn!("Admin request rejected: missing or invalid X-API-Token");
                    Err(json_error(
                        StatusCode::UNAUTHORIZED,
                        "Admin access requires valid X-API-Token header",
                    ))
                }
            }
        }
        None => {
            tracing::error!("Admin request rejected: TEXTQUEST_API_TOKEN is not configured");
            Err(json_error(
                StatusCode::UNAUTHORIZED,
                "Admin access requires TEXTQUEST_API_TOKEN to be configured",
            ))
        }
    }
}

/// Stable lifecycle label persisted by the orchestrator for admin inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum AdminSessionLifecycle {
    Active,
    Paused,
    Error,
}

impl AdminSessionLifecycle {
    fn as_str(self) -> &'static str {
        match self {
            AdminSessionLifecycle::Active => "active",
            AdminSessionLifecycle::Paused => "paused",
            AdminSessionLifecycle::Error => "error",
        }
    }
}

/// Stable routing-scope kind persisted by the orchestrator for admin
/// inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum AdminRoutingScopeKind {
    OneToon,
    Group,
    AllSession,
}

/// Admin-safe routing summary for one managed session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct AdminRoutingScopeSnapshot {
    kind: AdminRoutingScopeKind,
    label: String,
    group_id: Option<u8>,
    toon_name: Option<String>,
}

/// Stable admin snapshot for one orchestrator-managed session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct AdminSessionSnapshot {
    session_id: u32,
    character_name: Option<String>,
    class_name: Option<String>,
    group_id: u8,
    routing_scope: AdminRoutingScopeSnapshot,
    lifecycle_state: AdminSessionLifecycle,
}

fn read_admin_sessions(path: &Path) -> anyhow::Result<Vec<AdminSessionSnapshot>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let payload = std::fs::read(path)?;
    let sessions = serde_json::from_slice::<Vec<AdminSessionSnapshot>>(&payload)?;
    Ok(sessions)
}

/// Convert an EQ class ID to a short class name string (e.g. `2` → `"CLR"`).
///
/// Returns `None` for class_id 0 (unknown / not yet loaded).
fn class_name_from_id(class_id: u8) -> Option<String> {
    let label = match class_id {
        1 => "WAR",
        2 => "CLR",
        3 => "PAL",
        4 => "RNG",
        5 => "SK",
        6 => "DRU",
        7 => "MNK",
        8 => "BRD",
        9 => "ROG",
        10 => "SHM",
        11 => "NEC",
        12 => "WIZ",
        13 => "MAG",
        14 => "ENC",
        15 => "BST",
        16 => "BER",
        _ => return None,
    };
    Some(label.to_string())
}

/// List orchestrator-managed session inventory for the admin dashboard.
pub async fn list_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match read_admin_sessions(&state.admin_session_snapshot_path) {
        Ok(sessions) => (StatusCode::OK, Json(sessions)).into_response(),
        Err(error) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read admin session inventory: {error}"),
        )
        .into_response(),
    }
}

/// Admin session record for inventory and monitoring.
#[derive(Debug, Clone, Serialize)]
pub struct AdminSessionRecord {
    pub session_id: String,
    pub character_name: String,
    pub profile: Option<String>,
    pub group_id: Option<u8>,
    pub routing_scope: Option<String>,
    pub lifecycle: Option<String>,
    pub status: Option<String>,
    pub zone: Option<String>,
    pub level: Option<u32>,
    pub class_name: Option<String>,
    pub last_heartbeat: Option<String>,
}

/// Get all managed sessions for the admin dashboard.
///
/// **Authentication:** Requires valid X-API-Token header, independent of global auth_disabled flag.
/// Admin endpoints must always be protected, even in dev/test modes.
///
/// Priority order for session data:
/// 1. Admin session snapshot (`admin_session_snapshot_path`) — written by the
///    orchestrator and contains fully-enriched records (group_id, routing_scope,
///    class_name, lifecycle).  Used when available.
/// 2. Live session snapshot (`live_session_snapshot_path`) — enriched inline
///    from `session_control_state` (group_id, routing_scope) and derived class
///    name from `class_id`.
/// 3. Character config entries — placeholder records for demo / offline mode.
pub async fn list_admin_sessions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Guard: admin endpoints always require auth, regardless of global auth_disabled flag
    if let Err(error_response) = check_admin_auth(&state, &headers) {
        return error_response.into_response();
    }
    // ── Priority 1: admin snapshot (orchestrator-managed, fully enriched) ──
    if let Ok(admin_sessions) = read_admin_sessions(&state.admin_session_snapshot_path)
        && !admin_sessions.is_empty()
    {
        let records: Vec<AdminSessionRecord> = admin_sessions
            .into_iter()
            .map(|s| AdminSessionRecord {
                session_id: format!("session-{}", s.session_id),
                character_name: s.character_name.unwrap_or_default(),
                profile: None,
                group_id: Some(s.group_id),
                routing_scope: Some(s.routing_scope.label),
                lifecycle: Some(s.lifecycle_state.as_str().to_string()),
                status: Some(s.lifecycle_state.as_str().to_string()),
                zone: None,
                level: None,
                class_name: s.class_name,
                last_heartbeat: None,
            })
            .collect();
        return Json(records).into_response();
    }

    // ── Priority 2: live snapshot enriched with session_control_state ──
    if let Ok(live_sessions) = super::read_live_sessions(&state.live_session_snapshot_path)
        && !live_sessions.is_empty()
    {
        // Snapshot the control records once to avoid repeated lock acquisitions.
        let control_records: std::collections::HashMap<u32, (u8, String)> = {
            let records = state
                .session_control_state
                .records
                .lock()
                .expect("session_control_state lock poisoned");
            records
                .values()
                .map(|r| {
                    let scope_label = match &r.routing_scope {
                        super::session_control::RoutingScope::AllSession => {
                            "all_session".to_string()
                        }
                        super::session_control::RoutingScope::Group { group_id, label } => {
                            format!("group:{group_id}:{label}")
                        }
                    };
                    (r.session_id, (r.group_id, scope_label))
                })
                .collect()
        };

        let sessions: Vec<AdminSessionRecord> = live_sessions
            .into_iter()
            .map(|session| {
                let client_id = session.client_id;
                let (group_id, routing_scope) = control_records
                    .get(&client_id)
                    .map(|(g, s)| (Some(*g), Some(s.clone())))
                    .unwrap_or((None, None));

                AdminSessionRecord {
                    session_id: format!("session-{}", client_id),
                    character_name: session.character_name,
                    profile: None,
                    group_id,
                    routing_scope,
                    lifecycle: Some(session.status.clone()),
                    status: Some(session.status),
                    zone: if session.zone_long_name.is_empty() {
                        Some(session.zone_short_name)
                    } else {
                        Some(session.zone_long_name)
                    },
                    level: Some(session.level as u32),
                    class_name: class_name_from_id(session.class_id),
                    last_heartbeat: None,
                }
            })
            .collect();
        return Json(sessions).into_response();
    }

    // ── Priority 3: configured sessions (offline / demo placeholder) ──
    let configs = state.character_configs.read().await;
    let sessions: Vec<AdminSessionRecord> = configs
        .iter()
        .enumerate()
        .map(|(idx, (_name, cfg))| AdminSessionRecord {
            session_id: format!("session-{}", idx + 1),
            character_name: cfg.character_name.clone(),
            profile: None,
            group_id: None,
            routing_scope: None,
            lifecycle: Some("idle".to_string()),
            status: Some("offline".to_string()),
            zone: None,
            level: None,
            class_name: None,
            last_heartbeat: None,
        })
        .collect();

    Json(sessions).into_response()
}

/// Build the admin sub-router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/sessions", get(list_admin_sessions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_admin_sessions_returns_empty_inventory_when_file_is_missing() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let missing = tempdir.path().join("missing-admin-sessions.json");

        let sessions = read_admin_sessions(&missing).expect("missing file should be empty");

        assert!(sessions.is_empty());
    }

    #[test]
    fn read_admin_sessions_parses_persisted_inventory() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let snapshot_path = tempdir.path().join("admin-sessions.json");
        std::fs::write(
            &snapshot_path,
            serde_json::to_vec(&vec![serde_json::json!({
                "session_id": 4242,
                "character_name": "Cleric42",
                "class_name": "Cleric",
                "group_id": 2,
                "routing_scope": {
                    "kind": "group",
                    "label": "G2",
                    "group_id": 2,
                    "toon_name": null
                },
                "lifecycle_state": "paused"
            })])
            .expect("admin snapshot json"),
        )
        .expect("write snapshot");

        let sessions = read_admin_sessions(&snapshot_path).expect("snapshot should parse");

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, 4242);
        assert_eq!(sessions[0].character_name.as_deref(), Some("Cleric42"));
        assert_eq!(sessions[0].class_name.as_deref(), Some("Cleric"));
        assert_eq!(sessions[0].group_id, 2);
        assert_eq!(sessions[0].routing_scope.kind, AdminRoutingScopeKind::Group);
    }

    #[test]
    fn class_name_from_id_maps_known_classes() {
        assert_eq!(class_name_from_id(1).as_deref(), Some("WAR"));
        assert_eq!(class_name_from_id(2).as_deref(), Some("CLR"));
        assert_eq!(class_name_from_id(6).as_deref(), Some("DRU"));
        assert_eq!(class_name_from_id(13).as_deref(), Some("MAG"));
        assert_eq!(class_name_from_id(16).as_deref(), Some("BER"));
    }

    #[test]
    fn class_name_from_id_returns_none_for_unknown() {
        assert!(class_name_from_id(0).is_none());
        assert!(class_name_from_id(17).is_none());
        assert!(class_name_from_id(255).is_none());
    }

    #[test]
    fn admin_session_record_group_id_is_u8() {
        // Verify AdminSessionRecord serializes group_id as a number.
        let record = AdminSessionRecord {
            session_id: "session-1".to_string(),
            character_name: "Warrior1".to_string(),
            profile: None,
            group_id: Some(3),
            routing_scope: Some("group:3:G3".to_string()),
            lifecycle: Some("active".to_string()),
            status: Some("active".to_string()),
            zone: Some("crushbone".to_string()),
            level: Some(20),
            class_name: Some("WAR".to_string()),
            last_heartbeat: None,
        };
        let json = serde_json::to_value(&record).expect("serialize");
        assert_eq!(json["group_id"], 3);
        assert_eq!(json["class_name"], "WAR");
        assert_eq!(json["routing_scope"], "group:3:G3");
    }

    #[tokio::test]
    async fn list_admin_sessions_rejects_unauthenticated_access() {
        use axum::{
            body::Body,
            http::{Request, StatusCode},
        };
        use tower::ServiceExt;

        let state = crate::test_support::demo_app_state();
        let app = router().with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/sessions")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "admin sessions endpoint should require X-API-Token"
        );
    }

    #[tokio::test]
    async fn list_admin_sessions_accepts_valid_token() {
        use axum::{
            body::Body,
            http::{Request, StatusCode},
        };
        use tower::ServiceExt;

        let state = Arc::new({
            let mut s = crate::test_app_state();
            s.auth_disabled = false;
            s.api_token = Some("test-secret-token".to_string());
            s
        });
        let app = router().with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/sessions")
                    .header("x-api-token", "test-secret-token")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "admin sessions endpoint should accept valid token"
        );
    }
}
