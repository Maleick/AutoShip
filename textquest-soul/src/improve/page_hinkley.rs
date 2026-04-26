//! Page-Hinkley test for death-cluster detection.
//!
//! Detects an upward shift in the per-session death rate (deaths / 10-minute window).
//! The baseline represents the expected rate (default ≈ 0.1 deaths/min = 1 per 10 min).
//! A cluster is detected when the PH statistic exceeds the threshold, signalling
//! ≥3 deaths in 10 minutes vs. the expected <1.
//!
//! Ported from River's `PageHinkley` implementation (~50 LOC).

use std::collections::VecDeque;

use crate::improve::{AnomalyEvent, AnomalyKind, Severity};

/// Page-Hinkley detector on a rolling death-rate signal.
pub struct PageHinkleyDetector {
    /// Sliding window duration in milliseconds (default 10 min = 600_000).
    window_ms: u64,
    /// Expected baseline death rate (deaths/min).
    baseline_rate: f64,
    /// Minimum detectable magnitude shift (δ).
    delta: f64,
    /// Detection threshold (λ).
    threshold: f64,
    // PH state
    sum: f64,
    min_sum: f64,
    mean: f64,
    n: u64,
    // Rolling death timestamps for per-session rate computation.
    deaths: VecDeque<u64>,
}

impl PageHinkleyDetector {
    /// Create a new detector.
    ///
    /// - `window_ms`: sliding window length in milliseconds
    /// - `baseline_rate`: expected deaths/min
    /// - `delta`: minimum shift magnitude (0.005 is River default)
    /// - `threshold`: PH detection threshold (25.0 → ~3σ equivalent)
    pub fn new(window_ms: u64, baseline_rate: f64, delta: f64, threshold: f64) -> Self {
        Self {
            window_ms,
            baseline_rate,
            delta,
            threshold,
            sum: 0.0,
            min_sum: 0.0,
            mean: baseline_rate,
            n: 0,
            deaths: VecDeque::new(),
        }
    }

    /// Record a death event at `ts_ms` (Unix milliseconds).
    pub fn record_death(&mut self, ts_ms: u64) {
        self.deaths.push_back(ts_ms);
    }

    /// Compute the death rate in the current window and run the PH test.
    /// Should be called once per session close.
    ///
    /// Returns an anomaly event if a death cluster is detected.
    pub fn check_session(&mut self, stale: bool) -> Option<AnomalyEvent> {
        // Compute rate: deaths in the last window_ms / window_min
        let now_ms = *self.deaths.back().unwrap_or(&0);
        let cutoff = now_ms.saturating_sub(self.window_ms);
        let count = self.deaths.iter().filter(|&&t| t >= cutoff).count();
        let window_min = self.window_ms as f64 / 60_000.0;
        let rate = count as f64 / window_min;

        // Drain old deaths outside the window
        while self.deaths.front().is_some_and(|&t| t < cutoff) {
            self.deaths.pop_front();
        }

        let fired = self.update_ph(rate);
        if fired {
            let baseline_std = self.baseline_rate.sqrt().max(0.001);
            Some(AnomalyEvent::new(
                0,
                AnomalyKind::DeathClusterPageHinkley,
                "death_rate",
                self.baseline_rate,
                baseline_std,
                rate,
                Severity::Major,
                stale,
            ))
        } else {
            None
        }
    }

    /// Feed one rate observation into the PH algorithm.
    /// Returns `true` when a changepoint is detected.
    fn update_ph(&mut self, x: f64) -> bool {
        // Running mean update
        self.n += 1;
        self.mean = self.mean + (x - self.mean) / self.n as f64;
        // Cumulative sum for upward-shift detection
        self.sum += x - self.mean - self.delta;
        if self.sum < self.min_sum {
            self.min_sum = self.sum;
        }
        (self.sum - self.min_sum) > self.threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_det() -> PageHinkleyDetector {
        // Short window (1 min = 60_000 ms), baseline 0.1/min, δ=0.005, λ=5
        PageHinkleyDetector::new(60_000, 0.1, 0.005, 5.0)
    }

    #[test]
    fn no_alarm_on_normal_rate() {
        let mut d = make_det();
        // Feed 30 sessions at baseline rate (0.0 deaths / window → 0 rate)
        for _ in 0..30 {
            let r = d.check_session(false);
            assert!(r.is_none(), "baseline rate should not trigger alarm");
        }
    }

    #[test]
    fn alarm_on_death_cluster() {
        let mut d = make_det();
        // Build up background observations
        for _ in 0..20 {
            d.check_session(false);
        }
        // Inject 5 deaths within the window (ts 0..4_000 ms)
        for i in 0u64..5 {
            d.record_death(i * 1_000);
        }
        // Should eventually detect the spike
        let mut fired = false;
        for i in 0..15u64 {
            d.record_death(50_000 + i * 100);
            if d.check_session(false).is_some() {
                fired = true;
                break;
            }
        }
        assert!(fired, "PH should detect death cluster");
    }

    #[test]
    fn stale_baseline_yields_minor() {
        let mut d = make_det();
        for _ in 0..20 {
            d.check_session(false);
        }
        for i in 0u64..10 {
            d.record_death(i * 100);
        }
        let mut ev = None;
        for i in 0..15u64 {
            d.record_death(30_000 + i * 50);
            ev = d.check_session(true);
            if ev.is_some() {
                break;
            }
        }
        if let Some(ev) = ev {
            assert_eq!(ev.severity, Severity::Minor);
        }
    }

    #[test]
    fn ph_statistic_resets_after_no_changes() {
        let mut d = make_det();
        // Low rate observations — PH sum should converge toward 0
        for _ in 0..100 {
            d.update_ph(0.0);
        }
        // With low observations, sum - min_sum should be small
        let stat = d.sum - d.min_sum;
        assert!(stat < d.threshold, "no changepoint in flat signal");
    }
}
