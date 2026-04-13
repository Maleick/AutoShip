//! Integration tests for [`OllamaClient`] using a `wiremock` mock HTTP server.

use llm_client::OllamaClient;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn success_body(text: &str) -> serde_json::Value {
    serde_json::json!({
        "response": text,
        "done": true
    })
}

// ---------------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn generate_returns_response_text() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body("Hello from Ollama!")))
        .mount(&server)
        .await;

    let client = OllamaClient::with_options(server.uri(), "gemma3:27b".into());

    let result = client
        .generate("Say hello.", "You are helpful.")
        .await
        .unwrap();

    assert_eq!(result, "Hello from Ollama!");
}

#[tokio::test]
async fn generate_sends_correct_model_in_body() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .and(body_partial_json(serde_json::json!({ "model": "llama3" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body("ok")))
        .mount(&server)
        .await;

    let client = OllamaClient::with_options(server.uri(), "llama3".into());
    client.generate("prompt", "system").await.unwrap();
}

#[tokio::test]
async fn generate_sends_stream_false() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .and(body_partial_json(serde_json::json!({ "stream": false })))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body("ok")))
        .mount(&server)
        .await;

    let client = OllamaClient::with_options(server.uri(), "gemma3:27b".into());
    client.generate("prompt", "").await.unwrap();
}

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn generate_errors_on_http_500() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
        .mount(&server)
        .await;

    let client = OllamaClient::with_options(server.uri(), "gemma3:27b".into());
    let err = client.generate("prompt", "").await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("500") || msg.contains("internal"),
        "unexpected: {msg}"
    );
}

#[tokio::test]
async fn generate_errors_on_ollama_error_field() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "response": "",
            "done": false,
            "error": "model 'bad-model' not found"
        })))
        .mount(&server)
        .await;

    let client = OllamaClient::with_options(server.uri(), "bad-model".into());
    let err = client.generate("prompt", "").await.unwrap_err();
    assert!(
        err.to_string().contains("model 'bad-model' not found"),
        "unexpected: {err}"
    );
}

#[tokio::test]
async fn generate_errors_on_done_false() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "response": "partial",
            "done": false
        })))
        .mount(&server)
        .await;

    let client = OllamaClient::with_options(server.uri(), "gemma3:27b".into());
    let err = client.generate("prompt", "").await.unwrap_err();
    assert!(err.to_string().contains("done=false"), "unexpected: {err}");
}

// ---------------------------------------------------------------------------
// Custom rate limiter wires through correctly
// ---------------------------------------------------------------------------

#[tokio::test]
async fn generate_works_with_custom_rate_limiter() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingLimiter(Arc<AtomicUsize>);

    #[async_trait::async_trait]
    impl llm_client::LlmRateLimiter for CountingLimiter {
        async fn acquire(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_body("done")))
        .mount(&server)
        .await;

    let counter = Arc::new(AtomicUsize::new(0));
    let limiter = CountingLimiter(Arc::clone(&counter));

    let client =
        OllamaClient::with_options(server.uri(), "gemma3:27b".into()).with_rate_limiter(limiter);

    client.generate("hi", "").await.unwrap();
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "rate limiter should have been called once"
    );
}
