//! Chat event types and STML parsing utilities.
//!
//! This module provides the shared `ChatChannel` and `ChatEvent` types used by both the
//! injected DLL (to produce structured events from `dsp_chat` intercepts) and the
//! external orchestrator (to consume them from IPC and log files).
//!
//! # STML stripping
//!
//! EverQuest uses a simple markup language (STML) for in-game text with tags like
//! `<BR>`, `<c "#FF0000">`, and `</c>`.  [`strip_stml`] removes all such sequences and
//! collapses whitespace so that downstream parsers see plain ASCII.
//!
//! # Chat parsing
//!
//! [`parse_chat_text`] accepts a raw `dsp_chat` string (with or without STML tags),
//! strips markup, and pattern-matches the EQ verb syntax to produce a [`ChatEvent`].

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

/// A structured chat message extracted from EQ `dsp_chat` output or a log file line.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChatEvent {
    /// Which chat channel this message was on.
    pub channel: ChatChannel,
    /// Name of the sender (or `"You"` for outgoing tells).
    pub sender: String,
    /// The plain-text message content (STML tags already stripped).
    pub message: String,
}

/// Strip STML/HTML-like markup tags from EQ text.
///
/// EQ uses a simple markup language (STML) for in-game and dialog text with tags like
/// `<BR>`, `<c "#FF0000">`, and `</c>`.  This function removes all `<…>` sequences and
/// collapses runs of whitespace so that downstream parsers see plain ASCII.
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

/// Parse raw `dsp_chat` text (possibly STML-tagged) into a structured [`ChatEvent`].
///
/// The function first calls [`strip_stml`] to remove all markup tags, then
/// pattern-matches the EQ verb syntax:
///
/// | Pattern | Channel |
/// |---|---|
/// | `"You told {target}, '{msg}'"` | [`ChatChannel::TellOut`] |
/// | `"{sender} says, '{msg}'"` | [`ChatChannel::Say`] |
/// | `"{sender} tells you, '{msg}'"` | [`ChatChannel::Tell`] |
/// | `"{sender} tells the group, '{msg}'"` | [`ChatChannel::Group`] |
/// | `"{sender} says to your guild, '{msg}'"` | [`ChatChannel::Guild`] |
/// | `"{sender} tells the raid, '{msg}'"` | [`ChatChannel::Raid`] |
/// | `"{sender} shouts, '{msg}'"` | [`ChatChannel::Shout`] |
/// | `"{sender} says out of character, '{msg}'"` | [`ChatChannel::Ooc`] |
/// | `"{sender} auctions, '{msg}'"` | [`ChatChannel::Auction`] |
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
/// This is the core parsing logic shared by [`parse_chat_text`] and the log-file
/// parser (which has already stripped the EQ timestamp prefix).
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
        } else {
            lhs.strip_suffix(" auctions").map(|sender| ChatEvent {
                channel: ChatChannel::Auction,
                sender: sender.to_string(),
                message: msg.to_string(),
            })
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
