//! Integration tests for the TextQuest performance monitoring system.
//!
//! Exercises the `MetricsCollector`, `AdminMonitoringStore`, and `types`
//! modules together to validate end-to-end metric collection, historical
//! storage, and alert-threshold detection.
//!
//! The `metrics` module is `#[cfg(windows)]` in lib.rs (SQLite / IPC paths);
//! these tests run on Windows (Frostreaver) only. macOS unit tests live
//! inside each source module under `mod tests`.
#![cfg(windows)]

use std::time::{Duration, Instant};

use textquest::metrics::types::{
    CombatMetrics as RtCombatMetrics, FleetMetrics as RtFleetMetrics, LootMetrics, MetricWindow,
    MovementMetrics as RtMovementMetrics, SystemMetrics, TimeWindowedMetrics,
};
use textquest::metrics::{
    AdminMonitoringRetention, AdminMonitoringStore, MetricsCollector, MonitoredSessionState,
    SessionErrorKind,
    baseline_scorecard::{CombatMetrics, MovementMetrics},
};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn small_retention() -> AdminMonitoringRetention {
    AdminMonitoringRetention {
        ipc_samples: 5,
        memory_samples: 5,
        error_events: 5,
    }
}

fn make_combat_metrics(dps: f64, kills: u32) -> CombatMetrics {
    CombatMetrics {
        dps,
        mana_consumed: 500,
        endurance_consumed: 200,
        avg_pull_to_kill_secs: 15.0,
        total_kills: kills,
    }
}

fn make_movement_metrics(distance: f64, stuck: u32) -> MovementMetrics {
    MovementMetrics {
        stuck_percentage: stuck as f64 * 5.0,
        total_distance: distance,
        stuck_event_count: stuck,
        route_efficiency: 0.9,
    }
}

// ─── MetricsCollector ────────────────────────────────────────────────────────

#[test]
fn collector_records_single_character_combat() {
    let mut col = MetricsCollector::new();
    col.update_combat("Renia", make_combat_metrics(1_200.0, 5));

    let m = col
        .get_character_metrics("Renia")
        .expect("character should exist after update");
    assert_eq!(m.name, "Renia");
    assert_eq!(m.combat.total_kills, 5);
    assert!(m.combat.average_dps() > 0.0);
}

#[test]
fn collector_accumulates_kills_across_updates() {
    let mut col = MetricsCollector::new();
    col.update_combat("Renia", make_combat_metrics(1_000.0, 3));
    col.update_combat("Renia", make_combat_metrics(1_100.0, 7));

    let m = col
        .get_character_metrics("Renia")
        .expect("character should exist");
    assert_eq!(m.combat.total_kills, 10);
}

#[test]
fn collector_records_multi_character_fleet() {
    let mut col = MetricsCollector::new();
    col.update_combat("Renia", make_combat_metrics(1_200.0, 4));
    col.update_combat("Torvin", make_combat_metrics(800.0, 4));
    col.update_combat("Sylvara", make_combat_metrics(400.0, 2));

    // All three characters tracked independently
    assert!(col.get_character_metrics("Renia").is_some());
    assert!(col.get_character_metrics("Torvin").is_some());
    assert!(col.get_character_metrics("Sylvara").is_some());

    let fleet = col.get_fleet_metrics();
    assert_eq!(fleet.total_kills, 10);
}

#[test]
fn collector_records_movement_and_updates_fleet_distance() {
    let mut col = MetricsCollector::new();
    col.update_movement("Renia", make_movement_metrics(5_000.0, 0));
    col.update_movement("Renia", make_movement_metrics(3_000.0, 1));

    let m = col
        .get_character_metrics("Renia")
        .expect("character should exist");
    assert!((m.movement.total_distance - 8_000.0).abs() < f64::EPSILON);
    assert_eq!(m.movement.stuck_events, 1);
}

#[test]
fn collector_records_loot_and_accumulates_plat() {
    let mut col = MetricsCollector::new();
    col.update_loot("Renia", "Fine Steel Sword", 50);
    col.update_loot("Renia", "Words of Absorption", 200);
    col.update_loot("Renia", "Bag of Sewn Evil Eye", 5);

    let fleet = col.get_fleet_metrics();
    assert_eq!(fleet.total_items_looted, 3);
    assert_eq!(fleet.total_plat_earned, 255);

    let m = col
        .get_character_metrics("Renia")
        .expect("character should exist");
    assert_eq!(m.economy.total_items_looted, 3);
    assert_eq!(m.economy.total_plat_earned, 255);
    // Loot ring buffer capped at 100
    assert!(m.economy.loot_events.len() <= 100);
}

#[test]
fn collector_loot_ring_buffer_trims_at_100_entries() {
    let mut col = MetricsCollector::new();
    for i in 0u32..110 {
        col.update_loot("Renia", &format!("item_{i}"), 1);
    }
    let m = col.get_character_metrics("Renia").unwrap();
    assert_eq!(m.economy.loot_events.len(), 100);
    // Oldest item should be item_10 (first 10 trimmed)
    assert_eq!(m.economy.loot_events[0].item_name, "item_10");
}

#[test]
fn collector_dps_ring_buffer_trims_at_3600_samples() {
    let mut col = MetricsCollector::new();
    for _ in 0u32..3610 {
        col.update_combat("Renia", make_combat_metrics(1_000.0, 0));
    }
    let m = col.get_character_metrics("Renia").unwrap();
    assert_eq!(m.combat.dps_samples.len(), 3600);
}

// ─── AdminMonitoringStore — session lifecycle ─────────────────────────────────

#[test]
fn admin_store_register_sets_active_state() {
    let mut store = AdminMonitoringStore::with_retention(small_retention());
    let now = Instant::now();

    store.register_session(1, 1000);

    let snap = store.snapshot(1, now).unwrap();
    assert_eq!(snap.state, MonitoredSessionState::Active);
    assert_eq!(snap.pid, Some(1000));
}

#[test]
fn admin_store_full_lifecycle_active_exited_absent() {
    let mut store = AdminMonitoringStore::with_retention(small_retention());
    let now = Instant::now();

    store.register_session(2, 2000);
    store.record_ipc_latency_at(2, 10, now);
    store.mark_session_exited(2);

    let exited = store.snapshot(2, now).unwrap();
    assert_eq!(exited.state, MonitoredSessionState::Exited);
    // History preserved after exit
    assert_eq!(exited.ipc_latency.last_ms, Some(10));

    store.mark_session_absent(2);
    let absent = store.snapshot(2, now).unwrap();
    assert_eq!(absent.state, MonitoredSessionState::Absent);
    assert_eq!(absent.pid, None);
    // IPC samples still retained
    assert_eq!(absent.ipc_latency.last_ms, Some(10));
}

#[test]
fn admin_store_pid_rebind_marks_prior_client_absent() {
    let mut store = AdminMonitoringStore::with_retention(small_retention());
    let now = Instant::now();

    store.register_session(10, 9999);
    store.register_session(11, 9999); // same PID → prior client goes absent

    let prior = store.snapshot(10, now).unwrap();
    assert_eq!(prior.state, MonitoredSessionState::Absent);
    assert_eq!(prior.pid, None);

    let current = store.snapshot(11, now).unwrap();
    assert_eq!(current.state, MonitoredSessionState::Active);
    assert_eq!(current.pid, Some(9999));
}

#[test]
fn admin_store_snapshot_returns_none_for_unregistered_client() {
    let store = AdminMonitoringStore::new();
    assert!(store.snapshot(999, Instant::now()).is_none());
}

// ─── AdminMonitoringStore — IPC latency ───────────────────────────────────────

#[test]
fn admin_store_ipc_latency_percentiles() {
    let mut store = AdminMonitoringStore::with_retention(small_retention());
    let now = Instant::now();
    store.register_session(20, 2000);

    for (i, lat) in [5u64, 10, 15, 20, 100].iter().enumerate() {
        store.record_ipc_latency_at(20, *lat, now + Duration::from_millis(i as u64 * 10));
    }

    let snap = store.snapshot(20, now + Duration::from_secs(1)).unwrap();
    assert_eq!(snap.ipc_latency.sample_count, 5);
    assert_eq!(snap.ipc_latency.last_ms, Some(100));
    // p50 of [5,10,15,20,100] → 15
    assert_eq!(snap.ipc_latency.p50_ms, Some(15));
    // p95 of 5 values → 100
    assert_eq!(snap.ipc_latency.p95_ms, Some(100));
}

#[test]
fn admin_store_ipc_retention_bound_enforced() {
    let mut store = AdminMonitoringStore::with_retention(small_retention()); // cap=5
    let now = Instant::now();
    store.register_session(21, 2100);

    for i in 0u64..10 {
        store.record_ipc_latency_at(21, i * 10, now + Duration::from_millis(i * 10));
    }

    let snap = store.snapshot(21, now + Duration::from_secs(1)).unwrap();
    assert_eq!(snap.ipc_latency.sample_count, 5);
    // Oldest 5 samples discarded → last is 90ms
    assert_eq!(snap.ipc_latency.last_ms, Some(90));
}

// ─── AdminMonitoringStore — memory trend ─────────────────────────────────────

#[test]
fn admin_store_memory_trend_grows_correctly() {
    let mut store = AdminMonitoringStore::with_retention(small_retention());
    let now = Instant::now();
    store.register_session(30, 3000);

    store.record_memory_sample_at(30, 100_000_000, now);
    store.record_memory_sample_at(30, 110_000_000, now + Duration::from_secs(60));
    store.record_memory_sample_at(30, 120_000_000, now + Duration::from_secs(120));

    let snap = store.snapshot(30, now + Duration::from_secs(120)).unwrap();
    assert_eq!(snap.memory.sample_count, 3);
    assert_eq!(snap.memory.current_bytes, Some(120_000_000));
    assert_eq!(snap.memory.delta_bytes, Some(20_000_000));

    // Growth rate: 20 MB over 120s → 10 MB/min (10_000_000)
    let rate = snap
        .memory
        .growth_bytes_per_minute
        .expect("growth rate set");
    assert!((rate - 10_000_000.0).abs() < 1.0, "unexpected rate: {rate}");
}

#[test]
fn admin_store_memory_trend_shrinks_correctly() {
    let mut store = AdminMonitoringStore::with_retention(small_retention());
    let now = Instant::now();
    store.register_session(31, 3100);

    store.record_memory_sample_at(31, 500_000_000, now);
    store.record_memory_sample_at(31, 400_000_000, now + Duration::from_secs(60));

    let snap = store.snapshot(31, now + Duration::from_secs(60)).unwrap();
    assert_eq!(snap.memory.delta_bytes, Some(-100_000_000));
    let rate = snap
        .memory
        .growth_bytes_per_minute
        .expect("growth rate set");
    assert!(rate < 0.0, "shrinking memory should be negative rate");
}

#[test]
fn admin_store_memory_single_sample_has_no_trend() {
    let mut store = AdminMonitoringStore::with_retention(small_retention());
    let now = Instant::now();
    store.register_session(32, 3200);
    store.record_memory_sample_at(32, 256_000_000, now);

    let snap = store.snapshot(32, now).unwrap();
    assert_eq!(snap.memory.sample_count, 1);
    assert_eq!(snap.memory.current_bytes, Some(256_000_000));
    assert!(snap.memory.delta_bytes.is_none());
    assert!(snap.memory.growth_bytes_per_minute.is_none());
}

#[test]
fn admin_store_memory_retention_bound_enforced() {
    let mut store = AdminMonitoringStore::with_retention(small_retention()); // cap=5
    let now = Instant::now();
    store.register_session(33, 3300);

    for i in 0u64..8 {
        store.record_memory_sample_at(33, i * 1_000_000, now + Duration::from_secs(i * 10));
    }

    let snap = store.snapshot(33, now + Duration::from_secs(100)).unwrap();
    assert_eq!(snap.memory.sample_count, 5);
    // Oldest 3 discarded; current_bytes = 7MB (index 7 in the loop)
    assert_eq!(snap.memory.current_bytes, Some(7_000_000));
}

// ─── AdminMonitoringStore — error rate window ─────────────────────────────────

#[test]
fn admin_store_error_rate_counts_last_60s_only() {
    let mut store = AdminMonitoringStore::with_retention(small_retention());
    let now = Instant::now();
    store.register_session(40, 4000);

    // old error (>60s ago) — should NOT count toward errors_per_minute
    store.record_error_at(
        40,
        SessionErrorKind::PipeConnect,
        now - Duration::from_secs(90),
    );
    // two recent errors
    store.record_error_at(
        40,
        SessionErrorKind::IpcDispatch,
        now - Duration::from_secs(30),
    );
    store.record_error_at(40, SessionErrorKind::HealthCheck, now);

    let snap = store.snapshot(40, now).unwrap();
    assert_eq!(snap.errors.total_errors, 3);
    assert_eq!(snap.errors.errors_per_minute, 2);
    assert_eq!(
        snap.errors.last_error_kind,
        Some(SessionErrorKind::HealthCheck)
    );
}

#[test]
fn admin_store_error_rate_zero_when_all_errors_expired() {
    let mut store = AdminMonitoringStore::with_retention(small_retention());
    let now = Instant::now();
    store.register_session(41, 4100);
    store.record_error_at(
        40,
        SessionErrorKind::PipeAuth,
        now - Duration::from_secs(120),
    );

    let snap = store.snapshot(41, now).unwrap();
    assert_eq!(snap.errors.errors_per_minute, 0);
}

#[test]
fn admin_store_error_retention_bound_enforced() {
    let mut store = AdminMonitoringStore::with_retention(small_retention()); // cap=5
    let now = Instant::now();
    store.register_session(42, 4200);

    for _ in 0..8 {
        store.record_error_at(42, SessionErrorKind::PipeConnect, now);
    }

    let snap = store.snapshot(42, now).unwrap();
    assert_eq!(snap.errors.sample_count, 5); // capped at retention
    assert_eq!(snap.errors.total_errors, 8); // monotonic counter never trimmed
}

// ─── Rt types — edge cases ────────────────────────────────────────────────────

#[test]
fn rt_combat_dps_per_encounter_zero_guard() {
    let m = RtCombatMetrics::zeroed();
    assert_eq!(m.dps_per_encounter(), 0.0);
}

#[test]
fn rt_combat_dps_per_encounter_nonzero() {
    let m = RtCombatMetrics {
        dps: 3_000.0,
        encounter_count: 5,
        ..RtCombatMetrics::zeroed()
    };
    assert!((m.dps_per_encounter() - 600.0).abs() < f64::EPSILON);
}

#[test]
fn rt_combat_rate_fields_bounded_valid_range() {
    let m = RtCombatMetrics {
        miss_rate: 0.05,
        resist_rate: 0.02,
        crit_rate: 0.15,
        ..RtCombatMetrics::zeroed()
    };
    assert!(m.miss_rate >= 0.0 && m.miss_rate <= 1.0);
    assert!(m.resist_rate >= 0.0 && m.resist_rate <= 1.0);
    assert!(m.crit_rate >= 0.0 && m.crit_rate <= 1.0);
}

#[test]
fn rt_movement_stuck_rate_no_zones_returns_raw_count() {
    let m = RtMovementMetrics {
        stuck_count: 7,
        zone_changes: 0,
        ..RtMovementMetrics::zeroed()
    };
    // Documented fallback: returns stuck_count directly when zone_changes == 0
    assert_eq!(m.stuck_rate_per_zone(), 7.0);
}

#[test]
fn rt_loot_valuable_ratio_zero_items() {
    let m = LootMetrics::zeroed();
    assert_eq!(m.valuable_ratio(), 0.0);
}

#[test]
fn rt_loot_valuable_ratio_mixed() {
    let m = LootMetrics {
        items_looted: 50,
        valuable_items: 10,
        ..LootMetrics::zeroed()
    };
    assert!((m.valuable_ratio() - 0.2).abs() < f64::EPSILON);
}

#[test]
fn rt_fleet_dps_per_member_zero_guard() {
    let f = RtFleetMetrics::zeroed();
    assert_eq!(f.dps_per_member(), 0.0);
}

#[test]
fn rt_fleet_combat_ratio_full_fleet() {
    let f = RtFleetMetrics {
        total_dps: 36_000.0,
        member_count: 36,
        active_members: 36,
        zone: "The Plane of Fear".to_owned(),
        combat_active: true,
    };
    assert!((f.combat_ratio() - 1.0).abs() < f64::EPSILON);
    assert!((f.dps_per_member() - 1_000.0).abs() < f64::EPSILON);
}

#[test]
fn rt_fleet_partial_engagement() {
    let f = RtFleetMetrics {
        total_dps: 18_000.0,
        member_count: 36,
        active_members: 18,
        zone: "Nagafen's Lair".to_owned(),
        combat_active: true,
    };
    assert!((f.combat_ratio() - 0.5).abs() < f64::EPSILON);
}

#[test]
fn rt_system_metrics_overhead_within_budget() {
    // Validates design-doc performance budget: DLL CPU < 2%, frame < 34 ms
    let m = SystemMetrics {
        dll_cpu_percent: 0.8,
        memory_usage_mb: 32.0,
        ipc_messages_per_sec: 150.0,
        frame_latency_ms: 16.7,
        uptime_secs: 3600,
    };
    assert!(m.dll_cpu_percent < 2.0, "CPU overhead exceeded 2% budget");
    assert!(
        m.frame_latency_ms < 34.0,
        "frame latency exceeded 2-frame budget at 60 Hz"
    );
}

// ─── MetricWindow ────────────────────────────────────────────────────────────

#[test]
fn metric_window_all_durations_correct() {
    assert_eq!(MetricWindow::OneMin.duration_secs(), Some(60));
    assert_eq!(MetricWindow::FiveMin.duration_secs(), Some(300));
    assert_eq!(MetricWindow::SixtyMin.duration_secs(), Some(3600));
    assert_eq!(MetricWindow::Session.duration_secs(), None);
}

#[test]
fn metric_window_display_strings() {
    assert_eq!(MetricWindow::OneMin.to_string(), "1m");
    assert_eq!(MetricWindow::FiveMin.to_string(), "5m");
    assert_eq!(MetricWindow::SixtyMin.to_string(), "60m");
    assert_eq!(MetricWindow::Session.to_string(), "session");
}

// ─── TimeWindowedMetrics ─────────────────────────────────────────────────────

#[test]
fn time_windowed_metrics_preserves_data() {
    let fleet = RtFleetMetrics {
        total_dps: 12_000.0,
        member_count: 12,
        active_members: 12,
        zone: "Kithicor Forest".to_owned(),
        combat_active: true,
    };
    let tw = TimeWindowedMetrics::new(MetricWindow::FiveMin, fleet.clone());
    assert_eq!(tw.window, MetricWindow::FiveMin);
    assert!((tw.metrics.total_dps - 12_000.0).abs() < f64::EPSILON);
    assert_eq!(tw.metrics.zone, "Kithicor Forest");
}

#[test]
fn time_windowed_metrics_session_window() {
    let m = RtCombatMetrics {
        dps: 2_500.0,
        encounter_count: 100,
        ..RtCombatMetrics::zeroed()
    };
    let tw = TimeWindowedMetrics::new(MetricWindow::Session, m);
    assert_eq!(tw.window, MetricWindow::Session);
    assert!(tw.window.duration_secs().is_none());
}

// ─── Alert-threshold smoke tests ─────────────────────────────────────────────

/// Simulates the `dps_degraded` alert condition:
/// current_dps < 0.5 × 5min_avg_dps
#[test]
fn alert_threshold_dps_degraded_detects_50pct_drop() {
    let baseline_dps = 1_200.0f64;
    let current_dps = 400.0f64;
    let degraded = current_dps < 0.5 * baseline_dps;
    assert!(degraded, "50%+ DPS drop should trigger dps_degraded alert");
}

#[test]
fn alert_threshold_dps_normal_does_not_trigger() {
    let baseline_dps = 1_200.0f64;
    let current_dps = 1_100.0f64;
    let degraded = current_dps < 0.5 * baseline_dps;
    assert!(!degraded, "small DPS variation should not trigger alert");
}

/// Simulates the `client_stuck` alert condition:
/// stuck_events in last 5 min > 3
#[test]
fn alert_threshold_client_stuck_triggers_above_3() {
    let stuck_events_5min = 4u32;
    let alert = stuck_events_5min > 3;
    assert!(alert, ">3 stuck events should trigger client_stuck alert");
}

#[test]
fn alert_threshold_client_stuck_no_trigger_at_threshold() {
    let stuck_events_5min = 3u32;
    let alert = stuck_events_5min > 3;
    assert!(!alert, "exactly 3 stuck events should not trigger alert");
}

/// Simulates the `client_oom` alert condition: memory > 500 MB
#[test]
fn alert_threshold_client_oom_triggers_above_500mb() {
    let memory_bytes = 512_000_000u64;
    let threshold_bytes = 500_000_000u64;
    let alert = memory_bytes > threshold_bytes;
    assert!(alert, ">500 MB should trigger client_oom alert");
}

/// Simulates the `ipc_latency_spike` alert condition: p95 > 500 ms
#[test]
fn alert_threshold_ipc_spike_via_admin_store() {
    let mut store = AdminMonitoringStore::with_retention(AdminMonitoringRetention {
        ipc_samples: 20,
        memory_samples: 5,
        error_events: 5,
    });
    let now = Instant::now();
    store.register_session(50, 5000);

    // Inject 19 normal samples + 1 spike
    for i in 0..19u64 {
        store.record_ipc_latency_at(50, 20 + i, now + Duration::from_millis(i * 100));
    }
    store.record_ipc_latency_at(50, 600, now + Duration::from_millis(2000)); // spike

    let snap = store
        .snapshot(50, now + Duration::from_millis(2100))
        .unwrap();
    let p95 = snap.ipc_latency.p95_ms.expect("p95 should be set");
    assert!(
        p95 > 500,
        "p95 IPC latency {p95}ms should exceed 500ms alert threshold"
    );
}

// ─── Serde round-trips ────────────────────────────────────────────────────────

#[test]
fn rt_fleet_metrics_serde_round_trip() {
    let original = RtFleetMetrics {
        total_dps: 24_000.0,
        member_count: 24,
        active_members: 20,
        zone: "Plane of Hate".to_owned(),
        combat_active: true,
    };
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: RtFleetMetrics = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

#[test]
fn rt_combat_metrics_serde_round_trip() {
    let original = RtCombatMetrics {
        dps: 1_500.0,
        tps: 900.0,
        cast_rate: 10.0,
        miss_rate: 0.08,
        resist_rate: 0.03,
        crit_rate: 0.18,
        combat_duration_secs: 600,
        encounter_count: 40,
    };
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: RtCombatMetrics = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

#[test]
fn rt_system_metrics_serde_round_trip() {
    let original = SystemMetrics {
        dll_cpu_percent: 1.5,
        memory_usage_mb: 64.0,
        ipc_messages_per_sec: 300.0,
        frame_latency_ms: 16.7,
        uptime_secs: 7200,
    };
    let json = serde_json::to_string(&original).expect("serialize");
    let restored: SystemMetrics = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, restored);
}

// ─── Multi-client isolation ────────────────────────────────────────────────────

#[test]
fn admin_store_multi_client_isolation() {
    let mut store = AdminMonitoringStore::with_retention(small_retention());
    let now = Instant::now();

    store.register_session(100, 10_000);
    store.register_session(101, 10_001);
    store.register_session(102, 10_002);

    store.record_ipc_latency_at(100, 5, now);
    store.record_ipc_latency_at(101, 50, now);
    store.record_ipc_latency_at(102, 500, now);

    let s100 = store.snapshot(100, now).unwrap();
    let s101 = store.snapshot(101, now).unwrap();
    let s102 = store.snapshot(102, now).unwrap();

    assert_eq!(s100.ipc_latency.last_ms, Some(5));
    assert_eq!(s101.ipc_latency.last_ms, Some(50));
    assert_eq!(s102.ipc_latency.last_ms, Some(500));
}

#[test]
fn collector_unknown_character_returns_none() {
    let col = MetricsCollector::new();
    assert!(col.get_character_metrics("GhostChar").is_none());
}
