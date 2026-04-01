use std::collections::HashMap;

use ratatui::widgets::TableState;

use super::app::{NavClientStatus, SpawnFilter};
use crate::eq::map_parser::ZoneMap;
use crate::nav::mesh::NavMeshOverlay;

// ─── Per-screen state sub-structs ────────────────────────────────────────────

/// State for the Spawns screen — selection, filtering, and search.
pub struct SpawnsScreenState {
    pub table_state: TableState,
    pub spawn_filter: String,
    pub spawn_type_filter: SpawnFilter,
    pub search_mode: bool,
}

impl SpawnsScreenState {
    #[must_use]
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
    #[must_use]
    pub fn new() -> Self {
        Self {
            hex_address: 0,
            hex_data: Vec::new(),
            hex_label: String::from("No address selected"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapViewportMode {
    Auto,
    Local,
    Global,
}

impl MapViewportMode {
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Local,
            Self::Local => Self::Global,
            Self::Global => Self::Auto,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Local => "local",
            Self::Global => "global",
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
    pub viewport_mode: MapViewportMode,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub show_navmesh: bool,
    pub navmesh_overlay: Option<NavMeshOverlay>,
}

impl MapScreenState {
    #[must_use]
    pub fn new() -> Self {
        let map_dir = resolve_map_dir();
        Self {
            zone_map: None,
            map_dir,
            loaded_zone: String::new(),
            z_filter_range: 50.0,
            viewport_mode: MapViewportMode::Auto,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            show_navmesh: true,
            navmesh_overlay: None,
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

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.25).min(4.0);
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.25).max(0.35);
    }

    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        self.pan_x += delta_x;
        self.pan_y += delta_y;
    }

    pub fn reset_viewport(&mut self) {
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
    }

    pub fn cycle_viewport_mode(&mut self) -> MapViewportMode {
        self.viewport_mode = self.viewport_mode.next();
        self.reset_viewport();
        self.viewport_mode
    }

    pub fn toggle_navmesh(&mut self) -> bool {
        self.show_navmesh = !self.show_navmesh;
        self.show_navmesh
    }
}

/// State for the composite Overview screen.
pub struct OverviewScreenState {
    pub show_groups: bool,
    pub show_filters: bool,
    pub character_collapsed: bool,
    pub groups_collapsed: bool,
    pub filters_collapsed: bool,
    pub combat_collapsed: bool,
    pub session_collapsed: bool,
}

impl OverviewScreenState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            show_groups: true,
            show_filters: true,
            character_collapsed: false,
            groups_collapsed: false,
            filters_collapsed: false,
            combat_collapsed: false,
            session_collapsed: false,
        }
    }
}

/// State for the composite Tactical screen.
pub struct TacticalScreenState {
    pub map_maximized: bool,
    pub show_named: bool,
    pub show_navigation: bool,
    pub named_collapsed: bool,
    pub navigation_collapsed: bool,
}

impl TacticalScreenState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            map_maximized: false,
            show_named: true,
            show_navigation: true,
            named_collapsed: false,
            navigation_collapsed: false,
        }
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
    #[must_use]
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
    /// Command usage frequency — tracks how often each command is used.
    pub command_frequency: HashMap<String, u32>,
    /// Cached top-N favorites (recalculated on each command execution).
    pub favorites: Vec<String>,
}

impl CommandBarState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            command_mode: false,
            command_buffer: String::new(),
            command_history: Vec::new(),
            command_history_idx: None,
            command_frequency: HashMap::new(),
            favorites: Vec::new(),
        }
    }

    /// Record a command execution and update favorites.
    pub fn record_command(&mut self, cmd: &str) {
        let normalized = Self::normalize_command(cmd);
        if normalized.is_empty() {
            return;
        }
        *self.command_frequency.entry(normalized).or_insert(0) += 1;
        self.recalculate_favorites();
    }

    /// Normalize a command for frequency tracking.
    /// Strips arguments for grouping: "ma Warrior" → "ma", "G1 /sit" → "G1 /sit"
    /// but preserves slash commands: "all /sit" → "all /sit"
    fn normalize_command(cmd: &str) -> String {
        let trimmed = cmd.trim();
        let parts: Vec<&str> = trimmed.splitn(3, ' ').collect();

        // Determine how many tokens to keep based on the command keyword.
        let token_count = match parts.first().copied() {
            // Group broadcast + slash: "all /sit", "G1 /follow" → keep 2
            // Single meaningful arg: "ma Warrior", "mt Tank" → keep 2
            Some(
                "all" | "G1" | "G2" | "G3" | "G4" | "G5" | "G6" | "ma" | "mt" | "mode" | "login"
                | "nav" | "track" | "ch",
            ) => 2,
            Some("engage") => 1,
            // Camp subcommands: "camp start permafrost" → keep all 3
            Some("camp") => 3,
            // Everything else: just the base command
            _ => return trimmed.to_string(),
        };

        parts[..parts.len().min(token_count)].join(" ")
    }

    /// Recalculate the top-9 favorites from frequency data.
    fn recalculate_favorites(&mut self) {
        let mut freq_list: Vec<(String, u32)> = self
            .command_frequency
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        freq_list.sort_by_key(|item| std::cmp::Reverse(item.1));
        self.favorites = freq_list.into_iter().take(9).map(|(cmd, _)| cmd).collect();
    }

    /// Get the favorite command at index (0-based, for F1=0, F2=1, etc.).
    pub fn get_favorite(&self, idx: usize) -> Option<&str> {
        self.favorites.get(idx).map(std::string::String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_frequency_tracking() {
        let mut state = CommandBarState::new();
        state.record_command("ma Warrior");
        state.record_command("ma Warrior");
        state.record_command("engage 100");
        assert_eq!(state.command_frequency.get("ma Warrior"), Some(&2));
        assert_eq!(state.command_frequency.get("engage"), Some(&1));
    }

    #[test]
    fn favorites_sorted_by_frequency() {
        let mut state = CommandBarState::new();
        for _ in 0..5 {
            state.record_command("ma Warrior");
        }
        for _ in 0..3 {
            state.record_command("all /sit");
        }
        for _ in 0..1 {
            state.record_command("status");
        }
        assert_eq!(state.favorites[0], "ma Warrior");
        assert_eq!(state.favorites[1], "all /sit");
        assert_eq!(state.favorites[2], "status");
    }

    #[test]
    fn favorites_max_nine() {
        let mut state = CommandBarState::new();
        for i in 0..15 {
            state.record_command(&format!("cmd{}", i));
        }
        assert_eq!(state.favorites.len(), 9);
    }

    #[test]
    fn get_favorite_returns_none_for_empty() {
        let state = CommandBarState::new();
        assert!(state.get_favorite(0).is_none());
    }

    #[test]
    fn normalize_preserves_group_slash_commands() {
        assert_eq!(CommandBarState::normalize_command("all /sit"), "all /sit");
        assert_eq!(
            CommandBarState::normalize_command("G1 /follow"),
            "G1 /follow"
        );
    }

    #[test]
    fn normalize_preserves_camp_with_name() {
        assert_eq!(
            CommandBarState::normalize_command("camp start permafrost"),
            "camp start permafrost"
        );
    }

    #[test]
    fn normalize_command_with_target() {
        assert_eq!(
            CommandBarState::normalize_command("ma Warrior"),
            "ma Warrior"
        );
        assert_eq!(CommandBarState::normalize_command("engage 100"), "engage");
    }

    #[test]
    fn map_viewport_mode_cycles() {
        let mut state = MapScreenState::new();
        assert_eq!(state.viewport_mode, MapViewportMode::Auto);
        assert_eq!(state.cycle_viewport_mode(), MapViewportMode::Local);
        assert_eq!(state.cycle_viewport_mode(), MapViewportMode::Global);
        assert_eq!(state.cycle_viewport_mode(), MapViewportMode::Auto);
    }

    #[test]
    fn map_zoom_is_clamped() {
        let mut state = MapScreenState::new();
        for _ in 0..20 {
            state.zoom_in();
        }
        assert!((state.zoom - 4.0).abs() < f32::EPSILON);

        for _ in 0..40 {
            state.zoom_out();
        }
        assert!((state.zoom - 0.35).abs() < f32::EPSILON);
    }

    #[test]
    fn map_reset_viewport_clears_pan_and_zoom() {
        let mut state = MapScreenState::new();
        state.zoom_in();
        state.pan(42.0, -18.0);
        state.reset_viewport();
        assert!((state.zoom - 1.0).abs() < f32::EPSILON);
        assert!((state.pan_x - 0.0).abs() < f32::EPSILON);
        assert!((state.pan_y - 0.0).abs() < f32::EPSILON);
    }
}
