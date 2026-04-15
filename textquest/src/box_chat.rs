//! EQBC-style box-chat runtime for cross-machine slash-command relay.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context, Result, anyhow, bail};
use textquest_common::box_chat::{BoxChatConfig, OutboundRoute, WireMessage, parse_slash_route};

const IO_POLL_INTERVAL: Duration = Duration::from_millis(200);
const RECONNECT_DELAY: Duration = Duration::from_secs(2);
const CONFIG_RELOAD_INTERVAL: Duration = Duration::from_secs(1);

static MANAGER: OnceLock<BoxChatManager> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalClient {
    pid: u32,
    name: String,
    name_lower: String,
}

#[derive(Debug, Default)]
struct SharedState {
    local_clients: Mutex<Vec<LocalClient>>,
}

struct ListenerHandle {
    stop: Arc<AtomicBool>,
    join: thread::JoinHandle<()>,
    hub: Arc<HubState>,
}

impl ListenerHandle {
    fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.join.join();
    }
}

struct ConnectorHandle {
    stop: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
    tx: mpsc::Sender<ConnectorCommand>,
    join: thread::JoinHandle<()>,
}

impl ConnectorHandle {
    fn update_characters(&self, characters: Vec<String>) {
        let _ = self.tx.send(ConnectorCommand::UpdateCharacters(characters));
    }

    fn send_route(&self, route: &OutboundRoute) {
        let message = match route {
            OutboundRoute::Broadcast { command } => WireMessage::Broadcast {
                command: command.clone(),
            },
            OutboundRoute::Target { character, command } => WireMessage::Target {
                character: character.clone(),
                command: command.clone(),
            },
        };
        let _ = self.tx.send(ConnectorCommand::Route(message));
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.tx.send(ConnectorCommand::Stop);
        let _ = self.join.join();
    }
}

enum ConnectorCommand {
    UpdateCharacters(Vec<String>),
    Route(WireMessage),
    Stop,
}

#[derive(Default)]
struct HubState {
    next_peer_id: AtomicUsize,
    peers: Mutex<HashMap<usize, mpsc::Sender<WireMessage>>>,
}

impl HubState {
    fn register_peer(&self, sender: mpsc::Sender<WireMessage>) -> usize {
        let peer_id = self.next_peer_id.fetch_add(1, Ordering::Relaxed);
        self.peers
            .lock()
            .expect("hub peer lock")
            .insert(peer_id, sender);
        peer_id
    }

    fn remove_peer(&self, peer_id: usize) {
        self.peers.lock().expect("hub peer lock").remove(&peer_id);
    }

    fn relay_execute(&self, route: &OutboundRoute, skip_peer: Option<usize>) -> bool {
        let message = match route {
            OutboundRoute::Broadcast { command } => WireMessage::ExecuteBroadcast {
                command: command.clone(),
            },
            OutboundRoute::Target { character, command } => WireMessage::ExecuteTarget {
                character: character.clone(),
                command: command.clone(),
            },
        };
        self.broadcast(message, skip_peer)
    }

    fn broadcast(&self, message: WireMessage, skip_peer: Option<usize>) -> bool {
        let mut sent = false;
        let peers = self.peers.lock().expect("hub peer lock").clone();
        for (peer_id, sender) in peers {
            if skip_peer == Some(peer_id) {
                continue;
            }
            if sender.send(message.clone()).is_ok() {
                sent = true;
            }
        }
        sent
    }
}

struct RuntimeState {
    config_path: PathBuf,
    config: BoxChatConfig,
    mode: RuntimeMode,
    listener: Option<ListenerHandle>,
    connector: Option<ConnectorHandle>,
    last_reload_check: Option<Instant>,
    last_loaded_mtime: Option<SystemTime>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            config_path: default_config_path(),
            config: BoxChatConfig::default(),
            mode: RuntimeMode::Full,
            listener: None,
            connector: None,
            last_reload_check: None,
            last_loaded_mtime: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeMode {
    Full,
    ConnectorOnly,
}

struct BoxChatManager {
    runtime: Mutex<RuntimeState>,
    shared: Arc<SharedState>,
}

impl BoxChatManager {
    fn global() -> &'static Self {
        MANAGER.get_or_init(|| Self {
            runtime: Mutex::new(RuntimeState::default()),
            shared: Arc::new(SharedState::default()),
        })
    }

    fn configure(&self, config_path: PathBuf, config: BoxChatConfig) -> Result<()> {
        self.configure_with_mode(config_path, config, RuntimeMode::Full)
    }

    fn configure_connector_only(&self, config_path: PathBuf, config: BoxChatConfig) -> Result<()> {
        self.configure_with_mode(config_path, config, RuntimeMode::ConnectorOnly)
    }

    fn configure_with_mode(
        &self,
        config_path: PathBuf,
        config: BoxChatConfig,
        mode: RuntimeMode,
    ) -> Result<()> {
        let characters = self.character_names();
        let mut runtime = self.runtime.lock().expect("box chat runtime lock");
        let old_config = runtime.config.clone();
        let old_mode = runtime.mode;
        runtime.config_path = config_path;
        runtime.config = config.clone();
        runtime.mode = mode;
        runtime.last_reload_check = Some(Instant::now());
        runtime.last_loaded_mtime = config_modified_time(&runtime.config_path)?;

        if !config.enabled {
            stop_listener(&mut runtime);
            stop_connector(&mut runtime);
            return Ok(());
        }

        let listener_allowed = matches!(mode, RuntimeMode::Full);
        if listener_allowed {
            let listener_needs_restart = runtime.listener.is_none()
                || !old_config.enabled
                || old_config.port != config.port
                || old_mode != mode;
            if listener_needs_restart {
                stop_listener(&mut runtime);
                runtime.listener = Some(start_listener(config.port, Arc::clone(&self.shared))?);
            }
        } else {
            stop_listener(&mut runtime);
        }

        let suppress_self_connector =
            listener_allowed && runtime.listener.is_some() && is_self_connector_target(&config);
        if suppress_self_connector {
            tracing::info!(
                host = %config.host,
                port = config.port,
                "Skipping box-chat upstream connector for self endpoint"
            );
            stop_connector(&mut runtime);
        } else if config.auto_connect {
            let connector_needs_restart = runtime.connector.is_none()
                || !old_config.enabled
                || !old_config.auto_connect
                || old_config.host != config.host
                || old_config.port != config.port
                || old_mode != mode;
            if connector_needs_restart {
                stop_connector(&mut runtime);
                runtime.connector = Some(start_connector(
                    config.clone(),
                    characters.clone(),
                    Arc::clone(&self.shared),
                )?);
            } else if let Some(connector) = runtime.connector.as_ref() {
                connector.update_characters(characters);
            }
        } else {
            stop_connector(&mut runtime);
        }

        Ok(())
    }

    fn stop(&self) {
        let mut runtime = self.runtime.lock().expect("box chat runtime lock");
        stop_listener(&mut runtime);
        stop_connector(&mut runtime);
        runtime.config = BoxChatConfig::default();
        runtime.mode = RuntimeMode::Full;
        runtime.last_reload_check = None;
        runtime.last_loaded_mtime = None;
    }

    fn reload_config(&self) -> Result<Option<BoxChatConfig>> {
        let (config_path, current, mode, last_loaded_mtime) = {
            let mut runtime = self.runtime.lock().expect("box chat runtime lock");
            let now = Instant::now();
            if runtime
                .last_reload_check
                .is_some_and(|last| now.duration_since(last) < CONFIG_RELOAD_INTERVAL)
            {
                return Ok(None);
            }
            runtime.last_reload_check = Some(now);
            (
                runtime.config_path.clone(),
                runtime.config.clone(),
                runtime.mode,
                runtime.last_loaded_mtime,
            )
        };
        let next_mtime = config_modified_time(&config_path)?;
        if next_mtime == last_loaded_mtime {
            return Ok(None);
        }

        let next = load_box_chat_config(&config_path)?;
        if next == current {
            let mut runtime = self.runtime.lock().expect("box chat runtime lock");
            runtime.last_loaded_mtime = next_mtime;
            return Ok(None);
        }
        self.configure_with_mode(config_path, next.clone(), mode)?;
        Ok(Some(next))
    }

    fn update_local_clients(&self, clients: Vec<(u32, String)>) {
        let next_clients = clients
            .into_iter()
            .filter_map(|(pid, name)| {
                let trimmed = name.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(LocalClient {
                        pid,
                        name: trimmed.to_string(),
                        name_lower: trimmed.to_ascii_lowercase(),
                    })
                }
            })
            .collect::<Vec<_>>();

        *self
            .shared
            .local_clients
            .lock()
            .expect("box chat client lock") = next_clients;

        let characters = self.character_names();
        if let Some(connector) = self
            .runtime
            .lock()
            .expect("box chat runtime lock")
            .connector
            .as_ref()
        {
            connector.update_characters(characters);
        }
    }

    fn dispatch_if_box_chat(&self, input: &str) -> Result<Option<DispatchReport>> {
        let route = match parse_slash_route(input) {
            None => return Ok(None),
            Some(Ok(route)) => route,
            Some(Err(error)) => bail!(error),
        };

        let report = self.dispatch_route(&route)?;
        Ok(Some(report))
    }

    fn dispatch_route(&self, route: &OutboundRoute) -> Result<DispatchReport> {
        let (local_sent, local_failed) = execute_route_locally(&self.shared, route)?;

        let (relayed_to_peers, forwarded_to_server, server_connected) = {
            let runtime = self.runtime.lock().expect("box chat runtime lock");

            if !runtime.config.enabled {
                (false, false, false)
            } else {
                let relayed = runtime
                    .listener
                    .as_ref()
                    .is_some_and(|listener| listener.hub.relay_execute(route, None));
                let connected = runtime
                    .connector
                    .as_ref()
                    .is_some_and(ConnectorHandle::is_connected);
                if let Some(connector) = runtime.connector.as_ref() {
                    connector.send_route(route);
                }
                (relayed, runtime.connector.is_some(), connected)
            }
        };

        Ok(DispatchReport {
            route: route.clone(),
            local_sent,
            local_failed,
            relayed_to_peers,
            forwarded_to_server,
            server_connected,
        })
    }

    fn character_names(&self) -> Vec<String> {
        self.shared
            .local_clients
            .lock()
            .expect("box chat client lock")
            .iter()
            .map(|client| client.name.clone())
            .collect()
    }
}

/// Result of dispatching a box-chat route locally and over the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchReport {
    pub route: OutboundRoute,
    pub local_sent: usize,
    pub local_failed: usize,
    pub relayed_to_peers: bool,
    pub forwarded_to_server: bool,
    pub server_connected: bool,
}

impl DispatchReport {
    #[must_use]
    pub fn summary(&self) -> String {
        let route = match &self.route {
            OutboundRoute::Broadcast { command } => format!("bc {command}"),
            OutboundRoute::Target { character, command } => format!("bct {character} {command}"),
        };
        format!(
            "{route} → local sent {}, local failed {}, peer relay {}, upstream {}",
            self.local_sent,
            self.local_failed,
            on_off(self.relayed_to_peers),
            if self.forwarded_to_server {
                if self.server_connected {
                    "connected"
                } else {
                    "queued/disconnected"
                }
            } else {
                "off"
            }
        )
    }
}

#[must_use]
pub fn default_config_path() -> PathBuf {
    PathBuf::from("config/textquest.toml")
}

/// Start or reconfigure the box-chat runtime for a long-lived TextQuest
/// process.
pub fn configure(config_path: PathBuf, config: BoxChatConfig) -> Result<()> {
    BoxChatManager::global().configure(config_path, config)
}

/// Start box-chat in a connector-only mode suitable for one-shot CLI dispatch.
pub fn configure_connector_only(config_path: PathBuf, config: BoxChatConfig) -> Result<()> {
    BoxChatManager::global().configure_connector_only(config_path, config)
}

/// Stop all box-chat listener and connector threads in this process.
pub fn stop() {
    BoxChatManager::global().stop();
}

/// Re-read the configured TOML file and apply any box-chat changes without
/// restart.
pub fn reload_from_disk() -> Result<Option<BoxChatConfig>> {
    BoxChatManager::global().reload_config()
}

/// Update the set of live characters that belong to this TextQuest process.
pub fn update_local_clients<I>(clients: I)
where
    I: IntoIterator<Item = (u32, String)>,
{
    BoxChatManager::global().update_local_clients(clients.into_iter().collect());
}

/// Parse and dispatch a `/bc`-style route if `input` matches one.
pub fn dispatch_if_box_chat(input: &str) -> Result<Option<DispatchReport>> {
    BoxChatManager::global().dispatch_if_box_chat(input)
}

fn load_box_chat_config(path: &Path) -> Result<BoxChatConfig> {
    if path.exists() {
        Ok(crate::config::AppConfig::load(path)?.box_chat)
    } else {
        Ok(BoxChatConfig::default())
    }
}

fn config_modified_time(path: &Path) -> Result<Option<SystemTime>> {
    match std::fs::metadata(path) {
        Ok(metadata) => metadata
            .modified()
            .map(Some)
            .map_err(|error| anyhow!(error).context("failed to read box-chat config mtime")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(anyhow!(error).context("failed to stat box-chat config")),
    }
}

fn is_self_connector_target(config: &BoxChatConfig) -> bool {
    let host = config
        .host
        .trim()
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();
    matches!(host.as_str(), "127.0.0.1" | "localhost" | "0.0.0.0" | "::1")
        || host == node_name().to_ascii_lowercase()
}

fn execute_route_locally(shared: &SharedState, route: &OutboundRoute) -> Result<(usize, usize)> {
    let clients = shared
        .local_clients
        .lock()
        .expect("box chat client lock")
        .clone();

    let target_pids: Vec<u32> = match route {
        OutboundRoute::Broadcast { .. } => clients.iter().map(|client| client.pid).collect(),
        OutboundRoute::Target { character, .. } => {
            let wanted = character.to_ascii_lowercase();
            clients
                .iter()
                .filter(|client| client.name_lower == wanted)
                .map(|client| client.pid)
                .collect()
        }
    };

    let command = match route {
        OutboundRoute::Broadcast { command } | OutboundRoute::Target { command, .. } => command,
    };

    let mut local_sent = 0usize;
    let mut local_failed = 0usize;

    for pid in target_pids {
        match crate::command_dispatch::dispatch_local_command(pid, command) {
            Ok(()) => local_sent += 1,
            Err(error) => {
                local_failed += 1;
                tracing::warn!(pid, %command, %error, "Failed local box-chat dispatch");
            }
        }
    }

    Ok((local_sent, local_failed))
}

fn start_listener(port: u16, shared: Arc<SharedState>) -> Result<ListenerHandle> {
    let stop = Arc::new(AtomicBool::new(false));
    let hub = Arc::new(HubState::default());
    let listener = TcpListener::bind(("0.0.0.0", port))
        .with_context(|| format!("failed to bind box-chat listener on 0.0.0.0:{port}"))?;
    listener
        .set_nonblocking(true)
        .context("failed to set box-chat listener nonblocking")?;

    let stop_flag = Arc::clone(&stop);
    let hub_state = Arc::clone(&hub);
    let join = thread::Builder::new()
        .name(format!("textquest-box-chat-listener-{port}"))
        .spawn(move || listener_loop(listener, stop_flag, hub_state, shared))
        .context("failed to spawn box-chat listener thread")?;

    Ok(ListenerHandle { stop, join, hub })
}

fn listener_loop(
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    hub: Arc<HubState>,
    shared: Arc<SharedState>,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, addr)) => {
                tracing::info!(%addr, "Box-chat peer connected");
                spawn_hub_peer(
                    stream,
                    Arc::clone(&hub),
                    Arc::clone(&shared),
                    Arc::clone(&stop),
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(IO_POLL_INTERVAL);
            }
            Err(error) => {
                tracing::warn!(%error, "Box-chat listener accept failed");
                thread::sleep(IO_POLL_INTERVAL);
            }
        }
    }
}

fn spawn_hub_peer(
    stream: TcpStream,
    hub: Arc<HubState>,
    shared: Arc<SharedState>,
    stop: Arc<AtomicBool>,
) {
    let (tx, rx) = mpsc::channel::<WireMessage>();
    let peer_id = hub.register_peer(tx);
    let mut writer_stream = match stream.try_clone() {
        Ok(clone) => clone,
        Err(error) => {
            tracing::warn!(%error, "Failed to clone box-chat peer stream");
            hub.remove_peer(peer_id);
            return;
        }
    };
    let _ = writer_stream.set_write_timeout(Some(IO_POLL_INTERVAL));

    let writer_hub = Arc::clone(&hub);
    let writer_stop = Arc::clone(&stop);
    thread::spawn(move || {
        while !writer_stop.load(Ordering::Relaxed) {
            match rx.recv_timeout(IO_POLL_INTERVAL) {
                Ok(message) => {
                    if let Err(error) = write_message(&mut writer_stream, &message) {
                        tracing::warn!(%error, peer_id, "Box-chat peer write failed");
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        writer_hub.remove_peer(peer_id);
    });

    let _ = stream.set_read_timeout(Some(IO_POLL_INTERVAL));
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        while !stop.load(Ordering::Relaxed) {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if let Some(message) = parse_message(&line) {
                        handle_hub_message(&hub, &shared, peer_id, message);
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(error) => {
                    tracing::warn!(%error, peer_id, "Box-chat peer read failed");
                    break;
                }
            }
        }
        hub.remove_peer(peer_id);
    });
}

fn handle_hub_message(hub: &HubState, shared: &SharedState, peer_id: usize, message: WireMessage) {
    match message {
        WireMessage::Broadcast { command } => {
            let route = OutboundRoute::Broadcast { command };
            let _ = execute_route_locally(shared, &route);
            let _ = hub.relay_execute(&route, Some(peer_id));
        }
        WireMessage::Target { character, command } => {
            let route = OutboundRoute::Target { character, command };
            let _ = execute_route_locally(shared, &route);
            let _ = hub.relay_execute(&route, Some(peer_id));
        }
        WireMessage::ExecuteBroadcast { command } => {
            let _ = execute_route_locally(shared, &OutboundRoute::Broadcast { command });
        }
        WireMessage::ExecuteTarget { character, command } => {
            let _ = execute_route_locally(shared, &OutboundRoute::Target { character, command });
        }
        WireMessage::Hello {
            node_name,
            characters,
        } => {
            tracing::debug!(%node_name, ?characters, peer_id, "Box-chat hello received");
        }
        WireMessage::UpdateCharacters { characters } => {
            tracing::debug!(
                ?characters,
                peer_id,
                "Box-chat peer character update received"
            );
        }
    }
}

fn start_connector(
    config: BoxChatConfig,
    characters: Vec<String>,
    shared: Arc<SharedState>,
) -> Result<ConnectorHandle> {
    let stop = Arc::new(AtomicBool::new(false));
    let connected = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<ConnectorCommand>();
    let stop_flag = Arc::clone(&stop);
    let connected_flag = Arc::clone(&connected);
    let join = thread::Builder::new()
        .name(format!("textquest-box-chat-connector-{}", config.port))
        .spawn(move || connector_loop(config, characters, shared, rx, stop_flag, connected_flag))
        .context("failed to spawn box-chat connector thread")?;

    Ok(ConnectorHandle {
        stop,
        connected,
        tx,
        join,
    })
}

fn connector_loop(
    config: BoxChatConfig,
    mut characters: Vec<String>,
    shared: Arc<SharedState>,
    rx: mpsc::Receiver<ConnectorCommand>,
    stop: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
) {
    let node_name = node_name();
    let endpoint = format!("{}:{}", config.host, config.port);

    while !stop.load(Ordering::Relaxed) {
        connected.store(false, Ordering::Relaxed);
        match TcpStream::connect(&endpoint) {
            Ok(mut stream) => {
                tracing::info!(%endpoint, "Box-chat upstream connected");
                let _ = stream.set_read_timeout(Some(IO_POLL_INTERVAL));
                let _ = stream.set_write_timeout(Some(IO_POLL_INTERVAL));
                let reader_stream = match stream.try_clone() {
                    Ok(clone) => clone,
                    Err(error) => {
                        tracing::warn!(%error, %endpoint, "Failed to clone upstream box-chat stream");
                        thread::sleep(RECONNECT_DELAY);
                        continue;
                    }
                };
                let mut reader = BufReader::new(reader_stream);
                connected.store(true, Ordering::Relaxed);

                if let Err(error) = write_message(
                    &mut stream,
                    &WireMessage::Hello {
                        node_name: node_name.clone(),
                        characters: characters.clone(),
                    },
                ) {
                    tracing::warn!(%error, %endpoint, "Failed to send box-chat hello");
                    connected.store(false, Ordering::Relaxed);
                    thread::sleep(RECONNECT_DELAY);
                    continue;
                }

                'connected: loop {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }

                    let mut line = String::new();
                    match reader.read_line(&mut line) {
                        Ok(0) => break 'connected,
                        Ok(_) => {
                            if let Some(message) = parse_message(&line) {
                                handle_connector_message(&shared, message);
                            }
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) => {}
                        Err(error) => {
                            tracing::warn!(%error, %endpoint, "Box-chat upstream read failed");
                            break 'connected;
                        }
                    }

                    loop {
                        match rx.try_recv() {
                            Ok(ConnectorCommand::UpdateCharacters(next)) => {
                                characters = next.clone();
                                if let Err(error) = write_message(
                                    &mut stream,
                                    &WireMessage::UpdateCharacters { characters: next },
                                ) {
                                    tracing::warn!(%error, %endpoint, "Failed to send box-chat character update");
                                    break 'connected;
                                }
                            }
                            Ok(ConnectorCommand::Route(message)) => {
                                if let Err(error) = write_message(&mut stream, &message) {
                                    tracing::warn!(%error, %endpoint, "Failed to send upstream box-chat route");
                                    break 'connected;
                                }
                            }
                            Ok(ConnectorCommand::Stop) => return,
                            Err(mpsc::TryRecvError::Empty) => break,
                            Err(mpsc::TryRecvError::Disconnected) => return,
                        }
                    }
                }
            }
            Err(error) => {
                tracing::debug!(%error, %endpoint, "Box-chat upstream connect failed");
            }
        }

        connected.store(false, Ordering::Relaxed);
        if stop.load(Ordering::Relaxed) {
            break;
        }

        let reconnect_until = std::time::Instant::now() + RECONNECT_DELAY;
        while std::time::Instant::now() < reconnect_until {
            match rx.recv_timeout(IO_POLL_INTERVAL) {
                Ok(ConnectorCommand::UpdateCharacters(next)) => {
                    characters = next;
                }
                Ok(ConnectorCommand::Route(_)) => {}
                Ok(ConnectorCommand::Stop) => return,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    }
}

fn handle_connector_message(shared: &SharedState, message: WireMessage) {
    match message {
        WireMessage::ExecuteBroadcast { command } => {
            let _ = execute_route_locally(shared, &OutboundRoute::Broadcast { command });
        }
        WireMessage::ExecuteTarget { character, command } => {
            let _ = execute_route_locally(shared, &OutboundRoute::Target { character, command });
        }
        WireMessage::Hello {
            node_name,
            characters,
        } => {
            tracing::debug!(%node_name, ?characters, "Box-chat upstream hello received");
        }
        WireMessage::UpdateCharacters { characters } => {
            tracing::debug!(?characters, "Box-chat upstream character update received");
        }
        WireMessage::Broadcast { .. } | WireMessage::Target { .. } => {
            tracing::debug!("Ignoring routed box-chat payload from upstream");
        }
    }
}

fn parse_message(line: &str) -> Option<WireMessage> {
    match serde_json::from_str::<WireMessage>(line.trim()) {
        Ok(message) => Some(message),
        Err(error) => {
            tracing::warn!(payload = line.trim(), %error, "Failed to parse box-chat message");
            None
        }
    }
}

fn write_message(stream: &mut TcpStream, message: &WireMessage) -> Result<()> {
    let mut payload =
        serde_json::to_vec(message).context("failed to serialize box-chat message")?;
    payload.push(b'\n');
    stream
        .write_all(&payload)
        .map_err(|error| anyhow!(error))
        .context("failed to write box-chat message")?;
    stream.flush().map_err(|error| anyhow!(error))?;
    Ok(())
}

fn stop_listener(runtime: &mut RuntimeState) {
    if let Some(listener) = runtime.listener.take() {
        listener.stop();
    }
}

fn stop_connector(runtime: &mut RuntimeState) {
    if let Some(connector) = runtime.connector.take() {
        connector.stop();
    }
}

fn node_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("textquest-{}", std::process::id()))
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn dispatch_report_summary_mentions_route_and_transport() {
        let summary = DispatchReport {
            route: OutboundRoute::Target {
                character: "Cleric01".to_string(),
                command: "/cast 1".to_string(),
            },
            local_sent: 1,
            local_failed: 0,
            relayed_to_peers: true,
            forwarded_to_server: true,
            server_connected: true,
        }
        .summary();

        assert!(summary.contains("bct Cleric01 /cast 1"));
        assert!(summary.contains("peer relay on"));
        assert!(summary.contains("upstream connected"));
    }

    #[test]
    fn load_box_chat_config_defaults_when_file_missing() {
        let path = std::env::temp_dir().join(format!(
            "textquest-box-chat-missing-{}.toml",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);

        let config = load_box_chat_config(&path).expect("missing file should default");
        assert_eq!(config, BoxChatConfig::default());
    }
}
