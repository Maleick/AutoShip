use crate::dataset::ContextActionPair;
use crate::training::TrainedModel;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const MIN_ACTION_MATCH_RATE: f32 = 0.80;
const MIN_SAMPLES_PER_CLUSTER: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeldOutMetrics {
    pub action_match_rate: f32,
    pub total_samples: usize,
    pub correct_predictions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageReport {
    pub well_covered_clusters: usize,
    pub underfitted_clusters: Vec<String>,
    pub coverage_ok: bool,
}

pub struct QualityGates;

impl QualityGates {
    pub fn evaluate_heldout(model: &TrainedModel, heldout: &[&ContextActionPair]) -> Result<HeldOutMetrics> {
        let mut correct = 0;

        for pair in heldout {
            if let Ok(pred_action) = model.predict(&pair.context) {
                if pred_action == pair.action {
                    correct += 1;
                }
            }
        }

        let rate = if heldout.is_empty() {
            0.0
        } else {
            correct as f32 / heldout.len() as f32
        };

        Ok(HeldOutMetrics {
            action_match_rate: rate,
            total_samples: heldout.len(),
            correct_predictions: correct,
        })
    }

    pub fn check_action_match(metrics: &HeldOutMetrics) -> Result<()> {
        if metrics.action_match_rate >= MIN_ACTION_MATCH_RATE {
            tracing::info!(
                "Action match rate {:.2}% >= {:.2}% threshold",
                metrics.action_match_rate * 100.0,
                MIN_ACTION_MATCH_RATE * 100.0
            );
            Ok(())
        } else {
            Err(crate::BehaviorCloningError::QualityGateFailed(format!(
                "Action match rate {:.2}% < {:.2}% minimum",
                metrics.action_match_rate * 100.0,
                MIN_ACTION_MATCH_RATE * 100.0
            )))
        }
    }

    pub fn check_coverage(
        training_pairs: &[&ContextActionPair],
        heldout_pairs: &[&ContextActionPair],
    ) -> Result<CoverageReport> {
        // Cluster contexts by quantizing to identify distinct decision points
        let train_clusters = Self::cluster_contexts(training_pairs);
        let heldout_clusters = Self::cluster_contexts(heldout_pairs);

        let mut underfitted = Vec::new();
        let mut well_covered = 0;

        for cluster_id in heldout_clusters.keys() {
            let train_count = train_clusters.get(cluster_id).cloned().unwrap_or(0);

            if train_count < MIN_SAMPLES_PER_CLUSTER {
                underfitted.push(format!("cluster_{}", cluster_id));
            } else {
                well_covered += 1;
            }
        }

        let coverage_ok = underfitted.is_empty();

        Ok(CoverageReport {
            well_covered_clusters: well_covered,
            underfitted_clusters: underfitted.clone(),
            coverage_ok,
        })
    }

    fn cluster_contexts(pairs: &[&ContextActionPair]) -> HashMap<String, usize> {
        let mut clusters: HashMap<String, usize> = HashMap::new();

        for pair in pairs {
            // Simple clustering: quantize context to bins
            let cluster_key = Self::quantize_context(&pair.context);
            *clusters.entry(cluster_key).or_insert(0) += 1;
        }

        clusters
    }

    fn quantize_context(context: &[f32]) -> String {
        // Quantize to 10 bins per dimension for clustering
        let quantized: Vec<i32> = context.iter().map(|&v| (v * 10.0) as i32).collect();
        format!("{:?}", quantized)
    }
}
