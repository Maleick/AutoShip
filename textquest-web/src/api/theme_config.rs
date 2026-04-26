//! Theme configuration API — persists theme preferences to textquest.toml.

use std::path::PathBuf;

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThemeConfig {
    #[serde(default)]
    pub mode: String, // "light" or "dark"
    #[serde(default)]
    pub contrast: String, // "normal" or "high-contrast"
}

impl ThemeConfig {
    pub fn default() -> Self {
        Self {
            mode: "dark".to_string(),
            contrast: "normal".to_string(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.mode != "light" && self.mode != "dark" {
            return Err("Invalid theme mode: must be 'light' or 'dark'".to_string());
        }
        if self.contrast != "normal" && self.contrast != "high-contrast" {
            return Err("Invalid contrast mode: must be 'normal' or 'high-contrast'".to_string());
        }
        Ok(())
    }
}

fn textquest_config_path() -> PathBuf {
    std::env::current_dir()
        .ok()
        .map(|dir| {
            if dir.join("textquest-web").exists() {
                dir.join("textquest-web/config/textquest.toml")
            } else {
                dir.join("config/textquest.toml")
            }
        })
        .unwrap_or_else(|| PathBuf::from("config/textquest.toml"))
}

fn read_theme_settings_from_disk() -> Result<ThemeConfig, String> {
    let path = textquest_config_path();
    if !path.exists() {
        return Ok(ThemeConfig::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    let doc = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?;

    let Some(item) = doc.get("theme") else {
        return Ok(ThemeConfig::default());
    };
    let settings = toml_edit::de::from_str::<ThemeConfig>(&item.to_string())
        .map_err(|error| format!("Failed to decode [theme]: {error}"))?;
    Ok(settings)
}

fn write_theme_settings_to_disk(settings: &ThemeConfig) -> Result<(), String> {
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
    table["mode"] = toml_edit::value(&settings.mode);
    table["contrast"] = toml_edit::value(&settings.contrast);

    doc["theme"] = toml_edit::Item::Table(table);

    std::fs::write(&path, doc.to_string())
        .map_err(|error| format!("Failed to write {}: {error}", path.display()))?;

    Ok(())
}

pub async fn get_theme_config() -> impl IntoResponse {
    match read_theme_settings_from_disk() {
        Ok(config) => (StatusCode::OK, Json(config)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}

pub async fn put_theme_config(
    State(_state): State<Arc<AppState>>,
    Json(config): Json<ThemeConfig>,
) -> impl IntoResponse {
    // Validate theme configuration
    if let Err(error) = config.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": error})),
        )
            .into_response();
    }

    // Write to disk
    match write_theme_settings_to_disk(&config) {
        Ok(()) => (StatusCode::OK, Json(config)).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": error})),
        )
            .into_response(),
    }
}
