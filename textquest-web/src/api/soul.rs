//! REST API handlers for Soul Engine audit log.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::AppState;

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

// ── CSV helpers ───────────────────────────────────────────────────────────────

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
}
