use crate::eq::structs::SpawnInfo;
use crate::soul::coordinator::SoulCoordinator;

/// Which panel is currently focused for keyboard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    SpawnList,
    HexDump,
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
            client_status: format!("Attached to PID {}", pid),
        }
    }
}

/// Application state for the TUI debugger.
pub struct App {
    pub running: bool,
    pub active_panel: ActivePanel,

    // Multi-client state
    pub clients: Vec<ClientState>,
    pub selected_client: usize,

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
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            active_panel: ActivePanel::SpawnList,

            clients: Vec::new(),
            selected_client: 0,

            server_name: String::from("Firiona Vie"),

            local_player: None,
            target: None,
            spawns: Vec::new(),
            status_message: String::from("Waiting for EQ process..."),
            tick_count: 0,

            spawn_scroll: 0,
            spawn_selected: 0,
            spawn_filter: String::new(),

            hex_address: 0,
            hex_data: Vec::new(),
            hex_label: String::from("No address selected"),

            refresh_rate_ms: 250,

            eq_base: 0,
            attached_pid: None,

            soul_coordinator: None,
            soul_tick_counter: 0,
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
        if self.spawn_filter.is_empty() {
            self.spawns.iter().collect()
        } else {
            let filter = self.spawn_filter.to_lowercase();
            self.spawns
                .iter()
                .filter(|s| {
                    s.displayed_name.to_lowercase().contains(&filter)
                        || s.class_str().to_lowercase().contains(&filter)
                        || s.spawn_type.to_string().to_lowercase().contains(&filter)
                })
                .collect()
        }
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
        self.spawn_selected = 0;
    }

    pub fn toggle_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::SpawnList => ActivePanel::HexDump,
            ActivePanel::HexDump => ActivePanel::SpawnList,
        };
    }
}
