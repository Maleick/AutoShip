//! Bayesian Online Changepoint Detection (BOCPD) for stuck-event rate spikes.
//!
//! Implements the truncated BOCPD algorithm (Adams & MacKay 2007) with a
//! Gaussian likelihood and conjugate Normal prior.  Only activates after
//! [`MIN_SESSIONS`] observations per (route_id, node_id) key.
//!
//! The BOCPD algorithm runs continuously from session 1 so that the run-length
//! distribution grows to ~30 during the baseline period.  Anomaly detection
//! only checks P(r ≤ 5) after [`MIN_SESSIONS`] observations have been seen.
//! A high short-run probability means the distribution shifted from a long run
//! to a short one, indicating a recent changepoint.

use std::collections::HashMap;

use crate::improve::{AnomalyEvent, AnomalyKind, Severity};

/// Sessions required before the detector activates.
pub const MIN_SESSIONS: usize = 30;
/// Maximum number of observations retained per key to bound memory/CPU.
const MAX_OBSERVATIONS: usize = 512;
/// Maximum run length tracked (truncation for numerical stability).
const MAX_RUN_LENGTH: usize = 120;
/// Run-length threshold for changepoint detection: P(r ≤ K) > cp_threshold.
const SHORT_RUN_K: usize = 5;

/// Per-(route_id, node_id) state.
struct KeyState {
    observations: Vec<f64>,
    /// Run-length probability distribution (normalised log-probs).
    run_probs: Vec<f64>,
    /// Sufficient statistics per run length: (count, running mean).
    run_stats: Vec<(usize, f64)>,
    /// Noise variance — set to Poisson floor at min_sessions.
    baseline_var: f64,
}

impl KeyState {
    fn new() -> Self {
        // Initial state: one run-length hypothesis at r=0 with log-prob 0 = ln(1.0).
        Self {
            observations: Vec::new(),
            run_probs: vec![0.0],
            run_stats: vec![(0, 0.0)],
            baseline_var: 1.0,
        }
    }

    fn session_count(&self) -> usize {
        self.observations.len()
    }

    fn push_observation(&mut self, value: f64) {
        self.observations.push(value);
        if self.observations.len() > MAX_OBSERVATIONS {
            self.observations.remove(0);
        }
    }

    /// Update baseline variance from observations.  Does NOT reset run state.
    fn init_baseline(&mut self) {
        let n = self.observations.len() as f64;
        let mean = self.observations.iter().sum::<f64>() / n;
        let var = self
            .observations
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>()
            / n;
        // Poisson floor: for count data, var ≥ mean.  Prevents degenerate
        // zero-variance baselines when all observations are identical.
        self.baseline_var = var.max(mean.max(1.0));
    }

    /// Advance the run-length distribution by one observation.
    /// Returns P(r ≤ SHORT_RUN_K) as the changepoint indicator.
    fn bocpd_step(&mut self, x: f64, hazard: f64) -> f64 {
        let sigma2 = self.baseline_var;
        let n = self.run_probs.len();

        // Log predictive probability for each current run length.
        let log_preds: Vec<f64> = self
            .run_stats
            .iter()
            .map(|&(r, mean_r)| {
                let pred_var = sigma2 * (1.0 + 1.0 / (r as f64 + 1.0));
                log_gaussian_pdf(x, mean_r, pred_var)
            })
            .collect();

        let mut new_probs = vec![f64::NEG_INFINITY; n + 1];
        let mut new_stats = vec![(0usize, 0.0f64); n + 1];

        // Growth: r_{t+1} = r_t + 1 (no changepoint)
        for r in 0..n {
            new_probs[r + 1] = self.run_probs[r] + log_preds[r] + (1.0 - hazard).ln();
            let (cnt, mean) = self.run_stats[r];
            let new_cnt = cnt + 1;
            new_stats[r + 1] = (new_cnt, mean + (x - mean) / new_cnt as f64);
        }

        // Changepoint: r_{t+1} = 0 (new run starting)
        new_probs[0] = log_sum_exp(
            &self
                .run_probs
                .iter()
                .zip(log_preds.iter())
                .map(|(&lp, &lpred)| lp + lpred + hazard.ln())
                .collect::<Vec<_>>(),
        );
        // New run mean is seeded with the current observation.
        new_stats[0] = (0, x);

        // Truncate to keep memory bounded.
        if new_probs.len() > MAX_RUN_LENGTH {
            new_probs.truncate(MAX_RUN_LENGTH);
            new_stats.truncate(MAX_RUN_LENGTH);
        }

        // Normalise to log-probabilities.
        let log_total = log_sum_exp(&new_probs);
        for lp in &mut new_probs {
            *lp -= log_total;
        }

        self.run_probs = new_probs;
        self.run_stats = new_stats;

        // Changepoint indicator: probability mass at short run lengths.
        // A distribution concentrated at r ≤ 5 means a changepoint occurred recently.
        self.run_probs[..self.run_probs.len().min(SHORT_RUN_K + 1)]
            .iter()
            .map(|&lp| lp.exp())
            .sum()
    }
}

/// BOCPD detector, one state per (route_id, node_id) key.
pub struct BocpdDetector {
    /// Hazard rate h = 1/mean_run_length.
    hazard: f64,
    /// Minimum sessions before anomaly detection activates (≥30 per spec).
    min_sessions: usize,
    /// Changepoint threshold: fire when P(r ≤ 5) > cp_threshold.
    cp_threshold: f64,
    states: HashMap<(u64, u64), KeyState>,
}

impl BocpdDetector {
    /// Create a new detector.
    ///
    /// - `mean_run_length`: expected sessions between changepoints (sets hazard = 1/N)
    /// - `min_sessions`: baseline requirement (30 per spec)
    pub fn new(mean_run_length: f64, min_sessions: usize) -> Self {
        Self {
            hazard: 1.0 / mean_run_length,
            min_sessions,
            cp_threshold: 0.5,
            states: HashMap::new(),
        }
    }

    /// Feed one per-session stuck-event count for a (route_id, node_id) pair.
    pub fn update(&mut self, route_id: u64, node_id: u64, stale: bool) -> Option<AnomalyEvent> {
        self.feed(route_id, node_id, 1.0, stale)
    }

    /// Feed a specific stuck count value.
    fn feed(
        &mut self,
        route_id: u64,
        node_id: u64,
        value: f64,
        stale: bool,
    ) -> Option<AnomalyEvent> {
        let key = (route_id, node_id);
        let hazard = self.hazard;
        let state = self.states.entry(key).or_insert_with(KeyState::new);

        state.push_observation(value);
        let n = state.session_count();

        // Update baseline variance once when min_sessions is reached.
        if n == self.min_sessions {
            state.init_baseline();
        }

        // Run BOCPD on every observation to build up the run-length distribution.
        let cp_prob = state.bocpd_step(value, hazard);

        // Only report anomalies after the baseline window has fully passed.
        if n <= self.min_sessions {
            return None;
        }

        if cp_prob > self.cp_threshold {
            let mean = state.observations.iter().sum::<f64>() / n as f64;
            let std = state.baseline_var.sqrt();
            Some(AnomalyEvent::new(
                0,
                AnomalyKind::StuckRateBocpd { route_id, node_id },
                "stuck_rate",
                mean,
                std,
                value,
                Severity::Average,
                stale,
            ))
        } else {
            None
        }
    }

    /// Feed a specific stuck count value (for testing).
    #[cfg(test)]
    pub fn update_with_value(
        &mut self,
        route_id: u64,
        node_id: u64,
        value: f64,
        stale: bool,
    ) -> Option<AnomalyEvent> {
        self.feed(route_id, node_id, value, stale)
    }
}

/// Gaussian log-PDF: log N(x; μ, σ²).
fn log_gaussian_pdf(x: f64, mean: f64, var: f64) -> f64 {
    if var <= 0.0 {
        return f64::NEG_INFINITY;
    }
    -0.5 * ((x - mean).powi(2) / var + var.ln() + (2.0 * std::f64::consts::PI).ln())
}

/// Log-sum-exp for numerical stability: log(Σ exp(xs)).
fn log_sum_exp(xs: &[f64]) -> f64 {
    let max = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if max.is_infinite() {
        return f64::NEG_INFINITY;
    }
    max + xs.iter().map(|&x| (x - max).exp()).sum::<f64>().ln()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_baseline(d: &mut BocpdDetector, route: u64, node: u64, value: f64, n: usize) {
        for _ in 0..n {
            d.update_with_value(route, node, value, false);
        }
    }

    #[test]
    fn no_alarm_below_min_sessions() {
        let mut d = BocpdDetector::new(250.0, MIN_SESSIONS);
        for _ in 0..MIN_SESSIONS - 1 {
            let r = d.update(1, 1, false);
            assert!(r.is_none());
        }
    }

    #[test]
    fn no_alarm_on_stable_stuck_count() {
        let mut d = BocpdDetector::new(250.0, MIN_SESSIONS);
        // Build baseline + 20 extra so run-length grows to ~50 (P(r≤5) ≈ 0)
        build_baseline(&mut d, 1, 1, 2.0, MIN_SESSIONS + 20);
        for _ in 0..10 {
            let r = d.update_with_value(1, 1, 2.0, false);
            assert!(r.is_none(), "stable value should not trigger alarm");
        }
    }

    #[test]
    fn alarm_on_stuck_rate_spike() {
        let mut d = BocpdDetector::new(250.0, MIN_SESSIONS);
        // After 30 baseline obs, run-length ≈ 30 → P(r≤5) ≈ 0.
        // Two anomalous observations (20.0) shift distribution to r≤2 → fires.
        build_baseline(&mut d, 2, 3, 2.0, MIN_SESSIONS);
        let mut fired = false;
        for _ in 0..5 {
            if d.update_with_value(2, 3, 20.0, false).is_some() {
                fired = true;
                break;
            }
        }
        assert!(fired, "BOCPD should detect stuck-rate spike");
    }

    #[test]
    fn stale_baseline_yields_minor() {
        let mut d = BocpdDetector::new(250.0, MIN_SESSIONS);
        build_baseline(&mut d, 4, 5, 2.0, MIN_SESSIONS);
        let mut ev = None;
        for _ in 0..5 {
            ev = d.update_with_value(4, 5, 20.0, true);
            if ev.is_some() {
                break;
            }
        }
        if let Some(ev) = ev {
            assert_eq!(ev.severity, Severity::Minor);
        }
    }

    #[test]
    fn independent_state_per_key() {
        let mut d = BocpdDetector::new(250.0, MIN_SESSIONS);
        build_baseline(&mut d, 1, 1, 2.0, MIN_SESSIONS);
        // (1, 2) is still in warmup — should not fire
        let r = d.update_with_value(1, 2, 20.0, false);
        assert!(r.is_none());
    }

    #[test]
    fn log_sum_exp_known_values() {
        // log(0.5) + log(0.5) → log_sum_exp → log(1.0)
        let vals = vec![0.5_f64.ln(), 0.5_f64.ln()];
        let result = log_sum_exp(&vals);
        assert!((result.exp() - 1.0).abs() < 1e-6, "got {}", result.exp());
    }

    #[test]
    fn observations_are_bounded_per_key() {
        let mut d = BocpdDetector::new(250.0, MIN_SESSIONS);
        for _ in 0..(MAX_OBSERVATIONS + 25) {
            d.update_with_value(7, 11, 2.0, false);
        }

        let key = (7, 11);
        let state = d.states.get(&key).expect("state should exist");
        assert_eq!(state.observations.len(), MAX_OBSERVATIONS);
    }
}
