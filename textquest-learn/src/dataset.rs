use crate::bookmarks::BookmarkStore;
use crate::ledger::ExperienceLedger;
use crate::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextActionPair {
    pub context: Vec<f32>, // Flattened context vector (8-256 dims depending on schema)
    pub action: u32,
}

#[derive(Debug)]
pub struct FlaggedSegmentDataset {
    pub class: String,
    pub pairs: Vec<ContextActionPair>,
    pub context_schema_version: String,
}

impl FlaggedSegmentDataset {
    pub fn build(
        ledger: &ExperienceLedger,
        bookmarks: &BookmarkStore,
        class: &str,
        context_schema_version: &str,
    ) -> Result<Self> {
        let mut pairs = Vec::new();

        for bookmark in bookmarks.filter_by_label("good") {
            let segment = ledger.get_segment(&bookmark.session_id, bookmark.start_flag_idx, bookmark.end_flag_idx);

            for entry in segment {
                if entry.class == class {
                    // Flatten context JSON to vector
                    let context_vec = Self::flatten_context(&entry.context)?;
                    pairs.push(ContextActionPair {
                        context: context_vec,
                        action: entry.action,
                    });
                }
            }
        }

        if pairs.is_empty() {
            return Err(crate::BehaviorCloningError::Dataset(format!(
                "No flagged segments found for class '{}'",
                class
            )));
        }

        Ok(FlaggedSegmentDataset {
            class: class.to_string(),
            pairs,
            context_schema_version: context_schema_version.to_string(),
        })
    }

    fn flatten_context(context: &serde_json::Value) -> Result<Vec<f32>> {
        match context {
            serde_json::Value::Array(arr) => {
                arr.iter()
                    .map(|v| {
                        v.as_f64()
                            .ok_or_else(|| {
                                crate::BehaviorCloningError::Dataset(
                                    "Context element is not a number".to_string(),
                                )
                            })
                            .map(|f| f as f32)
                    })
                    .collect()
            }
            serde_json::Value::Object(obj) => {
                let mut vec = Vec::new();
                for (_key, val) in obj.iter() {
                    if let Some(f) = val.as_f64() {
                        vec.push(f as f32);
                    }
                }
                if vec.is_empty() {
                    Err(crate::BehaviorCloningError::Dataset(
                        "Failed to extract numeric values from context".to_string(),
                    ))
                } else {
                    Ok(vec)
                }
            }
            _ => Err(crate::BehaviorCloningError::Dataset(
                "Context must be array or object".to_string(),
            )),
        }
    }

    pub fn split_train_heldout(&self, heldout_ratio: f32) -> (Vec<&ContextActionPair>, Vec<&ContextActionPair>) {
        let heldout_count = (self.pairs.len() as f32 * heldout_ratio) as usize;
        let train_count = self.pairs.len() - heldout_count;

        let train: Vec<_> = self.pairs.iter().take(train_count).collect();
        let heldout: Vec<_> = self.pairs.iter().skip(train_count).collect();

        (train, heldout)
    }

    pub fn context_dim(&self) -> usize {
        self.pairs.first().map(|p| p.context.len()).unwrap_or(0)
    }

    pub fn num_actions(&self) -> u32 {
        self.pairs.iter().map(|p| p.action).max().unwrap_or(0) + 1
    }
}
