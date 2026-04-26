//! Robust z-score (MAD) detector for mana P10 collapse.
//!
//! Maintains a rolling window of mana-P10 observations and computes the
//! robust z-score using the Median Absolute Deviation (MAD):
//!
//!   z = |x − median| / (1.4826 · MAD)
//!
//! The 1.4826 constant makes MAD a consistent estimator for the standard
//! deviation of a normal distribution.  Fires when z > threshold (default 3.5).

use std::collections::VecDeque;

use crate::improve::{AnomalyEvent, AnomalyKind, Severity};

/// Minimum window size before the detector activates.
const MIN_WINDOW: usize = 10;

/// MAD robust z-score detector.
pub struct MadDetector {
    window: VecDeque<f64>,
    window_size: usize,
    /// MAD z-score threshold (default 3.5).
    threshold: f64,
}

impl MadDetector {
    pub fn new(window_size: usize, threshold: f64) -> Self {
        Self {
            window: VecDeque::with_capacity(window_size),
            window_size,
            threshold,
        }
    }

    /// Feed one mana P10 observation.
    pub fn update(&mut self, value: f64, stale: bool) -> Option<AnomalyEvent> {
        // Maintain sliding window
        if self.window.len() >= self.window_size {
            self.window.pop_front();
        }
        self.window.push_back(value);

        if self.window.len() < MIN_WINDOW {
            return None;
        }

        let values: Vec<f64> = self.window.iter().cloned().collect();
        let (median, baseline_std) = median_and_mad(&values);
        let z = mad_zscore(value, median, baseline_std);

        if z > self.threshold {
            Some(AnomalyEvent::new(
                0,
                AnomalyKind::ManaCollapseMad,
                "mana_p10",
                median,
                baseline_std,
                value,
                Severity::Major,
                stale,
            ))
        } else {
            None
        }
    }
}

/// Compute the median and scaled MAD (1.4826 · MAD) of a slice.
fn median_and_mad(values: &[f64]) -> (f64, f64) {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = percentile_sorted(&sorted, 0.5);

    let mut devs: Vec<f64> = sorted.iter().map(|&v| (v - median).abs()).collect();
    devs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mad = percentile_sorted(&devs, 0.5);

    // 1.4826 makes MAD a consistent estimator of σ for Gaussian data
    (median, 1.4826 * mad)
}

/// Median of a pre-sorted slice.
fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let n = sorted.len();
    let idx = (p * (n - 1) as f64) as usize;
    if n.is_multiple_of(2) && p == 0.5 {
        (sorted[idx] + sorted[(idx + 1).min(n - 1)]) / 2.0
    } else {
        sorted[idx]
    }
}

/// Compute robust z-score for a single value against the baseline.
fn mad_zscore(x: f64, median: f64, scaled_mad: f64) -> f64 {
    if scaled_mad < f64::EPSILON {
        return 0.0;
    }
    (x - median).abs() / scaled_mad
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_window(d: &mut MadDetector, value: f64, n: usize) {
        for _ in 0..n {
            d.update(value, false);
        }
    }

    #[test]
    fn no_alarm_below_min_window() {
        let mut d = MadDetector::new(200, 3.5);
        for i in 0..MIN_WINDOW - 1 {
            let r = d.update(80.0 + i as f64, false);
            assert!(r.is_none());
        }
    }

    #[test]
    fn no_alarm_on_stable_mana() {
        let mut d = MadDetector::new(200, 3.5);
        fill_window(&mut d, 80.0, 200);
        // Values near the median should not fire
        let r = d.update(79.0, false);
        assert!(r.is_none());
    }

    #[test]
    fn alarm_on_mana_collapse() {
        let mut d = MadDetector::new(200, 3.5);
        // Baseline: mana P10 around 70–80
        for i in 0..200 {
            d.update(70.0 + (i % 10) as f64, false);
        }
        // Sudden collapse to 5%
        let ev = d.update(2.0, false);
        assert!(ev.is_some(), "severe mana collapse should fire");
        let ev = ev.unwrap();
        assert_eq!(ev.kind, AnomalyKind::ManaCollapseMad);
        assert_eq!(ev.severity, Severity::Major);
    }

    #[test]
    fn stale_baseline_demotes_to_minor() {
        let mut d = MadDetector::new(200, 3.5);
        for i in 0..200 {
            d.update(70.0 + (i % 10) as f64, false);
        }
        let ev = d.update(2.0, true);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().severity, Severity::Minor);
    }

    #[test]
    fn median_and_mad_known_values() {
        // {1, 1, 2, 2, 4, 6, 9} → median=2, MAD=1, scaled=1.4826
        let vals = vec![1.0, 1.0, 2.0, 2.0, 4.0, 6.0, 9.0];
        let (median, scaled_mad) = median_and_mad(&vals);
        assert!((median - 2.0).abs() < 1e-6, "median");
        // MAD of abs deviations from 2: {1,1,0,0,2,4,7} → sorted {0,0,1,1,2,4,7} → median=1
        assert!((scaled_mad - 1.4826).abs() < 1e-3, "scaled MAD");
    }

    #[test]
    fn zero_mad_no_panic() {
        // Constant signal → MAD = 0 → z = 0 → no alarm
        let mut d = MadDetector::new(50, 3.5);
        for _ in 0..50 {
            d.update(50.0, false);
        }
        let r = d.update(50.0, false);
        assert!(r.is_none());
    }
}
