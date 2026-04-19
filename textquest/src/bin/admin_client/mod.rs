//! Admin API client for textquest-admin CLI.

#![allow(dead_code)]

use serde::Deserialize;
use std::io::Read;

pub struct AdminClient {
    base_url: String,
}

#[derive(Debug, Deserialize)]
pub struct SessionInfo {
    pub session_id: u32,
    pub character_name: Option<String>,
    pub class_name: Option<String>,
    pub group_id: u8,
    pub routing_scope: RoutingScope,
    pub lifecycle_state: String,
}

#[derive(Debug, Deserialize)]
pub struct RoutingScope {
    pub kind: String,
    pub label: String,
    pub group_id: Option<u8>,
    pub toon_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Diagnostics {
    pub memory_mb: u64,
    pub cpu_percent: f32,
    pub ipc_latency_p50: f64,
    pub ipc_latency_p95: f64,
    pub ipc_latency_p99: f64,
    pub status: String,
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

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl AdminClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
        }
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

    pub fn restore_backup(&self, session_id: u32, backup_id: &str) -> Result<BackupRestoreResponse, String> {
        self.post(&format!("/api/admin/sessions/{}/backups/{}/restore", session_id, backup_id))
    }
}

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
        let resp: BackupRestoreResponse = serde_json::from_str(json).expect("parse restore response");
        assert_eq!(resp.session_id, 42);
        assert_eq!(resp.backup_id, "backup-2024-04-18-123456");
        assert!(resp.message.contains("Restore"));
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
}
