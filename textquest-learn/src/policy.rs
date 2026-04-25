use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
pub struct PolicyManifest {
    pub version: String,
    pub algorithm: String,
    pub class: String,
    pub reward_spec: String,
    pub training_data_hash: String,
    pub context_schema_version: String,
    pub git_sha: String,
    pub ope_report: OpeReport,
    pub bc_warm_start: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OpeReport {
    pub wis_mean: f32,
    pub wis_lower_ci: f32,
    pub wis_upper_ci: f32,
    pub fqe_mean: f32,
    pub fqe_lower_ci: f32,
    pub fqe_upper_ci: f32,
    pub action_coverage: f32,
    pub bc_baseline_wis: f32,
    pub beats_bc: bool,
}

/// Metadata recorded alongside an exported behavior-cloning model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyMetadata {
    /// Character class this model was trained for.
    pub class: String,
    /// Map from session_id → (start_flag_idx, end_flag_idx) for the training corpus.
    pub training_data_manifest: Vec<(String, (u64, u64))>,
    /// Version string of the context schema used when building the dataset.
    pub context_schema_version: String,
    /// Git SHA of the exporter binary at training time.
    pub exporter_git_sha: String,
    /// Action-match rate on the held-out split (0–1).
    pub held_out_action_match_rate: f32,
    /// Total training samples used.
    pub total_training_samples: usize,
    /// Total held-out samples used for evaluation.
    pub total_heldout_samples: usize,
    /// Context cluster names that were under-fitted (below coverage threshold).
    pub underfitted_context_clusters: Vec<String>,
}

/// Policy artifact holder (ONNX inference handled via Python sidecar)
pub struct Policy {
    onnx_path: PathBuf,
    state_dim: usize,
    action_dim: usize,
}

impl Policy {
    pub fn load(onnx_path: &Path, state_dim: usize, action_dim: usize) -> Result<Self> {
        anyhow::ensure!(
            onnx_path.exists(),
            "ONNX model not found: {:?}",
            onnx_path
        );

        Ok(Policy {
            onnx_path: onnx_path.to_path_buf(),
            state_dim,
            action_dim,
        })
    }

    pub fn path(&self) -> &Path {
        &self.onnx_path
    }

    pub fn state_dim(&self) -> usize {
        self.state_dim
    }

    pub fn action_dim(&self) -> usize {
        self.action_dim
    }
}
