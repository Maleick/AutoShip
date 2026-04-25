//! EQBC client — connects to a hub and exposes `/bc`, `/bca`, `/bct` commands.

use std::net::SocketAddr;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    sync::{broadcast, mpsc},
    time::{self, Duration},
};
use tracing::{debug, info, warn};

use super::{EqbcMessage, MAX_LINE_BYTES};

/// Events delivered to the owning orchestrator session.
#[derive(Debug, Clone)]
pub enum ClientEvent {
    /// Server sent EXECUTE — run this slash command on this character.
    Execute { command: String },
    /// Connected peer list was updated by the server.
    Peers { names: Vec<String> },
    /// Connection to the hub was established or re-established.
    Connected,
    /// Connection to the hub was lost.
    Disconnected,
}

/// Outbound command from the orchestrator to the hub.
#[derive(Debug)]
pub enum ClientCommand {
    /// Broadcast a command to every connected peer (`/bc`, `/bca`).
    Broadcast { command: String },
    /// Send a command to one named peer (`/bct <name> <cmd>`).
    Target { name: String, command: String },
    /// Disconnect and shut down.
    Shutdown,
}

/// EQBC client handle.  The actual I/O runs in a spawned task; use
/// [`EqbcClient::subscribe`] for events and [`EqbcClient::send`] for commands.
pub struct EqbcClient {
    cmd_tx: mpsc::Sender<ClientCommand>,
    event_tx: broadcast::Sender<ClientEvent>,
}

impl EqbcClient {
    /// Connect to `server_addr` and register `character_name`.
    ///
    /// Spawns a background task that reconnects on disconnect.
    ///
    /// # Errors
    ///
    /// Returns an error if the initial connection cannot be made.
    pub async fn connect(
        server_addr: SocketAddr,
        character_name: String,
    ) -> anyhow::Result<Self> {
        let (cmd_tx, cmd_rx) = mpsc::channel::<ClientCommand>(64);
        let (event_tx, _) = broadcast::channel::<ClientEvent>(128);
        let event_tx2 = event_tx.clone();

        tokio::spawn(run_client(server_addr, character_name, cmd_rx, event_tx2));

        Ok(Self { cmd_tx, event_tx })
    }

    /// Subscribe to events from the hub.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ClientEvent> {
        self.event_tx.subscribe()
    }

    /// Send a command to the hub.
    ///
    /// # Errors
    ///
    /// Returns an error if the client task has exited.
    pub async fn send(&self, cmd: ClientCommand) -> anyhow::Result<()> {
        self.cmd_tx.send(cmd).await?;
        Ok(())
    }

    /// Broadcast a `/bc`-style command.
    pub async fn bc(&self, command: &str) -> anyhow::Result<()> {
        self.send(ClientCommand::Broadcast {
            command: command.to_string(),
        })
        .await
    }

    /// Target a specific character with `/bct`.
    pub async fn bct(&self, name: &str, command: &str) -> anyhow::Result<()> {
        self.send(ClientCommand::Target {
            name: name.to_string(),
            command: command.to_string(),
        })
        .await
    }
}

async fn run_client(
    server_addr: SocketAddr,
    character_name: String,
    mut cmd_rx: mpsc::Receiver<ClientCommand>,
    event_tx: broadcast::Sender<ClientEvent>,
) {
    let mut reconnect_delay = Duration::from_secs(2);

    loop {
        match TcpStream::connect(server_addr).await {
            Ok(stream) => {
                reconnect_delay = Duration::from_secs(2);
                info!("EQBC connected to {server_addr} as {character_name}");
                let _ = event_tx.send(ClientEvent::Connected);

                if let Err(e) =
                    session_loop(&character_name, stream, &mut cmd_rx, &event_tx).await
                {
                    warn!("EQBC session ended: {e}");
                }
                let _ = event_tx.send(ClientEvent::Disconnected);
            }
            Err(e) => {
                warn!("EQBC connect to {server_addr} failed: {e}; retry in {reconnect_delay:?}");
            }
        }

        // Check for shutdown before reconnecting
        match cmd_rx.try_recv() {
            Ok(ClientCommand::Shutdown) | Err(mpsc::error::TryRecvError::Disconnected) => {
                info!("EQBC client shutting down");
                return;
            }
            _ => {}
        }

        time::sleep(reconnect_delay).await;
        reconnect_delay = (reconnect_delay * 2).min(Duration::from_secs(60));
    }
}

async fn session_loop(
    character_name: &str,
    stream: TcpStream,
    cmd_rx: &mut mpsc::Receiver<ClientCommand>,
    event_tx: &broadcast::Sender<ClientEvent>,
) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    // Register with the hub
    writer
        .write_all(EqbcMessage::Identity { name: character_name.to_string() }.encode().as_bytes())
        .await?;

    let mut ping_interval = time::interval(Duration::from_secs(30));
    ping_interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            line = lines.next_line() => {
                match line? {
                    None => return Ok(()),
                    Some(raw) => {
                        if raw.len() > MAX_LINE_BYTES {
                            warn!("EQBC oversized server line, skipping");
                            continue;
                        }
                        let Some(msg) = EqbcMessage::parse(&raw) else { continue };
                        match msg {
                            EqbcMessage::Execute { command } => {
                                debug!("EQBC execute: {command}");
                                let _ = event_tx.send(ClientEvent::Execute { command });
                            }
                            EqbcMessage::Peers { names } => {
                                let _ = event_tx.send(ClientEvent::Peers { names });
                            }
                            EqbcMessage::Ping => {
                                writer.write_all(b"PONG\n").await?;
                            }
                            _ => {}
                        }
                    }
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    None | Some(ClientCommand::Shutdown) => return Ok(()),
                    Some(ClientCommand::Broadcast { command }) => {
                        writer.write_all(
                            EqbcMessage::Broadcast { command }.encode().as_bytes()
                        ).await?;
                    }
                    Some(ClientCommand::Target { name, command }) => {
                        writer.write_all(
                            EqbcMessage::Target { name, command }.encode().as_bytes()
                        ).await?;
                    }
                }
            }
            _ = ping_interval.tick() => {
                writer.write_all(b"PING\n").await?;
            }
        }
    }
}
