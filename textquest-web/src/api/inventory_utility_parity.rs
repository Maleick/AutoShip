use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{Json, extract::State, http::StatusCode};
use textquest_common::api_types::ErrorResponse;
use textquest_common::inventory_utility::InventoryUtilityConfig;

use crate::AppState;

use crate::error::json_error_pair as json_error;

fn config_path() -> PathBuf {
    crate::api::textquest_config_path()
        .parent()
        .map(|parent| parent.join("inventory-utility-parity.json"))
        .unwrap_or_else(|| PathBuf::from("config/inventory-utility-parity.json"))
}

pub fn load_config_from_path(path: &Path) -> Result<InventoryUtilityConfig, String> {
    if !path.exists() {
        return Ok(InventoryUtilityConfig::default());
    }

    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))
}

pub fn load_config_from_disk() -> Result<InventoryUtilityConfig, String> {
    load_config_from_path(&config_path())
}

pub fn write_config_to_path(path: &Path, config: &InventoryUtilityConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    let encoded = serde_json::to_string_pretty(config)
        .map_err(|error| format!("Failed to serialize inventory utility config: {error}"))?;
    std::fs::write(path, encoded)
        .map_err(|error| format!("Failed to write {}: {error}", path.display()))
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> Json<InventoryUtilityConfig> {
    Json(state.inventory_utility_parity.read().await.clone())
}

pub async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(config): Json<InventoryUtilityConfig>,
) -> Result<Json<InventoryUtilityConfig>, (StatusCode, Json<ErrorResponse>)> {
    let _write_guard = state.inventory_utility_parity_write_lock.lock().await;
    if let Err(error) = write_config_to_path(&state.inventory_utility_parity_path, &config) {
        return Err(json_error(StatusCode::INTERNAL_SERVER_ERROR, error));
    }
    *state.inventory_utility_parity.write().await = config.clone();
    Ok(Json(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_config_returns_default_plugin_mappings() {
        let state = crate::test_support::demo_app_state();

        let Json(config) = get_config(State(state)).await;

        assert!(
            config
                .plugin_mappings
                .iter()
                .any(|mapping| mapping.plugin == "MQ2LinkDB")
        );
    }

    #[tokio::test]
    async fn put_config_persists_changes() {
        let state = crate::test_support::demo_app_state();
        let config = InventoryUtilityConfig {
            reward_routing: vec![],
            ..InventoryUtilityConfig::default()
        };

        let Json(saved) = put_config(State(state.clone()), Json(config.clone()))
            .await
            .expect("save should succeed");

        assert_eq!(saved.reward_routing, config.reward_routing);

        let persisted = load_config_from_path(&state.inventory_utility_parity_path)
            .expect("persisted config should load");
        assert_eq!(persisted.reward_routing, config.reward_routing);
    }
}
