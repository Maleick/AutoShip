//! Test fixtures for stats aggregation tests.

#[cfg(test)]
pub mod fixtures {
    use crate::stats::aggregator::SessionEvent;
    use chrono::Utc;
    use serde_json::json;

    pub fn sample_kill_event(zone: &str, duration_ms: i64, party_comp: &str) -> SessionEvent {
        SessionEvent {
            timestamp: Utc::now().to_rfc3339(),
            session_id: "test-sess".to_string(),
            iteration: 1,
            event_type: "ScenarioMobKill".to_string(),
            details: json!({
                "zone": zone,
                "duration_ms": duration_ms,
                "party_comp": party_comp,
            }),
        }
    }

    pub fn sample_loot_event(zone: &str, plat_value: i64) -> SessionEvent {
        SessionEvent {
            timestamp: Utc::now().to_rfc3339(),
            session_id: "test-sess".to_string(),
            iteration: 1,
            event_type: "Loot".to_string(),
            details: json!({
                "zone": zone,
                "plat_value": plat_value,
            }),
        }
    }

    pub fn sample_spell_cast_event(spell_name: &str, success: bool) -> SessionEvent {
        SessionEvent {
            timestamp: Utc::now().to_rfc3339(),
            session_id: "test-sess".to_string(),
            iteration: 1,
            event_type: "SpellCast".to_string(),
            details: json!({
                "spell_name": spell_name,
                "success": success,
            }),
        }
    }

    pub fn sample_pull_event() -> SessionEvent {
        SessionEvent {
            timestamp: Utc::now().to_rfc3339(),
            session_id: "test-sess".to_string(),
            iteration: 1,
            event_type: "Pull".to_string(),
            details: json!({}),
        }
    }
}
