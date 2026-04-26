use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use textquest::{
    alerts::{AlertKind, AlertRecord, NewAlert},
    config::AlertingConfig,
};

use crate::AppState;

use super::json_error;

const DEFAULT_OPERATOR: &str = "web-dashboard";
const ALERT_HISTORY_LIMIT: u32 = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertsResponse {
    pub alerts: Vec<AlertRecord>,
    pub unread_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfigResponse {
    pub config: AlertingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckResponse {
    pub updated: u64,
    pub unread_count: u64,
}

fn redacted_config_metadata(config: &AlertingConfig) -> String {
    serde_json::json!({
        "enable_discord": config.enable_discord,
        "discord_webhook_configured": !config.discord_webhook_url.trim().is_empty(),
        "enable_email": config.enable_email,
        "smtp_server": &config.smtp_server,
        "smtp_port": config.smtp_port,
        "smtp_username": &config.smtp_username,
        "smtp_password_configured": !config.smtp_password.trim().is_empty(),
        "email_from": &config.email_from,
        "email_recipients": &config.email_recipients,
        "email_subject_prefix": &config.email_subject_prefix,
        "warning_batch_window_secs": config.warning_batch_window_secs,
        "thresholds": &config.thresholds,
    })
    .to_string()
}

pub fn router() -> axum::Router<Arc<AppState>> {
    axum::Router::new()
        .route("/", get(list_alerts))
        .route("/config", get(get_alert_config).put(put_alert_config))
        .route("/ack-all", post(ack_all_alerts))
        .route("/{id}", get(get_alert))
        .route("/{id}/ack", post(ack_alert))
}

pub async fn list_alerts(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match (
        state.alert_store.recent(ALERT_HISTORY_LIMIT),
        state.alert_store.unread_count(),
    ) {
        (Ok(alerts), Ok(unread_count)) => (
            StatusCode::OK,
            Json(AlertsResponse {
                alerts,
                unread_count,
            }),
        )
            .into_response(),
        (Err(error), _) | (_, Err(error)) => {
            tracing::error!(%error, "Failed to read alert history");
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to read alert history",
            )
            .into_response()
        }
    }
}

pub async fn get_alert(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.alert_store.get(id) {
        Ok(Some(alert)) => (StatusCode::OK, Json(alert)).into_response(),
        Ok(None) => {
            json_error(StatusCode::NOT_FOUND, format!("Alert {id} was not found")).into_response()
        }
        Err(error) => {
            tracing::error!(%error, alert_id = id, "Failed to fetch alert");
            json_error(StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch alert").into_response()
        }
    }
}

pub async fn get_alert_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let config = state.alert_config.read().await.clone();
    (StatusCode::OK, Json(AlertConfigResponse { config })).into_response()
}

pub async fn put_alert_config(
    State(state): State<Arc<AppState>>,
    Json(config): Json<AlertingConfig>,
) -> impl IntoResponse {
    // Persist to disk before mutating in-memory state so a failed write
    // doesn't silently diverge the dashboard from config/alerting.toml.
    // Path is held on AppState so tests can isolate with a tempfile.
    let persist_target = config.clone();
    let persist_path = state.alerting_config_path.clone();
    let persist_result = tokio::task::spawn_blocking(move || {
        crate::persist_alerting_config(&persist_path, &persist_target)
    })
    .await;

    match persist_result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            tracing::error!(%error, "Failed to persist alerting config to disk");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to persist alerting config",
            )
            .into_response();
        }
        Err(join_error) => {
            tracing::error!(%join_error, "Alerting-config persist task panicked");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to persist alerting config",
            )
            .into_response();
        }
    }

    {
        let mut current = state.alert_config.write().await;
        *current = config.clone();
    }

    let audit_message = format!(
        "Alert configuration updated via web UI (discord={}, email={}, batch={}s)",
        config.enable_discord, config.enable_email, config.warning_batch_window_secs
    );
    let audit = NewAlert::new(
        textquest::alerts::AlertSeverity::Info,
        AlertKind::ConfigChanged,
        audit_message,
    )
    .with_source("web")
    .with_actor(DEFAULT_OPERATOR)
    .with_metadata_json(redacted_config_metadata(&config));

    if let Err(error) = state.alert_store.insert(&audit) {
        tracing::warn!(%error, "Failed to persist alert config audit entry");
    }

    StatusCode::NO_CONTENT.into_response()
}

pub async fn ack_alert(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.alert_store.get(id) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return json_error(StatusCode::NOT_FOUND, format!("Alert {id} was not found"))
                .into_response();
        }
        Err(error) => {
            tracing::error!(%error, alert_id = id, "Failed to fetch alert before ack");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to acknowledge alert",
            )
            .into_response();
        }
    }

    let updated = match state.alert_store.acknowledge(id, DEFAULT_OPERATOR) {
        Ok(n) => n as u64,
        Err(error) => {
            tracing::error!(%error, alert_id = id, "Failed to acknowledge alert");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to acknowledge alert",
            )
            .into_response();
        }
    };

    if updated == 0 {
        return json_error(
            StatusCode::CONFLICT,
            format!("Alert {id} was already acknowledged"),
        )
        .into_response();
    }

    match state.alert_store.unread_count() {
        Ok(unread_count) => (
            StatusCode::OK,
            Json(AckResponse {
                updated,
                unread_count,
            }),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "Failed to count unread alerts after ack");
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to acknowledge alert",
            )
            .into_response()
        }
    }
}

pub async fn ack_all_alerts(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let updated = match state.alert_store.acknowledge_all(DEFAULT_OPERATOR) {
        Ok(updated) => updated as u64,
        Err(error) => {
            tracing::error!(%error, "Failed to acknowledge all alerts");
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to acknowledge alerts",
            )
            .into_response();
        }
    };

    match state.alert_store.unread_count() {
        Ok(unread_count) => (
            StatusCode::OK,
            Json(AckResponse {
                updated,
                unread_count,
            }),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "Failed to count unread alerts after ack-all");
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to acknowledge alerts",
            )
            .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::Response;
    use http_body_util::BodyExt;
    use serde_json::Value;
    use textquest::alerts::AlertSeverity;

    async fn response_json(response: Response) -> Value {
        let body = response
            .into_body()
            .collect()
            .await
            .expect("body should collect")
            .to_bytes();
        serde_json::from_slice(&body).expect("body should be valid JSON")
    }

    fn test_state() -> Arc<AppState> {
        crate::test_support::demo_app_state()
    }

    #[tokio::test]
    async fn list_alerts_returns_recent_and_unread_count() {
        let state = test_state();
        let unread = NewAlert::new(
            AlertSeverity::Critical,
            AlertKind::Death,
            "Frostreaver died",
        );
        let read = NewAlert::new(AlertSeverity::Warning, AlertKind::Stuck, "Noxus is stuck");
        let read_id = state.alert_store.insert(&read).expect("insert read");
        state.alert_store.insert(&unread).expect("insert unread");
        state
            .alert_store
            .acknowledge(read_id, DEFAULT_OPERATOR)
            .expect("ack existing");

        let response = list_alerts(State(state)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let json = response_json(response).await;
        assert_eq!(json["unread_count"], 1);
        assert_eq!(json["alerts"].as_array().expect("alerts array").len(), 2);
        assert_eq!(json["alerts"][0]["message"], "Frostreaver died");
    }

    #[tokio::test]
    async fn ack_alert_marks_record_read() {
        let state = test_state();
        let alert_id = state
            .alert_store
            .insert(&NewAlert::new(
                AlertSeverity::Warning,
                AlertKind::Stuck,
                "Aelrindel stopped moving",
            ))
            .expect("insert alert");

        let response = ack_alert(State(state.clone()), Path(alert_id))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let json = response_json(response).await;
        assert_eq!(json["updated"], 1);
        assert_eq!(json["unread_count"], 0);

        let saved = state
            .alert_store
            .get(alert_id)
            .expect("fetch alert")
            .expect("alert exists");
        assert_eq!(saved.acknowledged_by.as_deref(), Some(DEFAULT_OPERATOR));
        assert!(saved.acknowledged_at.is_some());
    }

    #[tokio::test]
    async fn put_alert_config_updates_state_and_logs_audit_alert() {
        let state = test_state();
        let mut updated = AlertingConfig {
            enable_discord: true,
            enable_email: true,
            discord_webhook_url: "https://discord.example/webhook".into(),
            smtp_server: "smtp.example.com".into(),
            smtp_port: 2525,
            smtp_username: "operator".into(),
            smtp_password: "secret".into(),
            email_from: "alerts@example.com".into(),
            email_recipients: vec!["ops@example.com".into()],
            email_subject_prefix: "[Alert] ".into(),
            warning_batch_window_secs: 90,
            thresholds: textquest::config::AlertThresholdConfig::default(),
            audio: Default::default(),
        };
        updated.thresholds.memory_warning_mb = 256;

        let response = put_alert_config(State(state.clone()), Json(updated.clone()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(*state.alert_config.read().await, updated);

        let recent = state.alert_store.recent(5).expect("recent alerts");
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].kind, AlertKind::ConfigChanged);
        assert!(recent[0].message.contains("Alert configuration updated"));
        let metadata = recent[0]
            .metadata_json
            .as_deref()
            .expect("config audit metadata");
        assert!(!metadata.contains("https://discord.example/webhook"));
        assert!(!metadata.contains("secret"));
        assert!(metadata.contains("\"discord_webhook_configured\":true"));
        assert!(metadata.contains("\"smtp_password_configured\":true"));
    }

    #[tokio::test]
    async fn put_alert_config_persists_to_configured_path() {
        let state = test_state();
        let persist_path = state.alerting_config_path.clone();

        let config = AlertingConfig {
            enable_discord: true,
            discord_webhook_url: "https://discord.example/webhook".into(),
            warning_batch_window_secs: 45,
            ..AlertingConfig::default()
        };

        let response = put_alert_config(State(state.clone()), Json(config.clone()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let contents =
            std::fs::read_to_string(&persist_path).expect("alerting config persisted to disk");
        let parsed: AlertingConfig =
            toml::from_str(&contents).expect("persisted config round-trips");
        assert!(parsed.enable_discord);
        assert_eq!(parsed.warning_batch_window_secs, 45);
        assert_eq!(
            parsed.discord_webhook_url,
            "https://discord.example/webhook"
        );

        std::fs::remove_file(&persist_path).ok();
    }
}
