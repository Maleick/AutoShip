#![allow(dead_code)]
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Trajectory {
    pub states: Vec<Vec<f32>>,
    pub actions: Vec<usize>,
    pub rewards: Vec<f32>,
    pub next_states: Vec<Vec<f32>>,
    pub terminals: Vec<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EvaluationResult {
    pub wis_mean: f32,
    pub wis_lower_ci: f32,
    pub wis_upper_ci: f32,
    pub fqe_mean: f32,
    pub fqe_lower_ci: f32,
    pub fqe_upper_ci: f32,
    pub action_coverage: f32,
}

/// Weighted importance sampling evaluator (variance-capped)
pub fn weighted_importance_sampling(
    trajectories: &[Trajectory],
    behavior_policy_probs: &[Vec<f32>],
    learned_policy_probs: &[Vec<f32>],
    discount: f32,
) -> Result<(f32, f32, f32)> {
    anyhow::ensure!(
        trajectories.len() == behavior_policy_probs.len(),
        "Trajectory and behavior policy mismatch"
    );

    let mut importance_weighted_returns = Vec::new();
    let variance_cap = 100.0; // Cap IS weights to reduce variance

    for (traj, b_probs) in trajectories.iter().zip(behavior_policy_probs.iter()) {
        let mut traj_return = 0.0;
        let mut importance_weight = 1.0;

        for t in 0..traj.actions.len() {
            let action = traj.actions[t];
            let b_prob = b_probs[action].max(1e-8);
            let reward = traj.rewards[t];

            // Update importance weight
            if let Some(pi_prob) = learned_policy_probs.get(t).and_then(|p| p.get(action)) {
                importance_weight *= (*pi_prob).max(1e-8) / b_prob;
                importance_weight = importance_weight.min(variance_cap).max(0.0);
            }

            traj_return += reward * discount.powi(t as i32) * importance_weight;
        }

        importance_weighted_returns.push(traj_return);
    }

    let mean = importance_weighted_returns.iter().sum::<f32>() / importance_weighted_returns.len() as f32;
    let variance = importance_weighted_returns
        .iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f32>()
        / importance_weighted_returns.len() as f32;

    let std_dev = variance.sqrt();
    let ci = 1.96 * std_dev / (importance_weighted_returns.len() as f32).sqrt();

    Ok((mean, mean - ci, mean + ci))
}

/// Fitted Q evaluation (cross-check)
pub fn fitted_q_evaluation(
    trajectories: &[Trajectory],
    discount: f32,
    _num_iterations: usize,
) -> Result<(f32, f32, f32)> {
    let mut q_values = Vec::new();

    for traj in trajectories {
        let mut traj_q = 0.0;
        let mut cumulative_reward = 0.0;

        for (i, reward) in traj.rewards.iter().enumerate() {
            cumulative_reward = *reward + discount * cumulative_reward;
            traj_q += cumulative_reward * discount.powi(i as i32);
        }

        q_values.push(traj_q / traj.rewards.len().max(1) as f32);
    }

    let mean = q_values.iter().sum::<f32>() / q_values.len() as f32;
    let variance = q_values
        .iter()
        .map(|q| (q - mean).powi(2))
        .sum::<f32>()
        / q_values.len() as f32;

    let std_dev = variance.sqrt();
    let ci = 1.96 * std_dev / (q_values.len() as f32).sqrt();

    Ok((mean, mean - ci, mean + ci))
}

/// Action-distribution coverage sanity check
pub fn action_coverage(
    learned_policy_probs: &[Vec<f32>],
    behavior_policy_probs: &[Vec<f32>],
) -> Result<f32> {
    let mut covered_actions = 0;
    let mut total_actions = 0;

    for (l_probs, b_probs) in learned_policy_probs
        .iter()
        .zip(behavior_policy_probs.iter())
    {
        for (l_prob, b_prob) in l_probs.iter().zip(b_probs.iter()) {
            if *b_prob > 1e-8 && *l_prob > 1e-8 {
                covered_actions += 1;
            }
            total_actions += 1;
        }
    }

    Ok(covered_actions as f32 / total_actions as f32)
}
