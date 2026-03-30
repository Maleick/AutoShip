use std::collections::{HashMap, VecDeque};

use super::theme::{Theme, ThemeKind};
use crate::camp::config::CampConfig;
use crate::camp::state::{CampMember, Role};
use crate::config::AccountsConfig;
use crate::eq::log_parser::{ChatEvent, LootDatabase};
use crate::eq::log_watcher::LogWatcher;
use crate::eq::map_parser::ZoneMap;
use crate::eq::named_db::NamedMobDatabase;
use crate::eq::named_tracker::NamedTracker;
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
    Navigation,
}

impl ActiveScreen {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Spawns => "Spawns",
            Self::Character => "Character",
            Self::Map => "Map",
            Self::Groups => "Groups",
            Self::Navigation => "Nav",
        }
    }

    pub const ALL: [ActiveScreen; 6] = [
        Self::Dashboard,
        Self::Spawns,
        Self::Character,
        Self::Map,
        Self::Groups,
        Self::Navigation,
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

/// Status of a user-tracked spawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackedStatus {
    Up,
    Down,
    Unknown,
}

impl TrackedStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Up => "UP",
            Self::Down => "DOWN",
            Self::Unknown => "???",
        }
    }

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
    pub name: String,
    pub status: TrackedStatus,
    pub last_seen_tick: Option<u64>,
    pub last_x: f32,
    pub last_y: f32,
    pub last_z: f32,
}

/// Definition for a logical group of accounts.
#[derive(Debug, Clone)]
pub struct GroupDef {
    pub id: u8,
    pub name: String,
    pub account_range: (u8, u8),
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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

    // Active group focus: None = aggregate view, Some(0..5) = focused on group
    pub active_group: Option<usize>,

    // Server name from config
    pub server_name: String,

    // Legacy single-client fields kept for compatibility
    pub local_player: Option<SpawnInfo>,
    pub target: Option<SpawnInfo>,
    pub spawns: Vec<SpawnInfo>,
    pub status_message: String,
    pub tick_count: u64,

    // Spawn list state
    #[allow(dead_code)]
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

    // Named spawn tracking
    pub named_tracker: NamedTracker,

    // User-tracked spawns (via :track command)
    pub tracked_spawns: HashMap<String, TrackedSpawn>,

    // Help overlay
    pub help_visible: bool,

    // Operating mode (camp vs hunt)
    pub operating_mode: crate::camp::hunt::OperatingMode,

    // Combat roles
    pub main_assist: Option<String>,
    pub main_tank: Option<String>,

    // Heal-cancel toggle (cleric duck on high HP during cast)
    pub heal_cancel_enabled: bool,

    // Account config for login automation
    pub accounts_config: Option<AccountsConfig>,

    // Log parsing / session stats
    pub loot_database: LootDatabase,
    pub log_watchers: Vec<LogWatcher>,
    pub session_start: std::time::Instant,
    /// Ring buffer of recent chat events (capped at 200).
    pub chat_events: VecDeque<ChatEvent>,

    // Navigation state
    pub nav_selected: usize,
    pub nav_statuses: HashMap<u32, NavClientStatus>,

    // Theme
    pub theme_kind: ThemeKind,
    pub theme: Theme,
}

/// Navigation status for a single client.
#[derive(Debug, Clone)]
pub struct NavClientStatus {
    pub destination: String,
    pub status: String,
    #[allow(dead_code)]
    pub eta_secs: Option<u32>,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            active_screen: ActiveScreen::Dashboard,
            active_panel: ActivePanel::SpawnList,

            clients: Vec::new(),
            selected_client: 0,
            active_group: None,
            groups: Self::build_default_groups(),

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

            named_tracker: {
                let db = NamedMobDatabase::load(std::path::Path::new("config/named_mobs")).ok();
                match db {
                    Some(db) => NamedTracker::with_db(db),
                    None => NamedTracker::new(),
                }
            },
            tracked_spawns: HashMap::new(),

            help_visible: false,

            operating_mode: crate::camp::hunt::OperatingMode::Camp,

            main_assist: None,
            main_tank: None,
            heal_cancel_enabled: true,

            accounts_config: AccountsConfig::load(std::path::Path::new("config/accounts.toml"))
                .ok(),

            loot_database: LootDatabase::new(),
            log_watchers: Vec::new(),
            session_start: std::time::Instant::now(),
            chat_events: VecDeque::with_capacity(200),

            nav_selected: 0,
            nav_statuses: HashMap::new(),

            theme_kind: ThemeKind::DarkModern,
            theme: ThemeKind::DarkModern.build(),
        }
    }

    /// Cycle to the next theme.
    pub fn cycle_theme(&mut self) {
        self.theme_kind = self.theme_kind.next();
        self.theme = self.theme_kind.build();
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

                        let name = default_names
                            .get((id - 1) as usize)
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("Group {}", id));

                        GroupDef {
                            id: id as u8,
                            name,
                            account_range: (lo, hi),
                            default_camp: format!("Camp {}", id),
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

    /// Rebuild group definitions from accounts config. Called when config changes.
    #[allow(dead_code)]
    pub fn rebuild_groups_from_config(&mut self) {
        self.groups = Self::build_default_groups();
    }

    /// Get the number of groups that have at least one connected client.
    #[allow(dead_code)]
    pub fn active_group_count(&self) -> usize {
        self.groups
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.clients_in_group_idx(*i).is_empty())
            .count()
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

    /// Focus on a specific group (0-indexed). Pass None to return to aggregate view.
    pub fn set_active_group(&mut self, group: Option<usize>) {
        if let Some(idx) = group {
            if idx < self.groups.len() {
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
                if let Some(g) = self.groups.get(idx) {
                    // Find the zone of the first online member
                    let zone = self
                        .clients_in_group_idx(idx)
                        .first()
                        .map(|c| c.zone_name.as_str())
                        .unwrap_or("???");
                    format!("G{} {} ({})", g.id, g.name, zone)
                } else {
                    String::from("All Groups")
                }
            }
        }
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
    pub fn visible_clients(&self) -> Vec<&ClientState> {
        match self.active_group {
            None => self.clients.iter().collect(),
            Some(idx) => self.clients_in_group_idx(idx),
        }
    }

    /// Get PIDs of clients in the focused group (or all if aggregate).
    pub fn focused_pids(&self) -> Vec<u32> {
        self.visible_clients().iter().map(|c| c.pid).collect()
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
        let info: Option<(String, u32, usize)> = {
            let filtered = self.filtered_spawns();
            filtered
                .get(self.spawn_selected)
                .map(|s| (s.displayed_name.clone(), s.spawn_id, self.spawn_selected))
        };
        if let Some((name, id, _idx)) = info {
            self.hex_label = format!("Raw memory: {} (ID {})", name, id);
            self.status_message = format!("Inspecting: {}", name);

            // On Windows, read real spawn memory; on macOS, generate demo hex data
            #[cfg(windows)]
            {
                self.hex_data = self.read_spawn_hex_data(id);
                self.hex_address = 0;
            }
            #[cfg(not(windows))]
            {
                self.hex_data = generate_demo_hex_data(&name, id);
                self.hex_address = 0x1000;
            }
        }
    }

    /// Read spawn memory on Windows for hex dump display.
    #[cfg(windows)]
    fn read_spawn_hex_data(&self, spawn_id: u32) -> Vec<u8> {
        use crate::process::memory::ProcessHandle;
        use dmft_common::offsets;

        if let Some(client) = self.active_client() {
            if let Ok(proc) = ProcessHandle::open(client.pid) {
                // Find the spawn address by walking the spawn list
                let mgr_ptr_addr =
                    match offsets::rebase(offsets::PINST_SPAWN_MANAGER, client.eq_base) {
                        Some(a) => a,
                        None => return Vec::new(),
                    };
                let mgr_addr = match proc.read_ptr(mgr_ptr_addr) {
                    Ok(a) if a != 0 => a,
                    _ => return Vec::new(),
                };

                let list_addr = mgr_addr + offsets::spawn_manager::PLAYER_LIST;
                let mut current = proc.read_ptr(list_addr).unwrap_or(0);
                while current != 0 {
                    let sid = proc
                        .read::<u32>(current + offsets::player_base::SPAWN_ID)
                        .unwrap_or(0);
                    if sid == spawn_id {
                        return proc.read_bytes(current, 0x200).unwrap_or_default();
                    }
                    current = proc
                        .read_ptr(current + offsets::player_base::NEXT)
                        .unwrap_or(0);
                }
            }
        }
        Vec::new()
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

    pub fn toggle_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::SpawnList => ActivePanel::HexDump,
            ActivePanel::HexDump => ActivePanel::SpawnList,
        };
    }

    /// Tab-complete the current command buffer.
    /// Supports multi-level completion: first tab completes command name,
    /// subsequent tabs complete context-specific arguments.
    pub fn complete_command(&mut self) {
        let buf = self.command_buffer.clone();
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
                    .map(|c| c.zone_name.clone())
                    .unwrap_or_else(|| "camp".into());
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

        // :ma <Tab> → character names
        if let Some(rest) = prefix.strip_prefix("ma ") {
            let names = self.list_character_names();
            self.complete_with_candidates("ma ", rest, &names);
            return;
        }

        // :mt <Tab> → character names
        if let Some(rest) = prefix.strip_prefix("mt ") {
            let names = self.list_character_names();
            self.complete_with_candidates("mt ", rest, &names);
            return;
        }

        // :invite <Tab> → character names
        if let Some(rest) = prefix.strip_prefix("invite ") {
            let names = self.list_character_names();
            self.complete_with_candidates("invite ", rest, &names);
            return;
        }

        // :heal <Tab> → cancel
        if let Some(rest) = prefix.strip_prefix("heal ") {
            let subs: Vec<String> = vec!["cancel".into()];
            self.complete_with_candidates("heal ", rest, &subs);
            return;
        }

        // :all <Tab> → common slash commands
        if let Some(rest) = prefix.strip_prefix("all ") {
            let slash_cmds: Vec<String> = vec![
                "/sit".into(),
                "/stand".into(),
                "/camp".into(),
                "/follow".into(),
                "/assist".into(),
                "/disband".into(),
            ];
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
                let slash_cmds: Vec<String> = vec![
                    "/sit".into(),
                    "/stand".into(),
                    "/camp".into(),
                    "/follow".into(),
                    "/assist".into(),
                    "/disband".into(),
                ];
                self.complete_with_candidates(&format!("{} ", cmd_prefix_str), rest, &slash_cmds);
                return;
            }
        }

        // --- Top-level command completion ---
        let mut candidates: Vec<String> = vec![
            "help".into(),
            "camp".into(),
            "login".into(),
            "mode".into(),
            "all".into(),
            "inject".into(),
            "status".into(),
            "track".into(),
            "untrack".into(),
            "ma".into(),
            "mt".into(),
            "engage".into(),
            "disengage".into(),
            "invite".into(),
            "accept".into(),
            "heal".into(),
            "G1".into(),
            "G2".into(),
            "G3".into(),
            "G4".into(),
            "G5".into(),
            "G6".into(),
        ];

        for client in &self.clients {
            candidates.push(client.pid.to_string());
            if !client.character_name.is_empty() {
                candidates.push(client.character_name.clone());
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
                    format!("\"{}\"", name)
                } else {
                    name.to_string()
                };
                self.command_buffer = format!("{}{} ", cmd_prefix, formatted);
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
                    self.command_buffer = format!("{}{}", cmd_prefix, common);
                }
                // Show available options (truncate if too many)
                let display: Vec<&str> = matches.iter().take(10).map(|s| s.as_str()).collect();
                let suffix = if matches.len() > 10 {
                    format!(" (+{} more)", matches.len() - 10)
                } else {
                    String::new()
                };
                self.status_message = format!("{}{}", display.join(" | "), suffix);
            }
        }
    }

    /// List available camp config file names from config/camps/.
    fn list_camp_names(&self) -> Vec<String> {
        let camps_dir = std::path::Path::new("config/camps");
        match std::fs::read_dir(camps_dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let path = e.path();
                    if path.extension().is_some_and(|ext| ext == "toml") {
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List all spawn display names in the current zone.
    fn list_character_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .clients
            .iter()
            .filter(|c| !c.character_name.is_empty())
            .map(|c| c.character_name.clone())
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
            self.status_message = format!("Already tracking: {}", name);
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

        self.status_message = format!("Tracking: {} [{}]", tracked.name, tracked.status.label());
        self.tracked_spawns.insert(key, tracked);
    }

    /// Remove a spawn from the user-tracked list.
    pub fn untrack_spawn(&mut self, name: &str) {
        let key = name.to_lowercase();
        if self.tracked_spawns.remove(&key).is_some() {
            self.status_message = format!("Untracked: {}", name);
        } else {
            self.status_message = format!("Not tracking: {}", name);
        }
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

    /// Parse a group prefix like "G1", "G2", ..., "G6" from the first word.
    /// Returns (group_idx 0-based, remaining command) if found.
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

    /// Get PIDs for a specific group index (0-based).
    fn pids_for_group(&self, group_idx: usize) -> Vec<u32> {
        self.clients_in_group_idx(group_idx)
            .iter()
            .map(|c| c.pid)
            .collect()
    }

    /// Execute the current command buffer content.
    pub fn execute_command(&mut self, orchestrator: &mut Orchestrator) {
        let input = self.command_buffer.trim().to_string();
        if input.is_empty() {
            return;
        }

        // Save to history
        self.command_history.push(input.clone());

        // Check for group prefix: :G1 /sit, :G2 camp start, etc.
        if let Some((group_idx, rest)) = self.parse_group_prefix(&input) {
            if rest.is_empty() {
                // Just ":G1" with nothing after — focus on that group
                self.set_active_group(Some(group_idx));
                return;
            }
            let g = &self.groups[group_idx];
            let group_name = format!("G{} {}", g.id, g.name);
            let pids = self.pids_for_group(group_idx);
            if pids.is_empty() {
                self.status_message = format!("{}: no online members", group_name);
                return;
            }
            let slash_cmd = rest;
            let mut ok = 0usize;
            let mut fail = 0usize;
            for pid in &pids {
                match send_slash_command(*pid, slash_cmd) {
                    Ok(()) => ok += 1,
                    Err(_) => fail += 1,
                }
            }
            self.status_message = format!(
                "{} {} → sent to {}, failed {}",
                group_name, slash_cmd, ok, fail
            );
            return;
        }

        let parts: Vec<&str> = input.splitn(3, ' ').collect();
        match parts[0] {
            "help" => {
                self.help_visible = true;
            }
            "camp" => {
                self.execute_camp_command(&parts[1..], orchestrator);
            }
            "status" => {
                let client_count = self.clients.len();
                let visible_count = self.visible_clients().len();
                if self.active_group.is_some() {
                    self.status_message = format!(
                        "{} visible / {} total client(s) connected",
                        visible_count, client_count
                    );
                } else {
                    self.status_message = format!("{} client(s) connected", client_count);
                }
            }
            "login" => {
                self.execute_login_command(&parts[1..]);
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
                    self.status_message = String::from("Usage: untrack <name>");
                }
            }
            "mode" => match parts.get(1).copied() {
                Some("camp") => {
                    self.operating_mode = crate::camp::hunt::OperatingMode::Camp;
                    self.status_message = String::from("Switched to Camp mode");
                }
                Some("hunt") => {
                    self.operating_mode = crate::camp::hunt::OperatingMode::Hunt;
                    self.status_message = String::from("Switched to Hunt mode");
                }
                _ => {
                    self.status_message = format!(
                        "Current mode: {}. Usage: mode <camp|hunt>",
                        self.operating_mode
                    );
                }
            },
            "ma" => {
                if let Some(name) = parts.get(1) {
                    self.main_assist = Some(name.to_string());
                    // Send /assist command to all DPS in active group
                    let pids = self.focused_pids();
                    let mut ok = 0;
                    for pid in &pids {
                        if send_slash_command(*pid, &format!("/assist {}", name)).is_ok() {
                            ok += 1;
                        }
                    }
                    tracing::info!(target = %name, sent = ok, "Main Assist set");
                    self.status_message = format!("MA → {} (sent /assist to {} clients)", name, ok);
                } else {
                    self.status_message = match &self.main_assist {
                        Some(ma) => format!("Main Assist: {}", ma),
                        None => "No MA set. Usage: ma <character_name>".into(),
                    };
                }
            }
            "mt" => {
                if let Some(name) = parts.get(1) {
                    self.main_tank = Some(name.to_string());
                    tracing::info!(target = %name, "Main Tank set");
                    self.status_message = format!("MT → {}", name);
                } else {
                    self.status_message = match &self.main_tank {
                        Some(mt) => format!("Main Tank: {}", mt),
                        None => "No MT set. Usage: mt <character_name>".into(),
                    };
                }
            }
            "engage" => {
                let pids = self.focused_pids();
                let target_id = parts
                    .get(1)
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                let mut ok = 0;
                for pid in &pids {
                    let cmd = dmft_common::ipc::Command::CombatEngage { target_id };
                    if send_ipc_command(*pid, &cmd).is_ok() {
                        ok += 1;
                    }
                }
                tracing::info!(target_id, sent = ok, "Combat engage sent");
                self.status_message = format!("Engage → {} clients (target_id={})", ok, target_id);
            }
            "disengage" => {
                let pids = self.focused_pids();
                let mut ok = 0;
                for pid in &pids {
                    let cmd = dmft_common::ipc::Command::CombatDisengage;
                    if send_ipc_command(*pid, &cmd).is_ok() {
                        ok += 1;
                    }
                }
                tracing::info!(sent = ok, "Combat disengage sent");
                self.status_message = format!("Disengage → {} clients", ok);
            }
            "invite" => {
                if let Some(name) = parts.get(1) {
                    if let Some(client) = self.active_client() {
                        let pid = client.pid;
                        let slash = format!("/invite {}", name);
                        match send_slash_command(pid, &slash) {
                            Ok(()) => {
                                tracing::info!(target = %name, pid, "Group invite sent");
                                self.status_message = format!("Invited {} (via PID {})", name, pid);
                            }
                            Err(e) => {
                                self.status_message = format!("Invite failed: {}", e);
                            }
                        }
                    } else {
                        self.status_message = String::from("No active client to send invite from");
                    }
                } else {
                    self.status_message = String::from("Usage: invite <character_name>");
                }
            }
            "accept" => {
                if let Some(client) = self.active_client() {
                    let pid = client.pid;
                    match send_slash_command(pid, "/accept") {
                        Ok(()) => {
                            tracing::info!(pid, "Group invite accepted");
                            self.status_message = format!("Accepted group invite (PID {})", pid);
                        }
                        Err(e) => {
                            self.status_message = format!("Accept failed: {}", e);
                        }
                    }
                } else {
                    self.status_message = String::from("No active client to accept on");
                }
            }
            "heal" => match parts.get(1).copied() {
                Some("cancel") => {
                    self.heal_cancel_enabled = !self.heal_cancel_enabled;
                    let state = if self.heal_cancel_enabled {
                        "ON"
                    } else {
                        "OFF"
                    };
                    tracing::info!(enabled = self.heal_cancel_enabled, "Heal-cancel toggled");
                    self.status_message = format!("Heal-cancel: {}", state);
                }
                _ => {
                    let state = if self.heal_cancel_enabled {
                        "ON"
                    } else {
                        "OFF"
                    };
                    self.status_message = format!(
                        "Heal-cancel is {}. Usage: heal cancel (toggles on/off)",
                        state
                    );
                }
            },
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
                    self.status_message =
                        format!("all {} → sent to {}, failed {}", slash_cmd, ok, fail);
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
                                self.status_message = format!("{} → {}", pid, slash_cmd);
                            }
                            Err(e) => {
                                self.status_message = format!("Error sending to {}: {}", pid, e);
                            }
                        }
                    } else {
                        self.status_message = format!("Usage: {} <slash command>", pid);
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
            None => {
                self.status_message = String::from(
                    "Usage: camp <start|stop|status|list|add|remove|next|prev> [name]",
                );
            }
            Some("start") => {
                let camp_name = match args.get(1) {
                    Some(name) => *name,
                    None => {
                        self.status_message = String::from(
                            "Usage: camp start <name>  (loads config/camps/<name>.toml)",
                        );
                        return;
                    }
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
                            format!("Camp '{}' started with {} members", camp_name, count);
                    }
                    Err(e) => {
                        self.status_message = format!("Failed to load camp '{}': {}", camp_name, e);
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
                let camp_name = match args.get(1) {
                    Some(name) => *name,
                    None => {
                        self.status_message =
                            String::from("Usage: camp add <name>  (saves current position)");
                        return;
                    }
                };

                let (center, zone) = match &self.local_player {
                    Some(player) => {
                        let zone = self
                            .active_client()
                            .map(|c| c.zone_name.clone())
                            .unwrap_or_else(|| "unknown".into());
                        ([player.x, player.y, player.z], zone)
                    }
                    None => {
                        self.status_message =
                            String::from("No player data — cannot save camp position");
                        return;
                    }
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
                        self.status_message = format!("Failed to save camp '{}': {}", camp_name, e);
                    }
                }
            }
            Some("remove") => {
                let camp_name = match args.get(1) {
                    Some(name) => *name,
                    None => {
                        self.status_message = String::from("Usage: camp remove <name>");
                        return;
                    }
                };

                let path = std::path::Path::new("config/camps").join(format!("{}.toml", camp_name));
                if path.exists() {
                    match std::fs::remove_file(&path) {
                        Ok(()) => {
                            self.status_message = format!("Camp '{}' removed", camp_name);
                        }
                        Err(e) => {
                            self.status_message =
                                format!("Failed to remove camp '{}': {}", camp_name, e);
                        }
                    }
                } else {
                    self.status_message = format!("Camp '{}' not found", camp_name);
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
                                    format!("Advanced: {} → {} ({} members)", current, to, count);
                            }
                            Err(e) => {
                                self.status_message =
                                    format!("Failed to load next camp '{}': {}", next_name, e);
                            }
                        },
                        None => {
                            self.status_message =
                                format!("Camp '{}' has no next camp configured", current);
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
                                    format!("Fell back: {} → {} ({} members)", current, to, count);
                            }
                            Err(e) => {
                                self.status_message =
                                    format!("Failed to load prev camp '{}': {}", prev_name, e);
                            }
                        },
                        None => {
                            self.status_message =
                                format!("Camp '{}' has no previous camp configured", current);
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
                    self.status_message = format!("Camp '{}' started with {} members", name, count);
                }
                Err(_) => {
                    self.status_message = format!(
                        "Unknown camp subcommand or config: '{}'. Try: start|stop|status|list|add|remove|next|prev",
                        name
                    );
                }
            },
        }
    }

    /// Handle `login <subcommand>` from the command bar.
    ///
    /// Subcommands:
    ///   login             — list all configured accounts and status
    ///   login all         — launch all configured accounts
    ///   login G<n>        — launch all accounts in group n
    ///   login <name>      — launch a single account by name
    fn execute_login_command(&mut self, args: &[&str]) {
        let accounts = match &self.accounts_config {
            Some(cfg) => cfg.clone(),
            None => {
                self.status_message =
                    String::from("No accounts config — create config/accounts.toml");
                return;
            }
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
                            format!("No accounts configured for group {}", group_id);
                    } else {
                        self.enqueue_account_launches(&group_accounts);
                    }
                } else {
                    self.status_message = format!("Invalid group: {}", arg);
                }
            }

            // :login <account_name> — launch a single account
            Some(name) => {
                if let Some(entry) = accounts.find_account(name) {
                    self.enqueue_account_launches(std::slice::from_ref(entry));
                } else {
                    self.status_message = format!("Account '{}' not found in config", name);
                }
            }
        }
    }

    /// Handle `track <subcommand>` from the command bar.
    fn execute_track_command(&mut self, args: &[&str]) {
        match args.first().copied() {
            None => {
                self.status_message = String::from("Usage: track <name> | track list");
            }
            Some("list") => {
                if self.tracked_spawns.is_empty() {
                    self.status_message = String::from("No spawns tracked");
                } else {
                    let entries: Vec<String> = self
                        .tracked_spawns
                        .values()
                        .map(|t| format!("{} [{}]", t.name, t.status.label()))
                        .collect();
                    self.status_message = format!("Tracked: {}", entries.join(", "));
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
            let eq_path = std::path::Path::new("C:\\EverQuest");
            // TODO: Read eq_path from AppConfig.launch.eq_path instead of hardcoding
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
                    // TODO: Wire into LaunchCoordinator for staggered launch + state tracking
                    // TODO: After window title shows "[DMFT] EQ - <CharName>", auto-inject DLL
                    // TODO: After DLL injection, auto-form groups + set camp
                }
                Err(e) => {
                    tracing::error!(account = "[redacted]", %e, "Failed to launch EQ client");
                    failed += 1;
                }
            }
        }

        self.status_message = format!(
            "Login: launched {}, failed {} of {} queued",
            launched, failed, count
        );
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

/// Load the disk-based session token for IPC auth.
/// The token was written by --inject-pid (via write_session_token_file) before DLL injection.
fn generate_session_token(pid: u32) -> [u8; 32] {
    let token_path = std::env::temp_dir()
        .join("dmft")
        .join(format!("token_{}.bin", pid));

    if let Ok(data) = std::fs::read(&token_path)
        && data.len() == 32
    {
        let mut token = [0u8; 32];
        token.copy_from_slice(&data);
        return token;
    }

    // Fallback: PID-derived (won't match DLL's random token — will fail auth)
    tracing::warn!(
        pid,
        "No session token file found for TUI — auth will likely fail"
    );
    let pid_bytes = pid.to_le_bytes();
    let mut token = [0u8; 32];
    for (i, byte) in token.iter_mut().enumerate() {
        *byte = pid_bytes[i % 4] ^ (i as u8);
    }
    token
}

/// Extract account number from a character name or window title.
/// Looks for trailing digits (e.g., "frostreaver05" → 5).
pub fn extract_account_number(name: &str) -> Option<u8> {
    let digits: String = name
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return None;
    }
    let digits: String = digits.chars().rev().collect();
    digits.parse().ok()
}

/// Send a slash command to a specific PID via named pipe.
fn send_slash_command(pid: u32, command: &str) -> anyhow::Result<()> {
    use dmft_common::ipc::Command;
    send_ipc_command(
        pid,
        &Command::SlashCommand {
            command: command.to_string(),
        },
    )
}

fn send_ipc_command(pid: u32, cmd: &dmft_common::ipc::Command) -> anyhow::Result<()> {
    use crate::ipc::pipe::CommandPipe;

    let pipe = CommandPipe::connect(pid)?;
    let token = generate_session_token(pid);
    pipe.send_raw_token(&token)?;
    pipe.send_async(cmd)?;
    Ok(())
}
