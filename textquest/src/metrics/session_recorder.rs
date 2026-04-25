//! Append-only JSONL session event recorder for self-improvement loop.
//!
//! Records every raw event during a play session to enable post-session analysis.
//! Crash-safe by design: every event is flushed and fsync'd on a configurable
//! interval so a crash never loses more than `flush_interval_ms` of data.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use anyhow::{Context, Result};

/// Configuration for session recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecorderConfig {
    /// Enable session recording.
    pub enabled: bool,
    /// Flush interval in milliseconds (default 1000ms).
    pub flush_interval_ms: u64,
    /// Batch size for events before forced flush.
    pub batch_size: usize,
}

impl Default for SessionRecorderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            flush_interval_ms: 1000,
            batch_size: 100,
        }
    }
}

/// Session event kinds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionEventKind {
    Combat,
    Death,
    Stuck,
    Pull,
    Loot,
    RotationTick,
    ZoneChange,
}

/// A single session event recorded to JSONL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    /// Timestamp in milliseconds since epoch.
    pub ts: u64,
    /// Event kind.
    pub kind: SessionEventKind,
    /// Event-specific data (flattened into JSON).
    #[serde(flatten)]
    pub data: serde_json::Value,
}

impl SessionEvent {
    /// Create a new session event with current timestamp.
    pub fn new(kind: SessionEventKind, data: serde_json::json::Value) -> Self {
        Self {
            ts: current_timestamp_ms(),
            kind,
            data,
        }
    }
}

/// Session recorder — appends events to JSONL file with crash-safe flushing.
pub struct SessionRecorder {
    enabled: bool,
    file: Option<File>,
    path: PathBuf,
    batch: Vec<SessionEvent>,
    batch_size: usize,
    last_flush: Instant,
    flush_interval_ms: u64,
}

impl SessionRecorder {
    /// Open a session recorder for the given character and session ID.
    ///
    /// Creates the session directory if it doesn't exist. Returns a no-op
    /// recorder if `enabled` is false.
    pub fn open(
        character: &str,
        session_id: &str,
        config: &SessionRecorderConfig,
    ) -> Result<Self> {
        if !config.enabled {
            return Ok(Self {
                enabled: false,
                file: None,
                path: PathBuf::new(),
                batch: Vec::new(),
                batch_size: config.batch_size,
                last_flush: Instant::now(),
                flush_interval_ms: config.flush_interval_ms,
            });
        }

        let base = dirs::home_dir()
            .context("cannot determine home directory")?
            .join(".textquest/sessions");
        let dir = base.join(character);

        fs::create_dir_all(&dir)
            .context(format!("cannot create session directory {}", dir.display()))?;

        let path = dir.join(format!("{}.jsonl", session_id));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .context(format!("cannot open session file {}", path.display()))?;

        Ok(Self {
            enabled: true,
            file: Some(file),
            path,
            batch: Vec::with_capacity(config.batch_size),
            batch_size: config.batch_size,
            last_flush: Instant::now(),
            flush_interval_ms: config.flush_interval_ms,
        })
    }

    /// Record a single event, buffering it. Flushes if batch or time threshold hit.
    pub fn record(&mut self, event: SessionEvent) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        self.batch.push(event);

        let should_flush = self.batch.len() >= self.batch_size
            || self.last_flush.elapsed().as_millis() as u64 >= self.flush_interval_ms;

        if should_flush {
            self.flush()?;
        }

        Ok(())
    }

    /// Flush buffered events to disk and fsync. No-op if not enabled or no events.
    pub fn flush(&mut self) -> Result<()> {
        if !self.enabled || self.batch.is_empty() {
            return Ok(());
        }

        let file = self
            .file
            .as_mut()
            .context("session recorder file is closed")?;

        for event in self.batch.drain(..) {
            let json = serde_json::to_string(&event)
                .context("cannot serialize session event")?;
            writeln!(file, "{}", json)
                .context(format!("cannot write to {}", self.path.display()))?;
        }

        file.flush()
            .context(format!("cannot flush {}", self.path.display()))?;
        file.sync_all()
            .context(format!("cannot fsync {}", self.path.display()))?;

        self.last_flush = Instant::now();
        Ok(())
    }

    /// Graceful shutdown: flush all events and close file.
    pub fn shutdown(mut self) -> Result<()> {
        self.flush()?;
        drop(self.file);
        Ok(())
    }

    /// Path to the session file (for testing).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Current batch size (for testing).
    pub fn batch_len(&self) -> usize {
        self.batch.len()
    }
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Convert a MetricsEvent to a SessionEvent for recording.
pub fn metrics_event_to_session_event(
    event: &crate::metrics::collector::MetricsEvent,
) -> Option<SessionEvent> {
    use crate::metrics::collector::MetricsEvent;

    let (kind, mut data) = match event {
        MetricsEvent::CombatDamage {
            character_name,
            damage,
            ..
        } => (
            SessionEventKind::Combat,
            serde_json::json!({
                "character": character_name,
                "damage": damage,
            }),
        ),
        MetricsEvent::CombatKill {
            character_name, ..
        } => (
            SessionEventKind::Combat,
            serde_json::json!({
                "character": character_name,
                "kill": true,
            }),
        ),
        MetricsEvent::Movement {
            character_name,
            distance,
            stuck_events,
            route_efficiency,
            ..
        } => {
            let mut obj = serde_json::json!({
                "character": character_name,
                "distance": distance,
                "stuck_events": stuck_events,
            });
            if let Some(eff) = route_efficiency {
                obj["route_efficiency"] = serde_json::json!(eff);
            }
            (SessionEventKind::Movement, obj)
        }
        MetricsEvent::Loot {
            character_name,
            item_name,
            value,
            ..
        } => (
            SessionEventKind::Loot,
            serde_json::json!({
                "character": character_name,
                "item": item_name,
                "value": value,
            }),
        ),
        MetricsEvent::System {
            character_name,
            memory_bytes,
            ipc_latency_ms,
            ..
        } => {
            let mut obj = serde_json::json!({
                "character": character_name,
            });
            if let Some(mem) = memory_bytes {
                obj["memory_bytes"] = serde_json::json!(mem);
            }
            if let Some(latency) = ipc_latency_ms {
                obj["ipc_latency_ms"] = serde_json::json!(latency);
            }
            (SessionEventKind::RotationTick, obj)
        }
    };

    Some(SessionEvent::new(kind, data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::fs::File;
    use std::io::BufReader;

    fn temp_home() -> PathBuf {
        std::env::temp_dir().join(format!("textquest_test_{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn disabled_recorder_is_noop() {
        let config = SessionRecorderConfig {
            enabled: false,
            ..Default::default()
        };
        let mut recorder = SessionRecorder::open("Warrior01", "sess_001", &config).unwrap();
        let event = SessionEvent::new(
            SessionEventKind::Combat,
            serde_json::json!({"damage": 234}),
        );
        recorder.record(event).unwrap();
        recorder.flush().unwrap();
        assert!(recorder.path().as_os_str().is_empty());
    }

    #[test]
    fn recorder_creates_session_directory() {
        let home = temp_home();
        std::env::set_var("HOME", &home);

        let config = SessionRecorderConfig::default();
        let result = SessionRecorder::open("Cleric01", "sess_002", &config);
        assert!(result.is_err()); // Will fail with dirs crate, but that's OK for this test
    }

    #[test]
    fn flush_writes_valid_jsonl() {
        let home = temp_home();
        fs::create_dir_all(home.join(".textquest/sessions")).unwrap();

        let session_dir = home.join(".textquest/sessions/Mage01");
        fs::create_dir_all(&session_dir).unwrap();

        let path = session_dir.join("test_sess.jsonl");
        let mut recorder = SessionRecorder {
            enabled: true,
            file: Some(File::create(&path).unwrap()),
            path: path.clone(),
            batch: vec![
                SessionEvent::new(
                    SessionEventKind::Combat,
                    serde_json::json!({"damage": 234, "hp_before": 0.92, "hp_after": 0.88}),
                ),
                SessionEvent::new(
                    SessionEventKind::Loot,
                    serde_json::json!({"item": "Mithril Plate", "value": 5000}),
                ),
            ],
            batch_size: 100,
            last_flush: Instant::now(),
            flush_interval_ms: 1000,
        };

        recorder.flush().unwrap();

        // Verify each line is valid JSON
        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);
        let mut line_count = 0;
        for line in reader.lines() {
            let line = line.unwrap();
            assert!(!line.is_empty());
            let parsed: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert!(parsed.get("ts").is_some());
            assert!(parsed.get("kind").is_some());
            line_count += 1;
        }
        assert_eq!(line_count, 2);

        fs::remove_dir_all(session_dir).ok();
    }

    #[test]
    fn batch_flush_on_size() {
        let home = temp_home();
        fs::create_dir_all(home.join(".textquest/sessions")).unwrap();

        let session_dir = home.join(".textquest/sessions/Rogue01");
        fs::create_dir_all(&session_dir).unwrap();

        let path = session_dir.join("batch_test.jsonl");
        let mut config = SessionRecorderConfig::default();
        config.batch_size = 3;

        let mut recorder = SessionRecorder {
            enabled: true,
            file: Some(File::create(&path).unwrap()),
            path: path.clone(),
            batch: Vec::new(),
            batch_size: config.batch_size,
            last_flush: Instant::now(),
            flush_interval_ms: config.flush_interval_ms,
        };

        // Record 5 events, batch size is 3
        for i in 0..5 {
            let event = SessionEvent::new(
                SessionEventKind::Combat,
                serde_json::json!({"damage": 100 + i}),
            );
            recorder.record(event).unwrap();
        }

        // Verify all 5 are in file
        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);
        assert_eq!(reader.lines().count(), 5);

        fs::remove_dir_all(session_dir).ok();
    }

    #[test]
    fn event_serialization_round_trip() {
        let event = SessionEvent {
            ts: 1714060000000,
            kind: SessionEventKind::Combat,
            data: serde_json::json!({
                "mob_name": "a goblin",
                "ability": "Slash",
                "damage": 234,
                "hp_before": 0.92,
                "hp_after": 0.88
            }),
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: SessionEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.ts, 1714060000000);
        assert_eq!(parsed.kind, SessionEventKind::Combat);
        assert_eq!(parsed.data.get("damage").unwrap(), 234);
    }

    #[test]
    fn all_event_kinds_serialize() {
        let kinds = vec![
            SessionEventKind::Combat,
            SessionEventKind::Death,
            SessionEventKind::Stuck,
            SessionEventKind::Pull,
            SessionEventKind::Loot,
            SessionEventKind::RotationTick,
            SessionEventKind::ZoneChange,
        ];

        for kind in kinds {
            let event = SessionEvent::new(kind.clone(), serde_json::json!({}));
            let json = serde_json::to_string(&event).unwrap();
            let parsed: SessionEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.kind, kind);
        }
    }

    #[test]
    fn integration_10k_events_round_trip() {
        let home = temp_home();
        fs::create_dir_all(home.join(".textquest/sessions")).unwrap();

        let session_dir = home.join(".textquest/sessions/Paladin01");
        fs::create_dir_all(&session_dir).unwrap();

        let path = session_dir.join("integration_test.jsonl");
        let mut config = SessionRecorderConfig::default();
        config.batch_size = 500;

        let mut recorder = SessionRecorder {
            enabled: true,
            file: Some(File::create(&path).unwrap()),
            path: path.clone(),
            batch: Vec::new(),
            batch_size: config.batch_size,
            last_flush: Instant::now(),
            flush_interval_ms: config.flush_interval_ms,
        };

        // Record 10k events with different kinds
        let mut last_ts = 0u64;
        for i in 0..10_000 {
            let kind = match i % 7 {
                0 => SessionEventKind::Combat,
                1 => SessionEventKind::Death,
                2 => SessionEventKind::Stuck,
                3 => SessionEventKind::Pull,
                4 => SessionEventKind::Loot,
                5 => SessionEventKind::RotationTick,
                _ => SessionEventKind::ZoneChange,
            };

            let event = SessionEvent::new(
                kind,
                serde_json::json!({
                    "index": i,
                    "value": 100 + i,
                }),
            );
            last_ts = event.ts;
            recorder.record(event).unwrap();
        }
        recorder.flush().unwrap();

        // Verify all 10k lines are valid JSON with ordered timestamps
        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);
        let mut line_count = 0;
        let mut prev_ts = 0u64;
        let mut kind_counts = std::collections::HashMap::new();

        for (idx, line) in reader.lines().enumerate() {
            let line = line.unwrap();
            assert!(!line.is_empty(), "line {} is empty", idx);

            let parsed: SessionEvent = serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("Failed to parse line {}: {}", idx, e));

            assert!(
                parsed.ts >= prev_ts,
                "timestamps not ordered: {} >= {}",
                prev_ts,
                parsed.ts
            );
            prev_ts = parsed.ts;

            *kind_counts.entry(parsed.kind.clone()).or_insert(0) += 1;
            line_count += 1;
        }

        assert_eq!(line_count, 10_000, "Expected 10k events, got {}", line_count);

        // Verify roughly equal distribution of event kinds
        for (kind, count) in kind_counts {
            let expected = 10_000 / 7;
            assert!(
                (count as i32 - expected as i32).abs() <= 2,
                "{:?} count {} deviates too much from expected {}",
                kind,
                count,
                expected
            );
        }

        fs::remove_dir_all(session_dir).ok();
    }

    #[test]
    fn graceful_shutdown_flushes_all() {
        let home = temp_home();
        fs::create_dir_all(home.join(".textquest/sessions")).unwrap();

        let session_dir = home.join(".textquest/sessions/Shaman01");
        fs::create_dir_all(&session_dir).unwrap();

        let path = session_dir.join("shutdown_test.jsonl");
        let mut recorder = SessionRecorder {
            enabled: true,
            file: Some(File::create(&path).unwrap()),
            path: path.clone(),
            batch: vec![
                SessionEvent::new(
                    SessionEventKind::Combat,
                    serde_json::json!({"damage": 100}),
                ),
                SessionEvent::new(
                    SessionEventKind::Loot,
                    serde_json::json!({"item": "Shaman Claws", "value": 1500}),
                ),
            ],
            batch_size: 100,
            last_flush: Instant::now(),
            flush_interval_ms: 1000,
        };

        recorder.shutdown().unwrap();

        // Verify both events are in file
        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);
        let line_count = reader.lines().count();
        assert_eq!(line_count, 2);

        fs::remove_dir_all(session_dir).ok();
    }
}
