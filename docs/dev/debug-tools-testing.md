# Debug Tools Testing Guide

This document covers testing practices for debug tooling in TextQuest, including debug formatting, binding diagnostics, and observability infrastructure.

## What Are Debug Tools

Debug tools in TextQuest include:

| Tool | Purpose | Location |
|------|---------|----------|
| **Debug formatting** | `Debug` trait impls for types | Various `impl Debug for T` |
| **Binding diagnostics** | `eq_fn!` call tracing | `textquest-common/src/bindings.rs` |
| **Structured logging** | `tracing` with fields | Throughout codebase |
| **Observability** | Metrics collection | `textquest-common/src/observability.rs` |
| **Test-only validation** | Debug assertions | `#[cfg(test)]` modules |

## Testing Debug Formatting

### Password Redaction

Verify that sensitive fields are redacted in debug output:

```rust
#[test]
fn command_debug_redacts_password() {
    let cmd = BoxControllerCommand::Login(LoginCredentials {
        account: "TestAccount".into(),
        password: "secret123".into(),
        server: "Teek".into(),
    });

    let debug_output = format!("{:?}", cmd);

    assert!(debug_output.contains("[REDACTED]"));
    assert!(!debug_output.contains("secret123"));
    assert!(debug_output.contains("account"));
}
```

### Enum Variant Naming

Verify enum variants appear in debug output:

```rust
#[test]
fn login_phase_debug_format() {
    let phase = LoginPhase::AtLoginScreen;
    let debug = format!("{:?}", phase);
    assert!(debug.contains("AtLoginScreen"));
}
```

### Error Formatting

Verify error messages include context:

```rust
#[test]
fn login_error_debug_format() {
    let err = LoginError::Network("connection refused".into());
    let debug = format!("{:?}", err);
    assert!(debug.contains("Foo"));
    assert!(debug.contains("Bar"));
}
```

### Struct Debug

Verify struct debug output includes key fields:

```rust
#[test]
fn resolver_debug_format() {
    let r = ServerNameResolver::from("TestServer");
    let debug = format!("{:?}", r);
    assert!(debug.contains("ServerNameResolver"));
}
```

## Testing Binding Diagnostics

### Enabling/Disabling Debug Flag

Test that the binding debug flag can be toggled:

```rust
#[test]
fn set_binding_debug_flag_is_toggleable() {
    use textquest_common::bindings::set_binding_debug_enabled;

    // Disable first
    set_binding_debug_enabled(false);

    // Enable
    set_binding_debug_enabled(true);

    // Verify it can be disabled again
    set_binding_debug_enabled(false);
}
```

### Debug Output Verification

When enabled, binding functions emit structured debug logs:

```rust
#[test]
fn binding_debug_emits_structured_logs() {
    use textquest_common::bindings::{set_binding_debug_enabled, eq_fn};
    use std::sync::Once;

    static INIT: Once = Once::new();

    INIT.call_once(|| {
        set_binding_debug_enabled(true);
    });

    // Call an eq_fn - will emit tracing::debug! when enabled
    // Logs contain function name, address, and args
}
```

## Testing Structured Logging

### Capturing Logs in Tests

Use `tracing::test::TestWriter` or custom subscriber:

```rust
#[test]
fn test_log_output() {
    use tracing::{info, warn, error, Subscriber};
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let collector = tracing_subscriber::fmt::TestWriter::new();

    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer().with_writer(collector)
    );

    subscriber.try_init().ok();

    info!(zone = "sebilis", spawn_id = 42, "test log");
}
```

### Verifying Log Fields

```rust
#[test]
fn structured_log_contains_fields() {
    // Logs use structured fields, not concatenation
    // Verify fields appear in output
}
```

## Testing Observability

### Metric Recording

```rust
#[test]
fn test_metric_recording() {
    use textquest_common::observability::{
        Metric, MetricKind, InMemoryCollector, MetricsCollector,
    };
    use std::time::SystemTime;

    let collector = InMemoryCollector::new();

    collector.record(Metric {
        name: "test_counter".to_string(),
        kind: MetricKind::Counter,
        value: 1.0,
        labels: vec![("zone".to_string(), "test".to_string())],
        timestamp: SystemTime::now(),
    });

    let sum = collector.sum_by_name("test_counter");
    assert!(sum.is_some());
}
```

### Custom Collector

```rust
#[test]
fn custom_collector_implements_trait() {
    use textquest_common::observability::{MetricsCollector, Metric};

    struct TestCollector;

    impl MetricsCollector for TestCollector {
        fn record(&self, _metric: Metric) {}
        fn all_metrics(&self) -> Vec<Metric> { vec![] }
        fn clear(&self) {}
        fn metric_count(&self) -> usize { 0 }
    }

    let collector = TestCollector;
    assert_eq!(collector.metric_count(), 0);
}
```

## Coverage Expectations

For debug tooling code:

| Component | Coverage Target |
|-----------|-----------------|
| `Debug` impls | 70%+ |
| Binding debug toggle | 80%+ |
| Redaction logic | 90%+ |
| Metrics recording | 70%+ |

Debug tooling is considered supporting infrastructure. Higher priority is core logic coverage.

## Running Debug Tool Tests

```bash
# Run all debug-related tests
cargo test debug

# Run command formatting tests
cargo test command_debug

# Run binding tests
cargo test binding

# Observability tests
cargo test metric

# With tracing output
cargo test -- --nocapture
```

## Common Patterns

### Test-Only Validation

```rust
#[cfg(test)]
mod validation {
    #[test]
    fn debug_format_includes_key_fields() {
        // Test-only validation of debug output
    }
}
```

### Snapshot Testing

```rust
#[test]
fn snapshot_camp_debug() {
    let camp = CampLoop::new(config, members);
    let debug = format!("{:?}", camp);
    assert!(debug.contains("Idle"));
}
```

### Error Context Testing

```rust
#[test]
fn error_includes_context() {
    let err = MyError::ParseError { line: 10, col: 5 };
    let debug = format!("{:?}", err);
    assert!(debug.contains("10"));
    assert!(debug.contains("5"));
}
```