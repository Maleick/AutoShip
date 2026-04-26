//! Policy representation, inference, and versioning.

use serde::{Deserialize, Serialize};

/// Marker trait for policy implementations.
pub trait Policy: Send + Sync {
    /// Infer action from state.
    fn infer(&self, _state: &[u8]) -> anyhow::Result<Vec<f32>> {
        Ok(vec![])
    }
}

/// Metadata recorded alongside a behavior-cloned policy artifact.
///
/// Captures the training provenance and held-out evaluation results so that
/// downstream promotion/canary tooling can reason about the policy without
/// re-running training.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyMetadata {
    /// Character class the policy was trained for (e.g. `"cleric"`).
    pub class: String,
    /// `(session_id, (start_flag_idx, end_flag_idx))` pairs identifying the
    /// flagged segments that contributed to training.
    pub training_data_manifest: Vec<(String, (u64, u64))>,
    /// Context schema version used to build the dataset.
    pub context_schema_version: String,
    /// Git SHA of the exporter binary at training time.
    pub exporter_git_sha: String,
    /// Held-out action match rate (0.0–1.0).
    pub held_out_action_match_rate: f32,
    /// Total training context-action pairs.
    pub total_training_samples: usize,
    /// Total held-out context-action pairs.
    pub total_heldout_samples: usize,
    /// Cluster IDs that did not meet the minimum training-sample threshold.
    pub underfitted_context_clusters: Vec<String>,
}

/// Off-policy evaluation report produced by the WIS / FQE evaluators.
///
/// Both estimators report a mean and a 95% confidence interval; promotion
/// requires the candidate's lower CI to dominate the baseline's upper CI on
/// both estimators.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpeReport {
    pub wis_mean: f64,
    pub wis_lower_ci: f64,
    pub wis_upper_ci: f64,
    pub fqe_mean: f64,
    pub fqe_lower_ci: f64,
    pub fqe_upper_ci: f64,
    /// Fraction of dataset actions covered by the candidate policy (0.0–1.0).
    pub action_coverage: f64,
}

/// Manifest describing a trained RL policy artifact (separate from the BC
/// `PolicyMetadata`). Carries the OPE report used by promotion gates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyManifest {
    pub class: String,
    pub algorithm: String,
    pub artifact_path: String,
    pub trained_at: String,
    pub ope_report: OpeReport,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_marker_exists() {
        struct DummyPolicy;
        impl Policy for DummyPolicy {}
    }

    #[test]
    fn policy_metadata_roundtrip() {
        let meta = PolicyMetadata {
            class: "cleric".into(),
            training_data_manifest: vec![("s0".into(), (0, 10))],
            context_schema_version: "v1".into(),
            exporter_git_sha: "abc".into(),
            held_out_action_match_rate: 0.9,
            total_training_samples: 100,
            total_heldout_samples: 25,
            underfitted_context_clusters: vec![],
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: PolicyMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back.class, "cleric");
    }
}
