//! Ollama HTTP client for TextQuest LLM integration.
//!
//! Provides [`OllamaClient`] which sends `/api/generate` POST requests to a
//! locally-running Ollama instance and returns the full generated text.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;

/// Default Ollama base URL.
pub const DEFAULT_BASE_URL: &str = "http://localhost:11434";

/// Default model for generation.
pub const DEFAULT_MODEL: &str = "gemma3:27b";

/// Per-request timeout in seconds.
const REQUEST_TIMEOUT_SECS: u64 = 10;

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

/// Request body sent to Ollama `/api/generate`.
#[derive(Debug, Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    system: &'a str,
    /// When `false`, Ollama returns a single JSON object instead of NDJSON.
    stream: bool,
}

/// Response from Ollama `/api/generate` (non-streaming).
#[derive(Debug, Deserialize)]
struct GenerateResponse {
    response: String,
    done: bool,
    #[serde(default)]
    error: Option<String>,
}

// ---------------------------------------------------------------------------
// Rate limiter trait
// ---------------------------------------------------------------------------

/// Simple rate-limiter interface.
///
/// Callers that need to pace requests (e.g. to avoid overwhelming Ollama under
/// heavy multibox load) can implement this trait and pass it to
/// [`OllamaClient::with_rate_limiter`].
#[async_trait::async_trait]
pub trait LlmRateLimiter: Send + Sync + 'static {
    /// Wait until the next request is allowed.
    async fn acquire(&self);
}

/// A no-op [`LlmRateLimiter`] that never throttles.
pub struct NoRateLimit;

#[async_trait::async_trait]
impl LlmRateLimiter for NoRateLimit {
    async fn acquire(&self) {}
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Async HTTP client for the Ollama local inference server.
///
/// # Example
/// ```no_run
/// use llm_client::OllamaClient;
///
/// #[tokio::main]
/// async fn main() {
///     let client = OllamaClient::new();
///     let reply = client.generate("Tell me a joke.", "You are a comedian.").await.unwrap();
///     println!("{reply}");
/// }
/// ```
pub struct OllamaClient {
    /// Base URL of the Ollama server (no trailing slash).
    pub base_url: String,
    /// Model identifier passed to Ollama.
    pub model: String,
    /// Underlying HTTP client (reused across requests).
    http: reqwest::Client,
    /// Optional rate limiter.
    rate_limiter: Box<dyn LlmRateLimiter>,
}

impl std::fmt::Debug for OllamaClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OllamaClient")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .finish()
    }
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OllamaClient {
    /// Create a client with defaults (`http://localhost:11434`, `gemma3:27b`).
    pub fn new() -> Self {
        Self::with_options(DEFAULT_BASE_URL.to_string(), DEFAULT_MODEL.to_string())
    }

    /// Create a client with custom `base_url` and `model`.
    pub fn with_options(base_url: String, model: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("failed to build reqwest client");

        Self {
            base_url,
            model,
            http,
            rate_limiter: Box::new(NoRateLimit),
        }
    }

    /// Attach a [`LlmRateLimiter`] to throttle outbound requests.
    pub fn with_rate_limiter(mut self, limiter: impl LlmRateLimiter) -> Self {
        self.rate_limiter = Box::new(limiter);
        self
    }

    /// Send `prompt` to Ollama and return the generated text.
    ///
    /// `system` is used as the system prompt (may be empty).
    ///
    /// # Errors
    /// Returns an error if the HTTP request fails, Ollama returns a non-200
    /// status, the response body cannot be parsed, or the 10-second timeout
    /// is exceeded.
    pub async fn generate(&self, prompt: &str, system: &str) -> Result<String> {
        self.rate_limiter.acquire().await;

        let url = format!("{}/api/generate", self.base_url);

        let body = GenerateRequest {
            model: &self.model,
            prompt,
            system,
            stream: false,
        };

        let fut = async {
            let resp = self
                .http
                .post(&url)
                .json(&body)
                .send()
                .await
                .with_context(|| format!("HTTP POST to {url} failed"))?;

            let status = resp.status();
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                bail!("Ollama returned HTTP {status}: {text}");
            }

            let resp_body: GenerateResponse = resp
                .json()
                .await
                .context("failed to deserialise Ollama response")?;

            if let Some(err) = resp_body.error {
                bail!("Ollama error: {err}");
            }

            if !resp_body.done {
                // Non-streaming mode should always return done=true.
                bail!("Ollama response marked done=false in non-streaming mode");
            }

            Ok(resp_body.response)
        };

        timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS), fut)
            .await
            .context("request timed out after 10 seconds")?
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Construction & accessors -------------------------------------------

    #[test]
    fn new_uses_default_base_url() {
        let c = OllamaClient::new();
        assert_eq!(c.base_url, DEFAULT_BASE_URL);
    }

    #[test]
    fn new_uses_default_model() {
        let c = OllamaClient::new();
        assert_eq!(c.model, DEFAULT_MODEL);
    }

    #[test]
    fn with_options_sets_base_url() {
        let c = OllamaClient::with_options("http://remotehost:11434".into(), "llama3".into());
        assert_eq!(c.base_url, "http://remotehost:11434");
    }

    #[test]
    fn with_options_sets_model() {
        let c = OllamaClient::with_options("http://remotehost:11434".into(), "llama3".into());
        assert_eq!(c.model, "llama3");
    }

    #[test]
    fn default_trait_impl_matches_new() {
        let a = OllamaClient::new();
        let b = OllamaClient::default();
        assert_eq!(a.base_url, b.base_url);
        assert_eq!(a.model, b.model);
    }

    #[test]
    fn debug_impl_does_not_panic() {
        let c = OllamaClient::new();
        let s = format!("{c:?}");
        assert!(s.contains("OllamaClient"));
    }

    // --- GenerateRequest serialisation --------------------------------------

    #[test]
    fn generate_request_serialises_stream_false() {
        let req = GenerateRequest {
            model: "gemma3:27b",
            prompt: "hello",
            system: "sys",
            stream: false,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["stream"], false);
    }

    #[test]
    fn generate_request_includes_model() {
        let req = GenerateRequest {
            model: "llama3",
            prompt: "hi",
            system: "",
            stream: false,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["model"], "llama3");
    }

    #[test]
    fn generate_request_includes_prompt_and_system() {
        let req = GenerateRequest {
            model: "m",
            prompt: "my prompt",
            system: "my system",
            stream: false,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["prompt"], "my prompt");
        assert_eq!(json["system"], "my system");
    }

    // --- GenerateResponse deserialisation -----------------------------------

    #[test]
    fn deserialise_successful_response() {
        let raw = r#"{"response":"hello","done":true}"#;
        let r: GenerateResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(r.response, "hello");
        assert!(r.done);
        assert!(r.error.is_none());
    }

    #[test]
    fn deserialise_error_field() {
        let raw = r#"{"response":"","done":false,"error":"model not found"}"#;
        let r: GenerateResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(r.error.unwrap(), "model not found");
    }

    // --- Rate limiter -------------------------------------------------------

    #[tokio::test]
    async fn no_rate_limit_acquire_does_not_block() {
        let limiter = NoRateLimit;
        // Should complete instantly — if it blocks the test times out.
        limiter.acquire().await;
    }
}
