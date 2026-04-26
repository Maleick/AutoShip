use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

use crate::AppState;

// ---------------------------------------------------------------------------
// Serde length guards
// ---------------------------------------------------------------------------

/// Maximum number of rules accepted in a single import payload.
pub const IMPORT_MAX_RULES: usize = 1000;
/// Maximum byte length for string name/pattern fields in an import payload.
pub const IMPORT_MAX_STRING_BYTES: usize = 4096;

/// Deserialise a `Vec<T>` and reject payloads with more than
/// [`IMPORT_MAX_RULES`] elements.
fn deserialize_bounded_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let v: Vec<T> = Vec::deserialize(deserializer)?;
    if v.len() > IMPORT_MAX_RULES {
        return Err(serde::de::Error::custom(format!(
            "array exceeds maximum length of {} items",
            IMPORT_MAX_RULES
        )));
    }
    Ok(v)
}

/// Deserialise a `String` and reject values longer than
/// [`IMPORT_MAX_STRING_BYTES`] bytes.
fn deserialize_bounded_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.len() > IMPORT_MAX_STRING_BYTES {
        return Err(serde::de::Error::custom(format!(
            "string field exceeds maximum length of {} bytes",
            IMPORT_MAX_STRING_BYTES
        )));
    }
    Ok(s)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPatternRuleDto {
    pub id: String,
    pub name: String,
    pub pattern: String,
    #[serde(rename = "patternType")]
    pub pattern_type: String,
    pub channels: Vec<String>,
    pub action: ActionDto,
    pub priority: u32,
    pub enabled: bool,
    #[serde(rename = "cooldownSecs")]
    pub cooldown_secs: u32,
    #[serde(rename = "fireCount")]
    #[serde(default)]
    pub fire_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDto {
    #[serde(rename = "actionType")]
    pub action_type: String,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    pub pattern: String,
    #[serde(rename = "patternType")]
    pub pattern_type: String,
    pub channels: Vec<String>,
    pub action: ActionDto,
    pub priority: u32,
    #[serde(default)]
    pub enabled: bool,
    #[serde(rename = "cooldownSecs")]
    #[serde(default)]
    pub cooldown_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRuleRequest {
    pub name: Option<String>,
    pub pattern: Option<String>,
    #[serde(rename = "patternType")]
    pub pattern_type: Option<String>,
    pub channels: Option<Vec<String>>,
    pub action: Option<ActionDto>,
    pub priority: Option<u32>,
    pub enabled: Option<bool>,
    #[serde(rename = "cooldownSecs")]
    pub cooldown_secs: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleStats {
    #[serde(rename = "totalRules")]
    pub total_rules: usize,
    #[serde(rename = "enabledRules")]
    pub enabled_rules: usize,
    #[serde(rename = "totalFires")]
    pub total_fires: u64,
}

// ---------------------------------------------------------------------------
// Import-specific validated DTOs
// ---------------------------------------------------------------------------

/// A single rule DTO accepted in the import endpoint.  String name and
/// channels array are length-guarded to prevent unbounded allocations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRuleDto {
    pub id: String,
    #[serde(deserialize_with = "deserialize_bounded_string")]
    pub name: String,
    #[serde(deserialize_with = "deserialize_bounded_string")]
    pub pattern: String,
    #[serde(rename = "patternType")]
    pub pattern_type: String,
    #[serde(deserialize_with = "deserialize_bounded_vec")]
    pub channels: Vec<String>,
    pub action: ActionDto,
    pub priority: u32,
    pub enabled: bool,
    #[serde(rename = "cooldownSecs")]
    pub cooldown_secs: u32,
    #[serde(rename = "fireCount")]
    #[serde(default)]
    pub fire_count: u64,
}

/// Top-level wrapper for `POST /api/chat-pattern-rules/import`.
///
/// Accepts at most [`IMPORT_MAX_RULES`] rules per request.
#[derive(Debug, Deserialize)]
pub struct ImportRulesPayload(
    #[serde(deserialize_with = "deserialize_bounded_vec")] pub Vec<ImportRuleDto>,
);

fn chat_pattern_rules_path() -> PathBuf {
    crate::api::textquest_config_path()
        .parent()
        .map(|p| p.join("chat_pattern_rules.toml"))
        .unwrap_or_else(|| PathBuf::from("config/chat_pattern_rules.toml"))
}

fn load_rules_from_disk()
-> Result<Vec<textquest_common::chat_pattern_rules::ChatPatternRule>, String> {
    let path = chat_pattern_rules_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let config = textquest_common::chat_pattern_rules::ChatPatternRulesConfig::load(&path)
        .map_err(|e| format!("Failed to load rules: {}", e))?;
    Ok(config.rules)
}

fn save_rules_to_disk(
    rules: &[textquest_common::chat_pattern_rules::ChatPatternRule],
) -> Result<(), String> {
    let path = chat_pattern_rules_path();
    let config = textquest_common::chat_pattern_rules::ChatPatternRulesConfig {
        rules: rules.to_vec(),
    };
    config
        .save(&path)
        .map_err(|e| format!("Failed to save rules: {}", e))
}

fn rule_to_dto(rule: &textquest_common::chat_pattern_rules::ChatPatternRule) -> ChatPatternRuleDto {
    let pattern_type = match rule.pattern_type {
        textquest_common::chat_pattern_rules::PatternType::Literal => "literal",
        textquest_common::chat_pattern_rules::PatternType::Regex => "regex",
    };

    let channels = rule
        .channels
        .iter()
        .map(|c| match c {
            textquest_common::chat::ChatChannel::Say => "say",
            textquest_common::chat::ChatChannel::Tell => "tell",
            textquest_common::chat::ChatChannel::TellOut => "tell_out",
            textquest_common::chat::ChatChannel::Group => "group",
            textquest_common::chat::ChatChannel::Guild => "guild",
            textquest_common::chat::ChatChannel::Raid => "raid",
            textquest_common::chat::ChatChannel::Shout => "shout",
            textquest_common::chat::ChatChannel::Ooc => "ooc",
            textquest_common::chat::ChatChannel::Auction => "auction",
        })
        .map(String::from)
        .collect();

    let (action_type, payload) = match &rule.action {
        textquest_common::chat_pattern_rules::RuleAction::ExecuteCommand(cmd) => {
            ("execute_command".to_string(), cmd.clone())
        }
        textquest_common::chat_pattern_rules::RuleAction::SendIpcCommand(cmd) => {
            ("send_ipc".to_string(), cmd.clone())
        }
        textquest_common::chat_pattern_rules::RuleAction::TriggerAlert(name) => {
            ("trigger_alert".to_string(), name.clone())
        }
    };

    ChatPatternRuleDto {
        id: rule.id.clone(),
        name: rule.name.clone(),
        pattern: rule.pattern.clone(),
        pattern_type: pattern_type.to_string(),
        channels,
        action: ActionDto {
            action_type,
            payload,
        },
        priority: rule.priority,
        enabled: rule.enabled,
        cooldown_secs: rule.cooldown_secs,
        fire_count: rule.fire_count(),
    }
}

fn dto_to_rule(dto: &ChatPatternRuleDto) -> textquest_common::chat_pattern_rules::ChatPatternRule {
    let pattern_type = match dto.pattern_type.as_str() {
        "regex" => textquest_common::chat_pattern_rules::PatternType::Regex,
        _ => textquest_common::chat_pattern_rules::PatternType::Literal,
    };

    let channels = dto
        .channels
        .iter()
        .filter_map(|c| match c.as_str() {
            "say" => Some(textquest_common::chat::ChatChannel::Say),
            "tell" => Some(textquest_common::chat::ChatChannel::Tell),
            "tell_out" => Some(textquest_common::chat::ChatChannel::TellOut),
            "group" => Some(textquest_common::chat::ChatChannel::Group),
            "guild" => Some(textquest_common::chat::ChatChannel::Guild),
            "raid" => Some(textquest_common::chat::ChatChannel::Raid),
            "shout" => Some(textquest_common::chat::ChatChannel::Shout),
            "ooc" => Some(textquest_common::chat::ChatChannel::Ooc),
            "auction" => Some(textquest_common::chat::ChatChannel::Auction),
            _ => None,
        })
        .collect();

    let action = match dto.action.action_type.as_str() {
        "send_ipc" => textquest_common::chat_pattern_rules::RuleAction::SendIpcCommand(
            dto.action.payload.clone(),
        ),
        "trigger_alert" => textquest_common::chat_pattern_rules::RuleAction::TriggerAlert(
            dto.action.payload.clone(),
        ),
        _ => textquest_common::chat_pattern_rules::RuleAction::ExecuteCommand(
            dto.action.payload.clone(),
        ),
    };

    let mut rule = textquest_common::chat_pattern_rules::ChatPatternRule::new(
        dto.name.clone(),
        dto.pattern.clone(),
        pattern_type,
        action,
    )
    .with_channels(channels)
    .with_priority(dto.priority)
    .with_cooldown(dto.cooldown_secs)
    .with_fire_count(dto.fire_count);
    rule.id = dto.id.clone();
    rule.enabled = dto.enabled;
    rule
}

pub async fn list_rules(State(state): State<Arc<AppState>>) -> Json<Vec<ChatPatternRuleDto>> {
    let engine = state.chat_pattern_rules.read().await;
    let rules = engine.get_rules();
    let dtos: Vec<ChatPatternRuleDto> = rules.iter().map(rule_to_dto).collect();
    Json(dtos)
}

pub async fn get_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let engine = state.chat_pattern_rules.read().await;
    match engine.get_rule(&id) {
        Some(rule) => (StatusCode::OK, Json(rule_to_dto(rule))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Rule not found" })),
        )
            .into_response(),
    }
}

pub async fn create_rule(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateRuleRequest>,
) -> impl IntoResponse {
    let pattern_type = match req.pattern_type.as_str() {
        "regex" => textquest_common::chat_pattern_rules::PatternType::Regex,
        _ => textquest_common::chat_pattern_rules::PatternType::Literal,
    };

    let channels = req
        .channels
        .iter()
        .filter_map(|c| match c.as_str() {
            "say" => Some(textquest_common::chat::ChatChannel::Say),
            "tell" => Some(textquest_common::chat::ChatChannel::Tell),
            "tell_out" => Some(textquest_common::chat::ChatChannel::TellOut),
            "group" => Some(textquest_common::chat::ChatChannel::Group),
            "guild" => Some(textquest_common::chat::ChatChannel::Guild),
            "raid" => Some(textquest_common::chat::ChatChannel::Raid),
            "shout" => Some(textquest_common::chat::ChatChannel::Shout),
            "ooc" => Some(textquest_common::chat::ChatChannel::Ooc),
            "auction" => Some(textquest_common::chat::ChatChannel::Auction),
            _ => None,
        })
        .collect();

    let action = match req.action.action_type.as_str() {
        "send_ipc" => {
            textquest_common::chat_pattern_rules::RuleAction::SendIpcCommand(req.action.payload)
        }
        "trigger_alert" => {
            textquest_common::chat_pattern_rules::RuleAction::TriggerAlert(req.action.payload)
        }
        _ => textquest_common::chat_pattern_rules::RuleAction::ExecuteCommand(req.action.payload),
    };

    let mut rule = textquest_common::chat_pattern_rules::ChatPatternRule::new(
        req.name,
        req.pattern,
        pattern_type,
        action,
    )
    .with_channels(channels)
    .with_priority(req.priority);

    rule.enabled = req.enabled;
    rule.cooldown_secs = req.cooldown_secs;

    {
        let mut engine = state.chat_pattern_rules.write().await;
        engine.add_rule(rule.clone());
        if let Err(e) = save_rules_to_disk(engine.get_rules()) {
            tracing::warn!(%e, "Failed to persist rules after create");
        }
    }

    (StatusCode::CREATED, Json(rule_to_dto(&rule))).into_response()
}

pub async fn update_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateRuleRequest>,
) -> impl IntoResponse {
    let mut engine = state.chat_pattern_rules.write().await;

    let rule = match engine.get_mut_rule(&id) {
        Some(r) => r,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Rule not found" })),
            )
                .into_response();
        }
    };

    if let Some(name) = req.name {
        rule.name = name;
    }
    if let Some(pattern) = req.pattern {
        rule.pattern = pattern;
    }
    if let Some(pt) = req.pattern_type {
        rule.pattern_type = match pt.as_str() {
            "regex" => textquest_common::chat_pattern_rules::PatternType::Regex,
            _ => textquest_common::chat_pattern_rules::PatternType::Literal,
        };
    }
    if let Some(channels) = req.channels {
        rule.channels = channels
            .iter()
            .filter_map(|c| match c.as_str() {
                "say" => Some(textquest_common::chat::ChatChannel::Say),
                "tell" => Some(textquest_common::chat::ChatChannel::Tell),
                "tell_out" => Some(textquest_common::chat::ChatChannel::TellOut),
                "group" => Some(textquest_common::chat::ChatChannel::Group),
                "guild" => Some(textquest_common::chat::ChatChannel::Guild),
                "raid" => Some(textquest_common::chat::ChatChannel::Raid),
                "shout" => Some(textquest_common::chat::ChatChannel::Shout),
                "ooc" => Some(textquest_common::chat::ChatChannel::Ooc),
                "auction" => Some(textquest_common::chat::ChatChannel::Auction),
                _ => None,
            })
            .collect();
    }
    if let Some(action) = req.action {
        rule.action = match action.action_type.as_str() {
            "send_ipc" => {
                textquest_common::chat_pattern_rules::RuleAction::SendIpcCommand(action.payload)
            }
            "trigger_alert" => {
                textquest_common::chat_pattern_rules::RuleAction::TriggerAlert(action.payload)
            }
            _ => textquest_common::chat_pattern_rules::RuleAction::ExecuteCommand(action.payload),
        };
    }
    if let Some(priority) = req.priority {
        rule.priority = priority;
    }
    if let Some(enabled) = req.enabled {
        rule.enabled = enabled;
    }
    if let Some(cooldown) = req.cooldown_secs {
        rule.cooldown_secs = cooldown;
    }

    let updated_rule = rule.clone();

    if let Err(e) = save_rules_to_disk(engine.get_rules()) {
        tracing::warn!(%e, "Failed to persist rules after update");
    }

    (StatusCode::OK, Json(rule_to_dto(&updated_rule))).into_response()
}

pub async fn delete_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut engine = state.chat_pattern_rules.write().await;

    if engine.remove_rule(&id).is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Rule not found" })),
        )
            .into_response();
    }

    if let Err(e) = save_rules_to_disk(engine.get_rules()) {
        tracing::warn!(%e, "Failed to persist rules after delete");
    }

    StatusCode::NO_CONTENT.into_response()
}

pub async fn toggle_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut engine = state.chat_pattern_rules.write().await;

    let rule = match engine.get_mut_rule(&id) {
        Some(r) => r,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Rule not found" })),
            )
                .into_response();
        }
    };

    rule.enabled = !rule.enabled;
    let updated_rule = rule.clone();

    if let Err(e) = save_rules_to_disk(engine.get_rules()) {
        tracing::warn!(%e, "Failed to persist rules after toggle");
    }

    (StatusCode::OK, Json(rule_to_dto(&updated_rule))).into_response()
}

pub async fn get_stats(State(state): State<Arc<AppState>>) -> Json<RuleStats> {
    let engine = state.chat_pattern_rules.read().await;
    let stats = engine.get_stats();
    Json(RuleStats {
        total_rules: stats.total_rules,
        enabled_rules: stats.enabled_rules,
        total_fires: stats.total_fires,
    })
}

pub async fn reset_cooldown(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut engine = state.chat_pattern_rules.write().await;
    engine.reset_cooldown(&id);
    StatusCode::NO_CONTENT.into_response()
}

pub async fn reset_all_cooldowns(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let mut engine = state.chat_pattern_rules.write().await;
    engine.reset_all_cooldowns();
    StatusCode::NO_CONTENT.into_response()
}

pub async fn import_rules(
    State(state): State<Arc<AppState>>,
    Json(ImportRulesPayload(rules)): Json<ImportRulesPayload>,
) -> impl IntoResponse {
    let imported_count = rules.len();
    let mut engine = state.chat_pattern_rules.write().await;
    // Convert ImportRuleDto → ChatPatternRuleDto → ChatPatternRule.
    // ImportRuleDto has the same shape as ChatPatternRuleDto; mapping keeps
    // the conversion pipeline unchanged.
    let imported: Vec<textquest_common::chat_pattern_rules::ChatPatternRule> = rules
        .into_iter()
        .map(|dto| {
            let full_dto = ChatPatternRuleDto {
                id: dto.id,
                name: dto.name,
                pattern: dto.pattern,
                pattern_type: dto.pattern_type,
                channels: dto.channels,
                action: dto.action,
                priority: dto.priority,
                enabled: dto.enabled,
                cooldown_secs: dto.cooldown_secs,
                fire_count: dto.fire_count,
            };
            dto_to_rule(&full_dto)
        })
        .collect();

    for rule in imported {
        engine.add_rule(rule);
    }

    if let Err(e) = save_rules_to_disk(engine.get_rules()) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to import rules: {}", e) })),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "imported": imported_count })),
    )
        .into_response()
}

pub fn load_rules_state() -> RwLock<textquest_common::chat_pattern_rules::ChatPatternRuleEngine> {
    match load_rules_from_disk() {
        Ok(rules) => RwLock::new(
            textquest_common::chat_pattern_rules::ChatPatternRuleEngine::with_rules(rules),
        ),
        Err(e) => {
            tracing::warn!(%e, "Failed to load rules from disk, starting fresh");
            RwLock::new(textquest_common::chat_pattern_rules::ChatPatternRuleEngine::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_to_dto_converts_correctly() {
        let rule = textquest_common::chat_pattern_rules::ChatPatternRule::new(
            "Test Rule",
            "hello",
            textquest_common::chat_pattern_rules::PatternType::Literal,
            textquest_common::chat_pattern_rules::RuleAction::ExecuteCommand("/echo hi".into()),
        )
        .with_channels(vec![
            textquest_common::chat::ChatChannel::Say,
            textquest_common::chat::ChatChannel::Tell,
        ])
        .with_priority(10);

        let dto = rule_to_dto(&rule);

        assert_eq!(dto.name, "Test Rule");
        assert_eq!(dto.pattern, "hello");
        assert_eq!(dto.pattern_type, "literal");
        assert_eq!(dto.channels, vec!["say", "tell"]);
        assert_eq!(dto.priority, 10);
        assert_eq!(dto.action.action_type, "execute_command");
        assert_eq!(dto.action.payload, "/echo hi");
    }

    #[test]
    fn dto_to_rule_converts_correctly() {
        let dto = ChatPatternRuleDto {
            id: "test-id".to_string(),
            name: "Test".to_string(),
            pattern: "test".to_string(),
            pattern_type: "regex".to_string(),
            channels: vec!["group".to_string(), "raid".to_string()],
            action: ActionDto {
                action_type: "trigger_alert".to_string(),
                payload: "Alert!".to_string(),
            },
            priority: 5,
            enabled: true,
            cooldown_secs: 30,
            fire_count: 0,
        };

        let rule = dto_to_rule(&dto);

        assert_eq!(rule.name, "Test");
        assert_eq!(rule.pattern, "test");
        assert_eq!(
            rule.pattern_type,
            textquest_common::chat_pattern_rules::PatternType::Regex
        );
        assert_eq!(rule.channels.len(), 2);
        assert_eq!(
            rule.action,
            textquest_common::chat_pattern_rules::RuleAction::TriggerAlert("Alert!".into())
        );
        assert_eq!(rule.priority, 5);
        assert!(rule.enabled);
        assert_eq!(rule.cooldown_secs, 30);
    }

    // -----------------------------------------------------------------------
    // Length-guard tests — issue #3409
    // -----------------------------------------------------------------------

    fn make_import_rule_dto_json(name: &str, channels: Vec<String>) -> serde_json::Value {
        serde_json::json!({
            "id": "x",
            "name": name,
            "pattern": "test",
            "patternType": "literal",
            "channels": channels,
            "action": { "actionType": "trigger_alert", "payload": "!" },
            "priority": 1,
            "enabled": true,
            "cooldownSecs": 0,
            "fireCount": 0
        })
    }

    /// A valid import payload (1 rule, short name) should deserialise without
    /// error and produce the correct rule count.
    #[test]
    fn import_payload_valid_deserialization() {
        let dto = make_import_rule_dto_json("Valid Rule", vec!["say".to_string()]);
        let payload: Vec<ImportRuleDto> = serde_json::from_value(serde_json::json!([dto])).unwrap();
        assert_eq!(payload.len(), 1);
        assert_eq!(payload[0].name, "Valid Rule");
    }

    /// An import payload with more than IMPORT_MAX_RULES items must be
    /// rejected with a serde error (the axum Json extractor will surface this
    /// as a 422 Unprocessable Entity).
    #[test]
    fn import_payload_oversized_array_rejected() {
        let dto = make_import_rule_dto_json("Rule", vec![]);
        let big_array: Vec<serde_json::Value> = (0..=IMPORT_MAX_RULES).map(|_| dto.clone()).collect();
        let result: Result<ImportRulesPayload, _> = serde_json::from_value(serde_json::Value::Array(big_array));
        assert!(
            result.is_err(),
            "Expected deserialization error for oversized array, but got Ok"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("exceeds maximum length"),
            "Expected 'exceeds maximum length' in error, got: {err}"
        );
    }

    /// An import payload containing a rule whose `name` field exceeds
    /// IMPORT_MAX_STRING_BYTES bytes must be rejected.
    #[test]
    fn import_payload_oversized_name_rejected() {
        let long_name = "x".repeat(IMPORT_MAX_STRING_BYTES + 1);
        let dto = make_import_rule_dto_json(&long_name, vec![]);
        let result: Result<Vec<ImportRuleDto>, _> = serde_json::from_value(serde_json::json!([dto]));
        assert!(
            result.is_err(),
            "Expected deserialization error for oversized name, but got Ok"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("exceeds maximum length"),
            "Expected 'exceeds maximum length' in error, got: {err}"
        );
    }

    /// An import payload containing a rule with an oversized `channels` array
    /// (more than IMPORT_MAX_RULES entries in the channels field) must be
    /// rejected.
    #[test]
    fn import_payload_oversized_channels_array_rejected() {
        let channels: Vec<String> = (0..=IMPORT_MAX_RULES).map(|i| format!("chan{i}")).collect();
        let dto = make_import_rule_dto_json("Rule", channels);
        let result: Result<Vec<ImportRuleDto>, _> = serde_json::from_value(serde_json::json!([dto]));
        assert!(
            result.is_err(),
            "Expected deserialization error for oversized channels, but got Ok"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("exceeds maximum length"),
            "Expected 'exceeds maximum length' in error, got: {err}"
        );
    }
}
