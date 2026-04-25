//! Thompson sampling (Bayesian linear regression) contextual bandit.
//!
//! The posterior over arm θ_k is N(μ_k, v² A_k^{-1}).
//! At training time the Cholesky factor of v² A_k^{-1} is pre-computed and stored.
//! At serve time: θ_k = μ_k + L_k @ z where z ~ N(0, I); pick argmax θ_k.T x.

use rand::Rng;

/// Box-Muller transform — sample one value from N(0,1).
fn sample_standard_normal<R: Rng>(rng: &mut R) -> f32 {
    let u1: f32 = rng.gen_range(1e-10_f32..1.0_f32);
    let u2: f32 = rng.r#gen::<f32>();
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
}

use crate::bandit::{
    context::ContextVec,
    linalg::{
        axpy, cholesky_lower, dot, identity, lower_tri_mat_vec, mat_scale, mat_vec,
        sherman_morrison_update,
    },
    model::SerializedArm,
};

/// One arm of a Thompson-sampling model.
#[derive(Debug, Clone)]
pub struct ThompsonArm {
    pub arm_id: usize,
    pub arm_label: String,
    d: usize,
    /// (A_k)^{-1}, maintained via Sherman-Morrison.
    pub a_inv: Vec<f32>,
    /// b_k = Σ r_t x_t
    pub b: Vec<f32>,
    /// Pre-computed θ̂ = A_inv @ b.
    pub mu: Vec<f32>,
    /// Pre-computed L = chol(v² A_inv); empty until `finalize()` is called.
    pub chol_l: Vec<f32>,
    pub n_updates: u64,
    /// Exploration variance scale.
    pub v: f32,
}

impl ThompsonArm {
    pub fn new(arm_id: usize, label: impl Into<String>, d: usize, v: f32) -> Self {
        Self {
            arm_id,
            arm_label: label.into(),
            d,
            a_inv: identity(d),
            b: vec![0.0; d],
            mu: vec![0.0; d],
            chol_l: vec![],
            n_updates: 0,
            v,
        }
    }

    /// Update arm with observed reward `r` for context `x`.
    pub fn update(&mut self, x: &[f32], r: f32) {
        let d = self.d;
        debug_assert_eq!(x.len(), d);
        sherman_morrison_update(&mut self.a_inv, x, x, d);
        axpy(&mut self.b, x, r);
        self.mu = mat_vec(&self.a_inv, &self.b, d);
        self.n_updates += 1;
        // Invalidate Cholesky — recomputed on next finalize() or serve.
        self.chol_l.clear();
    }

    /// Pre-compute (or refresh) the Cholesky factor of `v² A_inv`.
    /// Call this after training before serializing.  At serve time `serve()` calls
    /// this lazily if `chol_l` is empty.
    pub fn finalize(&mut self) {
        let d = self.d;
        let scaled = mat_scale(&self.a_inv, self.v * self.v, d);
        self.chol_l = cholesky_lower(&scaled, d)
            .unwrap_or_else(|| identity(d).iter().map(|x| x * self.v).collect());
    }

    /// Sample θ from the posterior and return θ.T x.
    pub fn sample_score<R: Rng>(&mut self, x: &[f32], rng: &mut R) -> f32 {
        if self.chol_l.is_empty() {
            self.finalize();
        }
        let d = self.d;
        let z: Vec<f32> = (0..d)
            .map(|_| sample_standard_normal(rng))
            .collect();
        let perturbation = lower_tri_mat_vec(&self.chol_l, &z, d);
        // θ = μ + L @ z
        let mut theta = self.mu.clone();
        for i in 0..d {
            theta[i] += perturbation[i];
        }
        dot(&theta, x)
    }

    pub fn to_serialized(&self) -> SerializedArm {
        let mut arm = self.clone();
        if arm.chol_l.is_empty() {
            arm.finalize();
        }
        SerializedArm {
            arm_id: arm.arm_id,
            arm_label: arm.arm_label.clone(),
            a_inv: arm.a_inv.clone(),
            b: arm.b.clone(),
            mu: arm.mu.clone(),
            chol_l: arm.chol_l.clone(),
            n_updates: arm.n_updates,
        }
    }

    pub fn from_serialized(s: &SerializedArm, d: usize, v: f32) -> Self {
        Self {
            arm_id: s.arm_id,
            arm_label: s.arm_label.clone(),
            d,
            a_inv: s.a_inv.clone(),
            b: s.b.clone(),
            mu: s.mu.clone(),
            chol_l: s.chol_l.clone(),
            n_updates: s.n_updates,
            v,
        }
    }
}

/// Multi-arm Thompson sampling model for a single decision point.
#[derive(Debug, Clone)]
pub struct ThompsonModel {
    pub arms: Vec<ThompsonArm>,
    pub d: usize,
}

impl ThompsonModel {
    pub fn new(arm_labels: &[&str], d: usize, v: f32) -> Self {
        let arms = arm_labels
            .iter()
            .enumerate()
            .map(|(i, &label)| ThompsonArm::new(i, label, d, v))
            .collect();
        Self { arms, d }
    }

    /// Sample from each arm's posterior and return the arm with the highest sampled score.
    pub fn select<R: Rng>(&mut self, x: &ContextVec, rng: &mut R) -> (usize, f32) {
        let x_slice = &x[..self.d];
        let mut best_id = 0;
        let mut best_score = f32::NEG_INFINITY;
        for arm in &mut self.arms {
            let score = arm.sample_score(x_slice, rng);
            if score > best_score {
                best_score = score;
                best_id = arm.arm_id;
            }
        }
        (best_id, best_score)
    }

    pub fn update(&mut self, arm_id: usize, x: &ContextVec, r: f32) {
        self.arms[arm_id].update(&x[..self.d], r);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bandit::context::CTX_DIM;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    /// Synthetic bandit: arm 1 reward 1.0, arm 0 reward 0.2.
    /// After 10K steps Thompson should predominantly pick arm 1.
    #[test]
    fn test_thompson_converges_10k_steps() {
        let d = 4;
        let mut model = ThompsonModel::new(&["bad", "good"], d, 0.5);
        let mut rng = SmallRng::seed_from_u64(42);

        let mut ctx = [0.0f32; CTX_DIM];
        for i in 0..d {
            ctx[i] = 1.0;
        }

        let mut arm1_picks = 0usize;
        for step in 0..10_000usize {
            let (arm_id, _) = model.select(&ctx, &mut rng);
            let reward = if arm_id == 1 { 1.0 } else { 0.2 };
            model.update(arm_id, &ctx, reward);
            if step > 5_000 && arm_id == 1 {
                arm1_picks += 1;
            }
        }
        assert!(
            arm1_picks > 4_300,
            "Thompson did not converge: arm1 picks = {}",
            arm1_picks
        );
    }
}
