//! Real-time progress monitoring — fleet-level and per-client snapshots.
//!
//! `ProgressReport` is emitted by the orchestrator loop at a configurable
//! interval and consumed by the TUI dashboard and any downstream metrics
//! pipeline (Discord, SQLite, etc.).

use std::time::{Duration, Instant};

/// Per-client progress snapshot captured at one point in time.
#[derive(Debug, Clone)]
pub struct ClientProgressSnapshot {
    /// OS process ID.
    pub pid: u32,
    /// Character name, if bound.
    pub character_name: String,
    /// Zone short name.
    pub zone: String,
    /// XP gain rate over the last 60-minute window (percent per hour).
    pub xp_per_hour: f32,
    /// AA XP gain rate over the last 60-minute window (percent per hour).
    pub aa_xp_per_hour: f32,
    /// Kill/loot events logged since session start.
    pub kills_session: usize,
    /// Loot items collected since session start.
    pub items_looted_session: u32,
    /// Deaths recorded since session start.
    pub deaths_session: usize,
    /// Whether the client is currently in active combat.
    pub in_combat: bool,
    /// Seconds since the orchestrator registered this client.
    pub uptime_secs: u64,
}

impl ClientProgressSnapshot {
    /// Returns kills-per-hour derived from session totals and uptime.
    #[must_use]
    pub fn kills_per_hour(&self) -> f64 {
        if self.uptime_secs == 0 {
            return 0.0;
        }
        self.kills_session as f64 / (self.uptime_secs as f64 / 3600.0)
    }

    /// Returns loot items per hour derived from session totals and uptime.
    #[must_use]
    pub fn loot_rate_per_hour(&self) -> f64 {
        if self.uptime_secs == 0 {
            return 0.0;
        }
        self.items_looted_session as f64 / (self.uptime_secs as f64 / 3600.0)
    }

    /// Returns deaths per hour derived from session totals and uptime.
    #[must_use]
    pub fn deaths_per_hour(&self) -> f64 {
        if self.uptime_secs == 0 {
            return 0.0;
        }
        self.deaths_session as f64 / (self.uptime_secs as f64 / 3600.0)
    }
}

/// Fleet-level progress report emitted at a configurable interval.
///
/// Consumers (TUI, Discord webhook, metrics pipeline) receive this via
/// [`crate::orchestrator_loop::LoopEvent::ProgressReported`].
#[derive(Debug, Clone)]
pub struct ProgressReport {
    /// Monotonic instant when this snapshot was captured.
    pub captured_at: Instant,
    /// Total clients currently tracked by the orchestrator.
    pub total_clients: usize,
    /// Clients with an active camp loop running.
    pub active_clients: usize,
    /// Fleet-wide XP/hour average across clients that have rate data.
    pub fleet_xp_per_hour_avg: f32,
    /// Fleet-wide kill rate per hour (session totals / uptime).
    pub fleet_kills_per_hour: f64,
    /// Fleet-wide loot rate per hour.
    pub fleet_loot_rate_per_hour: f64,
    /// Fleet-wide deaths per hour.
    pub fleet_deaths_per_hour: f64,
    /// Total deaths across all clients this session.
    pub fleet_deaths_total: usize,
    /// Total kills across all clients this session.
    pub fleet_kills_total: usize,
    /// Per-client snapshots ordered by PID.
    pub clients: Vec<ClientProgressSnapshot>,
}

impl ProgressReport {
    /// Build a fleet report from a collection of per-client snapshots.
    #[must_use]
    pub fn from_snapshots(snapshots: Vec<ClientProgressSnapshot>) -> Self {
        let total_clients = snapshots.len();
        let active_clients = snapshots.iter().filter(|c| c.in_combat).count();

        // XP/hour average — only include clients with non-zero rate.
        let xp_rates: Vec<f32> = snapshots
            .iter()
            .map(|c| c.xp_per_hour)
            .filter(|&r| r > 0.0)
            .collect();
        let fleet_xp_per_hour_avg = if xp_rates.is_empty() {
            0.0
        } else {
            xp_rates.iter().sum::<f32>() / xp_rates.len() as f32
        };

        let fleet_kills_per_hour = snapshots.iter().map(|c| c.kills_per_hour()).sum();
        let fleet_loot_rate_per_hour = snapshots.iter().map(|c| c.loot_rate_per_hour()).sum();
        let fleet_deaths_per_hour = snapshots.iter().map(|c| c.deaths_per_hour()).sum();
        let fleet_deaths_total = snapshots.iter().map(|c| c.deaths_session).sum();
        let fleet_kills_total = snapshots.iter().map(|c| c.kills_session).sum();

        Self {
            captured_at: Instant::now(),
            total_clients,
            active_clients,
            fleet_xp_per_hour_avg,
            fleet_kills_per_hour,
            fleet_loot_rate_per_hour,
            fleet_deaths_per_hour,
            fleet_deaths_total,
            fleet_kills_total,
            clients: snapshots,
        }
    }

    /// Elapsed time since this report was captured.
    #[must_use]
    pub fn age(&self) -> Duration {
        self.captured_at.elapsed()
    }

    /// Camp uptime fraction: active_clients / total_clients (0.0–1.0).
    #[must_use]
    pub fn camp_uptime_ratio(&self) -> f64 {
        if self.total_clients == 0 {
            return 0.0;
        }
        self.active_clients as f64 / self.total_clients as f64
    }
}

// ── Progress tracker ──────────────────────────────────────────────────────────

/// Tracks per-client session counters for progress reporting.
///
/// The orchestrator loop holds one instance and calls `record_kill`,
/// `record_loot`, `record_death`, and `register_client` as events arrive.
/// `build_snapshots` is called on each reporting interval.
pub struct ProgressTracker {
    clients: std::collections::HashMap<u32, ClientSessionCounters>,
}

#[derive(Debug, Default)]
struct ClientSessionCounters {
    character_name: String,
    zone: String,
    kills: usize,
    items_looted: u32,
    deaths: usize,
    registered_at: Option<Instant>,
    in_combat: bool,
    xp_per_hour: f32,
    aa_xp_per_hour: f32,
}

impl ProgressTracker {
    /// Create an empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            clients: std::collections::HashMap::new(),
        }
    }

    /// Register a newly-launched client.
    pub fn register_client(&mut self, pid: u32, character_name: impl Into<String>) {
        let entry = self.clients.entry(pid).or_default();
        entry.character_name = character_name.into();
        if entry.registered_at.is_none() {
            entry.registered_at = Some(Instant::now());
        }
    }

    /// Remove a client that has disconnected or been camped.
    pub fn remove_client(&mut self, pid: u32) {
        self.clients.remove(&pid);
    }

    /// Update the character name and zone for an already-registered client.
    pub fn update_client_meta(
        &mut self,
        pid: u32,
        character_name: impl Into<String>,
        zone: impl Into<String>,
        in_combat: bool,
    ) {
        let entry = self.clients.entry(pid).or_default();
        entry.character_name = character_name.into();
        entry.zone = zone.into();
        entry.in_combat = in_combat;
    }

    /// Update XP rates for a client (sourced from `XpTracker`).
    pub fn update_xp_rates(&mut self, pid: u32, xp_per_hour: f32, aa_xp_per_hour: f32) {
        let entry = self.clients.entry(pid).or_default();
        entry.xp_per_hour = xp_per_hour;
        entry.aa_xp_per_hour = aa_xp_per_hour;
    }

    /// Record a kill event for a client.
    pub fn record_kill(&mut self, pid: u32) {
        self.clients.entry(pid).or_default().kills += 1;
    }

    /// Record looted items for a client.
    pub fn record_loot(&mut self, pid: u32, items: u32) {
        self.clients.entry(pid).or_default().items_looted += items;
    }

    /// Record a death event for a client.
    pub fn record_death(&mut self, pid: u32) {
        self.clients.entry(pid).or_default().deaths += 1;
    }

    /// Build per-client snapshots for the current moment.
    #[must_use]
    pub fn build_snapshots(&self) -> Vec<ClientProgressSnapshot> {
        let mut snapshots: Vec<ClientProgressSnapshot> = self
            .clients
            .iter()
            .map(|(&pid, counters)| {
                let uptime_secs = counters
                    .registered_at
                    .map(|t| t.elapsed().as_secs())
                    .unwrap_or(0);

                ClientProgressSnapshot {
                    pid,
                    character_name: counters.character_name.clone(),
                    zone: counters.zone.clone(),
                    xp_per_hour: counters.xp_per_hour,
                    aa_xp_per_hour: counters.aa_xp_per_hour,
                    kills_session: counters.kills,
                    items_looted_session: counters.items_looted,
                    deaths_session: counters.deaths,
                    in_combat: counters.in_combat,
                    uptime_secs,
                }
            })
            .collect();

        snapshots.sort_by_key(|s| s.pid);
        snapshots
    }

    /// Build a full `ProgressReport` from current state.
    #[must_use]
    pub fn build_report(&self) -> ProgressReport {
        ProgressReport::from_snapshots(self.build_snapshots())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ClientProgressSnapshot ────────────────────────────────────────────────

    #[test]
    fn snapshot_derived_rates_zero_uptime() {
        let snap = ClientProgressSnapshot {
            pid: 1,
            character_name: "Warrior".into(),
            zone: "gfaydark".into(),
            xp_per_hour: 5.0,
            aa_xp_per_hour: 0.0,
            kills_session: 100,
            items_looted_session: 50,
            deaths_session: 2,
            in_combat: false,
            uptime_secs: 0,
        };
        assert_eq!(snap.kills_per_hour(), 0.0);
        assert_eq!(snap.loot_rate_per_hour(), 0.0);
        assert_eq!(snap.deaths_per_hour(), 0.0);
    }

    #[test]
    fn snapshot_kills_per_hour_basic() {
        let snap = ClientProgressSnapshot {
            pid: 2,
            character_name: "Rogue".into(),
            zone: "kithicor".into(),
            xp_per_hour: 0.0,
            aa_xp_per_hour: 0.0,
            kills_session: 60,
            items_looted_session: 30,
            deaths_session: 1,
            in_combat: true,
            uptime_secs: 3600,
        };
        // 60 kills / 1 hour = 60.0
        let diff = (snap.kills_per_hour() - 60.0).abs();
        assert!(diff < 0.001, "expected ~60.0, got {}", snap.kills_per_hour());
    }

    #[test]
    fn snapshot_deaths_per_hour() {
        let snap = ClientProgressSnapshot {
            pid: 3,
            character_name: "Cleric".into(),
            zone: "nro".into(),
            xp_per_hour: 0.0,
            aa_xp_per_hour: 0.0,
            kills_session: 0,
            items_looted_session: 0,
            deaths_session: 2,
            in_combat: false,
            uptime_secs: 1800, // 30 minutes
        };
        // 2 deaths / 0.5 hours = 4 deaths/hour
        let diff = (snap.deaths_per_hour() - 4.0).abs();
        assert!(diff < 0.001, "expected ~4.0, got {}", snap.deaths_per_hour());
    }

    // ── ProgressReport ────────────────────────────────────────────────────────

    fn make_snap(pid: u32, kills: usize, deaths: usize, xp_hr: f32, in_combat: bool) -> ClientProgressSnapshot {
        ClientProgressSnapshot {
            pid,
            character_name: format!("Char{pid}"),
            zone: "test".into(),
            xp_per_hour: xp_hr,
            aa_xp_per_hour: 0.0,
            kills_session: kills,
            items_looted_session: kills as u32,
            deaths_session: deaths,
            in_combat,
            uptime_secs: 3600,
        }
    }

    #[test]
    fn report_from_empty_snapshots() {
        let report = ProgressReport::from_snapshots(vec![]);
        assert_eq!(report.total_clients, 0);
        assert_eq!(report.active_clients, 0);
        assert_eq!(report.fleet_xp_per_hour_avg, 0.0);
        assert_eq!(report.fleet_kills_per_hour, 0.0);
        assert_eq!(report.camp_uptime_ratio(), 0.0);
    }

    #[test]
    fn report_fleet_aggregates() {
        let snaps = vec![
            make_snap(1, 60, 0, 10.0, true),
            make_snap(2, 30, 2, 20.0, false),
        ];
        let report = ProgressReport::from_snapshots(snaps);
        assert_eq!(report.total_clients, 2);
        assert_eq!(report.active_clients, 1);

        // XP avg = (10 + 20) / 2 = 15.0
        let diff = (report.fleet_xp_per_hour_avg - 15.0).abs();
        assert!(diff < 0.01, "expected ~15.0 avg xp/hr, got {}", report.fleet_xp_per_hour_avg);

        // kills/hr: (60 + 30) / 1 hr each = 90.0
        let diff2 = (report.fleet_kills_per_hour - 90.0).abs();
        assert!(diff2 < 0.01, "expected ~90.0 kills/hr, got {}", report.fleet_kills_per_hour);

        assert_eq!(report.fleet_deaths_total, 2);
        assert_eq!(report.fleet_kills_total, 90);
    }

    #[test]
    fn report_camp_uptime_ratio() {
        let snaps = vec![
            make_snap(1, 0, 0, 0.0, true),
            make_snap(2, 0, 0, 0.0, true),
            make_snap(3, 0, 0, 0.0, false),
        ];
        let report = ProgressReport::from_snapshots(snaps);
        let ratio = report.camp_uptime_ratio();
        // 2 active / 3 total
        let diff = (ratio - 2.0 / 3.0).abs();
        assert!(diff < 0.001, "expected ~0.667, got {ratio}");
    }

    #[test]
    fn report_xp_avg_skips_zero_rates() {
        let snaps = vec![
            make_snap(1, 0, 0, 0.0, false), // zero XP — excluded from avg
            make_snap(2, 0, 0, 30.0, true),
        ];
        let report = ProgressReport::from_snapshots(snaps);
        // Only one non-zero rate (30.0)
        let diff = (report.fleet_xp_per_hour_avg - 30.0).abs();
        assert!(diff < 0.01, "expected ~30.0 avg xp/hr, got {}", report.fleet_xp_per_hour_avg);
    }

    // ── ProgressTracker ───────────────────────────────────────────────────────

    #[test]
    fn tracker_register_and_snapshot() {
        let mut tracker = ProgressTracker::new();
        tracker.register_client(42, "Cleric");
        let snaps = tracker.build_snapshots();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].pid, 42);
        assert_eq!(snaps[0].character_name, "Cleric");
    }

    #[test]
    fn tracker_record_events() {
        let mut tracker = ProgressTracker::new();
        tracker.register_client(1, "Warrior");
        tracker.record_kill(1);
        tracker.record_kill(1);
        tracker.record_loot(1, 5);
        tracker.record_death(1);

        let snaps = tracker.build_snapshots();
        assert_eq!(snaps[0].kills_session, 2);
        assert_eq!(snaps[0].items_looted_session, 5);
        assert_eq!(snaps[0].deaths_session, 1);
    }

    #[test]
    fn tracker_remove_client() {
        let mut tracker = ProgressTracker::new();
        tracker.register_client(10, "Mage");
        tracker.remove_client(10);
        assert!(tracker.build_snapshots().is_empty());
    }

    #[test]
    fn tracker_update_xp_rates() {
        let mut tracker = ProgressTracker::new();
        tracker.register_client(5, "Druid");
        tracker.update_xp_rates(5, 12.5, 3.0);
        let snaps = tracker.build_snapshots();
        let diff = (snaps[0].xp_per_hour - 12.5).abs();
        assert!(diff < 0.001, "expected 12.5 xp/hr, got {}", snaps[0].xp_per_hour);
        let diff2 = (snaps[0].aa_xp_per_hour - 3.0).abs();
        assert!(diff2 < 0.001, "expected 3.0 aa xp/hr, got {}", snaps[0].aa_xp_per_hour);
    }

    #[test]
    fn tracker_snapshots_sorted_by_pid() {
        let mut tracker = ProgressTracker::new();
        tracker.register_client(30, "Z");
        tracker.register_client(10, "A");
        tracker.register_client(20, "M");
        let snaps = tracker.build_snapshots();
        assert_eq!(snaps[0].pid, 10);
        assert_eq!(snaps[1].pid, 20);
        assert_eq!(snaps[2].pid, 30);
    }

    #[test]
    fn tracker_build_report_roundtrip() {
        let mut tracker = ProgressTracker::new();
        tracker.register_client(1, "Shaman");
        tracker.record_kill(1);
        let report = tracker.build_report();
        assert_eq!(report.total_clients, 1);
        assert_eq!(report.fleet_kills_total, 1);
    }

    #[test]
    fn tracker_interval_compliance() {
        // Simulate interval compliance: two build_report calls within 100ms of each other.
        let mut tracker = ProgressTracker::new();
        tracker.register_client(1, "Necro");
        let r1 = tracker.build_report();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let r2 = tracker.build_report();
        // Both reports should be fresh (age < 1 second)
        assert!(r1.age() < std::time::Duration::from_secs(1));
        assert!(r2.age() < std::time::Duration::from_secs(1));
        // r2 was captured after r1
        assert!(r2.captured_at >= r1.captured_at);
    }
}
