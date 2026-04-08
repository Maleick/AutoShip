use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

// Re-export chat types from the shared crate so existing call-sites don't need updating.
use textquest_common::chat::parse_stripped_chat_text;
pub use textquest_common::chat::{ChatChannel, ChatEvent};

/// Events parsed from EQ log lines.
#[derive(Debug, Clone, PartialEq)]
pub enum LogEvent {
    /// An item was looted from a corpse.
    Loot {
        /// Character who looted (empty if self).
        character: String,
        /// Name of the looted item.
        item: String,
    },
    /// A mob was killed.
    Kill {
        /// Name of the slain mob.
        mob: String,
    },
    /// Currency was looted from a corpse.
    Money {
        /// Platinum coins.
        plat: u32,
        /// Gold coins.
        gold: u32,
        /// Silver coins.
        silver: u32,
        /// Copper coins.
        copper: u32,
    },
    /// An experience gain event.
    Experience {
        /// Whether this was party (group) experience.
        party: bool,
    },
    /// The player died.
    Death {
        /// Name of what killed the player.
        killed_by: String,
    },
    /// Entered a new zone.
    ZoneEnter {
        /// Name of the zone entered.
        zone: String,
    },
    /// A chat message was received.
    Chat(ChatEvent),
}

/// Strip terminal/control characters from untrusted log-derived text.
fn strip_control_chars(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

/// Parse a single EQ log line into a `LogEvent`, if it matches a known pattern.
pub fn parse_log_line(line: &str) -> Option<LogEvent> {
    // Strip the EQ timestamp prefix: "[Thu Mar 28 12:34:56 2026] "
    let text = if line.starts_with('[') {
        match line.find(']') {
            Some(i) => line[i + 1..].trim_start(),
            None => line,
        }
    } else {
        line
    };

    // --You have looted a Rusty Short Sword.--
    if let Some(rest) = text.strip_prefix("--You have looted a ")
        && let Some(item) = rest.strip_suffix(".--")
    {
        return Some(LogEvent::Loot {
            character: String::new(),
            item: strip_control_chars(item),
        });
    }

    // You have slain a moss snake!
    if let Some(rest) = text.strip_prefix("You have slain ")
        && let Some(mob) = rest.strip_suffix('!')
    {
        return Some(LogEvent::Kill {
            mob: strip_control_chars(mob),
        });
    }

    // You have been slain by a moss snake!
    if let Some(rest) = text.strip_prefix("You have been slain by ")
        && let Some(killed_by) = rest.strip_suffix('!')
    {
        return Some(LogEvent::Death {
            killed_by: strip_control_chars(killed_by),
        });
    }

    // You receive 5 platinum, 3 gold, 2 silver and 1 copper from the corpse.
    if text.starts_with("You receive ") && text.contains(" from the corpse") {
        let mut plat = 0u32;
        let mut gold = 0u32;
        let mut silver = 0u32;
        let mut copper = 0u32;

        let words: Vec<&str> = text.split_whitespace().collect();
        for (i, word) in words.iter().enumerate() {
            if i == 0 {
                continue;
            }
            if let Ok(amount) = word.parse::<u32>()
                && let Some(currency) = words.get(i + 1)
            {
                let currency = currency.trim_matches(|c: char| !c.is_alphabetic());
                match currency {
                    "platinum" => plat = amount,
                    "gold" => gold = amount,
                    "silver" => silver = amount,
                    "copper" => copper = amount,
                    _ => {}
                }
            }
        }

        return Some(LogEvent::Money {
            plat,
            gold,
            silver,
            copper,
        });
    }

    // You gain experience! / You gain party experience!
    if text == "You gain experience!" {
        return Some(LogEvent::Experience { party: false });
    }
    if text == "You gain party experience!" {
        return Some(LogEvent::Experience { party: true });
    }

    // You have entered West Freeport.
    if let Some(rest) = text.strip_prefix("You have entered ")
        && let Some(zone) = rest.strip_suffix('.')
    {
        return Some(LogEvent::ZoneEnter {
            zone: strip_control_chars(zone),
        });
    }

    // Tell out: "You told Soandso, 'message'"
    // Chat channels: pattern "Sender <verb>, 'message'"
    // Delegate to the shared parser in textquest-common.
    if let Some(chat) = parse_stripped_chat_text(text) {
        return Some(LogEvent::Chat(chat));
    }

    None
}

/// Accumulates stats from parsed log events.
#[derive(Debug, Default)]
pub struct LootDatabase {
    /// Item name → count looted
    pub items: HashMap<String, u32>,
    /// Mob name → kill count
    pub kills: HashMap<String, u32>,
    /// Total platinum accumulated
    pub total_plat: u64,
    /// Total gold accumulated
    pub total_gold: u64,
    /// Total silver accumulated
    pub total_silver: u64,
    /// Total copper accumulated
    pub total_copper: u64,
    /// Total XP gain events
    pub total_xp_events: u64,
    /// Total deaths
    pub deaths: u32,
    /// Timestamps of XP events for sliding-window rate calculation.
    xp_event_times: VecDeque<Instant>,
}

impl LootDatabase {
    /// Maximum amount of XP event history retained for windowed rate calculations.
    const XP_EVENT_RETENTION: Duration = Duration::from_secs(24 * 60 * 60);

    /// Creates a new empty loot database.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a log event and update stats.
    pub fn record(&mut self, event: &LogEvent) {
        match event {
            LogEvent::Loot { item, .. } => {
                *self.items.entry(item.clone()).or_insert(0) += 1;
            }
            LogEvent::Kill { mob } => {
                *self.kills.entry(mob.clone()).or_insert(0) += 1;
            }
            LogEvent::Money {
                plat,
                gold,
                silver,
                copper,
            } => {
                self.total_plat += u64::from(*plat);
                self.total_gold += u64::from(*gold);
                self.total_silver += u64::from(*silver);
                self.total_copper += u64::from(*copper);
            }
            LogEvent::Experience { .. } => {
                self.total_xp_events += 1;
                let now = Instant::now();
                self.xp_event_times.push_back(now);
                self.prune_xp_events_older_than(now, Self::XP_EVENT_RETENTION);
            }
            LogEvent::Death { .. } => {
                self.deaths += 1;
            }
            LogEvent::ZoneEnter { .. } | LogEvent::Chat(_) => {}
        }
    }

    fn prune_xp_events_older_than(&mut self, now: Instant, window: Duration) {
        let Some(cutoff) = now.checked_sub(window) else {
            return;
        };
        while let Some(&ts) = self.xp_event_times.front() {
            if ts < cutoff {
                self.xp_event_times.pop_front();
            } else {
                break;
            }
        }
    }

    /// Process a raw log line — parses and records if it matches.
    pub fn process_line(&mut self, line: &str) -> Option<LogEvent> {
        if let Some(event) = parse_log_line(line) {
            self.record(&event);
            Some(event)
        } else {
            None
        }
    }

    /// XP events per hour within the last `window` duration.
    #[must_use]
    pub fn xp_rate_windowed(&self, window: std::time::Duration) -> f64 {
        let now = Instant::now();
        let _count = match now.checked_sub(window) {
            Some(cutoff) => self.xp_event_times.iter().filter(|t| **t >= cutoff).count(),
            // Window exceeds OS uptime; all recorded events are within the window.
            None => self.xp_event_times.len(),
        } as f64;
        let window_hours = window.as_secs_f64() / 3600.0;
        if window_hours == 0.0 {
            return 0.0;
        }

        let now = Instant::now();
        let count = self
            .xp_event_times
            .iter()
            .filter(|t| {
                now.checked_duration_since(**t)
                    .map(|elapsed| elapsed <= window)
                    .unwrap_or(false)
            })
            .count() as f64;

        count / window_hours
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_loot() {
        let line = "[Thu Mar 28 12:34:56 2026] --You have looted a Rusty Short Sword.--";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Loot {
                character: String::new(),
                item: "Rusty Short Sword".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_loot_no_timestamp() {
        let line = "--You have looted a Cloth Cap.--";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Loot {
                character: String::new(),
                item: "Cloth Cap".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_kill() {
        let line = "[Thu Mar 28 12:35:00 2026] You have slain a moss snake!";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Kill {
                mob: "a moss snake".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_death() {
        let line = "[Thu Mar 28 12:36:00 2026] You have been slain by a fire beetle!";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Death {
                killed_by: "a fire beetle".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_money_full() {
        let line = "[Thu Mar 28 12:37:00 2026] You receive 5 platinum, 3 gold, 2 silver and 1 copper from the corpse.";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Money {
                plat: 5,
                gold: 3,
                silver: 2,
                copper: 1,
            }
        );
    }

    #[test]
    fn test_parse_money_partial() {
        let line = "You receive 12 platinum from the corpse.";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Money {
                plat: 12,
                gold: 0,
                silver: 0,
                copper: 0,
            }
        );
    }

    #[test]
    fn test_parse_experience() {
        let line = "[Thu Mar 28 12:38:00 2026] You gain experience!";
        let event = parse_log_line(line).unwrap();
        assert_eq!(event, LogEvent::Experience { party: false });
    }

    #[test]
    fn test_parse_party_experience() {
        let line = "[Thu Mar 28 12:38:01 2026] You gain party experience!";
        let event = parse_log_line(line).unwrap();
        assert_eq!(event, LogEvent::Experience { party: true });
    }

    #[test]
    fn test_parse_zone_enter() {
        let line = "[Thu Mar 28 12:39:00 2026] You have entered West Freeport.";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::ZoneEnter {
                zone: "West Freeport".to_string(),
            }
        );
    }

    #[test]
    fn test_xp_rate_windowed_empty() {
        let db = LootDatabase::new();
        let rate = db.xp_rate_windowed(std::time::Duration::from_secs(900));
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn test_xp_rate_windowed_counts_recent() {
        let mut db = LootDatabase::new();
        db.process_line("You gain experience!");
        db.process_line("You gain experience!");
        // 2 events just now in a 15-min window → rate > 0
        let rate = db.xp_rate_windowed(std::time::Duration::from_secs(900));
        assert!(rate > 0.0);
    }

    #[test]
    fn test_xp_rate_windowed_counts_when_window_exceeds_uptime() {
        let mut db = LootDatabase::new();
        db.xp_event_times.push_back(Instant::now());
        let rate = db.xp_rate_windowed(std::time::Duration::from_secs(86_400));
        assert!(rate > 0.0);
    }

    #[test]
    fn test_parse_say() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso says, 'Hello there!'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Chat(ChatEvent {
                channel: ChatChannel::Say,
                sender: "Soandso".to_string(),
                message: "Hello there!".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_tell_in() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso tells you, 'Need a rez?'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Chat(ChatEvent {
                channel: ChatChannel::Tell,
                sender: "Soandso".to_string(),
                message: "Need a rez?".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_tell_out() {
        let line = "[Thu Mar 28 12:40:00 2026] You told Soandso, 'On my way'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Chat(ChatEvent {
                channel: ChatChannel::TellOut,
                sender: "You".to_string(),
                message: "-> Soandso: On my way".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_group() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso tells the group, 'INC 3'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Chat(ChatEvent {
                channel: ChatChannel::Group,
                sender: "Soandso".to_string(),
                message: "INC 3".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_guild() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso says to your guild, 'Raid at 8pm'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Chat(ChatEvent {
                channel: ChatChannel::Guild,
                sender: "Soandso".to_string(),
                message: "Raid at 8pm".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_shout() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso shouts, 'WTS Fungi!'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Chat(ChatEvent {
                channel: ChatChannel::Shout,
                sender: "Soandso".to_string(),
                message: "WTS Fungi!".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_ooc() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso says out of character, 'Anyone need buffs?'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Chat(ChatEvent {
                channel: ChatChannel::Ooc,
                sender: "Soandso".to_string(),
                message: "Anyone need buffs?".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_auction() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso auctions, 'WTB SoW'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Chat(ChatEvent {
                channel: ChatChannel::Auction,
                sender: "Soandso".to_string(),
                message: "WTB SoW".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_unrecognized_line() {
        let line = "[Thu Mar 28 12:40:00 2026] You begin casting Cure Disease.";
        assert!(parse_log_line(line).is_none());
    }

    #[test]
    fn test_loot_database_accumulates() {
        let mut db = LootDatabase::new();

        db.process_line("--You have looted a Rusty Short Sword.--");
        db.process_line("--You have looted a Rusty Short Sword.--");
        db.process_line("--You have looted a Cloth Cap.--");
        db.process_line("You have slain a moss snake!");
        db.process_line("You have slain a moss snake!");
        db.process_line("You have slain a fire beetle!");
        db.process_line("You receive 5 platinum, 3 gold from the corpse.");
        db.process_line("You receive 2 platinum from the corpse.");
        db.process_line("You gain experience!");
        db.process_line("You gain party experience!");
        db.process_line("You have been slain by a griffon!");

        assert_eq!(db.items["Rusty Short Sword"], 2);
        assert_eq!(db.items["Cloth Cap"], 1);
        assert_eq!(db.kills["a moss snake"], 2);
        assert_eq!(db.kills["a fire beetle"], 1);
        assert_eq!(db.total_plat, 7);
        assert_eq!(db.total_gold, 3);
        assert_eq!(db.total_xp_events, 2);
        assert_eq!(db.deaths, 1);
    }

    // --- Edge case tests ---

    #[test]
    fn test_parse_empty_line() {
        assert!(parse_log_line("").is_none());
    }

    #[test]
    fn test_parse_timestamp_only() {
        assert!(parse_log_line("[Thu Mar 28 12:34:56 2026] ").is_none());
    }

    #[test]
    fn test_parse_malformed_timestamp_no_close_bracket() {
        let line = "[Thu Mar 28 12:34:56 You have looted a Sword.--";
        // No closing ], falls through to raw line parsing
        assert!(parse_log_line(line).is_none());
    }

    #[test]
    fn test_parse_money_copper_only() {
        let line = "You receive 7 copper from the corpse.";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Money {
                plat: 0,
                gold: 0,
                silver: 0,
                copper: 7,
            }
        );
    }

    #[test]
    fn test_parse_money_gold_silver() {
        let line = "You receive 2 gold and 15 silver from the corpse.";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Money {
                plat: 0,
                gold: 2,
                silver: 15,
                copper: 0,
            }
        );
    }

    #[test]
    fn test_parse_raid_chat() {
        let line = "[Thu Mar 28 12:40:00 2026] Raidleader tells the raid, 'Pull to camp!'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Chat(ChatEvent {
                channel: ChatChannel::Raid,
                sender: "Raidleader".to_string(),
                message: "Pull to camp!".to_string(),
            })
        );
    }

    #[test]
    fn test_chat_channel_equality() {
        assert_eq!(ChatChannel::Say, ChatChannel::Say);
        assert_ne!(ChatChannel::Say, ChatChannel::Shout);
        assert_ne!(ChatChannel::Tell, ChatChannel::TellOut);
    }

    #[test]
    fn test_loot_database_process_line_returns_event() {
        let mut db = LootDatabase::new();
        let event = db.process_line("--You have looted a Rusty Axe.--");
        assert!(event.is_some());
        let event = db.process_line("Random unrecognized text");
        assert!(event.is_none());
    }

    #[test]
    fn test_loot_database_zone_enter_no_stats() {
        let mut db = LootDatabase::new();
        db.process_line("[Thu Mar 28 12:39:00 2026] You have entered Kithicor Forest.");
        assert_eq!(db.deaths, 0);
        assert_eq!(db.total_xp_events, 0);
    }

    #[test]
    fn test_loot_database_chat_no_stats() {
        let mut db = LootDatabase::new();
        db.process_line("[Thu Mar 28 12:40:00 2026] Soandso says, 'Hello!'");
        assert_eq!(db.items.len(), 0);
        assert_eq!(db.kills.len(), 0);
    }

    #[test]
    fn test_parse_loot_item_with_special_chars() {
        let line = "--You have looted a Glowing Black Stone.--";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Loot {
                character: String::new(),
                item: "Glowing Black Stone".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_loot_item_strips_control_chars() {
        let line = "--You have looted a \u{1b}[31mRusty Dagger\u{1b}[0m.--";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Loot {
                character: String::new(),
                item: "[31mRusty Dagger[0m".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_kill_named_mob() {
        let line = "You have slain Emperor Crush!";
        let event = parse_log_line(line).unwrap();
        assert_eq!(
            event,
            LogEvent::Kill {
                mob: "Emperor Crush".to_string(),
            }
        );
    }

    #[test]
    fn test_loot_database_defaults() {
        let db = LootDatabase::new();
        assert!(db.items.is_empty());
        assert!(db.kills.is_empty());
        assert_eq!(db.total_plat, 0);
        assert_eq!(db.total_gold, 0);
        assert_eq!(db.total_silver, 0);
        assert_eq!(db.total_copper, 0);
        assert_eq!(db.total_xp_events, 0);
        assert_eq!(db.deaths, 0);
    }
}
