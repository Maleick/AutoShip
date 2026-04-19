//! Orchestrator dashboard — consolidated operator surface for session, group,
//! navigation, economy, combat, and system visibility.

use std::collections::{HashMap, HashSet, VecDeque};

#[cfg(target_os = "linux")]
use std::fs;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, Wrap},
};

use super::widgets::{Sparkline, panel, render_sparkline, themed_header_row, truncate_inline};
use crate::{
    eq::structs::StandState,
    tui::{
        app::{ActivePanel, App, ClientState},
        theme::Theme,
    },
};

const MAX_HISTORY_SAMPLES: usize = 60;
const MAX_DEATH_LOG: usize = 20;
const MAX_ERROR_LOG: usize = 50;

/// Dashboard sub-tabs shown inside the Orchestrator screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrchestratorTab {
    Session,
    Group,
    Navigation,
    Economy,
    Combat,
    System,
}

impl OrchestratorTab {
    pub const ALL: [Self; 6] = [
        Self::Session,
        Self::Group,
        Self::Navigation,
        Self::Economy,
        Self::Combat,
        Self::System,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Session => "Session",
            Self::Group => "Group",
            Self::Navigation => "Navigation",
            Self::Economy => "Economy",
            Self::Combat => "Combat",
            Self::System => "System",
        }
    }

    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Session => Self::Group,
            Self::Group => Self::Navigation,
            Self::Navigation => Self::Economy,
            Self::Economy => Self::Combat,
            Self::Combat => Self::System,
            Self::System => Self::Session,
        }
    }

    #[must_use]
    pub fn prev(self) -> Self {
        match self {
            Self::Session => Self::System,
            Self::Group => Self::Session,
            Self::Navigation => Self::Group,
            Self::Economy => Self::Navigation,
            Self::Combat => Self::Economy,
            Self::System => Self::Combat,
        }
    }
}

/// A single relay command entry kept in the dashboard history ring.
#[derive(Debug, Clone)]
pub struct RelayCommandEntry {
    pub session_id: String,
    pub command: String,
    pub success: bool,
    pub latency_ms: u64,
}

/// Rolling relay statistics derived from recent relay commands.
#[derive(Debug, Clone, Default)]
pub struct RelayStats {
    pub avg_latency_ms: u64,
    pub success_count: usize,
    pub fail_count: usize,
}

impl RelayStats {
    #[must_use]
    pub fn success_rate(&self) -> f32 {
        let total = self.success_count + self.fail_count;
        if total == 0 {
            100.0
        } else {
            (self.success_count as f32 / total as f32) * 100.0
        }
    }
}

#[derive(Debug, Clone)]
struct DeathLogEntry {
    tick: u64,
    message: String,
}

#[derive(Debug, Clone)]
struct DashboardTelemetry {
    tick_count: u64,
    total_dps: u64,
    total_plat_tenths: u64,
    vendor_aborted: bool,
    clients: Vec<ClientTelemetry>,
}

#[derive(Debug, Clone)]
struct ClientTelemetry {
    pid: u32,
    name: String,
    current_spell: Option<String>,
    is_present: bool,
    is_dead: bool,
    recovery_status: String,
    error: Option<String>,
}

/// Signal feed entry for orchestrator status updates.
#[derive(Debug, Clone)]
pub struct SignalEntry {
    /// ISO timestamp when the signal was generated.
    pub timestamp: String,
    /// Signal type (e.g., "route_complete", "client_death", "economy_update").
    pub kind: String,
    /// Human-readable signal message.
    pub message: String,
}

/// Phase timeline entry for multi-phase operations.
#[derive(Debug, Clone)]
pub struct PhaseEntry {
    /// Timestamp or duration marker for the phase.
    pub time: String,
    /// Phase name (e.g., "Regroup", "Engage", "Loot").
    pub phase: String,
    /// Additional notes about the phase.
    pub note: String,
    /// Whether this phase is currently active.
    pub active: bool,
}

/// Mutable state for the orchestrator dashboard.
#[derive(Debug, Clone)]
pub struct OrchestratorDashboardState {
    pub active_tab: OrchestratorTab,
    pub command_history: VecDeque<RelayCommandEntry>,
    pub total_dps_samples: VecDeque<u64>,
    pub profit_samples: VecDeque<u64>,
    pub spell_usage: HashMap<String, u64>,
    seen_casts: HashMap<u32, String>,
    dead_clients: HashSet<u32>,
    active_errors: HashSet<String>,
    death_log: VecDeque<DeathLogEntry>,
    error_log: VecDeque<String>,
    /// Feed of operational signals (route complete, death, economy event).
    pub signal_feed: Vec<SignalEntry>,
    /// Timeline of multi-phase operation phases.
    pub phase_timeline: Vec<PhaseEntry>,
}

impl OrchestratorDashboardState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_tab: OrchestratorTab::Session,
            command_history: VecDeque::new(),
            total_dps_samples: VecDeque::new(),
            profit_samples: VecDeque::new(),
            spell_usage: HashMap::new(),
            seen_casts: HashMap::new(),
            dead_clients: HashSet::new(),
            active_errors: HashSet::new(),
            death_log: VecDeque::new(),
            error_log: VecDeque::new(),
            signal_feed: Vec::new(),
            phase_timeline: Vec::new(),
        }
    }

    pub fn next_tab(&mut self) {
        self.active_tab = self.active_tab.next();
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = self.active_tab.prev();
    }

    pub fn push_command(&mut self, entry: RelayCommandEntry) {
        if self.command_history.len() >= 10 {
            self.command_history.pop_front();
        }
        self.command_history.push_back(entry);
    }

    #[must_use]
    pub fn relay_stats(&self) -> RelayStats {
        let success_count = self
            .command_history
            .iter()
            .filter(|entry| entry.success)
            .count();
        let fail_count = self.command_history.len().saturating_sub(success_count);
        let avg_latency_ms = if self.command_history.is_empty() {
            0
        } else {
            self.command_history
                .iter()
                .map(|entry| entry.latency_ms)
                .sum::<u64>()
                / self.command_history.len() as u64
        };
        RelayStats {
            avg_latency_ms,
            success_count,
            fail_count,
        }
    }

    fn apply_telemetry(&mut self, telemetry: DashboardTelemetry) {
        push_capped_u64(
            &mut self.total_dps_samples,
            telemetry.total_dps,
            MAX_HISTORY_SAMPLES,
        );
        push_capped_u64(
            &mut self.profit_samples,
            telemetry.total_plat_tenths,
            MAX_HISTORY_SAMPLES,
        );

        let mut current_errors = HashSet::new();
        let mut new_errors = Vec::new();

        for client in telemetry.clients {
            match client.current_spell {
                Some(spell) => {
                    if self.seen_casts.get(&client.pid) != Some(&spell) {
                        *self.spell_usage.entry(spell.clone()).or_insert(0) += 1;
                    }
                    self.seen_casts.insert(client.pid, spell);
                }
                None => {
                    self.seen_casts.remove(&client.pid);
                }
            }

            if client.is_dead {
                if self.dead_clients.insert(client.pid) {
                    self.push_death_log(
                        telemetry.tick_count,
                        format!("{} down • {}", client.name, client.recovery_status),
                    );
                }
            } else if client.is_present && self.dead_clients.remove(&client.pid) {
                self.push_death_log(
                    telemetry.tick_count,
                    format!("{} recovered • {}", client.name, client.recovery_status),
                );
            }

            if let Some(error) = client.error
                && current_errors.insert(error.clone())
                && !self.active_errors.contains(&error)
            {
                new_errors.push(error);
            }
        }

        if telemetry.vendor_aborted {
            let error = String::from("Economy: vendor cycle aborted");
            if current_errors.insert(error.clone()) && !self.active_errors.contains(&error) {
                new_errors.push(error);
            }
        }

        for error in new_errors {
            self.push_error(error);
        }

        self.active_errors = current_errors;
    }

    fn push_death_log(&mut self, tick: u64, message: String) {
        if self.death_log.len() >= MAX_DEATH_LOG {
            self.death_log.pop_front();
        }
        self.death_log.push_back(DeathLogEntry { tick, message });
    }

    fn push_error(&mut self, message: String) {
        if self.error_log.back().is_some_and(|line| line == &message) {
            return;
        }
        if self.error_log.len() >= MAX_ERROR_LOG {
            self.error_log.pop_front();
        }
        self.error_log.push_back(message);
    }

    #[must_use]
    fn top_spells(&self, limit: usize) -> Vec<(&str, u64)> {
        let mut entries: Vec<(&str, u64)> = self
            .spell_usage
            .iter()
            .map(|(spell, count)| (spell.as_str(), *count))
            .collect();
        entries.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
        entries.truncate(limit);
        entries
    }

    #[must_use]
    fn recent_deaths(&self, limit: usize) -> Vec<&DeathLogEntry> {
        self.death_log.iter().rev().take(limit).collect()
    }

    #[must_use]
    fn recent_errors(&self, limit: usize) -> Vec<&String> {
        self.error_log.iter().rev().take(limit).collect()
    }
}

impl Default for OrchestratorDashboardState {
    fn default() -> Self {
        Self::new()
    }
}

/// Draw the orchestrator dashboard inside the body area.
pub fn draw_orchestrator_screen(frame: &mut Frame, area: Rect, app: &mut App) {
    let telemetry = capture_dashboard_telemetry(app);
    app.orchestrator_state.apply_telemetry(telemetry);

    // Main area (left ~101 cols) + Sidebar (right ~44 cols)
    let main_sidebar = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(60), Constraint::Length(44)])
        .split(area);

    let main_area = main_sidebar[0];
    let sidebar_area = main_sidebar[1];

    // Main: Fleet Header (top) + Slots Table (bottom)
    let main_sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(8)])
        .split(main_area);

    draw_fleet_header(frame, main_sections[0], app);
    draw_slots_table(frame, main_sections[1], app);

    // Sidebar: Signal Feed (top) + Phase Timeline (bottom)
    let sidebar_sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Min(8)])
        .split(sidebar_area);

    draw_signal_feed(frame, sidebar_sections[0], app);
    draw_phase_timeline(frame, sidebar_sections[1], app);
}

fn draw_fleet_header(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    let intent = "Execute";
    let phase = "Combat";
    let hz = 10.0;
    let last_tick = 12345u64;

    let lines = vec![
        Line::from(vec![
            Span::styled("Active Intent: ", Style::default().fg(t.text_normal)),
            Span::styled(
                intent,
                Style::default()
                    .fg(t.text_server)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Phase: ", Style::default().fg(t.text_normal)),
            Span::styled(
                phase,
                Style::default()
                    .fg(if phase == "Execute" {
                        t.hp_high
                    } else {
                        t.text_accent
                    })
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Cadence: ", Style::default().fg(t.text_normal)),
            Span::styled(format!("{} Hz", hz), Style::default().fg(t.text_bright)),
            Span::raw(" · "),
            Span::styled(
                format!("tick {}", last_tick),
                Style::default().fg(t.text_muted),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("p ", Style::default().fg(t.text_muted)),
            Span::raw("pause · "),
            Span::styled("r ", Style::default().fg(t.text_muted)),
            Span::raw("resume · "),
            Span::styled("A ", Style::default().fg(t.text_muted)),
            Span::raw("abort intent · "),
            Span::styled("enter ", Style::default().fg(t.text_muted)),
            Span::raw("drill into slot"),
        ]),
    ];

    let content = Paragraph::new(lines).wrap(Wrap { trim: true });
    let block = panel("Fleet Orchestrator", Style::default().fg(t.text_server), t);
    frame.render_widget(content.block(block), area);
}

fn draw_slots_table(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let clients = app.visible_clients();

    let live_count = clients.iter().filter(|c| c.connected).count();
    let configured_count = clients.len();
    let blocked_count = clients.iter().filter(|c| !c.connected).count();

    let title = format!(
        "Slots · {} live · {} configured · {} blocked",
        live_count, configured_count, blocked_count
    );

    let rows = clients.iter().enumerate().map(|(idx, client)| {
        let slot = format!("S{:02}", idx + 1);
        let name = app.client_command_target(client);
        let (state_label, state_color) = client_status_label(app, client);
        let fsm = "—";
        let latency = "—".to_string();
        let lat_style = if latency != "—" && latency.parse::<u64>().unwrap_or(0) > 30 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(t.hp_high)
        };

        let health_bar = render_health_bar(state_label, t);
        let profile = "—";

        Row::new(vec![
            Cell::from(slot),
            Cell::from(name),
            Cell::from(Span::styled(state_label, Style::default().fg(state_color))),
            Cell::from(fsm),
            Cell::from(Span::styled(latency, lat_style)),
            Cell::from(health_bar),
            Cell::from(profile),
        ])
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Length(14),
            Constraint::Length(12),
            Constraint::Length(14),
            Constraint::Length(6),
            Constraint::Length(22),
            Constraint::Min(10),
        ],
    )
    .header(themed_header_row(
        &["Slot", "Name", "State", "FSM", "Lat", "Health", "Profile"],
        t,
    ))
    .block(panel(title, Style::default().fg(t.text_server), t));

    frame.render_widget(table, area);
}

fn signal_kind_color(kind: &str, t: &Theme) -> Color {
    match kind {
        "CH" | "CAST" => t.text_accent,
        "NAV" => t.text_highlight,
        "ALERT" => t.hp_low,
        "XP" => t.hp_high,
        "LOOT" => t.text_server,
        "MED" => t.text_secondary,
        _ => t.text_bright,
    }
}

fn draw_signal_feed(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let visible = area.height.saturating_sub(2) as usize;

    let lines: Vec<Line<'static>> = if app.orchestrator_state.signal_feed.is_empty() {
        vec![Line::from(Span::styled(
            "No signals yet",
            Style::default().fg(t.text_muted),
        ))]
    } else {
        app.orchestrator_state
            .signal_feed
            .iter()
            .rev()
            .take(visible)
            .map(|sig| {
                let color = signal_kind_color(&sig.kind, t);
                Line::from(vec![
                    Span::styled(sig.timestamp.clone(), Style::default().fg(t.text_muted)),
                    Span::raw("  "),
                    Span::styled(
                        format!("{:<5}", sig.kind),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(sig.message.clone(), Style::default().fg(t.text_normal)),
                ])
            })
            .collect()
    };

    let content = Paragraph::new(lines).wrap(Wrap { trim: true });
    let block = panel("Signal Feed", Style::default().fg(t.text_accent), t);
    frame.render_widget(content.block(block), area);
}

fn draw_phase_timeline(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let visible = area.height.saturating_sub(2) as usize;

    let lines: Vec<Line<'static>> = if app.orchestrator_state.phase_timeline.is_empty() {
        vec![Line::from(Span::styled(
            "No phases recorded",
            Style::default().fg(t.text_muted),
        ))]
    } else {
        app.orchestrator_state
            .phase_timeline
            .iter()
            .rev()
            .take(visible)
            .map(|entry| {
                let phase_color = if entry.active {
                    t.hp_high
                } else {
                    t.text_accent
                };
                Line::from(vec![
                    Span::styled(entry.time.clone(), Style::default().fg(t.text_muted)),
                    Span::raw("  "),
                    Span::styled(
                        entry.phase.clone(),
                        Style::default()
                            .fg(phase_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        entry.note.clone(),
                        Style::default().fg(if entry.active {
                            t.text_bright
                        } else {
                            t.text_muted
                        }),
                    ),
                ])
            })
            .collect()
    };

    let content = Paragraph::new(lines).wrap(Wrap { trim: true });
    let block = panel("Phase Timeline", Style::default().fg(t.text_server), t);
    frame.render_widget(content.block(block), area);
}

fn render_health_bar(state: &str, t: &Theme) -> Line<'static> {
    let (filled, empty, label, fill_color) = match state {
        "Online" | "Combat" => (18, 0, "OK", t.hp_high),
        "Paused" | "Stuck" => (11, 7, "RCV", t.text_highlight),
        "Dead" => (2, 16, "BLK", t.hp_low),
        _ => (0, 18, "OFF", t.text_muted),
    };

    let mut spans = Vec::new();
    for _ in 0..filled {
        spans.push(Span::styled("█", Style::default().fg(fill_color)));
    }
    for _ in 0..empty {
        spans.push(Span::styled("·", Style::default().fg(t.text_muted)));
    }
    spans.push(Span::raw(" "));
    spans.push(Span::styled(label, Style::default().fg(fill_color)));

    Line::from(spans)
}

struct DashboardGroup {
    name: String,
    leader: String,
    members: Vec<&'static str>,
}

fn dashboard_groups(app: &App) -> Vec<DashboardGroup> {
    if app.has_live_group_data() {
        let (live_groups, _) = app.build_live_groups();
        return live_groups
            .into_iter()
            .map(|group| DashboardGroup {
                name: group.name(),
                leader: group.leader,
                members: vec![],
            })
            .collect();
    }

    app.groups
        .iter()
        .enumerate()
        .map(|(idx, group)| DashboardGroup {
            name: format!("G{} {}", group.id, group.name),
            leader: app
                .clients_in_group_idx(idx)
                .first()
                .map(|client| app.client_command_target(client))
                .unwrap_or_else(|| String::from("—")),
            members: vec![],
        })
        .filter(|group| !group.members.is_empty())
        .collect()
}

trait LiveGroupName {
    fn name(&self) -> String;
}

impl LiveGroupName for crate::tui::app::LiveGroup {
    fn name(&self) -> String {
        format!("{} ({})", self.leader, self.zone)
    }
}

fn split_main_aside(area: Rect) -> [Rect; 2] {
    let chunks = if area.width < 110 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
            .split(area)
    };
    [chunks[0], chunks[1]]
}

fn session_duration(app: &App) -> String {
    let elapsed = app.session_start.elapsed().as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        elapsed / 3600,
        (elapsed % 3600) / 60,
        elapsed % 60
    )
}

fn current_zone_label(app: &App) -> String {
    app.active_client()
        .map(|client| client.zone_name.clone())
        .unwrap_or_else(|| String::from("Unknown"))
}

fn total_plat(app: &App) -> f64 {
    app.loot_database.total_plat as f64
        + app.loot_database.total_gold as f64 / 10.0
        + app.loot_database.total_silver as f64 / 100.0
        + app.loot_database.total_copper as f64 / 1000.0
}

fn plat_per_hour(app: &App) -> f64 {
    let hours = app.session_start.elapsed().as_secs_f64() / 3600.0;
    if hours > 0.01 {
        total_plat(app) / hours
    } else {
        0.0
    }
}

fn items_per_hour(app: &App) -> f64 {
    let hours = app.session_start.elapsed().as_secs_f64() / 3600.0;
    if hours > 0.01 {
        total_loot_items(app) as f64 / hours
    } else {
        0.0
    }
}

fn total_loot_items(app: &App) -> u32 {
    app.loot_database.items.values().copied().sum()
}

fn format_plat_rate(rate: f64) -> String {
    if rate > 0.0 {
        format!("{rate:.1}")
    } else {
        String::from("—")
    }
}

fn format_countdown(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes:02}:{secs:02}")
    }
}

fn capture_dashboard_telemetry(app: &App) -> DashboardTelemetry {
    let clients = app
        .visible_clients()
        .into_iter()
        .map(|client| ClientTelemetry {
            pid: client.pid,
            name: app.client_command_target(client),
            current_spell: current_spell_name(client),
            is_present: client.local_player.is_some(),
            is_dead: client.local_player.as_ref().is_some_and(|player| {
                matches!(player.stand_state, StandState::Dead) || player.hp_current == 0
            }),
            recovery_status: recovery_status_label(app, client),
            error: client_error_summary(app, client),
        })
        .collect();

    DashboardTelemetry {
        tick_count: app.tick_count,
        total_dps: app
            .visible_clients()
            .iter()
            .map(|client| estimated_client_dps(client, app))
            .sum(),
        total_plat_tenths: (total_plat(app) * 10.0).round() as u64,
        vendor_aborted: matches!(
            app.economy_state.vendor_status,
            crate::tui::ui::economy_controls::VendorCycleStatus::Aborted
        ),
        clients,
    }
}

fn push_capped_u64(buffer: &mut VecDeque<u64>, value: u64, cap: usize) {
    if buffer.len() >= cap {
        buffer.pop_front();
    }
    buffer.push_back(value);
}

fn current_spell_name(client: &ClientState) -> Option<String> {
    client
        .local_player
        .as_ref()
        .and_then(|player| player.cast_state.as_ref())
        .filter(|cast| cast.is_casting())
        .map(|cast| {
            cast.spell_name
                .clone()
                .unwrap_or_else(|| format!("Spell {}", cast.spell_id))
        })
}

fn estimated_client_dps(client: &ClientState, app: &App) -> u64 {
    let Some(player) = client.local_player.as_ref() else {
        return 0;
    };
    if matches!(player.stand_state, StandState::Dead | StandState::Sitting) {
        return 0;
    }

    let in_combat = client.target.is_some()
        || player
            .cast_state
            .as_ref()
            .is_some_and(|cast| cast.is_casting())
        || (client.is_demo && (app.tick_count + u64::from(client.pid)) % 6 < 3);

    if !in_combat {
        return 0;
    }

    let class = player.class_str();
    let class_factor = match class.as_str() {
        "WAR" | "MNK" | "ROG" | "BER" => 5,
        "WIZ" | "MAG" | "NEC" => 6,
        "RNG" | "SHD" | "PAL" | "BST" => 4,
        "CLR" | "DRU" | "SHM" | "ENC" | "BRD" => 3,
        _ => 4,
    };
    let mut total = u64::from(player.level).saturating_mul(class_factor);
    if player
        .cast_state
        .as_ref()
        .is_some_and(|cast| cast.is_casting())
    {
        total += 90;
    }
    total
}

fn client_status_label(app: &App, client: &ClientState) -> (&'static str, Color) {
    let t = &app.theme;
    if client.local_player.is_none() {
        return ("Offline", t.hp_low);
    }
    if client
        .local_player
        .as_ref()
        .is_some_and(|player| matches!(player.stand_state, StandState::Dead))
    {
        return ("Dead", t.hp_low);
    }
    if app.automation_paused {
        return ("Paused", t.text_highlight);
    }
    if app
        .nav_state
        .nav_statuses
        .get(&client.pid)
        .is_some_and(|status| status.status.is_stuck())
        || app
            .zone_status_state
            .zone_statuses
            .get(&client.pid)
            .is_some_and(|status| status.stuck)
    {
        return ("Stuck", t.hp_low);
    }
    if estimated_client_dps(client, app) > 0 {
        return ("Combat", t.text_highlight);
    }
    ("Online", t.hp_high)
}

fn client_resource_summary(client: &ClientState) -> (String, String, String) {
    let Some(player) = client.local_player.as_ref() else {
        return (String::from("—"), String::from("—"), String::from("—"));
    };
    let endurance_pct = if player.endurance_max > 0 {
        (f64::from(player.endurance_current.max(0)) / f64::from(player.endurance_max)) * 100.0
    } else {
        100.0
    };
    (
        format!("{:.0}%", player.mana_pct()),
        format!("{:.0}%", endurance_pct),
        player.level.to_string(),
    )
}

fn client_location_label(app: &App, client: &ClientState) -> String {
    if let Some(status) = app.nav_state.nav_statuses.get(&client.pid) {
        return status.route_state.clone();
    }
    client.local_player.as_ref().map_or_else(
        || String::from("Waiting"),
        |player| format!("{:.0},{:.0}", player.x, player.y),
    )
}

fn recovery_status_label(app: &App, client: &ClientState) -> String {
    if let Some(zone) = app.zone_status_state.zone_statuses.get(&client.pid)
        && zone.stuck
    {
        return format!("{} recovery", zone.fsm_state.label());
    }
    if let Some(nav) = app.nav_state.nav_statuses.get(&client.pid)
        && let Some(recovery) = &nav.recovery_state
    {
        return recovery.clone();
    }
    String::from("no recovery")
}

fn client_error_summary(app: &App, client: &ClientState) -> Option<String> {
    if client.local_player.is_none() {
        return Some(format!("{}: offline", app.client_command_target(client)));
    }
    if let Some(zone) = app.zone_status_state.zone_statuses.get(&client.pid) {
        if zone.stuck {
            return Some(format!(
                "{}: zone {} stuck",
                app.client_command_target(client),
                zone.fsm_state.label()
            ));
        }
        if zone.timeout_secs == Some(0) {
            return Some(format!(
                "{}: zone timeout",
                app.client_command_target(client)
            ));
        }
    }
    if let Some(nav) = app.nav_state.nav_statuses.get(&client.pid) {
        if let Some(reason) = &nav.failure_reason {
            return Some(format!("{}: {}", app.client_command_target(client), reason));
        }
        if let Some(blocker) = nav.blocker_summary() {
            return Some(format!(
                "{}: {}",
                app.client_command_target(client),
                blocker
            ));
        }
    }
    None
}

fn role_label(client: &ClientState) -> &'static str {
    let class = client
        .local_player
        .as_ref()
        .map(|player| player.class_str())
        .unwrap_or_else(|| String::from("UNK"));
    match class.as_str() {
        "WAR" | "PAL" | "SHD" => "Tank",
        "CLR" | "DRU" | "SHM" => "Healer",
        "ENC" | "BRD" => "Control",
        "WIZ" | "MAG" | "NEC" | "MNK" | "ROG" | "BER" | "RNG" | "BST" => "DPS",
        _ => "Support",
    }
}

fn group_formation_label(members: &[&ClientState]) -> (String, Color) {
    if members.is_empty() {
        return (String::from("No members"), Color::DarkGray);
    }

    let zones: HashSet<&str> = members
        .iter()
        .map(|client| client.zone_name.as_str())
        .collect();
    if zones.len() > 1 {
        return (format!("Split ({} zones)", zones.len()), Color::Yellow);
    }

    let positions: Vec<(f32, f32)> = members
        .iter()
        .filter_map(|client| {
            client
                .local_player
                .as_ref()
                .map(|player| (player.x, player.y))
        })
        .collect();
    if positions.len() < 2 {
        return (String::from("Solo"), Color::Green);
    }

    let (min_x, max_x) = positions.iter().fold(
        (f32::INFINITY, f32::NEG_INFINITY),
        |(min_v, max_v), (x, _)| (min_v.min(*x), max_v.max(*x)),
    );
    let (min_y, max_y) = positions.iter().fold(
        (f32::INFINITY, f32::NEG_INFINITY),
        |(min_v, max_v), (_, y)| (min_v.min(*y), max_v.max(*y)),
    );
    let spread = (max_x - min_x).abs().max((max_y - min_y).abs());
    if spread <= 40.0 {
        (String::from("All together"), Color::Green)
    } else if spread <= 120.0 {
        (format!("Loose ({spread:.0}u)"), Color::Yellow)
    } else {
        (format!("Spread ({spread:.0}u)"), Color::Red)
    }
}

fn group_spell_sync_label(members: &[&ClientState]) -> (String, Color) {
    let ready = members
        .iter()
        .filter_map(|client| client.local_player.as_ref())
        .filter(|player| {
            player.mana_pct() >= 40.0 && !matches!(player.stand_state, StandState::Dead)
        })
        .count();
    let total = members.len();
    let color = if ready == total {
        Color::Green
    } else if ready * 2 >= total {
        Color::Yellow
    } else {
        Color::Red
    };
    (format!("{ready}/{total} ready"), color)
}

fn nav_status_color(status: Option<&crate::tui::app::NavClientStatus>, t: &Theme) -> Color {
    match status.map(|status| &status.status) {
        Some(nav) if nav.is_stuck() => t.hp_low,
        Some(nav) if nav.is_paused() => t.text_secondary,
        Some(nav) if nav.is_arrived() => t.hp_high,
        Some(nav) if nav.is_moving() => t.text_highlight,
        _ => t.text_muted,
    }
}

fn zone_fsm_color(
    status: Option<&crate::tui::ui::zone_status_panel::ZoneClientStatus>,
    t: &Theme,
) -> Color {
    match status {
        Some(status) if status.timeout_secs == Some(0) => t.hp_low,
        Some(status) if status.stuck => t.text_highlight,
        Some(status) => match status.fsm_state {
            crate::tui::ui::zone_status_panel::ZoneFsmState::Walking => t.hp_high,
            crate::tui::ui::zone_status_panel::ZoneFsmState::Zoning => t.text_accent,
            crate::tui::ui::zone_status_panel::ZoneFsmState::Recovering => t.text_secondary,
            crate::tui::ui::zone_status_panel::ZoneFsmState::Idle => t.text_muted,
        },
        None => t.text_muted,
    }
}

fn vendor_status_color(app: &App, t: &Theme) -> Color {
    match app.economy_state.vendor_status {
        crate::tui::ui::economy_controls::VendorCycleStatus::Active => t.hp_high,
        crate::tui::ui::economy_controls::VendorCycleStatus::Paused => t.text_highlight,
        crate::tui::ui::economy_controls::VendorCycleStatus::Idle => t.text_muted,
        crate::tui::ui::economy_controls::VendorCycleStatus::Aborted => t.hp_low,
    }
}

fn wishlist_lines(app: &App, width: usize, t: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if !app.economy_state.loot_recent_items.is_empty() {
        for item in app.economy_state.loot_recent_items.iter().take(6) {
            lines.push(Line::from(vec![
                Span::styled("• ", Style::default().fg(t.text_highlight)),
                Span::styled(
                    truncate_inline(item, width.saturating_sub(6)),
                    Style::default().fg(t.text_normal),
                ),
            ]));
        }
        return lines;
    }

    let mut top_items: Vec<(&String, &u32)> = app.loot_database.items.iter().collect();
    top_items.sort_by(|left, right| right.1.cmp(left.1).then_with(|| left.0.cmp(right.0)));
    if top_items.is_empty() {
        return vec![Line::from(Span::styled(
            "No wishlist or loot-watch hits yet",
            Style::default().fg(t.text_muted),
        ))];
    }

    lines.push(Line::from(Span::styled(
        "Recent high-value candidates",
        Style::default().fg(t.text_secondary),
    )));
    for (name, count) in top_items.into_iter().take(6) {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{count:>2}× "),
                Style::default().fg(t.text_highlight),
            ),
            Span::styled(
                truncate_inline(name, width.saturating_sub(8)),
                Style::default().fg(t.text_normal),
            ),
        ]));
    }
    lines
}

fn process_memory_label(pid: u32) -> String {
    process_memory_mb(pid)
        .map(|mb| format!("{mb} MB"))
        .unwrap_or_else(|| String::from("—"))
}

fn process_memory_mb(pid: u32) -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let path = format!("/proc/{pid}/status");
        let content = fs::read_to_string(path).ok()?;
        let line = content.lines().find(|line| line.starts_with("VmRSS:"))?;
        let kb = line
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<u64>().ok())?;
        Some(kb / 1024)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        None
    }
}

fn refresh_age_label(at: Option<std::time::Instant>) -> String {
    at.map(|instant| format!("{}s", instant.elapsed().as_secs()))
        .unwrap_or_else(|| String::from("—"))
}

fn latency_percentiles(latencies: &[u64]) -> (u64, u64, u64) {
    if latencies.is_empty() {
        return (0, 0, 0);
    }
    let mut sorted = latencies.to_vec();
    sorted.sort_unstable();
    (
        percentile(&sorted, 50),
        percentile(&sorted, 95),
        percentile(&sorted, 99),
    )
}

fn percentile(sorted: &[u64], pct: usize) -> u64 {
    let idx = ((sorted.len().saturating_sub(1)) * pct) / 100;
    sorted[idx.min(sorted.len().saturating_sub(1))]
}

fn system_health_label(app: &App) -> (String, Color) {
    let visible = app.visible_clients();
    let offline = visible
        .iter()
        .filter(|client| client.local_player.is_none())
        .count();
    let stuck = visible
        .iter()
        .filter(|client| {
            app.nav_state
                .nav_statuses
                .get(&client.pid)
                .is_some_and(|status| status.status.is_stuck())
                || app
                    .zone_status_state
                    .zone_statuses
                    .get(&client.pid)
                    .is_some_and(|status| status.stuck)
        })
        .count();

    if offline == 0 && stuck == 0 && !app.automation_paused {
        (String::from("Healthy"), app.theme.hp_high)
    } else if offline <= 1 && stuck <= 1 {
        (
            format!("Degraded ({offline} off / {stuck} stuck)"),
            app.theme.text_highlight,
        )
    } else {
        (
            format!("Critical ({offline} off / {stuck} stuck)"),
            app.theme.hp_low,
        )
    }
}

fn max_sample<I>(values: I) -> f64
where
    I: Iterator<Item = u64>,
{
    values.max().unwrap_or(1) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_state_starts_on_session_tab() {
        let state = OrchestratorDashboardState::new();
        assert_eq!(state.active_tab, OrchestratorTab::Session);
        assert!(state.command_history.is_empty());
    }

    #[test]
    fn dashboard_state_tab_cycle_wraps() {
        let mut state = OrchestratorDashboardState::new();
        state.prev_tab();
        assert_eq!(state.active_tab, OrchestratorTab::System);
        state.next_tab();
        assert_eq!(state.active_tab, OrchestratorTab::Session);
    }

    #[test]
    fn relay_stats_are_derived_from_history() {
        let mut state = OrchestratorDashboardState::new();
        state.push_command(RelayCommandEntry {
            session_id: String::from("fleet"),
            command: String::from("engage"),
            success: true,
            latency_ms: 40,
        });
        state.push_command(RelayCommandEntry {
            session_id: String::from("fleet"),
            command: String::from("loot"),
            success: false,
            latency_ms: 120,
        });
        let stats = state.relay_stats();
        assert_eq!(stats.success_count, 1);
        assert_eq!(stats.fail_count, 1);
        assert_eq!(stats.avg_latency_ms, 80);
    }

    #[test]
    fn telemetry_only_logs_new_errors_until_state_changes() {
        let mut state = OrchestratorDashboardState::new();
        let telemetry = DashboardTelemetry {
            tick_count: 1,
            total_dps: 0,
            total_plat_tenths: 0,
            vendor_aborted: false,
            clients: vec![
                ClientTelemetry {
                    pid: 1,
                    name: String::from("Alpha"),
                    current_spell: None,
                    is_present: true,
                    is_dead: false,
                    recovery_status: String::from("no recovery"),
                    error: Some(String::from("Alpha: offline")),
                },
                ClientTelemetry {
                    pid: 2,
                    name: String::from("Beta"),
                    current_spell: None,
                    is_present: true,
                    is_dead: false,
                    recovery_status: String::from("no recovery"),
                    error: Some(String::from("Beta: zone stuck")),
                },
            ],
        };

        state.apply_telemetry(telemetry.clone());
        assert_eq!(state.recent_errors(10).len(), 2);

        state.apply_telemetry(telemetry);
        assert_eq!(state.recent_errors(10).len(), 2);
    }

    #[test]
    fn telemetry_does_not_log_recovery_when_dead_client_disappears() {
        let mut state = OrchestratorDashboardState::new();
        state.apply_telemetry(DashboardTelemetry {
            tick_count: 1,
            total_dps: 0,
            total_plat_tenths: 0,
            vendor_aborted: false,
            clients: vec![ClientTelemetry {
                pid: 7,
                name: String::from("Cleric"),
                current_spell: None,
                is_present: true,
                is_dead: true,
                recovery_status: String::from("corpse recovery"),
                error: None,
            }],
        });
        state.apply_telemetry(DashboardTelemetry {
            tick_count: 2,
            total_dps: 0,
            total_plat_tenths: 0,
            vendor_aborted: false,
            clients: vec![ClientTelemetry {
                pid: 7,
                name: String::from("Cleric"),
                current_spell: None,
                is_present: false,
                is_dead: false,
                recovery_status: String::from("awaiting reconnect"),
                error: Some(String::from("Cleric: offline")),
            }],
        });
        assert_eq!(state.recent_deaths(10).len(), 1);
        assert!(state.recent_deaths(10)[0].message.contains("down"));

        state.apply_telemetry(DashboardTelemetry {
            tick_count: 3,
            total_dps: 0,
            total_plat_tenths: 0,
            vendor_aborted: false,
            clients: vec![ClientTelemetry {
                pid: 7,
                name: String::from("Cleric"),
                current_spell: None,
                is_present: true,
                is_dead: false,
                recovery_status: String::from("back in zone"),
                error: None,
            }],
        });
        assert_eq!(state.recent_deaths(10).len(), 2);
        assert!(state.recent_deaths(10)[0].message.contains("recovered"));
    }

    #[test]
    fn latency_percentiles_handle_empty_input() {
        assert_eq!(latency_percentiles(&[]), (0, 0, 0));
    }
}
