use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("signature verification failed for artifact {scope}/{version}")]
    InvalidSignature { scope: String, version: String },

    #[error("artifact not found: {scope}/{version}")]
    NotFound { scope: String, version: String },

    #[error("no previous version to roll back to for scope {scope}")]
    NoPreviousVersion { scope: String },

    #[error("registry database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("artifact directory missing: {0}")]
    MissingArtifact(String),
}
