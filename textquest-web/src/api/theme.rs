use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

/// Supported theme options.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThemeOption {
    Light,
    Dark,
}

impl Default for ThemeOption {
    fn default() -> Self {
        Self::Dark
    }
}

impl std::fmt::Display for ThemeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Light => write!(f, "light"),
            Self::Dark => write!(f, "dark"),
        }
    }
}

impl std::str::FromStr for ThemeOption {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err(format!("Invalid theme: {}", s)),
        }
    }
}

/// Theme configuration request/response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub theme: ThemeOption,
}

/// Get the current theme preference.
pub async fn get_theme(
    State(_state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let theme = ThemeOption::Dark; // Default for now
    Json(ThemeConfig { theme })
}

/// Set the theme preference.
pub async fn set_theme(
    State(state): State<Arc<AppState>>,
    Json(config): Json<ThemeConfig>,
) -> impl IntoResponse {
    // Acquire write lock for theme config
    let _lock = state.theme_write_lock.lock().await;

    // Validate theme selection
    match config.theme {
        ThemeOption::Light | ThemeOption::Dark => {
            // Store in in-memory state
            let mut current = state.current_theme.write().await;
            *current = config.theme;

            // TODO: Persist to TOML config file
            // This would involve reading the config, updating [theme] section, and writing back

            Json(ThemeConfig {
                theme: config.theme,
            })
            .into_response()
        }
    }
}

/// Toggle between light and dark themes.
pub async fn toggle_theme(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let _lock = state.theme_write_lock.lock().await;

    let mut current = state.current_theme.write().await;
    let new_theme = match *current {
        ThemeOption::Light => ThemeOption::Dark,
        ThemeOption::Dark => ThemeOption::Light,
    };
    *current = new_theme;

    // TODO: Persist to TOML config file

    Json(ThemeConfig { theme: new_theme }).into_response()
}
