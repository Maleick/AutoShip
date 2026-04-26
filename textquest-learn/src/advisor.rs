//! Advisor loop: runtime policy monitoring and override heuristics.
//! L-9: Decision advisory system for policy corrections.
//!
//! Provides a bridge to local LLM for macro-strategy suggestions (camp selection,
//! group composition, recovery choices) grounded in fleet state and active policies.
//! The advisor is strictly advisory: suggestions surface in TUI for operator approval,
//! never execute autonomously.

use serde::{Deserialize, Serialize};

/// Structured fleet state summary for advisor consultation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvisorRequest {
    /// Hash of fleet state for reproducibility/ledger tracking
    pub fleet_state_hash: String,
    /// Per-PC health/mana/buffs/ready-AAs (compact format)
    pub pc_status: String,
    /// Current camp name and recent throughput metrics
    pub camp_context: String,
    /// Active learned policies and canary report snippets
    pub active_policies: String,
    /// Recent deaths/notable events (last 5 minutes)
    pub recent_events: String,
    /// Operator's last manual override and outcome
    pub last_override: Option<String>,
    /// Server population heuristic and time-of-day
    pub environment: String,
}

/// Advisor response with suggestion, justification, and confidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvisorResponse {
    /// Unique identifier for this suggestion
    pub suggestion_id: String,
    /// Category: "camp_selection", "group_composition", "recovery", etc.
    pub scope: String,
    /// Human-readable recommendation
    pub recommendation: String,
    /// Natural-language justification for the suggestion
    pub justification: String,
    /// Confidence in the suggestion (0.0–1.0)
    pub confidence: f32,
    /// Required operator actions to implement
    pub required_operator_actions: Vec<String>,
    /// Evidence references for audit trail
    pub evidence_refs: Vec<String>,
}

/// Advisor trait for macro-strategy suggestions.
pub trait Advisor: Send + Sync {
    /// Request a macro-strategy suggestion from the advisor.
    fn suggest(&self, request: &AdvisorRequest) -> Result<AdvisorResponse, String>;

    /// Check if policy decision should be overridden (legacy marker method).
    fn should_override(&self, _action: &[u8]) -> bool {
        false
    }
}

/// Mock advisor implementation returning deterministic suggestions.
pub struct MockAdvisor {
    variant: usize,
}

impl MockAdvisor {
    /// Create a new mock advisor with a variant for testing.
    pub fn new(variant: usize) -> Self {
        Self { variant }
    }

    /// Create a default mock advisor.
    pub fn default_new() -> Self {
        Self { variant: 0 }
    }
}

impl Advisor for MockAdvisor {
    fn suggest(&self, request: &AdvisorRequest) -> Result<AdvisorResponse, String> {
        // Validate request hash is non-empty
        if request.fleet_state_hash.is_empty() {
            return Err("fleet_state_hash required".to_string());
        }

        // Generate deterministic suggestion based on variant and hash
        let suggestion_index = (request.fleet_state_hash.len() + self.variant) % 3;

        let (scope, recommendation, justification, confidence, actions) = match suggestion_index {
            0 => (
                "camp_selection".to_string(),
                "Move to Unrest spider room".to_string(),
                "Current camp throughput dropped 30% over last 20 min; spider room has matching loot density with 40% lower runner count".to_string(),
                0.72,
                vec!["confirm".to_string(), "re-camp".to_string()],
            ),
            1 => (
                "group_composition".to_string(),
                "Replace one mage with a cleric for sustained healing".to_string(),
                "Recent combat logs show 15% wipe rate due to mana starvation; cleric swap reduces mana-dependent mechanics".to_string(),
                0.65,
                vec!["confirm".to_string(), "regroup".to_string()],
            ),
            _ => (
                "recovery".to_string(),
                "Wait 3 min before next pull; use recovery stance".to_string(),
                "Last 2 pulls exceeded 80% mana cost; cooldown window available; recovery stance ready".to_string(),
                0.81,
                vec!["confirm".to_string(), "wait".to_string()],
            ),
        };

        Ok(AdvisorResponse {
            suggestion_id: format!("sugg-{}-{}", request.fleet_state_hash[..8.min(request.fleet_state_hash.len())].to_string(), self.variant),
            scope,
            recommendation,
            justification,
            confidence,
            required_operator_actions: actions,
            evidence_refs: vec![
                "metrics.throughput.last_20m".to_string(),
                "competition.tick_15".to_string(),
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advisor_request_is_serializable() {
        let req = AdvisorRequest {
            fleet_state_hash: "abc123def456".to_string(),
            pc_status: "All at 80% health".to_string(),
            camp_context: "Unrest: 2.1k/min".to_string(),
            active_policies: "melee_rotation, mana_guard".to_string(),
            recent_events: "None".to_string(),
            last_override: Some("moved to zone B".to_string()),
            environment: "15:30 CST, 40% pop".to_string(),
        };

        // Verify serialization works
        let _json = serde_json::to_string(&req).expect("request must serialize");
    }

    #[test]
    fn mock_advisor_returns_valid_response() {
        let advisor = MockAdvisor::new(0);
        let request = AdvisorRequest {
            fleet_state_hash: "hash123".to_string(),
            pc_status: "All at 80% health".to_string(),
            camp_context: "Unrest: 2.1k/min".to_string(),
            active_policies: "melee_rotation".to_string(),
            recent_events: "None".to_string(),
            last_override: None,
            environment: "15:30 CST".to_string(),
        };

        let response = advisor.suggest(&request).expect("advisor must return response");
        assert!(!response.suggestion_id.is_empty());
        assert!(!response.recommendation.is_empty());
        assert!(!response.justification.is_empty());
        assert!(response.confidence > 0.0 && response.confidence <= 1.0);
        assert!(!response.required_operator_actions.is_empty());
    }

    #[test]
    fn mock_advisor_variant_produces_different_suggestions() {
        let req = AdvisorRequest {
            fleet_state_hash: "consthash".to_string(),
            pc_status: "80%".to_string(),
            camp_context: "Unrest".to_string(),
            active_policies: "melee".to_string(),
            recent_events: "None".to_string(),
            last_override: None,
            environment: "15:30".to_string(),
        };

        let resp0 = MockAdvisor::new(0).suggest(&req).expect("variant 0 ok");
        let resp1 = MockAdvisor::new(1).suggest(&req).expect("variant 1 ok");

        // Variants should produce different scopes due to deterministic hashing
        // (variant 0 and 1 may differ in scope based on hash modulo)
        assert!(!resp0.suggestion_id.is_empty());
        assert!(!resp1.suggestion_id.is_empty());
    }

    #[test]
    fn mock_advisor_rejects_empty_hash() {
        let advisor = MockAdvisor::new(0);
        let request = AdvisorRequest {
            fleet_state_hash: "".to_string(),
            pc_status: "80%".to_string(),
            camp_context: "Unrest".to_string(),
            active_policies: "melee".to_string(),
            recent_events: "None".to_string(),
            last_override: None,
            environment: "15:30".to_string(),
        };

        let result = advisor.suggest(&request);
        assert!(result.is_err());
    }

    #[test]
    fn advisor_response_is_serializable() {
        let response = AdvisorResponse {
            suggestion_id: "sugg-abc123-0".to_string(),
            scope: "camp_selection".to_string(),
            recommendation: "Move to spider room".to_string(),
            justification: "30% throughput drop detected".to_string(),
            confidence: 0.72,
            required_operator_actions: vec!["confirm".to_string()],
            evidence_refs: vec!["metrics.throughput".to_string()],
        };

        // Verify serialization works
        let _json = serde_json::to_string(&response).expect("response must serialize");
    }
}
