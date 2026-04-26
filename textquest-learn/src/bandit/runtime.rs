//! DLL-side bandit runtime — loads a model file and serves picks in ≤50 μs.
//!
//! The runtime is generic over the RNG so the DLL can supply a fast PRNG while
//! tests use a seeded deterministic RNG.

use rand::rngs::SmallRng;
use rand::SeedableRng;
use siphasher::sip::SipHasher13;
use std::hash::Hasher;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bandit::{
    context::{ContextSpec, ContextVec, CTX_DIM},
    linucb::LinUcbArm,
    model::{AlgorithmTag, ModelFile},
    shadow::{PolicyMode, ShadowLog},
    thompson::ThompsonArm,
};

/// Errors that can occur when loading or using a [`BanditRuntime`].
#[derive(Debug, thiserror::Error)]
pub enum BanditError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("decode error: {0}")]
    Decode(#[from] bincode::error::DecodeError),
    #[error("context spec hash mismatch: model={model_hash:#x} runtime={runtime_hash:#x}")]
    SchemaMismatch { model_hash: u64, runtime_hash: u64 },
    #[error("model version {found} != expected {expected}")]
    VersionMismatch { found: u32, expected: u32 },
    #[error("invalid model context_dim={found}; expected {expected} and <= {max}")]
    InvalidContextDim {
        found: u32,
        expected: u32,
        max: usize,
    },
}

enum ArmState {
    LinUcb(LinUcbArm),
    Thompson(ThompsonArm),
}

impl ArmState {
    fn arm_id(&self) -> usize {
        match self {
            ArmState::LinUcb(a) => a.arm_id,
            ArmState::Thompson(a) => a.arm_id,
        }
    }
    fn arm_label(&self) -> &str {
        match self {
            ArmState::LinUcb(a) => &a.arm_label,
            ArmState::Thompson(a) => &a.arm_label,
        }
    }
}

/// Loaded, ready-to-serve bandit for a single decision scope.
pub struct BanditRuntime {
    model: ModelFile,
    arms: Vec<ArmState>,
    pub policy_mode: PolicyMode,
    rng: SmallRng,
    context_dim: usize,
}

impl std::fmt::Debug for BanditRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BanditRuntime")
            .field("scope", &self.model.scope.as_key())
            .field("algorithm", &self.model.algorithm)
            .field("n_arms", &self.arms.len())
            .field("policy_mode", &self.policy_mode)
            .finish()
    }
}

impl BanditRuntime {
    /// Load and validate a model file from disk.
    ///
    /// `expected_spec` must match the spec hash stored in the model or
    /// `BanditError::SchemaMismatch` is returned.
    pub fn load(path: &Path, expected_spec: &ContextSpec) -> Result<Self, BanditError> {
        let data = std::fs::read(path)?;
        Self::from_bytes(&data, expected_spec)
    }

    /// Load from an in-memory byte slice (useful for embedding in the DLL).
    pub fn from_bytes(data: &[u8], expected_spec: &ContextSpec) -> Result<Self, BanditError> {
        use crate::bandit::model::MODEL_FILE_VERSION;
        let model = ModelFile::from_bytes(data)?;

        if model.version != MODEL_FILE_VERSION {
            return Err(BanditError::VersionMismatch {
                found: model.version,
                expected: MODEL_FILE_VERSION,
            });
        }
        let expected_hash = expected_spec.hash_fingerprint();
        if model.context_spec_hash != expected_hash {
            return Err(BanditError::SchemaMismatch {
                model_hash: model.context_spec_hash,
                runtime_hash: expected_hash,
            });
        }

        if model.context_dim != expected_spec.dim || (model.context_dim as usize) > CTX_DIM {
            return Err(BanditError::InvalidContextDim {
                found: model.context_dim,
                expected: expected_spec.dim,
                max: CTX_DIM,
            });
        }

        let d = model.context_dim as usize;
        let param = model.exploration_param;
        let arms: Vec<ArmState> = model
            .arms
            .iter()
            .map(|s| match model.algorithm {
                AlgorithmTag::LinUcb => ArmState::LinUcb(LinUcbArm::from_serialized(s, d)),
                AlgorithmTag::Thompson => {
                    ArmState::Thompson(ThompsonArm::from_serialized(s, d, param))
                }
            })
            .collect();

        Ok(Self {
            model,
            arms,
            policy_mode: PolicyMode::Shadow,
            rng: SmallRng::from_os_rng(),
            context_dim: d,
        })
    }

    /// Select the best arm for context `x` according to the bandit.
    ///
    /// For LinUCB: deterministic argmax UCB score.
    /// For Thompson: samples from posterior, argmax sampled score.
    pub fn select(&mut self, x: &ContextVec) -> usize {
        match self.model.algorithm {
            AlgorithmTag::LinUcb => self.select_linucb(x),
            AlgorithmTag::Thompson => self.select_thompson(x),
        }
    }

    fn select_linucb(&self, x: &ContextVec) -> usize {
        let alpha = self.model.exploration_param;
        let d = self.context_dim;
        let x_slice = &x[..d];
        self.arms
            .iter()
            .filter_map(|a| {
                if let ArmState::LinUcb(arm) = a {
                    Some((arm.arm_id, arm.score(x_slice, alpha)))
                } else {
                    None
                }
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(id, _)| id)
            .unwrap_or(0)
    }

    fn select_thompson(&mut self, x: &ContextVec) -> usize {
        let d = self.context_dim;
        let x_slice = &x[..d];
        let rng = &mut self.rng;
        self.arms
            .iter_mut()
            .filter_map(|a| {
                if let ArmState::Thompson(arm) = a {
                    let score = arm.sample_score(x_slice, rng);
                    Some((arm.arm_id, score))
                } else {
                    None
                }
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .map(|(id, _)| id)
            .unwrap_or(0)
    }

    /// Shadow-mode dispatch: pick both bandit and rule arm; log shadow event;
    /// return only the rule-based arm (or bandit arm if policy is Live).
    ///
    /// `rule_arm` is the arm index chosen by the rule-based rotation.
    pub fn serve_with_shadow<W: Write>(
        &mut self,
        x: &ContextVec,
        rule_arm: usize,
        shadow_log: &mut ShadowLog<W>,
    ) -> usize {
        let bandit_arm = self.select(x);

        let state_hash = self.hash_context(x);
        let ts_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let bandit_label = self
            .arms
            .iter()
            .find(|a| a.arm_id() == bandit_arm)
            .map(|a| a.arm_label().to_string())
            .unwrap_or_default();
        let rule_label = self
            .arms
            .iter()
            .find(|a| a.arm_id() == rule_arm)
            .map(|a| a.arm_label().to_string())
            .unwrap_or_default();

        shadow_log.record(
            ts_secs,
            state_hash,
            bandit_arm,
            &bandit_label,
            rule_arm,
            &rule_label,
        );

        if self.policy_mode.is_shadow() {
            rule_arm
        } else {
            bandit_arm
        }
    }

    fn hash_context(&self, x: &ContextVec) -> u64 {
        let mut h = SipHasher13::new();
        for &v in x.iter() {
            h.write_u32(v.to_bits());
        }
        h.finish()
    }

    pub fn scope_key(&self) -> String {
        self.model.scope.as_key()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bandit::{
        context::{ContextSpec, CTX_DIM},
        linucb::LinUcbModel,
        model::{AlgorithmTag, BanditScope, ModelFile, MODEL_FILE_VERSION},
        shadow::ShadowLog,
    };

    fn make_linucb_bytes(spec: &ContextSpec) -> Vec<u8> {
        let d = spec.dim as usize;
        let linucb = LinUcbModel::new(&["CH", "Heal", "Cure"], d, 1.0);
        let file = ModelFile {
            version: MODEL_FILE_VERSION,
            context_spec_hash: spec.hash_fingerprint(),
            scope: BanditScope::new("cleric", "heal_picker"),
            algorithm: AlgorithmTag::LinUcb,
            exploration_param: 1.0,
            context_dim: spec.dim,
            arms: linucb.arms.iter().map(|a| a.to_serialized()).collect(),
        };
        file.to_bytes().unwrap()
    }

    #[test]
    fn test_shadow_mode_executes_rule_pick() {
        let spec = ContextSpec::base();
        let bytes = make_linucb_bytes(&spec);
        let mut runtime = BanditRuntime::from_bytes(&bytes, &spec).unwrap();
        assert!(runtime.policy_mode.is_shadow());

        let ctx = [0.5f32; CTX_DIM];
        let rule_arm = 2usize;
        let buf: Vec<u8> = Vec::new();
        let scope = BanditScope::new("cleric", "heal_picker");
        let mut log = ShadowLog::new(buf, scope);

        let result = runtime.serve_with_shadow(&ctx, rule_arm, &mut log);
        assert_eq!(result, rule_arm, "shadow mode must return rule pick");
    }

    #[test]
    fn test_schema_drift_rejected_at_load() {
        let spec_train = ContextSpec::base();
        let spec_runtime = ContextSpec::with_abilities(&["heal", "cure"]);
        let bytes = make_linucb_bytes(&spec_train);
        let err = BanditRuntime::from_bytes(&bytes, &spec_runtime).unwrap_err();
        assert!(
            matches!(err, BanditError::SchemaMismatch { .. }),
            "expected SchemaMismatch"
        );
    }

    #[test]
    fn test_oversized_context_dim_rejected_at_load() {
        let spec = ContextSpec::base();
        let mut model = ModelFile::from_bytes(&make_linucb_bytes(&spec)).unwrap();
        model.context_spec_hash = spec.hash_fingerprint();
        model.context_dim = (CTX_DIM as u32) + 1;
        let bytes = model.to_bytes().unwrap();

        let err = BanditRuntime::from_bytes(&bytes, &spec).unwrap_err();
        assert!(
            matches!(err, BanditError::InvalidContextDim { .. }),
            "expected InvalidContextDim"
        );
    }

    #[test]
    fn test_context_dim_mismatch_rejected_at_load() {
        let spec = ContextSpec::base();
        let mut model = ModelFile::from_bytes(&make_linucb_bytes(&spec)).unwrap();
        model.context_spec_hash = spec.hash_fingerprint();
        model.context_dim = spec.dim + 1;
        let bytes = model.to_bytes().unwrap();

        let err = BanditRuntime::from_bytes(&bytes, &spec).unwrap_err();
        assert!(
            matches!(err, BanditError::InvalidContextDim { .. }),
            "expected InvalidContextDim"
        );
    }

    #[test]
    fn test_select_returns_valid_arm() {
        let spec = ContextSpec::base();
        let bytes = make_linucb_bytes(&spec);
        let mut runtime = BanditRuntime::from_bytes(&bytes, &spec).unwrap();
        let ctx = [0.0f32; CTX_DIM];
        let arm = runtime.select(&ctx);
        assert!(arm < 3);
    }
}
