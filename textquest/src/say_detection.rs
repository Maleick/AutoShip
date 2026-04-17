//! Say channel detection and alerting — MQ2Say parity.
//!
//! Detects specific text patterns in the `/say` channel and triggers
//! alerts or actions. Used for detecting NPC dialogue (quest triggers,
//! warning messages), player communication attempts, and GM messages
//! arriving via /say.

use regex::RegexBuilder;
use textquest_common::chat::{ChatChannel, ChatEvent};

/// Pattern matching mode for say detection rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SayPattern {
    /// Case-insensitive substring match.
    Substring,
    /// Case-insensitive exact string match.
    Exact,
    /// Case-insensitive regular-expression pattern.
    Regex,
}

/// Action to take when a say rule matches.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SayAction {
    /// Fire an alert through the configured notification channels.
    Alert,
    /// Send a slash command string via IPC to all clients.
    Broadcast(String),
    /// Run a slash command on the client that saw the `/say` line.
    Command(String),
}

/// A rule match with the action that should be executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SayMatch {
    /// Human-readable rule name.
    pub rule_name: String,
    /// Source pattern that matched.
    pub pattern: String,
    /// Action to execute.
    pub action: SayAction,
}

/// A say detection rule: pattern → action.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SayRule {
    /// Human-readable name for this rule.
    pub name: String,
    /// Pattern text to match against say messages.
    pub pattern: String,
    /// Pattern matching mode.
    pub pattern_type: SayPattern,
    /// Action to take when matched.
    pub action: SayAction,
    /// Whether this rule is active.
    pub enabled: bool,
    /// Number of times this rule has matched.
    #[serde(default)]
    pub match_count: u64,
}

impl SayRule {
    /// Create a new enabled rule with zero match count.
    pub fn new(
        name: impl Into<String>,
        pattern: impl Into<String>,
        pattern_type: SayPattern,
        action: SayAction,
    ) -> Self {
        Self {
            name: name.into(),
            pattern: pattern.into(),
            pattern_type,
            action,
            enabled: true,
            match_count: 0,
        }
    }

    fn ascii_icontains(haystack: &str, needle: &str) -> bool {
        if needle.is_empty() {
            return false;
        }

        let haystack = haystack.as_bytes();
        let needle = needle.as_bytes();
        if needle.len() > haystack.len() {
            return false;
        }

        haystack.windows(needle.len()).any(|window| {
            window
                .iter()
                .zip(needle.iter())
                .all(|(left, right)| left.eq_ignore_ascii_case(right))
        })
    }

    /// Check if this rule's pattern matches the given message text.
    pub fn matches(&self, message: &str) -> bool {
        if !self.enabled {
            return false;
        }

        match self.pattern_type {
            SayPattern::Substring => Self::ascii_icontains(message, &self.pattern),
            SayPattern::Exact => message.eq_ignore_ascii_case(&self.pattern),
            SayPattern::Regex => RegexBuilder::new(&self.pattern)
                .case_insensitive(true)
                .build()
                .is_ok_and(|regex| regex.is_match(message)),
        }
    }

    /// Create an alert rule.
    pub fn alert(name: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self::new(name, pattern, SayPattern::Substring, SayAction::Alert)
    }

    /// Create a broadcast rule.
    pub fn broadcast(
        name: impl Into<String>,
        pattern: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        Self::new(
            name,
            pattern,
            SayPattern::Substring,
            SayAction::Broadcast(command.into()),
        )
    }

    /// Create a local-command rule.
    pub fn command(
        name: impl Into<String>,
        pattern: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        Self::new(
            name,
            pattern,
            SayPattern::Substring,
            SayAction::Command(command.into()),
        )
    }
}

/// Engine for evaluating say detection rules against chat events.
#[derive(Debug, Default)]
pub struct SayDetector {
    /// All registered rules (enabled and disabled).
    pub rules: Vec<SayRule>,
}

impl SayDetector {
    /// Create a new detector with no rules.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rule to the detector.
    pub fn add_rule(&mut self, rule: SayRule) {
        self.rules.push(rule);
    }

    /// Clear all rules.
    pub fn clear_rules(&mut self) {
        self.rules.clear();
    }

    /// Evaluate all enabled rules against a say channel chat event.
    pub fn evaluate(&mut self, event: &ChatEvent) -> Vec<SayMatch> {
        if event.channel != ChatChannel::Say {
            return Vec::new();
        }

        let mut matches = Vec::new();
        for rule in &mut self.rules {
            if rule.matches(&event.message) {
                rule.match_count += 1;
                matches.push(SayMatch {
                    rule_name: rule.name.clone(),
                    pattern: rule.pattern.clone(),
                    action: rule.action.clone(),
                });
            }
        }

        matches
    }

    /// Check if any rule matches (for lightweight detection).
    pub fn has_match(&self, event: &ChatEvent) -> bool {
        if event.channel != ChatChannel::Say {
            return false;
        }

        self.rules.iter().any(|rule| rule.matches(&event.message))
    }

    /// Get count of enabled rules.
    pub fn enabled_count(&self) -> usize {
        self.rules.iter().filter(|rule| rule.enabled).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn say_event(text: &str) -> ChatEvent {
        ChatEvent {
            channel: ChatChannel::Say,
            sender: "TestNPC".into(),
            message: text.into(),
            target: None,
        }
    }

    fn non_say_event(text: &str) -> ChatEvent {
        ChatEvent {
            channel: ChatChannel::Tell,
            sender: "TestPlayer".into(),
            message: text.into(),
            target: Some("MyChar".into()),
        }
    }

    #[test]
    fn substring_match_is_case_insensitive() {
        let rule = SayRule::alert("quest", "quest");
        assert!(rule.matches("I have a quest for you"));
        assert!(rule.matches("QUEST"));
        assert!(rule.matches("A Quest Begins"));
        assert!(rule.matches("questing"));
        assert!(!rule.matches("hail and well met"));
    }

    #[test]
    fn exact_match_requires_full_string() {
        let rule = SayRule::new("exact test", "hello", SayPattern::Exact, SayAction::Alert);
        assert!(rule.matches("hello"));
        assert!(rule.matches("HELLO"));
        assert!(!rule.matches("hello world"));
        assert!(!rule.matches("say hello"));
    }

    #[test]
    fn regex_pattern_matches_case_insensitive_word_boundary() {
        let rule = SayRule::new(
            "regex test",
            r"\bwarning\b",
            SayPattern::Regex,
            SayAction::Alert,
        );
        assert!(rule.matches("WARNING: danger ahead"));
        assert!(rule.matches("This is a warning"));
        assert!(!rule.matches("nowarning"));
    }

    #[test]
    fn invalid_regex_does_not_match() {
        let rule = SayRule::new("bad regex", "(", SayPattern::Regex, SayAction::Alert);
        assert!(!rule.matches("anything"));
    }

    #[test]
    fn disabled_rule_never_matches() {
        let mut rule = SayRule::alert("disabled", "test");
        rule.enabled = false;
        assert!(!rule.matches("test message"));
    }

    #[test]
    fn say_channel_only_matches_say_events() {
        let mut detector = SayDetector::new();
        detector.add_rule(SayRule::alert("test", "help"));

        assert!(detector.has_match(&say_event("help me!")));
        assert!(!detector.has_match(&non_say_event("help me!")));
    }

    #[test]
    fn match_count_increments() {
        let mut detector = SayDetector::new();
        detector.add_rule(SayRule::alert("test", "help"));

        detector.evaluate(&say_event("help me!"));
        detector.evaluate(&say_event("help!"));
        detector.evaluate(&say_event("not matching"));

        assert_eq!(detector.rules[0].match_count, 2);
    }

    #[test]
    fn multiple_rules_can_fire() {
        let mut detector = SayDetector::new();
        detector.add_rule(SayRule::alert("alert1", "danger"));
        detector.add_rule(SayRule::broadcast("broadcast1", "danger", "/bc GM alert!"));

        let matches = detector.evaluate(&say_event("danger detected!"));
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].rule_name, "alert1");
        assert_eq!(matches[1].rule_name, "broadcast1");
    }

    #[test]
    fn command_and_broadcast_payloads_are_preserved() {
        let mut detector = SayDetector::new();
        detector.add_rule(SayRule::broadcast("gold", "gold", "/bc Found gold!"));
        detector.add_rule(SayRule::command("hail", "hail", "/say ready"));

        let matches = detector.evaluate(&say_event("You found gold, hail the herald!"));

        assert_eq!(
            matches,
            vec![
                SayMatch {
                    rule_name: "gold".into(),
                    pattern: "gold".into(),
                    action: SayAction::Broadcast("/bc Found gold!".into()),
                },
                SayMatch {
                    rule_name: "hail".into(),
                    pattern: "hail".into(),
                    action: SayAction::Command("/say ready".into()),
                },
            ]
        );
    }

    #[test]
    fn enabled_count_excludes_disabled_rules() {
        let mut detector = SayDetector::new();
        detector.add_rule(SayRule::alert("enabled1", "test"));
        detector.add_rule(SayRule::alert("disabled", "test"));
        detector.rules[1].enabled = false;

        assert_eq!(detector.enabled_count(), 1);
    }
}
