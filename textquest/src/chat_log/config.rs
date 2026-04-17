//! Chat logging configuration types.

use serde::{Deserialize, Serialize};

impl From<textquest_common::chat::ChatChannel> for ChatChannel {
    fn from(other: textquest_common::chat::ChatChannel) -> Self {
        match other {
            textquest_common::chat::ChatChannel::Say => ChatChannel::Say,
            textquest_common::chat::ChatChannel::Tell => ChatChannel::Tell,
            textquest_common::chat::ChatChannel::TellOut => ChatChannel::Tell,
            textquest_common::chat::ChatChannel::Group => ChatChannel::Group,
            textquest_common::chat::ChatChannel::Guild => ChatChannel::Guild,
            textquest_common::chat::ChatChannel::Raid => ChatChannel::Raid,
            textquest_common::chat::ChatChannel::Shout => ChatChannel::Shout,
            textquest_common::chat::ChatChannel::Ooc => ChatChannel::Ooc,
            textquest_common::chat::ChatChannel::Auction => ChatChannel::Auction,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatChannel {
    Say,
    Tell,
    Group,
    Raid,
    Guild,
    Ooc,
    Shout,
    Auction,
    Shout2,
    Pet,
    Spontaneous,
    Mpets,
    MQ2,
}

impl ChatChannel {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "say" => Some(Self::Say),
            "tell" => Some(Self::Tell),
            "group" => Some(Self::Group),
            "raid" => Some(Self::Raid),
            "guild" => Some(Self::Guild),
            "ooc" => Some(Self::Ooc),
            "shout" => Some(Self::Shout),
            "auction" => Some(Self::Auction),
            "shout2" => Some(Self::Shout2),
            "pet" => Some(Self::Pet),
            "spontaneous" => Some(Self::Spontaneous),
            "mpets" => Some(Self::Mpets),
            "mq2" => Some(Self::MQ2),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Say => "say",
            Self::Tell => "tell",
            Self::Group => "group",
            Self::Raid => "raid",
            Self::Guild => "guild",
            Self::Ooc => "ooc",
            Self::Shout => "shout",
            Self::Auction => "auction",
            Self::Shout2 => "shout2",
            Self::Pet => "pet",
            Self::Spontaneous => "spontaneous",
            Self::Mpets => "mpets",
            Self::MQ2 => "mq2",
        }
    }
}

impl std::fmt::Display for ChatChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "trace" => Some(Self::Trace),
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" => Some(Self::Warn),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RotationStrategy {
    Daily,
    Size(u64),
    None,
}

impl Default for RotationStrategy {
    fn default() -> Self {
        Self::Size(10 * 1024 * 1024)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatLogConfig {
    pub enabled: bool,
    pub channels: Vec<ChatChannel>,
    pub rotation_strategy: RotationStrategy,
    pub max_file_size_bytes: u64,
    pub min_level: LogLevel,
    pub log_eq_chat: bool,
}

impl Default for ChatLogConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            channels: vec![ChatChannel::MQ2],
            rotation_strategy: RotationStrategy::default(),
            max_file_size_bytes: 10 * 1024 * 1024,
            min_level: LogLevel::Info,
            log_eq_chat: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PerCharacterChatLogConfig {
    pub character_name: String,
    pub enabled: bool,
    pub channels: Vec<ChatChannel>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_channel_from_str() {
        assert_eq!(ChatChannel::parse("say"), Some(ChatChannel::Say));
        assert_eq!(ChatChannel::parse("SAY"), Some(ChatChannel::Say));
        assert_eq!(ChatChannel::parse("guild"), Some(ChatChannel::Guild));
        assert_eq!(ChatChannel::parse("unknown"), None);
    }

    #[test]
    fn chat_channel_display() {
        assert_eq!(ChatChannel::Say.to_string(), "say");
        assert_eq!(ChatChannel::MQ2.to_string(), "mq2");
    }

    #[test]
    fn log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn log_level_from_str() {
        assert_eq!(LogLevel::parse("debug"), Some(LogLevel::Debug));
        assert_eq!(LogLevel::parse("INFO"), Some(LogLevel::Info));
        assert_eq!(LogLevel::parse("invalid"), None);
    }

    #[test]
    fn default_chat_log_config() {
        let config = ChatLogConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.channels, vec![ChatChannel::MQ2]);
        assert!(matches!(
            config.rotation_strategy,
            RotationStrategy::Size(_)
        ));
        assert_eq!(config.min_level, LogLevel::Info);
        assert!(!config.log_eq_chat);
    }

    #[test]
    fn chat_log_config_deserializes() {
        let toml_str = r#"
            enabled = true
            channels = ["say", "group", "guild"]
            rotation_strategy = "daily"
            min_level = "debug"
            log_eq_chat = true
        "#;
        let config: ChatLogConfig = toml::from_str(toml_str).unwrap();
        assert!(config.enabled);
        assert!(config.channels.contains(&ChatChannel::Say));
        assert!(config.channels.contains(&ChatChannel::Group));
        assert!(config.channels.contains(&ChatChannel::Guild));
        assert!(matches!(config.rotation_strategy, RotationStrategy::Daily));
        assert_eq!(config.min_level, LogLevel::Debug);
        assert!(config.log_eq_chat);
    }

    #[test]
    fn rotation_strategy_size_deserializes() {
        let toml_str = r#"
            enabled = true
            rotation_strategy = { size = 5242880 }
        "#;
        let config: ChatLogConfig = toml::from_str(toml_str).unwrap();
        assert!(matches!(
            config.rotation_strategy,
            RotationStrategy::Size(5242880)
        ));
    }

    #[test]
    fn rotation_strategy_none_deserializes() {
        let toml_str = r#"
            enabled = true
            rotation_strategy = "none"
        "#;
        let config: ChatLogConfig = toml::from_str(toml_str).unwrap();
        assert!(matches!(config.rotation_strategy, RotationStrategy::None));
    }
}
