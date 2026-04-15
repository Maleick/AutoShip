//! Shared box-chat configuration, routing, and wire types.

use serde::{Deserialize, Serialize};

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

/// Parsed outbound route for a local `/bc`-style command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboundRoute {
    /// Broadcast a slash command to every connected client.
    Broadcast { command: String },
    /// Send a slash command to one named character.
    Target { character: String, command: String },
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
    /// Execute a broadcast on the receiving peer.
    ExecuteBroadcast { command: String },
    /// Execute a targeted command on the receiving peer.
    ExecuteTarget { character: String, command: String },
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
