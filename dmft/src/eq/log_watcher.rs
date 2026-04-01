use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;

use super::log_parser::{LogEvent, LootDatabase};

/// Tails an EQ log file and feeds parsed events into a `LootDatabase`.
pub struct LogWatcher {
    path: PathBuf,
    last_position: u64,
    database: LootDatabase,
}

impl LogWatcher {
    /// Create a new log watcher positioned at the end of the given file.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        // Start at the end of the file so we only capture new events
        let last_position = std::fs::metadata(&path).map_or(0, |m| m.len());

        Self {
            path,
            last_position,
            database: LootDatabase::new(),
        }
    }

    /// Read new lines since last poll, parse them, and return any recognized events.
    pub fn poll(&mut self) -> Vec<LogEvent> {
        let mut events = Vec::new();

        let Ok(file) = File::open(&self.path) else {
            return events; // File not found — graceful on macOS
        };

        let Ok(metadata) = file.metadata() else {
            return events;
        };

        // If the file shrank (log rotation), reset to beginning
        if metadata.len() < self.last_position {
            self.last_position = 0;
        }

        // Nothing new
        if metadata.len() == self.last_position {
            return events;
        }

        let mut reader = BufReader::new(file);
        if reader.seek(SeekFrom::Start(self.last_position)).is_err() {
            return events;
        }

        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break, // EOF or error
                Ok(_) => {
                    if let Some(event) = self.database.process_line(line.trim_end()) {
                        events.push(event);
                    }
                }
            }
        }

        self.last_position = reader.stream_position().unwrap_or(self.last_position);
        events
    }

    /// Reference to the accumulated loot database.
    #[must_use]
    pub fn database(&self) -> &LootDatabase {
        &self.database
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_poll_reads_new_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("eqlog_Test_server.txt");

        // Create file with initial content (simulates existing log)
        {
            let mut f = File::create(&path).unwrap();
            writeln!(f, "[Thu Mar 28 12:00:00 2026] Loading, please wait...").unwrap();
        }

        let mut watcher = LogWatcher::new(path.clone());

        // First poll — nothing new since we started at EOF
        let events = watcher.poll();
        assert!(events.is_empty());

        // Append new lines
        {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "[Thu Mar 28 12:01:00 2026] You have slain a moss snake!").unwrap();
            writeln!(f, "[Thu Mar 28 12:01:01 2026] You gain experience!").unwrap();
            writeln!(
                f,
                "[Thu Mar 28 12:01:02 2026] You receive 3 platinum from the corpse."
            )
            .unwrap();
        }

        let events = watcher.poll();
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], LogEvent::Kill { .. }));
        assert!(matches!(events[1], LogEvent::Experience { .. }));
        assert!(matches!(events[2], LogEvent::Money { plat: 3, .. }));

        // Database should have accumulated
        assert_eq!(watcher.database().kills["a moss snake"], 1);
        assert_eq!(watcher.database().total_xp_events, 1);
        assert_eq!(watcher.database().total_plat, 3);
    }

    #[test]
    fn test_poll_missing_file() {
        let mut watcher = LogWatcher::new(PathBuf::from("/nonexistent/eqlog.txt"));
        let events = watcher.poll();
        assert!(events.is_empty());
    }

    #[test]
    fn test_poll_handles_log_rotation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("eqlog_Test_server.txt");

        // Create file with some content
        {
            let mut f = File::create(&path).unwrap();
            writeln!(f, "[Thu Mar 28 12:00:00 2026] You have slain a rat!").unwrap();
            writeln!(f, "[Thu Mar 28 12:00:01 2026] You gain experience!").unwrap();
        }

        let mut watcher = LogWatcher::new(path.clone());

        // Simulate log rotation — file gets truncated/replaced with smaller content
        {
            let mut f = File::create(&path).unwrap();
            writeln!(f, "[Thu Mar 28 13:00:00 2026] You have slain a bat!").unwrap();
        }

        let events = watcher.poll();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], LogEvent::Kill { ref mob } if mob == "a bat"));
    }

    #[test]
    fn test_new_starts_at_eof() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("eqlog.txt");
        {
            let mut f = File::create(&path).unwrap();
            writeln!(f, "old line 1").unwrap();
            writeln!(f, "old line 2").unwrap();
        }
        let watcher = LogWatcher::new(path);
        // Fresh watcher should have empty database
        assert_eq!(watcher.database().total_xp_events, 0);
    }

    #[test]
    fn test_poll_empty_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("eqlog.txt");
        File::create(&path).unwrap();
        let mut watcher = LogWatcher::new(path);
        let events = watcher.poll();
        assert!(events.is_empty());
    }

    #[test]
    fn test_poll_unrecognized_lines_filtered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("eqlog.txt");
        File::create(&path).unwrap();
        let mut watcher = LogWatcher::new(path.clone());
        // Append unrecognized lines
        {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "Random combat message that is not parsed").unwrap();
            writeln!(f, "Another line that means nothing").unwrap();
        }
        let events = watcher.poll();
        assert!(events.is_empty());
    }

    #[test]
    fn test_poll_successive_calls_no_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("eqlog.txt");
        File::create(&path).unwrap();
        let mut watcher = LogWatcher::new(path.clone());

        // First append
        {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "You gain experience!").unwrap();
        }
        let events = watcher.poll();
        assert_eq!(events.len(), 1);

        // Second poll without new data
        let events = watcher.poll();
        assert!(events.is_empty());

        // Second append
        {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "You gain party experience!").unwrap();
        }
        let events = watcher.poll();
        assert_eq!(events.len(), 1);
        assert_eq!(watcher.database().total_xp_events, 2);
    }

    #[test]
    fn test_database_accessor() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("eqlog.txt");
        File::create(&path).unwrap();
        let mut watcher = LogWatcher::new(path.clone());
        {
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "You have slain a rat!").unwrap();
            writeln!(f, "You have slain a rat!").unwrap();
        }
        watcher.poll();
        let db = watcher.database();
        assert_eq!(db.kills["a rat"], 2);
    }
}
