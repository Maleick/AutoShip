//! Session aggregation pipeline — derives high-level summaries from raw fleet events.
//!
//! Reads FleetEvent streams and produces SessionSummary with metrics:
//! - kills/hour, mana efficiency, pull rate, camp efficiency, death rate, downtime patterns.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::metrics::events::FleetEvent;

/// Session-level summary aggregated from raw events.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SessionSummary {
    /// Unix timestamp when session started.
    pub session_start: i64,
    /// Unix timestamp when session ended.
    pub session_end: i64,
    /// Total elapsed time in seconds.
    pub elapsed_secs: u64,
    /// Total kills recorded.
    pub total_kills: u32,
    /// Total deaths recorded.
    pub total_deaths: u32,
    /// Kills per hour (rate).
    pub kills_per_hour: f64,
    /// Death rate as fraction (deaths / kills, 0.0-1.0).
    pub death_rate: f64,
    /// Average kills per pull (kills / pull_count).
    pub pull_rate: f64,
    /// Camp efficiency: kills per zone visit.
    pub camp_efficiency: f64,
    /// Mana efficiency: damage per spell cast.
    pub mana_efficiency: f64,
    /// Total damage dealt across session.
    pub total_damage_dealt: u64,
    /// Total damage taken across session.
    pub total_damage_taken: u64,
    /// Total spells cast.
    pub total_spells_cast: u32,
    /// Total items looted.
    pub total_items_looted: u32,
    /// Number of zone changes (proxies for pulls/camps).
    pub zone_changes: u32,
    /// Character activity periods: vec of (start_timestamp, end_timestamp, zone).
    pub downtime_patterns: Vec<(i64, i64, String)>,
}

impl SessionSummary {
    /// Create empty summary for a given session start time.
    pub fn new(session_start: i64) -> Self {
        Self {
            session_start,
            ..Default::default()
        }
    }

    /// Aggregate a slice of FleetEvents into a SessionSummary.
    pub fn from_events(events: &[FleetEvent]) -> Self {
        if events.is_empty() {
            return Self::default();
        }

        let mut summary = Self::new(events[0].timestamp());
        let mut char_zones: HashMap<u32, (i64, String)> = HashMap::new();
        let mut downtime_segments: Vec<(i64, i64, String)> = Vec::new();

        for event in events {
            match event {
                FleetEvent::Kill {
                    source_pid,
                    target_name: _,
                    target_level: _,
                    zone,
                    timestamp,
                } => {
                    summary.total_kills += 1;
                    summary.session_end = *timestamp;

                    char_zones.insert(*source_pid, (*timestamp, zone.clone()));
                }
                FleetEvent::Death {
                    pid,
                    character_name: _,
                    zone,
                    timestamp,
                } => {
                    summary.total_deaths += 1;
                    summary.session_end = *timestamp;
                    char_zones.insert(*pid, (*timestamp, zone.clone()));
                }
                FleetEvent::LootDrop {
                    pid: _,
                    item_name: _,
                    item_id: _,
                    zone: _,
                    timestamp,
                } => {
                    summary.total_items_looted += 1;
                    summary.session_end = *timestamp;
                }
                FleetEvent::ZoneChange {
                    pid,
                    from_zone: _,
                    to_zone,
                    timestamp,
                } => {
                    summary.zone_changes += 1;
                    summary.session_end = *timestamp;

                    if let Some((prev_timestamp, _prev_zone)) = char_zones.get(pid) {
                        downtime_segments.push((*prev_timestamp, *timestamp, to_zone.clone()));
                    }
                    char_zones.insert(*pid, (*timestamp, to_zone.clone()));
                }
                FleetEvent::LevelUp {
                    pid,
                    character_name: _,
                    new_level: _,
                    timestamp,
                } => {
                    summary.session_end = *timestamp;
                    char_zones.insert(*pid, (*timestamp, String::new()));
                }
                FleetEvent::CombatRound {
                    pid: _,
                    damage_dealt,
                    damage_taken,
                    duration_ms: _,
                    timestamp,
                } => {
                    summary.total_damage_dealt += damage_dealt;
                    summary.total_damage_taken += damage_taken;
                    summary.session_end = *timestamp;
                    summary.total_spells_cast += 1;
                }
            }
        }

        summary.downtime_patterns = downtime_segments;
        summary.elapsed_secs = (summary.session_end - summary.session_start).max(0) as u64;

        summary.compute_rates();
        summary
    }

    /// Calculate derived rate metrics from raw counts.
    fn compute_rates(&mut self) {
        if self.elapsed_secs == 0 {
            return;
        }

        let hours = self.elapsed_secs as f64 / 3600.0;
        if hours > 0.0 {
            self.kills_per_hour = self.total_kills as f64 / hours;
        }

        if self.total_kills > 0 {
            self.death_rate = self.total_deaths as f64 / self.total_kills as f64;
        }

        let pull_count = self.zone_changes.max(1) as f64;
        self.pull_rate = self.total_kills as f64 / pull_count;

        if self.zone_changes > 0 {
            self.camp_efficiency = self.total_kills as f64 / self.zone_changes as f64;
        }

        if self.total_spells_cast > 0 {
            self.mana_efficiency =
                self.total_damage_dealt as f64 / self.total_spells_cast as f64;
        }
    }
}

/// Pipeline processor: reads events and yields aggregated summaries.
pub struct AggregationPipeline {
    events: Vec<FleetEvent>,
}

impl AggregationPipeline {
    /// Create new aggregation pipeline.
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
        }
    }

    /// Add event to pipeline buffer.
    pub fn add_event(&mut self, event: FleetEvent) {
        self.events.push(event);
    }

    /// Add multiple events.
    pub fn add_events(&mut self, events: Vec<FleetEvent>) {
        self.events.extend(events);
    }

    /// Generate session summary from buffered events.
    pub fn aggregate(&self) -> SessionSummary {
        SessionSummary::from_events(&self.events)
    }

    /// Generate per-character summaries by partitioning events by PID.
    pub fn aggregate_per_character(&self) -> HashMap<u32, SessionSummary> {
        let mut by_pid: HashMap<u32, Vec<FleetEvent>> = HashMap::new();

        for event in &self.events {
            by_pid.entry(event.pid()).or_insert_with(Vec::new).push(event.clone());
        }

        by_pid
            .into_iter()
            .map(|(pid, events)| (pid, SessionSummary::from_events(&events)))
            .collect()
    }

    /// Reset pipeline state.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

impl Default for AggregationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_from_empty_events() {
        let summary = SessionSummary::from_events(&[]);
        assert_eq!(summary.total_kills, 0);
        assert_eq!(summary.total_deaths, 0);
        assert_eq!(summary.elapsed_secs, 0);
    }

    #[test]
    fn summary_kills_per_hour() {
        let events = vec![
            FleetEvent::Kill {
                source_pid: 1,
                target_name: "Mob1".to_string(),
                target_level: 60,
                zone: "PoP".to_string(),
                timestamp: 1000,
            },
            FleetEvent::Kill {
                source_pid: 1,
                target_name: "Mob2".to_string(),
                target_level: 60,
                zone: "PoP".to_string(),
                timestamp: 4600, // 3600 seconds later
            },
        ];

        let summary = SessionSummary::from_events(&events);
        assert_eq!(summary.total_kills, 2);
        assert_eq!(summary.elapsed_secs, 3600);
        assert!((summary.kills_per_hour - 2.0).abs() < 0.01);
    }

    #[test]
    fn summary_death_rate() {
        let events = vec![
            FleetEvent::Kill {
                source_pid: 1,
                target_name: "Mob".to_string(),
                target_level: 60,
                zone: "PoP".to_string(),
                timestamp: 1000,
            },
            FleetEvent::Death {
                pid: 1,
                character_name: "Char".to_string(),
                zone: "PoP".to_string(),
                timestamp: 2000,
            },
        ];

        let summary = SessionSummary::from_events(&events);
        assert_eq!(summary.total_kills, 1);
        assert_eq!(summary.total_deaths, 1);
        assert!((summary.death_rate - 1.0).abs() < 0.01);
    }

    #[test]
    fn summary_pull_rate() {
        let events = vec![
            FleetEvent::ZoneChange {
                pid: 1,
                from_zone: "PoP".to_string(),
                to_zone: "PoP".to_string(),
                timestamp: 1000,
            },
            FleetEvent::Kill {
                source_pid: 1,
                target_name: "Mob".to_string(),
                target_level: 60,
                zone: "PoP".to_string(),
                timestamp: 1500,
            },
            FleetEvent::ZoneChange {
                pid: 1,
                from_zone: "PoP".to_string(),
                to_zone: "PoP".to_string(),
                timestamp: 2000,
            },
            FleetEvent::Kill {
                source_pid: 1,
                target_name: "Mob2".to_string(),
                target_level: 60,
                zone: "PoP".to_string(),
                timestamp: 2500,
            },
        ];

        let summary = SessionSummary::from_events(&events);
        assert_eq!(summary.total_kills, 2);
        assert_eq!(summary.zone_changes, 2);
        assert!((summary.pull_rate - 1.0).abs() < 0.01);
    }

    #[test]
    fn summary_mana_efficiency() {
        let events = vec![
            FleetEvent::CombatRound {
                pid: 1,
                damage_dealt: 1000,
                damage_taken: 100,
                duration_ms: 1000,
                timestamp: 1000,
            },
            FleetEvent::CombatRound {
                pid: 1,
                damage_dealt: 1000,
                damage_taken: 100,
                duration_ms: 1000,
                timestamp: 2000,
            },
        ];

        let summary = SessionSummary::from_events(&events);
        assert_eq!(summary.total_spells_cast, 2);
        assert_eq!(summary.total_damage_dealt, 2000);
        assert!((summary.mana_efficiency - 1000.0).abs() < 0.01);
    }

    #[test]
    fn pipeline_aggregate() {
        let mut pipeline = AggregationPipeline::new();
        pipeline.add_event(FleetEvent::Kill {
            source_pid: 1,
            target_name: "Mob".to_string(),
            target_level: 60,
            zone: "PoP".to_string(),
            timestamp: 1000,
        });

        let summary = pipeline.aggregate();
        assert_eq!(summary.total_kills, 1);
    }

    #[test]
    fn pipeline_per_character() {
        let mut pipeline = AggregationPipeline::new();
        pipeline.add_events(vec![
            FleetEvent::Kill {
                source_pid: 1,
                target_name: "Mob".to_string(),
                target_level: 60,
                zone: "PoP".to_string(),
                timestamp: 1000,
            },
            FleetEvent::Kill {
                source_pid: 2,
                target_name: "Mob2".to_string(),
                target_level: 60,
                zone: "PoP".to_string(),
                timestamp: 1000,
            },
        ]);

        let per_char = pipeline.aggregate_per_character();
        assert_eq!(per_char.len(), 2);
        assert_eq!(per_char[&1].total_kills, 1);
        assert_eq!(per_char[&2].total_kills, 1);
    }
}
