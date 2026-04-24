//! Shared box-chat configuration, routing, and wire types.

use serde::{Deserialize, Serialize};

use crate::box_controller::{BoxControllerClientState, BoxControllerCommand};

/// Configuration for the EQBC-style TCP relay used by TextQuest instances.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoxChatConfig {
    /// Enable the box-chat runtime in long-lived TextQuest processes.
    pub enabled: bool,
    /// Server host to connect to when `auto_connect` is enabled.
    pub host: String,
    /// Shared TCP port used by the relay server.
    pub port: u16,
    /// Maintain an outbound connection to `host:port`.
    pub auto_connect: bool,
}

impl Default for BoxChatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: "127.0.0.1".to_string(),
            port: 2112,
            auto_connect: false,
        }
    }
}

/// Minimal bridge for peers that speak legacy EQBC-style line commands instead
/// of TextQuest's native JSON-line messages.
pub trait EqbcLineCodec {
    /// Return a legacy line representation when this message can be expressed
    /// as an EQBC-style text command.
    fn to_eqbc_line(&self) -> Option<String>;
}

/// Parsed outbound route for a local `/bc`-style command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboundRoute {
    /// Broadcast a slash command to every connected client.
    Broadcast { command: String },
    /// Send a slash command to one named character.
    Target { character: String, command: String },
}

impl OutboundRoute {
    /// Convert this local route into the submit message sent to a relay hub.
    #[must_use]
    pub fn submit_message(&self) -> WireMessage {
        match self {
            Self::Broadcast { command } => WireMessage::Broadcast {
                command: command.clone(),
            },
            Self::Target { character, command } => WireMessage::Target {
                character: character.clone(),
                command: command.clone(),
            },
        }
    }

    /// Convert this route into the execute message delivered by a relay hub.
    #[must_use]
    pub fn execute_message(&self) -> WireMessage {
        match self {
            Self::Broadcast { command } => WireMessage::ExecuteBroadcast {
                command: command.clone(),
            },
            Self::Target { character, command } => WireMessage::ExecuteTarget {
                character: character.clone(),
                command: command.clone(),
            },
        }
    }

    /// Render this route as the EQBC command family operators expect.
    #[must_use]
    pub fn eqbc_command_line(&self) -> String {
        match self {
            Self::Broadcast { command } => format!("/bc {command}"),
            Self::Target { character, command } => format!("/bct {character} {command}"),
        }
    }
}

/// JSON-line message exchanged between TextQuest box-chat peers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WireMessage {
    /// Initial hello sent after establishing a TCP connection.
    Hello {
        node_name: String,
        characters: Vec<String>,
    },
    /// Update the set of characters currently owned by a peer.
    UpdateCharacters { characters: Vec<String> },
    /// Submit a broadcast route to the hub.
    Broadcast { command: String },
    /// Submit a targeted route to the hub.
    Target { character: String, command: String },
    /// Forward an in-game tell over the relay without executing a slash
    /// command on receivers.
    TellForward {
        from: String,
        to: String,
        message: String,
    },
    /// Publish an in-game channel message over the relay without executing a
    /// slash command on receivers.
    ChannelBroadcast {
        channel: String,
        from: String,
        message: String,
    },
    /// Broadcast a structured unified controller command to all connected
    /// clients.
    BoxControllerCommand { command: BoxControllerCommand },
    /// Publish the latest automation state for one node's connected clients.
    BoxControllerState {
        node_name: String,
        clients: Vec<BoxControllerClientState>,
    },
    /// Execute a broadcast on the receiving peer.
    ExecuteBroadcast { command: String },
    /// Execute a targeted command on the receiving peer.
    ExecuteTarget { character: String, command: String },
}

impl EqbcLineCodec for WireMessage {
    fn to_eqbc_line(&self) -> Option<String> {
        match self {
            Self::Broadcast { command } | Self::ExecuteBroadcast { command } => Some(
                OutboundRoute::Broadcast {
                    command: command.clone(),
                }
                .eqbc_command_line(),
            ),
            Self::Target { character, command } | Self::ExecuteTarget { character, command } => {
                Some(
                    OutboundRoute::Target {
                        character: character.clone(),
                        command: command.clone(),
                    }
                    .eqbc_command_line(),
                )
            }
            Self::TellForward { from, to, message } => {
                Some(format!("/bc [tell] {from} -> {to}: {message}"))
            }
            Self::ChannelBroadcast {
                channel,
                from,
                message,
            } => Some(format!("/bc [{channel}] {from}: {message}")),
            Self::Hello { .. }
            | Self::UpdateCharacters { .. }
            | Self::BoxControllerCommand { .. }
            | Self::BoxControllerState { .. } => None,
        }
    }
}

/// Parse an EQBC-style slash command into a structured outbound route.
///
/// Supported forms:
/// - `/bc /command`
/// - `/bca //command`
/// - `/bcaa //command`
/// - `/bct Character //command`
#[must_use]
pub fn parse_slash_route(input: &str) -> Option<Result<OutboundRoute, String>> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let command = parts.next()?.to_ascii_lowercase();
    let rest = parts.next().unwrap_or("").trim();

    match command.as_str() {
        "/bc" | "/bca" | "/bcaa" => {
            Some(normalize_payload(rest).map(|command| OutboundRoute::Broadcast { command }))
        }
        "/bct" => Some(parse_target_route(rest)),
        _ => None,
    }
}

/// Parse a legacy EQBC line into the native TextQuest wire message that should
/// be submitted to the relay hub.
#[must_use]
pub fn parse_eqbc_line(input: &str) -> Option<Result<WireMessage, String>> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.starts_with('{') {
        return None;
    }

    let route_input = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };

    parse_slash_route(&route_input).map(|result| result.map(|route| route.submit_message()))
}

fn parse_target_route(rest: &str) -> Result<OutboundRoute, String> {
    let mut parts = rest.splitn(2, char::is_whitespace);
    let character = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Missing target character for /bct".to_string())?;
    let payload = parts.next().unwrap_or("").trim();
    let command = normalize_payload(payload)?;
    Ok(OutboundRoute::Target {
        character: character.to_string(),
        command,
    })
}

fn normalize_payload(payload: &str) -> Result<String, String> {
    let trimmed = payload.trim();
    if trimmed.is_empty() {
        return Err("Missing box chat payload".to_string());
    }

    if let Some(stripped) = trimmed.strip_prefix("//") {
        let command = stripped.trim_start();
        if command.is_empty() {
            return Err("Box chat payload must contain a slash command".to_string());
        }
        return Ok(format!("/{command}"));
    }

    if trimmed.starts_with('/') {
        return Ok(trimmed.to_string());
    }

    Err("Box chat payload must start with '/' or '//'".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_eqbc_line_accepts_unslashed_broadcast_alias() {
        let parsed = parse_eqbc_line("bc //sit")
            .expect("legacy line should be recognized")
            .expect("legacy line should parse");

        assert_eq!(
            parsed,
            WireMessage::Broadcast {
                command: "/sit".to_string()
            }
        );
    }

    #[test]
    fn parse_eqbc_line_accepts_target_alias() {
        let parsed = parse_eqbc_line("bct Cleric01 //cast 1")
            .expect("legacy target should be recognized")
            .expect("legacy target should parse");

        assert_eq!(
            parsed,
            WireMessage::Target {
                character: "Cleric01".to_string(),
                command: "/cast 1".to_string()
            }
        );
    }

    #[test]
    fn wire_message_renders_legacy_target_line() {
        let line = WireMessage::ExecuteTarget {
            character: "Cleric01".to_string(),
            command: "/cast 1".to_string(),
        }
        .to_eqbc_line();

        assert_eq!(line.as_deref(), Some("/bct Cleric01 /cast 1"));
    }
}
