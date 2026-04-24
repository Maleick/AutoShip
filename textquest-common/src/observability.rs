//! Observability infrastructure for metrics collection and structured logging.
//!
//! This module provides pluggable metrics collection with support for counters,
//! gauges, and histograms. The trait-based design allows different backends
//! (in-memory, Prometheus, etc.) without core dependencies.
//!
//! # Logging Conventions
//!
//! - Use the `tracing` crate with structured fields
//! - Log levels: ERROR (crashes), WARN (recovery), INFO (state changes), DEBUG
//!   (IPC details)
//! - File output: `./logs/textquest.log` (orchestrator),
//!   `%TEMP%/textquest/textquest-dll.log` (DLL)
//! - Never use `println!` / `eprintln!` for structured logs (reserved for
//!   CLI/TUI output only)
//!
//! # Metrics Naming Conventions
//!
//! - Use snake_case for metric names: `command_latency_ms`, `spawn_count`,
//!   `ipc_errors_total`
//! - Include unit in the name when appropriate: `_ms`, `_bytes`, `_total`
//! - Use labels/tags for dimensionality: `{"client_id": "42", "zone":
//!   "sebilis"}`
//!
//! # Example
//!
//! ```ignore
//! use textquest_common::observability::{MetricKind, Metric, InMemoryCollector, MetricsCollector};
//! use std::time::{SystemTime, UNIX_EPOCH};
//!
//! let mut collector = InMemoryCollector::new();
//!
//! // Record a counter metric
//! let metric = Metric {
//!     name: "spawn_count".to_string(),
//!     kind: MetricKind::Counter,
//!     value: 5.0,
//!     labels: vec![("zone".to_string(), "sebilis".to_string())],
//!     timestamp: SystemTime::now(),
//! };
//! collector.record(metric);
//!
//! // Record a gauge metric
//! let memory_metric = Metric {
//!     name: "process_memory_mb".to_string(),
//!     kind: MetricKind::Gauge,
//!     value: 256.5,
//!     labels: vec![],
//!     timestamp: SystemTime::now(),
//! };
//! collector.record(memory_metric);
//! ```

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

/// Wall-clock latency budget for one TUI render-thread tick.
pub const TUI_TICK_WARN_BUDGET: Duration = Duration::from_millis(50);

/// Return whether a TUI tick should emit a latency warning.
pub fn should_warn_tui_tick_latency(elapsed: Duration, perf_trace_enabled: bool) -> bool {
    perf_trace_enabled && elapsed > TUI_TICK_WARN_BUDGET
}

/// Convert a TUI tick wall-clock duration into the logged millisecond field.
pub fn tui_tick_latency_elapsed_ms(elapsed: Duration) -> u128 {
    elapsed.as_millis()
}

/// Metric kind enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricKind {
    /// Counter: monotonically increasing value
    Counter,
    /// Gauge: point-in-time measurement
    Gauge,
    /// Histogram: distribution of values
    Histogram,
}

/// A single metric observation with name, kind, value, and labels.
#[derive(Debug, Clone)]
pub struct Metric {
    /// Metric name (e.g., "command_latency_ms")
    pub name: String,
    /// Metric kind (Counter, Gauge, or Histogram)
    pub kind: MetricKind,
    /// Metric value
    pub value: f64,
    /// Optional labels for dimensionality (e.g., zone, client_id)
    pub labels: Vec<(String, String)>,
    /// Timestamp of observation
    pub timestamp: SystemTime,
}

/// Pluggable metrics collection backend.
///
/// Implementers can provide different storage and export strategies
/// (in-memory aggregation, Prometheus push, etc.) without changing the caller
/// API.
pub trait MetricsCollector: Send + Sync {
    /// Record a single metric observation.
    fn record(&self, metric: Metric);

    /// Retrieve all recorded metrics (implementation-dependent).
    ///
    /// Returns a vector of all metrics currently stored.
    fn all_metrics(&self) -> Vec<Metric>;

    /// Clear all recorded metrics.
    fn clear(&self);

    /// Return the count of recorded metrics.
    fn metric_count(&self) -> usize;
}

/// In-memory metrics collector for testing and ephemeral storage.
///
/// Thread-safe implementation using Arc<Mutex<>>. Suitable for integration
/// tests and scenarios where metrics need to be queried synchronously.
pub struct InMemoryCollector {
    metrics: Arc<Mutex<Vec<Metric>>>,
}

impl InMemoryCollector {
    /// Create a new empty in-memory collector.
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a new collector with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            metrics: Arc::new(Mutex::new(Vec::with_capacity(capacity))),
        }
    }

    /// Get a summary of metrics by name and kind.
    ///
    /// Returns a map from metric name to a tuple of (kind, count).
    pub fn summary(&self) -> HashMap<String, (MetricKind, usize)> {
        let metrics = self
            .metrics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut summary = HashMap::new();

        for metric in metrics.iter() {
            let entry = summary
                .entry(metric.name.clone())
                .or_insert((metric.kind, 0));
            entry.1 += 1;
        }

        summary
    }

    /// Get all metrics of a specific kind.
    pub fn metrics_by_kind(&self, kind: MetricKind) -> Vec<Metric> {
        self.metrics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|m| m.kind == kind)
            .cloned()
            .collect()
    }

    /// Get metrics by name.
    pub fn metrics_by_name(&self, name: &str) -> Vec<Metric> {
        self.metrics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|m| m.name == name)
            .cloned()
            .collect()
    }

    /// Get the sum of values for all metrics with a given name.
    ///
    /// Useful for aggregating counters or summing gauge readings.
    pub fn sum_by_name(&self, name: &str) -> f64 {
        self.metrics_by_name(name).iter().map(|m| m.value).sum()
    }

    /// Get the average value for all metrics with a given name.
    pub fn average_by_name(&self, name: &str) -> Option<f64> {
        let metrics = self.metrics_by_name(name);
        if metrics.is_empty() {
            return None;
        }
        let sum: f64 = metrics.iter().map(|m| m.value).sum();
        Some(sum / metrics.len() as f64)
    }
}

impl Default for InMemoryCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector for InMemoryCollector {
    fn record(&self, metric: Metric) {
        self.metrics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(metric);
    }

    fn all_metrics(&self) -> Vec<Metric> {
        self.metrics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn clear(&self) {
        self.metrics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    fn metric_count(&self) -> usize {
        self.metrics
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_collector_record_and_retrieve() {
        let collector = InMemoryCollector::new();

        let metric = Metric {
            name: "test_counter".to_string(),
            kind: MetricKind::Counter,
            value: 42.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        };

        collector.record(metric.clone());

        let all = collector.all_metrics();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "test_counter");
        assert_eq!(all[0].value, 42.0);
    }

    #[test]
    fn test_metric_count() {
        let collector = InMemoryCollector::new();

        let metric1 = Metric {
            name: "counter1".to_string(),
            kind: MetricKind::Counter,
            value: 10.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        };

        let metric2 = Metric {
            name: "gauge1".to_string(),
            kind: MetricKind::Gauge,
            value: 99.5,
            labels: vec![],
            timestamp: SystemTime::now(),
        };

        collector.record(metric1);
        collector.record(metric2);

        assert_eq!(collector.metric_count(), 2);
    }

    #[test]
    fn test_clear() {
        let collector = InMemoryCollector::new();

        let metric = Metric {
            name: "test".to_string(),
            kind: MetricKind::Counter,
            value: 1.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        };

        collector.record(metric);
        assert_eq!(collector.metric_count(), 1);

        collector.clear();
        assert_eq!(collector.metric_count(), 0);
    }

    #[test]
    fn test_metrics_by_kind() {
        let collector = InMemoryCollector::new();

        collector.record(Metric {
            name: "counter1".to_string(),
            kind: MetricKind::Counter,
            value: 10.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        collector.record(Metric {
            name: "gauge1".to_string(),
            kind: MetricKind::Gauge,
            value: 99.5,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        collector.record(Metric {
            name: "counter2".to_string(),
            kind: MetricKind::Counter,
            value: 5.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        let counters = collector.metrics_by_kind(MetricKind::Counter);
        assert_eq!(counters.len(), 2);

        let gauges = collector.metrics_by_kind(MetricKind::Gauge);
        assert_eq!(gauges.len(), 1);
    }

    #[test]
    fn test_metrics_by_name() {
        let collector = InMemoryCollector::new();

        collector.record(Metric {
            name: "spawn_count".to_string(),
            kind: MetricKind::Counter,
            value: 10.0,
            labels: vec![("zone".to_string(), "sebilis".to_string())],
            timestamp: SystemTime::now(),
        });

        collector.record(Metric {
            name: "spawn_count".to_string(),
            kind: MetricKind::Counter,
            value: 5.0,
            labels: vec![("zone".to_string(), "txevu".to_string())],
            timestamp: SystemTime::now(),
        });

        collector.record(Metric {
            name: "other_metric".to_string(),
            kind: MetricKind::Gauge,
            value: 42.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        let spawn_metrics = collector.metrics_by_name("spawn_count");
        assert_eq!(spawn_metrics.len(), 2);

        let other = collector.metrics_by_name("other_metric");
        assert_eq!(other.len(), 1);
    }

    #[test]
    fn test_sum_by_name() {
        let collector = InMemoryCollector::new();

        collector.record(Metric {
            name: "requests".to_string(),
            kind: MetricKind::Counter,
            value: 100.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        collector.record(Metric {
            name: "requests".to_string(),
            kind: MetricKind::Counter,
            value: 50.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        assert_eq!(collector.sum_by_name("requests"), 150.0);
    }

    #[test]
    fn test_average_by_name() {
        let collector = InMemoryCollector::new();

        collector.record(Metric {
            name: "latency_ms".to_string(),
            kind: MetricKind::Histogram,
            value: 10.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        collector.record(Metric {
            name: "latency_ms".to_string(),
            kind: MetricKind::Histogram,
            value: 20.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        collector.record(Metric {
            name: "latency_ms".to_string(),
            kind: MetricKind::Histogram,
            value: 30.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        assert_eq!(collector.average_by_name("latency_ms"), Some(20.0));
    }

    #[test]
    fn test_average_by_name_empty() {
        let collector = InMemoryCollector::new();
        assert_eq!(collector.average_by_name("nonexistent"), None);
    }

    #[test]
    fn tui_tick_latency_warning_requires_perf_trace_and_exceeded_budget() {
        assert!(!should_warn_tui_tick_latency(
            std::time::Duration::from_millis(51),
            false,
        ));
        assert!(!should_warn_tui_tick_latency(
            std::time::Duration::from_millis(50),
            true,
        ));
        assert!(should_warn_tui_tick_latency(
            std::time::Duration::from_millis(51),
            true,
        ));
    }

    #[test]
    fn tui_tick_latency_warning_reports_elapsed_milliseconds() {
        assert_eq!(
            tui_tick_latency_elapsed_ms(std::time::Duration::from_millis(51)),
            51
        );
    }

    #[test]
    fn test_summary() {
        let collector = InMemoryCollector::new();

        collector.record(Metric {
            name: "counter1".to_string(),
            kind: MetricKind::Counter,
            value: 1.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        collector.record(Metric {
            name: "counter1".to_string(),
            kind: MetricKind::Counter,
            value: 2.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        collector.record(Metric {
            name: "gauge1".to_string(),
            kind: MetricKind::Gauge,
            value: 99.5,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        let summary = collector.summary();

        assert_eq!(summary.len(), 2);
        assert_eq!(summary["counter1"], (MetricKind::Counter, 2));
        assert_eq!(summary["gauge1"], (MetricKind::Gauge, 1));
    }

    #[test]
    fn test_with_capacity() {
        let collector = InMemoryCollector::with_capacity(10);
        assert_eq!(collector.metric_count(), 0);

        collector.record(Metric {
            name: "test".to_string(),
            kind: MetricKind::Gauge,
            value: 1.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        assert_eq!(collector.metric_count(), 1);
    }

    #[test]
    fn test_poisoned_metrics_lock_recovers_for_collection() {
        let collector = InMemoryCollector::new();

        let poison_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut metrics = collector
                .metrics
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            metrics.push(Metric {
                name: "before_poison".to_string(),
                kind: MetricKind::Counter,
                value: 1.0,
                labels: vec![],
                timestamp: SystemTime::now(),
            });
            panic!("poison metrics lock");
        }));
        assert!(poison_result.is_err());

        collector.record(Metric {
            name: "after_poison".to_string(),
            kind: MetricKind::Counter,
            value: 2.0,
            labels: vec![],
            timestamp: SystemTime::now(),
        });

        assert_eq!(collector.metric_count(), 2);
        assert_eq!(collector.all_metrics().len(), 2);
        assert_eq!(collector.metrics_by_kind(MetricKind::Counter).len(), 2);
        assert_eq!(collector.metrics_by_name("after_poison").len(), 1);
        assert_eq!(collector.sum_by_name("after_poison"), 2.0);
        assert_eq!(collector.average_by_name("after_poison"), Some(2.0));

        let summary = collector.summary();
        assert_eq!(summary["before_poison"], (MetricKind::Counter, 1));
        assert_eq!(summary["after_poison"], (MetricKind::Counter, 1));

        collector.clear();
        assert_eq!(collector.metric_count(), 0);
    }
}
