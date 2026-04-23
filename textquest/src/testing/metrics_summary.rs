//! Per-scenario metrics aggregation and JSON export.
//!
//! [`MetricsSummary`] collects [`ScenarioResult`] values for a single named
//! scenario and derives statistical summaries (p50, p95, p99) from duration
//! and per-metric samples.

use std::collections::HashMap;

use serde_json::Value;

use super::scenario::{MetricValue, ScenarioResult};

// ── MetricStats ──────────────────────────────────────────────────────────────

/// Descriptive statistics for a single named metric across multiple runs.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricStats {
    /// Sum of all samples.
    pub sum: f64,
    /// Arithmetic mean.
    pub avg: f64,
    /// Minimum sample.
    pub min: f64,
    /// Maximum sample.
    pub max: f64,
    /// 50th percentile (median).
    pub p50: f64,
    /// 95th percentile.
    pub p95: f64,
    /// 99th percentile.
    pub p99: f64,
}

impl MetricStats {
    /// Build `MetricStats` from a non-empty slice of finite `f64` samples.
    ///
    /// Non-finite values (`NaN`, `±Inf`) are silently discarded before
    /// statistics are computed.  If *all* values are non-finite the returned
    /// struct will have all fields set to `0.0`.
    pub fn from_samples(raw: &[f64]) -> Self {
        let mut samples: Vec<f64> = raw.iter().copied().filter(|v| v.is_finite()).collect();
        if samples.is_empty() {
            return Self {
                sum: 0.0,
                avg: 0.0,
                min: 0.0,
                max: 0.0,
                p50: 0.0,
                p95: 0.0,
                p99: 0.0,
            };
        }
        samples.sort_by(|a, b| a.partial_cmp(b).expect("finite"));

        let sum: f64 = samples.iter().copied().sum();
        let avg = sum / samples.len() as f64;
        let min = samples[0];
        let max = *samples.last().expect("non-empty");

        Self {
            sum,
            avg,
            min,
            max,
            p50: percentile_of(&samples, 50.0),
            p95: percentile_of(&samples, 95.0),
            p99: percentile_of(&samples, 99.0),
        }
    }

    /// Serialize to a [`serde_json::Value`] object.
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "sum":  self.sum,
            "avg":  self.avg,
            "min":  self.min,
            "max":  self.max,
            "p50":  self.p50,
            "p95":  self.p95,
            "p99":  self.p99,
        })
    }
}

// ── MetricsSummary ───────────────────────────────────────────────────────────

/// Aggregated statistics for a single scenario across multiple runs.
///
/// Build one via [`MetricsSummary::from_results`] or the builder-style
/// [`MetricsSummary::new`] / [`MetricsSummary::add_result`] pair.
#[derive(Debug, Clone)]
pub struct MetricsSummary {
    /// Name of the scenario these results belong to.
    pub scenario_name: String,
    /// Total number of runs recorded.
    pub total_iterations: u32,
    /// Number of runs that completed successfully.
    pub success_count: u32,
    /// Per-named-metric statistics derived from all runs.
    pub metrics: HashMap<String, MetricStats>,
    /// All error messages collected across failed runs.
    pub errors: Vec<String>,
    // Internal: raw duration samples for building duration stats.
    duration_samples: Vec<f64>,
    // Internal: raw per-metric float samples.
    metric_samples: HashMap<String, Vec<f64>>,
}

impl MetricsSummary {
    /// Create an empty summary for `scenario_name`.
    pub fn new(scenario_name: impl Into<String>) -> Self {
        Self {
            scenario_name: scenario_name.into(),
            total_iterations: 0,
            success_count: 0,
            metrics: HashMap::new(),
            errors: Vec::new(),
            duration_samples: Vec::new(),
            metric_samples: HashMap::new(),
        }
    }

    /// Incorporate a single [`ScenarioResult`] into this summary.
    pub fn add_result(&mut self, result: &ScenarioResult) {
        self.total_iterations += 1;
        if result.success {
            self.success_count += 1;
        }
        self.errors.extend(result.errors.iter().cloned());
        self.duration_samples
            .push(result.duration.as_secs_f64() * 1_000.0); // ms

        for (name, value) in &result.metrics {
            let samples = self.metric_samples.entry(name.clone()).or_default();
            match value {
                MetricValue::Counter(n) => samples.push(*n as f64),
                MetricValue::Gauge(f) => samples.push(*f),
                MetricValue::Histogram(vs) => samples.extend_from_slice(vs),
            }
        }
    }

    /// Build statistics from accumulated raw samples.
    ///
    /// Call this once after all [`add_result`](Self::add_result) calls to
    /// populate [`Self::metrics`] with the derived [`MetricStats`].  This also
    /// adds a synthetic `"duration_ms"` entry for scenario wall-clock times.
    pub fn finalize(&mut self) {
        self.metrics.insert(
            "duration_ms".to_string(),
            MetricStats::from_samples(&self.duration_samples),
        );
        for (name, samples) in &self.metric_samples {
            self.metrics
                .insert(name.clone(), MetricStats::from_samples(samples));
        }
    }

    /// Convenience: build a finalized `MetricsSummary` from a slice of
    /// `ScenarioResult` values produced by a single named scenario.
    pub fn from_results(scenario_name: impl Into<String>, results: &[ScenarioResult]) -> Self {
        let mut summary = Self::new(scenario_name);
        for r in results {
            summary.add_result(r);
        }
        summary.finalize();
        summary
    }

    /// Fraction of runs that succeeded (`0.0`–`1.0`).
    ///
    /// Returns `0.0` for an empty summary.
    pub fn pass_rate(&self) -> f64 {
        if self.total_iterations == 0 {
            return 0.0;
        }
        self.success_count as f64 / self.total_iterations as f64
    }

    /// Serialize the summary to a [`serde_json::Value`].
    pub fn to_json(&self) -> Value {
        let metrics_json: serde_json::Map<String, Value> = self
            .metrics
            .iter()
            .map(|(k, v)| (k.clone(), v.to_json()))
            .collect();

        serde_json::json!({
            "scenario_name":    self.scenario_name,
            "total_iterations": self.total_iterations,
            "success_count":    self.success_count,
            "pass_rate":        self.pass_rate(),
            "errors":           self.errors,
            "metrics":          Value::Object(metrics_json),
        })
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// Nearest-rank percentile on a *sorted* non-empty slice.
fn percentile_of(sorted: &[f64], p: f64) -> f64 {
    let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::testing::scenario::{MetricValue, ScenarioResult};

    fn success(ms: u64) -> ScenarioResult {
        ScenarioResult::success(Duration::from_millis(ms))
    }

    fn failure(ms: u64, msg: &str) -> ScenarioResult {
        ScenarioResult::failure(Duration::from_millis(ms), vec![msg.to_string()])
    }

    // ── MetricStats ───────────────────────────────────────────────────────────

    #[test]
    fn test_metric_stats_basic() {
        let stats = MetricStats::from_samples(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert_eq!(stats.sum, 15.0);
        assert_eq!(stats.avg, 3.0);
        // p50 of sorted [1,2,3,4,5] → index 2 → 3.0
        assert_eq!(stats.p50, 3.0);
    }

    #[test]
    fn test_metric_stats_single_value() {
        let stats = MetricStats::from_samples(&[42.0]);
        assert_eq!(stats.min, 42.0);
        assert_eq!(stats.max, 42.0);
        assert_eq!(stats.p50, 42.0);
        assert_eq!(stats.p95, 42.0);
        assert_eq!(stats.p99, 42.0);
    }

    #[test]
    fn test_metric_stats_empty_returns_zeros() {
        let stats = MetricStats::from_samples(&[]);
        assert_eq!(stats.sum, 0.0);
        assert_eq!(stats.avg, 0.0);
        assert_eq!(stats.p50, 0.0);
    }

    #[test]
    fn test_metric_stats_filters_nan_and_inf() {
        // NaN and Inf are discarded; only 2.0 and 4.0 remain.
        let stats =
            MetricStats::from_samples(&[f64::NAN, 2.0, f64::INFINITY, 4.0, f64::NEG_INFINITY]);
        assert_eq!(stats.min, 2.0);
        assert_eq!(stats.max, 4.0);
        assert_eq!(stats.sum, 6.0);
    }

    #[test]
    fn test_metric_stats_all_non_finite_returns_zeros() {
        let stats = MetricStats::from_samples(&[f64::NAN, f64::INFINITY]);
        assert_eq!(stats.sum, 0.0);
        assert_eq!(stats.p99, 0.0);
    }

    #[test]
    fn test_metric_stats_to_json() {
        let stats = MetricStats::from_samples(&[10.0, 20.0, 30.0]);
        let json = stats.to_json();
        assert_eq!(json["sum"], 60.0);
        assert_eq!(json["avg"], 20.0);
        assert_eq!(json["min"], 10.0);
        assert_eq!(json["max"], 30.0);
    }

    // ── MetricsSummary ────────────────────────────────────────────────────────

    #[test]
    fn test_summary_zero_events() {
        let summary = MetricsSummary::from_results("empty", &[]);
        assert_eq!(summary.total_iterations, 0);
        assert_eq!(summary.success_count, 0);
        assert_eq!(summary.pass_rate(), 0.0);
        assert!(summary.errors.is_empty());
    }

    #[test]
    fn test_summary_single_success() {
        let results = vec![success(100)];
        let summary = MetricsSummary::from_results("s", &results);
        assert_eq!(summary.total_iterations, 1);
        assert_eq!(summary.success_count, 1);
        assert!((summary.pass_rate() - 1.0).abs() < 1e-9);
        let dur = &summary.metrics["duration_ms"];
        // 100 ms
        assert!((dur.p50 - 100.0).abs() < 1e-6);
    }

    #[test]
    fn test_summary_pass_rate() {
        let results = vec![success(10), success(20), failure(5, "boom")];
        let summary = MetricsSummary::from_results("rate_test", &results);
        assert_eq!(summary.total_iterations, 3);
        assert_eq!(summary.success_count, 2);
        assert!((summary.pass_rate() - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn test_summary_errors_collected() {
        let results = vec![failure(1, "err-a"), failure(2, "err-b")];
        let summary = MetricsSummary::from_results("failing", &results);
        assert!(summary.errors.contains(&"err-a".to_string()));
        assert!(summary.errors.contains(&"err-b".to_string()));
    }

    #[test]
    fn test_summary_duration_percentiles() {
        // Durations: 10, 20, 30, 40, 50, 60, 70, 80, 90, 100 ms
        let results: Vec<ScenarioResult> = (1..=10u64).map(|i| success(i * 10)).collect();
        let summary = MetricsSummary::from_results("perc", &results);
        let dur = &summary.metrics["duration_ms"];
        assert_eq!(dur.min, 10.0);
        assert_eq!(dur.max, 100.0);
        // p50 of 10 sorted values: index round(0.5 * 9) = round(4.5) = 5 → value[5] = 60.0
        assert_eq!(dur.p50, 60.0);
    }

    #[test]
    fn test_summary_named_metric_counter() {
        let result = ScenarioResult::success(Duration::from_millis(10))
            .with_metric("frames", MetricValue::Counter(100));
        let summary = MetricsSummary::from_results("counter_test", &[result]);
        let stats = &summary.metrics["frames"];
        assert_eq!(stats.sum, 100.0);
        assert_eq!(stats.min, 100.0);
    }

    #[test]
    fn test_summary_named_metric_gauge() {
        let r1 = ScenarioResult::success(Duration::from_millis(10))
            .with_metric("cpu", MetricValue::Gauge(0.3));
        let r2 = ScenarioResult::success(Duration::from_millis(10))
            .with_metric("cpu", MetricValue::Gauge(0.7));
        let summary = MetricsSummary::from_results("gauge_test", &[r1, r2]);
        let stats = &summary.metrics["cpu"];
        assert!((stats.avg - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_summary_named_metric_histogram() {
        let result = ScenarioResult::success(Duration::from_millis(10))
            .with_metric("latency", MetricValue::Histogram(vec![1.0, 2.0, 3.0]));
        let summary = MetricsSummary::from_results("hist_test", &[result]);
        let stats = &summary.metrics["latency"];
        assert_eq!(stats.sum, 6.0);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 3.0);
    }

    #[test]
    fn test_summary_to_json() {
        let results = vec![success(50), failure(60, "oops")];
        let summary = MetricsSummary::from_results("json_test", &results);
        let json = summary.to_json();
        assert_eq!(json["scenario_name"], "json_test");
        assert_eq!(json["total_iterations"], 2);
        assert_eq!(json["success_count"], 1);
        assert!((json["pass_rate"].as_f64().unwrap() - 0.5).abs() < 1e-9);
        assert!(json["metrics"]["duration_ms"].is_object());
        // errors array should contain "oops"
        let errs = json["errors"].as_array().unwrap();
        assert!(errs.iter().any(|e| e.as_str() == Some("oops")));
    }
}
