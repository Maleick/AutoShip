pub mod bandit;
pub mod bandits;
pub mod evaluator;
pub mod policy;
pub mod py_trainer;
pub mod reward;

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
