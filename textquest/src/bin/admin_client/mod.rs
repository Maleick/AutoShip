//! Admin API client for textquest-admin CLI.

#![allow(dead_code)]

use serde::Deserialize;
use std::io::Read;
use textquest_common::api_types::ErrorResponse;

pub struct AdminClient {
    base_url: String,
    origin: String,
}

#[derive(Debug, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub character_name: String,
    pub class_name: Option<String>,
    pub group_id: Option<u8>,
    pub routing_scope: Option<String>,
    pub lifecycle: Option<String>,
    pub status: Option<String>,
    pub zone: Option<String>,
    pub level: Option<u32>,
    pub last_heartbeat: Option<String>,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct ConfigAudit {
    pub character_name: String,
    pub class_name: String,
    pub group_id: u8,
    pub items: Vec<ConfigAuditItem>,
    pub issues: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigAuditItem {
    pub name: String,
    pub consistent: bool,
    pub detail: String,
}

#[derive(Debug, Deserialize)]
pub struct SessionLifecycleResponse {
    pub session_id: u32,
    pub operation: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct BackupId {
    pub backup_id: String,
}

#[derive(Debug, Deserialize)]
pub struct BackupList {
    pub backups: Vec<BackupId>,
}

#[derive(Debug, Deserialize)]
pub struct BackupRestoreResponse {
    pub session_id: u32,
    pub backup_id: String,
    pub message: String,
}

impl AdminClient {
    pub fn new(base_url: &str) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        let origin = origin_from_base_url(&base_url);
        Self { base_url, origin }
    }

    fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        let response = ureq::get(&url)
            .call()
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let mut body = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut body)
            .map_err(|e| format!("Failed to read response: {}", e))?;

        if status == 200 {
            serde_json::from_slice(&body).map_err(|e| format!("Failed to parse response: {}", e))
        } else {
            let err: ErrorResponse = serde_json::from_slice(&body).unwrap_or(ErrorResponse {
                error: format!("HTTP error: {}", status),
            });
            Err(err.error)
        }
    }

    fn post<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);
        let response = ureq::post(&url)
            .set("Origin", &self.origin)
            .call()
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let mut body = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut body)
            .map_err(|e| format!("Failed to read response: {}", e))?;

        if status == 200 || status == 202 {
            serde_json::from_slice(&body).map_err(|e| format!("Failed to parse response: {}", e))
        } else {
            let err: ErrorResponse = serde_json::from_slice(&body).unwrap_or(ErrorResponse {
                error: format!("HTTP error: {}", status),
            });
            Err(err.error)
        }
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionInfo>, String> {
        self.get("/api/admin/sessions")
    }

    pub fn get_diagnostics(&self, session_id: u32) -> Result<Diagnostics, String> {
        self.get(&format!("/api/admin/diagnostics/{}", session_id))
    }

    pub fn get_logs(&self, session_id: u32, lines: u32) -> Result<Vec<String>, String> {
        self.get(&format!("/api/admin/logs/{}?lines={}", session_id, lines))
    }

    pub fn get_config_audit(&self, session_id: u32) -> Result<ConfigAudit, String> {
        self.get(&format!("/api/admin/config/audit/{}", session_id))
    }

    pub fn start_session(&self, session_id: u32) -> Result<SessionLifecycleResponse, String> {
        self.post(&format!("/api/admin/sessions/{}/start", session_id))
    }

    pub fn stop_session(&self, session_id: u32) -> Result<SessionLifecycleResponse, String> {
        self.post(&format!("/api/admin/sessions/{}/stop", session_id))
    }

    pub fn restart_session(&self, session_id: u32) -> Result<SessionLifecycleResponse, String> {
        self.post(&format!("/api/admin/sessions/{}/restart", session_id))
    }

    pub fn create_backup(&self, session_id: u32) -> Result<BackupId, String> {
        self.post(&format!("/api/admin/sessions/{}/backups", session_id))
    }

    pub fn list_backups(&self, session_id: u32) -> Result<BackupList, String> {
        self.get(&format!("/api/admin/sessions/{}/backups", session_id))
    }

    pub fn restore_backup(
        &self,
        session_id: u32,
        backup_id: &str,
    ) -> Result<BackupRestoreResponse, String> {
        self.post(&format!(
            "/api/admin/sessions/{}/backups/{}/restore",
            session_id, backup_id
        ))
    }
}

fn origin_from_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return trimmed.to_string();
    };
    let authority = rest.split('/').next().unwrap_or(rest);
    if authority.is_empty() {
        trimmed.to_string()
    } else {
        format!("{scheme}://{authority}")
    }
}

fn main() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_id_deserialization() {
        let json = r#"{"backup_id":"backup-2024-04-18-123456"}"#;
        let backup: BackupId = serde_json::from_str(json).expect("parse backup_id");
        assert_eq!(backup.backup_id, "backup-2024-04-18-123456");
    }

    #[test]
    fn test_backup_list_deserialization() {
        let json = r#"{
            "backups": [
                {"backup_id":"backup-2024-04-18-123456"},
                {"backup_id":"backup-2024-04-17-654321"}
            ]
        }"#;
        let list: BackupList = serde_json::from_str(json).expect("parse backup list");
        assert_eq!(list.backups.len(), 2);
        assert_eq!(list.backups[0].backup_id, "backup-2024-04-18-123456");
        assert_eq!(list.backups[1].backup_id, "backup-2024-04-17-654321");
    }

    #[test]
    fn test_backup_restore_response_deserialization() {
        let json = r#"{
            "session_id":42,
            "backup_id":"backup-2024-04-18-123456",
            "message":"Restore request queued for session 42"
        }"#;
        let resp: BackupRestoreResponse =
            serde_json::from_str(json).expect("parse restore response");
        assert_eq!(resp.session_id, 42);
        assert_eq!(resp.backup_id, "backup-2024-04-18-123456");
        assert!(resp.message.contains("Restore"));
    }

    #[test]
    fn session_info_deserializes_backend_admin_record() {
        let json = r#"{
            "session_id": "session-42",
            "character_name": "Cleric",
            "profile": null,
            "group_id": 2,
            "routing_scope": "group:2:main",
            "lifecycle": "active",
            "status": "active",
            "zone": "Plane of Knowledge",
            "level": 65,
            "class_name": "CLR",
            "last_heartbeat": null
        }"#;

        let session: SessionInfo = serde_json::from_str(json).expect("parse admin session");

        assert_eq!(session.session_id, "session-42");
        assert_eq!(session.character_name, "Cleric");
        assert_eq!(session.class_name.as_deref(), Some("CLR"));
        assert_eq!(session.group_id, Some(2));
        assert_eq!(session.routing_scope.as_deref(), Some("group:2:main"));
        assert_eq!(session.lifecycle.as_deref(), Some("active"));
    }

    #[test]
    fn diagnostics_deserializes_backend_payload() {
        let json = r#"{
            "session_id": 42,
            "uptime_seconds": 3600,
            "character_name": "Cleric",
            "zone": "Plane of Knowledge",
            "hp": 88.5,
            "mana": 76.0,
            "action_count": 12,
            "error_count": 1
        }"#;

        let diagnostics: Diagnostics = serde_json::from_str(json).expect("parse diagnostics");

        assert_eq!(diagnostics.session_id, 42);
        assert_eq!(diagnostics.uptime_seconds, 3600);
        assert_eq!(diagnostics.character_name, "Cleric");
        assert_eq!(diagnostics.zone, "Plane of Knowledge");
        assert_eq!(diagnostics.hp, 88.5);
        assert_eq!(diagnostics.mana, 76.0);
        assert_eq!(diagnostics.action_count, 12);
        assert_eq!(diagnostics.error_count, 1);
    }

    #[test]
    fn admin_client_normalizes_base_url_and_origin() {
        let client = AdminClient::new("http://127.0.0.1:3001/api/");

        assert_eq!(client.base_url, "http://127.0.0.1:3001/api");
        assert_eq!(client.origin, "http://127.0.0.1:3001");
    }

    #[test]
    fn origin_from_base_url_handles_plain_host_urls() {
        assert_eq!(
            origin_from_base_url("http://localhost:3001/"),
            "http://localhost:3001"
        );
        assert_eq!(
            origin_from_base_url("https://example.invalid/admin/api"),
            "https://example.invalid"
        );
    }
}
