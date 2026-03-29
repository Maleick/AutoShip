use std::collections::HashMap;

/// EQ chat channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatChannel {
    Say,
    Tell,
    TellOut,
    Group,
    Guild,
    Raid,
    Shout,
    Ooc,
    Auction,
}

/// A parsed chat message.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatEvent {
    pub channel: ChatChannel,
    pub sender: String,
    pub message: String,
}

/// Events parsed from EQ log lines.
#[derive(Debug, Clone, PartialEq)]
pub enum LogEvent {
    Loot { character: String, item: String },
    Kill { mob: String },
    Money { plat: u32, gold: u32, silver: u32, copper: u32 },
    Experience { party: bool },
    Death { killed_by: String },
    ZoneEnter { zone: String },
    Chat(ChatEvent),
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
        && let Some(item) = rest.strip_suffix(".--") {
            return Some(LogEvent::Loot {
                character: String::new(),
                item: item.to_string(),
            });
        }

    // You have slain a moss snake!
    if let Some(rest) = text.strip_prefix("You have slain ")
        && let Some(mob) = rest.strip_suffix('!') {
            return Some(LogEvent::Kill {
                mob: mob.to_string(),
            });
        }

    // You have been slain by a moss snake!
    if let Some(rest) = text.strip_prefix("You have been slain by ")
        && let Some(killed_by) = rest.strip_suffix('!') {
            return Some(LogEvent::Death {
                killed_by: killed_by.to_string(),
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
                && let Some(currency) = words.get(i + 1) {
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

        return Some(LogEvent::Money { plat, gold, silver, copper });
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
        && let Some(zone) = rest.strip_suffix('.') {
            return Some(LogEvent::ZoneEnter {
                zone: zone.to_string(),
            });
        }

    // Tell out: "You told Soandso, 'message'"
    if let Some(rest) = text.strip_prefix("You told ") {
        if let Some(rest2) = rest.strip_suffix('\'') {
            if let Some((target, msg)) = rest2.split_once(", '") {
                return Some(LogEvent::Chat(ChatEvent {
                    channel: ChatChannel::TellOut,
                    sender: "You".to_string(),
                    message: format!("-> {}: {}", target, msg),
                }));
            }
        }
    }

    // Chat channels: pattern "Sender <verb>, 'message'"
    if let Some(rest) = text.strip_suffix('\'') {
        if let Some((lhs, msg)) = rest.split_once(", '") {
            let chat = if let Some(sender) = lhs.strip_suffix(" says") {
                Some(ChatEvent { channel: ChatChannel::Say, sender: sender.to_string(), message: msg.to_string() })
            } else if let Some(sender) = lhs.strip_suffix(" tells you") {
                Some(ChatEvent { channel: ChatChannel::Tell, sender: sender.to_string(), message: msg.to_string() })
            } else if let Some(sender) = lhs.strip_suffix(" tells the group") {
                Some(ChatEvent { channel: ChatChannel::Group, sender: sender.to_string(), message: msg.to_string() })
            } else if let Some(sender) = lhs.strip_suffix(" says to your guild") {
                Some(ChatEvent { channel: ChatChannel::Guild, sender: sender.to_string(), message: msg.to_string() })
            } else if let Some(sender) = lhs.strip_suffix(" tells the raid") {
                Some(ChatEvent { channel: ChatChannel::Raid, sender: sender.to_string(), message: msg.to_string() })
            } else if let Some(sender) = lhs.strip_suffix(" shouts") {
                Some(ChatEvent { channel: ChatChannel::Shout, sender: sender.to_string(), message: msg.to_string() })
            } else if let Some(sender) = lhs.strip_suffix(" says out of character") {
                Some(ChatEvent { channel: ChatChannel::Ooc, sender: sender.to_string(), message: msg.to_string() })
            } else if let Some(sender) = lhs.strip_suffix(" auctions") {
                Some(ChatEvent { channel: ChatChannel::Auction, sender: sender.to_string(), message: msg.to_string() })
            } else {
                None
            };
            if let Some(event) = chat {
                return Some(LogEvent::Chat(event));
            }
        }
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
}

impl LootDatabase {
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
            LogEvent::Money { plat, gold, silver, copper } => {
                self.total_plat += *plat as u64;
                self.total_gold += *gold as u64;
                self.total_silver += *silver as u64;
                self.total_copper += *copper as u64;
            }
            LogEvent::Experience { .. } => {
                self.total_xp_events += 1;
            }
            LogEvent::Death { .. } => {
                self.deaths += 1;
            }
            LogEvent::ZoneEnter { .. } => {}
            LogEvent::Chat(_) => {}
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
    fn test_parse_say() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso says, 'Hello there!'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(event, LogEvent::Chat(ChatEvent {
            channel: ChatChannel::Say,
            sender: "Soandso".to_string(),
            message: "Hello there!".to_string(),
        }));
    }

    #[test]
    fn test_parse_tell_in() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso tells you, 'Need a rez?'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(event, LogEvent::Chat(ChatEvent {
            channel: ChatChannel::Tell,
            sender: "Soandso".to_string(),
            message: "Need a rez?".to_string(),
        }));
    }

    #[test]
    fn test_parse_tell_out() {
        let line = "[Thu Mar 28 12:40:00 2026] You told Soandso, 'On my way'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(event, LogEvent::Chat(ChatEvent {
            channel: ChatChannel::TellOut,
            sender: "You".to_string(),
            message: "-> Soandso: On my way".to_string(),
        }));
    }

    #[test]
    fn test_parse_group() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso tells the group, 'INC 3'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(event, LogEvent::Chat(ChatEvent {
            channel: ChatChannel::Group,
            sender: "Soandso".to_string(),
            message: "INC 3".to_string(),
        }));
    }

    #[test]
    fn test_parse_guild() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso says to your guild, 'Raid at 8pm'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(event, LogEvent::Chat(ChatEvent {
            channel: ChatChannel::Guild,
            sender: "Soandso".to_string(),
            message: "Raid at 8pm".to_string(),
        }));
    }

    #[test]
    fn test_parse_shout() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso shouts, 'WTS Fungi!'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(event, LogEvent::Chat(ChatEvent {
            channel: ChatChannel::Shout,
            sender: "Soandso".to_string(),
            message: "WTS Fungi!".to_string(),
        }));
    }

    #[test]
    fn test_parse_ooc() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso says out of character, 'Anyone need buffs?'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(event, LogEvent::Chat(ChatEvent {
            channel: ChatChannel::Ooc,
            sender: "Soandso".to_string(),
            message: "Anyone need buffs?".to_string(),
        }));
    }

    #[test]
    fn test_parse_auction() {
        let line = "[Thu Mar 28 12:40:00 2026] Soandso auctions, 'WTB SoW'";
        let event = parse_log_line(line).unwrap();
        assert_eq!(event, LogEvent::Chat(ChatEvent {
            channel: ChatChannel::Auction,
            sender: "Soandso".to_string(),
            message: "WTB SoW".to_string(),
        }));
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
}
