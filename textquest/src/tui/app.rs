use std::collections::{HashMap, VecDeque};

use super::cast::{CastDisplay, live_cast_display, short_cast_label};
use super::command::{self, HelpSection};
use super::config_panel::ConfigPanelState;
use super::demo_data::{DemoRole, demo_client_cast_info, demo_client_profile};
use super::menu::MenuState;
use super::theme::{Theme, ThemeKind};
use super::ui::ch_chain::{
    CastState as ChPanelCastState, ChChainPanelState, ChainCleric, ChainStats,
};
use super::wizard::WizardState;
use crate::camp::config::CampConfig;
use crate::camp::state::{CampMember, Role};
use crate::config::AccountsConfig;
use crate::eq::log_parser::{ChatEvent, LootDatabase};
use crate::eq::log_watcher::LogWatcher;
use crate::eq::named_db::NamedMobDatabase;
use crate::eq::named_tracker::{NamedAlert, NamedTracker};
use crate::eq::spawn_alert::{MatchSource, SpawnAlertEvent, SpawnAlertFeed};
use crate::eq::structs::{SpawnInfo, SpawnType};
use crate::orchestrator::Orchestrator;
use crate::soul::coordinator::SoulCoordinator;
use anyhow::Context;
use ratatui::style::Color;

// Re-export extracted types so existing `use tui::app::*` paths still work.
pub use super::client::ClientState;
pub use super::state::{
    CommandBarState, HexDumpState, MapScreenState, MapViewportMode, NavigationScreenState,
    OverviewScreenState, PacketMonitorState, SpawnsScreenState, TacticalScreenState,
};
use super::state::{
    FilteredSpawnCache, FilteredSpawnCacheKey, MapClickAction, MapFilterKind, MapHighlight,
    MapLocMarker, MapNameStyle, MapRadiusOverlay, MapSpawnPresentationCache, MapVisibilityPreset,
    NamedMapMarker,
};
use super::state::{load_named_markers_pub, save_named_markers};

/// Which screen is currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveScreen {
    /// Character roster and group overview.
    Overview,
    /// Map and spawn list tactical view.
    Tactical,
    /// Waypoint navigation management.
    Navigation,
    /// Debug panels (raw spawns, hex dump).
    Debug,
    /// Packet/opcode monitor — live network sniffer.
    PacketMonitor,
}

impl ActiveScreen {
    /// Returns the human-readable label for this screen tab.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Overview => "Characters",
            Self::Tactical => "Map",
            Self::Navigation => "Navigation",
            Self::Debug => "Debug",
            Self::PacketMonitor => "Packets",
        }
    }

    /// All screen variants for iteration.
    pub const ALL: [ActiveScreen; 5] = [
        Self::Overview,
        Self::Tactical,
        Self::Navigation,
        Self::Debug,
        Self::PacketMonitor,
    ];
}

/// Which panel is currently focused for keyboard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    /// Character roster list on the overview screen.
    OverviewRoster,
    /// Selected character detail panel.
    OverviewCharacter,
    /// Group membership panel.
    OverviewGroups,
    /// Spawn filter/scope panel.
    OverviewFilters,
    /// Combat status panel (assist, CH chain).
    OverviewCombat,
    /// Session statistics panel (uptime, loot).
    OverviewSession,
    /// Heal/buff/debuff priority queue panel.
    OverviewPriorities,
    /// Zone map display on tactical screen.
    TacticalMap,
    /// Spawn list on tactical screen.
    TacticalSpawns,
    /// Named mob tracker on tactical screen.
    TacticalNamed,
    /// Navigation waypoints panel.
    TacticalNavigation,
    /// Raw spawn data table (debug).
    DebugSpawns,
    /// Memory hex dump panel (debug).
    DebugHexDump,
    /// Ghidra offset explorer panel (debug).
    DebugExplorer,
    /// Packet monitor scrolling log.
    PacketMonitorLog,
    /// EQ Internals offset browser panel (debug).
    DebugInternals,
}

/// Layout preset for panel arrangement within a screen.
///
/// Each screen supports 2-3 layout variants. `Ctrl+E` cycles through them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutPreset {
    /// Default arrangement (typically top/bottom or balanced).
    #[default]
    Default,
    /// Alternate arrangement (e.g., left/right split or single-dominant).
    Alternate,
    /// Third option — screen-specific meaning (e.g., focused/minimized).
    Compact,
}

impl LayoutPreset {
    /// Cycle to the next preset.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Default => Self::Alternate,
            Self::Alternate => Self::Compact,
            Self::Compact => Self::Default,
        }
    }

    /// Human-readable label for the status bar.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Alternate => "Wide",
            Self::Compact => "Compact",
        }
    }
}

/// Spawn type filter for the spawn list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnFilter {
    /// Show all spawn types.
    All,
    /// Show only player characters.
    Pc,
    /// Show only non-player characters.
    Npc,
    /// Show only named (rare) mobs.
    Named,
}

impl SpawnFilter {
    /// Cycles to the next filter variant.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Pc,
            Self::Pc => Self::Npc,
            Self::Npc => Self::Named,
            Self::Named => Self::All,
        }
    }

    /// Returns the display label for this filter.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Pc => "PC",
            Self::Npc => "NPC",
            Self::Named => "Named",
        }
    }
}

/// Sort column for the spawn list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpawnSort {
    /// No explicit sort — use original spawn order.
    Default,
    /// Sort alphabetically by name.
    Name,
    /// Sort by class abbreviation.
    Class,
    /// Sort by level.
    Level,
    /// Sort by distance from local player.
    Distance,
}

impl SpawnSort {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Default => Self::Name,
            Self::Name => Self::Class,
            Self::Class => Self::Level,
            Self::Level => Self::Distance,
            Self::Distance => Self::Default,
        }
    }

    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Name => "Name",
            Self::Class => "Class",
            Self::Level => "Level",
            Self::Distance => "Dist",
        }
    }
}

/// Navigation target scope for spawn nav-to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavScope {
    Active,
    Group,
    All,
}

impl NavScope {
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Active => Self::Group,
            Self::Group => Self::All,
            Self::All => Self::Active,
        }
    }

    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Group => "Group",
            Self::All => "All",
        }
    }
}

/// Status of a user-tracked spawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackedStatus {
    /// Spawn is currently alive in the zone.
    Up,
    /// Spawn has despawned or been killed.
    Down,
    /// Spawn status cannot be determined.
    Unknown,
}

impl TrackedStatus {
    /// Returns the short status label (UP, DOWN, ???).
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Up => "UP",
            Self::Down => "DOWN",
            Self::Unknown => "???",
        }
    }

    /// Returns the color for rendering this status.
    #[must_use]
    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            Self::Up => Color::Green,
            Self::Down => Color::Red,
            Self::Unknown => Color::DarkGray,
        }
    }
}

/// A user-tracked spawn (via :track command).
#[derive(Debug, Clone)]
pub struct TrackedSpawn {
    /// Spawn name being tracked.
    pub name: String,
    /// Current up/down/unknown status.
    pub status: TrackedStatus,
    /// Tick count when this spawn was last seen alive.
    pub last_seen_tick: Option<u64>,
    /// Last known X coordinate.
    pub last_x: f32,
    /// Last known Y coordinate.
    pub last_y: f32,
    /// Last known Z coordinate.
    pub last_z: f32,
}

/// Definition for a logical group of accounts.
#[derive(Debug, Clone)]
pub struct GroupDef {
    /// Numeric group identifier (1-based).
    pub id: u8,
    /// Human-readable group name (e.g., "Alpha").
    pub name: String,
    /// Inclusive range of account numbers in this group.
    pub account_range: (u8, u8),
    /// Default camp assignment for this group.
    #[allow(dead_code)]
    pub default_camp: String,
}

/// A group built dynamically from live EQ `GroupInfo` data.
#[derive(Debug, Clone)]
pub struct LiveGroup {
    /// Group leader name.
    pub leader: String,
    /// Member names and their connected `ClientState` index (if any).
    pub member_names: Vec<String>,
    /// Zone the leader (or majority of members) is in.
    pub zone: String,
}

/// Cached CH chain status for TUI display (avoids reaching into Orchestrator).
#[derive(Clone, Debug)]
pub struct ChChainStatus {
    /// Number of clerics in the chain.
    pub members: usize,
    /// Interval between heals in seconds.
    pub interval_secs: f32,
    /// Whether the chain dynamically adjusts timing.
    pub is_adaptive: bool,
    /// Spawn ID of the heal target.
    pub target_id: u32,
}

/// Help overlay jump target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpFocus {
    Section(HelpSection),
    Command(&'static str),
}

/// Severity level for the transient toast lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastLevel {
    #[must_use]
    pub fn ttl_ticks(self) -> u64 {
        match self {
            Self::Info | Self::Success => 24,
            Self::Warning => 40,
            Self::Error => 56,
        }
    }
}

/// Transient notification shown above the main UI chrome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub message: String,
    pub level: ToastLevel,
    pub set_tick: u64,
    pub ttl_ticks: u64,
}

/// Application state for the TUI command center.
pub struct App {
    /// Whether the application is still running (false triggers shutdown).
    pub running: bool,
    /// Currently displayed screen tab.
    pub active_screen: ActiveScreen,
    /// Currently focused panel for keyboard input.
    pub active_panel: ActivePanel,
    /// Per-screen layout presets (cycled with Ctrl+E).
    pub layout_presets: [LayoutPreset; 5],

    /// Connected EQ client states.
    pub clients: Vec<ClientState>,
    /// Index of the currently selected client in `clients`.
    pub selected_client: usize,

    /// Configured group definitions (6 groups of 6 accounts each).
    pub groups: Vec<GroupDef>,

    /// Active group focus: `None` = aggregate view, `Some(idx)` = single group.
    pub active_group: Option<usize>,

    /// Explicit routing scope for command dispatch (M8 Orchestrator).
    ///
    /// Controls which clients receive commands sent from the TUI command bar.
    /// Updated by `:scope`, `:G<n>`, and character-prefix commands.
    pub routing_scope: textquest_common::routing::RoutingScope,

    /// EQ server name from config.
    pub server_name: String,

    /// Legacy: local player from the selected client (kept for backward compat).
    pub local_player: Option<SpawnInfo>,
    /// Legacy: target of the selected client.
    pub target: Option<SpawnInfo>,
    /// Legacy: spawn list from the selected client.
    pub spawns: Vec<SpawnInfo>,
    /// Last selected-client spawn revision copied into `spawns`.
    synced_spawn_revision: Option<(u32, u64)>,
    /// Cached filtered spawn indices for the selected client.
    filtered_spawn_cache: FilteredSpawnCache,
    /// Cached tactical-map spawn overlay cells.
    pub(crate) map_spawn_cache: MapSpawnPresentationCache,
    /// Status bar message displayed at the bottom of the TUI.
    pub status_message: String,
    /// Monotonic tick counter incremented each refresh cycle.
    pub tick_count: u64,

    /// Overview screen UI state (collapse flags, selection).
    pub overview_state: OverviewScreenState,
    /// Debug spawn list UI state (table selection, filters).
    pub spawns_state: SpawnsScreenState,
    /// Hex dump panel state (address, cursor).
    pub hex_state: HexDumpState,
    /// Ghidra offset explorer state.
    pub explorer_state: super::state::ExplorerScreenState,
    /// EQ Internals offset browser state.
    pub eq_internals_state: super::state::EqInternalsState,
    /// Ghidra database handle (loaded from data/ghidra.db if present).
    pub ghidra_db: Option<textquest_common::ghidra_db::GhidraDatabase>,

    /// TUI refresh interval in milliseconds.
    pub refresh_rate_ms: u64,

    /// EQ module base address (legacy, from first attached client).
    pub eq_base: u64,
    /// PID of the attached EQ process (legacy, first client).
    pub attached_pid: Option<u32>,

    /// Soul Engine coordinator for LLM-driven character personalities.
    pub soul_coordinator: Option<SoulCoordinator>,
    /// Tick counter for soul engine update throttling.
    pub soul_tick_counter: u64,

    /// Map panel state (zoom, pan, overlays).
    pub map_state: MapScreenState,
    /// Tactical screen state (named/nav panel visibility, collapse flags).
    pub tactical_state: TacticalScreenState,

    /// Privacy mode — hides character names and server for screenshots.
    pub privacy_mode: bool,

    /// Command bar state (input text, history, visibility).
    pub cmd_state: CommandBarState,

    /// Named mob tracker for rare spawn monitoring.
    pub named_tracker: NamedTracker,

    /// Spawn alert feed.
    pub spawn_alert_feed: SpawnAlertFeed,
    /// Whether to auto-alert on named NPC spawns.
    pub spawn_watch_named: bool,

    /// User-tracked spawns registered via the `:track` command.
    pub tracked_spawns: HashMap<String, TrackedSpawn>,

    /// Whether the help overlay is currently visible.
    pub help_visible: bool,
    pub help_scroll: usize,
    /// Optional jump target applied the next time help is drawn.
    pub help_focus: Option<HelpFocus>,

    /// Current operating mode (camp or hunt).
    pub operating_mode: crate::camp::hunt::OperatingMode,

    /// Name of the main assist character, if set.
    pub main_assist: Option<String>,
    /// Name of the main tank character, if set.
    pub main_tank: Option<String>,

    /// Whether heal-cancel is enabled (cleric ducks on high HP during cast).
    pub heal_cancel_enabled: bool,

    /// Cached CH chain status (updated each tick from Orchestrator).
    pub ch_chain_status: Option<ChChainStatus>,

    /// Account configuration for login automation.
    pub accounts_config: Option<AccountsConfig>,

    /// Accumulated loot data from log parsing.
    pub loot_database: LootDatabase,
    /// Per-client log file watchers for chat/loot events.
    pub log_watchers: Vec<LogWatcher>,
    /// Timestamp when this TUI session started.
    pub session_start: std::time::Instant,
    /// Ring buffer of recent chat events (capped at 200).
    pub chat_events: VecDeque<ChatEvent>,

    /// Navigation screen state (waypoint list, route display).
    pub nav_state: NavigationScreenState,
    /// Packet/opcode monitor state.
    pub packet_monitor_state: PacketMonitorState,

    /// EQ install path for launch operations (from config or default).
    pub launch_eq_path: String,

    /// Active theme variant identifier.
    pub theme_kind: ThemeKind,
    /// Resolved theme colors and styles.
    pub theme: Theme,

    /// Discord webhook sender for alerts, if configured.
    pub discord_webhook: Option<crate::discord::webhook::WebhookSender>,
    /// Discord bridge for bidirectional chat relay.
    pub discord_bridge: Option<crate::discord::bridge::TuiBridge>,
    /// Normalized sender allowlist for Discord bridge command execution.
    pub discord_command_allowed_senders: std::collections::HashSet<String>,

    /// Dropdown menu bar state.
    pub menu_state: MenuState,
    /// Onboarding wizard state.
    pub wizard_state: WizardState,
    /// Configuration panel state.
    pub config_panel_state: ConfigPanelState,
    /// CH chain configuration panel state.
    pub ch_chain_panel_state: ChChainPanelState,

    /// Command aliases mapping (e.g., "h" → "help", "q" → "quit").
    pub command_aliases: HashMap<String, String>,
    /// Transient toast feedback shown above the main chrome.
    pub toast: Option<Toast>,

    /// Whether operator has paused all automation (HOME to pause, END to resume).
    pub automation_paused: bool,

    /// Live priority queue snapshots per character (updated each tick).
    pub priority_snapshots: Vec<super::priorities::PrioritySnapshot>,
}

/// Navigation status for a single client.
#[derive(Debug, Clone)]
pub struct NavClientStatus {
    /// Name of the navigation destination.
    pub destination: String,
    /// Current navigator state (idle, moving, stuck, arrived).
    pub status: textquest_common::nav::NavStatus,
    /// Estimated time of arrival in seconds, if calculable.
    #[allow(dead_code)]
    pub eta_secs: Option<u32>,
    /// Active navigation waypoints for map overlay rendering.
    pub waypoints: Vec<textquest_common::nav::Waypoint>,
    /// Whether a navmesh-backed path exists.
    pub path_exists: bool,
    /// Total planned path length (world units), if known.
    pub path_length: Option<f32>,
    /// Human-readable reason when pathfinding failed.
    pub failure_reason: Option<String>,
    /// Human-readable route selection or wait state.
    pub route_state: String,
    /// Human-readable recovery state when navigation is blocked or stuck.
    pub recovery_state: Option<String>,
    /// Operator-visible blockers that explain why travel is waiting or degraded.
    pub blockers: Vec<String>,
    /// Whether this status was injected by the deterministic demo script.
    pub is_demo_scripted: bool,
}

impl NavClientStatus {
    /// Short progress label for the current navigation status.
    #[must_use]
    pub fn progress_summary(&self) -> String {
        match &self.status {
            textquest_common::nav::NavStatus::Idle => String::from("Standing by"),
            textquest_common::nav::NavStatus::Moving {
                waypoint_index,
                waypoint_count,
                distance_remaining,
            } => format!(
                "WP {}/{} • {:.0}u remaining",
                waypoint_index.saturating_add(1),
                (*waypoint_count).max(1),
                distance_remaining
            ),
            textquest_common::nav::NavStatus::Paused {
                reason,
                waypoint_index,
                waypoint_count,
                distance_remaining,
            } => {
                let reason_label = match reason {
                    textquest_common::nav::PauseReason::Warp => "warp",
                    textquest_common::nav::PauseReason::UserPause => "paused",
                    textquest_common::nav::PauseReason::UserInput => "user input",
                    textquest_common::nav::PauseReason::GmNearby => "GM nearby",
                };
                format!(
                    "Paused ({reason_label}) • WP {}/{} • {:.0}u remaining",
                    waypoint_index.saturating_add(1),
                    (*waypoint_count).max(1),
                    distance_remaining
                )
            }
            textquest_common::nav::NavStatus::Stuck { recovery_attempt } => {
                format!("Recovery attempt {}", recovery_attempt)
            }
            textquest_common::nav::NavStatus::Arrived => String::from("Destination reached"),
            textquest_common::nav::NavStatus::Following {
                leader_name,
                distance_to_anchor,
                returning,
            } => {
                if *returning {
                    format!("Returning to {leader_name} ({distance_to_anchor:.0}u)")
                } else {
                    format!("Following {leader_name} ({distance_to_anchor:.0}u)")
                }
            }
            textquest_common::nav::NavStatus::Sticking {
                target_id,
                distance,
                in_range,
            } => {
                if *in_range {
                    format!("Sticking #{target_id} • {distance:.0}u (in range)")
                } else {
                    format!("Sticking #{target_id} • {distance:.0}u")
                }
            }
        }
    }

    /// Single-line blocker summary suitable for narrow cards and tables.
    #[must_use]
    pub fn blocker_summary(&self) -> Option<String> {
        let mut reasons = Vec::new();
        if let Some(reason) = &self.failure_reason {
            reasons.push(reason.clone());
        }
        reasons.extend(self.blockers.clone());

        if reasons.is_empty() {
            None
        } else {
            Some(reasons.join(" | "))
        }
    }
}

struct FocusedNavClient {
    pid: u32,
    client_name: String,
    zone_short: String,
    position: (f32, f32, f32),
    is_demo: bool,
}

impl App {
    /// Create a new TUI application with default state.
    #[must_use]
    pub fn new() -> Self {
        let mut app = Self {
            running: true,
            active_screen: ActiveScreen::Overview,
            active_panel: ActivePanel::OverviewRoster,
            layout_presets: [LayoutPreset::Default; 5],

            clients: Vec::new(),
            selected_client: 0,
            active_group: None,
            groups: Self::build_default_groups(),
            routing_scope: textquest_common::routing::RoutingScope::AllSession,

            server_name: String::from("Firiona Vie"),

            local_player: None,
            target: None,
            spawns: Vec::new(),
            synced_spawn_revision: None,
            filtered_spawn_cache: FilteredSpawnCache::default(),
            map_spawn_cache: MapSpawnPresentationCache::default(),
            status_message: String::from("Waiting for EQ process..."),
            tick_count: 0,

            overview_state: OverviewScreenState::new(),
            spawns_state: SpawnsScreenState::new(),
            hex_state: HexDumpState::new(),
            explorer_state: super::state::ExplorerScreenState::new(),
            eq_internals_state: super::state::EqInternalsState::new(),
            ghidra_db: None,

            refresh_rate_ms: 250,

            eq_base: 0,
            attached_pid: None,

            soul_coordinator: None,
            soul_tick_counter: 0,

            map_state: MapScreenState::new(),
            tactical_state: TacticalScreenState::new(),

            privacy_mode: false,

            cmd_state: CommandBarState::new(),

            named_tracker: {
                let db = NamedMobDatabase::load(std::path::Path::new("config/named_mobs")).ok();
                match db {
                    Some(db) => NamedTracker::with_db(db),
                    None => NamedTracker::new(),
                }
            },
            spawn_alert_feed: SpawnAlertFeed::new(200),
            spawn_watch_named: true,
            tracked_spawns: HashMap::new(),

            help_visible: false,
            help_scroll: 0,
            help_focus: None,

            operating_mode: crate::camp::hunt::OperatingMode::Camp,

            main_assist: None,
            main_tank: None,
            heal_cancel_enabled: true,
            ch_chain_status: None,

            accounts_config: AccountsConfig::load(std::path::Path::new("config/accounts.toml"))
                .ok(),

            loot_database: LootDatabase::new(),
            log_watchers: Vec::new(),
            session_start: std::time::Instant::now(),
            chat_events: VecDeque::with_capacity(200),

            nav_state: NavigationScreenState::new(),
            packet_monitor_state: PacketMonitorState::new(),

            launch_eq_path: String::from(r"C:\EverQuest"),

            theme_kind: ThemeKind::load_saved(),
            theme: ThemeKind::load_saved().build(),

            discord_webhook: None,
            discord_bridge: None,
            discord_command_allowed_senders: std::collections::HashSet::new(),

            menu_state: MenuState::new(),
            wizard_state: WizardState::new(),
            config_panel_state: ConfigPanelState::new(),
            ch_chain_panel_state: ChChainPanelState::new(),

            command_aliases: Self::build_default_aliases(),
            toast: None,
            automation_paused: false,
            priority_snapshots: Vec::new(),
        };
        app.cmd_state.load_history_from_disk();
        app
    }

    /// Build default command aliases.
    fn build_default_aliases() -> HashMap<String, String> {
        let mut aliases = HashMap::new();
        for entry in command::command_entries() {
            for alias in entry.aliases {
                aliases.insert((*alias).to_string(), entry.phrase.to_string());
            }
        }
        aliases
    }

    /// Open the help overlay and optionally jump to a section or command.
    pub fn open_help(&mut self, focus: HelpFocus) {
        self.help_visible = true;
        self.help_focus = Some(focus);
    }

    /// Set a transient toast notification message.
    pub fn set_toast(&mut self, level: ToastLevel, msg: impl Into<String>) {
        let message = msg.into();
        let ttl_ticks = level.ttl_ticks();
        if let Some(toast) = self.toast.as_mut()
            && toast.level == level
            && toast.message == message
        {
            toast.set_tick = self.tick_count;
            toast.ttl_ticks = ttl_ticks;
            return;
        }
        self.toast = Some(Toast {
            message,
            level,
            set_tick: self.tick_count,
            ttl_ticks,
        });
    }

    /// Update the status line and optionally elevate the same message to a toast.
    pub fn set_feedback(&mut self, level: ToastLevel, msg: impl Into<String>, show_toast: bool) {
        let message = msg.into();
        self.status_message = message.clone();
        if show_toast {
            self.set_toast(level, message);
        }
    }

    /// Clear expired toast messages.
    pub fn clear_expired_toast(&mut self) {
        if self
            .toast
            .as_ref()
            .is_some_and(|toast| self.tick_count.saturating_sub(toast.set_tick) > toast.ttl_ticks)
        {
            self.toast = None;
        }
    }

    /// Build a cast-strip display model for a client, when actively casting.
    #[must_use]
    pub fn client_cast_display(&self, client: &ClientState) -> Option<CastDisplay> {
        if client.is_demo {
            let name = if !client.character_name.is_empty() {
                client.character_name.as_str()
            } else {
                client
                    .local_player
                    .as_ref()
                    .map_or("", |player| player.displayed_name.as_str())
            };
            if let Some(cast) =
                demo_client_cast_info(name, client.pid, self.tick_count, self.refresh_rate_ms)
            {
                return Some(CastDisplay::exact_progress(
                    cast.spell_label,
                    short_cast_label(cast.spell_label),
                    f64::from(cast.progress),
                    cast.total_cast_ms as f32 / 1000.0,
                ));
            }
        }

        let player = client.local_player.as_ref()?;
        let cast = player.cast_state.as_ref()?;
        if !cast.is_casting() {
            return None;
        }

        Some(live_cast_display(
            cast,
            player.class,
            self.tick_count,
            client.pid,
        ))
    }

    /// Initialize Discord integration from config.
    pub fn init_discord(&mut self, config: &crate::config::DiscordConfig) {
        self.discord_command_allowed_senders = config
            .command_allowed_senders
            .iter()
            .map(|sender| sender.trim().to_ascii_lowercase())
            .filter(|sender| !sender.is_empty())
            .collect();
        if !config.webhook_url.is_empty() || !config.channels.is_empty() {
            tracing::info!("Discord webhook enabled");
            self.discord_webhook = Some(crate::discord::webhook::WebhookSender::with_channels(
                config.webhook_url.clone(),
                config.channels.clone(),
            ));
        }
    }

    /// Send a Discord alert if webhook is configured.
    #[allow(dead_code)] // Called from alert sites as they're wired up
    pub fn discord_alert(
        &self,
        title: &str,
        message: &str,
        level: crate::discord::webhook::AlertLevel,
    ) {
        if let Some(ref webhook) = self.discord_webhook {
            webhook.send(crate::discord::webhook::DiscordAlert::simple(
                title,
                message,
                level,
                crate::discord::webhook::EventCategory::Status,
            ));
        }
    }

    /// Cycle to the next theme.
    pub fn cycle_theme(&mut self) {
        self.theme_kind = self.theme_kind.next();
        self.theme = self.theme_kind.build();
        self.theme_kind.save();
    }

    fn default_panel_for_screen(screen: ActiveScreen) -> ActivePanel {
        match screen {
            ActiveScreen::Overview => ActivePanel::OverviewRoster,
            ActiveScreen::Tactical => ActivePanel::TacticalMap,
            ActiveScreen::Navigation => ActivePanel::TacticalNavigation,
            ActiveScreen::Debug => ActivePanel::DebugSpawns,
            ActiveScreen::PacketMonitor => ActivePanel::PacketMonitorLog,
        }
    }

    fn visible_panels(&self) -> Vec<ActivePanel> {
        match self.active_screen {
            ActiveScreen::Overview => {
                let mut panels = vec![ActivePanel::OverviewRoster, ActivePanel::OverviewCharacter];
                if self.overview_state.show_groups {
                    panels.push(ActivePanel::OverviewGroups);
                }
                if self.overview_state.show_filters {
                    panels.push(ActivePanel::OverviewFilters);
                }
                panels.push(ActivePanel::OverviewCombat);
                if !self.priority_snapshots.is_empty() {
                    panels.push(ActivePanel::OverviewPriorities);
                }
                panels.push(ActivePanel::OverviewSession);
                panels
            }
            ActiveScreen::Tactical => {
                let mut panels = vec![ActivePanel::TacticalMap, ActivePanel::TacticalSpawns];
                if self.tactical_state.show_named {
                    panels.push(ActivePanel::TacticalNamed);
                }
                if self.tactical_state.show_navigation {
                    panels.push(ActivePanel::TacticalNavigation);
                }
                panels
            }
            ActiveScreen::Navigation => vec![ActivePanel::TacticalNavigation],
            ActiveScreen::Debug => {
                vec![
                    ActivePanel::DebugSpawns,
                    ActivePanel::DebugHexDump,
                    ActivePanel::DebugExplorer,
                    ActivePanel::DebugInternals,
                ]
            }
            ActiveScreen::PacketMonitor => vec![ActivePanel::PacketMonitorLog],
        }
    }

    /// Returns `true` if the given panel currently has keyboard focus.
    pub fn is_panel_focused(&self, panel: ActivePanel) -> bool {
        self.active_panel == panel
    }

    /// Switches to the given screen and resets panel focus.
    pub fn set_active_screen(&mut self, screen: ActiveScreen) {
        self.active_screen = screen;
        self.active_panel = Self::default_panel_for_screen(screen);
        self.ensure_panel_focus();
    }

    /// Ensures the active panel is visible; resets to first visible if not.
    pub fn ensure_panel_focus(&mut self) {
        let visible = self.visible_panels();
        if !visible.contains(&self.active_panel)
            && let Some(panel) = visible.first().copied()
        {
            self.active_panel = panel;
        }
    }

    /// Cycles focus to the next visible panel on the current screen.
    pub fn toggle_panel(&mut self) {
        let visible = self.visible_panels();
        if visible.is_empty() {
            return;
        }

        let current = visible
            .iter()
            .position(|panel| *panel == self.active_panel)
            .unwrap_or(0);
        self.active_panel = visible[(current + 1) % visible.len()];
    }

    /// Returns the current layout preset for the active screen.
    #[must_use]
    pub fn current_layout(&self) -> LayoutPreset {
        let idx = self.screen_index(self.active_screen);
        self.layout_presets[idx]
    }

    /// Cycle the layout preset for the active screen (Ctrl+E).
    pub fn cycle_layout(&mut self) {
        let idx = self.screen_index(self.active_screen);
        self.layout_presets[idx] = self.layout_presets[idx].next();
    }

    fn screen_index(&self, screen: ActiveScreen) -> usize {
        match screen {
            ActiveScreen::Overview => 0,
            ActiveScreen::Tactical => 1,
            ActiveScreen::Navigation => 2,
            ActiveScreen::Debug => 3,
            ActiveScreen::PacketMonitor => 4,
        }
    }

    /// Toggles the group section visibility on the overview screen.
    pub fn toggle_groups_visibility(&mut self) {
        self.overview_state.show_groups = !self.overview_state.show_groups;
        if self.overview_state.show_groups {
            self.active_panel = ActivePanel::OverviewGroups;
            self.status_message = String::from("Overview: group section shown");
        } else {
            self.status_message = String::from("Overview: group section hidden");
        }
        self.ensure_panel_focus();
    }

    /// Toggles the scope/filter section visibility on the overview screen.
    pub fn toggle_filters_visibility(&mut self) {
        self.overview_state.show_filters = !self.overview_state.show_filters;
        if self.overview_state.show_filters {
            self.active_panel = ActivePanel::OverviewFilters;
            self.status_message = String::from("Overview: scope section shown");
        } else {
            self.status_message = String::from("Overview: scope section hidden");
        }
        self.ensure_panel_focus();
    }

    /// Collapses or expands the currently focused panel section.
    pub fn toggle_focused_section(&mut self) {
        let state = match self.active_panel {
            ActivePanel::OverviewCharacter => {
                self.overview_state.character_collapsed = !self.overview_state.character_collapsed;
                Some(("Character", self.overview_state.character_collapsed))
            }
            ActivePanel::OverviewGroups => {
                self.overview_state.groups_collapsed = !self.overview_state.groups_collapsed;
                Some(("Groups", self.overview_state.groups_collapsed))
            }
            ActivePanel::OverviewFilters => {
                self.overview_state.filters_collapsed = !self.overview_state.filters_collapsed;
                Some(("Scope", self.overview_state.filters_collapsed))
            }
            ActivePanel::OverviewCombat => {
                self.overview_state.combat_collapsed = !self.overview_state.combat_collapsed;
                Some(("Combat", self.overview_state.combat_collapsed))
            }
            ActivePanel::OverviewSession => {
                self.overview_state.session_collapsed = !self.overview_state.session_collapsed;
                Some(("Session", self.overview_state.session_collapsed))
            }
            ActivePanel::OverviewPriorities => {
                self.overview_state.priorities_collapsed =
                    !self.overview_state.priorities_collapsed;
                Some(("Priorities", self.overview_state.priorities_collapsed))
            }
            ActivePanel::TacticalNamed => {
                self.tactical_state.named_collapsed = !self.tactical_state.named_collapsed;
                Some(("Named", self.tactical_state.named_collapsed))
            }
            ActivePanel::TacticalNavigation => {
                self.tactical_state.navigation_collapsed =
                    !self.tactical_state.navigation_collapsed;
                Some(("Navigation", self.tactical_state.navigation_collapsed))
            }
            _ => None,
        };

        self.status_message = match state {
            Some((label, true)) => format!("{label}: collapsed"),
            Some((label, false)) => format!("{label}: expanded"),
            None => String::from("Focused pane does not collapse"),
        };
    }

    /// Toggles the tactical map between maximized and split view.
    pub fn toggle_tactical_map_maximized(&mut self) {
        self.tactical_state.map_maximized = !self.tactical_state.map_maximized;
        self.active_screen = ActiveScreen::Tactical;
        self.active_panel = ActivePanel::TacticalMap;
        self.status_message = if self.tactical_state.map_maximized {
            String::from("Map: maximized view enabled")
        } else {
            String::from("Map: split view restored")
        };
        self.ensure_panel_focus();
    }

    pub fn cycle_tactical_map_view(&mut self) {
        let mode = self.map_state.cycle_viewport_mode();
        self.active_screen = ActiveScreen::Tactical;
        self.active_panel = ActivePanel::TacticalMap;
        self.status_message = format!("Map: {} view", mode.label());
        self.ensure_panel_focus();
    }

    pub fn zoom_tactical_map_in(&mut self) {
        self.map_state.zoom_in();
        self.status_message = format!("Map: zoom {:.2}x", self.map_state.zoom);
    }

    pub fn zoom_tactical_map_out(&mut self) {
        self.map_state.zoom_out();
        self.status_message = format!("Map: zoom {:.2}x", self.map_state.zoom);
    }

    pub fn reset_tactical_map_view(&mut self) {
        self.map_state.reset_viewport();
        self.status_message = format!("Map: {} view reset", self.map_state.viewport_mode.label());
    }

    pub fn pan_tactical_map_left(&mut self) {
        let step = self.map_pan_step();
        self.pan_tactical_map(-step, 0.0);
    }

    pub fn pan_tactical_map_right(&mut self) {
        let step = self.map_pan_step();
        self.pan_tactical_map(step, 0.0);
    }

    pub fn pan_tactical_map_up(&mut self) {
        let step = self.map_pan_step();
        self.pan_tactical_map(0.0, -step);
    }

    pub fn pan_tactical_map_down(&mut self) {
        let step = self.map_pan_step();
        self.pan_tactical_map(0.0, step);
    }

    pub fn toggle_tactical_navmesh_overlay(&mut self) {
        let enabled = self.map_state.toggle_navmesh();
        if enabled {
            if let Some(zone) = self.current_zone_short_name()
                && self.map_state.navmesh_overlay.is_none()
            {
                self.load_zone_navmesh_overlay(&zone);
            }
            let segment_count = self
                .map_state
                .navmesh_overlay
                .as_ref()
                .map_or(0, |overlay| overlay.segment_count());
            self.status_message = if segment_count > 0 {
                format!("Map: navmesh overlay on ({segment_count} segments)")
            } else {
                String::from("Map: navmesh overlay enabled (no mesh available)")
            };
        } else {
            self.status_message = String::from("Map: navmesh overlay hidden");
        }
        self.active_screen = ActiveScreen::Tactical;
        self.active_panel = ActivePanel::TacticalMap;
        self.ensure_panel_focus();
    }

    fn pan_tactical_map(&mut self, delta_x: f32, delta_y: f32) {
        self.map_state.pan(delta_x, delta_y);
        self.active_screen = ActiveScreen::Tactical;
        self.active_panel = ActivePanel::TacticalMap;
    }

    fn map_pan_step(&self) -> f32 {
        let zoom = self.map_state.zoom.max(0.35);
        let global_step = (self.current_map_max_dimension() / 12.0).clamp(45.0, 320.0);
        let local_step = if self.tactical_state.map_maximized {
            70.0
        } else {
            45.0
        };
        let base_step = match self.map_state.viewport_mode {
            MapViewportMode::Local => local_step,
            MapViewportMode::Global => global_step,
            MapViewportMode::Auto => {
                if self.map_auto_uses_local_view() {
                    local_step
                } else {
                    global_step
                }
            }
        };
        base_step / zoom
    }

    fn map_auto_uses_local_view(&self) -> bool {
        self.tactical_state.map_maximized || self.current_map_max_dimension() > 1_200.0
    }

    fn current_map_max_dimension(&self) -> f32 {
        let map_dim = self
            .map_state
            .zone_map
            .as_ref()
            .map(|map| map.bounds.width().max(map.bounds.height()))
            .unwrap_or(0.0);
        let mesh_dim = self
            .map_state
            .navmesh_overlay
            .as_ref()
            .map(|overlay| overlay.bounds.max_dimension())
            .unwrap_or(0.0);

        map_dim.max(mesh_dim).max(600.0)
    }

    pub fn expand_selected_character(&mut self) {
        self.overview_state.character_collapsed = false;
        self.active_screen = ActiveScreen::Overview;
        self.active_panel = ActivePanel::OverviewCharacter;
        self.status_message = self
            .active_client()
            .and_then(|client| client.local_player.as_ref())
            .map_or_else(
                || String::from("Character: no client selected"),
                |player| format!("Character: {}", self.redact_name(&player.displayed_name)),
            );
        self.ensure_panel_focus();
    }

    /// Build default group definitions. If accounts config exists, derives groups
    /// from the configured group IDs. Otherwise falls back to 6 default groups.
    fn build_default_groups() -> Vec<GroupDef> {
        let default_names = ["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot"];

        if let Ok(accts) = AccountsConfig::load(std::path::Path::new("config/accounts.toml")) {
            // Discover unique group IDs from account config
            let mut group_ids: Vec<u32> = accts.accounts.iter().map(|a| a.group).collect();
            group_ids.sort();
            group_ids.dedup();
            group_ids.retain(|&id| id > 0); // skip ungrouped (0)

            if !group_ids.is_empty() {
                return group_ids
                    .iter()
                    .map(|&id| {
                        let accounts_in_group: Vec<&crate::config::AccountEntry> =
                            accts.accounts.iter().filter(|a| a.group == id).collect();

                        // Derive account range from actual account numbers
                        let account_nums: Vec<u8> = accounts_in_group
                            .iter()
                            .filter_map(|a| extract_account_number(&a.name))
                            .collect();
                        let lo = account_nums.iter().copied().min().unwrap_or(1);
                        let hi = account_nums.iter().copied().max().unwrap_or(lo);

                        let name = default_names.get((id - 1) as usize).map_or_else(
                            || format!("Group {id}"),
                            std::string::ToString::to_string,
                        );

                        GroupDef {
                            id: id as u8,
                            name,
                            account_range: (lo, hi),
                            default_camp: format!("Camp {id}"),
                        }
                    })
                    .collect();
            }
        }

        // Fallback: 6 groups with 6 slots each
        (0..6)
            .map(|i| {
                let lo = (i * 6 + 1) as u8;
                let hi = ((i + 1) * 6) as u8;
                GroupDef {
                    id: (i + 1) as u8,
                    name: default_names[i].to_string(),
                    account_range: (lo, hi),
                    default_camp: format!("Camp {}", default_names[i]),
                }
            })
            .collect()
    }

    /// Get the currently selected client, if any.
    pub fn active_client(&self) -> Option<&ClientState> {
        self.clients.get(self.selected_client)
    }

    pub(crate) fn selected_client_spawn_revision(&self) -> u64 {
        self.active_client()
            .map_or(0, |client| client.spawn_revision)
    }

    fn invalidate_spawn_caches(&mut self) {
        self.filtered_spawn_cache.clear();
        self.map_spawn_cache.clear();
    }

    fn mark_client_spawn_refresh_stale(&mut self, idx: usize) {
        if let Some(client) = self.clients.get_mut(idx) {
            client.last_spawn_refresh = None;
        }
    }

    /// Returns the short zone name for the selected client's current zone.
    pub fn current_zone_short_name(&self) -> Option<String> {
        self.active_client()
            .map(|client| super::run::zone_to_short_name(&client.zone_name))
    }

    /// Checks whether the selected client's zone has a cached navmesh.
    pub fn current_zone_has_cached_mesh(&self) -> Option<bool> {
        self.current_zone_short_name()
            .map(|zone| crate::nav::mesh::has_cached_zone_mesh(&zone))
    }

    /// Returns the display name for a client, redacted if privacy mode is on.
    pub fn client_command_target(&self, client: &ClientState) -> String {
        let name = if !client.character_name.is_empty() {
            client.character_name.as_str()
        } else if let Some(player) = &client.local_player {
            player.displayed_name.as_str()
        } else {
            return format!("PID {}", client.pid);
        };

        self.redact_name(name).into_owned()
    }

    fn select_client_idx(&mut self, idx: usize) {
        if idx >= self.clients.len() {
            return;
        }

        self.selected_client = idx;
        self.mark_client_spawn_refresh_stale(idx);
        self.sync_from_selected_client();
        self.spawns_state.table_state.select(Some(0));
        self.reload_map_for_selected_client();
    }

    /// Sync the legacy single-client fields from the selected client.
    /// This keeps backward compatibility with code that reads `app.local_player`, etc.
    pub fn sync_from_selected_client(&mut self) {
        if let Some(client) = self.clients.get(self.selected_client) {
            let revision_key = (client.pid, client.spawn_revision);
            let local_player = client.local_player.clone();
            let target = client.target.clone();
            let updated_spawns =
                (self.synced_spawn_revision != Some(revision_key)).then(|| client.spawns.clone());
            let pid = client.pid;
            let eq_base = client.eq_base;

            self.local_player = local_player;
            self.target = target;
            if let Some(spawns) = updated_spawns {
                self.spawns = spawns;
                self.synced_spawn_revision = Some(revision_key);
                self.invalidate_spawn_caches();
            }
            self.attached_pid = Some(pid);
            self.eq_base = eq_base;
        } else if self.clients.is_empty() {
            // No clients — clear data
            self.local_player = None;
            self.target = None;
            self.spawns.clear();
            self.synced_spawn_revision = None;
            self.invalidate_spawn_caches();
        }
    }

    /// Sync CH chain status from orchestrator into cached display state.
    pub fn sync_ch_chain_status(&mut self, orchestrator: &Orchestrator) {
        self.ch_chain_status = if orchestrator.combat.ch_chain_active() {
            orchestrator
                .combat
                .ch_chain
                .as_ref()
                .map(|chain| ChChainStatus {
                    members: chain.members().len(),
                    interval_secs: chain.interval_secs(),
                    is_adaptive: chain.is_adaptive(),
                    target_id: chain.target_id(),
                })
        } else {
            self.demo_ch_chain_status()
        };
    }

    /// Sync CH chain data into the dedicated CH chain management panel.
    pub fn sync_ch_chain_panel_state(&mut self, orchestrator: &Orchestrator) {
        let Some(chain) = orchestrator.combat.ch_chain.as_ref() else {
            if self.sync_demo_ch_chain_panel_state() {
                return;
            }
            self.ch_chain_panel_state.clerics.clear();
            self.ch_chain_panel_state.selected = 0;
            self.ch_chain_panel_state.target_id = 0;
            self.ch_chain_panel_state.target_name.clear();
            self.ch_chain_panel_state.cast_time_secs = 10.0;
            self.ch_chain_panel_state.overlap_buffer_secs = 0.5;
            self.ch_chain_panel_state.chain_delay_secs = 0.0;
            self.ch_chain_panel_state.adaptive = false;
            self.ch_chain_panel_state.stats = ChainStats::default();
            return;
        };

        self.ch_chain_panel_state.target_id = chain.target_id();
        self.ch_chain_panel_state.target_name =
            self.find_spawn_name(chain.target_id()).unwrap_or_default();
        self.ch_chain_panel_state.cast_time_secs = 10.0;
        self.ch_chain_panel_state.overlap_buffer_secs = 0.5;
        self.ch_chain_panel_state.chain_delay_secs = chain.interval_secs();
        self.ch_chain_panel_state.adaptive = chain.is_adaptive();

        let cast_progress = chain.cast_progress();
        self.ch_chain_panel_state.clerics = chain
            .members()
            .iter()
            .copied()
            .enumerate()
            .map(|(index, pid)| {
                let mut name = self
                    .client_name_for_pid(pid)
                    .unwrap_or_else(|| format!("PID {pid}"));
                if name.is_empty() {
                    name = format!("PID {pid}");
                }
                ChainCleric {
                    name,
                    pid,
                    position: (index as u8) + 1,
                    timing_offset_ms: 0,
                    cast_display: if let Some((active_index, progress)) = cast_progress
                        && active_index == index
                    {
                        Some(CastDisplay::exact_progress(
                            "Complete Heal",
                            "CH",
                            f64::from(progress),
                            self.ch_chain_panel_state.cast_time_secs,
                        ))
                    } else {
                        None
                    },
                    cast_state: if let Some((active_index, progress)) = cast_progress
                        && active_index == index
                    {
                        ChPanelCastState::Casting(progress)
                    } else {
                        ChPanelCastState::Idle
                    },
                }
            })
            .collect();

        if self.ch_chain_panel_state.selected >= self.ch_chain_panel_state.clerics.len() {
            self.ch_chain_panel_state.selected = 0;
        }
    }

    /// Sync all CH chain summaries and management panel model state.
    pub fn sync_ch_chain_state(&mut self, orchestrator: &Orchestrator) {
        self.sync_ch_chain_status(orchestrator);
        self.sync_ch_chain_panel_state(orchestrator);
    }

    /// Cycle to the next client.
    pub fn next_client(&mut self) {
        if !self.clients.is_empty() {
            let next = (self.selected_client + 1) % self.clients.len();
            self.select_client_idx(next);
        }
    }

    /// Cycle to the previous client.
    pub fn prev_client(&mut self) {
        if !self.clients.is_empty() {
            let prev = if self.selected_client == 0 {
                self.clients.len() - 1
            } else {
                self.selected_client - 1
            };
            self.select_client_idx(prev);
        }
    }

    /// Focus on a specific group (0-indexed). Pass None to return to aggregate view.
    /// When live group data is available, validates against live group count;
    /// otherwise validates against config group count.
    pub fn set_active_group(&mut self, group: Option<usize>) {
        if let Some(idx) = group {
            if self.has_live_group_data() {
                let (live_groups, _) = self.build_live_groups();
                if idx < live_groups.len() {
                    self.active_group = Some(idx);
                    self.status_message = format!(
                        "Viewing: {} ({})",
                        live_groups[idx].leader, live_groups[idx].zone
                    );
                }
            } else if idx < self.groups.len() {
                self.active_group = Some(idx);
                let g = &self.groups[idx];
                self.status_message = format!("Viewing: G{} {}", g.id, g.name);
            }
        } else {
            self.active_group = None;
            self.status_message = String::from("Viewing: All Groups");
        }
    }

    /// Returns the display label for the current group focus.
    pub fn group_focus_label(&self) -> String {
        match self.active_group {
            None => String::from("All Groups"),
            Some(idx) => {
                if self.has_live_group_data() {
                    let (live_groups, _) = self.build_live_groups();
                    if let Some(lg) = live_groups.get(idx) {
                        format!("{} ({})", lg.leader, lg.zone)
                    } else {
                        String::from("All Groups")
                    }
                } else if let Some(g) = self.groups.get(idx) {
                    // Find the zone of the first online member
                    let zone = self
                        .clients_in_group_idx(idx)
                        .first()
                        .map_or("???", |c| c.zone_name.as_str());
                    format!("G{} {} ({})", g.id, g.name, zone)
                } else {
                    String::from("All Groups")
                }
            }
        }
    }

    /// Summarize the current combat context for operator feedback.
    pub fn combat_status_summary(&self) -> String {
        let mode = format!("{}", self.operating_mode);
        let scope = self.routing_scope.label();
        let focused = self.focused_pids().len();
        let visible = self.visible_clients().len();
        let ma = self.main_assist.as_deref().unwrap_or("—");
        let mt = self.main_tank.as_deref().unwrap_or("—");
        format!(
            "Combat: mode={mode} | scope={scope} | focused={focused}/{visible} clients | MA={ma} | MT={mt}"
        )
    }

    /// Get a short display label for the client's configured group, if known.
    pub fn client_group_label(&self, client: &ClientState) -> Option<&'static str> {
        const LABELS: &[&str] = &["G0", "G1", "G2", "G3", "G4", "G5", "G6", "G7", "G8", "G9"];

        let name = if !client.character_name.is_empty() {
            client.character_name.as_str()
        } else if let Some(player) = &client.local_player {
            player.displayed_name.as_str()
        } else {
            return None;
        };

        let account_num = extract_account_number(name)?;
        self.groups
            .iter()
            .find(|group| {
                let (lo, hi) = group.account_range;
                account_num >= lo && account_num <= hi
            })
            .and_then(|group| LABELS.get(group.id as usize).copied())
    }

    /// Get clients belonging to the group at the given index (0-based).
    pub fn clients_in_group_idx(&self, group_idx: usize) -> Vec<&ClientState> {
        if let Some(group) = self.groups.get(group_idx) {
            let (lo, hi) = group.account_range;
            self.clients
                .iter()
                .filter(|c| {
                    let name = if !c.character_name.is_empty() {
                        &c.character_name
                    } else if let Some(p) = &c.local_player {
                        &p.displayed_name
                    } else {
                        return false;
                    };
                    if let Some(num) = extract_account_number(name) {
                        num >= lo && num <= hi
                    } else {
                        false
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get clients visible under the current group focus.
    /// Returns all clients if aggregate view, or only the focused group's clients.
    /// When live group data is available, filters by live group membership.
    pub fn visible_clients(&self) -> Vec<&ClientState> {
        match self.active_group {
            None => self.clients.iter().collect(),
            Some(idx) => {
                if self.has_live_group_data() {
                    let (live_groups, _) = self.build_live_groups();
                    if let Some(lg) = live_groups.get(idx) {
                        lg.member_names
                            .iter()
                            .filter_map(|name| self.find_client_by_name(name))
                            .collect()
                    } else {
                        self.clients.iter().collect()
                    }
                } else {
                    self.clients_in_group_idx(idx)
                }
            }
        }
    }

    /// Get clients targeted by the current routing scope.
    pub fn routed_clients(&self) -> Vec<&ClientState> {
        use textquest_common::routing::RoutingScope;

        match &self.routing_scope {
            RoutingScope::AllSession => self.clients.iter().collect(),
            RoutingScope::Group { group_id, .. } => {
                let idx = usize::from(group_id.saturating_sub(1));
                self.clients_in_group_idx(idx)
            }
            RoutingScope::OneToon { name } => self.find_client_by_name(name).into_iter().collect(),
        }
    }

    /// Get PIDs of clients targeted by the current routing scope.
    pub fn focused_pids(&self) -> Vec<u32> {
        self.routed_clients().iter().map(|c| c.pid).collect()
    }

    /// Return the number of focused clients without allocating a Vec.
    pub fn focused_pid_count(&self) -> usize {
        self.routed_clients().len()
    }

    /// Send an IPC command to all focused clients, returning the success count.
    fn send_ipc_to_focused(&self, cmd: &textquest_common::ipc::Command) -> usize {
        self.focused_pids()
            .iter()
            .filter(|pid| send_ipc_command(**pid, cmd).is_ok())
            .count()
    }

    /// Returns `true` if any connected client has live `GroupInfo` data.
    pub fn has_live_group_data(&self) -> bool {
        self.clients.iter().any(|c| c.group_info.is_some())
    }

    /// Build dynamic group list from live EQ group membership.
    /// Groups clients by `leader_name` — same leader means same group.
    /// Returns an ordered list of `LiveGroup` plus a list of ungrouped client indices.
    pub fn build_live_groups(&self) -> (Vec<LiveGroup>, Vec<usize>) {
        let mut groups_map: HashMap<String, LiveGroup> = HashMap::new();
        let mut grouped_indices: std::collections::HashSet<usize> =
            std::collections::HashSet::new();

        for (idx, client) in self.clients.iter().enumerate() {
            if let Some(gi) = &client.group_info {
                if gi.leader_name.is_empty() {
                    continue;
                }
                grouped_indices.insert(idx);

                let entry = groups_map
                    .entry(gi.leader_name.clone())
                    .or_insert_with(|| LiveGroup {
                        leader: gi.leader_name.clone(),
                        member_names: Vec::new(),
                        zone: client.zone_name.clone(),
                    });

                // Merge member names from this client's perspective
                for member in &gi.members {
                    if !member.is_empty() && !entry.member_names.contains(member) {
                        entry.member_names.push(member.clone());
                    }
                }
            }
        }

        // Ensure leader is first in member list
        for group in groups_map.values_mut() {
            if let Some(pos) = group.member_names.iter().position(|n| n == &group.leader) {
                group.member_names.swap(0, pos);
            }
        }

        // Collect ungrouped clients (those not mentioned in any group)
        let all_grouped_names: std::collections::HashSet<&str> = groups_map
            .values()
            .flat_map(|g| g.member_names.iter().map(std::string::String::as_str))
            .collect();

        let ungrouped: Vec<usize> = self
            .clients
            .iter()
            .enumerate()
            .filter(|(idx, c)| {
                if grouped_indices.contains(idx) {
                    return false;
                }
                let name = if !c.character_name.is_empty() {
                    c.character_name.as_str()
                } else if let Some(p) = &c.local_player {
                    p.displayed_name.as_str()
                } else {
                    return true;
                };
                !all_grouped_names.contains(name)
            })
            .map(|(idx, _)| idx)
            .collect();

        // Sort groups by leader name for stable ordering
        let mut groups: Vec<LiveGroup> = groups_map.into_values().collect();
        groups.sort_by(|a, b| a.leader.cmp(&b.leader));

        (groups, ungrouped)
    }

    fn find_client_index_by_name(&self, name: &str) -> Option<usize> {
        let lower = name.to_lowercase();
        self.clients.iter().position(|c| {
            if !c.character_name.is_empty() {
                return c.character_name.to_lowercase() == lower;
            }
            if let Some(p) = &c.local_player {
                return p.displayed_name.to_lowercase() == lower;
            }
            false
        })
    }

    /// Find a client by character name (case-insensitive).
    pub fn find_client_by_name(&self, name: &str) -> Option<&ClientState> {
        self.find_client_index_by_name(name)
            .and_then(|idx| self.clients.get(idx))
    }

    fn client_name_for_pid(&self, pid: u32) -> Option<String> {
        for client in &self.clients {
            if client.pid != pid {
                continue;
            }

            if !client.character_name.is_empty() {
                return Some(client.character_name.clone());
            }

            if let Some(player) = &client.local_player {
                if !player.displayed_name.is_empty() {
                    return Some(player.displayed_name.clone());
                }
                if !player.name.is_empty() {
                    return Some(player.name.clone());
                }
            }

            break;
        }

        None
    }

    fn demo_ch_chain_status(&self) -> Option<ChChainStatus> {
        let clerics: Vec<&ClientState> =
            self.clients
                .iter()
                .filter(|client| {
                    client.is_demo
                        && demo_client_profile(client.character_name.as_str(), client.pid)
                            .is_some_and(|profile| {
                                matches!(
                                    profile.role,
                                    DemoRole::ChainCleric | DemoRole::ChainClericTwo
                                )
                            })
                })
                .collect();
        if clerics.is_empty() {
            return None;
        }

        let target_name = clerics
            .iter()
            .find_map(|client| {
                let name = client.character_name.as_str();
                demo_client_profile(name, client.pid).and_then(|profile| profile.target_spawn_name)
            })
            .unwrap_or("Dmft01");
        let target_id = self.find_spawn_id_by_name(target_name).unwrap_or(0);

        Some(ChChainStatus {
            members: clerics.len(),
            interval_secs: 2.5,
            is_adaptive: false,
            target_id,
        })
    }

    fn sync_demo_ch_chain_panel_state(&mut self) -> bool {
        let mut clerics: Vec<ChainCleric> = self
            .clients
            .iter()
            .filter_map(|client| {
                if !client.is_demo {
                    return None;
                }
                let profile = demo_client_profile(&client.character_name, client.pid)?;
                if !matches!(
                    profile.role,
                    DemoRole::ChainCleric | DemoRole::ChainClericTwo
                ) {
                    return None;
                }

                let cast_display = self.client_cast_display(client);
                let cast_state = if let Some(display) = &cast_display {
                    if display.label == "Complete Heal" {
                        ChPanelCastState::Casting(display.progress as f32)
                    } else {
                        ChPanelCastState::Idle
                    }
                } else {
                    ChPanelCastState::Idle
                };

                Some(ChainCleric {
                    name: client.character_name.clone(),
                    pid: client.pid,
                    position: 0,
                    timing_offset_ms: 0,
                    cast_display,
                    cast_state,
                })
            })
            .collect();
        if clerics.is_empty() {
            return false;
        }

        clerics.sort_by_key(|cleric| cleric.pid);
        for (index, cleric) in clerics.iter_mut().enumerate() {
            cleric.position = (index + 1) as u8;
        }

        let target_name = clerics
            .iter()
            .find_map(|cleric| {
                demo_client_profile(&cleric.name, cleric.pid)
                    .and_then(|profile| profile.target_spawn_name)
            })
            .unwrap_or("Dmft01");

        self.ch_chain_panel_state.target_id = self.find_spawn_id_by_name(target_name).unwrap_or(0);
        self.ch_chain_panel_state.target_name = target_name.to_string();
        self.ch_chain_panel_state.cast_time_secs = 10.0;
        self.ch_chain_panel_state.overlap_buffer_secs = 0.5;
        self.ch_chain_panel_state.chain_delay_secs = 2.5;
        self.ch_chain_panel_state.adaptive = false;
        self.ch_chain_panel_state.stats = ChainStats::default();
        self.ch_chain_panel_state.clerics = clerics;
        if self.ch_chain_panel_state.selected >= self.ch_chain_panel_state.clerics.len() {
            self.ch_chain_panel_state.selected = 0;
        }
        true
    }

    fn find_spawn_name(&self, spawn_id: u32) -> Option<String> {
        self.clients.iter().find_map(|client| {
            if let Some(player) = &client.local_player
                && player.spawn_id == spawn_id
            {
                return Some(player.displayed_name.clone());
            }
            client.spawns.iter().find_map(|spawn| {
                (spawn.spawn_id == spawn_id).then(|| spawn.displayed_name.clone())
            })
        })
    }

    fn find_spawn_id_by_name(&self, name: &str) -> Option<u32> {
        self.clients.iter().find_map(|client| {
            if let Some(player) = &client.local_player
                && (player.displayed_name.eq_ignore_ascii_case(name)
                    || player.name.eq_ignore_ascii_case(name))
            {
                return Some(player.spawn_id);
            }
            client.spawns.iter().find_map(|spawn| {
                if spawn.displayed_name.eq_ignore_ascii_case(name)
                    || spawn.name.eq_ignore_ascii_case(name)
                {
                    Some(spawn.spawn_id)
                } else {
                    None
                }
            })
        })
    }

    /// Returns spawns filtered by type and text search criteria.
    pub fn filtered_spawns(&self) -> Vec<&SpawnInfo> {
        let filter = self.spawns_state.spawn_filter.to_ascii_lowercase();
        self.spawns
            .iter()
            .filter(|spawn| {
                spawn_matches_filter(spawn, self.spawns_state.spawn_type_filter, filter.as_str())
            })
            .collect()
    }

    pub fn filtered_spawn_indices(&mut self) -> &[usize] {
        let key = FilteredSpawnCacheKey {
            client_pid: self.active_client().map(|client| client.pid),
            spawn_revision: self.selected_client_spawn_revision(),
            spawn_filter: self.spawns_state.spawn_filter.to_ascii_lowercase(),
            spawn_type_filter: self.spawns_state.spawn_type_filter,
            sort_column: self.spawns_state.sort_column,
            sort_ascending: self.spawns_state.sort_ascending,
        };

        if self.filtered_spawn_cache.key.as_ref() != Some(&key) {
            let mut indices: Vec<usize> = self
                .spawns
                .iter()
                .enumerate()
                .filter_map(|(index, spawn)| {
                    spawn_matches_filter(spawn, key.spawn_type_filter, key.spawn_filter.as_str())
                        .then_some(index)
                })
                .collect();

            let player_pos: Option<(f32, f32)> = self
                .active_client()
                .and_then(|c| c.local_player.as_ref())
                .map(|p| (p.x, p.y));
            let spawns = &self.spawns;
            let asc = key.sort_ascending;
            match key.sort_column {
                SpawnSort::Default => {}
                SpawnSort::Name => indices.sort_by(|&a, &b| {
                    let c = spawns[a].displayed_name.cmp(&spawns[b].displayed_name);
                    if asc { c } else { c.reverse() }
                }),
                SpawnSort::Class => indices.sort_by(|&a, &b| {
                    let c = spawns[a].class_str().cmp(&spawns[b].class_str());
                    if asc { c } else { c.reverse() }
                }),
                SpawnSort::Level => indices.sort_by(|&a, &b| {
                    let c = spawns[a].level.cmp(&spawns[b].level);
                    if asc { c } else { c.reverse() }
                }),
                SpawnSort::Distance => {
                    if let Some((px, py)) = player_pos {
                        indices.sort_by(|&a, &b| {
                            let da = (spawns[a].x - px).powi(2) + (spawns[a].y - py).powi(2);
                            let db = (spawns[b].x - px).powi(2) + (spawns[b].y - py).powi(2);
                            let c = da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal);
                            if asc { c } else { c.reverse() }
                        });
                    }
                }
            }

            self.filtered_spawn_cache.indices = indices;
            self.filtered_spawn_cache.key = Some(key);
        }

        &self.filtered_spawn_cache.indices
    }

    pub fn filtered_spawn_count(&mut self) -> usize {
        self.filtered_spawn_indices().len()
    }

    pub fn filtered_spawn_at(&mut self, filtered_index: usize) -> Option<&SpawnInfo> {
        let spawn_index = self.filtered_spawn_indices().get(filtered_index).copied()?;
        self.spawns.get(spawn_index)
    }

    pub fn selected_filtered_spawn(&mut self) -> Option<&SpawnInfo> {
        let selected = self.spawn_selected();
        self.filtered_spawn_at(selected)
    }

    /// Cycles the spawn type filter (All -> PC -> NPC -> Named).
    pub fn cycle_spawn_filter(&mut self) {
        self.spawns_state.spawn_type_filter = self.spawns_state.spawn_type_filter.next();
        self.spawns_state.table_state.select(Some(0));
        self.status_message = format!("Filter: {}", self.spawns_state.spawn_type_filter.label());
    }
    /// Cycles the spawn sort column (Default -> Name -> Class -> Level -> Distance).
    pub fn cycle_spawn_sort(&mut self) {
        let next = self.spawns_state.sort_column.next();
        if next == SpawnSort::Default {
            self.spawns_state.sort_ascending = true;
        }
        self.spawns_state.sort_column = next;
        self.spawns_state.table_state.select(Some(0));
        let arrow = if self.spawns_state.sort_ascending {
            "\u{2191}"
        } else {
            "\u{2193}"
        };
        self.status_message = format!("Sort: {} {arrow}", next.label());
    }

    /// Toggles the sort direction (ascending / descending).
    pub fn toggle_spawn_sort_direction(&mut self) {
        self.spawns_state.sort_ascending = !self.spawns_state.sort_ascending;
        self.spawns_state.table_state.select(Some(0));
        let dir = if self.spawns_state.sort_ascending {
            "\u{2191} asc"
        } else {
            "\u{2193} desc"
        };
        self.status_message = format!("Sort: {} {dir}", self.spawns_state.sort_column.label());
    }

    /// Cycles the navigation scope (Active -> Group -> All).
    pub fn cycle_nav_scope(&mut self) {
        self.spawns_state.nav_scope = self.spawns_state.nav_scope.next();
        self.status_message = format!("Nav scope: {}", self.spawns_state.nav_scope.label());
    }

    /// Moves the spawn list selection down by one row.
    pub fn spawn_list_down(&mut self) {
        let count = self.filtered_spawn_count();
        self.spawns_state.table_state.select_next();
        // Clamp to last item
        if let Some(sel) = self.spawns_state.table_state.selected()
            && sel >= count
        {
            self.spawns_state
                .table_state
                .select(Some(count.saturating_sub(1)));
        }
    }

    /// Moves the spawn list selection up by one row.
    pub fn spawn_list_up(&mut self) {
        self.spawns_state.table_state.select_previous();
    }

    /// Moves the spawn list selection down by one page.
    pub fn spawn_list_page_down(&mut self) {
        let page_size = Self::dynamic_page_size();
        let max = self.filtered_spawn_count().saturating_sub(1);
        let current = self.spawn_selected();
        self.spawns_state
            .table_state
            .select(Some((current + page_size).min(max)));
    }

    /// Moves the spawn list selection up by one page.
    pub fn spawn_list_page_up(&mut self) {
        let page_size = Self::dynamic_page_size();
        let current = self.spawn_selected();
        self.spawns_state
            .table_state
            .select(Some(current.saturating_sub(page_size)));
    }

    /// Compute page size from terminal height. Uses the spawn table area
    /// (terminal height minus chrome: header, status bar, column headers, borders).
    /// Falls back to 25 rows if terminal size cannot be determined.
    fn dynamic_page_size() -> usize {
        const CHROME_ROWS: u16 = 8; // header + tabs + column header + borders + status bar
        const FALLBACK: usize = 25;
        match crossterm::terminal::size() {
            Ok((_w, h)) => (h.saturating_sub(CHROME_ROWS) as usize).max(5),
            Err(_) => FALLBACK,
        }
    }

    /// Convenience accessor for the current spawn selection index.
    pub fn spawn_selected(&self) -> usize {
        self.spawns_state.table_state.selected().unwrap_or(0)
    }

    /// Select an EQ internals offset and auto-scroll the hex dump to its address.
    ///
    /// For globals/functions (absolute preferred-base addresses), sets the hex dump
    /// address directly. For struct field offsets (PlayerBase, PlayerZone, SpawnManager),
    /// shows the offset value. On macOS, generates demo data at the address.
    pub fn internals_select_offset(&mut self) {
        let entry = self.eq_internals_state.selected_entry().cloned();
        if let Some(entry) = entry {
            let is_absolute = matches!(
                entry.category,
                super::state::OffsetCategory::Globals | super::state::OffsetCategory::Functions
            );

            if is_absolute {
                // Preferred-base address — show hex at that address.
                self.hex_state.hex_label = format!("{} @ 0x{:X}", entry.name, entry.value);
                #[cfg(windows)]
                {
                    // On Windows with a live process, rebase and read real memory.
                    // For now, set address to the preferred-base value.
                    self.hex_state.hex_address = entry.value as usize;
                    self.hex_state.hex_data = vec![0u8; 0x200];
                }
                #[cfg(not(windows))]
                {
                    self.hex_state.hex_address = entry.value as usize;
                    self.hex_state.hex_data =
                        generate_demo_hex_data_for_offset(&entry.name, entry.value);
                }
            } else {
                // Struct field offset — display it as a relative offset.
                self.hex_state.hex_label = format!("{} (offset +0x{:X})", entry.name, entry.value);
                #[cfg(windows)]
                {
                    self.hex_state.hex_address = entry.value as usize;
                    self.hex_state.hex_data = vec![0u8; 0x200];
                }
                #[cfg(not(windows))]
                {
                    self.hex_state.hex_address = entry.value as usize;
                    self.hex_state.hex_data =
                        generate_demo_hex_data_for_offset(&entry.name, entry.value);
                }
            }

            self.status_message = format!("EQ Internals: {}", entry.name);
        }
    }

    /// Toggle the struct field annotation overlay on the hex dump.
    pub fn toggle_hex_annotations(&mut self) {
        self.hex_state.show_annotations = !self.hex_state.show_annotations;
        if self.hex_state.show_annotations && self.hex_state.annotations.is_empty() {
            // Default to PlayerBase annotations when viewing spawn memory.
            self.hex_state.load_player_base_annotations();
        }
        let state = if self.hex_state.show_annotations {
            "ON"
        } else {
            "OFF"
        };
        self.status_message = format!("Annotations {state}");
    }

    /// Scrolls the hex dump view down by 256 bytes.
    pub fn hex_scroll_down(&mut self) {
        self.hex_state.hex_address = self.hex_state.hex_address.wrapping_add(0x100);
    }

    /// Scrolls the hex dump view up by 256 bytes.
    pub fn hex_scroll_up(&mut self) {
        self.hex_state.hex_address = self.hex_state.hex_address.wrapping_sub(0x100);
    }

    /// Set the debug pane to view a specific spawn's raw memory.
    pub fn debug_selected_spawn(&mut self) {
        // Extract data from the borrow before mutating self
        let sel = self.spawn_selected();
        let info: Option<(String, u32, usize)> = {
            self.filtered_spawn_at(sel)
                .map(|s| (s.displayed_name.clone(), s.spawn_id, sel))
        };
        if let Some((name, id, _idx)) = info {
            self.hex_state.hex_label = format!("Raw memory: {name} (ID {id})");
            self.status_message = format!("Debug: {name}");

            // On Windows, read real spawn memory; on macOS, generate demo hex data
            #[cfg(windows)]
            {
                self.hex_state.hex_data = self.read_spawn_hex_data(id);
                self.hex_state.hex_address = 0;
            }
            #[cfg(not(windows))]
            {
                self.hex_state.hex_data = generate_demo_hex_data(&name, id);
                self.hex_state.hex_address = 0x1000;
            }

            // Pre-load PlayerBase annotations for spawn memory context.
            self.hex_state.load_player_base_annotations();

            self.set_active_screen(ActiveScreen::Debug);
            self.active_panel = ActivePanel::DebugHexDump;
        }
    }

    /// Navigate clients to the currently selected spawn's location.
    /// Respects `nav_scope`: Active sends to selected client, Group to focused group, All to everyone.
    pub fn navigate_to_selected_spawn(&mut self) {
        let info = self.selected_filtered_spawn().map(|s| {
            (
                s.displayed_name.clone(),
                textquest_common::nav::Waypoint::new(s.x, s.y, s.z),
            )
        });
        if let Some((name, waypoint)) = info {
            let scope = self.spawns_state.nav_scope;
            match scope {
                NavScope::Active => {
                    if let Some(client) = self.active_client() {
                        let pid = client.pid;
                        let cmd = textquest_common::ipc::Command::NavigateTo {
                            waypoints: vec![waypoint],
                        };
                        if send_ipc_command(pid, &cmd).is_ok() {
                            self.set_feedback(
                                ToastLevel::Success,
                                format!("Nav \u{2192} {name} (active client)"),
                                true,
                            );
                        } else {
                            self.set_feedback(
                                ToastLevel::Warning,
                                format!("Failed to send nav to active client for {name}"),
                                true,
                            );
                        }
                    }
                }
                NavScope::Group => {
                    self.execute_waypoint_navigation(&name, waypoint, None);
                }
                NavScope::All => {
                    // Temporarily clear group filter so visible_clients() returns all.
                    let saved_group = self.active_group;
                    self.active_group = None;
                    self.execute_waypoint_navigation(&name, waypoint, None);
                    self.active_group = saved_group;
                }
            }
        }
    }

    /// Target the currently selected spawn by spawn ID.
    pub fn target_selected_spawn(&mut self) {
        let info = self
            .selected_filtered_spawn()
            .map(|s| (s.displayed_name.clone(), s.spawn_id));
        if let Some((name, spawn_id)) = info {
            let cmd = textquest_common::ipc::Command::SetTarget { spawn_id };
            let ok = self.send_ipc_to_focused(&cmd);
            if ok == 0 {
                self.set_feedback(
                    ToastLevel::Warning,
                    format!("No clients connected to target {name}"),
                    true,
                );
            } else {
                self.set_feedback(
                    ToastLevel::Success,
                    format!("Target → {name} (sent to {ok} clients)"),
                    true,
                );
            }
        }
    }

    /// Read spawn memory on Windows for hex dump display.
    #[cfg(windows)]
    fn read_spawn_hex_data(&self, spawn_id: u32) -> Vec<u8> {
        use crate::process::memory::ProcessHandle;
        use textquest_common::offsets;

        // Bound traversal to avoid hangs on corrupt or cyclic spawn lists.
        const MAX_HEX_SPAWN_SCAN: usize = 4096;

        if let Some(client) = self.active_client()
            && let Ok(proc) = ProcessHandle::open(client.pid)
        {
            // Find the spawn address by walking the spawn list
            let Some(mgr_ptr_addr) = offsets::rebase(offsets::PINST_SPAWN_MANAGER, client.eq_base)
            else {
                return Vec::new();
            };
            let mgr_addr = match proc.read_ptr(mgr_ptr_addr) {
                Ok(a) if a != 0 => a,
                _ => return Vec::new(),
            };

            let list_addr = mgr_addr + offsets::spawn_manager::PLAYER_LIST;
            let mut current = proc.read_ptr(list_addr).unwrap_or(0);
            let mut scanned = 0usize;
            while current != 0 && scanned < MAX_HEX_SPAWN_SCAN {
                let sid = proc
                    .read::<u32>(current + offsets::player_base::SPAWN_ID)
                    .unwrap_or(0);
                if sid == spawn_id {
                    return proc.read_bytes(current, 0x200).unwrap_or_default();
                }
                current = proc
                    .read_ptr(current + offsets::player_base::NEXT)
                    .unwrap_or(0);
                scanned += 1;
            }
        }
        Vec::new()
    }

    /// Clears the spawn list text filter and resets selection.
    pub fn clear_filter(&mut self) {
        self.spawns_state.spawn_filter.clear();
        self.spawns_state.search_mode = false;
        self.spawns_state.table_state.select(Some(0));
    }

    /// Toggles privacy mode, which hides character names and server.
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
            if let Some(player) = &client.local_player
                && (player.displayed_name == name || player.name == name)
            {
                return std::borrow::Cow::Owned(format!("Toon-{:02}", i + 1));
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

    /// Tab-complete the current command buffer.
    /// Supports multi-level completion: first tab completes command name,
    /// subsequent tabs complete context-specific arguments.
    pub fn complete_command(&mut self) {
        let buf = self.cmd_state.command_buffer.clone();
        let prefix = buf.trim_start();

        // --- Argument-level completion (command already typed + space) ---

        // :camp <Tab> → camp subcommands + saved camp names
        if let Some(rest) = prefix.strip_prefix("camp ") {
            if let Some(camp_prefix) = rest.strip_prefix("start ") {
                self.complete_with_candidates("camp start ", camp_prefix, &self.list_camp_names());
            } else if let Some(camp_prefix) = rest.strip_prefix("remove ") {
                self.complete_with_candidates("camp remove ", camp_prefix, &self.list_camp_names());
            } else if let Some(add_rest) = rest.strip_prefix("add ") {
                // Suggest zone-based name
                let zone = self
                    .active_client()
                    .map_or_else(|| "camp".into(), |c| c.zone_name.clone());
                let suggestion = vec![zone];
                self.complete_with_candidates("camp add ", add_rest, &suggestion);
            } else {
                // Subcommands + saved camp names (bare name = shortcut for start)
                let mut sub_cmds: Vec<String> = vec![
                    "start".into(),
                    "stop".into(),
                    "status".into(),
                    "list".into(),
                    "add".into(),
                    "remove".into(),
                    "next".into(),
                    "prev".into(),
                ];
                sub_cmds.extend(self.list_camp_names());
                self.complete_with_candidates("camp ", rest, &sub_cmds);
            }
            return;
        }

        // :track <Tab> → cycle current zone spawn names
        if let Some(rest) = prefix.strip_prefix("track ") {
            if rest == "list" || rest.starts_with("list ") {
                return; // "list" is complete
            }
            let spawn_names = self.list_spawn_names();
            self.complete_with_candidates("track ", rest, &spawn_names);
            return;
        }

        // :untrack <Tab> → cycle tracked spawn names
        if let Some(rest) = prefix.strip_prefix("untrack ") {
            let tracked_names: Vec<String> = self
                .tracked_spawns
                .values()
                .map(|t| t.name.clone())
                .collect();
            self.complete_with_candidates("untrack ", rest, &tracked_names);
            return;
        }

        // :login <Tab> → login subcommands + account names
        if let Some(rest) = prefix.strip_prefix("login ") {
            let mut candidates: Vec<String> = vec!["all".into()];
            for g in &self.groups {
                candidates.push(format!("G{}", g.id));
            }
            if let Some(accts) = &self.accounts_config {
                for entry in &accts.accounts {
                    candidates.push(entry.name.clone());
                }
            }
            self.complete_with_candidates("login ", rest, &candidates);
            return;
        }

        // :mode <Tab> → camp/hunt
        if let Some(rest) = prefix.strip_prefix("mode ") {
            let modes: Vec<String> = vec!["camp".into(), "hunt".into()];
            self.complete_with_candidates("mode ", rest, &modes);
            return;
        }

        // :ma / :mt / :invite <Tab> → character names
        for cmd in &["ma ", "mt ", "invite "] {
            if let Some(rest) = prefix.strip_prefix(cmd) {
                let names = self.list_character_names();
                self.complete_with_candidates(cmd, rest, &names);
                return;
            }
        }

        // :heal <Tab> → cancel
        if let Some(rest) = prefix.strip_prefix("heal ") {
            let subs: Vec<String> = vec!["cancel".into()];
            self.complete_with_candidates("heal ", rest, &subs);
            return;
        }

        // :ch <Tab> → CH chain subcommands
        if let Some(rest) = prefix.strip_prefix("ch ") {
            let subs: Vec<String> = vec![
                "start".into(),
                "stop".into(),
                "status".into(),
                "add".into(),
                "rm".into(),
                "interval".into(),
                "adaptive".into(),
            ];
            self.complete_with_candidates("ch ", rest, &subs);
            return;
        }

        if let Some(rest) = prefix.strip_prefix("chui ") {
            let subs: Vec<String> = vec!["open".into(), "close".into(), "toggle".into()];
            self.complete_with_candidates("chui ", rest, &subs);
            return;
        }

        // Common slash commands shared by :all and :G1-G6 completions
        let slash_cmds: Vec<String> = ["/sit", "/stand", "/camp", "/follow", "/assist", "/disband"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();

        // :all <Tab> → common slash commands
        if let Some(rest) = prefix.strip_prefix("all ") {
            self.complete_with_candidates("all ", rest, &slash_cmds);
            return;
        }

        // :G1-G6 <Tab> → common slash commands for group targeting
        let upper_prefix = prefix.to_uppercase();
        if let Some(digit) = upper_prefix
            .strip_prefix('G')
            .and_then(|s| s.chars().next())
            && ('1'..='6').contains(&digit)
            && prefix.len() >= 2
        {
            let cmd_prefix_str = &prefix[..2];
            let rest = prefix[2..].trim_start();
            if !rest.is_empty() {
                self.complete_with_candidates(&format!("{cmd_prefix_str} "), rest, &slash_cmds);
                return;
            }
        }

        // :stop <Tab> → "all" + connected client character names
        if let Some(rest) = prefix.strip_prefix("stop ") {
            let mut names: Vec<String> = vec!["all".into()];
            names.extend(self.clients.iter().map(|c| c.character_name.clone()));
            self.complete_with_candidates("stop ", rest, &names);
            return;
        }

        // :restart <Tab> → "all" + client names (connected + configured accounts)
        if let Some(rest) = prefix.strip_prefix("restart ") {
            let mut names: Vec<String> = vec!["all".into()];
            names.extend(self.clients.iter().map(|c| c.character_name.clone()));
            if let Some(cfg) = &self.accounts_config {
                for acct in &cfg.accounts {
                    if !names
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(&acct.character))
                    {
                        names.push(acct.character.clone());
                    }
                }
            }
            self.complete_with_candidates("restart ", rest, &names);
            return;
        }

        // :nav <Tab> → zone short names from cached meshes + saved camps
        if let Some(rest) = prefix.strip_prefix("nav ") {
            let mut zone_names = self.list_available_zones();
            zone_names.extend(self.list_camp_names());
            zone_names.sort();
            zone_names.dedup();
            self.complete_with_candidates("nav ", rest, &zone_names);
            return;
        }

        // --- Top-level command completion ---
        let mut candidates = command::top_level_completion_candidates();
        candidates.extend([
            String::from("G1"),
            String::from("G2"),
            String::from("G3"),
            String::from("G4"),
            String::from("G5"),
            String::from("G6"),
        ]);

        for client in &self.clients {
            if !client.character_name.is_empty() {
                candidates.push(client.character_name.clone());
            } else if let Some(player) = &client.local_player {
                candidates.push(player.displayed_name.clone());
            }
        }

        self.complete_with_candidates("", prefix, &candidates);
    }

    /// Generic tab-completion helper. Given a command prefix (e.g. "camp "),
    /// the user's partial input, and a list of candidates, complete or show options.
    fn complete_with_candidates(&mut self, cmd_prefix: &str, input: &str, candidates: &[String]) {
        // For multi-word matching, strip leading quote
        let search = input.trim_start_matches('"').to_lowercase();

        let matches: Vec<&String> = candidates
            .iter()
            .filter(|c| c.to_lowercase().starts_with(&search))
            .collect();

        match matches.len() {
            0 => {}
            1 => {
                let name = &matches[0];
                // Quote multi-word names
                let formatted = if name.contains(' ') {
                    format!("\"{name}\"")
                } else {
                    name.to_string()
                };
                self.cmd_state
                    .set_buffer(format!("{cmd_prefix}{formatted} "));
            }
            _ => {
                // Complete common prefix
                let first = matches[0].to_lowercase();
                let common_len = first
                    .char_indices()
                    .take_while(|&(i, ch)| {
                        matches.iter().all(|s| {
                            s.to_lowercase().get(i..i + ch.len_utf8())
                                == first.get(i..i + ch.len_utf8())
                        })
                    })
                    .map(|(i, ch)| i + ch.len_utf8())
                    .last()
                    .unwrap_or(0);

                if common_len > search.len() {
                    let common = &matches[0][..common_len];
                    self.cmd_state.set_buffer(format!("{cmd_prefix}{common}"));
                }
                // Show available options (truncate if too many)
                let display: Vec<&str> = matches.iter().take(10).map(|s| s.as_str()).collect();
                let suffix = if matches.len() > 10 {
                    format!(" (+{} more)", matches.len() - 10)
                } else {
                    String::new()
                };
                self.set_feedback(
                    ToastLevel::Info,
                    format!("Matches: {}{}", display.join(" | "), suffix),
                    false,
                );
            }
        }
    }

    /// List available camp config file names from config/camps/.
    fn list_camp_names(&self) -> Vec<String> {
        let camps_dir = std::path::Path::new("config/camps");
        match std::fs::read_dir(camps_dir) {
            Ok(entries) => entries
                .filter_map(std::result::Result::ok)
                .filter_map(|e| {
                    let path = e.path();
                    if path.extension().is_some_and(|ext| ext == "toml") {
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .map(std::string::ToString::to_string)
                    } else {
                        None
                    }
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List available zone names from cached navmesh files.
    /// Returns zone short names like "permafrost", "eastwastes", etc.
    fn list_available_zones(&self) -> Vec<String> {
        let mesh_dir = std::path::Path::new("data/meshes");
        if let Ok(entries) = std::fs::read_dir(mesh_dir) {
            return entries
                .filter_map(std::result::Result::ok)
                .filter_map(|e| {
                    let path = e.path();
                    if path.extension().is_some_and(|ext| ext == "navmesh") {
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .map(std::string::ToString::to_string)
                    } else {
                        None
                    }
                })
                .collect();
        }

        // Fallback: known TLP zone short names
        const FALLBACK_ZONES: &[&str] = &[
            "permafrost",
            "eastwastes",
            "greatdivide",
            "iceclad",
            "thurgadina",
            "thurgadinb",
            "velketor",
            "kael",
            "skyshrine",
            "westwastes",
            "sirens",
            "cobaltscale",
            "templeveeshan",
            "sleeper",
            "necropolis",
            "crystal",
            "wakening",
            "frozenshadow",
            "gukbottom",
            "guktop",
            "mistmoore",
            "unrest",
            "crushbone",
            "blackburrow",
            "soldungb",
            "soldunga",
            "lavastorm",
            "nektulos",
            "commonlands",
            "freeporteast",
            "freportnorth",
            "freeportwest",
            "northkarana",
            "southkarana",
            "eastkarana",
            "westkarana",
            "highkeep",
            "rivervale",
            "misty",
            "everfrost",
            "halas",
            "qeynos",
        ];
        FALLBACK_ZONES.iter().map(|s| (*s).to_string()).collect()
    }

    /// List all spawn display names in the current zone.
    fn list_character_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .clients
            .iter()
            .filter_map(|c| {
                if !c.character_name.is_empty() {
                    Some(c.character_name.clone())
                } else {
                    c.local_player.as_ref().map(|p| p.displayed_name.clone())
                }
            })
            .collect();
        names.sort();
        names.dedup();
        names
    }

    fn list_spawn_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .spawns
            .iter()
            .map(|s| s.displayed_name.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Update user-tracked spawns against the current spawn list.
    /// Updates tracked spawn statuses against the current spawn list.
    pub fn update_tracked_spawns(&mut self) {
        for tracked in self.tracked_spawns.values_mut() {
            let found = self
                .spawns
                .iter()
                .find(|s| s.displayed_name.to_lowercase() == tracked.name.to_lowercase());
            match found {
                Some(spawn) => {
                    tracked.status = TrackedStatus::Up;
                    tracked.last_seen_tick = Some(self.tick_count);
                    tracked.last_x = spawn.x;
                    tracked.last_y = spawn.y;
                    tracked.last_z = spawn.z;
                }
                None => {
                    if tracked.status == TrackedStatus::Up {
                        tracked.status = TrackedStatus::Down;
                    }
                }
            }
        }
    }

    /// Add a spawn to the user-tracked list.
    pub fn track_spawn(&mut self, name: &str) {
        let key = name.to_lowercase();
        if self.tracked_spawns.contains_key(&key) {
            self.set_feedback(ToastLevel::Info, format!("Already tracking: {name}"), false);
            return;
        }

        // Check if spawn exists in current spawn list
        let found = self
            .spawns
            .iter()
            .find(|s| s.displayed_name.to_lowercase() == key);

        let tracked = match found {
            Some(spawn) => TrackedSpawn {
                name: spawn.displayed_name.clone(),
                status: TrackedStatus::Up,
                last_seen_tick: Some(self.tick_count),
                last_x: spawn.x,
                last_y: spawn.y,
                last_z: spawn.z,
            },
            None => TrackedSpawn {
                name: name.to_string(),
                status: TrackedStatus::Unknown,
                last_seen_tick: None,
                last_x: 0.0,
                last_y: 0.0,
                last_z: 0.0,
            },
        };

        self.set_feedback(
            ToastLevel::Success,
            format!("Tracking: {} [{}]", tracked.name, tracked.status.label()),
            true,
        );
        self.tracked_spawns.insert(key, tracked);
    }

    /// Remove a spawn from the user-tracked list.
    pub fn untrack_spawn(&mut self, name: &str) {
        let key = name.to_lowercase();
        if self.tracked_spawns.remove(&key).is_some() {
            self.set_feedback(ToastLevel::Success, format!("Untracked: {name}"), true);
        } else {
            self.set_feedback(ToastLevel::Warning, format!("Not tracking: {name}"), true);
        }
    }

    /// Get the zone name of the active client, or empty string.
    fn current_zone(&self) -> String {
        self.active_client()
            .map(|c| c.zone_name.clone())
            .unwrap_or_default()
    }

    pub fn update_spawn_alerts(&mut self) {
        let tick = self.tick_count;
        let zone = self.current_zone();
        if self.spawn_watch_named {
            let alerts = self.named_tracker.update(&self.spawns, tick);
            for alert in &alerts {
                let (name, is_up) = match alert {
                    NamedAlert::SpawnUp { name, .. } => (name.clone(), true),
                    NamedAlert::SpawnDown { name, .. } => (name.clone(), false),
                };
                self.spawn_alert_feed.push(SpawnAlertEvent {
                    spawn_name: name.clone(),
                    zone: zone.clone(),
                    is_up,
                    timestamp: std::time::SystemTime::now(),
                    tick,
                    match_source: MatchSource::Named,
                });
                let label = if is_up { "UP" } else { "DOWN" };
                let level = if is_up {
                    ToastLevel::Success
                } else {
                    ToastLevel::Warning
                };
                self.set_feedback(level, format!("[Named] {name} {label} in {zone}"), true);
            }
        }
        self.check_watched_spawn_changes();
    }

    fn check_watched_spawn_changes(&mut self) {
        use crate::eq::named_tracker::is_named;
        if self.spawn_alert_feed.watch_patterns().is_empty() {
            return;
        }
        let tick = self.tick_count;
        let zone = self.current_zone();

        // Build set of currently matched spawn names.
        let mut current_matched: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut new_events: Vec<SpawnAlertEvent> = Vec::new();
        for spawn in &self.spawns {
            if is_named(&spawn.displayed_name) {
                continue;
            }
            if let Some(pattern) = self
                .spawn_alert_feed
                .matches_any_pattern(&spawn.displayed_name)
            {
                current_matched.insert(spawn.displayed_name.clone());
                // Only alert on first appearance.
                if !self
                    .spawn_alert_feed
                    .watched_seen()
                    .contains(&spawn.displayed_name)
                {
                    new_events.push(SpawnAlertEvent {
                        spawn_name: spawn.displayed_name.clone(),
                        zone: zone.clone(),
                        is_up: true,
                        timestamp: std::time::SystemTime::now(),
                        tick,
                        match_source: MatchSource::WatchPattern(pattern.to_string()),
                    });
                }
            }
        }

        // Detect despawns: names previously seen but no longer matched.
        let previously_seen: Vec<String> = self
            .spawn_alert_feed
            .watched_seen()
            .iter()
            .filter(|n| !current_matched.contains(n.as_str()))
            .cloned()
            .collect();
        for name in &previously_seen {
            self.spawn_alert_feed.unmark_watched_seen(name);
            let event = SpawnAlertEvent {
                spawn_name: name.clone(),
                zone: zone.clone(),
                is_up: false,
                timestamp: std::time::SystemTime::now(),
                tick,
                match_source: MatchSource::WatchPattern(String::new()),
            };
            self.spawn_alert_feed.push(event);
            self.set_feedback(
                ToastLevel::Warning,
                format!("[Watch] {name} DOWN in {zone}"),
                false,
            );
        }

        // Mark new appearances and fire alerts.
        for event in new_events {
            let name = event.spawn_name.clone();
            let z = event.zone.clone();
            self.spawn_alert_feed.mark_watched_seen(name.clone());
            self.spawn_alert_feed.push(event);
            self.set_feedback(
                ToastLevel::Info,
                format!("[Watch] {name} spotted in {z}"),
                false,
            );
        }
    }

    fn execute_watch_command(&mut self, args: &[&str]) {
        match args.first().copied() {
            None => {
                self.usage_feedback(
                    "watch",
                    "Usage: watch <pattern> | watch list | watch named [on|off]",
                );
            }
            Some("list") => {
                let patterns = self.spawn_alert_feed.watch_patterns();
                if patterns.is_empty() {
                    self.set_feedback(
                        ToastLevel::Info,
                        String::from("No watch patterns set"),
                        false,
                    );
                } else {
                    let list: Vec<&str> = patterns.iter().map(|p| p.raw.as_str()).collect();
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Watch patterns: {}", list.join(", ")),
                        false,
                    );
                }
            }
            Some("named") => {
                let new_state = match args.get(1).copied() {
                    Some("on") | Some("true") => true,
                    Some("off") | Some("false") => false,
                    _ => !self.spawn_watch_named,
                };
                self.spawn_watch_named = new_state;
                let label = if new_state { "ON" } else { "OFF" };
                self.set_feedback(
                    ToastLevel::Success,
                    format!("Named spawn alerts: {label}"),
                    true,
                );
            }
            Some(_) => {
                let full = args.join(" ");
                let clean = full.trim_matches('"');
                let parsed = self.spawn_alert_feed.add_watch(clean);
                let msg = format!("Watching: {} ({:?})", parsed.raw, parsed.mode);
                self.set_feedback(ToastLevel::Success, msg, true);
            }
        }
    }

    fn execute_alerts_command(&mut self, args: &[&str]) {
        match args.first().copied() {
            None => {
                let count = self.spawn_alert_feed.len();
                if count == 0 {
                    self.set_feedback(ToastLevel::Info, String::from("Alert feed is empty"), false);
                } else {
                    let events = self.spawn_alert_feed.events();
                    let recent: Vec<String> = events
                        .iter()
                        .rev()
                        .take(5)
                        .map(|e| {
                            let a = if e.is_up { "\u{25b2}" } else { "\u{25bc}" };
                            format!("{a} {}", e.spawn_name)
                        })
                        .collect();
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Alerts ({count}): {}", recent.join(", ")),
                        false,
                    );
                }
            }
            Some("clear") => {
                self.spawn_alert_feed.clear();
                self.set_feedback(
                    ToastLevel::Success,
                    String::from("Alert feed cleared"),
                    true,
                );
            }
            Some("count") => {
                let count = self.spawn_alert_feed.len();
                self.set_feedback(
                    ToastLevel::Info,
                    format!("Alert feed: {count} events"),
                    false,
                );
            }
            Some(sub) => {
                self.usage_feedback("alerts", format!("Unknown subcommand: {sub}"));
            }
        }
    }

    /// Load the zone map for the given zone short name from the map directory.
    pub fn load_zone_map(&mut self, zone_short_name: &str) {
        let zone_short_name = zone_short_name.trim().to_ascii_lowercase();
        if zone_short_name.is_empty() {
            self.map_state.loaded_zone.clear();
            self.map_state.zone_map = None;
            self.map_state.navmesh_overlay = None;
            self.map_state.reset_viewport();
            return;
        }

        // Skip if already loaded for this zone.
        if self.map_state.loaded_zone == zone_short_name {
            if self.map_state.show_navmesh && self.map_state.navmesh_overlay.is_none() {
                self.load_zone_navmesh_overlay(&zone_short_name);
            }
            return;
        }
        match crate::eq::map_parser::load_zone_map(&self.map_state.map_dir, &zone_short_name) {
            Ok(map) if !map.lines.is_empty() || !map.points.is_empty() => {
                tracing::info!(
                    zone = zone_short_name.as_str(),
                    lines = map.lines.len(),
                    points = map.points.len(),
                    "Loaded zone map"
                );
                self.map_state.loaded_zone = zone_short_name.clone();
                self.map_state.zone_map = Some(map);
            }
            Ok(_) => {
                tracing::debug!(
                    zone = zone_short_name.as_str(),
                    "No map data found for zone"
                );
                self.map_state.loaded_zone = zone_short_name.clone();
                self.map_state.zone_map = None;
            }
            Err(e) => {
                tracing::warn!(zone = zone_short_name.as_str(), error = %e, "Failed to load zone map");
                self.map_state.loaded_zone = zone_short_name.clone();
                self.map_state.zone_map = None;
            }
        }

        self.map_state.reset_viewport();
        self.map_state.navmesh_overlay = None;
        if self.map_state.show_navmesh {
            self.load_zone_navmesh_overlay(&zone_short_name);
        }
    }

    /// Reload the zone map for the currently selected client's zone.
    /// Called after switching clients or when a zone change is detected.
    pub fn reload_map_for_selected_client(&mut self) {
        if let Some(client) = self.clients.get(self.selected_client) {
            let zone = super::run::zone_to_short_name(&client.zone_name);
            self.load_zone_map(&zone);
        }
    }

    fn load_zone_navmesh_overlay(&mut self, zone_short_name: &str) {
        #[cfg(not(windows))]
        {
            tracing::debug!(
                zone = zone_short_name,
                "Skipping navmesh overlay load on non-Windows"
            );
            self.map_state.navmesh_overlay = None;
        }

        #[cfg(windows)]
        match crate::nav::mesh::load_zone_overlay(zone_short_name) {
            Ok(overlay) if !overlay.is_empty() => {
                tracing::info!(
                    zone = zone_short_name,
                    outer_lines = overlay.outer_lines.len(),
                    inner_lines = overlay.inner_lines.len(),
                    "Loaded navmesh overlay"
                );
                self.map_state.navmesh_overlay = Some(overlay);
            }
            Ok(_) => {
                tracing::debug!(
                    zone = zone_short_name,
                    "Navmesh overlay contained no segments"
                );
                self.map_state.navmesh_overlay = None;
            }
            Err(error) => {
                tracing::warn!(zone = zone_short_name, %error, "Failed to load navmesh overlay");
                self.map_state.navmesh_overlay = None;
            }
        }
    }

    /// Parse a group prefix like "G1", "G2", ..., "G6" from the first word.
    /// Returns (`group_idx` 0-based, remaining command) if found.
    fn parse_group_prefix<'a>(&self, input: &'a str) -> Option<(usize, &'a str)> {
        let trimmed = input.trim();
        let bytes = trimmed.as_bytes();
        if bytes.len() >= 2 && (bytes[0] == b'G' || bytes[0] == b'g') && bytes[1].is_ascii_digit() {
            let num = (bytes[1] - b'0') as usize;
            if (1..=6).contains(&num) {
                let rest = trimmed[2..].trim();
                return Some((num - 1, rest));
            }
        }
        None
    }

    /// Parse a direct client target from the first word.
    /// Examples: `Tank`, `cleric01 /sit`, `@all /sit`.
    fn parse_client_prefix<'a>(&self, input: &'a str) -> Option<(usize, &'a str)> {
        let trimmed = input.trim();
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let raw_target = parts.next()?;
        let rest = parts.next().map_or("", str::trim);
        let forced = raw_target.starts_with('@');
        let target = raw_target.trim_start_matches('@');

        if target.is_empty() || (!forced && is_reserved_command_name(target)) {
            return None;
        }

        self.find_client_index_by_name(target)
            .map(|idx| (idx, rest))
    }

    fn resolve_nav_target(
        &self,
        args: &[&str],
    ) -> Option<(String, textquest_common::nav::Waypoint, Option<String>)> {
        let normalized: Vec<&str> = args
            .iter()
            .copied()
            .filter(|part| !part.is_empty())
            .collect();
        if normalized.is_empty() {
            return None;
        }

        if normalized.len() == 3
            && let (Ok(x), Ok(y), Ok(z)) = (
                normalized[0].parse::<f32>(),
                normalized[1].parse::<f32>(),
                normalized[2].parse::<f32>(),
            )
        {
            return Some((
                format!("{x:.0} {y:.0} {z:.0}"),
                textquest_common::nav::Waypoint::new(x, y, z),
                None,
            ));
        }

        let destination = normalized.join(" ");
        CampConfig::load(&destination).ok().map(|camp| {
            (
                destination,
                textquest_common::nav::Waypoint::new(
                    camp.camp_center[0],
                    camp.camp_center[1],
                    camp.camp_center[2],
                ),
                Some(super::run::zone_to_short_name(&camp.zone)),
            )
        })
    }

    fn execute_waypoint_navigation(
        &mut self,
        destination_label: &str,
        target: textquest_common::nav::Waypoint,
        zone_hint: Option<&str>,
    ) {
        let focused_clients: Vec<FocusedNavClient> = self
            .routed_clients()
            .into_iter()
            .filter_map(|client| {
                client.local_player.as_ref().map(|player| FocusedNavClient {
                    pid: client.pid,
                    client_name: self.client_command_target(client),
                    zone_short: super::run::zone_to_short_name(&client.zone_name),
                    position: (player.x, player.y, player.z),
                    is_demo: client.is_demo,
                })
            })
            .collect();

        if focused_clients.is_empty() {
            self.set_feedback(
                ToastLevel::Warning,
                String::from("No focused clients with position data for navigation."),
                true,
            );
            return;
        }

        let mut mesh_routes = 0usize;
        let mut fallback_routes = 0usize;
        let mut blocked_routes = 0usize;
        let mut sent = 0usize;
        let mut previews = 0usize;
        let mut skipped = 0usize;
        let mut failed = 0usize;

        for focused_client in focused_clients {
            if let Some(expected_zone) = zone_hint
                && focused_client.zone_short != expected_zone
            {
                self.nav_state.nav_statuses.insert(
                    focused_client.pid,
                    NavClientStatus {
                        destination: destination_label.to_string(),
                        status: textquest_common::nav::NavStatus::Idle,
                        eta_secs: None,
                        waypoints: Vec::new(),
                        path_exists: false,
                        path_length: None,
                        failure_reason: Some(format!(
                            "Zone mismatch: {} vs expected {}",
                            focused_client.zone_short, expected_zone
                        )),
                        route_state: String::from("Awaiting zone match"),
                        recovery_state: Some(String::from("Zone transition pending")),
                        blockers: vec![format!(
                            "Current zone {} does not match route zone {}.",
                            focused_client.zone_short, expected_zone
                        )],
                        is_demo_scripted: false,
                    },
                );
                skipped += 1;
                continue;
            }

            let route = crate::nav::mesh::plan_route(
                &focused_client.zone_short,
                focused_client.position,
                (target.x, target.y, target.z),
            );

            match route.source {
                crate::nav::mesh::RouteSource::NavMesh => mesh_routes += 1,
                crate::nav::mesh::RouteSource::StraightLineFallback => fallback_routes += 1,
                crate::nav::mesh::RouteSource::NavMeshBlocked => blocked_routes += 1,
            }

            let delivered = if route.waypoints.is_empty() {
                false
            } else if focused_client.is_demo {
                previews += 1;
                true
            } else {
                let cmd = textquest_common::ipc::Command::NavigateTo {
                    waypoints: route.waypoints.clone(),
                };
                match send_ipc_command(focused_client.pid, &cmd) {
                    Ok(()) => {
                        sent += 1;
                        true
                    }
                    Err(error) => {
                        failed += 1;
                        tracing::warn!(
                            pid = focused_client.pid,
                            client = %focused_client.client_name,
                            %error,
                            "Failed to send NavigateTo from TUI"
                        );
                        false
                    }
                }
            };

            if delivered || matches!(route.source, crate::nav::mesh::RouteSource::NavMeshBlocked) {
                let failure_kind = route.metrics.failure_kind;
                let from = textquest_common::nav::Waypoint::new(
                    focused_client.position.0,
                    focused_client.position.1,
                    focused_client.position.2,
                );
                let distance_remaining = from.distance_3d(&target);
                let waypoint_count = route.waypoints.len().max(1);
                let status =
                    if matches!(route.source, crate::nav::mesh::RouteSource::NavMeshBlocked) {
                        textquest_common::nav::NavStatus::Idle
                    } else if distance_remaining <= 5.0 {
                        textquest_common::nav::NavStatus::Arrived
                    } else {
                        textquest_common::nav::NavStatus::Moving {
                            waypoint_index: 0,
                            waypoint_count,
                            distance_remaining,
                        }
                    };
                let route_state = match route.source {
                    crate::nav::mesh::RouteSource::NavMesh => String::from("Navmesh route"),
                    crate::nav::mesh::RouteSource::StraightLineFallback => {
                        String::from("Fallback route")
                    }
                    crate::nav::mesh::RouteSource::NavMeshBlocked => String::from("Replan blocked"),
                };
                let recovery_state = if route.metrics.replan_recommended {
                    Some(String::from("Replan recommended"))
                } else {
                    None
                };
                let blockers = match route.source {
                    crate::nav::mesh::RouteSource::NavMesh => Vec::new(),
                    crate::nav::mesh::RouteSource::StraightLineFallback => {
                        let failure_label =
                            failure_kind.map(|kind| kind.label()).unwrap_or("data gap");
                        vec![format!(
                            "Navmesh {failure_label} for {}; using deterministic straight-line fallback.",
                            focused_client.zone_short
                        )]
                    }
                    crate::nav::mesh::RouteSource::NavMeshBlocked => {
                        let failure_label = failure_kind
                            .map(|kind| kind.label())
                            .unwrap_or("transient blockage");
                        let mut blockers = vec![format!(
                            "Navmesh {failure_label} for {}; holding position instead of taking a straight-line shortcut.",
                            focused_client.zone_short
                        )];
                        if let Some(reason) = route.metrics.failure_reason.as_ref() {
                            blockers.push(reason.clone());
                        }
                        blockers
                    }
                };

                self.nav_state.nav_statuses.insert(
                    focused_client.pid,
                    NavClientStatus {
                        destination: destination_label.to_string(),
                        status,
                        eta_secs: None,
                        waypoints: route.waypoints,
                        path_exists: route.metrics.path_exists,
                        path_length: route.metrics.path_length,
                        failure_reason: route.metrics.failure_reason.clone(),
                        route_state,
                        recovery_state,
                        blockers,
                        is_demo_scripted: false,
                    },
                );
            }
        }

        let mut details = Vec::new();
        if mesh_routes > 0 {
            details.push(format!("{} mesh", mesh_routes));
        }
        if fallback_routes > 0 {
            details.push(format!("{} fallback", fallback_routes));
        }
        if blocked_routes > 0 {
            details.push(format!("{} blocked", blocked_routes));
        }
        if sent > 0 {
            details.push(format!("{} sent", sent));
        }
        if previews > 0 {
            details.push(format!("{} preview", previews));
        }
        if skipped > 0 {
            details.push(format!("{} skipped", skipped));
        }
        if failed > 0 {
            details.push(format!("{} failed", failed));
        }

        self.set_feedback(
            if failed > 0 || blocked_routes > 0 || (sent == 0 && previews == 0) {
                ToastLevel::Warning
            } else {
                ToastLevel::Success
            },
            format!(
                "Nav → {} ({})",
                destination_label,
                if details.is_empty() {
                    String::from("no clients routed")
                } else {
                    details.join(", ")
                }
            ),
            sent > 0 || previews > 0 || failed > 0,
        );

        if sent > 0 || previews > 0 {
            self.set_active_screen(ActiveScreen::Tactical);
            if self.tactical_state.show_navigation {
                self.active_panel = ActivePanel::TacticalNavigation;
            }
        }
    }

    fn parse_all_prefix<'a>(&self, input: &'a str) -> Option<&'a str> {
        let trimmed = input.trim();
        let rest = trimmed.strip_prefix("all")?;
        if rest.is_empty() || !rest.starts_with(char::is_whitespace) {
            return None;
        }
        Some(rest.trim())
    }

    fn with_temporary_routing_scope<T>(
        &mut self,
        scope: textquest_common::routing::RoutingScope,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let previous_scope = std::mem::replace(&mut self.routing_scope, scope);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self)));
        self.routing_scope = previous_scope;
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn routed_pids_for_scope(
        &mut self,
        scope: &textquest_common::routing::RoutingScope,
    ) -> Vec<u32> {
        match scope {
            textquest_common::routing::RoutingScope::AllSession => {
                self.clients.iter().map(|client| client.pid).collect()
            }
            _ => self.with_temporary_routing_scope(scope.clone(), |app| app.focused_pids()),
        }
    }

    fn dispatch_scoped_slash_command(
        &mut self,
        scope: textquest_common::routing::RoutingScope,
        slash_cmd: &str,
    ) {
        let pids = self.routed_pids_for_scope(&scope);

        if pids.is_empty() {
            self.set_feedback(
                ToastLevel::Warning,
                format!("{}: no online members.", scope.label()),
                true,
            );
            return;
        }

        let mut ok = 0usize;
        let mut fail = 0usize;
        for pid in &pids {
            match send_slash_command(*pid, slash_cmd) {
                Ok(()) => ok += 1,
                Err(_) => fail += 1,
            }
        }

        self.set_feedback(
            if fail > 0 {
                ToastLevel::Warning
            } else {
                ToastLevel::Success
            },
            format!(
                "{} {slash_cmd} → sent to {ok}, failed {fail}",
                scope.label()
            ),
            fail > 0 || ok > 0,
        );
    }

    fn split_command<'a>(&self, input: &'a str) -> (&'a str, &'a str) {
        let trimmed = input.trim();
        match trimmed.split_once(char::is_whitespace) {
            Some((command, rest)) => (command, rest.trim()),
            None => (trimmed, ""),
        }
    }

    fn usage_feedback(&mut self, command: &str, reason: impl Into<String>) {
        let reason = reason.into();
        if let Some(entry) = command::command_entry(command) {
            self.set_feedback(
                ToastLevel::Warning,
                format!(
                    "{reason} Usage: {}. Example: :{}",
                    entry.usage, entry.example
                ),
                true,
            );
        } else {
            self.set_feedback(ToastLevel::Warning, reason, true);
        }
    }

    fn unknown_command_feedback(&mut self, input: &str) {
        if let Some(suggestion) = command::did_you_mean(input) {
            let alias_note = suggestion
                .alias
                .map(|alias| format!(" Alias: :{alias}."))
                .unwrap_or_default();
            self.set_feedback(
                ToastLevel::Warning,
                format!(
                    "Unknown command: '{input}'. Did you mean :{}?{}",
                    suggestion.phrase, alias_note
                ),
                true,
            );
        } else {
            self.set_feedback(
                ToastLevel::Warning,
                format!(
                    "Unknown command: '{input}'. Use :help for workflows or :commands for the reference."
                ),
                true,
            );
        }
    }

    /// Execute the current command buffer content.
    pub fn execute_command(&mut self, orchestrator: &mut Orchestrator) {
        let input = self.cmd_state.command_buffer.trim().to_string();
        if input.is_empty() {
            return;
        }
        let input = command::normalize_command_alias(&input);

        // Save to history and track frequency for favorites
        self.cmd_state.command_history.push(input.clone());
        self.cmd_state.record_command(&input);
        if let Err(e) = self.cmd_state.save_history_to_disk() {
            tracing::debug!(error = %e, "Failed to persist command history");
        }

        self.execute_normalized_command(&input, orchestrator);
    }

    fn execute_normalized_command(&mut self, input: &str, orchestrator: &mut Orchestrator) {
        let input = command::normalize_command_alias(input);

        if let Some((group_idx, rest)) = self.parse_group_prefix(&input) {
            let Some(group) = self.groups.get(group_idx) else {
                self.usage_feedback(
                    "scope",
                    format!("Group G{} not found. Use :scope G1..G6.", group_idx + 1),
                );
                return;
            };

            let scope = textquest_common::routing::RoutingScope::Group {
                group_id: group.id,
                label: group.name.clone(),
            };

            if rest.is_empty() {
                self.routing_scope = scope;
                self.set_active_group(Some(group_idx));
                self.set_feedback(
                    ToastLevel::Info,
                    format!("Scope changed to G{}", group_idx + 1),
                    false,
                );
            } else if rest.starts_with('/') {
                self.dispatch_scoped_slash_command(scope, rest);
            } else {
                self.with_temporary_routing_scope(scope, |app| {
                    app.execute_normalized_command(rest, orchestrator);
                });
            }
            return;
        }

        if let Some(rest) = self.parse_all_prefix(&input) {
            let scope = textquest_common::routing::RoutingScope::AllSession;
            if rest.starts_with('/') {
                self.dispatch_scoped_slash_command(scope, rest);
            } else {
                self.with_temporary_routing_scope(scope, |app| {
                    app.execute_normalized_command(rest, orchestrator);
                });
            }
            return;
        }

        if let Some((client_idx, rest)) = self.parse_client_prefix(&input) {
            let target_name = self.client_command_target(&self.clients[client_idx]);
            let scope = textquest_common::routing::RoutingScope::OneToon {
                name: target_name.clone(),
            };

            if rest.is_empty() {
                self.routing_scope = scope;
                self.active_group = None;
                self.select_client_idx(client_idx);
                self.expand_selected_character();
                self.set_feedback(
                    ToastLevel::Info,
                    format!("Focused client: {target_name}"),
                    false,
                );
            } else if rest.starts_with('/') {
                self.select_client_idx(client_idx);
                self.dispatch_scoped_slash_command(scope, rest);
            } else {
                self.with_temporary_routing_scope(scope, |app| {
                    app.execute_normalized_command(rest, orchestrator);
                });
            }
            return;
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        let (command_name, rest) = self.split_command(&input);
        match command_name {
            "help" => {
                if rest.is_empty() {
                    self.help_scroll = 0;
                    self.open_help(HelpFocus::Section(HelpSection::Workflows));
                    self.set_feedback(
                        ToastLevel::Info,
                        String::from("Help opened. Start with the live workflows section."),
                        false,
                    );
                } else if let Some(entry) = command::command_entry(rest) {
                    self.open_help(HelpFocus::Command(entry.phrase));
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Help opened for :{}", entry.phrase),
                        false,
                    );
                } else if let Some(section) = command::help_section_for_command(rest) {
                    self.open_help(HelpFocus::Section(section));
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Help opened for '{rest}'."),
                        false,
                    );
                } else {
                    self.usage_feedback("help", format!("Unknown help topic '{rest}'."));
                }
            }
            "commands" => {
                self.open_help(HelpFocus::Section(HelpSection::Combat));
                self.set_feedback(
                    ToastLevel::Info,
                    String::from("Command reference opened."),
                    false,
                );
            }
            "camp" => {
                self.execute_camp_command(&parts[1..], orchestrator);
            }
            "nav" => {
                let destination = rest.to_string();
                if destination.is_empty() {
                    self.usage_feedback("nav", "Missing navigation target.");
                } else if let Some((label, target, zone_hint)) =
                    self.resolve_nav_target(&parts[1..])
                {
                    self.execute_waypoint_navigation(&label, target, zone_hint.as_deref());
                } else {
                    let cmd = textquest_common::ipc::Command::SlashCommand {
                        command: format!("/nav to {destination}"),
                    };
                    let ok = self.send_ipc_to_focused(&cmd);
                    if ok == 0 {
                        self.set_feedback(
                            ToastLevel::Warning,
                            String::from(
                                "No clients connected for navigation. Use :status to confirm scope.",
                            ),
                            true,
                        );
                    } else {
                        tracing::info!(destination, sent = ok, "Navigation slash command sent");
                        self.set_feedback(
                            ToastLevel::Success,
                            format!("Nav slash → {destination} (sent to {ok} clients)"),
                            true,
                        );
                        self.set_active_screen(ActiveScreen::Tactical);
                        if self.tactical_state.show_navigation {
                            self.active_panel = ActivePanel::TacticalNavigation;
                        }
                    }
                }
            }
            "mapfilter" => self.handle_mapfilter_command(&parts),
            "mapclick" => self.handle_mapclick_command(&parts),
            "maploc" => self.handle_maploc_command(&parts),
            "mapmarker" => self.handle_mapmarker_command(&parts),
            "mapshow" => self.handle_mapshow_command(&parts),
            "maphide" => self.handle_maphide_command(&parts),
            "mapnames" => self.handle_mapnames_command(&parts),
            "map" => self.handle_map_layer_command(&parts),
            "highlight" | "hl" => self.handle_highlight_command(&parts),
            "circle" => self.handle_circle_command(&parts, orchestrator),
            "loot" => {
                let ok = self.send_ipc_to_focused(&textquest_common::ipc::Command::LootCorpse);
                if ok == 0 {
                    self.set_feedback(
                        ToastLevel::Warning,
                        "Loot: no clients received command. Check connection with :status",
                        true,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Loot → sent to {ok} clients"),
                        false,
                    );
                }
            }
            "door" => {
                let ok = self.send_ipc_to_focused(&textquest_common::ipc::Command::InteractDoor);
                if ok == 0 {
                    self.set_feedback(
                        ToastLevel::Warning,
                        "Door: no clients received command. Check connection with :status",
                        true,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Door interact → sent to {ok} clients"),
                        false,
                    );
                }
            }
            "click" => {
                let sub = parts.get(1).map(|s| s.to_ascii_lowercase());
                let cmd = match sub.as_deref() {
                    Some("door") => textquest_common::ipc::Command::InteractDoor,
                    _ => textquest_common::ipc::Command::ClickObject,
                };
                let label = if matches!(sub.as_deref(), Some("door")) {
                    "door"
                } else {
                    "item"
                };
                let ok = self.send_ipc_to_focused(&cmd);
                if ok == 0 {
                    self.set_feedback(
                        ToastLevel::Warning,
                        format!("Click {label}: no clients received command. Check connection with :status"),
                        true,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Click {label} → sent to {ok} clients"),
                        false,
                    );
                }
            }
            "status" => {
                let client_count = self.clients.len();
                let visible_count = self.visible_clients().len();
                let status_arg = parts.get(1).map(|s| s.to_ascii_lowercase());
                match status_arg.as_deref() {
                    Some("overview") => {
                        let zone = self
                            .active_client()
                            .map_or_else(String::new, |client| client.zone_name.clone());
                        let active_screen = self.active_screen.label();
                        let active_mode = self.operating_mode;
                        let clients_in_filter = if self.active_group.is_some() {
                            visible_count
                        } else {
                            client_count
                        };
                        let map_zone = self.map_state.loaded_zone.clone();
                        self.set_feedback(
                            ToastLevel::Info,
                            format!(
                            "Overview: {client_count} connected, {clients_in_filter} visible | zone={zone} | mode={active_mode:?} | screen={active_screen} | map={map_zone}",
                            ),
                            false,
                        );
                        if self.wizard_state.active {
                            self.status_message.push_str(" | wizard active");
                        }
                        if self.config_panel_state.active {
                            self.status_message.push_str(" | config panel open");
                        }
                    }
                    _ => {
                        if self.active_group.is_some() {
                            self.set_feedback(
                                ToastLevel::Info,
                                format!(
                                "{visible_count} visible / {client_count} total client(s) connected"
                                ),
                                false,
                            );
                        } else {
                            self.set_feedback(
                                ToastLevel::Info,
                                format!("{client_count} client(s) connected"),
                                false,
                            );
                        }
                    }
                }
            }
            "login" => {
                self.execute_login_command(&parts[1..]);
            }
            "profile" => {
                self.execute_profile_command(&parts[1..]);
            }
            "stop" => {
                self.execute_stop_command(&parts[1..], orchestrator);
            }
            "restart" => {
                self.execute_restart_command(&parts[1..], orchestrator);
            }
            "track" => {
                self.execute_track_command(&parts[1..]);
            }
            "untrack" => {
                if parts.get(1).is_some() {
                    // Rejoin remaining parts for multi-word names, strip quotes
                    let full_name = parts[1..].join(" ");
                    let clean = full_name.trim_matches('"');
                    self.untrack_spawn(clean);
                } else {
                    self.usage_feedback("untrack", "Missing tracked spawn name.");
                }
            }
            "watch" => {
                self.execute_watch_command(&parts[1..]);
            }
            "unwatch" => {
                if parts.get(1).is_some() {
                    let full = parts[1..].join(" ");
                    let clean = full.trim_matches('"');
                    if self.spawn_alert_feed.remove_watch(clean) {
                        self.set_feedback(
                            ToastLevel::Success,
                            format!("Removed watch: {clean}"),
                            true,
                        );
                    } else {
                        self.set_feedback(
                            ToastLevel::Warning,
                            format!("No watch pattern: {clean}"),
                            true,
                        );
                    }
                } else {
                    self.usage_feedback("unwatch", "Missing pattern.");
                }
            }
            "alerts" => {
                self.execute_alerts_command(&parts[1..]);
            }
            "mode" => match parts.get(1).copied() {
                Some("camp") => {
                    self.operating_mode = crate::camp::hunt::OperatingMode::Camp;
                    self.set_feedback(
                        ToastLevel::Success,
                        String::from("Switched to Camp mode"),
                        true,
                    );
                }
                Some("hunt") => {
                    self.operating_mode = crate::camp::hunt::OperatingMode::Hunt;
                    self.set_feedback(
                        ToastLevel::Success,
                        String::from("Switched to Hunt mode"),
                        true,
                    );
                }
                Some(other) => {
                    self.usage_feedback("mode", format!("Invalid mode '{other}'."));
                }
                None => {
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Current mode: {}", self.operating_mode),
                        false,
                    );
                }
            },
            "combat" => match parts.get(1).copied() {
                None | Some("status") | Some("summary") => {
                    self.set_feedback(ToastLevel::Info, self.combat_status_summary(), false);
                }
                Some("scope") => {
                    self.set_feedback(
                        ToastLevel::Info,
                        format!(
                            "Combat scope: {} | focused {} clients",
                            self.routing_scope.label(),
                            self.focused_pids().len()
                        ),
                        false,
                    );
                }
                Some(other) => {
                    self.usage_feedback("combat", format!("Unknown combat option '{other}'."));
                }
            },
            "ma" => {
                if let Some(name) = parts.get(1) {
                    self.main_assist = Some(name.to_string());
                    // Send /assist command to all DPS in active group
                    let pids = self.focused_pids();
                    let mut ok = 0;
                    for pid in &pids {
                        if send_slash_command(*pid, &format!("/assist {name}")).is_ok() {
                            ok += 1;
                        }
                    }
                    tracing::info!(target = %name, sent = ok, "Main Assist set");
                    self.set_feedback(
                        ToastLevel::Success,
                        format!("MA → {name} (sent /assist to {ok} clients)"),
                        true,
                    );
                } else {
                    match &self.main_assist {
                        Some(ma) => {
                            self.set_feedback(
                                ToastLevel::Info,
                                format!("Main Assist: {ma}"),
                                false,
                            );
                        }
                        None => self.usage_feedback("ma", "No Main Assist is set."),
                    }
                }
            }
            "mt" => {
                if let Some(name) = parts.get(1) {
                    self.main_tank = Some(name.to_string());
                    tracing::info!(target = %name, "Main Tank set");
                    self.set_feedback(ToastLevel::Success, format!("MT → {name}"), true);
                } else {
                    match &self.main_tank {
                        Some(mt) => {
                            self.set_feedback(ToastLevel::Info, format!("Main Tank: {mt}"), false);
                        }
                        None => self.usage_feedback("mt", "No Main Tank is set."),
                    }
                }
            }
            "engage" => {
                let target_id = if rest.is_empty() {
                    0
                } else if let Ok(target_id) = rest.parse::<u32>() {
                    target_id
                } else {
                    self.usage_feedback("engage", format!("Invalid target id '{rest}'."));
                    return;
                };
                let ok = self.send_ipc_to_focused(&textquest_common::ipc::Command::CombatEngage {
                    target_id,
                });
                tracing::info!(target_id, sent = ok, "Combat engage sent");
                if ok == 0 {
                    self.set_feedback(
                        ToastLevel::Warning,
                        "Engage: no clients received command. Check connection with :status",
                        true,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Success,
                        format!("Engage → {ok} clients (target_id={target_id})"),
                        true,
                    );
                }
            }
            "disengage" => {
                if !rest.is_empty() {
                    self.usage_feedback("disengage", "Unexpected arguments.");
                    return;
                }
                let ok = self.send_ipc_to_focused(&textquest_common::ipc::Command::CombatDisengage);
                tracing::info!(sent = ok, "Combat disengage sent");
                if ok == 0 {
                    self.set_feedback(
                        ToastLevel::Warning,
                        "Disengage: no clients received command. Check connection with :status",
                        true,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Success,
                        format!("Disengage → {ok} clients"),
                        true,
                    );
                }
            }
            "invite" => {
                if let Some(name) = parts.get(1) {
                    if let Some(client) = self.active_client() {
                        let pid = client.pid;
                        let slash = format!("/invite {name}");
                        match send_slash_command(pid, &slash) {
                            Ok(()) => {
                                tracing::info!(target = %name, pid, "Group invite sent");
                                let from = self.client_command_target(client);
                                self.set_feedback(
                                    ToastLevel::Success,
                                    format!("Invited {name} from {from}"),
                                    true,
                                );
                            }
                            Err(e) => {
                                self.set_feedback(
                                    ToastLevel::Error,
                                    format!("Invite failed: {e}"),
                                    true,
                                );
                            }
                        }
                    } else {
                        self.set_feedback(
                            ToastLevel::Warning,
                            String::from("No active client to send invite from."),
                            true,
                        );
                    }
                } else {
                    self.usage_feedback("invite", "Missing invite target.");
                }
            }
            "accept" => {
                if !rest.is_empty() {
                    self.usage_feedback("accept", "Unexpected arguments.");
                    return;
                }
                if let Some(client) = self.active_client() {
                    let pid = client.pid;
                    match send_slash_command(pid, "/accept") {
                        Ok(()) => {
                            tracing::info!(pid, "Group invite accepted");
                            let on_client = self.client_command_target(client);
                            self.set_feedback(
                                ToastLevel::Success,
                                format!("Accepted group invite on {on_client}"),
                                true,
                            );
                        }
                        Err(e) => {
                            self.set_feedback(
                                ToastLevel::Error,
                                format!("Accept failed: {e}"),
                                true,
                            );
                        }
                    }
                } else {
                    self.set_feedback(
                        ToastLevel::Warning,
                        String::from("No active client to accept on."),
                        true,
                    );
                }
            }
            "heal" => {
                if let Some("cancel") = parts.get(1).copied() {
                    self.heal_cancel_enabled = !self.heal_cancel_enabled;
                    let state = if self.heal_cancel_enabled {
                        "ON"
                    } else {
                        "OFF"
                    };
                    tracing::info!(enabled = self.heal_cancel_enabled, "Heal-cancel toggled");
                    self.set_feedback(ToastLevel::Success, format!("Heal-cancel: {state}"), true);
                } else if rest.is_empty() {
                    let state = if self.heal_cancel_enabled {
                        "ON"
                    } else {
                        "OFF"
                    };
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Heal-cancel is {state}. Use :heal cancel to toggle it."),
                        false,
                    );
                } else {
                    self.usage_feedback("heal cancel", format!("Unknown heal option '{rest}'."));
                }
            }
            "ch" => {
                self.execute_ch_command(&parts[1..], orchestrator);
            }
            "inject" => {
                if self.clients.is_empty() {
                    self.set_feedback(
                        ToastLevel::Warning,
                        String::from("Inject: no clients connected. Connect a client first."),
                        true,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Info,
                        "Inject: DLL injection placeholder (not yet wired). Will inject into active client.",
                        true,
                    );
                }
            }
            "quit" => {
                self.running = false;
                self.set_feedback(
                    ToastLevel::Info,
                    String::from("Shutting down TextQuest TUI..."),
                    false,
                );
            }
            "all" => {
                if rest.is_empty() {
                    self.usage_feedback("all", "Missing slash command to broadcast.");
                } else if !rest.starts_with('/') {
                    self.usage_feedback("all", "Broadcasts require a slash command.");
                } else {
                    let pids: Vec<u32> = self.clients.iter().map(|c| c.pid).collect();
                    if pids.is_empty() {
                        self.set_feedback(
                            ToastLevel::Warning,
                            format!(
                                "all {rest}: no clients connected. Use :login to connect first."
                            ),
                            true,
                        );
                        return;
                    }
                    let mut ok = 0usize;
                    let mut fail = 0usize;
                    for pid in &pids {
                        match send_slash_command(*pid, rest) {
                            Ok(()) => ok += 1,
                            Err(_) => fail += 1,
                        }
                    }
                    self.set_feedback(
                        if fail > 0 {
                            ToastLevel::Warning
                        } else {
                            ToastLevel::Success
                        },
                        format!("all {rest} → sent to {ok}, failed {fail}"),
                        fail > 0 || ok > 0,
                    );
                }
            }
            "scope" => {
                self.execute_scope_command(&parts[1..]);
            }
            "session" => {
                self.execute_session_command(&parts[1..]);
            }
            "wizard" => {
                self.wizard_state.start();
                self.set_feedback(
                    ToastLevel::Info,
                    String::from("Starting setup wizard..."),
                    false,
                );
            }
            "config" => {
                self.config_panel_state.active = !self.config_panel_state.active;
                if self.config_panel_state.active {
                    let group_names: Vec<String> =
                        self.groups.iter().map(|g| g.name.clone()).collect();
                    let toon_names = self.list_character_names();
                    self.config_panel_state
                        .set_available_scopes(group_names, toon_names);
                    self.config_panel_state.rebuild_tree();
                    self.config_panel_state.sync_from_app(
                        self.theme_kind.label(),
                        self.privacy_mode,
                        self.main_assist.as_deref(),
                        self.main_tank.as_deref(),
                        self.heal_cancel_enabled,
                        &format!("{}", self.operating_mode),
                    );
                    self.set_feedback(
                        ToastLevel::Info,
                        String::from("Configuration panel opened"),
                        false,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Info,
                        String::from("Configuration panel closed"),
                        false,
                    );
                }
            }
            "chui" => match parts.get(1).copied() {
                Some("open") => {
                    self.ch_chain_panel_state.active = true;
                    self.sync_ch_chain_panel_state(orchestrator);
                    self.set_feedback(
                        ToastLevel::Info,
                        String::from("CH chain panel opened"),
                        false,
                    );
                }
                Some("close") => {
                    self.ch_chain_panel_state.active = false;
                    self.set_feedback(
                        ToastLevel::Info,
                        String::from("CH chain panel closed"),
                        false,
                    );
                }
                Some("toggle") => {
                    self.ch_chain_panel_state.active = !self.ch_chain_panel_state.active;
                    if self.ch_chain_panel_state.active {
                        self.sync_ch_chain_panel_state(orchestrator);
                        self.set_feedback(
                            ToastLevel::Info,
                            String::from("CH chain panel opened"),
                            false,
                        );
                    } else {
                        self.set_feedback(
                            ToastLevel::Info,
                            String::from("CH chain panel closed"),
                            false,
                        );
                    }
                }
                Some("status") => {
                    if let Some(chain) = &self.ch_chain_status {
                        self.set_feedback(
                            ToastLevel::Info,
                            format!(
                                "CH chain: {} clerics, {:.1}s interval ({})",
                                chain.members,
                                chain.interval_secs,
                                if chain.is_adaptive {
                                    "adaptive"
                                } else {
                                    "fixed"
                                }
                            ),
                            false,
                        );
                    } else {
                        self.usage_feedback("ch start", "CH chain inactive.");
                    }
                }
                None => {
                    self.ch_chain_panel_state.active = !self.ch_chain_panel_state.active;
                    if self.ch_chain_panel_state.active {
                        self.sync_ch_chain_panel_state(orchestrator);
                        self.set_feedback(
                            ToastLevel::Info,
                            String::from("CH chain panel opened"),
                            false,
                        );
                    } else {
                        self.set_feedback(
                            ToastLevel::Info,
                            String::from("CH chain panel closed"),
                            false,
                        );
                    }
                }
                Some(other) => {
                    self.usage_feedback("chui", format!("Unknown chui option '{other}'."))
                }
            },
            "theme" => {
                self.cycle_theme();
                self.set_feedback(
                    ToastLevel::Success,
                    format!("Theme: {}", self.theme_kind.label()),
                    true,
                );
            }
            "privacy" => {
                self.toggle_privacy();
                let state = if self.privacy_mode { "ON" } else { "OFF" };
                self.set_feedback(ToastLevel::Success, format!("Privacy mode: {state}"), true);
            }
            _ => {
                // Try to parse first token as PID
                if let Ok(pid) = command_name.parse::<u32>() {
                    if rest.is_empty() || !rest.starts_with('/') {
                        self.set_feedback(
                            ToastLevel::Warning,
                            format!("PID targets expect a slash command. Example: :{pid} /assist Warrior"),
                            true,
                        );
                    } else {
                        match send_slash_command(pid, rest) {
                            Ok(()) => {
                                self.set_feedback(
                                    ToastLevel::Success,
                                    format!("{pid} → {rest}"),
                                    true,
                                );
                            }
                            Err(e) => {
                                self.set_feedback(
                                    ToastLevel::Error,
                                    format!("Error sending to {pid}: {e}"),
                                    true,
                                );
                            }
                        }
                    }
                } else {
                    self.unknown_command_feedback(&input);
                }
            }
        }
    }

    /// Handle `camp <subcommand>` from the command bar.
    fn execute_camp_command(&mut self, args: &[&str], orchestrator: &mut Orchestrator) {
        match args.first().copied() {
            None => {
                self.status_message = String::from(
                    "Usage: camp <start|stop|status|list|add|remove|next|prev> [name]",
                );
            }
            Some("start") => {
                let camp_name = if let Some(name) = args.get(1) {
                    *name
                } else {
                    self.status_message =
                        String::from("Usage: camp start <name>  (loads config/camps/<name>.toml)");
                    return;
                };

                match CampConfig::load(camp_name) {
                    Ok(config) => {
                        let members = self.build_camp_members();
                        if members.is_empty() {
                            self.status_message =
                                String::from("No clients connected — cannot start camp");
                            return;
                        }
                        let count = members.len();
                        orchestrator.start_camp(config, members);
                        self.status_message =
                            format!("Camp '{camp_name}' started with {count} members");
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to load camp '{camp_name}': {e}");
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
            Some("list") => {
                let names = self.list_camp_names();
                if names.is_empty() {
                    self.status_message = String::from("No saved camps (config/camps/ is empty)");
                } else {
                    self.status_message = format!("Camps: {}", names.join(", "));
                }
            }
            Some("add") => {
                let camp_name = if let Some(name) = args.get(1) {
                    *name
                } else {
                    self.status_message =
                        String::from("Usage: camp add <name>  (saves current position)");
                    return;
                };

                let (center, zone) = if let Some(player) = &self.local_player {
                    let zone = self
                        .active_client()
                        .map_or_else(|| "unknown".into(), |c| c.zone_name.clone());
                    ([player.x, player.y, player.z], zone)
                } else {
                    self.status_message =
                        String::from("No player data — cannot save camp position");
                    return;
                };

                let config = CampConfig {
                    name: camp_name.to_string(),
                    zone,
                    camp_center: center,
                    pull_point: [center[0] + 50.0, center[1] + 50.0, center[2]],
                    pull_radius: 200.0,
                    camp_radius: 30.0,
                    leash_radius: 100.0,
                    rest_mana_pct: 60,
                    pull_mana_pct: 30,
                    level_range: [1, 60],
                    pull_mob_names: Vec::new(),
                    ignore_mob_names: Vec::new(),
                    burn_mob_names: Vec::new(),
                    return_no_aggro: false,
                    next_camp: None,
                    prev_camp: None,
                };

                match config.save() {
                    Ok(()) => {
                        self.status_message = format!(
                            "Camp '{}' saved at ({:.0}, {:.0}, {:.0})",
                            camp_name, center[0], center[1], center[2]
                        );
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to save camp '{camp_name}': {e}");
                    }
                }
            }
            Some("remove") => {
                let camp_name = if let Some(name) = args.get(1) {
                    *name
                } else {
                    self.status_message = String::from("Usage: camp remove <name>");
                    return;
                };

                let path = std::path::Path::new("config/camps").join(format!("{camp_name}.toml"));
                if path.exists() {
                    match std::fs::remove_file(&path) {
                        Ok(()) => {
                            self.status_message = format!("Camp '{camp_name}' removed");
                        }
                        Err(e) => {
                            self.status_message =
                                format!("Failed to remove camp '{camp_name}': {e}");
                        }
                    }
                } else {
                    self.status_message = format!("Camp '{camp_name}' not found");
                }
            }
            Some("next") => match &orchestrator.active_camp {
                None => {
                    self.status_message = String::from("No active camp — start one first");
                }
                Some(camp) => {
                    let current = camp.config.name.clone();
                    match &camp.config.next_camp {
                        Some(next_name) => match CampConfig::load(next_name) {
                            Ok(config) => {
                                let members = self.build_camp_members();
                                if members.is_empty() {
                                    self.status_message =
                                        String::from("No clients connected — cannot advance camp");
                                    return;
                                }
                                let count = members.len();
                                let to = config.name.clone();
                                orchestrator.start_camp(config, members);
                                self.status_message =
                                    format!("Advanced: {current} → {to} ({count} members)");
                            }
                            Err(e) => {
                                self.status_message =
                                    format!("Failed to load next camp '{next_name}': {e}");
                            }
                        },
                        None => {
                            self.status_message =
                                format!("Camp '{current}' has no next camp configured");
                        }
                    }
                }
            },
            Some("prev") => match &orchestrator.active_camp {
                None => {
                    self.status_message = String::from("No active camp — start one first");
                }
                Some(camp) => {
                    let current = camp.config.name.clone();
                    match &camp.config.prev_camp {
                        Some(prev_name) => match CampConfig::load(prev_name) {
                            Ok(config) => {
                                let members = self.build_camp_members();
                                if members.is_empty() {
                                    self.status_message =
                                        String::from("No clients connected — cannot fall back");
                                    return;
                                }
                                let count = members.len();
                                let to = config.name.clone();
                                orchestrator.start_camp(config, members);
                                self.status_message =
                                    format!("Fell back: {current} → {to} ({count} members)");
                            }
                            Err(e) => {
                                self.status_message =
                                    format!("Failed to load prev camp '{prev_name}': {e}");
                            }
                        },
                        None => {
                            self.status_message =
                                format!("Camp '{current}' has no previous camp configured");
                        }
                    }
                }
            },
            // Bare camp name — shortcut for camp start <name>
            Some(name) => match CampConfig::load(name) {
                Ok(config) => {
                    let members = self.build_camp_members();
                    if members.is_empty() {
                        self.status_message =
                            String::from("No clients connected — cannot start camp");
                        return;
                    }
                    let count = members.len();
                    orchestrator.start_camp(config, members);
                    self.status_message = format!("Camp '{name}' started with {count} members");
                }
                Err(_) => {
                    self.status_message = format!(
                        "Unknown camp subcommand or config: '{name}'. Try: start|stop|status|list|add|remove|next|prev"
                    );
                }
            },
        }
    }

    /// Handle `ch <subcommand>` — CH chain management from the command bar.
    ///
    /// Subcommands:
    ///   ch start <pid1,pid2,...> <interval> <`target_id`> [`spell_slot`]
    ///   ch stop                  — Stop the running CH chain
    ///   ch add <pid>             — Add a cleric to the chain
    ///   ch remove <pid>          — Remove a cleric from the chain (`rm` alias supported)
    ///   ch interval <seconds>    — Set the interval between casts
    ///   ch adaptive on|off       — Toggle adaptive timing mode
    ///   ch status                — Show current chain status
    fn execute_ch_command(&mut self, args: &[&str], orchestrator: &mut Orchestrator) {
        match args.first().copied() {
            None | Some("status") => {
                if let Some(chain) = orchestrator
                    .combat
                    .ch_chain
                    .as_ref()
                    .filter(|c| c.is_active())
                {
                    let members = chain.members().len();
                    let interval = chain.interval_secs();
                    let adaptive = if chain.is_adaptive() {
                        "adaptive"
                    } else {
                        "fixed"
                    };
                    let target = chain.target_id();
                    self.set_feedback(
                        ToastLevel::Info,
                        format!(
                            "CH chain: {members} clerics, {interval:.1}s interval ({adaptive}), target={target}"
                        ),
                        false,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Info,
                        String::from(
                            "CH chain inactive. Start one with :ch start <pid1,pid2,...> <interval_secs> <target_id> [spell_slot].",
                        ),
                        false,
                    );
                }
            }
            Some("start") => {
                // ch start <pid1,pid2,...> <interval> <target_id> [spell_slot]
                let Some(pids_str) = args.get(1) else {
                    self.usage_feedback("ch start", "Missing cleric PID list.");
                    return;
                };
                let pids: Vec<u32> = pids_str
                    .split(',')
                    .filter_map(|s| s.trim().parse::<u32>().ok())
                    .collect();
                if pids.is_empty() {
                    self.usage_feedback("ch start", "No valid PIDs found in the list.");
                    return;
                }
                let interval: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(3.0);
                let target_id: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
                let spell_slot: u8 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1);

                orchestrator
                    .combat
                    .start_ch_chain(pids.clone(), interval, target_id, spell_slot);
                tracing::info!(
                    pids = ?pids,
                    interval,
                    target_id,
                    spell_slot,
                    "CH chain started from TUI"
                );
                self.set_feedback(
                    ToastLevel::Success,
                    format!(
                        "CH chain started: {} clerics, {:.1}s interval, target={}, slot={}",
                        pids.len(),
                        interval,
                        target_id,
                        spell_slot
                    ),
                    true,
                );
            }
            Some("stop") => {
                if orchestrator.combat.ch_chain_active() {
                    orchestrator.combat.stop_ch_chain();
                    tracing::info!("CH chain stopped from TUI");
                    self.set_feedback(ToastLevel::Success, String::from("CH chain stopped"), true);
                } else {
                    self.usage_feedback("ch start", "No CH chain is running.");
                }
            }
            Some("add") => {
                if let Some(pid_str) = args.get(1) {
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        if orchestrator.combat.ch_chain_active() {
                            orchestrator.combat.ch_chain_add(pid);
                            tracing::info!(pid, "Cleric added to CH chain");
                            self.set_feedback(
                                ToastLevel::Success,
                                format!("Added PID {pid} to CH chain"),
                                true,
                            );
                        } else {
                            self.usage_feedback("ch start", "No CH chain is running.");
                        }
                    } else {
                        self.usage_feedback("ch add", format!("Invalid PID '{pid_str}'."));
                    }
                } else {
                    self.usage_feedback("ch add", "Missing cleric PID.");
                }
            }
            Some("rm" | "remove") => {
                if let Some(pid_str) = args.get(1) {
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        if orchestrator.combat.ch_chain_active() {
                            orchestrator.combat.ch_chain_remove(pid);
                            tracing::info!(pid, "Cleric removed from CH chain");
                            self.set_feedback(
                                ToastLevel::Success,
                                format!("Removed PID {pid} from CH chain"),
                                true,
                            );
                        } else {
                            self.usage_feedback("ch start", "No CH chain is running.");
                        }
                    } else {
                        self.usage_feedback("ch remove", format!("Invalid PID '{pid_str}'."));
                    }
                } else {
                    self.usage_feedback("ch remove", "Missing cleric PID.");
                }
            }
            Some("interval") => {
                if let Some(secs_str) = args.get(1) {
                    if let Ok(secs) = secs_str.parse::<f32>() {
                        if orchestrator.combat.ch_chain_active() {
                            orchestrator.combat.ch_chain_set_interval(secs);
                            tracing::info!(interval = secs, "CH chain interval updated");
                            self.set_feedback(
                                ToastLevel::Success,
                                format!("CH chain interval set to {secs:.1}s"),
                                true,
                            );
                        } else {
                            self.usage_feedback("ch start", "No CH chain is running.");
                        }
                    } else {
                        self.usage_feedback(
                            "ch interval",
                            format!("Invalid seconds '{secs_str}'."),
                        );
                    }
                } else {
                    self.usage_feedback("ch interval", "Missing interval seconds.");
                }
            }
            Some("adaptive") => match args.get(1).copied() {
                Some("on" | "true" | "1") => {
                    if orchestrator.combat.ch_chain_active() {
                        if let Some(chain) = &mut orchestrator.combat.ch_chain {
                            chain.set_adaptive(true);
                            tracing::info!("CH chain adaptive mode enabled");
                            self.set_feedback(
                                ToastLevel::Success,
                                String::from("CH chain: adaptive timing ON"),
                                true,
                            );
                        }
                    } else {
                        self.usage_feedback("ch start", "No CH chain is running.");
                    }
                }
                Some("off" | "false" | "0") => {
                    if orchestrator.combat.ch_chain_active() {
                        if let Some(chain) = &mut orchestrator.combat.ch_chain {
                            chain.set_adaptive(false);
                            tracing::info!("CH chain adaptive mode disabled");
                            self.set_feedback(
                                ToastLevel::Success,
                                String::from("CH chain: adaptive timing OFF"),
                                true,
                            );
                        }
                    } else {
                        self.usage_feedback("ch start", "No CH chain is running.");
                    }
                }
                _ => {
                    self.usage_feedback("ch adaptive", "Expected 'on' or 'off'.");
                }
            },
            Some(sub) => {
                if let Some(suggestion) = command::did_you_mean(&format!("ch {sub}")) {
                    self.set_feedback(
                        ToastLevel::Warning,
                        format!(
                            "Unknown CH subcommand '{sub}'. Did you mean :{}?",
                            suggestion.phrase
                        ),
                        true,
                    );
                } else {
                    self.usage_feedback("ch", format!("Unknown CH subcommand '{sub}'."));
                }
            }
        }
        // Sync cached display state after any CH chain mutation
        self.sync_ch_chain_state(orchestrator);
    }

    /// Handle `login <subcommand>` from the command bar.
    ///
    /// Subcommands:
    ///   login             — list all configured accounts and status
    ///   login all         — launch all configured accounts
    ///   login G<n>        — launch all accounts in group n
    ///   login <name>      — launch a single account by name
    fn execute_login_command(&mut self, args: &[&str]) {
        let accounts = if let Some(cfg) = &self.accounts_config {
            cfg.clone()
        } else {
            self.status_message = String::from("No accounts config — create config/accounts.toml");
            return;
        };

        match args.first().copied() {
            // :login — list all accounts and their online/offline status
            None => {
                if accounts.accounts.is_empty() {
                    self.status_message = String::from("No accounts configured");
                    return;
                }
                let online_chars: Vec<String> = self
                    .clients
                    .iter()
                    .map(|c| c.character_name.to_lowercase())
                    .collect();

                let mut lines: Vec<String> = Vec::new();
                for acct in &accounts.accounts {
                    let is_online = online_chars
                        .iter()
                        .any(|c| !c.is_empty() && c == &acct.character.to_lowercase());
                    let status = if is_online { "ONLINE" } else { "offline" };
                    lines.push(format!(
                        "  {} ({} G{}) [{}]",
                        acct.name, acct.class, acct.group, status
                    ));
                }
                let online_count = accounts
                    .accounts
                    .iter()
                    .filter(|a| {
                        online_chars
                            .iter()
                            .any(|c| !c.is_empty() && c == &a.character.to_lowercase())
                    })
                    .count();
                self.status_message = format!(
                    "{}/{} accounts online. Use :login all | G<n> | <name>",
                    online_count,
                    accounts.accounts.len()
                );
                tracing::info!(
                    total = accounts.accounts.len(),
                    online = online_count,
                    "Login status query"
                );
                for line in &lines {
                    tracing::info!("{}", line);
                }
            }

            // :login all — enqueue all accounts for launch
            Some("all") => {
                self.enqueue_account_launches(&accounts.accounts);
            }

            // :login G<n> — launch accounts in a specific group
            Some(arg) if arg.starts_with('G') || arg.starts_with('g') => {
                if let Ok(group_id) = arg[1..].parse::<u32>() {
                    let group_accounts: Vec<_> = accounts
                        .accounts_for_group(group_id)
                        .into_iter()
                        .cloned()
                        .collect();
                    if group_accounts.is_empty() {
                        self.status_message =
                            format!("No accounts configured for group {group_id}");
                    } else {
                        self.enqueue_account_launches(&group_accounts);
                    }
                } else {
                    self.status_message = format!("Invalid group: {arg}");
                }
            }

            // :login <account_name> — launch a single account
            Some(name) => {
                if let Some(entry) = accounts.find_account(name) {
                    self.enqueue_account_launches(std::slice::from_ref(entry));
                } else {
                    self.status_message = format!("Account '{name}' not found in config");
                }
            }
        }
    }

    /// Handle `profile <subcommand>` from the command bar.
    ///
    /// Subcommands:
    ///   profile             — list all configured profile groups
    ///   profile list        — list all configured profile groups
    ///   profile launch <name> — queue all accounts in the named profile for launch
    fn execute_profile_command(&mut self, args: &[&str]) {
        let accounts = if let Some(cfg) = &self.accounts_config {
            cfg.clone()
        } else {
            self.status_message = String::from("No accounts config — create config/accounts.toml");
            return;
        };

        match args.first().copied() {
            None | Some("list") => {
                if accounts.profile_groups.is_empty() {
                    self.status_message = String::from(
                        "No profile groups configured. Add [[profile_groups]] to accounts.toml",
                    );
                    return;
                }
                let online_chars: Vec<String> = self
                    .clients
                    .iter()
                    .map(|c| c.character_name.to_lowercase())
                    .collect();
                let mut lines: Vec<String> = Vec::new();
                for pg in &accounts.profile_groups {
                    let count = accounts.accounts_for_group(pg.id).len();
                    let online = accounts
                        .accounts_for_group(pg.id)
                        .iter()
                        .filter(|a| {
                            online_chars
                                .iter()
                                .any(|c| !c.is_empty() && c == &a.character.to_lowercase())
                        })
                        .count();
                    let hotkey_str = pg
                        .hotkey
                        .as_deref()
                        .map_or(String::from("(no hotkey)"), |h| format!("[{h}]"));
                    lines.push(format!(
                        "  {} (G{}) {} — {}/{} online",
                        pg.name, pg.id, hotkey_str, online, count,
                    ));
                }
                self.status_message = format!(
                    "{} profile group(s). Use :profile launch <name>",
                    accounts.profile_groups.len()
                );
                for line in &lines {
                    tracing::info!("{}", line);
                }
            }

            Some("launch") => {
                let name = args.get(1).copied().unwrap_or("");
                if name.is_empty() {
                    self.usage_feedback("profile launch", "Missing profile name.");
                    return;
                }
                self.launch_profile_by_name(name, &accounts);
            }

            Some(sub) => {
                self.usage_feedback(
                    "profile",
                    format!(
                        "Unknown subcommand '{sub}'. Use: profile list | profile launch <name>"
                    ),
                );
            }
        }
    }

    /// Launch all accounts in the named profile group.
    fn launch_profile_by_name(&mut self, name: &str, accounts: &crate::config::AccountsConfig) {
        match accounts.accounts_for_profile_name(name) {
            None => {
                self.status_message = format!("Profile '{name}' not found in accounts.toml");
            }
            Some(group_accounts) if group_accounts.is_empty() => {
                self.status_message =
                    format!("Profile '{name}' has no accounts configured (check group ID)");
            }
            Some(group_accounts) => {
                let owned: Vec<crate::config::AccountEntry> =
                    group_accounts.into_iter().cloned().collect();
                self.enqueue_account_launches(&owned);
            }
        }
    }

    /// Launch the profile group assigned to the given hotkey string (e.g., `"F1"`).
    ///
    /// Called directly from the TUI event handler for `Ctrl+F1`–`Ctrl+F9` keypresses.
    pub fn launch_profile_hotkey(&mut self, hotkey: &str) {
        let Some(accounts) = self.accounts_config.clone() else {
            self.status_message = String::from("No accounts config — create config/accounts.toml");
            return;
        };
        let Some(pg) = accounts.profile_by_hotkey(hotkey) else {
            self.status_message = format!("Ctrl+{hotkey}: no profile group assigned this hotkey");
            return;
        };
        let name = pg.name.clone();
        self.launch_profile_by_name(&name, &accounts);
    }

    /// Handle `stop <name|all>` — eject DLL and remove client.
    ///
    ///   stop all         — eject all connected clients
    ///   stop <name>      — eject a single client by character name
    fn execute_stop_command(&mut self, args: &[&str], orchestrator: &mut Orchestrator) {
        match args.first().copied() {
            None => {
                self.status_message =
                    String::from("Usage: stop <name|all>  (ejects DLL from client)");
            }
            Some("all") => {
                let pids: Vec<u32> = self.clients.iter().map(|c| c.pid).collect();
                let count = pids.len();
                for pid in pids {
                    orchestrator.eject_client(pid);
                }
                self.status_message = format!("Ejected {count} client(s)");
            }
            Some(name) => {
                if let Some(client) = self
                    .clients
                    .iter()
                    .find(|c| c.character_name.eq_ignore_ascii_case(name))
                {
                    let pid = client.pid;
                    let char_name = client.character_name.clone();
                    orchestrator.eject_client(pid);
                    self.status_message = format!("Ejected {char_name} (PID {pid})");
                } else {
                    self.status_message = format!("Client '{name}' not found");
                }
            }
        }
    }

    /// Handle `restart <name|all>` — eject then re-launch via login automation.
    ///
    ///   restart all         — restart all clients
    ///   restart <name>      — restart a single client
    fn execute_restart_command(&mut self, args: &[&str], orchestrator: &mut Orchestrator) {
        match args.first().copied() {
            None => {
                self.status_message =
                    String::from("Usage: restart <name|all>  (ejects DLL then re-launches)");
            }
            Some("all") => {
                // Eject all first
                let pids: Vec<u32> = self.clients.iter().map(|c| c.pid).collect();
                let count = pids.len();
                for pid in &pids {
                    orchestrator.eject_client(*pid);
                }
                // Then re-launch all
                self.execute_login_command(&["all"]);
                self.status_message =
                    format!("Restarting {count} client(s) — ejected, re-launching...");
            }
            Some(name) => {
                // Eject the specific client
                if let Some(client) = self
                    .clients
                    .iter()
                    .find(|c| c.character_name.eq_ignore_ascii_case(name))
                {
                    let pid = client.pid;
                    let char_name = client.character_name.clone();
                    orchestrator.eject_client(pid);
                    // Re-launch via login
                    self.execute_login_command(&[name]);
                    self.status_message =
                        format!("Restarting {char_name} (PID {pid}) — ejected, re-launching...");
                } else {
                    // Maybe the client isn't connected but the account exists — just launch
                    self.execute_login_command(&[name]);
                }
            }
        }
    }

    /// Handle `track <subcommand>` from the command bar.
    fn execute_track_command(&mut self, args: &[&str]) {
        match args.first().copied() {
            None => {
                self.usage_feedback("track", "Missing spawn name.");
            }
            Some("list") => {
                if self.tracked_spawns.is_empty() {
                    self.set_feedback(ToastLevel::Info, String::from("No spawns tracked"), false);
                } else {
                    let entries: Vec<String> = self
                        .tracked_spawns
                        .values()
                        .map(|t| format!("{} [{}]", t.name, t.status.label()))
                        .collect();
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Tracked: {}", entries.join(", ")),
                        false,
                    );
                }
            }
            Some(_) => {
                // Join all args for multi-word names, strip quotes
                let full_name = args.join(" ");
                let clean = full_name.trim_matches('"');
                self.track_spawn(clean);
            }
        }
    }

    /// Handle the `:scope` command — show or change the active routing scope.
    ///
    /// `:scope`          → show current scope  
    /// `:scope all`      → AllSession  
    /// `:scope G1`       → Group 1  
    /// `:scope <name>`   → OneToon targeting that character
    fn execute_scope_command(&mut self, args: &[&str]) {
        use textquest_common::routing::RoutingScope;

        match args.first().copied() {
            None | Some("") => {
                // Show current scope
                let label = self.routing_scope.label();
                let focused = self.focused_pids().len();
                self.set_feedback(
                    ToastLevel::Info,
                    format!("Routing scope: {label} ({focused} client(s) in scope)"),
                    false,
                );
            }
            Some("all") => {
                self.routing_scope = RoutingScope::AllSession;
                self.active_group = None;
                let focused = self.focused_pids().len();
                self.set_feedback(
                    ToastLevel::Success,
                    format!("Scope → All ({focused} clients)"),
                    true,
                );
            }
            Some(arg) => {
                // Check for G<n> group syntax
                if let Some(stripped) = arg.strip_prefix('G').or_else(|| arg.strip_prefix('g'))
                    && let Ok(group_id) = stripped.parse::<u8>()
                {
                    // group_id is 1-based; groups vec is 0-indexed
                    let idx = (group_id as usize).saturating_sub(1);
                    if idx < self.groups.len() {
                        let g = &self.groups[idx];
                        let label = g.name.clone();
                        self.routing_scope = RoutingScope::Group {
                            group_id,
                            label: label.clone(),
                        };
                        self.set_active_group(Some(idx));
                        self.set_feedback(
                            ToastLevel::Success,
                            format!("Scope → G{group_id} {label}"),
                            true,
                        );
                    } else {
                        self.usage_feedback(
                            "scope",
                            format!("Group G{group_id} not found. Use :scope G1..G6."),
                        );
                    }
                    return;
                }
                // Treat as character name
                if self.find_client_by_name(arg).is_some() {
                    let name = arg.to_string();
                    self.routing_scope = RoutingScope::OneToon { name: name.clone() };
                    if let Some(idx) = self.find_client_index_by_name(&name) {
                        self.select_client_idx(idx);
                    }
                    self.active_group = None;
                    self.set_feedback(ToastLevel::Success, format!("Scope → @{name}"), true);
                } else {
                    self.usage_feedback(
                        "scope",
                        format!("'{arg}' is not a connected client. Use :scope all, :scope G1, or :scope <character>."),
                    );
                }
            }
        }
    }

    /// Handle the `:session` command — show slot lifecycle states.
    ///
    /// `:session`        → compact summary  
    /// `:session list`   → per-slot lifecycle table  
    /// `:session status` → same as compact summary
    fn execute_session_command(&mut self, args: &[&str]) {
        match args.first().copied() {
            None | Some("status") => {
                // Compact summary: live / recovering / blocked counts
                let total = self.clients.len();
                if total == 0 {
                    self.set_feedback(
                        ToastLevel::Warning,
                        String::from("No sessions connected. Use :login to start clients."),
                        false,
                    );
                    return;
                }
                let scope_label = self.routing_scope.label();
                self.set_feedback(
                    ToastLevel::Info,
                    format!("Session: {total} slot(s) connected | scope={scope_label}"),
                    false,
                );
            }
            Some("list") => {
                // Per-slot table
                if self.clients.is_empty() {
                    self.set_feedback(
                        ToastLevel::Warning,
                        String::from("No sessions connected. Use :login to start clients."),
                        false,
                    );
                    return;
                }
                let rows: Vec<String> = self
                    .clients
                    .iter()
                    .map(|c| {
                        let name = if !c.character_name.is_empty() {
                            c.character_name.as_str()
                        } else {
                            "???"
                        };
                        let zone = if c.zone_name.is_empty() {
                            "—"
                        } else {
                            &c.zone_name
                        };
                        let status = if c.client_status.is_empty() {
                            "connected"
                        } else {
                            &c.client_status
                        };
                        format!("{name} [{zone}] {status}")
                    })
                    .collect();
                self.set_feedback(
                    ToastLevel::Info,
                    format!("Sessions: {}", rows.join(" | ")),
                    false,
                );
            }
            Some(other) => {
                self.usage_feedback(
                    "session",
                    format!(
                        "Unknown subcommand '{other}'. Use: session, session list, session status"
                    ),
                );
            }
        }
    }

    /// Enqueue accounts for staggered launch via the spawner.
    /// On non-Windows (macOS dev), logs what would happen and updates status.
    fn enqueue_account_launches(&mut self, entries: &[crate::config::AccountEntry]) {
        use crate::config::AccountsConfig;

        let count = entries.len();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();

        if cfg!(not(windows)) {
            // Dev mode: log what would be launched
            self.status_message = format!(
                "Launch queued: {} account(s) [dev mode — Windows only]. Accounts: {}",
                count,
                names.join(", ")
            );
            tracing::info!(
                count,
                accounts = ?names,
                "Login launch queued (stub — not on Windows)"
            );
            return;
        }

        // On Windows: use the spawner to launch each account with stagger
        let mut launched = 0u32;
        let mut failed = 0u32;
        for entry in entries {
            let info = AccountsConfig::to_account_info(entry);
            let eq_path = std::path::Path::new(&self.launch_eq_path);
            match crate::launcher::spawner::spawn_eq_client(
                eq_path,
                &info.account_name,
                &info.server_name,
                &[],
            ) {
                Ok(spawned) => {
                    tracing::info!(
                        pid = spawned.pid,
                        account = "[redacted]",
                        server = %info.server_name,
                        "Launched EQ client for login"
                    );
                    launched += 1;
                    // Post-launch automation (M2.5 roadmap):
                    // 1. Wire into LaunchCoordinator for staggered launch + state tracking
                    // 2. After window title shows "[TQ] EQ - <CharName>", auto-inject DLL
                    // 3. After DLL injection, auto-form groups + set camp
                }
                Err(e) => {
                    tracing::error!(account = "[redacted]", %e, "Failed to launch EQ client");
                    failed += 1;
                }
            }
        }

        self.status_message =
            format!("Login: launched {launched}, failed {failed} of {count} queued");
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
                    _ => Role::Dps,
                };
                let name = if client.character_name.is_empty() {
                    format!("Client-{}", client.pid)
                } else {
                    client.character_name.clone()
                };
                CampMember::new(client.pid, name, role)
            })
            .collect()
    }

    // ─── Map parity command handlers ─────────────────────────────────────

    fn handle_circle_command(&mut self, parts: &[&str], orchestrator: &mut Orchestrator) {
        use textquest_common::ipc::Command;
        use textquest_common::nav::{CircleConfig, CircleMode, Waypoint};

        let sub = parts.get(1).map(|s| s.to_ascii_lowercase());
        match sub.as_deref() {
            Some("off") => {
                let ok = self.send_ipc_to_focused(&Command::CircleOff);
                if ok == 0 {
                    self.set_feedback(
                        ToastLevel::Warning,
                        "Circle off: no clients received command.",
                        true,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Success,
                        format!("Circle kite stopped (sent to {ok} clients)"),
                        true,
                    );
                }
            }
            Some("on") | None => {
                let mut config = CircleConfig::default();
                let mut i = 2usize;
                // Optional radius
                if let Some(&r_str) = parts.get(i) {
                    if let Ok(r) = r_str.parse::<f32>() {
                        config.radius = r;
                        i += 1;
                    }
                }
                // Optional mode tokens
                while let Some(&token) = parts.get(i) {
                    match token.to_ascii_lowercase().as_str() {
                        "cw" | "clockwise" => config.mode = CircleMode::Cw,
                        "ccw" | "counterclockwise" => config.mode = CircleMode::Ccw,
                        "drunken" => config.mode = CircleMode::Drunken,
                        "backward" => config.mode = CircleMode::Backward,
                        _ => {}
                    }
                    i += 1;
                }
                let mode_label = match config.mode {
                    CircleMode::Cw => "CW",
                    CircleMode::Ccw => "CCW",
                    CircleMode::Drunken => "Drunken",
                    CircleMode::Backward => "Backward",
                };
                let ok = self.send_ipc_to_focused(&Command::CircleKite { config: config.clone() });
                if ok == 0 {
                    self.set_feedback(
                        ToastLevel::Warning,
                        "Circle kite: no clients received command.",
                        true,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Success,
                        format!(
                            "Circle kite started — radius={:.0} mode={mode_label} (sent to {ok} clients)",
                            config.radius
                        ),
                        true,
                    );
                }
                let _ = orchestrator;
            }
            Some("loc") => {
                // circle loc Y X [radius]
                let y: Option<f32> = parts.get(2).and_then(|s| s.parse().ok());
                let x: Option<f32> = parts.get(3).and_then(|s| s.parse().ok());
                let radius: Option<f32> = parts.get(4).and_then(|s| s.parse().ok());
                match (y, x) {
                    (Some(y), Some(x)) => {
                        let mut config = CircleConfig::default();
                        if let Some(r) = radius {
                            config.radius = r;
                        }
                        config.center = Some(Waypoint::new(x, y, 0.0));
                        let ok = self.send_ipc_to_focused(&Command::CircleKite { config: config.clone() });
                        if ok == 0 {
                            self.set_feedback(
                                ToastLevel::Warning,
                                "Circle loc: no clients received command.",
                                true,
                            );
                        } else {
                            self.set_feedback(
                                ToastLevel::Success,
                                format!(
                                    "Circle kite at ({y}, {x}) radius={:.0} (sent to {ok} clients)",
                                    config.radius
                                ),
                                true,
                            );
                        }
                    }
                    _ => {
                        self.usage_feedback(
                            "circle",
                            "Usage: circle loc Y X [radius]",
                        );
                    }
                }
                let _ = orchestrator;
            }
            _ => {
                self.usage_feedback(
                    "circle",
                    "Usage: circle on [radius] [cw|ccw|drunken|backward] | circle off | circle loc Y X [radius]",
                );
            }
        }
    }

    fn handle_mapfilter_command(&mut self, parts: &[&str]) {
        match parts.get(1).copied() {
            None => {
                self.set_feedback(ToastLevel::Info, self.map_state.filters.summary(), false);
            }
            Some(arg) if arg.eq_ignore_ascii_case("reset") => {
                self.map_state.filters.set_all(true);
                self.map_spawn_cache.clear();
                self.set_feedback(
                    ToastLevel::Success,
                    String::from("Map filters reset (all ON)"),
                    true,
                );
            }
            Some(arg)
                if arg.eq_ignore_ascii_case("castradius") || arg.eq_ignore_ascii_case("cr") =>
            {
                self.handle_mapfilter_radius(parts, true);
            }
            Some(arg)
                if arg.eq_ignore_ascii_case("spellradius") || arg.eq_ignore_ascii_case("sr") =>
            {
                self.handle_mapfilter_radius(parts, false);
            }
            Some(arg) if arg.eq_ignore_ascii_case("targetpath") => {
                let v = parts
                    .get(2)
                    .and_then(|s| parse_bool_flag(s))
                    .unwrap_or(!self.map_state.show_target_path);
                self.map_state.show_target_path = v;
                self.set_feedback(
                    ToastLevel::Info,
                    format!("Target path {}", if v { "ON" } else { "OFF" }),
                    true,
                );
            }
            Some(arg) if arg.eq_ignore_ascii_case("targetline") => {
                let v = parts
                    .get(2)
                    .and_then(|s| parse_bool_flag(s))
                    .unwrap_or(!self.map_state.show_target_line);
                self.map_state.show_target_line = v;
                self.set_feedback(
                    ToastLevel::Info,
                    format!("Target line {}", if v { "ON" } else { "OFF" }),
                    true,
                );
            }
            Some(arg) if arg.eq_ignore_ascii_case("save") => {
                let Some(name) = parts.get(2) else {
                    self.usage_feedback("mapfilter save", "Usage: mapfilter save <name>");
                    return;
                };
                self.map_state.save_preset((*name).to_string());
                self.set_feedback(
                    ToastLevel::Success,
                    format!("Preset \"{name}\" saved"),
                    true,
                );
            }
            Some(arg) if arg.eq_ignore_ascii_case("load") => {
                let Some(name) = parts.get(2) else {
                    self.usage_feedback("mapfilter load", "Usage: mapfilter load <name>");
                    return;
                };
                if self.map_state.load_preset(name) {
                    self.map_spawn_cache.clear();
                    self.set_feedback(
                        ToastLevel::Success,
                        format!("Preset \"{name}\" loaded"),
                        true,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Warning,
                        format!("No preset named \"{name}\""),
                        false,
                    );
                }
            }
            Some(arg) if arg.eq_ignore_ascii_case("delete") => {
                let Some(name) = parts.get(2) else {
                    self.usage_feedback("mapfilter delete", "Usage: mapfilter delete <name>");
                    return;
                };
                if self.map_state.delete_preset(name) {
                    self.set_feedback(
                        ToastLevel::Success,
                        format!("Preset \"{name}\" deleted"),
                        true,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Warning,
                        format!("No preset named \"{name}\""),
                        false,
                    );
                }
            }
            Some(arg) if arg.eq_ignore_ascii_case("list") => {
                if self.map_state.saved_presets.is_empty() {
                    self.set_feedback(ToastLevel::Info, String::from("No saved presets"), false);
                } else {
                    let names: Vec<&str> = self
                        .map_state
                        .saved_presets
                        .iter()
                        .map(|p| p.name.as_str())
                        .collect();
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Presets: {}", names.join(", ")),
                        false,
                    );
                }
            }
            Some(arg) => {
                let Some(kind) = MapFilterKind::parse_kind(arg) else {
                    self.usage_feedback("mapfilter", "Usage: mapfilter <type> [on|off]");
                    return;
                };
                let ns = if let Some(st) = parts.get(2) {
                    let Some(on) = parse_bool_flag(st) else {
                        self.usage_feedback("mapfilter", "Usage: mapfilter <type> [on|off]");
                        return;
                    };
                    self.map_state.filters.set(kind, on);
                    on
                } else {
                    self.map_state.filters.toggle(kind)
                };
                self.map_spawn_cache.clear();
                self.set_feedback(
                    ToastLevel::Info,
                    format!(
                        "{} {} | {}",
                        kind.label(),
                        if ns { "ON" } else { "OFF" },
                        self.map_state.filters.summary()
                    ),
                    true,
                );
            }
        }
    }

    fn handle_mapfilter_radius(&mut self, parts: &[&str], is_cast: bool) {
        let label = if is_cast { "Cast" } else { "Spell" };
        let field = if is_cast {
            &mut self.map_state.cast_radius
        } else {
            &mut self.map_state.spell_radius
        };
        match parts.get(2).map(|s| s.to_ascii_lowercase()).as_deref() {
            None | Some("off" | "0" | "clear") => {
                *field = None;
                self.set_feedback(ToastLevel::Info, format!("{label} radius cleared"), true);
            }
            Some(val) => {
                let Ok(r) = val.parse::<f32>() else {
                    let c = if is_cast { "castradius" } else { "spellradius" };
                    self.usage_feedback(
                        &format!("mapfilter {c}"),
                        format!("Usage: mapfilter {c} <radius> [color]"),
                    );
                    return;
                };
                let color = parts
                    .get(3)
                    .and_then(|c| super::theme::parse_color_name(c))
                    .unwrap_or(if is_cast { Color::Cyan } else { Color::Yellow });
                *field = Some(MapRadiusOverlay {
                    radius: r,
                    color,
                    label: format!("{label} {r:.0}"),
                });
                self.set_feedback(
                    ToastLevel::Success,
                    format!("{label} radius {r:.0} ({color:?})"),
                    true,
                );
            }
        }
    }

    fn handle_mapclick_command(&mut self, parts: &[&str]) {
        match parts.get(1).map(|s| s.to_ascii_lowercase()).as_deref() {
            None => {
                let c = match self.map_state.click_action {
                    MapClickAction::None => "none",
                    MapClickAction::PlaceLoc => "loc",
                    MapClickAction::Navigate => "nav",
                };
                self.set_feedback(ToastLevel::Info, format!("Map click action: {c}"), false);
            }
            Some("none" | "off") => {
                self.map_state.click_action = MapClickAction::None;
                self.set_feedback(ToastLevel::Info, String::from("Map click: disabled"), true);
            }
            Some("loc" | "marker") => {
                self.map_state.click_action = MapClickAction::PlaceLoc;
                self.set_feedback(
                    ToastLevel::Info,
                    String::from("Map click: place loc marker"),
                    true,
                );
            }
            Some("nav" | "navigate") => {
                self.map_state.click_action = MapClickAction::Navigate;
                self.set_feedback(
                    ToastLevel::Info,
                    String::from("Map click: navigate to point"),
                    true,
                );
            }
            _ => {
                self.usage_feedback("mapclick", "Usage: mapclick [none|loc|nav]");
            }
        }
    }

    fn handle_maploc_command(&mut self, parts: &[&str]) {
        match parts.get(1).map(|s| s.to_ascii_lowercase()).as_deref() {
            None => {
                if let Some(p) = &self.local_player {
                    let m = MapLocMarker {
                        x: p.x,
                        y: p.y,
                        label: String::from("loc"),
                    };
                    let msg = format!("Loc set at ({:.0}, {:.0})", p.x, p.y);
                    self.map_state.loc_marker = Some(m);
                    self.set_feedback(ToastLevel::Success, msg, true);
                } else {
                    self.set_feedback(
                        ToastLevel::Warning,
                        String::from("No player position available"),
                        false,
                    );
                }
            }
            Some("recall" | "show") => {
                if let Some(l) = &self.map_state.loc_marker {
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Loc: ({:.0}, {:.0}) \"{}\"", l.x, l.y, l.label),
                        false,
                    );
                } else {
                    self.set_feedback(ToastLevel::Info, String::from("No loc marker set"), false);
                }
            }
            Some("clear" | "off") => {
                self.map_state.loc_marker = None;
                self.set_feedback(ToastLevel::Info, String::from("Loc marker cleared"), true);
            }
            _ => {
                self.usage_feedback("maploc", "Usage: maploc [recall|clear]");
            }
        }
    }

    fn handle_mapmarker_command(&mut self, parts: &[&str]) {
        match parts.get(1).map(|s| s.to_ascii_lowercase()).as_deref() {
            // :mapmarker set <name> [x y [z]]
            Some("set") => {
                let Some(name) = parts.get(2) else {
                    self.usage_feedback(
                        "mapmarker",
                        "Usage: mapmarker set <name> [x y [z]]",
                    );
                    return;
                };
                let name = (*name).to_string();

                // Explicit coordinates supplied?
                let (x, y, z, zone) = if let (Some(xs), Some(ys)) =
                    (parts.get(3), parts.get(4))
                {
                    let Ok(xv) = xs.parse::<f32>() else {
                        self.set_feedback(
                            ToastLevel::Warning,
                            format!("Invalid x coordinate: {xs}"),
                            false,
                        );
                        return;
                    };
                    let Ok(yv) = ys.parse::<f32>() else {
                        self.set_feedback(
                            ToastLevel::Warning,
                            format!("Invalid y coordinate: {ys}"),
                            false,
                        );
                        return;
                    };
                    let zv = parts
                        .get(5)
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(0.0);
                    let zone = self
                        .active_client()
                        .and_then(|c| c.local_player.as_ref())
                        .and(self.current_zone_short_name())
                        .unwrap_or_default();
                    (xv, yv, zv, zone)
                } else {
                    // Use player position.
                    let Some(player) = self
                        .active_client()
                        .and_then(|c| c.local_player.as_ref())
                    else {
                        self.set_feedback(
                            ToastLevel::Warning,
                            String::from("No player position available"),
                            false,
                        );
                        return;
                    };
                    let (px, py, pz) = (player.x, player.y, player.z);
                    let zone = self.current_zone_short_name().unwrap_or_default();
                    (px, py, pz, zone)
                };

                let key = name.to_ascii_lowercase();
                // Remove existing marker with the same name (case-insensitive).
                self.map_state
                    .named_markers
                    .retain(|m| m.name.to_ascii_lowercase() != key);
                let marker = NamedMapMarker {
                    name: name.clone(),
                    x,
                    y,
                    z,
                    label: None,
                    zone,
                };
                self.map_state.named_markers.push(marker);

                let msg = format!("Marker \"{name}\" set at ({x:.0}, {y:.0})");
                if let Err(e) = save_named_markers(
                    &self.map_state.marker_file.clone(),
                    &self.map_state.named_markers.clone(),
                ) {
                    self.set_feedback(
                        ToastLevel::Warning,
                        format!("{msg} (save failed: {e})"),
                        false,
                    );
                } else {
                    self.set_feedback(ToastLevel::Success, msg, true);
                }
            }

            // :mapmarker recall <name> — show marker info
            Some("recall" | "show") => {
                let Some(name) = parts.get(2) else {
                    self.usage_feedback("mapmarker", "Usage: mapmarker recall <name>");
                    return;
                };
                let key = name.to_ascii_lowercase();
                if let Some(m) = self
                    .map_state
                    .named_markers
                    .iter()
                    .find(|m| m.name.to_ascii_lowercase() == key)
                {
                    let zone_tag = if m.zone.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", m.zone)
                    };
                    self.set_feedback(
                        ToastLevel::Info,
                        format!(
                            "Marker \"{}\": ({:.0}, {:.0}, {:.0}){}",
                            m.name, m.x, m.y, m.z, zone_tag
                        ),
                        false,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Warning,
                        format!("No marker named \"{name}\""),
                        false,
                    );
                }
            }

            // :mapmarker clear <name> | all
            Some("clear" | "delete" | "remove") => {
                let Some(name) = parts.get(2) else {
                    self.usage_feedback("mapmarker", "Usage: mapmarker clear <name|all>");
                    return;
                };
                let msg = if name.to_ascii_lowercase() == "all" {
                    self.map_state.named_markers.clear();
                    String::from("All markers cleared")
                } else {
                    let key = name.to_ascii_lowercase();
                    let before = self.map_state.named_markers.len();
                    self.map_state
                        .named_markers
                        .retain(|m| m.name.to_ascii_lowercase() != key);
                    if self.map_state.named_markers.len() < before {
                        format!("Marker \"{name}\" cleared")
                    } else {
                        self.set_feedback(
                            ToastLevel::Warning,
                            format!("No marker named \"{name}\""),
                            false,
                        );
                        return;
                    }
                };
                if let Err(e) = save_named_markers(
                    &self.map_state.marker_file.clone(),
                    &self.map_state.named_markers.clone(),
                ) {
                    self.set_feedback(
                        ToastLevel::Warning,
                        format!("{msg} (save failed: {e})"),
                        false,
                    );
                } else {
                    self.set_feedback(ToastLevel::Success, msg, true);
                }
            }

            // :mapmarker list
            Some("list" | "ls") => {
                if self.map_state.named_markers.is_empty() {
                    self.set_feedback(
                        ToastLevel::Info,
                        String::from("No named markers set"),
                        false,
                    );
                } else {
                    let lines: Vec<String> = self
                        .map_state
                        .named_markers
                        .iter()
                        .map(|m| {
                            let zone_tag = if m.zone.is_empty() {
                                String::new()
                            } else {
                                format!(" [{}]", m.zone)
                            };
                            format!("  {} ({:.0}, {:.0}){}", m.name, m.x, m.y, zone_tag)
                        })
                        .collect();
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Markers:\n{}", lines.join("\n")),
                        false,
                    );
                }
            }

            // :mapmarker save — explicit disk flush
            Some("save") => {
                let path = self.map_state.marker_file.clone();
                let markers = self.map_state.named_markers.clone();
                match save_named_markers(&path, &markers) {
                    Ok(()) => self.set_feedback(
                        ToastLevel::Success,
                        String::from("Markers saved"),
                        true,
                    ),
                    Err(e) => self.set_feedback(
                        ToastLevel::Warning,
                        format!("Save failed: {e}"),
                        false,
                    ),
                }
            }

            // :mapmarker load — reload from disk
            Some("load" | "reload") => {
                let path = self.map_state.marker_file.clone();
                self.map_state.named_markers = load_named_markers_pub(&path);
                let n = self.map_state.named_markers.len();
                self.set_feedback(
                    ToastLevel::Success,
                    format!("Loaded {n} marker(s) from disk"),
                    true,
                );
            }

            _ => {
                self.usage_feedback(
                    "mapmarker",
                    "Usage: mapmarker <set|recall|clear|list|save|load> [name] [x y [z]]",
                );
            }
        }
    }

    fn handle_mapshow_command(&mut self, parts: &[&str]) {
        let Some(pn) = parts.get(1) else {
            self.usage_feedback(
                "mapshow",
                "Usage: mapshow <all|tactical|nav|geometry|spawns>",
            );
            return;
        };
        let preset = match pn.to_ascii_lowercase().as_str() {
            "all" => MapVisibilityPreset::All,
            "tactical" | "tac" => MapVisibilityPreset::Tactical,
            "navigation" | "nav" => MapVisibilityPreset::Navigation,
            "geometry" | "geo" => MapVisibilityPreset::GeometryOnly,
            "spawns" => MapVisibilityPreset::SpawnsOnly,
            _ => {
                self.usage_feedback(
                    "mapshow",
                    "Usage: mapshow <all|tactical|nav|geometry|spawns>",
                );
                return;
            }
        };
        self.map_state.apply_visibility_preset(preset);
        self.map_spawn_cache.clear();
        self.set_feedback(
            ToastLevel::Success,
            format!("Visibility preset: {pn}"),
            true,
        );
    }

    fn handle_maphide_command(&mut self, parts: &[&str]) {
        let Some(layer) = parts.get(1) else {
            self.usage_feedback(
                "maphide",
                "Usage: maphide <geometry|spawns|paths|mesh|labels|annotations>",
            );
            return;
        };
        let msg = match layer.to_ascii_lowercase().as_str() {
            "geometry" | "geo" => {
                self.map_state.show_geometry = false;
                "Geometry hidden"
            }
            "spawns" => {
                self.map_state.show_spawns = false;
                self.map_spawn_cache.clear();
                "Spawns hidden"
            }
            "paths" | "nav" => {
                self.map_state.show_nav_paths = false;
                "Nav paths hidden"
            }
            "mesh" | "navmesh" => {
                self.map_state.show_navmesh = false;
                "Navmesh hidden"
            }
            "labels" => {
                self.map_state.show_labels = false;
                "Labels hidden"
            }
            "annotations" => {
                self.map_state.show_annotations = false;
                "Annotations hidden"
            }
            _ => {
                self.usage_feedback(
                    "maphide",
                    "Usage: maphide <geometry|spawns|paths|mesh|labels|annotations>",
                );
                return;
            }
        };
        self.set_feedback(ToastLevel::Info, String::from(msg), true);
    }

    fn handle_mapnames_command(&mut self, parts: &[&str]) {
        match parts.get(1) {
            None => {
                self.map_state.name_style = self.map_state.name_style.next();
                self.set_feedback(
                    ToastLevel::Info,
                    format!("Map labels: {}", self.map_state.name_style.label()),
                    true,
                );
            }
            Some(s) => {
                if let Some(style) = MapNameStyle::parse(s) {
                    self.map_state.name_style = style;
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Map labels: {}", style.label()),
                        true,
                    );
                } else {
                    self.usage_feedback(
                        "mapnames",
                        "Usage: mapnames [off|name|namelevel|nameclass]",
                    );
                }
            }
        }
    }

    fn handle_map_layer_command(&mut self, parts: &[&str]) {
        let Some(ln) = parts.get(1) else {
            self.usage_feedback("map", "Usage: map <layer> [on|off]");
            return;
        };
        let num = match ln.to_ascii_lowercase().as_str() {
            "geometry" | "geo" => 1u8,
            "spawns" => 2,
            "paths" | "nav" => 3,
            "mesh" | "navmesh" => 4,
            "labels" => 5,
            "annotations" => 6,
            _ => {
                self.usage_feedback("map", "Usage: map <layer> [on|off]");
                return;
            }
        };
        if let Some(st) = parts.get(2) {
            let Some(on) = parse_bool_flag(st) else {
                self.usage_feedback("map", "Usage: map <layer> [on|off]");
                return;
            };
            match num {
                1 => self.map_state.show_geometry = on,
                2 => self.map_state.show_spawns = on,
                3 => self.map_state.show_nav_paths = on,
                4 => self.map_state.show_navmesh = on,
                5 => self.map_state.show_labels = on,
                6 => self.map_state.show_annotations = on,
                _ => {}
            }
            let l = match num {
                1 => "Geometry",
                2 => "Spawns",
                3 => "Nav paths",
                4 => "Navmesh",
                5 => "Labels",
                _ => "Annotations",
            };
            self.map_spawn_cache.clear();
            self.set_feedback(
                ToastLevel::Info,
                format!("{l} {}", if on { "ON" } else { "OFF" }),
                true,
            );
        } else {
            let msg = self.map_state.toggle_layer(num);
            self.map_spawn_cache.clear();
            self.set_feedback(ToastLevel::Info, String::from(msg), true);
        }
    }

    fn handle_highlight_command(&mut self, parts: &[&str]) {
        match parts.get(1).map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("add") => {
                let Some(pat) = parts.get(2) else {
                    self.usage_feedback(
                        "highlight",
                        "Usage: highlight add <pattern> [color=X] [size=N] [pulse]",
                    );
                    return;
                };
                let (mut color, mut size, mut pulse) = (None, 1u8, false);
                for &a in parts.iter().skip(3) {
                    if let Some(c) = a.strip_prefix("color=") {
                        color = super::theme::parse_color_name(c);
                    } else if let Some(s) = a.strip_prefix("size=") {
                        size = s.parse().unwrap_or(1_u8).clamp(1, 3);
                    } else if a.eq_ignore_ascii_case("pulse") {
                        pulse = true;
                    }
                }
                let pat_str = (*pat).to_string();
                let pat_lower = pat_str.to_ascii_lowercase();
                self.map_state.add_highlight(MapHighlight {
                    pattern: pat_str,
                    pattern_lower: pat_lower,
                    color,
                    size,
                    pulse,
                });
                self.set_feedback(
                    ToastLevel::Success,
                    format!("Highlight added for \"{pat}\""),
                    true,
                );
            }
            Some("remove" | "rm") => {
                let Some(pat) = parts.get(2) else {
                    self.usage_feedback("highlight", "Usage: highlight remove <pattern>");
                    return;
                };
                let n = self.map_state.remove_highlight(pat);
                if n > 0 {
                    self.set_feedback(
                        ToastLevel::Success,
                        format!("Removed {n} highlight(s)"),
                        true,
                    );
                } else {
                    self.set_feedback(
                        ToastLevel::Warning,
                        format!("No highlight matching \"{pat}\""),
                        false,
                    );
                }
            }
            Some("list") => {
                if self.map_state.highlights.is_empty() {
                    self.set_feedback(ToastLevel::Info, String::from("No highlights"), false);
                } else {
                    let items: Vec<String> = self
                        .map_state
                        .highlights
                        .iter()
                        .map(|h| {
                            let mut d = format!("\"{}\"", h.pattern);
                            if let Some(c) = h.color {
                                d.push_str(&format!(" color={c:?}"));
                            }
                            if h.size > 1 {
                                d.push_str(&format!(" size={}", h.size));
                            }
                            if h.pulse {
                                d.push_str(" pulse");
                            }
                            d
                        })
                        .collect();
                    self.set_feedback(
                        ToastLevel::Info,
                        format!("Highlights: {}", items.join(", ")),
                        false,
                    );
                }
            }
            Some("clear") => {
                let n = self.map_state.highlights.len();
                self.map_state.highlights.clear();
                self.set_feedback(
                    ToastLevel::Success,
                    format!("Cleared {n} highlight(s)"),
                    true,
                );
            }
            _ => {
                self.usage_feedback(
                    "highlight",
                    "Usage: highlight <add|remove|list|clear> [pattern] [color=X] [size=N] [pulse]",
                );
            }
        }
    }
}

/// Parse "on"/"1"/"true" → true, "off"/"0"/"false" → false.
fn parse_bool_flag(s: &str) -> Option<bool> {
    match s.to_ascii_lowercase().as_str() {
        "on" | "1" | "true" => Some(true),
        "off" | "0" | "false" => Some(false),
        _ => None,
    }
}

/// Generate demo hex data for the hex dump viewer in macOS demo mode.
/// Produces a realistic-looking spawn struct with recognizable fields.
#[cfg(not(windows))]
fn generate_demo_hex_data(name: &str, spawn_id: u32) -> Vec<u8> {
    let mut data = vec![0u8; 0x200];

    // Write spawn ID at a typical offset
    let id_bytes = spawn_id.to_le_bytes();
    data[0x00..0x04].copy_from_slice(&id_bytes);

    // Write name as ASCII at a recognizable offset
    let name_bytes = name.as_bytes();
    let len = name_bytes.len().min(63);
    data[0x10..0x10 + len].copy_from_slice(&name_bytes[..len]);

    // Write some float-like position data
    let x_bytes = 1234.5f32.to_le_bytes();
    let y_bytes = (-567.8f32).to_le_bytes();
    let z_bytes = 12.0f32.to_le_bytes();
    data[0x80..0x84].copy_from_slice(&x_bytes);
    data[0x84..0x88].copy_from_slice(&y_bytes);
    data[0x88..0x8C].copy_from_slice(&z_bytes);

    // Add some non-zero bytes to make it look realistic
    for i in (0xA0..0x200).step_by(7) {
        data[i] = ((i * 13 + spawn_id as usize) & 0xFF) as u8;
    }

    data
}

/// Generate demo hex data for an EQ Internals offset entry (macOS demo mode).
#[cfg(not(windows))]
fn generate_demo_hex_data_for_offset(name: &str, addr: u64) -> Vec<u8> {
    let mut data = vec![0u8; 0x200];

    // Write the address itself at the start as a pointer-like value.
    let addr_bytes = addr.to_le_bytes();
    data[0x00..0x08].copy_from_slice(&addr_bytes);

    // Write the offset name as ASCII.
    let name_bytes = name.as_bytes();
    let len = name_bytes.len().min(63);
    data[0x10..0x10 + len].copy_from_slice(&name_bytes[..len]);

    // Fill with deterministic non-zero bytes based on address.
    for i in (0x80..0x200).step_by(5) {
        data[i] = ((i.wrapping_mul(17).wrapping_add(addr as usize)) & 0xFF) as u8;
    }

    data
}

/// Extract account number from a character name or window title.
/// Looks for trailing digits (e.g., "player05" → 5).
pub fn extract_account_number(name: &str) -> Option<u8> {
    let digits: String = name
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.is_empty() {
        return None;
    }
    let digits: String = digits.chars().rev().collect();
    digits.parse().ok()
}

/// Get a command syntax hint for the <command> input fragment.
pub fn command_syntax_hint(input: &str) -> Option<&'static str> {
    command::find_command_hint(input).map(|hint| hint.usage)
}

/// Get a matching inline example for the current command fragment.
pub fn command_example_hint(input: &str) -> Option<&'static str> {
    command::find_command_hint(input).map(|hint| hint.example)
}

#[cfg(test)]
fn command_help_detail(command: &str) -> Option<&'static str> {
    command::command_entry(command).map(|entry| entry.summary)
}

/// Send a slash command to a specific PID via named pipe.
fn send_slash_command(pid: u32, command: &str) -> anyhow::Result<()> {
    use textquest_common::ipc::Command;

    if let Some(message) = crate::nav::try_handle_local_slash_command(pid, command)? {
        tracing::info!(pid, %message, "Handled local slash command");
        return Ok(());
    }

    send_ipc_command(
        pid,
        &Command::SlashCommand {
            command: command.to_string(),
        },
    )
}

fn send_ipc_command(pid: u32, cmd: &textquest_common::ipc::Command) -> anyhow::Result<()> {
    use crate::ipc::pipe::CommandPipe;

    let token = crate::ipc::load_session_token(pid).with_context(|| {
        format!("missing session token for PID {pid}; inject the DLL before sending commands")
    })?;
    let session_id = textquest_common::ipc::session_id_from_token(&token);
    let pipe = CommandPipe::connect(pid, session_id)?;
    pipe.send_raw_token(&token)?;
    pipe.send_async(cmd)?;
    Ok(())
}

fn is_reserved_command_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "help"
            | "camp"
            | "nav"
            | "loot"
            | "door"
            | "click"
            | "status"
            | "login"
            | "launch"
            | "stop"
            | "restart"
            | "track"
            | "untrack"
            | "watch"
            | "unwatch"
            | "alerts"
            | "mode"
            | "combat"
            | "ma"
            | "assist"
            | "mt"
            | "tank"
            | "engage"
            | "pull"
            | "disengage"
            | "invite"
            | "accept"
            | "heal"
            | "ch"
            | "chui"
            | "inject"
            | "profile"
            | "all"
            | "scope"
            | "session"
    )
}

fn spawn_matches_filter(spawn: &SpawnInfo, spawn_filter: SpawnFilter, text_filter: &str) -> bool {
    let matches_type = match spawn_filter {
        SpawnFilter::All => true,
        SpawnFilter::Pc => spawn.spawn_type == SpawnType::Player,
        SpawnFilter::Npc => spawn.spawn_type == SpawnType::Npc,
        SpawnFilter::Named => {
            spawn.spawn_type == SpawnType::Npc
                && !spawn.displayed_name.starts_with("a ")
                && !spawn.displayed_name.starts_with("an ")
        }
    };
    if !matches_type {
        return false;
    }

    if text_filter.is_empty() {
        return true;
    }

    ascii_icontains(&spawn.displayed_name, text_filter)
        || ascii_icontains(&spawn.class_str(), text_filter)
        || ascii_icontains(spawn.spawn_type.as_str(), text_filter)
}

/// Case-insensitive substring search for ASCII strings, without heap allocation.
/// `needle` is expected to be already lowercased.
fn ascii_icontains(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let n = needle.as_bytes();
    haystack
        .as_bytes()
        .windows(n.len())
        .any(|w| w.eq_ignore_ascii_case(n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eq::structs::{SpawnInfo, SpawnType, StandState};
    use crate::orchestrator::Orchestrator;

    fn test_spawn(name: &str) -> SpawnInfo {
        SpawnInfo {
            name: name.into(),
            displayed_name: name.into(),
            lastname: String::new(),
            spawn_id: 1,
            spawn_type: SpawnType::Player,
            level: 60,
            class_id: 1,
            class: None,
            stand_state: StandState::Standing,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 100,
            hp_max: 100,
            mana_current: 100,
            mana_max: 100,
            endurance_current: 100,
            endurance_max: 100,
            is_gm: false,
            race_id: 1,
            buff_slots: Vec::new(),
            spellbook: Vec::new(),
            memorized_spells: Vec::new(),
            cast_state: None,
        }
    }

    fn test_client(pid: u32, name: &str) -> ClientState {
        let mut client = ClientState::new(pid, 0);
        client.character_name = name.into();
        client.local_player = Some(test_spawn(name));
        client
    }

    #[test]
    fn client_command_target_respects_privacy_mode() {
        let mut app = App::new();
        app.clients.push(test_client(42, "Alpha"));
        app.privacy_mode = true;

        let target = app.client_command_target(&app.clients[0]);

        assert_eq!(target, "Toon-01");
    }

    #[test]
    fn parse_client_prefix_preserves_reserved_commands() {
        let mut app = App::new();
        app.clients.push(test_client(1, "all"));

        assert_eq!(app.parse_client_prefix("all /sit"), None);
        assert_eq!(app.parse_client_prefix("@all /sit"), Some((0, "/sit")));
    }

    #[test]
    fn split_command_preserves_full_slash_tail() {
        let app = App::new();
        assert_eq!(app.split_command("all /assist Bob"), ("all", "/assist Bob"));
        assert_eq!(
            app.split_command("1234 /assist Bob"),
            ("1234", "/assist Bob")
        );
    }

    #[test]
    fn parse_group_prefix_preserves_full_slash_tail() {
        let app = App::new();
        assert_eq!(
            app.parse_group_prefix("G1 /assist Bob"),
            Some((0, "/assist Bob"))
        );
    }

    #[test]
    fn send_ipc_command_errors_when_session_token_is_missing() {
        let pid = u32::MAX - 7;
        let login_token_path = std::env::temp_dir()
            .join("textquest")
            .join(format!("login_token_{}.bin", pid));
        let _ = std::fs::remove_file(&login_token_path);

        let error = send_ipc_command(
            pid,
            &textquest_common::ipc::Command::SlashCommand {
                command: String::from("/sit"),
            },
        )
        .expect_err("missing login token should fail before pipe connect");

        assert!(
            error
                .to_string()
                .contains("missing session token for PID 4294967288"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn did_you_mean_close_match() {
        assert_eq!(
            command::did_you_mean("campp").map(|suggestion| suggestion.phrase),
            Some("camp")
        );
        assert_eq!(
            command::did_you_mean("navv").map(|suggestion| suggestion.phrase),
            Some("nav")
        );
        assert_eq!(
            command::did_you_mean("engge").map(|suggestion| suggestion.phrase),
            Some("engage")
        );
        assert_eq!(
            command::did_you_mean("disengag").map(|suggestion| suggestion.phrase),
            Some("disengage")
        );
    }

    #[test]
    fn did_you_mean_no_match() {
        assert_eq!(command::did_you_mean("xyzzy"), None);
        assert_eq!(command::did_you_mean("foobarqux"), None);
    }

    #[test]
    fn did_you_mean_exact_match_returns_itself() {
        assert_eq!(
            command::did_you_mean("help").map(|suggestion| suggestion.phrase),
            Some("help")
        );
        assert_eq!(
            command::did_you_mean("status").map(|suggestion| suggestion.phrase),
            Some("status")
        );
    }

    #[test]
    fn did_you_mean_prefix_match() {
        assert_eq!(
            command::did_you_mean("ca").map(|suggestion| suggestion.phrase),
            Some("camp")
        );
        assert_eq!(
            command::did_you_mean("st").map(|suggestion| suggestion.phrase),
            Some("status")
        );
    }

    #[test]
    fn normalize_command_aliases() {
        assert_eq!(command::normalize_command_alias("h"), "help");
        assert_eq!(command::normalize_command_alias("q"), "quit");
        assert_eq!(command::normalize_command_alias("chui"), "chui");
        assert_eq!(command::normalize_command_alias("cfg"), "config");
        assert_eq!(command::normalize_command_alias("cmds"), "commands");
        assert_eq!(command::normalize_command_alias("s"), "status");
        assert_eq!(
            command::normalize_command_alias("overview"),
            "status overview"
        );
        assert_eq!(command::normalize_command_alias("camp start"), "camp start");
        assert_eq!(command::normalize_command_alias("assist Bob"), "ma Bob");
        assert_eq!(command::normalize_command_alias("tank Bob"), "mt Bob");
        assert_eq!(command::normalize_command_alias("pull 1234"), "engage 1234");
        assert_eq!(
            command::normalize_command_alias("combat status"),
            "combat status"
        );
    }

    #[test]
    fn combat_status_summary_reports_scope_and_focus() {
        let mut app = App::new();
        app.main_assist = Some(String::from("Warrior"));
        app.main_tank = Some(String::from("Paladin"));

        let summary = app.combat_status_summary();

        assert!(summary.contains("Combat: mode=Camp"));
        assert!(summary.contains("scope=All Groups"));
        assert!(summary.contains("focused=0/0 clients"));
        assert!(summary.contains("MA=Warrior"));
        assert!(summary.contains("MT=Paladin"));
    }

    #[test]
    fn command_help_detail_aliases() {
        assert!(command_help_detail("h").is_some());
        assert!(command_help_detail("cmds").is_some());
        assert!(command_help_detail("cfg").is_some());
        assert!(command_help_detail("assist").is_some());
        assert!(command_help_detail("tank").is_some());
        assert!(command_help_detail("pull").is_some());
        assert!(command_help_detail("bogus").is_none());
    }

    #[test]
    fn known_commands_has_all_expected_commands() {
        let names: Vec<&str> = command::command_entries()
            .iter()
            .flat_map(|entry| std::iter::once(entry.phrase).chain(entry.aliases.iter().copied()))
            .collect();
        for expected in &[
            "help",
            "commands",
            "status",
            "camp",
            "nav",
            "loot",
            "door",
            "click",
            "login",
            "launch",
            "profile",
            "stop",
            "restart",
            "track",
            "untrack",
            "watch",
            "unwatch",
            "alerts",
            "mode",
            "combat",
            "ma",
            "assist",
            "mt",
            "tank",
            "engage",
            "pull",
            "disengage",
            "invite",
            "accept",
            "heal",
            "ch",
            "chui",
            "inject",
            "all",
            "scope",
            "session",
            "cmds",
            "quit",
            "config",
        ] {
            assert!(
                names.contains(expected),
                "Command metadata missing '{expected}'"
            );
        }
    }

    #[test]
    fn help_commands_open_expected_focus_targets() {
        let mut app = App::new();
        let mut orchestrator = Orchestrator::new();

        app.cmd_state.command_buffer = String::from("help nav");
        app.execute_command(&mut orchestrator);
        assert!(app.help_visible);
        assert_eq!(app.help_focus, Some(HelpFocus::Command("nav")));

        app.help_visible = false;
        app.help_focus = None;
        app.cmd_state.command_buffer = String::from("commands");
        app.execute_command(&mut orchestrator);
        assert!(app.help_visible);
        assert_eq!(
            app.help_focus,
            Some(HelpFocus::Section(HelpSection::Combat))
        );
    }

    #[test]
    fn invalid_mode_feedback_includes_usage_and_example() {
        let mut app = App::new();
        let mut orchestrator = Orchestrator::new();

        app.cmd_state.command_buffer = String::from("mode raid");
        app.execute_command(&mut orchestrator);

        let toast = app.toast.as_ref().expect("warning toast");
        assert_eq!(toast.level, ToastLevel::Warning);
        assert!(app.status_message.contains("Invalid mode 'raid'."));
        assert!(app.status_message.contains("Usage: mode <camp|hunt>."));
        assert!(app.status_message.contains("Example: :mode hunt"));
    }

    #[test]
    fn toast_dedupe_refreshes_timestamp_and_expires_after_ttl() {
        let mut app = App::new();
        app.tick_count = 10;
        app.set_toast(ToastLevel::Warning, "Camp loop paused");
        let first_tick = app.toast.as_ref().expect("toast").set_tick;
        assert_eq!(first_tick, 10);
        assert_eq!(
            app.toast.as_ref().expect("toast").ttl_ticks,
            ToastLevel::Warning.ttl_ticks()
        );

        app.tick_count = 25;
        app.set_toast(ToastLevel::Warning, "Camp loop paused");
        let refreshed = app.toast.as_ref().expect("toast");
        assert_eq!(refreshed.set_tick, 25);

        app.tick_count = 65;
        app.clear_expired_toast();
        assert!(
            app.toast.is_some(),
            "warning should still be visible at ttl"
        );

        app.tick_count = 66;
        app.clear_expired_toast();
        assert!(
            app.toast.is_none(),
            "warning should clear once ttl is exceeded"
        );
    }

    #[test]
    fn ch_status_feedback_is_informational_when_inactive() {
        let mut app = App::new();
        let mut orchestrator = Orchestrator::new();

        app.cmd_state.command_buffer = String::from("ch status");
        app.execute_command(&mut orchestrator);

        assert_eq!(app.toast, None);
        assert!(app.status_message.contains("CH chain inactive."));
        assert!(app.status_message.contains(":ch start <pid1,pid2,...>"));
    }

    #[test]
    fn sync_from_selected_client_only_copies_spawns_when_revision_changes() {
        let mut app = App::new();
        let mut client = test_client(42, "Alpha");
        client.spawn_revision = 1;
        client.spawns = vec![test_spawn("Guard")];
        app.clients.push(client);

        app.sync_from_selected_client();
        assert_eq!(app.spawns.len(), 1);
        assert_eq!(app.spawns[0].displayed_name, "Guard");

        app.clients[0].spawns = vec![test_spawn("Wizard")];
        app.sync_from_selected_client();
        assert_eq!(app.spawns.len(), 1);
        assert_eq!(app.spawns[0].displayed_name, "Guard");

        app.clients[0].spawn_revision = 2;
        app.sync_from_selected_client();
        assert_eq!(app.spawns.len(), 1);
        assert_eq!(app.spawns[0].displayed_name, "Wizard");
    }

    #[test]
    fn next_client_marks_new_selection_for_immediate_spawn_refresh() {
        let now = std::time::Instant::now();
        let mut app = App::new();
        let mut alpha = test_client(1, "Alpha");
        let mut bravo = test_client(2, "Bravo");
        alpha.last_spawn_refresh = Some(now);
        bravo.last_spawn_refresh = Some(now);
        app.clients.push(alpha);
        app.clients.push(bravo);

        app.next_client();

        assert_eq!(app.selected_client, 1);
        assert!(app.clients[1].last_spawn_refresh.is_none());
        assert_eq!(app.spawns_state.table_state.selected(), Some(0));
    }

    // ── M8 routing scope tests ──────────────────────────────────────────────

    #[test]
    fn app_default_routing_scope_is_all_session() {
        let app = App::new();
        assert_eq!(
            app.routing_scope,
            textquest_common::routing::RoutingScope::AllSession
        );
    }

    #[test]
    fn scope_command_no_args_shows_current_scope() {
        let mut app = App::new();
        app.clients.push(test_client(1, "Warrior"));
        app.execute_scope_command(&[]);
        // Feedback should include "All" (the default scope label)
        assert!(app.status_message.contains("All") || !app.status_message.is_empty());
    }

    #[test]
    fn scope_command_all_resets_to_all_session() {
        use textquest_common::routing::RoutingScope;
        let mut app = App::new();
        // Start with a narrowed scope
        app.routing_scope = RoutingScope::OneToon {
            name: "Warrior".into(),
        };
        app.execute_scope_command(&["all"]);
        assert_eq!(app.routing_scope, RoutingScope::AllSession);
        assert!(app.active_group.is_none());
    }

    #[test]
    fn scope_command_group_sets_group_scope() {
        use textquest_common::routing::RoutingScope;
        let mut app = App::new();
        app.execute_scope_command(&["G1"]);
        assert!(matches!(
            app.routing_scope,
            RoutingScope::Group { group_id: 1, .. }
        ));
    }

    #[test]
    fn scope_command_character_name_sets_one_toon_scope() {
        use textquest_common::routing::RoutingScope;
        let mut app = App::new();
        app.clients.push(test_client(1, "Cleric"));
        app.execute_scope_command(&["Cleric"]);
        assert!(matches!(&app.routing_scope, RoutingScope::OneToon { name } if name == "Cleric"));
        assert!(app.active_group.is_none());
    }

    #[test]
    fn scope_command_unknown_character_shows_error_feedback() {
        let mut app = App::new();
        app.execute_scope_command(&["NonExistentToon"]);
        // Feedback should mention the unknown name
        assert!(app.status_message.contains("NonExistentToon") || !app.status_message.is_empty());
    }

    #[test]
    fn focused_pids_follow_routing_scope_not_ui_group_filter() {
        use textquest_common::routing::RoutingScope;

        let mut app = App::new();
        app.clients.push(test_client(1, "Toon01"));
        app.clients.push(test_client(2, "Toon07"));
        app.active_group = Some(0);
        app.routing_scope = RoutingScope::OneToon {
            name: "Toon07".into(),
        };

        assert_eq!(app.focused_pids(), vec![2]);
        assert_eq!(app.focused_pid_count(), 1);
    }

    #[test]
    fn parse_all_prefix_requires_a_separate_target_token() {
        let app = App::new();

        assert_eq!(
            app.parse_all_prefix("all combat scope"),
            Some("combat scope")
        );
        assert_eq!(
            app.parse_all_prefix("all   combat scope"),
            Some("combat scope")
        );
        assert_eq!(app.parse_all_prefix("all"), None);
        assert_eq!(app.parse_all_prefix("allcombat"), None);
    }

    #[test]
    fn temporary_routing_scope_restores_after_return_and_panic() {
        use textquest_common::routing::RoutingScope;

        let mut app = App::new();
        let group_scope = RoutingScope::Group {
            group_id: 1,
            label: "Alpha".into(),
        };

        let label = app
            .with_temporary_routing_scope(group_scope.clone(), |inner| inner.routing_scope.label());
        assert_eq!(label, "G1 Alpha");
        assert_eq!(app.routing_scope, RoutingScope::AllSession);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            app.with_temporary_routing_scope(group_scope, |_| panic!("boom"));
        }));
        assert!(result.is_err());
        assert_eq!(app.routing_scope, RoutingScope::AllSession);
    }

    #[test]
    fn routed_pids_for_all_scope_uses_all_clients_without_leaking_scope() {
        use textquest_common::routing::RoutingScope;

        let mut app = App::new();
        app.clients.push(test_client(1, "Toon01"));
        app.clients.push(test_client(2, "Toon07"));
        app.routing_scope = RoutingScope::OneToon {
            name: "Toon01".into(),
        };

        assert_eq!(
            app.routed_pids_for_scope(&RoutingScope::AllSession),
            vec![1, 2]
        );
        assert_eq!(
            app.routing_scope,
            RoutingScope::OneToon {
                name: "Toon01".into()
            }
        );
    }

    #[test]
    fn group_prefix_routes_non_slash_commands_through_temporary_scope() {
        use textquest_common::routing::RoutingScope;

        let mut app = App::new();
        let mut orchestrator = Orchestrator::new();
        app.clients.push(test_client(1, "Toon01"));
        app.clients.push(test_client(2, "Toon07"));
        app.cmd_state.command_buffer = String::from("G1 combat scope");

        app.execute_command(&mut orchestrator);

        assert!(app.status_message.contains("Combat scope: G1"));
        assert!(app.status_message.contains("focused 1 clients"));
        assert_eq!(app.routing_scope, RoutingScope::AllSession);
    }

    #[test]
    fn all_prefix_routes_non_slash_commands_through_temporary_scope() {
        use textquest_common::routing::RoutingScope;

        let mut app = App::new();
        let mut orchestrator = Orchestrator::new();
        app.clients.push(test_client(1, "Toon01"));
        app.clients.push(test_client(2, "Toon07"));
        app.routing_scope = RoutingScope::OneToon {
            name: "Toon01".into(),
        };
        app.cmd_state.command_buffer = String::from("all combat scope");

        app.execute_command(&mut orchestrator);

        assert!(app.status_message.contains("Combat scope: All"));
        assert!(app.status_message.contains("focused 2 clients"));
        assert_eq!(
            app.routing_scope,
            RoutingScope::OneToon {
                name: "Toon01".into()
            }
        );
    }

    #[test]
    fn toon_prefix_routes_non_slash_commands_through_temporary_scope() {
        use textquest_common::routing::RoutingScope;

        let mut app = App::new();
        let mut orchestrator = Orchestrator::new();
        app.clients.push(test_client(1, "Toon01"));
        app.clients.push(test_client(2, "Toon07"));
        app.cmd_state.command_buffer = String::from("Toon07 combat scope");

        app.execute_command(&mut orchestrator);

        assert!(app.status_message.contains("Combat scope: @Toon07"));
        assert!(app.status_message.contains("focused 1 clients"));
        assert_eq!(app.routing_scope, RoutingScope::AllSession);
    }

    #[test]
    fn session_command_no_clients_warns() {
        let mut app = App::new();
        app.execute_session_command(&[]);
        assert!(!app.status_message.is_empty());
    }

    #[test]
    fn session_command_list_with_clients() {
        let mut app = App::new();
        app.clients.push(test_client(1, "Warrior"));
        app.clients.push(test_client(2, "Cleric"));
        app.execute_session_command(&["list"]);
        // Feedback should mention both character names
        let msg = &app.status_message;
        assert!(msg.contains("Warrior") || msg.contains("2 slot"));
    }

    #[test]
    fn session_command_status_shows_connected_count() {
        let mut app = App::new();
        app.clients.push(test_client(1, "Warrior"));
        app.execute_session_command(&["status"]);
        assert!(app.status_message.contains('1') || !app.status_message.is_empty());
    }

    #[test]
    fn is_reserved_command_name_includes_scope_and_session() {
        assert!(is_reserved_command_name("scope"));
        assert!(is_reserved_command_name("session"));
        assert!(is_reserved_command_name("SCOPE"));
        assert!(is_reserved_command_name("SESSION"));
    }

    #[test]
    fn automation_paused_starts_false() {
        let app = App::new();
        assert!(!app.automation_paused);
    }

    #[test]
    fn automation_pause_resume_toggles() {
        let mut app = App::new();
        assert!(!app.automation_paused);

        app.automation_paused = true;
        assert!(app.automation_paused);

        app.automation_paused = false;
        assert!(!app.automation_paused);
    }

    #[test]
    fn automation_pause_idempotent() {
        let mut app = App::new();
        app.automation_paused = true;
        app.automation_paused = true;
        assert!(app.automation_paused);
    }

    // ── Layout presets ──────────────────────────────────────────────────────

    #[test]
    fn layout_preset_cycles_through_all() {
        let p = LayoutPreset::Default;
        assert_eq!(p.next(), LayoutPreset::Alternate);
        assert_eq!(p.next().next(), LayoutPreset::Compact);
        assert_eq!(p.next().next().next(), LayoutPreset::Default);
    }

    #[test]
    fn layout_preset_labels() {
        assert_eq!(LayoutPreset::Default.label(), "Default");
        assert_eq!(LayoutPreset::Alternate.label(), "Wide");
        assert_eq!(LayoutPreset::Compact.label(), "Compact");
    }

    #[test]
    fn cycle_layout_per_screen() {
        let mut app = App::new();
        assert_eq!(app.current_layout(), LayoutPreset::Default);
        app.cycle_layout();
        assert_eq!(app.current_layout(), LayoutPreset::Alternate);

        // Switch screen — separate preset
        app.set_active_screen(ActiveScreen::Tactical);
        assert_eq!(app.current_layout(), LayoutPreset::Default);
        app.cycle_layout();
        assert_eq!(app.current_layout(), LayoutPreset::Alternate);

        // Go back to overview — preserved
        app.set_active_screen(ActiveScreen::Overview);
        assert_eq!(app.current_layout(), LayoutPreset::Alternate);
    }
}
