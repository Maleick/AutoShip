//! Chat event types and STML parsing utilities.
//!
//! This module provides the shared `ChatChannel` and `ChatEvent` types used by
//! both the injected DLL (to produce structured events from `dsp_chat`
//! intercepts) and the external orchestrator (to consume them from IPC and log
//! files).
//!
//! # STML stripping
//!
//! EverQuest uses a simple markup language (STML) for in-game text with tags
//! like `<BR>`, `<c "#FF0000">`, and `</c>`.  [`strip_stml`] removes all such
//! sequences and collapses whitespace so that downstream parsers see plain
//! ASCII.
//!
//! # Chat parsing
//!
//! [`parse_chat_text`] accepts a raw `dsp_chat` string (with or without STML
//! tags), strips markup, and pattern-matches the EQ verb syntax to produce a
//! [`ChatEvent`].
//!
//! # Chat output logging
//!
//! MQ2Log-style per-character chat output logging is configured via
//! [`ChatLogConfig`].

use serde::{Deserialize, Serialize};

/// EQ chat channel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChatChannel {
    /// Local /say channel.
    Say,
    /// Incoming /tell (private message).
    Tell,
    /// Outgoing /tell sent by the player.
    TellOut,
    /// Group chat channel.
    Group,
    /// Guild chat channel.
    Guild,
    /// Raid chat channel.
    Raid,
    /// Zone-wide /shout channel.
    Shout,
    /// Out-of-character chat channel.
    Ooc,
    /// /auction channel.
    Auction,
}

/// A structured chat message extracted from EQ `dsp_chat` output or a log file
/// line.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChatEvent {
    /// Which chat channel this message was on.
    pub channel: ChatChannel,
    /// Name of the sender (or `"You"` for outgoing tells).
    pub sender: String,
    /// The plain-text message content (STML tags already stripped).
    pub message: String,
}

/// Log rotation strategy for per-character chat log files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogRotation {
    /// No rotation — append to a single log file indefinitely.
    None,
    /// Rotate daily at midnight.
    Daily,
    /// Rotate when the file exceeds the specified size in bytes.
    BySize(u64),
}

impl Default for LogRotation {
    fn default() -> Self {
        Self::Daily
    }
}

/// Log level filter for chat output logging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    /// Log all captured chat output.
    Info,
    /// Log all captured chat output with additional debug metadata.
    Debug,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

/// Configuration for per-character MQ2Log-style chat output logging.
///
/// When enabled, all MQ2 output is written to `logs/server_charname.log`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatLogConfig {
    /// Enable per-character chat output logging.
    pub enabled: bool,
    /// Log rotation strategy.
    pub rotation: LogRotation,
    /// Log level filter.
    pub level: LogLevel,
    /// EQ chat channels to log. If empty, all channels are logged.
    pub channels: Vec<ChatChannel>,
}

impl Default for ChatLogConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            rotation: LogRotation::default(),
            level: LogLevel::default(),
            channels: Vec::new(),
        }
    }
}

/// Strip STML/HTML-like markup tags from EQ text.
///
/// EQ uses a simple markup language (STML) for in-game and dialog text with
/// tags like `<BR>`, `<c "#FF0000">`, and `</c>`.  This function removes all
/// `<…>` sequences and collapses runs of whitespace so that downstream parsers
/// see plain ASCII.
///
/// # Examples
///
/// ```
/// use textquest_common::chat::strip_stml;
/// assert_eq!(strip_stml("<c \"#FF0000\">Error:</c> bad password"), "Error: bad password");
/// assert_eq!(strip_stml("Line one<BR>Line two"), "Line oneLine two");
/// assert_eq!(strip_stml("plain text"), "plain text");
/// ```
pub fn strip_stml(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;

    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }

    // Collapse runs of whitespace into a single space and trim the edges.
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parse raw `dsp_chat` text (possibly STML-tagged) into a structured
/// [`ChatEvent`].
///
/// The function first calls [`strip_stml`] to remove all markup tags, then
/// pattern-matches the EQ verb syntax:
///
/// | Pattern | Channel |
/// |---|---|
/// | `"You told {target}, '{msg}'"` | [`ChatChannel::TellOut`] |
/// | `"{sender} says, '{msg}'"` | [`ChatChannel::Say`] |
/// | `"You say, '{msg}'"` | [`ChatChannel::Say`] |
/// | `"{sender} tells you, '{msg}'"` | [`ChatChannel::Tell`] |
/// | `"{sender} tells the group, '{msg}'"` | [`ChatChannel::Group`] |
/// | `"You tell the group, '{msg}'"` | [`ChatChannel::Group`] |
/// | `"{sender} says to your guild, '{msg}'"` | [`ChatChannel::Guild`] |
/// | `"You say to your guild, '{msg}'"` | [`ChatChannel::Guild`] |
/// | `"{sender} tells the raid, '{msg}'"` | [`ChatChannel::Raid`] |
/// | `"You tell the raid, '{msg}'"` | [`ChatChannel::Raid`] |
/// | `"{sender} shouts, '{msg}'"` | [`ChatChannel::Shout`] |
/// | `"You shout, '{msg}'"` | [`ChatChannel::Shout`] |
/// | `"{sender} says out of character, '{msg}'"` | [`ChatChannel::Ooc`] |
/// | `"You say out of character, '{msg}'"` | [`ChatChannel::Ooc`] |
/// | `"{sender} auctions, '{msg}'"` | [`ChatChannel::Auction`] |
/// | `"You auction, '{msg}'"` | [`ChatChannel::Auction`] |
///
/// Returns `None` if the text does not match any known pattern.
///
/// # Examples
///
/// ```
/// use textquest_common::chat::{parse_chat_text, ChatChannel, ChatEvent};
///
/// let event = parse_chat_text("Soandso says, 'Hello!'").unwrap();
/// assert_eq!(event.channel, ChatChannel::Say);
/// assert_eq!(event.sender, "Soandso");
/// assert_eq!(event.message, "Hello!");
///
/// // STML tags are stripped automatically.
/// let event = parse_chat_text("<c \"#FF0000\">Soandso tells you, 'Need a rez?'</c>").unwrap();
/// assert_eq!(event.channel, ChatChannel::Tell);
/// assert_eq!(event.message, "Need a rez?");
///
/// // Unrecognised text returns None.
/// assert!(parse_chat_text("You gain experience!").is_none());
/// ```
pub fn parse_chat_text(raw: &str) -> Option<ChatEvent> {
    let text = strip_stml(raw);
    parse_stripped_chat_text(&text)
}

/// Parse already-stripped (no STML tags) chat text into a [`ChatEvent`].
///
/// This is the core parsing logic shared by [`parse_chat_text`] and the
/// log-file parser (which has already stripped the EQ timestamp prefix).
pub fn parse_stripped_chat_text(text: &str) -> Option<ChatEvent> {
    // Tell out: "You told Soandso, 'message'"
    if let Some(rest) = text.strip_prefix("You told ")
        && let Some(rest2) = rest.strip_suffix('\'')
        && let Some((target, msg)) = rest2.split_once(", '")
    {
        return Some(ChatEvent {
            channel: ChatChannel::TellOut,
            sender: "You".to_string(),
            message: format!("-> {target}: {msg}"),
        });
    }

    // Chat channels: pattern "Sender <verb>, 'message'"
    if let Some(rest) = text.strip_suffix('\'')
        && let Some((lhs, msg)) = rest.split_once(", '")
    {
        let event = if let Some(sender) = lhs.strip_suffix(" says") {
            Some(ChatEvent {
                channel: ChatChannel::Say,
                sender: sender.to_string(),
                message: msg.to_string(),
            })
        } else if let Some(sender) = lhs.strip_suffix(" tells you") {
            Some(ChatEvent {
                channel: ChatChannel::Tell,
                sender: sender.to_string(),
                message: msg.to_string(),
            })
        } else if let Some(sender) = lhs.strip_suffix(" tells the group") {
            Some(ChatEvent {
                channel: ChatChannel::Group,
                sender: sender.to_string(),
                message: msg.to_string(),
            })
        } else if let Some(sender) = lhs.strip_suffix(" says to your guild") {
            Some(ChatEvent {
                channel: ChatChannel::Guild,
                sender: sender.to_string(),
                message: msg.to_string(),
            })
        } else if let Some(sender) = lhs.strip_suffix(" tells the raid") {
            Some(ChatEvent {
                channel: ChatChannel::Raid,
                sender: sender.to_string(),
                message: msg.to_string(),
            })
        } else if let Some(sender) = lhs.strip_suffix(" shouts") {
            Some(ChatEvent {
                channel: ChatChannel::Shout,
                sender: sender.to_string(),
                message: msg.to_string(),
            })
        } else if let Some(sender) = lhs.strip_suffix(" says out of character") {
            Some(ChatEvent {
                channel: ChatChannel::Ooc,
                sender: sender.to_string(),
                message: msg.to_string(),
            })
        } else if let Some(sender) = lhs.strip_suffix(" auctions") {
            Some(ChatEvent {
                channel: ChatChannel::Auction,
                sender: sender.to_string(),
                message: msg.to_string(),
            })
        // Self-authored channel messages: EQ uses singular verb forms for the
        // local player. These must be explicitly matched because the
        // sender-prefix strip_suffix approach only works for
        // third-person verbs (" says", " shouts", etc.).
        } else if lhs == "You say" {
            Some(ChatEvent {
                channel: ChatChannel::Say,
                sender: "You".to_string(),
                message: msg.to_string(),
            })
        } else if lhs == "You shout" {
            Some(ChatEvent {
                channel: ChatChannel::Shout,
                sender: "You".to_string(),
                message: msg.to_string(),
            })
        } else if lhs == "You say out of character" {
            Some(ChatEvent {
                channel: ChatChannel::Ooc,
                sender: "You".to_string(),
                message: msg.to_string(),
            })
        } else if lhs == "You auction" {
            Some(ChatEvent {
                channel: ChatChannel::Auction,
                sender: "You".to_string(),
                message: msg.to_string(),
            })
        } else if lhs == "You tell the group" {
            Some(ChatEvent {
                channel: ChatChannel::Group,
                sender: "You".to_string(),
                message: msg.to_string(),
            })
        } else if lhs == "You tell the raid" {
            Some(ChatEvent {
                channel: ChatChannel::Raid,
                sender: "You".to_string(),
                message: msg.to_string(),
            })
        } else if lhs == "You say to your guild" {
            Some(ChatEvent {
                channel: ChatChannel::Guild,
                sender: "You".to_string(),
                message: msg.to_string(),
            })
        } else {
            None
        };
        if event.is_some() {
            return event;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── strip_stml ──────────────────────────────────────────────────────────

    #[test]
    fn strip_stml_removes_color_tags() {
        assert_eq!(
            strip_stml("<c \"#FF0000\">Error:</c> Your password is incorrect."),
            "Error: Your password is incorrect."
        );
    }

    #[test]
    fn strip_stml_handles_br_tags() {
        assert_eq!(strip_stml("Line one<BR>Line two"), "Line oneLine two");
        assert_eq!(strip_stml("Line one <BR> Line two"), "Line one Line two");
    }

    #[test]
    fn strip_stml_collapses_whitespace() {
        assert_eq!(
            strip_stml("  multiple   spaces  here  "),
            "multiple spaces here"
        );
    }

    #[test]
    fn strip_stml_plain_text_unchanged() {
        assert_eq!(strip_stml("No tags here"), "No tags here");
    }

    #[test]
    fn strip_stml_empty_string() {
        assert_eq!(strip_stml(""), "");
    }

    // ── parse_chat_text ─────────────────────────────────────────────────────

    #[test]
    fn parse_say() {
        let ev = parse_chat_text("Soandso says, 'Hello there!'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Say);
        assert_eq!(ev.sender, "Soandso");
        assert_eq!(ev.message, "Hello there!");
    }

    #[test]
    fn parse_tell_in() {
        let ev = parse_chat_text("Soandso tells you, 'Need a rez?'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Tell);
        assert_eq!(ev.sender, "Soandso");
        assert_eq!(ev.message, "Need a rez?");
    }

    #[test]
    fn parse_tell_out() {
        let ev = parse_chat_text("You told Soandso, 'On my way'").unwrap();
        assert_eq!(ev.channel, ChatChannel::TellOut);
        assert_eq!(ev.sender, "You");
        assert_eq!(ev.message, "-> Soandso: On my way");
    }

    #[test]
    fn parse_group() {
        let ev = parse_chat_text("Soandso tells the group, 'INC 3'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Group);
        assert_eq!(ev.sender, "Soandso");
        assert_eq!(ev.message, "INC 3");
    }

    #[test]
    fn parse_guild() {
        let ev = parse_chat_text("Soandso says to your guild, 'Good fight!'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Guild);
        assert_eq!(ev.sender, "Soandso");
        assert_eq!(ev.message, "Good fight!");
    }

    #[test]
    fn parse_raid() {
        let ev = parse_chat_text("Raidleader tells the raid, 'Pull to camp!'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Raid);
        assert_eq!(ev.sender, "Raidleader");
        assert_eq!(ev.message, "Pull to camp!");
    }

    #[test]
    fn parse_shout() {
        let ev = parse_chat_text("Soandso shouts, 'WTS Fungi!'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Shout);
        assert_eq!(ev.sender, "Soandso");
        assert_eq!(ev.message, "WTS Fungi!");
    }

    #[test]
    fn parse_ooc() {
        let ev = parse_chat_text("Soandso says out of character, 'Anyone need buffs?'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Ooc);
        assert_eq!(ev.sender, "Soandso");
        assert_eq!(ev.message, "Anyone need buffs?");
    }

    #[test]
    fn parse_auction() {
        let ev = parse_chat_text("Merchant auctions, 'WTB Fungi Tunic'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Auction);
        assert_eq!(ev.sender, "Merchant");
        assert_eq!(ev.message, "WTB Fungi Tunic");
    }

    #[test]
    fn parse_unrecognised_returns_none() {
        assert!(parse_chat_text("You gain experience!").is_none());
        assert!(parse_chat_text("").is_none());
        assert!(parse_chat_text("Some random system message.").is_none());
    }

    #[test]
    fn parse_you_say() {
        let ev = parse_chat_text("You say, 'Hello there!'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Say);
        assert_eq!(ev.sender, "You");
        assert_eq!(ev.message, "Hello there!");
    }

    #[test]
    fn parse_you_shout() {
        let ev = parse_chat_text("You shout, 'WTS Fungi!'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Shout);
        assert_eq!(ev.sender, "You");
        assert_eq!(ev.message, "WTS Fungi!");
    }

    #[test]
    fn parse_you_say_out_of_character() {
        let ev = parse_chat_text("You say out of character, 'Anyone need buffs?'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Ooc);
        assert_eq!(ev.sender, "You");
        assert_eq!(ev.message, "Anyone need buffs?");
    }

    #[test]
    fn parse_you_auction() {
        let ev = parse_chat_text("You auction, 'WTB Fungi Tunic'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Auction);
        assert_eq!(ev.sender, "You");
        assert_eq!(ev.message, "WTB Fungi Tunic");
    }

    #[test]
    fn parse_you_tell_the_group() {
        let ev = parse_chat_text("You tell the group, 'INC 3'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Group);
        assert_eq!(ev.sender, "You");
        assert_eq!(ev.message, "INC 3");
    }

    #[test]
    fn parse_you_tell_the_raid() {
        let ev = parse_chat_text("You tell the raid, 'Pull to camp!'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Raid);
        assert_eq!(ev.sender, "You");
        assert_eq!(ev.message, "Pull to camp!");
    }

    #[test]
    fn parse_you_say_to_your_guild() {
        let ev = parse_chat_text("You say to your guild, 'Good fight!'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Guild);
        assert_eq!(ev.sender, "You");
        assert_eq!(ev.message, "Good fight!");
    }

    #[test]
    fn parse_you_say_spoofed_cast_feedback_is_structured() {
        // This is the critical security test: a player saying a spoofed cast-feedback
        // phrase must be recognized as structured channel chat (not None), so that
        // should_forward_to_combat returns false and it never reaches cast-outcome
        // parsing.
        let ev =
            parse_chat_text("You say, 'You don\\'t have enough mana to cast this spell.'").unwrap();
        assert_eq!(ev.channel, ChatChannel::Say);
        assert_eq!(ev.sender, "You");
        // The message body itself is irrelevant here — what matters is that
        // `Some` is returned.
    }

    #[test]
    fn parse_strips_stml_before_matching() {
        let ev = parse_chat_text("<c \"#FF0000\">Soandso tells you, 'Need a rez?'</c>").unwrap();
        assert_eq!(ev.channel, ChatChannel::Tell);
        assert_eq!(ev.sender, "Soandso");
        assert_eq!(ev.message, "Need a rez?");
    }

    #[test]
    fn parse_stml_tagged_group_message() {
        let ev =
            parse_chat_text("<c \"#00FF00\">Healer tells the group, 'CH chain go!'</c>").unwrap();
        assert_eq!(ev.channel, ChatChannel::Group);
        assert_eq!(ev.sender, "Healer");
        assert_eq!(ev.message, "CH chain go!");
    }
}
