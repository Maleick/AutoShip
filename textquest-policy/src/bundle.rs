use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Metadata stored alongside a policy artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyManifest {
    /// Policy scope, e.g. "cleric.heal_picker", "warrior.rotation".
    pub scope: String,
    /// Version tag, e.g. "bandits.v3", "rl.v2".
    pub version: String,
    /// Human-readable source bundle identifier (e.g. canary run ID).
    pub source_bundle: String,
    /// SHA-256 hex digest of `policy.bin` for integrity verification.
    pub sha256: String,
    /// ISO-8601 timestamp when this artifact was produced by L-7 canary.
    pub produced_at: String,
}

/// Loaded, verified policy bundle held in the hot-swap slot.
#[derive(Debug, Clone)]
pub struct PolicyBundle {
    pub scope: String,
    pub version: String,
    pub source_bundle: String,
    /// Raw policy parameters / model weights as opaque bytes.
    pub data: Vec<u8>,
    pub promoted_at: SystemTime,
}

impl PolicyBundle {
    /// A sentinel bundle representing "rule-based strategy active — no learned policy".
    pub fn rule_based(scope: impl Into<String>) -> Self {
        Self {
            scope: scope.into(),
            version: "rule-based".to_string(),
            source_bundle: "built-in".to_string(),
            data: Vec::new(),
            promoted_at: SystemTime::UNIX_EPOCH,
        }
    }

    pub fn is_rule_based(&self) -> bool {
        self.version == "rule-based"
    }
}
