//! REST API handlers for the web dashboard.

use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

/// Health check endpoint.
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(Serialize)]
pub struct SessionInfo {
    pub client_id: u32,
    pub character_name: String,
    pub zone: String,
    pub level: u8,
    pub hp_pct: f32,
    pub mana_pct: f32,
    pub status: String,
}

/// List active sessions (placeholder — will connect to IPC).
pub async fn list_sessions() -> Json<Vec<SessionInfo>> {
    // TODO: Read real session data from IPC / shared memory
    Json(vec![SessionInfo {
        client_id: 1,
        character_name: "Frostreaver".into(),
        zone: "East Commonlands".into(),
        level: 50,
        hp_pct: 100.0,
        mana_pct: 85.0,
        status: "idle".into(),
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_returns_ok() {
        let Json(resp) = health().await;
        assert_eq!(resp.status, "ok");
    }

    #[tokio::test]
    async fn sessions_returns_placeholder() {
        let Json(sessions) = list_sessions().await;
        assert!(!sessions.is_empty());
        assert_eq!(sessions[0].character_name, "Frostreaver");
    }
}
