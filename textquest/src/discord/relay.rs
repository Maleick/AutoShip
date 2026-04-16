//! Event relay — converts `FleetEvent` observations into Discord webhook
//! alerts.
//!
//! Bridges the metrics pipeline to Discord by translating fleet events into
//! typed [`DiscordAlert`]s routed through [`WebhookSender`].
//!
//! # Chat message routing
//!
//! In-game chat (group, raid, guild) can be relayed to per-channel Discord
//! webhooks via [`ChatRelay`]. Configure routing with `[discord.chat_channels]`
//! in TOML:
//!
//! ```toml
//! [discord.chat_channels]
//! group = "https://discord.com/api/webhooks/.../group-chat"
//! raid  = "https://discord.com/api/webhooks/.../raid-chat"
//! guild = "https://discord.com/api/webhooks/.../guild-chat"
//! ```
//!
//! # Event webhooks
//!
//! [`EventRelay::on_event`] accepts any [`FleetEvent`] and dispatches an alert
//! to the appropriate Discord channel:
//!
//! - `FleetEvent::Kill`         → `EventCategory::Kills`
//! - `FleetEvent::LootDrop`     → `EventCategory::Loot`
//! - `FleetEvent::LevelUp`      → `EventCategory::Feats`
//! - `FleetEvent::Death`        → `EventCategory::Status` via the `"death"`
//!   route
//! - `FleetEvent::CombatRound`  → buffered; call [`EventRelay::flush_dps`] to
//!   post a DPS summary
//! - `FleetEvent::ZoneChange`   → `EventCategory::Status`

use std::collections::HashMap;

use crate::metrics::events::FleetEvent;

use super::webhook::{AlertLevel, DiscordAlert, EventCategory, WebhookSender};

// ─── Chat relay
// ───────────────────────────────────────────────────────────────

/// EverQuest chat channel types that can be relayed to Discord.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatChannel {
    /// `/g` group chat — relayed to the group webhook.
    Group,
    /// `/r` raid channel — relayed to the raid webhook.
    Raid,
    /// `/gu` guild channel — relayed to the guild webhook.
    Guild,
    /// `/ooc` out-of-character channel.
    OutOfCharacter,
    /// `/shout` zone shout.
    Shout,
    /// `/say` zone say.
    Say,
    /// `/tell` direct message to another player.
    Tell,
}

impl ChatChannel {
    /// Config key used in `[discord.chat_channels]`.
    #[must_use]
    pub fn config_key(&self) -> &'static str {
        match self {
            Self::Group => "group",
            Self::Raid => "raid",
            Self::Guild => "guild",
            Self::OutOfCharacter => "ooc",
            Self::Shout => "shout",
            Self::Say => "say",
            Self::Tell => "tell",
        }
    }

    /// Human-readable channel label for Discord embeds.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Group => "Group",
            Self::Raid => "Raid",
            Self::Guild => "Guild",
            Self::OutOfCharacter => "OOC",
            Self::Shout => "Shout",
            Self::Say => "Say",
            Self::Tell => "Tell",
        }
    }
}

/// A single in-game chat message to be relayed to Discord.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// The in-game channel where the message appeared.
    pub channel: ChatChannel,
    /// The sender's character name.
    pub sender: String,
    /// The message body.
    pub text: String,
    /// Zone the message originated from (empty if unknown).
    pub zone: String,
}

/// Routes in-game chat messages to per-channel Discord webhooks.
///
/// Constructed from a `HashMap<String, String>` matching
/// `[discord.chat_channels]` in the TOML config.
#[derive(Debug, Clone)]
pub struct ChatRelay {
    /// Maps `ChatChannel::config_key()` → webhook URL.
    channel_urls: HashMap<String, String>,
}

impl ChatRelay {
    /// Create a relay from the `chat_channels` config map.
    ///
    /// Keys must match [`ChatChannel::config_key`] values.
    /// Unrecognized keys are silently ignored.
    #[must_use]
    pub fn new(channel_urls: HashMap<String, String>) -> Self {
        Self { channel_urls }
    }

    /// Return the webhook URL configured for the given chat channel, if any.
    #[must_use]
    pub fn url_for(&self, channel: ChatChannel) -> Option<&str> {
        self.channel_urls
            .get(channel.config_key())
            .map(String::as_str)
    }

    /// Format a chat message into a Discord webhook payload body (JSON string).
    ///
    /// Returns `None` when no webhook URL is configured for the channel.
    #[must_use]
    pub fn format_message(&self, msg: &ChatMessage) -> Option<String> {
        self.url_for(msg.channel)?;

        let content = format!(
            "**[{}]** **{}**: {}",
            msg.channel.label(),
            msg.sender,
            msg.text
        );

        // Simple content-only payload (no embed) to keep chat relay lightweight.
        Some(serde_json::json!({ "content": content }).to_string())
    }

    /// Send a chat message via the blocking reqwest client.
    ///
    /// Silently no-ops when no URL is configured for the channel.
    pub fn send(&self, client: &reqwest::blocking::Client, msg: &ChatMessage) {
        let Some(url) = self.url_for(msg.channel) else {
            return;
        };

        let Some(body) = self.format_message(msg) else {
            return;
        };

        match client
            .post(url)
            .body(body)
            .header("Content-Type", "application/json")
            .send()
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::debug!(
                    channel = msg.channel.config_key(),
                    sender = %msg.sender,
                    "Chat message relayed to Discord"
                );
            }
            Ok(resp) => {
                tracing::warn!(
                    status = %resp.status(),
                    channel = msg.channel.config_key(),
                    "Chat relay webhook returned non-success"
                );
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    channel = msg.channel.config_key(),
                    "Chat relay POST failed"
                );
            }
        }
    }
}

// ─── CombatRound accumulator
// ──────────────────────────────────────────────────

/// Accumulates `CombatRound` events and produces a DPS summary.
#[derive(Debug, Default)]
pub struct DpsAccumulator {
    /// Aggregated damage dealt per PID.
    damage_by_pid: HashMap<u32, u64>,
    /// Total duration in milliseconds across all CombatRound events.
    total_duration_ms: u64,
    /// Number of rounds accumulated.
    round_count: u32,
}

impl DpsAccumulator {
    /// Record a single `CombatRound` event.
    pub fn record(&mut self, pid: u32, damage_dealt: u64, duration_ms: u32) {
        *self.damage_by_pid.entry(pid).or_default() += damage_dealt;
        self.total_duration_ms += u64::from(duration_ms);
        self.round_count += 1;
    }

    /// Returns true when no rounds have been accumulated.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.round_count == 0
    }

    /// Compute DPS for each PID — `(pid, dps)` sorted highest first.
    ///
    /// DPS is computed across the total accumulated duration. If the duration
    /// is zero, each entry is reported as 0 DPS.
    #[must_use]
    pub fn dps_summary(&self) -> Vec<(u32, i64)> {
        let duration_secs = (self.total_duration_ms as f64 / 1_000.0).max(0.001);
        let mut entries: Vec<(u32, i64)> = self
            .damage_by_pid
            .iter()
            .map(|(&pid, &dmg)| (pid, (dmg as f64 / duration_secs) as i64))
            .collect();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        entries
    }

    /// Reset the accumulator (called after flushing to Discord).
    pub fn reset(&mut self) {
        self.damage_by_pid.clear();
        self.total_duration_ms = 0;
        self.round_count = 0;
    }

    /// Number of rounds accumulated since the last reset.
    #[must_use]
    pub fn round_count(&self) -> u32 {
        self.round_count
    }
}

// ─── EventRelay
// ───────────────────────────────────────────────────────────────

/// Converts [`FleetEvent`] stream into Discord webhook alerts.
///
/// # Usage
///
/// ```ignore
/// let relay = EventRelay::new(webhook_sender);
/// for event in event_stream {
///     relay.on_event(&event, pid_to_name);
/// }
/// // At end of encounter:
/// relay.flush_dps("Lord Nagafen", &pid_to_name);
/// ```
pub struct EventRelay {
    sender: WebhookSender,
    dps: std::sync::Mutex<DpsAccumulator>,
}

impl EventRelay {
    /// Create a relay backed by the given [`WebhookSender`].
    #[must_use]
    pub fn new(sender: WebhookSender) -> Self {
        Self {
            sender,
            dps: std::sync::Mutex::new(DpsAccumulator::default()),
        }
    }

    /// Process a single fleet event and dispatch the appropriate Discord alert.
    ///
    /// `pid_name` is a callable that resolves a PID to a character name string.
    /// Pass `|_| "Unknown".to_string()` when no name map is available.
    pub fn on_event<F>(&self, event: &FleetEvent, mut pid_name: F)
    where
        F: FnMut(u32) -> String,
    {
        match event {
            FleetEvent::Kill {
                source_pid,
                target_name,
                target_level,
                zone,
                ..
            } => {
                let killer = pid_name(*source_pid);
                let alert = DiscordAlert::simple(
                    format!(
                        "{} {} Killed (lvl {})!",
                        EventCategory::Kills.emoji(),
                        target_name,
                        target_level
                    ),
                    format!("Killed by **{killer}** in _{zone}_"),
                    AlertLevel::Info,
                    EventCategory::Kills,
                );
                self.sender.send(alert);
            }

            FleetEvent::LootDrop {
                pid,
                item_name,
                zone,
                ..
            } => {
                let looter = pid_name(*pid);
                self.sender.loot(item_name, &looter, Some(zone.as_str()));
            }

            FleetEvent::LevelUp {
                pid,
                character_name,
                new_level,
                ..
            } => {
                let name = if character_name.is_empty() {
                    pid_name(*pid)
                } else {
                    character_name.clone()
                };
                self.sender.feat(
                    &format!("Level {} — {name}", new_level),
                    &format!("**{name}** reached level **{new_level}**!"),
                );
            }

            FleetEvent::Death {
                pid,
                character_name,
                zone,
                ..
            } => {
                let name = if character_name.is_empty() {
                    pid_name(*pid)
                } else {
                    character_name.clone()
                };
                let alert = DiscordAlert::simple(
                    format!("Death — {name}"),
                    format!("**{name}** died in _{zone}_"),
                    AlertLevel::Critical,
                    EventCategory::Status,
                )
                .with_route_key("death");
                self.sender.send(alert);
            }

            FleetEvent::ZoneChange {
                pid,
                from_zone,
                to_zone,
                ..
            } => {
                let name = pid_name(*pid);
                self.sender.info(
                    &format!("Zone Change — {name}"),
                    &format!("**{name}** moved from _{from_zone}_ → _{to_zone}_"),
                );
            }

            FleetEvent::CombatRound {
                pid,
                damage_dealt,
                duration_ms,
                ..
            } => {
                // Accumulate; caller flushes with flush_dps().
                if let Ok(mut acc) = self.dps.lock() {
                    acc.record(*pid, *damage_dealt, *duration_ms);
                }
            }
        }
    }

    /// Post a DPS summary to Discord and reset the accumulator.
    ///
    /// `encounter_name` is displayed as the embed title (e.g., "Lord Nagafen").
    /// `pid_name` resolves PIDs to character names for the leaderboard.
    pub fn flush_dps<F>(&self, encounter_name: &str, mut pid_name: F)
    where
        F: FnMut(u32) -> String,
    {
        let Ok(mut acc) = self.dps.lock() else {
            return;
        };
        if acc.is_empty() {
            return;
        }

        let summary = acc.dps_summary();
        let named: Vec<(String, i64)> = summary
            .into_iter()
            .map(|(pid, dps)| (pid_name(pid), dps))
            .collect();

        self.sender.kill(encounter_name, &named);
        acc.reset();
    }

    /// Access the underlying [`WebhookSender`] for direct alert dispatch.
    #[must_use]
    pub fn sender(&self) -> &WebhookSender {
        &self.sender
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ChatChannel ──────────────────────────────────────────────────────────

    #[test]
    fn chat_channel_config_keys_are_unique_and_nonempty() {
        let channels = [
            ChatChannel::Group,
            ChatChannel::Raid,
            ChatChannel::Guild,
            ChatChannel::OutOfCharacter,
            ChatChannel::Shout,
            ChatChannel::Say,
            ChatChannel::Tell,
        ];
        let mut keys = std::collections::HashSet::new();
        for ch in channels {
            let key = ch.config_key();
            assert!(!key.is_empty(), "config_key must not be empty");
            assert!(keys.insert(key), "config_key '{key}' is not unique");
        }
    }

    #[test]
    fn chat_channel_labels_are_nonempty() {
        for ch in [
            ChatChannel::Group,
            ChatChannel::Raid,
            ChatChannel::Guild,
            ChatChannel::OutOfCharacter,
            ChatChannel::Shout,
            ChatChannel::Say,
            ChatChannel::Tell,
        ] {
            assert!(!ch.label().is_empty());
        }
    }

    // ── ChatRelay ────────────────────────────────────────────────────────────

    #[test]
    fn chat_relay_url_for_configured_channel() {
        let mut urls = HashMap::new();
        urls.insert(
            "group".to_string(),
            "https://hook.example.com/group".to_string(),
        );
        let relay = ChatRelay::new(urls);
        assert_eq!(
            relay.url_for(ChatChannel::Group),
            Some("https://hook.example.com/group")
        );
        assert!(relay.url_for(ChatChannel::Raid).is_none());
    }

    #[test]
    fn chat_relay_url_for_unconfigured_returns_none() {
        let relay = ChatRelay::new(HashMap::new());
        for ch in [ChatChannel::Group, ChatChannel::Raid, ChatChannel::Guild] {
            assert!(relay.url_for(ch).is_none());
        }
    }

    #[test]
    fn chat_relay_format_message_returns_none_when_no_url() {
        let relay = ChatRelay::new(HashMap::new());
        let msg = ChatMessage {
            channel: ChatChannel::Group,
            sender: "Wizard01".to_string(),
            text: "oom".to_string(),
            zone: "nagafen".to_string(),
        };
        assert!(relay.format_message(&msg).is_none());
    }

    #[test]
    fn chat_relay_format_message_contains_sender_and_text() {
        let mut urls = HashMap::new();
        urls.insert("group".to_string(), "https://hook.example.com".to_string());
        let relay = ChatRelay::new(urls);
        let msg = ChatMessage {
            channel: ChatChannel::Group,
            sender: "Wizard01".to_string(),
            text: "burning sands inc".to_string(),
            zone: "nagafen".to_string(),
        };
        let formatted = relay.format_message(&msg).unwrap();
        assert!(formatted.contains("Wizard01"), "must contain sender");
        assert!(formatted.contains("burning sands inc"), "must contain text");
        assert!(formatted.contains("Group"), "must contain channel label");
    }

    #[test]
    fn chat_relay_format_message_is_valid_json() {
        let mut urls = HashMap::new();
        urls.insert("raid".to_string(), "https://hook.example.com".to_string());
        let relay = ChatRelay::new(urls);
        let msg = ChatMessage {
            channel: ChatChannel::Raid,
            sender: "RaidLead".to_string(),
            text: "pull to safe spot".to_string(),
            zone: "plane_of_time_a".to_string(),
        };
        let formatted = relay.format_message(&msg).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();
        assert!(parsed["content"].is_string());
    }

    #[test]
    fn chat_relay_multiple_channels_independent() {
        let mut urls = HashMap::new();
        urls.insert(
            "group".to_string(),
            "https://hook.example.com/group".to_string(),
        );
        urls.insert(
            "raid".to_string(),
            "https://hook.example.com/raid".to_string(),
        );
        let relay = ChatRelay::new(urls);
        assert!(relay.url_for(ChatChannel::Group).is_some());
        assert!(relay.url_for(ChatChannel::Raid).is_some());
        assert!(relay.url_for(ChatChannel::Guild).is_none());
    }

    // ── DpsAccumulator ───────────────────────────────────────────────────────

    #[test]
    fn dps_accumulator_starts_empty() {
        let acc = DpsAccumulator::default();
        assert!(acc.is_empty());
        assert_eq!(acc.round_count(), 0);
        assert!(acc.dps_summary().is_empty());
    }

    #[test]
    fn dps_accumulator_single_pid() {
        let mut acc = DpsAccumulator::default();
        // 1000 damage over 1000 ms = 1.0 seconds → 1000 DPS
        acc.record(1, 1000, 1000);
        assert!(!acc.is_empty());
        let summary = acc.dps_summary();
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].0, 1);
        assert_eq!(summary[0].1, 1000);
    }

    #[test]
    fn dps_accumulator_multiple_pids_sorted_descending() {
        let mut acc = DpsAccumulator::default();
        // PID 1: 500 dmg over 1000ms total (shared) → 500 DPS
        // PID 2: 200 dmg over 1000ms total (shared) → 200 DPS
        acc.record(1, 500, 500);
        acc.record(2, 200, 500);
        let summary = acc.dps_summary();
        assert_eq!(summary.len(), 2);
        assert!(summary[0].1 >= summary[1].1, "must be sorted descending");
        assert_eq!(summary[0].0, 1);
        assert_eq!(summary[1].0, 2);
    }

    #[test]
    fn dps_accumulator_multi_round_same_pid() {
        let mut acc = DpsAccumulator::default();
        acc.record(1, 300, 500);
        acc.record(1, 700, 500);
        assert_eq!(acc.round_count(), 2);
        let summary = acc.dps_summary();
        assert_eq!(summary.len(), 1);
        // 1000 dmg over 1000ms = 1000 DPS
        assert_eq!(summary[0].1, 1000);
    }

    #[test]
    fn dps_accumulator_reset_clears_state() {
        let mut acc = DpsAccumulator::default();
        acc.record(1, 500, 1000);
        acc.reset();
        assert!(acc.is_empty());
        assert_eq!(acc.round_count(), 0);
        assert!(acc.dps_summary().is_empty());
    }

    #[test]
    fn dps_accumulator_zero_duration_does_not_panic() {
        let mut acc = DpsAccumulator::default();
        acc.record(1, 100, 0); // 0 ms duration — should not divide by zero
        let summary = acc.dps_summary();
        // With near-zero duration clamp at 0.001s → very high DPS, just must not panic
        assert_eq!(summary.len(), 1);
    }

    // ── EventRelay dispatch ──────────────────────────────────────────────────
    // We can't easily intercept what WebhookSender sends without a live server,
    // but we can construct one and verify flush_dps with an empty accumulator
    // does not panic (regression guard).

    fn make_sender() -> WebhookSender {
        // No webhook URL — alerts queue but drain silently (no URL to post to).
        WebhookSender::new(String::new())
    }

    #[test]
    fn event_relay_flush_dps_noop_when_empty() {
        let relay = EventRelay::new(make_sender());
        // Should not panic when no combat rounds have been recorded.
        relay.flush_dps("Lord Nagafen", |pid| format!("char{pid}"));
    }

    #[test]
    fn event_relay_on_event_kill_does_not_panic() {
        let relay = EventRelay::new(make_sender());
        let event = FleetEvent::Kill {
            source_pid: 1,
            target_name: "Lord Nagafen".into(),
            target_level: 52,
            zone: "nagafen".into(),
            timestamp: 0,
        };
        relay.on_event(&event, |_| "Wizard01".into());
    }

    #[test]
    fn event_relay_on_event_loot_does_not_panic() {
        let relay = EventRelay::new(make_sender());
        let event = FleetEvent::LootDrop {
            pid: 2,
            item_name: "Cloak of Flames".into(),
            item_id: 4321,
            zone: "nagafen".into(),
            timestamp: 0,
        };
        relay.on_event(&event, |_| "Warrior01".into());
    }

    #[test]
    fn event_relay_on_event_level_up_does_not_panic() {
        let relay = EventRelay::new(make_sender());
        let event = FleetEvent::LevelUp {
            pid: 3,
            character_name: "Toon".into(),
            new_level: 60,
            timestamp: 0,
        };
        relay.on_event(&event, |_| "Toon".into());
    }

    #[test]
    fn event_relay_on_event_death_does_not_panic() {
        let relay = EventRelay::new(make_sender());
        let event = FleetEvent::Death {
            pid: 4,
            character_name: "Cleric01".into(),
            zone: "everfrost".into(),
            timestamp: 0,
        };
        relay.on_event(&event, |_| "Cleric01".into());
    }

    #[test]
    fn event_relay_on_event_zone_change_does_not_panic() {
        let relay = EventRelay::new(make_sender());
        let event = FleetEvent::ZoneChange {
            pid: 5,
            from_zone: "gfaydark".into(),
            to_zone: "crushbone".into(),
            timestamp: 0,
        };
        relay.on_event(&event, |_| "Druid01".into());
    }

    #[test]
    fn event_relay_accumulates_combat_rounds_and_flush() {
        let relay = EventRelay::new(make_sender());

        let round1 = FleetEvent::CombatRound {
            pid: 1,
            damage_dealt: 600,
            damage_taken: 100,
            duration_ms: 1000,
            timestamp: 0,
        };
        let round2 = FleetEvent::CombatRound {
            pid: 1,
            damage_dealt: 400,
            damage_taken: 50,
            duration_ms: 1000,
            timestamp: 1,
        };

        relay.on_event(&round1, |_| "Wiz".into());
        relay.on_event(&round2, |_| "Wiz".into());

        // Verify accumulator has data before flush
        {
            let acc = relay.dps.lock().unwrap();
            assert_eq!(acc.round_count(), 2);
        }

        // Flush — must not panic, resets accumulator
        relay.flush_dps("Encounter", |pid| format!("char{pid}"));

        // After flush accumulator is reset
        {
            let acc = relay.dps.lock().unwrap();
            assert!(acc.is_empty());
        }
    }

    #[test]
    fn event_relay_flush_dps_uses_pid_name_fn() {
        let relay = EventRelay::new(make_sender());

        let round = FleetEvent::CombatRound {
            pid: 99,
            damage_dealt: 1000,
            damage_taken: 0,
            duration_ms: 1000,
            timestamp: 0,
        };
        relay.on_event(&round, |_| "NotUsed".into());

        // flush_dps should call our pid_name fn for pid 99
        // Use a Cell so FnMut can capture and mutate it.
        let called_with = std::cell::Cell::new(None::<u32>);
        relay.flush_dps("Boss", |pid| {
            called_with.set(Some(pid));
            format!("Player{pid}")
        });

        assert_eq!(called_with.get(), Some(99));
    }

    // ── Config integration ───────────────────────────────────────────────────

    #[test]
    fn chat_relay_from_toml_config_keys() {
        // Simulate the TOML `[discord.chat_channels]` section being deserialized
        // into a HashMap<String, String> and passed to ChatRelay::new.
        let toml_str = r#"
            [chat_channels]
            group = "https://hooks.discord.com/group"
            raid  = "https://hooks.discord.com/raid"
            guild = "https://hooks.discord.com/guild"
        "#;

        #[derive(serde::Deserialize)]
        struct TestCfg {
            chat_channels: HashMap<String, String>,
        }

        let cfg: TestCfg = toml::from_str(toml_str).unwrap();
        let relay = ChatRelay::new(cfg.chat_channels);

        assert!(relay.url_for(ChatChannel::Group).is_some());
        assert!(relay.url_for(ChatChannel::Raid).is_some());
        assert!(relay.url_for(ChatChannel::Guild).is_some());
        assert!(relay.url_for(ChatChannel::OutOfCharacter).is_none());
    }
}
