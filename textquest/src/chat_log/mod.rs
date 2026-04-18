//! Chat logging module — writes per-character chat output to log files.
//!
//! Inspired by MQ2Log. Each character gets its own log file named
//! `logs/server_charname.log`. Supports log rotation (daily or by file size)
//! and filtering by log level.

pub mod config;
pub mod writer;

use std::collections::{HashMap, hash_map::Entry};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use textquest_common::ipc::ChatMessageInfo;

pub use config::{ChatChannel, ChatLogConfig, LogLevel, RotationStrategy};

struct ChatLogWriter {
    file: Option<BufWriter<std::fs::File>>,
    path: PathBuf,
    current_size_bytes: u64,
    last_rotation_date: String,
}

impl ChatLogWriter {
    fn new(path: PathBuf) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let current_size_bytes = file.metadata()?.len();
        let last_rotation_date = Self::today_date();
        Ok(Self {
            file: Some(BufWriter::new(file)),
            path,
            current_size_bytes,
            last_rotation_date,
        })
    }

    fn today_date() -> String {
        chrono::Local::now().format("%Y-%m-%d").to_string()
    }

    fn check_rotation(
        &mut self,
        strategy: &RotationStrategy,
        _max_size_bytes: u64,
    ) -> std::io::Result<()> {
        match strategy {
            RotationStrategy::Daily => {
                let today = Self::today_date();
                if today != self.last_rotation_date {
                    self.rotate(&today)?;
                }
            }
            RotationStrategy::Size(max_size) => {
                if self.current_size_bytes >= *max_size {
                    let date = Self::today_date();
                    self.rotate(&date)?;
                }
            }
            RotationStrategy::None => {}
        }
        Ok(())
    }

    fn rotate(&mut self, date_suffix: &str) -> std::io::Result<()> {
        if let Some(file) = self.file.as_mut() {
            file.flush()?;
        }
        if let Some(file) = self.file.take() {
            let file = file.into_inner().map_err(|error| error.into_error())?;
            drop(file);
        }

        let archive_dir = self.path.parent().unwrap_or(Path::new("."));
        let stem = self
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("chat");
        let ext = self
            .path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("log");
        let archive_name = format!("{}.{}.{}", stem, date_suffix, ext);
        let archive_path = archive_dir.join(&archive_name);

        if !archive_path.exists() {
            std::fs::rename(&self.path, &archive_path)?;
        } else {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let unique_name = format!("{}.{}.{}.{}", stem, date_suffix, timestamp, ext);
            let unique_path = archive_dir.join(&unique_name);
            std::fs::rename(&self.path, &unique_path)?;
        }

        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)?;
        self.file = Some(BufWriter::new(file));
        self.current_size_bytes = 0;
        self.last_rotation_date = date_suffix.to_string();
        Ok(())
    }

    fn write_line(&mut self, line: &str, level: LogLevel) -> std::io::Result<()> {
        let formatted_line = format!("[{}] {line}\n", level);
        let file = self
            .file
            .as_mut()
            .expect("chat log writer should always have an active file");
        file.write_all(formatted_line.as_bytes())?;
        file.flush()?;
        self.current_size_bytes += formatted_line.len() as u64;
        Ok(())
    }
}

pub struct ChatLogManager {
    writers: HashMap<String, ChatLogWriter>,
    config: ChatLogConfig,
    log_dir: PathBuf,
}

impl ChatLogManager {
    pub fn new(config: ChatLogConfig, log_dir: PathBuf) -> std::io::Result<Self> {
        Ok(Self {
            writers: HashMap::new(),
            config,
            log_dir,
        })
    }

    fn log_path(&self, server: &str, character: &str) -> PathBuf {
        let sanitized_server = server.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        let sanitized_char = character.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        self.log_dir
            .join(format!("{}_{}.log", sanitized_server, sanitized_char))
    }

    fn writer_for(&mut self, server: &str, character: &str) -> std::io::Result<&mut ChatLogWriter> {
        let key = format!("{}/{}", server, character);
        let path = self.log_path(server, character);

        match self.writers.entry(key) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                std::fs::create_dir_all(&self.log_dir)?;
                let writer = ChatLogWriter::new(path)?;
                Ok(entry.insert(writer))
            }
        }
    }

    pub fn log_message(
        &mut self,
        server: &str,
        character: &str,
        message: &ChatMessageInfo,
        channel: Option<ChatChannel>,
    ) -> std::io::Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        if let Some(ch) = channel
            && !self.config.channels.contains(&ch)
        {
            return Ok(());
        }

        let rotation_strategy = self.config.rotation_strategy;
        let max_file_size_bytes = self.config.max_file_size_bytes;
        let writer = self.writer_for(server, character)?;

        writer.check_rotation(&rotation_strategy, max_file_size_bytes)?;

        let timestamp = Self::format_timestamp(message.timestamp_ms);
        let channel_prefix = channel.map(|c| format!("[{}] ", c)).unwrap_or_default();
        let line = format!("{}{} {}", timestamp, channel_prefix, message.text);

        writer.write_line(&line, LogLevel::Info)
    }

    pub fn log_mq2_output(
        &mut self,
        server: &str,
        character: &str,
        text: &str,
        level: LogLevel,
    ) -> std::io::Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        if level < self.config.min_level {
            return Ok(());
        }

        let rotation_strategy = self.config.rotation_strategy;
        let max_file_size_bytes = self.config.max_file_size_bytes;
        let writer = self.writer_for(server, character)?;

        writer.check_rotation(&rotation_strategy, max_file_size_bytes)?;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let timestamp = Self::format_timestamp(now_ms);
        let line = format!("{}{}", timestamp, text);

        writer.write_line(&line, level)
    }

    fn format_timestamp(timestamp_ms: u64) -> String {
        let secs = timestamp_ms / 1000;
        let millis = timestamp_ms % 1000;
        let total_days = secs / 86400;
        let remaining_secs = secs % 86400;
        let hours = remaining_secs / 3600;
        let minutes = (remaining_secs % 3600) / 60;
        let secs = remaining_secs % 60;

        let year_base = 1970;
        let mut year = year_base;
        let mut remaining_days = total_days as i64;

        loop {
            let days_in_year = if Self::is_leap_year(year) { 366 } else { 365 };
            if remaining_days < days_in_year {
                break;
            }
            remaining_days -= days_in_year;
            year += 1;
        }

        let is_leap = Self::is_leap_year(year);
        let days_in_months = if is_leap {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };

        let mut month = 1;
        for &days_in_month in &days_in_months {
            if remaining_days < days_in_month as i64 {
                break;
            }
            remaining_days -= days_in_month as i64;
            month += 1;
        }
        let day = remaining_days + 1;

        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            year, month, day, hours, minutes, secs, millis
        )
    }

    fn is_leap_year(year: i64) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    pub fn update_config(&mut self, config: ChatLogConfig) {
        self.config = config;
    }

    pub fn get_config(&self) -> &ChatLogConfig {
        &self.config
    }

    pub fn close_writer(&mut self, server: &str, character: &str) {
        let key = format!("{}/{}", server, character);
        if let Some(mut writer) = self.writers.remove(&key)
            && let Some(file) = writer.file.as_mut()
        {
            let _ = file.flush();
        }
    }

    pub fn close_all(&mut self) {
        for (_, mut writer) in self.writers.drain() {
            if let Some(file) = writer.file.as_mut() {
                let _ = file.flush();
            }
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::io::Read;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_log_dir() -> PathBuf {
        let temp = std::env::temp_dir();
        let unique = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = temp.join(format!(
            "textquest-chat-log-test-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    fn default_config() -> ChatLogConfig {
        ChatLogConfig {
            enabled: true,
            channels: vec![ChatChannel::Say, ChatChannel::MQ2],
            rotation_strategy: RotationStrategy::Size(1024 * 1024),
            max_file_size_bytes: 1024 * 1024,
            min_level: LogLevel::Info,
            log_eq_chat: false,
        }
    }

    #[test]
    fn log_message_creates_file() {
        let dir = temp_log_dir();
        let mut config = default_config();
        config.channels = vec![ChatChannel::Say];
        config.log_eq_chat = true;
        let mut manager = ChatLogManager::new(config, dir.clone()).unwrap();

        let message = ChatMessageInfo {
            text: "Test chat message".to_string(),
            color: 273,
            timestamp_ms: 1700000000000,
        };

        manager
            .log_message("Firiona Vie", "TestChar", &message, Some(ChatChannel::Say))
            .unwrap();

        let log_path = dir.join("Firiona Vie_TestChar.log");
        assert!(log_path.exists());

        let mut contents = String::new();
        std::fs::File::open(&log_path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert!(contents.contains("Test chat message"));
    }

    #[test]
    fn log_message_respects_disabled() {
        let dir = temp_log_dir();
        let mut config = default_config();
        config.enabled = false;
        config.channels = vec![ChatChannel::Say];
        config.log_eq_chat = true;
        let mut manager = ChatLogManager::new(config, dir.clone()).unwrap();

        let message = ChatMessageInfo {
            text: "Test chat message".to_string(),
            color: 273,
            timestamp_ms: 1700000000000,
        };

        manager
            .log_message("Firiona Vie", "TestChar", &message, Some(ChatChannel::Say))
            .unwrap();

        let log_path = dir.join("Firiona Vie_TestChar.log");
        assert!(!log_path.exists());
    }

    #[test]
    fn log_message_filters_channels() {
        let dir = temp_log_dir();
        let mut config = default_config();
        config.channels = vec![ChatChannel::Group];
        config.log_eq_chat = true;
        let mut manager = ChatLogManager::new(config, dir.clone()).unwrap();

        let message = ChatMessageInfo {
            text: "Test group message".to_string(),
            color: 273,
            timestamp_ms: 1700000000000,
        };

        manager
            .log_message(
                "Firiona Vie",
                "TestChar",
                &message,
                Some(ChatChannel::Group),
            )
            .unwrap();

        let log_path = dir.join("Firiona Vie_TestChar.log");
        assert!(log_path.exists());

        let mut contents = String::new();
        std::fs::File::open(&log_path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert!(contents.contains("Test group message"));

        manager
            .log_message("Firiona Vie", "TestChar", &message, Some(ChatChannel::Say))
            .unwrap();

        let mut contents2 = String::new();
        std::fs::File::open(&log_path)
            .unwrap()
            .read_to_string(&mut contents2)
            .unwrap();
        assert_eq!(contents, contents2);
    }

    #[test]
    fn log_mq2_output_respects_level_filter() {
        let dir = temp_log_dir();
        let mut config = default_config();
        config.min_level = LogLevel::Debug;
        let mut manager = ChatLogManager::new(config, dir.clone()).unwrap();

        manager
            .log_mq2_output("Firiona Vie", "TestChar", "Debug message", LogLevel::Debug)
            .unwrap();

        let log_path = dir.join("Firiona Vie_TestChar.log");
        assert!(log_path.exists());

        let mut contents = String::new();
        std::fs::File::open(&log_path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert!(contents.contains("Debug message"));
    }

    #[test]
    fn format_timestamp_works() {
        let timestamp_ms = 1700000000000u64;
        let formatted = ChatLogManager::format_timestamp(timestamp_ms);
        assert!(formatted.contains("2023-11-14"));
        assert!(formatted.contains(":"));
    }

    #[test]
    fn sanitize_path_characters() {
        let dir = temp_log_dir();
        let manager = ChatLogManager::new(default_config(), dir).unwrap();
        let path = manager.log_path("Firiona|Vie", "Test:Char");
        assert!(!path.to_string_lossy().contains('|'));
        assert!(!path.to_string_lossy().contains(':'));
    }

    #[test]
    fn close_writer_works() {
        let dir = temp_log_dir();
        let config = default_config();
        let mut manager = ChatLogManager::new(config, dir.clone()).unwrap();

        let message = ChatMessageInfo {
            text: "Test".to_string(),
            color: 273,
            timestamp_ms: 1700000000000,
        };

        manager
            .log_message("Firiona Vie", "TestChar", &message, None)
            .unwrap();
        manager.close_writer("Firiona Vie", "TestChar");

        let log_path = dir.join("Firiona Vie_TestChar.log");
        assert!(log_path.exists());
    }

    #[test]
    fn size_rotation_archives_before_reopening_log_file() {
        let dir = temp_log_dir();
        let mut config = default_config();
        config.rotation_strategy = RotationStrategy::Size(32);
        config.max_file_size_bytes = 32;
        let mut manager = ChatLogManager::new(config, dir.clone()).unwrap();

        let first = ChatMessageInfo {
            text: "first rotation trigger line".to_string(),
            color: 273,
            timestamp_ms: 1700000000000,
        };
        let second = ChatMessageInfo {
            text: "second line after rotate".to_string(),
            color: 273,
            timestamp_ms: 1700000001000,
        };

        manager
            .log_message("Firiona Vie", "TestChar", &first, Some(ChatChannel::Say))
            .unwrap();
        manager
            .log_message("Firiona Vie", "TestChar", &second, Some(ChatChannel::Say))
            .unwrap();

        let log_path = dir.join("Firiona Vie_TestChar.log");
        let mut current = String::new();
        std::fs::File::open(&log_path)
            .unwrap()
            .read_to_string(&mut current)
            .unwrap();
        assert!(current.contains("second line after rotate"));
        assert!(!current.contains("first rotation trigger line"));

        let archived = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                let file_name = entry.file_name();
                let file_name = file_name.to_string_lossy();
                file_name.starts_with("Firiona Vie_TestChar.")
                    && file_name.ends_with(".log")
                    && file_name != "Firiona Vie_TestChar.log"
            })
            .expect("rotated archive");
        let mut archived_contents = String::new();
        std::fs::File::open(archived.path())
            .unwrap()
            .read_to_string(&mut archived_contents)
            .unwrap();
        assert!(archived_contents.contains("first rotation trigger line"));
    }

    #[test]
    fn size_rotation_archives_before_reopening_active_log() {
        let dir = temp_log_dir();
        let mut config = default_config();
        config.rotation_strategy = RotationStrategy::Size(1);
        config.max_file_size_bytes = 1;
        let mut manager = ChatLogManager::new(config, dir.clone()).unwrap();

        let first = ChatMessageInfo {
            text: "First line".to_string(),
            color: 273,
            timestamp_ms: 1700000000000,
        };
        let second = ChatMessageInfo {
            text: "Second line".to_string(),
            color: 273,
            timestamp_ms: 1700000001000,
        };

        manager
            .log_message("Firiona Vie", "TestChar", &first, Some(ChatChannel::Say))
            .unwrap();
        manager
            .log_message("Firiona Vie", "TestChar", &second, Some(ChatChannel::Say))
            .unwrap();

        let log_path = dir.join("Firiona Vie_TestChar.log");
        let current = std::fs::read_to_string(&log_path).expect("current log");
        assert!(current.contains("Second line"));
        assert!(!current.contains("First line"));

        let archive_count = std::fs::read_dir(&dir)
            .expect("archive dir")
            .filter_map(Result::ok)
            .filter(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                name.starts_with("Firiona Vie_TestChar.")
                    && name.ends_with(".log")
                    && name != "Firiona Vie_TestChar.log"
            })
            .count();
        assert!(archive_count >= 1, "expected rotated archive file");
    }

    #[test]
    fn write_line_tracks_full_written_bytes() {
        let path = temp_log_dir().join("size-test.log");
        let mut writer = ChatLogWriter::new(path.clone()).unwrap();

        writer.write_line("payload", LogLevel::Warn).unwrap();

        let on_disk = std::fs::metadata(&path).unwrap().len();
        assert_eq!(writer.current_size_bytes, on_disk);
    }

    #[cfg(unix)]
    #[test]
    fn log_message_returns_error_when_writer_creation_fails() {
        use std::os::unix::fs::PermissionsExt;

        let temp_root = temp_log_dir();
        let read_only_root = temp_root.join("readonly");
        std::fs::create_dir_all(&read_only_root).unwrap();
        std::fs::set_permissions(&read_only_root, std::fs::Permissions::from_mode(0o555)).unwrap();

        let mut config = default_config();
        config.channels = vec![ChatChannel::Say];
        config.log_eq_chat = true;
        let nested_log_dir = read_only_root.join("nested");
        let mut manager = ChatLogManager::new(config, nested_log_dir).unwrap();

        let message = ChatMessageInfo {
            text: "Test chat message".to_string(),
            color: 273,
            timestamp_ms: 1700000000000,
        };

        let error = manager
            .log_message("Firiona Vie", "TestChar", &message, Some(ChatChannel::Say))
            .expect_err("writer creation should fail");
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);

        std::fs::set_permissions(&read_only_root, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::remove_dir_all(&temp_root).ok();
    }
}

#[cfg(test)]
mod cross_platform_tests {
    use super::*;
    use textquest_common::ipc::ChatMessageInfo;

    fn enabled_config(channels: Vec<ChatChannel>) -> ChatLogConfig {
        ChatLogConfig {
            enabled: true,
            channels,
            rotation_strategy: RotationStrategy::None,
            max_file_size_bytes: 10 * 1024 * 1024,
            min_level: LogLevel::Trace,
            log_eq_chat: false,
        }
    }

    // ── Pure-function tests ──────────────────────────────────────────────

    #[test]
    fn is_leap_year_divisible_by_4_but_not_100() {
        assert!(ChatLogManager::is_leap_year(2024));
        assert!(ChatLogManager::is_leap_year(2000));
        assert!(!ChatLogManager::is_leap_year(1900));
        assert!(!ChatLogManager::is_leap_year(2023));
    }

    #[test]
    fn format_timestamp_unix_epoch() {
        // Epoch (0 ms) is 1970-01-01 00:00:00.000
        let formatted = ChatLogManager::format_timestamp(0);
        assert_eq!(formatted, "1970-01-01 00:00:00.000");
    }

    #[test]
    fn format_timestamp_known_date() {
        // 2023-11-14 22:13:20.000 UTC → 1700000000 seconds
        let formatted = ChatLogManager::format_timestamp(1_700_000_000_000);
        assert!(
            formatted.starts_with("2023-11-14"),
            "expected 2023-11-14, got {formatted}"
        );
    }

    #[test]
    fn format_timestamp_preserves_milliseconds() {
        // 1 second + 500 ms past epoch
        let formatted = ChatLogManager::format_timestamp(1_500);
        assert!(
            formatted.ends_with(".500"),
            "expected .500 suffix, got {formatted}"
        );
    }

    #[test]
    fn log_path_sanitizes_special_characters() {
        let dir = tempfile::tempdir().unwrap();
        let manager = ChatLogManager::new(
            enabled_config(vec![ChatChannel::Say]),
            dir.path().to_path_buf(),
        )
        .unwrap();
        let path = manager.log_path("Firiona|Vie", "Test:Char*");
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(!name.contains('|'), "pipe not sanitized: {name}");
        assert!(!name.contains(':'), "colon not sanitized: {name}");
        assert!(!name.contains('*'), "asterisk not sanitized: {name}");
    }

    // ── I/O behaviour tests ──────────────────────────────────────────────

    #[test]
    fn log_message_writes_to_file_when_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let config = enabled_config(vec![ChatChannel::Say]);
        let mut manager = ChatLogManager::new(config, dir.path().to_path_buf()).unwrap();

        let msg = ChatMessageInfo {
            text: "Hello world".to_string(),
            color: 0,
            timestamp_ms: 0,
        };
        manager
            .log_message("Server", "Char", &msg, Some(ChatChannel::Say))
            .unwrap();

        let content = std::fs::read_to_string(dir.path().join("Server_Char.log")).unwrap();
        assert!(content.contains("Hello world"));
    }

    #[test]
    fn log_message_skips_write_when_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = enabled_config(vec![ChatChannel::Say]);
        config.enabled = false;
        let mut manager = ChatLogManager::new(config, dir.path().to_path_buf()).unwrap();

        let msg = ChatMessageInfo {
            text: "Should not appear".to_string(),
            color: 0,
            timestamp_ms: 0,
        };
        manager
            .log_message("Server", "Char", &msg, Some(ChatChannel::Say))
            .unwrap();

        assert!(!dir.path().join("Server_Char.log").exists());
    }

    #[test]
    fn log_message_skips_write_for_filtered_channel() {
        let dir = tempfile::tempdir().unwrap();
        // Only Guild is in the allow-list.
        let config = enabled_config(vec![ChatChannel::Guild]);
        let mut manager = ChatLogManager::new(config, dir.path().to_path_buf()).unwrap();

        let msg = ChatMessageInfo {
            text: "Say message".to_string(),
            color: 0,
            timestamp_ms: 0,
        };
        // Sending on Say (not in allow-list) should be silently dropped.
        manager
            .log_message("Server", "Char", &msg, Some(ChatChannel::Say))
            .unwrap();

        assert!(!dir.path().join("Server_Char.log").exists());
    }

    #[test]
    fn log_mq2_output_skips_write_below_min_level() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = enabled_config(vec![]);
        config.min_level = LogLevel::Error;
        let mut manager = ChatLogManager::new(config, dir.path().to_path_buf()).unwrap();

        manager
            .log_mq2_output("Server", "Char", "debug output", LogLevel::Debug)
            .unwrap();

        assert!(!dir.path().join("Server_Char.log").exists());
    }

    #[test]
    fn log_mq2_output_writes_at_or_above_min_level() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = enabled_config(vec![]);
        config.min_level = LogLevel::Warn;
        let mut manager = ChatLogManager::new(config, dir.path().to_path_buf()).unwrap();

        manager
            .log_mq2_output("Server", "Char", "warning output", LogLevel::Warn)
            .unwrap();

        let content = std::fs::read_to_string(dir.path().join("Server_Char.log")).unwrap();
        assert!(content.contains("warning output"));
    }

    #[test]
    fn update_config_changes_enabled_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = enabled_config(vec![ChatChannel::MQ2]);
        config.enabled = false;
        let mut manager = ChatLogManager::new(config, dir.path().to_path_buf()).unwrap();

        let new_config = ChatLogConfig {
            enabled: true,
            channels: vec![ChatChannel::MQ2],
            ..ChatLogConfig::default()
        };
        manager.update_config(new_config);

        assert!(manager.get_config().enabled);
    }
}
