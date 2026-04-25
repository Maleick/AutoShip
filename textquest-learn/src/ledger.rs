use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperienceEntry {
    pub session_id: String,
    pub timestamp: u64,
    pub class: String,
    pub context: serde_json::Value, // Raw context (varies by schema version)
    pub action: u32,                 // Discrete action ID
    pub reward: f32,                 // Offline reward signal
}

#[derive(Debug, Clone)]
pub struct ExperienceLedger {
    entries: Vec<ExperienceEntry>,
}

impl ExperienceLedger {
    pub fn load_from_dir(ledger_dir: &Path) -> Result<Self> {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(ledger_dir)
            .map_err(|e| crate::BehaviorCloningError::Io(e))?
        {
            let entry = entry.map_err(|e| crate::BehaviorCloningError::Io(e))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                let data = std::fs::read_to_string(&path)
                    .map_err(|e| crate::BehaviorCloningError::Io(e))?;
                for line in data.lines() {
                    if !line.trim().is_empty() {
                        let entry: ExperienceEntry = serde_json::from_str(line)
                            .map_err(|e| {
                                crate::BehaviorCloningError::Dataset(format!(
                                    "Failed to parse experience entry: {}",
                                    e
                                ))
                            })?;
                        entries.push(entry);
                    }
                }
            }
        }
        entries.sort_by_key(|e| e.timestamp);
        Ok(ExperienceLedger { entries })
    }

    pub fn get_entries_for_session(&self, session_id: &str) -> Vec<&ExperienceEntry> {
        self.entries
            .iter()
            .filter(|e| e.session_id == session_id)
            .collect()
    }

    pub fn get_entries_for_class(&self, class: &str) -> Vec<&ExperienceEntry> {
        self.entries.iter().filter(|e| e.class == class).collect()
    }

    pub fn get_segment(
        &self,
        session_id: &str,
        start_idx: u64,
        end_idx: u64,
    ) -> Vec<&ExperienceEntry> {
        self.get_entries_for_session(session_id)
            .into_iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let i_u64 = i as u64;
                if i_u64 >= start_idx && i_u64 < end_idx {
                    Some(e)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn all_entries(&self) -> &[ExperienceEntry] {
        &self.entries
    }
}
