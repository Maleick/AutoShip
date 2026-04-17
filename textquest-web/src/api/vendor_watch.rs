//! REST API handlers for vendor item watch configuration and alert history.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use textquest_common::ipc::{MerchantListing, MerchantWindowSnapshot};
use tokio::sync::RwLock;
use tokio::time::{self, Duration};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VendorWatchItem {
    pub item_name: String,
    pub max_price_copper: Option<u64>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VendorWatchConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub items: Vec<VendorWatchItem>,
}

impl Default for VendorWatchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VendorWatchAlertEntry {
    pub id: u64,
    pub vendor_name: String,
    pub item_name: String,
    pub expected_max_price_copper: Option<u64>,
    pub actual_price_copper: Option<u64>,
    pub price_delta_copper: Option<i64>,
    pub within_budget: Option<bool>,
    pub quantity: u32,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VendorWatchStats {
    pub total_alerts: u64,
    pub watched_items: u64,
    pub budget_hits: u64,
}

pub struct VendorWatchState {
    pub entries: RwLock<Vec<VendorWatchAlertEntry>>,
    pub enabled: RwLock<bool>,
    pub items: RwLock<Vec<VendorWatchItem>>,
    pub next_id: AtomicU64,
}

const MAX_VENDOR_WATCH_ALERT_HISTORY: usize = 512;

impl VendorWatchState {
    pub fn new_from_disk_or_default() -> Arc<Self> {
        let config = load_vendor_watch_config_from_disk().unwrap_or_default();
        Arc::new(Self {
            entries: RwLock::new(Vec::new()),
            enabled: RwLock::new(config.enabled),
            items: RwLock::new(config.items),
            next_id: AtomicU64::new(1),
        })
    }

    pub fn new_demo() -> Arc<Self> {
        Arc::new(Self {
            entries: RwLock::new(vec![
                VendorWatchAlertEntry {
                    id: 1,
                    vendor_name: "Merchant_Leah".into(),
                    item_name: "Fungi Covered Scale Tunic".into(),
                    expected_max_price_copper: Some(500000),
                    actual_price_copper: Some(475000),
                    price_delta_copper: Some(-25000),
                    within_budget: Some(true),
                    quantity: 1,
                    timestamp: "2026-04-16T19:14:00Z".into(),
                },
                VendorWatchAlertEntry {
                    id: 2,
                    vendor_name: "Barkeep Salla".into(),
                    item_name: "Holgresh Elder Beads".into(),
                    expected_max_price_copper: Some(900000),
                    actual_price_copper: Some(975000),
                    price_delta_copper: Some(75000),
                    within_budget: Some(false),
                    quantity: 1,
                    timestamp: "2026-04-16T20:03:00Z".into(),
                },
            ]),
            enabled: RwLock::new(true),
            items: RwLock::new(vec![
                VendorWatchItem {
                    item_name: "Fungi Covered Scale Tunic".into(),
                    max_price_copper: Some(500000),
                    enabled: true,
                },
                VendorWatchItem {
                    item_name: "Holgresh Elder Beads".into(),
                    max_price_copper: Some(900000),
                    enabled: true,
                },
            ]),
            next_id: AtomicU64::new(3),
        })
    }

    pub async fn config(&self) -> VendorWatchConfig {
        VendorWatchConfig {
            enabled: *self.enabled.read().await,
            items: self.items.read().await.clone(),
        }
    }

    pub async fn replace_config(&self, config: &VendorWatchConfig) {
        *self.enabled.write().await = config.enabled;
        *self.items.write().await = config.items.clone();
    }

    pub async fn get_stats(&self) -> VendorWatchStats {
        let entries = self.entries.read().await;
        let items = self.items.read().await;
        VendorWatchStats {
            total_alerts: entries.len() as u64,
            watched_items: items.iter().filter(|item| item.enabled).count() as u64,
            budget_hits: entries
                .iter()
                .filter(|entry| entry.within_budget == Some(true))
                .count() as u64,
        }
    }

    pub async fn add_entry(&self, entry: VendorWatchAlertEntry) -> VendorWatchAlertEntry {
        let entry = VendorWatchAlertEntry {
            id: self.next_id.fetch_add(1, Ordering::SeqCst),
            ..entry
        };
        let mut entries = self.entries.write().await;
        entries.push(entry.clone());
        let overflow = entries.len().saturating_sub(MAX_VENDOR_WATCH_ALERT_HISTORY);
        if overflow > 0 {
            entries.drain(..overflow);
        }
        entry
    }
}

#[derive(Debug, Deserialize)]
pub struct VendorWatchAlertQuery {
    #[serde(default)]
    pub offset: usize,
    pub limit: Option<usize>,
    pub vendor_name: Option<String>,
    pub item_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VendorWatchAlertPage {
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub entries: Vec<VendorWatchAlertEntry>,
}

const fn default_enabled() -> bool {
    true
}

fn textquest_config_path() -> PathBuf {
    std::env::var("TEXTQUEST_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/textquest.toml"))
}

fn normalize_item_name(item_name: &str) -> String {
    item_name.trim().to_ascii_lowercase()
}

fn normalize_vendor_name(vendor_name: &str) -> String {
    vendor_name.trim().to_ascii_lowercase()
}

fn vendor_listing_key(
    vendor_name: &str,
    item_name: &str,
    price_copper: Option<u64>,
    quantity: u32,
) -> String {
    format!(
        "{}|{}|{}|{}",
        normalize_vendor_name(vendor_name),
        normalize_item_name(item_name),
        price_copper.map_or_else(|| String::from("unknown"), |price| price.to_string()),
        quantity
    )
}

fn clean_vendor_name(raw: Option<&str>) -> String {
    let raw = raw.unwrap_or("Merchant");
    let trimmed = raw.trim();
    if let Some(stripped) = trimmed.strip_prefix('>')
        && let Some(stripped) = stripped.strip_suffix('<')
    {
        let cleaned = stripped.trim();
        if !cleaned.is_empty() {
            return cleaned.to_string();
        }
    }
    if trimmed.is_empty() {
        String::from("Merchant")
    } else {
        trimmed.to_string()
    }
}

fn iso_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn validate_config(config: &VendorWatchConfig) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for item in &config.items {
        if item.item_name.trim().is_empty() {
            return Err("Vendor watch item name must not be empty".into());
        }
        let key = normalize_item_name(&item.item_name);
        if !seen.insert(key) {
            return Err(format!("Duplicate vendor watch item: {}", item.item_name));
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct VendorWatchConfigFile {
    #[serde(default)]
    vendor_watch: VendorWatchConfig,
}

fn load_vendor_watch_config_from_path(path: &FsPath) -> Result<VendorWatchConfig, String> {
    if !path.exists() {
        return Ok(VendorWatchConfig::default());
    }

    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    toml::from_str::<VendorWatchConfigFile>(&content)
        .map(|config| config.vendor_watch)
        .map_err(|error| format!("Failed to decode {}: {error}", path.display()))
}

fn load_vendor_watch_config_from_disk() -> Result<VendorWatchConfig, String> {
    load_vendor_watch_config_from_path(&textquest_config_path())
}

fn save_vendor_watch_config_to_path(
    path: &FsPath,
    config: &VendorWatchConfig,
) -> Result<(), String> {
    validate_config(config)?;

    let mut doc = if path.exists() {
        let content = std::fs::read_to_string(path)
            .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
        content
            .parse::<DocumentMut>()
            .map_err(|error| format!("Failed to parse {}: {error}", path.display()))?
    } else {
        DocumentMut::new()
    };

    let mut vendor_watch = Table::new();
    vendor_watch["enabled"] = value(config.enabled);

    let mut items = ArrayOfTables::new();
    for item in &config.items {
        let mut row = Table::new();
        row["item_name"] = value(item.item_name.clone());
        row["enabled"] = value(item.enabled);
        if let Some(max_price_copper) = item.max_price_copper {
            let max_price_copper = i64::try_from(max_price_copper)
                .map_err(|_| format!("max_price_copper is too large for {}", item.item_name))?;
            row["max_price_copper"] = value(max_price_copper);
        }
        items.push(row);
    }
    vendor_watch["items"] = Item::ArrayOfTables(items);
    doc["vendor_watch"] = Item::Table(vendor_watch);

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

    std::fs::write(&temp_path, doc.to_string())
        .map_err(|error| format!("Failed to write temp file {}: {error}", temp_path.display()))?;
    replace_file_with_overwrite_fallback(
        &temp_path,
        path,
        |from: &FsPath, to: &FsPath| std::fs::rename(from, to),
        |target: &FsPath| std::fs::remove_file(target),
    )?;
    Ok(())
}

fn replace_file_with_overwrite_fallback<Rename, Remove>(
    temp_path: &FsPath,
    path: &FsPath,
    mut rename: Rename,
    mut remove_file: Remove,
) -> Result<(), String>
where
    Rename: FnMut(&FsPath, &FsPath) -> std::io::Result<()>,
    Remove: FnMut(&FsPath) -> std::io::Result<()>,
{
    match rename(temp_path, path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            remove_file(path).map_err(|remove_error| {
                let _ = remove_file(temp_path);
                format!(
                    "Failed to replace config {} with {} after destination already existed: {remove_error}",
                    path.display(),
                    temp_path.display()
                )
            })?;

            rename(temp_path, path).map_err(|rename_error| {
                let _ = remove_file(temp_path);
                format!(
                    "Failed to replace config {} with {} after removing the existing destination: {rename_error}",
                    path.display(),
                    temp_path.display()
                )
            })
        }
        Err(error) => {
            let _ = remove_file(temp_path);
            Err(format!(
                "Failed to replace config {} with {}: {error}",
                path.display(),
                temp_path.display()
            ))
        }
    }
}

fn save_vendor_watch_config_to_disk(config: &VendorWatchConfig) -> Result<(), String> {
    save_vendor_watch_config_to_path(&textquest_config_path(), config)
}

pub async fn list_alerts(
    State(state): State<Arc<AppState>>,
    Query(params): Query<VendorWatchAlertQuery>,
) -> impl IntoResponse {
    let entries = state.vendor_watch_state.entries.read().await;
    let mut filtered: Vec<&VendorWatchAlertEntry> = entries
        .iter()
        .filter(|entry| {
            params.vendor_name.as_deref().is_none_or(|vendor_name| {
                entry
                    .vendor_name
                    .to_ascii_lowercase()
                    .contains(&vendor_name.to_ascii_lowercase())
            })
        })
        .filter(|entry| {
            params.item_name.as_deref().is_none_or(|item_name| {
                entry
                    .item_name
                    .to_ascii_lowercase()
                    .contains(&item_name.to_ascii_lowercase())
            })
        })
        .collect();

    filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let total = filtered.len();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset;
    let page = filtered
        .into_iter()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect();

    (
        StatusCode::OK,
        Json(VendorWatchAlertPage {
            total,
            offset,
            limit,
            entries: page,
        }),
    )
}

pub async fn get_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.vendor_watch_state.config().await)
}

pub async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(config): Json<VendorWatchConfig>,
) -> impl IntoResponse {
    if let Err(error) = validate_config(&config) {
        return super::json_error(StatusCode::BAD_REQUEST, error).into_response();
    }
    if let Err(error) = save_vendor_watch_config_to_disk(&config) {
        return super::json_error(StatusCode::INTERNAL_SERVER_ERROR, error).into_response();
    }
    state.vendor_watch_state.replace_config(&config).await;
    (StatusCode::OK, Json(config)).into_response()
}

pub async fn get_watch_list(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.vendor_watch_state.items.read().await.clone())
}

pub async fn put_watch_item(
    State(state): State<Arc<AppState>>,
    Json(item): Json<VendorWatchItem>,
) -> impl IntoResponse {
    let mut config = state.vendor_watch_state.config().await;
    let key = normalize_item_name(&item.item_name);
    if let Some(existing) = config
        .items
        .iter_mut()
        .find(|existing| normalize_item_name(&existing.item_name) == key)
    {
        *existing = item;
    } else {
        config.items.push(item);
    }

    if let Err(error) = validate_config(&config) {
        return super::json_error(StatusCode::BAD_REQUEST, error).into_response();
    }
    if let Err(error) = save_vendor_watch_config_to_disk(&config) {
        return super::json_error(StatusCode::INTERNAL_SERVER_ERROR, error).into_response();
    }
    state.vendor_watch_state.replace_config(&config).await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn delete_watch_item(
    State(state): State<Arc<AppState>>,
    Path(item_name): Path<String>,
) -> impl IntoResponse {
    let mut config = state.vendor_watch_state.config().await;
    let before = config.items.len();
    let key = normalize_item_name(&item_name);
    config
        .items
        .retain(|item| normalize_item_name(&item.item_name) != key);
    if before == config.items.len() {
        return super::json_error(
            StatusCode::NOT_FOUND,
            format!("Vendor watch item not found: {item_name}"),
        )
        .into_response();
    }

    if let Err(error) = save_vendor_watch_config_to_disk(&config) {
        return super::json_error(StatusCode::INTERNAL_SERVER_ERROR, error).into_response();
    }
    state.vendor_watch_state.replace_config(&config).await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn get_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.vendor_watch_state.get_stats().await)
}

pub async fn clear_alerts(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    state.vendor_watch_state.entries.write().await.clear();
    StatusCode::NO_CONTENT
}

pub fn broadcast_vendor_watch_alert(state: &AppState, alert: &VendorWatchAlertEntry) {
    let event = serde_json::json!({
        "type": "vendor_watch_alert",
        "data": alert,
    });
    let _ = state.event_tx.send(event.to_string());
}

fn build_alert_entry(
    vendor_name: &str,
    watch_item: &VendorWatchItem,
    listing: &MerchantListing,
) -> VendorWatchAlertEntry {
    let price_delta_copper = match (listing.price_copper, watch_item.max_price_copper) {
        (Some(actual), Some(expected)) => Some(actual as i64 - expected as i64),
        _ => None,
    };
    let within_budget = match (listing.price_copper, watch_item.max_price_copper) {
        (Some(actual), Some(expected)) => Some(actual <= expected),
        _ => None,
    };

    VendorWatchAlertEntry {
        id: 0,
        vendor_name: vendor_name.to_string(),
        item_name: listing
            .item_name
            .clone()
            .unwrap_or_else(|| watch_item.item_name.clone()),
        expected_max_price_copper: watch_item.max_price_copper,
        actual_price_copper: listing.price_copper,
        price_delta_copper,
        within_budget,
        quantity: listing.quantity.unwrap_or(0),
        timestamp: iso_timestamp(),
    }
}

fn collect_live_alerts_for_windows(
    config: &VendorWatchConfig,
    previous_visible: &HashSet<String>,
    windows: &[MerchantWindowSnapshot],
) -> (Vec<VendorWatchAlertEntry>, HashSet<String>) {
    let mut alerts = Vec::new();
    let mut next_visible = HashSet::new();
    let mut seen = previous_visible.clone();

    for window in windows {
        let vendor_name = clean_vendor_name(
            window
                .vendor_name
                .as_deref()
                .or(window.window_text.as_deref()),
        );

        for listing in &window.listings {
            let Some(item_name) = listing.item_name.as_deref() else {
                continue;
            };
            let Some(watch_item) = config.items.iter().find(|item| {
                item.enabled
                    && normalize_item_name(&item.item_name) == normalize_item_name(item_name)
            }) else {
                continue;
            };

            let listing_key = vendor_listing_key(
                &vendor_name,
                item_name,
                listing.price_copper,
                listing.quantity.unwrap_or(0),
            );
            next_visible.insert(listing_key.clone());
            if seen.contains(&listing_key) {
                continue;
            }

            alerts.push(build_alert_entry(&vendor_name, watch_item, listing));
            seen.insert(listing_key);
        }
    }

    (alerts, next_visible)
}

pub fn spawn_vendor_watch_loop(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_secs(2));
        let mut active_by_pid: HashMap<u32, HashSet<String>> = HashMap::new();
        let mut last_config_signature = String::new();

        loop {
            ticker.tick().await;

            let config = state.vendor_watch_state.config().await;
            let config_signature = serde_json::to_string(&config).unwrap_or_default();
            if config_signature != last_config_signature {
                active_by_pid.clear();
                last_config_signature = config_signature;
            }

            if !config.enabled || !config.items.iter().any(|item| item.enabled) {
                active_by_pid.clear();
                continue;
            }

            let query_results = match tokio::task::spawn_blocking(
                crate::live_ipc::query_merchant_windows,
            )
            .await
            {
                Ok(results) => results,
                Err(error) => {
                    tracing::warn!(error = %error, "Vendor watch loop failed to join merchant query task");
                    continue;
                }
            };

            let mut next_active = HashMap::new();
            for result in query_results {
                let previous_visible = active_by_pid.get(&result.pid).cloned().unwrap_or_default();
                let (alerts, visible_now) =
                    collect_live_alerts_for_windows(&config, &previous_visible, &result.windows);

                if !visible_now.is_empty() {
                    next_active.insert(result.pid, visible_now);
                }

                for alert in alerts {
                    let stored = state.vendor_watch_state.add_entry(alert).await;
                    broadcast_vendor_watch_alert(state.as_ref(), &stored);
                    tracing::info!(
                        pid = result.pid,
                        vendor = %stored.vendor_name,
                        item = %stored.item_name,
                        price_copper = stored.actual_price_copper.unwrap_or_default(),
                        "Vendor watch alert fired"
                    );
                }
            }

            active_by_pid = next_active;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_vendor_watch_config_round_trips() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("textquest.toml");
        let config = VendorWatchConfig {
            enabled: true,
            items: vec![
                VendorWatchItem {
                    item_name: "Fungi Covered Scale Tunic".into(),
                    max_price_copper: Some(500000),
                    enabled: true,
                },
                VendorWatchItem {
                    item_name: "Holgresh Elder Beads".into(),
                    max_price_copper: None,
                    enabled: false,
                },
            ],
        };

        save_vendor_watch_config_to_path(&path, &config).unwrap();
        let loaded = load_vendor_watch_config_from_path(&path).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn replace_vendor_watch_config_retries_after_destination_exists_error() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("textquest.toml");
        let temp_path = temp_dir.path().join(".textquest.toml.tmp");
        std::fs::write(&path, "old").unwrap();
        std::fs::write(&temp_path, "new").unwrap();

        let rename_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let remove_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let rename_counter = Arc::clone(&rename_calls);
        let remove_counter = Arc::clone(&remove_calls);
        replace_file_with_overwrite_fallback(
            &temp_path,
            &path,
            move |from: &FsPath, to: &FsPath| {
                let attempt = rename_counter.fetch_add(1, Ordering::SeqCst);
                if attempt == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        format!("{} already exists", to.display()),
                    ));
                }
                std::fs::rename(from, to)
            },
            move |target: &FsPath| {
                remove_counter.fetch_add(1, Ordering::SeqCst);
                std::fs::remove_file(target)
            },
        )
        .unwrap();

        assert_eq!(rename_calls.load(Ordering::SeqCst), 2);
        assert_eq!(remove_calls.load(Ordering::SeqCst), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert!(!temp_path.exists());
    }

    #[tokio::test]
    async fn vendor_watch_stats_count_budget_hits() {
        let state = VendorWatchState::new_demo();
        let stats = state.get_stats().await;
        assert_eq!(stats.total_alerts, 2);
        assert_eq!(stats.watched_items, 2);
        assert_eq!(stats.budget_hits, 1);
    }

    #[tokio::test]
    async fn vendor_watch_history_is_capped_to_recent_entries() {
        let state = VendorWatchState::new_demo();
        state.entries.write().await.clear();
        state.next_id.store(1, Ordering::SeqCst);

        for id in 0..(MAX_VENDOR_WATCH_ALERT_HISTORY as u64 + 8) {
            state
                .add_entry(VendorWatchAlertEntry {
                    id,
                    vendor_name: "Merchant".into(),
                    item_name: format!("Item {id}"),
                    expected_max_price_copper: None,
                    actual_price_copper: Some(1),
                    price_delta_copper: None,
                    within_budget: None,
                    quantity: 1,
                    timestamp: "2026-04-17T09:00:00Z".into(),
                })
                .await;
        }

        let entries = state.entries.read().await;
        assert_eq!(entries.len(), MAX_VENDOR_WATCH_ALERT_HISTORY);
        assert_eq!(
            entries.first().map(|entry| entry.item_name.as_str()),
            Some("Item 8")
        );
        assert_eq!(
            entries.last().map(|entry| entry.item_name.as_str()),
            Some("Item 519")
        );
    }

    #[test]
    fn vendor_watch_config_rejects_duplicate_item_names() {
        let config = VendorWatchConfig {
            enabled: true,
            items: vec![
                VendorWatchItem {
                    item_name: "Fungi".into(),
                    max_price_copper: Some(1),
                    enabled: true,
                },
                VendorWatchItem {
                    item_name: " fungi ".into(),
                    max_price_copper: Some(2),
                    enabled: true,
                },
            ],
        };

        let error = validate_config(&config).unwrap_err();
        assert!(error.contains("Duplicate vendor watch item"));
    }

    #[test]
    fn collect_live_alerts_captures_price_delta_and_budget_status() {
        let config = VendorWatchConfig {
            enabled: true,
            items: vec![VendorWatchItem {
                item_name: "Fungi Covered Scale Tunic".into(),
                max_price_copper: Some(500_000),
                enabled: true,
            }],
        };
        let windows = vec![MerchantWindowSnapshot {
            vendor_name: Some("Merchant_Leah".into()),
            window_text: Some("> Merchant_Leah <".into()),
            window_sidl_name: Some("MerchantWnd".into()),
            list_sidl_name: Some("MW_ItemList".into()),
            row_count: 1,
            listings: vec![MerchantListing {
                row_index: 0,
                columns: vec![
                    String::new(),
                    "Fungi Covered Scale Tunic".into(),
                    "--".into(),
                    String::new(),
                    "475".into(),
                ],
                item_name: Some("Fungi Covered Scale Tunic".into()),
                price_text: Some("475".into()),
                price_copper: Some(475_000),
                quantity: None,
                quantity_text: Some("--".into()),
                infinite_quantity: true,
            }],
        }];

        let (alerts, visible) = collect_live_alerts_for_windows(&config, &HashSet::new(), &windows);

        assert_eq!(visible.len(), 1);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].vendor_name, "Merchant_Leah");
        assert_eq!(alerts[0].item_name, "Fungi Covered Scale Tunic");
        assert_eq!(alerts[0].actual_price_copper, Some(475_000));
        assert_eq!(alerts[0].expected_max_price_copper, Some(500_000));
        assert_eq!(alerts[0].price_delta_copper, Some(-25_000));
        assert_eq!(alerts[0].within_budget, Some(true));
        assert_eq!(alerts[0].quantity, 0);
    }

    #[test]
    fn collect_live_alerts_dedupes_existing_listing_keys() {
        let config = VendorWatchConfig {
            enabled: true,
            items: vec![VendorWatchItem {
                item_name: "Holgresh Elder Beads".into(),
                max_price_copper: Some(900_000),
                enabled: true,
            }],
        };
        let windows = vec![MerchantWindowSnapshot {
            vendor_name: Some("Barkeep Salla".into()),
            window_text: Some("Barkeep Salla".into()),
            window_sidl_name: Some("MerchantWnd".into()),
            list_sidl_name: Some("MW_ItemList".into()),
            row_count: 1,
            listings: vec![MerchantListing {
                row_index: 0,
                columns: vec![
                    String::new(),
                    "Holgresh Elder Beads".into(),
                    "1".into(),
                    String::new(),
                    "975".into(),
                ],
                item_name: Some("Holgresh Elder Beads".into()),
                price_text: Some("975".into()),
                price_copper: Some(975_000),
                quantity: Some(1),
                quantity_text: Some("1".into()),
                infinite_quantity: false,
            }],
        }];

        let existing = HashSet::from([vendor_listing_key(
            "Barkeep Salla",
            "Holgresh Elder Beads",
            Some(975_000),
            1,
        )]);

        let (alerts, visible) = collect_live_alerts_for_windows(&config, &existing, &windows);

        assert!(alerts.is_empty());
        assert_eq!(visible, existing);
    }
}
