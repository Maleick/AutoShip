use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

use crate::AppState;

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

fn chat_pattern_rules_path() -> PathBuf {
    std::env::var("TEXTQUEST_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/textquest.toml"))
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
        fire_count: rule.fire_count,
    }
}

fn dto_to_rule(dto: ChatPatternRuleDto) -> textquest_common::chat_pattern_rules::ChatPatternRule {
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
        "send_ipc" => {
            textquest_common::chat_pattern_rules::RuleAction::SendIpcCommand(dto.action.payload)
        }
        "trigger_alert" => {
            textquest_common::chat_pattern_rules::RuleAction::TriggerAlert(dto.action.payload)
        }
        _ => textquest_common::chat_pattern_rules::RuleAction::ExecuteCommand(dto.action.payload),
    };

    textquest_common::chat_pattern_rules::ChatPatternRule {
        id: dto.id,
        name: dto.name,
        pattern: dto.pattern,
        pattern_type,
        channels,
        action,
        priority: dto.priority,
        enabled: dto.enabled,
        cooldown_secs: dto.cooldown_secs,
        fire_count: dto.fire_count,
    }
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
    Json(rules): Json<Vec<ChatPatternRuleDto>>,
) -> impl IntoResponse {
    let mut engine = state.chat_pattern_rules.write().await;
    let imported: Vec<textquest_common::chat_pattern_rules::ChatPatternRule> =
        rules.iter().map(dto_to_rule).collect();

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
        Json(serde_json::json!({ "imported": rules.len() })),
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

        let rule = dto_to_rule(dto.clone());

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
}
