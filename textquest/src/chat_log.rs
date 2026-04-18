use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};

use textquest_common::chat::{ChatChannel, ChatEvent, ChatLogConfig, LogLevel, LogRotation};

pub struct ChatLogWriter {
    config: ChatLogConfig,
    writers: Mutex<HashMap<String, LogWriterHandle>>,
    log_dir: PathBuf,
}

struct LogWriterHandle {
    writer: BufWriter<File>,
    current_date: Option<String>,
    current_size: u64,
    max_size: Option<u64>,
}

impl ChatLogWriter {
    pub fn new(log_dir: PathBuf, config: ChatLogConfig) -> io::Result<Self> {
        fs::create_dir_all(&log_dir)?;
        Ok(Self {
            config,
            writers: Mutex::new(HashMap::new()),
            log_dir,
        })
    }

    pub fn reconfigure(&mut self, config: ChatLogConfig) -> io::Result<()> {
        if !config.enabled {
            self.writers.lock().unwrap().clear();
        }
        self.config = config;
        Ok(())
    }

    pub fn write_event(&self, character: &str, server: &str, event: &ChatEvent) -> io::Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        if !self.should_log_channel(&event.channel) {
            return Ok(());
        }

        let file_path = self.resolve_log_path(server, character);
        let mut writers = self.writers.lock().unwrap();
        let handle = writers.entry(format!("{server}/{character}")).or_insert_with(|| {
            let writer = self.open_writer(&file_path);
            let (current_date, current_size, max_size) = match &self.config.rotation {
                LogRotation::None | LogRotation::Daily => {
                    (Some(current_date_string()), 0, None)
                }
                LogRotation::BySize(max) => (None, 0, Some(*max)),
            };
            LogWriterHandle {
                writer,
                current_date,
                current_size,
                max_size,
            }
        });

        self.write_to_handle(handle, &file_path, character, server, event)
    }

    fn should_log_channel(&self, channel: &ChatChannel) -> bool {
        self.config.channels.is_empty() || self.config.channels.contains(channel)
    }

    fn resolve_log_path(&self, server: &str, character: &str) -> PathBuf {
        self.log_dir.join(format!("{server}_{character}.log"))
    }

    fn open_writer(&self, path: &Path) -> BufWriter<File> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap_or_else(|_| {
                let temp_dir = std::env::temp_dir().join("textquest");
                fs::create_dir_all(&temp_dir).expect("fallback log directory must be creatable");
                let temp = temp_dir.join("fallback.log");
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&temp)
                    .expect("fallback log must be openable")
            });
        BufWriter::new(file)
    }

    fn write_to_handle(
        &self,
        handle: &mut LogWriterHandle,
        file_path: &Path,
        character: &str,
        server: &str,
        event: &ChatEvent,
    ) -> io::Result<()> {
        let now = SystemTime::now();
        let timestamp = format_timestamp(&now);

        let line = match self.config.level {
            LogLevel::Info => format!(
                "[{timestamp}] [{channel}] {sender}: {message}\n",
                timestamp = timestamp,
                channel = format_channel_name(&event.channel),
                sender = event.sender,
                message = event.message
            ),
            LogLevel::Debug => format!(
                "[{timestamp}] [{channel}] [{server}/{character}] {sender}: {message}\n",
                timestamp = timestamp,
                channel = format_channel_name(&event.channel),
                server = server,
                character = character,
                sender = event.sender,
                message = event.message
            ),
        };

        let line_bytes = line.as_bytes();

        if self.should_rotate(handle, line_bytes.len() as u64) {
            drop(handle.writer.flush());
            handle.writer = self.open_writer(file_path);
            handle.current_size = 0;
            if matches!(self.config.rotation, LogRotation::Daily) {
                handle.current_date = Some(current_date_string());
            }
        }

        handle.writer.write_all(line_bytes)?;
        handle.writer.flush()?;
        handle.current_size += line_bytes.len() as u64;

        Ok(())
    }

    fn should_rotate(&self, handle: &LogWriterHandle, additional_bytes: u64) -> bool {
        match &self.config.rotation {
            LogRotation::None => false,
            LogRotation::Daily => {
                handle.current_date.as_ref() != Some(&current_date_string())
            }
            LogRotation::BySize(max) => {
                handle.current_size + additional_bytes > *max
            }
        }
    }

    pub fn flush_all(&self) -> io::Result<()> {
        let mut writers = self.writers.lock().unwrap();
        for handle in writers.values_mut() {
            handle.writer.flush()?;
        }
        Ok(())
    }

    pub fn close_character(&self, character: &str, server: &str) -> io::Result<()> {
        let key = format!("{server}/{character}");
        let mut writers = self.writers.lock().unwrap();
        if let Some(mut handle) = writers.remove(&key) {
            handle.writer.flush()?;
        }
        Ok(())
    }
}

fn format_timestamp(time: &SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(*time)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

fn current_date_string() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn format_channel_name(channel: &ChatChannel) -> &'static str {
    match channel {
        ChatChannel::Say => "say",
        ChatChannel::Tell => "tell",
        ChatChannel::TellOut => "tell_out",
        ChatChannel::Group => "group",
        ChatChannel::Guild => "guild",
        ChatChannel::Raid => "raid",
        ChatChannel::Shout => "shout",
        ChatChannel::Ooc => "ooc",
        ChatChannel::Auction => "auction",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_event(channel: ChatChannel) -> ChatEvent {
        ChatEvent {
            channel,
            sender: "TestSender".to_string(),
            message: "Test message".to_string(),
        }
    }

    #[test]
    fn test_write_event_creates_file() {
        let dir = tempdir().unwrap();
        let config = ChatLogConfig {
            enabled: true,
            rotation: LogRotation::None,
            level: LogLevel::Info,
            channels: vec![],
        };
        let writer = ChatLogWriter::new(dir.path().to_path_buf(), config).unwrap();
        let event = make_event(ChatChannel::Say);

        writer.write_event("MyChar", "MyServer", &event).unwrap();

        let log_path = dir.path().join("MyServer_MyChar.log");
        assert!(log_path.exists());
        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("TestSender"));
        assert!(content.contains("Test message"));
    }

    #[test]
    fn test_channel_filtering() {
        let dir = tempdir().unwrap();
        let config = ChatLogConfig {
            enabled: true,
            rotation: LogRotation::None,
            level: LogLevel::Info,
            channels: vec![ChatChannel::Say, ChatChannel::Tell],
        };
        let writer = ChatLogWriter::new(dir.path().to_path_buf(), config).unwrap();

        writer.write_event("MyChar", "MyServer", &make_event(ChatChannel::Say)).unwrap();
        writer.write_event("MyChar", "MyServer", &make_event(ChatChannel::Tell)).unwrap();
        writer.write_event("MyChar", "MyServer", &make_event(ChatChannel::Group)).unwrap();

        let log_path = dir.path().join("MyServer_MyChar.log");
        let content = fs::read_to_string(&log_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_disabled_writer_ignores_events() {
        let dir = tempdir().unwrap();
        let config = ChatLogConfig {
            enabled: false,
            rotation: LogRotation::None,
            level: LogLevel::Info,
            channels: vec![],
        };
        let writer = ChatLogWriter::new(dir.path().to_path_buf(), config).unwrap();
        let event = make_event(ChatChannel::Say);

        writer.write_event("MyChar", "MyServer", &event).unwrap();

        let log_path = dir.path().join("MyServer_MyChar.log");
        assert!(!log_path.exists());
    }

    #[test]
    fn test_size_rotation() {
        let dir = tempdir().unwrap();
        let config = ChatLogConfig {
            enabled: true,
            rotation: LogRotation::BySize(50),
            level: LogLevel::Info,
            channels: vec![],
        };
        let writer = ChatLogWriter::new(dir.path().to_path_buf(), config).unwrap();
        let event = ChatEvent {
            channel: ChatChannel::Say,
            sender: "X".to_string(),
            message: "Y".to_string(),
        };

        for i in 0..10 {
            let evt = ChatEvent {
                channel: ChatChannel::Say,
                sender: format!("Char{}", i),
                message: "A".repeat(10).to_string(),
            };
            writer.write_event("MyChar", "MyServer", &evt).unwrap();
        }

        writer.flush_all().unwrap();
    }

    #[test]
    fn test_timestamp_format() {
        let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1700000000);
        let ts = format_timestamp(&time);
        assert!(ts.contains('-'));
        assert!(ts.contains(':'));
    }
}
