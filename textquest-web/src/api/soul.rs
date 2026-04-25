//! REST API handlers for Soul Engine audit log.
//!
//! # Soul Engine Narrative Integration
//!
//! The Soul Engine receives session debrief data on session close, allowing
//! character personalities to reference real session outcomes in their dialogue.
//! Soul Engine is **never** a source of numeric truth — it reads metrics from
//! aggregates and reflects them back, but cannot modify configuration or author
//! numeric claims.
//!
//! ## No Numeric Authority
//!
//! Soul Engine personalities may reference session statistics in their dialogue,
//! but must follow these strict constraints:
//!
//! 1. **Read-Only Metrics**: All numeric values come from the `SessionDebrief`
//!    struct, which is populated by the aggregation engine. Soul Engine never
//!    authors or paraphrases numeric values.
//!
//! 2. **Exact Value Mapping**: When Soul Engine refers to a metric (e.g., "you
//!    defeated 47 mobs"), the number must come directly from the debrief payload,
//!    not from LLM inference or estimation.
//!
//! 3. **No Config Authority**: Soul Engine cannot write to character configuration,
//!    trait assignments, or numeric settings. It may only suggest that the operator
//!    review improvements via the `/improve` panel.
//!
//! 4. **One-Way Flow**: Debrief data flows into Soul Engine on session close, and
//!    personalities consume it. There is no feedback loop — Soul Engine output does
//!    not feed back into the suggestion engine or aggregation logic.
//!
//! 5. **Templated References**: Soul Engine uses templated placeholders in prompts
//!    (e.g., `{damage_dealt}`, `{deaths}`) rather than free-form numeric claims.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::AppState;

// ── Session debrief types ────────────────────────────────────────────────────

/// Session outcome summary fed to Soul Engine personalities.
///
/// This struct captures top-N suggestions and session statistics that
/// personalities may reference in their next conversation. All numeric
/// values must come from actual aggregates, never LLM-authored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDebrief {
    /// Session identifier.
    pub session_id: String,
    /// Character's ID for this session.
    pub character_id: u64,
    /// Notable wins/successes this session.
    pub wins: Vec<String>,
    /// Notable losses/failures this session.
    pub losses: Vec<String>,
    /// Top-N suggestions from the improvement engine.
    /// These are references to improvement items, not authored by Soul Engine.
    pub suggestions: Vec<String>,
    /// Session duration in seconds.
    pub duration_secs: u64,
    /// Total damage dealt.
    pub damage_dealt: u64,
    /// Total damage taken.
    pub damage_taken: u64,
    /// Mobs defeated.
    pub mobs_defeated: u32,
    /// Deaths.
    pub deaths: u32,
    /// Timestamp (ISO-8601).
    pub created_at: String,
}

// ── Soul state (panel) types ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulState {
    pub character_id: String,
    pub mood: String,
    pub personality_traits: Vec<String>,
    pub memory_count: u32,
    pub last_event: Option<String>,
}

fn demo_soul_states() -> Vec<SoulState> {
    vec![
        SoulState {
            character_id: "Frostreaver".into(),
            mood: "focused".into(),
            personality_traits: vec!["cautious".into(), "loyal".into(), "stoic".into()],
            memory_count: 142,
            last_event: Some("Recalled Lower Guk speed clear.".into()),
        },
        SoulState {
            character_id: "Shadowdancer".into(),
            mood: "excited".into(),
            personality_traits: vec!["bold".into(), "mischievous".into(), "curious".into()],
            memory_count: 87,
            last_event: Some("Flagged rare spawn: Maestro of Rancor.".into()),
        },
        SoulState {
            character_id: "Ironclad".into(),
            mood: "content".into(),
            personality_traits: vec!["disciplined".into(), "protective".into()],
            memory_count: 201,
            last_event: Some("Completed stable CH chain rotation.".into()),
        },
    ]
}

// ── In-memory audit store ────────────────────────────────────────────────────

/// A single entry in the in-memory audit log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: u64,
    pub character_id: u64,
    pub action_type: String,
    pub action_json: serde_json::Value,
    pub reason: Option<String>,
    pub operator_id: Option<String>,
    pub created_at: String,
}

/// Shared soul audit state.
pub struct SoulAuditState {
    pub entries: RwLock<Vec<AuditEntry>>,
    /// Latest session debrief, stored per character. Updated on session close.
    /// Used by personalities to reference real session data in dialogue.
    pub debrief_by_character: RwLock<std::collections::HashMap<u64, SessionDebrief>>,
    next_id: std::sync::atomic::AtomicU64,
}

impl SoulAuditState {
    /// Create a new empty audit state seeded with demo data.
    pub fn new_demo() -> Arc<Self> {
        let demo = vec![
            AuditEntry {
                id: 1,
                character_id: 1,
                action_type: "say".into(),
                action_json: serde_json::json!({"message": "Hail, adventurer!", "channel": "say", "sentiment": 0.8}),
                reason: Some("idle behavior".into()),
                operator_id: Some("soul_engine".into()),
                created_at: "2026-04-13 00:00:01".into(),
            },
            AuditEntry {
                id: 2,
                character_id: 1,
                action_type: "mood_change".into(),
                action_json: serde_json::json!({"from": "Neutral", "to": "Happy", "trigger": "kill"}),
                reason: Some("mob killed".into()),
                operator_id: Some("soul_engine".into()),
                created_at: "2026-04-13 00:01:00".into(),
            },
            AuditEntry {
                id: 3,
                character_id: 2,
                action_type: "emote".into(),
                action_json: serde_json::json!({"emote": "wave"}),
                reason: None,
                operator_id: Some("soul_engine".into()),
                created_at: "2026-04-13 00:02:00".into(),
            },
        ];
        Arc::new(Self {
            next_id: std::sync::atomic::AtomicU64::new(4),
            entries: RwLock::new(demo),
            debrief_by_character: RwLock::new(std::collections::HashMap::new()),
        })
    }

    /// Append a new entry and return its assigned id.
    pub async fn append(
        &self,
        character_id: u64,
        action_type: String,
        action_json: serde_json::Value,
        reason: Option<String>,
        operator_id: Option<String>,
        created_at: String,
    ) -> u64 {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut entries = self.entries.write().await;
        entries.push(AuditEntry {
            id,
            character_id,
            action_type,
            action_json,
            reason,
            operator_id,
            created_at,
        });
        id
    }
}

// ── Query params ─────────────────────────────────────────────────────────────

/// Pagination and date-filter parameters used by the list endpoints.
#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    /// Starting index for pagination (0-based).
    #[serde(default)]
    pub offset: usize,
    /// Maximum number of entries to return (default 100).
    pub limit: Option<usize>,
    /// Optional start date filter (ISO-8601, inclusive).
    pub start: Option<String>,
    /// Optional end date filter (ISO-8601, inclusive).
    pub end: Option<String>,
}

// ── Response types ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct AuditPage {
    total: usize,
    offset: usize,
    limit: usize,
    entries: Vec<AuditEntry>,
}

// ── Handlers ─────────────────────────────────────────────────────────────────

/// `GET /api/soul` — list current soul states for all characters.
pub async fn list_soul_states() -> impl IntoResponse {
    Json(demo_soul_states())
}

/// `GET /api/soul/:character_id` — fetch soul state for one character.
pub async fn get_soul_state(Path(character_id): Path<String>) -> impl IntoResponse {
    let soul = demo_soul_states()
        .into_iter()
        .find(|s| s.character_id.eq_ignore_ascii_case(&character_id));
    match soul {
        Some(s) => Ok(Json(s)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// `GET /api/soul/audit/:character_id` — audit log for a single character.
pub async fn get_character_audit(
    State(state): State<Arc<AppState>>,
    Path(character_id): Path<u64>,
    Query(params): Query<AuditQuery>,
) -> impl IntoResponse {
    let entries = state.soul_audit.entries.read().await;

    let mut filtered: Vec<&AuditEntry> = entries
        .iter()
        .filter(|e| e.character_id == character_id)
        .filter(|e| {
            params
                .start
                .as_deref()
                .is_none_or(|s| e.created_at.as_str() >= s)
        })
        .filter(|e| {
            params
                .end
                .as_deref()
                .is_none_or(|s| e.created_at.as_str() <= s)
        })
        .collect();

    // Most-recent first
    filtered.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let total = filtered.len();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset;

    let page: Vec<AuditEntry> = filtered
        .into_iter()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect();

    Json(AuditPage {
        total,
        offset,
        limit,
        entries: page,
    })
}

/// `GET /api/soul/audit` — all audit log entries, paginated.
pub async fn get_all_audit(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuditQuery>,
) -> impl IntoResponse {
    let entries = state.soul_audit.entries.read().await;

    let mut filtered: Vec<&AuditEntry> = entries
        .iter()
        .filter(|e| {
            params
                .start
                .as_deref()
                .is_none_or(|s| e.created_at.as_str() >= s)
        })
        .filter(|e| {
            params
                .end
                .as_deref()
                .is_none_or(|s| e.created_at.as_str() <= s)
        })
        .collect();

    filtered.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let total = filtered.len();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset;

    let page: Vec<AuditEntry> = filtered
        .into_iter()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect();

    Json(AuditPage {
        total,
        offset,
        limit,
        entries: page,
    })
}

/// `GET /api/soul/audit/:character_id/export.csv` — CSV export for a character.
pub async fn export_character_audit_csv(
    State(state): State<Arc<AppState>>,
    Path(character_id): Path<u64>,
) -> impl IntoResponse {
    let entries = state.soul_audit.entries.read().await;

    let mut filtered: Vec<&AuditEntry> = entries
        .iter()
        .filter(|e| e.character_id == character_id)
        .collect();

    filtered.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let csv = build_csv(&filtered);

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "text/csv; charset=utf-8"
            .parse()
            .expect("valid header value"),
    );
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"soul_audit_{character_id}.csv\"")
            .parse()
            .expect("valid header value"),
    );

    (StatusCode::OK, headers, csv)
}

/// `GET /api/soul/audit/export.csv` — CSV export for all characters.
pub async fn export_all_audit_csv(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let entries = state.soul_audit.entries.read().await;
    let mut all: Vec<&AuditEntry> = entries.iter().collect();
    all.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let csv = build_csv(&all);

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "text/csv; charset=utf-8"
            .parse()
            .expect("valid header value"),
    );
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        "attachment; filename=\"soul_audit_all.csv\""
            .parse()
            .expect("valid header value"),
    );

    (StatusCode::OK, headers, csv)
}

// ── Session debrief handlers ─────────────────────────────────────────────────

/// `POST /api/soul/debrief` — receive session debrief on session close.
///
/// Soul Engine stores the debrief and makes it available to personalities
/// for reference in their next conversation. Personalities may reference
/// debrief content to provide context-aware advice.
///
/// # Constraints
///
/// - Soul Engine **cannot** write debrief data back to the configuration.
/// - Numeric values in debrief are read-only — they come from aggregates.
/// - All Soul Engine references to debrief must use the provided values,
///   never paraphrase or author new numbers.
pub async fn receive_session_debrief(
    State(state): State<Arc<AppState>>,
    Json(debrief): Json<SessionDebrief>,
) -> impl IntoResponse {
    let mut debriefs = state.soul_audit.debrief_by_character.write().await;
    debriefs.insert(debrief.character_id, debrief.clone());
    (StatusCode::OK, Json(serde_json::json!({"status": "debrief_received"})))
}

/// `GET /api/soul/debrief/:character_id` — retrieve latest session debrief.
///
/// Returns the most recent session debrief for a character. Used by
/// personality systems to reference session outcomes in dialogue.
pub async fn get_session_debrief(
    State(state): State<Arc<AppState>>,
    Path(character_id): Path<u64>,
) -> impl IntoResponse {
    let debriefs = state.soul_audit.debrief_by_character.read().await;
    match debriefs.get(&character_id) {
        Some(debrief) => Ok(Json(debrief.clone())),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// ── CSV helpers
// ───────────────────────────────────────────────────────────────

fn csv_field(s: &str) -> String {
    let formula_safe = if matches!(s.trim_start().chars().next(), Some('=' | '+' | '-' | '@')) {
        format!("'{s}")
    } else {
        s.to_owned()
    };

    if formula_safe.contains(',') || formula_safe.contains('"') || formula_safe.contains('\n') {
        format!("\"{}\"", formula_safe.replace('"', "\"\""))
    } else {
        formula_safe
    }
}

fn build_csv(entries: &[&AuditEntry]) -> String {
    let mut out =
        String::from("id,character_id,action_type,action_json,reason,operator_id,created_at\n");
    for e in entries {
        let action_json_str = e.action_json.to_string();
        let reason = e.reason.as_deref().unwrap_or("");
        let operator = e.operator_id.as_deref().unwrap_or("");
        out.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            e.id,
            e.character_id,
            csv_field(&e.action_type),
            csv_field(&action_json_str),
            csv_field(reason),
            csv_field(operator),
            csv_field(&e.created_at),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_field_escapes_commas() {
        assert_eq!(csv_field("hello, world"), "\"hello, world\"");
    }

    #[test]
    fn csv_field_escapes_quotes() {
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn csv_field_passthrough_plain() {
        assert_eq!(csv_field("say"), "say");
    }

    #[test]
    fn csv_field_neutralizes_formula_cells() {
        assert_eq!(csv_field("=1+1"), "'=1+1");
        assert_eq!(csv_field("+SUM(A1:A2)"), "'+SUM(A1:A2)");
        assert_eq!(csv_field("-cmd"), "'-cmd");
        assert_eq!(csv_field("@evil"), "'@evil");

        assert_eq!(csv_field(" =1+1"), "' =1+1");
        assert_eq!(csv_field("\t=1+1"), "'\t=1+1");
        assert_eq!(csv_field(" +SUM(A1:A2)"), "' +SUM(A1:A2)");
        assert_eq!(csv_field("\t+SUM(A1:A2)"), "'\t+SUM(A1:A2)");
        assert_eq!(csv_field(" -cmd"), "' -cmd");
        assert_eq!(csv_field("\t-cmd"), "'\t-cmd");
        assert_eq!(csv_field(" @evil"), "' @evil");
        assert_eq!(csv_field("\t@evil"), "'\t@evil");

        assert_eq!(csv_field("=1,2"), "\"'=1,2\"");
        assert_eq!(csv_field("=say \"hi\""), "\"'=say \"\"hi\"\"\"");
        assert_eq!(csv_field("=1\n2"), "\"'=1\n2\"");
    }

    #[test]
    fn build_csv_header_present() {
        let csv = build_csv(&[]);
        assert!(csv.starts_with(
            "id,character_id,action_type,action_json,reason,operator_id,created_at\n"
        ));
    }

    #[test]
    fn build_csv_one_row() {
        let e = AuditEntry {
            id: 1,
            character_id: 5,
            action_type: "say".into(),
            action_json: serde_json::json!({"message": "hi"}),
            reason: None,
            operator_id: Some("engine".into()),
            created_at: "2026-04-13 00:00:00".into(),
        };
        let csv = build_csv(&[&e]);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].starts_with("1,5,say,"));
    }

    #[test]
    fn session_debrief_numeric_values_preserved() {
        let debrief = SessionDebrief {
            session_id: "s123".into(),
            character_id: 42,
            wins: vec!["cleared Lower Guk".into()],
            losses: vec!["died at zone line".into()],
            suggestions: vec!["flag dangerous zones".into()],
            duration_secs: 3600,
            damage_dealt: 50000,
            damage_taken: 12000,
            mobs_defeated: 47,
            deaths: 2,
            created_at: "2026-04-25 10:00:00".into(),
        };

        let json = serde_json::to_value(&debrief).expect("serialize");
        assert_eq!(json["damage_dealt"], 50000);
        assert_eq!(json["damage_taken"], 12000);
        assert_eq!(json["mobs_defeated"], 47);
        assert_eq!(json["deaths"], 2);
    }
}
