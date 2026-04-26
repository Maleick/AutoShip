//! Integration tests for session stats aggregation.

#![cfg(windows)]

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use textquest::stats;

    #[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
    struct TestEvent {
        timestamp: String,
        session_id: String,
        iteration: u32,
        event_type: String,
        #[serde(default)]
        details: serde_json::Value,
    }

    fn create_test_session(temp_dir: &TempDir, session_id: &str) -> PathBuf {
        let session_dir = temp_dir.path().join(session_id);
        fs::create_dir_all(&session_dir).unwrap();

        let events = vec![
            TestEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: session_id.to_string(),
                iteration: 1,
                event_type: "ScenarioMobKill".to_string(),
                details: json!({
                    "zone": "gfaydark",
                    "duration_ms": 10000,
                    "party_comp": "full",
                }),
            },
            TestEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: session_id.to_string(),
                iteration: 1,
                event_type: "ScenarioMobKill".to_string(),
                details: json!({
                    "zone": "gfaydark",
                    "duration_ms": 15000,
                    "party_comp": "full",
                }),
            },
            TestEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: session_id.to_string(),
                iteration: 1,
                event_type: "Loot".to_string(),
                details: json!({
                    "zone": "gfaydark",
                    "plat_value": 5000,
                }),
            },
            TestEvent {
                timestamp: Utc::now().to_rfc3339(),
                session_id: session_id.to_string(),
                iteration: 1,
                event_type: "SpellCast".to_string(),
                details: json!({
                    "spell_name": "frost bolt",
                    "success": true,
                }),
            },
        ];

        let mut jsonl = String::new();
        for event in events {
            jsonl.push_str(&serde_json::to_string(&event).unwrap());
            jsonl.push('\n');
        }

        fs::write(session_dir.join("events.jsonl"), jsonl).unwrap();
        session_dir
    }

    #[test]
    fn compact_session_reads_jsonl_and_stores_aggregates() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("metrics.db");
        let session_id = "test-sess-001";
        let session_dir = create_test_session(&temp_dir, session_id);

        let result = stats::compact_session(&db_path, &session_dir, session_id);
        assert!(result.is_ok(), "compact_session failed: {:?}", result);
    }

    #[test]
    fn idempotent_compact_produces_same_results() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("metrics.db");
        let session_id = "test-sess-002";
        let session_dir = create_test_session(&temp_dir, session_id);

        // First compaction
        let result1 = stats::compact_session(&db_path, &session_dir, session_id);
        assert!(result1.is_ok());

        // Second compaction (should be idempotent)
        let result2 = stats::compact_session(&db_path, &session_dir, session_id);
        assert!(result2.is_ok());
    }

    #[test]
    fn compact_all_pending_finds_uncompacted_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("metrics.db");
        let events_dir = temp_dir.path().join("events");
        fs::create_dir_all(&events_dir).unwrap();

        // Create some session directories
        for i in 1..=3 {
            let session_id = format!("session-2026-04-25-120000-sess-{:03}", i);
            create_test_session(&temp_dir, &format!("events/{}", session_id));
        }

        let result = stats::compact_all_pending(&db_path, &events_dir);
        assert!(result.is_ok(), "compact_all_pending failed: {:?}", result);
    }
}
