//! Session directory management for test output.
//!
//! Provides `SessionManager` for creating timestamped session directories
//! and `SessionMetadata` for recording session configuration.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use super::event_log::{EventType, TestEvent};

/// Metadata written to `metadata.json` in the session directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Unique session identifier (short UUID-like string).
    pub session_id: String,
    /// UTC timestamp when the session was created.
    pub start_time: DateTime<Utc>,
    /// Account names participating in this session.
    pub accounts: Vec<String>,
    /// Scenario names being exercised.
    pub scenarios: Vec<String>,
    /// Target duration in seconds, if set.
    pub target_duration: Option<u64>,
    /// Hash of the configuration used for reproducibility.
    pub config_hash: String,
}

/// Manages creation and lifecycle of a timestamped session directory.
///
/// # Session directory format
///
/// `{base_dir}/session-{YYYY-MM-DD-HHMMSS}-{short_id}/`
pub struct SessionManager {
    /// Root path of this session.
    pub session_dir: PathBuf,
    /// Metadata written to `metadata.json`.
    pub metadata: SessionMetadata,
}

impl SessionManager {
    /// Create a new session directory under `base_dir`.
    ///
    /// The directory is created immediately and `metadata.json` is written.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created or the metadata file
    /// cannot be written.
    pub fn new(
        base_dir: impl AsRef<Path>,
        accounts: Vec<String>,
        scenarios: Vec<String>,
    ) -> Result<Self> {
        let base_dir = base_dir.as_ref();
        std::fs::create_dir_all(base_dir)
            .with_context(|| format!("failed to create base session dir {base_dir:?}"))?;

        for _ in 0..8 {
            let now = Utc::now();
            let short_id = Self::short_id(&now);
            let dir_name = format!("session-{}-{}", now.format("%Y-%m-%d-%H%M%S"), short_id);
            let session_dir = base_dir.join(&dir_name);

            match std::fs::create_dir(&session_dir) {
                Ok(()) => {
                    let metadata = SessionMetadata {
                        session_id: short_id,
                        start_time: now,
                        accounts: accounts.clone(),
                        scenarios: scenarios.clone(),
                        target_duration: None,
                        config_hash: String::new(),
                    };

                    let meta_path = session_dir.join("metadata.json");
                    let json = serde_json::to_string_pretty(&metadata)
                        .context("failed to serialize session metadata")?;
                    std::fs::write(&meta_path, json)
                        .with_context(|| format!("failed to write metadata to {meta_path:?}"))?;

                    return Ok(Self {
                        session_dir,
                        metadata,
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(err)
                        .with_context(|| format!("failed to create session dir {session_dir:?}"));
                }
            }
        }

        Err(anyhow::anyhow!(
            "failed to create a unique session directory after multiple attempts"
        ))
    }

    /// Set the target duration (seconds) and rewrite `metadata.json`.
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata file cannot be written.
    pub fn set_target_duration(&mut self, secs: u64) -> Result<()> {
        self.metadata.target_duration = Some(secs);
        self.write_metadata()
    }

    /// Set the config hash and rewrite `metadata.json`.
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata file cannot be written.
    pub fn set_config_hash(&mut self, hash: impl Into<String>) -> Result<()> {
        self.metadata.config_hash = hash.into();
        self.write_metadata()
    }

    /// Return the path to `metadata.json`.
    pub fn metadata_path(&self) -> PathBuf {
        self.session_dir.join("metadata.json")
    }

    /// Export a full test session as one compact JSON document.
    ///
    /// The export is intended for external tools such as `jq`, Python
    /// `json.load`, or analytics notebooks. It includes raw `metadata.json`,
    /// all `events.jsonl` entries in order, derived aggregate metrics, and a
    /// short summary suitable for dashboards.
    ///
    /// # Errors
    ///
    /// Returns an error if `metadata.json` cannot be read or parsed, if
    /// `events.jsonl` contains invalid JSON, or if the export cannot be
    /// serialized.
    pub fn export_session_json(session_dir: &Path) -> Result<String> {
        let metadata = read_metadata_json(session_dir)?;
        let events = read_event_log(session_dir)?;
        let metrics = aggregate_metrics(&metadata, &events);
        let summary = serde_json::json!({
            "total_iterations": metrics["total_iterations"],
            "success_rate": metrics["success_rate"],
            "total_duration_seconds": metrics["total_duration_seconds"],
        });

        let raw_events: Vec<Value> = events.into_iter().map(|event| event.raw).collect();
        let export = serde_json::json!({
            "metadata": metadata,
            "events": raw_events,
            "metrics": metrics,
            "summary": summary,
        });

        serde_json::to_string(&export).context("failed to serialize session export")
    }

    /// Rewrite `metadata.json` in place.
    fn write_metadata(&self) -> Result<()> {
        let meta_path = self.metadata_path();
        let json = serde_json::to_string_pretty(&self.metadata)
            .context("failed to serialize session metadata")?;
        std::fs::write(&meta_path, json)
            .with_context(|| format!("failed to write metadata to {meta_path:?}"))?;
        Ok(())
    }

    /// Generate a short identifier derived from the nanosecond component of `now`.
    fn short_id(now: &DateTime<Utc>) -> String {
        format!("{:06x}", now.timestamp_subsec_nanos() % 0x100_0000)
    }
}

/// Export a full test session as one compact JSON document.
///
/// This free function mirrors [`SessionManager::export_session_json`] for
/// callers that do not already have a `SessionManager` in scope.
pub fn export_session_json(session_dir: &Path) -> Result<String> {
    SessionManager::export_session_json(session_dir)
}

#[derive(Debug)]
struct ExportEvent {
    raw: Value,
    typed: TestEvent,
}

fn read_metadata_json(session_dir: &Path) -> Result<Value> {
    let path = session_dir.join("metadata.json");
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read metadata from {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse metadata JSON from {}", path.display()))
}

fn read_event_log(session_dir: &Path) -> Result<Vec<ExportEvent>> {
    let path = session_dir.join("events.jsonl");
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = File::open(&path).with_context(|| format!("failed to open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for (idx, line) in reader.lines().enumerate() {
        let line_number = idx + 1;
        let line = line.with_context(|| {
            format!("failed to read line {line_number} from {}", path.display())
        })?;
        if line.trim().is_empty() {
            continue;
        }

        let raw: Value = serde_json::from_str(&line).with_context(|| {
            format!(
                "failed to parse event JSON on line {line_number} from {}",
                path.display()
            )
        })?;
        let typed: TestEvent = serde_json::from_value(raw.clone()).with_context(|| {
            format!(
                "failed to decode event on line {line_number} from {}",
                path.display()
            )
        })?;
        events.push(ExportEvent { raw, typed });
    }

    Ok(events)
}

fn aggregate_metrics(metadata: &Value, events: &[ExportEvent]) -> Value {
    let mut events_by_type: BTreeMap<&'static str, u64> = BTreeMap::new();
    let mut iterations_observed = BTreeSet::new();
    let mut error_iterations = BTreeSet::new();
    let mut scenario_complete_count = 0_u64;
    let mut scenario_success_count = 0_u64;
    let mut error_count = 0_u64;
    let mut first_event_time: Option<DateTime<Utc>> = None;
    let mut last_event_time: Option<DateTime<Utc>> = None;

    for event in events {
        let event_type = event_type_name(&event.typed.event_type);
        *events_by_type.entry(event_type).or_insert(0) += 1;
        iterations_observed.insert(event.typed.iteration);

        first_event_time = Some(match first_event_time {
            Some(existing) => existing.min(event.typed.timestamp),
            None => event.typed.timestamp,
        });
        last_event_time = Some(match last_event_time {
            Some(existing) => existing.max(event.typed.timestamp),
            None => event.typed.timestamp,
        });

        match event.typed.event_type {
            EventType::ScenarioComplete => {
                scenario_complete_count += 1;
                if event
                    .typed
                    .details
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(true)
                {
                    scenario_success_count += 1;
                }
            }
            EventType::Error => {
                error_count += 1;
                error_iterations.insert(event.typed.iteration);
            }
            EventType::LoginStart
            | EventType::LoginComplete
            | EventType::ScenarioStart
            | EventType::ScenarioPull
            | EventType::ScenarioMobKill
            | EventType::LogoutStart
            | EventType::LogoutComplete => {}
        }
    }

    let fallback_iterations = iterations_observed.len() as u64;
    let total_iterations = if scenario_complete_count > 0 {
        scenario_complete_count
    } else {
        fallback_iterations
    };
    let successful_iterations = if scenario_complete_count > 0 {
        scenario_success_count
    } else {
        fallback_iterations.saturating_sub(error_iterations.len() as u64)
    };
    let success_rate = if total_iterations == 0 {
        0.0
    } else {
        successful_iterations as f64 / total_iterations as f64
    };
    let total_duration_seconds =
        session_duration_seconds(metadata, first_event_time, last_event_time);

    let mut counts = Map::new();
    for (event_type, count) in events_by_type {
        counts.insert(event_type.to_string(), Value::from(count));
    }

    serde_json::json!({
        "total_events": events.len() as u64,
        "events_by_type": Value::Object(counts),
        "total_iterations": total_iterations,
        "successful_iterations": successful_iterations,
        "failed_iterations": total_iterations.saturating_sub(successful_iterations),
        "success_rate": success_rate,
        "error_count": error_count,
        "total_duration_seconds": total_duration_seconds,
        "first_event_time": first_event_time.map(|ts| ts.to_rfc3339()),
        "last_event_time": last_event_time.map(|ts| ts.to_rfc3339()),
    })
}

fn session_duration_seconds(
    metadata: &Value,
    first_event_time: Option<DateTime<Utc>>,
    last_event_time: Option<DateTime<Utc>>,
) -> i64 {
    let start_time = metadata
        .get("start_time")
        .and_then(Value::as_str)
        .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
        .map(|ts| ts.with_timezone(&Utc))
        .or(first_event_time);

    match (start_time, last_event_time) {
        (Some(start), Some(end)) => (end - start).num_seconds().max(0),
        _ => 0,
    }
}

fn event_type_name(event_type: &EventType) -> &'static str {
    match event_type {
        EventType::LoginStart => "login_start",
        EventType::LoginComplete => "login_complete",
        EventType::ScenarioStart => "scenario_start",
        EventType::ScenarioPull => "scenario_pull",
        EventType::ScenarioMobKill => "scenario_mob_kill",
        EventType::ScenarioComplete => "scenario_complete",
        EventType::LogoutStart => "logout_start",
        EventType::LogoutComplete => "logout_complete",
        EventType::Error => "error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::fs;
    use tempfile::TempDir;

    fn tmp() -> TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn event_at(
        session_id: &str,
        iteration: u32,
        event_type: EventType,
        offset_secs: i64,
        details: Value,
    ) -> TestEvent {
        TestEvent {
            timestamp: Utc
                .with_ymd_and_hms(2026, 4, 24, 12, 0, 0)
                .single()
                .expect("valid timestamp")
                + chrono::Duration::seconds(offset_secs),
            session_id: session_id.to_string(),
            iteration,
            event_type,
            details,
        }
    }

    fn write_events(session_dir: &Path, events: &[TestEvent]) {
        let body = events
            .iter()
            .map(|event| serde_json::to_string(event).expect("serialize event"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(session_dir.join("events.jsonl"), format!("{body}\n")).expect("write events");
    }

    #[test]
    fn test_session_dir_created() {
        let dir = tmp();
        let sm = SessionManager::new(
            dir.path(),
            vec!["account1".into()],
            vec!["scenario_a".into()],
        )
        .expect("SessionManager::new");
        assert!(sm.session_dir.exists(), "session dir must be created");
    }

    #[test]
    fn test_metadata_json_written() {
        let dir = tmp();
        let sm = SessionManager::new(
            dir.path(),
            vec!["acct1".into(), "acct2".into()],
            vec!["s1".into()],
        )
        .expect("SessionManager::new");
        let meta_path = sm.metadata_path();
        assert!(meta_path.exists(), "metadata.json must exist");
        let raw = fs::read_to_string(&meta_path).expect("read metadata.json");
        let meta: SessionMetadata = serde_json::from_str(&raw).expect("parse metadata.json");
        assert_eq!(meta.accounts, vec!["acct1", "acct2"]);
        assert_eq!(meta.scenarios, vec!["s1"]);
    }

    #[test]
    fn test_session_dir_format() {
        let dir = tmp();
        let sm = SessionManager::new(dir.path(), vec![], vec![]).expect("SessionManager::new");
        let name = sm
            .session_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        // format: session-YYYY-MM-DD-HHMMSS-XXXXXX
        assert!(
            name.starts_with("session-"),
            "dir name must start with 'session-': {name}"
        );
        let parts: Vec<&str> = name.split('-').collect();
        // parts: ["session", YYYY, MM, DD, HHMMSS, short_id]
        assert_eq!(parts.len(), 6, "unexpected dir name format: {name}");
    }

    #[test]
    fn test_set_target_duration() {
        let dir = tmp();
        let mut sm = SessionManager::new(dir.path(), vec![], vec![]).expect("SessionManager::new");
        sm.set_target_duration(300).expect("set_target_duration");
        let raw = fs::read_to_string(sm.metadata_path()).expect("read metadata.json");
        let meta: SessionMetadata = serde_json::from_str(&raw).expect("parse");
        assert_eq!(meta.target_duration, Some(300));
    }

    #[test]
    fn test_set_config_hash() {
        let dir = tmp();
        let mut sm = SessionManager::new(dir.path(), vec![], vec![]).expect("SessionManager::new");
        sm.set_config_hash("abc123").expect("set_config_hash");
        let raw = fs::read_to_string(sm.metadata_path()).expect("read metadata.json");
        let meta: SessionMetadata = serde_json::from_str(&raw).expect("parse");
        assert_eq!(meta.config_hash, "abc123");
    }

    #[test]
    fn test_export_session_json_includes_metadata_events_metrics_and_summary() {
        let dir = tmp();
        let mut sm =
            SessionManager::new(dir.path(), vec!["acct1".into()], vec!["scenario_a".into()])
                .expect("SessionManager::new");
        sm.set_config_hash("abc123").expect("set_config_hash");

        write_events(
            &sm.session_dir,
            &[
                event_at(
                    &sm.metadata.session_id,
                    1,
                    EventType::ScenarioStart,
                    10,
                    serde_json::json!({"scenario": "scenario_a"}),
                ),
                event_at(
                    &sm.metadata.session_id,
                    1,
                    EventType::ScenarioComplete,
                    40,
                    serde_json::json!({"success": true}),
                ),
                event_at(
                    &sm.metadata.session_id,
                    2,
                    EventType::ScenarioComplete,
                    70,
                    serde_json::json!({"success": false}),
                ),
                event_at(
                    &sm.metadata.session_id,
                    2,
                    EventType::Error,
                    75,
                    serde_json::json!({"message": "failed pull"}),
                ),
            ],
        );

        let exported =
            SessionManager::export_session_json(&sm.session_dir).expect("export session JSON");
        let parsed: Value = serde_json::from_str(&exported).expect("export must be valid JSON");

        assert_eq!(parsed["metadata"]["config_hash"], "abc123");
        assert_eq!(parsed["events"].as_array().expect("events array").len(), 4);
        assert_eq!(parsed["metrics"]["total_events"], 4);
        assert_eq!(parsed["metrics"]["events_by_type"]["scenario_complete"], 2);
        assert_eq!(parsed["metrics"]["events_by_type"]["error"], 1);
        assert_eq!(parsed["metrics"]["successful_iterations"], 1);
        assert_eq!(parsed["summary"]["total_iterations"], 2);
        assert_eq!(parsed["summary"]["success_rate"], 0.5);
        assert!(
            parsed["summary"]["total_duration_seconds"]
                .as_i64()
                .expect("duration seconds")
                >= 0
        );
    }

    #[test]
    fn test_export_session_json_is_valid_with_no_event_log() {
        let dir = tmp();
        let sm = SessionManager::new(dir.path(), vec![], vec![]).expect("SessionManager::new");

        let exported = export_session_json(&sm.session_dir).expect("export empty session");
        let parsed: Value = serde_json::from_str(&exported).expect("valid JSON");

        assert_eq!(parsed["events"].as_array().expect("events array").len(), 0);
        assert_eq!(parsed["metrics"]["total_events"], 0);
        assert_eq!(parsed["summary"]["total_iterations"], 0);
    }

    #[test]
    fn test_export_session_json_handles_large_event_counts_compactly() {
        let dir = tmp();
        let sm = SessionManager::new(dir.path(), vec![], vec![]).expect("SessionManager::new");
        let events: Vec<TestEvent> = (0..5_000)
            .map(|idx| {
                event_at(
                    &sm.metadata.session_id,
                    idx,
                    EventType::ScenarioComplete,
                    i64::from(idx),
                    serde_json::json!({"success": idx % 10 != 0}),
                )
            })
            .collect();
        write_events(&sm.session_dir, &events);

        let exported =
            SessionManager::export_session_json(&sm.session_dir).expect("export large session");
        let parsed: Value = serde_json::from_str(&exported).expect("valid JSON");

        assert_eq!(
            parsed["events"].as_array().expect("events array").len(),
            5_000
        );
        assert_eq!(parsed["summary"]["total_iterations"], 5_000);
        assert_eq!(parsed["metrics"]["failed_iterations"], 500);
        assert!(exported.len() < 100 * 1024 * 1024);
    }
}
