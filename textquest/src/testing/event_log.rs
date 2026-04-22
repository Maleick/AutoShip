//! Append-only structured JSON event logging for test sessions.
//!
//! [`EventLog`] writes one JSON object per line (JSONL) to a file, capturing
//! all test lifecycle events with timestamps and session identifiers.  The log
//! is wrapped in `Arc<Mutex<>>` so multiple tokio tasks can emit events safely.

use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── EventType ─────────────────────────────────────────────────────────────────

/// All discrete event types emitted during a test run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    LoginStart,
    LoginComplete,
    ScenarioStart,
    ScenarioPull,
    ScenarioMobKill,
    ScenarioComplete,
    LogoutStart,
    LogoutComplete,
    Error,
}

// ── TestEvent ─────────────────────────────────────────────────────────────────

/// A single structured event written to the JSONL log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestEvent {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub iteration: u32,
    pub event_type: EventType,
    pub details: Value,
}

impl TestEvent {
    /// Convenience constructor — fills `timestamp` automatically.
    pub fn new(
        session_id: impl Into<String>,
        iteration: u32,
        event_type: EventType,
        details: Value,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id: session_id.into(),
            iteration,
            event_type,
            details,
        }
    }
}

// ── EventLog ──────────────────────────────────────────────────────────────────

/// Append-only JSONL event log.
///
/// Wrap in `Arc<Mutex<EventLog>>` for concurrent access across tokio tasks.
pub struct EventLog {
    file: BufWriter<File>,
    session_id: String,
}

impl EventLog {
    /// Open (or create) `path` for append-only writing.
    pub fn new(path: &Path, session_id: &str) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create log directory '{}'", parent.display()))?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("open event log '{}'", path.display()))?;
        Ok(Self {
            file: BufWriter::new(file),
            session_id: session_id.to_owned(),
        })
    }

    /// Write one event as a JSON line.  Flushes immediately to minimize data
    /// loss on crash.
    pub fn log_event(&mut self, event: TestEvent) -> Result<()> {
        let line = serde_json::to_string(&event).context("serialize event")?;
        self.file
            .write_all(line.as_bytes())
            .context("write event line")?;
        self.file.write_all(b"\n").context("write newline")?;
        self.file.flush().context("flush event log")?;
        Ok(())
    }

    /// Accessor used by tests to verify session identity.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

// ── SharedEventLog ────────────────────────────────────────────────────────────

/// Thread-safe handle to an [`EventLog`].
pub type SharedEventLog = Arc<Mutex<EventLog>>;

/// Wrap a freshly opened [`EventLog`] in an `Arc<Mutex<>>`.
pub fn open_shared(path: &Path, session_id: &str) -> Result<SharedEventLog> {
    Ok(Arc::new(Mutex::new(EventLog::new(path, session_id)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use tempfile::tempdir;

    fn make_event(session: &str, iter: u32, et: EventType) -> TestEvent {
        TestEvent::new(session, iter, et, serde_json::json!({"account": "test01"}))
    }

    #[test]
    fn event_serialization_round_trips() {
        let ev = make_event("sess-abc", 1, EventType::LoginStart);
        let json = serde_json::to_string(&ev).unwrap();
        let back: TestEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, "sess-abc");
        assert_eq!(back.iteration, 1);
        assert_eq!(back.event_type, EventType::LoginStart);
    }

    #[test]
    fn all_event_types_serialize() {
        let types = [
            EventType::LoginStart,
            EventType::LoginComplete,
            EventType::ScenarioStart,
            EventType::ScenarioPull,
            EventType::ScenarioMobKill,
            EventType::ScenarioComplete,
            EventType::LogoutStart,
            EventType::LogoutComplete,
            EventType::Error,
        ];
        for et in &types {
            let ev = make_event("s", 0, et.clone());
            let json = serde_json::to_string(&ev).expect("serialize");
            // Must be a valid JSON line (no embedded newlines)
            assert!(!json.contains('\n'), "unexpected newline in {json}");
            let back: TestEvent = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(&back.event_type, et);
        }
    }

    #[test]
    fn event_log_writes_valid_jsonl() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");

        let mut log = EventLog::new(&path, "test-session").unwrap();
        assert_eq!(log.session_id(), "test-session");

        log.log_event(make_event("test-session", 0, EventType::LoginStart))
            .unwrap();
        log.log_event(make_event("test-session", 0, EventType::LoginComplete))
            .unwrap();
        log.log_event(make_event("test-session", 1, EventType::ScenarioStart))
            .unwrap();

        drop(log);

        let mut content = String::new();
        File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();

        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3, "expected 3 JSONL lines, got {}", lines.len());
        for line in &lines {
            serde_json::from_str::<TestEvent>(line).expect("each line must be valid JSON");
        }

        // Session ID present in all lines
        assert!(lines.iter().all(|l| l.contains("test-session")));
    }

    #[test]
    fn event_log_appends_across_opens() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");

        {
            let mut log = EventLog::new(&path, "s1").unwrap();
            log.log_event(make_event("s1", 0, EventType::LoginStart))
                .unwrap();
        }
        {
            let mut log = EventLog::new(&path, "s1").unwrap();
            log.log_event(make_event("s1", 1, EventType::LoginComplete))
                .unwrap();
        }

        let mut content = String::new();
        File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content.lines().count(), 2, "should have 2 lines after two opens");
    }

    #[test]
    fn concurrent_writes_no_data_loss() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("concurrent.jsonl");
        let shared = open_shared(&path, "concurrent").unwrap();

        let mut handles = vec![];
        for i in 0u32..20 {
            let log = Arc::clone(&shared);
            let h = std::thread::spawn(move || {
                let ev = make_event("concurrent", i, EventType::ScenarioMobKill);
                log.lock().unwrap().log_event(ev).unwrap();
            });
            handles.push(h);
        }
        for h in handles {
            h.join().unwrap();
        }

        let mut content = String::new();
        File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 20, "expected 20 lines, got {}", lines.len());
        for line in &lines {
            serde_json::from_str::<TestEvent>(line).expect("valid JSON");
        }
    }

    #[test]
    fn large_event_volume() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("large.jsonl");
        let mut log = EventLog::new(&path, "large").unwrap();

        for i in 0u32..1_000 {
            log.log_event(make_event("large", i, EventType::ScenarioMobKill))
                .unwrap();
        }
        drop(log);

        let mut content = String::new();
        File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content.lines().count(), 1_000);
    }

    #[test]
    fn creates_parent_directory_if_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("deep").join("events.jsonl");
        let mut log = EventLog::new(&path, "nested-test").unwrap();
        log.log_event(make_event("nested-test", 0, EventType::LogoutComplete))
            .unwrap();
        assert!(path.exists());
    }
}
