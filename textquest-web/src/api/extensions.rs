//! Schema-driven extension catalog API for RedGuides / OpenVanilla parity.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    Json, Router,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, put},
};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::sync::RwLock;

use crate::AppState;

type SettingsMap = Map<String, Value>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionDomain {
    Combat,
    Navigation,
    Loot,
    Awareness,
    Economy,
    OperatorUtilities,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityTier {
    Native,
    Adapted,
    Legacy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionSourceKind {
    TextquestNative,
    LegacyProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfigProvenanceKind {
    ExtensionCatalog,
    LegacyImport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Character,
    Group,
    Session,
}

impl ScopeKind {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "character" => Ok(Self::Character),
            "group" => Ok(Self::Group),
            "session" => Ok(Self::Session),
            _ => Err(format!(
                "Unsupported scope kind `{raw}`. Expected character, group, or session."
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Boolean,
    Integer,
    String,
    Enum,
    StringArray,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionFieldOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionFieldSchema {
    pub key: String,
    pub label: String,
    pub description: String,
    pub kind: FieldKind,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<ExtensionFieldOption>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i64>,
    pub default_value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigProvenanceInfo {
    pub kind: ConfigProvenanceKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdapterHealth {
    Healthy,
    Degraded,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionRuntimeStatus {
    pub enabled: bool,
    pub adapter_health: AdapterHealth,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<String>,
    pub last_sync_message: String,
}

impl Default for ExtensionRuntimeStatus {
    fn default() -> Self {
        Self {
            enabled: false,
            adapter_health: AdapterHealth::Disabled,
            degraded_reason: None,
            last_sync_at: None,
            last_sync_message: "Runtime disabled".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScopeRef {
    pub kind: ScopeKind,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionScopeOverride {
    pub scope: ScopeRef,
    pub settings: SettingsMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionCatalogEntry {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub domain: ExtensionDomain,
    pub compatibility_tier: CompatibilityTier,
    pub source_kind: ExtensionSourceKind,
    pub config_provenance: ConfigProvenanceInfo,
    pub supported_scopes: Vec<ScopeKind>,
    pub schema: Vec<ExtensionFieldSchema>,
    pub settings: SettingsMap,
    pub overrides: Vec<ExtensionScopeOverride>,
    pub runtime: ExtensionRuntimeStatus,
    #[serde(default)]
    pub unsupported_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_source_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedCatalogStore {
    #[serde(default)]
    entries: HashMap<String, PersistedCatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedCatalogEntry {
    #[serde(default)]
    settings: SettingsMap,
    #[serde(default)]
    overrides: Vec<ExtensionScopeOverride>,
    #[serde(default)]
    runtime: ExtensionRuntimeStatus,
    #[serde(default)]
    unsupported_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    legacy_source_name: Option<String>,
}

#[derive(Debug)]
pub struct ExtensionCatalogState {
    path: PathBuf,
    store: RwLock<PersistedCatalogStore>,
    write_lock: tokio::sync::Mutex<()>,
}

impl ExtensionCatalogState {
    pub fn load(path: PathBuf) -> Arc<Self> {
        let store = load_store(&path).unwrap_or_else(|error| {
            tracing::warn!(%error, path = %path.display(), "Falling back to empty extension catalog store");
            PersistedCatalogStore::default()
        });
        Arc::new(Self {
            path,
            store: RwLock::new(store),
            write_lock: tokio::sync::Mutex::new(()),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdateRequest {
    pub settings: SettingsMap,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeUpdateRequest {
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ExtensionRuntimeEvent {
    #[serde(rename = "type")]
    kind: &'static str,
    entry: ExtensionCatalogEntry,
}

#[derive(Debug)]
struct StaticExtensionDefinition {
    id: &'static str,
    display_name: &'static str,
    description: &'static str,
    domain: ExtensionDomain,
    compatibility_tier: CompatibilityTier,
    source_kind: ExtensionSourceKind,
    config_provenance: ConfigProvenanceInfo,
    schema: Vec<ExtensionFieldSchema>,
    default_settings: SettingsMap,
}

pub fn extension_catalog_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("TEXTQUEST_EXTENSION_CATALOG_PATH")
        && !override_path.trim().is_empty()
    {
        return PathBuf::from(override_path);
    }

    crate::data_dir().join("config/extensions-catalog.json")
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/catalog", get(list_catalog))
        .route("/catalog/{id}", get(get_catalog_entry))
        .route("/catalog/{id}/settings", put(put_catalog_settings))
        .route(
            "/catalog/{id}/scopes/{scope_kind}/{scope_id}",
            put(put_scope_override).delete(delete_scope_override),
        )
        .route("/catalog/{id}/runtime", put(put_runtime_status))
}

pub async fn list_catalog(State(state): State<Arc<AppState>>) -> Response {
    let mut entries = Vec::new();
    for definition in all_extension_definitions() {
        match build_catalog_entry(&state, &definition).await {
            Ok(entry) => entries.push(entry),
            Err(error) => return json_error(StatusCode::INTERNAL_SERVER_ERROR, error),
        }
    }

    entries.sort_by(|left, right| {
        domain_rank(&left.domain)
            .cmp(&domain_rank(&right.domain))
            .then_with(|| left.display_name.cmp(&right.display_name))
    });
    (StatusCode::OK, Json(entries)).into_response()
}

pub async fn get_catalog_entry(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let Some(definition) = extension_definition(&id) else {
        return json_error(StatusCode::NOT_FOUND, format!("Unknown extension `{id}`"));
    };

    match build_catalog_entry(&state, &definition).await {
        Ok(entry) => (StatusCode::OK, Json(entry)).into_response(),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}

pub async fn put_catalog_settings(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(update): Json<SettingsUpdateRequest>,
) -> Response {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let Some(definition) = extension_definition(&id) else {
        return json_error(StatusCode::NOT_FOUND, format!("Unknown extension `{id}`"));
    };

    if let Err(error) = validate_settings(&definition.schema, &update.settings) {
        return json_error(StatusCode::BAD_REQUEST, error);
    }

    match mutate_store(&state, |store| {
        let entry = store.entries.entry(id.clone()).or_default();
        entry.settings = update.settings.clone();
        Ok(())
    })
    .await
    {
        Ok(()) => match build_catalog_entry(&state, &definition).await {
            Ok(entry) => (StatusCode::OK, Json(entry)).into_response(),
            Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error),
        },
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}

pub async fn put_scope_override(
    State(state): State<Arc<AppState>>,
    AxumPath((id, scope_kind, scope_id)): AxumPath<(String, String, String)>,
    headers: HeaderMap,
    Json(update): Json<SettingsUpdateRequest>,
) -> Response {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let Some(definition) = extension_definition(&id) else {
        return json_error(StatusCode::NOT_FOUND, format!("Unknown extension `{id}`"));
    };
    let kind = match ScopeKind::parse(&scope_kind) {
        Ok(kind) => kind,
        Err(error) => return json_error(StatusCode::BAD_REQUEST, error),
    };

    if scope_id.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "Scope id must not be empty");
    }
    if let Err(error) = validate_settings(&definition.schema, &update.settings) {
        return json_error(StatusCode::BAD_REQUEST, error);
    }

    let scope = ScopeRef {
        kind,
        id: scope_id.clone(),
    };
    let override_entry = ExtensionScopeOverride {
        scope: scope.clone(),
        settings: update.settings.clone(),
    };

    match mutate_store(&state, |store| {
        let entry = store.entries.entry(id.clone()).or_default();
        if let Some(existing) = entry
            .overrides
            .iter_mut()
            .find(|candidate| candidate.scope.kind == scope.kind && candidate.scope.id == scope.id)
        {
            *existing = override_entry.clone();
        } else {
            entry.overrides.push(override_entry.clone());
        }
        entry.overrides.sort_by(|left, right| {
            scope_rank(&left.scope.kind)
                .cmp(&scope_rank(&right.scope.kind))
                .then_with(|| left.scope.id.cmp(&right.scope.id))
        });
        Ok(())
    })
    .await
    {
        Ok(()) => (StatusCode::OK, Json(override_entry)).into_response(),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}

pub async fn delete_scope_override(
    State(state): State<Arc<AppState>>,
    AxumPath((id, scope_kind, scope_id)): AxumPath<(String, String, String)>,
    headers: HeaderMap,
) -> Response {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let Some(_definition) = extension_definition(&id) else {
        return json_error(StatusCode::NOT_FOUND, format!("Unknown extension `{id}`"));
    };
    let kind = match ScopeKind::parse(&scope_kind) {
        Ok(kind) => kind,
        Err(error) => return json_error(StatusCode::BAD_REQUEST, error),
    };

    match mutate_store(&state, |store| {
        let Some(entry) = store.entries.get_mut(&id) else {
            return Ok(false);
        };
        let initial_len = entry.overrides.len();
        entry
            .overrides
            .retain(|candidate| !(candidate.scope.kind == kind && candidate.scope.id == scope_id));
        Ok(initial_len != entry.overrides.len())
    })
    .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => json_error(StatusCode::NOT_FOUND, "Scope override not found"),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}

pub async fn put_runtime_status(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    headers: HeaderMap,
    Json(update): Json<RuntimeUpdateRequest>,
) -> Response {
    if !crate::api::loot::is_trusted_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let Some(definition) = extension_definition(&id) else {
        return json_error(StatusCode::NOT_FOUND, format!("Unknown extension `{id}`"));
    };

    match mutate_store(&state, |store| {
        let entry = store.entries.entry(id.clone()).or_default();
        entry.runtime = runtime_for_toggle(&definition, entry, update.enabled);
        Ok(())
    })
    .await
    {
        Ok(()) => match build_catalog_entry(&state, &definition).await {
            Ok(entry) => {
                broadcast_runtime_event(&state, entry.clone());
                (StatusCode::OK, Json(entry.runtime)).into_response()
            }
            Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error),
        },
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error),
    }
}

fn broadcast_runtime_event(state: &AppState, entry: ExtensionCatalogEntry) {
    let event = ExtensionRuntimeEvent {
        kind: "extension.runtime",
        entry,
    };
    match serde_json::to_string(&event) {
        Ok(payload) => {
            let _ = state.event_tx.send(payload);
        }
        Err(error) => tracing::warn!(%error, "Failed to encode extension runtime event"),
    }
}

async fn mutate_store<F, T>(state: &Arc<AppState>, apply: F) -> Result<T, String>
where
    F: FnOnce(&mut PersistedCatalogStore) -> Result<T, String>,
{
    let catalog = state.extension_catalog_state.as_ref();
    let _guard = catalog.write_lock.lock().await;
    let current = catalog.store.read().await.clone();
    let mut next = current.clone();
    let output = apply(&mut next)?;
    persist_store(&catalog.path, &next)?;
    *catalog.store.write().await = next;
    Ok(output)
}

async fn build_catalog_entry(
    state: &Arc<AppState>,
    definition: &StaticExtensionDefinition,
) -> Result<ExtensionCatalogEntry, String> {
    let persisted = {
        let store = state.extension_catalog_state.store.read().await;
        store
            .entries
            .get(definition.id)
            .cloned()
            .unwrap_or_default()
    };
    let (settings, unsupported_setting_fields) = resolved_settings(
        &definition.schema,
        &definition.default_settings,
        &persisted.settings,
    );
    validate_settings(&definition.schema, &settings)?;
    let unsupported_fields =
        combined_unsupported_fields(&persisted.unsupported_fields, &unsupported_setting_fields);

    Ok(ExtensionCatalogEntry {
        id: definition.id.to_string(),
        display_name: definition.display_name.to_string(),
        description: definition.description.to_string(),
        domain: definition.domain.clone(),
        compatibility_tier: definition.compatibility_tier.clone(),
        source_kind: definition.source_kind.clone(),
        config_provenance: definition.config_provenance.clone(),
        supported_scopes: vec![ScopeKind::Character, ScopeKind::Group, ScopeKind::Session],
        schema: definition.schema.clone(),
        settings,
        overrides: persisted.overrides.clone(),
        runtime: normalized_runtime(definition, &persisted.runtime, &unsupported_fields),
        unsupported_fields,
        legacy_source_name: persisted.legacy_source_name,
    })
}

fn merged_settings(
    schema: &[ExtensionFieldSchema],
    default_settings: &SettingsMap,
    persisted_settings: &SettingsMap,
) -> (SettingsMap, Vec<String>) {
    let supported_keys: std::collections::HashSet<&str> =
        schema.iter().map(|field| field.key.as_str()).collect();
    let mut merged = default_settings.clone();
    let mut stale_fields = Vec::new();
    for (key, value) in persisted_settings {
        if supported_keys.contains(key.as_str()) {
            merged.insert(key.clone(), value.clone());
        } else {
            stale_fields.push(key.clone());
        }
    }
    stale_fields.sort();
    (merged, stale_fields)
}

fn resolved_settings(
    schema: &[ExtensionFieldSchema],
    default_settings: &SettingsMap,
    persisted_settings: &SettingsMap,
) -> (SettingsMap, Vec<String>) {
    let (merged, stale_fields) = merged_settings(schema, default_settings, persisted_settings);
    let (sanitized, invalid_fields) = sanitize_settings(schema, &merged);
    (
        sanitized,
        combined_unsupported_fields(&stale_fields, &invalid_fields),
    )
}

fn sanitize_settings(
    schema: &[ExtensionFieldSchema],
    settings: &SettingsMap,
) -> (SettingsMap, Vec<String>) {
    let mut sanitized = settings.clone();
    let mut invalid_fields = Vec::new();

    for field in schema {
        let Some(value) = sanitized.get(field.key.as_str()) else {
            continue;
        };
        if validate_setting_value(field, &field.key, value).is_err() {
            sanitized.insert(field.key.clone(), field.default_value.clone());
            invalid_fields.push(field.key.clone());
        }
    }

    invalid_fields.sort();
    (sanitized, invalid_fields)
}

fn combined_unsupported_fields(
    persisted_unsupported_fields: &[String],
    stale_setting_fields: &[String],
) -> Vec<String> {
    let mut unsupported_fields = persisted_unsupported_fields.to_vec();
    for field in stale_setting_fields {
        if !unsupported_fields.iter().any(|existing| existing == field) {
            unsupported_fields.push(field.clone());
        }
    }
    unsupported_fields.sort();
    unsupported_fields
}

fn normalized_runtime(
    definition: &StaticExtensionDefinition,
    runtime: &ExtensionRuntimeStatus,
    unsupported_fields: &[String],
) -> ExtensionRuntimeStatus {
    if runtime.enabled {
        runtime.clone()
    } else if definition.source_kind == ExtensionSourceKind::LegacyProfile
        || !unsupported_fields.is_empty()
    {
        ExtensionRuntimeStatus {
            enabled: false,
            adapter_health: AdapterHealth::Degraded,
            degraded_reason: Some(
                "Imported legacy settings still require adapter review.".to_string(),
            ),
            last_sync_at: runtime.last_sync_at.clone(),
            last_sync_message: if runtime.last_sync_message.is_empty() {
                "Awaiting runtime enable".to_string()
            } else {
                runtime.last_sync_message.clone()
            },
        }
    } else {
        let mut runtime = runtime.clone();
        runtime.adapter_health = AdapterHealth::Disabled;
        if runtime.last_sync_message.is_empty() {
            runtime.last_sync_message = "Runtime disabled".to_string();
        }
        runtime
    }
}

fn runtime_for_toggle(
    definition: &StaticExtensionDefinition,
    persisted: &PersistedCatalogEntry,
    enabled: bool,
) -> ExtensionRuntimeStatus {
    let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    if !enabled {
        return ExtensionRuntimeStatus {
            enabled: false,
            adapter_health: AdapterHealth::Disabled,
            degraded_reason: None,
            last_sync_at: Some(timestamp),
            last_sync_message: "Runtime disabled from dashboard".to_string(),
        };
    }

    let (_, unsupported_setting_fields) = resolved_settings(
        &definition.schema,
        &definition.default_settings,
        &persisted.settings,
    );
    let unsupported_fields =
        combined_unsupported_fields(&persisted.unsupported_fields, &unsupported_setting_fields);

    if definition.source_kind == ExtensionSourceKind::LegacyProfile
        || !unsupported_fields.is_empty()
    {
        let count = unsupported_fields.len();
        return ExtensionRuntimeStatus {
            enabled: true,
            adapter_health: AdapterHealth::Degraded,
            degraded_reason: Some(
                "Unsupported imported settings require manual adapter review.".to_string(),
            ),
            last_sync_at: Some(timestamp),
            last_sync_message: if count == 0 {
                "Enabled in degraded mode for imported legacy profile".to_string()
            } else {
                format!("Enabled in degraded mode with {count} unsupported fields")
            },
        };
    }

    ExtensionRuntimeStatus {
        enabled: true,
        adapter_health: AdapterHealth::Healthy,
        degraded_reason: None,
        last_sync_at: Some(timestamp),
        last_sync_message: "Runtime toggle applied from dashboard".to_string(),
    }
}

fn validate_settings(
    schema: &[ExtensionFieldSchema],
    settings: &SettingsMap,
) -> Result<(), String> {
    let schema_by_key: HashMap<&str, &ExtensionFieldSchema> = schema
        .iter()
        .map(|field| (field.key.as_str(), field))
        .collect();

    for field in schema {
        if field.required && !settings.contains_key(field.key.as_str()) {
            return Err(format!("Missing required setting `{}`", field.key));
        }
    }

    for (key, value) in settings {
        let Some(field) = schema_by_key.get(key.as_str()) else {
            return Err(format!("Unsupported setting `{key}`"));
        };

        validate_setting_value(field, key, value)?;
    }

    Ok(())
}

fn validate_setting_value(
    field: &ExtensionFieldSchema,
    key: &str,
    value: &Value,
) -> Result<(), String> {
    match field.kind {
        FieldKind::Boolean if !value.is_boolean() => {
            return Err(format!("Setting `{key}` must be a boolean"));
        }
        FieldKind::Integer => {
            let Some(number) = value.as_i64() else {
                return Err(format!("Setting `{key}` must be an integer"));
            };
            if let Some(min) = field.min
                && number < min
            {
                return Err(format!("Setting `{key}` must be >= {min}"));
            }
            if let Some(max) = field.max
                && number > max
            {
                return Err(format!("Setting `{key}` must be <= {max}"));
            }
        }
        FieldKind::String | FieldKind::Enum => {
            let Some(string) = value.as_str() else {
                return Err(format!("Setting `{key}` must be a string"));
            };
            if field.required && string.trim().is_empty() {
                return Err(format!("Setting `{key}` must not be blank"));
            }
            if matches!(field.kind, FieldKind::Enum)
                && !field.options.iter().any(|option| option.value == string)
            {
                return Err(format!(
                    "Setting `{key}` must be one of the supported values"
                ));
            }
        }
        FieldKind::StringArray => {
            let Some(items) = value.as_array() else {
                return Err(format!("Setting `{key}` must be an array"));
            };
            if items.iter().any(|item| item.as_str().is_none()) {
                return Err(format!("Setting `{key}` must contain only strings"));
            }
        }
        _ => {}
    }

    Ok(())
}

use crate::error::json_error;

fn load_store(path: &Path) -> Result<PersistedCatalogStore, String> {
    if !path.exists() {
        return Ok(PersistedCatalogStore::default());
    }

    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    serde_json::from_str::<PersistedCatalogStore>(&raw)
        .map_err(|error| format!("Failed to parse {}: {error}", path.display()))
}

fn persist_store(path: &Path, store: &PersistedCatalogStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {error}", parent.display()))?;
    }

    let encoded = serde_json::to_string_pretty(store)
        .map_err(|error| format!("Failed to serialize extension catalog: {error}"))?;
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

    std::fs::write(&temp_path, encoded)
        .map_err(|error| format!("Failed to write {}: {error}", temp_path.display()))?;
    replace_file(&temp_path, path).map_err(|error| {
        let _ = std::fs::remove_file(&temp_path);
        format!(
            "Failed to replace {} with {}: {error}",
            path.display(),
            temp_path.display()
        )
    })?;
    Ok(())
}

fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    replace_file_with(
        from,
        to,
        |source, destination| std::fs::rename(source, destination),
        |path| std::fs::remove_file(path),
        Path::exists,
    )
}

fn replace_file_with<Rename, Remove, Exists>(
    from: &Path,
    to: &Path,
    mut rename: Rename,
    mut remove_file: Remove,
    exists: Exists,
) -> std::io::Result<()>
where
    Rename: FnMut(&Path, &Path) -> std::io::Result<()>,
    Remove: FnMut(&Path) -> std::io::Result<()>,
    Exists: Fn(&Path) -> bool,
{
    match rename(from, to) {
        Ok(()) => Ok(()),
        Err(error) if should_retry_replace(&error) && exists(to) => {
            let backup_path = replace_backup_path(to);
            if exists(&backup_path) {
                remove_file(&backup_path)?;
            }

            rename(to, &backup_path)?;

            match rename(from, to) {
                Ok(()) => {
                    let _ = remove_file(&backup_path);
                    Ok(())
                }
                Err(retry_error) => {
                    if !exists(to) {
                        let restore_error = rename(&backup_path, to).err();
                        if let Some(restore_error) = restore_error {
                            return Err(std::io::Error::new(
                                restore_error.kind(),
                                format!(
                                    "failed to replace {}: {retry_error}; failed to restore original file: {restore_error}",
                                    to.display()
                                ),
                            ));
                        }
                    }
                    Err(retry_error)
                }
            }
        }
        Err(error) => Err(error),
    }
}

fn replace_backup_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("extension-catalog");
    path.with_file_name(format!(".{file_name}.replace-backup"))
}

fn should_retry_replace(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::PermissionDenied
    )
}

fn extension_definition(id: &str) -> Option<StaticExtensionDefinition> {
    all_extension_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
}

fn all_extension_definitions() -> Vec<StaticExtensionDefinition> {
    vec![
        StaticExtensionDefinition {
            id: "mq2xassist",
            display_name: "MQ2XAssist",
            description: "Cross-group assist targeting with per-scope main-assist overrides.",
            domain: ExtensionDomain::Combat,
            compatibility_tier: CompatibilityTier::Adapted,
            source_kind: ExtensionSourceKind::TextquestNative,
            config_provenance: provenance(
                ConfigProvenanceKind::ExtensionCatalog,
                "Dashboard sidecar",
                Some("config/extensions-catalog.json"),
            ),
            schema: vec![
                field_boolean(
                    "enabled",
                    "Enabled",
                    "Enable xassist parity behavior.",
                    false,
                ),
                field_string(
                    "maName",
                    "Main Assist",
                    "Character name to mirror for assist decisions.",
                    true,
                    "Noxus",
                ),
            ],
            default_settings: settings(json!({
                "enabled": false,
                "maName": "Noxus"
            })),
        },
        StaticExtensionDefinition {
            id: "mq2moveutils",
            display_name: "MQ2MoveUtils",
            description: "Camp-control and stick-style movement thresholds exposed as dashboard settings.",
            domain: ExtensionDomain::Navigation,
            compatibility_tier: CompatibilityTier::Adapted,
            source_kind: ExtensionSourceKind::TextquestNative,
            config_provenance: provenance(
                ConfigProvenanceKind::ExtensionCatalog,
                "Dashboard sidecar",
                Some("config/extensions-catalog.json"),
            ),
            schema: vec![
                field_boolean(
                    "enabled",
                    "Enabled",
                    "Enable move-utils parity controls.",
                    false,
                ),
                field_integer(
                    "campRadius",
                    "Camp Radius",
                    "Maximum camp drift radius in game units.",
                    5,
                    500,
                    60,
                ),
                field_boolean(
                    "returnToCamp",
                    "Return To Camp",
                    "Return to the saved camp anchor when navigation finishes.",
                    true,
                ),
                field_integer(
                    "stuckRetryCount",
                    "Stuck Retry Count",
                    "How many times to retry path unsticks before degrading.",
                    0,
                    10,
                    3,
                ),
            ],
            default_settings: settings(json!({
                "enabled": false,
                "campRadius": 60,
                "returnToCamp": true,
                "stuckRetryCount": 3
            })),
        },
        StaticExtensionDefinition {
            id: "mq2autoaccept",
            display_name: "MQ2AutoAccept",
            description: "Auto-accept request policies for invites, trades, tasks, DZ adds, and teleports.",
            domain: ExtensionDomain::Loot,
            compatibility_tier: CompatibilityTier::Adapted,
            source_kind: ExtensionSourceKind::TextquestNative,
            config_provenance: provenance(
                ConfigProvenanceKind::ExtensionCatalog,
                "Dashboard sidecar",
                Some("config/extensions-catalog.json"),
            ),
            schema: vec![
                field_boolean(
                    "enabled",
                    "Enabled",
                    "Master toggle for auto-accept parity.",
                    false,
                ),
                field_boolean(
                    "acceptGroupInvites",
                    "Group Invites",
                    "Accept group invites.",
                    true,
                ),
                field_boolean(
                    "acceptTrades",
                    "Trades",
                    "Accept trade windows automatically.",
                    true,
                ),
                field_boolean(
                    "acceptTaskAdds",
                    "Task Adds",
                    "Accept task add prompts.",
                    true,
                ),
                field_boolean(
                    "acceptDzAdds",
                    "DZ Adds",
                    "Accept expedition invites.",
                    true,
                ),
                field_boolean(
                    "acceptTranslocates",
                    "Translocates",
                    "Accept wizard and druid teleports.",
                    true,
                ),
                field_boolean(
                    "acceptAnchors",
                    "Anchors",
                    "Accept guild anchor prompts.",
                    true,
                ),
                field_enum(
                    "trustMode",
                    "Trust Mode",
                    "Who is allowed to trigger auto-accept.",
                    "anyone",
                    vec![("anyone", "Anyone"), ("trust_list", "Trust List")],
                ),
                field_string_array(
                    "trustedPlayers",
                    "Trusted Players",
                    "Case-insensitive allowlist used when trust mode is `trust_list`.",
                ),
            ],
            default_settings: settings(json!({
                "enabled": false,
                "acceptGroupInvites": true,
                "acceptTrades": true,
                "acceptTaskAdds": true,
                "acceptDzAdds": true,
                "acceptTranslocates": true,
                "acceptAnchors": true,
                "trustMode": "anyone",
                "trustedPlayers": []
            })),
        },
        StaticExtensionDefinition {
            id: "mq2posse",
            display_name: "MQ2Posse",
            description: "Awareness watchlists and zone transition routing surfaced as dashboard-managed settings.",
            domain: ExtensionDomain::Awareness,
            compatibility_tier: CompatibilityTier::Adapted,
            source_kind: ExtensionSourceKind::TextquestNative,
            config_provenance: provenance(
                ConfigProvenanceKind::ExtensionCatalog,
                "Dashboard sidecar",
                Some("config/extensions-catalog.json"),
            ),
            schema: vec![
                field_boolean(
                    "enabled",
                    "Enabled",
                    "Enable posse-style watch routing.",
                    false,
                ),
                field_string_array(
                    "watchZones",
                    "Watch Zones",
                    "Zones to monitor for entry and exit events.",
                ),
                field_boolean(
                    "alertOnEntry",
                    "Alert On Entry",
                    "Emit alerts when watched names enter a zone.",
                    true,
                ),
                field_boolean(
                    "alertOnExit",
                    "Alert On Exit",
                    "Emit alerts when watched names leave a zone.",
                    true,
                ),
            ],
            default_settings: settings(json!({
                "enabled": false,
                "watchZones": [],
                "alertOnEntry": true,
                "alertOnExit": true
            })),
        },
        StaticExtensionDefinition {
            id: "mq2vendors",
            display_name: "MQ2Vendors",
            description: "Vendor stock and pricing watchlists with dashboard-managed economy settings.",
            domain: ExtensionDomain::Economy,
            compatibility_tier: CompatibilityTier::Adapted,
            source_kind: ExtensionSourceKind::TextquestNative,
            config_provenance: provenance(
                ConfigProvenanceKind::ExtensionCatalog,
                "Dashboard sidecar",
                Some("config/extensions-catalog.json"),
            ),
            schema: vec![
                field_boolean(
                    "enabled",
                    "Enabled",
                    "Enable vendor stock watchlists.",
                    false,
                ),
                field_integer(
                    "scanIntervalSecs",
                    "Scan Interval",
                    "Seconds between vendor scans.",
                    5,
                    600,
                    45,
                ),
                field_string_array(
                    "watchItems",
                    "Watch Items",
                    "Wanted item names to alert on.",
                ),
                field_boolean(
                    "announceInBoxChat",
                    "Announce In Box Chat",
                    "Broadcast vendor hits over MQ2EQBC-style relay.",
                    true,
                ),
            ],
            default_settings: settings(json!({
                "enabled": false,
                "scanIntervalSecs": 45,
                "watchItems": [],
                "announceInBoxChat": true
            })),
        },
        StaticExtensionDefinition {
            id: "mq2eqbc",
            display_name: "MQ2EQBC",
            description: "Operator relay and broadcast command routing parity for multibox orchestration.",
            domain: ExtensionDomain::OperatorUtilities,
            compatibility_tier: CompatibilityTier::Adapted,
            source_kind: ExtensionSourceKind::TextquestNative,
            config_provenance: provenance(
                ConfigProvenanceKind::ExtensionCatalog,
                "Dashboard sidecar",
                Some("config/extensions-catalog.json"),
            ),
            schema: vec![
                field_boolean(
                    "enabled",
                    "Enabled",
                    "Enable the box chat relay integration.",
                    false,
                ),
                field_string(
                    "host",
                    "Host",
                    "Hostname or IP of the relay hub.",
                    true,
                    "127.0.0.1",
                ),
                field_integer(
                    "port",
                    "Port",
                    "TCP port for the relay hub.",
                    1,
                    65535,
                    2112,
                ),
                field_boolean(
                    "autoConnect",
                    "Auto Connect",
                    "Connect automatically when the runtime enables the relay.",
                    false,
                ),
            ],
            default_settings: settings(json!({
                "enabled": false,
                "host": "127.0.0.1",
                "port": 2112,
                "autoConnect": false
            })),
        },
        StaticExtensionDefinition {
            id: "kissassist_profile",
            display_name: "KissAssist Imported Profile",
            description: "Imported legacy INI profile normalized into the extension catalog with unsupported fields called out explicitly.",
            domain: ExtensionDomain::Combat,
            compatibility_tier: CompatibilityTier::Legacy,
            source_kind: ExtensionSourceKind::LegacyProfile,
            config_provenance: provenance(
                ConfigProvenanceKind::LegacyImport,
                "Imported legacy profile",
                None,
            ),
            schema: vec![
                field_boolean(
                    "enabled",
                    "Enabled",
                    "Enable the imported legacy profile.",
                    false,
                ),
                field_string(
                    "assistName",
                    "Assist Name",
                    "Primary assist character carried over from the imported INI.",
                    true,
                    "TankName",
                ),
                field_enum(
                    "mode",
                    "Mode",
                    "Primary legacy execution mode.",
                    "camp",
                    vec![("camp", "Camp"), ("manual", "Manual"), ("puller", "Puller")],
                ),
                field_integer(
                    "healAtPct",
                    "Heal At %",
                    "Legacy healing threshold translated into TextQuest semantics.",
                    1,
                    100,
                    65,
                ),
                field_integer(
                    "medStartPct",
                    "Med Start %",
                    "Legacy med threshold carried into the imported profile.",
                    1,
                    100,
                    35,
                ),
            ],
            default_settings: settings(json!({
                "enabled": false,
                "assistName": "TankName",
                "mode": "camp",
                "healAtPct": 65,
                "medStartPct": 35
            })),
        },
    ]
}

fn provenance(kind: ConfigProvenanceKind, label: &str, path: Option<&str>) -> ConfigProvenanceInfo {
    ConfigProvenanceInfo {
        kind,
        label: label.to_string(),
        path: path.map(str::to_string),
    }
}

fn settings(value: Value) -> SettingsMap {
    match value {
        Value::Object(map) => map,
        _ => SettingsMap::new(),
    }
}

fn field_boolean(
    key: &str,
    label: &str,
    description: &str,
    default_value: bool,
) -> ExtensionFieldSchema {
    ExtensionFieldSchema {
        key: key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        kind: FieldKind::Boolean,
        required: true,
        options: Vec::new(),
        min: None,
        max: None,
        default_value: Value::Bool(default_value),
    }
}

fn field_integer(
    key: &str,
    label: &str,
    description: &str,
    min: i64,
    max: i64,
    default_value: i64,
) -> ExtensionFieldSchema {
    ExtensionFieldSchema {
        key: key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        kind: FieldKind::Integer,
        required: true,
        options: Vec::new(),
        min: Some(min),
        max: Some(max),
        default_value: Value::from(default_value),
    }
}

fn field_string(
    key: &str,
    label: &str,
    description: &str,
    required: bool,
    default_value: &str,
) -> ExtensionFieldSchema {
    ExtensionFieldSchema {
        key: key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        kind: FieldKind::String,
        required,
        options: Vec::new(),
        min: None,
        max: None,
        default_value: Value::String(default_value.to_string()),
    }
}

fn field_enum(
    key: &str,
    label: &str,
    description: &str,
    default_value: &str,
    options: Vec<(&str, &str)>,
) -> ExtensionFieldSchema {
    ExtensionFieldSchema {
        key: key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        kind: FieldKind::Enum,
        required: true,
        options: options
            .into_iter()
            .map(|(value, label)| ExtensionFieldOption {
                value: value.to_string(),
                label: label.to_string(),
            })
            .collect(),
        min: None,
        max: None,
        default_value: Value::String(default_value.to_string()),
    }
}

fn field_string_array(key: &str, label: &str, description: &str) -> ExtensionFieldSchema {
    ExtensionFieldSchema {
        key: key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        kind: FieldKind::StringArray,
        required: true,
        options: Vec::new(),
        min: None,
        max: None,
        default_value: Value::Array(Vec::new()),
    }
}

fn domain_rank(domain: &ExtensionDomain) -> u8 {
    match domain {
        ExtensionDomain::Combat => 0,
        ExtensionDomain::Navigation => 1,
        ExtensionDomain::Loot => 2,
        ExtensionDomain::Awareness => 3,
        ExtensionDomain::Economy => 4,
        ExtensionDomain::OperatorUtilities => 5,
    }
}

fn scope_rank(kind: &ScopeKind) -> u8 {
    match kind {
        ScopeKind::Character => 0,
        ScopeKind::Group => 1,
        ScopeKind::Session => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::HashSet,
        io::ErrorKind,
    };

    #[test]
    fn validate_settings_rejects_unknown_field() {
        let definition = extension_definition("mq2eqbc").expect("definition");
        let result = validate_settings(
            &definition.schema,
            &settings(json!({
                "enabled": true,
                "host": "127.0.0.1",
                "port": 2112,
                "autoConnect": false,
                "mysteryField": true
            })),
        );

        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("mysteryField"),
            "unexpected validation error"
        );
    }

    #[test]
    fn runtime_toggle_marks_legacy_profiles_degraded() {
        let definition = extension_definition("kissassist_profile").expect("definition");
        let persisted = PersistedCatalogEntry {
            unsupported_fields: vec!["BurnIfNamed".to_string()],
            ..PersistedCatalogEntry::default()
        };

        let runtime = runtime_for_toggle(&definition, &persisted, true);
        assert!(runtime.enabled);
        assert_eq!(runtime.adapter_health, AdapterHealth::Degraded);
        assert!(
            runtime
                .degraded_reason
                .as_deref()
                .unwrap_or_default()
                .contains("Unsupported imported settings")
        );
    }

    #[tokio::test]
    async fn build_catalog_entry_merges_default_settings_with_partial_persisted_values() {
        let state = crate::test_support::demo_app_state();
        let definition = extension_definition("mq2eqbc").expect("definition");
        {
            let mut store = state.extension_catalog_state.store.write().await;
            store.entries.insert(
                definition.id.to_string(),
                PersistedCatalogEntry {
                    settings: settings(json!({
                        "enabled": true
                    })),
                    ..PersistedCatalogEntry::default()
                },
            );
        }

        let entry = build_catalog_entry(&state, &definition)
            .await
            .expect("catalog entry");

        assert_eq!(entry.settings.get("enabled"), Some(&Value::Bool(true)));
        assert_eq!(
            entry.settings.get("host"),
            Some(&Value::String("127.0.0.1".to_string()))
        );
        assert_eq!(entry.settings.get("port"), Some(&Value::from(2112)));
        assert_eq!(entry.settings.get("autoConnect"), Some(&Value::Bool(false)));
    }

    #[tokio::test]
    async fn build_catalog_entry_tolerates_stale_persisted_settings_keys() {
        let state = crate::test_support::demo_app_state();
        let definition = extension_definition("mq2eqbc").expect("definition");
        {
            let mut store = state.extension_catalog_state.store.write().await;
            store.entries.insert(
                definition.id.to_string(),
                PersistedCatalogEntry {
                    settings: settings(json!({
                        "enabled": true,
                        "mysteryField": true
                    })),
                    ..PersistedCatalogEntry::default()
                },
            );
        }

        let entry = build_catalog_entry(&state, &definition)
            .await
            .expect("catalog entry");

        assert_eq!(entry.settings.get("enabled"), Some(&Value::Bool(true)));
        assert_eq!(
            entry.settings.get("host"),
            Some(&Value::String("127.0.0.1".to_string()))
        );
        assert_eq!(entry.settings.get("mysteryField"), None);
        assert_eq!(entry.unsupported_fields, vec!["mysteryField".to_string()]);
        assert_eq!(entry.runtime.adapter_health, AdapterHealth::Degraded);
    }

    #[tokio::test]
    async fn build_catalog_entry_tolerates_invalid_persisted_values() {
        let state = crate::test_support::demo_app_state();
        let definition = extension_definition("mq2eqbc").expect("definition");
        {
            let mut store = state.extension_catalog_state.store.write().await;
            store.entries.insert(
                definition.id.to_string(),
                PersistedCatalogEntry {
                    settings: settings(json!({
                        "enabled": true,
                        "host": "",
                        "port": "invalid",
                        "autoConnect": "nope"
                    })),
                    ..PersistedCatalogEntry::default()
                },
            );
        }

        let entry = build_catalog_entry(&state, &definition)
            .await
            .expect("catalog entry");

        assert_eq!(entry.settings.get("enabled"), Some(&Value::Bool(true)));
        assert_eq!(
            entry.settings.get("host"),
            Some(&Value::String("127.0.0.1".to_string()))
        );
        assert_eq!(entry.settings.get("port"), Some(&Value::from(2112)));
        assert_eq!(entry.settings.get("autoConnect"), Some(&Value::Bool(false)));
        assert_eq!(
            entry.unsupported_fields,
            vec![
                "autoConnect".to_string(),
                "host".to_string(),
                "port".to_string()
            ]
        );
        assert_eq!(entry.runtime.adapter_health, AdapterHealth::Degraded);
    }

    #[tokio::test]
    async fn build_catalog_entry_serializes_empty_unsupported_fields() {
        let state = crate::test_support::demo_app_state();
        let definition = extension_definition("mq2eqbc").expect("definition");

        let entry = build_catalog_entry(&state, &definition)
            .await
            .expect("catalog entry");
        let payload = serde_json::to_value(&entry).expect("serialize entry");

        assert_eq!(
            payload.get("unsupportedFields"),
            Some(&Value::Array(Vec::new()))
        );
    }

    #[test]
    fn replace_file_retries_when_destination_already_exists() {
        let from = Path::new("extensions.tmp");
        let to = Path::new("extensions.json");
        let rename_calls = RefCell::new(Vec::new());
        let removed_paths = RefCell::new(Vec::new());

        let result = replace_file_with(
            from,
            to,
            |source, destination| {
                let mut calls = rename_calls.borrow_mut();
                let call = calls.len();
                calls.push((source.to_path_buf(), destination.to_path_buf()));
                if call == 0 {
                    return Err(std::io::Error::from(ErrorKind::AlreadyExists));
                }
                Ok(())
            },
            |path: &Path| {
                removed_paths.borrow_mut().push(path.to_path_buf());
                Ok(())
            },
            |path| path == to,
        );

        assert!(result.is_ok());
        assert_eq!(rename_calls.borrow().len(), 3);
        assert_eq!(
            rename_calls.borrow()[0],
            (from.to_path_buf(), to.to_path_buf())
        );
        assert_eq!(
            rename_calls.borrow()[2],
            (from.to_path_buf(), to.to_path_buf())
        );
        assert_eq!(removed_paths.borrow().len(), 1);
    }

    #[test]
    fn replace_file_preserves_destination_when_retry_fails() {
        let from = Path::new("extensions.tmp");
        let to = Path::new("extensions.json");
        let existing_paths = RefCell::new(HashSet::from([from.to_path_buf(), to.to_path_buf()]));
        let replace_attempts = Cell::new(0);

        let result = replace_file_with(
            from,
            to,
            |source, destination| {
                let mut existing = existing_paths.borrow_mut();
                if source == from && destination == to {
                    let attempt = replace_attempts.get();
                    replace_attempts.set(attempt + 1);
                    if attempt == 0 {
                        return Err(std::io::Error::from(ErrorKind::AlreadyExists));
                    }
                    return Err(std::io::Error::from(ErrorKind::PermissionDenied));
                }

                if !existing.remove(source) {
                    return Err(std::io::Error::from(ErrorKind::NotFound));
                }
                existing.insert(destination.to_path_buf());
                Ok(())
            },
            |path: &Path| {
                existing_paths.borrow_mut().remove(path);
                Ok(())
            },
            |path| existing_paths.borrow().contains(path),
        );

        assert!(result.is_err());
        assert!(
            existing_paths.borrow().contains(to),
            "destination should still exist after a failed retry"
        );
    }
}
