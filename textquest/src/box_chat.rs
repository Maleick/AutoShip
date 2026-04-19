//! EQBC-style box-chat runtime for cross-machine slash-command relay.

use std::{
    collections::{HashMap, HashSet},
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
use textquest_common::{
    box_chat::{BoxChatConfig, OutboundRoute, WireMessage, parse_slash_route},
    box_controller::{
        BoxControllerClientSnapshot, BoxControllerClientState, BoxControllerCommand,
        BoxControllerSnapshot,
    },
};

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
    controller: Mutex<ControllerRegistry>,
    connector_nodes: Mutex<HashSet<String>>,
}

#[derive(Debug, Default)]
struct ControllerRegistry {
    local_states: HashMap<String, BoxControllerClientState>,
    remote_states: HashMap<String, RemoteControllerState>,
    last_command: Option<BoxControllerCommand>,
}

#[derive(Debug, Default)]
struct RemoteControllerState {
    display_name: String,
    clients: HashMap<String, BoxControllerClientState>,
}

impl ControllerRegistry {
    fn normalize_node_key(node: &str) -> String {
        node.to_ascii_lowercase()
    }

    fn has_local_clients(&self) -> bool {
        !self.local_states.is_empty()
    }

    fn sync_local_clients(&mut self, clients: &[LocalClient]) {
        let expected = clients
            .iter()
            .map(|client| client.name_lower.clone())
            .collect::<HashSet<_>>();
        self.local_states
            .retain(|character, _| expected.contains(character));

        for client in clients {
            self.local_states
                .entry(client.name_lower.clone())
                .and_modify(|state| state.character_name = client.name.clone())
                .or_insert_with(|| BoxControllerClientState::new(client.name.clone()));
        }
    }

    fn apply_local_command(&mut self, command: &BoxControllerCommand) {
        self.last_command = Some(command.clone());
        for state in self.local_states.values_mut() {
            state.apply_command(command);
        }
    }

    fn update_remote_state(&mut self, node: &str, clients: Vec<BoxControllerClientState>) {
        let key = Self::normalize_node_key(node);
        let states = clients
            .into_iter()
            .map(|state| (state.character_name.to_ascii_lowercase(), state))
            .collect::<HashMap<_, _>>();
        if states.is_empty() {
            self.remote_states.remove(&key);
        } else {
            self.remote_states.insert(
                key,
                RemoteControllerState {
                    display_name: node.to_string(),
                    clients: states,
                },
            );
        }
    }

    fn seed_remote_characters(&mut self, node: &str, characters: &[String]) {
        let key = Self::normalize_node_key(node);
        let expected = characters
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let state = self
            .remote_states
            .entry(key)
            .or_insert_with(|| RemoteControllerState {
                display_name: node.to_string(),
                clients: HashMap::new(),
            });
        state.display_name = node.to_string();
        let states = &mut state.clients;
        states.retain(|character, _| expected.contains(character));

        for character in characters {
            let key = character.to_ascii_lowercase();
            states
                .entry(key)
                .and_modify(|state| state.character_name = character.clone())
                .or_insert_with(|| BoxControllerClientState::new(character.clone()));
        }
    }

    fn remove_remote_node(&mut self, node: &str) {
        self.remote_states.remove(&Self::normalize_node_key(node));
    }

    fn local_state_message(&self) -> WireMessage {
        let mut clients = self.local_states.values().cloned().collect::<Vec<_>>();
        clients.sort_by(|left, right| left.character_name.cmp(&right.character_name));
        WireMessage::BoxControllerState {
            node_name: node_name(),
            clients,
        }
    }

    fn all_state_messages(&self) -> Vec<WireMessage> {
        let mut messages = Vec::new();
        if self.has_local_clients() {
            messages.push(self.local_state_message());
        }
        let mut nodes = self.remote_states.iter().collect::<Vec<_>>();
        nodes.sort_by_key(|(node_name, _)| *node_name);

        for (_, state) in nodes {
            let mut clients = state.clients.values().cloned().collect::<Vec<_>>();
            clients.sort_by(|left, right| left.character_name.cmp(&right.character_name));
            messages.push(WireMessage::BoxControllerState {
                node_name: state.display_name.clone(),
                clients,
            });
        }

        messages
    }

    fn snapshot(&self, relay_enabled: bool) -> BoxControllerSnapshot {
        let local_node = node_name();
        let mut clients = self
            .local_states
            .values()
            .map(|state| BoxControllerClientSnapshot::from_state(local_node.clone(), state))
            .collect::<Vec<_>>();

        let mut remote_nodes = self.remote_states.iter().collect::<Vec<_>>();
        remote_nodes.sort_by_key(|(node_name, _)| *node_name);
        for (_, remote_state) in remote_nodes {
            let mut next = remote_state
                .clients
                .values()
                .map(|state| {
                    BoxControllerClientSnapshot::from_state(
                        remote_state.display_name.clone(),
                        state,
                    )
                })
                .collect::<Vec<_>>();
            next.sort_by(|left, right| left.character_name.cmp(&right.character_name));
            clients.extend(next);
        }

        clients.sort_by(|left, right| {
            left.node_name
                .cmp(&right.node_name)
                .then_with(|| left.character_name.cmp(&right.character_name))
        });

        BoxControllerSnapshot {
            connected_clients: clients.len(),
            relay_enabled,
            last_command: self.last_command.clone(),
            clients,
        }
    }
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
        self.send_message(message);
    }

    fn send_box_controller_command(&self, command: BoxControllerCommand) {
        self.send_message(WireMessage::BoxControllerCommand { command });
    }

    fn send_box_controller_state(&self, message: WireMessage) {
        self.send_message(message);
    }

    fn send_message(&self, message: WireMessage) {
        let _ = self.tx.send(ConnectorCommand::Send(message));
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
    Send(WireMessage),
    Stop,
}

#[derive(Default)]
struct HubState {
    next_peer_id: AtomicUsize,
    peers: Mutex<HashMap<usize, mpsc::Sender<WireMessage>>>,
    peer_nodes: Mutex<HashMap<usize, String>>,
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

    fn remove_peer(&self, peer_id: usize) -> Option<(String, bool)> {
        self.peers.lock().expect("hub peer lock").remove(&peer_id);
        let mut peer_nodes = self.peer_nodes.lock().expect("hub peer-node lock");
        let node_name = peer_nodes.remove(&peer_id)?;
        let normalized = ControllerRegistry::normalize_node_key(&node_name);
        let still_connected = peer_nodes
            .values()
            .any(|other| ControllerRegistry::normalize_node_key(other) == normalized);
        Some((node_name, still_connected))
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

    fn send_to_peer(&self, peer_id: usize, message: WireMessage) -> bool {
        self.peers
            .lock()
            .expect("hub peer lock")
            .get(&peer_id)
            .is_some_and(|sender| sender.send(message).is_ok())
    }

    fn set_peer_node_name(&self, peer_id: usize, node_name: String) {
        self.peer_nodes
            .lock()
            .expect("hub peer-node lock")
            .insert(peer_id, node_name);
    }

    fn peer_node_name(&self, peer_id: usize) -> Option<String> {
        self.peer_nodes
            .lock()
            .expect("hub peer-node lock")
            .get(&peer_id)
            .cloned()
    }
}

fn disconnect_peer(hub: &HubState, shared: &SharedState, peer_id: usize) {
    if let Some((node_name, still_connected)) = hub.remove_peer(peer_id) {
        if still_connected {
            return;
        }
        shared
            .controller
            .lock()
            .expect("controller registry lock")
            .remove_remote_node(&node_name);
        let _ = hub.broadcast(
            WireMessage::BoxControllerState {
                node_name,
                clients: Vec::new(),
            },
            None,
        );
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

        {
            let clients = self
                .shared
                .local_clients
                .lock()
                .expect("box chat client lock")
                .clone();
            self.shared
                .controller
                .lock()
                .expect("box controller lock")
                .sync_local_clients(&clients);
        }

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

        self.broadcast_local_controller_state();
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

    fn dispatch_box_controller_command(&self, command: BoxControllerCommand) {
        self.shared
            .controller
            .lock()
            .expect("box controller lock")
            .apply_local_command(&command);

        let should_publish_state = self
            .shared
            .controller
            .lock()
            .expect("box controller lock")
            .has_local_clients();
        let state_message = should_publish_state.then(|| self.local_controller_state_message());
        let runtime = self.runtime.lock().expect("box chat runtime lock");

        if let Some(listener) = runtime.listener.as_ref() {
            let _ = listener.hub.broadcast(
                WireMessage::BoxControllerCommand {
                    command: command.clone(),
                },
                None,
            );
            if let Some(state_message) = state_message.clone() {
                let _ = listener.hub.broadcast(state_message, None);
            }
        }

        if let Some(connector) = runtime.connector.as_ref() {
            connector.send_box_controller_command(command);
            if let Some(state_message) = state_message {
                connector.send_box_controller_state(state_message);
            }
        }
    }

    fn local_controller_state_message(&self) -> WireMessage {
        self.shared
            .controller
            .lock()
            .expect("box controller lock")
            .local_state_message()
    }

    fn broadcast_local_controller_state(&self) {
        let state_message = self.local_controller_state_message();
        let runtime = self.runtime.lock().expect("box chat runtime lock");

        if let Some(listener) = runtime.listener.as_ref() {
            let _ = listener.hub.broadcast(state_message.clone(), None);
        }
        if let Some(connector) = runtime.connector.as_ref() {
            connector.send_box_controller_state(state_message);
        }
    }

    fn controller_snapshot(&self) -> BoxControllerSnapshot {
        let runtime = self.runtime.lock().expect("box chat runtime lock");
        let relay_enabled = runtime.listener.is_some()
            || runtime
                .connector
                .as_ref()
                .is_some_and(ConnectorHandle::is_connected);
        drop(runtime);
        self.shared
            .controller
            .lock()
            .expect("box controller lock")
            .snapshot(relay_enabled)
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

/// Broadcast a unified box-controller command to all local and connected box
/// chat clients.
pub fn dispatch_box_controller_command(command: BoxControllerCommand) {
    BoxChatManager::global().dispatch_box_controller_command(command);
}

/// Snapshot the current unified box-controller state known to this process.
#[must_use]
pub fn controller_snapshot() -> BoxControllerSnapshot {
    BoxChatManager::global().controller_snapshot()
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

fn local_controller_state_message(shared: &SharedState) -> WireMessage {
    shared
        .controller
        .lock()
        .expect("box controller lock")
        .local_state_message()
}

fn apply_local_controller_command(shared: &SharedState, command: &BoxControllerCommand) {
    shared
        .controller
        .lock()
        .expect("box controller lock")
        .apply_local_command(command);
}

fn update_remote_controller_state(
    shared: &SharedState,
    node_name: &str,
    clients: Vec<BoxControllerClientState>,
) {
    shared
        .controller
        .lock()
        .expect("box controller lock")
        .update_remote_state(node_name, clients);
}

fn seed_remote_controller_characters(shared: &SharedState, node_name: &str, characters: &[String]) {
    shared
        .controller
        .lock()
        .expect("box controller lock")
        .seed_remote_characters(node_name, characters);
}

fn send_known_controller_states_to_peer(hub: &HubState, shared: &SharedState, peer_id: usize) {
    let messages = shared
        .controller
        .lock()
        .expect("box controller lock")
        .all_state_messages();

    for message in messages {
        let _ = hub.send_to_peer(peer_id, message);
    }
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
            disconnect_peer(&hub, &shared, peer_id);
            return;
        }
    };
    let _ = writer_stream.set_write_timeout(Some(IO_POLL_INTERVAL));

    let writer_hub = Arc::clone(&hub);
    let writer_shared = Arc::clone(&shared);
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
        disconnect_peer(&writer_hub, &writer_shared, peer_id);
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
        disconnect_peer(&hub, &shared, peer_id);
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
        WireMessage::BoxControllerCommand { command } => {
            apply_local_controller_command(shared, &command);
            let _ = hub.broadcast(WireMessage::BoxControllerCommand { command }, Some(peer_id));
            let _ = hub.broadcast(local_controller_state_message(shared), None);
        }
        WireMessage::BoxControllerState { node_name, clients } => {
            hub.set_peer_node_name(peer_id, node_name.clone());
            update_remote_controller_state(shared, &node_name, clients.clone());
            let _ = hub.broadcast(
                WireMessage::BoxControllerState { node_name, clients },
                Some(peer_id),
            );
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
            hub.set_peer_node_name(peer_id, node_name.clone());
            seed_remote_controller_characters(shared, &node_name, &characters);
            send_known_controller_states_to_peer(hub, shared, peer_id);
            tracing::debug!(%node_name, ?characters, peer_id, "Box-chat hello received");
        }
        WireMessage::UpdateCharacters { characters } => {
            if let Some(node_name) = hub.peer_node_name(peer_id) {
                seed_remote_controller_characters(shared, &node_name, &characters);
            }
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

                let has_local_clients = shared
                    .controller
                    .lock()
                    .expect("box controller lock")
                    .has_local_clients();
                if has_local_clients {
                    let state_message = local_controller_state_message(&shared);
                    if let Err(error) = write_message(&mut stream, &state_message) {
                        tracing::warn!(%error, %endpoint, "Failed to send initial box-controller state");
                        connected.store(false, Ordering::Relaxed);
                        thread::sleep(RECONNECT_DELAY);
                        continue;
                    }
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
                                let publish_local_state =
                                    matches!(message, WireMessage::BoxControllerCommand { .. });
                                handle_connector_message(&shared, message);
                                if publish_local_state {
                                    let has_local_clients = shared
                                        .controller
                                        .lock()
                                        .expect("box controller lock")
                                        .has_local_clients();
                                    if has_local_clients {
                                        let state_message = local_controller_state_message(&shared);
                                        if let Err(error) =
                                            write_message(&mut stream, &state_message)
                                        {
                                            tracing::warn!(%error, %endpoint, "Failed to publish updated box-controller state");
                                            break 'connected;
                                        }
                                    }
                                }
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
                            Ok(ConnectorCommand::Send(message)) => {
                                if let Err(error) = write_message(&mut stream, &message) {
                                    tracing::warn!(%error, %endpoint, "Failed to send upstream box-chat route");
                                    break 'connected;
                                }
                            }
                            Ok(ConnectorCommand::Stop) => {
                                prune_connector_remote_states(&shared);
                                return;
                            }
                            Err(mpsc::TryRecvError::Empty) => break,
                            Err(mpsc::TryRecvError::Disconnected) => {
                                prune_connector_remote_states(&shared);
                                return;
                            }
                        }
                    }
                }
            }
            Err(error) => {
                tracing::debug!(%error, %endpoint, "Box-chat upstream connect failed");
            }
        }

        prune_connector_remote_states(&shared);
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
                Ok(ConnectorCommand::Send(_)) => {}
                Ok(ConnectorCommand::Stop) => {
                    prune_connector_remote_states(&shared);
                    return;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if stop.load(Ordering::Relaxed) {
                        prune_connector_remote_states(&shared);
                        return;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    prune_connector_remote_states(&shared);
                    return;
                }
            }
        }
    }
}

fn track_connector_node(shared: &SharedState, node_name: &str) {
    shared
        .connector_nodes
        .lock()
        .expect("connector node lock")
        .insert(ControllerRegistry::normalize_node_key(node_name));
}

fn prune_connector_remote_states(shared: &SharedState) {
    let nodes = shared
        .connector_nodes
        .lock()
        .expect("connector node lock")
        .drain()
        .collect::<Vec<_>>();
    if nodes.is_empty() {
        return;
    }

    let mut controller = shared.controller.lock().expect("controller registry lock");
    for node in nodes {
        controller.remote_states.remove(&node);
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
            track_connector_node(shared, &node_name);
            seed_remote_controller_characters(shared, &node_name, &characters);
            tracing::debug!(%node_name, ?characters, "Box-chat upstream hello received");
        }
        WireMessage::UpdateCharacters { characters } => {
            tracing::debug!(?characters, "Box-chat upstream character update received");
        }
        WireMessage::BoxControllerCommand { command } => {
            apply_local_controller_command(shared, &command);
        }
        WireMessage::BoxControllerState { node_name, clients } => {
            track_connector_node(shared, &node_name);
            update_remote_controller_state(shared, &node_name, clients);
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
    use textquest_common::box_controller::BoxControllerMode;

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

    #[test]
    fn controller_registry_normalizes_remote_node_names() {
        let mut registry = ControllerRegistry::default();
        registry.seed_remote_characters("Raid-PC", &["Cleric".to_string()]);

        let mut paused = BoxControllerClientState::new("Cleric".to_string());
        paused.apply_command(&BoxControllerCommand::Pause);
        registry.update_remote_state("raid-pc", vec![paused]);

        assert_eq!(registry.remote_states.len(), 1);

        let snapshot = registry.snapshot(true);
        assert_eq!(snapshot.connected_clients, 1);
        assert_eq!(snapshot.clients[0].mode, BoxControllerMode::Paused);
    }

    #[test]
    fn disconnecting_peer_clears_remote_controller_state() {
        let mut registry = ControllerRegistry::default();
        registry.seed_remote_characters("Raid-PC", &["Cleric".to_string()]);

        let hub = HubState::default();
        let peer_id = hub.register_peer(mpsc::channel::<WireMessage>().0);
        hub.set_peer_node_name(peer_id, "RAID-pc".to_string());

        let shared = SharedState::default();
        {
            let mut controller = shared.controller.lock().expect("controller registry lock");
            controller.remote_states = registry.remote_states;
        }

        disconnect_peer(&hub, &shared, peer_id);

        let snapshot = shared
            .controller
            .lock()
            .expect("controller registry lock")
            .snapshot(true);
        assert_eq!(snapshot.connected_clients, 0);
    }

    #[test]
    fn disconnecting_peer_broadcasts_empty_controller_state() {
        let hub = HubState::default();
        let remaining_rx = {
            let (remaining_tx, remaining_rx) = mpsc::channel::<WireMessage>();
            let _ = hub.register_peer(remaining_tx);
            remaining_rx
        };
        let departing_peer = hub.register_peer(mpsc::channel::<WireMessage>().0);
        hub.set_peer_node_name(departing_peer, "RAID-pc".to_string());

        let shared = SharedState::default();
        {
            let mut controller = shared.controller.lock().expect("controller registry lock");
            controller.seed_remote_characters("Raid-PC", &["Cleric".to_string()]);
        }

        disconnect_peer(&hub, &shared, departing_peer);

        let message = remaining_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("peer tombstone broadcast");
        let WireMessage::BoxControllerState { node_name, clients } = message else {
            assert!(false, "expected controller state tombstone");
        };
        assert_eq!(node_name, "RAID-pc");
        assert!(clients.is_empty());
    }

    #[test]
    fn prune_connector_remote_states_clears_tracked_nodes() {
        let shared = SharedState::default();
        seed_remote_controller_characters(&shared, "Upstream-A", &["Cleric".to_string()]);
        update_remote_controller_state(
            &shared,
            "Upstream-A",
            vec![BoxControllerClientState::new("Cleric".to_string())],
        );
        track_connector_node(&shared, "Upstream-A");

        prune_connector_remote_states(&shared);

        let snapshot = shared
            .controller
            .lock()
            .expect("controller registry lock")
            .snapshot(true);
        assert_eq!(snapshot.connected_clients, 0);
    }

    #[test]
    fn connector_updates_from_same_host_are_retained() {
        let shared = SharedState::default();
        let local_host = node_name();

        seed_remote_controller_characters(&shared, &local_host, &["Cleric".to_string()]);
        update_remote_controller_state(
            &shared,
            &local_host,
            vec![BoxControllerClientState::new("Cleric".to_string())],
        );

        let snapshot = shared
            .controller
            .lock()
            .expect("controller registry lock")
            .snapshot(true);
        assert_eq!(snapshot.connected_clients, 1);
        assert_eq!(snapshot.clients[0].node_name, local_host);
    }

    #[test]
    fn disconnecting_stale_duplicate_peer_preserves_active_node_state() {
        let hub = HubState::default();
        let (observer_tx, observer_rx) = mpsc::channel::<WireMessage>();
        let _observer_id = hub.register_peer(observer_tx);
        let departing_peer = hub.register_peer(mpsc::channel::<WireMessage>().0);
        let active_peer = hub.register_peer(mpsc::channel::<WireMessage>().0);
        hub.set_peer_node_name(departing_peer, "RAID-pc".to_string());
        hub.set_peer_node_name(active_peer, "raid-PC".to_string());

        let shared = SharedState::default();
        {
            let mut controller = shared.controller.lock().expect("controller registry lock");
            controller.seed_remote_characters("Raid-PC", &["Cleric".to_string()]);
        }

        disconnect_peer(&hub, &shared, departing_peer);

        let snapshot = shared
            .controller
            .lock()
            .expect("controller registry lock")
            .snapshot(true);
        assert_eq!(snapshot.connected_clients, 1);
        assert!(
            observer_rx
                .recv_timeout(Duration::from_millis(100))
                .is_err(),
            "stale duplicate disconnect should not broadcast a tombstone"
        );
    }

    #[test]
    fn dropping_last_local_client_broadcasts_empty_controller_state() {
        let shared = Arc::new(SharedState::default());
        let listener = start_listener(0, Arc::clone(&shared)).expect("listener");
        let (peer_tx, peer_rx) = mpsc::channel::<WireMessage>();
        let _peer_id = listener.hub.register_peer(peer_tx);

        let manager = BoxChatManager {
            runtime: Mutex::new(RuntimeState {
                listener: Some(listener),
                ..RuntimeState::default()
            }),
            shared,
        };

        manager.update_local_clients([(1_u32, "Frostreaver".to_string())].into_iter().collect());
        let _ = peer_rx.recv_timeout(Duration::from_secs(1));

        manager.update_local_clients(Vec::new());
        let message = peer_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("empty controller state broadcast");

        let WireMessage::BoxControllerState { clients, .. } = message else {
            assert!(false, "expected controller state message");
        };
        assert!(
            clients.is_empty(),
            "expected local state to clear on broadcast"
        );

        let listener = manager
            .runtime
            .lock()
            .expect("box chat runtime lock")
            .listener
            .take()
            .expect("listener handle");
        listener.stop();
    }

    #[test]
    fn controller_snapshot_reports_disconnected_runtime_as_offline() {
        let manager = BoxChatManager {
            runtime: Mutex::new(RuntimeState {
                config: BoxChatConfig {
                    enabled: true,
                    ..BoxChatConfig::default()
                },
                ..RuntimeState::default()
            }),
            shared: Arc::new(SharedState::default()),
        };

        let snapshot = manager.controller_snapshot();
        assert!(!snapshot.relay_enabled);
    }

    #[test]
    fn controller_snapshot_reports_listener_runtime_as_online() {
        let shared = Arc::new(SharedState::default());
        let listener = start_listener(0, Arc::clone(&shared)).expect("listener");
        let manager = BoxChatManager {
            runtime: Mutex::new(RuntimeState {
                listener: Some(listener),
                ..RuntimeState::default()
            }),
            shared,
        };

        let snapshot = manager.controller_snapshot();
        assert!(snapshot.relay_enabled);

        let listener = manager
            .runtime
            .lock()
            .expect("box chat runtime lock")
            .listener
            .take()
            .expect("listener handle");
        listener.stop();
    }
}
