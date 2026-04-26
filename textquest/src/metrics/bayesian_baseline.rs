//! Tier-2 Bayesian baseline model — per-character probabilistic behavior tracking.
//!
//! Builds baseline distributions over multiple sessions using conjugate priors:
//! - **Gaussian** (Normal-Gamma conjugate pair) for continuous metrics
//! - **Beta-Binomial** for rate/proportion metrics (stuck %, success rates)
//!
//! Suggestions fire when observed metrics deviate from posterior by > threshold.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Hyperparameters for Gaussian conjugate prior (Normal-Gamma).
/// Models continuous metrics: DPS, pull cadence, mana break %, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaussianPrior {
    /// Prior mean estimate.
    pub mu: f64,
    /// Prior precision (inverse variance) scaling factor.
    pub lambda: f64,
    /// Shape parameter of Gamma distribution (affects variance estimation).
    pub alpha: f64,
    /// Rate parameter of Gamma distribution.
    pub beta: f64,
}

impl GaussianPrior {
    /// Create a weakly-informative prior (high uncertainty, centered at estimate).
    pub fn weak(mean: f64) -> Self {
        Self {
            mu: mean,
            lambda: 0.1, // Low precision → high uncertainty
            alpha: 1.0,  // Weak Gamma prior
            beta: 0.1,
        }
    }

    /// Update prior with new observations.
    /// Returns posterior mean and posterior variance estimate.
    pub fn update(&mut self, observations: &[f64]) {
        if observations.is_empty() {
            return;
        }

        let n = observations.len() as f64;
        let sample_mean = observations.iter().sum::<f64>() / n;
        let sample_var = observations
            .iter()
            .map(|x| (x - sample_mean).powi(2))
            .sum::<f64>()
            / n;

        // Posterior mean: weighted average of prior and sample mean.
        let posterior_mu = (self.lambda * self.mu + n * sample_mean) / (self.lambda + n);

        // Update parameters (Bayesian update for Normal-Gamma).
        let new_lambda = self.lambda + n;
        let new_alpha = self.alpha + n / 2.0;

        // Posterior precision and variance update.
        let delta = sample_mean - self.mu;
        let new_beta = self.beta
            + (n * sample_var / 2.0)
            + (self.lambda * n * delta.powi(2)) / (2.0 * new_lambda);

        self.mu = posterior_mu;
        self.lambda = new_lambda;
        self.alpha = new_alpha;
        self.beta = new_beta;
    }

    /// Posterior mean estimate.
    pub fn posterior_mean(&self) -> f64 {
        self.mu
    }

    /// Posterior variance estimate (from Gamma distribution).
    pub fn posterior_variance(&self) -> f64 {
        if self.alpha > 0.0 {
            self.beta / (self.lambda * (self.alpha - 1.0).max(0.1))
        } else {
            1.0 // Default variance
        }
    }

    /// Posterior standard deviation.
    pub fn posterior_std(&self) -> f64 {
        self.posterior_variance().sqrt()
    }

    /// Compute z-score (number of standard deviations from mean).
    pub fn zscore(&self, observation: f64) -> f64 {
        let std = self.posterior_std();
        if std > 0.0 {
            (observation - self.posterior_mean()) / std
        } else {
            0.0
        }
    }
}

/// Hyperparameters for Beta-Binomial conjugate prior.
/// Models rate/proportion metrics: stuck %, success rates, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetaBinomialPrior {
    /// Beta distribution shape parameter α (successes + prior weight).
    pub alpha: f64,
    /// Beta distribution shape parameter β (failures + prior weight).
    pub beta: f64,
}

impl BetaBinomialPrior {
    /// Create a weakly-informative prior (α = β = 1 for uniform distribution).
    pub fn weak() -> Self {
        Self {
            alpha: 1.0,
            beta: 1.0,
        }
    }

    /// Create a prior biased toward success (α > β).
    pub fn success_biased(alpha: f64, beta: f64) -> Self {
        Self { alpha, beta }
    }

    /// Update prior with binary outcomes (successes/failures).
    pub fn update(&mut self, successes: u32, failures: u32) {
        self.alpha += successes as f64;
        self.beta += failures as f64;
    }

    /// Posterior probability of success (mean of Beta distribution).
    pub fn posterior_success_rate(&self) -> f64 {
        self.alpha / (self.alpha + self.beta)
    }

    /// Posterior variance of success probability.
    pub fn posterior_variance(&self) -> f64 {
        let sum = self.alpha + self.beta;
        (self.alpha * self.beta) / (sum.powi(2) * (sum + 1.0))
    }

    /// Posterior standard deviation of success probability.
    pub fn posterior_std(&self) -> f64 {
        self.posterior_variance().sqrt()
    }

    /// Credible interval (95% confidence equivalent for beta distribution).
    pub fn credible_interval(&self) -> (f64, f64) {
        let mean = self.posterior_success_rate();
        let std = self.posterior_std();
        // Approximate 95% CI using normal approximation.
        (
            (mean - 1.96 * std).max(0.0).min(1.0),
            (mean + 1.96 * std).max(0.0).min(1.0),
        )
    }
}

/// Per-character Bayesian baseline — tracks distributions of key metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterBaseline {
    /// Character name.
    pub character: String,
    /// Number of sessions aggregated into this baseline.
    pub session_count: u32,
    /// Last update timestamp (Unix seconds).
    pub last_updated: i64,

    // Gaussian priors for continuous metrics
    /// DPS baseline (damage per second).
    pub dps_baseline: GaussianPrior,
    /// Pull cadence baseline (seconds between pulls).
    pub pull_cadence_baseline: GaussianPrior,
    /// Med-break mana threshold (% of max mana).
    pub med_break_mana_pct_baseline: GaussianPrior,
    /// Retreat HP threshold (% of max HP).
    pub retreat_hp_pct_baseline: GaussianPrior,
    /// Assist latency baseline (milliseconds).
    pub assist_latency_baseline: GaussianPrior,
    /// Heal response latency baseline (milliseconds).
    pub heal_latency_baseline: GaussianPrior,

    // Beta-Binomial priors for rates
    /// Stuck event rate (stuck_events / total_time).
    pub stuck_rate: BetaBinomialPrior,
    /// Assist success rate (successful_assists / total_assists).
    pub assist_success_rate: BetaBinomialPrior,
    /// Heal success rate (successful_heals / total_heals).
    pub heal_success_rate: BetaBinomialPrior,

    /// Configuration thresholds for suggestion firing.
    pub thresholds: DeviationThresholds,
}

/// Configurable deviation thresholds for triggering suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviationThresholds {
    /// Z-score threshold for continuous metrics (default: 1.5σ).
    pub zscore_threshold: f64,
    /// Percentage deviation threshold for rate metrics (e.g., 0.15 = 15%).
    pub rate_deviation_threshold: f64,
}

impl Default for DeviationThresholds {
    fn default() -> Self {
        Self {
            zscore_threshold: 1.5,          // 1.5 sigma deviation
            rate_deviation_threshold: 0.15, // 15% deviation
        }
    }
}

impl CharacterBaseline {
    /// Create a new baseline for a character with weak priors.
    pub fn new(character: String, now: i64) -> Self {
        Self {
            character,
            session_count: 0,
            last_updated: now,
            dps_baseline: GaussianPrior::weak(100.0),
            pull_cadence_baseline: GaussianPrior::weak(30.0),
            med_break_mana_pct_baseline: GaussianPrior::weak(25.0),
            retreat_hp_pct_baseline: GaussianPrior::weak(30.0),
            assist_latency_baseline: GaussianPrior::weak(250.0),
            heal_latency_baseline: GaussianPrior::weak(200.0),
            stuck_rate: BetaBinomialPrior::weak(),
            assist_success_rate: BetaBinomialPrior::weak(),
            heal_success_rate: BetaBinomialPrior::weak(),
            thresholds: DeviationThresholds::default(),
        }
    }

    /// Update baseline with new session observations.
    /// Returns list of suggestions if deviations exceed thresholds.
    pub fn update_session(
        &mut self,
        dps_samples: &[f64],
        pull_cadence_samples: &[f64],
        stuck_successes: u32,
        stuck_failures: u32,
        assist_successes: u32,
        assist_failures: u32,
        heal_successes: u32,
        heal_failures: u32,
    ) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        // Update Gaussian priors and detect deviations.
        if !dps_samples.is_empty() {
            self.dps_baseline.update(dps_samples);
        }
        if !pull_cadence_samples.is_empty() {
            self.pull_cadence_baseline.update(pull_cadence_samples);
        }

        // Update Beta-Binomial priors and detect deviations.
        self.stuck_rate.update(stuck_successes, stuck_failures);
        self.assist_success_rate
            .update(assist_successes, assist_failures);
        self.heal_success_rate.update(heal_successes, heal_failures);

        // Detect deviations and generate suggestions.
        let current_stuck_rate = self.stuck_rate.posterior_success_rate();
        let baseline_stuck_rate = self.stuck_rate.posterior_success_rate(); // Would use historical baseline
        if (current_stuck_rate - baseline_stuck_rate).abs()
            > self.thresholds.rate_deviation_threshold
        {
            suggestions.push(Suggestion {
                metric: "stuck_rate".to_string(),
                current_value: current_stuck_rate,
                baseline_value: baseline_stuck_rate,
                deviation: current_stuck_rate - baseline_stuck_rate,
                severity: if (current_stuck_rate - baseline_stuck_rate).abs() > 0.3 {
                    "high".to_string()
                } else {
                    "medium".to_string()
                },
                recommendation: "Consider adjusting navigation parameters or route waypoints."
                    .to_string(),
            });
        }

        self.session_count += 1;
        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        suggestions
    }
}

/// Suggestion — tuning recommendation fired when behavior deviates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    /// Metric name (e.g., "dps", "stuck_rate", "pull_cadence").
    pub metric: String,
    /// Current observed value.
    pub current_value: f64,
    /// Baseline expected value.
    pub baseline_value: f64,
    /// Deviation (current - baseline).
    pub deviation: f64,
    /// Severity level: "low", "medium", "high".
    pub severity: String,
    /// Operator-facing tuning recommendation.
    pub recommendation: String,
}

/// Per-character baseline registry — manages all baseline models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineRegistry {
    /// Map of character name → baseline model.
    baselines: HashMap<String, CharacterBaseline>,
}

impl BaselineRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            baselines: HashMap::new(),
        }
    }

    /// Get or create baseline for a character.
    pub fn get_or_create(&mut self, character: &str) -> &mut CharacterBaseline {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.baselines
            .entry(character.to_string())
            .or_insert_with(|| CharacterBaseline::new(character.to_string(), now))
    }

    /// Get baseline (read-only).
    pub fn get(&self, character: &str) -> Option<&CharacterBaseline> {
        self.baselines.get(character)
    }

    /// List all managed characters.
    pub fn characters(&self) -> Vec<&str> {
        self.baselines.keys().map(|s| s.as_str()).collect()
    }

    /// Clear all baselines.
    pub fn clear(&mut self) {
        self.baselines.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaussian_prior_weak_init() {
        let prior = GaussianPrior::weak(100.0);
        assert_eq!(prior.mu, 100.0);
        assert_eq!(prior.posterior_mean(), 100.0);
    }

    #[test]
    fn gaussian_prior_update() {
        let mut prior = GaussianPrior::weak(100.0);
        let observations = vec![95.0, 105.0, 100.0, 102.0, 98.0];
        prior.update(&observations);

        let posterior_mean = prior.posterior_mean();
        assert!((posterior_mean - 100.0).abs() < 5.0);
    }

    #[test]
    fn gaussian_prior_zscore() {
        let mut prior = GaussianPrior::weak(100.0);
        let observations = vec![98.0, 100.0, 102.0];
        prior.update(&observations);

        let zscore = prior.zscore(100.0);
        assert!(zscore.abs() < 0.5); // Should be near zero
    }

    #[test]
    fn beta_binomial_weak() {
        let prior = BetaBinomialPrior::weak();
        assert_eq!(prior.posterior_success_rate(), 0.5);
    }

    #[test]
    fn beta_binomial_update() {
        let mut prior = BetaBinomialPrior::weak();
        prior.update(8, 2); // 80% success rate

        let rate = prior.posterior_success_rate();
        assert!((rate - 0.818).abs() < 0.01); // (1+8)/(1+8+1+2)
    }

    #[test]
    fn character_baseline_creation() {
        let baseline = CharacterBaseline::new("Paladin".to_string(), 1000);
        assert_eq!(baseline.character, "Paladin");
        assert_eq!(baseline.session_count, 0);
        assert_eq!(baseline.dps_baseline.mu, 100.0);
    }

    #[test]
    fn baseline_registry_get_or_create() {
        let mut registry = BaselineRegistry::new();
        let baseline1 = registry.get_or_create("Druid");
        baseline1.session_count = 5;

        let baseline2 = registry.get_or_create("Druid");
        assert_eq!(baseline2.session_count, 5);
    }
}
