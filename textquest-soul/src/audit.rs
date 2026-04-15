//! Soul audit logging — append-only JSONL log of key Soul Engine state changes.
//!
//! Each significant action (mood change, memory record, LLM request, event
//! processed) is serialized as a single JSON line and appended to
//! `logs/soul_audit.log`.

use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use textquest_common::types::ClientId;

/// The category of auditable Soul Engine action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditActionType {
    /// A character's mood state changed.
    MoodChange,
    /// A memory entry was written to the store.
    MemoryRecord,
    /// An LLM request was enqueued.
    LlmRequest,
    /// A game or player event was processed.
    EventProcessed,
}

/// A single audit log entry, serialized as one JSON line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Monotonically increasing sequence number within this logger instance.
    pub seq: u64,
    /// ISO-8601 timestamp (UTC).
    pub timestamp: DateTime<Utc>,
    /// The character this action belongs to.
    pub character_id: ClientId,
    /// What kind of action occurred.
    pub action_type: AuditActionType,
    /// Human-readable description of the action.
    pub description: String,
    /// State before the action (optional, action-specific JSON blob).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_state: Option<serde_json::Value>,
    /// State after the action (optional, action-specific JSON blob).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_state: Option<serde_json::Value>,
}

/// Append-only JSONL audit logger for the Soul Engine.
///
/// Thread-safe via an internal `Mutex`; safe to share across the coordinator.
pub struct SoulAuditLogger {
    log_path: PathBuf,
    inner: Mutex<AuditLoggerInner>,
}

struct AuditLoggerInner {
    file: File,
    seq: u64,
}

impl SoulAuditLogger {
    /// Open (or create) the audit log at `log_path`.
    ///
    /// The parent directory must already exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened/created.
    pub fn open(log_path: impl AsRef<Path>) -> Result<Self> {
        let log_path = log_path.as_ref().to_path_buf();

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        Ok(Self {
            log_path,
            inner: Mutex::new(AuditLoggerInner { file, seq: 0 }),
        })
    }

    /// Append an audit entry to the log.
    ///
    /// Serializes the entry as a single JSON line followed by a newline.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or the file write fails.
    pub fn log(
        &self,
        character_id: ClientId,
        action_type: AuditActionType,
        description: impl Into<String>,
        before_state: Option<serde_json::Value>,
        after_state: Option<serde_json::Value>,
    ) -> Result<()> {
        let mut inner = self.inner.lock().expect("audit logger mutex poisoned");
        inner.seq += 1;

        let entry = AuditEntry {
            seq: inner.seq,
            timestamp: Utc::now(),
            character_id,
            action_type,
            description: description.into(),
            before_state,
            after_state,
        };

        let mut line = serde_json::to_string(&entry)?;
        line.push('\n');
        inner.file.write_all(line.as_bytes())?;
        inner.file.flush()?;

        tracing::debug!(
            seq = entry.seq,
            character_id,
            action = ?entry.action_type,
            description = %entry.description,
            "soul audit entry written"
        );

        Ok(())
    }

    /// Return the path of the log file this logger writes to.
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    /// Read all entries from the audit log that match `character_id`.
    ///
    /// Parses every line in the JSONL file; lines that fail to parse are
    /// skipped with a `tracing::warn`. Useful for tests and diagnostics.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened for reading.
    pub fn entries_for(&self, character_id: ClientId) -> Result<Vec<AuditEntry>> {
        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);
        let mut results = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<AuditEntry>(&line) {
                Ok(entry) if entry.character_id == character_id => results.push(entry),
                Ok(_) => {} // different character, skip
                Err(e) => {
                    tracing::warn!(error = %e, "failed to parse audit log line, skipping");
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_logger() -> (SoulAuditLogger, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("soul_audit.log");
        let logger = SoulAuditLogger::open(&path).unwrap();
        (logger, dir)
    }

    // -------------------------------------------------------------------------
    // Entry creation
    // -------------------------------------------------------------------------

    #[test]
    fn log_entry_writes_to_file() {
        let (logger, _dir) = make_logger();
        logger
            .log(1, AuditActionType::MoodChange, "mood changed", None, None)
            .unwrap();

        let entries = logger.entries_for(1).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].character_id, 1);
        assert_eq!(entries[0].action_type, AuditActionType::MoodChange);
        assert_eq!(entries[0].description, "mood changed");
    }

    #[test]
    fn log_entry_seq_increments() {
        let (logger, _dir) = make_logger();
        logger
            .log(1, AuditActionType::MemoryRecord, "first", None, None)
            .unwrap();
        logger
            .log(1, AuditActionType::LlmRequest, "second", None, None)
            .unwrap();
        logger
            .log(1, AuditActionType::EventProcessed, "third", None, None)
            .unwrap();

        let entries = logger.entries_for(1).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].seq, 1);
        assert_eq!(entries[1].seq, 2);
        assert_eq!(entries[2].seq, 3);
    }

    #[test]
    fn log_entry_captures_before_after_state() {
        let (logger, _dir) = make_logger();
        let before = json!({"mood": "Neutral"});
        let after = json!({"mood": "Excited"});

        logger
            .log(
                2,
                AuditActionType::MoodChange,
                "mood shift",
                Some(before.clone()),
                Some(after.clone()),
            )
            .unwrap();

        let entries = logger.entries_for(2).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].before_state, Some(before));
        assert_eq!(entries[0].after_state, Some(after));
    }

    // -------------------------------------------------------------------------
    // Serialization
    // -------------------------------------------------------------------------

    #[test]
    fn entry_round_trips_through_json() {
        let entry = AuditEntry {
            seq: 42,
            timestamp: Utc::now(),
            character_id: 7,
            action_type: AuditActionType::LlmRequest,
            description: "enqueued LLM request".into(),
            before_state: None,
            after_state: Some(json!({"queued": true})),
        };

        let serialized = serde_json::to_string(&entry).unwrap();
        let deserialized: AuditEntry = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.seq, 42);
        assert_eq!(deserialized.character_id, 7);
        assert_eq!(deserialized.action_type, AuditActionType::LlmRequest);
        assert_eq!(deserialized.description, "enqueued LLM request");
        assert!(deserialized.before_state.is_none());
        assert_eq!(deserialized.after_state, Some(json!({"queued": true})));
    }

    #[test]
    fn action_type_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&AuditActionType::MoodChange).unwrap(),
            "\"mood_change\""
        );
        assert_eq!(
            serde_json::to_string(&AuditActionType::MemoryRecord).unwrap(),
            "\"memory_record\""
        );
        assert_eq!(
            serde_json::to_string(&AuditActionType::LlmRequest).unwrap(),
            "\"llm_request\""
        );
        assert_eq!(
            serde_json::to_string(&AuditActionType::EventProcessed).unwrap(),
            "\"event_processed\""
        );
    }

    #[test]
    fn jsonl_file_is_one_entry_per_line() {
        let (logger, _dir) = make_logger();
        logger
            .log(1, AuditActionType::MoodChange, "a", None, None)
            .unwrap();
        logger
            .log(2, AuditActionType::MemoryRecord, "b", None, None)
            .unwrap();

        let content = std::fs::read_to_string(logger.log_path()).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        // Each line must be valid JSON
        for line in lines {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(v.is_object());
        }
    }

    // -------------------------------------------------------------------------
    // Character filter query
    // -------------------------------------------------------------------------

    #[test]
    fn entries_for_filters_by_character_id() {
        let (logger, _dir) = make_logger();

        // Log entries for three different characters
        logger
            .log(1, AuditActionType::MoodChange, "char 1 mood", None, None)
            .unwrap();
        logger
            .log(2, AuditActionType::MemoryRecord, "char 2 mem", None, None)
            .unwrap();
        logger
            .log(
                1,
                AuditActionType::EventProcessed,
                "char 1 event",
                None,
                None,
            )
            .unwrap();
        logger
            .log(3, AuditActionType::LlmRequest, "char 3 llm", None, None)
            .unwrap();

        let char1 = logger.entries_for(1).unwrap();
        let char2 = logger.entries_for(2).unwrap();
        let char3 = logger.entries_for(3).unwrap();
        let char99 = logger.entries_for(99).unwrap();

        assert_eq!(char1.len(), 2);
        assert_eq!(char2.len(), 1);
        assert_eq!(char3.len(), 1);
        assert!(char99.is_empty());

        // Verify the right entries came back for char 1
        assert_eq!(char1[0].action_type, AuditActionType::MoodChange);
        assert_eq!(char1[1].action_type, AuditActionType::EventProcessed);
    }

    #[test]
    fn entries_for_returns_empty_on_missing_file_entries() {
        let (logger, _dir) = make_logger();
        // No entries written yet
        let entries = logger.entries_for(42).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn open_creates_file_if_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new_audit.log");
        assert!(!path.exists());
        let _logger = SoulAuditLogger::open(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn open_appends_to_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("soul_audit.log");

        {
            let logger = SoulAuditLogger::open(&path).unwrap();
            logger
                .log(1, AuditActionType::MoodChange, "first session", None, None)
                .unwrap();
        }

        // Re-open — should append, not truncate
        {
            let logger = SoulAuditLogger::open(&path).unwrap();
            logger
                .log(
                    1,
                    AuditActionType::MemoryRecord,
                    "second session",
                    None,
                    None,
                )
                .unwrap();

            let entries = logger.entries_for(1).unwrap();
            assert_eq!(entries.len(), 2);
        }
    }
}
