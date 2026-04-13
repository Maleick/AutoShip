//! Kill tracking and advanced DPS analytics.
//!
//! Aggregates kill events into per-mob and per-session statistics,
//! computes DPS metrics, and provides efficiency scoring for
//! camp optimization feedback.

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// A single recorded kill with timing and damage data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KillRecord {
    /// Mob name that was killed.
    pub mob_name: String,
    /// Mob level at time of kill.
    pub mob_level: u8,
    /// Zone where the kill occurred.
    pub zone: String,
    /// PID of the client that landed the killing blow.
    pub killer_pid: u32,
    /// Duration from pull to death in milliseconds.
    pub kill_time_ms: u64,
    /// Total damage dealt to this mob across all clients.
    pub total_damage: u64,
    /// Unix epoch timestamp of the kill.
    pub timestamp: i64,
}

impl KillRecord {
    /// Effective DPS for this kill (damage / seconds).
    pub fn dps(&self) -> f64 {
        if self.kill_time_ms == 0 {
            return 0.0;
        }
        self.total_damage as f64 / (self.kill_time_ms as f64 / 1000.0)
    }
}

/// Per-mob aggregate statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MobStats {
    /// Total times this mob has been killed.
    pub kill_count: u32,
    /// Fastest kill time in milliseconds.
    pub best_time_ms: u64,
    /// Average kill time in milliseconds.
    pub avg_time_ms: u64,
    /// Total damage dealt across all kills of this mob.
    pub total_damage: u64,
    /// Average DPS across all kills of this mob.
    pub avg_dps: f64,
}

/// Session-level DPS statistics for a single client.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientDpsStats {
    pub pid: u32,
    pub total_damage: u64,
    pub total_combat_time_ms: u64,
    pub kill_count: u32,
}

impl ClientDpsStats {
    /// Session average DPS (total damage / total combat time).
    pub fn session_dps(&self) -> f64 {
        if self.total_combat_time_ms == 0 {
            return 0.0;
        }
        self.total_damage as f64 / (self.total_combat_time_ms as f64 / 1000.0)
    }
}

/// Session efficiency score components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EfficiencyScore {
    /// Kills per hour.
    pub kills_per_hour: f64,
    /// Average kill time in seconds.
    pub avg_kill_time_secs: f64,
    /// Deaths per kill ratio (lower is better).
    pub death_ratio: f64,
    /// Composite score (0-100).
    pub score: f64,
}

/// Tracks kill events and computes analytics.
pub struct KillTracker {
    kills: Vec<KillRecord>,
    /// Session start timestamp (Unix epoch).
    session_start: i64,
    /// Deaths this session.
    death_count: u32,
}

impl KillTracker {
    /// Creates a new tracker with the given session start time.
    pub fn new(session_start: i64) -> Self {
        Self {
            kills: Vec::new(),
            session_start,
            death_count: 0,
        }
    }

    /// Records a kill event.
    pub fn record_kill(&mut self, record: KillRecord) {
        self.kills.push(record);
    }

    /// Records a death (used for efficiency scoring).
    pub fn record_death(&mut self) {
        self.death_count += 1;
    }

    /// Total kills this session.
    pub fn total_kills(&self) -> usize {
        self.kills.len()
    }

    /// Total deaths this session.
    pub fn total_deaths(&self) -> u32 {
        self.death_count
    }

    /// Returns per-mob aggregated statistics.
    pub fn mob_stats(&self) -> HashMap<String, MobStats> {
        let mut stats: HashMap<String, Vec<&KillRecord>> = HashMap::new();
        for kill in &self.kills {
            stats.entry(kill.mob_name.clone()).or_default().push(kill);
        }

        stats
            .into_iter()
            .map(|(name, records)| {
                let kill_count = records.len() as u32;
                let best_time_ms = records.iter().map(|r| r.kill_time_ms).min().unwrap_or(0);
                let total_time: u64 = records.iter().map(|r| r.kill_time_ms).sum();
                let avg_time_ms = if kill_count > 0 {
                    total_time / u64::from(kill_count)
                } else {
                    0
                };
                let total_damage: u64 = records.iter().map(|r| r.total_damage).sum();
                let avg_dps = if total_time > 0 {
                    total_damage as f64 / (total_time as f64 / 1000.0)
                } else {
                    0.0
                };

                (
                    name,
                    MobStats {
                        kill_count,
                        best_time_ms,
                        avg_time_ms,
                        total_damage,
                        avg_dps,
                    },
                )
            })
            .collect()
    }

    /// Returns per-client DPS statistics.
    pub fn client_dps(&self) -> HashMap<u32, ClientDpsStats> {
        let mut stats: HashMap<u32, ClientDpsStats> = HashMap::new();
        for kill in &self.kills {
            let entry = stats.entry(kill.killer_pid).or_insert(ClientDpsStats {
                pid: kill.killer_pid,
                ..Default::default()
            });
            entry.total_damage += kill.total_damage;
            entry.total_combat_time_ms += kill.kill_time_ms;
            entry.kill_count += 1;
        }
        stats
    }

    /// Returns kills within the given time window (from `since` timestamp).
    pub fn kills_since(&self, since: i64) -> Vec<&KillRecord> {
        self.kills.iter().filter(|k| k.timestamp >= since).collect()
    }

    /// Returns kills for a specific zone.
    pub fn kills_in_zone(&self, zone: &str) -> Vec<&KillRecord> {
        self.kills.iter().filter(|k| k.zone == zone).collect()
    }

    /// Computes session efficiency score.
    pub fn efficiency(&self, current_time: i64) -> EfficiencyScore {
        let session_duration =
            Duration::from_secs((current_time - self.session_start).max(0) as u64);
        let hours = session_duration.as_secs_f64() / 3600.0;

        let kills_per_hour = if hours > 0.0 {
            self.kills.len() as f64 / hours
        } else {
            0.0
        };

        let total_kill_time: u64 = self.kills.iter().map(|k| k.kill_time_ms).sum();
        let avg_kill_time_secs = if self.kills.is_empty() {
            0.0
        } else {
            (total_kill_time as f64 / self.kills.len() as f64) / 1000.0
        };

        let death_ratio = if self.kills.is_empty() {
            0.0
        } else {
            self.death_count as f64 / self.kills.len() as f64
        };

        // Composite score: weighted sum, capped at 100.
        // Higher kills/hr and lower death ratio = better.
        // Baseline: 60 kills/hr with 0 deaths = 100.
        let kph_component = (kills_per_hour / 60.0).min(1.0) * 60.0;
        let death_penalty = (death_ratio * 20.0).min(40.0);
        let time_component = if avg_kill_time_secs > 0.0 {
            (30.0 / avg_kill_time_secs).min(40.0)
        } else {
            0.0
        };
        let score = (kph_component + time_component - death_penalty).clamp(0.0, 100.0);

        EfficiencyScore {
            kills_per_hour,
            avg_kill_time_secs,
            death_ratio,
            score,
        }
    }

    /// Returns the top N mobs by kill count.
    pub fn top_mobs(&self, n: usize) -> Vec<(String, u32)> {
        let stats = self.mob_stats();
        let mut sorted: Vec<(String, u32)> = stats
            .into_iter()
            .map(|(name, s)| (name, s.kill_count))
            .collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(n);
        sorted
    }

    /// All recorded kills (for serialization/persistence).
    pub fn all_kills(&self) -> &[KillRecord] {
        &self.kills
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(mob: &str, level: u8, kill_time_ms: u64, damage: u64, ts: i64) -> KillRecord {
        KillRecord {
            mob_name: mob.to_string(),
            mob_level: level,
            zone: "gfaydark".to_string(),
            killer_pid: 1000,
            kill_time_ms,
            total_damage: damage,
            timestamp: ts,
        }
    }

    #[test]
    fn kill_record_dps() {
        let r = make_record("orc_pawn", 5, 10_000, 5000, 100);
        assert!((r.dps() - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn kill_record_dps_zero_time() {
        let r = make_record("orc_pawn", 5, 0, 5000, 100);
        assert_eq!(r.dps(), 0.0);
    }

    #[test]
    fn tracker_record_and_count() {
        let mut tracker = KillTracker::new(0);
        assert_eq!(tracker.total_kills(), 0);
        tracker.record_kill(make_record("orc_pawn", 5, 10_000, 5000, 100));
        tracker.record_kill(make_record("orc_centurion", 8, 15_000, 8000, 200));
        assert_eq!(tracker.total_kills(), 2);
    }

    #[test]
    fn tracker_deaths() {
        let mut tracker = KillTracker::new(0);
        assert_eq!(tracker.total_deaths(), 0);
        tracker.record_death();
        tracker.record_death();
        assert_eq!(tracker.total_deaths(), 2);
    }

    #[test]
    fn mob_stats_aggregation() {
        let mut tracker = KillTracker::new(0);
        tracker.record_kill(make_record("orc_pawn", 5, 10_000, 5000, 100));
        tracker.record_kill(make_record("orc_pawn", 5, 8_000, 4000, 200));
        tracker.record_kill(make_record("orc_centurion", 8, 15_000, 8000, 300));

        let stats = tracker.mob_stats();
        let pawn = &stats["orc_pawn"];
        assert_eq!(pawn.kill_count, 2);
        assert_eq!(pawn.best_time_ms, 8_000);
        assert_eq!(pawn.avg_time_ms, 9_000);
        assert_eq!(pawn.total_damage, 9000);
        // avg_dps = 9000 / (18000ms / 1000) = 9000 / 18 = 500
        assert!((pawn.avg_dps - 500.0).abs() < f64::EPSILON);

        let cent = &stats["orc_centurion"];
        assert_eq!(cent.kill_count, 1);
        assert_eq!(cent.best_time_ms, 15_000);
    }

    #[test]
    fn client_dps_stats() {
        let mut tracker = KillTracker::new(0);
        let mut r1 = make_record("orc_pawn", 5, 10_000, 5000, 100);
        r1.killer_pid = 1000;
        let mut r2 = make_record("orc_pawn", 5, 8_000, 4000, 200);
        r2.killer_pid = 2000;
        let mut r3 = make_record("orc_centurion", 8, 12_000, 6000, 300);
        r3.killer_pid = 1000;

        tracker.record_kill(r1);
        tracker.record_kill(r2);
        tracker.record_kill(r3);

        let dps = tracker.client_dps();
        let client1 = &dps[&1000];
        assert_eq!(client1.kill_count, 2);
        assert_eq!(client1.total_damage, 11_000);
        assert_eq!(client1.total_combat_time_ms, 22_000);
        // session_dps = 11000 / 22 = 500
        assert!((client1.session_dps() - 500.0).abs() < f64::EPSILON);

        let client2 = &dps[&2000];
        assert_eq!(client2.kill_count, 1);
        // session_dps = 4000 / 8 = 500
        assert!((client2.session_dps() - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn client_dps_zero_time() {
        let stats = ClientDpsStats::default();
        assert_eq!(stats.session_dps(), 0.0);
    }

    #[test]
    fn kills_since_filter() {
        let mut tracker = KillTracker::new(0);
        tracker.record_kill(make_record("orc_pawn", 5, 10_000, 5000, 100));
        tracker.record_kill(make_record("orc_pawn", 5, 10_000, 5000, 200));
        tracker.record_kill(make_record("orc_pawn", 5, 10_000, 5000, 300));

        let recent = tracker.kills_since(200);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].timestamp, 200);
        assert_eq!(recent[1].timestamp, 300);
    }

    #[test]
    fn kills_in_zone_filter() {
        let mut tracker = KillTracker::new(0);
        tracker.record_kill(make_record("orc_pawn", 5, 10_000, 5000, 100));
        let mut nektulos_kill = make_record("skeleton", 10, 12_000, 6000, 200);
        nektulos_kill.zone = "nektulos".to_string();
        tracker.record_kill(nektulos_kill);

        let gfay = tracker.kills_in_zone("gfaydark");
        assert_eq!(gfay.len(), 1);
        let nek = tracker.kills_in_zone("nektulos");
        assert_eq!(nek.len(), 1);
        let empty = tracker.kills_in_zone("commons");
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn efficiency_score_basic() {
        let mut tracker = KillTracker::new(0);
        // Simulate 1 hour of kills: 60 kills, 30s each, 0 deaths
        for i in 0..60 {
            tracker.record_kill(make_record("orc_pawn", 5, 30_000, 15_000, (i * 60) + 30));
        }
        let eff = tracker.efficiency(3600);
        assert!((eff.kills_per_hour - 60.0).abs() < 0.1);
        assert!((eff.avg_kill_time_secs - 30.0).abs() < 0.1);
        assert_eq!(eff.death_ratio, 0.0);
        assert!(eff.score > 50.0);
    }

    #[test]
    fn efficiency_with_deaths() {
        let mut tracker = KillTracker::new(0);
        for i in 0..10 {
            tracker.record_kill(make_record("orc_pawn", 5, 30_000, 15_000, i * 60));
        }
        tracker.record_death();
        tracker.record_death();
        tracker.record_death();
        let eff = tracker.efficiency(600);
        assert!(eff.death_ratio > 0.0);
        // Score should be lower due to deaths
        let mut tracker2 = KillTracker::new(0);
        for i in 0..10 {
            tracker2.record_kill(make_record("orc_pawn", 5, 30_000, 15_000, i * 60));
        }
        let eff2 = tracker2.efficiency(600);
        assert!(eff2.score > eff.score);
    }

    #[test]
    fn efficiency_empty_session() {
        let tracker = KillTracker::new(0);
        let eff = tracker.efficiency(3600);
        assert_eq!(eff.kills_per_hour, 0.0);
        assert_eq!(eff.avg_kill_time_secs, 0.0);
        assert_eq!(eff.death_ratio, 0.0);
        assert_eq!(eff.score, 0.0);
    }

    #[test]
    fn top_mobs_sorted() {
        let mut tracker = KillTracker::new(0);
        for i in 0..5 {
            tracker.record_kill(make_record("orc_pawn", 5, 10_000, 5000, i));
        }
        for i in 0..3 {
            tracker.record_kill(make_record("orc_centurion", 8, 15_000, 8000, 10 + i));
        }
        tracker.record_kill(make_record("gnoll_pup", 3, 5_000, 2000, 20));

        let top = tracker.top_mobs(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0], ("orc_pawn".to_string(), 5));
        assert_eq!(top[1], ("orc_centurion".to_string(), 3));
    }

    #[test]
    fn all_kills_accessor() {
        let mut tracker = KillTracker::new(0);
        tracker.record_kill(make_record("orc_pawn", 5, 10_000, 5000, 100));
        assert_eq!(tracker.all_kills().len(), 1);
        assert_eq!(tracker.all_kills()[0].mob_name, "orc_pawn");
    }

    #[test]
    fn serde_kill_record_roundtrip() {
        let r = make_record("orc_pawn", 5, 10_000, 5000, 100);
        let json = serde_json::to_string(&r).unwrap();
        let back: KillRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }
}
