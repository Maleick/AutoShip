use std::collections::HashMap;

use ratatui::style::Color;
use ratatui::widgets::TableState;

use super::app::{NavClientStatus, SpawnFilter};
use super::theme::ThemeKind;
use crate::eq::map_parser::ZoneMap;
use crate::eq::named_tracker;
use crate::eq::structs::{SpawnInfo, SpawnType};
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
    pub map_filter_bits: u8,
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

/// A single struct field annotation for the hex dump overlay.
#[derive(Debug, Clone)]
pub struct FieldAnnotation {
    /// Byte offset from the start of the hex data.
    pub offset: usize,
    /// Size of the field in bytes.
    pub size: usize,
    /// Human-readable field name.
    pub name: String,
    /// Color index (cycled for visual distinction between adjacent fields).
    pub color_idx: u8,
}

/// State for the hex dump viewer panel.
pub struct HexDumpState {
    /// Base address for the hex dump display.
    pub hex_address: usize,
    /// Raw bytes to display in the hex viewer.
    pub hex_data: Vec<u8>,
    /// Label shown above the hex dump (e.g., spawn name).
    pub hex_label: String,
    /// Whether struct field annotations are shown.
    pub show_annotations: bool,
    /// Known field annotations for the current hex data context.
    pub annotations: Vec<FieldAnnotation>,
}

impl HexDumpState {
    /// Creates a new hex dump state with no data loaded.
    #[must_use]
    pub fn new() -> Self {
        Self {
            hex_address: 0,
            hex_data: Vec::new(),
            hex_label: String::from("No address selected"),
            show_annotations: false,
            annotations: Vec::new(),
        }
    }

    /// Look up the annotation covering a given byte offset, if any.
    #[must_use]
    pub fn annotation_at(&self, offset: usize) -> Option<&FieldAnnotation> {
        self.annotations
            .iter()
            .find(|a| offset >= a.offset && offset < a.offset + a.size)
    }

    /// Build annotations from PlayerBase struct field offsets.
    ///
    /// These annotations apply when viewing raw spawn memory (base at struct start).
    pub fn load_player_base_annotations(&mut self) {
        use textquest_common::offsets::player_base;

        let fields: Vec<(&str, usize, usize)> = vec![
            ("prev", player_base::PREV, 8),
            ("next", player_base::NEXT, 8),
            ("lastName", player_base::LASTNAME, 64),
            ("y", player_base::Y, 4),
            ("x", player_base::X, 4),
            ("z", player_base::Z, 4),
            ("speedCurrent", player_base::SPEED_CURRENT, 4),
            ("speedZ", player_base::SPEED_Z, 4),
            ("speedRun", player_base::SPEED_RUN, 4),
            ("heading", player_base::HEADING, 4),
            ("speedHeading", player_base::SPEED_HEADING, 4),
            ("name", player_base::NAME, 64),
            ("displayedName", player_base::DISPLAYED_NAME, 64),
            ("type", player_base::TYPE, 1),
            ("spawnId", player_base::SPAWN_ID, 4),
        ];

        self.annotations = fields
            .into_iter()
            .enumerate()
            .map(|(i, (name, offset, size))| FieldAnnotation {
                offset,
                size,
                name: name.to_string(),
                color_idx: (i % 6) as u8,
            })
            .collect();

        // Sort by offset for consistent display.
        self.annotations.sort_by_key(|a| a.offset);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapFilterKind {
    Npc,
    Pc,
    Corpse,
    Ground,
    Pet,
    Named,
    Untargetable,
}

impl MapFilterKind {
    pub fn parse_kind(input: &str) -> Option<Self> {
        match input.to_ascii_lowercase().as_str() {
            "npc" => Some(Self::Npc),
            "pc" => Some(Self::Pc),
            "corpse" | "corpses" => Some(Self::Corpse),
            "ground" => Some(Self::Ground),
            "pet" | "pets" => Some(Self::Pet),
            "named" | "nameds" => Some(Self::Named),
            "untargetable" | "untargetables" | "untarget" => Some(Self::Untargetable),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Npc => "NPC",
            Self::Pc => "PC",
            Self::Corpse => "Corpse",
            Self::Ground => "Ground",
            Self::Pet => "Pet",
            Self::Named => "Named",
            Self::Untargetable => "Untargetable",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MapFilters {
    pub show_npc: bool,
    pub show_pc: bool,
    pub show_corpse: bool,
    pub show_ground: bool,
    pub show_pet: bool,
    pub show_named: bool,
    pub show_untargetable: bool,
}

impl Default for MapFilters {
    fn default() -> Self {
        Self {
            show_npc: true,
            show_pc: true,
            show_corpse: true,
            show_ground: true,
            show_pet: true,
            show_named: true,
            show_untargetable: true,
        }
    }
}

impl MapFilters {
    #[must_use]
    pub fn toggle(&mut self, kind: MapFilterKind) -> bool {
        let current = self.get(kind);
        self.set(kind, !current);
        !current
    }

    pub fn set(&mut self, kind: MapFilterKind, enabled: bool) {
        match kind {
            MapFilterKind::Npc => self.show_npc = enabled,
            MapFilterKind::Pc => self.show_pc = enabled,
            MapFilterKind::Corpse => self.show_corpse = enabled,
            MapFilterKind::Ground => self.show_ground = enabled,
            MapFilterKind::Pet => self.show_pet = enabled,
            MapFilterKind::Named => self.show_named = enabled,
            MapFilterKind::Untargetable => self.show_untargetable = enabled,
        }
    }

    #[must_use]
    pub fn get(&self, kind: MapFilterKind) -> bool {
        match kind {
            MapFilterKind::Npc => self.show_npc,
            MapFilterKind::Pc => self.show_pc,
            MapFilterKind::Corpse => self.show_corpse,
            MapFilterKind::Ground => self.show_ground,
            MapFilterKind::Pet => self.show_pet,
            MapFilterKind::Named => self.show_named,
            MapFilterKind::Untargetable => self.show_untargetable,
        }
    }

    pub fn set_all(&mut self, enabled: bool) {
        self.show_npc = enabled;
        self.show_pc = enabled;
        self.show_corpse = enabled;
        self.show_ground = enabled;
        self.show_pet = enabled;
        self.show_named = enabled;
        self.show_untargetable = enabled;
    }

    #[must_use]
    pub fn cache_key_bits(&self) -> u8 {
        let mut bits = 0u8;
        bits |= u8::from(self.show_npc);
        bits |= u8::from(self.show_pc) << 1;
        bits |= u8::from(self.show_corpse) << 2;
        bits |= u8::from(self.show_ground) << 3;
        bits |= u8::from(self.show_pet) << 4;
        bits |= u8::from(self.show_named) << 5;
        bits |= u8::from(self.show_untargetable) << 6;
        bits
    }

    #[must_use]
    pub fn allows_spawn(&self, spawn: &SpawnInfo) -> bool {
        let is_pc = spawn.spawn_type == SpawnType::Player;
        let is_corpse = spawn.spawn_type == SpawnType::Corpse;
        let is_npc = matches!(spawn.spawn_type, SpawnType::Npc);
        let is_pet = is_npc && Self::looks_like_pet(spawn);
        let is_named = is_npc && named_tracker::is_named(&spawn.displayed_name);
        let is_untargetable = matches!(spawn.spawn_type, SpawnType::Unknown(_));

        if is_pc && !self.show_pc {
            return false;
        }
        if is_corpse && !self.show_corpse {
            return false;
        }
        if is_pet && !self.show_pet {
            return false;
        }
        if is_named && !self.show_named {
            return false;
        }
        if is_untargetable && !self.show_untargetable {
            return false;
        }
        if is_npc && !self.show_npc {
            return false;
        }
        if Self::looks_like_ground(spawn) && !self.show_ground {
            return false;
        }

        true
    }

    #[must_use]
    pub fn summary(&self) -> String {
        let mut off: Vec<&'static str> = Vec::new();
        if !self.show_npc {
            off.push("NPC");
        }
        if !self.show_pc {
            off.push("PC");
        }
        if !self.show_corpse {
            off.push("Corpse");
        }
        if !self.show_ground {
            off.push("Ground");
        }
        if !self.show_pet {
            off.push("Pet");
        }
        if !self.show_named {
            off.push("Named");
        }
        if !self.show_untargetable {
            off.push("Untargetable");
        }

        if off.is_empty() {
            String::from("Map filters: all ON")
        } else {
            format!("Map filters off: {}", off.join(", "))
        }
    }

    #[must_use]
    pub fn inline_flags(&self) -> String {
        let mut out = String::from("filters[");
        out.push_str(if self.show_npc { "Npc" } else { "-" });
        out.push('/');
        out.push_str(if self.show_pc { "Pc" } else { "-" });
        out.push('/');
        out.push_str(if self.show_corpse { "Corp" } else { "-" });
        out.push('/');
        out.push_str(if self.show_ground { "Gnd" } else { "-" });
        out.push('/');
        out.push_str(if self.show_pet { "Pet" } else { "-" });
        out.push('/');
        out.push_str(if self.show_named { "Nm" } else { "-" });
        out.push('/');
        out.push_str(if self.show_untargetable { "Unt" } else { "-" });
        out.push(']');
        out
    }

    fn looks_like_pet(spawn: &SpawnInfo) -> bool {
        if matches!(spawn.spawn_type, SpawnType::Player | SpawnType::Corpse) {
            return false;
        }
        let lower = spawn.displayed_name.to_ascii_lowercase();
        lower.contains("`s pet")
            || lower.contains("'s pet")
            || lower.contains("`s warder")
            || lower.contains("'s warder")
            || lower.contains(" warder of ")
            || lower.starts_with("pet of ")
    }

    fn looks_like_ground(spawn: &SpawnInfo) -> bool {
        // Ground spawns are not currently emitted by the reader; keep a hook
        // for future support without filtering anything today.
        let _ = spawn;
        false
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
    /// Show layer-2 annotations (compass roses, grid overlays).
    pub show_annotations: bool,
    /// MQ2Map-style visibility toggles for map overlay entities.
    pub filters: MapFilters,
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
            show_labels: false,
            show_annotations: false,
            filters: MapFilters::default(),
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
            6 => {
                self.show_annotations = !self.show_annotations;
                if self.show_annotations {
                    "Annotations ON"
                } else {
                    "Annotations OFF"
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
    /// Whether the slot-profile panel is visible.
    pub show_profile: bool,
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
    /// Whether the slot-profile panel is collapsed.
    pub profile_collapsed: bool,
    /// Whether the priorities panel is collapsed.
    pub priorities_collapsed: bool,
}

impl OverviewScreenState {
    /// Creates a new overview state with all panels visible and expanded.
    #[must_use]
    pub fn new() -> Self {
        Self {
            show_groups: true,
            show_filters: true,
            show_profile: true,
            character_collapsed: false,
            groups_collapsed: false,
            filters_collapsed: false,
            combat_collapsed: false,
            session_collapsed: false,
            profile_collapsed: false,
            priorities_collapsed: false,
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
    const HISTORY_FILE: &'static str = "config/.textquest_command_history";

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

// ─── Explorer state ─────────────────────────────────────────────────────────

/// Category filter for the offset explorer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExplorerCategory {
    #[default]
    All,
    Functions,
    AaAbilities,
    Opcodes,
    UiWidgets,
}

impl ExplorerCategory {
    /// Human-readable label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Functions => "Functions",
            Self::AaAbilities => "AA",
            Self::Opcodes => "Opcodes",
            Self::UiWidgets => "UI",
        }
    }

    /// Cycle to the next category.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Functions,
            Self::Functions => Self::AaAbilities,
            Self::AaAbilities => Self::Opcodes,
            Self::Opcodes => Self::UiWidgets,
            Self::UiWidgets => Self::All,
        }
    }
}

/// A single explorer entry (function/offset from the Ghidra DB).
#[derive(Debug, Clone)]
pub struct ExplorerEntry {
    pub address: u64,
    pub name: String,
    pub category: Option<String>,
    pub usability: Option<String>,
    pub size: Option<u64>,
}

/// State for the Ghidra offset explorer panel.
pub struct ExplorerScreenState {
    /// Ratatui table state for scroll/selection.
    pub table_state: TableState,
    /// Text search filter.
    pub search_filter: String,
    /// Whether in search input mode.
    pub search_mode: bool,
    /// Active category filter.
    pub category_filter: ExplorerCategory,
    /// Filtered function list (rebuilt on filter change).
    pub filtered_functions: Vec<ExplorerEntry>,
    /// Total count before filtering (for display).
    pub total_count: usize,
}

impl ExplorerScreenState {
    #[must_use]
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            table_state,
            search_filter: String::new(),
            search_mode: false,
            category_filter: ExplorerCategory::All,
            filtered_functions: Vec::new(),
            total_count: 0,
        }
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        let i = self.table_state.selected().unwrap_or(0).saturating_sub(1);
        self.table_state.select(Some(i));
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        let max = self.filtered_functions.len().saturating_sub(1);
        let i = self.table_state.selected().unwrap_or(0);
        self.table_state.select(Some((i + 1).min(max)));
    }

    /// Get the currently selected entry's address (for hex view linking).
    #[must_use]
    pub fn selected_address(&self) -> Option<u64> {
        let idx = self.table_state.selected()?;
        self.filtered_functions.get(idx).map(|e| e.address)
    }
}

// ─── Packet Monitor state ───────────────────────────────────────────────────

/// A single captured network packet record for the TUI packet monitor.
#[derive(Clone)]
pub struct PacketRecord {
    /// PID of the client that captured the packet.
    pub client_id: u32,
    /// EQ protocol opcode identifier.
    pub opcode: u16,
    /// Whether the packet was inbound or outbound.
    pub direction: textquest_common::ipc::PacketDirection,
    /// Timestamp in milliseconds (from DLL).
    pub timestamp_ms: u64,
    /// Size of the packet payload in bytes.
    pub payload_size: u32,
}

/// State for the packet/opcode monitor panel.
pub struct PacketMonitorState {
    /// Ring buffer of captured packets (newest at the end).
    pub packets: Vec<PacketRecord>,
    /// Maximum number of packets to retain.
    pub capacity: usize,
    /// Whether the view auto-scrolls to follow new packets.
    pub auto_scroll: bool,
    /// Scroll offset from the bottom (0 = latest).
    pub scroll_offset: usize,
    /// Optional opcode filter — when set, only show packets matching this opcode.
    pub filter_opcode: Option<u16>,
    /// Optional direction filter.
    pub filter_direction: Option<textquest_common::ipc::PacketDirection>,
    /// Whether the panel is paused (stops consuming new packets into view).
    pub paused: bool,
}

impl PacketMonitorState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            packets: Vec::with_capacity(1024),
            capacity: 10_000,
            auto_scroll: true,
            scroll_offset: 0,
            filter_opcode: None,
            filter_direction: None,
            paused: false,
        }
    }

    /// Push a new packet record, evicting the oldest if at capacity.
    pub fn push(&mut self, record: PacketRecord) {
        if self.packets.len() >= self.capacity {
            self.packets.remove(0);
        }
        self.packets.push(record);
    }

    /// Returns packets matching the current filters.
    pub fn filtered_packets(&self) -> Vec<&PacketRecord> {
        self.packets
            .iter()
            .filter(|p| {
                self.filter_opcode.is_none_or(|op| p.opcode == op)
                    && self.filter_direction.is_none_or(|d| p.direction == d)
            })
            .collect()
    }

    /// Scroll up by one line.
    pub fn scroll_up(&mut self) {
        self.auto_scroll = false;
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    /// Scroll down by one line.
    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
        if self.scroll_offset == 0 {
            self.auto_scroll = true;
        }
    }

    /// Toggle pause state.
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// Clear all captured packets.
    pub fn clear(&mut self) {
        self.packets.clear();
        self.scroll_offset = 0;
        self.auto_scroll = true;
    }
}

// ─── EQ Internals panel state ───────────────────────────────────────────────

/// Category filter for the EQ Internals offset browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OffsetCategory {
    #[default]
    All,
    Globals,
    PlayerBase,
    PlayerZone,
    SpawnManager,
    Functions,
}

impl OffsetCategory {
    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Globals => "Globals",
            Self::PlayerBase => "PlayerBase",
            Self::PlayerZone => "PlayerZone",
            Self::SpawnManager => "SpawnMgr",
            Self::Functions => "Functions",
        }
    }

    /// Cycle to next category.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Globals,
            Self::Globals => Self::PlayerBase,
            Self::PlayerBase => Self::PlayerZone,
            Self::PlayerZone => Self::SpawnManager,
            Self::SpawnManager => Self::Functions,
            Self::Functions => Self::All,
        }
    }
}

/// A single entry in the EQ Internals offset list.
#[derive(Debug, Clone)]
pub struct OffsetEntry {
    /// Human-readable name (e.g. "pinstLocalPlayer", "player_base::x").
    pub name: String,
    /// Preferred-base address or struct field offset value.
    pub value: u64,
    /// Which category this offset belongs to.
    pub category: OffsetCategory,
}

/// State for the EQ Internals offset browser panel.
pub struct EqInternalsState {
    /// Ratatui table state for scroll/selection.
    pub table_state: TableState,
    /// All offset entries (built once from `OffsetDatabase`).
    pub all_entries: Vec<OffsetEntry>,
    /// Filtered entries after applying category + search.
    pub filtered_entries: Vec<OffsetEntry>,
    /// Active category filter.
    pub category_filter: OffsetCategory,
    /// Text search filter.
    pub search_filter: String,
    /// Whether in search input mode.
    pub search_mode: bool,
}

impl EqInternalsState {
    /// Build state from the compiled offset database.
    #[must_use]
    pub fn new() -> Self {
        let db = textquest_common::offset_db::OffsetDatabase::from_compiled_offsets();
        let mut entries = Vec::new();

        // Globals (pointer addresses).
        let mut globals: Vec<_> = db.globals.iter().collect();
        globals.sort_by_key(|(_, v)| *v);
        for (name, addr) in &globals {
            entries.push(OffsetEntry {
                name: name.to_string(),
                value: **addr,
                category: OffsetCategory::Globals,
            });
        }

        // PlayerBase field offsets.
        let mut pb: Vec<_> = db.player_base.iter().collect();
        pb.sort_by_key(|(_, v)| *v);
        for (name, off) in &pb {
            entries.push(OffsetEntry {
                name: format!("player_base::{name}"),
                value: **off as u64,
                category: OffsetCategory::PlayerBase,
            });
        }

        // PlayerZoneClient field offsets.
        let mut pz: Vec<_> = db.player_zone.iter().collect();
        pz.sort_by_key(|(_, v)| *v);
        for (name, off) in &pz {
            entries.push(OffsetEntry {
                name: format!("player_zone::{name}"),
                value: **off as u64,
                category: OffsetCategory::PlayerZone,
            });
        }

        // SpawnManager field offsets.
        let mut sm: Vec<_> = db.spawn_manager.iter().collect();
        sm.sort_by_key(|(_, v)| *v);
        for (name, off) in &sm {
            entries.push(OffsetEntry {
                name: format!("spawn_manager::{name}"),
                value: **off as u64,
                category: OffsetCategory::SpawnManager,
            });
        }

        // Function addresses.
        let mut funcs: Vec<_> = db.functions.iter().collect();
        funcs.sort_by_key(|(_, v)| *v);
        for (name, addr) in &funcs {
            entries.push(OffsetEntry {
                name: name.to_string(),
                value: **addr,
                category: OffsetCategory::Functions,
            });
        }

        let filtered = entries.clone();
        let mut table_state = TableState::default();
        table_state.select(Some(0));

        Self {
            table_state,
            all_entries: entries,
            filtered_entries: filtered,
            category_filter: OffsetCategory::All,
            search_filter: String::new(),
            search_mode: false,
        }
    }

    /// Rebuild the filtered list from current category + search filter.
    pub fn apply_filter(&mut self) {
        let cat = self.category_filter;
        let query = self.search_filter.to_lowercase();
        self.filtered_entries = self
            .all_entries
            .iter()
            .filter(|e| cat == OffsetCategory::All || e.category == cat)
            .filter(|e| query.is_empty() || e.name.to_lowercase().contains(&query))
            .cloned()
            .collect();
        // Reset selection to stay in bounds.
        let sel = self
            .table_state
            .selected()
            .unwrap_or(0)
            .min(self.filtered_entries.len().saturating_sub(1));
        self.table_state.select(Some(sel));
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        let i = self.table_state.selected().unwrap_or(0).saturating_sub(1);
        self.table_state.select(Some(i));
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        let max = self.filtered_entries.len().saturating_sub(1);
        let i = self.table_state.selected().unwrap_or(0);
        self.table_state.select(Some((i + 1).min(max)));
    }

    /// Get the currently selected entry.
    #[must_use]
    pub fn selected_entry(&self) -> Option<&OffsetEntry> {
        let idx = self.table_state.selected()?;
        self.filtered_entries.get(idx)
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
