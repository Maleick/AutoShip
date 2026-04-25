//! DanNet peer node — binds a UDP socket and manages peer discovery + dispatch.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    net::UdpSocket,
    sync::{broadcast, Mutex},
    time,
};
use tracing::{debug, info, warn};

use super::{DanNetFrame, MAX_FRAME_BYTES};
use crate::config::DanNetConfig;

/// Events emitted by a running [`DanNetNode`].
#[derive(Debug, Clone)]
pub enum NodeEvent {
    /// A peer joined or refreshed its presence.
    PeerHello { peer: String, group: String, addr: SocketAddr },
    /// A peer left the network.
    PeerBye { peer: String },
    /// An execute command arrived for this node.
    Execute { sender: String, command: String },
    /// An observe reply arrived with a TLO value.
    ObserveReply { target: String, query: String, value: String },
    /// Peer list snapshot (sent after any change).
    Peers { names: Vec<String> },
}

/// TLO resolver callback — given a query string, return the current value.
/// The orchestrator registers this so DanNet can answer observe requests.
pub type TloResolver = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

struct PeerEntry {
    addr: SocketAddr,
    /// Server group of this peer — used for group-scoped command filtering.
    #[allow(dead_code)]
    group: String,
    last_seen: Instant,
}

/// A running DanNet peer node.
pub struct DanNetNode {
    event_tx: broadcast::Sender<NodeEvent>,
    socket: Arc<UdpSocket>,
    config: DanNetConfig,
    peers: Arc<Mutex<HashMap<String, PeerEntry>>>,
    my_name: String,
    instance_id: u64,
    tlo_resolver: Option<TloResolver>,
}

impl DanNetNode {
    /// Bind the UDP socket and create the node.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be bound.
    pub async fn bind(config: DanNetConfig, tlo_resolver: Option<TloResolver>) -> anyhow::Result<Self> {
        let addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse()?;
        let socket = UdpSocket::bind(addr).await?;
        let character_name = config.character_name.clone();
        let instance_id = rand_instance_id();
        info!("DanNet node bound on :{} as {character_name}", config.port);

        let (event_tx, _) = broadcast::channel(256);

        Ok(Self {
            event_tx,
            socket: Arc::new(socket),
            config,
            peers: Arc::new(Mutex::new(HashMap::new())),
            my_name: character_name,
            instance_id,
            tlo_resolver,
        })
    }

    /// Subscribe to node events.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<NodeEvent> {
        self.event_tx.subscribe()
    }

    /// Send a `/dgae`-style command to all peers in this node's group.
    ///
    /// # Errors
    ///
    /// Returns an error if encoding or send fails.
    pub async fn dgae(&self, command: &str) -> anyhow::Result<()> {
        let frame = DanNetFrame::GroupExecute {
            group: self.config.group.clone(),
            command: command.to_string(),
        };
        self.broadcast_frame(&frame).await
    }

    /// Send a `/dggaexecute`-style command to a named group.
    ///
    /// # Errors
    ///
    /// Returns an error if encoding or send fails.
    pub async fn dggaexecute(&self, group: &str, command: &str) -> anyhow::Result<()> {
        let frame = DanNetFrame::GroupExecute {
            group: group.to_string(),
            command: command.to_string(),
        };
        self.broadcast_frame(&frame).await
    }

    /// Start observing a TLO query on a remote peer.
    ///
    /// # Errors
    ///
    /// Returns an error if encoding or send fails.
    pub async fn observe(&self, target_peer: &str, query: &str) -> anyhow::Result<()> {
        let frame = DanNetFrame::Observe {
            requester: self.my_name.clone(),
            target: target_peer.to_string(),
            query: query.to_string(),
        };
        self.send_to_peer(target_peer, &frame).await
    }

    /// Stop observing a TLO query on a remote peer.
    ///
    /// # Errors
    ///
    /// Returns an error if encoding or send fails.
    pub async fn unobserve(&self, target_peer: &str, query: &str) -> anyhow::Result<()> {
        let frame = DanNetFrame::Unobserve {
            requester: self.my_name.clone(),
            target: target_peer.to_string(),
            query: query.to_string(),
        };
        self.send_to_peer(target_peer, &frame).await
    }

    /// Return current known peer names.
    pub async fn peer_names(&self) -> Vec<String> {
        self.peers.lock().await.keys().cloned().collect()
    }

    /// Run the node event loop (announce + receive).  Cancel the task to stop.
    pub async fn run(self) {
        let socket = Arc::clone(&self.socket);
        let peers = Arc::clone(&self.peers);
        let event_tx = self.event_tx.clone();
        let my_name = self.my_name.clone();
        let group = self.config.group.clone();
        let instance_id = self.instance_id;
        let tlo_resolver = self.tlo_resolver.clone();

        // Hello announce ticker
        let announce_socket = Arc::clone(&socket);
        let announce_peers_ref = Arc::clone(&peers);
        let announce_name = my_name.clone();
        let announce_group = group.clone();
        let static_peers: Vec<String> = self.config.peers.clone();
        tokio::spawn(async move {
            let mut ticker = time::interval(Duration::from_secs(10));
            ticker.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            let hello = DanNetFrame::Hello {
                peer: announce_name.clone(),
                group: announce_group.clone(),
                instance_id,
            };
            let Ok(bytes) = hello.encode() else { return };
            loop {
                ticker.tick().await;
                // Send to all static peers
                for addr_str in &static_peers {
                    if let Ok(addr) = addr_str.parse::<SocketAddr>() {
                        let _ = announce_socket.send_to(&bytes, addr).await;
                    }
                }
                // Prune stale peers (>60s)
                let now = Instant::now();
                let mut map = announce_peers_ref.lock().await;
                map.retain(|_, e| now.duration_since(e.last_seen) < Duration::from_secs(60));
            }
        });

        // Receive loop
        let mut buf = vec![0u8; MAX_FRAME_BYTES + 64];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((n, from_addr)) => {
                    let data = &buf[..n];
                    match DanNetFrame::decode(data) {
                        Ok(frame) => {
                            handle_frame(
                                frame,
                                from_addr,
                                &my_name,
                                &group,
                                &peers,
                                &event_tx,
                                &tlo_resolver,
                                &socket,
                            )
                            .await;
                        }
                        Err(e) => {
                            debug!("DanNet bad frame from {from_addr}: {e}");
                        }
                    }
                }
                Err(e) => {
                    warn!("DanNet recv error: {e}");
                }
            }
        }
    }

    async fn broadcast_frame(&self, frame: &DanNetFrame) -> anyhow::Result<()> {
        let bytes = frame.encode()?;
        let map = self.peers.lock().await;
        for entry in map.values() {
            let _ = self.socket.send_to(&bytes, entry.addr).await;
        }
        Ok(())
    }

    async fn send_to_peer(&self, target_name: &str, frame: &DanNetFrame) -> anyhow::Result<()> {
        let bytes = frame.encode()?;
        let map = self.peers.lock().await;
        if let Some(entry) = map.get(target_name) {
            self.socket.send_to(&bytes, entry.addr).await?;
        } else {
            anyhow::bail!("DanNet peer {target_name} not found");
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_frame(
    frame: DanNetFrame,
    from_addr: SocketAddr,
    my_name: &str,
    my_group: &str,
    peers: &Arc<Mutex<HashMap<String, PeerEntry>>>,
    event_tx: &broadcast::Sender<NodeEvent>,
    tlo_resolver: &Option<TloResolver>,
    socket: &UdpSocket,
) {
    match frame {
        DanNetFrame::Hello { peer, group, instance_id: _ } => {
            if peer == my_name {
                return; // ignore self
            }
            let mut map = peers.lock().await;
            let is_new = !map.contains_key(&peer);
            map.insert(
                peer.clone(),
                PeerEntry {
                    addr: from_addr,
                    group: group.clone(),
                    last_seen: Instant::now(),
                },
            );
            if is_new {
                info!("DanNet new peer: {peer} ({group}) at {from_addr}");
                let names: Vec<_> = map.keys().cloned().collect();
                drop(map);
                let _ = event_tx.send(NodeEvent::PeerHello { peer, group, addr: from_addr });
                let _ = event_tx.send(NodeEvent::Peers { names });
            } else {
                map.get_mut(&peer).unwrap().last_seen = Instant::now();
            }
        }
        DanNetFrame::GroupExecute { group, command } => {
            if group == my_group || group == "all" {
                let _ = event_tx.send(NodeEvent::Execute {
                    sender: from_addr.to_string(),
                    command,
                });
            }
        }
        DanNetFrame::PeerExecute { target, command } => {
            if target == my_name {
                let _ = event_tx.send(NodeEvent::Execute {
                    sender: from_addr.to_string(),
                    command,
                });
            }
        }
        DanNetFrame::Observe { requester: _, target, query } => {
            if target == my_name {
                if let Some(resolver) = tlo_resolver {
                    let value = resolver(&query).unwrap_or_default();
                    let reply = DanNetFrame::ObserveReply {
                        target: my_name.to_string(),
                        query,
                        value,
                    };
                    if let Ok(bytes) = reply.encode() {
                        let _ = socket.send_to(&bytes, from_addr).await;
                    }
                }
            }
        }
        DanNetFrame::Unobserve { .. } => {
            // Observation state is stateless (poll-on-request) at this layer.
        }
        DanNetFrame::ObserveReply { target, query, value } => {
            let _ = event_tx.send(NodeEvent::ObserveReply { target, query, value });
        }
        DanNetFrame::Bye { peer, .. } => {
            let mut map = peers.lock().await;
            map.remove(&peer);
            let names: Vec<_> = map.keys().cloned().collect();
            drop(map);
            info!("DanNet peer left: {peer}");
            let _ = event_tx.send(NodeEvent::PeerBye { peer });
            let _ = event_tx.send(NodeEvent::Peers { names });
        }
    }
}

fn rand_instance_id() -> u64 {
    // Simple non-crypto random — just needs to distinguish processes.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
        ^ (std::process::id() as u64) << 32
}
