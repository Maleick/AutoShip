//! High-level contextual bandit interface and implementations.
//!
//! This module provides:
//! - `Context` wrapper over fixed-width feature vectors
//! - `Arm` trait for decision outcomes
//! - `BanditPolicy` trait for arm-selection strategies
//! - `EpsilonGreedyBandit` baseline implementation
//! - `LinUCBBandit` wrapper over low-level LinUCB model

use crate::bandit::context::ContextVec;
use crate::bandit::linucb::LinUcbModel;
use rand::Rng;

/// Fixed-width context feature vector (alias for clarity).
pub type Context = ContextVec;

/// Trait for any decision outcome that can be rewarded.
pub trait Arm: Send + Sync {
    /// Unique identifier for this arm.
    fn id(&self) -> usize;

    /// Human-readable label (e.g., "camp_radius_5", "heal_conservative").
    fn label(&self) -> &str;
}

/// Abstract policy for arm selection and reward observation.
pub trait BanditPolicy: Send + Sync {
    /// Select an arm given the current context.
    /// Returns the arm ID and an optional score/confidence value.
    fn select_arm(&mut self, context: &Context) -> (usize, f32);

    /// Observe a reward for a previously-selected arm in a given context.
    fn update(&mut self, arm_id: usize, context: &Context, reward: f32);

    /// Optional: Finalize before serialization (e.g., pre-compute Cholesky).
    fn finalize(&mut self) {}

    /// Optional: Get human-readable policy name for logging.
    fn policy_name(&self) -> &str {
        "unknown"
    }
}

/// Baseline epsilon-greedy bandit: exploit best-observed arm with probability (1-ε),
/// else explore uniformly at random.
#[derive(Debug, Clone)]
pub struct EpsilonGreedyBandit {
    /// Number of arms.
    n_arms: usize,
    /// Cumulative rewards per arm.
    rewards_sum: Vec<f32>,
    /// Number of pulls per arm.
    pulls: Vec<u64>,
    /// Exploration probability (0.0 = pure exploit, 1.0 = pure explore).
    epsilon: f32,
    #[allow(dead_code)]
    arm_labels: Vec<String>,
}

impl EpsilonGreedyBandit {
    /// Create a new epsilon-greedy bandit with the given arm labels and exploration rate.
    pub fn new(arm_labels: &[&str], epsilon: f32) -> Self {
        let n_arms = arm_labels.len();
        assert!(n_arms > 0, "must have at least one arm");
        assert!(
            (0.0..=1.0).contains(&epsilon),
            "epsilon must be in [0, 1]"
        );
        Self {
            n_arms,
            rewards_sum: vec![0.0; n_arms],
            pulls: vec![0; n_arms],
            epsilon,
            arm_labels: arm_labels.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Mean reward observed for arm `arm_id`.
    pub fn mean_reward(&self, arm_id: usize) -> f32 {
        if self.pulls[arm_id] == 0 {
            0.0
        } else {
            self.rewards_sum[arm_id] / self.pulls[arm_id] as f32
        }
    }

    /// Number of times arm `arm_id` has been pulled.
    pub fn pull_count(&self, arm_id: usize) -> u64 {
        self.pulls[arm_id]
    }
}

impl BanditPolicy for EpsilonGreedyBandit {
    fn select_arm(&mut self, _context: &Context) -> (usize, f32) {
        // Epsilon-greedy: with probability epsilon, pick uniformly; else pick best mean reward.
        let mut rng = rand::rng();
        let arm_id = if rng.random::<f32>() < self.epsilon {
            rng.random_range(0..self.n_arms)
        } else {
            (0..self.n_arms)
                .max_by(|&a, &b| {
                    self.mean_reward(a)
                        .partial_cmp(&self.mean_reward(b))
                        .unwrap()
                })
                .unwrap()
        };
        let score = self.mean_reward(arm_id);
        (arm_id, score)
    }

    fn update(&mut self, arm_id: usize, _context: &Context, reward: f32) {
        assert!(
            arm_id < self.n_arms,
            "arm_id {} out of range [0, {})",
            arm_id,
            self.n_arms
        );
        self.rewards_sum[arm_id] += reward;
        self.pulls[arm_id] += 1;
    }

    fn policy_name(&self) -> &str {
        "epsilon-greedy"
    }
}

/// Contextual LinUCB bandit: leverages context features to learn per-arm reward functions.
/// Maintains a low-rank linear model `θ_k.T x` per arm, with optimism under uncertainty.
pub struct LinUCBBandit {
    model: LinUcbModel,
    d: usize,
}

impl LinUCBBandit {
    /// Create a new LinUCB bandit with the given arm labels, context dimension, and exploration bonus.
    ///
    /// `alpha` controls the confidence radius. Typical range: 0.1 to 1.0.
    /// Higher values → more exploration.
    pub fn new(arm_labels: &[&str], d: usize, alpha: f32) -> Self {
        assert!(d > 0, "context dimension must be > 0");
        assert!(d <= 32, "context dimension must be <= 32");
        assert!(alpha > 0.0, "alpha must be positive");
        let model = LinUcbModel::new(arm_labels, d, alpha);
        Self { model, d }
    }

    /// Access the underlying LinUCB model for advanced use cases.
    pub fn model(&self) -> &LinUcbModel {
        &self.model
    }

    /// Mutable access to the underlying model.
    pub fn model_mut(&mut self) -> &mut LinUcbModel {
        &mut self.model
    }
}

impl BanditPolicy for LinUCBBandit {
    fn select_arm(&mut self, context: &Context) -> (usize, f32) {
        let x_slice = &context[..self.d];
        // Compute UCB score for each arm, select max.
        let (arm_id, score) = self.model.arms
            .iter()
            .map(|arm| {
                let s = arm.score(x_slice, self.model.alpha);
                (arm.arm_id, s)
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .expect("at least one arm");
        (arm_id, score)
    }

    fn update(&mut self, arm_id: usize, context: &Context, reward: f32) {
        self.model.update(arm_id, context, reward);
    }

    fn policy_name(&self) -> &str {
        "linucb"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epsilon_greedy_basic() {
        let mut bandit = EpsilonGreedyBandit::new(&["a", "b"], 0.0);
        let ctx = [0.0f32; 32];

        // Pull arm 0 with high reward.
        bandit.update(0, &ctx, 1.0);
        bandit.update(0, &ctx, 1.0);

        // Pull arm 1 with low reward.
        bandit.update(1, &ctx, 0.1);

        // With epsilon=0, should always pick arm 0.
        for _ in 0..10 {
            let (arm, _) = bandit.select_arm(&ctx);
            assert_eq!(arm, 0, "should always pick best arm with epsilon=0");
        }

        assert_eq!(bandit.pull_count(0), 2);
        assert_eq!(bandit.pull_count(1), 1);
        assert!((bandit.mean_reward(0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_epsilon_greedy_exploration() {
        let mut bandit = EpsilonGreedyBandit::new(&["a", "b"], 1.0);
        let ctx = [0.0f32; 32];

        // With epsilon=1.0, should explore uniformly.
        // After 100 pulls, both arms should have been picked at least once (very high probability).
        for _ in 0..100 {
            let (arm, _) = bandit.select_arm(&ctx);
            bandit.update(arm, &ctx, 0.5);
        }

        assert!(bandit.pull_count(0) > 0, "arm 0 should have been pulled");
        assert!(bandit.pull_count(1) > 0, "arm 1 should have been pulled");
    }

    #[test]
    fn test_linucb_basic() {
        let mut bandit = LinUCBBandit::new(&["a", "b"], 4, 0.5);
        let d = bandit.d;
        let mut ctx = [0.0f32; 32];
        for x in ctx[..4].iter_mut() {
            *x = 1.0;
        }

        // Simulate arm 1 being better.
        for _ in 0..100 {
            let (arm, _) = bandit.select_arm(&ctx);
            let reward = if arm == 1 { 1.0 } else { 0.2 };
            bandit.model_mut().arms[arm].update(&ctx[..d], reward);
        }

        // After enough steps, arm 1 should be selected most of the time.
        let mut arm1_count = 0;
        for _ in 0..100 {
            let (arm, _) = bandit.select_arm(&ctx);
            if arm == 1 {
                arm1_count += 1;
            }
            let reward = if arm == 1 { 1.0 } else { 0.2 };
            bandit.model_mut().arms[arm].update(&ctx[..d], reward);
        }
        assert!(arm1_count > 50, "linucb should converge on arm 1");
    }

    #[test]
    fn test_linucb_dimensions() {
        let bandit = LinUCBBandit::new(&["x", "y", "z"], 12, 1.0);
        assert_eq!(bandit.model.arms.len(), 3);
        assert_eq!(bandit.model.d, 12);
    }

    #[test]
    fn test_policy_name() {
        let eg = EpsilonGreedyBandit::new(&["a"], 0.5);
        assert_eq!(eg.policy_name(), "epsilon-greedy");

        let linucb = LinUCBBandit::new(&["a"], 4, 0.5);
        assert_eq!(linucb.policy_name(), "linucb");
    }
}
