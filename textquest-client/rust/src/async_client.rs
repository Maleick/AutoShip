//! Asynchronous client implementation

use crate::{ClientConfig, CommandResponse, Error, HealthResponse, Result, SessionInfo, GroupAssignment};
use reqwest::Client;

pub struct AsyncClient {
    inner: Client,
    config: ClientConfig,
}

impl AsyncClient {
    pub fn new(config: ClientConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to build HTTP client");

        Self { inner: client, config }
    }

    fn build_url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url, path)
    }

    fn add_auth(&self, mut req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref token) = self.config.api_token {
            req.header("X-API-Token", token)
        } else {
            req
        }
    }

    pub async fn health(&self) -> Result<HealthResponse> {
        let url = self.build_url("/api/health");
        let response = self.add_auth(self.inner.get(&url)).send().await?;
        if !response.status().is_success() {
            let error: crate::models::ErrorResponse = response.json().await.unwrap_or(crate::models::ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        let health = response.json().await?;
        Ok(health)
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionInfo>> {
        let url = self.build_url("/api/sessions");
        let response = self.add_auth(self.inner.get(&url)).send().await?;
        if !response.status().is_success() {
            let error: crate::models::ErrorResponse = response.json().await.unwrap_or(crate::models::ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        let sessions = response.json().await?;
        Ok(sessions)
    }

    pub async fn pause_session(&self, session_id: u32) -> Result<()> {
        let url = self.build_url(&format!("/api/control/pause/{}", session_id));
        let response = self.add_auth(self.inner.put(&url)).send().await?;
        if !response.status().is_success() {
            let error: crate::models::ErrorResponse = response.json().await.unwrap_or(crate::models::ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        Ok(())
    }

    pub async fn resume_session(&self, session_id: u32) -> Result<()> {
        let url = self.build_url(&format!("/api/control/resume/{}", session_id));
        let response = self.add_auth(self.inner.put(&url)).send().await?;
        if !response.status().is_success() {
            let error: crate::models::ErrorResponse = response.json().await.unwrap_or(crate::models::ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        Ok(())
    }

    pub async fn set_session_group(&self, session_id: u32, group_id: u8) -> Result<()> {
        let url = self.build_url(&format!("/api/control/group/{}", session_id));
        let assignment = GroupAssignment { group_id };
        let response = self.add_auth(self.inner.put(&url).json(&assignment)).send().await?;
        if !response.status().is_success() {
            let error: crate::models::ErrorResponse = response.json().await.unwrap_or(crate::models::ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        Ok(())
    }

    pub async fn broadcast_all(&self, session_id: u32) -> Result<()> {
        let url = self.build_url(&format!("/api/control/broadcast-all/{}", session_id));
        let response = self.add_auth(self.inner.put(&url)).send().await?;
        if !response.status().is_success() {
            let error: crate::models::ErrorResponse = response.json().await.unwrap_or(crate::models::ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        Ok(())
    }

    pub async fn relay_command(&self, command: &str, target: Option<&str>) -> Result<String> {
        let url = self.build_url("/api/command");
        let body = serde_json::json!({
            "command": command,
            "target": target
        });
        let response = self.add_auth(self.inner.post(&url).json(&body)).send().await?;
        if !response.status().is_success() {
            let error: crate::models::ErrorResponse = response.json().await.unwrap_or(crate::models::ErrorResponse {
                error: "Unknown error".to_string(),
            });
            return Err(Error::Api(error.error));
        }
        let resp: CommandResponse = response.json().await?;
        Ok(resp.message)
    }
}