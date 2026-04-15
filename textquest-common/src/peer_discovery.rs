//! UDP peer-discovery announcement types shared by orchestrator instances.

use crate::types::ClientId;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// Discovery packet format version.
pub const DISCOVERY_PROTOCOL_VERSION: u16 = 1;
/// Maximum encoded UDP discovery packet size.
pub const MAX_DISCOVERY_PACKET_SIZE: usize = 8 * 1024;
/// Maximum number of session snapshots carried in one announcement.
pub const MAX_DISCOVERY_SESSIONS: usize = 64;
/// Maximum allowed node-name length.
pub const MAX_DISCOVERY_NODE_NAME_LEN: usize = 64;
/// Maximum allowed per-session string length.
pub const MAX_DISCOVERY_STRING_LEN: usize = 64;

/// Lightweight snapshot of one locally-known session advertised to peers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerSessionAnnouncement {
    /// Stable orchestrator-side client identifier, when known.
    #[serde(default)]
    pub client_id: Option<ClientId>,
    /// Local EQ process identifier.
    pub pid: u32,
    /// In-game character name, when known.
    #[serde(default)]
    pub character_name: Option<String>,
    /// Current zone short name, when known.
    #[serde(default)]
    pub zone_short_name: Option<String>,
    /// Whether the session is currently healthy and command-ready.
    #[serde(default)]
    pub active: bool,
}

/// Multicast announcement published by one TextQuest instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerAnnouncement {
    /// Packet format version.
    pub version: u16,
    /// Random per-process instance identifier used to ignore
    /// self-announcements.
    pub instance_id: u64,
    /// Human-readable machine / node label.
    pub node_name: String,
    /// Sender wall-clock time in unix milliseconds.
    pub sent_at_unix_ms: u64,
    /// Session snapshots currently known to the sender.
    #[serde(default)]
    pub sessions: Vec<PeerSessionAnnouncement>,
}

impl PeerAnnouncement {
    /// Validate announcement bounds before encode or after decode.
    ///
    /// # Errors
    ///
    /// Returns an error when fields exceed supported transport bounds.
    pub fn validate(&self) -> Result<()> {
        if self.version != DISCOVERY_PROTOCOL_VERSION {
            bail!("unsupported discovery protocol version {}", self.version);
        }
        if self.node_name.trim().is_empty() {
            bail!("discovery node_name must not be empty");
        }
        if self.node_name.len() > MAX_DISCOVERY_NODE_NAME_LEN {
            bail!("discovery node_name exceeds {MAX_DISCOVERY_NODE_NAME_LEN} bytes");
        }
        if self.sessions.len() > MAX_DISCOVERY_SESSIONS {
            bail!("discovery session count exceeds {}", MAX_DISCOVERY_SESSIONS);
        }

        for session in &self.sessions {
            validate_optional_string("character_name", &session.character_name)?;
            validate_optional_string("zone_short_name", &session.zone_short_name)?;
        }

        Ok(())
    }

    /// Encode an announcement into a single UDP datagram payload.
    ///
    /// # Errors
    ///
    /// Returns an error if validation or serialization fails.
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let payload = bincode::serde::encode_to_vec(self, bincode::config::standard())?;
        if payload.len() > MAX_DISCOVERY_PACKET_SIZE {
            bail!(
                "encoded discovery packet exceeds {} bytes",
                MAX_DISCOVERY_PACKET_SIZE
            );
        }
        Ok(payload)
    }

    /// Decode and validate an announcement from a UDP datagram payload.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload is invalid or malformed.
    pub fn decode(payload: &[u8]) -> Result<Self> {
        if payload.len() > MAX_DISCOVERY_PACKET_SIZE {
            bail!(
                "discovery packet exceeds {} bytes",
                MAX_DISCOVERY_PACKET_SIZE
            );
        }
        let config = bincode::config::standard().with_limit::<MAX_DISCOVERY_PACKET_SIZE>();
        let (announcement, _): (Self, usize) = bincode::serde::decode_from_slice(payload, config)?;
        announcement.validate()?;
        Ok(announcement)
    }
}

fn validate_optional_string(field: &str, value: &Option<String>) -> Result<()> {
    if let Some(value) = value {
        if value.trim().is_empty() {
            bail!("discovery {field} must not be blank when present");
        }
        if value.len() > MAX_DISCOVERY_STRING_LEN {
            bail!("discovery {field} exceeds {MAX_DISCOVERY_STRING_LEN} bytes");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_announcement() -> PeerAnnouncement {
        PeerAnnouncement {
            version: DISCOVERY_PROTOCOL_VERSION,
            instance_id: 42,
            node_name: "raid-host-01".to_string(),
            sent_at_unix_ms: 1_716_000_000_000,
            sessions: vec![PeerSessionAnnouncement {
                client_id: Some(7),
                pid: 1234,
                character_name: Some("Camrene".to_string()),
                zone_short_name: Some("poknowledge".to_string()),
                active: true,
            }],
        }
    }

    #[test]
    fn announcement_roundtrips() {
        let announcement = sample_announcement();
        let encoded = announcement.encode().unwrap();
        let decoded = PeerAnnouncement::decode(&encoded).unwrap();
        assert_eq!(decoded, announcement);
    }

    #[test]
    fn announcement_rejects_blank_node_name() {
        let mut announcement = sample_announcement();
        announcement.node_name = "   ".to_string();
        assert!(announcement.encode().is_err());
    }

    #[test]
    fn announcement_rejects_too_many_sessions() {
        let mut announcement = sample_announcement();
        announcement.sessions = (0..=MAX_DISCOVERY_SESSIONS)
            .map(|pid| PeerSessionAnnouncement {
                client_id: None,
                pid: pid as u32,
                character_name: None,
                zone_short_name: None,
                active: false,
            })
            .collect();
        assert!(announcement.encode().is_err());
    }

    #[test]
    fn decode_rejects_oversized_payload() {
        let payload = vec![0u8; MAX_DISCOVERY_PACKET_SIZE + 1];
        assert!(PeerAnnouncement::decode(&payload).is_err());
    }

    #[test]
    fn decode_rejects_wrong_protocol_version() {
        let mut announcement = sample_announcement();
        announcement.version += 1;
        let payload =
            bincode::serde::encode_to_vec(&announcement, bincode::config::standard()).unwrap();
        assert!(PeerAnnouncement::decode(&payload).is_err());
    }

    // ─── Additional validation edge cases ────────────────────────────────

    #[test]
    fn announcement_rejects_oversized_node_name() {
        let mut announcement = sample_announcement();
        announcement.node_name = "x".repeat(MAX_DISCOVERY_NODE_NAME_LEN + 1);
        assert!(announcement.validate().is_err());
    }

    #[test]
    fn announcement_accepts_max_node_name_length() {
        let mut announcement = sample_announcement();
        announcement.node_name = "x".repeat(MAX_DISCOVERY_NODE_NAME_LEN);
        assert!(announcement.validate().is_ok());
    }

    #[test]
    fn announcement_rejects_blank_character_name() {
        let mut announcement = sample_announcement();
        announcement.sessions[0].character_name = Some("   ".to_string());
        assert!(announcement.validate().is_err());
    }

    #[test]
    fn announcement_rejects_oversized_character_name() {
        let mut announcement = sample_announcement();
        announcement.sessions[0].character_name = Some("z".repeat(MAX_DISCOVERY_STRING_LEN + 1));
        assert!(announcement.validate().is_err());
    }

    #[test]
    fn announcement_rejects_blank_zone_short_name() {
        let mut announcement = sample_announcement();
        announcement.sessions[0].zone_short_name = Some("  ".to_string());
        assert!(announcement.validate().is_err());
    }

    #[test]
    fn announcement_rejects_oversized_zone_short_name() {
        let mut announcement = sample_announcement();
        announcement.sessions[0].zone_short_name = Some("a".repeat(MAX_DISCOVERY_STRING_LEN + 1));
        assert!(announcement.validate().is_err());
    }

    #[test]
    fn announcement_accepts_none_optional_fields() {
        let mut announcement = sample_announcement();
        announcement.sessions[0].character_name = None;
        announcement.sessions[0].zone_short_name = None;
        announcement.sessions[0].client_id = None;
        assert!(announcement.validate().is_ok());
    }

    #[test]
    fn announcement_empty_sessions_is_valid() {
        let mut announcement = sample_announcement();
        announcement.sessions.clear();
        assert!(announcement.validate().is_ok());
        let encoded = announcement.encode().unwrap();
        let decoded = PeerAnnouncement::decode(&encoded).unwrap();
        assert!(decoded.sessions.is_empty());
    }

    #[test]
    fn announcement_max_sessions_boundary() {
        let mut announcement = sample_announcement();
        announcement.sessions = (0..MAX_DISCOVERY_SESSIONS)
            .map(|pid| PeerSessionAnnouncement {
                client_id: None,
                pid: pid as u32,
                character_name: None,
                zone_short_name: None,
                active: false,
            })
            .collect();
        // Exactly at the limit should be valid
        assert!(announcement.validate().is_ok());
    }

    #[test]
    fn decode_empty_payload_fails() {
        assert!(PeerAnnouncement::decode(&[]).is_err());
    }

    #[test]
    fn decode_garbage_payload_fails() {
        assert!(PeerAnnouncement::decode(&[0xFF, 0xFE, 0xFD]).is_err());
    }
}
