# Monitoring Testing Guidelines

This document describes testing approaches for the admin-monitoring system. For observability infrastructure, see [observability.md](./observability.md).

## Overview

The admin-monitoring system tracks per-session metrics:

- **IPC latency**: Round-trip times with percentile analysis (p50, p95, p99)
- **Memory trends**: Resident memory samples with growth rate calculation
- **Error rates**: Categorized errors with 60-second rolling window

## Test Architecture

### Core Types

```rust
use textquest::metrics::admin_monitoring::{
    AdminMonitoringStore, AdminMonitoringRetention, MonitoredSessionState,
    SessionErrorKind, SessionMonitoringSnapshot,
};

let store = AdminMonitoringStore::new();
```

### Retention Configuration

```rust
let retention = AdminMonitoringRetention {
    ipc_samples: 10,
    memory_samples: 10,
    error_events: 5,
};
let store = AdminMonitoringStore::with_retention(retention);
```

Default retention: 120 samples for IPC/memory, 120 events for errors.

## Test Patterns

### Session Lifecycle

```rust
#[test]
fn session_lifecycle_records_state_transitions() {
    let mut store = AdminMonitoringStore::new();
    let now = Instant::now();

    store.register_session(1, 100);
    let active = store.snapshot(1, now).expect("active session");
    assert_eq!(active.state, MonitoredSessionState::Active);
    assert_eq!(active.pid, Some(100));

    store.mark_session_exited(1);
    let exited = store.snapshot(1, now).expect("exited session");
    assert_eq!(exited.state, MonitoredSessionState::Exited);

    store.mark_session_absent(1);
    let absent = store.snapshot(1, now).expect("absent session");
    assert_eq!(absent.state, MonitoredSessionState::Absent);
}
```

### IPC Latency Tracking

```rust
#[test]
fn ipc_latency_computes_percentiles() {
    let mut store = AdminMonitoringStore::new();
    let now = Instant::now();

    store.register_session(2, 200);

    let latencies = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
    for (i, latency) in latencies.iter().enumerate() {
        store.record_ipc_latency_at(2, *latency, now + Duration::from_secs(i as u64));
    }

    let snapshot = store.snapshot(2, now + Duration::from_secs(10)).expect("snapshot");
    assert_eq!(snapshot.ipc_latency.sample_count, 10);
    assert_eq!(snapshot.ipc_latency.last_ms, Some(100));
    assert_eq!(snapshot.ipc_latency.p50_ms, Some(50));
    assert_eq!(snapshot.ipc_latency.p95_ms, Some(95));
    assert_eq!(snapshot.ipc_latency.p99_ms, Some(100));
}
```

### Memory Trend Analysis

```rust
#[test]
fn memory_trend_shows_growth_rate() {
    let mut store = AdminMonitoringStore::new();
    let start = Instant::now();

    store.register_session(3, 300);

    store.record_memory_sample_at(3, 256_000_000, start);
    store.record_memory_sample_at(3, 512_000_000, start + Duration::from_secs(60));

    let snapshot = store.snapshot(3, start + Duration::from_secs(60)).expect("snapshot");
    assert_eq!(snapshot.memory.current_bytes, Some(512_000_000));
    assert_eq!(snapshot.memory.delta_bytes, Some(256_000_000));
    assert!(snapshot.memory.growth_bytes_per_minute.is_some());
}
```

### Error Rate Tracking

```rust
#[test]
fn error_rate_counts_within_window() {
    let mut store = AdminMonitoringStore::with_retention(AdminMonitoringRetention {
        ipc_samples: 10,
        memory_samples: 10,
        error_events: 10,
    });
    let start = Instant::now();

    store.register_session(4, 400);

    store.record_error_at(4, SessionErrorKind::PipeConnect, start - Duration::from_secs(90));
    store.record_error_at(4, SessionErrorKind::PipeAuth, start - Duration::from_secs(30));
    store.record_error_at(4, SessionErrorKind::IpcDispatch, start);

    let snapshot = store.snapshot(4, start).expect("snapshot");
    assert_eq!(snapshot.errors.total_errors, 3);
    assert_eq!(snapshot.errors.errors_per_minute, 2);
    assert_eq!(
        snapshot.errors.last_error_kind,
        Some(SessionErrorKind::IpcDispatch)
    );
}
```

### PID Rebinding

```rust
#[test]
fn rebinding_pid_preserves_previous_session() {
    let mut store = AdminMonitoringStore::new();
    let now = Instant::now();

    store.register_session(1, 100);
    store.record_ipc_latency_at(1, 50, now);

    store.register_session(2, 100);

    let previous = store.snapshot(1, now).expect("previous client");
    assert_eq!(previous.state, MonitoredSessionState::Absent);
    assert!(previous.ipc_latency.p50_ms.is_some());

    let current = store.snapshot(2, now).expect("current client");
    assert_eq!(current.state, MonitoredSessionState::Active);
}
```

### Sample Retention

```rust
#[test]
fn samples_trim_to_retention_limit() {
    let mut store = AdminMonitoringStore::with_retention(AdminMonitoringRetention {
        ipc_samples: 3,
        memory_samples: 10,
        error_events: 10,
    });
    let start = Instant::now();

    store.register_session(5, 500);

    for i in 0..5 {
        store.record_ipc_latency_at(5, (i + 1) * 10, start + Duration::from_secs(i));
    }

    let snapshot = store.snapshot(5, start + Duration::from_secs(5)).expect("snapshot");
    assert_eq!(snapshot.ipc_latency.sample_count, 3);
    assert_eq!(snapshot.ipc_latency.p50_ms, Some(40));
}
```

## Running Tests

```bash
cargo test admin_monitoring
cargo test -p textquest -- admin_monitoring
cargo test --doc -p textquest admin_monitoring
```

## Integration Testing

### With Orchestrator

```rust
#[test]
fn orchestrator_integration_monitoring_snapshot() {
    let mut orch = create_test_orchestrator();

    let client_id = orch.add_client("Frostreaver", "p200");
    orch.bind_monitored_client(client_id, 1234);

    orch.record_ipc_latency(client_id, 45);

    let snapshot = orch.monitoring_snapshot(client_id).expect("snapshot");
    assert!(snapshot.ipc_latency.last_ms.is_some());
}
```

### With Metrics Collector

```rust
#[test]
fn monitoring_with_in_memory_metrics() {
    use textquest_common::observability::{Metric, MetricKind, InMemoryCollector};

    let collector = InMemoryCollector::new();
    let monitoring = AdminMonitoringStore::new();

    monitoring.register_session(1, 100);

    for i in 0..10 {
        let latency = ((i + 1) * 10) as u64;
        monitoring.record_ipc_latency(1, latency);
        collector.record(Metric {
            name: "ipc_latency_ms".to_string(),
            kind: MetricKind::Histogram,
            value: latency as f64,
            labels: vec![("client_id".to_string(), "1".to_string())],
            timestamp: SystemTime::now(),
        });
    }

    let monitoring_snapshot = monitoring.snapshot(1, Instant::now()).unwrap();
    let avg_collector = collector.average_by_name("ipc_latency_ms");

    assert!(avg_collector.is_some());
    assert!(monitoring_snapshot.ipc_latency.p50_ms.is_some());
}
```

## Fuzzing

The bounded `VecDeque` storage inherently limits memory growth:

```rust
#[test]
fn stress_retention_limits_prevent_unbounded_memory() {
    let mut store = AdminMonitoringStore::with_retention(AdminMonitoringRetention {
        ipc_samples: 120,
        memory_samples: 120,
        error_events: 120,
    });
    let start = Instant::now();

    store.register_session(99, 9999);

    for i in 0..10_000 {
        store.record_ipc_latency_at(99, i, start + Duration::from_millis(i));
    }

    let snapshot = store.snapshot(99, start + Duration::from_millis(10_000)).unwrap();
    assert_eq!(snapshot.ipc_latency.sample_count, 120);
}
```

## Debugging Failed Assertions

When `p50_ms` or `p95_ms` is `None`:

```rust
fn debug_snapshot(snapshot: &SessionMonitoringSnapshot) {
    eprintln!("client_id: {}", snapshot.client_id);
    eprintln!("state: {:?}", snapshot.state);
    eprintln!("IPC samples: {}", snapshot.ipc_latency.sample_count);
    eprintln!("last_ms: {:?}", snapshot.ipc_latency.last_ms);
    eprintln!("p50_ms: {:?}", snapshot.ipc_latency.p50_ms);
    eprintln!("memory samples: {}", snapshot.memory.sample_count);
    eprintln!("current_bytes: {:?}", snapshot.memory.current_bytes);
    eprintln!("errors: {}", snapshot.errors.total_errors);
}
```

## Platform-Specific

Memory sampling is platform-specific:

| Platform | Function | Returns |
|----------|----------|---------|
| Windows | `sample_process_memory_bytes(pid)` | Working set size |
| Linux | `sample_process_memory_bytes(pid)` | VmRSS |
| Other | `sample_process_memory_bytes(pid)` | `None` |

```rust
#[test]
#[cfg(windows)]
fn memory_sample_windows() {
    let bytes = sample_process_memory_bytes(1234);
    assert!(bytes.is_some());
}

#[test]
#[cfg(not(windows))]
fn memory_sample_nonwindows() {
    let bytes = sample_process_memory_bytes(1234);
    assert!(bytes.is_none());
}
```

## Checklist

- [ ] Session lifecycle tests cover register, exit, absent states
- [ ] IPC latency tests verify percentile calculations
- [ ] Memory trend tests verify growth rate with 2+ samples
- [ ] Error rate tests verify 60-second window filtering
- [ ] PID rebinding tests verify previous session history preserved
- [ ] Retention limit tests verify bounded VecDeque
- [ ] Platform-specific tests use `#[cfg(...)]`