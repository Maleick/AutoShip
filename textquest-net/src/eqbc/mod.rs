//! EQBC-compatible TCP transport.
//!
//! Implements both the server hub (EQBCS) and per-character client.
//!
//! # Wire protocol
//!
//! EQBC uses newline-terminated ASCII lines over TCP.  Each line is one of:
//!
//! ```text
//! IDENTITY:<charname>              client→server  register name
//! BROADCAST:<command>             client→server  /bc equivalent
//! TARGET:<charname>:<command>     client→server  /bct equivalent
//! EXECUTE:<command>               server→client  run this command
//! PEERS:<name1>,<name2>,...       server→client  connected peer list
//! PING                            bidirectional  keepalive
//! PONG                            bidirectional  keepalive reply
//! ```

pub mod client;
pub mod server;

pub use client::EqbcClient;
pub use server::EqbcServer;

/// Maximum line length accepted from any peer. Prevents memory exhaustion from
/// malformed or adversarial clients.
pub const MAX_LINE_BYTES: usize = 4096;

/// Decoded EQBC wire message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EqbcMessage {
    Identity { name: String },
    Broadcast { command: String },
    Target { name: String, command: String },
    Execute { command: String },
    Peers { names: Vec<String> },
    Ping,
    Pong,
}

impl EqbcMessage {
    /// Parse one line of EQBC wire text.
    ///
    /// # Errors
    ///
    /// Returns `None` for unrecognised or malformed lines; callers should
    /// silently skip those.
    #[must_use]
    pub fn parse(line: &str) -> Option<Self> {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.eq_ignore_ascii_case("PING") {
            return Some(Self::Ping);
        }
        if line.eq_ignore_ascii_case("PONG") {
            return Some(Self::Pong);
        }
        if let Some(rest) = line.strip_prefix("IDENTITY:") {
            let name = rest.trim().to_string();
            if name.is_empty() {
                return None;
            }
            return Some(Self::Identity { name });
        }
        if let Some(rest) = line.strip_prefix("BROADCAST:") {
            return Some(Self::Broadcast {
                command: rest.to_string(),
            });
        }
        if let Some(rest) = line.strip_prefix("TARGET:") {
            let (name, command) = rest.split_once(':')?;
            return Some(Self::Target {
                name: name.to_string(),
                command: command.to_string(),
            });
        }
        if let Some(rest) = line.strip_prefix("EXECUTE:") {
            return Some(Self::Execute {
                command: rest.to_string(),
            });
        }
        if let Some(rest) = line.strip_prefix("PEERS:") {
            let names = rest
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            return Some(Self::Peers { names });
        }
        None
    }

    /// Encode this message to a wire line (newline-terminated).
    #[must_use]
    pub fn encode(&self) -> String {
        match self {
            Self::Identity { name } => format!("IDENTITY:{name}\n"),
            Self::Broadcast { command } => format!("BROADCAST:{command}\n"),
            Self::Target { name, command } => format!("TARGET:{name}:{command}\n"),
            Self::Execute { command } => format!("EXECUTE:{command}\n"),
            Self::Peers { names } => format!("PEERS:{}\n", names.join(",")),
            Self::Ping => "PING\n".to_string(),
            Self::Pong => "PONG\n".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_broadcast() {
        let msg = EqbcMessage::Broadcast {
            command: "/assist".to_string(),
        };
        let line = msg.encode();
        assert_eq!(EqbcMessage::parse(&line), Some(msg));
    }

    #[test]
    fn round_trip_target() {
        let msg = EqbcMessage::Target {
            name: "Warrior".to_string(),
            command: "/attack".to_string(),
        };
        let line = msg.encode();
        assert_eq!(EqbcMessage::parse(&line), Some(msg));
    }

    #[test]
    fn parse_peers() {
        let msg = EqbcMessage::parse("PEERS:Alice,Bob,Carol\n");
        assert_eq!(
            msg,
            Some(EqbcMessage::Peers {
                names: vec!["Alice".into(), "Bob".into(), "Carol".into()],
            })
        );
    }

    #[test]
    fn unknown_line_returns_none() {
        assert!(EqbcMessage::parse("GARBAGE:stuff").is_none());
    }
}
