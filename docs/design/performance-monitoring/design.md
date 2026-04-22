# Performance Monitoring System Design

> **Phase 4.2a** — Architecture document for the TextQuest performance monitoring system.
> Parent: [#802](https://github.com/Maleick/TextQuest/issues/802)
> Blocks: [#1112](https://github.com/Maleick/TextQuest/issues/1112) (Core implementation)
> See also: [#1138](https://github.com/Maleick/TextQuest/issues/1138) (Circuit breaker), [#1142](https://github.com/Maleick/TextQuest/issues/1142) (Metrics types)

---

## 1. Objectives

| Goal                           | Rationale                                                                               |
| ------------------------------ | --------------------------------------------------------------------------------------- |
| Real-time fleet visibility     | Operator must see DPS, loot/hr, plat/hr per client without querying individual sessions |
| Anomaly detection              | Stuck clients, dead clients, and degraded DPS must surface as actionable alerts         |
| Historical trend tracking      | SQLite backend stores per-session snapshots for post-run analysis and RL feedback (M9)  |
| Performance budget enforcement | Monitoring overhead must not measurably impact game-client frame timing                 |
| Prometheus-compatible export   | Grafana dashboards for long-running farm sessions; optional, off by default             |

---

## 2. Current State (as of Phase 4.2a)

The `textquest/src/metrics/` module already provides a significant foundation:

| Existing Type                | File                    | Purpose                                                            |
| ---------------------------- | ----------------------- | ------------------------------------------------------------------ |
| `MetricsCollector`           | `collector.rs`          | Per-character metric aggregation with 1min/5min/60min windows      |
| `AggregateMetrics`           | `collector.rs`          | DPS, damage, kills, deaths, items, plat, movement, spells, healing |
| `FleetMetrics`               | `collector.rs`          | Multi-character rollup                                             |
| `BaselineScorecard`          | `baseline_scorecard.rs` | Before/after delta capture for optimization loops                  |
| `CombatMetrics`              | `baseline_scorecard.rs` | DPS, mana/endurance consumption, pull-to-kill timing               |
| `MovementMetrics`            | `baseline_scorecard.rs` | Stuck %, route efficiency, stuck event count                       |
| `EconomyMetrics`             | `baseline_scorecard.rs` | Items/hr, plat/hr, top-drop tracking                               |
| `GroupCoordinationMetrics`   | `baseline_scorecard.rs` | Assist timing, heal latency, aggro distribution                    |
| `AdminMonitoringStore`       | `admin_monitoring.rs`   | Per-session IPC latency, memory, error rates                       |
| `KillTracker` / `KillRecord` | `kill_tracker.rs`       | Per-mob DPS stats, efficiency scoring                              |
| `FleetEventLog`              | `events.rs`             | Structured event stream for combat, economy, movement              |
| `MetricsStore`               | `store.rs`              | SQLite persistence layer                                           |
| `XpTracker`                  | `xp_tracker.rs`         | XP/hour tracking per client                                        |

**Gap analysis:** The collection and storage infrastructure is largely built. What is missing:

1. A unified **sampling scheduler** that drives metric collection on a configurable cadence
2. A **Prometheus text-format exporter** (HTTP endpoint or file sink)
3. Explicit **alert thresholds** linked to the alerting system in `alerts.rs`
4. A **circuit-breaker tie-in** (issue #1138) to suppress or escalate metrics during unhealthy states
5. A **TUI surface** for real-time fleet metrics (currently partially covered by the performance overlay)

---

## 3. Metrics Taxonomy

### 3.1 Metric Kinds

| Kind               | Description                                              | Example                                                    |
| ------------------ | -------------------------------------------------------- | ---------------------------------------------------------- |
| **Counter**        | Monotonically increasing, resets only on process restart | `kills_total`, `items_looted_total`, `plat_earned_total`   |
| **Gauge**          | Instantaneous snapshot, can go up or down                | `current_dps`, `memory_bytes`, `stuck_percentage`          |
| **Histogram**      | Distribution of observed values in buckets               | `pull_to_kill_secs`, `ipc_roundtrip_ms`, `heal_latency_ms` |
| **Rate (derived)** | Computed over a rolling window from counters             | `kills_per_hour`, `items_per_hour`, `plat_per_hour`        |

### 3.2 Combat Metrics

| Metric                   | Kind      | Labels              | Unit    |
| ------------------------ | --------- | ------------------- | ------- |
| `tq_kills_total`         | Counter   | `client_id`, `zone` | count   |
| `tq_damage_dealt_total`  | Counter   | `client_id`         | points  |
| `tq_damage_taken_total`  | Counter   | `client_id`         | points  |
| `tq_deaths_total`        | Counter   | `client_id`         | count   |
| `tq_dps_current`         | Gauge     | `client_id`         | dps     |
| `tq_pull_to_kill_secs`   | Histogram | `client_id`         | seconds |
| `tq_spells_cast_total`   | Counter   | `client_id`         | count   |
| `tq_healing_done_total`  | Counter   | `client_id`         | points  |
| `tq_mana_consumed_total` | Counter   | `client_id`         | points  |

### 3.3 Economy Metrics

| Metric                  | Kind    | Labels                   | Unit     |
| ----------------------- | ------- | ------------------------ | -------- |
| `tq_items_looted_total` | Counter | `client_id`, `item_name` | count    |
| `tq_plat_earned_total`  | Counter | `client_id`              | plat     |
| `tq_items_per_hour`     | Gauge   | `client_id`              | items/hr |
| `tq_plat_per_hour`      | Gauge   | `client_id`              | plat/hr  |

### 3.4 Movement / Navigation Metrics

| Metric                       | Kind      | Labels              | Unit        |
| ---------------------------- | --------- | ------------------- | ----------- |
| `tq_stuck_events_total`      | Counter   | `client_id`, `zone` | count       |
| `tq_stuck_duration_secs`     | Histogram | `client_id`         | seconds     |
| `tq_distance_traveled_total` | Counter   | `client_id`         | game-units  |
| `tq_route_efficiency`        | Gauge     | `client_id`         | ratio (0–1) |

### 3.5 Operational / Admin Metrics

| Metric                    | Kind      | Labels                    | Unit         |
| ------------------------- | --------- | ------------------------- | ------------ |
| `tq_ipc_roundtrip_ms`     | Histogram | `client_id`               | milliseconds |
| `tq_session_memory_bytes` | Gauge     | `client_id`               | bytes        |
| `tq_session_error_rate`   | Gauge     | `client_id`, `error_kind` | errors/min   |
| `tq_clients_active`       | Gauge     | —                         | count        |
| `tq_clients_stuck`        | Gauge     | —                         | count        |

### 3.6 XP / Progression Metrics

| Metric           | Kind  | Labels               | Unit  |
| ---------------- | ----- | -------------------- | ----- |
| `tq_xp_per_hour` | Gauge | `client_id`, `level` | xp/hr |
| `tq_aa_per_hour` | Gauge | `client_id`          | aa/hr |

---

## 4. Data Collection Strategy

### 4.1 Sampling Cadence

```
┌──────────────────────────────────────────────────────────────────┐
│  EQ 6-second server tick ──► in-memory event accumulation        │
│  1-second orchestrator loop ──► gauge snapshots (DPS, memory)    │
│  60-second flush ──► SQLite commit + Prometheus scrape endpoint   │
│  5-minute scorecard ──► BaselineScorecard delta calculation       │
│  Session end ──► final rollup + archival write                    │
└──────────────────────────────────────────────────────────────────┘
```

**Rationale:** The EQ server tick is 6 seconds; sampling faster than the tick provides no new combat data. The 1-second orchestrator loop is the minimum useful cadence for operational health metrics (IPC latency, memory). SQLite commits are batched to 60 seconds to avoid write amplification.

### 4.2 Sampling Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Per-client IPC handler (DLL → textquest daemon)                │
│    └─► Event: CombatEvent | LootEvent | MovementEvent | XpEvent │
│         │                                                        │
│         ▼                                                        │
│  MetricsCollector (in-memory, per client_id)                    │
│    - 1min / 5min / 60min rolling windows                        │
│    - atomic counter increments (no lock contention on hot path) │
│         │                                                        │
│         ▼                                                        │
│  SamplingScheduler (background tokio task)                      │
│    - 1s: snapshot gauges → AdminMonitoringStore                 │
│    - 60s: flush to MetricsStore (SQLite)                        │
│    - 60s: update Prometheus gauge/counter state                 │
│    - 5m: compute BaselineScorecard delta                        │
│         │                                                        │
│         ├─► MetricsStore (SQLite)                               │
│         ├─► PrometheusExporter (optional HTTP /metrics)         │
│         └─► AlertEvaluator → alerts.rs routing                  │
└─────────────────────────────────────────────────────────────────┘
```

### 4.3 Performance Budget

| Operation                  | Target overhead | Hard limit |
| -------------------------- | --------------- | ---------- |
| Per-event metric update    | < 1 µs          | 10 µs      |
| 1-second gauge snapshot    | < 500 µs total  | 2 ms       |
| 60-second SQLite flush     | < 10 ms         | 50 ms      |
| Prometheus scrape response | < 5 ms          | 20 ms      |
| Total CPU overhead         | < 0.5%          | 2%         |

All metric updates on the hot path (event ingestion) must be lock-free atomic operations. The `SamplingScheduler` runs on a dedicated tokio task and never blocks the IPC dispatch loop.

---

## 5. Emit Surfaces

### 5.1 TUI Dashboard (Primary)

The TUI performance screen (see `docs/design/tui_refresh/`) exposes:

- **Fleet overview:** per-client DPS, kills/hr, items/hr, plat/hr in a table
- **Health indicators:** memory, IPC latency, error rate per client
- **Time-window selector:** 1min / 5min / 60min toggle
- **Alert feed:** recent anomaly alerts with severity coloring

This is the primary operational surface. No additional TUI work is required by this design — the data model (`FleetMetrics`, `AdminMonitoringStore`) already feeds the existing overlay.

### 5.2 SQLite Historical Store (Secondary)

`MetricsStore` persists per-session snapshots. Schema targets:

- `session_metrics` — one row per client per 60s flush
- `session_events` — raw `FleetEvent` log (combat, loot, movement)
- `kill_records` — per-mob stats for post-run analysis
- `scorecards` — `BaselineScorecard` deltas for RL feedback (M9)

Retention policy: keep 30 days of raw events; keep 1 year of 60s aggregates; keep all scorecards.

### 5.3 Prometheus Export (Optional / Off by Default)

A `PrometheusExporter` module exposes a `/metrics` HTTP endpoint on `localhost:9100` (configurable). Output is standard Prometheus text format.

Enable via config:

```toml
[metrics]
prometheus_enabled = false
prometheus_port = 9100
prometheus_path = "/metrics"
```

Scrape interval recommendation: 60s (matches the SQLite flush cadence).

### 5.4 Discord Alerts (Operational)

High-severity anomalies route through `alerts.rs` → `DiscordAlert`. Examples:

- Client stuck > 5 minutes
- DPS drops > 50% vs. 5-minute baseline
- Client memory > 500 MB
- IPC error rate > 10 errors/minute

---

## 6. Alert Thresholds

Thresholds are configurable in `config.toml` under `[alerts.performance]`. Defaults:

| Alert               | Condition                                      | Severity |
| ------------------- | ---------------------------------------------- | -------- |
| `client_stuck`      | stuck_events in last 5min > 3                  | Warning  |
| `client_dead`       | kills in last 10min == 0 AND zone is camp zone | Warning  |
| `dps_degraded`      | current_dps < 0.5 × 5min_avg_dps               | Warning  |
| `client_oom`        | memory_bytes > 500 MB                          | Critical |
| `ipc_latency_spike` | p95 ipc_roundtrip_ms > 500 ms                  | Warning  |
| `high_error_rate`   | session_error_rate > 10/min                    | Critical |

---

## 7. Circuit Breaker Integration (#1138)

The `SamplingScheduler` consults the circuit breaker state before dispatching work:

| Circuit State        | Sampling Behavior                                                            |
| -------------------- | ---------------------------------------------------------------------------- |
| `Closed` (healthy)   | Normal cadence — all sampling active                                         |
| `Open` (tripped)     | Administrative metrics only (memory, IPC); combat/economy sampling suspended |
| `HalfOpen` (probing) | Reduced cadence — 10s gauge snapshots, no SQLite flush until re-closed       |

This prevents the monitoring system from amplifying load on an already-struggling client.

---

## 8. Metrics Types Module (#1142)

Issue #1142 will define the canonical metric type primitives shared across the codebase. This design anticipates:

```rust
/// Marker trait for all metric value types.
pub trait MetricValue: Clone + Send + Sync + 'static {}

/// A labeled counter — monotonically increasing u64.
pub struct Counter { /* ... */ }

/// A labeled gauge — f64 snapshot.
pub struct Gauge { /* ... */ }

/// A histogram with configurable bucket boundaries.
pub struct Histogram { /* buckets: Vec<f64>, counts: Vec<u64>, sum: f64, count: u64 */ }
```

The `MetricsCollector` will be refactored to use these primitives once #1142 lands. Until then, existing `AggregateMetrics` fields serve as the data model.

---

## 9. rgmercs Reference

The rgmercs `performance.lua` module tracks:

- DPS per spell/ability
- Mana efficiency (damage per mana)
- Burn vs. sustained rotation comparison
- Kill time distributions

TextQuest's design maps directly:

- `tq_dps_current` → rgmercs DPS tracking
- `tq_pull_to_kill_secs` histogram → rgmercs kill time distribution
- `tq_mana_consumed_total` → rgmercs mana efficiency
- `BaselineScorecard` → rgmercs burn vs. sustained comparison

---

## 10. Acceptance Criteria

- [x] Key metrics defined (see §3 — combat, economy, movement, operational, XP)
- [x] Data collection strategy documented (see §4 — sampling cadence, architecture, performance budget)
- [x] Emit surfaces identified (TUI, SQLite, Prometheus, Discord alerts)
- [x] Alert thresholds specified (see §6)
- [x] Circuit breaker integration point defined (see §7)
- [x] Metrics types module dependency documented (see §8)
- [x] rgmercs reference incorporated (see §9)

---

## 11. Implementation Order

1. **#1142** — Define `Counter`, `Gauge`, `Histogram` primitives
2. **#1138** — Circuit breaker (needed for §7)
3. **#1112** — Core implementation: `SamplingScheduler`, `PrometheusExporter`, alert threshold wiring
4. **TUI integration** — Wire `FleetMetrics` into performance overlay (partially done)
5. **M9 feedback loop** — `BaselineScorecard` delta pipeline to RL trainer
