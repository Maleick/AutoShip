use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use uuid::Uuid;
use zstd::stream::write::Encoder as ZstdEncoder;

pub const SESSION_STREAM_VERSION: u32 = 2;
const OPERATOR_STREAM_FILE: &str = "operator.ndjson.zst";
const ORCHESTRATOR_STREAM_FILE: &str = "orchestrator.ndjson.zst";
const OPERATOR_CAPTURE_LAYER: &str = "input_layer";
const REDACTED_BOOKMARK: &str = "[redacted]";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperatorStreamHeader {
    pub version: u32,
    pub session_id: String,
    pub captured_at: String,
    pub privacy_policy: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrchestratorStreamHeader {
    pub version: u32,
    pub session_id: String,
    pub policy_sha: String,
    pub config_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OperatorEvent {
    pub elapsed_secs: f64,
    #[serde(flatten)]
    pub kind: OperatorEventKind,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperatorEventKind {
    Keypress {
        input: String,
        key: String,
    },
    Command {
        command: String,
    },
    Mode {
        mode: String,
    },
    Bookmark {
        note: String,
        note_hash: String,
        redacted: bool,
    },
}

impl OperatorEvent {
    fn keypress(elapsed_secs: f64, key: &KeyEvent) -> Self {
        Self {
            elapsed_secs,
            kind: OperatorEventKind::Keypress {
                input: key_input_label(key),
                key: key_code_label(key.code, key.modifiers),
            },
        }
    }

    fn command(elapsed_secs: f64, command: impl Into<String>) -> Self {
        Self {
            elapsed_secs,
            kind: OperatorEventKind::Command {
                command: command.into(),
            },
        }
    }

    fn mode(elapsed_secs: f64, mode: impl Into<String>) -> Self {
        Self {
            elapsed_secs,
            kind: OperatorEventKind::Mode { mode: mode.into() },
        }
    }

    fn bookmark(elapsed_secs: f64, note: impl AsRef<str>) -> Self {
        let note = note.as_ref();
        let note_hash = sha256_hex(note.as_bytes());
        let note = if bookmark_text_redaction_enabled() {
            REDACTED_BOOKMARK.to_string()
        } else {
            note.to_owned()
        };

        Self {
            elapsed_secs,
            kind: OperatorEventKind::Bookmark {
                note,
                note_hash,
                redacted: bookmark_text_redaction_enabled(),
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DecisionEvent {
    pub elapsed_secs: f64,
    pub actor: String,
    pub kind: String,
    pub inputs_hash: String,
    pub inputs: Value,
    #[serde(flatten)]
    pub details: BTreeMap<String, Value>,
}

impl DecisionEvent {
    pub fn new(
        elapsed_secs: f64,
        actor: impl Into<String>,
        kind: impl Into<String>,
        inputs_hash: impl Into<String>,
        inputs: Value,
    ) -> Self {
        Self {
            elapsed_secs,
            actor: actor.into(),
            kind: kind.into(),
            inputs_hash: inputs_hash.into(),
            inputs,
            details: BTreeMap::new(),
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        let value = serde_json::to_value(value).unwrap_or(Value::Null);
        self.details.insert(key.into(), value);
        self
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct AttributionMetrics {
    pub decisions_per_minute_operator: f64,
    pub decisions_per_minute_orchestrator: f64,
    pub lead_actor: LeadActor,
    pub operator_overrides: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LeadActor {
    Operator,
    #[default]
    Mixed,
    Orchestrator,
}

impl LeadActor {
    pub fn classify(operator_events: u64, orchestrator_decisions: u64) -> Self {
        match (operator_events, orchestrator_decisions) {
            (0, 0) => Self::Mixed,
            (op, 0) if op > 0 => Self::Operator,
            (0, orch) if orch > 0 => Self::Orchestrator,
            (op, orch) if op > orch => Self::Operator,
            (op, orch) if orch > op => Self::Orchestrator,
            _ => Self::Mixed,
        }
    }
}

struct ReplayStreamWriter {
    writer: Box<dyn Write + Send>,
}

impl std::fmt::Debug for ReplayStreamWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReplayStreamWriter").finish_non_exhaustive()
    }
}

impl ReplayStreamWriter {
    fn new(path: PathBuf, header: impl Serialize) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)?;
        let writer = BufWriter::new(file);
        let encoder = ZstdEncoder::new(writer, 0)?;
        let mut writer: Box<dyn Write + Send> = Box::new(encoder.auto_finish());
        write_json_line(&mut writer, &header)?;
        Ok(Self { writer })
    }

    fn write_event<T: Serialize>(&mut self, event: &T) -> std::io::Result<()> {
        write_json_line(&mut self.writer, event)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

#[derive(Debug)]
struct ReplayState {
    started_at: Instant,
    session_id: String,
    operator_writer: Option<ReplayStreamWriter>,
    orchestrator_writer: Option<ReplayStreamWriter>,
    orchestrator_config_hash: String,
    operator_event_count: u64,
    orchestrator_decision_count: u64,
    operator_override_count: u64,
    inputs_by_hash: HashMap<String, Value>,
}

impl Default for ReplayState {
    fn default() -> Self {
        Self {
            started_at: Instant::now(),
            session_id: Uuid::new_v4().simple().to_string(),
            operator_writer: None,
            orchestrator_writer: None,
            orchestrator_config_hash: String::from("unknown"),
            operator_event_count: 0,
            orchestrator_decision_count: 0,
            operator_override_count: 0,
            inputs_by_hash: HashMap::new(),
        }
    }
}

static REPLAY_STATE: OnceLock<Mutex<ReplayState>> = OnceLock::new();

fn state() -> &'static Mutex<ReplayState> {
    REPLAY_STATE.get_or_init(|| Mutex::new(ReplayState::default()))
}

pub fn session_id() -> String {
    state().lock().expect("session replay state").session_id.clone()
}

pub fn policy_sha() -> String {
    option_env!("TEXTQUEST_POLICY_SHA")
        .map(str::to_owned)
        .unwrap_or_else(|| String::from("unknown"))
}

pub fn set_orchestrator_config_hash(hash: impl Into<String>) {
    let mut guard = state().lock().expect("session replay state");
    guard.orchestrator_config_hash = hash.into();
}

pub fn record_operator_keypress(key: KeyEvent) {
    let mut guard = state().lock().expect("session replay state");
    let event = OperatorEvent::keypress(guard.started_at.elapsed().as_secs_f64(), &key);
    guard.operator_event_count = guard.operator_event_count.saturating_add(1);
    if let Some(writer) = ensure_operator_writer(&mut guard) {
        let _ = writer.write_event(&event);
        let _ = writer.flush();
    }
}

pub fn record_operator_command(command: impl AsRef<str>) {
    let command = command.as_ref().trim();
    if command.is_empty() {
        return;
    }

    let mut guard = state().lock().expect("session replay state");
    let event = OperatorEvent::command(guard.started_at.elapsed().as_secs_f64(), command);
    guard.operator_event_count = guard.operator_event_count.saturating_add(1);
    if is_override_candidate(command) {
        guard.operator_override_count = guard.operator_override_count.saturating_add(1);
    }
    if let Some(writer) = ensure_operator_writer(&mut guard) {
        let _ = writer.write_event(&event);
        let _ = writer.flush();
    }
}

pub fn record_operator_mode(mode: impl AsRef<str>) {
    let mode = mode.as_ref().trim();
    if mode.is_empty() {
        return;
    }

    let mut guard = state().lock().expect("session replay state");
    let event = OperatorEvent::mode(guard.started_at.elapsed().as_secs_f64(), mode);
    guard.operator_event_count = guard.operator_event_count.saturating_add(1);
    if let Some(writer) = ensure_operator_writer(&mut guard) {
        let _ = writer.write_event(&event);
        let _ = writer.flush();
    }
}

pub fn record_operator_bookmark(note: impl AsRef<str>) {
    let note = note.as_ref().trim();
    if note.is_empty() {
        return;
    }

    let mut guard = state().lock().expect("session replay state");
    let event = OperatorEvent::bookmark(guard.started_at.elapsed().as_secs_f64(), note);
    guard.operator_event_count = guard.operator_event_count.saturating_add(1);
    if let Some(writer) = ensure_operator_writer(&mut guard) {
        let _ = writer.write_event(&event);
        let _ = writer.flush();
    }
}

pub fn record_orchestrator_decision(
    actor: impl Into<String>,
    kind: impl Into<String>,
    inputs: Value,
) -> DecisionEventBuilder {
    DecisionEventBuilder {
        actor: actor.into(),
        kind: kind.into(),
        inputs,
    }
}

pub struct DecisionEventBuilder {
    actor: String,
    kind: String,
    inputs: Value,
}

impl DecisionEventBuilder {
    pub fn with_detail(self, key: impl Into<String>, value: impl Serialize) -> DecisionEvent {
        let mut details = BTreeMap::new();
        let value = serde_json::to_value(value).unwrap_or(Value::Null);
        details.insert(key.into(), value);
        self.build(details)
    }

    pub fn build(self, details: BTreeMap<String, Value>) -> DecisionEvent {
        let inputs_hash = canonical_inputs_hash(&self.inputs);
        let mut guard = state().lock().expect("session replay state");
        guard.inputs_by_hash.insert(inputs_hash.clone(), self.inputs.clone());
        let event = DecisionEvent {
            elapsed_secs: guard.started_at.elapsed().as_secs_f64(),
            actor: self.actor,
            kind: self.kind,
            inputs_hash,
            inputs: self.inputs,
            details,
        };
        guard.orchestrator_decision_count = guard.orchestrator_decision_count.saturating_add(1);
        if let Some(writer) = ensure_orchestrator_writer(&mut guard) {
            let _ = writer.write_event(&event);
            let _ = writer.flush();
        }
        event
    }
}

pub fn record_orchestrator_decision_event(event: DecisionEvent) {
    let mut guard = state().lock().expect("session replay state");
    guard
        .inputs_by_hash
        .insert(event.inputs_hash.clone(), event.inputs.clone());
    guard.orchestrator_decision_count = guard.orchestrator_decision_count.saturating_add(1);
    if let Some(writer) = ensure_orchestrator_writer(&mut guard) {
        let _ = writer.write_event(&event);
        let _ = writer.flush();
    }
}

pub fn current_attribution_metrics() -> AttributionMetrics {
    let guard = state().lock().expect("session replay state");
    let elapsed = guard.started_at.elapsed();
    AttributionMetrics {
        decisions_per_minute_operator: decisions_per_minute(guard.operator_event_count, elapsed),
        decisions_per_minute_orchestrator: decisions_per_minute(
            guard.orchestrator_decision_count,
            elapsed,
        ),
        lead_actor: LeadActor::classify(
            guard.operator_event_count,
            guard.orchestrator_decision_count,
        ),
        operator_overrides: guard.operator_override_count,
    }
}

pub fn lookup_inputs(inputs_hash: &str) -> Option<Value> {
    state()
        .lock()
        .expect("session replay state")
        .inputs_by_hash
        .get(inputs_hash)
        .cloned()
}

pub fn decisions_per_minute(decisions: u64, elapsed: Duration) -> f64 {
    let elapsed_secs = elapsed.as_secs_f64();
    if elapsed_secs <= 0.0 {
        return 0.0;
    }

    (decisions as f64) * 60.0 / elapsed_secs
}

pub fn canonical_inputs_hash(value: &Value) -> String {
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    let digest = Sha256::digest(bytes);
    format!("sha256:{}", hex_lower(&digest))
}

pub fn canonical_inputs_value<K, V, I>(fields: I) -> Value
where
    K: Into<String>,
    V: Serialize,
    I: IntoIterator<Item = (K, V)>,
{
    let mut map = Map::new();
    for (key, value) in fields {
        let value = serde_json::to_value(value).unwrap_or(Value::Null);
        map.insert(key.into(), value);
    }
    Value::Object(map)
}

pub fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json).collect()),
        Value::Object(map) => {
            let mut ordered = Map::new();
            for (key, value) in map {
                ordered.insert(key.clone(), canonicalize_json(value));
            }
            Value::Object(ordered)
        }
        other => other.clone(),
    }
}

fn ensure_operator_writer(guard: &mut ReplayState) -> Option<&mut ReplayStreamWriter> {
    if guard.operator_writer.is_none() {
        let header = OperatorStreamHeader {
            version: SESSION_STREAM_VERSION,
            session_id: guard.session_id.clone(),
            captured_at: OPERATOR_CAPTURE_LAYER.to_string(),
            privacy_policy: String::from("R-5 default"),
        };
        let path = stream_path(OPERATOR_STREAM_FILE);
        guard.operator_writer = match ReplayStreamWriter::new(path, header) {
            Ok(writer) => Some(writer),
            Err(error) => {
                tracing::warn!(%error, "Failed to open operator replay stream");
                None
            }
        };
    }
    guard.operator_writer.as_mut()
}

fn ensure_orchestrator_writer(guard: &mut ReplayState) -> Option<&mut ReplayStreamWriter> {
    if guard.orchestrator_writer.is_none() {
        let header = OrchestratorStreamHeader {
            version: SESSION_STREAM_VERSION,
            session_id: guard.session_id.clone(),
            policy_sha: policy_sha(),
            config_hash: guard.orchestrator_config_hash.clone(),
        };
        let path = stream_path(ORCHESTRATOR_STREAM_FILE);
        guard.orchestrator_writer = match ReplayStreamWriter::new(path, header) {
            Ok(writer) => Some(writer),
            Err(error) => {
                tracing::warn!(%error, "Failed to open orchestrator replay stream");
                None
            }
        };
    }
    guard.orchestrator_writer.as_mut()
}

fn stream_path(file_name: &str) -> PathBuf {
    crate::paths::resolve_log_dir().join(file_name)
}

fn write_json_line<T: Serialize>(writer: &mut dyn Write, value: &T) -> std::io::Result<()> {
    serde_json::to_writer(&mut *writer, value).map_err(std::io::Error::other)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn key_input_label(key: &KeyEvent) -> String {
    match key.code {
        KeyCode::Char(ch) => ch.to_string(),
        KeyCode::F(n) => format!("F{n}"),
        other => other.to_string(),
    }
}

fn key_code_label(code: KeyCode, modifiers: KeyModifiers) -> String {
    let mut parts = Vec::new();
    if modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl");
    }
    if modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt");
    }
    if modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("Shift");
    }

    let key = match code {
        KeyCode::Backspace => String::from("Backspace"),
        KeyCode::Enter => String::from("Enter"),
        KeyCode::Left => String::from("Left"),
        KeyCode::Right => String::from("Right"),
        KeyCode::Up => String::from("Up"),
        KeyCode::Down => String::from("Down"),
        KeyCode::Home => String::from("Home"),
        KeyCode::End => String::from("End"),
        KeyCode::PageUp => String::from("PageUp"),
        KeyCode::PageDown => String::from("PageDown"),
        KeyCode::Tab => String::from("Tab"),
        KeyCode::BackTab => String::from("BackTab"),
        KeyCode::Delete => String::from("Delete"),
        KeyCode::Insert => String::from("Insert"),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::Char(ch) => ch.to_string(),
        other => format!("{other:?}"),
    };

    if parts.is_empty() {
        key
    } else {
        format!("{}+{key}", parts.join("+"))
    }
}

fn bookmark_text_redaction_enabled() -> bool {
    std::env::var("TEXTQUEST_OPERATOR_BOOKMARKS_UNREDACTED")
        .map(|value| value.trim().is_empty())
        .unwrap_or(true)
}

fn is_override_candidate(command: &str) -> bool {
    let first = command.split_whitespace().next().unwrap_or("");
    matches!(
        first.to_ascii_lowercase().as_str(),
        "mode"
            | "camp"
            | "hunt"
            | "nav"
            | "mapmarker"
            | "maploc"
            | "pause"
            | "resume"
            | "engage"
            | "disengage"
            | "loot"
            | "target"
            | "assist"
            | "cast"
            | "all"
            | "ma"
            | "mt"
            | "login"
            | "logout"
            | "relog"
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex_lower(&digest)
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{:02x}", byte);
    }
    out
}

fn current_time_sensitive_note(note: &str) -> String {
    if bookmark_text_redaction_enabled() {
        REDACTED_BOOKMARK.to_string()
    } else {
        note.to_owned()
    }
}

#[allow(dead_code)]
fn _bookmark_preview(note: &str) -> (String, String) {
    (current_time_sensitive_note(note), sha256_hex(note.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_inputs_hash_is_stable_across_key_order() {
        let a = json!({
            "config": {"z": 1, "a": 2},
            "state": {"target": "a frenzied ghoul", "hp_pct": 52},
        });
        let b = json!({
            "state": {"hp_pct": 52, "target": "a frenzied ghoul"},
            "config": {"a": 2, "z": 1},
        });

        assert_eq!(canonical_inputs_hash(&a), canonical_inputs_hash(&b));
    }

    #[test]
    fn lead_actor_classification_prefers_majority_side() {
        assert_eq!(LeadActor::classify(0, 0), LeadActor::Mixed);
        assert_eq!(LeadActor::classify(5, 0), LeadActor::Operator);
        assert_eq!(LeadActor::classify(0, 4), LeadActor::Orchestrator);
        assert_eq!(LeadActor::classify(3, 2), LeadActor::Operator);
        assert_eq!(LeadActor::classify(2, 4), LeadActor::Orchestrator);
        assert_eq!(LeadActor::classify(3, 3), LeadActor::Mixed);
    }

    #[test]
    fn decisions_per_minute_handles_zero_elapsed() {
        assert_eq!(decisions_per_minute(3, Duration::from_secs(0)), 0.0);
        assert_eq!(decisions_per_minute(6, Duration::from_secs(30)), 12.0);
    }

    #[test]
    fn canonical_inputs_value_preserves_requested_fields() {
        let value = canonical_inputs_value([
            ("policy_sha", "abc123"),
            ("config_hash", "def456"),
            ("inputs_hash", "sha256:deadbeef"),
        ]);

        assert_eq!(value["policy_sha"], "abc123");
        assert_eq!(value["config_hash"], "def456");
        assert_eq!(value["inputs_hash"], "sha256:deadbeef");
    }

    #[test]
    fn key_labels_include_modifiers() {
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL | KeyModifiers::SHIFT);
        assert_eq!(key_code_label(key.code, key.modifiers), "Ctrl+Shift+x");
        assert_eq!(key_input_label(&key), "x");
    }

    #[test]
    fn bookmark_preview_redacts_by_default() {
        let (note, hash) = _bookmark_preview("interesting wipe");
        assert_eq!(note, "[redacted]");
        assert!(!hash.is_empty());
    }
}
