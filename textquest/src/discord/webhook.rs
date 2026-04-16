//! Discord webhook sender — POST alerts to per-category Discord channels.
//!
//! Routes alerts to different webhook URLs based on event category.
//! Falls back to a default webhook URL when no category-specific channel is
//! configured. Supports per-alert-type routing overrides, rich embeds or plain
//! text payloads, and Discord-safe per-webhook rate limiting.

use std::{
    collections::{HashMap, VecDeque},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use textquest_common::integrations::{
    DiscordMentionPolicy, DiscordMessageMode, DiscordRouteConfig, Severity,
};

const DISCORD_WEBHOOK_LIMIT_PER_MINUTE: usize = 30;
const DISCORD_WEBHOOK_WINDOW_MS: u64 = 60_000;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AlertLevel {
    /// Green — informational (client logged in, camp started)
    Info,
    /// Yellow — warning (client disconnected, retry)
    Warning,
    /// Orange — error requiring operator attention.
    Error,
    /// Red — critical (mass failure, HVT spotted, client crashed)
    Critical,
}

impl AlertLevel {
    fn color(&self) -> u32 {
        match self {
            AlertLevel::Info => 0x002E_CC71,
            AlertLevel::Warning => 0x00F1_C40F,
            AlertLevel::Error => 0x00E6_7E22,
            AlertLevel::Critical => 0x00E7_4C3C,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            AlertLevel::Info => "INFO",
            AlertLevel::Warning => "WARNING",
            AlertLevel::Error => "ERROR",
            AlertLevel::Critical => "CRITICAL",
        }
    }
}

impl From<Severity> for AlertLevel {
    fn from(value: Severity) -> Self {
        match value {
            Severity::Info => AlertLevel::Info,
            Severity::Warning => AlertLevel::Warning,
            Severity::Error => AlertLevel::Error,
            Severity::Critical => AlertLevel::Critical,
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
    /// Optional alert-type routing key (for example `"death"` or `"status"`).
    pub route_key: Option<String>,
    /// Default payload format when no route override exists.
    pub message_mode: DiscordMessageMode,
    /// Default mention policy when no route override exists.
    pub mention_policy: DiscordMentionPolicy,
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
            route_key: None,
            message_mode: DiscordMessageMode::RichEmbed,
            mention_policy: DiscordMentionPolicy::None,
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

    /// Route this alert through a named notification route.
    #[must_use]
    pub fn with_route_key(mut self, route_key: impl Into<String>) -> Self {
        self.route_key = Some(route_key.into());
        self
    }

    /// Override the default message mode for this alert.
    #[must_use]
    pub fn with_message_mode(mut self, message_mode: DiscordMessageMode) -> Self {
        self.message_mode = message_mode;
        self
    }

    /// Override the default mention policy for this alert.
    #[must_use]
    pub fn with_mention_policy(mut self, mention_policy: DiscordMentionPolicy) -> Self {
        self.mention_policy = mention_policy;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedTarget {
    url: String,
    message_mode: DiscordMessageMode,
    mention_policy: DiscordMentionPolicy,
    level: AlertLevel,
}

/// Channel routing configuration — maps categories and named routes to webhook
/// URLs.
#[derive(Debug, Clone)]
struct ChannelRouter {
    default_url: Option<String>,
    category_urls: HashMap<String, String>,
    notification_routes: HashMap<String, DiscordRouteConfig>,
}

impl ChannelRouter {
    fn new(
        default_url: String,
        channels: HashMap<String, String>,
        notification_routes: HashMap<String, DiscordRouteConfig>,
    ) -> Self {
        Self {
            default_url: normalized_webhook_url(&default_url),
            category_urls: channels
                .into_iter()
                .filter_map(|(key, value)| normalized_webhook_url(&value).map(|url| (key, url)))
                .collect(),
            notification_routes,
        }
    }

    fn resolve_target(&self, alert: &DiscordAlert) -> Option<ResolvedTarget> {
        if let Some(route_key) = alert.route_key.as_deref()
            && let Some(route) = self.notification_routes.get(route_key)
        {
            if !route.enabled {
                return None;
            }

            let url = normalized_webhook_url(&route.webhook_url)
                .or_else(|| self.url_for_category(alert.category).map(str::to_owned))?;

            return Some(ResolvedTarget {
                url,
                message_mode: route.message_mode,
                mention_policy: route.mention_policy,
                level: AlertLevel::from(route.level),
            });
        }

        self.url_for_category(alert.category)
            .map(|url| ResolvedTarget {
                url: url.to_owned(),
                message_mode: alert.message_mode,
                mention_policy: alert.mention_policy,
                level: alert.level,
            })
    }

    fn url_for_category(&self, category: EventCategory) -> Option<&str> {
        self.category_urls
            .get(category.as_str())
            .map(String::as_str)
            .or(self.default_url.as_deref())
    }
}

fn normalized_webhook_url(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Debug, Clone)]
struct WebhookRateLimiter {
    scheduled_ms: VecDeque<u64>,
    limit: usize,
    window_ms: u64,
}

impl WebhookRateLimiter {
    fn new(limit: usize, window_ms: u64) -> Self {
        Self {
            scheduled_ms: VecDeque::new(),
            limit,
            window_ms,
        }
    }

    fn schedule_at(&mut self, now_ms: u64) -> u64 {
        self.prune(now_ms);
        let earliest_allowed = if self.scheduled_ms.len() < self.limit {
            now_ms
        } else {
            self.scheduled_ms
                .front()
                .copied()
                .unwrap_or(now_ms)
                .saturating_add(self.window_ms)
        };
        let scheduled = self
            .scheduled_ms
            .back()
            .copied()
            .map_or(earliest_allowed, |last| earliest_allowed.max(last));
        self.prune(scheduled);
        self.scheduled_ms.push_back(scheduled);
        scheduled
    }

    fn prune(&mut self, now_ms: u64) {
        while let Some(&front) = self.scheduled_ms.front() {
            if now_ms > front && now_ms.saturating_sub(front) > self.window_ms {
                self.scheduled_ms.pop_front();
            } else {
                break;
            }
        }
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
        Self::with_routes(default_url, channels, HashMap::new())
    }

    /// Create a webhook sender with per-category and per-alert-type routing.
    #[must_use]
    pub fn with_routes(
        default_url: String,
        channels: HashMap<String, String>,
        notification_routes: HashMap<String, DiscordRouteConfig>,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<DiscordAlert>();
        let router = ChannelRouter::new(default_url, channels, notification_routes);

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

    /// Convenience: send an info-level alert to the status route.
    pub fn info(&self, title: &str, message: &str) {
        self.send(
            DiscordAlert::simple(title, message, AlertLevel::Info, EventCategory::Status)
                .with_route_key("status"),
        );
    }

    /// Convenience: send a warning-level alert to the status route.
    pub fn warn(&self, title: &str, message: &str) {
        self.send(
            DiscordAlert::simple(title, message, AlertLevel::Warning, EventCategory::Status)
                .with_route_key("status"),
        );
    }

    /// Convenience: send a critical-level alert to the status route.
    pub fn critical(&self, title: &str, message: &str) {
        self.send(
            DiscordAlert::simple(title, message, AlertLevel::Critical, EventCategory::Status)
                .with_route_key("status"),
        );
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
        let started = Instant::now();
        let mut rate_limiters: HashMap<String, WebhookRateLimiter> = HashMap::new();

        while let Ok(alert) = rx.recv() {
            let Some(target) = router.resolve_target(&alert) else {
                tracing::debug!(
                    category = alert.category.as_str(),
                    route = alert.route_key.as_deref().unwrap_or("none"),
                    title = %alert.title,
                    "No webhook target configured for Discord alert, skipping"
                );
                continue;
            };

            let now_ms = started.elapsed().as_millis() as u64;
            let scheduled_ms = rate_limiters
                .entry(target.url.clone())
                .or_insert_with(|| {
                    WebhookRateLimiter::new(
                        DISCORD_WEBHOOK_LIMIT_PER_MINUTE,
                        DISCORD_WEBHOOK_WINDOW_MS,
                    )
                })
                .schedule_at(now_ms);
            if scheduled_ms > now_ms {
                thread::sleep(Duration::from_millis(scheduled_ms - now_ms));
            }

            let payload = build_payload(&alert, &target, chrono_now_iso());

            match client.post(&target.url).json(&payload).send() {
                Ok(resp) if resp.status().is_success() => {
                    tracing::debug!(
                        title = %alert.title,
                        category = alert.category.as_str(),
                        route = alert.route_key.as_deref().unwrap_or("none"),
                        "Discord alert sent"
                    );
                }
                Ok(resp) => {
                    tracing::warn!(
                        status = %resp.status(),
                        title = %alert.title,
                        route = alert.route_key.as_deref().unwrap_or("none"),
                        "Discord webhook returned non-success"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        title = %alert.title,
                        route = alert.route_key.as_deref().unwrap_or("none"),
                        "Discord webhook POST failed"
                    );
                }
            }
        }

        tracing::info!("Discord webhook sender shutting down");
    }
}

fn build_payload(
    alert: &DiscordAlert,
    target: &ResolvedTarget,
    timestamp: String,
) -> serde_json::Value {
    match target.message_mode {
        DiscordMessageMode::PlainText => serde_json::json!({
            "content": build_plain_text_content(alert, target),
            "allowed_mentions": allowed_mentions(target.mention_policy),
        }),
        DiscordMessageMode::RichEmbed => {
            let mut payload = serde_json::Map::new();
            payload.insert(
                "allowed_mentions".to_string(),
                allowed_mentions(target.mention_policy),
            );
            if matches!(target.mention_policy, DiscordMentionPolicy::Everyone) {
                payload.insert("content".to_string(), serde_json::json!("@everyone"));
            }
            payload.insert(
                "embeds".to_string(),
                serde_json::json!([{
                    "title": alert.title,
                    "description": alert.message,
                    "color": target.level.color(),
                    "fields": build_embed_fields(alert),
                    "footer": {
                        "text": format!("TextQuest • {}", footer_route_label(alert))
                    },
                    "timestamp": timestamp
                }]),
            );
            serde_json::Value::Object(payload)
        }
    }
}

fn build_embed_fields(alert: &DiscordAlert) -> Vec<serde_json::Value> {
    alert
        .fields
        .iter()
        .map(|field| {
            serde_json::json!({
                "name": field.name,
                "value": field.value,
                "inline": field.inline,
            })
        })
        .collect()
}

fn build_plain_text_content(alert: &DiscordAlert, target: &ResolvedTarget) -> String {
    let mut content = String::new();
    if matches!(target.mention_policy, DiscordMentionPolicy::Everyone) {
        content.push_str("@everyone ");
    }
    content.push_str(&format!("**[{}] {}**", target.level.label(), alert.title));
    if !alert.message.trim().is_empty() {
        content.push('\n');
        content.push_str(&alert.message);
    }
    for field in &alert.fields {
        content.push_str(&format!("\n- **{}:** {}", field.name, field.value));
    }
    content
}

fn footer_route_label(alert: &DiscordAlert) -> &str {
    alert
        .route_key
        .as_deref()
        .unwrap_or_else(|| alert.category.as_str())
}

fn allowed_mentions(policy: DiscordMentionPolicy) -> serde_json::Value {
    match policy {
        DiscordMentionPolicy::None => serde_json::json!({ "parse": [] }),
        DiscordMentionPolicy::Everyone => serde_json::json!({ "parse": ["everyone"] }),
    }
}

/// Get current time as ISO 8601 string (no chrono dependency).
fn chrono_now_iso() -> String {
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
        assert_eq!(AlertLevel::Error.color(), 0xE67E22);
        assert_eq!(AlertLevel::Critical.color(), 0xE74C3C);
    }

    #[test]
    fn severity_error_maps_to_error_level() {
        assert_eq!(AlertLevel::from(Severity::Error), AlertLevel::Error);
        assert_eq!(AlertLevel::from(Severity::Critical), AlertLevel::Critical);
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
        assert_eq!(alert.message_mode, DiscordMessageMode::RichEmbed);
        assert_eq!(alert.mention_policy, DiscordMentionPolicy::None);
        assert!(alert.route_key.is_none());
        assert!(alert.fields.is_empty());
    }

    #[test]
    fn alert_with_fields_and_route_key() {
        let alert = DiscordAlert::simple(
            "Kill",
            "Nagafen down",
            AlertLevel::Info,
            EventCategory::Kills,
        )
        .with_route_key("death")
        .with_message_mode(DiscordMessageMode::PlainText)
        .with_mention_policy(DiscordMentionPolicy::Everyone)
        .with_field("Top DPS", "Wizard01 — 5000", true)
        .with_field("Duration", "2m 30s", true);
        assert_eq!(alert.fields.len(), 2);
        assert_eq!(alert.fields[0].name, "Top DPS");
        assert!(alert.fields[0].inline);
        assert_eq!(alert.route_key.as_deref(), Some("death"));
        assert_eq!(alert.message_mode, DiscordMessageMode::PlainText);
        assert_eq!(alert.mention_policy, DiscordMentionPolicy::Everyone);
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
        let router = ChannelRouter::new(
            "https://default.example.com".into(),
            HashMap::new(),
            HashMap::new(),
        );
        let alert = DiscordAlert::simple("Test", "Hello", AlertLevel::Info, EventCategory::Kills);
        assert_eq!(
            router.resolve_target(&alert),
            Some(ResolvedTarget {
                url: "https://default.example.com".into(),
                message_mode: DiscordMessageMode::RichEmbed,
                mention_policy: DiscordMentionPolicy::None,
                level: AlertLevel::Info,
            })
        );
    }

    #[test]
    fn channel_router_category_override() {
        let mut channels = HashMap::new();
        channels.insert("kills".into(), "https://kills.example.com".into());
        let router = ChannelRouter::new(
            "https://default.example.com".into(),
            channels,
            HashMap::new(),
        );
        let kill_alert = DiscordAlert::simple("Kill", "", AlertLevel::Info, EventCategory::Kills);
        let loot_alert = DiscordAlert::simple("Loot", "", AlertLevel::Info, EventCategory::Loot);
        assert_eq!(
            router.resolve_target(&kill_alert).map(|target| target.url),
            Some("https://kills.example.com".into())
        );
        assert_eq!(
            router.resolve_target(&loot_alert).map(|target| target.url),
            Some("https://default.example.com".into())
        );
    }

    #[test]
    fn channel_router_notification_route_override_and_disable() {
        let mut routes = HashMap::new();
        routes.insert(
            "death".into(),
            DiscordRouteConfig {
                webhook_url: "https://death.example.com".into(),
                level: Severity::Critical,
                message_mode: DiscordMessageMode::PlainText,
                mention_policy: DiscordMentionPolicy::Everyone,
                enabled: true,
            },
        );
        routes.insert(
            "status".into(),
            DiscordRouteConfig {
                enabled: false,
                level: Severity::Info,
                ..DiscordRouteConfig::default()
            },
        );
        let router =
            ChannelRouter::new("https://default.example.com".into(), HashMap::new(), routes);
        let death_alert = DiscordAlert::simple(
            "Death",
            "Cleric down",
            AlertLevel::Warning,
            EventCategory::Status,
        )
        .with_route_key("death");
        let status_alert = DiscordAlert::simple(
            "Status",
            "Camp started",
            AlertLevel::Info,
            EventCategory::Status,
        )
        .with_route_key("status");

        assert_eq!(
            router.resolve_target(&death_alert),
            Some(ResolvedTarget {
                url: "https://death.example.com".into(),
                message_mode: DiscordMessageMode::PlainText,
                mention_policy: DiscordMentionPolicy::Everyone,
                level: AlertLevel::Critical,
            })
        );
        assert_eq!(router.resolve_target(&status_alert), None);
    }

    #[test]
    fn channel_router_notification_route_uses_default_webhook_when_route_url_empty() {
        let mut routes = HashMap::new();
        routes.insert(
            "death".into(),
            DiscordRouteConfig {
                level: Severity::Critical,
                mention_policy: DiscordMentionPolicy::Everyone,
                ..DiscordRouteConfig::default()
            },
        );
        let router =
            ChannelRouter::new("https://default.example.com".into(), HashMap::new(), routes);
        let alert = DiscordAlert::simple(
            "Death",
            "Corpse",
            AlertLevel::Warning,
            EventCategory::Status,
        )
        .with_route_key("death");
        assert_eq!(
            router.resolve_target(&alert),
            Some(ResolvedTarget {
                url: "https://default.example.com".into(),
                message_mode: DiscordMessageMode::RichEmbed,
                mention_policy: DiscordMentionPolicy::Everyone,
                level: AlertLevel::Critical,
            })
        );
    }

    #[test]
    fn build_plain_text_payload_includes_mention_and_fields() {
        let alert = DiscordAlert::simple(
            "Death — Cleric01",
            "**Cleric01** died in _everfrost_",
            AlertLevel::Critical,
            EventCategory::Status,
        )
        .with_route_key("death")
        .with_field("Zone", "everfrost", true)
        .with_field("Cause", "A frost giant", false);
        let target = ResolvedTarget {
            url: "https://discord.example.com".into(),
            message_mode: DiscordMessageMode::PlainText,
            mention_policy: DiscordMentionPolicy::Everyone,
            level: AlertLevel::Critical,
        };

        let payload = build_payload(&alert, &target, "2026-04-15T00:00:00Z".into());
        assert_eq!(
            payload["content"].as_str(),
            Some(
                "@everyone **[CRITICAL] Death — Cleric01**\n**Cleric01** died in _everfrost_\n- \
                 **Zone:** everfrost\n- **Cause:** A frost giant"
            )
        );
        assert_eq!(
            payload["allowed_mentions"]["parse"][0].as_str(),
            Some("everyone")
        );
        assert!(payload.get("embeds").is_none());
    }

    #[test]
    fn build_rich_embed_payload_uses_route_level_and_footer() {
        let alert = DiscordAlert::simple(
            "Status",
            "Camp started",
            AlertLevel::Warning,
            EventCategory::Status,
        )
        .with_route_key("status");
        let target = ResolvedTarget {
            url: "https://discord.example.com".into(),
            message_mode: DiscordMessageMode::RichEmbed,
            mention_policy: DiscordMentionPolicy::None,
            level: AlertLevel::Info,
        };

        let payload = build_payload(&alert, &target, "2026-04-15T00:00:00Z".into());
        assert_eq!(payload["embeds"][0]["color"].as_u64(), Some(0x2ECC71));
        assert_eq!(
            payload["embeds"][0]["footer"]["text"].as_str(),
            Some("TextQuest • status")
        );
        assert_eq!(
            payload["allowed_mentions"]["parse"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert!(payload.get("content").is_none());
    }

    #[test]
    fn webhook_rate_limiter_enforces_thirty_per_minute() {
        let mut limiter = WebhookRateLimiter::new(30, 60_000);

        for idx in 0..30 {
            assert_eq!(
                limiter.schedule_at(0),
                0,
                "schedule {idx} should be immediate"
            );
        }
        assert_eq!(limiter.schedule_at(0), 60_000);
        assert_eq!(limiter.schedule_at(1_000), 60_000);
        assert_eq!(limiter.schedule_at(60_000), 60_000);
        assert_eq!(limiter.schedule_at(60_001), 60_001);
    }
}
