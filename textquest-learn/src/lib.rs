pub mod bandit;
pub mod evaluator;
pub mod policy;
pub mod py_trainer;

#[cfg(feature = "full")]
pub mod bookmarks;
#[cfg(feature = "full")]
pub mod dataset;
#[cfg(feature = "full")]
pub mod ledger;
#[cfg(feature = "full")]
pub mod onnx_export;
#[cfg(feature = "full")]
pub mod quality_gates;
#[cfg(feature = "full")]
pub mod training;

pub use bandit::runtime::{BanditError, BanditRuntime};
pub use bandit::shadow::PolicyMode;
#[cfg(feature = "full")]
pub use dataset::FlaggedSegmentDataset;
pub use policy::PolicyMetadata;

pub type Result<T> = anyhow::Result<T>;

/// Errors from behavior-cloning training, dataset build, or export steps.
#[derive(Debug, thiserror::Error)]
pub enum BehaviorCloningError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("dataset error: {0}")]
    Dataset(String),
    #[error("training error: {0}")]
    Training(String),
    #[error("onnx export error: {0}")]
    OnnxExport(String),
    #[error("quality gate failed: {0}")]
    QualityGateFailed(String),
}

use policy::PolicyManifest;
use std::path::Path;

/// Promote a trained policy if it beats the BC baseline on both WIS and FQE
pub fn promote_if_better(
    manifest: &PolicyManifest,
    bc_baseline_manifest: &PolicyManifest,
) -> Result<bool> {
    // Both evaluators must show improvement
    let wis_improves = manifest.ope_report.wis_mean > bc_baseline_manifest.ope_report.wis_mean
        && manifest.ope_report.wis_lower_ci > bc_baseline_manifest.ope_report.wis_upper_ci;

    let fqe_improves = manifest.ope_report.fqe_mean > bc_baseline_manifest.ope_report.fqe_mean
        && manifest.ope_report.fqe_lower_ci > bc_baseline_manifest.ope_report.fqe_upper_ci;

    let coverage_ok = manifest.ope_report.action_coverage > 0.8;

    Ok(wis_improves && fqe_improves && coverage_ok)
}

/// Save policy manifest as JSON
pub fn save_manifest(manifest: &PolicyManifest, path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(manifest)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load policy manifest from JSON
pub fn load_manifest(path: &Path) -> Result<PolicyManifest> {
    let content = std::fs::read_to_string(path)?;
    let manifest = serde_json::from_str(&content)?;
    Ok(manifest)
}
