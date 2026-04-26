use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

// Schema version for replay bundle compatibility tracking
const SCHEMA_VERSION: &str = "1.0";

/// Single (state, action, reward, next_state) tuple from replay bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperienceEntry {
    // Identifiers
    pub session_id: String,
    pub timestamp: u64, // ms, session-relative
    pub pc_id: String,

    // Classification/sharding
    pub class: String,
    pub camp_id: String,

    // Policy attribution
    pub policy_version: String, // commit SHA of orchestrator

    // State and action
    pub state_blob: Vec<u8>, // compact binary encoding of PC + engaged-NPC vectors
    pub action: u32,         // discrete action ID
    pub action_id: String,   // canonical action label ("cast:CH", "med", "pull:<npc>", etc.)

    // Reward signal
    pub reward: f32, // offline reward; from L-2 spec

    // Transition
    pub next_state_blob: Vec<u8>, // state at t + Δ

    // Episode boundary
    pub terminal: bool, // death, zone change, camp break
}

/// Append-only writer for experience entries (replay bundle → ledger)
pub struct ExperienceLedgerWriter {
    path: std::path::PathBuf,
}

impl ExperienceLedgerWriter {
    /// Create a new append-only ledger writer
    pub fn new(path: &Path) -> Result<Self> {
        // Ensure parent dir exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(ExperienceLedgerWriter {
            path: path.to_path_buf(),
        })
    }

    /// Append a single entry to ledger (JSONL format)
    pub fn append(&mut self, entry: &ExperienceEntry) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        let line = serde_json::to_string(entry)
            .map_err(|e| anyhow!("Failed to serialize entry: {}", e))?;

        writeln!(file, "{}", line)?;

        Ok(())
    }

    /// Append multiple entries from a replay bundle (idempotent)
    pub fn append_batch(&mut self, entries: &[ExperienceEntry]) -> Result<()> {
        for entry in entries {
            self.append(entry)?;
        }
        Ok(())
    }
}

/// In-memory ledger (loaded from JSONL files)
#[derive(Debug, Clone)]
pub struct ExperienceLedger {
    entries: Vec<ExperienceEntry>,
    schema_version: String,
}

impl ExperienceLedger {
    /// Load all entries from a JSONL ledger file or directory
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let mut entries = Vec::new();

        let paths: Vec<_> = if path.is_dir() {
            std::fs::read_dir(path)?
                .filter_map(|e| {
                    e.ok().and_then(|entry| {
                        let p = entry.path();
                        if p.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                            Some(p)
                        } else {
                            None
                        }
                    })
                })
                .collect()
        } else {
            vec![path.to_path_buf()]
        };

        for fpath in paths {
            let file = std::fs::File::open(&fpath)?;
            let reader = BufReader::new(file);

            for (line_no, line) in reader.lines().enumerate() {
                let line = line?;
                if !line.trim().is_empty() {
                    let entry: ExperienceEntry = serde_json::from_str(&line)
                        .map_err(|e| anyhow!(
                            "Failed to parse entry at {}:{}: {}",
                            fpath.display(),
                            line_no + 1,
                            e
                        ))?;
                    entries.push(entry);
                }
            }
        }

        entries.sort_by_key(|e| (e.session_id.clone(), e.timestamp));

        Ok(ExperienceLedger {
            entries,
            schema_version: SCHEMA_VERSION.to_string(),
        })
    }

    /// Get entries for a specific session
    pub fn get_entries_for_session(&self, session_id: &str) -> Vec<&ExperienceEntry> {
        self.entries
            .iter()
            .filter(|e| e.session_id == session_id)
            .collect()
    }

    /// Get entries for a specific character class
    pub fn get_entries_for_class(&self, class: &str) -> Vec<&ExperienceEntry> {
        self.entries.iter().filter(|e| e.class == class).collect()
    }

    /// Get entries for a specific camp
    pub fn get_entries_for_camp(&self, camp_id: &str) -> Vec<&ExperienceEntry> {
        self.entries.iter().filter(|e| e.camp_id == camp_id).collect()
    }

    /// Get a segment of entries for a session by index range
    pub fn get_segment(
        &self,
        session_id: &str,
        start_idx: usize,
        end_idx: usize,
    ) -> Vec<&ExperienceEntry> {
        self.get_entries_for_session(session_id)
            .into_iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if i >= start_idx && i < end_idx {
                    Some(e)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all entries
    pub fn all_entries(&self) -> &[ExperienceEntry] {
        &self.entries
    }

    /// Get schema version
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    /// Total entry count
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_entry(
        session_id: &str,
        timestamp: u64,
        class: &str,
        action_id: &str,
    ) -> ExperienceEntry {
        ExperienceEntry {
            session_id: session_id.to_string(),
            timestamp,
            pc_id: format!("pc_{}", session_id),
            class: class.to_string(),
            camp_id: "camp_a".to_string(),
            policy_version: "abc123def".to_string(),
            state_blob: vec![1, 2, 3, 4],
            action: 1,
            action_id: action_id.to_string(),
            reward: 10.5,
            next_state_blob: vec![5, 6, 7, 8],
            terminal: false,
        }
    }

    #[test]
    fn test_ledger_writer_append_single() -> Result<()> {
        let tmpdir = TempDir::new().unwrap();
        let ledger_path = tmpdir.path().join("ledger.jsonl");

        let mut writer = ExperienceLedgerWriter::new(&ledger_path)?;
        let entry = create_test_entry("sess_1", 1000, "warrior", "cast:CH");
        writer.append(&entry)?;

        let content = fs::read_to_string(&ledger_path).unwrap();
        assert!(!content.is_empty());
        assert!(content.contains("sess_1"));

        Ok(())
    }

    #[test]
    fn test_ledger_writer_append_batch() -> Result<()> {
        let tmpdir = TempDir::new().unwrap();
        let ledger_path = tmpdir.path().join("ledger.jsonl");

        let mut writer = ExperienceLedgerWriter::new(&ledger_path)?;
        let entries = vec![
            create_test_entry("sess_1", 1000, "warrior", "cast:CH"),
            create_test_entry("sess_1", 2000, "warrior", "med"),
            create_test_entry("sess_1", 3000, "warrior", "pull:npc_1"),
        ];

        writer.append_batch(&entries)?;

        let ledger = ExperienceLedger::load_from_path(&ledger_path)?;
        assert_eq!(ledger.len(), 3);

        Ok(())
    }

    #[test]
    fn test_ledger_load_and_filter_by_session() -> Result<()> {
        let tmpdir = TempDir::new().unwrap();
        let ledger_path = tmpdir.path().join("ledger.jsonl");

        let mut writer = ExperienceLedgerWriter::new(&ledger_path)?;
        let entries = vec![
            create_test_entry("sess_1", 1000, "warrior", "cast:CH"),
            create_test_entry("sess_2", 1000, "mage", "cast:CH"),
            create_test_entry("sess_1", 2000, "warrior", "med"),
        ];
        writer.append_batch(&entries)?;

        let ledger = ExperienceLedger::load_from_path(&ledger_path)?;
        let sess_1 = ledger.get_entries_for_session("sess_1");

        assert_eq!(sess_1.len(), 2);
        assert!(sess_1.iter().all(|e| e.session_id == "sess_1"));

        Ok(())
    }

    #[test]
    fn test_ledger_filter_by_class() -> Result<()> {
        let tmpdir = TempDir::new().unwrap();
        let ledger_path = tmpdir.path().join("ledger.jsonl");

        let mut writer = ExperienceLedgerWriter::new(&ledger_path)?;
        let entries = vec![
            create_test_entry("sess_1", 1000, "warrior", "cast:CH"),
            create_test_entry("sess_2", 1000, "mage", "cast:CH"),
            create_test_entry("sess_1", 2000, "warrior", "med"),
        ];
        writer.append_batch(&entries)?;

        let ledger = ExperienceLedger::load_from_path(&ledger_path)?;
        let warriors = ledger.get_entries_for_class("warrior");

        assert_eq!(warriors.len(), 2);
        assert!(warriors.iter().all(|e| e.class == "warrior"));

        Ok(())
    }

    #[test]
    fn test_ledger_idempotent_reload() -> Result<()> {
        let tmpdir = TempDir::new().unwrap();
        let ledger_path = tmpdir.path().join("ledger.jsonl");

        let mut writer = ExperienceLedgerWriter::new(&ledger_path)?;
        let entries = vec![
            create_test_entry("sess_1", 1000, "warrior", "cast:CH"),
            create_test_entry("sess_1", 2000, "warrior", "med"),
        ];
        writer.append_batch(&entries)?;

        // Load first time
        let ledger1 = ExperienceLedger::load_from_path(&ledger_path)?;
        let hash1 = format!("{:?}", ledger1.all_entries());

        // Load again
        let ledger2 = ExperienceLedger::load_from_path(&ledger_path)?;
        let hash2 = format!("{:?}", ledger2.all_entries());

        assert_eq!(hash1, hash2);
        assert_eq!(ledger1.len(), ledger2.len());

        Ok(())
    }
}
