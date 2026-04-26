//! Conjugate prior implementations for Bayesian suggestion engine.
//!
//! Provides closed-form posterior updates for:
//! - Beta-Binomial: binary success/failure knobs
//! - Gaussian: continuous knobs with known variance

use serde::{Deserialize, Serialize};

/// Trait for Bayesian posteriors with closed-form conjugate updates.
pub trait BayesianPosterior: Send + Sync {
    /// Get the posterior mean (expected value).
    fn mean(&self) -> f64;
    /// Get the posterior standard deviation.
    fn std_dev(&self) -> f64;
    /// Update posterior with a single observation.
    fn update(&mut self, observation: f64);
    /// Check if mean differs from value by ≥ 1σ.
    fn should_suggest(&self, current_value: f64) -> bool {
        (self.mean() - current_value).abs() >= self.std_dev()
    }
    /// Suggestion with proposed value and confidence.
    fn suggestion(&self, current_value: f64) -> Option<Suggestion> {
        if self.should_suggest(current_value) {
            Some(Suggestion {
                proposed: self.mean(),
                confidence: 1.0 - (-0.5 * ((self.mean() - current_value) / self.std_dev()).powi(2)).exp(),
            })
        } else {
            None
        }
    }
}

/// A suggestion to change a knob value.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Suggestion {
    /// Proposed posterior mean value.
    pub proposed: f64,
    /// Confidence (0.0–1.0) based on posterior variance.
    pub confidence: f64,
}

/// Beta-Binomial conjugate posterior for binary success/failure knobs.
///
/// Tracks successes and failures with a Beta prior. Posterior is Beta(α, β)
/// where α = prior_successes + observed_successes,
/// β = prior_failures + observed_failures.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BetaBinomial {
    /// α parameter: successes (pseudocounts + observed).
    alpha: f64,
    /// β parameter: failures (pseudocounts + observed).
    beta: f64,
}

impl BetaBinomial {
    /// Create a Beta-Binomial posterior with default weak priors.
    /// Uses Beta(1, 1) uniform prior.
    pub fn new() -> Self {
        Self { alpha: 1.0, beta: 1.0 }
    }

    /// Create with custom prior pseudocounts.
    pub fn with_prior(prior_successes: f64, prior_failures: f64) -> Self {
        Self {
            alpha: prior_successes,
            beta: prior_failures,
        }
    }

    /// Get current alpha (success) count.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Get current beta (failure) count.
    pub fn beta(&self) -> f64 {
        self.beta
    }
}

impl Default for BetaBinomial {
    fn default() -> Self {
        Self::new()
    }
}

impl BayesianPosterior for BetaBinomial {
    fn mean(&self) -> f64 {
        self.alpha / (self.alpha + self.beta)
    }

    fn std_dev(&self) -> f64 {
        let n = self.alpha + self.beta;
        let numerator = self.alpha * self.beta;
        let denominator = n * n * (n + 1.0);
        (numerator / denominator).sqrt()
    }

    fn update(&mut self, observation: f64) {
        if observation > 0.5 {
            self.alpha += 1.0;
        } else {
            self.beta += 1.0;
        }
    }
}

/// Gaussian conjugate posterior for continuous knobs.
///
/// Assumes known variance. Posterior is N(μ, σ²) where:
/// μ = (prior_mean / prior_variance + sum(obs) / obs_variance) / (1 / prior_variance + n / obs_variance)
/// σ² = 1 / (1 / prior_variance + n / obs_variance)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Gaussian {
    /// Posterior mean.
    pub(crate) mean_val: f64,
    /// Posterior variance.
    pub(crate) variance: f64,
    /// Known observation variance (fixed).
    pub(crate) obs_variance: f64,
    /// Sum of observations (for tracking).
    pub(crate) sum_obs: f64,
    /// Count of observations.
    pub(crate) count_obs: f64,
}

impl Gaussian {
    /// Create a Gaussian posterior with weak prior (large prior variance).
    /// Assumes obs_variance = 1.0.
    pub fn new() -> Self {
        Self::with_variance(1.0)
    }

    /// Create with specified observation variance.
    pub fn with_variance(obs_variance: f64) -> Self {
        Self {
            mean_val: 0.0,
            variance: 100.0, // weak prior: large variance
            obs_variance,
            sum_obs: 0.0,
            count_obs: 0.0,
        }
    }

    /// Create with explicit prior mean and variance.
    pub fn with_prior(prior_mean: f64, prior_variance: f64, obs_variance: f64) -> Self {
        Self {
            mean_val: prior_mean,
            variance: prior_variance,
            obs_variance,
            sum_obs: 0.0,
            count_obs: 0.0,
        }
    }
}

impl Default for Gaussian {
    fn default() -> Self {
        Self::new()
    }
}

impl BayesianPosterior for Gaussian {
    fn mean(&self) -> f64 {
        self.mean_val
    }

    fn std_dev(&self) -> f64 {
        self.variance.sqrt()
    }

    fn update(&mut self, observation: f64) {
        self.sum_obs += observation;
        self.count_obs += 1.0;

        // Update posterior mean and variance using conjugate prior update
        let prior_mean = self.mean_val;
        let prior_precision = 1.0 / self.variance;
        let obs_precision = 1.0 / self.obs_variance;
        let data_mean = self.sum_obs / self.count_obs;

        // Updated precision (inverse variance)
        let posterior_precision = prior_precision + self.count_obs * obs_precision;

        // Updated mean
        self.mean_val = (prior_precision * prior_mean + self.count_obs * obs_precision * data_mean) / posterior_precision;

        // Updated variance
        self.variance = 1.0 / posterior_precision;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beta_binomial_converges_to_true_success_rate() {
        let mut posterior = BetaBinomial::new();
        // Simulate 100 successes out of 100
        for _ in 0..100 {
            posterior.update(1.0);
        }
        assert!((posterior.mean() - 1.0).abs() < 0.05);
        assert!(posterior.std_dev() < 0.05);
    }

    #[test]
    fn beta_binomial_converges_to_mixed_rate() {
        let mut posterior = BetaBinomial::new();
        // Simulate 70 successes out of 100
        for i in 0..100 {
            posterior.update(if i < 70 { 1.0 } else { 0.0 });
        }
        assert!((posterior.mean() - 0.7).abs() < 0.05);
    }

    #[test]
    fn gaussian_converges_to_true_mean() {
        let mut posterior = Gaussian::with_prior(0.0, 100.0, 1.0);
        // Simulate observations centered at 50
        for i in 0..100 {
            let obs = 50.0 + (i as f64 % 10.0 - 5.0); // small noise
            posterior.update(obs);
        }
        assert!((posterior.mean() - 50.0).abs() < 1.0);
    }

    #[test]
    fn gaussian_variance_decreases_with_observations() {
        let mut posterior1 = Gaussian::with_prior(0.0, 100.0, 1.0);
        let mut posterior2 = Gaussian::with_prior(0.0, 100.0, 1.0);

        for i in 0..10 {
            posterior1.update(50.0);
        }
        for i in 0..100 {
            posterior2.update(50.0);
        }

        assert!(posterior2.variance < posterior1.variance);
    }

    #[test]
    fn suggestion_respects_1sigma_threshold() {
        let mut posterior = BetaBinomial::new();
        for _ in 0..50 {
            posterior.update(1.0);
        }

        // Current = 0.4, posterior mean ≈ 0.98, std_dev small
        assert!(posterior.should_suggest(0.4));

        // Current = 0.99, posterior mean ≈ 0.98, diff << 1σ
        assert!(!posterior.should_suggest(0.99));
    }
}
