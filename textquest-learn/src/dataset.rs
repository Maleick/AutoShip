use crate::Result;
use crate::bookmarks::BookmarkStore;
use crate::ledger::ExperienceLedger;
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
            let start_idx = usize::try_from(bookmark.start_flag_idx).map_err(|_| {
                crate::BehaviorCloningError::Dataset(format!(
                    "Bookmark start index {} does not fit usize",
                    bookmark.start_flag_idx
                ))
            })?;
            let end_idx = usize::try_from(bookmark.end_flag_idx).map_err(|_| {
                crate::BehaviorCloningError::Dataset(format!(
                    "Bookmark end index {} does not fit usize",
                    bookmark.end_flag_idx
                ))
            })?;
            let segment = ledger.get_segment(&bookmark.session_id, start_idx, end_idx);

            for entry in segment {
                if entry.class == class {
                    // Convert compact state blob to model context vector.
                    let context_vec = Self::state_blob_to_context(&entry.state_blob)?;
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
            ))
            .into());
        }

        Ok(FlaggedSegmentDataset {
            class: class.to_string(),
            pairs,
            context_schema_version: context_schema_version.to_string(),
        })
    }

    fn state_blob_to_context(state_blob: &[u8]) -> Result<Vec<f32>> {
        if state_blob.is_empty() {
            return Err(anyhow::anyhow!(crate::BehaviorCloningError::Dataset(
                "State blob is empty".to_string(),
            )));
        }

        Ok(state_blob.iter().map(|&byte| byte as f32).collect())
    }

    pub fn split_train_heldout(
        &self,
        heldout_ratio: f32,
    ) -> (Vec<&ContextActionPair>, Vec<&ContextActionPair>) {
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
