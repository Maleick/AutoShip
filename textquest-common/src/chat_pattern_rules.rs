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

    pub fn with_fire_count(mut self, fire_count: u64) -> Self {
        self.fire_count = fire_count;
        self
    }

    pub fn matches_channel(&self, channel: &ChatChannel) -> bool {
        self.channels.is_empty() || self.channels.contains(channel)
    }

    #[must_use]
    pub fn fire_count(&self) -> u64 {
        self.fire_count
    }

    pub fn set_fire_count(&mut self, fire_count: u64) {
        self.fire_count = fire_count;
    }
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let nanos = now.as_nanos();
    let random = nanos ^ u128::from(std::process::id());
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
            let removed = self.rules.remove(pos);
            self.rule_states.remove(pos);
            Some(removed)
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
        assert_eq!(actions[0].1, RuleAction::ExecuteCommand("/cmd2".into()));
        assert_eq!(actions[1].1, RuleAction::ExecuteCommand("/cmd1".into()));
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
        let rules = vec![
            ChatPatternRule::new(
                "rule1",
                "hello",
                PatternType::Literal,
                RuleAction::ExecuteCommand("/cmd1".into()),
            ),
            ChatPatternRule::new(
                "rule2",
                r"\d+",
                PatternType::Regex,
                RuleAction::TriggerAlert("Test".into()),
            ),
        ];

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

    // ── remove_rule ─────────────────────────────────────────────────────────

    #[test]
    fn remove_rule_returns_removed_rule() {
        let mut engine = ChatPatternRuleEngine::new();
        let rule = make_rule("to-remove", "hello", PatternType::Literal);
        let id = rule.id.clone();
        let name = rule.name.clone();
        engine.add_rule(rule);

        let removed = engine.remove_rule(&id);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, name);
        assert!(engine.get_rules().is_empty());
    }

    #[test]
    fn remove_rule_unknown_id_returns_none() {
        let mut engine = ChatPatternRuleEngine::new();
        engine.add_rule(make_rule("keep", "hello", PatternType::Literal));
        assert!(engine.remove_rule("nonexistent-id").is_none());
        assert_eq!(engine.get_rules().len(), 1);
    }

    #[test]
    fn remove_rule_syncs_rule_states() {
        let mut engine = ChatPatternRuleEngine::new();
        engine.add_rule(make_rule("r1", "alpha", PatternType::Literal));
        engine.add_rule(make_rule("r2", "beta", PatternType::Literal));
        let r1_id = engine.get_rules()[0].id.clone();

        engine.remove_rule(&r1_id);
        assert_eq!(engine.rules.len(), 1);
        assert_eq!(engine.rule_states.len(), 1);
        assert_eq!(engine.rules[0].name, "r2");
    }

    #[test]
    fn remove_rule_last_rule_returns_removed() {
        let mut engine = ChatPatternRuleEngine::new();
        let rule = make_rule("only", "match", PatternType::Literal);
        let id = rule.id.clone();
        engine.add_rule(rule);

        let removed = engine.remove_rule(&id);
        assert!(removed.is_some(), "last rule should be returned");
        assert!(engine.get_rules().is_empty());
    }

    // ── get_rule / get_mut_rule ──────────────────────────────────────────────

    #[test]
    fn get_rule_finds_by_id() {
        let mut engine = ChatPatternRuleEngine::new();
        let rule = make_rule("findme", "pattern", PatternType::Literal);
        let id = rule.id.clone();
        engine.add_rule(rule);

        let found = engine.get_rule(&id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "findme");
    }

    #[test]
    fn get_rule_returns_none_for_unknown_id() {
        let engine = ChatPatternRuleEngine::new();
        assert!(engine.get_rule("no-such-id").is_none());
    }

    #[test]
    fn get_mut_rule_allows_mutation() {
        let mut engine = ChatPatternRuleEngine::new();
        let rule = make_rule("mutable", "pattern", PatternType::Literal);
        let id = rule.id.clone();
        engine.add_rule(rule);

        let mut_rule = engine.get_mut_rule(&id).unwrap();
        mut_rule.enabled = false;

        assert!(!engine.get_rule(&id).unwrap().enabled);
    }

    // ── set_rule_enabled ────────────────────────────────────────────────────

    #[test]
    fn set_rule_enabled_disables_and_enables() {
        let mut engine = ChatPatternRuleEngine::new();
        let rule = make_rule("toggle", "hello", PatternType::Literal);
        let id = rule.id.clone();
        engine.add_rule(rule);

        assert!(engine.set_rule_enabled(&id, false));
        assert!(!engine.get_rule(&id).unwrap().enabled);

        assert!(engine.set_rule_enabled(&id, true));
        assert!(engine.get_rule(&id).unwrap().enabled);
    }

    #[test]
    fn set_rule_enabled_returns_false_for_unknown_id() {
        let mut engine = ChatPatternRuleEngine::new();
        assert!(!engine.set_rule_enabled("nonexistent", false));
    }

    // ── clear_rules / set_rules ─────────────────────────────────────────────

    #[test]
    fn clear_rules_removes_all() {
        let mut engine = ChatPatternRuleEngine::new();
        engine.add_rule(make_rule("r1", "a", PatternType::Literal));
        engine.add_rule(make_rule("r2", "b", PatternType::Literal));

        engine.clear_rules();
        assert!(engine.get_rules().is_empty());
        assert!(engine.rule_states.is_empty());
    }

    #[test]
    fn set_rules_replaces_all_rules() {
        let mut engine = ChatPatternRuleEngine::new();
        engine.add_rule(make_rule("old", "old", PatternType::Literal));

        let new_rules = vec![
            make_rule("new1", "alpha", PatternType::Literal),
            make_rule("new2", "beta", PatternType::Literal),
        ];
        engine.set_rules(new_rules);

        let rules = engine.get_rules();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].name, "new1");
        assert_eq!(engine.rule_states.len(), 2);
    }

    // ── get_stats ────────────────────────────────────────────────────────────

    #[test]
    fn get_stats_empty_engine() {
        let engine = ChatPatternRuleEngine::new();
        let stats = engine.get_stats();
        assert_eq!(stats.total_rules, 0);
        assert_eq!(stats.enabled_rules, 0);
        assert_eq!(stats.total_fires, 0);
    }

    #[test]
    fn get_stats_counts_enabled_and_total() {
        let mut engine = ChatPatternRuleEngine::new();
        engine.add_rule(make_rule("r1", "a", PatternType::Literal));
        let rule2 = {
            let mut r = make_rule("r2", "b", PatternType::Literal);
            r.enabled = false;
            r
        };
        engine.add_rule(rule2);

        let stats = engine.get_stats();
        assert_eq!(stats.total_rules, 2);
        assert_eq!(stats.enabled_rules, 1);
        assert_eq!(stats.total_fires, 0);
    }

    #[test]
    fn get_stats_total_fires_accumulates() {
        let mut engine = ChatPatternRuleEngine::new();
        engine.add_rule(make_rule("r1", "ping", PatternType::Literal));
        engine.add_rule(make_rule("r2", "pong", PatternType::Literal));

        engine.evaluate(&ChatChannel::Say, "s", "ping!");
        engine.evaluate(&ChatChannel::Say, "s", "pong!");

        let stats = engine.get_stats();
        assert_eq!(stats.total_fires, 2);
    }

    // ── reset_all_cooldowns ──────────────────────────────────────────────────

    #[test]
    fn reset_all_cooldowns_re_enables_all_rules() {
        let mut engine = ChatPatternRuleEngine::new();
        engine.add_rule(make_rule("r1", "hello", PatternType::Literal).with_cooldown(60));
        engine.add_rule(make_rule("r2", "world", PatternType::Literal).with_cooldown(60));

        // Fire both rules to put them in cooldown
        engine.evaluate(&ChatChannel::Say, "s", "hello");
        engine.evaluate(&ChatChannel::Say, "s", "world");

        // Both should be blocked
        assert!(engine.evaluate(&ChatChannel::Say, "s", "hello").is_empty());
        assert!(engine.evaluate(&ChatChannel::Say, "s", "world").is_empty());

        engine.reset_all_cooldowns();

        // Both should fire again after reset
        assert_eq!(engine.evaluate(&ChatChannel::Say, "s", "hello").len(), 1);
        // r1 is in cooldown again; r2 was not triggered this round so it fires
        engine.reset_all_cooldowns();
        assert_eq!(engine.evaluate(&ChatChannel::Say, "s", "world").len(), 1);
    }

    // ── with_rules constructor ───────────────────────────────────────────────

    #[test]
    fn with_rules_initialises_states() {
        let rules = vec![
            make_rule("r1", "a", PatternType::Literal),
            make_rule("r2", "b", PatternType::Literal),
        ];
        let engine = ChatPatternRuleEngine::with_rules(rules);
        assert_eq!(engine.rules.len(), 2);
        assert_eq!(engine.rule_states.len(), 2);
    }

    // ── builder patterns on ChatPatternRule ──────────────────────────────────

    #[test]
    fn with_priority_sets_value() {
        let rule = make_rule("p", "x", PatternType::Literal).with_priority(42);
        assert_eq!(rule.priority, 42);
    }

    #[test]
    fn with_cooldown_sets_value() {
        let rule = make_rule("c", "x", PatternType::Literal).with_cooldown(30);
        assert_eq!(rule.cooldown_secs, 30);
    }

    // ── RuleAction Display ───────────────────────────────────────────────────

    #[test]
    fn rule_action_display_execute_command() {
        let action = RuleAction::ExecuteCommand("/assist".into());
        assert!(action.to_string().contains("/assist"));
    }

    #[test]
    fn rule_action_display_send_ipc_command() {
        let action = RuleAction::SendIpcCommand("/follow".into());
        assert!(action.to_string().contains("/follow"));
    }

    #[test]
    fn rule_action_display_trigger_alert() {
        let action = RuleAction::TriggerAlert("low_health".into());
        assert!(action.to_string().contains("low_health"));
    }

    // ── evaluate: no match / no rules ────────────────────────────────────────

    #[test]
    fn engine_with_no_rules_returns_empty() {
        let mut engine = ChatPatternRuleEngine::new();
        assert!(
            engine
                .evaluate(&ChatChannel::Say, "s", "anything")
                .is_empty()
        );
    }

    #[test]
    fn engine_returns_empty_when_no_pattern_matches() {
        let mut engine = ChatPatternRuleEngine::new();
        engine.add_rule(make_rule("r", "specific_keyword", PatternType::Literal));
        assert!(
            engine
                .evaluate(&ChatChannel::Say, "s", "unrelated message")
                .is_empty()
        );
    }

    // ── evaluate: sender included in match ───────────────────────────────────

    #[test]
    fn pattern_matched_against_sender_and_message() {
        let mut engine = ChatPatternRuleEngine::new();
        engine.add_rule(make_rule("r", "Adventurer", PatternType::Literal));

        // Match against sender name "Adventurer: msg"
        let actions = engine.evaluate(&ChatChannel::Say, "Adventurer", "hello");
        assert_eq!(actions.len(), 1);
    }

    // ── uuid_v4 produces valid-looking IDs ──────────────────────────────────

    #[test]
    fn uuid_v4_has_correct_format() {
        let id = uuid_v4();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 16);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
        // Version digit must be '4'
        assert!(parts[2].starts_with('4'));
    }
}
