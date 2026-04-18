//! Admin diagnostics API — performance metrics and IPC latency for sessions.

use std::{path::Path, sync::Arc};

use axum::{
    Json, extract::Path as AxumPath, extract::State, http::StatusCode, response::IntoResponse,
};

use crate::AppState;

use super::json_error;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Diagnostics {
    pub session_id: u32,
    pub uptime_seconds: u64,
    pub character_name: String,
    pub zone: String,
    pub hp: f32,
    pub mana: f32,
    pub action_count: u64,
    pub error_count: u64,
}

fn read_live_sessions(
    path: &Path,
) -> anyhow::Result<Vec<textquest_common::shared_client_state::SharedClientState>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let payload = std::fs::read(path)?;
    serde_json::from_slice::<Vec<textquest_common::shared_client_state::SharedClientState>>(
        &payload,
    )
    .map_err(|e| anyhow::anyhow!("Failed to parse sessions: {}", e))
}

pub async fn get_diagnostics(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<u32>,
) -> impl IntoResponse {
    let sessions = match read_live_sessions(&state.live_session_snapshot_path) {
        Ok(s) => s,
        Err(e) => {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read session data: {}", e),
            )
            .into_response();
        }
    };

    let session = match sessions.iter().find(|s| s.client_id == session_id) {
        Some(s) => s,
        None => {
            return json_error(
                StatusCode::NOT_FOUND,
                format!("Session {} not found", session_id),
            )
            .into_response();
        }
    };

    let diagnostics = Diagnostics {
        session_id,
        uptime_seconds: 0, // Placeholder: would need session metadata tracking
        character_name: session.character_name.clone(),
        zone: session.zone_long_name.clone(),
        hp: session.hp_pct,
        mana: session.mana_pct,
        action_count: 0, // Placeholder: would need action counting
        error_count: 0,  // Placeholder: would need error tracking
    };

    (StatusCode::OK, Json(diagnostics)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_serializes_correctly() {
        let diag = Diagnostics {
            session_id: 42,
            uptime_seconds: 3600,
            character_name: "TestChar".to_string(),
            zone: "Plane of Knowledge".to_string(),
            hp: 85.5,
            mana: 75.0,
            action_count: 100,
            error_count: 2,
        };

        let json = serde_json::to_string(&diag).expect("serialization");
        assert!(json.contains("\"session_id\":42"));
        assert!(json.contains("\"uptime_seconds\":3600"));
        assert!(json.contains("\"character_name\":\"TestChar\""));
        assert!(json.contains("\"zone\":\"Plane of Knowledge\""));
        assert!(json.contains("\"hp\":85.5"));
        assert!(json.contains("\"mana\":75.0"));
        assert!(json.contains("\"action_count\":100"));
        assert!(json.contains("\"error_count\":2"));
    }

    #[test]
    fn diagnostics_deserializes_correctly() {
        let json = r#"{
            "session_id": 99,
            "uptime_seconds": 7200,
            "character_name": "MyCharacter",
            "zone": "Kael Drakkel",
            "hp": 50.0,
            "mana": 25.5,
            "action_count": 500,
            "error_count": 10
        }"#;

        let diag: Diagnostics = serde_json::from_str(json).expect("deserialization");
        assert_eq!(diag.session_id, 99);
        assert_eq!(diag.uptime_seconds, 7200);
        assert_eq!(diag.character_name, "MyCharacter");
        assert_eq!(diag.zone, "Kael Drakkel");
        assert_eq!(diag.hp, 50.0);
        assert_eq!(diag.mana, 25.5);
        assert_eq!(diag.action_count, 500);
        assert_eq!(diag.error_count, 10);
    }

    #[test]
    fn read_live_sessions_returns_empty_when_path_does_not_exist() {
        let path = std::path::Path::new("/nonexistent/path/to/sessions.json");
        let result = read_live_sessions(path).expect("should succeed");
        assert!(result.is_empty());
    }
}
