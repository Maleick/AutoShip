use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionDebrief {
    pub schema_version: u32,
    pub session_id: String,
    pub duration_min: u32,
    pub zone_seq: Vec<String>,
    pub party: Vec<PartyMember>,
    pub by_character: BTreeMap<String, CharacterDebrief>,
    pub deltas_vs_p50: BTreeMap<String, f64>,
    pub notable_event_ids: Vec<String>,
    pub deaths: Vec<DeathSummary>,
    pub lessons_seed: Vec<String>,
    pub camp_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartyMember {
    pub name: String,
    #[serde(rename = "class")]
    pub class_name: String,
    pub role: String,
    pub level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterDebrief {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heals_total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overheal_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ooms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deaths: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub biggest_heal_event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burns: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fizzles: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interrupts: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeathSummary {
    pub who: String,
    pub attacker: String,
    pub last_5_damage_json: serde_json::Value,
}

pub fn build_debrief(session_id: &str, _db: &SessionDb) -> Result<SessionDebrief, Error> {
    let _ = session_id;
    unimplemented!("debrief aggregation scaffold");
}

// These aliases will be replaced with the crate's real DB and error types once
// the surrounding module layout is confirmed from the compiler.
type SessionDb = ();
type Error = anyhow::Error;
