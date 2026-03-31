use std::collections::HashMap;

use ratatui::widgets::TableState;

use super::app::{NavClientStatus, SpawnFilter};
use crate::eq::map_parser::ZoneMap;

// ─── Per-screen state sub-structs ────────────────────────────────────────────

/// State for the Spawns screen — selection, filtering, and search.
pub struct SpawnsScreenState {
    pub table_state: TableState,
    pub spawn_filter: String,
    pub spawn_type_filter: SpawnFilter,
    pub search_mode: bool,
}

impl SpawnsScreenState {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            table_state,
            spawn_filter: String::new(),
            spawn_type_filter: SpawnFilter::All,
            search_mode: false,
        }
    }
}

/// State for the hex dump viewer panel.
pub struct HexDumpState {
    pub hex_address: usize,
    pub hex_data: Vec<u8>,
    pub hex_label: String,
}

impl HexDumpState {
    pub fn new() -> Self {
        Self {
            hex_address: 0,
            hex_data: Vec::new(),
            hex_label: String::from("No address selected"),
        }
    }
}

/// State for the Map screen.
pub struct MapScreenState {
    pub zone_map: Option<ZoneMap>,
    pub map_dir: std::path::PathBuf,
    /// The zone short name currently loaded, used to avoid redundant reloads.
    pub loaded_zone: String,
    /// Z-depth filter range — spawns farther than this from the player's Z are hidden.
    pub z_filter_range: f32,
}

impl MapScreenState {
    pub fn new() -> Self {
        let map_dir = resolve_map_dir();
        Self {
            zone_map: None,
            map_dir,
            loaded_zone: String::new(),
            z_filter_range: 50.0,
        }
    }

    /// Increase Z filter range by 10 (max 500).
    pub fn increase_z_filter(&mut self) {
        self.z_filter_range = (self.z_filter_range + 10.0).min(500.0);
    }

    /// Decrease Z filter range by 10 (min 10).
    pub fn decrease_z_filter(&mut self) {
        self.z_filter_range = (self.z_filter_range - 10.0).max(10.0);
    }
}

/// Resolve the map directory to an absolute path.
/// Tries CWD-relative `config/maps` first, then falls back to exe-relative.
fn resolve_map_dir() -> std::path::PathBuf {
    let relative = std::path::PathBuf::from("config/maps");
    if relative.is_dir() {
        if let Ok(abs) = relative.canonicalize() {
            return abs;
        }
        return relative;
    }

    // Try relative to the executable
    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        let exe_relative = exe_dir.join("config/maps");
        if exe_relative.is_dir() {
            return exe_relative;
        }
    }

    tracing::warn!("Map directory 'config/maps' not found relative to CWD or executable");
    relative
}

/// State for the Navigation screen.
pub struct NavigationScreenState {
    pub nav_selected: usize,
    pub nav_statuses: HashMap<u32, NavClientStatus>,
}

impl NavigationScreenState {
    pub fn new() -> Self {
        Self {
            nav_selected: 0,
            nav_statuses: HashMap::new(),
        }
    }
}

/// State for the command bar (: mode).
pub struct CommandBarState {
    pub command_mode: bool,
    pub command_buffer: String,
    pub command_history: Vec<String>,
    pub command_history_idx: Option<usize>,
}

impl CommandBarState {
    pub fn new() -> Self {
        Self {
            command_mode: false,
            command_buffer: String::new(),
            command_history: Vec::new(),
            command_history_idx: None,
        }
    }
}
