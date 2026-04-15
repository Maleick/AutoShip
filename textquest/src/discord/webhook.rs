//! Discord webhook sender — POST alerts to per-category Discord channels.
//!
//! Routes alerts to different webhook URLs based on event category.
//! Falls back to a default webhook URL when no category-specific channel is
//! configured.

use std::{collections::HashMap, sync::mpsc, thread};

/// Event categories for channel routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventCategory {
    /// Boss kills, wipes, DPS charts.
    Kills,
    /// Item drops, wishlists.
    Loot,
    /// DZ lockouts, respawn timers.
    Timers,
    /// Achievements, milestones.
    Feats,
    /// Camp started, login complete, client status.
    Status,
}

impl EventCategory {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Kills => "kills",
            Self::Loot => "loot",
            Self::Timers => "timers",
            Self::Feats => "feats",
            Self::Status => "status",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            EventCategory::Kills => "\u{2694}\u{FE0F}", // ⚔️
            EventCategory::Loot => "\u{1F4E6}",         // 📦
            EventCategory::Timers => "\u{23F0}",        // ⏰
            EventCategory::Feats => "\u{1F3C6}",        // 🏆
            EventCategory::Status => "\u{1F4E1}",       // 📡
        }
    }
}

/// Alert severity level — controls embed color in Discord.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum AlertLevel {
    /// Green — informational (client logged in, camp started)
    Info,
    /// Yellow — warning (client disconnected, retry)
    Warning,
    /// Red — critical (mass failure, HVT spotted, client crashed)
    Critical,
}

impl AlertLevel {
    fn color(&self) -> u32 {
        match self {
            AlertLevel::Info => 0x002E_CC71,
            AlertLevel::Warning => 0x00F1_C40F,
            AlertLevel::Critical => 0x00E7_4C3C,
        }
    }
}

/// An embed field for rich Discord messages.
#[derive(Debug, Clone)]
pub struct EmbedField {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

/// A Discord alert to be sent via webhook.
#[derive(Debug, Clone)]
pub struct DiscordAlert {
    /// Alert title (shown as embed title in Discord).
    pub title: String,
    /// Alert body text.
    pub message: String,
    /// Severity level controlling the embed color.
    pub level: AlertLevel,
    /// Event category for channel routing.
    pub category: EventCategory,
    /// Optional embed fields for rich formatting.
    pub fields: Vec<EmbedField>,
}

impl DiscordAlert {
    /// Create a simple alert with no extra fields.
    #[must_use]
    pub fn simple(
        title: impl Into<String>,
        message: impl Into<String>,
        level: AlertLevel,
        category: EventCategory,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            level,
            category,
            fields: Vec::new(),
        }
    }

    /// Add a field to the embed.
    #[must_use]
    pub fn with_field(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
        inline: bool,
    ) -> Self {
        self.fields.push(EmbedField {
            name: name.into(),
            value: value.into(),
            inline,
        });
        self
    }
}

/// Channel routing configuration — maps categories to webhook URLs.
#[derive(Debug, Clone)]
struct ChannelRouter {
    default_url: Option<String>,
    category_urls: HashMap<String, String>,
}

impl ChannelRouter {
    fn new(default_url: String, channels: HashMap<String, String>) -> Self {
        Self {
            default_url: if default_url.is_empty() {
                None
            } else {
                Some(default_url)
            },
            category_urls: channels,
        }
    }

    fn url_for(&self, category: EventCategory) -> Option<&str> {
        self.category_urls
            .get(category.as_str())
            .map(String::as_str)
            .or(self.default_url.as_deref())
    }
}

/// Async webhook sender — queues alerts and sends them on a background thread.
#[allow(dead_code)]
pub struct WebhookSender {
    tx: mpsc::Sender<DiscordAlert>,
    _handle: thread::JoinHandle<()>,
}

#[allow(dead_code)]
impl WebhookSender {
    /// Create a new webhook sender with a single default URL.
    #[must_use]
    pub fn new(webhook_url: String) -> Self {
        Self::with_channels(webhook_url, HashMap::new())
    }

    /// Create a webhook sender with per-category channel routing.
    #[must_use]
    pub fn with_channels(default_url: String, channels: HashMap<String, String>) -> Self {
        let (tx, rx) = mpsc::channel::<DiscordAlert>();
        let router = ChannelRouter::new(default_url, channels);

        let handle = thread::Builder::new()
            .name("discord-webhook".into())
            .spawn(move || {
                Self::sender_loop(&router, rx);
            })
            .expect("failed to spawn discord webhook thread");

        Self {
            tx,
            _handle: handle,
        }
    }

    /// Queue an alert for delivery. Non-blocking — returns immediately.
    pub fn send(&self, alert: DiscordAlert) {
        if let Err(e) = self.tx.send(alert) {
            tracing::warn!("Discord webhook queue full or closed: {}", e);
        }
    }

    /// Convenience: send an info-level alert to the status channel.
    pub fn info(&self, title: &str, message: &str) {
        self.send(DiscordAlert::simple(
            title,
            message,
            AlertLevel::Info,
            EventCategory::Status,
        ));
    }

    /// Convenience: send a warning-level alert to the status channel.
    pub fn warn(&self, title: &str, message: &str) {
        self.send(DiscordAlert::simple(
            title,
            message,
            AlertLevel::Warning,
            EventCategory::Status,
        ));
    }

    /// Convenience: send a critical-level alert to the status channel.
    pub fn critical(&self, title: &str, message: &str) {
        self.send(DiscordAlert::simple(
            title,
            message,
            AlertLevel::Critical,
            EventCategory::Status,
        ));
    }

    /// Convenience: send a kill report to the kills channel.
    pub fn kill(&self, mob_name: &str, dps_summary: &[(String, i64)]) {
        let mut alert = DiscordAlert::simple(
            format!("{} {} Killed!", EventCategory::Kills.emoji(), mob_name),
            "",
            AlertLevel::Info,
            EventCategory::Kills,
        );
        for (i, (character, dps)) in dps_summary.iter().enumerate() {
            let rank = i + 1;
            alert = alert.with_field(format!("#{rank} {character}"), format!("{dps} DPS"), true);
        }
        self.send(alert);
    }

    /// Convenience: send a loot drop notification.
    pub fn loot(&self, item_name: &str, looter: &str, mob_name: Option<&str>) {
        let desc = match mob_name {
            Some(mob) => format!("**{looter}** looted from *{mob}*"),
            None => format!("**{looter}** picked up"),
        };
        self.send(DiscordAlert::simple(
            format!("{} {item_name}", EventCategory::Loot.emoji()),
            desc,
            AlertLevel::Info,
            EventCategory::Loot,
        ));
    }

    /// Convenience: send a lockout timer notification.
    pub fn lockout(&self, zone_name: &str, character: &str, expires_at: &str) {
        self.send(
            DiscordAlert::simple(
                format!("{} DZ Lockout: {zone_name}", EventCategory::Timers.emoji()),
                "",
                AlertLevel::Warning,
                EventCategory::Timers,
            )
            .with_field("Character", character, true)
            .with_field("Expires", expires_at, true),
        );
    }

    /// Convenience: send an achievement notification.
    pub fn feat(&self, title: &str, description: &str) {
        self.send(DiscordAlert::simple(
            format!("{} {title}", EventCategory::Feats.emoji()),
            description,
            AlertLevel::Info,
            EventCategory::Feats,
        ));
    }

    fn sender_loop(router: &ChannelRouter, rx: mpsc::Receiver<DiscordAlert>) {
        let client = reqwest::blocking::Client::new();

        while let Ok(alert) = rx.recv() {
            let Some(url) = router.url_for(alert.category) else {
                tracing::debug!(
                    category = alert.category.as_str(),
                    title = %alert.title,
                    "No webhook URL configured for category, skipping"
                );
                continue;
            };

            let fields_json: Vec<serde_json::Value> = alert
                .fields
                .iter()
                .map(|f| {
                    serde_json::json!({
                        "name": f.name,
                        "value": f.value,
                        "inline": f.inline,
                    })
                })
                .collect();

            let payload = serde_json::json!({
                "embeds": [{
                    "title": alert.title,
                    "description": alert.message,
                    "color": alert.level.color(),
                    "fields": fields_json,
                    "footer": {
                        "text": format!("TextQuest • {}", alert.category.as_str())
                    },
                    "timestamp": chrono_now_iso()
                }]
            });

            match client.post(url).json(&payload).send() {
                Ok(resp) if resp.status().is_success() => {
                    tracing::debug!(
                        title = %alert.title,
                        category = alert.category.as_str(),
                        "Discord alert sent"
                    );
                }
                Ok(resp) => {
                    tracing::warn!(
                        status = %resp.status(),
                        title = %alert.title,
                        "Discord webhook returned non-success"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        title = %alert.title,
                        "Discord webhook POST failed"
                    );
                }
            }
        }

        tracing::info!("Discord webhook sender shutting down");
    }
}

/// Get current time as ISO 8601 string (no chrono dependency).
fn chrono_now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    let year = 1970 + days / 365;
    let day_of_year = days % 365;
    let month = day_of_year / 30 + 1;
    let day = day_of_year % 30 + 1;
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_level_colors() {
        assert_eq!(AlertLevel::Info.color(), 0x2ECC71);
        assert_eq!(AlertLevel::Warning.color(), 0xF1C40F);
        assert_eq!(AlertLevel::Critical.color(), 0xE74C3C);
    }

    #[test]
    fn chrono_now_iso_format() {
        let ts = chrono_now_iso();
        assert!(ts.ends_with('Z'));
        assert!(ts.contains('T'));
        assert_eq!(ts.len(), 20);
    }

    #[test]
    fn discord_alert_constructible() {
        let alert = DiscordAlert::simple("Test", "Hello", AlertLevel::Info, EventCategory::Status);
        assert_eq!(alert.title, "Test");
        assert_eq!(alert.category, EventCategory::Status);
        assert!(alert.fields.is_empty());
    }

    #[test]
    fn alert_with_fields() {
        let alert = DiscordAlert::simple(
            "Kill",
            "Nagafen down",
            AlertLevel::Info,
            EventCategory::Kills,
        )
        .with_field("Top DPS", "Wizard01 — 5000", true)
        .with_field("Duration", "2m 30s", true);
        assert_eq!(alert.fields.len(), 2);
        assert_eq!(alert.fields[0].name, "Top DPS");
        assert!(alert.fields[0].inline);
    }

    #[test]
    fn event_category_roundtrip() {
        for cat in [
            EventCategory::Kills,
            EventCategory::Loot,
            EventCategory::Timers,
            EventCategory::Feats,
            EventCategory::Status,
        ] {
            assert!(!cat.as_str().is_empty());
            assert!(!cat.emoji().is_empty());
        }
    }

    #[test]
    fn channel_router_default_fallback() {
        let router = ChannelRouter::new("https://default.example.com".into(), HashMap::new());
        assert_eq!(
            router.url_for(EventCategory::Kills),
            Some("https://default.example.com")
        );
    }

    #[test]
    fn channel_router_category_override() {
        let mut channels = HashMap::new();
        channels.insert("kills".into(), "https://kills.example.com".into());
        let router = ChannelRouter::new("https://default.example.com".into(), channels);
        assert_eq!(
            router.url_for(EventCategory::Kills),
            Some("https://kills.example.com")
        );
        assert_eq!(
            router.url_for(EventCategory::Loot),
            Some("https://default.example.com")
        );
    }

    #[test]
    fn channel_router_no_default_no_channel() {
        let router = ChannelRouter::new(String::new(), HashMap::new());
        assert_eq!(router.url_for(EventCategory::Kills), None);
    }

    #[test]
    fn channel_router_no_default_with_channel() {
        let mut channels = HashMap::new();
        channels.insert("loot".into(), "https://loot.example.com".into());
        let router = ChannelRouter::new(String::new(), channels);
        assert_eq!(
            router.url_for(EventCategory::Loot),
            Some("https://loot.example.com")
        );
        assert_eq!(router.url_for(EventCategory::Kills), None);
    }
}
