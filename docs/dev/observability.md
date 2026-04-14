# Observability & Logging Standards

This document outlines the observability and logging conventions for the TextQuest project. It covers structured logging, metric collection, and observability infrastructure.

## Logging Conventions

### Logging Library

All structured logging uses the **`tracing` crate** with **`tracing-appender`** for file-based output.

- Do NOT use the `log` crate
- Reserved for `println!` / `eprintln!`: CLI output and TUI display only
- Never use `println!` / `eprintln!` for structured logs

### Log Levels by Category

| Level | Use Case | Example |
|-------|----------|---------|
| **ERROR** | Unrecoverable crashes, fatal conditions | Parsing corruption, process termination |
| **WARN** | Recovery actions, degraded state | Zone transition retry, failed spawn read |
| **INFO** | State changes, operational events | Client connected, group composition updated |
| **DEBUG** | IPC details, fine-grained diagnostics | Command sent to DLL, shared memory write |
| **TRACE** | Extremely verbose protocol details | Memory offset reads, packet inspection (rarely used) |

### Structured Fields

Always attach relevant context as structured fields:

```rust
use tracing::{info, warn, error};

// Good: structured context
info!(
    zone = "sebilis",
    spawn_id = 42,
    health_pct = 85.5,
    "spawn health updated"
);

// Bad: unstructured
println!("Spawn 42 health is 85.5%");
```

### Log File Locations

- **Orchestrator**: `./logs/textquest.log` (daily rolling, via `tracing-appender`)
- **DLL**: `%TEMP%/textquest/textquest-dll.log` (daily rolling, Windows only)
- **Web Dashboard**: stdout (no file logging configured yet)

## Metrics Naming Conventions

All metrics should follow consistent naming:

- **Case**: snake_case
- **Suffixes**: Include units when relevant
  - `_ms` for milliseconds (e.g., `command_latency_ms`)
  - `_bytes` for byte counts (e.g., `memory_used_bytes`)
  - `_total` for cumulative counters (e.g., `ipc_errors_total`)
- **Dimensionality**: Use labels for context
  - `zone` (e.g., "sebilis", "txevu")
  - `client_id` (e.g., "42", "druid-1")
  - `status` (e.g., "success", "timeout")

### Example Metrics

| Name | Kind | Labels | Purpose |
|------|------|--------|---------|
| `spawn_count` | Counter | zone, class | Total spawns encountered by zone/class |
| `command_latency_ms` | Histogram | client_id | Histogram of command round-trip times |
| `process_memory_mb` | Gauge | process_id | Current process memory usage |
| `ipc_errors_total` | Counter | error_type | Cumulative IPC failures |

## Metric Types

### Counter

- **Monotonically increasing** value
- Use for cumulative totals: requests, errors, events
- Never decrease (except on reset/clear)
- Example: `spawn_count`, `ipc_messages_sent_total`

### Gauge

- **Point-in-time** measurement
- Can go up or down
- Use for levels, sizes, temperatures: memory, latency, load
- Example: `process_memory_mb`, `active_spawns`

### Histogram

- **Distribution** of values
- Collects multiple samples over time
- Used for percentile analysis (p50, p95, p99)
- Example: `command_latency_ms`, `response_size_bytes`

## Metrics Collector API

### Using the Default Collector

```rust
use textquest_common::observability::{Metric, MetricKind, InMemoryCollector, MetricsCollector};
use std::time::SystemTime;

let collector = InMemoryCollector::new();

// Record a counter
collector.record(Metric {
    name: "spawn_count".to_string(),
    kind: MetricKind::Counter,
    value: 1.0,
    labels: vec![("zone".to_string(), "sebilis".to_string())],
    timestamp: SystemTime::now(),
});

// Record a gauge
collector.record(Metric {
    name: "process_memory_mb".to_string(),
    kind: MetricKind::Gauge,
    value: 256.5,
    labels: vec![],
    timestamp: SystemTime::now(),
});

// Query metrics
let all = collector.all_metrics();
let spawn_total = collector.sum_by_name("spawn_count");
let avg_memory = collector.average_by_name("process_memory_mb");
```

### Implementing a Custom Collector

To add a new backend (e.g., Prometheus, Grafana, external service):

```rust
use textquest_common::observability::{MetricsCollector, Metric};

struct CustomCollector {
    // Your storage/state here
}

impl MetricsCollector for CustomCollector {
    fn record(&self, metric: Metric) {
        // Handle the metric
    }

    fn all_metrics(&self) -> Vec<Metric> {
        // Return stored metrics
        vec![]
    }

    fn clear(&self) {
        // Reset state
    }

    fn metric_count(&self) -> usize {
        // Return count
        0
    }
}
```

## Future Work: Prometheus & Grafana

The current implementation provides in-memory collection suitable for:
- Integration testing
- Development/debugging
- Single-process scenarios

Future phases will add:
- **Prometheus export**: HTTP `/metrics` endpoint with standard Prometheus format
- **Grafana dashboards**: Real-time visualization of spawn counts, latency, error rates
- **Remote metrics**: Central collection from multiple orchestrator instances
- **Alerting**: Thresholds for anomalies (high latency, error spikes, memory leaks)

Reserved placeholders:
- `promethus_exporter` module (planned)
- `grafana_publisher` trait (planned)

## Integration Examples

### In Tests

```rust
#[test]
fn test_ipc_latency() {
    let collector = InMemoryCollector::new();
    
    // ... run IPC commands ...
    
    let avg_latency = collector.average_by_name("command_latency_ms");
    assert!(avg_latency.unwrap() < 100.0);
}
```

### In Production Code

```rust
use tracing::info;
use textquest_common::observability::{Metric, MetricKind, MetricsCollector};

fn process_spawn(collector: &dyn MetricsCollector) {
    info!("processing spawn");
    
    collector.record(Metric {
        name: "spawn_processed".to_string(),
        kind: MetricKind::Counter,
        value: 1.0,
        labels: vec![],
        timestamp: SystemTime::now(),
    });
}
```

## Logging Best Practices

1. **Use structured fields** — attach context to every log
2. **Choose the right level** — INFO for events, DEBUG for diagnostics, ERROR for failures
3. **Include identifiers** — always log zone, client_id, spawn_id when relevant
4. **No PII** — never log account names, passwords, or personal data
5. **Consistent terminology** — use exact field names across all logs
6. **Use tracing macros** — `info!`, `warn!`, `error!`, `debug!` (not manual concatenation)

## Monitoring Checklist

When adding a new feature, consider:

- [ ] Are errors properly logged?
- [ ] Is state change tracked via metrics?
- [ ] Do high-latency operations record timing?
- [ ] Are resource limits monitored (memory, connections)?
- [ ] Are anomalies visible via DEBUG logs?
- [ ] Can operators diagnose failures from logs?
