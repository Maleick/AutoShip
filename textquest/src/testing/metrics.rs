//! Metrics collection framework for TextQuest testing and observability.

use std::collections::HashMap;

/// A single metric value — can be a counter, gauge, or histogram.
#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
}

/// Aggregates multiple metric samples keyed by name.
///
/// Each key stores a `Vec<MetricValue>`. Counter and Gauge values are stored
/// individually; Histogram entries are flattened into the series.
#[derive(Debug, Default)]
pub struct MetricsAggregator {
    data: HashMap<String, Vec<MetricValue>>,
}

impl MetricsAggregator {
    /// Create a new, empty aggregator.
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Add a metric value under `key`.
    pub fn add(&mut self, key: impl Into<String>, value: MetricValue) {
        self.data.entry(key.into()).or_default().push(value);
    }

    /// Return the sum of all numeric samples for `key`.
    ///
    /// - `Counter(n)` contributes `n as f64`.
    /// - `Gauge(v)` contributes `v`.
    /// - `Histogram(vs)` contributes the sum of all values.
    ///
    /// Returns `None` if the key is unknown or has no samples.
    pub fn sum(&self, key: &str) -> Option<f64> {
        let values = self.data.get(key)?;
        if values.is_empty() {
            return None;
        }
        let total = values.iter().map(Self::scalar_sum).sum::<f64>();
        Some(total)
    }

    /// Return the arithmetic mean of all numeric samples for `key`.
    ///
    /// Returns `None` if the key is unknown or has no samples.
    pub fn average(&self, key: &str) -> Option<f64> {
        let values = self.data.get(key)?;
        if values.is_empty() {
            return None;
        }
        let all = Self::flatten(values);
        if all.is_empty() {
            return None;
        }
        Some(all.iter().copied().sum::<f64>() / all.len() as f64)
    }

    /// Return the `p`-th percentile (0.0–100.0) of numeric samples for `key`.
    ///
    /// Uses the nearest-rank method. Returns `None` if the key is unknown,
    /// has no samples, or `p` is out of range.
    pub fn percentile(&self, key: &str, p: f64) -> Option<f64> {
        if !(0.0..=100.0).contains(&p) {
            return None;
        }
        let values = self.data.get(key)?;
        let mut all = Self::flatten(values);
        if all.is_empty() {
            return None;
        }
        all.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((p / 100.0) * (all.len() as f64 - 1.0)).round() as usize;
        Some(all[idx.min(all.len() - 1)])
    }

    /// Serialize the aggregator state to a JSON string.
    ///
    /// Each key maps to an object with `sum`, `count`, and `average` fields.
    /// Returns a JSON object string.
    pub fn to_json(&self) -> String {
        let mut obj = serde_json::Map::new();
        for (key, values) in &self.data {
            let all = Self::flatten(values);
            let count = all.len();
            let sum: f64 = all.iter().copied().sum();
            let avg = if count > 0 { sum / count as f64 } else { 0.0 };
            let entry = serde_json::json!({
                "count": count,
                "sum": sum,
                "average": avg,
            });
            obj.insert(key.clone(), entry);
        }
        serde_json::Value::Object(obj).to_string()
    }

    // --- private helpers ---

    /// Sum all numeric values within a single MetricValue entry.
    fn scalar_sum(v: &MetricValue) -> f64 {
        match v {
            MetricValue::Counter(n) => *n as f64,
            MetricValue::Gauge(f) => *f,
            MetricValue::Histogram(vs) => vs.iter().copied().sum(),
        }
    }

    /// Flatten a slice of MetricValues into a single Vec<f64>.
    fn flatten(values: &[MetricValue]) -> Vec<f64> {
        let mut out = Vec::new();
        for v in values {
            match v {
                MetricValue::Counter(n) => out.push(*n as f64),
                MetricValue::Gauge(f) => out.push(*f),
                MetricValue::Histogram(vs) => out.extend_from_slice(vs),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_sum_and_average() {
        let mut agg = MetricsAggregator::new();
        agg.add("hits", MetricValue::Counter(10));
        agg.add("hits", MetricValue::Counter(20));
        agg.add("hits", MetricValue::Counter(30));
        assert_eq!(agg.sum("hits"), Some(60.0));
        assert_eq!(agg.average("hits"), Some(20.0));
    }

    #[test]
    fn test_gauge_sum_and_average() {
        let mut agg = MetricsAggregator::new();
        agg.add("cpu", MetricValue::Gauge(0.5));
        agg.add("cpu", MetricValue::Gauge(1.5));
        assert_eq!(agg.sum("cpu"), Some(2.0));
        assert_eq!(agg.average("cpu"), Some(1.0));
    }

    #[test]
    fn test_histogram_percentile() {
        let mut agg = MetricsAggregator::new();
        // Add a histogram with values 1..=9
        agg.add(
            "latency",
            MetricValue::Histogram(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]),
        );
        // p50 of [1..9] sorted is index 4 → 5.0
        let p50 = agg.percentile("latency", 50.0).unwrap();
        assert_eq!(p50, 5.0);
        // p100 is the max → 9.0
        let p100 = agg.percentile("latency", 100.0).unwrap();
        assert_eq!(p100, 9.0);
        // p0 is the min → 1.0
        let p0 = agg.percentile("latency", 0.0).unwrap();
        assert_eq!(p0, 1.0);
    }

    #[test]
    fn test_unknown_key_returns_none() {
        let agg = MetricsAggregator::new();
        assert!(agg.sum("missing").is_none());
        assert!(agg.average("missing").is_none());
        assert!(agg.percentile("missing", 50.0).is_none());
    }

    #[test]
    fn test_to_json_structure() {
        let mut agg = MetricsAggregator::new();
        agg.add("reqs", MetricValue::Counter(100));
        agg.add("reqs", MetricValue::Counter(200));
        let json = agg.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let reqs = &parsed["reqs"];
        assert_eq!(reqs["count"], 2);
        assert_eq!(reqs["sum"], 300.0);
        assert_eq!(reqs["average"], 150.0);
    }

    #[test]
    fn test_mixed_metric_types_sum() {
        let mut agg = MetricsAggregator::new();
        agg.add("mix", MetricValue::Counter(5));
        agg.add("mix", MetricValue::Gauge(2.5));
        agg.add("mix", MetricValue::Histogram(vec![1.0, 1.5]));
        // 5 + 2.5 + 1.0 + 1.5 = 10.0
        assert_eq!(agg.sum("mix"), Some(10.0));
    }

    #[test]
    fn test_percentile_out_of_range_returns_none() {
        let mut agg = MetricsAggregator::new();
        agg.add("x", MetricValue::Gauge(1.0));
        assert!(agg.percentile("x", -1.0).is_none());
        assert!(agg.percentile("x", 101.0).is_none());
    }
}
