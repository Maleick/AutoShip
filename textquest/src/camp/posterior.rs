//! Camp posterior fusion — Normal-Normal updater for camp × party telemetry.

use std::{collections::HashMap, fmt};

use serde::{Deserialize, Serialize};

/// Stable identifier for a camp.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CampId(String);

impl CampId {
    /// Create a new camp identifier.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the raw camp identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CampId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for CampId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for CampId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Stable signature for the current party composition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PartySignature(String);

impl PartySignature {
    /// Create a new party signature.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the raw party signature string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for PartySignature {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for PartySignature {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for PartySignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Axis key for one posterior series.
///
/// The newtype keeps the model extensible: the updater can support fixed axes
/// like `xp_per_hour` and `deaths_per_session` while also accepting future
/// telemetry names without a schema change.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Axis(String);

impl Axis {
    /// Construct a custom axis label.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// `xp_per_hour`
    #[must_use]
    pub fn xp_per_hour() -> Self {
        Self::new("xp_per_hour")
    }

    /// `aa_xp_per_hour`
    #[must_use]
    pub fn aa_xp_per_hour() -> Self {
        Self::new("aa_xp_per_hour")
    }

    /// `platinum_per_hour`
    #[must_use]
    pub fn platinum_per_hour() -> Self {
        Self::new("platinum_per_hour")
    }

    /// `deaths_per_session`
    #[must_use]
    pub fn deaths_per_session() -> Self {
        Self::new("deaths_per_session")
    }

    /// Borrow the raw axis label.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Axis {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Axis {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for Axis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Posterior summary for one camp/party/axis row.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Distribution {
    /// Posterior mean.
    pub mean: f64,
    /// Posterior standard deviation.
    pub std_dev: f64,
}

impl Distribution {
    /// Create a new normal distribution summary.
    #[must_use]
    pub fn new(mean: f64, std_dev: f64) -> Self {
        Self {
            mean,
            std_dev: std_dev.abs(),
        }
    }

    /// Posterior variance.
    #[must_use]
    pub fn variance(self) -> f64 {
        self.std_dev * self.std_dev
    }
}

impl Default for Distribution {
    fn default() -> Self {
        Self::new(0.0, 1.0)
    }
}

/// Configurable prior set for the camp posterior updater.
#[derive(Debug, Clone)]
pub struct CampPosteriorConfig {
    /// Prior strength in pseudo-sessions.
    pub prior_strength: f64,
    priors: HashMap<Axis, Distribution>,
    fallback_prior: Distribution,
}

impl Default for CampPosteriorConfig {
    fn default() -> Self {
        let fallback_prior = Distribution::default();
        let mut priors = HashMap::new();
        priors.insert(Axis::xp_per_hour(), fallback_prior);
        priors.insert(Axis::aa_xp_per_hour(), fallback_prior);
        priors.insert(Axis::platinum_per_hour(), fallback_prior);
        priors.insert(Axis::deaths_per_session(), fallback_prior);

        Self {
            prior_strength: 8.0,
            priors,
            fallback_prior,
        }
    }
}

impl CampPosteriorConfig {
    /// Override the prior strength.
    #[must_use]
    pub fn with_prior_strength(mut self, prior_strength: f64) -> Self {
        self.prior_strength = prior_strength;
        self
    }

    /// Override the prior for one axis.
    #[must_use]
    pub fn with_prior(mut self, axis: impl Into<Axis>, prior: Distribution) -> Self {
        self.priors.insert(axis.into(), prior);
        self
    }

    /// Return the configured prior for one axis.
    #[must_use]
    pub fn prior_for(&self, axis: &Axis) -> Distribution {
        self.priors
            .get(axis)
            .copied()
            .unwrap_or(self.fallback_prior)
    }
}

/// Normal-Normal fusion for a prior and telemetry summary.
///
/// This uses the prior-strength form described in the issue:
/// - posterior mean is a convex blend of prior mean and telemetry mean
/// - posterior variance shrinks smoothly as `n` grows
#[must_use]
pub fn posterior_from_summary(
    prior: Distribution,
    sample_mean: f64,
    sample_count: usize,
    prior_strength: f64,
) -> Distribution {
    if sample_count == 0 {
        return prior;
    }

    if prior_strength <= 0.0 {
        return Distribution::new(sample_mean, 0.0);
    }

    let n = sample_count as f64;
    let weight = n / (n + prior_strength);
    let mean = prior.mean + weight * (sample_mean - prior.mean);
    let variance = prior.variance() * (1.0 - weight);

    Distribution::new(mean, variance.sqrt())
}

/// Repository interface for camp posteriors.
pub trait CampPosteriorRepo {
    /// Load the posterior for one camp/party/axis.
    fn posterior(&self, camp_id: CampId, party: &PartySignature, axis: Axis) -> Distribution;

    /// Return the number of sessions fused into the camp/party posterior.
    fn n_sessions(&self, camp_id: CampId, party: &PartySignature) -> usize;

    /// Recompute all posterior rows for one camp after a session closes.
    fn refresh(&self, camp_id: CampId);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};

    #[test]
    fn posterior_converges_to_sample_mean_as_n_grows() {
        let prior = Distribution::new(10.0, 4.0);
        let sample_mean = 42.0;
        let prior_strength = 8.0;

        let mid = posterior_from_summary(prior, sample_mean, 8, prior_strength);
        let far = posterior_from_summary(prior, sample_mean, 800, prior_strength);

        assert!((far.mean - sample_mean).abs() < (mid.mean - sample_mean).abs());
        assert!((far.mean - sample_mean).abs() < 0.1);
    }

    #[test]
    fn posterior_mean_stays_between_prior_and_sample_mean() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(2689);

        for _ in 0..500 {
            let prior =
                Distribution::new(rng.random_range(-250.0..250.0), rng.random_range(0.5..40.0));
            let sample_mean = rng.random_range(-250.0..250.0);
            let sample_count = rng.random_range(1..2000);
            let prior_strength = rng.random_range(1.0..25.0);

            let posterior =
                posterior_from_summary(prior, sample_mean, sample_count, prior_strength);
            let low = prior.mean.min(sample_mean) - 1e-9;
            let high = prior.mean.max(sample_mean) + 1e-9;

            assert!(
                (low..=high).contains(&posterior.mean),
                "posterior mean {} must lie between prior {} and sample mean {}",
                posterior.mean,
                prior.mean,
                sample_mean
            );
        }
    }

    #[test]
    fn posterior_transition_is_smooth_around_kappa() {
        let prior = Distribution::new(12.0, 6.0);
        let sample_mean = 108.0;
        let prior_strength = 8.0;

        let before = posterior_from_summary(prior, sample_mean, 7, prior_strength);
        let at = posterior_from_summary(prior, sample_mean, 8, prior_strength);
        let after = posterior_from_summary(prior, sample_mean, 9, prior_strength);

        assert!(before.mean < at.mean);
        assert!(at.mean < after.mean);

        let step_left = at.mean - before.mean;
        let step_right = after.mean - at.mean;
        assert!((step_left - step_right).abs() < 1.0);
    }
}
