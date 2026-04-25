//! EWMA (Exponentially Weighted Moving Average) control chart detector.
//!
//! Detects sudden DPS drops for a given (class, zone) pair.
//! Fires when the EWMA statistic crosses the lower control limit (mean - L·σ_z).
//! Requires [`WARMUP_SAMPLES`] observations before activating.

use std::collections::HashMap;

use crate::improve::{AnomalyEvent, AnomalyKind, Severity};

/// Number of observations required to establish the baseline.
const WARMUP_SAMPLES: usize = 30;

struct EwmaState {
    warmup: Vec<f64>,
    baseline_mean: f64,
    baseline_std: f64,
    /// Current EWMA statistic z_t = λ·x_t + (1−λ)·z_{t−1}.
    ewma: f64,
    /// Lower control limit: mean − L·std·√(λ/(2−λ)).
    lcl: f64,
}

/// EWMA control chart, one state per (class, zone) key.
pub struct EwmaDetector {
    lambda: f64,
    threshold_sigma: f64,
    states: HashMap<(String, String), EwmaState>,
}

impl EwmaDetector {
    pub fn new(lambda: f64, threshold_sigma: f64) -> Self {
        Self {
            lambda,
            threshold_sigma,
            states: HashMap::new(),
        }
    }

    /// Feed one DPS observation.  Returns an anomaly event if the EWMA drops
    /// below the lower control limit.
    pub fn update(
        &mut self,
        class: &str,
        zone: &str,
        value: f64,
        stale: bool,
    ) -> Option<AnomalyEvent> {
        let key = (class.to_owned(), zone.to_owned());
        let lambda = self.lambda;
        let threshold_sigma = self.threshold_sigma;

        let state = self.states.entry(key).or_insert_with(|| EwmaState {
            warmup: Vec::new(),
            baseline_mean: 0.0,
            baseline_std: 0.0,
            ewma: value,
            lcl: f64::NEG_INFINITY,
        });

        if state.warmup.len() < WARMUP_SAMPLES {
            state.warmup.push(value);
            if state.warmup.len() == WARMUP_SAMPLES {
                let n = WARMUP_SAMPLES as f64;
                let mean = state.warmup.iter().sum::<f64>() / n;
                let variance =
                    state.warmup.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
                let std = variance.sqrt();
                state.baseline_mean = mean;
                state.baseline_std = std;
                state.ewma = mean;
                // LCL = mean − L · std · √(λ/(2−λ))
                state.lcl =
                    mean - threshold_sigma * std * (lambda / (2.0 - lambda)).sqrt();
            }
            return None;
        }

        state.ewma = lambda * value + (1.0 - lambda) * state.ewma;

        if state.ewma < state.lcl {
            Some(AnomalyEvent::new(
                0,
                AnomalyKind::DpsDropEwma {
                    class: class.to_owned(),
                    zone: zone.to_owned(),
                },
                "dps",
                state.baseline_mean,
                state.baseline_std,
                value,
                Severity::Major,
                stale,
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_baseline(det: &mut EwmaDetector, class: &str, zone: &str, mean: f64) {
        for _ in 0..WARMUP_SAMPLES {
            det.update(class, zone, mean, false);
        }
    }

    #[test]
    fn no_alarm_during_warmup() {
        let mut d = EwmaDetector::new(0.2, 3.0);
        for i in 0..WARMUP_SAMPLES - 1 {
            let r = d.update("warrior", "guk", 100.0 + i as f64, false);
            assert!(r.is_none(), "should not fire during warmup");
        }
    }

    #[test]
    fn no_alarm_on_stable_signal() {
        let mut d = EwmaDetector::new(0.2, 3.0);
        // Varied baseline to get non-zero std (≈2.5); LCL ≈ mean − 2.5 ≈ 96.9
        let values = [95.0_f64, 98.0, 102.0, 100.0, 99.0, 101.0, 97.0, 103.0, 100.0, 99.0]
            .iter()
            .cycle()
            .take(WARMUP_SAMPLES)
            .cloned()
            .collect::<Vec<_>>();
        for v in &values {
            d.update("warrior", "guk", *v, false);
        }
        // 98.0 is above LCL (~96.9) — should not alarm
        for _ in 0..20 {
            let r = d.update("warrior", "guk", 98.0, false);
            assert!(r.is_none(), "98.0 within normal range should not alarm");
        }
    }

    #[test]
    fn alarm_on_sudden_dps_drop() {
        let mut d = EwmaDetector::new(0.2, 3.0);
        // Baseline: mean=100, std≈0 (all identical → std=0, but LCL would be mean)
        // Use varied baseline to get non-zero std
        let values = [95.0, 98.0, 102.0, 100.0, 99.0, 101.0]
            .iter()
            .cycle()
            .take(WARMUP_SAMPLES)
            .cloned()
            .collect::<Vec<_>>();
        for v in &values {
            d.update("mage", "solb", *v, false);
        }
        // Feed severely depressed DPS — EWMA will eventually cross LCL
        let mut fired = false;
        for _ in 0..20 {
            if d.update("mage", "solb", 50.0, false).is_some() {
                fired = true;
                break;
            }
        }
        assert!(fired, "EWMA should detect severe DPS drop");
    }

    #[test]
    fn alarm_is_major_severity() {
        let mut d = EwmaDetector::new(0.2, 3.0);
        let values = [95.0, 98.0, 102.0, 100.0, 99.0, 101.0]
            .iter()
            .cycle()
            .take(WARMUP_SAMPLES)
            .cloned()
            .collect::<Vec<_>>();
        for v in &values {
            d.update("rogue", "kael", *v, false);
        }
        let mut ev = None;
        for _ in 0..30 {
            ev = d.update("rogue", "kael", 40.0, false);
            if ev.is_some() {
                break;
            }
        }
        let ev = ev.expect("should fire");
        assert_eq!(ev.severity, Severity::Major);
        assert!(!ev.stale_baseline);
    }

    #[test]
    fn stale_baseline_demotes_to_minor() {
        let mut d = EwmaDetector::new(0.2, 3.0);
        let values = [95.0, 98.0, 102.0, 100.0, 99.0, 101.0]
            .iter()
            .cycle()
            .take(WARMUP_SAMPLES)
            .cloned()
            .collect::<Vec<_>>();
        for v in &values {
            d.update("enc", "ntov", *v, false);
        }
        let mut ev = None;
        for _ in 0..30 {
            ev = d.update("enc", "ntov", 40.0, true);
            if ev.is_some() {
                break;
            }
        }
        let ev = ev.expect("should fire even with stale baseline");
        assert_eq!(ev.severity, Severity::Minor);
        assert!(ev.stale_baseline);
    }

    #[test]
    fn independent_state_per_class_zone() {
        let mut d = EwmaDetector::new(0.2, 3.0);
        build_baseline(&mut d, "warrior", "guk", 100.0);
        // "mage/solb" is still in warmup — should not fire
        let r = d.update("mage", "solb", 10.0, false);
        assert!(r.is_none());
    }
}
