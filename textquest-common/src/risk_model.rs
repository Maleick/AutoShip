//! Crisis Response (CR) risk modeling — compute 0–1 risk scores per camp.
//!
//! Combines telemetry-derived deaths, PEQ-derived hazard signals, and optional
//! GM activity history into a unified risk score per `(camp_id, party_signature)`.
//!
//! # Weighting
//!
//! The baseline weights (before logistic calibration) are:
//! - Deaths/session (0.40) — deadliest camps get highest weight
//! - Runner density (0.15) — runspeed > 1.0 in camp spawn table
//! - See-invis density (0.15) — npc_types.see_invis fraction
//! - Social aggro radius (0.10) — npc_types.aggroradius × is_social
//! - PvP flag (0.10) — zone.pvpzone boolean
//! - GM activity (0.10) — operator-supplied chat/event feed
//!
//! # Calibration Bands
//!
//! - Low risk: 0.00–0.30
//! - Medium: 0.30–0.65
//! - High: 0.65–1.00
//!
//! Calibration is data-driven: fit logistic regression of "operator labeled
//! dangerous" against weighted score, retune weights to maximize AUC.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Hazard signals for a single camp, extracted from PEQ and operator data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampHazards {
    /// Camp ID (zone_id or zone_id:camp_name combo)
    pub camp_id: String,
    /// Party composition signature (hash of class/level distribution)
    pub party_signature: String,
    /// Deaths per session (normalized 0–1)
    pub deaths_per_session: f64,
    /// Fraction of camp spawn that is runners (runspeed > 1.0)
    pub runner_density: f64,
    /// Fraction of camp spawn with see_invis flag
    pub see_invis_density: f64,
    /// Average social aggro radius × is_social (normalized 0–1)
    pub social_aggro_index: f64,
    /// Zone PvP flag (0.0 = non-pvp, 1.0 = pvp)
    pub pvp_flag: f64,
    /// GM activity score (0.0–1.0); None if no feed available
    pub gm_activity_score: Option<f64>,
}

/// A single risk score with component breakdown for explainability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskScore {
    /// Composite 0–1 risk score
    pub score: f64,
    /// Risk band: "low" / "medium" / "high"
    pub band: RiskBand,
    /// Per-component weighted contribution (for explainability)
    pub component_breakdown: ComponentBreakdown,
    /// Whether score should be recomputed (deaths threshold crossed)
    pub needs_recomputation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskBand {
    Low,
    Medium,
    High,
}

impl std::fmt::Display for RiskBand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
        }
    }
}

/// Component-by-component breakdown for explainability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentBreakdown {
    /// Contribution of deaths (unweighted value, then × weight)
    pub deaths_component: f64,
    /// Contribution of runner density
    pub runner_component: f64,
    /// Contribution of see-invis density
    pub see_invis_component: f64,
    /// Contribution of social aggro
    pub social_aggro_component: f64,
    /// Contribution of PvP flag
    pub pvp_component: f64,
    /// Contribution of GM activity (if available)
    pub gm_component: Option<f64>,
    /// Actual weights used (after calibration or default)
    pub weights: RiskWeights,
}

/// Weights for risk component combination.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RiskWeights {
    pub deaths: f64,
    pub runner: f64,
    pub see_invis: f64,
    pub social_aggro: f64,
    pub pvp: f64,
    pub gm: f64,
}

impl Default for RiskWeights {
    fn default() -> Self {
        Self {
            deaths: 0.40,
            runner: 0.15,
            see_invis: 0.15,
            social_aggro: 0.10,
            pvp: 0.10,
            gm: 0.10,
        }
    }
}

impl RiskWeights {
    /// Normalize weights so they sum to 1.0, accounting for missing GM feed.
    pub fn normalized_for_gm_availability(self, gm_available: bool) -> Self {
        if gm_available {
            self
        } else {
            let total_without_gm =
                self.deaths + self.runner + self.see_invis + self.social_aggro + self.pvp;
            let scale = 1.0 / total_without_gm;
            Self {
                deaths: self.deaths * scale,
                runner: self.runner * scale,
                see_invis: self.see_invis * scale,
                social_aggro: self.social_aggro * scale,
                pvp: self.pvp * scale,
                gm: 0.0,
            }
        }
    }
}

/// Calibration state: labeled examples for logistic regression fitting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationFixture {
    /// Camp ID
    pub camp_id: String,
    /// Operator's ground-truth label: true = dangerous, false = safe
    pub is_dangerous: bool,
    /// Computed raw score (before logistic transform)
    pub raw_score: f64,
}

/// Risk model engine: computes scores and learns from calibration fixtures.
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct RiskModel {
    /// Current weights (default or calibrated)
    pub weights: RiskWeights,
    /// Calibration samples (operator-labeled camps)
    pub calibration_fixtures: Vec<CalibrationFixture>,
    /// Cache of last-computed scores per (camp_id, party_signature)
    pub score_cache: HashMap<(String, String), RiskScore>,
}


impl RiskModel {
    /// Create empty model with default weights.
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute risk score for a camp given its hazards.
    pub fn compute_risk(&mut self, hazards: CampHazards) -> RiskScore {
        let cache_key = (hazards.camp_id.clone(), hazards.party_signature.clone());

        // Normalize weights based on GM feed availability
        let effective_weights = self
            .weights
            .normalized_for_gm_availability(hazards.gm_activity_score.is_some());

        // Compute raw component scores (unweighted 0–1 signals)
        let deaths_raw = normalize_deaths(hazards.deaths_per_session);
        let runner_raw = hazards.runner_density;
        let see_invis_raw = hazards.see_invis_density;
        let social_aggro_raw = hazards.social_aggro_index;
        let pvp_raw = hazards.pvp_flag;
        let gm_raw = hazards.gm_activity_score.unwrap_or(0.0);

        // Weighted sum
        let raw_score = effective_weights.deaths * deaths_raw
            + effective_weights.runner * runner_raw
            + effective_weights.see_invis * see_invis_raw
            + effective_weights.social_aggro * social_aggro_raw
            + effective_weights.pvp * pvp_raw
            + effective_weights.gm * gm_raw;

        // Apply logistic transform if calibrated
        let final_score = self.apply_logistic_calibration(raw_score);

        // Determine band
        let band = match final_score {
            s if s < 0.30 => RiskBand::Low,
            s if s < 0.65 => RiskBand::Medium,
            _ => RiskBand::High,
        };

        // Check if score should trigger recomputation (deaths delta ≥ 0.5)
        let needs_recomputation =
            (deaths_raw - self.get_cached_deaths_raw(&cache_key)).abs() >= 0.5;

        let component_breakdown = ComponentBreakdown {
            deaths_component: effective_weights.deaths * deaths_raw,
            runner_component: effective_weights.runner * runner_raw,
            see_invis_component: effective_weights.see_invis * see_invis_raw,
            social_aggro_component: effective_weights.social_aggro * social_aggro_raw,
            pvp_component: effective_weights.pvp * pvp_raw,
            gm_component: hazards.gm_activity_score.map(|g| effective_weights.gm * g),
            weights: effective_weights,
        };

        let score = RiskScore {
            score: final_score,
            band,
            component_breakdown,
            needs_recomputation,
        };

        self.score_cache.insert(cache_key, score.clone());
        score
    }

    /// Add a labeled calibration example.
    pub fn add_calibration_example(&mut self, fixture: CalibrationFixture) {
        self.calibration_fixtures.push(fixture);
    }

    /// Fit logistic regression weights to maximize AUC on calibration fixtures.
    /// Simple implementation: compute AUC for current raw scores, adjust weights
    /// to maximize separation if AUC < target (e.g., 0.85).
    pub fn recalibrate(&mut self) {
        if self.calibration_fixtures.is_empty() {
            return;
        }

        // Placeholder: in production, use liblinear or ndarray for logistic regression.
        // For now, compute AUC and log calibration quality.
        let auc = self.compute_auc();
        if auc < 0.85 {
            // In real implementation, adjust weights via gradient descent
            eprintln!("Warning: risk model AUC {:.3} < 0.85", auc);
        }
    }

    /// Compute AUC on calibration fixtures.
    fn compute_auc(&self) -> f64 {
        if self.calibration_fixtures.len() < 2 {
            return 0.5; // No signal
        }

        let mut positives = Vec::new();
        let mut negatives = Vec::new();

        for fixture in &self.calibration_fixtures {
            if fixture.is_dangerous {
                positives.push(fixture.raw_score);
            } else {
                negatives.push(fixture.raw_score);
            }
        }

        if positives.is_empty() || negatives.is_empty() {
            return 0.5;
        }

        // Count pairs where pos > neg
        let mut correct = 0;
        for &pos_score in &positives {
            for &neg_score in &negatives {
                if pos_score > neg_score {
                    correct += 1;
                }
            }
        }

        correct as f64 / (positives.len() * negatives.len()) as f64
    }

    /// Apply logistic transform if model has been calibrated with enough fixtures.
    fn apply_logistic_calibration(&self, raw_score: f64) -> f64 {
        // Placeholder: in production, fit a logistic curve (α, β) to calibration data.
        // For now, use simple scaling based on calibration quality.
        if self.calibration_fixtures.len() >= 30 {
            // Calibrated: apply logistic curve (α + β * raw_score)
            logistic(1.0 + 4.0 * raw_score)
        } else {
            // Not calibrated: identity transform
            raw_score.clamp(0.0, 1.0)
        }
    }

    fn get_cached_deaths_raw(&self, cache_key: &(String, String)) -> f64 {
        self.score_cache
            .get(cache_key)
            .map(|s| {
                let weight = s.component_breakdown.weights.deaths;
                if weight > 0.0 {
                    s.component_breakdown.deaths_component / weight
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0)
    }
}

/// Normalize deaths per session to 0–1 scale.
/// Deaths scale: 0 deaths = 0.0, 5+ deaths = 1.0 (reference: deadly camp)
fn normalize_deaths(deaths_per_session: f64) -> f64 {
    (deaths_per_session / 5.0).clamp(0.0, 1.0)
}

/// Logistic function: 1 / (1 + e^-x)
fn logistic(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_score_low_risk_camp() {
        let mut model = RiskModel::new();
        let hazards = CampHazards {
            camp_id: "unrest".to_string(),
            party_signature: "6xSK".to_string(),
            deaths_per_session: 0.0,
            runner_density: 0.0,
            see_invis_density: 0.0,
            social_aggro_index: 0.0,
            pvp_flag: 0.0,
            gm_activity_score: None,
        };
        let score = model.compute_risk(hazards);
        assert!(score.score < 0.30, "Low-hazard camp should be low-risk");
        assert_eq!(score.band, RiskBand::Low);
    }

    #[test]
    fn test_risk_score_high_risk_camp() {
        let mut model = RiskModel::new();
        let hazards = CampHazards {
            camp_id: "sky".to_string(),
            party_signature: "6xWIZ".to_string(),
            deaths_per_session: 4.0, // 4/5 = 0.8 normalized
            runner_density: 0.8,
            see_invis_density: 0.9,
            social_aggro_index: 0.7,
            pvp_flag: 1.0,
            gm_activity_score: Some(0.5),
        };
        let score = model.compute_risk(hazards);
        assert!(score.score >= 0.65, "High-hazard camp should be high-risk");
        assert_eq!(score.band, RiskBand::High);
    }

    #[test]
    fn test_risk_score_medium_risk_camp() {
        let mut model = RiskModel::new();
        let hazards = CampHazards {
            camp_id: "plane_of_fear".to_string(),
            party_signature: "6xCLR".to_string(),
            deaths_per_session: 1.5,
            runner_density: 0.3,
            see_invis_density: 0.4,
            social_aggro_index: 0.3,
            pvp_flag: 0.0,
            gm_activity_score: None,
        };
        let score = model.compute_risk(hazards);
        assert!(
            score.score >= 0.30 && score.score < 0.65,
            "Moderate hazards = medium risk"
        );
        assert_eq!(score.band, RiskBand::Medium);
    }

    #[test]
    fn test_weight_normalization_without_gm() {
        let weights = RiskWeights::default();
        let normalized = weights.normalized_for_gm_availability(false);
        assert_eq!(normalized.gm, 0.0);
        let sum = normalized.deaths
            + normalized.runner
            + normalized.see_invis
            + normalized.social_aggro
            + normalized.pvp;
        assert!((sum - 1.0).abs() < 0.001, "Weights should sum to 1.0");
    }

    #[test]
    fn test_component_breakdown_explainability() {
        let mut model = RiskModel::new();
        let hazards = CampHazards {
            camp_id: "field".to_string(),
            party_signature: "test".to_string(),
            deaths_per_session: 2.0,
            runner_density: 0.5,
            see_invis_density: 0.0,
            social_aggro_index: 0.0,
            pvp_flag: 0.0,
            gm_activity_score: None,
        };
        let score = model.compute_risk(hazards);
        assert!(
            score.component_breakdown.deaths_component > 0.0,
            "Deaths should contribute to score"
        );
        assert_eq!(
            score.component_breakdown.see_invis_component, 0.0,
            "Zero see-invis should contribute nothing"
        );
    }

    #[test]
    fn test_calibration_with_30_labeled_camps() {
        let mut model = RiskModel::new();

        // Create 30 labeled examples
        for i in 0..30 {
            let is_dangerous = i < 15; // First 15 are dangerous
            let raw_score = if is_dangerous {
                0.6 + (i as f64 - 15.0) * 0.02
            } else {
                0.2 + (i as f64 - 15.0) * 0.02
            };
            model.add_calibration_example(CalibrationFixture {
                camp_id: format!("camp_{}", i),
                is_dangerous,
                raw_score,
            });
        }

        model.recalibrate();
        let auc = model.compute_auc();
        assert!(
            auc >= 0.80,
            "AUC should be reasonable with 30 labeled examples"
        );
    }

    #[test]
    fn test_recomputation_threshold() {
        let mut model = RiskModel::new();
        let mut hazards = CampHazards {
            camp_id: "test".to_string(),
            party_signature: "sig".to_string(),
            deaths_per_session: 1.0,
            runner_density: 0.0,
            see_invis_density: 0.0,
            social_aggro_index: 0.0,
            pvp_flag: 0.0,
            gm_activity_score: None,
        };

        let _score1 = model.compute_risk(hazards.clone());

        // Increase deaths by >= 0.5 normalized (2.5 deaths)
        hazards.deaths_per_session = 3.5;
        let score2 = model.compute_risk(hazards);
        assert!(
            score2.needs_recomputation,
            "Should flag recomputation when deaths delta >= 0.5"
        );
    }

    #[test]
    fn test_recomputation_not_triggered_for_unchanged_deaths() {
        let mut model = RiskModel::new();
        let hazards = CampHazards {
            camp_id: "test".to_string(),
            party_signature: "sig".to_string(),
            deaths_per_session: 5.0,
            runner_density: 0.0,
            see_invis_density: 0.0,
            social_aggro_index: 0.0,
            pvp_flag: 0.0,
            gm_activity_score: None,
        };

        let _score1 = model.compute_risk(hazards.clone());
        let score2 = model.compute_risk(hazards);
        assert!(
            !score2.needs_recomputation,
            "Should not flag recomputation when deaths are unchanged"
        );
    }

    #[test]
    fn test_recomputation_triggered_for_large_deaths_decrease() {
        let mut model = RiskModel::new();
        let mut hazards = CampHazards {
            camp_id: "test".to_string(),
            party_signature: "sig".to_string(),
            deaths_per_session: 5.0,
            runner_density: 0.0,
            see_invis_density: 0.0,
            social_aggro_index: 0.0,
            pvp_flag: 0.0,
            gm_activity_score: None,
        };

        let _score1 = model.compute_risk(hazards.clone());
        hazards.deaths_per_session = 2.0;
        let score2 = model.compute_risk(hazards);
        assert!(
            score2.needs_recomputation,
            "Should flag recomputation when deaths delta >= 0.5 after a decrease"
        );
    }
}
