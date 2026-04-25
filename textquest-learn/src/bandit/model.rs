//! On-disk model file format (bincode v2, versioned).
//!
//! One `ModelFile` per (class, decision-point) scope.  The `context_spec_hash`
//! guards against loading a model trained on a different context schema.

use serde::{Deserialize, Serialize};

use crate::bandit::context::ContextSpec;

/// Bump this when the binary format changes in a breaking way.
pub const MODEL_FILE_VERSION: u32 = 1;

/// Identifies which class and decision point a model covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BanditScope {
    /// EQ class name in lowercase (e.g. "cleric", "wizard").
    pub class: String,
    /// Decision point within the class rotation (e.g. "heal_picker", "nuke_picker").
    pub decision_point: String,
}

impl BanditScope {
    pub fn new(class: impl Into<String>, decision_point: impl Into<String>) -> Self {
        Self {
            class: class.into(),
            decision_point: decision_point.into(),
        }
    }

    /// Canonical string representation used as a map key.
    pub fn as_key(&self) -> String {
        format!("{}.{}", self.class, self.decision_point)
    }
}

/// Which algorithm the model was trained with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlgorithmTag {
    LinUcb,
    Thompson,
}

/// Per-arm data stored in the model file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedArm {
    pub arm_id: usize,
    pub arm_label: String,
    /// A_k^{-1} in row-major f32 (d × d).
    pub a_inv: Vec<f32>,
    /// b_k vector (length d).
    pub b: Vec<f32>,
    /// Pre-computed θ̂ = A_inv @ b (length d).
    pub mu: Vec<f32>,
    /// Cholesky L of v² A_inv (length d×d); empty for LinUCB.
    pub chol_l: Vec<f32>,
    pub n_updates: u64,
}

/// Versioned model file containing all arm parameters for a scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFile {
    /// Format version — must equal [`MODEL_FILE_VERSION`] on load.
    pub version: u32,
    /// SipHash-13 fingerprint of the [`ContextSpec`] used during training.
    /// Loading with a mismatched spec is rejected as a schema-drift error.
    pub context_spec_hash: u64,
    pub scope: BanditScope,
    pub algorithm: AlgorithmTag,
    /// Exploration parameter (alpha for LinUCB, v for Thompson).
    pub exploration_param: f32,
    /// Context dimension used during training.
    pub context_dim: u32,
    pub arms: Vec<SerializedArm>,
}

impl ModelFile {
    /// Write to a byte buffer (bincode v2).
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::error::EncodeError> {
        bincode::serde::encode_to_vec(self, bincode::config::standard())
    }

    /// Read from a byte buffer (bincode v2).
    pub fn from_bytes(data: &[u8]) -> Result<Self, bincode::error::DecodeError> {
        let (model, _) = bincode::serde::decode_from_slice(data, bincode::config::standard())?;
        Ok(model)
    }

    /// Validate version + context spec hash match expectations.
    pub fn validate(&self, expected_spec: &ContextSpec) -> Result<(), ModelValidationError> {
        if self.version != MODEL_FILE_VERSION {
            return Err(ModelValidationError::VersionMismatch {
                found: self.version,
                expected: MODEL_FILE_VERSION,
            });
        }
        let expected_hash = expected_spec.hash_fingerprint();
        if self.context_spec_hash != expected_hash {
            return Err(ModelValidationError::SchemaMismatch {
                model_hash: self.context_spec_hash,
                runtime_hash: expected_hash,
            });
        }
        Ok(())
    }
}

/// Errors raised when loading or validating a model file.
#[derive(Debug, thiserror::Error)]
pub enum ModelValidationError {
    #[error("model version {found} != expected {expected}")]
    VersionMismatch { found: u32, expected: u32 },
    #[error("context spec hash mismatch: model={model_hash:#x} runtime={runtime_hash:#x}")]
    SchemaMismatch { model_hash: u64, runtime_hash: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bandit::context::ContextSpec;
    use crate::bandit::linucb::LinUcbModel;

    fn make_model_file(spec: &ContextSpec) -> ModelFile {
        let d = spec.dim as usize;
        let linucb = LinUcbModel::new(&["a", "b", "c"], d, 1.0);
        ModelFile {
            version: MODEL_FILE_VERSION,
            context_spec_hash: spec.hash_fingerprint(),
            scope: BanditScope::new("cleric", "heal_picker"),
            algorithm: AlgorithmTag::LinUcb,
            exploration_param: 1.0,
            context_dim: spec.dim,
            arms: linucb.arms.iter().map(|a| a.to_serialized()).collect(),
        }
    }

    #[test]
    fn test_roundtrip_bincode() {
        let spec = ContextSpec::base();
        let original = make_model_file(&spec);
        let bytes = original.to_bytes().unwrap();
        let decoded = ModelFile::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.version, MODEL_FILE_VERSION);
        assert_eq!(decoded.scope, original.scope);
        assert_eq!(decoded.arms.len(), 3);
    }

    #[test]
    fn test_schema_drift_rejected() {
        let spec_train = ContextSpec::base();
        let spec_runtime = ContextSpec::with_abilities(&["heal", "cure"]);
        let model = make_model_file(&spec_train);
        let err = model.validate(&spec_runtime).unwrap_err();
        assert!(
            matches!(err, ModelValidationError::SchemaMismatch { .. }),
            "expected SchemaMismatch, got: {err}"
        );
    }

    #[test]
    fn test_valid_spec_accepted() {
        let spec = ContextSpec::base();
        let model = make_model_file(&spec);
        model.validate(&spec).expect("same spec should pass");
    }
}
