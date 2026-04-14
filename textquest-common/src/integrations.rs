//! External integration infrastructure for notifications and alerts.
//!
//! Provides structured types for notification channels (Discord, Webhooks, Logs),
//! event types (Death, Stuck, ZoneFailed, etc.), and configuration loading from TOML.
//!
//! This module defines the contract for sending notifications to external systems
//! without implementing HTTP clients — that responsibility belongs to the integration
//! layer in `textquest` or `textquest-web`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Notification severity levels for filtering and routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    /// Informational event (group status, item found, etc.).
    Info,
    /// Warning (low health, long navigation, skill-up alert).
    Warning,
    /// Error condition requiring attention (stuck, zone failed, combat error).
    Error,
    /// Critical event (death, raid wipe, system failure).
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARNING"),
            Self::Error => write!(f, "ERROR"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Notification event types that can be sent to external integrations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationEvent {
    /// Character death or corpse recovery.
    Death,
    /// Character stuck (navmesh error, navigation timeout).
    Stuck,
    /// Zone transition failed (wall, quest barrier).
    ZoneFailed,
    /// Raid event alert (named spawn, critical event).
    RaidAlert,
    /// Economy-related alert (price shift, tradeskill batch).
    EconomyAlert,
    /// Low health warning (<30% mana or HP).
    LowHealth,
    /// Combat timeout or rotation error.
    CombatError,
    /// Multibox coordination event (group sync, wait signal).
    GroupSync,
    /// Skill up notification.
    SkillUp,
    /// Item looted or inventory event.
    LootEvent,
}

impl std::fmt::Display for NotificationEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Death => write!(f, "Death"),
            Self::Stuck => write!(f, "Stuck"),
            Self::ZoneFailed => write!(f, "Zone Failed"),
            Self::RaidAlert => write!(f, "Raid Alert"),
            Self::EconomyAlert => write!(f, "Economy Alert"),
            Self::LowHealth => write!(f, "Low Health"),
            Self::CombatError => write!(f, "Combat Error"),
            Self::GroupSync => write!(f, "Group Sync"),
            Self::SkillUp => write!(f, "Skill Up"),
            Self::LootEvent => write!(f, "Loot Event"),
        }
    }
}

/// Notification channel destination type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NotificationChannel {
    /// Discord webhook (configured with URL and optional role mention).
    Discord {
        webhook_url: String,
        mention_role_id: Option<String>,
    },
    /// Generic HTTP webhook.
    Webhook {
        url: String,
        headers: Option<HashMap<String, String>>,
    },
    /// Log file output (via tracing integration).
    Log,
}

impl NotificationChannel {
    /// Returns a human-readable name for this channel.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Discord { .. } => "Discord",
            Self::Webhook { .. } => "Webhook",
            Self::Log => "Log",
        }
    }
}

/// A single notification message to be sent to external systems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationMessage {
    /// Event type that triggered this notification.
    pub event: NotificationEvent,
    /// Severity level for routing and filtering.
    pub severity: Severity,
    /// ISO 8601 timestamp when the event occurred.
    pub timestamp: String,
    /// Character or entity name associated with the event.
    pub character_name: String,
    /// Zone short name (e.g., "qey2hh1").
    pub zone_name: String,
    /// Brief title or subject line.
    pub title: String,
    /// Detailed message body.
    pub details: String,
    /// Optional custom fields for extensibility.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl NotificationMessage {
    /// Create a new notification message.
    ///
    /// # Arguments
    ///
    /// * `event` - The type of event.
    /// * `severity` - The severity level.
    /// * `character_name` - Name of the affected character.
    /// * `zone_name` - Zone short name.
    /// * `title` - Brief subject line.
    /// * `details` - Detailed message body.
    #[must_use]
    pub fn new(
        event: NotificationEvent,
        severity: Severity,
        character_name: impl Into<String>,
        zone_name: impl Into<String>,
        title: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self {
            event,
            severity,
            timestamp: Self::current_timestamp(),
            character_name: character_name.into(),
            zone_name: zone_name.into(),
            title: title.into(),
            details: details.into(),
            metadata: HashMap::new(),
        }
    }

    /// Get the current timestamp in ISO 8601 format.
    /// Uses system time without external dependencies.
    fn current_timestamp() -> String {
        use std::time::SystemTime;

        match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(duration) => {
                let secs = duration.as_secs();
                let nanos = duration.subsec_nanos();

                // Simple ISO 8601-like format: YYYY-MM-DDTHH:MM:SS.sssZ
                // Calculate date components from unix timestamp
                let days_since_epoch = secs / 86400;
                let secs_today = secs % 86400;

                let hours = secs_today / 3600;
                let minutes = (secs_today % 3600) / 60;
                let seconds = secs_today % 60;
                let millis = nanos / 1_000_000;

                // Rough year calculation (good enough for logs)
                let mut year = 1970;
                let mut remaining_days = days_since_epoch;
                loop {
                    let days_in_year = if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                        366
                    } else {
                        365
                    };
                    if remaining_days < days_in_year as u64 {
                        break;
                    }
                    remaining_days -= days_in_year as u64;
                    year += 1;
                }

                // Month and day calculation
                let is_leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
                let days_in_months = if is_leap {
                    [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
                } else {
                    [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
                };

                let mut month = 1;
                let mut day_of_month = remaining_days as u32 + 1;
                for &days in &days_in_months {
                    if day_of_month <= days as u32 {
                        break;
                    }
                    day_of_month -= days as u32;
                    month += 1;
                }

                format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
                    year, month, day_of_month, hours, minutes, seconds, millis
                )
            }
            Err(_) => "1970-01-01T00:00:00.000Z".to_string(),
        }
    }

    /// Add a metadata field to this message.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Format this message as a plain text string (for logs or preview).
    #[must_use]
    pub fn format_plain(&self) -> String {
        format!(
            "[{}] {} | {} @ {} | {}\n{}",
            self.severity,
            self.event,
            self.character_name,
            self.zone_name,
            self.title,
            self.details
        )
    }

    /// Format this message for Discord markdown.
    #[must_use]
    pub fn format_discord(&self) -> String {
        let mut msg = format!(
            "**[{}] {}**\n```\nCharacter: {}\nZone: {}\nTime: {}\n```\n{}",
            self.severity,
            self.event,
            self.character_name,
            self.zone_name,
            self.timestamp,
            self.title
        );

        if !self.details.is_empty() {
            msg.push_str("\n\n");
            msg.push_str(&self.details);
        }

        if !self.metadata.is_empty() {
            msg.push_str("\n\n**Metadata:**");
            for (k, v) in &self.metadata {
                msg.push_str(&format!("\n• {}: {}", k, v));
            }
        }

        msg
    }
}

/// Configuration for a single notification channel and its filters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Channel destination (Discord, Webhook, or Log).
    #[serde(flatten)]
    pub channel: NotificationChannel,
    /// Minimum severity to send notifications on this channel.
    #[serde(default = "default_min_severity")]
    pub min_severity: Severity,
    /// Event types to include on this channel (if empty, all are included).
    #[serde(default)]
    pub event_filter: Vec<NotificationEvent>,
    /// Character names to include on this channel (if empty, all are included).
    #[serde(default)]
    pub character_filter: Vec<String>,
    /// Zone names to include on this channel (if empty, all are included).
    #[serde(default)]
    pub zone_filter: Vec<String>,
}

fn default_min_severity() -> Severity {
    Severity::Info
}

impl ChannelConfig {
    /// Check if this channel should receive a notification based on filters.
    #[must_use]
    pub fn matches(&self, msg: &NotificationMessage) -> bool {
        // Check severity
        if severity_level(msg.severity) < severity_level(self.min_severity) {
            return false;
        }

        // Check event filter
        if !self.event_filter.is_empty() && !self.event_filter.contains(&msg.event) {
            return false;
        }

        // Check character filter
        if !self.character_filter.is_empty() && !self.character_filter.contains(&msg.character_name)
        {
            return false;
        }

        // Check zone filter
        if !self.zone_filter.is_empty() && !self.zone_filter.contains(&msg.zone_name) {
            return false;
        }

        true
    }
}

fn severity_level(severity: Severity) -> u8 {
    match severity {
        Severity::Info => 0,
        Severity::Warning => 1,
        Severity::Error => 2,
        Severity::Critical => 3,
    }
}

/// Complete notification integration configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Enable/disable all notifications globally.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// List of configured notification channels.
    #[serde(default)]
    pub channels: Vec<ChannelConfig>,
}

fn default_true() -> bool {
    true
}

impl NotificationConfig {
    /// Load configuration from a TOML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed as TOML.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load configuration from a TOML string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string cannot be parsed as TOML.
    pub fn parse_toml(content: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config = toml::from_str(content)?;
        Ok(config)
    }

    /// Get all channels that match a notification message.
    #[must_use]
    pub fn matching_channels(&self, msg: &NotificationMessage) -> Vec<&NotificationChannel> {
        if !self.enabled {
            return Vec::new();
        }

        self.channels
            .iter()
            .filter(|ch| ch.matches(msg))
            .map(|ch| &ch.channel)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_message_creation() {
        let msg = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Critical,
            "TestChar",
            "qey2hh1",
            "Character Died",
            "Corpse at 10.5, 20.3, -5.2",
        );

        assert_eq!(msg.character_name, "TestChar");
        assert_eq!(msg.zone_name, "qey2hh1");
        assert_eq!(msg.event, NotificationEvent::Death);
        assert_eq!(msg.severity, Severity::Critical);
    }

    #[test]
    fn test_notification_message_metadata() {
        let msg = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Critical,
            "TestChar",
            "qey2hh1",
            "Died",
            "Details",
        )
        .with_metadata("killer", "SomeNPC")
        .with_metadata("exp_lost", "1000");

        assert_eq!(
            msg.metadata.get("killer").map(String::as_str),
            Some("SomeNPC")
        );
        assert_eq!(
            msg.metadata.get("exp_lost").map(String::as_str),
            Some("1000")
        );
    }

    #[test]
    fn test_notification_message_format_plain() {
        let msg = NotificationMessage::new(
            NotificationEvent::LowHealth,
            Severity::Warning,
            "TestChar",
            "crushbone",
            "Low Mana",
            "Mana below 25%",
        );

        let plain = msg.format_plain();
        assert!(plain.contains("WARNING"));
        assert!(plain.contains("Low Health"));
        assert!(plain.contains("TestChar"));
        assert!(plain.contains("crushbone"));
        assert!(plain.contains("Low Mana"));
        assert!(plain.contains("Mana below 25%"));
    }

    #[test]
    fn test_notification_message_format_discord() {
        let msg = NotificationMessage::new(
            NotificationEvent::RaidAlert,
            Severity::Critical,
            "GroupLeader",
            "sebilus",
            "Named Spawn",
            "High-value target detected",
        )
        .with_metadata("mob_name", "Queen of Sebilis");

        let discord = msg.format_discord();
        assert!(discord.contains("**[CRITICAL]"));
        assert!(discord.contains("Raid Alert"));
        assert!(discord.contains("GroupLeader"));
        assert!(discord.contains("sebilus"));
        assert!(discord.contains("Queen of Sebilis"));
    }

    #[test]
    fn test_channel_config_severity_filter() {
        let config = ChannelConfig {
            channel: NotificationChannel::Log,
            min_severity: Severity::Warning,
            event_filter: Vec::new(),
            character_filter: Vec::new(),
            zone_filter: Vec::new(),
        };

        let info_msg = NotificationMessage::new(
            NotificationEvent::LootEvent,
            Severity::Info,
            "Char",
            "zone",
            "Found item",
            "",
        );
        assert!(!config.matches(&info_msg));

        let warning_msg = NotificationMessage::new(
            NotificationEvent::LowHealth,
            Severity::Warning,
            "Char",
            "zone",
            "Low HP",
            "",
        );
        assert!(config.matches(&warning_msg));

        let critical_msg = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Critical,
            "Char",
            "zone",
            "Died",
            "",
        );
        assert!(config.matches(&critical_msg));
    }

    #[test]
    fn test_channel_config_event_filter() {
        let config = ChannelConfig {
            channel: NotificationChannel::Log,
            min_severity: Severity::Info,
            event_filter: vec![NotificationEvent::Death, NotificationEvent::RaidAlert],
            character_filter: Vec::new(),
            zone_filter: Vec::new(),
        };

        let death_msg = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Critical,
            "Char",
            "zone",
            "Died",
            "",
        );
        assert!(config.matches(&death_msg));

        let raid_msg = NotificationMessage::new(
            NotificationEvent::RaidAlert,
            Severity::Warning,
            "Char",
            "zone",
            "Raid event",
            "",
        );
        assert!(config.matches(&raid_msg));

        let loot_msg = NotificationMessage::new(
            NotificationEvent::LootEvent,
            Severity::Info,
            "Char",
            "zone",
            "Found item",
            "",
        );
        assert!(!config.matches(&loot_msg));
    }

    #[test]
    fn test_channel_config_character_filter() {
        let config = ChannelConfig {
            channel: NotificationChannel::Log,
            min_severity: Severity::Info,
            event_filter: Vec::new(),
            character_filter: vec!["Warrior".to_string(), "Cleric".to_string()],
            zone_filter: Vec::new(),
        };

        let warrior_msg = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Error,
            "Warrior",
            "zone",
            "Died",
            "",
        );
        assert!(config.matches(&warrior_msg));

        let mage_msg = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Error,
            "Mage",
            "zone",
            "Died",
            "",
        );
        assert!(!config.matches(&mage_msg));
    }

    #[test]
    fn test_channel_config_zone_filter() {
        let config = ChannelConfig {
            channel: NotificationChannel::Log,
            min_severity: Severity::Info,
            event_filter: Vec::new(),
            character_filter: Vec::new(),
            zone_filter: vec!["sebilus".to_string(), "nagafen".to_string()],
        };

        let sebilus_msg = NotificationMessage::new(
            NotificationEvent::RaidAlert,
            Severity::Warning,
            "Char",
            "sebilus",
            "Named",
            "",
        );
        assert!(config.matches(&sebilus_msg));

        let crushbone_msg = NotificationMessage::new(
            NotificationEvent::RaidAlert,
            Severity::Warning,
            "Char",
            "crushbone",
            "Named",
            "",
        );
        assert!(!config.matches(&crushbone_msg));
    }

    #[test]
    fn test_notification_config_parse() {
        let toml_str = r#"
enabled = true

[[channels]]
type = "log"
min_severity = "WARNING"
event_filter = ["death", "raid_alert"]
character_filter = ["Warrior", "Cleric"]

[[channels]]
type = "discord"
webhook_url = "https://discord.com/api/webhooks/123/abc"
mention_role_id = "456"
min_severity = "CRITICAL"
"#;

        let config = NotificationConfig::parse_toml(toml_str).expect("Failed to parse config");
        assert!(config.enabled);
        assert_eq!(config.channels.len(), 2);

        // Check first channel (Log)
        assert_eq!(config.channels[0].min_severity, Severity::Warning);
        assert_eq!(config.channels[0].event_filter.len(), 2);
        assert_eq!(config.channels[0].character_filter.len(), 2);

        // Check second channel (Discord)
        assert!(matches!(
            &config.channels[1].channel,
            NotificationChannel::Discord { webhook_url, .. } if webhook_url == "https://discord.com/api/webhooks/123/abc"
        ));
        assert_eq!(config.channels[1].min_severity, Severity::Critical);
    }

    #[test]
    fn test_notification_config_matching() {
        let config = NotificationConfig {
            enabled: true,
            channels: vec![
                ChannelConfig {
                    channel: NotificationChannel::Log,
                    min_severity: Severity::Warning,
                    event_filter: vec![NotificationEvent::Death, NotificationEvent::RaidAlert],
                    character_filter: vec!["Warrior".to_string()],
                    zone_filter: Vec::new(),
                },
                ChannelConfig {
                    channel: NotificationChannel::Discord {
                        webhook_url: "https://example.com/hook".to_string(),
                        mention_role_id: None,
                    },
                    min_severity: Severity::Critical,
                    event_filter: Vec::new(),
                    character_filter: Vec::new(),
                    zone_filter: Vec::new(),
                },
            ],
        };

        let warrior_death = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Critical,
            "Warrior",
            "zone",
            "Died",
            "",
        );

        let matching = config.matching_channels(&warrior_death);
        assert_eq!(matching.len(), 2); // Matches both channels

        let mage_death = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Critical,
            "Mage",
            "zone",
            "Died",
            "",
        );

        let matching = config.matching_channels(&mage_death);
        assert_eq!(matching.len(), 1); // Only Discord (critical severity)
    }

    #[test]
    fn test_notification_config_disabled() {
        let config = NotificationConfig {
            enabled: false,
            channels: vec![ChannelConfig {
                channel: NotificationChannel::Log,
                min_severity: Severity::Info,
                event_filter: Vec::new(),
                character_filter: Vec::new(),
                zone_filter: Vec::new(),
            }],
        };

        let msg = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Critical,
            "Char",
            "zone",
            "Died",
            "",
        );

        let matching = config.matching_channels(&msg);
        assert!(matching.is_empty());
    }

    #[test]
    fn test_notification_channel_names() {
        let log = NotificationChannel::Log;
        assert_eq!(log.name(), "Log");

        let discord = NotificationChannel::Discord {
            webhook_url: "http://example.com".to_string(),
            mention_role_id: None,
        };
        assert_eq!(discord.name(), "Discord");

        let webhook = NotificationChannel::Webhook {
            url: "http://example.com".to_string(),
            headers: None,
        };
        assert_eq!(webhook.name(), "Webhook");
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Info.to_string(), "INFO");
        assert_eq!(Severity::Warning.to_string(), "WARNING");
        assert_eq!(Severity::Error.to_string(), "ERROR");
        assert_eq!(Severity::Critical.to_string(), "CRITICAL");
    }

    #[test]
    fn test_notification_event_display() {
        assert_eq!(NotificationEvent::Death.to_string(), "Death");
        assert_eq!(NotificationEvent::RaidAlert.to_string(), "Raid Alert");
        assert_eq!(NotificationEvent::LowHealth.to_string(), "Low Health");
    }

    #[test]
    fn test_notification_event_display_all_variants() {
        assert_eq!(NotificationEvent::Stuck.to_string(), "Stuck");
        assert_eq!(NotificationEvent::ZoneFailed.to_string(), "Zone Failed");
        assert_eq!(NotificationEvent::EconomyAlert.to_string(), "Economy Alert");
        assert_eq!(NotificationEvent::CombatError.to_string(), "Combat Error");
        assert_eq!(NotificationEvent::GroupSync.to_string(), "Group Sync");
        assert_eq!(NotificationEvent::SkillUp.to_string(), "Skill Up");
        assert_eq!(NotificationEvent::LootEvent.to_string(), "Loot Event");
    }

    #[test]
    fn test_format_discord_empty_details_omits_detail_block() {
        let msg = NotificationMessage::new(
            NotificationEvent::GroupSync,
            Severity::Info,
            "Leader",
            "crushbone",
            "Group sync",
            "",
        );
        let discord = msg.format_discord();
        assert!(discord.contains("**[INFO]"));
        assert!(discord.contains("Group Sync"));
        // Empty details → no trailing blank line + detail text appended
        assert!(!discord.ends_with("\n\n"));
    }

    #[test]
    fn test_format_discord_with_no_metadata_omits_metadata_block() {
        let msg = NotificationMessage::new(
            NotificationEvent::SkillUp,
            Severity::Info,
            "Ranger",
            "fieldofbone",
            "Archery up",
            "Skill increased to 150",
        );
        let discord = msg.format_discord();
        assert!(!discord.contains("**Metadata:**"));
        assert!(discord.contains("Archery up"));
        assert!(discord.contains("Skill increased to 150"));
    }

    #[test]
    fn test_format_discord_with_mention_role_and_metadata() {
        let msg = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Critical,
            "Paladin",
            "gukbottom",
            "Died",
            "CR needed at -200, 300",
        )
        .with_metadata("x", "-200")
        .with_metadata("y", "300");

        let discord = msg.format_discord();
        assert!(discord.contains("**[CRITICAL]"));
        assert!(discord.contains("**Metadata:**"));
        assert!(discord.contains("x"));
        assert!(discord.contains("y"));
    }

    #[test]
    fn test_parse_toml_disabled_config() {
        let toml_str = r#"
enabled = false
"#;
        let config = NotificationConfig::parse_toml(toml_str).expect("parse failed");
        assert!(!config.enabled);
        assert!(config.channels.is_empty());
    }

    #[test]
    fn test_parse_toml_webhook_with_headers() {
        let toml_str = r#"
enabled = true

[[channels]]
type = "webhook"
url = "https://example.com/hook"
min_severity = "ERROR"

[channels.headers]
Authorization = "Bearer secret"
X-Custom = "value"
"#;
        let config = NotificationConfig::parse_toml(toml_str).expect("parse failed");
        assert_eq!(config.channels.len(), 1);
        assert!(matches!(
            &config.channels[0].channel,
            NotificationChannel::Webhook { url, headers: Some(h) }
                if url == "https://example.com/hook" && h.contains_key("Authorization")
        ));
        assert_eq!(config.channels[0].min_severity, Severity::Error);
    }

    #[test]
    fn test_parse_toml_invalid_returns_error() {
        let result = NotificationConfig::parse_toml("this is [not valid toml @@@@");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_file_reads_toml_from_disk() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
        writeln!(
            tmp,
            r#"enabled = true
[[channels]]
type = "log"
min_severity = "INFO"
"#
        )
        .expect("write");
        let config = NotificationConfig::from_file(tmp.path()).expect("from_file failed");
        assert!(config.enabled);
        assert_eq!(config.channels.len(), 1);
        assert!(matches!(&config.channels[0].channel, NotificationChannel::Log));
    }

    #[test]
    fn test_from_file_missing_path_returns_error() {
        let result = NotificationConfig::from_file("/tmp/textquest_nonexistent_xyz.toml");
        assert!(result.is_err());
    }

    #[test]
    fn test_channel_config_matches_combined_filters() {
        let config = ChannelConfig {
            channel: NotificationChannel::Log,
            min_severity: Severity::Warning,
            event_filter: vec![NotificationEvent::Death],
            character_filter: vec!["Tank".to_string()],
            zone_filter: vec!["gukbottom".to_string()],
        };

        // All conditions satisfied → match
        let ok = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Critical,
            "Tank",
            "gukbottom",
            "Died",
            "",
        );
        assert!(config.matches(&ok));

        // Wrong event → no match
        let wrong_event = NotificationMessage::new(
            NotificationEvent::Stuck,
            Severity::Critical,
            "Tank",
            "gukbottom",
            "Stuck",
            "",
        );
        assert!(!config.matches(&wrong_event));

        // Wrong character → no match
        let wrong_char = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Critical,
            "Healer",
            "gukbottom",
            "Died",
            "",
        );
        assert!(!config.matches(&wrong_char));

        // Wrong zone → no match
        let wrong_zone = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Critical,
            "Tank",
            "crushbone",
            "Died",
            "",
        );
        assert!(!config.matches(&wrong_zone));

        // Severity too low → no match
        let low_sev = NotificationMessage::new(
            NotificationEvent::Death,
            Severity::Info,
            "Tank",
            "gukbottom",
            "Died",
            "",
        );
        assert!(!config.matches(&low_sev));
    }

    #[test]
    fn test_severity_error_level_filtering() {
        let config = ChannelConfig {
            channel: NotificationChannel::Log,
            min_severity: Severity::Error,
            event_filter: Vec::new(),
            character_filter: Vec::new(),
            zone_filter: Vec::new(),
        };

        let info = NotificationMessage::new(
            NotificationEvent::LootEvent, Severity::Info, "C", "z", "t", "",
        );
        let warning = NotificationMessage::new(
            NotificationEvent::LootEvent, Severity::Warning, "C", "z", "t", "",
        );
        let error = NotificationMessage::new(
            NotificationEvent::LootEvent, Severity::Error, "C", "z", "t", "",
        );
        let critical = NotificationMessage::new(
            NotificationEvent::LootEvent, Severity::Critical, "C", "z", "t", "",
        );

        assert!(!config.matches(&info));
        assert!(!config.matches(&warning));
        assert!(config.matches(&error));
        assert!(config.matches(&critical));
    }

    #[test]
    fn test_notification_config_default_is_enabled() {
        let config: NotificationConfig = toml::from_str("").expect("parse empty");
        assert!(config.enabled);
        assert!(config.channels.is_empty());
    }
}
