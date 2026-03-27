use crate::eq::structs::SpawnInfo;
use crate::soul::coordinator::SoulCoordinator;

/// Which panel is currently focused for keyboard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    SpawnList,
    HexDump,
}

/// Application state for the TUI debugger.
pub struct App {
    pub running: bool,
    pub active_panel: ActivePanel,

    // Data
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

    // EQ connection info
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
            filtered.get(self.spawn_selected)
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
