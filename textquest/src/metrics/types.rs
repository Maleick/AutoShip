//! Real-time metrics data structures for performance monitoring.
//!
//! Defines typed snapshots for combat, movement, loot, system, and fleet
//! telemetry. These are the canonical transfer objects that flow from
//! collectors to dashboards, Discord reporters, and the performance store.
//!
//! # Structure hierarchy
//!
//! - [`CharacterMetrics`] — per-character snapshot bundling combat, movement,
//!   and loot sub-metrics.
//! - [`FleetMetrics`] — aggregate view across all active clients.
//! - [`SystemMetrics`] — DLL/host process health indicators.
//! - [`TimeWindowedMetrics`] — wraps any metric snapshot with a time window
//!   label for historical comparison.

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

// ── Combat ────────────────────────────────────────────────────────────────────

/// Per-character combat performance snapshot.
///
/// All rate fields (`dps`, `tps`, `cast_rate`, …) are computed over the
/// active combat duration of the current session window; they reset to zero
/// outside of combat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombatMetrics {
    /// Damage per second averaged over the encounter.
    pub dps: f64,
    /// Threat per second (for tanks / off-tanks).
    pub tps: f64,
    /// Spell/ability casts per minute.
    pub cast_rate: f64,
    /// Fraction of attacks that missed (0.0–1.0).
    pub miss_rate: f64,
    /// Fraction of spells that were resisted (0.0–1.0).
    pub resist_rate: f64,
    /// Fraction of attacks that critically hit (0.0–1.0).
    pub crit_rate: f64,
    /// Total seconds spent in active combat this session.
    pub combat_duration_secs: u32,
    /// Number of discrete encounters (kill events) this session.
    pub encounter_count: u32,
}

impl CombatMetrics {
    /// Returns a zero-value instance suitable as a starting baseline.
    pub fn zeroed() -> Self {
        Self {
            dps: 0.0,
            tps: 0.0,
            cast_rate: 0.0,
            miss_rate: 0.0,
            resist_rate: 0.0,
            crit_rate: 0.0,
            combat_duration_secs: 0,
            encounter_count: 0,
        }
    }

    /// DPS per encounter — useful for comparing pull efficiency.
    pub fn dps_per_encounter(&self) -> f64 {
        if self.encounter_count == 0 {
            return 0.0;
        }
        self.dps / self.encounter_count as f64
    }
}

// ── Movement ──────────────────────────────────────────────────────────────────

/// Per-character movement and navigation quality snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MovementMetrics {
    /// Fraction of navigation attempts that reached the destination (0.0–1.0).
    pub success_rate: f64,
    /// Number of times the character was detected as stuck this session.
    pub stuck_count: u32,
    /// Average movement speed in game units per second.
    pub avg_speed: f64,
    /// Total game-unit distance traveled this session.
    pub distance_traveled: f64,
    /// Number of successful zone transitions this session.
    pub zone_changes: u32,
}

impl MovementMetrics {
    /// Returns a zero-value instance suitable as a starting baseline.
    pub fn zeroed() -> Self {
        Self {
            success_rate: 0.0,
            stuck_count: 0,
            avg_speed: 0.0,
            distance_traveled: 0.0,
            zone_changes: 0,
        }
    }

    /// Stuck events per zone change — higher values indicate unstable routing.
    pub fn stuck_rate_per_zone(&self) -> f64 {
        if self.zone_changes == 0 {
            return self.stuck_count as f64;
        }
        self.stuck_count as f64 / self.zone_changes as f64
    }
}

// ── Loot ─────────────────────────────────────────────────────────────────────

/// Per-character loot economy snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LootMetrics {
    /// Total items looted this session.
    pub items_looted: u32,
    /// Total platinum (base currency) collected, including vendor proceeds.
    pub plat_collected: u64,
    /// Items looted per hour (rolling rate over the session).
    pub loot_rate_per_hour: f64,
    /// Items identified as high-value (above configured threshold).
    pub valuable_items: u32,
    /// Items looted and sold as vendor trash.
    pub vendor_trash: u32,
}

impl LootMetrics {
    /// Returns a zero-value instance suitable as a starting baseline.
    pub fn zeroed() -> Self {
        Self {
            items_looted: 0,
            plat_collected: 0,
            loot_rate_per_hour: 0.0,
            valuable_items: 0,
            vendor_trash: 0,
        }
    }

    /// Fraction of looted items that were valuable (0.0–1.0).
    pub fn valuable_ratio(&self) -> f64 {
        if self.items_looted == 0 {
            return 0.0;
        }
        self.valuable_items as f64 / self.items_looted as f64
    }
}

// ── System ────────────────────────────────────────────────────────────────────

/// DLL / host process health snapshot.
///
/// Tracks resource utilisation of the injected DLL and the IPC channel so the
/// orchestrator can detect runaway threads or message-queue back-pressure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// CPU utilisation of the DLL thread as a percentage (0.0–100.0).
    pub dll_cpu_percent: f64,
    /// Resident memory used by the DLL thread in megabytes.
    pub memory_usage_mb: f64,
    /// IPC messages dispatched per second (outbound).
    pub ipc_messages_per_sec: f64,
    /// Average time between game frames as seen by the DLL hook (milliseconds).
    pub frame_latency_ms: f64,
    /// Total seconds the DLL has been loaded and running.
    pub uptime_secs: u64,
}

impl SystemMetrics {
    /// Returns a zero-value instance suitable as a starting baseline.
    pub fn zeroed() -> Self {
        Self {
            dll_cpu_percent: 0.0,
            memory_usage_mb: 0.0,
            ipc_messages_per_sec: 0.0,
            frame_latency_ms: 0.0,
            uptime_secs: 0,
        }
    }
}

// ── Character ─────────────────────────────────────────────────────────────────

/// Complete per-character metric snapshot at a point in time.
///
/// Bundles all sub-metric categories with a wall-clock timestamp so snapshots
/// can be ordered and compared across time windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterMetrics {
    /// EverQuest character name (case-sensitive, matches in-game name).
    pub character_name: String,
    /// Combat performance for this character.
    pub combat: CombatMetrics,
    /// Movement/navigation quality for this character.
    pub movement: MovementMetrics,
    /// Loot economy output for this character.
    pub loot: LootMetrics,
    /// Wall-clock time when this snapshot was captured.
    pub timestamp: SystemTime,
}

impl CharacterMetrics {
    /// Create a new zeroed snapshot for the given character name.
    pub fn new(character_name: impl Into<String>) -> Self {
        Self {
            character_name: character_name.into(),
            combat: CombatMetrics::zeroed(),
            movement: MovementMetrics::zeroed(),
            loot: LootMetrics::zeroed(),
            timestamp: SystemTime::now(),
        }
    }
}

// ── Fleet ─────────────────────────────────────────────────────────────────────

/// Aggregate metric snapshot across all active fleet members.
///
/// Provides a fleet-level view for the TUI dashboard header and Discord
/// status reports without requiring iteration over individual characters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetMetrics {
    /// Sum of DPS across all active combat members.
    pub total_dps: f64,
    /// Total connected clients (including idle/OOC).
    pub member_count: usize,
    /// Clients currently in active combat.
    pub active_members: usize,
    /// Current zone shared by the majority of the fleet.
    pub zone: String,
    /// `true` if at least one member is in active combat.
    pub combat_active: bool,
}

impl FleetMetrics {
    /// Returns a zeroed fleet snapshot.
    pub fn zeroed() -> Self {
        Self {
            total_dps: 0.0,
            member_count: 0,
            active_members: 0,
            zone: String::new(),
            combat_active: false,
        }
    }

    /// DPS per active member — useful for per-capita efficiency analysis.
    pub fn dps_per_member(&self) -> f64 {
        if self.active_members == 0 {
            return 0.0;
        }
        self.total_dps / self.active_members as f64
    }

    /// Fraction of fleet members currently in combat (0.0–1.0).
    pub fn combat_ratio(&self) -> f64 {
        if self.member_count == 0 {
            return 0.0;
        }
        self.active_members as f64 / self.member_count as f64
    }
}

// ── Time-windowed wrapper ─────────────────────────────────────────────────────

/// A labelled time window used to tag metric snapshots for historical buckets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricWindow {
    /// Last 60 seconds.
    OneMin,
    /// Last 5 minutes.
    FiveMin,
    /// Last 60 minutes.
    SixtyMin,
    /// Full session (since DLL load or orchestrator start).
    Session,
}

impl MetricWindow {
    /// Duration in seconds, or `None` for the unbounded `Session` window.
    pub fn duration_secs(&self) -> Option<u64> {
        match self {
            Self::OneMin => Some(60),
            Self::FiveMin => Some(300),
            Self::SixtyMin => Some(3600),
            Self::Session => None,
        }
    }
}

impl std::fmt::Display for MetricWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OneMin => write!(f, "1m"),
            Self::FiveMin => write!(f, "5m"),
            Self::SixtyMin => write!(f, "60m"),
            Self::Session => write!(f, "session"),
        }
    }
}

/// A metric snapshot associated with a specific time window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindowedMetrics<T> {
    /// The time bucket this snapshot covers.
    pub window: MetricWindow,
    /// The actual metric data.
    pub metrics: T,
    /// Wall-clock time when this snapshot was captured.
    pub captured_at: SystemTime,
}

impl<T> TimeWindowedMetrics<T> {
    /// Wrap a metric snapshot with a window label, stamped at `now`.
    pub fn new(window: MetricWindow, metrics: T) -> Self {
        Self {
            window,
            metrics,
            captured_at: SystemTime::now(),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── CombatMetrics ─────────────────────────────────────────────────────────

    #[test]
    fn combat_metrics_zeroed_is_zero() {
        let m = CombatMetrics::zeroed();
        assert_eq!(m.dps, 0.0);
        assert_eq!(m.encounter_count, 0);
    }

    #[test]
    fn combat_metrics_example_values() {
        let m = CombatMetrics {
            dps: 1200.0,
            tps: 800.0,
            cast_rate: 12.5,
            miss_rate: 0.05,
            resist_rate: 0.02,
            crit_rate: 0.15,
            combat_duration_secs: 300,
            encounter_count: 20,
        };
        assert!((m.dps_per_encounter() - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn combat_metrics_dps_per_encounter_zero_encounters() {
        let m = CombatMetrics::zeroed();
        assert_eq!(m.dps_per_encounter(), 0.0);
    }

    // ── MovementMetrics ───────────────────────────────────────────────────────

    #[test]
    fn movement_metrics_zeroed_is_zero() {
        let m = MovementMetrics::zeroed();
        assert_eq!(m.stuck_count, 0);
        assert_eq!(m.distance_traveled, 0.0);
    }

    #[test]
    fn movement_metrics_example_values() {
        let m = MovementMetrics {
            success_rate: 0.97,
            stuck_count: 3,
            avg_speed: 2.8,
            distance_traveled: 15_000.0,
            zone_changes: 6,
        };
        // 3 stucks / 6 zones = 0.5
        assert!((m.stuck_rate_per_zone() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn movement_metrics_stuck_rate_no_zone_changes() {
        let m = MovementMetrics {
            stuck_count: 4,
            zone_changes: 0,
            ..MovementMetrics::zeroed()
        };
        // returns raw stuck_count when no zone changes recorded
        assert_eq!(m.stuck_rate_per_zone(), 4.0);
    }

    // ── LootMetrics ───────────────────────────────────────────────────────────

    #[test]
    fn loot_metrics_zeroed_is_zero() {
        let m = LootMetrics::zeroed();
        assert_eq!(m.items_looted, 0);
        assert_eq!(m.plat_collected, 0);
    }

    #[test]
    fn loot_metrics_example_values() {
        let m = LootMetrics {
            items_looted: 100,
            plat_collected: 5_000,
            loot_rate_per_hour: 120.0,
            valuable_items: 15,
            vendor_trash: 85,
        };
        assert!((m.valuable_ratio() - 0.15).abs() < f64::EPSILON);
    }

    #[test]
    fn loot_metrics_valuable_ratio_zero_items() {
        let m = LootMetrics::zeroed();
        assert_eq!(m.valuable_ratio(), 0.0);
    }

    // ── SystemMetrics ─────────────────────────────────────────────────────────

    #[test]
    fn system_metrics_zeroed_is_zero() {
        let m = SystemMetrics::zeroed();
        assert_eq!(m.dll_cpu_percent, 0.0);
        assert_eq!(m.uptime_secs, 0);
    }

    #[test]
    fn system_metrics_example_values() {
        let m = SystemMetrics {
            dll_cpu_percent: 1.2,
            memory_usage_mb: 48.5,
            ipc_messages_per_sec: 250.0,
            frame_latency_ms: 16.7,
            uptime_secs: 7200,
        };
        assert!(m.dll_cpu_percent < 5.0, "DLL should have low CPU overhead");
        assert!(m.frame_latency_ms < 34.0, "frame latency should be <2 frames at 60 Hz");
    }

    // ── CharacterMetrics ──────────────────────────────────────────────────────

    #[test]
    fn character_metrics_new_sets_name() {
        let m = CharacterMetrics::new("Alyssa");
        assert_eq!(m.character_name, "Alyssa");
        assert_eq!(m.combat.dps, 0.0);
        assert_eq!(m.loot.items_looted, 0);
    }

    // ── FleetMetrics ──────────────────────────────────────────────────────────

    #[test]
    fn fleet_metrics_zeroed_is_zero() {
        let m = FleetMetrics::zeroed();
        assert_eq!(m.member_count, 0);
        assert!(!m.combat_active);
    }

    #[test]
    fn fleet_metrics_example_values() {
        let m = FleetMetrics {
            total_dps: 36_000.0,
            member_count: 36,
            active_members: 30,
            zone: "The Plane of Fear".to_owned(),
            combat_active: true,
        };
        assert!((m.dps_per_member() - 1_200.0).abs() < f64::EPSILON);
        assert!((m.combat_ratio() - 30.0 / 36.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fleet_metrics_dps_per_member_no_active() {
        let m = FleetMetrics::zeroed();
        assert_eq!(m.dps_per_member(), 0.0);
    }

    // ── MetricWindow ──────────────────────────────────────────────────────────

    #[test]
    fn metric_window_duration_secs() {
        assert_eq!(MetricWindow::OneMin.duration_secs(), Some(60));
        assert_eq!(MetricWindow::FiveMin.duration_secs(), Some(300));
        assert_eq!(MetricWindow::SixtyMin.duration_secs(), Some(3600));
        assert_eq!(MetricWindow::Session.duration_secs(), None);
    }

    #[test]
    fn metric_window_display() {
        assert_eq!(MetricWindow::OneMin.to_string(), "1m");
        assert_eq!(MetricWindow::SixtyMin.to_string(), "60m");
        assert_eq!(MetricWindow::Session.to_string(), "session");
    }

    // ── TimeWindowedMetrics ───────────────────────────────────────────────────

    #[test]
    fn time_windowed_metrics_wraps_correctly() {
        let fleet = FleetMetrics {
            total_dps: 5_000.0,
            member_count: 6,
            active_members: 6,
            zone: "Kithicor Forest".to_owned(),
            combat_active: true,
        };
        let tw = TimeWindowedMetrics::new(MetricWindow::FiveMin, fleet.clone());
        assert_eq!(tw.window, MetricWindow::FiveMin);
        assert_eq!(tw.metrics.total_dps, fleet.total_dps);
    }

    // ── Serde round-trip ──────────────────────────────────────────────────────

    #[test]
    fn combat_metrics_serde_round_trip() {
        let original = CombatMetrics {
            dps: 900.0,
            tps: 600.0,
            cast_rate: 8.0,
            miss_rate: 0.1,
            resist_rate: 0.03,
            crit_rate: 0.12,
            combat_duration_secs: 120,
            encounter_count: 10,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: CombatMetrics = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn fleet_metrics_serde_round_trip() {
        let original = FleetMetrics {
            total_dps: 18_000.0,
            member_count: 18,
            active_members: 18,
            zone: "Nagafen's Lair".to_owned(),
            combat_active: true,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: FleetMetrics = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, deserialized);
    }
}
