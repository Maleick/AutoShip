//! Platform-independent mock scenarios for testing the scenario harness.
//!
//! These implementations do not touch EverQuest clients, process memory, the
//! filesystem, or the network. They are intended for CI-safe unit and
//! integration tests that need deterministic success, failure, duration, or
//! metric output.

use std::time::{Duration, Instant};

use crate::testing::scenario::{BoxScenarioFuture, MetricValue, ScenarioResult, TestScenario};

/// Completes immediately without metrics.
#[derive(Debug, Default)]
pub struct InstantScenario;

impl InstantScenario {
    /// Create an instant success scenario.
    pub fn new() -> Self {
        Self
    }
}

impl TestScenario for InstantScenario {
    fn name(&self) -> &str {
        "instant"
    }

    fn run(&mut self, _duration: Duration) -> BoxScenarioFuture<'_> {
        Box::pin(async { ScenarioResult::success(Duration::ZERO) })
    }
}

/// Waits for a configured duration and returns fixed countdown metrics.
#[derive(Debug, Clone)]
pub struct CountdownScenario {
    target_duration: Duration,
}

impl CountdownScenario {
    /// Create a countdown scenario that runs for `target_duration`.
    pub fn new(target_duration: Duration) -> Self {
        Self { target_duration }
    }

    /// Duration this scenario targets when the runner budget allows it.
    pub fn target_duration(&self) -> Duration {
        self.target_duration
    }
}

impl TestScenario for CountdownScenario {
    fn name(&self) -> &str {
        "countdown"
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        let target_duration = self.target_duration;
        Box::pin(async move {
            let run_duration = target_duration.min(duration);
            let started = Instant::now();
            tokio::time::sleep(run_duration).await;

            let completed = run_duration == target_duration;
            let mut result = if completed {
                ScenarioResult::success(started.elapsed())
            } else {
                ScenarioResult::failure(
                    started.elapsed(),
                    vec![format!(
                        "countdown budget exhausted before {:?}",
                        target_duration
                    )],
                )
            };

            result = result
                .with_metric(
                    "countdown_completed",
                    MetricValue::Counter(if completed { 1 } else { 0 }),
                )
                .with_metric(
                    "target_duration_ms",
                    MetricValue::Gauge(target_duration.as_secs_f64() * 1000.0),
                );
            result
        })
    }
}

/// Fails after a short configurable delay.
#[derive(Debug, Clone)]
pub struct FastFailScenario {
    delay: Duration,
    error: String,
}

impl FastFailScenario {
    /// Create a scenario that fails after `delay`.
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            error: "fast failure requested".to_string(),
        }
    }

    /// Override the error message returned by the failure.
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = error.into();
        self
    }
}

impl Default for FastFailScenario {
    fn default() -> Self {
        Self::new(Duration::from_millis(10))
    }
}

impl TestScenario for FastFailScenario {
    fn name(&self) -> &str {
        "fast_fail"
    }

    fn run(&mut self, duration: Duration) -> BoxScenarioFuture<'_> {
        let delay = self.delay.min(duration);
        let error = self.error.clone();
        Box::pin(async move {
            let started = Instant::now();
            tokio::time::sleep(delay).await;
            ScenarioResult::failure(started.elapsed(), vec![error])
        })
    }
}

/// Returns caller-controlled metrics for aggregation tests.
#[derive(Debug, Clone)]
pub struct MetricTestScenario {
    duration: Duration,
    metrics: Vec<(String, MetricValue)>,
}

impl MetricTestScenario {
    /// Create a metric scenario with the provided result duration and metrics.
    pub fn new<S: Into<String>>(duration: Duration, metrics: Vec<(S, MetricValue)>) -> Self {
        Self {
            duration,
            metrics: metrics
                .into_iter()
                .map(|(name, value)| (name.into(), value))
                .collect(),
        }
    }

    /// Create a metric scenario with representative counter, gauge, and
    /// histogram values.
    pub fn standard() -> Self {
        Self::new(
            Duration::from_millis(5),
            vec![
                ("iterations", MetricValue::Counter(3)),
                ("load", MetricValue::Gauge(0.75)),
                ("latency_ms", MetricValue::Histogram(vec![1.0, 2.0, 3.0])),
            ],
        )
    }
}

impl TestScenario for MetricTestScenario {
    fn name(&self) -> &str {
        "metric_test"
    }

    fn run(&mut self, _duration: Duration) -> BoxScenarioFuture<'_> {
        let duration = self.duration;
        let metrics = self.metrics.clone();
        Box::pin(async move {
            metrics.into_iter().fold(
                ScenarioResult::success(duration),
                |result, (name, value)| result.with_metric(name, value),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_timing_near(actual: Duration, expected: Duration) {
        assert!(
            actual >= expected,
            "actual duration {actual:?} should be at least expected {expected:?}"
        );
        assert!(
            actual <= expected + Duration::from_millis(250),
            "actual duration {actual:?} should stay close to expected {expected:?}"
        );
    }

    #[tokio::test]
    async fn instant_scenario_completes_without_metrics() {
        let mut scenario = InstantScenario::new();

        let result = scenario.run(Duration::from_secs(1)).await;

        assert_eq!(scenario.name(), "instant");
        assert!(result.success);
        assert_eq!(result.duration, Duration::ZERO);
        assert!(result.metrics.is_empty());
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn countdown_scenario_runs_for_target_duration() {
        let target = Duration::from_millis(20);
        let mut scenario = CountdownScenario::new(target);
        let started = Instant::now();

        let result = scenario.run(Duration::from_secs(1)).await;
        let elapsed = started.elapsed();

        assert_eq!(scenario.name(), "countdown");
        assert!(result.success);
        assert_timing_near(elapsed, target);
        assert_timing_near(result.duration, target);
        assert_eq!(
            result.metrics.get("countdown_completed"),
            Some(&MetricValue::Counter(1))
        );
        assert_eq!(
            result.metrics.get("target_duration_ms"),
            Some(&MetricValue::Gauge(20.0))
        );
    }

    #[tokio::test]
    async fn fast_fail_scenario_fails_after_delay() {
        let delay = Duration::from_millis(5);
        let mut scenario = FastFailScenario::new(delay).with_error("expected failure");

        let result = scenario.run(Duration::from_secs(1)).await;

        assert_eq!(scenario.name(), "fast_fail");
        assert!(!result.success);
        assert_timing_near(result.duration, delay);
        assert_eq!(result.errors, vec!["expected failure"]);
        assert!(result.metrics.is_empty());
    }

    #[tokio::test]
    async fn metric_test_scenario_returns_controlled_metrics() {
        let mut scenario = MetricTestScenario::new(
            Duration::from_millis(7),
            vec![
                ("events", MetricValue::Counter(12)),
                ("cpu", MetricValue::Gauge(0.5)),
                ("samples", MetricValue::Histogram(vec![2.0, 4.0])),
            ],
        );

        let result = scenario.run(Duration::from_secs(1)).await;

        assert_eq!(scenario.name(), "metric_test");
        assert!(result.success);
        assert_eq!(result.duration, Duration::from_millis(7));
        assert_eq!(
            result.metrics.get("events"),
            Some(&MetricValue::Counter(12))
        );
        assert_eq!(result.metrics.get("cpu"), Some(&MetricValue::Gauge(0.5)));
        assert_eq!(
            result.metrics.get("samples"),
            Some(&MetricValue::Histogram(vec![2.0, 4.0]))
        );
    }
}
