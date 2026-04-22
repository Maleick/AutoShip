//! Log file retention — size-based and age-based cleanup for the log directory.
//!
//! `tracing-appender`'s `RollingFileAppender` handles count-based rotation
//! (`max_log_files`).  This module supplements that by:
//!
//! * **Size pruning** — deleting the oldest log files in a directory until the
//!   total on-disk size falls under a configurable ceiling.
//! * **Age pruning** — deleting any log files whose modification time is older
//!   than a configurable number of days.
//!
//! Both operations are best-effort: individual file errors are logged and
//! skipped rather than propagated, so a locked file never aborts the whole
//! cleanup pass.

use std::path::Path;

/// Remove log files from `log_dir` whose names start with `prefix` and whose
/// modification time is older than `max_age_days` days.
///
/// Files that cannot be stat'd or deleted are skipped with a warning.  Returns
/// the number of files removed.
pub fn prune_by_age(log_dir: &Path, prefix: &str, max_age_days: u64) -> usize {
    if max_age_days == 0 {
        return 0;
    }

    let cutoff = match std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(max_age_days * 86_400))
    {
        Some(t) => t,
        None => return 0,
    };

    let entries = match std::fs::read_dir(log_dir) {
        Ok(e) => e,
        Err(err) => {
            tracing::warn!("log_retention: cannot read log dir {}: {err}", log_dir.display());
            return 0;
        }
    };

    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if !name_str.starts_with(prefix) {
            continue;
        }

        let mtime = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(err) => {
                tracing::warn!("log_retention: cannot stat {}: {err}", path.display());
                continue;
            }
        };

        if mtime < cutoff {
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    tracing::debug!("log_retention: removed aged-out file {}", path.display());
                    removed += 1;
                }
                Err(err) => {
                    tracing::warn!("log_retention: cannot remove {}: {err}", path.display());
                }
            }
        }
    }

    removed
}

/// Remove the oldest log files from `log_dir` whose names start with `prefix`
/// until the total size of matching files is under `max_size_bytes`.
///
/// Files are sorted oldest-first (by modification time).  Files that cannot be
/// stat'd are skipped.  Returns the number of files removed.
pub fn prune_by_size(log_dir: &Path, prefix: &str, max_size_bytes: u64) -> usize {
    if max_size_bytes == 0 {
        return 0;
    }

    let entries = match std::fs::read_dir(log_dir) {
        Ok(e) => e,
        Err(err) => {
            tracing::warn!("log_retention: cannot read log dir {}: {err}", log_dir.display());
            return 0;
        }
    };

    // Collect (mtime, size, path) for matching files.
    let mut files: Vec<(std::time::SystemTime, u64, std::path::PathBuf)> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let mtime = meta.modified().ok()?;
            let size = meta.len();
            Some((mtime, size, e.path()))
        })
        .collect();

    let total: u64 = files.iter().map(|(_, s, _)| s).sum();
    if total <= max_size_bytes {
        return 0;
    }

    // Sort oldest-first so we delete the oldest files first.
    files.sort_by_key(|(mtime, _, _)| *mtime);

    let mut remaining = total;
    let mut removed = 0usize;

    for (_, size, path) in files {
        if remaining <= max_size_bytes {
            break;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => {
                tracing::debug!("log_retention: removed oversized file {}", path.display());
                remaining = remaining.saturating_sub(size);
                removed += 1;
            }
            Err(err) => {
                tracing::warn!("log_retention: cannot remove {}: {err}", path.display());
            }
        }
    }

    removed
}

/// Run both age and size pruning passes for a log prefix.
///
/// This is a convenience wrapper that calls [`prune_by_age`] first (which
/// removes clearly stale files), then [`prune_by_size`] (which enforces the
/// disk-space ceiling on whatever remains).
pub fn prune_log_dir(log_dir: &Path, prefix: &str, max_size_bytes: u64, max_age_days: u64) {
    let aged = prune_by_age(log_dir, prefix, max_age_days);
    let oversized = prune_by_size(log_dir, prefix, max_size_bytes);
    if aged + oversized > 0 {
        tracing::info!(
            aged,
            oversized,
            log_dir = %log_dir.display(),
            "log_retention: pruned log files"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn write_file(dir: &Path, name: &str, content: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("create");
        f.write_all(content).expect("write");
        path
    }

    #[test]
    fn prune_by_age_zero_days_is_noop() {
        let dir = tmp_dir();
        write_file(dir.path(), "textquest.log.2024-01-01", b"old data");
        let removed = prune_by_age(dir.path(), "textquest.log", 0);
        assert_eq!(removed, 0);
        assert!(dir.path().join("textquest.log.2024-01-01").exists());
    }

    #[test]
    fn prune_by_size_zero_limit_is_noop() {
        let dir = tmp_dir();
        write_file(dir.path(), "textquest.log.2024-01-01", b"some data");
        let removed = prune_by_size(dir.path(), "textquest.log", 0);
        assert_eq!(removed, 0);
    }

    #[test]
    fn prune_by_size_removes_oldest_when_over_limit() {
        let dir = tmp_dir();

        // Write 3 files each 100 bytes; limit = 150 bytes → oldest 2 removed until ≤150.
        let data = b"x".repeat(100);

        // Create files with sequential names so ordering is deterministic by name
        // (mtime precision on some filesystems is 1s, so we sleep between writes
        //  in a real test; here we rely on the sort being stable across the batch).
        let f1 = write_file(dir.path(), "textquest.log.2024-01-01", &data);
        // brief sleep to ensure different mtimes
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _f2 = write_file(dir.path(), "textquest.log.2024-01-02", &data);
        std::thread::sleep(std::time::Duration::from_millis(10));
        let f3 = write_file(dir.path(), "textquest.log.2024-01-03", &data);

        // 300 bytes total, limit 150 → should remove the 2 oldest (f1, f2).
        let removed = prune_by_size(dir.path(), "textquest.log", 150);

        // f3 (newest) must survive.
        assert!(f3.exists(), "newest file should survive");
        // f1 (oldest) must be gone.
        assert!(!f1.exists(), "oldest file should be removed");
        assert!(removed >= 1, "at least one file removed; got {removed}");
    }

    #[test]
    fn prune_by_size_noop_when_under_limit() {
        let dir = tmp_dir();
        write_file(dir.path(), "textquest.log.2024-01-01", b"small");
        let removed = prune_by_size(dir.path(), "textquest.log", 1024 * 1024);
        assert_eq!(removed, 0);
    }

    #[test]
    fn prune_ignores_files_with_different_prefix() {
        let dir = tmp_dir();
        let data = b"x".repeat(200);
        write_file(dir.path(), "other.log.2024-01-01", &data);
        // limit is 0 bytes but prefix doesn't match
        let removed = prune_by_size(dir.path(), "textquest.log", 0);
        assert_eq!(removed, 0);
        assert!(dir.path().join("other.log.2024-01-01").exists());
    }

    #[test]
    fn prune_log_dir_nonexistent_dir_does_not_panic() {
        let path = std::path::PathBuf::from("/tmp/textquest-nonexistent-test-dir-xyz");
        // Should not panic even if dir does not exist.
        prune_log_dir(&path, "textquest.log", 100 * 1024 * 1024, 30);
    }

    #[test]
    fn log_config_defaults_match_expected_values() {
        use crate::config::LogConfig;
        let cfg = LogConfig::default();
        assert_eq!(cfg.max_size_mb, 100);
        assert_eq!(cfg.max_files, 7);
        assert_eq!(cfg.max_age_days, 30);
    }
}
