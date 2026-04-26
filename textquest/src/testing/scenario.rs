//! `TestScenario` trait and supporting types for scenario-based integration testing.
//!
//! A `TestScenario` encapsulates a named, async, time-bounded test that collects
//! structured metrics and error strings as evidence of pass/fail.

use std::{collections::HashMap, future::Future, pin::Pin, time::Duration};

use textquest_common::login::AccountInfo;
use textquest_common::types::SpawnData;

// ── Metric value ────────────────────────────────────────────────────────────

/// A single metric observation produced by a [`TestScenario`].
#[derive(Debug, Clone, PartialEq)]
pub enum MetricValue {
    /// A monotonically-increasing count (e.g. "frames processed").
    Counter(u64),
    /// An instantaneous measurement (e.g. "memory usage in bytes").
    Gauge(f64),
    /// A distribution of samples (e.g. "round-trip latencies in ms").
    Histogram(Vec<f64>),
}

// ── Scenario result ──────────────────────────────────────────────────────────

/// The outcome of running a [`TestScenario`].
#[derive(Debug, Clone)]
pub struct ScenarioResult {
    /// Whether the scenario completed without errors.
    pub success: bool,
    /// Wall-clock time the scenario ran for.
    pub duration: Duration,
    /// Named metrics collected during the run.
    pub metrics: HashMap<String, MetricValue>,
    /// Human-readable error messages, if any.
    pub errors: Vec<String>,
}

impl ScenarioResult {
    /// Create a successful result with no metrics or errors.
    pub fn success(duration: Duration) -> Self {
        Self {
            success: true,
            duration,
            metrics: HashMap::new(),
            errors: Vec::new(),
        }
    }

    /// Create a failed result carrying one or more error messages.
    pub fn failure(duration: Duration, errors: Vec<String>) -> Self {
        Self {
            success: false,
            duration,
            metrics: HashMap::new(),
            errors,
        }
    }

    /// Attach a metric to this result.
    pub fn with_metric(mut self, name: impl Into<String>, value: MetricValue) -> Self {
        self.metrics.insert(name.into(), value);
        self
    }
}

// ── TestScenario trait ───────────────────────────────────────────────────────

/// A named, async scenario that runs for a bounded duration and returns structured results.
///
/// Implementors describe a self-contained behaviour test.  The orchestrator
/// calls [`run`](TestScenario::run) with a wall-clock budget; the scenario
/// should honour that budget and return a [`ScenarioResult`] regardless of
/// whether it succeeded.
/// A boxed, pinned `Send` future — the return type of [`TestScenario::run`].
pub type BoxScenarioFuture<'a> = Pin<Box<dyn Future<Output = ScenarioResult> + Send + 'a>>;

/// A named, async scenario that runs for a bounded duration and returns structured results.
///
/// Implementors describe a self-contained behaviour test.  The orchestrator
/// calls [`run`](TestScenario::run) with a wall-clock budget; the scenario
/// should honour that budget and return a [`ScenarioResult`] regardless of
/// whether it succeeded.
///
/// The trait is object-safe (`dyn TestScenario`) — `run` returns a
/// `Pin<Box<dyn Future>>` rather than using RPITIT.
pub trait TestScenario: Send {
    /// A short, human-readable identifier for this scenario (e.g. `"spawn_read_roundtrip"`).
    fn name(&self) -> &str;

    /// Execute the scenario for at most `duration`, returning structured results.
    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_>;
}

// ── Stub scenario implementations ──────────────────────────────────────────

/// Common mock scenario that returns a prebuilt result and can be reused across
/// multiple tests.
pub struct MockScenario {
    name: String,
    result: ScenarioResult,
    runs: u64,
}

impl MockScenario {
    /// Create a successful mock scenario.
    #[must_use]
    pub fn pass(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            result: ScenarioResult::success(Duration::from_millis(0)),
            runs: 0,
        }
    }

    /// Create a failing mock scenario.
    #[must_use]
    pub fn fail(name: impl Into<String>, errors: Vec<String>) -> Self {
        Self {
            name: name.into(),
            result: ScenarioResult::failure(Duration::from_millis(0), errors),
            runs: 0,
        }
    }

    /// Attach a metric to the built result.
    #[must_use]
    pub fn with_metric(mut self, name: impl Into<String>, value: MetricValue) -> Self {
        self.result.metrics.insert(name.into(), value);
        self
    }

    /// Return the number of times this scenario has run.
    pub fn run_count(&self) -> u64 {
        self.runs
    }
}

impl TestScenario for MockScenario {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        self.runs += 1;
        let mut result = self.result.clone();
        result.duration = duration;
        result
            .metrics
            .insert("mock_run".to_string(), MetricValue::Counter(self.runs));
        Box::pin(async move { result })
    }
}

/// A short-lived countdown scenario useful for verifying runner short-circuit
/// behavior and timeout control.
pub struct CountdownScenario {
    name: String,
    remaining: u64,
}

impl CountdownScenario {
    /// Create a countdown scenario with an explicit number of allowed runs.
    #[must_use]
    pub fn new(name: impl Into<String>, ticks: u64) -> Self {
        Self {
            name: name.into(),
            remaining: ticks,
        }
    }

    /// Create a deterministic "ready to fail" countdown.
    #[must_use]
    pub fn exhausted(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            remaining: 0,
        }
    }

    /// Remaining ticks before failure.
    pub fn remaining(&self) -> u64 {
        self.remaining
    }
}

impl TestScenario for CountdownScenario {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        if self.remaining == 0 {
            let error = format!("{} has already reached zero", self.name);
            return Box::pin(async move { ScenarioResult::failure(duration, vec![error]) });
        }

        self.remaining -= 1;
        let mut result = ScenarioResult::success(duration)
            .with_metric("remaining_ticks", MetricValue::Counter(self.remaining))
            .with_metric("runs", MetricValue::Counter(1));
        result.duration = duration;
        Box::pin(async move { result })
    }
}

/// Scenario that fails immediately with a configurable error.
pub struct FastFailScenario {
    name: String,
    error: String,
}

impl FastFailScenario {
    /// Create a scenario that always returns a single error.
    #[must_use]
    pub fn new(name: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            error: error.into(),
        }
    }
}

impl TestScenario for FastFailScenario {
    fn name(&self) -> &str {
        &self.name
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        let error = self.error.clone();
        Box::pin(async move { ScenarioResult::failure(duration, vec![error]) })
    }
}

// ── Test data generators ───────────────────────────────────────────────────

/// Build a compact `AccountInfo` value for scenario tests.
#[must_use]
pub fn account_info(
    account: impl Into<String>,
    character: impl Into<String>,
    class: impl Into<String>,
) -> AccountInfo {
    AccountInfo {
        account_name: account.into(),
        character_name: character.into(),
        class_name: class.into(),
        level: 60,
        group_id: 1,
        server_name: "TestServer".to_string(),
    }
}

/// Build a realistic-looking spawn entry for deterministic scenario testing.
#[must_use]
pub fn spawn_entry(id: u32, name: impl Into<String>, level: u8) -> SpawnData {
    SpawnData {
        spawn_id: id,
        name: name.into(),
        displayed_name: String::new(),
        spawn_type: 1,
        level,
        class_id: 1,
        race_id: 1,
        x: 100.0 + id as f32,
        y: 200.0 + id as f32,
        z: 0.0,
        heading: 0.0,
        hp_current: 1000,
        hp_max: 1000,
        mana_current: 1000,
        mana_max: 1000,
        endurance_current: 1000,
        endurance_max: 1000,
        speed_run: 0.0,
        stand_state: 0,
        is_gm: false,
    }
}

/// Build a small list of nearby spawns around a stable seed.
#[must_use]
pub fn spawn_wave(count: usize, seed_id: u32, base_name: impl Into<String>) -> Vec<SpawnData> {
    let name_prefix = base_name.into();
    (0..count)
        .map(|index| {
            let id = seed_id + index as u32;
            spawn_entry(id, format!("{name_prefix}-{id}"), 55)
        })
        .collect()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Minimal scenario that always succeeds immediately.
    struct AlwaysPass {
        counter: u64,
    }

    impl TestScenario for AlwaysPass {
        fn name(&self) -> &str {
            "always_pass"
        }

        fn run(&mut self, _duration: Duration) -> BoxScenarioFuture<'_> {
            self.counter += 1;
            Box::pin(async { ScenarioResult::success(Duration::from_millis(0)) })
        }
    }

    /// Scenario that always fails with a specific error message.
    struct AlwaysFail;

    impl TestScenario for AlwaysFail {
        fn name(&self) -> &str {
            "always_fail"
        }

        fn run(&mut self, _duration: Duration) -> BoxScenarioFuture<'_> {
            Box::pin(async {
                ScenarioResult::failure(
                    Duration::from_millis(1),
                    vec!["intentional failure".to_string()],
                )
            })
        }
    }

    /// Scenario that emits several metric types.
    struct MetricEmitter;

    impl TestScenario for MetricEmitter {
        fn name(&self) -> &str {
            "metric_emitter"
        }

        fn run(&mut self, _duration: Duration) -> BoxScenarioFuture<'_> {
            Box::pin(async {
                ScenarioResult::success(Duration::from_millis(5))
                    .with_metric("frames", MetricValue::Counter(100))
                    .with_metric("memory_mb", MetricValue::Gauge(42.5))
                    .with_metric(
                        "latency_ms",
                        MetricValue::Histogram(vec![1.0, 2.5, 3.0, 1.5]),
                    )
            })
        }
    }

    // ── tests ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_always_pass_scenario_succeeds() {
        let mut scenario = AlwaysPass { counter: 0 };
        assert_eq!(scenario.name(), "always_pass");

        let result = scenario.run(Duration::from_secs(1)).await;

        assert!(result.success, "AlwaysPass should succeed");
        assert!(result.errors.is_empty(), "AlwaysPass should have no errors");
        assert_eq!(scenario.counter, 1, "counter should be incremented");
    }

    #[tokio::test]
    async fn test_always_fail_scenario_reports_errors() {
        let mut scenario = AlwaysFail;
        assert_eq!(scenario.name(), "always_fail");

        let result = scenario.run(Duration::from_secs(1)).await;

        assert!(!result.success, "AlwaysFail should not succeed");
        assert!(
            !result.errors.is_empty(),
            "AlwaysFail should have at least one error"
        );
        assert_eq!(result.errors[0], "intentional failure");
    }

    #[tokio::test]
    async fn test_metric_emitter_produces_all_metric_types() {
        let mut scenario = MetricEmitter;
        assert_eq!(scenario.name(), "metric_emitter");

        let result = scenario.run(Duration::from_secs(1)).await;

        assert!(result.success);

        assert!(
            matches!(
                result.metrics.get("frames"),
                Some(MetricValue::Counter(100))
            ),
            "expected Counter(100) for 'frames'"
        );
        assert!(
            matches!(result.metrics.get("memory_mb"), Some(MetricValue::Gauge(v)) if (*v - 42.5).abs() < f64::EPSILON),
            "expected Gauge(42.5) for 'memory_mb'"
        );
        assert!(
            matches!(result.metrics.get("latency_ms"), Some(MetricValue::Histogram(v)) if v.len() == 4),
            "expected Histogram with 4 samples for 'latency_ms'"
        );
    }

    #[test]
    fn test_scenario_result_success_builder() {
        let d = Duration::from_millis(123);
        let r = ScenarioResult::success(d);
        assert!(r.success);
        assert_eq!(r.duration, d);
        assert!(r.metrics.is_empty());
        assert!(r.errors.is_empty());
    }

    #[test]
    fn test_scenario_result_failure_builder() {
        let d = Duration::from_millis(50);
        let r = ScenarioResult::failure(d, vec!["oops".into()]);
        assert!(!r.success);
        assert_eq!(r.errors, vec!["oops"]);
    }

    #[test]
    fn test_scenario_result_duration_is_recorded() {
        let start = Instant::now();
        let elapsed = start.elapsed();
        // Sanity: duration round-trips correctly through ScenarioResult.
        let r = ScenarioResult::success(elapsed);
        assert_eq!(r.duration, elapsed);
    }
}
