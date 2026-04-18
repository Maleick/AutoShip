//! TextQuest client error types

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API returned error: {0}")]
    Api(String),

    #[error("Deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Request timeout after {0} seconds")]
    Timeout(u64),

    #[error("Authentication required")]
    AuthRequired,
}