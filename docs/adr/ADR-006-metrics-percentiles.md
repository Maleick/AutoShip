# ADR-006: Metrics with Percentiles

**Status:** Accepted  
**Date:** 2026-04-18  
**Author:** TextQuest Team

## Context

TextQuest's fleet produces thousands of measurements every minute:

- **Latencies** — IPC roundtrip time, spawn read delay, spell cast lag
- **Throughput** — DPS dealt, items looted per hour, spells cast per minute
- **Resource usage** — Memory per DLL instance, CPU spike during zone transitions
- **Error rates** — Failed ability casts, vendor transaction timeouts

Operators and developers need to answer:

- "How is combat performance?" — Raw averages don't capture the full picture. If 95 spells cast at 100ms and 5 cast at 2000ms, the average is 200ms, but the user experiences the 2-second delay.
- "Is the system degrading?" — Compare p50 from this week to last week. Detect slow creep in latency.
- "What's the tail latency?" — Knowing p99 (99th percentile) helps set SLAs. "99% of spell casts complete within 500ms."

Naive approach: Only track means. Shortcomings:

- Outliers are hidden (one 10-second event buried in average of thousands of 100ms events)
- No visibility into distribution shape (bimodal distributions look normal as average)
- Cannot set meaningful timeout values for client operations

## Decision

Implement **SQLite-backed fleet metrics** with **P50, P95, P99 aggregation**:

1. **Raw metrics collection** — During execution, scenarios (ADR-002) and operations collect histograms:

```rust
struct LatencyBucket {
    name: String,     // "ipc_spawn_read", "spell_cast_lag"
    samples: Vec<f64>, // milliseconds
}

// During test run
let mut latencies = Vec::new();
for i in 0..1000 {
    let start = Instant::now();
    let result = ipc_client.read_spawn().await?;
    latencies.push(start.elapsed().as_secs_f64() * 1000.0); // ms
}
```

2. **SQLite schema** — Histogram data is persisted to a metrics database:

```sql
CREATE TABLE IF NOT EXISTS latency_histogram (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp   TEXT NOT NULL DEFAULT (datetime('now')),
    metric_name TEXT NOT NULL,
    character   TEXT,
    samples     TEXT NOT NULL,  -- JSON array of floats
    count       INTEGER NOT NULL,
    min         REAL NOT NULL,
    max         REAL NOT NULL,
    mean        REAL NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_latency_metric ON latency_histogram(metric_name);
CREATE INDEX IF NOT EXISTS idx_latency_char   ON latency_histogram(character);
```

3. **Percentile calculation** — Post-aggregation, compute P50/P95/P99 from raw samples:

```rust
pub fn percentile(samples: &[f64], p: f64) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }

    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let index = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted.get(index.saturating_sub(1)).copied()
}

// Usage
let p50 = percentile(&latencies, 50.0)?;  // median
let p95 = percentile(&latencies, 95.0)?;  // 95th percentile
let p99 = percentile(&latencies, 99.0)?;  // 99th percentile
```

4. **Reporting** — Fleet dashboard queries metrics and displays trends:

```sql
-- Last hour of spawn read latencies for all characters
SELECT
    metric_name,
    character,
    COUNT(*) as runs,
    MIN(mean) as min_mean,
    MAX(mean) as max_mean,
    -- Aggregate percentiles across all runs
    percentile_cont(0.50) WITHIN GROUP (ORDER BY mean) as p50,
    percentile_cont(0.95) WITHIN GROUP (ORDER BY mean) as p95,
    percentile_cont(0.99) WITHIN GROUP (ORDER BY mean) as p99
FROM latency_histogram
WHERE timestamp > datetime('now', '-1 hour')
  AND metric_name = 'ipc_spawn_read'
GROUP BY character;
```

Example output:

```
metric         | character    | runs | min_mean | max_mean | p50   | p95   | p99
---------------|--------------|------|----------|----------|-------|-------|-------
ipc_spawn_read | frostreaver01| 120  | 8.2      | 145.3    | 10.5  | 25.1  | 98.4
ipc_spawn_read | frostreaver02| 120  | 9.1      | 132.7    | 11.2  | 28.3  | 102.1
```

## Rationale

1. **Captures Distribution Shape** — Percentiles reveal the tail behavior that averages hide. Knowing p99 = 500ms is more actionable than mean = 150ms.

2. **Trend detection** — Comparing p50/p95/p99 week-over-week detects gradual degradation that single averages might miss.

3. **SLA-friendly** — "95% of operations complete within 50ms" (p95 = 50) is an SLA. Averages are not.

4. **Flexible storage** — SQLite is self-contained (no external service required) and queries are ad-hoc (no rigid schema).

5. **Debuggable** — If a metric degrades, the full histogram is stored, so you can query the raw samples and understand the outlier population.

6. **Language-agnostic** — Percentiles are computed in SQL; any tool can query the database (Python, spreadsheet, web frontend).

## Implementation Notes

- **Histogram size** — Store raw samples as a JSON array in the `samples` TEXT column. For large histograms (1000+ samples), consider lossy compression (e.g., store every 10th sample).
- **Rotation** — Archive old metric tables (>30 days) to a `metrics_archive_YYYY_MM.db` file to keep the active database fast.
- **Concurrent writes** — SQLite's default journal mode (DELETE) can be slow under concurrent load. Use WAL mode for better throughput.
- **Real-time aggregation** — The dashboard can compute percentiles on-the-fly from the raw samples, or pre-compute and cache them (e.g., every 5 minutes).

Example percentile computation in SQL (PostgreSQL-style; SQLite requires a UDF or approximation):

```sql
-- Approximate percentile in SQLite (not exact, but good enough)
SELECT
    metric_name,
    -- P50: median
    (SELECT json_extract(samples, '$[' || (COUNT(*) / 2) || ']')
     FROM latency_histogram l2
     WHERE l2.metric_name = l1.metric_name),
    -- P95: 95th element
    (SELECT json_extract(samples, '$[' || CAST(COUNT(*) * 0.95 AS INT) || ']')
     FROM latency_histogram l2
     WHERE l2.metric_name = l1.metric_name),
    -- P99: 99th element
    (SELECT json_extract(samples, '$[' || CAST(COUNT(*) * 0.99 AS INT) || ']')
     FROM latency_histogram l2
     WHERE l2.metric_name = l1.metric_name)
FROM latency_histogram l1
WHERE timestamp > datetime('now', '-1 hour')
GROUP BY metric_name;
```

## Alternatives Considered

1. **Counter + gauge only** — Track only min/max/mean. Missing tail latency information; insufficient for SLAs.
2. **Time-series database (Prometheus, InfluxDB)** — Specialized for metrics, but requires external service and is optimized for small samples (not histograms).
3. **Percentile approximation (HyperLogLog, T-Digest)** — Reduces storage at cost of precision. Good for streaming; overkill for fleet metrics that are post-processed once per test run.

## Related ADRs

- ADR-002: Test Scenario Trait Architecture (scenarios collect metric histograms)
- ADR-003: JSON Event Streaming (raw latency events can be aggregated into histograms)
- ADR-004: Per-Account Test Runners (each runner reports percentiles in its TestRunResult)

## References

- `textquest/src/metrics/store.rs` — MetricsStore and SQL schema
- `textquest/src/testing/scenario.rs` — ScenarioResult with metric histograms
