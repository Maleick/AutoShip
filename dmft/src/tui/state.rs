use std::collections::HashMap;

use ratatui::style::Color;
use ratatui::widgets::TableState;

use super::app::{NavClientStatus, SpawnFilter};
use super::theme::ThemeKind;
use crate::eq::map_parser::ZoneMap;
use crate::nav::mesh::NavMeshOverlay;

// ─── Per-screen state sub-structs ────────────────────────────────────────────

/// State for the Spawns screen — selection, filtering, and search.
pub struct SpawnsScreenState {
    /// Ratatui table widget state (tracks selected row and scroll offset).
    pub table_state: TableState,
    /// Text search filter string for spawn names.
    pub spawn_filter: String,
    /// Active spawn type filter (All, PC, NPC, Named).
    pub spawn_type_filter: SpawnFilter,
    /// Whether the user is currently typing a search query.
    pub search_mode: bool,
}

impl SpawnsScreenState {
    /// Creates a new spawn screen state with default filter settings.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilteredSpawnCacheKey {
    pub client_pid: Option<u32>,
    pub spawn_revision: u64,
    pub spawn_filter: String,
    pub spawn_type_filter: SpawnFilter,
}

#[derive(Default)]
pub(crate) struct FilteredSpawnCache {
    pub key: Option<FilteredSpawnCacheKey>,
    pub indices: Vec<usize>,
}

impl FilteredSpawnCache {
    pub fn clear(&mut self) {
        self.key = None;
        self.indices.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MapSpawnPresentationKey {
    pub client_pid: Option<u32>,
    pub spawn_revision: u64,
    pub selected_spawn_id: Option<u32>,
    pub width: u16,
    pub height: u16,
    pub z_filter_bits: u32,
    pub player_z_bits: Option<u32>,
    pub show_spawns: bool,
    pub theme_kind: ThemeKind,
    pub center_x_bits: u32,
    pub center_y_bits: u32,
    pub scale_bits: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MapSpawnPresentationCell {
    pub row: u16,
    pub col: u16,
    pub ch: char,
    pub color: Color,
}

#[derive(Default)]
pub(crate) struct MapSpawnPresentationCache {
    pub key: Option<MapSpawnPresentationKey>,
    pub cells: Vec<MapSpawnPresentationCell>,
    pub selected_spawn: Option<(f32, f32, u32)>,
}

impl MapSpawnPresentationCache {
    pub fn clear(&mut self) {
        self.key = None;
        self.cells.clear();
        self.selected_spawn = None;
    }
}

/// State for the hex dump viewer panel.
pub struct HexDumpState {
    /// Base address for the hex dump display.
    pub hex_address: usize,
    /// Raw bytes to display in the hex viewer.
    pub hex_data: Vec<u8>,
    /// Label shown above the hex dump (e.g., spawn name).
    pub hex_label: String,
}

impl HexDumpState {
    /// Creates a new hex dump state with no data loaded.
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
    /// Parsed zone map data (lines and points), if loaded.
    pub zone_map: Option<ZoneMap>,
    /// Directory path where map files are stored.
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
    /// Map layer visibility flags.
    pub show_geometry: bool,
    pub show_spawns: bool,
    pub show_nav_paths: bool,
    pub show_labels: bool,
}

impl MapScreenState {
    /// Creates a new map state, resolving the map directory path.
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
            show_geometry: true,
            show_spawns: true,
            show_nav_paths: true,
            show_labels: true,
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

    /// Toggle a map layer by number:
    /// 1=geometry, 2=spawns, 3=nav paths, 4=mesh, 5=labels.
    pub fn toggle_layer(&mut self, layer: u8) -> &'static str {
        match layer {
            1 => {
                self.show_geometry = !self.show_geometry;
                if self.show_geometry {
                    "Geometry ON"
                } else {
                    "Geometry OFF"
                }
            }
            2 => {
                self.show_spawns = !self.show_spawns;
                if self.show_spawns {
                    "Spawns ON"
                } else {
                    "Spawns OFF"
                }
            }
            3 => {
                self.show_nav_paths = !self.show_nav_paths;
                if self.show_nav_paths {
                    "Nav paths ON"
                } else {
                    "Nav paths OFF"
                }
            }
            4 => {
                self.show_navmesh = !self.show_navmesh;
                if self.show_navmesh {
                    "Navmesh ON"
                } else {
                    "Navmesh OFF"
                }
            }
            5 => {
                self.show_labels = !self.show_labels;
                if self.show_labels {
                    "Labels ON"
                } else {
                    "Labels OFF"
                }
            }
            _ => "Unknown layer",
        }
    }
}

/// State for the composite Overview screen.
pub struct OverviewScreenState {
    /// Whether the groups panel is visible.
    pub show_groups: bool,
    /// Whether the filters/scope panel is visible.
    pub show_filters: bool,
    /// Whether the character detail panel is collapsed.
    pub character_collapsed: bool,
    /// Whether the groups panel is collapsed.
    pub groups_collapsed: bool,
    /// Whether the filters panel is collapsed.
    pub filters_collapsed: bool,
    /// Whether the combat panel is collapsed.
    pub combat_collapsed: bool,
    /// Whether the session stats panel is collapsed.
    pub session_collapsed: bool,
}

impl OverviewScreenState {
    /// Creates a new overview state with all panels visible and expanded.
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
    /// Whether the map is in full-screen maximized mode.
    pub map_maximized: bool,
    /// Whether the named mob tracker panel is visible.
    pub show_named: bool,
    /// Whether the navigation panel is visible.
    pub show_navigation: bool,
    /// Whether the named panel is collapsed.
    pub named_collapsed: bool,
    /// Whether the navigation panel is collapsed.
    pub navigation_collapsed: bool,
}

impl TacticalScreenState {
    /// Creates a new tactical state with side panels visible.
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
    /// Currently selected navigation entry index.
    pub nav_selected: usize,
    /// Per-client navigation statuses keyed by PID.
    pub nav_statuses: HashMap<u32, NavClientStatus>,
}

impl NavigationScreenState {
    /// Creates a new navigation state with no active statuses.
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
    /// Whether the command bar is active (user is typing).
    pub command_mode: bool,
    /// Current text in the command input buffer.
    pub command_buffer: String,
    /// Cursor position within the input buffer, measured in chars.
    pub cursor: usize,
    /// History of previously executed commands.
    pub command_history: Vec<String>,
    /// Index into command history for up/down navigation.
    pub command_history_idx: Option<usize>,
    /// Draft preserved before the user starts browsing history.
    pub history_draft: Option<String>,
    /// Command usage frequency — tracks how often each command is used.
    pub command_frequency: HashMap<String, u32>,
    /// Cached top-N favorites (recalculated on each command execution).
    pub favorites: Vec<String>,
}

impl CommandBarState {
    const HISTORY_FILE: &'static str = "config/.dmft_command_history";

    /// Creates a new command bar state with empty buffer and history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            command_mode: false,
            command_buffer: String::new(),
            cursor: 0,
            command_history: Vec::new(),
            command_history_idx: None,
            history_draft: None,
            command_frequency: HashMap::new(),
            favorites: Vec::new(),
        }
    }

    /// Enter command mode, optionally with a prefilled command.
    pub fn enter(&mut self, prefill: Option<&str>) {
        self.command_mode = true;
        self.command_history_idx = None;
        self.history_draft = None;
        self.command_buffer = prefill.unwrap_or_default().to_string();
        self.cursor = self.command_buffer.chars().count();
    }

    /// Exit command mode and clear transient editor state.
    pub fn exit(&mut self) {
        self.command_mode = false;
        self.command_history_idx = None;
        self.history_draft = None;
        self.command_buffer.clear();
        self.cursor = 0;
    }

    /// Replace the full command buffer.
    pub fn set_buffer(&mut self, value: impl Into<String>) {
        self.command_buffer = value.into();
        self.cursor = self.command_buffer.chars().count();
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, ch: char) {
        let mut chars: Vec<char> = self.command_buffer.chars().collect();
        let idx = self.cursor.min(chars.len());
        chars.insert(idx, ch);
        self.command_buffer = chars.into_iter().collect();
        self.cursor = idx + 1;
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut chars: Vec<char> = self.command_buffer.chars().collect();
        let idx = self.cursor.min(chars.len()) - 1;
        chars.remove(idx);
        self.command_buffer = chars.into_iter().collect();
        self.cursor = idx;
    }

    /// Delete the character under the cursor.
    pub fn delete(&mut self) {
        let mut chars: Vec<char> = self.command_buffer.chars().collect();
        let idx = self.cursor.min(chars.len());
        if idx >= chars.len() {
            return;
        }
        chars.remove(idx);
        self.command_buffer = chars.into_iter().collect();
        self.cursor = idx.min(self.command_buffer.chars().count());
    }

    /// Move the cursor left by one character.
    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Move the cursor right by one character.
    pub fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.command_buffer.chars().count());
    }

    /// Move the cursor to the start of the buffer.
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move the cursor to the end of the buffer.
    pub fn move_end(&mut self) {
        self.cursor = self.command_buffer.chars().count();
    }

    /// Browse to the previous command in history.
    pub fn history_prev(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        if self.command_history_idx.is_none() {
            self.history_draft = Some(self.command_buffer.clone());
        }

        let idx = match self.command_history_idx {
            Some(idx) => idx.saturating_sub(1),
            None => self.command_history.len() - 1,
        };
        self.command_history_idx = Some(idx);
        self.set_buffer(self.command_history[idx].clone());
    }

    /// Browse to the next command in history, restoring the in-progress draft at the end.
    pub fn history_next(&mut self) {
        match self.command_history_idx {
            Some(idx) if idx + 1 < self.command_history.len() => {
                let next = idx + 1;
                self.command_history_idx = Some(next);
                self.set_buffer(self.command_history[next].clone());
            }
            Some(_) => {
                self.command_history_idx = None;
                let draft = self.history_draft.take().unwrap_or_default();
                self.set_buffer(draft);
            }
            None => {}
        }
    }

    /// Load command history from disk and rebuild frequency favorites.
    pub fn load_history_from_disk(&mut self) {
        let Ok(content) = std::fs::read_to_string(Self::HISTORY_FILE) else {
            return;
        };
        self.command_history = content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        self.command_frequency.clear();
        for cmd in &self.command_history {
            let normalized = Self::normalize_command(cmd);
            if !normalized.is_empty() {
                *self.command_frequency.entry(normalized).or_insert(0) += 1;
            }
        }
        self.recalculate_favorites();
    }

    /// Persist command history to disk (best-effort).
    pub fn save_history_to_disk(&self) -> std::io::Result<()> {
        let mut payload = self.command_history.join("\n");
        if !payload.is_empty() {
            payload.push('\n');
        }
        std::fs::write(Self::HISTORY_FILE, payload)
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
    fn command_editor_insert_delete_and_cursor_motion() {
        let mut state = CommandBarState::new();
        state.enter(Some("mode hunt"));
        state.move_left();
        state.move_left();
        state.insert_char('e');
        assert_eq!(state.command_buffer, "mode huent");
        state.backspace();
        assert_eq!(state.command_buffer, "mode hunt");
        state.move_home();
        state.delete();
        assert_eq!(state.command_buffer, "ode hunt");
        state.move_end();
        assert_eq!(state.cursor, state.command_buffer.chars().count());
    }

    #[test]
    fn history_navigation_restores_draft() {
        let mut state = CommandBarState::new();
        state.command_history = vec![
            String::from("mode hunt"),
            String::from("nav gfay"),
            String::from("track list"),
        ];
        state.enter(Some("ma Warrior"));

        state.history_prev();
        assert_eq!(state.command_buffer, "track list");
        state.history_prev();
        assert_eq!(state.command_buffer, "nav gfay");
        state.history_next();
        assert_eq!(state.command_buffer, "track list");
        state.history_next();
        assert_eq!(state.command_buffer, "ma Warrior");
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
