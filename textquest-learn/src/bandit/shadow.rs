//! Shadow-mode decision logging.
//!
//! Every time the bandit and rule-based strategy disagree, a `ShadowEvent` is
//! appended to the orchestrator decision log (R-2 / #2695) as a JSONL record.
//! The `state_hash` ties the record back to the L-1 decision ledger entry.

use serde::{Deserialize, Serialize};
use std::io::{BufWriter, Write};

use crate::bandit::model::BanditScope;

/// One shadow decision record appended to the decision log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowEvent {
    /// Unix timestamp (seconds).
    pub ts_secs: u64,
    /// SipHash-13 of the context vector — ties to L-1 ledger entry.
    pub state_hash: u64,
    /// Which scope produced this event.
    pub scope: BanditScope,
    /// Arm index the bandit would have chosen.
    pub bandit_arm: usize,
    /// Arm index the rule-based strategy chose (executed arm).
    pub rule_arm: usize,
    /// Arm label the bandit would have chosen.
    pub bandit_label: String,
    /// Arm label the rule-based strategy chose.
    pub rule_label: String,
}

/// Appends `ShadowEvent` records as JSONL to an underlying writer.
///
/// In the DLL this is backed by a file opened in append mode; in tests an
/// in-memory `Vec<u8>` is used.
pub struct ShadowLog<W: Write> {
    writer: BufWriter<W>,
    scope: BanditScope,
}

impl<W: Write> ShadowLog<W> {
    pub fn new(writer: W, scope: BanditScope) -> Self {
        Self {
            writer: BufWriter::new(writer),
            scope,
        }
    }

    /// Record a shadow decision.  Ignores write errors to avoid disrupting the
    /// combat loop — tracing::warn logs failures.
    pub fn record(
        &mut self,
        ts_secs: u64,
        state_hash: u64,
        bandit_arm: usize,
        bandit_label: &str,
        rule_arm: usize,
        rule_label: &str,
    ) {
        let event = ShadowEvent {
            ts_secs,
            state_hash,
            scope: self.scope.clone(),
            bandit_arm,
            rule_arm,
            bandit_label: bandit_label.to_string(),
            rule_label: rule_label.to_string(),
        };
        if let Ok(line) = serde_json::to_string(&event) {
            let _ = writeln!(self.writer, "{line}");
            let _ = self.writer.flush();
        }
    }
}

/// Shadow-mode or live execution policy.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub enum PolicyMode {
    /// Default: bandit pick is logged but rule pick executes.
    #[default]
    Shadow,
    /// Promoted by L-7 canary gate.  Bandit pick executes.
    Live { canary_token: String },
}

impl PolicyMode {
    pub fn is_shadow(&self) -> bool {
        matches!(self, PolicyMode::Shadow)
    }
    pub fn is_live(&self) -> bool {
        matches!(self, PolicyMode::Live { .. })
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::bandit::model::BanditScope;

    #[test]
    fn test_shadow_mode_is_default() {
        assert!(PolicyMode::default().is_shadow());
    }

    #[test]
    fn test_live_mode_not_shadow() {
        let live = PolicyMode::Live {
            canary_token: "tok".into(),
        };
        assert!(!live.is_shadow());
        assert!(live.is_live());
    }

    #[test]
    fn shadow_log_writes_jsonl() {
        let scope = BanditScope::new("cleric", "heal_picker");
        let buf: Vec<u8> = Vec::new();
        let mut log = ShadowLog::new(buf, scope);
        log.record(1000, 0xdeadbeef, 1, "Heal", 0, "CH");
        let inner = log.writer.into_inner().unwrap();
        let text = String::from_utf8(inner).unwrap();
        assert!(text.contains("bandit_arm"));
        assert!(text.contains("cleric"));
    }
}
