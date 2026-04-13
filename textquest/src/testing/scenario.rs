//! `TestScenario` trait and supporting types for scenario-based integration testing.
//!
//! A `TestScenario` encapsulates a named, async, time-bounded test that collects
//! structured metrics and error strings as evidence of pass/fail.

use std::collections::HashMap;
use std::time::Duration;

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
pub trait TestScenario: Send {
    /// A short, human-readable identifier for this scenario (e.g. `"spawn_read_roundtrip"`).
    fn name(&self) -> &str;

    /// Execute the scenario for at most `duration`, returning structured results.
    fn run(
        &mut self,
        duration: Duration,
    ) -> impl std::future::Future<Output = ScenarioResult> + Send;
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

        async fn run(&mut self, _duration: Duration) -> ScenarioResult {
            self.counter += 1;
            ScenarioResult::success(Duration::from_millis(0))
        }
    }

    /// Scenario that always fails with a specific error message.
    struct AlwaysFail;

    impl TestScenario for AlwaysFail {
        fn name(&self) -> &str {
            "always_fail"
        }

        async fn run(&mut self, _duration: Duration) -> ScenarioResult {
            ScenarioResult::failure(
                Duration::from_millis(1),
                vec!["intentional failure".to_string()],
            )
        }
    }

    /// Scenario that emits several metric types.
    struct MetricEmitter;

    impl TestScenario for MetricEmitter {
        fn name(&self) -> &str {
            "metric_emitter"
        }

        async fn run(&mut self, _duration: Duration) -> ScenarioResult {
            ScenarioResult::success(Duration::from_millis(5))
                .with_metric("frames", MetricValue::Counter(100))
                .with_metric("memory_mb", MetricValue::Gauge(42.5))
                .with_metric(
                    "latency_ms",
                    MetricValue::Histogram(vec![1.0, 2.5, 3.0, 1.5]),
                )
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
