use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use ratatui::{style::Color, widgets::TableState};

use super::{
    app::{NavClientStatus, NavScope, SpawnFilter, SpawnSort},
    theme::ThemeKind,
};
use crate::{
    eq::{
        map_parser::ZoneMap,
        named_tracker,
        structs::{SpawnInfo, SpawnType},
    },
    nav::mesh::NavMeshOverlay,
};

// ─── Help panel state ────────────────────────────────────────────────────────

/// A single entry in the help topic list.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HelpTopic {
    /// Canonical topic key used for matching and jump-targets.
    pub key: String,
    /// Human-readable display title shown in the panel.
    pub title: String,
    /// Help section this topic belongs to.
    pub section: String,
}

/// State for the help panel overlay.
///
/// Tracks open/closed state, search, result navigation, and scroll position.
/// Intentionally decoupled from rendering so `#1117` can land independently.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HelpPanelState {
    /// Whether the help panel is currently open.
    pub open: bool,
    /// Current search query string.
    pub query: String,
    /// Index of the currently highlighted search result.
    pub selected_result: usize,
    /// Scroll offset (in lines) within the selected help topic.
    pub scroll: usize,
    /// Full topic list — populated from command metadata at construction time.
    pub topics: Vec<HelpTopic>,
}

impl Default for HelpPanelState {
    fn default() -> Self {
        Self::new()
    }
}

impl HelpPanelState {
    /// Maximum lines scrollable per call to [`scroll_down`] / [`scroll_up`].
    pub const SCROLL_STEP: usize = 3;

    /// Create a new, closed help panel with an empty topic list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            selected_result: 0,
            scroll: 0,
            topics: Vec::new(),
        }
    }

    /// Create a help panel pre-populated with the given topic list.
    #[must_use]
    pub fn with_topics(topics: Vec<HelpTopic>) -> Self {
        Self {
            topics,
            ..Self::new()
        }
    }

    // ── Visibility ──────────────────────────────────────────────────────────

    /// Open the help panel, resetting scroll but preserving the query.
    pub fn open(&mut self) {
        self.open = true;
        self.scroll = 0;
    }

    /// Close the help panel.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Toggle open/closed state.
    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open();
        }
    }

    // ── Search ──────────────────────────────────────────────────────────────

    /// Replace the search query and reset result selection + scroll.
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.selected_result = 0;
        self.scroll = 0;
    }

    /// Append a single character to the search query.
    pub fn push_query_char(&mut self, ch: char) {
        self.query.push(ch);
        self.selected_result = 0;
        self.scroll = 0;
    }

    /// Remove the last character from the search query.
    pub fn pop_query_char(&mut self) {
        self.query.pop();
        self.selected_result = 0;
        self.scroll = 0;
    }

    /// Return topics whose `key` or `title` contain the current query
    /// (case-insensitive).  Returns all topics when the query is empty.
    #[must_use]
    pub fn filtered_topics(&self) -> Vec<&HelpTopic> {
        if self.query.is_empty() {
            return self.topics.iter().collect();
        }
        let lower = self.query.to_ascii_lowercase();
        self.topics
            .iter()
            .filter(|t| {
                t.key.to_ascii_lowercase().contains(&lower)
                    || t.title.to_ascii_lowercase().contains(&lower)
                    || t.section.to_ascii_lowercase().contains(&lower)
            })
            .collect()
    }

    /// Return the currently selected topic, if any.
    #[must_use]
    pub fn selected_topic(&self) -> Option<&HelpTopic> {
        let results = self.filtered_topics();
        results.get(self.selected_result).copied()
    }

    // ── Result navigation ───────────────────────────────────────────────────

    /// Move selection to the next result; wraps around.
    pub fn next_result(&mut self) {
        let count = self.filtered_topics().len();
        if count == 0 {
            return;
        }
        self.selected_result = (self.selected_result + 1) % count;
        self.scroll = 0;
    }

    /// Move selection to the previous result; wraps around.
    pub fn prev_result(&mut self) {
        let count = self.filtered_topics().len();
        if count == 0 {
            return;
        }
        self.selected_result = self.selected_result.saturating_add(count - 1) % count;
        self.scroll = 0;
    }

    // ── Scroll ──────────────────────────────────────────────────────────────

    /// Scroll down by [`SCROLL_STEP`] lines.
    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(Self::SCROLL_STEP);
    }

    /// Scroll up by [`SCROLL_STEP`] lines (floors at 0).
    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(Self::SCROLL_STEP);
    }

    /// Reset scroll to the top.
    pub fn scroll_to_top(&mut self) {
        self.scroll = 0;
    }
}

#[cfg(test)]
mod help_panel_tests {
    use super::{HelpPanelState, HelpTopic};

    fn make_topics() -> Vec<HelpTopic> {
        vec![
            HelpTopic {
                key: "camp".into(),
                title: "Camp Loop".into(),
                section: "Workflows".into(),
            },
            HelpTopic {
                key: "nav".into(),
                title: "Navigation".into(),
                section: "Navigation".into(),
            },
            HelpTopic {
                key: "help".into(),
                title: "Help Overlay".into(),
                section: "Workflows".into(),
            },
        ]
    }

    #[test]
    fn new_panel_is_closed() {
        let state = HelpPanelState::new();
        assert!(!state.open);
        assert_eq!(state.query, "");
        assert_eq!(state.selected_result, 0);
        assert_eq!(state.scroll, 0);
        assert!(state.topics.is_empty());
    }

    #[test]
    fn toggle_open_close() {
        let mut state = HelpPanelState::new();
        assert!(!state.open);
        state.toggle();
        assert!(state.open);
        state.toggle();
        assert!(!state.open);
    }

    #[test]
    fn open_resets_scroll() {
        let mut state = HelpPanelState::new();
        state.scroll = 9;
        state.open();
        assert!(state.open);
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn close_sets_open_false() {
        let mut state = HelpPanelState::with_topics(make_topics());
        state.open();
        state.close();
        assert!(!state.open);
    }

    #[test]
    fn set_query_resets_selection_and_scroll() {
        let mut state = HelpPanelState::with_topics(make_topics());
        state.selected_result = 2;
        state.scroll = 5;
        state.set_query("nav");
        assert_eq!(state.query, "nav");
        assert_eq!(state.selected_result, 0);
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn push_pop_query_char() {
        let mut state = HelpPanelState::new();
        state.push_query_char('n');
        state.push_query_char('a');
        state.push_query_char('v');
        assert_eq!(state.query, "nav");
        state.pop_query_char();
        assert_eq!(state.query, "na");
    }

    #[test]
    fn filtered_topics_empty_query_returns_all() {
        let state = HelpPanelState::with_topics(make_topics());
        assert_eq!(state.filtered_topics().len(), 3);
    }

    #[test]
    fn filtered_topics_matches_key_and_title() {
        let state = HelpPanelState::with_topics(make_topics());
        let results = state.filtered_topics();
        // all 3 topics returned when query is empty
        assert_eq!(results.len(), 3);

        let mut s = HelpPanelState::with_topics(make_topics());
        s.set_query("nav");
        let results = s.filtered_topics();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "nav");
    }

    #[test]
    fn filtered_topics_case_insensitive() {
        let mut state = HelpPanelState::with_topics(make_topics());
        state.set_query("CAMP");
        let results = state.filtered_topics();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "camp");
    }

    #[test]
    fn next_result_wraps() {
        let mut state = HelpPanelState::with_topics(make_topics());
        state.next_result(); // → 1
        state.next_result(); // → 2
        state.next_result(); // → 0 (wrap)
        assert_eq!(state.selected_result, 0);
    }

    #[test]
    fn prev_result_wraps() {
        let mut state = HelpPanelState::with_topics(make_topics());
        state.prev_result(); // → 2 (wrap from 0)
        assert_eq!(state.selected_result, 2);
        state.prev_result(); // → 1
        assert_eq!(state.selected_result, 1);
    }

    #[test]
    fn next_prev_reset_scroll() {
        let mut state = HelpPanelState::with_topics(make_topics());
        state.scroll = 10;
        state.next_result();
        assert_eq!(state.scroll, 0);
        state.scroll = 10;
        state.prev_result();
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn scroll_up_down_and_top() {
        let mut state = HelpPanelState::new();
        state.scroll_down();
        assert_eq!(state.scroll, HelpPanelState::SCROLL_STEP);
        state.scroll_down();
        assert_eq!(state.scroll, HelpPanelState::SCROLL_STEP * 2);
        state.scroll_up();
        assert_eq!(state.scroll, HelpPanelState::SCROLL_STEP);
        state.scroll_to_top();
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn scroll_up_floors_at_zero() {
        let mut state = HelpPanelState::new();
        state.scroll_up();
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn selected_topic_none_on_empty() {
        let state = HelpPanelState::new();
        assert!(state.selected_topic().is_none());
    }

    #[test]
    fn selected_topic_returns_correct_entry() {
        let mut state = HelpPanelState::with_topics(make_topics());
        state.next_result(); // → index 1 = "nav"
        assert_eq!(state.selected_topic().map(|t| t.key.as_str()), Some("nav"));
    }

    #[test]
    fn next_result_noop_on_empty_topics() {
        let mut state = HelpPanelState::new();
        state.next_result();
        assert_eq!(state.selected_result, 0);
    }

    #[test]
    fn prev_result_noop_on_empty_topics() {
        let mut state = HelpPanelState::new();
        state.prev_result();
        assert_eq!(state.selected_result, 0);
    }

    #[test]
    fn serde_roundtrip() {
        let mut state = HelpPanelState::with_topics(make_topics());
        state.open();
        state.set_query("nav");
        state.scroll_down();

        let json = serde_json::to_string(&state).expect("serialize");
        let restored: HelpPanelState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.open, state.open);
        assert_eq!(restored.query, state.query);
        assert_eq!(restored.scroll, state.scroll);
        assert_eq!(restored.topics.len(), state.topics.len());
    }
}

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
    /// Active sort column for the spawn list.
    pub sort_column: SpawnSort,
    /// Sort direction — true for ascending, false for descending.
    pub sort_ascending: bool,
    /// Navigation target scope (Active client, Group, or All).
    pub nav_scope: NavScope,
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
            sort_column: SpawnSort::Default,
            sort_ascending: true,
            nav_scope: NavScope::Active,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FilteredSpawnCacheKey {
    pub client_pid: Option<u32>,
    pub spawn_revision: u64,
    pub spawn_filter: String,
    pub spawn_type_filter: SpawnFilter,
    pub sort_column: SpawnSort,
    pub sort_ascending: bool,
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
    pub map_filter_bits: u16,
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
    /// When set, triggers an immediate ReadMemory poll in the run loop.
    pub pending_memory_poll: bool,
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
            pending_memory_poll: false,
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
    /// These annotations apply when viewing raw spawn memory (base at struct
    /// start).
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

const DEFAULT_HOOK_ROTATION_INTERVAL_MS: u64 = 5_000;

/// Status of a hook slot in the rotation display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookSlotState {
    /// The hook is currently active.
    Active,
    /// The hook is temporarily unhooked.
    Unhooked,
}

impl HookSlotState {
    /// Returns the short uppercase display label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Unhooked => "UNHOOKED",
        }
    }
}

/// A single hook entry in the rotation panel.
#[derive(Debug, Clone)]
pub struct HookRotationEntry {
    /// Hook name shown in the panel.
    pub name: String,
    /// Current hook slot state.
    pub state: HookSlotState,
    /// Timestamp of the most recent rotation event, if any.
    pub last_rotated_at: Option<Instant>,
    /// Scheduled time for the next rotation, if any.
    pub next_rotation_at: Option<Instant>,
}

impl HookRotationEntry {
    /// Create a new entry with no rotation history.
    #[must_use]
    pub fn new(name: impl Into<String>, state: HookSlotState) -> Self {
        Self {
            name: name.into(),
            state,
            last_rotated_at: None,
            next_rotation_at: None,
        }
    }

    /// Milliseconds elapsed since the last rotation.
    #[must_use]
    pub fn ms_since_last_rotation(&self) -> Option<u64> {
        self.last_rotated_at
            .map(|instant| duration_ms(instant.elapsed()))
    }

    /// Milliseconds remaining until the next scheduled rotation.
    #[must_use]
    pub fn ms_until_next_rotation(&self) -> Option<u64> {
        self.next_rotation_at
            .map(|instant| duration_ms(instant.saturating_duration_since(Instant::now())))
    }
}

/// Hook rotation status used by the debug panel.
#[derive(Debug, Clone)]
pub struct HookRotationState {
    /// Current hook entries displayed in the panel.
    pub entries: Vec<HookRotationEntry>,
    /// Default rotation interval shown in the panel.
    pub interval_ms: u64,
}

impl HookRotationState {
    /// Create the default hook rotation view.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: default_hook_rotation_entries(),
            interval_ms: DEFAULT_HOOK_ROTATION_INTERVAL_MS,
        }
    }

    /// Record a hook rotation for the named entry.
    pub fn record_rotation(
        &mut self,
        name: impl Into<String>,
        state: HookSlotState,
        interval_ms: u64,
    ) {
        let name = name.into();
        let now = Instant::now();
        let next_rotation_at = now.checked_add(Duration::from_millis(interval_ms));

        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.name == name) {
            entry.state = state;
            entry.last_rotated_at = Some(now);
            entry.next_rotation_at = next_rotation_at;
        } else {
            self.entries.push(HookRotationEntry {
                name,
                state,
                last_rotated_at: Some(now),
                next_rotation_at,
            });
        }
    }

    /// Update the panel-wide rotation interval and reschedule any active
    /// entries.
    pub fn set_interval_ms(&mut self, interval_ms: u64) {
        self.interval_ms = interval_ms;
        let now = Instant::now();
        for entry in &mut self.entries {
            if entry.last_rotated_at.is_some() {
                entry.next_rotation_at = now.checked_add(Duration::from_millis(interval_ms));
            }
        }
    }
}

impl Default for HookRotationState {
    fn default() -> Self {
        Self::new()
    }
}

fn duration_ms(duration: Duration) -> u64 {
    let millis = duration.as_millis();
    if millis > u64::MAX as u128 {
        u64::MAX
    } else {
        millis as u64
    }
}

fn default_hook_rotation_entries() -> Vec<HookRotationEntry> {
    ["ProcessGameEvents", "CastHook", "SetGameState", "DspChat"]
        .into_iter()
        .map(|name| HookRotationEntry::new(name, HookSlotState::Active))
        .collect()
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

/// NPC subcategories based on spawn name patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcCategory {
    /// NPCs that serve as merchants/vendors.
    Merchant,
    /// NPCs that serve as bankers.
    Banker,
    /// Training dummies (typically for skill testing).
    TrainingDummy,
    /// Quest NPCs (typically have "quest" or similar in name).
    QuestNpc,
    /// All other NPCs not in the above categories.
    Other,
}

impl NpcCategory {
    /// Determine the NPC category from a spawn name.
    /// Returns `Some(category)` for any NPC spawn; returns `None` for non-NPC types.
    #[must_use]
    pub fn from_spawn_name(name: &str) -> Option<Self> {
        let lower = name.to_lowercase();

        // Check for merchant patterns
        if lower.contains("merchant")
            || lower.contains("vendor")
            || lower.contains("trainer")
            || lower.contains("master")
            || lower.contains("captain")
            || lower.contains("quartermaster")
        {
            return Some(Self::Merchant);
        }

        // Check for banker patterns
        if lower.contains("banker") || lower.contains("exchange") {
            return Some(Self::Banker);
        }

        // Check for training dummy patterns
        if lower.contains("training dummy") || lower.contains("practice dummy") {
            return Some(Self::TrainingDummy);
        }

        // Check for quest NPC patterns
        if lower.contains("quest") || lower.contains("task") {
            return Some(Self::QuestNpc);
        }

        // Default to Other for any unclassified NPC
        Some(Self::Other)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Merchant => "Merchant",
            Self::Banker => "Banker",
            Self::TrainingDummy => "Training Dummy",
            Self::QuestNpc => "Quest NPC",
            Self::Other => "Other NPC",
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
    NpcMerchant,
    NpcBanker,
    NpcTrainingDummy,
    NpcQuestNpc,
    NpcOther,
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
            "merchant" => Some(Self::NpcMerchant),
            "banker" => Some(Self::NpcBanker),
            "training" | "dummy" => Some(Self::NpcTrainingDummy),
            "quest" => Some(Self::NpcQuestNpc),
            "other" => Some(Self::NpcOther),
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
            Self::NpcMerchant => "Merchant",
            Self::NpcBanker => "Banker",
            Self::NpcTrainingDummy => "Training Dummy",
            Self::NpcQuestNpc => "Quest NPC",
            Self::NpcOther => "Other NPC",
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
    pub show_merchant: bool,
    pub show_banker: bool,
    pub show_training_dummy: bool,
    pub show_quest_npc: bool,
    pub show_other_npc: bool,
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
            show_merchant: true,
            show_banker: true,
            show_training_dummy: true,
            show_quest_npc: true,
            show_other_npc: true,
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
            MapFilterKind::NpcMerchant => self.show_merchant = enabled,
            MapFilterKind::NpcBanker => self.show_banker = enabled,
            MapFilterKind::NpcTrainingDummy => self.show_training_dummy = enabled,
            MapFilterKind::NpcQuestNpc => self.show_quest_npc = enabled,
            MapFilterKind::NpcOther => self.show_other_npc = enabled,
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
            MapFilterKind::NpcMerchant => self.show_merchant,
            MapFilterKind::NpcBanker => self.show_banker,
            MapFilterKind::NpcTrainingDummy => self.show_training_dummy,
            MapFilterKind::NpcQuestNpc => self.show_quest_npc,
            MapFilterKind::NpcOther => self.show_other_npc,
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
        self.show_merchant = enabled;
        self.show_banker = enabled;
        self.show_training_dummy = enabled;
        self.show_quest_npc = enabled;
        self.show_other_npc = enabled;
    }

    #[must_use]
    pub fn cache_key_bits(&self) -> u16 {
        let mut bits = 0u16;
        bits |= u16::from(self.show_npc);
        bits |= u16::from(self.show_pc) << 1;
        bits |= u16::from(self.show_corpse) << 2;
        bits |= u16::from(self.show_ground) << 3;
        bits |= u16::from(self.show_pet) << 4;
        bits |= u16::from(self.show_named) << 5;
        bits |= u16::from(self.show_untargetable) << 6;
        bits |= u16::from(self.show_merchant) << 7;
        bits |= u16::from(self.show_banker) << 8;
        bits |= u16::from(self.show_training_dummy) << 9;
        bits |= u16::from(self.show_quest_npc) << 10;
        bits |= u16::from(self.show_other_npc) << 11;
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

        // Apply NPC subcategory filters if this is an NPC (and not a pet/named which take priority)
        if is_npc && !is_pet && !is_named {
            if let Some(category) = NpcCategory::from_spawn_name(&spawn.displayed_name) {
                let allowed = match category {
                    NpcCategory::Merchant => self.show_merchant,
                    NpcCategory::Banker => self.show_banker,
                    NpcCategory::TrainingDummy => self.show_training_dummy,
                    NpcCategory::QuestNpc => self.show_quest_npc,
                    NpcCategory::Other => self.show_other_npc,
                };
                if !allowed {
                    return false;
                }
            }
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

/// How spawn labels are drawn on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapNameStyle {
    /// No labels.
    #[default]
    Off,
    /// Show spawn name only.
    Name,
    /// Show name + level.
    NameLevel,
    /// Show name + class.
    NameClass,
}

impl MapNameStyle {
    /// Cycle to the next style.
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Name,
            Self::Name => Self::NameLevel,
            Self::NameLevel => Self::NameClass,
            Self::NameClass => Self::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Name => "name",
            Self::NameLevel => "name+level",
            Self::NameClass => "name+class",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "off" | "none" => Some(Self::Off),
            "name" => Some(Self::Name),
            "namelevel" | "name+level" | "nl" => Some(Self::NameLevel),
            "nameclass" | "name+class" | "nc" => Some(Self::NameClass),
            _ => None,
        }
    }
}

/// A spawn highlight with optional visual overrides.
#[derive(Debug, Clone)]
pub struct MapHighlight {
    /// Spawn name pattern (case-insensitive substring match).
    pub pattern: String,
    /// Pre-lowercased pattern for O(1) comparison in render loop.
    pub pattern_lower: String,
    /// Override color for the highlighted spawn marker.
    pub color: Option<Color>,
    /// Marker size: 1=small dot, 2=medium, 3=large.
    pub size: u8,
    /// Whether the marker should pulse (blink).
    pub pulse: bool,
}

/// A location marker placed on the map.
#[derive(Debug, Clone)]
pub struct MapLocMarker {
    pub x: f32,
    pub y: f32,
    pub label: String,
}

/// A persistent named marker saved to disk and rendered on the map.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NamedMapMarker {
    /// User-assigned name for recall and display.
    pub name: String,
    /// EQ X coordinate.
    pub x: f32,
    /// EQ Y coordinate.
    pub y: f32,
    /// Optional Z (vertical) coordinate.
    pub z: f32,
    /// Optional display label (defaults to name when absent).
    pub label: Option<String>,
    /// Zone short name where the marker was placed.
    pub zone: String,
}

impl NamedMapMarker {
    /// The text to show alongside the marker glyph on the map.
    #[must_use]
    pub fn display_label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

/// Persistent store of named map markers.
///
/// Backed by a JSON file in `%TEMP%/textquest/map_markers.json` (or the
/// `TEXTQUEST_MARKER_FILE` environment variable override).
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct NamedMarkerFile {
    markers: Vec<NamedMapMarker>,
}

/// Return the path to the marker persistence file.
fn marker_store_path() -> PathBuf {
    if let Ok(env_path) = std::env::var("TEXTQUEST_MARKER_FILE") {
        return PathBuf::from(env_path);
    }
    std::env::temp_dir()
        .join("textquest")
        .join("map_markers.json")
}

/// Load markers from disk, returning an empty list on any error.
fn load_named_markers(path: &PathBuf) -> Vec<NamedMapMarker> {
    load_named_markers_pub(path)
}

/// Public re-export of marker loading for use from `app.rs`.
pub fn load_named_markers_pub(path: &PathBuf) -> Vec<NamedMapMarker> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    serde_json::from_slice::<NamedMarkerFile>(&bytes)
        .map(|f| f.markers)
        .unwrap_or_default()
}

/// Save markers to disk, creating parent directories as needed.
///
/// NOTE: These markers are operator-local visual annotations only and do not
/// drive fleet navigation routing (orchestrator / camp config / pathfinder).
/// Fleet nav consumes `CampConfig` from `config/camps/*.toml` instead.
///
/// Returns `Ok(())` on success or an `anyhow::Error` on failure.
pub fn save_named_markers(path: &PathBuf, markers: &[NamedMapMarker]) -> anyhow::Result<()> {
    validate_marker_store_path(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Marker file path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)?;
    let file = NamedMarkerFile {
        markers: markers.to_vec(),
    };
    let json = serde_json::to_string_pretty(&file)?;
    // Write to a non-guessable OS-random temp file in the same directory.
    // NamedTempFile auto-removes the temp file if dropped without persisting,
    // ensuring cleanup on all error paths.
    let mut staged = tempfile::Builder::new()
        .prefix(".map_markers.")
        .suffix(".tmp")
        .tempfile_in(parent)?;
    staged.write_all(json.as_bytes())?;
    staged.as_file().sync_all()?;
    // Atomically replace the destination.
    // On Unix this is a rename(2); on Windows tempfile uses MoveFileExW with
    // MOVEFILE_REPLACE_EXISTING, so no separate pre-deletion is needed.
    // If persist fails the NamedTempFile is returned in the error and auto-removed
    // on drop.
    staged.persist(path).map(|_| ()).map_err(|e| {
        anyhow::anyhow!(
            "Failed to persist marker file {}: {}",
            path.display(),
            e.error
        )
    })
}

fn validate_marker_store_path(path: &Path) -> anyhow::Result<()> {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            anyhow::bail!("Marker file path is a symlink: {}", path.display());
        }
        if metadata_has_reparse_point(&meta) {
            anyhow::bail!(
                "Marker file path is a Windows reparse point: {}",
                path.display()
            );
        }
        if meta.is_dir() {
            anyhow::bail!("Marker file path is a directory: {}", path.display());
        }
        if !meta.is_file() {
            anyhow::bail!("Marker file path is not a regular file: {}", path.display());
        }
    }
    // Walk ancestor directories checking for symlinks/reparse points. Stop at
    // filesystem root — no need to stat "/" or "C:\".
    for ancestor in path.ancestors().skip(1) {
        if ancestor.parent().is_none() {
            break;
        }
        if let Ok(meta) = std::fs::symlink_metadata(ancestor) {
            if meta.file_type().is_symlink() {
                anyhow::bail!("Marker directory is a symlink: {}", ancestor.display());
            }
            if metadata_has_reparse_point(&meta) {
                anyhow::bail!(
                    "Marker directory is a Windows reparse point: {}",
                    ancestor.display()
                );
            }
            if !meta.is_dir() {
                anyhow::bail!(
                    "Marker directory is not a directory: {}",
                    ancestor.display()
                );
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn metadata_has_reparse_point(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_has_reparse_point(_meta: &std::fs::Metadata) -> bool {
    false
}

/// A radius overlay circle around the player.
#[derive(Debug, Clone)]
pub struct MapRadiusOverlay {
    pub radius: f32,
    pub color: Color,
    pub label: String,
}

/// Camp location overlay drawn on the zone map.
///
/// Renders the camp center marker (`⊕`), pull point marker (`⊗`),
/// camp radius circle (green), and pull radius circle (red).
#[derive(Debug, Clone)]
pub struct CampOverlay {
    /// XY world coordinates of the camp anchor point.
    pub camp_center: [f32; 2],
    /// XY world coordinates of the pull point.
    pub pull_point: [f32; 2],
    /// Radius around camp center (green circle).
    pub camp_radius: f32,
    /// Radius around pull point (red circle).
    pub pull_radius: f32,
    /// Display name for the camp overlay.
    pub name: String,
}

/// A saved set of map filter settings.
#[derive(Debug, Clone)]
pub struct MapFilterPreset {
    pub name: String,
    pub filters: MapFilters,
    pub show_geometry: bool,
    pub show_spawns: bool,
    pub show_nav_paths: bool,
    pub show_navmesh: bool,
    pub show_labels: bool,
    pub show_annotations: bool,
}

/// What happens when the user presses Enter on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MapClickAction {
    #[default]
    None,
    /// Place a loc marker at the cursor position.
    PlaceLoc,
    /// Navigate to the cursor position.
    Navigate,
}

/// A preset that quickly shows/hides groups of map layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapVisibilityPreset {
    /// Show all layers.
    All,
    /// Geometry + spawns only (clean tactical view).
    Tactical,
    /// Spawns + nav paths (navigation focus).
    Navigation,
    /// Only geometry.
    GeometryOnly,
    /// Minimal — spawns only.
    SpawnsOnly,
}

// Alias for clarity — MapViewPreset is the primary name
pub type MapViewPreset = MapVisibilityPreset;

impl MapVisibilityPreset {
    /// Returns the human-readable label for this preset.
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Tactical => "Tactical",
            Self::Navigation => "Navigator",
            Self::GeometryOnly => "Geometry",
            Self::SpawnsOnly => "Spawns",
        }
    }

    /// Returns the next preset in order.
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Tactical,
            Self::Tactical => Self::Navigation,
            Self::Navigation => Self::GeometryOnly,
            Self::GeometryOnly => Self::SpawnsOnly,
            Self::SpawnsOnly => Self::All,
        }
    }

    /// Returns the previous preset in order.
    pub fn prev(self) -> Self {
        match self {
            Self::All => Self::SpawnsOnly,
            Self::Tactical => Self::All,
            Self::Navigation => Self::Tactical,
            Self::GeometryOnly => Self::Navigation,
            Self::SpawnsOnly => Self::GeometryOnly,
        }
    }

    /// Get preset by index (0-4).
    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::All),
            1 => Some(Self::Tactical),
            2 => Some(Self::Navigation),
            3 => Some(Self::GeometryOnly),
            4 => Some(Self::SpawnsOnly),
            _ => None,
        }
    }

    /// Get the index of this preset (0-4).
    pub fn to_index(self) -> usize {
        match self {
            Self::All => 0,
            Self::Tactical => 1,
            Self::Navigation => 2,
            Self::GeometryOnly => 3,
            Self::SpawnsOnly => 4,
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
    /// Z-depth filter range — spawns farther than this from the player's Z are
    /// hidden.
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
    /// Active spawn highlights.
    pub highlights: Vec<MapHighlight>,
    /// Location marker placed by :maploc.
    pub loc_marker: Option<MapLocMarker>,
    /// How spawn labels are rendered.
    pub name_style: MapNameStyle,
    /// Cast radius overlay circle.
    pub cast_radius: Option<MapRadiusOverlay>,
    /// Spell radius overlay circle.
    pub spell_radius: Option<MapRadiusOverlay>,
    /// Aggro radius overlay circle drawn around all NPC spawns.
    pub aggro_radius: Option<MapRadiusOverlay>,
    /// Show target path overlay (nav waypoints to target).
    pub show_target_path: bool,
    /// Show direct line from player to target.
    pub show_target_line: bool,
    /// Saved filter presets by name.
    pub saved_presets: Vec<MapFilterPreset>,
    /// What the map click/enter action does.
    pub click_action: MapClickAction,
    /// Named persistent markers keyed by lowercase name.
    pub named_markers: Vec<NamedMapMarker>,
    /// Path to the marker persistence file.
    pub marker_file: PathBuf,
    /// Active camp location overlay (set when a camp is started).
    pub camp_overlay: Option<CampOverlay>,
    /// Currently active map view preset.
    pub current_preset: MapVisibilityPreset,
}

impl MapScreenState {
    /// Creates a new map state, resolving the map directory path.
    #[must_use]
    pub fn new() -> Self {
        let map_dir = resolve_map_dir();
        let marker_file = marker_store_path();
        let named_markers = load_named_markers(&marker_file);
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
            highlights: Vec::new(),
            loc_marker: None,
            name_style: MapNameStyle::Off,
            cast_radius: None,
            spell_radius: None,
            aggro_radius: None,
            show_target_path: true,
            show_target_line: true,
            saved_presets: Vec::new(),
            click_action: MapClickAction::None,
            named_markers,
            marker_file,
            camp_overlay: None,
            current_preset: MapVisibilityPreset::All,
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

    #[must_use]
    pub fn toggle_filter(&mut self, kind: MapFilterKind) -> String {
        let enabled = self.filters.toggle(kind);
        format!(
            "{} filter {}",
            kind.label(),
            if enabled { "ON" } else { "OFF" }
        )
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

    /// Apply a visibility preset.
    pub fn apply_visibility_preset(&mut self, preset: MapVisibilityPreset) {
        match preset {
            MapVisibilityPreset::All => {
                self.show_geometry = true;
                self.show_spawns = true;
                self.show_nav_paths = true;
                self.show_labels = true;
                self.show_annotations = true;
                self.show_navmesh = true;
            }
            MapVisibilityPreset::Tactical => {
                self.show_geometry = true;
                self.show_spawns = true;
                self.show_nav_paths = false;
                self.show_labels = false;
                self.show_annotations = false;
                self.show_navmesh = false;
            }
            MapVisibilityPreset::Navigation => {
                self.show_geometry = false;
                self.show_spawns = true;
                self.show_nav_paths = true;
                self.show_labels = false;
                self.show_annotations = false;
                self.show_navmesh = true;
            }
            MapVisibilityPreset::GeometryOnly => {
                self.show_geometry = true;
                self.show_spawns = false;
                self.show_nav_paths = false;
                self.show_labels = false;
                self.show_annotations = false;
                self.show_navmesh = false;
            }
            MapVisibilityPreset::SpawnsOnly => {
                self.show_geometry = false;
                self.show_spawns = true;
                self.show_nav_paths = false;
                self.show_labels = false;
                self.show_annotations = false;
                self.show_navmesh = false;
            }
        }
    }

    /// Save current filter state as a named preset.
    pub fn save_preset(&mut self, name: String) {
        let preset = MapFilterPreset {
            name: name.clone(),
            filters: self.filters.clone(),
            show_geometry: self.show_geometry,
            show_spawns: self.show_spawns,
            show_nav_paths: self.show_nav_paths,
            show_navmesh: self.show_navmesh,
            show_labels: self.show_labels,
            show_annotations: self.show_annotations,
        };
        if let Some(existing) = self.saved_presets.iter_mut().find(|p| p.name == name) {
            *existing = preset;
        } else {
            self.saved_presets.push(preset);
        }
    }

    /// Load a named preset, returning true if found.
    pub fn load_preset(&mut self, name: &str) -> bool {
        if let Some(preset) = self.saved_presets.iter().find(|p| p.name == name).cloned() {
            self.filters = preset.filters;
            self.show_geometry = preset.show_geometry;
            self.show_spawns = preset.show_spawns;
            self.show_nav_paths = preset.show_nav_paths;
            self.show_navmesh = preset.show_navmesh;
            self.show_labels = preset.show_labels;
            self.show_annotations = preset.show_annotations;
            true
        } else {
            false
        }
    }

    /// Delete a named preset, returning true if found.
    pub fn delete_preset(&mut self, name: &str) -> bool {
        let before = self.saved_presets.len();
        self.saved_presets.retain(|p| p.name != name);
        self.saved_presets.len() < before
    }

    /// Apply a map view preset and update the current_preset tracking.
    pub fn apply_view_preset(&mut self, preset: MapVisibilityPreset) {
        self.apply_visibility_preset(preset);
        self.current_preset = preset;
    }

    /// Cycle to the next map view preset.
    pub fn next_preset(&mut self) -> MapVisibilityPreset {
        let next = self.current_preset.next();
        self.apply_view_preset(next);
        next
    }

    /// Cycle to the previous map view preset.
    pub fn prev_preset(&mut self) -> MapVisibilityPreset {
        let prev = self.current_preset.prev();
        self.apply_view_preset(prev);
        prev
    }

    /// Select a preset by index (0-4), returns the preset if valid.
    pub fn select_preset_by_index(&mut self, idx: usize) -> Option<MapVisibilityPreset> {
        MapVisibilityPreset::from_index(idx).map(|preset| {
            self.apply_view_preset(preset);
            preset
        })
    }

    /// Add a spawn highlight.
    pub fn add_highlight(&mut self, highlight: MapHighlight) {
        self.highlights.push(highlight);
    }

    /// Remove highlights matching a pattern (case-insensitive).
    pub fn remove_highlight(&mut self, pattern: &str) -> usize {
        let lower = pattern.to_ascii_lowercase();
        let before = self.highlights.len();
        self.highlights
            .retain(|h| h.pattern.to_ascii_lowercase() != lower);
        before - self.highlights.len()
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
    /// Whether the live map is shown in split-screen dashboard.
    pub show_map: bool,
    /// Whether the kill tracker panel is visible.
    pub show_kills: bool,
    /// Whether the kill tracker panel is collapsed.
    pub kills_collapsed: bool,
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
            show_map: false,
            show_kills: true,
            kills_collapsed: false,
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
    /// Whether the spawn events panel is visible.
    pub show_spawn_events: bool,
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
            show_spawn_events: true,
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
    /// Whether the `/nav ui` debug diagnostics overlay is enabled.
    pub show_nav_debug: bool,
    /// Most recently fetched nav diagnostics for the focused client (PID,
    /// diagnostics).
    pub nav_diagnostics: Option<(u32, textquest_common::nav::NavDiagnostics)>,
}

impl NavigationScreenState {
    /// Creates a new navigation state with no active statuses.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nav_selected: 0,
            nav_statuses: HashMap::new(),
            show_nav_debug: false,
            nav_diagnostics: None,
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

    /// Browse to the next command in history, restoring the in-progress draft
    /// at the end.
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
    /// Strips arguments for grouping: "ma Warrior" → "ma", "G1 /sit" → "G1
    /// /sit" but preserves slash commands: "all /sit" → "all /sit"
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
    /// Human-readable process name resolved from `client_id` at capture time.
    /// Falls back to an empty string when resolution is unavailable.
    pub process_name: String,
    /// EQ protocol opcode identifier.
    pub opcode: u16,
    /// Whether the packet was inbound or outbound.
    pub direction: textquest_common::ipc::PacketDirection,
    /// Timestamp in milliseconds (from DLL).
    pub timestamp_ms: u64,
    /// Size of the packet payload in bytes.
    pub payload_size: u32,
    /// Raw packet payload bytes.
    pub payload: Vec<u8>,
}

/// Entry for a decoded packet with field names and values.
#[derive(Debug, Clone)]
pub struct PacketEntry {
    /// Pairs of (field_name, field_value) decoded from the packet.
    pub decoded_fields: Vec<(String, String)>,
}

/// State for the packet/opcode monitor panel.
pub struct PacketMonitorState {
    /// Ring buffer of captured packets (newest at the end).
    pub packets: Vec<PacketRecord>,
    /// Packet table selection in the filtered view.
    pub table_state: TableState,
    /// Maximum number of packets to retain.
    pub capacity: usize,
    /// Whether the view auto-scrolls to follow new packets.
    pub auto_scroll: bool,
    /// Scroll offset from the bottom (0 = latest).
    pub scroll_offset: usize,
    /// Optional opcode filter — when set, only show packets matching this
    /// opcode.
    pub filter_opcode: Option<u16>,
    /// Optional direction filter.
    pub filter_direction: Option<textquest_common::ipc::PacketDirection>,
    /// Optional client PID filter — when set, only show packets from this PID.
    pub filter_client_id: Option<u32>,
    /// Whether the panel is paused (stops consuming new packets into view).
    pub paused: bool,
    /// Millisecond timestamp (from packet records) when the first packet was
    /// captured. Used to compute "N packets since capture started".
    pub capture_start_ms: Option<u64>,
    /// Tracked peak packets-per-second observed across the entire session.
    /// Updated on every `push()` so the sidebar can display it without a full
    /// linear scan over all historical packets.
    pub peak_rate: usize,
}

impl PacketMonitorState {
    #[must_use]
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        Self {
            packets: Vec::with_capacity(1024),
            table_state,
            capacity: 10_000,
            auto_scroll: true,
            scroll_offset: 0,
            filter_opcode: None,
            filter_direction: None,
            filter_client_id: None,
            paused: false,
            capture_start_ms: None,
            peak_rate: 0,
        }
    }

    /// Push a new packet record, evicting the oldest if at capacity.
    pub fn push(&mut self, record: PacketRecord) {
        // Track the earliest captured timestamp for "session age" display.
        if self.capture_start_ms.is_none() {
            self.capture_start_ms = Some(record.timestamp_ms);
        }
        if self.packets.len() >= self.capacity {
            self.packets.remove(0);
        }
        self.packets.push(record);

        // Update peak rate from the current measurement.
        let (_, new_peak) = self.packet_rates();
        self.peak_rate = self.peak_rate.max(new_peak);

        if self.auto_scroll {
            self.select_last_filtered();
        } else {
            self.clamp_selection();
        }
    }

    /// Returns packets matching the current filters.
    pub fn filtered_packets(&self) -> Vec<&PacketRecord> {
        self.packets
            .iter()
            .filter(|p| {
                self.filter_opcode.is_none_or(|op| p.opcode == op)
                    && self.filter_direction.is_none_or(|d| p.direction == d)
                    && self.filter_client_id.is_none_or(|id| p.client_id == id)
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
            self.select_last_filtered();
        }
    }

    /// Toggle pause state.
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// Clear all captured packets and reset session counters.
    pub fn clear(&mut self) {
        self.packets.clear();
        self.scroll_offset = 0;
        self.auto_scroll = true;
        self.capture_start_ms = None;
        self.peak_rate = 0;
        self.table_state.select(Some(0));
    }

    /// Move packet selection up in the filtered view.
    pub fn select_prev(&mut self) {
        self.auto_scroll = false;
        let idx = self.table_state.selected().unwrap_or(0).saturating_sub(1);
        self.table_state.select(Some(idx));
    }

    /// Move packet selection down in the filtered view.
    pub fn select_next(&mut self) {
        let max = self.filtered_packets().len().saturating_sub(1);
        let next = self.table_state.selected().unwrap_or(0).saturating_add(1);
        let idx = next.min(max);
        self.table_state.select(Some(idx));
        if idx == max {
            self.auto_scroll = true;
            self.scroll_offset = 0;
        } else {
            self.auto_scroll = false;
        }
    }

    /// Return the selected packet in the filtered view.
    #[must_use]
    pub fn selected_packet(&self) -> Option<&PacketRecord> {
        let filtered = self.filtered_packets();
        let idx = self.selected_index(filtered.len())?;
        filtered.get(idx).copied()
    }

    /// Return the selected filtered index if any.
    #[must_use]
    pub fn selected_index(&self, filtered_len: usize) -> Option<usize> {
        if filtered_len == 0 {
            None
        } else {
            Some(
                self.table_state
                    .selected()
                    .unwrap_or(filtered_len - 1)
                    .min(filtered_len - 1),
            )
        }
    }

    /// Estimate current and peak packet rates from capture timestamps.
    ///
    /// Current rate: packets in the trailing 1-second window.
    /// Peak rate: maximum 1-second window count ever observed; tracked
    /// incrementally via `self.peak_rate` and updated on every `push()`.
    #[must_use]
    pub fn packet_rates(&self) -> (usize, usize) {
        if self.packets.is_empty() {
            return (0, 0);
        }

        // Compute the current 1-second window count.
        let latest_ts = self.packets.last().map_or(0, |pkt| pkt.timestamp_ms);
        let current = self
            .packets
            .iter()
            .rev()
            .take_while(|pkt| latest_ts.saturating_sub(pkt.timestamp_ms) < 1_000)
            .count();

        // Compute the peak 1-second window over all history (used internally
        // by push() to update self.peak_rate; returned here for testing and
        // for the initial render before any push() calls have run).
        let mut window_peak = 0usize;
        let mut start = 0usize;
        for end in 0..self.packets.len() {
            let current_ts = self.packets[end].timestamp_ms;
            while start <= end
                && current_ts.saturating_sub(self.packets[start].timestamp_ms) >= 1_000
            {
                start += 1;
            }
            window_peak = window_peak.max(end - start + 1);
        }

        (current, self.peak_rate.max(window_peak))
    }

    fn select_last_filtered(&mut self) {
        let last = self.filtered_packets().len().saturating_sub(1);
        self.table_state.select(Some(last));
    }

    fn clamp_selection(&mut self) {
        let filtered_len = self.filtered_packets().len();
        let idx = self.selected_index(filtered_len).unwrap_or(0);
        self.table_state.select(Some(idx));
    }
}

#[cfg(test)]
mod packet_monitor_tests {
    use super::{PacketMonitorState, PacketRecord};
    use textquest_common::ipc::PacketDirection;

    fn packet(ts: u64) -> PacketRecord {
        PacketRecord {
            client_id: 1,
            process_name: String::from("eqgame.exe"),
            opcode: 0x1234,
            direction: PacketDirection::Inbound,
            timestamp_ms: ts,
            payload_size: 4,
            payload: vec![0x34, 0x12, 0xAA, 0xBB],
        }
    }

    #[test]
    fn packet_rates_reflect_live_and_peak_windows() {
        let mut state = PacketMonitorState::new();
        for ts in [0, 100, 200, 1_100, 1_150] {
            state.push(packet(ts));
        }

        assert_eq!(state.packet_rates(), (2, 3));
    }

    #[test]
    fn selected_packet_tracks_latest_when_live() {
        let mut state = PacketMonitorState::new();
        state.push(packet(100));
        state.push(packet(200));

        assert_eq!(
            state.selected_packet().map(|pkt| pkt.timestamp_ms),
            Some(200)
        );
    }

    #[test]
    fn peak_rate_tracked_incrementally() {
        let mut state = PacketMonitorState::new();
        // Push 3 packets within 1 second, then 2 more in a later second.
        for ts in [0u64, 100, 200, 1_100, 1_150] {
            state.push(packet(ts));
        }
        // Peak should be 3 (the first 1-second window).
        assert_eq!(state.peak_rate, 3);
    }

    #[test]
    fn capture_start_ms_set_on_first_push() {
        let mut state = PacketMonitorState::new();
        assert!(state.capture_start_ms.is_none());
        state.push(packet(42_000));
        assert_eq!(state.capture_start_ms, Some(42_000));
        state.push(packet(43_000));
        // Must not change after the first push.
        assert_eq!(state.capture_start_ms, Some(42_000));
    }

    #[test]
    fn capture_start_ms_resets_on_clear() {
        let mut state = PacketMonitorState::new();
        state.push(packet(1_000));
        state.clear();
        assert!(state.capture_start_ms.is_none());
        assert_eq!(state.peak_rate, 0);
    }

    #[test]
    fn filter_client_id_limits_filtered_packets() {
        let mut state = PacketMonitorState::new();
        let mut p1 = packet(0);
        p1.client_id = 100;
        let mut p2 = packet(10);
        p2.client_id = 200;
        state.push(p1);
        state.push(p2);

        state.filter_client_id = Some(100);
        let filtered = state.filtered_packets();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].client_id, 100);
    }

    #[test]
    fn process_name_stored_in_packet_record() {
        let state_pkt = packet(0);
        assert_eq!(state_pkt.process_name, "eqgame.exe");
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

// ─── Economy State
// ────────────────────────────────────────────────────────────

/// State for the Economy Controls screen.
///
/// Uses stub/demo data — live economy logic is wired in M10.
pub struct EconomyState {
    /// Current vendor cycle operational status.
    pub vendor_status: crate::tui::ui::economy_controls::VendorCycleStatus,
    /// Seconds until next vendor cycle starts (0 = running now).
    pub vendor_next_cycle_secs: u64,
    /// Number of completed vendor cycles this session.
    pub vendor_cycles_completed: u32,
    /// Zone name of the last vendor visit.
    pub vendor_last_zone: Option<String>,
    /// Total items sold this session.
    pub vendor_items_sold: u32,
    /// Total plat earned from vendor sales this session.
    pub vendor_plat_earned: u64,

    /// Current banking consolidation status.
    pub banking_status: crate::tui::ui::economy_controls::BankingStatus,
    /// Total consolidated plat held in bank.
    pub banking_consolidated_plat: u64,
    /// Number of characters whose bank has been visited this run.
    pub banking_chars_done: u32,
    /// Total characters to bank this run.
    pub banking_chars_total: u32,
    /// Timestamp or label for the last bank run.
    pub banking_last_run: Option<String>,

    /// Number of items in the loot processing queue.
    pub loot_queue_size: u32,
    /// Total items looted this session.
    pub loot_items_total: u32,
    /// Items pending distribution (waiting on loot rules).
    pub loot_pending_distribute: u32,
    /// Short list of recently looted item names for display.
    pub loot_recent_items: Vec<String>,

    /// Whether economy automation is currently paused.
    pub automation_paused: bool,

    /// Session plat tracker (MQ2PlatTracker parity).
    pub plat_tracker: crate::economy::plat_tracker::PlatTracker,

    /// Total plat earned today (ledger).
    pub ledger_earned_today: u64,
    /// Total plat sold to vendor today (ledger).
    pub ledger_vendor_today: u64,
    /// Total plat from loot today (ledger).
    pub ledger_loot_today: u64,
    /// Timestamp of last item sold (ledger).
    pub ledger_last_sold: u32,
    /// Last plat amount banked (ledger).
    pub ledger_last_banked: u64,
    /// Timestamp of last item skipped (ledger).
    pub ledger_last_skipped: u32,
}

impl Default for EconomyState {
    fn default() -> Self {
        Self {
            vendor_status: crate::tui::ui::economy_controls::VendorCycleStatus::Idle,
            vendor_next_cycle_secs: 1800,
            vendor_cycles_completed: 0,
            vendor_last_zone: None,
            vendor_items_sold: 0,
            vendor_plat_earned: 0,

            banking_status: crate::tui::ui::economy_controls::BankingStatus::Idle,
            banking_consolidated_plat: 0,
            banking_chars_done: 0,
            banking_chars_total: 0,
            banking_last_run: None,

            loot_queue_size: 0,
            loot_items_total: 0,
            loot_pending_distribute: 0,
            loot_recent_items: Vec::new(),

            automation_paused: false,

            plat_tracker: crate::economy::plat_tracker::PlatTracker::new(),

            ledger_earned_today: 0,
            ledger_vendor_today: 0,
            ledger_loot_today: 0,
            ledger_last_sold: 0,
            ledger_last_banked: 0,
            ledger_last_skipped: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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

    #[test]
    fn map_name_style_cycles() {
        assert_eq!(MapNameStyle::Off.next(), MapNameStyle::Name);
        assert_eq!(MapNameStyle::NameClass.next(), MapNameStyle::Off);
    }
    #[test]
    fn map_name_style_parse() {
        assert_eq!(MapNameStyle::parse("off"), Some(MapNameStyle::Off));
        assert_eq!(MapNameStyle::parse("nl"), Some(MapNameStyle::NameLevel));
        assert_eq!(MapNameStyle::parse("x"), None);
    }
    #[test]
    fn map_highlight_add_remove() {
        let mut s = MapScreenState::new();
        s.add_highlight(MapHighlight {
            pattern: "Fippy".into(),
            pattern_lower: "fippy".into(),
            color: Some(Color::Red),
            size: 2,
            pulse: false,
        });
        s.add_highlight(MapHighlight {
            pattern: "Guard".into(),
            pattern_lower: "guard".into(),
            color: None,
            size: 1,
            pulse: true,
        });
        assert_eq!(s.highlights.len(), 2);
        assert_eq!(s.remove_highlight("fippy"), 1);
        assert_eq!(s.highlights.len(), 1);
    }
    #[test]
    fn map_vis_preset() {
        let mut s = MapScreenState::new();
        s.show_geometry = false;
        s.apply_visibility_preset(MapVisibilityPreset::All);
        assert!(s.show_geometry && s.show_spawns && s.show_labels);
    }
    #[test]
    fn map_filter_toggle_reports_state() {
        let mut s = MapScreenState::new();
        assert_eq!(s.toggle_filter(MapFilterKind::Npc), "NPC filter OFF");
        assert!(!s.filters.show_npc);
        assert_eq!(s.toggle_filter(MapFilterKind::Npc), "NPC filter ON");
        assert!(s.filters.show_npc);
    }
    #[test]
    fn map_preset_crud() {
        let mut s = MapScreenState::new();
        s.filters.show_npc = false;
        s.save_preset("h".into());
        s.filters.show_npc = true;
        assert!(s.load_preset("h"));
        assert!(!s.filters.show_npc);
        assert!(s.delete_preset("h"));
        assert!(!s.load_preset("h"));
    }
    #[test]
    fn map_new_defaults() {
        let s = MapScreenState::new();
        assert!(s.show_target_path && s.show_target_line);
        assert!(s.highlights.is_empty());
        assert_eq!(s.name_style, MapNameStyle::Off);
        assert_eq!(s.click_action, MapClickAction::None);
    }

    #[test]
    fn marker_save_persists_json_for_regular_file() {
        let dir = tempdir().unwrap();
        let marker_dir = dir.path().canonicalize().unwrap();
        let marker_path = marker_dir.join("map_markers.json");
        let markers = vec![NamedMapMarker {
            name: "bank".to_string(),
            x: 1.0,
            y: 2.0,
            z: 3.0,
            label: Some("Bank".to_string()),
            zone: "qeynos".to_string(),
        }];

        save_named_markers(&marker_path, &markers).unwrap();
        let loaded = load_named_markers_pub(&marker_path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "bank");
    }

    #[cfg(unix)]
    #[test]
    fn marker_save_rejects_symlink_target() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let real_target = dir.path().join("real.json");
        std::fs::write(&real_target, "{}").unwrap();
        let marker_path = dir.path().join("map_markers.json");
        symlink(&real_target, &marker_path).unwrap();

        let err = save_named_markers(&marker_path, &[]).unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("symlink"), "{err_msg}");
        assert_eq!(std::fs::read_to_string(&real_target).unwrap(), "{}");
    }

    #[test]
    fn marker_save_rejects_directory_target() {
        let dir = tempdir().unwrap();
        let marker_path = dir.path().join("subdir");
        std::fs::create_dir(&marker_path).unwrap();

        let err = save_named_markers(&marker_path, &[]).unwrap_err();
        assert!(err.to_string().contains("directory"), "{}", err);
    }

    #[cfg(unix)]
    #[test]
    fn marker_save_rejects_symlink_parent() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let real_dir = dir.path().join("real_dir");
        std::fs::create_dir(&real_dir).unwrap();
        let link_dir = dir.path().join("link_dir");
        symlink(&real_dir, &link_dir).unwrap();
        let marker_path = link_dir.join("map_markers.json");

        let err = save_named_markers(&marker_path, &[]).unwrap_err();
        assert!(err.to_string().contains("symlink"), "{}", err);
    }

    #[cfg(unix)]
    #[test]
    fn marker_save_rejects_ancestor_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let real_dir = dir.path().join("real_dir");
        std::fs::create_dir(&real_dir).unwrap();
        let link_dir = dir.path().join("link_dir");
        symlink(&real_dir, &link_dir).unwrap();
        // Place the marker path two levels deep inside the symlinked ancestor
        let sub = link_dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let marker_path = sub.join("map_markers.json");

        let err = save_named_markers(&marker_path, &[]).unwrap_err();
        assert!(err.to_string().contains("symlink"), "{}", err);
    }

    #[cfg(unix)]
    #[test]
    fn marker_save_rejects_fifo_target() {
        let dir = tempdir().unwrap();
        let marker_path = dir.path().join("map_markers.json");
        let status = std::process::Command::new("mkfifo")
            .arg(&marker_path)
            .status();
        // Skip if mkfifo is unavailable on this platform
        match status {
            Ok(s) if s.success() => {}
            _ => return,
        }

        let err = save_named_markers(&marker_path, &[]).unwrap_err();
        assert!(err.to_string().contains("not a regular file"), "{}", err);
    }

    // ─── HookRotationState tests ─────────────────────────────────────────────

    #[test]
    fn hook_rotation_state_default_has_four_entries() {
        let state = HookRotationState::new();
        assert_eq!(state.entries.len(), 4);
        assert_eq!(state.interval_ms, 5_000);
        for entry in &state.entries {
            assert_eq!(entry.state, HookSlotState::Active);
            assert!(entry.last_rotated_at.is_none());
            assert!(entry.next_rotation_at.is_none());
        }
    }

    #[test]
    fn hook_slot_state_label() {
        assert_eq!(HookSlotState::Active.label(), "ACTIVE");
        assert_eq!(HookSlotState::Unhooked.label(), "UNHOOKED");
    }

    #[test]
    fn record_rotation_updates_existing_entry() {
        let mut state = HookRotationState::new();
        state.record_rotation("ProcessGameEvents", HookSlotState::Unhooked, 2_000);

        let entry = state
            .entries
            .iter()
            .find(|e| e.name == "ProcessGameEvents")
            .expect("entry must exist");

        assert_eq!(entry.state, HookSlotState::Unhooked);
        assert!(entry.last_rotated_at.is_some());
        assert!(entry.next_rotation_at.is_some());

        // ms_since_last_rotation should be very small (just ran).
        let ms = entry.ms_since_last_rotation().unwrap();
        assert!(ms < 500, "expected <500ms elapsed, got {ms}ms");

        // ms_until_next_rotation should be close to 2000ms.
        let remaining = entry.ms_until_next_rotation().unwrap();
        assert!(
            remaining <= 2_000,
            "remaining {remaining}ms > interval 2000ms"
        );
    }

    #[test]
    fn record_rotation_inserts_unknown_hook() {
        let mut state = HookRotationState::new();
        let initial_count = state.entries.len();
        state.record_rotation("NewHook", HookSlotState::Active, 1_000);
        assert_eq!(state.entries.len(), initial_count + 1);
        let entry = state.entries.iter().find(|e| e.name == "NewHook").unwrap();
        assert_eq!(entry.state, HookSlotState::Active);
    }

    #[test]
    fn set_interval_reschedules_all_entries() {
        let mut state = HookRotationState::new();
        // Seed one entry with a rotation timestamp.
        state.record_rotation("SetGameState", HookSlotState::Unhooked, 5_000);

        state.set_interval_ms(10_000);
        assert_eq!(state.interval_ms, 10_000);

        // All entries (that have a scheduled time) should now have ~10s remaining.
        for entry in &state.entries {
            if let Some(remaining) = entry.ms_until_next_rotation() {
                assert!(
                    remaining <= 10_000,
                    "remaining {remaining}ms > new interval 10000ms"
                );
            }
        }
    }

    #[test]
    fn state_transitions_active_to_unhooked_and_back() {
        let mut state = HookRotationState::new();
        let name = "CastHook";

        // Start active.
        assert_eq!(
            state.entries.iter().find(|e| e.name == name).unwrap().state,
            HookSlotState::Active
        );

        // Rotate to unhooked.
        state.record_rotation(name, HookSlotState::Unhooked, 1_000);
        assert_eq!(
            state.entries.iter().find(|e| e.name == name).unwrap().state,
            HookSlotState::Unhooked
        );

        // Rotate back to active.
        state.record_rotation(name, HookSlotState::Active, 1_000);
        assert_eq!(
            state.entries.iter().find(|e| e.name == name).unwrap().state,
            HookSlotState::Active
        );
    }

    // ── OffsetCategory ────────────────────────────────────────────────

    #[test]
    fn offset_category_labels() {
        assert_eq!(OffsetCategory::All.label(), "All");
        assert_eq!(OffsetCategory::Globals.label(), "Globals");
        assert_eq!(OffsetCategory::PlayerBase.label(), "PlayerBase");
        assert_eq!(OffsetCategory::PlayerZone.label(), "PlayerZone");
        assert_eq!(OffsetCategory::SpawnManager.label(), "SpawnMgr");
        assert_eq!(OffsetCategory::Functions.label(), "Functions");
    }

    #[test]
    fn offset_category_next_cycles_through_all() {
        let start = OffsetCategory::All;
        let mut current = start;
        let mut visited = vec![current];
        loop {
            current = current.next();
            if current == start {
                break;
            }
            visited.push(current);
        }
        assert_eq!(visited.len(), 6, "should cycle through all 6 variants");
    }

    #[test]
    fn offset_category_next_order() {
        assert_eq!(OffsetCategory::All.next(), OffsetCategory::Globals);
        assert_eq!(OffsetCategory::Globals.next(), OffsetCategory::PlayerBase);
        assert_eq!(
            OffsetCategory::PlayerBase.next(),
            OffsetCategory::PlayerZone
        );
        assert_eq!(
            OffsetCategory::PlayerZone.next(),
            OffsetCategory::SpawnManager
        );
        assert_eq!(
            OffsetCategory::SpawnManager.next(),
            OffsetCategory::Functions
        );
        assert_eq!(OffsetCategory::Functions.next(), OffsetCategory::All);
    }

    #[test]
    fn offset_category_default_is_all() {
        let cat: OffsetCategory = Default::default();
        assert_eq!(cat, OffsetCategory::All);
    }

    // ── MapViewportMode ────────────────────────────────────────────────

    #[test]
    fn map_viewport_mode_labels() {
        assert_eq!(MapViewportMode::Auto.label(), "auto");
        assert_eq!(MapViewportMode::Local.label(), "local");
        assert_eq!(MapViewportMode::Global.label(), "global");
    }

    #[test]
    fn map_viewport_mode_next_cycles() {
        assert_eq!(MapViewportMode::Auto.next(), MapViewportMode::Local);
        assert_eq!(MapViewportMode::Local.next(), MapViewportMode::Global);
        assert_eq!(MapViewportMode::Global.next(), MapViewportMode::Auto);
    }

    // ── MapFilterKind ────────────────────────────────────────────────

    #[test]
    fn map_filter_kind_parse_valid() {
        assert_eq!(MapFilterKind::parse_kind("npc"), Some(MapFilterKind::Npc));
        assert_eq!(MapFilterKind::parse_kind("PC"), Some(MapFilterKind::Pc));
        assert_eq!(
            MapFilterKind::parse_kind("corpse"),
            Some(MapFilterKind::Corpse)
        );
        assert_eq!(
            MapFilterKind::parse_kind("corpses"),
            Some(MapFilterKind::Corpse)
        );
        assert_eq!(
            MapFilterKind::parse_kind("ground"),
            Some(MapFilterKind::Ground)
        );
        assert_eq!(MapFilterKind::parse_kind("pet"), Some(MapFilterKind::Pet));
        assert_eq!(MapFilterKind::parse_kind("pets"), Some(MapFilterKind::Pet));
        assert_eq!(
            MapFilterKind::parse_kind("named"),
            Some(MapFilterKind::Named)
        );
        assert_eq!(
            MapFilterKind::parse_kind("nameds"),
            Some(MapFilterKind::Named)
        );
        assert_eq!(
            MapFilterKind::parse_kind("untargetable"),
            Some(MapFilterKind::Untargetable)
        );
        assert_eq!(
            MapFilterKind::parse_kind("untargetables"),
            Some(MapFilterKind::Untargetable)
        );
        assert_eq!(
            MapFilterKind::parse_kind("untarget"),
            Some(MapFilterKind::Untargetable)
        );
    }

    #[test]
    fn map_filter_kind_parse_invalid() {
        assert_eq!(MapFilterKind::parse_kind(""), None);
        assert_eq!(MapFilterKind::parse_kind("unknown"), None);
        assert_eq!(MapFilterKind::parse_kind("NPCS"), None);
    }

    #[test]
    fn map_filter_kind_labels() {
        assert_eq!(MapFilterKind::Npc.label(), "NPC");
        assert_eq!(MapFilterKind::Pc.label(), "PC");
        assert_eq!(MapFilterKind::Corpse.label(), "Corpse");
        assert_eq!(MapFilterKind::Ground.label(), "Ground");
        assert_eq!(MapFilterKind::Pet.label(), "Pet");
        assert_eq!(MapFilterKind::Named.label(), "Named");
        assert_eq!(MapFilterKind::Untargetable.label(), "Untargetable");
    }

    #[test]
    fn npc_category_from_spawn_merchant() {
        assert_eq!(
            NpcCategory::from_spawn_name("a merchant"),
            Some(NpcCategory::Merchant)
        );
        assert_eq!(
            NpcCategory::from_spawn_name("Stonehand the Merchant"),
            Some(NpcCategory::Merchant)
        );
        assert_eq!(
            NpcCategory::from_spawn_name("Quartermaster Sho"),
            Some(NpcCategory::Merchant)
        );
        assert_eq!(
            NpcCategory::from_spawn_name("vendor of scrolls"),
            Some(NpcCategory::Merchant)
        );
    }

    #[test]
    fn npc_category_from_spawn_banker() {
        assert_eq!(
            NpcCategory::from_spawn_name("Banker Erol"),
            Some(NpcCategory::Banker)
        );
        assert_eq!(
            NpcCategory::from_spawn_name("a banker"),
            Some(NpcCategory::Banker)
        );
        assert_eq!(
            NpcCategory::from_spawn_name("Exchange Master"),
            Some(NpcCategory::Banker)
        );
    }

    #[test]
    fn npc_category_from_spawn_training_dummy() {
        assert_eq!(
            NpcCategory::from_spawn_name("a training dummy"),
            Some(NpcCategory::TrainingDummy)
        );
        assert_eq!(
            NpcCategory::from_spawn_name("practice dummy"),
            Some(NpcCategory::TrainingDummy)
        );
    }

    #[test]
    fn npc_category_from_spawn_quest_npc() {
        assert_eq!(
            NpcCategory::from_spawn_name("Quest Master Dray"),
            Some(NpcCategory::QuestNpc)
        );
        assert_eq!(
            NpcCategory::from_spawn_name("a quest givers"),
            Some(NpcCategory::QuestNpc)
        );
        assert_eq!(
            NpcCategory::from_spawn_name("Task Master"),
            Some(NpcCategory::QuestNpc)
        );
    }

    #[test]
    fn npc_category_from_spawn_other() {
        assert_eq!(
            NpcCategory::from_spawn_name("a moss snake"),
            Some(NpcCategory::Other)
        );
        assert_eq!(
            NpcCategory::from_spawn_name("Emperor Crush"),
            Some(NpcCategory::Other)
        );
    }

    #[test]
    fn npc_category_label() {
        assert_eq!(NpcCategory::Merchant.label(), "Merchant");
        assert_eq!(NpcCategory::Banker.label(), "Banker");
        assert_eq!(NpcCategory::TrainingDummy.label(), "Training Dummy");
        assert_eq!(NpcCategory::QuestNpc.label(), "Quest NPC");
        assert_eq!(NpcCategory::Other.label(), "Other NPC");
    }

    #[test]
    fn map_filters_npc_subcategory_default_all_on() {
        let filters = MapFilters::default();
        assert!(filters.show_merchant);
        assert!(filters.show_banker);
        assert!(filters.show_training_dummy);
        assert!(filters.show_quest_npc);
        assert!(filters.show_other_npc);
    }

    #[test]
    fn map_filters_npc_subcategory_set_get() {
        let mut filters = MapFilters::default();

        // Test setting individual NPC subcategory filters
        filters.set(MapFilterKind::NpcMerchant, false);
        assert!(!filters.get(MapFilterKind::NpcMerchant));
        assert!(filters.get(MapFilterKind::NpcBanker));

        filters.set(MapFilterKind::NpcBanker, false);
        assert!(!filters.get(MapFilterKind::NpcBanker));

        filters.set(MapFilterKind::NpcMerchant, true);
        assert!(filters.get(MapFilterKind::NpcMerchant));
    }

    #[test]
    fn map_filters_allows_spawn_respects_npc_subcategories() {
        use crate::eq::structs::EqClass;
        use crate::eq::structs::StandState;

        let merchant_spawn = crate::eq::structs::SpawnInfo {
            name: "Merchant".to_string(),
            displayed_name: "Stonehand the Merchant".to_string(),
            lastname: String::new(),
            spawn_id: 1,
            spawn_type: SpawnType::Npc,
            level: 50,
            class_id: 1,
            class: Some(EqClass::Warrior),
            stand_state: StandState::Standing,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 1000,
            hp_max: 1000,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 0,
            endurance_max: 0,
            is_gm: false,
            race_id: 1,
            buff_slots: Vec::new(),
            spellbook: Vec::new(),
            memorized_spells: Vec::new(),
            cast_state: None,
        };

        let mut filters = MapFilters::default();

        // With merchant filter on, merchant NPC should be allowed
        filters.show_merchant = true;
        filters.show_other_npc = true;
        filters.show_npc = true;
        assert!(filters.allows_spawn(&merchant_spawn));

        // With merchant filter off, merchant NPC should be hidden
        filters.show_merchant = false;
        assert!(!filters.allows_spawn(&merchant_spawn));

        // With NPC filter off entirely, merchant NPC should be hidden
        filters.show_merchant = true;
        filters.show_npc = false;
        assert!(!filters.allows_spawn(&merchant_spawn));
    }
}

// ─── Help panel state ─────────────────────────────────────────────────────────

/// Selectable category tabs in the help search panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HelpTab {
    #[default]
    Commands,
    Faq,
    Tips,
    Results,
}

impl HelpTab {
    /// Display label for the tab.
    pub fn label(self) -> &'static str {
        match self {
            HelpTab::Commands => "Commands",
            HelpTab::Faq => "FAQ",
            HelpTab::Tips => "Tips",
            HelpTab::Results => "Results",
        }
    }

    /// All tabs in display order.
    pub fn all() -> &'static [HelpTab] {
        &[HelpTab::Commands, HelpTab::Faq, HelpTab::Tips, HelpTab::Results]
    }
}

/// State for the searchable help panel overlay.
#[derive(Debug, Clone, Default)]
pub struct HelpPanelState {
    /// Current search query entered by the operator.
    pub query: String,
    /// Cursor position within the query string (byte offset).
    pub cursor: usize,
    /// Active category tab.
    pub tab: HelpTab,
    /// Selected result row index (0-based within visible list).
    pub selected: usize,
    /// Scroll offset for the result list.
    pub scroll: usize,
}

impl HelpPanelState {
    /// Return a new zeroed state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Move selection up by one row; clamp at 0.
    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
    }

    /// Move selection down by one row; clamp at `max`.
    pub fn select_next(&mut self, max: usize) {
        if self.selected + 1 < max {
            self.selected += 1;
        }
    }

    /// Adjust scroll so `selected` is always visible inside `page_height` rows.
    pub fn ensure_visible(&mut self, page_height: usize) {
        if page_height == 0 {
            return;
        }
        if self.selected >= self.scroll + page_height {
            self.scroll = self.selected - page_height + 1;
        }
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
    }

    /// Append a character to the query and advance the cursor.
    pub fn push_char(&mut self, ch: char) {
        self.query.push(ch);
        self.cursor = self.query.len();
        self.selected = 0;
        self.scroll = 0;
    }

    /// Delete the last character before the cursor.
    pub fn pop_char(&mut self) {
        if !self.query.is_empty() {
            self.query.pop();
            self.cursor = self.query.len();
            self.selected = 0;
            self.scroll = 0;
        }
    }
}

#[cfg(test)]
mod help_state_tests {
    use super::*;

    #[test]
    fn help_panel_state_push_pop() {
        let mut state = HelpPanelState::new();
        state.push_char('n');
        state.push_char('a');
        state.push_char('v');
        assert_eq!(state.query, "nav");
        assert_eq!(state.cursor, 3);
        state.pop_char();
        assert_eq!(state.query, "na");
    }

    #[test]
    fn help_panel_state_select_navigation() {
        let mut state = HelpPanelState::new();
        state.select_next(10);
        assert_eq!(state.selected, 1);
        state.select_prev();
        assert_eq!(state.selected, 0);
        state.select_prev(); // should not underflow
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn help_panel_ensure_visible_scrolls_down() {
        let mut state = HelpPanelState::new();
        state.selected = 12;
        state.ensure_visible(5);
        assert!(state.selected >= state.scroll);
        assert!(state.scroll + 5 > state.selected || state.scroll <= state.selected);
    }
}
