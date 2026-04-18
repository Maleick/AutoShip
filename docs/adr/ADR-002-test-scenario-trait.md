# ADR-002: Test Scenario Trait Architecture

**Status:** Accepted  
**Date:** 2026-04-18  
**Author:** TextQuest Team

## Context

TextQuest runs multi-account, multi-group automation scenarios on live EQ servers. Testing requires:

- **Named scenarios** — "spawn_read_roundtrip", "vendor_transaction", "zone_transition_loop"
- **Bounded execution** — Run for at most N seconds, then stop (no infinite loops on CI)
- **Structured metrics** — Collect counters, gauges, and histograms during execution
- **Error evidence** — Return error strings explaining why a scenario failed
- **Async execution** — Work with tokio spawned tasks and IPC wait channels
- **Pluggability** — Add new scenarios without modifying the test runner

Monolithic test functions don't scale: each scenario becomes a copy-paste variation with its own timeout logic, metric collection, and result handling.

## Decision

Implement a **trait-based scenario harness** in `textquest/src/testing/scenario.rs`:

```rust
/// A named, async scenario that runs for a bounded duration and returns structured results.
pub trait TestScenario: Send {
    /// A short, human-readable identifier for this scenario.
    fn name(&self) -> &str;

    /// Execute the scenario for at most `duration`, returning structured results.
    async fn run(&mut self, duration: Duration) -> ScenarioResult;
}

pub enum MetricValue {
    Counter(u64),     // monotonically increasing
    Gauge(f64),       // instantaneous measurement
    Histogram(Vec<f64>), // distribution of samples
}

pub struct ScenarioResult {
    pub success: bool,
    pub duration: Duration,
    pub metrics: HashMap<String, MetricValue>,
    pub errors: Vec<String>,
}
```

Implementors write concrete scenarios:

```rust
struct SpawnReadRoundtrip {
    ipc_client: Arc<IpcClient>,
}

impl TestScenario for SpawnReadRoundtrip {
    fn name(&self) -> &str { "spawn_read_roundtrip" }

    async fn run(&mut self, duration: Duration) -> ScenarioResult {
        let start = Instant::now();
        let mut latencies = Vec::new();
        let mut errors = Vec::new();

        while start.elapsed() < duration {
            match self.ipc_client.read_spawn().await {
                Ok((spawn, latency_ms)) => latencies.push(latency_ms as f64),
                Err(e) => errors.push(format!("spawn read failed: {}", e)),
            }
        }

        let mut result = if errors.is_empty() {
            ScenarioResult::success(start.elapsed())
        } else {
            ScenarioResult::failure(start.elapsed(), errors)
        };
        result = result.with_metric("roundtrip_ms", MetricValue::Histogram(latencies));
        result
    }
}
```

## Rationale

1. **Composition Over Inheritance** — Trait-based design allows mixing scenarios with different behaviors without deep class hierarchies.

2. **Timeout Built-in** — Every scenario respects a hard duration limit. No special handling needed per test; the harness enforces it.

3. **Flexible Metrics** — Counters, gauges, and histograms cover 90% of observability needs. Scenarios can compute percentiles post-run if needed.

4. **Testability** — Scenarios are unit-testable in isolation (mock IPC, mock duration, check results).

5. **Extensibility** — New scenarios are single structs + one impl block. No test framework changes needed.

6. **Structured Errors** — `ScenarioResult.errors` is a `Vec<String>`, not a panic. Tests can aggregate errors from multiple scenarios and report them cleanly.

## Implementation Notes

- Scenarios are assumed to be **Send** (may span threads/tasks). If a scenario needs blocking I/O, wrap it in `spawn_blocking`.
- The orchestrator runner calls `scenario.run(duration)` and records the result in a test output log.
- Histograms (e.g., latencies) are stored as `Vec<f64>`. Percentile calculation happens at report time, not during collection.
- Each scenario owns its IPC client, spawned tasks, and cleanup. If cleanup fails, record it in `errors`.

## Alternatives Considered

1. **Callback-based** — Pass closure functions for setup/run/teardown. Less clear ownership; harder to share state.
2. **Monolithic test functions** — Write each test as a separate async function. Requires duplicate timeout logic and metric collection in each test.
3. **BDD-style macros** — Cucumber-like DSL. Overkill for the use case; hides the actual async logic.

## Related ADRs

- ADR-004: Per-Account Test Runners (orchestrates multiple TestScenario instances)
- ADR-006: Metrics with Percentiles (post-processes histogram data from scenarios)

## References

- `textquest/src/testing/scenario.rs` — TestScenario trait and ScenarioResult
- `textquest/src/testing/` — Concrete scenario implementations
