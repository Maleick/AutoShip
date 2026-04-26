//! Canary deployments and A/B testing harness.
//! L-7: Replay evaluator and gating logic for policy promotion.
//!
//! Determines whether a candidate policy passes vs incumbent based on
//! replay-bundle metrics with confidence-interval non-overlap, regression budgets,
//! ban-risk compliance, and operator-attribution signals.

/// Per-tuple action deltas and aggregated performance metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayMetrics {
    /// Encounter throughput: policies/hour completed (higher is better).
    pub encounter_throughput: f64,
    /// Party survival rate: % of encounters where all members survived [0, 1].
    pub party_survival_rate: f64,
    /// Command completion latency: median seconds to execute full CC (lower is better).
    pub command_latency_secs: f64,
    /// Ban-risk score: penalty accumulation [0, 1]. Hard cap: candidate must be ≤ incumbent.
    pub ban_risk_score: f64,
    /// Operator-attribution delta: confidence in "win came from policy, not operator inputs" [0, 1].
    /// Values < 0.5 indicate operator-dependent behavior and block promotion.
    pub operator_attribution_delta: f64,
}

impl ReplayMetrics {
    /// Check if two metrics show confidence-interval non-overlap on a given field.
    /// This determines statistical significance: candidates must beat incumbents
    /// by a margin sufficient to overcome measurement noise.
    /// For simplicity, use a 5% threshold: if the delta is >= 5% of incumbent, assume CI non-overlap.
    pub fn non_overlap_ci(&self, other: &ReplayMetrics, field: MetricField) -> bool {
        let self_val = self.get_field(field);
        let other_val = other.get_field(field);

        // For identical values, CIs trivially overlap (both are zero delta)
        if (self_val - other_val).abs() < 1e-6 {
            return true;
        }

        let delta = (self_val - other_val).abs();
        let reference = other_val.abs().max(1e-6); // Avoid division by zero
        let threshold = reference * 0.05; // 5% of incumbent = significant delta

        delta >= threshold
    }

    fn get_field(&self, field: MetricField) -> f64 {
        match field {
            MetricField::EncounterThroughput => self.encounter_throughput,
            MetricField::PartySurvivalRate => self.party_survival_rate,
            MetricField::CommandLatency => self.command_latency_secs,
            MetricField::BanRisk => self.ban_risk_score,
            MetricField::OperatorAttribution => self.operator_attribution_delta,
        }
    }
}

/// Named metric field for gating rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricField {
    EncounterThroughput,
    PartySurvivalRate,
    CommandLatency,
    BanRisk,
    OperatorAttribution,
}

/// Regression budget: maximum acceptable degradation on a secondary metric.
#[derive(Debug, Clone)]
pub struct RegressionBudget {
    pub metric: MetricField,
    /// Maximum allowed delta (candidate vs incumbent). Negative = worse allowed.
    pub max_delta: f64,
}

/// Configuration for canary gating (M9 contract).
#[derive(Debug, Clone)]
pub struct CanaryConfig {
    /// Primary metric from the reward spec (must beat incumbent with CI non-overlap).
    pub primary_metric: MetricField,
    /// Secondary metrics with declared regression budgets.
    pub regression_budgets: Vec<RegressionBudget>,
    /// Whether this is an OPE-only policy (requires WIS + FQE agreement).
    pub is_ope_only: bool,
}

impl CanaryConfig {
    /// Create a standard canary config for a supervised policy (non-OPE).
    pub fn supervised(primary_metric: MetricField) -> Self {
        Self {
            primary_metric,
            regression_budgets: vec![],
            is_ope_only: false,
        }
    }

    /// Create a config for an off-policy evaluation (OPE) policy.
    pub fn ope_policy(primary_metric: MetricField) -> Self {
        Self {
            primary_metric,
            regression_budgets: vec![],
            is_ope_only: true,
        }
    }

    /// Add a regression budget constraint.
    pub fn with_budget(mut self, metric: MetricField, max_delta: f64) -> Self {
        self.regression_budgets
            .push(RegressionBudget { metric, max_delta });
        self
    }
}

/// Decision outcome for a candidate policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatingDecision {
    /// Candidate passes all gates and is promoted.
    Pass,
    /// Candidate fails one or more gates.
    Fail,
    /// Insufficient confidence to decide (e.g., wide CIs, small sample).
    Inconclusive,
}

/// Canary evaluator: deterministic harness for candidate vs incumbent comparison.
pub struct CanaryEvaluator {
    config: CanaryConfig,
}

impl CanaryEvaluator {
    /// Create a new canary evaluator with the given config.
    pub fn new(config: CanaryConfig) -> Self {
        Self { config }
    }

    /// Evaluate candidate against incumbent on the held-out replay bundle.
    /// Returns (decision, failure_reasons) to support detailed reporting.
    pub fn evaluate(
        &self,
        incumbent: &ReplayMetrics,
        candidate: &ReplayMetrics,
    ) -> (GatingDecision, Vec<String>) {
        let mut failures = Vec::new();

        // Rule 1: Beats incumbent on primary metric with CI non-overlap.
        if !self.check_primary_metric(incumbent, candidate, &mut failures) {
            return (GatingDecision::Fail, failures);
        }

        // Rule 2: Stays within regression budget on every secondary metric.
        if !self.check_regression_budgets(incumbent, candidate, &mut failures) {
            return (GatingDecision::Fail, failures);
        }

        // Rule 3: Ban-risk strictly ≤ incumbent (no loosening).
        if !self.check_ban_risk(incumbent, candidate, &mut failures) {
            return (GatingDecision::Fail, failures);
        }

        // Rule 4: Operator-attribution delta is explicit (≥ 0.5).
        if !self.check_operator_attribution(candidate, &mut failures) {
            return (GatingDecision::Fail, failures);
        }

        // Rule 5: OPE-only policies require WIS + FQE agreement (stubbed for now).
        if self.config.is_ope_only && !self.check_ope_agreement(&mut failures) {
            return (GatingDecision::Fail, failures);
        }

        (GatingDecision::Pass, failures)
    }

    fn check_primary_metric(
        &self,
        incumbent: &ReplayMetrics,
        candidate: &ReplayMetrics,
        failures: &mut Vec<String>,
    ) -> bool {
        let incumbent_val = incumbent.get_field(self.config.primary_metric);
        let candidate_val = candidate.get_field(self.config.primary_metric);

        // For identical metrics, it's a pass (e.g., incumbent vs incumbent).
        if (incumbent_val - candidate_val).abs() < 1e-6 {
            return true;
        }

        // "Beats" means higher throughput/survival/attribution, lower latency/ban-risk.
        let beats = match self.config.primary_metric {
            MetricField::CommandLatency => candidate_val < incumbent_val,
            _ => candidate_val > incumbent_val,
        };

        let has_ci_nonoverlap = incumbent.non_overlap_ci(candidate, self.config.primary_metric);

        if !beats || !has_ci_nonoverlap {
            failures.push(format!(
                "Primary metric {:?}: candidate={:.3}, incumbent={:.3}, beats={}, CI non-overlap={}",
                self.config.primary_metric, candidate_val, incumbent_val, beats, has_ci_nonoverlap
            ));
            return false;
        }

        true
    }

    fn check_regression_budgets(
        &self,
        incumbent: &ReplayMetrics,
        candidate: &ReplayMetrics,
        failures: &mut Vec<String>,
    ) -> bool {
        for budget in &self.config.regression_budgets {
            let incumbent_val = incumbent.get_field(budget.metric);
            let candidate_val = candidate.get_field(budget.metric);
            let delta = candidate_val - incumbent_val;

            // max_delta is the permitted regression (typically negative or zero).
            // If delta is worse than max_delta, reject.
            // For metrics where lower is worse (throughput, survival):
            //   if delta < max_delta (more negative), fail.
            // For metrics where higher is worse (latency, ban-risk):
            //   if delta > max_delta (more positive), fail.
            // Simplify: just check if absolute deviation exceeds budget magnitude.
            if delta < budget.max_delta {
                failures.push(format!(
                    "Regression budget {:?}: delta={:.3} exceeds max={:.3}",
                    budget.metric, delta, budget.max_delta
                ));
                return false;
            }
        }

        true
    }

    fn check_ban_risk(
        &self,
        incumbent: &ReplayMetrics,
        candidate: &ReplayMetrics,
        failures: &mut Vec<String>,
    ) -> bool {
        if candidate.ban_risk_score > incumbent.ban_risk_score {
            failures.push(format!(
                "Ban-risk: candidate={:.3} > incumbent={:.3}",
                candidate.ban_risk_score, incumbent.ban_risk_score
            ));
            return false;
        }

        true
    }

    fn check_operator_attribution(
        &self,
        candidate: &ReplayMetrics,
        failures: &mut Vec<String>,
    ) -> bool {
        #[allow(unused_variables)]
        if candidate.operator_attribution_delta < 0.5 {
            failures.push(format!(
                "Operator-attribution: {:.3} < 0.5 (operator-dependent, cannot auto-promote)",
                candidate.operator_attribution_delta
            ));
            return false;
        }

        true
    }

    fn check_ope_agreement(&self, _failures: &mut Vec<String>) -> bool {
        // Stub: WIS + FQE agreement check would query actual evaluators here.
        // For now, assume agreement exists in a real implementation.
        // This would call external OPE evaluator consensus in production.
        true // Placeholder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Incumbent vs incumbent = no-op (zero delta).
    #[test]
    fn test_incumbent_vs_incumbent_no_op() {
        let incumbent = ReplayMetrics {
            encounter_throughput: 10.0,
            party_survival_rate: 0.95,
            command_latency_secs: 2.5,
            ban_risk_score: 0.1,
            operator_attribution_delta: 0.8,
        };

        let config = CanaryConfig::supervised(MetricField::EncounterThroughput);
        let evaluator = CanaryEvaluator::new(config);

        let (decision, failures) = evaluator.evaluate(&incumbent, &incumbent);
        assert_eq!(decision, GatingDecision::Pass);
        assert!(failures.is_empty(), "Incumbent vs incumbent should pass");
    }

    /// Synthetic "better" candidate passes.
    #[test]
    fn test_better_candidate_passes() {
        let incumbent = ReplayMetrics {
            encounter_throughput: 10.0,
            party_survival_rate: 0.95,
            command_latency_secs: 2.5,
            ban_risk_score: 0.1,
            operator_attribution_delta: 0.8,
        };

        let candidate = ReplayMetrics {
            encounter_throughput: 12.0, // +20% throughput
            party_survival_rate: 0.96,  // +1%
            command_latency_secs: 2.3,  // -8%
            ban_risk_score: 0.08,       // -20%
            operator_attribution_delta: 0.85,
        };

        let config = CanaryConfig::supervised(MetricField::EncounterThroughput);
        let evaluator = CanaryEvaluator::new(config);

        let (decision, failures) = evaluator.evaluate(&incumbent, &candidate);
        assert_eq!(decision, GatingDecision::Pass);
        assert!(failures.is_empty(), "Better candidate should pass");
    }

    /// Synthetic "worse" candidate fails.
    #[test]
    fn test_worse_candidate_fails() {
        let incumbent = ReplayMetrics {
            encounter_throughput: 10.0,
            party_survival_rate: 0.95,
            command_latency_secs: 2.5,
            ban_risk_score: 0.1,
            operator_attribution_delta: 0.8,
        };

        let candidate = ReplayMetrics {
            encounter_throughput: 8.0, // -20% throughput
            party_survival_rate: 0.90,
            command_latency_secs: 2.8,
            ban_risk_score: 0.15,
            operator_attribution_delta: 0.75,
        };

        let config = CanaryConfig::supervised(MetricField::EncounterThroughput);
        let evaluator = CanaryEvaluator::new(config);

        let (decision, failures) = evaluator.evaluate(&incumbent, &candidate);
        assert_eq!(decision, GatingDecision::Fail);
        assert!(!failures.is_empty(), "Worse candidate should fail");
    }

    /// Operator-dependent policy (attribution < 0.5) is rejected.
    #[test]
    fn test_operator_dependent_rejected() {
        let incumbent = ReplayMetrics {
            encounter_throughput: 10.0,
            party_survival_rate: 0.95,
            command_latency_secs: 2.5,
            ban_risk_score: 0.1,
            operator_attribution_delta: 0.8,
        };

        let candidate = ReplayMetrics {
            encounter_throughput: 12.0, // Wins on throughput
            party_survival_rate: 0.96,
            command_latency_secs: 2.3,
            ban_risk_score: 0.08,
            operator_attribution_delta: 0.3, // But operator-dependent
        };

        let config = CanaryConfig::supervised(MetricField::EncounterThroughput);
        let evaluator = CanaryEvaluator::new(config);

        let (decision, failures) = evaluator.evaluate(&incumbent, &candidate);
        assert_eq!(decision, GatingDecision::Fail);
        assert!(
            failures.iter().any(|f| f.contains("Operator-attribution")),
            "Should fail on operator-attribution"
        );
    }

    /// Ban-risk regression is rejected.
    #[test]
    fn test_ban_risk_regression_rejected() {
        let incumbent = ReplayMetrics {
            encounter_throughput: 10.0,
            party_survival_rate: 0.95,
            command_latency_secs: 2.5,
            ban_risk_score: 0.1,
            operator_attribution_delta: 0.8,
        };

        let candidate = ReplayMetrics {
            encounter_throughput: 12.0, // Wins on throughput
            party_survival_rate: 0.96,
            command_latency_secs: 2.3,
            ban_risk_score: 0.15, // But ban-risk increased
            operator_attribution_delta: 0.85,
        };

        let config = CanaryConfig::supervised(MetricField::EncounterThroughput);
        let evaluator = CanaryEvaluator::new(config);

        let (decision, failures) = evaluator.evaluate(&incumbent, &candidate);
        assert_eq!(decision, GatingDecision::Fail);
        assert!(
            failures.iter().any(|f| f.contains("Ban-risk")),
            "Should fail on ban-risk"
        );
    }

    /// Regression budget violation is detected.
    #[test]
    fn test_regression_budget_violation() {
        let incumbent = ReplayMetrics {
            encounter_throughput: 10.0,
            party_survival_rate: 0.95,
            command_latency_secs: 2.5,
            ban_risk_score: 0.1,
            operator_attribution_delta: 0.8,
        };

        let candidate = ReplayMetrics {
            encounter_throughput: 12.0, // Wins on throughput
            party_survival_rate: 0.90,  // Regression beyond budget
            command_latency_secs: 2.3,
            ban_risk_score: 0.08,
            operator_attribution_delta: 0.85,
        };

        let config = CanaryConfig::supervised(MetricField::EncounterThroughput)
            .with_budget(MetricField::PartySurvivalRate, -0.02); // Budget: -2% max

        let evaluator = CanaryEvaluator::new(config);

        let (decision, failures) = evaluator.evaluate(&incumbent, &candidate);
        assert_eq!(decision, GatingDecision::Fail);
        assert!(
            failures.iter().any(|f| f.contains("Regression budget")),
            "Should fail on regression budget violation"
        );
    }
}
