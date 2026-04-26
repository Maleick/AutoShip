//! STL residual + MAD detector for loot-rate drops on farmed camps.
//!
//! Implements a simplified STL (Seasonal-Trend decomposition using Loess)
//! by estimating the trend via a centered moving average and the seasonal
//! component as the average de-trended value at each position in the period.
//!
//! Only activates after [`MIN_SESSIONS`] observations per camp (≥30 per spec).
//!
//! Steps:
//! 1. Trend: centered moving average of window `2*period + 1`.
//! 2. De-trend: raw − trend.
//! 3. Seasonal: mean de-trended value at each of the `period` positions.
//! 4. Residual: raw − trend − seasonal.
//! 5. Fire when MAD robust z-score of the latest residual exceeds `mad_threshold`.

use std::collections::HashMap;

use crate::improve::{AnomalyEvent, AnomalyKind, Severity};

/// Sessions required before the detector activates.
pub const MIN_SESSIONS: usize = 30;
/// Maximum number of observations retained per camp to bound memory/CPU.
const MAX_OBSERVATIONS: usize = 512;

struct CampState {
    observations: Vec<f64>,
    /// Seasonal period (e.g. 7 for weekly camp patterns).
    period: usize,
    mad_threshold: f64,
}

impl CampState {
    fn new(period: usize, mad_threshold: f64) -> Self {
        Self {
            observations: Vec::new(),
            period,
            mad_threshold,
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

    /// Compute trend via centered moving average.
    fn trend(&self) -> Vec<f64> {
        let n = self.observations.len();
        let half = self.period;
        let window = 2 * half + 1;
        let mut trend = vec![f64::NAN; n];
        for i in half..n.saturating_sub(half) {
            let sum: f64 = self.observations[i.saturating_sub(half)..=(i + half)]
                .iter()
                .sum();
            trend[i] = sum / window as f64;
        }
        // Extrapolate edges with nearest valid value
        if let Some(&first_valid) = trend.iter().find(|v| !v.is_nan()) {
            for v in trend.iter_mut() {
                if v.is_nan() {
                    *v = first_valid;
                    break;
                }
            }
        }
        // Fill forward then backward for remaining NaNs
        let mut last = trend.iter().find(|v| !v.is_nan()).cloned().unwrap_or(0.0);
        for v in trend.iter_mut() {
            if !v.is_nan() {
                last = *v;
            } else {
                *v = last;
            }
        }
        trend
    }

    /// Estimate seasonal component from de-trended observations.
    fn seasonal(&self, trend: &[f64]) -> Vec<f64> {
        let n = self.observations.len();
        let p = self.period;
        let mut bucket_sums = vec![0.0; p];
        let mut bucket_counts = vec![0usize; p];

        for (i, (&obs, &tr)) in self.observations.iter().zip(trend.iter()).enumerate() {
            let pos = i % p;
            bucket_sums[pos] += obs - tr;
            bucket_counts[pos] += 1;
        }

        let bucket_means: Vec<f64> = bucket_sums
            .iter()
            .zip(bucket_counts.iter())
            .map(|(&s, &c)| if c > 0 { s / c as f64 } else { 0.0 })
            .collect();

        (0..n).map(|i| bucket_means[i % p]).collect()
    }

    /// Compute STL residuals for all observations.
    fn residuals(&self) -> Vec<f64> {
        let trend = self.trend();
        let seasonal = self.seasonal(&trend);
        self.observations
            .iter()
            .zip(trend.iter())
            .zip(seasonal.iter())
            .map(|((&obs, &tr), &se)| obs - tr - se)
            .collect()
    }

    /// Check whether the latest observation is anomalous.
    /// Returns (baseline_mean, baseline_std, residual) or None.
    fn check_latest(&self) -> Option<(f64, f64, f64)> {
        let residuals = self.residuals();
        if residuals.is_empty() {
            return None;
        }
        let latest = *residuals.last()?;
        let baseline: Vec<f64> = residuals[..residuals.len().saturating_sub(1)].to_vec();
        if baseline.is_empty() {
            return None;
        }
        let (median, scaled_mad) = median_and_mad(&baseline);
        if scaled_mad < f64::EPSILON {
            return None;
        }
        let z = (latest - median).abs() / scaled_mad;
        if z > self.mad_threshold && latest < median {
            // Only fire on drops (latest < median of residuals)
            Some((median, scaled_mad, latest))
        } else {
            None
        }
    }
}

/// STL + MAD detector, one state per camp name.
pub struct StlMadDetector {
    period: usize,
    min_sessions: usize,
    mad_threshold: f64,
    states: HashMap<String, CampState>,
}

impl StlMadDetector {
    pub fn new(period: usize, min_sessions: usize, mad_threshold: f64) -> Self {
        Self {
            period,
            min_sessions,
            mad_threshold,
            states: HashMap::new(),
        }
    }

    /// Feed one per-session loot-rate observation for a named camp.
    pub fn update(&mut self, camp: &str, value: f64, stale: bool) -> Option<AnomalyEvent> {
        let period = self.period;
        let mad_threshold = self.mad_threshold;
        let min_sessions = self.min_sessions;

        let state = self
            .states
            .entry(camp.to_owned())
            .or_insert_with(|| CampState::new(period, mad_threshold));

        state.push_observation(value);

        if state.session_count() < min_sessions {
            return None;
        }

        if let Some((baseline_mean, baseline_std, residual)) = state.check_latest() {
            Some(AnomalyEvent::new(
                0,
                AnomalyKind::LootRateStlMad {
                    camp: camp.to_owned(),
                },
                "loot_rate",
                baseline_mean,
                baseline_std,
                residual,
                Severity::Average,
                stale,
            ))
        } else {
            None
        }
    }
}

fn median_and_mad(values: &[f64]) -> (f64, f64) {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if sorted.len() % 2 == 0 {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };
    let mut devs: Vec<f64> = sorted.iter().map(|&v| (v - median).abs()).collect();
    devs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mad = if devs.len() % 2 == 0 {
        (devs[devs.len() / 2 - 1] + devs[devs.len() / 2]) / 2.0
    } else {
        devs[devs.len() / 2]
    };
    (median, 1.4826 * mad)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_camp(d: &mut StlMadDetector, camp: &str, base: f64, period: usize, n: usize) {
        for i in 0..n {
            let seasonal = (i % period) as f64 * 2.0;
            d.update(camp, base + seasonal, false);
        }
    }

    #[test]
    fn no_alarm_below_min_sessions() {
        let mut d = StlMadDetector::new(7, MIN_SESSIONS, 3.5);
        for _ in 0..MIN_SESSIONS - 1 {
            let r = d.update("camp_guk", 100.0, false);
            assert!(r.is_none());
        }
    }

    #[test]
    fn no_alarm_on_normal_loot_rate() {
        let mut d = StlMadDetector::new(7, MIN_SESSIONS, 3.5);
        build_camp(&mut d, "camp_sol", 100.0, 7, MIN_SESSIONS + 14);
        // Values following the same seasonal pattern — residuals ≈ 0
        let r = d.update("camp_sol", 104.0, false); // within normal range
        assert!(r.is_none());
    }

    #[test]
    fn alarm_on_loot_rate_crash() {
        let mut d = StlMadDetector::new(7, MIN_SESSIONS, 3.5);
        // Stable loot rate with small seasonal variation
        for i in 0..MIN_SESSIONS + 14 {
            let v = 100.0 + (i % 7) as f64;
            d.update("ntov_camp", v, false);
        }
        // Sudden loot-rate crash (camp dead / rare spawn)
        let r = d.update("ntov_camp", 5.0, false);
        assert!(r.is_some(), "should detect loot-rate crash");
        let ev = r.unwrap();
        assert_eq!(
            ev.kind,
            AnomalyKind::LootRateStlMad {
                camp: "ntov_camp".into()
            }
        );
    }

    #[test]
    fn stale_baseline_yields_minor() {
        let mut d = StlMadDetector::new(7, MIN_SESSIONS, 3.5);
        for i in 0..MIN_SESSIONS + 14 {
            let v = 100.0 + (i % 7) as f64;
            d.update("velious_camp", v, false);
        }
        let r = d.update("velious_camp", 5.0, true);
        assert!(r.is_some());
        assert_eq!(r.unwrap().severity, Severity::Minor);
    }

    #[test]
    fn independent_state_per_camp() {
        let mut d = StlMadDetector::new(7, MIN_SESSIONS, 3.5);
        build_camp(&mut d, "camp_a", 100.0, 7, MIN_SESSIONS);
        // camp_b is in warmup
        let r = d.update("camp_b", 5.0, false);
        assert!(r.is_none());
    }

    #[test]
    fn observations_are_bounded_per_camp() {
        let mut d = StlMadDetector::new(7, MIN_SESSIONS, 3.5);
        for i in 0..(MAX_OBSERVATIONS + 25) {
            d.update("bounded_camp", 100.0 + (i % 7) as f64, false);
        }

        let state = d.states.get("bounded_camp").expect("state should exist");
        assert_eq!(state.observations.len(), MAX_OBSERVATIONS);
    }
}
