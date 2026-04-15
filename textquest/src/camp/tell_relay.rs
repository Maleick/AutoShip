//! Tell relaying and chat forwarding — OpenVanilla MQ2RelayTells parity.
//!
//! `TellRelay` records incoming tells and can format relay commands to forward
//! them to another character via the IPC slash-command channel.

#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::needless_pass_by_value,
    clippy::too_long_first_doc_paragraph,
    clippy::return_self_not_must_use
)]

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single tell that has been recorded by the relay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TellEntry {
    /// The character who sent the tell.
    pub from: String,
    /// The character who received the tell.
    pub to: String,
    /// The tell message body.
    pub message: String,
    /// UTC timestamp when the tell arrived.
    pub timestamp: DateTime<Utc>,
    /// Whether this tell has been forwarded to another character.
    pub forwarded: bool,
}

/// Configuration for the tell relay, suitable for TOML deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TellRelayConfig {
    /// Enable or disable the relay.
    #[serde(default = "bool_true")]
    pub enabled: bool,
    /// Keywords that trigger a mention alert (case-insensitive).
    #[serde(default)]
    pub mention_keywords: Vec<String>,
    /// Maximum number of tells to retain in history.
    #[serde(default = "default_max_history")]
    pub max_history: usize,
}

fn bool_true() -> bool {
    true
}

fn default_max_history() -> usize {
    100
}

impl Default for TellRelayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mention_keywords: Vec::new(),
            max_history: 100,
        }
    }
}

/// Inter-character tell relay — records tells and formats forward commands.
#[derive(Debug, Clone)]
pub struct TellRelay {
    /// Whether the relay is active.
    pub enabled: bool,
    /// Bounded ring-buffer of recent tells.
    pub history: VecDeque<TellEntry>,
    /// Keywords that trigger a mention alert (stored lower-cased for fast
    /// matching).
    pub mention_keywords: Vec<String>,
    /// Maximum entries kept in `history`.
    max_history: usize,
}

impl TellRelay {
    /// Create a new relay from a config.
    #[must_use]
    pub fn from_config(cfg: &TellRelayConfig) -> Self {
        Self {
            enabled: cfg.enabled,
            history: VecDeque::new(),
            mention_keywords: cfg
                .mention_keywords
                .iter()
                .map(|k| k.to_lowercase())
                .collect(),
            max_history: cfg.max_history,
        }
    }

    /// Create a relay with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::from_config(&TellRelayConfig::default())
    }

    /// Record an incoming tell.
    ///
    /// Trims history to `max_history` by dropping the oldest entry when full.
    pub fn record_tell(
        &mut self,
        from: impl Into<String>,
        to: impl Into<String>,
        message: impl Into<String>,
        now: DateTime<Utc>,
    ) {
        let entry = TellEntry {
            from: from.into(),
            to: to.into(),
            message: message.into(),
            timestamp: now,
            forwarded: false,
        };

        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(entry);
    }

    /// Format a relay entry as an EQ slash command string.
    ///
    /// Returns a `/tell` command that wraps the original tell in a relay
    /// header, suitable for delivery through the IPC command channel.
    #[must_use]
    pub fn forwarded_as_cmd(entry: &TellEntry) -> String {
        format!(
            "/tell {} [Relay from {}]: {}",
            entry.to, entry.from, entry.message
        )
    }

    /// Returns `true` if `message` contains any of the configured mention
    /// keywords.
    ///
    /// Comparison is case-insensitive.
    #[must_use]
    pub fn mentions_keyword(&self, message: &str) -> bool {
        if self.mention_keywords.is_empty() {
            return false;
        }
        let lower = message.to_lowercase();
        self.mention_keywords
            .iter()
            .any(|kw| lower.contains(kw.as_str()))
    }
}

impl Default for TellRelay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    // ── record_tell ──────────────────────────────────────────────────────────

    #[test]
    fn test_record_tell_stores_entry() {
        let mut relay = TellRelay::new();
        relay.record_tell("Sender", "Receiver", "hello", ts(1_000_000));
        assert_eq!(relay.history.len(), 1);
        let entry = &relay.history[0];
        assert_eq!(entry.from, "Sender");
        assert_eq!(entry.to, "Receiver");
        assert_eq!(entry.message, "hello");
        assert!(!entry.forwarded);
    }

    #[test]
    fn test_record_tell_caps_at_max_history() {
        let cfg = TellRelayConfig {
            enabled: true,
            mention_keywords: vec![],
            max_history: 3,
        };
        let mut relay = TellRelay::from_config(&cfg);

        relay.record_tell("A", "B", "msg1", ts(1));
        relay.record_tell("A", "B", "msg2", ts(2));
        relay.record_tell("A", "B", "msg3", ts(3));
        // At capacity — adding a 4th should drop the oldest
        relay.record_tell("A", "B", "msg4", ts(4));

        assert_eq!(relay.history.len(), 3);
        // Oldest (msg1) should have been dropped
        assert_eq!(relay.history[0].message, "msg2");
        assert_eq!(relay.history[2].message, "msg4");
    }

    #[test]
    fn test_record_tell_exactly_at_max_history() {
        let cfg = TellRelayConfig {
            max_history: 5,
            ..TellRelayConfig::default()
        };
        let mut relay = TellRelay::from_config(&cfg);
        for i in 0..5u32 {
            relay.record_tell("A", "B", format!("msg{i}"), ts(i64::from(i)));
        }
        assert_eq!(relay.history.len(), 5);
    }

    // ── forwarded_as_cmd ─────────────────────────────────────────────────────

    #[test]
    fn test_forwarded_as_cmd_format() {
        let entry = TellEntry {
            from: "Kira".to_string(),
            to: "Warrior01".to_string(),
            message: "help me".to_string(),
            timestamp: ts(0),
            forwarded: false,
        };
        let cmd = TellRelay::forwarded_as_cmd(&entry);
        assert_eq!(cmd, "/tell Warrior01 [Relay from Kira]: help me");
    }

    #[test]
    fn test_forwarded_as_cmd_preserves_spaces_in_message() {
        let entry = TellEntry {
            from: "Alpha".to_string(),
            to: "Beta".to_string(),
            message: "  spaced out  ".to_string(),
            timestamp: ts(0),
            forwarded: false,
        };
        let cmd = TellRelay::forwarded_as_cmd(&entry);
        assert_eq!(cmd, "/tell Beta [Relay from Alpha]:   spaced out  ");
    }

    // ── mentions_keyword ─────────────────────────────────────────────────────

    #[test]
    fn test_mentions_keyword_case_insensitive() {
        let cfg = TellRelayConfig {
            mention_keywords: vec!["dragon".to_string()],
            ..TellRelayConfig::default()
        };
        let relay = TellRelay::from_config(&cfg);

        assert!(relay.mentions_keyword("A DRAGON appeared!"));
        assert!(relay.mentions_keyword("a dragon appeared!"));
        assert!(relay.mentions_keyword("Dragon is here"));
        assert!(!relay.mentions_keyword("no monsters here"));
    }

    #[test]
    fn test_mentions_keyword_multiple_keywords() {
        let cfg = TellRelayConfig {
            mention_keywords: vec!["help".to_string(), "sos".to_string()],
            ..TellRelayConfig::default()
        };
        let relay = TellRelay::from_config(&cfg);

        assert!(relay.mentions_keyword("SOS I need backup"));
        assert!(relay.mentions_keyword("Help please"));
        assert!(!relay.mentions_keyword("everything is fine"));
    }

    #[test]
    fn test_mentions_keyword_empty_keywords_never_matches() {
        let relay = TellRelay::new(); // no keywords configured
        assert!(!relay.mentions_keyword("dragon help sos anything"));
    }

    #[test]
    fn test_mentions_keyword_empty_message() {
        let cfg = TellRelayConfig {
            mention_keywords: vec!["dragon".to_string()],
            ..TellRelayConfig::default()
        };
        let relay = TellRelay::from_config(&cfg);
        assert!(!relay.mentions_keyword(""));
    }

    // ── disabled relay ────────────────────────────────────────────────────────

    #[test]
    fn test_disabled_relay_skips_forward() {
        // When the relay is disabled, callers should check `enabled` before
        // calling `forwarded_as_cmd`. This test verifies that `enabled = false`
        // is correctly set from config and that a caller can gate on it.
        let cfg = TellRelayConfig {
            enabled: false,
            mention_keywords: vec![],
            max_history: 100,
        };
        let mut relay = TellRelay::from_config(&cfg);
        assert!(!relay.enabled, "relay should be disabled");

        relay.record_tell("A", "B", "hello", ts(0));

        // Simulate what the caller would do: skip forwarding if disabled
        let forwarded_count = relay
            .history
            .iter()
            .filter(|_| relay.enabled) // gate on enabled
            .count();
        assert_eq!(forwarded_count, 0, "disabled relay should forward 0 tells");
    }

    // ── config defaults ───────────────────────────────────────────────────────

    #[test]
    fn test_config_defaults() {
        let cfg = TellRelayConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.max_history, 100);
        assert!(cfg.mention_keywords.is_empty());
    }

    #[test]
    fn test_from_config_lowercases_keywords() {
        let cfg = TellRelayConfig {
            mention_keywords: vec!["Dragon".to_string(), "NAMED".to_string()],
            ..TellRelayConfig::default()
        };
        let relay = TellRelay::from_config(&cfg);
        assert!(
            relay
                .mention_keywords
                .iter()
                .all(|k| k == k.to_lowercase().as_str())
        );
    }

    // ── serialization ─────────────────────────────────────────────────────────

    #[test]
    fn test_tell_entry_serialization_roundtrip() {
        let entry = TellEntry {
            from: "Alice".to_string(),
            to: "Bob".to_string(),
            message: "test".to_string(),
            timestamp: ts(999_999),
            forwarded: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let restored: TellEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.from, "Alice");
        assert_eq!(restored.to, "Bob");
        assert_eq!(restored.message, "test");
        assert!(restored.forwarded);
        assert_eq!(restored.timestamp, entry.timestamp);
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let cfg = TellRelayConfig {
            enabled: false,
            mention_keywords: vec!["raid".to_string()],
            max_history: 50,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: TellRelayConfig = serde_json::from_str(&json).unwrap();
        assert!(!restored.enabled);
        assert_eq!(restored.max_history, 50);
        assert_eq!(restored.mention_keywords, vec!["raid"]);
    }
}
