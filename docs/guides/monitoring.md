# Performance Monitoring Guide

> **Phase 4.2d** — Usage guide for the TextQuest performance monitoring system.
> Design reference: [`docs/design/performance-monitoring/design.md`](../design/performance-monitoring/design.md)

---

## Overview

TextQuest's performance monitoring system collects per-character and fleet-wide metrics in real-time, stores them in SQLite for historical analysis, and optionally exports them in Prometheus format. This guide explains how to instrument new code, query existing metrics, and interpret monitoring data.

---

## Architecture at a Glance

```
EQ DLL event → MetricsCollector (in-memory, per client)
                    │
                    ▼
           SamplingScheduler (background task — #1112)
                    │
          ┌─────────┼────────────┐
          ▼         ▼            ▼
    MetricsStore  Prometheus  AlertEvaluator
    (SQLite)      (optional)  (alerts.rs)
```

See `textquest/src/metrics/` for the full module layout.

---

## Module Reference

| Module | Type | Purpose |
|--------|------|---------|
| `metrics/types.rs` | `CombatMetrics`, `FleetMetrics`, `SystemMetrics` | Real-time snapshot types |
| `metrics/collector.rs` | `MetricsCollector` | Per-character aggregation with 1min/5min/60min windows |
| `metrics/admin_monitoring.rs` | `AdminMonitoringStore` | IPC latency, memory, error rate per session |
| `metrics/baseline_scorecard.rs` | `BaselineScorecard` | Before/after delta for optimization loops |
| `metrics/sampling.rs` | `SamplingScheduler` | Periodic collection driver (scaffold — #1112) |
| `metrics/exporter.rs` | `PrometheusExporter` | HTTP `/metrics` endpoint (scaffold — #1112) |
| `metrics/store.rs` | `MetricsStore` | SQLite persistence |
| `metrics/events.rs` | `FleetEventLog` | Structured event stream |
| `metrics/kill_tracker.rs` | `KillTracker` | Per-mob DPS stats and efficiency scoring |
| `metrics/xp_tracker.rs` | `XpTracker` | XP/hour and AA/hour per client |

---

## Instrumentation

### Recording Combat Events

```rust
use textquest::metrics::MetricsCollector;
use textquest::metrics::baseline_scorecard::CombatMetrics;

let mut collector = MetricsCollector::new();

// Called each time you receive a combat update from the DLL
collector.update_combat("Renia", CombatMetrics {
    dps: 1_200.0,
    mana_consumed: 450,
    endurance_consumed: 200,
    avg_pull_to_kill_secs: 14.5,
    total_kills: 3,
});
```

### Recording Movement Events

```rust
use textquest::metrics::baseline_scorecard::MovementMetrics;

collector.update_movement("Renia", MovementMetrics {
    stuck_percentage: 0.0,
    total_distance: 2_400.0,
    stuck_event_count: 0,
    route_efficiency: 0.92,
});
```

### Recording Loot Events

```rust
// item_name: display name; value: platinum equivalent
collector.update_loot("Renia", "Velious Plate Breastplate", 500);
collector.update_loot("Renia", "Words of Dissolution", 25);
```

### Recording Admin / Health Metrics

```rust
use textquest::metrics::{AdminMonitoringStore, SessionErrorKind};

let mut admin = AdminMonitoringStore::new();

// When a new EQ process is attached
admin.register_session(client_id, pid);

// After each successful IPC round-trip
admin.record_ipc_latency(client_id, round_trip_ms);

// After sampling process memory
admin.record_memory_sample(client_id, memory_bytes);

// On IPC errors
admin.record_error(client_id, SessionErrorKind::IpcDispatch);
```

---

## Querying Metrics

### Per-Character Snapshot

```rust
if let Some(char_metrics) = collector.get_character_metrics("Renia") {
    println!("DPS:    {:.1}", char_metrics.combat.average_dps());
    println!("Kills:  {}", char_metrics.combat.total_kills);
    println!("Items:  {}", char_metrics.economy.total_items_looted);
    println!("Plat:   {}", char_metrics.economy.total_plat_earned);
}
```

### Fleet Aggregate

```rust
let fleet = collector.get_fleet_metrics();
println!("Fleet DPS:    {:.1}", fleet.fleet_dps);
println!("Total kills:  {}", fleet.total_kills);
println!("Total plat:   {}", fleet.total_plat_earned);
```

### Admin Monitoring Snapshot

```rust
use std::time::Instant;

let now = Instant::now();
if let Some(snap) = admin.snapshot(client_id, now) {
    println!("State:      {:?}", snap.state);
    println!("IPC p50:    {:?} ms", snap.ipc_latency.p50_ms);
    println!("IPC p95:    {:?} ms", snap.ipc_latency.p95_ms);
    println!("Memory:     {:?} bytes", snap.memory.current_bytes);
    println!("Errors/min: {}", snap.errors.errors_per_minute);
}
```

### Real-Time Type Snapshots

The `types.rs` module provides lightweight snapshot structs suitable for TUI rendering and Prometheus export:

```rust
use textquest::metrics::types::{CombatMetrics, FleetMetrics, MetricWindow, TimeWindowedMetrics};

let fleet_snap = FleetMetrics {
    total_dps: collector.get_fleet_metrics().fleet_dps,
    member_count: 36,
    active_members: 30,
    zone: "The Plane of Fear".to_owned(),
    combat_active: true,
};

// Wrap with a time window label
let windowed = TimeWindowedMetrics::new(MetricWindow::FiveMin, fleet_snap);
println!("Window: {}", windowed.window);    // "5m"
println!("DPS:    {:.1}", windowed.metrics.total_dps);
println!("DPS/member: {:.1}", windowed.metrics.dps_per_member());
```

---

## Metric Windows

All time-windowed metrics use the `MetricWindow` enum:

| Window | Duration | Use case |
|--------|----------|----------|
| `OneMin` | 60 s | Burst/spike detection |
| `FiveMin` | 300 s | Alert baselines (DPS degraded, stuck events) |
| `SixtyMin` | 3600 s | Session trend analysis |
| `Session` | unbounded | Full session rollup |

```rust
use textquest::metrics::types::MetricWindow;

assert_eq!(MetricWindow::FiveMin.duration_secs(), Some(300));
assert_eq!(MetricWindow::Session.duration_secs(), None);
```

---

## Alert Thresholds

Thresholds are evaluated against `FiveMin` window data by default. Current defaults (configurable in `config.toml` under `[alerts.performance]`):

| Alert | Condition | Severity |
|-------|-----------|----------|
| `client_stuck` | `stuck_events (5min) > 3` | Warning |
| `dps_degraded` | `current_dps < 0.5 × 5min_avg_dps` | Warning |
| `client_oom` | `memory_bytes > 500 MB` | Critical |
| `ipc_latency_spike` | `p95 IPC > 500 ms` | Warning |
| `high_error_rate` | `errors/min > 10` | Critical |

### Example: Checking DPS Degradation

```rust
let baseline_dps = five_min_avg_dps; // from collector
let current_dps  = combat_metrics.dps;

if current_dps < 0.5 * baseline_dps {
    // Route through alerts.rs → DiscordAlert
    warn!("dps_degraded: {current_dps:.0} dps (baseline {baseline_dps:.0})");
}
```

### Example: Checking Memory OOM

```rust
let threshold_bytes = 500_000_000u64;
if let Some(mem) = snap.memory.current_bytes {
    if mem > threshold_bytes {
        error!("client_oom: {mem} bytes for client {client_id}");
    }
}
```

---

## Prometheus Export (Optional)

Prometheus export is **disabled by default**. To enable:

```toml
# config.toml
[metrics]
prometheus_enabled = true
prometheus_port    = 9100
prometheus_path    = "/metrics"
```

Scrape the endpoint with:

```bash
curl http://localhost:9100/metrics
```

Recommended Prometheus scrape interval: **60 seconds** (matches the SQLite flush cadence).

All exported metrics use the `tq_` prefix. Key gauges and counters:

```
# HELP tq_dps_current Current DPS averaged over the session window
# TYPE tq_dps_current gauge
tq_dps_current{client_id="1"} 1200.0

# HELP tq_kills_total Total kill events since process start
# TYPE tq_kills_total counter
tq_kills_total{client_id="1",zone="The Plane of Fear"} 428

# HELP tq_ipc_roundtrip_ms IPC round-trip latency histogram
# TYPE tq_ipc_roundtrip_ms histogram
tq_ipc_roundtrip_ms_bucket{client_id="1",le="10"} 120
tq_ipc_roundtrip_ms_bucket{client_id="1",le="50"} 310
tq_ipc_roundtrip_ms_bucket{client_id="1",le="+Inf"} 315
```

> **Note:** The `PrometheusExporter` implementation is a scaffold pending #1112. The metric naming conventions above are final; the HTTP listener is not yet wired.

---

## Understanding Your Metrics

### DPS

- **`combat.average_dps()`** — rolling average over all DPS samples for this session. Resets when the `MetricsCollector` is recreated (typically at orchestrator restart).
- **`fleet.fleet_dps`** — sum of all active character DPS samples. Use `fleet.dps_per_member()` to normalize.
- **Watch for:** DPS dropping >50% vs. 5-min baseline → `dps_degraded` alert. Common causes: character death, stuck in place, spell interruption loop.

### IPC Latency

- **`p50_ms`** — typical round-trip; should be <10 ms on localhost.
- **`p95_ms`** — tail latency; alert fires at >500 ms. High p95 usually indicates EQ client CPU saturation or Windows scheduler jitter.
- **`p99_ms`** — worst-case; useful for diagnosing transient freezes.

### Memory

- **`current_bytes`** — most recent RSS sample for the EQ client process.
- **`growth_bytes_per_minute`** — rolling growth rate. Negative = shrinking (GC, unload). Positive trending toward 500 MB → `client_oom` risk.
- **Baseline:** healthy EQ client sits at 150–300 MB RSS; DLL adds ~2–8 MB overhead.

### Stuck Events

- **`stuck_event_count`** — discrete stuck detections (character in same position for >N seconds).
- **`stuck_percentage`** — fraction of session time spent stuck.
- **Alert:** >3 stuck events in 5 minutes → `client_stuck`. Investigate zone mesh gaps or navmesh dead zones.

### Economy (Items / Plat)

- **`items_looted`** / **`plat_earned`** — monotonic session counters.
- **`loot_rate_per_hour`** (real-time type) — rolling rate. Compare against `BaselineScorecard.economy` to detect camp degradation.
- **`valuable_ratio()`** — fraction of items above the configured value threshold. Use to tune item-filter rules.

---

## Example: Post-Session Analysis

```rust
// After a farming session ends, inspect the BaselineScorecard delta
let delta = scorecard.delta(&pre_session, &post_session);
println!("DPS change:      {:.1}", delta.combat.dps_delta);
println!("Kill rate:       {:.2} kills/hr", delta.combat.kills_per_hour);
println!("Plat/hr:         {:.0}", delta.economy.plat_per_hour);
println!("Stuck pct:       {:.1}%", delta.movement.stuck_pct_change);
println!("Route efficiency:{:.2}", delta.movement.route_efficiency_delta);
```

---

## Adding a New Metric

1. **Define the field** in the appropriate `types.rs` struct (or `baseline_scorecard.rs` for optimization metrics).
2. **Update the collector** (`collector.rs`) to accept and aggregate the new value.
3. **Add a `tq_` Prometheus metric name** to the design doc taxonomy (§3) and `exporter.rs` stub.
4. **Write a unit test** in the relevant `mod tests` block covering the zero-value guard and a non-trivial example.
5. **Update this guide** with a row in the taxonomy table and a usage snippet.

---

## Related Issues

| Issue | Status | Topic |
|-------|--------|-------|
| #1112 | Pending | Core implementation: `SamplingScheduler`, `PrometheusExporter` |
| #1113 | Done | Historical tracking SQLite schema |
| #1138 | Pending | Circuit breaker integration |
| #1142 | Pending | `Counter`, `Gauge`, `Histogram` primitive types |
| #802  | In progress | Parent epic: Phase 4 fleet intelligence |
