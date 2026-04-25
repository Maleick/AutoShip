//! DanNet-compatible peer-to-peer transport.
//!
//! Implements the core DanNet semantics that rgmercs depends on:
//! - `/dnet observe <peer> <tlo_query>` — subscribe to a TLO value
//! - `/dnet unobserve <peer> <tlo_query>`
//! - `/dgae <command>` — execute on all peers in my server group
//! - `/dggaexecute <group> <command>` — execute on named group
//! - `${DanNet[<peer>].Q[<query>]}` TLO compatibility (via TLO stubs in DLL)
//!
//! # Wire protocol
//!
//! JSON-newline over UDP.  Each datagram contains one [`DanNetFrame`].
//! Max frame size is 8 KB — matches the peer-discovery crate limit.

pub mod peer;

pub use peer::{DanNetNode, NodeEvent};

/// Maximum UDP datagram payload size accepted.
pub const MAX_FRAME_BYTES: usize = 8 * 1024;

use serde::{Deserialize, Serialize};

/// DanNet wire frame sent between peers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DanNetFrame {
    /// Announce presence on the network.
    Hello {
        peer: String,
        group: String,
        instance_id: u64,
    },
    /// Execute a command on every peer in a group.
    GroupExecute { group: String, command: String },
    /// Execute a command on a specific named peer.
    PeerExecute { target: String, command: String },
    /// Observe request: sender wants `query` value from `target` periodically.
    Observe {
        requester: String,
        target: String,
        query: String,
    },
    /// Stop observing a query.
    Unobserve {
        requester: String,
        target: String,
        query: String,
    },
    /// Observation response carrying the current TLO value.
    ObserveReply {
        target: String,
        query: String,
        value: String,
    },
    /// Graceful goodbye.
    Bye { peer: String, instance_id: u64 },
}

impl DanNetFrame {
    /// Encode to JSON bytes (no trailing newline — this is a datagram protocol).
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Decode from JSON bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if parsing fails or the frame is oversized.
    pub fn decode(bytes: &[u8]) -> anyhow::Result<Self> {
        anyhow::ensure!(
            bytes.len() <= MAX_FRAME_BYTES,
            "DanNet frame too large: {} bytes",
            bytes.len()
        );
        Ok(serde_json::from_slice(bytes)?)
    }
}

/// Parsed `/dnet` slash-command variants.
#[derive(Debug, Clone, PartialEq)]
pub enum DnetCommand {
    /// `/dgae <command>` — group all execute
    GroupAllExecute { command: String },
    /// `/dggaexecute <group> <command>` — named group execute
    GroupExecute { group: String, command: String },
    /// `/dnet observe <peer> <query>`
    Observe { peer: String, query: String },
    /// `/dnet unobserve <peer> <query>`
    Unobserve { peer: String, query: String },
    /// `/dnet peers` — list known peers
    ListPeers,
}

impl DnetCommand {
    /// Parse a raw slash command line (leading `/` optional).
    #[must_use]
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim_start_matches('/');
        if let Some(rest) = input.strip_prefix("dgae ") {
            return Some(Self::GroupAllExecute {
                command: rest.trim().to_string(),
            });
        }
        if let Some(rest) = input.strip_prefix("dggaexecute ") {
            let (group, command) = rest.trim().split_once(' ')?;
            return Some(Self::GroupExecute {
                group: group.to_string(),
                command: command.trim().to_string(),
            });
        }
        if let Some(rest) = input.strip_prefix("dnet ") {
            let rest = rest.trim();
            if rest == "peers" {
                return Some(Self::ListPeers);
            }
            if let Some(args) = rest.strip_prefix("observe ") {
                let (peer, query) = args.split_once(' ')?;
                return Some(Self::Observe {
                    peer: peer.to_string(),
                    query: query.trim().to_string(),
                });
            }
            if let Some(args) = rest.strip_prefix("unobserve ") {
                let (peer, query) = args.split_once(' ')?;
                return Some(Self::Unobserve {
                    peer: peer.to_string(),
                    query: query.trim().to_string(),
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dgae() {
        let cmd = DnetCommand::parse("/dgae /camp");
        assert_eq!(
            cmd,
            Some(DnetCommand::GroupAllExecute {
                command: "/camp".into()
            })
        );
    }

    #[test]
    fn parse_dggaexecute() {
        let cmd = DnetCommand::parse("/dggaexecute healers /casting 1");
        assert_eq!(
            cmd,
            Some(DnetCommand::GroupExecute {
                group: "healers".into(),
                command: "/casting 1".into()
            })
        );
    }

    #[test]
    fn frame_round_trip() {
        let f = DanNetFrame::GroupExecute {
            group: "all".into(),
            command: "/sit".into(),
        };
        let bytes = f.encode().unwrap();
        let f2 = DanNetFrame::decode(&bytes).unwrap();
        assert_eq!(f, f2);
    }
}
