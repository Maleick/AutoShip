//! Chat log settings API handlers.

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use std::{path::PathBuf, sync::Arc};
use textquest_common::chat::{ChatChannel, ChatLogConfig, LogLevel, LogRotation};

pub fn textquest_config_path() -> PathBuf {
    crate::api::textquest_config_path()
}

fn read_chat_log_settings_from_disk() -> Result<ChatLogConfig, String> {
    let path = textquest_config_path();
    if !path.exists() {
        return Ok(ChatLogConfig::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    let doc = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;

    let Some(item) = doc.get("chat_log") else {
        return Ok(ChatLogConfig::default());
    };
    let settings = toml_edit::de::from_str::<ChatLogConfig>(&item.to_string())
        .map_err(|error| format!("Failed to decode [chat_log]: {error}"))?;
    Ok(settings)
}

fn write_chat_log_settings_to_disk(settings: &ChatLogConfig) -> Result<(), String> {
    let path = textquest_config_path();
    let mut doc = if path.exists() {
        let content = std::fs::read_to_string(&path)
            .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
        content
            .parse::<toml_edit::DocumentMut>()
            .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?
    } else {
        toml_edit::DocumentMut::new()
    };

    let mut table = toml_edit::Table::new();
    table["enabled"] = toml_edit::value(settings.enabled);
    match &settings.rotation {
        LogRotation::None => {
            table["rotation"] = toml_edit::value("none");
        }
        LogRotation::Daily => {
            table["rotation"] = toml_edit::value("daily");
        }
        LogRotation::BySize(size) => {
            table["rotation"] = toml_edit::value(format!("by_size:{}", size));
        }
    }
    match &settings.level {
        LogLevel::Info => {
            table["level"] = toml_edit::value("info");
        }
        LogLevel::Debug => {
            table["level"] = toml_edit::value("debug");
        }
    }
    let channels: Vec<&str> = settings
        .channels
        .iter()
        .map(|c| match c {
            ChatChannel::Say => "say",
            ChatChannel::Tell => "tell",
            ChatChannel::TellOut => "tell_out",
            ChatChannel::Group => "group",
            ChatChannel::Guild => "guild",
            ChatChannel::Raid => "raid",
            ChatChannel::Shout => "shout",
            ChatChannel::Ooc => "ooc",
            ChatChannel::Auction => "auction",
        })
        .collect();
    let arr: toml_edit::Array = channels.into_iter().collect();
    table["channels"] = toml_edit::value(arr);
    doc["chat_log"] = toml_edit::Item::Table(table);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("Failed to determine file name for {}", path.display()))?;
    let temp_path = path.with_file_name(format!(
        ".{file_name}.tmp.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("Failed to build temp path for {}: {error}", path.display()))?
            .as_nanos()
    ));

    let mut temp_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|error| {
            format!(
                "Failed to create temp file {}: {error}",
                temp_path.display()
            )
        })?;
    std::io::Write::write_all(&mut temp_file, doc.to_string().as_bytes())
        .map_err(|error| format!("Failed to write temp file {}: {error}", temp_path.display()))?;
    temp_file
        .sync_all()
        .map_err(|error| format!("Failed to sync temp file {}: {error}", temp_path.display()))?;
    drop(temp_file);
    std::fs::rename(&temp_path, &path).map_err(|error| {
        let _ = std::fs::remove_file(&temp_path);
        format!(
            "Failed to replace {} with {}: {error}",
            path.display(),
            temp_path.display()
        )
    })?;
    Ok(())
}

pub async fn get_chat_log_settings() -> impl IntoResponse {
    match read_chat_log_settings_from_disk() {
        Ok(settings) => (StatusCode::OK, Json(settings)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(crate::api::ErrorResponse { error }),
        )
            .into_response(),
    }
}

pub async fn put_chat_log_settings(
    State(state): State<Arc<crate::AppState>>,
    Json(settings): Json<ChatLogConfig>,
) -> impl IntoResponse {
    let _write_guard = state.chat_log_write_lock.lock().await;
    match write_chat_log_settings_to_disk(&settings) {
        Ok(()) => (StatusCode::OK, Json(settings)).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(crate::api::ErrorResponse { error }),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, http::Request, response::IntoResponse, routing::put};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use std::time::Duration;
    use tower::ServiceExt;

    struct ConfigPathGuard {
        previous: Option<PathBuf>,
    }

    impl ConfigPathGuard {
        fn set(path: &std::path::Path) -> Self {
            let mut lock = crate::api::test_config_override()
                .write()
                .expect("test_config_override lock poisoned");
            let previous = lock.take();
            *lock = Some(path.to_path_buf());
            Self { previous }
        }
    }

    impl Drop for ConfigPathGuard {
        fn drop(&mut self) {
            let mut lock = crate::api::test_config_override()
                .write()
                .expect("test_config_override lock poisoned");
            *lock = self.previous.take();
        }
    }

    fn config_path_lock() -> &'static tokio::sync::Mutex<()> {
        static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    async fn put_settings(app: Router, body: Vec<u8>) -> StatusCode {
        let request = Request::builder()
            .method("PUT")
            .uri("/api/chat-log/settings")
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .expect("put request");
        app.oneshot(request).await.expect("put response").status()
    }

    #[tokio::test]
    async fn concurrent_put_chat_log_settings_wait_for_write_lock_and_persist_one_complete_payload()
    {
        let _lock = config_path_lock().lock().await;
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("textquest.toml");
        std::fs::write(&path, "[general]\nname = \"keeper\"\n").expect("write config");
        let _guard = ConfigPathGuard::set(&path);
        let state = std::sync::Arc::new(crate::test_app_state());
        let write_guard = state.chat_log_write_lock.lock().await;
        let app = Router::new()
            .route("/api/chat-log/settings", put(put_chat_log_settings))
            .with_state(state.clone());

        let first = ChatLogConfig {
            enabled: true,
            rotation: LogRotation::Daily,
            level: LogLevel::Debug,
            channels: vec![ChatChannel::Say, ChatChannel::Group],
        };
        let second = ChatLogConfig {
            enabled: false,
            rotation: LogRotation::None,
            level: LogLevel::Info,
            channels: vec![ChatChannel::Tell, ChatChannel::Guild],
        };
        let first_body = serde_json::to_vec(&first).expect("serialize first settings");
        let second_body = serde_json::to_vec(&second).expect("serialize second settings");
        let expected_first = json!({
            "enabled": true,
            "rotation": "daily",
            "level": "debug",
            "channels": ["say", "group"],
        });
        let expected_second = json!({
            "enabled": false,
            "rotation": "none",
            "level": "info",
            "channels": ["tell", "guild"],
        });

        let first_task = tokio::spawn(put_settings(app.clone(), first_body));
        let second_task = tokio::spawn(put_settings(app, second_body));

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            !first_task.is_finished() && !second_task.is_finished(),
            "PUT requests completed while the chat-log write lock was held"
        );

        drop(write_guard);
        assert_eq!(
            first_task.await.expect("first put task"),
            StatusCode::OK,
            "first PUT status"
        );
        assert_eq!(
            second_task.await.expect("second put task"),
            StatusCode::OK,
            "second PUT status"
        );

        let content = std::fs::read_to_string(&path).expect("read final config");
        let doc = toml::from_str::<toml::Value>(&content).expect("parse final config");
        let chat_log = doc
            .get("chat_log")
            .and_then(toml::Value::as_table)
            .expect("chat_log table");
        let channels = chat_log
            .get("channels")
            .and_then(toml::Value::as_array)
            .expect("channels array")
            .iter()
            .map(|value| value.as_str().expect("channel string"))
            .collect::<Vec<_>>();
        let final_value = json!({
            "enabled": chat_log
                .get("enabled")
                .and_then(toml::Value::as_bool)
                .expect("enabled bool"),
            "rotation": chat_log
                .get("rotation")
                .and_then(toml::Value::as_str)
                .expect("rotation string"),
            "level": chat_log
                .get("level")
                .and_then(toml::Value::as_str)
                .expect("level string"),
            "channels": channels,
        });
        assert!(
            final_value == expected_first || final_value == expected_second,
            "final settings should match one complete PUT payload, got {final_value}"
        );
    }

    #[tokio::test]
    async fn get_chat_log_settings_error_response_omits_filesystem_paths() {
        let _lock = config_path_lock().lock().await;
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("textquest.toml");
        std::fs::write(&path, "chat_log =").expect("write malformed config");
        let _guard = ConfigPathGuard::set(&path);

        let response = get_chat_log_settings().await.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = response
            .into_body()
            .collect()
            .await
            .expect("collect response body")
            .to_bytes();
        let json: Value = serde_json::from_slice(&body).expect("json error response");
        let error = json
            .get("error")
            .and_then(Value::as_str)
            .expect("error message field");

        assert!(
            !error.contains('/') && !error.contains('\\'),
            "error leaked filesystem path characters: {error}"
        );
    }
}
