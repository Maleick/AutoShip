//! LinUCB (disjoint) contextual bandit.
//!
//! Each arm maintains its own `A_inv` (d×d) and `b` (d).  Sherman-Morrison
//! rank-1 updates keep `A_inv` current without ever computing a full matrix
//! inversion at serve time.  Selection is O(n_arms * d²).

use crate::bandit::{
    context::ContextVec,
    linalg::{axpy, dot, identity, mat_vec, quadratic_form, sherman_morrison_update},
    model::SerializedArm,
};

/// One arm of a LinUCB disjoint model.
#[derive(Debug, Clone)]
pub struct LinUcbArm {
    pub arm_id: usize,
    pub arm_label: String,
    d: usize,
    /// (A_k)^{-1}, maintained via Sherman-Morrison.
    pub a_inv: Vec<f32>,
    /// b_k = Σ r_t x_t
    pub b: Vec<f32>,
    /// Pre-computed theta = A_inv @ b (updated after each reward observation).
    pub mu: Vec<f32>,
    pub n_updates: u64,
}

impl LinUcbArm {
    pub fn new(arm_id: usize, label: impl Into<String>, d: usize) -> Self {
        Self {
            arm_id,
            arm_label: label.into(),
            d,
            a_inv: identity(d),
            b: vec![0.0; d],
            mu: vec![0.0; d],
            n_updates: 0,
        }
    }

    /// Upper confidence bound score for context `x`.
    pub fn score(&self, x: &[f32], alpha: f32) -> f32 {
        let d = self.d;
        debug_assert_eq!(x.len(), d);
        let exploit = dot(&self.mu, x);
        let explore = alpha * quadratic_form(&self.a_inv, x, d).max(0.0).sqrt();
        exploit + explore
    }

    /// Update arm with observed reward `r` for context `x`.
    pub fn update(&mut self, x: &[f32], r: f32) {
        let d = self.d;
        debug_assert_eq!(x.len(), d);
        // A_k += x x.T  (via Sherman-Morrison on the inverse)
        sherman_morrison_update(&mut self.a_inv, x, x, d);
        // b_k += r * x
        axpy(&mut self.b, x, r);
        // Refresh theta = A_inv @ b
        self.mu = mat_vec(&self.a_inv, &self.b, d);
        self.n_updates += 1;
    }

    /// Serialize to the compact on-disk format (no Cholesky — LinUCB does not need it).
    pub fn to_serialized(&self) -> SerializedArm {
        SerializedArm {
            arm_id: self.arm_id,
            arm_label: self.arm_label.clone(),
            a_inv: self.a_inv.clone(),
            b: self.b.clone(),
            mu: self.mu.clone(),
            chol_l: vec![],
            n_updates: self.n_updates,
        }
    }

    /// Reconstruct from serialized form.
    pub fn from_serialized(s: &SerializedArm, d: usize) -> Self {
        Self {
            arm_id: s.arm_id,
            arm_label: s.arm_label.clone(),
            d,
            a_inv: s.a_inv.clone(),
            b: s.b.clone(),
            mu: s.mu.clone(),
            n_updates: s.n_updates,
        }
    }
}

/// Multi-arm LinUCB model for a single decision point.
#[derive(Debug, Clone)]
pub struct LinUcbModel {
    pub arms: Vec<LinUcbArm>,
    pub alpha: f32,
    pub d: usize,
}

impl LinUcbModel {
    pub fn new(arm_labels: &[&str], d: usize, alpha: f32) -> Self {
        let arms = arm_labels
            .iter()
            .enumerate()
            .map(|(i, &label)| LinUcbArm::new(i, label, d))
            .collect();
        Self { arms, alpha, d }
    }

    /// Select the arm with the highest UCB score. Returns `(arm_id, score)`.
    pub fn select(&self, x: &ContextVec) -> (usize, f32) {
        let alpha = self.alpha;
        self.arms
            .iter()
            .map(|arm| (arm.arm_id, arm.score(x, alpha)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .expect("at least one arm")
    }

    /// Observe reward `r` for arm `arm_id` in context `x`.
    pub fn update(&mut self, arm_id: usize, x: &ContextVec, r: f32) {
        self.arms[arm_id].update(x, r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bandit::context::CTX_DIM;

    /// Synthetic bandit: arm 1 has true reward 1.0, arm 0 has reward 0.2.
    /// After 10K steps LinUCB should predominantly pick arm 1.
    #[test]
    fn test_linucb_converges_10k_steps() {
        let d = 4;
        let mut model = LinUcbModel::new(&["bad", "good"], d, 0.5);

        // Fixed context (all ones in first d slots).
        let mut ctx = [0.0f32; CTX_DIM];
        for i in 0..d {
            ctx[i] = 1.0;
        }
        let ctx_slice = &ctx[..d];

        let mut arm1_picks = 0usize;
        for step in 0..10_000usize {
            let (arm_id, _) = {
                // evaluate over slice-compatible arms
                let alpha = model.alpha;
                let best = model
                    .arms
                    .iter()
                    .map(|arm| (arm.arm_id, arm.score(ctx_slice, alpha)))
                    .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                    .unwrap();
                best
            };
            let reward = if arm_id == 1 { 1.0 } else { 0.2 };
            model.arms[arm_id].update(ctx_slice, reward);
            if step > 5_000 && arm_id == 1 {
                arm1_picks += 1;
            }
        }
        // Arm 1 should win > 90% of the last 5K steps.
        assert!(
            arm1_picks > 4_500,
            "LinUCB did not converge: arm1 picks = {}",
            arm1_picks
        );
    }
}
