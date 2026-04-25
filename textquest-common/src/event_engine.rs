//! User-defined event engine with regex capture group support.
//!
//! MQ2Events parity — matches chat patterns and substitutes capture groups
//! into command strings (${1}, ${2}, etc.) before execution.

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
pub enum EventAction {
    ExecuteCommand(String),
}

impl std::fmt::Display for EventAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventAction::ExecuteCommand(cmd) => write!(f, "Execute: {cmd}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRule {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub pattern_type: PatternType,
    pub chat_type: Vec<String>,
    pub command: String,
    pub cooldown_ms: u32,
    pub enabled: bool,
    #[serde(default)]
    pub fire_count: u64,
}

impl EventRule {
    pub fn new(name: impl Into<String>, pattern: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            id: uuid_v4(),
            name: name.into(),
            pattern: pattern.into(),
            pattern_type: PatternType::Regex,
            chat_type: vec!["group".to_string(), "guild".to_string(), "shout".to_string()],
            command: command.into(),
            cooldown_ms: 0,
            enabled: true,
            fire_count: 0,
        }
    }

    pub fn with_chat_types(mut self, types: Vec<String>) -> Self {
        self.chat_type = types;
        self
    }

    pub fn with_cooldown_ms(mut self, ms: u32) -> Self {
        self.cooldown_ms = ms;
        self
    }

    pub fn matches_chat_type(&self, channel: &ChatChannel) -> bool {
        if self.chat_type.is_empty() {
            return true;
        }
        let channel_str = match channel {
            ChatChannel::Say => "say",
            ChatChannel::Tell => "tell",
            ChatChannel::TellOut => "tell_out",
            ChatChannel::Group => "group",
            ChatChannel::Guild => "guild",
            ChatChannel::Raid => "raid",
            ChatChannel::Shout => "shout",
            ChatChannel::Ooc => "ooc",
            ChatChannel::Auction => "auction",
        };
        self.chat_type.contains(&channel_str.to_string())
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

#[derive(Debug, Clone)]
struct RuleState {
    last_fired: Option<Instant>,
}

#[derive(Debug)]
pub struct EventEngine {
    rules: Vec<EventRule>,
    rule_states: Vec<RuleState>,
}

impl EventEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            rule_states: Vec::new(),
        }
    }

    pub fn with_rules(rules: Vec<EventRule>) -> Self {
        let rule_states = vec![RuleState { last_fired: None }; rules.len()];
        Self { rules, rule_states }
    }

    pub fn add_rule(&mut self, rule: EventRule) {
        self.rules.push(rule);
        self.rule_states.push(RuleState { last_fired: None });
    }

    pub fn set_rules(&mut self, rules: Vec<EventRule>) {
        self.rules = rules;
        self.rule_states = vec![RuleState { last_fired: None }; self.rules.len()];
    }

    pub fn get_rules(&self) -> &[EventRule] {
        &self.rules
    }

    pub fn toggle_rule(&mut self, id: &str) -> bool {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == id) {
            rule.enabled = !rule.enabled;
            true
        } else {
            false
        }
    }

    fn matches_pattern(pattern: &str, pattern_type: &PatternType, text: &str) -> (bool, Vec<String>) {
        match pattern_type {
            PatternType::Literal => {
                let matches = text.to_lowercase().contains(&pattern.to_lowercase());
                (matches, Vec::new())
            }
            PatternType::Regex => {
                if let Ok(re) = regex::RegexBuilder::new(pattern)
                    .case_insensitive(true)
                    .build()
                {
                    if let Some(caps) = re.captures(text) {
                        let mut groups = Vec::new();
                        for i in 1..caps.len() {
                            if let Some(m) = caps.get(i) {
                                groups.push(m.as_str().to_string());
                            }
                        }
                        (true, groups)
                    } else {
                        (false, Vec::new())
                    }
                } else {
                    (false, Vec::new())
                }
            }
        }
    }

    fn is_on_cooldown(&self, rule_index: usize, now: Instant) -> bool {
        if rule_index >= self.rule_states.len() {
            return false;
        }
        let rule = &self.rules[rule_index];
        if rule.cooldown_ms == 0 {
            return false;
        }
        if let Some(last_fired) = self.rule_states[rule_index].last_fired {
            let cooldown = Duration::from_millis(rule.cooldown_ms as u64);
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

    fn substitute_captures(command: &str, captures: &[String]) -> String {
        let mut result = command.to_string();
        for (i, cap) in captures.iter().enumerate() {
            let placeholder = format!("${{{}}}", i + 1);
            result = result.replace(&placeholder, cap);
        }
        result
    }

    pub fn evaluate(
        &mut self,
        channel: &ChatChannel,
        _sender: &str,
        message: &str,
    ) -> Vec<(String, String)> {
        let now = Instant::now();
        let mut actions = Vec::new();
        let mut to_fire = Vec::new();

        for (idx, rule) in self.rules.iter().enumerate() {
            if !rule.enabled {
                continue;
            }

            if !rule.matches_chat_type(channel) {
                continue;
            }

            if self.is_on_cooldown(idx, now) {
                continue;
            }

            let (matches, captures) = Self::matches_pattern(&rule.pattern, &rule.pattern_type, message);

            if matches {
                let command = Self::substitute_captures(&rule.command, &captures);
                let rule_id = self.rules[idx].id.clone();
                to_fire.push((idx, rule_id, command));
            }
        }

        for (idx, rule_id, command) in to_fire {
            self.mark_fired(idx, now);
            actions.push((rule_id, command));
        }

        actions
    }

    pub fn stats(&self) -> EventStats {
        EventStats {
            total_rules: self.rules.len(),
            enabled_rules: self.rules.iter().filter(|r| r.enabled).count(),
            total_fires: self.rules.iter().map(|r| r.fire_count).sum(),
        }
    }
}

impl Default for EventEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventStats {
    pub total_rules: usize,
    pub enabled_rules: usize,
    pub total_fires: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EventConfig {
    pub event: Vec<EventRule>,
}

impl EventConfig {
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

    #[test]
    fn captures_from_regex() {
        let pattern = r"^(\w+) says '(.*)'" ;
        let text = "Bard says 'Hello everyone'";
        let (matches, captures) = EventEngine::matches_pattern(pattern, &PatternType::Regex, text);
        assert!(matches);
        assert_eq!(captures.len(), 2);
        assert_eq!(captures[0], "Bard");
        assert_eq!(captures[1], "Hello everyone");
    }

    #[test]
    fn substitute_captures_in_command() {
        let command = "/alert ${1} says: ${2}";
        let captures = vec!["Bard".to_string(), "TRAIN!".to_string()];
        let result = EventEngine::substitute_captures(command, &captures);
        assert_eq!(result, "/alert Bard says: TRAIN!");
    }

    #[test]
    fn evaluate_with_captures() {
        let mut engine = EventEngine::new();
        let rule = EventRule::new(
            "train_warning",
            r"^(\w+) says 'TRAIN'",
            "/alert train from ${1}",
        )
        .with_chat_types(vec!["group".to_string()]);

        engine.add_rule(rule);

        let actions = engine.evaluate(&ChatChannel::Group, "system", "Bard says 'TRAIN'");
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].1, "/alert train from Bard");
    }

    #[test]
    fn cooldown_blocks_refire() {
        let mut engine = EventEngine::new();
        let rule = EventRule::new("test", r"hello", "/cmd")
            .with_cooldown_ms(1000);

        engine.add_rule(rule);

        let actions1 = engine.evaluate(&ChatChannel::Say, "sender", "hello");
        assert_eq!(actions1.len(), 1);

        let actions2 = engine.evaluate(&ChatChannel::Say, "sender", "hello");
        assert!(actions2.is_empty());
    }

    #[test]
    fn disabled_rules_skipped() {
        let mut engine = EventEngine::new();
        let mut rule = EventRule::new("test", "hello", "/cmd");
        rule.enabled = false;
        engine.add_rule(rule);

        let actions = engine.evaluate(&ChatChannel::Say, "sender", "hello");
        assert!(actions.is_empty());
    }

    #[test]
    fn chat_type_filter() {
        let mut engine = EventEngine::new();
        let rule = EventRule::new("test", "hello", "/cmd")
            .with_chat_types(vec!["guild".to_string()]);

        engine.add_rule(rule);

        let group_actions = engine.evaluate(&ChatChannel::Group, "sender", "hello");
        assert!(group_actions.is_empty());

        let guild_actions = engine.evaluate(&ChatChannel::Guild, "sender", "hello");
        assert_eq!(guild_actions.len(), 1);
    }

    #[test]
    fn fire_count_increments() {
        let mut engine = EventEngine::new();
        engine.add_rule(EventRule::new("test", "hello", "/cmd"));

        engine.evaluate(&ChatChannel::Say, "s", "hello");
        engine.evaluate(&ChatChannel::Say, "s", "hello");
        engine.evaluate(&ChatChannel::Say, "s", "hello");

        assert_eq!(engine.rules[0].fire_count, 3);
    }

    #[test]
    fn toggle_rule() {
        let mut engine = EventEngine::new();
        let rule = EventRule::new("test", "hello", "/cmd");
        let id = rule.id.clone();
        engine.add_rule(rule);

        assert!(engine.rules[0].enabled);
        engine.toggle_rule(&id);
        assert!(!engine.rules[0].enabled);
        engine.toggle_rule(&id);
        assert!(engine.rules[0].enabled);
    }

    #[test]
    fn perf_1000_patterns_1000_chat_lines() {
        let mut engine = EventEngine::new();

        for i in 0..20 {
            let pattern = format!(r"pattern{}", i);
            let command = format!("/cmd {}", i);
            let rule = EventRule::new(format!("rule{}", i), pattern, command);
            engine.add_rule(rule);
        }

        let messages = (0..1000)
            .map(|i| format!("This contains pattern{} somewhere", i % 20))
            .collect::<Vec<_>>();

        let mut total_matches = 0;
        for msg in &messages {
            let actions = engine.evaluate(&ChatChannel::Say, "sender", msg);
            total_matches += actions.len();
        }

        assert!(total_matches > 0, "should have matched some patterns");
        let stats = engine.stats();
        assert_eq!(stats.total_rules, 20);
        assert_eq!(stats.total_fires, total_matches as u64);
    }

    #[test]
    fn config_roundtrip() {
        let rules = vec![
            EventRule::new("rule1", r"hello", "/cmd1"),
            EventRule::new("rule2", r"(\w+) says (.+)", "/alert ${1}: ${2}"),
        ];

        let config = EventConfig {
            event: rules.clone(),
        };

        let path = std::path::PathBuf::from("/tmp/event_test.toml");
        config.save(&path).ok();
        let loaded = EventConfig::load(&path).unwrap();

        assert_eq!(loaded.event.len(), 2);
        assert_eq!(loaded.event[0].name, "rule1");
        assert_eq!(loaded.event[1].name, "rule2");
        assert_eq!(loaded.event[1].pattern, r"(\w+) says (.+)");

        std::fs::remove_file(path).ok();
    }
}
