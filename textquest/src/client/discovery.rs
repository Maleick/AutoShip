//! Optional UDP multicast peer discovery for orchestrator instances.

use super::session::EqSession;
use crate::config::PeerDiscoveryConfig;
use rand::random;
use std::{
    collections::HashMap,
    io::ErrorKind,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use textquest_common::peer_discovery::{PeerAnnouncement, PeerSessionAnnouncement};

/// Remote peer currently visible through multicast discovery.
#[derive(Debug, Clone)]
pub struct RemotePeer {
    /// Random sender instance ID.
    pub instance_id: u64,
    /// Human-readable sender node name.
    pub node_name: String,
    /// Source address of the latest announcement.
    pub addr: SocketAddr,
    /// Latest advertised sessions.
    pub sessions: Vec<PeerSessionAnnouncement>,
    /// Local receipt time of the latest announcement.
    pub last_seen: Instant,
}

/// Significant peer-discovery state changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerDiscoveryEvent {
    /// A new peer became visible.
    PeerDiscovered {
        instance_id: u64,
        node_name: String,
        session_count: usize,
    },
    /// A previously visible peer timed out.
    PeerExpired { instance_id: u64, node_name: String },
}

#[derive(Default)]
struct PeerDirectory {
    peers: HashMap<u64, RemotePeer>,
}

impl PeerDirectory {
    fn apply(
        &mut self,
        announcement: PeerAnnouncement,
        addr: SocketAddr,
        now: Instant,
    ) -> Option<PeerDiscoveryEvent> {
        let is_new = !self.peers.contains_key(&announcement.instance_id);
        let node_name = announcement.node_name.clone();
        let session_count = announcement.sessions.len();
        self.peers.insert(
            announcement.instance_id,
            RemotePeer {
                instance_id: announcement.instance_id,
                node_name: announcement.node_name,
                addr,
                sessions: announcement.sessions,
                last_seen: now,
            },
        );
        is_new.then_some(PeerDiscoveryEvent::PeerDiscovered {
            instance_id: announcement.instance_id,
            node_name,
            session_count,
        })
    }

    fn prune_stale(&mut self, ttl: Duration, now: Instant) -> Vec<PeerDiscoveryEvent> {
        let mut expired = Vec::new();
        self.peers.retain(|instance_id, peer| {
            if now.duration_since(peer.last_seen) > ttl {
                expired.push(PeerDiscoveryEvent::PeerExpired {
                    instance_id: *instance_id,
                    node_name: peer.node_name.clone(),
                });
                false
            } else {
                true
            }
        });
        expired
    }

    fn peers(&self) -> Vec<&RemotePeer> {
        self.peers.values().collect()
    }
}

/// Multicast discovery transport state.
pub struct MulticastPeerDiscovery {
    instance_id: u64,
    node_name: String,
    announce_interval: Duration,
    peer_ttl: Duration,
    target: SocketAddrV4,
    recv_socket: UdpSocket,
    send_socket: UdpSocket,
    next_announcement_at: Instant,
    directory: PeerDirectory,
}

impl MulticastPeerDiscovery {
    /// Create a multicast discovery transport from config.
    ///
    /// # Errors
    ///
    /// Returns an error when the multicast configuration is invalid or sockets
    /// cannot be created.
    pub fn new(config: &PeerDiscoveryConfig) -> anyhow::Result<Self> {
        let bind_addr: Ipv4Addr = config.bind_addr.parse()?;
        let multicast_addr: Ipv4Addr = config.multicast_addr.parse()?;
        if !multicast_addr.is_multicast() {
            anyhow::bail!("multicast_addr must be an IPv4 multicast address");
        }

        let recv_socket = UdpSocket::bind(SocketAddrV4::new(bind_addr, config.port))?;
        recv_socket.set_nonblocking(true)?;
        recv_socket.join_multicast_v4(&multicast_addr, &bind_addr)?;

        let send_socket = UdpSocket::bind(SocketAddrV4::new(bind_addr, 0))?;
        send_socket.set_nonblocking(true)?;
        send_socket.set_multicast_loop_v4(true)?;
        send_socket.set_multicast_ttl_v4(config.multicast_ttl)?;

        let now = Instant::now();
        Ok(Self {
            instance_id: random(),
            node_name: resolved_node_name(config),
            announce_interval: Duration::from_millis(config.announce_interval_ms.max(100)),
            peer_ttl: Duration::from_millis(config.peer_ttl_ms.max(1_000)),
            target: SocketAddrV4::new(multicast_addr, config.port),
            recv_socket,
            send_socket,
            next_announcement_at: now,
            directory: PeerDirectory::default(),
        })
    }

    /// Send due announcements, ingest inbound packets, and expire stale peers.
    pub fn tick<'a>(
        &mut self,
        sessions: impl IntoIterator<Item = &'a EqSession>,
    ) -> Vec<PeerDiscoveryEvent> {
        let now = Instant::now();
        if now >= self.next_announcement_at {
            if let Err(error) = self.send_announcement(sessions) {
                tracing::warn!(error = %error, "Failed to send multicast peer discovery");
            }
            self.next_announcement_at = now + self.announce_interval;
        }

        let mut events = self.read_announcements(now);
        events.extend(self.directory.prune_stale(self.peer_ttl, now));
        events
    }

    /// Get the current remote peer snapshots.
    #[must_use]
    pub fn peers(&self) -> Vec<&RemotePeer> {
        self.directory.peers()
    }

    fn send_announcement<'a>(
        &self,
        sessions: impl IntoIterator<Item = &'a EqSession>,
    ) -> anyhow::Result<()> {
        let announcement = PeerAnnouncement {
            version: textquest_common::peer_discovery::DISCOVERY_PROTOCOL_VERSION,
            instance_id: self.instance_id,
            node_name: self.node_name.clone(),
            sent_at_unix_ms: now_unix_ms(),
            sessions: sessions.into_iter().map(session_snapshot).collect(),
        };
        let payload = announcement.encode()?;
        self.send_socket.send_to(&payload, self.target)?;
        Ok(())
    }

    fn read_announcements(&mut self, now: Instant) -> Vec<PeerDiscoveryEvent> {
        let mut events = Vec::new();
        let mut buffer = [0u8; textquest_common::peer_discovery::MAX_DISCOVERY_PACKET_SIZE];

        loop {
            match self.recv_socket.recv_from(&mut buffer) {
                Ok((size, addr)) => match PeerAnnouncement::decode(&buffer[..size]) {
                    Ok(announcement) if announcement.instance_id == self.instance_id => {}
                    Ok(announcement) => {
                        if let Some(event) = self.directory.apply(announcement, addr, now) {
                            events.push(event);
                        }
                    }
                    Err(error) => {
                        tracing::debug!(error = %error, "Ignoring malformed multicast discovery packet");
                    }
                },
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => {
                    tracing::warn!(error = %error, "Failed to read multicast discovery packet");
                    break;
                }
            }
        }

        events
    }
}

fn session_snapshot(session: &EqSession) -> PeerSessionAnnouncement {
    PeerSessionAnnouncement {
        client_id: Some(session.client_id),
        pid: session.pid,
        character_name: session.character_name.clone(),
        zone_short_name: session
            .last_state
            .as_ref()
            .map(|state| state.zone_short_name.clone()),
        active: session.is_active(),
    }
}

fn resolved_node_name(config: &PeerDiscoveryConfig) -> String {
    let configured = config.node_name.trim();
    if !configured.is_empty() {
        return configured.to_string();
    }

    for key in ["COMPUTERNAME", "HOSTNAME"] {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim();
            if !value.is_empty() {
                return value.to_string();
            }
        }
    }

    format!("textquest-{:016x}", random::<u64>())
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::types::{GameState, SpawnData};

    fn sample_session() -> EqSession {
        let mut session = EqSession::new(7, 1234);
        session.character_name = Some("Camrene".to_string());
        session.last_state = Some(GameState {
            client_id: 7,
            local_player: Some(SpawnData {
                displayed_name: "Camrene".to_string(),
                name: "Camrene".to_string(),
                ..SpawnData::default()
            }),
            target: None,
            nearby_spawns: vec![],
            timestamp_ms: 0,
            nav_status: textquest_common::nav::NavStatus::Idle,
            combat_status: textquest_common::combat::CombatStatus::Idle,
            zone_short_name: "poknowledge".to_string(),
            zone_long_name: "Plane of Knowledge".to_string(),
            active_buffs: vec![],
            pet: None,
            actual_version: None,
        });
        session
    }

    #[test]
    fn session_snapshot_captures_basic_fields() {
        let session = sample_session();
        let snapshot = session_snapshot(&session);
        assert_eq!(snapshot.client_id, Some(7));
        assert_eq!(snapshot.pid, 1234);
        assert_eq!(snapshot.character_name.as_deref(), Some("Camrene"));
        assert_eq!(snapshot.zone_short_name.as_deref(), Some("poknowledge"));
        assert!(!snapshot.active);
    }

    #[test]
    fn resolved_node_name_prefers_config() {
        let config = PeerDiscoveryConfig {
            node_name: "raid-pc".to_string(),
            ..PeerDiscoveryConfig::default()
        };
        assert_eq!(resolved_node_name(&config), "raid-pc");
    }

    #[test]
    fn directory_reports_new_and_expired_peers() {
        let mut directory = PeerDirectory::default();
        let now = Instant::now();
        let event = directory.apply(
            PeerAnnouncement {
                version: textquest_common::peer_discovery::DISCOVERY_PROTOCOL_VERSION,
                instance_id: 99,
                node_name: "peer-box".to_string(),
                sent_at_unix_ms: 0,
                sessions: vec![PeerSessionAnnouncement {
                    client_id: Some(1),
                    pid: 321,
                    character_name: Some("Scout".to_string()),
                    zone_short_name: None,
                    active: true,
                }],
            },
            "127.0.0.1:35353".parse().unwrap(),
            now,
        );
        assert_eq!(
            event,
            Some(PeerDiscoveryEvent::PeerDiscovered {
                instance_id: 99,
                node_name: "peer-box".to_string(),
                session_count: 1,
            })
        );

        let expired = directory.prune_stale(Duration::from_millis(1), now + Duration::from_secs(1));
        assert_eq!(
            expired,
            vec![PeerDiscoveryEvent::PeerExpired {
                instance_id: 99,
                node_name: "peer-box".to_string(),
            }]
        );
    }
}
