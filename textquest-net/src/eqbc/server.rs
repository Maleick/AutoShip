//! EQBCS-compatible TCP hub server.
//!
//! The server accepts client connections, tracks registered character names,
//! and relays BROADCAST/TARGET messages to the appropriate recipients.

use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::{broadcast, Mutex},
    time::{self, Duration},
};
use tracing::{debug, info, warn};

use super::{read_bounded_line, EqbcMessage, MAX_LINE_BYTES};

/// Event emitted by the server when a routed command arrives.
#[derive(Debug, Clone)]
pub enum ServerEvent {
    /// A broadcast command from `sender` for all clients.
    Broadcast { sender: String, command: String },
    /// A targeted command from `sender` for `target`.
    Target {
        sender: String,
        target: String,
        command: String,
    },
    /// A client registered or disconnected.
    PeerListChanged { peers: Vec<String> },
}

type PeerMap = Arc<Mutex<HashMap<String, tokio::sync::mpsc::Sender<String>>>>;

/// TextQuest-native EQBC server hub.
///
/// Bind with [`EqbcServer::bind`], then call [`EqbcServer::run`] inside a
/// Tokio task.
pub struct EqbcServer {
    listener: TcpListener,
    event_tx: broadcast::Sender<ServerEvent>,
    peers: PeerMap,
}

impl EqbcServer {
    /// Bind to `addr` and create a new server.  `event_capacity` sets the
    /// broadcast channel buffer; 128 is a safe default.
    ///
    /// # Errors
    ///
    /// Returns an error if the TCP listener cannot be bound.
    pub async fn bind(addr: SocketAddr, event_capacity: usize) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let (event_tx, _) = broadcast::channel(event_capacity);
        info!("EQBC server listening on {addr}");
        Ok(Self {
            listener,
            event_tx,
            peers: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Subscribe to server events (command relay, peer-list changes).
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.event_tx.subscribe()
    }

    /// Accept connections forever.  Cancel the task to shut down.
    pub async fn run(self) {
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    debug!("EQBC new connection from {addr}");
                    let peers = Arc::clone(&self.peers);
                    let event_tx = self.event_tx.clone();
                    tokio::spawn(handle_client(stream, addr, peers, event_tx));
                }
                Err(e) => {
                    warn!("EQBC accept error: {e}");
                }
            }
        }
    }
}

async fn handle_client(
    stream: TcpStream,
    addr: SocketAddr,
    peers: PeerMap,
    event_tx: broadcast::Sender<ServerEvent>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(64);
    let mut character_name: Option<String> = None;

    // Keepalive ticker
    let mut ping_interval = time::interval(Duration::from_secs(30));
    ping_interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            line = read_bounded_line(&mut reader, MAX_LINE_BYTES) => {
                match line {
                    Ok(Some(raw)) => {
                        let Some(msg) = EqbcMessage::parse(&raw) else { continue };
                        match msg {
                            EqbcMessage::Identity { name } => {
                                character_name = Some(name.clone());
                                let mut map = peers.lock().await;
                                map.insert(name.clone(), tx.clone());
                                info!("EQBC peer registered: {name}");
                                let peer_names: Vec<_> = map.keys().cloned().collect();
                                drop(map);
                                // Broadcast updated peer list to all
                                broadcast_peer_list(&peers, &peer_names).await;
                                let _ = event_tx.send(ServerEvent::PeerListChanged {
                                    peers: peer_names,
                                });
                            }
                            EqbcMessage::Broadcast { command } => {
                                let sender = character_name.clone().unwrap_or_else(|| addr.to_string());
                                relay_broadcast(&peers, &sender, &command).await;
                                let _ = event_tx.send(ServerEvent::Broadcast {
                                    sender,
                                    command,
                                });
                            }
                            EqbcMessage::Target { name, command } => {
                                let sender = character_name.clone().unwrap_or_else(|| addr.to_string());
                                relay_target(&peers, &name, &command).await;
                                let _ = event_tx.send(ServerEvent::Target {
                                    sender,
                                    target: name,
                                    command,
                                });
                            }
                            EqbcMessage::Ping => {
                                let _ = tx.send(EqbcMessage::Pong.encode()).await;
                            }
                            EqbcMessage::Pong => {}
                            _ => {}
                        }
                    }
                    Ok(None) => break, // EOF
                    Err(e) => {
                        warn!("EQBC read error from {addr}: {e}");
                        break;
                    }
                }
            }
            Some(outbound) = rx.recv() => {
                if writer.write_all(outbound.as_bytes()).await.is_err() {
                    break;
                }
            }
            _ = ping_interval.tick() => {
                if writer.write_all(b"PING\n").await.is_err() {
                    break;
                }
            }
        }
    }

    // Clean up on disconnect
    if let Some(name) = &character_name {
        let mut map = peers.lock().await;
        map.remove(name);
        info!("EQBC peer disconnected: {name}");
        let peer_names: Vec<_> = map.keys().cloned().collect();
        drop(map);
        broadcast_peer_list(&peers, &peer_names).await;
        let _ = event_tx.send(ServerEvent::PeerListChanged { peers: peer_names });
    }
}

async fn relay_broadcast(peers: &PeerMap, sender: &str, command: &str) {
    let execute = EqbcMessage::Execute {
        command: command.to_string(),
    }
    .encode();
    let map = peers.lock().await;
    for (name, tx) in map.iter() {
        if name != sender {
            let _ = tx.try_send(execute.clone());
        }
    }
}

async fn relay_target(peers: &PeerMap, target: &str, command: &str) {
    let execute = EqbcMessage::Execute {
        command: command.to_string(),
    }
    .encode();
    let map = peers.lock().await;
    if let Some(tx) = map.get(target) {
        let _ = tx.try_send(execute);
    }
}

async fn broadcast_peer_list(peers: &PeerMap, names: &[String]) {
    let peer_msg = EqbcMessage::Peers {
        names: names.to_vec(),
    }
    .encode();
    let map = peers.lock().await;
    for tx in map.values() {
        let _ = tx.try_send(peer_msg.clone());
    }
}
