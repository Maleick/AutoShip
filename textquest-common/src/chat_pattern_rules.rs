//! User-defined chat pattern rule engine for TextQuest.
//!
//! This module provides a configurable if-then system for matching incoming
//! game chat messages against user-defined patterns and triggering actions.
//! Inspired by MQ2Events and MQ2React.
//!
//! # Pattern Types
//!
//! - **Literal**: Exact substring match (case-insensitive)
//! - **Regex**: Full regular expression match (case-insensitive by default)
//!
//! # Event Sources
//!
//! Rules can match any combination of chat channels: Say, Tell, Group, Guild,
//! Raid, Shout, OOC, Auction, and System.
//!
//! # Action Types
//!
//! - **ExecuteCommand**: Run an EQ slash command on the matching session
//! - **SendIpcCommand**: Send an IPC command to other sessions
//! - **TriggerAlert**: Trigger a sound/visual alert

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use crate::chat::ChatChannel;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternType {
    #[default]
    Literal,
    Regex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    ExecuteCommand(String),
    SendIpcCommand(String),
    TriggerAlert(String),
}

impl std::fmt::Display for RuleAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleAction::ExecuteCommand(cmd) => write!(f, "Execute: {cmd}"),
            RuleAction::SendIpcCommand(cmd) => write!(f, "IPC: {cmd}"),
            RuleAction::TriggerAlert(name) => write!(f, "Alert: {name}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatPatternRule {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub pattern_type: PatternType,
    pub channels: Vec<ChatChannel>,
    pub action: RuleAction,
    pub priority: u32,
    pub enabled: bool,
    pub cooldown_secs: u32,
    #[serde(default)]
    fire_count: u64,
}

impl ChatPatternRule {
    pub fn new(
        name: impl Into<String>,
        pattern: impl Into<String>,
        pattern_type: PatternType,
        action: RuleAction,
    ) -> Self {
        Self {
            id: uuid_v4(),
            name: name.into(),
            pattern: pattern.into(),
            pattern_type,
            channels: Vec::new(),
            action,
            priority: 100,
            enabled: true,
            cooldown_secs: 0,
            fire_count: 0,
        }
    }

    pub fn with_channels(mut self, channels: Vec<ChatChannel>) -> Self {
        self.channels = channels;
        self
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_cooldown(mut self, secs: u32) -> Self {
        self.cooldown_secs = secs;
        self
    }

    pub fn matches_channel(&self, channel: &ChatChannel) -> bool {
        self.channels.is_empty() || self.channels.contains(channel)
    }
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let nanos = now.as_nanos();
    let random: u64 = (nanos as u64) ^ std::process::id() as u64;
    format!(
        "{:016x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        random >> 96,
        (random >> 80) & 0xFFFF,
        (random >> 64) & 0xFFF,
        0x8000 | ((random >> 48) & 0x3FFF),
        random & 0xFFFFFFFFFFFF
    )
}

#[derive(Debug, Clone, PartialEq)]
pub enum PatternMatchResult<'a> {
    Matched {
        rule: &'a ChatPatternRule,
        action: &'a RuleAction,
    },
    NoMatch,
    CooldownActive,
}

#[derive(Debug, Clone, Default)]
struct RuleState {
    last_fired: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct ChatPatternRuleEngine {
    rules: Vec<ChatPatternRule>,
    rule_states: Vec<RuleState>,
}

impl ChatPatternRuleEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_rules(rules: Vec<ChatPatternRule>) -> Self {
        let rule_states = vec![RuleState::default(); rules.len()];
        Self { rules, rule_states }
    }

    pub fn add_rule(&mut self, rule: ChatPatternRule) {
        self.rules.push(rule);
        self.rule_states.push(RuleState::default());
    }

    pub fn remove_rule(&mut self, id: &str) -> Option<ChatPatternRule> {
        if let Some(pos) = self.rules.iter().position(|r| r.id == id) {
            self.rules.remove(pos);
            self.rule_states.remove(pos);
            self.rules.get(pos).cloned()
        } else {
            None
        }
    }

    pub fn get_rule(&self, id: &str) -> Option<&ChatPatternRule> {
        self.rules.iter().find(|r| r.id == id)
    }

    pub fn get_rules(&self) -> &[ChatPatternRule] {
        &self.rules
    }

    pub fn get_mut_rule(&mut self, id: &str) -> Option<&mut ChatPatternRule> {
        self.rules.iter_mut().find(|r| r.id == id)
    }

    pub fn set_rule_enabled(&mut self, id: &str, enabled: bool) -> bool {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == id) {
            rule.enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn clear_rules(&mut self) {
        self.rules.clear();
        self.rule_states.clear();
    }

    pub fn set_rules(&mut self, rules: Vec<ChatPatternRule>) {
        self.rules = rules;
        self.rule_states = vec![RuleState::default(); self.rules.len()];
    }

    fn matches_pattern(pattern: &str, pattern_type: &PatternType, text: &str) -> bool {
        match pattern_type {
            PatternType::Literal => text.to_lowercase().contains(&pattern.to_lowercase()),
            PatternType::Regex => {
                if let Ok(re) = regex::RegexBuilder::new(&format!("(?i){}", pattern))
                    .case_insensitive(true)
                    .build()
                {
                    re.is_match(text)
                } else {
                    text.to_lowercase().contains(&pattern.to_lowercase())
                }
            }
        }
    }

    fn is_on_cooldown(&self, rule_index: usize, now: Instant) -> bool {
        if rule_index >= self.rule_states.len() {
            return false;
        }
        let rule = &self.rules[rule_index];
        if rule.cooldown_secs == 0 {
            return false;
        }
        if let Some(last_fired) = self.rule_states[rule_index].last_fired {
            let cooldown = Duration::from_secs(rule.cooldown_secs as u64);
            now.duration_since(last_fired) < cooldown
        } else {
            false
        }
    }

    fn mark_fired(&mut self, rule_index: usize, now: Instant) {
        if rule_index < self.rule_states.len() {
            self.rule_states[rule_index].last_fired = Some(now);
        }
        if rule_index < self.rules.len() {
            self.rules[rule_index].fire_count += 1;
        }
    }

    pub fn evaluate(
        &mut self,
        channel: &ChatChannel,
        sender: &str,
        message: &str,
    ) -> Vec<(String, RuleAction)> {
        let now = Instant::now();
        let mut actions = Vec::new();

        let mut rule_indices: Vec<usize> = (0..self.rules.len()).collect();
        rule_indices.sort_by_key(|&i| self.rules[i].priority);

        for idx in rule_indices {
            let rule = &self.rules[idx];

            if !rule.enabled {
                continue;
            }

            if !rule.matches_channel(channel) {
                continue;
            }

            if self.is_on_cooldown(idx, now) {
                continue;
            }

            let full_text = format!("{}: {}", sender, message);
            let matches = Self::matches_pattern(&rule.pattern, &rule.pattern_type, &full_text)
                || Self::matches_pattern(&rule.pattern, &rule.pattern_type, message);

            if matches {
                let rule_id = rule.id.clone();
                let rule_action = rule.action.clone();
                self.mark_fired(idx, now);
                actions.push((rule_id, rule_action));
            }
        }

        actions
    }

    pub fn reset_cooldown(&mut self, rule_id: &str) {
        if let Some(pos) = self.rules.iter().position(|r| r.id == rule_id)
            && pos < self.rule_states.len()
        {
            self.rule_states[pos].last_fired = None;
        }
    }

    pub fn reset_all_cooldowns(&mut self) {
        for state in &mut self.rule_states {
            state.last_fired = None;
        }
    }

    pub fn get_stats(&self) -> RuleEngineStats {
        RuleEngineStats {
            total_rules: self.rules.len(),
            enabled_rules: self.rules.iter().filter(|r| r.enabled).count(),
            total_fires: self.rules.iter().map(|r| r.fire_count).sum(),
        }
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let config = ChatPatternRulesConfig {
            rules: self.rules.clone(),
        };
        config.save(path)
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RuleEngineStats {
    pub total_rules: usize,
    pub enabled_rules: usize,
    pub total_fires: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatPatternRulesConfig {
    pub rules: Vec<ChatPatternRule>,
}

impl ChatPatternRulesConfig {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rule(name: &str, pattern: &str, pattern_type: PatternType) -> ChatPatternRule {
        ChatPatternRule::new(
            name,
            pattern,
            pattern_type,
            RuleAction::ExecuteCommand("/echo test".into()),
        )
    }

    #[test]
    fn literal_match_is_case_insensitive() {
        let rule = make_rule("test", "hello world", PatternType::Literal);
        assert!(ChatPatternRuleEngine::matches_pattern(
            &rule.pattern,
            &rule.pattern_type,
            "Hello World"
        ));
        assert!(ChatPatternRuleEngine::matches_pattern(
            &rule.pattern,
            &rule.pattern_type,
            "HELLO WORLD"
        ));
        assert!(ChatPatternRuleEngine::matches_pattern(
            &rule.pattern,
            &rule.pattern_type,
            "someone said hello world!"
        ));
    }

    #[test]
    fn literal_no_match() {
        let rule = make_rule("test", "hello", PatternType::Literal);
        assert!(!ChatPatternRuleEngine::matches_pattern(
            &rule.pattern,
            &rule.pattern_type,
            "goodbye"
        ));
    }

    #[test]
    fn regex_match() {
        let rule = make_rule("test", r"\d+ gold", PatternType::Regex);
        assert!(ChatPatternRuleEngine::matches_pattern(
            &rule.pattern,
            &rule.pattern_type,
            "You receive 500 gold"
        ));
        assert!(ChatPatternRuleEngine::matches_pattern(
            &rule.pattern,
            &rule.pattern_type,
            "123 gold"
        ));
    }

    #[test]
    fn regex_wildcard() {
        let rule = make_rule("test", r"HP.*regen", PatternType::Regex);
        assert!(ChatPatternRuleEngine::matches_pattern(
            &rule.pattern,
            &rule.pattern_type,
            "Your HP is regenerating"
        ));
        assert!(ChatPatternRuleEngine::matches_pattern(
            &rule.pattern,
            &rule.pattern_type,
            "HP regen active"
        ));
    }

    #[test]
    fn rule_channels_empty_matches_all() {
        let rule = make_rule("test", "hello", PatternType::Literal);
        assert!(rule.matches_channel(&ChatChannel::Say));
        assert!(rule.matches_channel(&ChatChannel::Tell));
        assert!(rule.matches_channel(&ChatChannel::Guild));
    }

    #[test]
    fn rule_channels_filter() {
        let rule = make_rule("test", "hello", PatternType::Literal)
            .with_channels(vec![ChatChannel::Tell, ChatChannel::Group]);
        assert!(rule.matches_channel(&ChatChannel::Tell));
        assert!(rule.matches_channel(&ChatChannel::Group));
        assert!(!rule.matches_channel(&ChatChannel::Say));
        assert!(!rule.matches_channel(&ChatChannel::Guild));
    }

    #[test]
    fn engine_evaluates_rules_in_priority_order() {
        let mut engine = ChatPatternRuleEngine::new();
        engine.add_rule(
            ChatPatternRule::new(
                "low",
                "test",
                PatternType::Literal,
                RuleAction::ExecuteCommand("/cmd1".into()),
            )
            .with_priority(100),
        );
        engine.add_rule(
            ChatPatternRule::new(
                "high",
                "test",
                PatternType::Literal,
                RuleAction::ExecuteCommand("/cmd2".into()),
            )
            .with_priority(1),
        );

        let actions = engine.evaluate(&ChatChannel::Say, "sender", "test message");
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].1, &RuleAction::ExecuteCommand("/cmd2".into()));
        assert_eq!(actions[1].1, &RuleAction::ExecuteCommand("/cmd1".into()));
    }

    #[test]
    fn disabled_rules_not_evaluated() {
        let mut engine = ChatPatternRuleEngine::new();
        let mut rule = ChatPatternRule::new(
            "test",
            "hello",
            PatternType::Literal,
            RuleAction::ExecuteCommand("/cmd".into()),
        );
        rule.enabled = false;
        engine.add_rule(rule);

        let actions = engine.evaluate(&ChatChannel::Say, "sender", "hello world");
        assert!(actions.is_empty());
    }

    #[test]
    fn cooldown_prevents_immediate_refire() {
        let mut engine = ChatPatternRuleEngine::new();
        engine.add_rule(
            ChatPatternRule::new(
                "test",
                "hello",
                PatternType::Literal,
                RuleAction::ExecuteCommand("/cmd".into()),
            )
            .with_cooldown(5),
        );

        let actions1 = engine.evaluate(&ChatChannel::Say, "sender", "hello world");
        assert_eq!(actions1.len(), 1);

        let actions2 = engine.evaluate(&ChatChannel::Say, "sender", "hello again");
        assert!(actions2.is_empty());

        let rule_id = engine.rules[0].id.clone();
        engine.reset_cooldown(&rule_id);
        let actions3 = engine.evaluate(&ChatChannel::Say, "sender", "hello once more");
        assert_eq!(actions3.len(), 1);
    }

    #[test]
    fn fire_count_increments() {
        let mut engine = ChatPatternRuleEngine::new();
        engine.add_rule(ChatPatternRule::new(
            "test",
            "hello",
            PatternType::Literal,
            RuleAction::ExecuteCommand("/cmd".into()),
        ));

        engine.evaluate(&ChatChannel::Say, "sender", "hello world");
        engine.evaluate(&ChatChannel::Say, "sender", "hello again");
        let rule_id = engine.rules[0].id.clone();
        engine.reset_cooldown(&rule_id);
        engine.evaluate(&ChatChannel::Say, "sender", "hello once more");

        assert_eq!(engine.rules[0].fire_count, 3);
    }

    #[test]
    fn config_roundtrip() {
        let mut rules = Vec::new();
        rules.push(ChatPatternRule::new(
            "rule1",
            "hello",
            PatternType::Literal,
            RuleAction::ExecuteCommand("/cmd1".into()),
        ));
        rules.push(ChatPatternRule::new(
            "rule2",
            r"\d+",
            PatternType::Regex,
            RuleAction::TriggerAlert("Test".into()),
        ));

        let config = ChatPatternRulesConfig { rules };
        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: ChatPatternRulesConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(config.rules.len(), deserialized.rules.len());
        assert_eq!(config.rules[0].name, deserialized.rules[0].name);
        assert_eq!(
            config.rules[1].pattern_type,
            deserialized.rules[1].pattern_type
        );
    }

    #[test]
    fn channel_serialization() {
        let rule = ChatPatternRule::new(
            "test",
            "hello",
            PatternType::Literal,
            RuleAction::ExecuteCommand("/cmd".into()),
        )
        .with_channels(vec![ChatChannel::Tell, ChatChannel::Group]);

        let serialized = serde_json::to_string(&rule).unwrap();
        let deserialized: ChatPatternRule = serde_json::from_str(&serialized).unwrap();

        assert_eq!(rule.channels, deserialized.channels);
    }
}
