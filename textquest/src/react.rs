//! MQ2React native equivalent — configurable pattern → action rule engine.
//!
//! Users define rules that match incoming chat messages (any channel) against
//! substring, exact, or regular-expression patterns and fire one of several
//! actions: alert, play a sound, speak via TTS, send a Discord webhook, or
//! dispatch an IPC command.
//!
//! Rules are persisted to a SQLite database so they survive restarts and can
//! be edited through the web UI at runtime.  The engine pre-compiles regex
//! patterns on load so evaluation over ≥50 active rules remains O(n) with
//! negligible per-message overhead.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use regex::Regex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

#[cfg(windows)]
use crate::camp::event_triggers::GameEvent;

// ── public types ───────────────────────────────────────────────────────────

/// How a rule pattern is matched against incoming text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactPattern {
    /// Case-insensitive substring containment check.
    Substring,
    /// Case-insensitive whole-string equality.
    Exact,
    /// Case-insensitive regular expression (RE2 / Rust regex syntax).
    Regex,
}

/// Action executed when a rule matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum ReactAction {
    /// Raise an operational alert with the given message template.
    /// `{match}` in the template is replaced with the matched text.
    Alert(String),
    /// Play a sound file (absolute path).  Empty string → system beep.
    PlaySound(String),
    /// Speak text via TTS.  Supports `{match}` substitution.
    Speak(String),
    /// Send a Discord webhook alert with the given message.
    DiscordAlert(String),
    /// Dispatch an IPC slash command to all active clients.
    IpcCommand(String),
}

/// A single React rule: condition → action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactRule {
    /// Database row ID. `None` for unsaved rules.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Human-readable name shown in the web UI.
    pub name: String,
    /// Optional EQ channel filter (e.g. `"say"`, `"tell"`, `"group"`).
    /// `None` matches messages on any channel.
    pub channel_filter: Option<String>,
    /// Pattern text to match.
    pub pattern: String,
    /// Pattern matching mode.
    pub pattern_type: ReactPattern,
    /// Action to take on match.
    pub action: ReactAction,
    /// Rules are evaluated in ascending priority order (lower number first).
    #[serde(default)]
    pub priority: i32,
    /// `false` disables without deleting.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Cumulative match count (informational, persisted).
    #[serde(default)]
    pub fire_count: u64,
}

fn default_true() -> bool {
    true
}

/// Result of evaluating a rule against a message.
#[derive(Debug, Clone)]
pub struct ReactMatch {
    /// Rule that fired.
    pub rule_id: i64,
    /// Rule name.
    pub rule_name: String,
    /// Action to execute.
    pub action: ReactAction,
}

// ── compiled rule (internal) ───────────────────────────────────────────────

struct CompiledRule {
    id: i64,
    name: String,
    channel_filter: Option<String>,
    pattern: String,
    pattern_type: ReactPattern,
    action: ReactAction,
    priority: i32,
    enabled: bool,
    compiled_regex: Option<Regex>,
}

impl CompiledRule {
    fn from_rule(rule: &ReactRule) -> Result<Self> {
        let compiled_regex = if rule.pattern_type == ReactPattern::Regex {
            let re = Regex::new(&format!("(?i){}", rule.pattern)).with_context(|| {
                format!(
                    "invalid regex in rule {:?}: {}",
                    rule.name, rule.pattern
                )
            })?;
            Some(re)
        } else {
            None
        };

        Ok(Self {
            id: rule.id.unwrap_or(0),
            name: rule.name.clone(),
            channel_filter: rule.channel_filter.clone(),
            pattern: rule.pattern.clone(),
            pattern_type: rule.pattern_type,
            action: rule.action.clone(),
            priority: rule.priority,
            enabled: rule.enabled,
            compiled_regex,
        })
    }

    fn matches(&self, text: &str, channel: Option<&str>) -> bool {
        if !self.enabled {
            return false;
        }

        // Channel filter: skip if channel does not match.
        if let Some(ref filter) = self.channel_filter {
            match channel {
                Some(ch) if ch.eq_ignore_ascii_case(filter) => {}
                _ => return false,
            }
        }

        match self.pattern_type {
            ReactPattern::Substring => ascii_icontains(text, &self.pattern),
            ReactPattern::Exact => text.eq_ignore_ascii_case(&self.pattern),
            ReactPattern::Regex => {
                if let Some(ref re) = self.compiled_regex {
                    re.is_match(text)
                } else {
                    false
                }
            }
        }
    }
}

fn ascii_icontains(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.len() > h.len() {
        return false;
    }
    h.windows(n.len())
        .any(|w| w.iter().zip(n.iter()).all(|(a, b)| a.eq_ignore_ascii_case(b)))
}

// ── engine ─────────────────────────────────────────────────────────────────

const REACT_DB_DEFAULT: &str = "data/react_rules.db";
const REACT_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS react_rules (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    name           TEXT    NOT NULL,
    channel_filter TEXT,
    pattern        TEXT    NOT NULL,
    pattern_type   TEXT    NOT NULL DEFAULT 'substring',
    action_type    TEXT    NOT NULL,
    action_value   TEXT    NOT NULL DEFAULT '',
    priority       INTEGER NOT NULL DEFAULT 0,
    enabled        INTEGER NOT NULL DEFAULT 1,
    fire_count     INTEGER NOT NULL DEFAULT 0
);
";

/// The React rule engine — loads, evaluates, and persists pattern→action rules.
///
/// Wraps a SQLite connection for persistence and keeps a sorted, pre-compiled
/// in-memory rule set for zero-allocation hot-path evaluation.
pub struct ReactEngine {
    db: Arc<Mutex<Connection>>,
    rules: Vec<CompiledRule>,
    last_reload: Option<Instant>,
    reload_interval: Duration,
}

impl ReactEngine {
    /// Open (or create) the rule database at `db_path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or migrated.
    pub fn open(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create react db dir {}", parent.display()))?;
        }

        let conn = Connection::open(db_path)
            .with_context(|| format!("open react db {}", db_path.display()))?;
        conn.execute_batch(REACT_SCHEMA)
            .context("apply react db schema")?;

        let db = Arc::new(Mutex::new(conn));
        let rules = Self::load_rules_from_db(&db)?;

        Ok(Self {
            db,
            rules,
            last_reload: Some(Instant::now()),
            reload_interval: Duration::from_secs(5),
        })
    }

    /// Open the default database path (`data/react_rules.db`).
    pub fn open_default() -> Result<Self> {
        Self::open(REACT_DB_DEFAULT)
    }

    /// Reload rules from the database if the reload interval has elapsed.
    /// Call this from the orchestrator tick loop.
    pub fn tick(&mut self) {
        let should_reload = self
            .last_reload
            .map(|t| t.elapsed() >= self.reload_interval)
            .unwrap_or(true);

        if should_reload {
            match Self::load_rules_from_db(&self.db) {
                Ok(rules) => {
                    self.rules = rules;
                    self.last_reload = Some(Instant::now());
                }
                Err(e) => {
                    tracing::warn!("react rule reload failed: {e}");
                }
            }
        }
    }

    /// Evaluate all enabled rules against `text` received on `channel`.
    ///
    /// Returns matched rules in priority order (lowest priority number first).
    /// Updating the fire count in the DB is deferred — call
    /// [`ReactEngine::record_fires`] with the returned matches.
    pub fn evaluate(&self, text: &str, channel: Option<&str>) -> Vec<ReactMatch> {
        self.rules
            .iter()
            .filter(|r| r.matches(text, channel))
            .map(|r| ReactMatch {
                rule_id: r.id,
                rule_name: r.name.clone(),
                action: r.action.clone(),
            })
            .collect()
    }

    /// Evaluate a [`GameEvent`] against all rules.
    ///
    /// Only [`GameEvent::ChatReceived`] events are matched; other variants
    /// return an empty vec.
    #[cfg(windows)]
    pub fn evaluate_event(&self, event: &GameEvent) -> Vec<ReactMatch> {
        match event {
            GameEvent::ChatReceived { message } => self.evaluate(message, None),
            _ => vec![],
        }
    }

    /// Persist incremented fire counts for the given matches.
    pub fn record_fires(&self, matches: &[ReactMatch]) {
        if matches.is_empty() {
            return;
        }
        if let Ok(db) = self.db.lock() {
            for m in matches {
                let _ = db.execute(
                    "UPDATE react_rules SET fire_count = fire_count + 1 WHERE id = ?1",
                    params![m.rule_id],
                );
            }
        }
    }

    /// Insert a new rule and return its assigned `id`.
    ///
    /// # Errors
    ///
    /// Returns an error if serialisation or the INSERT fails.
    pub fn add_rule(&mut self, rule: &ReactRule) -> Result<i64> {
        let (action_type, action_value) = encode_action(&rule.action);
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        db.execute(
            "INSERT INTO react_rules
             (name, channel_filter, pattern, pattern_type, action_type, action_value,
              priority, enabled, fire_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                rule.name,
                rule.channel_filter,
                rule.pattern,
                pattern_type_str(rule.pattern_type),
                action_type,
                action_value,
                rule.priority,
                rule.enabled as i64,
                rule.fire_count as i64,
            ],
        )
        .context("insert react rule")?;
        let id = db.last_insert_rowid();
        drop(db);

        // Reload the in-memory cache immediately.
        self.rules = Self::load_rules_from_db(&self.db)?;
        Ok(id)
    }

    /// Update an existing rule by its `id`.
    ///
    /// # Errors
    ///
    /// Returns an error if the rule is not found or the UPDATE fails.
    pub fn update_rule(&mut self, rule: &ReactRule) -> Result<()> {
        let id = rule.id.context("rule has no id")?;
        let (action_type, action_value) = encode_action(&rule.action);
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        let changed = db
            .execute(
                "UPDATE react_rules SET
               name = ?1, channel_filter = ?2, pattern = ?3, pattern_type = ?4,
               action_type = ?5, action_value = ?6, priority = ?7, enabled = ?8
             WHERE id = ?9",
                params![
                    rule.name,
                    rule.channel_filter,
                    rule.pattern,
                    pattern_type_str(rule.pattern_type),
                    action_type,
                    action_value,
                    rule.priority,
                    rule.enabled as i64,
                    id,
                ],
            )
            .context("update react rule")?;
        anyhow::ensure!(changed == 1, "react rule {id} not found");
        drop(db);
        self.rules = Self::load_rules_from_db(&self.db)?;
        Ok(())
    }

    /// Delete a rule by its `id`.
    pub fn delete_rule(&mut self, id: i64) -> Result<()> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        db.execute("DELETE FROM react_rules WHERE id = ?1", params![id])
            .context("delete react rule")?;
        drop(db);
        self.rules = Self::load_rules_from_db(&self.db)?;
        Ok(())
    }

    /// Return all rules (enabled and disabled) sorted by priority.
    pub fn list_rules(&self) -> Result<Vec<ReactRule>> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        list_rules_from_conn(&db)
    }

    /// Returns the number of currently loaded (enabled + disabled) rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    fn load_rules_from_db(db: &Arc<Mutex<Connection>>) -> Result<Vec<CompiledRule>> {
        let db = db
            .lock()
            .map_err(|_| anyhow::anyhow!("db lock poisoned"))?;
        let rules = list_rules_from_conn(&db)?;
        let mut compiled: Vec<CompiledRule> = rules
            .iter()
            .filter_map(|r| match CompiledRule::from_rule(r) {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::warn!("skipping react rule {:?}: {e}", r.name);
                    None
                }
            })
            .collect();
        compiled.sort_by_key(|c| c.priority);
        Ok(compiled)
    }
}

fn list_rules_from_conn(conn: &Connection) -> Result<Vec<ReactRule>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, channel_filter, pattern, pattern_type,
                    action_type, action_value, priority, enabled, fire_count
             FROM react_rules ORDER BY priority ASC, id ASC",
        )
        .context("prepare list react rules")?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i32>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
            ))
        })
        .context("query react rules")?;

    let mut rules = Vec::new();
    for row in rows {
        let (id, name, channel_filter, pattern, pt_str, at_str, av_str, priority, enabled, fire_count) =
            row.context("read react rule row")?;

        let pattern_type = parse_pattern_type(&pt_str);
        let action = decode_action(&at_str, &av_str);

        rules.push(ReactRule {
            id: Some(id),
            name,
            channel_filter,
            pattern,
            pattern_type,
            action,
            priority,
            enabled: enabled != 0,
            fire_count: fire_count as u64,
        });
    }

    Ok(rules)
}

fn pattern_type_str(pt: ReactPattern) -> &'static str {
    match pt {
        ReactPattern::Substring => "substring",
        ReactPattern::Exact => "exact",
        ReactPattern::Regex => "regex",
    }
}

fn parse_pattern_type(s: &str) -> ReactPattern {
    match s {
        "exact" => ReactPattern::Exact,
        "regex" => ReactPattern::Regex,
        _ => ReactPattern::Substring,
    }
}

fn encode_action(action: &ReactAction) -> (&'static str, String) {
    match action {
        ReactAction::Alert(msg) => ("alert", msg.clone()),
        ReactAction::PlaySound(path) => ("play_sound", path.clone()),
        ReactAction::Speak(text) => ("speak", text.clone()),
        ReactAction::DiscordAlert(msg) => ("discord_alert", msg.clone()),
        ReactAction::IpcCommand(cmd) => ("ipc_command", cmd.clone()),
    }
}

fn decode_action(action_type: &str, value: &str) -> ReactAction {
    match action_type {
        "play_sound" => ReactAction::PlaySound(value.to_string()),
        "speak" => ReactAction::Speak(value.to_string()),
        "discord_alert" => ReactAction::DiscordAlert(value.to_string()),
        "ipc_command" => ReactAction::IpcCommand(value.to_string()),
        _ => ReactAction::Alert(value.to_string()),
    }
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn temp_engine() -> ReactEngine {
        let dir = tempdir().expect("tempdir");
        let path = dir.into_path().join("react.db");
        ReactEngine::open(&path).expect("open engine")
    }

    fn make_rule(name: &str, pattern: &str, pt: ReactPattern, action: ReactAction) -> ReactRule {
        ReactRule {
            id: None,
            name: name.to_string(),
            channel_filter: None,
            pattern: pattern.to_string(),
            pattern_type: pt,
            action,
            priority: 0,
            enabled: true,
            fire_count: 0,
        }
    }

    #[test]
    fn substring_match() {
        let mut engine = temp_engine();
        let rule = make_rule(
            "test",
            "hello",
            ReactPattern::Substring,
            ReactAction::Alert("hi".into()),
        );
        engine.add_rule(&rule).expect("add rule");

        let matches = engine.evaluate("say HELLO world", None);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].rule_name, "test");
    }

    #[test]
    fn exact_match() {
        let mut engine = temp_engine();
        let rule = make_rule(
            "exact",
            "hello world",
            ReactPattern::Exact,
            ReactAction::Speak("hi".into()),
        );
        engine.add_rule(&rule).expect("add rule");

        assert_eq!(engine.evaluate("hello world", None).len(), 1);
        assert_eq!(engine.evaluate("hello world extra", None).len(), 0);
    }

    #[test]
    fn regex_match() {
        let mut engine = temp_engine();
        let rule = make_rule(
            "regex",
            r"named.*spawn",
            ReactPattern::Regex,
            ReactAction::PlaySound(String::new()),
        );
        engine.add_rule(&rule).expect("add rule");

        assert_eq!(
            engine.evaluate("Named mob spawn detected", None).len(),
            1
        );
        assert_eq!(engine.evaluate("just a regular mob", None).len(), 0);
    }

    #[test]
    fn channel_filter_respected() {
        let mut engine = temp_engine();
        let mut rule = make_rule(
            "chan",
            "test",
            ReactPattern::Substring,
            ReactAction::Alert("x".into()),
        );
        rule.channel_filter = Some("say".into());
        engine.add_rule(&rule).expect("add rule");

        assert_eq!(engine.evaluate("test message", Some("say")).len(), 1);
        assert_eq!(engine.evaluate("test message", Some("group")).len(), 0);
        assert_eq!(engine.evaluate("test message", None).len(), 0);
    }

    #[test]
    fn disabled_rule_skipped() {
        let mut engine = temp_engine();
        let mut rule = make_rule(
            "off",
            "hello",
            ReactPattern::Substring,
            ReactAction::Alert("x".into()),
        );
        rule.enabled = false;
        engine.add_rule(&rule).expect("add rule");
        assert_eq!(engine.evaluate("hello", None).len(), 0);
    }

    #[test]
    fn fifty_rules_no_performance_regression() {
        let mut engine = temp_engine();
        for i in 0..50 {
            let rule = make_rule(
                &format!("rule{i}"),
                &format!("pattern{i}"),
                ReactPattern::Substring,
                ReactAction::Alert(format!("alert{i}")),
            );
            engine.add_rule(&rule).expect("add rule");
        }
        assert_eq!(engine.rule_count(), 50);

        // Evaluate 1000 messages — should complete well under any timeout.
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _ = engine.evaluate("pattern42 is present in this message", None);
        }
        // 1000 evaluations over 50 rules should finish in < 500ms on any CI box.
        assert!(
            start.elapsed().as_millis() < 500,
            "evaluation too slow: {}ms",
            start.elapsed().as_millis()
        );
    }

    #[test]
    fn crud_roundtrip() {
        let mut engine = temp_engine();
        let rule = make_rule(
            "r1",
            "foo",
            ReactPattern::Substring,
            ReactAction::Alert("a".into()),
        );
        let id = engine.add_rule(&rule).expect("add");

        let rules = engine.list_rules().expect("list");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, Some(id));

        let mut updated = rules[0].clone();
        updated.pattern = "bar".into();
        engine.update_rule(&updated).expect("update");

        let rules2 = engine.list_rules().expect("list after update");
        assert_eq!(rules2[0].pattern, "bar");

        engine.delete_rule(id).expect("delete");
        assert_eq!(
            engine.list_rules().expect("list after delete").len(),
            0
        );
    }

    #[test]
    fn record_fires_increments_count() {
        let mut engine = temp_engine();
        let rule = make_rule(
            "fire",
            "hit",
            ReactPattern::Substring,
            ReactAction::Alert("a".into()),
        );
        engine.add_rule(&rule).expect("add");

        let matches = engine.evaluate("hit me", None);
        engine.record_fires(&matches);

        let rules = engine.list_rules().expect("list");
        assert_eq!(rules[0].fire_count, 1);
    }
}
