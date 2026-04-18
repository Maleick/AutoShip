//! Synchronous client implementation

use crate::{ClientConfig, Error, ErrorResponse, HealthResponse, Result, SessionInfo};
use reqwest::blocking::Client;

pub struct Client {
    inner: Client,
    config: ClientConfig,
}

impl Client {
    pub fn new(config: ClientConfig) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to build HTTP client");

        Self { inner: client, config }
    }

    fn build_url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url, path)
    }

    fn add_auth(&self, mut req: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        if let Some(ref token) = self.config.api_token {
            req.header("X-API-Token", token)
        } else {
            req
        }
    }

    pub fn health(&self) -> Result<HealthResponse> {
        let url = self.build_url("/api/health");
        let response = self.add_auth(self.inner.get(&url)).send()?;
        if !response.status().is_success() {
            let error: ErrorResponse = response.json().unwrap_or(ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        let health = response.json()?;
        Ok(health)
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionInfo>> {
        let url = self.build_url("/api/sessions");
        let response = self.add_auth(self.inner.get(&url)).send()?;
        if !response.status().is_success() {
            let error: ErrorResponse = response.json().unwrap_or(ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        let sessions = response.json()?;
        Ok(sessions)
    }

    pub fn pause_session(&self, session_id: u32) -> Result<()> {
        let url = self.build_url(&format!("/api/control/pause/{}", session_id));
        let response = self.add_auth(self.inner.put(&url)).send()?;
        if !response.status().is_success() {
            let error: ErrorResponse = response.json().unwrap_or(ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        Ok(())
    }

    pub fn resume_session(&self, session_id: u32) -> Result<()> {
        let url = self.build_url(&format!("/api/control/resume/{}", session_id));
        let response = self.add_auth(self.inner.put(&url)).send()?;
        if !response.status().is_success() {
            let error: ErrorResponse = response.json().unwrap_or(ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        Ok(())
    }

    pub fn set_session_group(&self, session_id: u32, group_id: u8) -> Result<()> {
        let url = self.build_url(&format!("/api/control/group/{}", session_id));
        let body = serde_json::json!({ "group_id": group_id });
        let response = self.add_auth(self.inner.put(&url).json(&body)).send()?;
        if !response.status().is_success() {
            let error: ErrorResponse = response.json().unwrap_or(ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        Ok(())
    }

    pub fn broadcast_all(&self, session_id: u32) -> Result<()> {
        let url = self.build_url(&format!("/api/control/broadcast-all/{}", session_id));
        let response = self.add_auth(self.inner.put(&url)).send()?;
        if !response.status().is_success() {
            let error: ErrorResponse = response.json().unwrap_or(ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        Ok(())
    }

    pub fn relay_command(&self, command: &str, target: Option<&str>) -> Result<String> {
        let url = self.build_url("/api/command");
        let body = serde_json::json!({
            "command": command,
            "target": target
        });
        let response = self.add_auth(self.inner.post(&url).json(&body)).send()?;
        if !response.status().is_success() {
            let error: ErrorResponse = response.json().unwrap_or(ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        let resp: serde_json::Value = response.json()?;
        Ok(resp["message"].as_str().unwrap_or("").to_string())
    }
}