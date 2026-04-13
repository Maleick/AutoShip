//! Session directory management for test output.
//!
//! Provides `SessionManager` for creating timestamped session directories
//! and `SessionMetadata` for recording session configuration.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Metadata written to `metadata.json` in the session directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Unique session identifier (short UUID-like string).
    pub session_id: String,
    /// UTC timestamp when the session was created.
    pub start_time: DateTime<Utc>,
    /// Account names participating in this session.
    pub accounts: Vec<String>,
    /// Scenario names being exercised.
    pub scenarios: Vec<String>,
    /// Target duration in seconds, if set.
    pub target_duration: Option<u64>,
    /// Hash of the configuration used for reproducibility.
    pub config_hash: String,
}

/// Manages creation and lifecycle of a timestamped session directory.
///
/// # Session directory format
///
/// `{base_dir}/session-{YYYY-MM-DD-HHMMSS}-{short_id}/`
pub struct SessionManager {
    /// Root path of this session.
    pub session_dir: PathBuf,
    /// Metadata written to `metadata.json`.
    pub metadata: SessionMetadata,
}

impl SessionManager {
    /// Create a new session directory under `base_dir`.
    ///
    /// The directory is created immediately and `metadata.json` is written.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created or the metadata file
    /// cannot be written.
    pub fn new(
        base_dir: impl AsRef<Path>,
        accounts: Vec<String>,
        scenarios: Vec<String>,
    ) -> Result<Self> {
        let base_dir = base_dir.as_ref();
        std::fs::create_dir_all(base_dir)
            .with_context(|| format!("failed to create base session dir {base_dir:?}"))?;

        for _ in 0..8 {
            let now = Utc::now();
            let short_id = Self::short_id(&now);
            let dir_name = format!("session-{}-{}", now.format("%Y-%m-%d-%H%M%S"), short_id);
            let session_dir = base_dir.join(&dir_name);

            match std::fs::create_dir(&session_dir) {
                Ok(()) => {
                    let metadata = SessionMetadata {
                        session_id: short_id,
                        start_time: now,
                        accounts: accounts.clone(),
                        scenarios: scenarios.clone(),
                        target_duration: None,
                        config_hash: String::new(),
                    };

                    let meta_path = session_dir.join("metadata.json");
                    let json = serde_json::to_string_pretty(&metadata)
                        .context("failed to serialize session metadata")?;
                    std::fs::write(&meta_path, json)
                        .with_context(|| format!("failed to write metadata to {meta_path:?}"))?;

                    return Ok(Self {
                        session_dir,
                        metadata,
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(err)
                        .with_context(|| format!("failed to create session dir {session_dir:?}"));
                }
            }
        }

        Err(anyhow::anyhow!(
            "failed to create a unique session directory after multiple attempts"
        ))
    }

    /// Set the target duration (seconds) and rewrite `metadata.json`.
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata file cannot be written.
    pub fn set_target_duration(&mut self, secs: u64) -> Result<()> {
        self.metadata.target_duration = Some(secs);
        self.write_metadata()
    }

    /// Set the config hash and rewrite `metadata.json`.
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata file cannot be written.
    pub fn set_config_hash(&mut self, hash: impl Into<String>) -> Result<()> {
        self.metadata.config_hash = hash.into();
        self.write_metadata()
    }

    /// Return the path to `metadata.json`.
    pub fn metadata_path(&self) -> PathBuf {
        self.session_dir.join("metadata.json")
    }

    /// Rewrite `metadata.json` in place.
    fn write_metadata(&self) -> Result<()> {
        let meta_path = self.metadata_path();
        let json = serde_json::to_string_pretty(&self.metadata)
            .context("failed to serialize session metadata")?;
        std::fs::write(&meta_path, json)
            .with_context(|| format!("failed to write metadata to {meta_path:?}"))?;
        Ok(())
    }

    /// Generate a short identifier derived from the nanosecond component of `now`.
    fn short_id(now: &DateTime<Utc>) -> String {
        format!("{:06x}", now.timestamp_subsec_nanos() % 0x100_0000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn tmp() -> TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn test_session_dir_created() {
        let dir = tmp();
        let sm = SessionManager::new(
            dir.path(),
            vec!["account1".into()],
            vec!["scenario_a".into()],
        )
        .expect("SessionManager::new");
        assert!(sm.session_dir.exists(), "session dir must be created");
    }

    #[test]
    fn test_metadata_json_written() {
        let dir = tmp();
        let sm = SessionManager::new(
            dir.path(),
            vec!["acct1".into(), "acct2".into()],
            vec!["s1".into()],
        )
        .expect("SessionManager::new");
        let meta_path = sm.metadata_path();
        assert!(meta_path.exists(), "metadata.json must exist");
        let raw = fs::read_to_string(&meta_path).expect("read metadata.json");
        let meta: SessionMetadata = serde_json::from_str(&raw).expect("parse metadata.json");
        assert_eq!(meta.accounts, vec!["acct1", "acct2"]);
        assert_eq!(meta.scenarios, vec!["s1"]);
    }

    #[test]
    fn test_session_dir_format() {
        let dir = tmp();
        let sm = SessionManager::new(dir.path(), vec![], vec![]).expect("SessionManager::new");
        let name = sm
            .session_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        // format: session-YYYY-MM-DD-HHMMSS-XXXXXX
        assert!(
            name.starts_with("session-"),
            "dir name must start with 'session-': {name}"
        );
        let parts: Vec<&str> = name.split('-').collect();
        // parts: ["session", YYYY, MM, DD, HHMMSS, short_id]
        assert_eq!(parts.len(), 6, "unexpected dir name format: {name}");
    }

    #[test]
    fn test_set_target_duration() {
        let dir = tmp();
        let mut sm = SessionManager::new(dir.path(), vec![], vec![]).expect("SessionManager::new");
        sm.set_target_duration(300).expect("set_target_duration");
        let raw = fs::read_to_string(sm.metadata_path()).expect("read metadata.json");
        let meta: SessionMetadata = serde_json::from_str(&raw).expect("parse");
        assert_eq!(meta.target_duration, Some(300));
    }

    #[test]
    fn test_set_config_hash() {
        let dir = tmp();
        let mut sm = SessionManager::new(dir.path(), vec![], vec![]).expect("SessionManager::new");
        sm.set_config_hash("abc123").expect("set_config_hash");
        let raw = fs::read_to_string(sm.metadata_path()).expect("read metadata.json");
        let meta: SessionMetadata = serde_json::from_str(&raw).expect("parse");
        assert_eq!(meta.config_hash, "abc123");
    }
}
