use crate::camp::config::CampConfig;
use crate::camp::state::{CampMember, Role};
use crate::eq::map_parser::ZoneMap;
use crate::eq::structs::{GroupInfo, SpawnInfo, SpawnType};
use crate::orchestrator::Orchestrator;
use crate::soul::coordinator::SoulCoordinator;

/// Which screen is currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveScreen {
    Dashboard,
    Spawns,
    Character,
    Map,
    Groups,
}

impl ActiveScreen {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Spawns => "Spawns",
            Self::Character => "Character",
            Self::Map => "Map",
            Self::Groups => "Groups",
        }
    }

    pub fn key(&self) -> char {
        match self {
            Self::Dashboard => '1',
            Self::Spawns => '2',
            Self::Character => '3',
            Self::Map => '4',
            Self::Groups => '5',
        }
    }

    pub const ALL: [ActiveScreen; 5] = [
        Self::Dashboard,
        Self::Spawns,
        Self::Character,
        Self::Map,
        Self::Groups,
    ];
}

/// Which panel is currently focused for keyboard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    SpawnList,
    HexDump,
}

/// Spawn type filter for the spawn list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnFilter {
    All,
    Pc,
    Npc,
    Named,
}

impl SpawnFilter {
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Pc,
            Self::Pc => Self::Npc,
            Self::Npc => Self::Named,
            Self::Named => Self::All,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Pc => "PC",
            Self::Npc => "NPC",
            Self::Named => "Named",
        }
    }
}

/// Definition for a logical group of accounts.
#[derive(Debug, Clone)]
pub struct GroupDef {
    pub id: u8,
    pub name: String,
    pub account_range: (u8, u8),
    pub default_camp: String,
}

/// Per-client state for each attached EQ process.
#[derive(Debug, Clone)]
pub struct ClientState {
    pub pid: u32,
    pub eq_base: u64,
    pub local_player: Option<SpawnInfo>,
    pub target: Option<SpawnInfo>,
    pub spawns: Vec<SpawnInfo>,
    pub zone_name: String,
    /// Character name parsed from the DLL-renamed window title.
    pub character_name: String,
    /// Group membership info for this client.
    pub group_info: Option<GroupInfo>,
    /// Status message specific to this client.
    pub client_status: String,
}

impl ClientState {
    pub fn new(pid: u32, eq_base: u64) -> Self {
        Self {
            pid,
            eq_base,
            local_player: None,
            target: None,
            spawns: Vec::new(),
            zone_name: String::from("Unknown"),
            character_name: String::new(),
            group_info: None,
            client_status: format!("Attached to PID {}", pid),
        }
    }
}

/// Application state for the TUI debugger.
pub struct App {
    pub running: bool,
    pub active_screen: ActiveScreen,
    pub active_panel: ActivePanel,

    // Multi-client state
    pub clients: Vec<ClientState>,
    pub selected_client: usize,

    // Group definitions (6 groups of 6 accounts each)
    pub groups: Vec<GroupDef>,

    // Server name from config
    pub server_name: String,

    // Legacy single-client fields kept for compatibility
    pub local_player: Option<SpawnInfo>,
    pub target: Option<SpawnInfo>,
    pub spawns: Vec<SpawnInfo>,
    pub status_message: String,
    pub tick_count: u64,

    // Spawn list state
    pub spawn_scroll: usize,
    pub spawn_selected: usize,
    pub spawn_filter: String,
    pub spawn_type_filter: SpawnFilter,
    pub search_mode: bool,

    // Hex dump state
    pub hex_address: usize,
    pub hex_data: Vec<u8>,
    pub hex_label: String,

    // Refresh timing
    pub refresh_rate_ms: u64,

    // EQ connection info (legacy — first client)
    pub eq_base: u64,
    pub attached_pid: Option<u32>,

    // Soul Engine
    pub soul_coordinator: Option<SoulCoordinator>,
    pub soul_tick_counter: u64,

    // Zone map data
    pub zone_map: Option<ZoneMap>,
    pub map_dir: std::path::PathBuf,

    // Privacy mode — hides own character names and server for screenshots
    pub privacy_mode: bool,

    // Command bar state (: mode)
    pub command_mode: bool,
    pub command_buffer: String,
    pub command_history: Vec<String>,
    pub command_history_idx: Option<usize>,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            active_screen: ActiveScreen::Dashboard,
            active_panel: ActivePanel::SpawnList,

            clients: Vec::new(),
            selected_client: 0,
            groups: vec![
                GroupDef { id: 1, name: "Alpha".into(), account_range: (1, 6), default_camp: "Camp A".into() },
                GroupDef { id: 2, name: "Bravo".into(), account_range: (7, 12), default_camp: "Camp B".into() },
                GroupDef { id: 3, name: "Charlie".into(), account_range: (13, 18), default_camp: "Camp C".into() },
                GroupDef { id: 4, name: "Delta".into(), account_range: (19, 24), default_camp: "Camp D".into() },
                GroupDef { id: 5, name: "Echo".into(), account_range: (25, 30), default_camp: "Camp E".into() },
                GroupDef { id: 6, name: "Foxtrot".into(), account_range: (31, 36), default_camp: "Camp F".into() },
            ],

            server_name: String::from("Firiona Vie"),

            local_player: None,
            target: None,
            spawns: Vec::new(),
            status_message: String::from("Waiting for EQ process..."),
            tick_count: 0,

            spawn_scroll: 0,
            spawn_selected: 0,
            spawn_filter: String::new(),
            spawn_type_filter: SpawnFilter::All,
            search_mode: false,

            hex_address: 0,
            hex_data: Vec::new(),
            hex_label: String::from("No address selected"),

            refresh_rate_ms: 250,

            eq_base: 0,
            attached_pid: None,

            soul_coordinator: None,
            soul_tick_counter: 0,

            zone_map: None,
            map_dir: std::path::PathBuf::from("config/maps"),

            privacy_mode: false,

            command_mode: false,
            command_buffer: String::new(),
            command_history: Vec::new(),
            command_history_idx: None,
        }
    }

    /// Get the currently selected client, if any.
    pub fn active_client(&self) -> Option<&ClientState> {
        self.clients.get(self.selected_client)
    }

    /// Sync the legacy single-client fields from the selected client.
    /// This keeps backward compatibility with code that reads app.local_player, etc.
    pub fn sync_from_selected_client(&mut self) {
        if let Some(client) = self.clients.get(self.selected_client) {
            self.local_player = client.local_player.clone();
            self.target = client.target.clone();
            self.spawns = client.spawns.clone();
            self.attached_pid = Some(client.pid);
            self.eq_base = client.eq_base;
        } else if self.clients.is_empty() {
            // No clients — clear data
            self.local_player = None;
            self.target = None;
            self.spawns.clear();
        }
    }

    /// Cycle to the next client.
    pub fn next_client(&mut self) {
        if !self.clients.is_empty() {
            self.selected_client = (self.selected_client + 1) % self.clients.len();
            self.sync_from_selected_client();
            self.spawn_selected = 0;
        }
    }

    /// Cycle to the previous client.
    pub fn prev_client(&mut self) {
        if !self.clients.is_empty() {
            if self.selected_client == 0 {
                self.selected_client = self.clients.len() - 1;
            } else {
                self.selected_client -= 1;
            }
            self.sync_from_selected_client();
            self.spawn_selected = 0;
        }
    }

    pub fn filtered_spawns(&self) -> Vec<&SpawnInfo> {
        self.spawns
            .iter()
            .filter(|s| {
                // Type filter
                match self.spawn_type_filter {
                    SpawnFilter::All => true,
                    SpawnFilter::Pc => s.spawn_type == SpawnType::Player,
                    SpawnFilter::Npc => s.spawn_type == SpawnType::Npc,
                    SpawnFilter::Named => {
                        s.spawn_type == SpawnType::Npc
                            && !s.displayed_name.starts_with("a ")
                            && !s.displayed_name.starts_with("an ")
                    }
                }
            })
            .filter(|s| {
                // Text search filter
                if self.spawn_filter.is_empty() {
                    return true;
                }
                let filter = self.spawn_filter.to_lowercase();
                s.displayed_name.to_lowercase().contains(&filter)
                    || s.class_str().to_lowercase().contains(&filter)
                    || s.spawn_type.to_string().to_lowercase().contains(&filter)
            })
            .collect()
    }

    pub fn cycle_spawn_filter(&mut self) {
        self.spawn_type_filter = self.spawn_type_filter.next();
        self.spawn_selected = 0;
        self.status_message = format!("Filter: {}", self.spawn_type_filter.label());
    }

    pub fn spawn_list_down(&mut self) {
        let max = self.filtered_spawns().len().saturating_sub(1);
        if self.spawn_selected < max {
            self.spawn_selected += 1;
        }
    }

    pub fn spawn_list_up(&mut self) {
        self.spawn_selected = self.spawn_selected.saturating_sub(1);
    }

    pub fn spawn_list_page_down(&mut self) {
        let max = self.filtered_spawns().len().saturating_sub(1);
        self.spawn_selected = (self.spawn_selected + 20).min(max);
    }

    pub fn spawn_list_page_up(&mut self) {
        self.spawn_selected = self.spawn_selected.saturating_sub(20);
    }

    pub fn hex_scroll_down(&mut self) {
        self.hex_address = self.hex_address.wrapping_add(0x100);
    }

    pub fn hex_scroll_up(&mut self) {
        self.hex_address = self.hex_address.wrapping_sub(0x100);
    }

    /// Set the hex dump to view a specific spawn's raw memory.
    pub fn inspect_selected_spawn(&mut self) {
        // Extract data from the borrow before mutating self
        let info: Option<(String, u32)> = {
            let filtered = self.filtered_spawns();
            filtered
                .get(self.spawn_selected)
                .map(|s| (s.displayed_name.clone(), s.spawn_id))
        };
        if let Some((name, id)) = info {
            self.hex_label = format!("Raw memory: {} (ID {})", name, id);
            self.status_message = format!("Inspecting: {}", name);
        }
    }

    pub fn clear_filter(&mut self) {
        self.spawn_filter.clear();
        self.search_mode = false;
        self.spawn_selected = 0;
    }

    pub fn toggle_privacy(&mut self) {
        self.privacy_mode = !self.privacy_mode;
        self.status_message = if self.privacy_mode {
            String::from("Privacy mode ON — names and server hidden")
        } else {
            String::from("Privacy mode OFF")
        };
    }

    /// Redact a name if it belongs to one of our connected characters.
    /// Returns the original name if privacy mode is off or it's not ours.
    pub fn redact_name<'a>(&self, name: &'a str) -> std::borrow::Cow<'a, str> {
        if !self.privacy_mode {
            return std::borrow::Cow::Borrowed(name);
        }
        for (i, client) in self.clients.iter().enumerate() {
            if let Some(player) = &client.local_player {
                if player.displayed_name == name || player.name == name {
                    return std::borrow::Cow::Owned(format!("Toon-{:02}", i + 1));
                }
            }
        }
        std::borrow::Cow::Borrowed(name)
    }

    /// Returns the server name, redacted if privacy mode is on.
    pub fn display_server(&self) -> &str {
        if self.privacy_mode {
            "[Hidden Server]"
        } else {
            &self.server_name
        }
    }

    pub fn toggle_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::SpawnList => ActivePanel::HexDump,
            ActivePanel::HexDump => ActivePanel::SpawnList,
        };
    }

    /// Load the zone map for the given zone short name from the map directory.
    pub fn load_zone_map(&mut self, zone_short_name: &str) {
        match crate::eq::map_parser::load_zone_map(&self.map_dir, zone_short_name) {
            Ok(map) if !map.lines.is_empty() => {
                tracing::info!(
                    zone = zone_short_name,
                    lines = map.lines.len(),
                    points = map.points.len(),
                    "Loaded zone map"
                );
                self.zone_map = Some(map);
            }
            Ok(_) => {
                tracing::debug!(zone = zone_short_name, "No map data found for zone");
                self.zone_map = None;
            }
            Err(e) => {
                tracing::warn!(zone = zone_short_name, error = %e, "Failed to load zone map");
                self.zone_map = None;
            }
        }
    }

    /// Execute the current command buffer content.
    pub fn execute_command(&mut self, orchestrator: &mut Orchestrator) {
        let input = self.command_buffer.trim().to_string();
        if input.is_empty() {
            return;
        }

        // Save to history
        self.command_history.push(input.clone());

        let parts: Vec<&str> = input.splitn(3, ' ').collect();
        match parts[0] {
            "camp" => {
                self.execute_camp_command(&parts[1..], orchestrator);
            }
            "status" => {
                let client_count = self.clients.len();
                self.status_message = format!("{} client(s) connected", client_count);
            }
            "inject" => {
                self.status_message = String::from("Inject requested (not yet wired)");
            }
            "all" => {
                if let Some(slash_cmd) = parts.get(1) {
                    let pids: Vec<u32> = self.clients.iter().map(|c| c.pid).collect();
                    let mut ok = 0usize;
                    let mut fail = 0usize;
                    for pid in &pids {
                        match send_slash_command(*pid, slash_cmd) {
                            Ok(()) => ok += 1,
                            Err(_) => fail += 1,
                        }
                    }
                    self.status_message = format!(
                        "all {} → sent to {}, failed {}",
                        slash_cmd, ok, fail
                    );
                } else {
                    self.status_message = String::from("Usage: all <slash command>");
                }
            }
            _ => {
                // Try to parse first token as PID
                if let Ok(pid) = parts[0].parse::<u32>() {
                    if let Some(slash_cmd) = parts.get(1) {
                        match send_slash_command(pid, slash_cmd) {
                            Ok(()) => {
                                self.status_message =
                                    format!("{} → {}", pid, slash_cmd);
                            }
                            Err(e) => {
                                self.status_message =
                                    format!("Error sending to {}: {}", pid, e);
                            }
                        }
                    } else {
                        self.status_message =
                            format!("Usage: {} <slash command>", pid);
                    }
                } else {
                    self.status_message = format!("Unknown command: {}", input);
                }
            }
        }
    }

    /// Handle `camp <subcommand>` from the command bar.
    fn execute_camp_command(&mut self, args: &[&str], orchestrator: &mut Orchestrator) {
        match args.first().copied() {
            Some("start") => {
                let camp_name = match args.get(1) {
                    Some(name) => *name,
                    None => {
                        self.status_message =
                            String::from("Usage: camp start <name>  (loads config/camps/<name>.toml)");
                        return;
                    }
                };

                match CampConfig::load(camp_name) {
                    Ok(config) => {
                        // Build members from currently connected clients
                        let members = self.build_camp_members();
                        if members.is_empty() {
                            self.status_message =
                                String::from("No clients connected — cannot start camp");
                            return;
                        }
                        let count = members.len();
                        orchestrator.start_camp(config, members);
                        self.status_message =
                            format!("Camp '{}' started with {} members", camp_name, count);
                    }
                    Err(e) => {
                        self.status_message =
                            format!("Failed to load camp '{}': {}", camp_name, e);
                    }
                }
            }
            Some("stop") => {
                orchestrator.stop_camp();
                self.status_message = String::from("Camp stopped");
            }
            Some("status") => {
                self.status_message = orchestrator.camp_status();
            }
            _ => {
                self.status_message =
                    String::from("Usage: camp <start|stop|status> [name]");
            }
        }
    }

    /// Build camp members from connected clients using simple role assignment.
    /// First client = Tank, second = Healer, third = Puller, rest = DPS.
    fn build_camp_members(&self) -> Vec<CampMember> {
        self.clients
            .iter()
            .enumerate()
            .map(|(i, client)| {
                let role = match i {
                    0 => Role::Tank,
                    1 => Role::Healer,
                    2 => Role::Puller,
                    _ => Role::DPS,
                };
                let name = if client.character_name.is_empty() {
                    format!("Client-{}", client.pid)
                } else {
                    client.character_name.clone()
                };
                CampMember {
                    pid: client.pid,
                    name,
                    role,
                }
            })
            .collect()
    }
}

/// Generate a PID-derived session token for IPC auth.
fn generate_session_token(pid: u32) -> [u8; 32] {
    let pid_bytes = pid.to_le_bytes();
    let mut token = [0u8; 32];
    for (i, byte) in token.iter_mut().enumerate() {
        *byte = pid_bytes[i % 4] ^ (i as u8);
    }
    token
}

/// Send a slash command to a specific PID via named pipe.
fn send_slash_command(pid: u32, command: &str) -> anyhow::Result<()> {
    use crate::ipc::pipe::CommandPipe;
    use dmft_common::ipc::Command;

    let pipe = CommandPipe::connect(pid)?;
    let token = generate_session_token(pid);
    pipe.send_raw_token(&token)?;
    pipe.send_async(&Command::SlashCommand {
        command: command.to_string(),
    })?;
    Ok(())
}
