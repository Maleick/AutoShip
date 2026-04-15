//! Orchestrator dashboard — consolidated operator surface for session, group,
//! navigation, economy, combat, and system visibility.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
};

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

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(4),
        ])
        .split(area);

    draw_dashboard_header(frame, sections[0], app);

    match app.orchestrator_state.active_tab {
        OrchestratorTab::Session => draw_session_tab(frame, sections[1], app),
        OrchestratorTab::Group => draw_group_tab(frame, sections[1], app),
        OrchestratorTab::Navigation => draw_navigation_tab(frame, sections[1], app),
        OrchestratorTab::Economy => draw_economy_tab(frame, sections[1], app),
        OrchestratorTab::Combat => draw_combat_tab(frame, sections[1], app),
        OrchestratorTab::System => draw_system_tab(frame, sections[1], app),
    }

    draw_dashboard_footer(frame, sections[2], app);
}

fn draw_dashboard_header(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let title = format!(
        " Operator Dashboard | {} | Zone {} | Time {} | Plat/Hr {} ",
        app.group_focus_label(),
        current_zone_label(app),
        session_duration(app),
        format_plat_rate(plat_per_hour(app)),
    );
    let tabs = Line::from(
        OrchestratorTab::ALL
            .iter()
            .enumerate()
            .flat_map(|(index, tab)| {
                let style = if *tab == app.orchestrator_state.active_tab {
                    t.tab_active
                } else {
                    t.tab_inactive
                };
                let mut spans = Vec::new();
                if index > 0 {
                    spans.push(Span::raw(" "));
                }
                spans.push(Span::styled(format!(" {} ", tab.label()), style));
                spans
            })
            .collect::<Vec<_>>(),
    );

    frame.render_widget(
        Paragraph::new(tabs)
            .block(panel(title, t.border_active, t))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_dashboard_footer(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let base = Line::from(vec![
        Span::styled("[←/→]", Style::default().fg(t.text_highlight)),
        Span::styled(" tabs  ", Style::default().fg(t.text_muted)),
        Span::styled("[↑/↓]", Style::default().fg(t.text_highlight)),
        Span::styled(" client  ", Style::default().fg(t.text_muted)),
        Span::styled("[Space]", Style::default().fg(t.text_highlight)),
        Span::styled(" pause/resume  ", Style::default().fg(t.text_muted)),
        Span::styled("[X]", Style::default().fg(t.hp_low)),
        Span::styled(" terminate", Style::default().fg(t.text_muted)),
    ]);

    let detail = match app.orchestrator_state.active_tab {
        OrchestratorTab::Session => Line::from(vec![
            Span::styled("[E]", Style::default().fg(t.text_highlight)),
            Span::styled(" engage  ", Style::default().fg(t.text_muted)),
            Span::styled("[D]", Style::default().fg(t.text_highlight)),
            Span::styled(" disengage", Style::default().fg(t.text_muted)),
        ]),
        OrchestratorTab::Group => Line::from(vec![
            Span::styled("[E]", Style::default().fg(t.text_highlight)),
            Span::styled(" pull  ", Style::default().fg(t.text_muted)),
            Span::styled("[C]", Style::default().fg(t.text_highlight)),
            Span::styled(" camp status  ", Style::default().fg(t.text_muted)),
            Span::styled("[N]", Style::default().fg(t.text_highlight)),
            Span::styled(" nav ui", Style::default().fg(t.text_muted)),
        ]),
        OrchestratorTab::Navigation => Line::from(vec![
            Span::styled("[N]", Style::default().fg(t.text_highlight)),
            Span::styled(" nav ui  ", Style::default().fg(t.text_muted)),
            Span::styled("[C]", Style::default().fg(t.text_highlight)),
            Span::styled(" camp status", Style::default().fg(t.text_muted)),
        ]),
        OrchestratorTab::Economy => Line::from(vec![
            Span::styled("[6]", Style::default().fg(t.text_highlight)),
            Span::styled(
                " full economy controls  ",
                Style::default().fg(t.text_muted),
            ),
            Span::styled("[C]", Style::default().fg(t.text_highlight)),
            Span::styled(" camp status", Style::default().fg(t.text_muted)),
        ]),
        OrchestratorTab::Combat => Line::from(vec![
            Span::styled("[E]", Style::default().fg(t.text_highlight)),
            Span::styled(" engage  ", Style::default().fg(t.text_muted)),
            Span::styled("[D]", Style::default().fg(t.text_highlight)),
            Span::styled(" disengage", Style::default().fg(t.text_muted)),
        ]),
        OrchestratorTab::System => Line::from(vec![
            Span::styled("[N]", Style::default().fg(t.text_highlight)),
            Span::styled(" nav ui  ", Style::default().fg(t.text_muted)),
            Span::styled("[C]", Style::default().fg(t.text_highlight)),
            Span::styled(" camp status", Style::default().fg(t.text_muted)),
        ]),
    };

    frame.render_widget(
        Paragraph::new(vec![base, detail])
            .block(panel(" Dashboard Shortcuts ", t.border_dim, t))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_session_tab(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = split_main_aside(area);
    draw_session_table(frame, chunks[0], app);
    draw_session_sidebar(frame, chunks[1], app);
}

fn draw_session_table(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let visible = app.visible_clients();
    let selected_pid = app.active_client().map(|client| client.pid);
    let header = themed_header_row(
        &[
            "", "Client", "Status", "Zone", "Lv", "Mana", "End", "DPS*", "Location",
        ],
        t,
    );

    let rows: Vec<Row<'_>> = visible
        .iter()
        .map(|client| {
            let is_selected = Some(client.pid) == selected_pid;
            let marker = if is_selected { "▶" } else { " " };
            let (status_label, status_color) = client_status_label(app, client);
            let (mana_label, end_label, level) = client_resource_summary(client);
            let dps = estimated_client_dps(client, app);
            let row_style = if is_selected {
                Style::default()
                    .bg(t.row_selected_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(marker).style(Style::default().fg(t.text_accent)),
                Cell::from(app.client_command_target(client))
                    .style(Style::default().fg(t.text_normal)),
                Cell::from(status_label).style(Style::default().fg(status_color)),
                Cell::from(client.zone_name.as_str()).style(Style::default().fg(t.text_secondary)),
                Cell::from(level).style(Style::default().fg(t.text_normal)),
                Cell::from(mana_label).style(Style::default().fg(t.mana_color)),
                Cell::from(end_label).style(Style::default().fg(t.text_highlight)),
                Cell::from(dps.to_string()).style(Style::default().fg(t.text_accent)),
                Cell::from(client_location_label(app, client))
                    .style(Style::default().fg(t.text_muted)),
            ])
            .style(row_style)
        })
        .collect();

    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Min(12),
                Constraint::Length(8),
                Constraint::Min(12),
                Constraint::Length(4),
                Constraint::Length(6),
                Constraint::Length(6),
                Constraint::Length(5),
                Constraint::Min(16),
            ],
        )
        .header(header)
        .block(panel(
            " Session Fleet ",
            if app.is_panel_focused(ActivePanel::OrchestratorDashboard) {
                t.border_active
            } else {
                t.border_primary
            },
            t,
        )),
        area,
    );
}

fn draw_session_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let stats = app.orchestrator_state.relay_stats();
    let total_dps = app
        .visible_clients()
        .iter()
        .map(|client| estimated_client_dps(client, app))
        .sum::<u64>();

    let lines = if let Some(client) = app.active_client() {
        let selected = app.client_command_target(client);
        let target = client
            .target
            .as_ref()
            .map(|spawn| app.redact_name(&spawn.displayed_name).into_owned())
            .unwrap_or_else(|| String::from("No target"));
        let formation = if let Some(player) = &client.local_player {
            format!("({:.0}, {:.0}, {:.0})", player.x, player.y, player.z)
        } else {
            String::from("Offline")
        };
        vec![
            Line::from(vec![
                Span::styled("Selected ", Style::default().fg(t.text_muted)),
                Span::styled(selected, Style::default().fg(t.text_normal)),
            ]),
            Line::from(vec![
                Span::styled("Target   ", Style::default().fg(t.text_muted)),
                Span::styled(target, Style::default().fg(t.text_highlight)),
            ]),
            Line::from(vec![
                Span::styled("Group    ", Style::default().fg(t.text_muted)),
                Span::styled(
                    app.client_group_label(client).unwrap_or("—"),
                    Style::default().fg(t.text_accent),
                ),
            ]),
            Line::from(vec![
                Span::styled("Pos      ", Style::default().fg(t.text_muted)),
                Span::styled(formation, Style::default().fg(t.text_secondary)),
            ]),
            Line::from(vec![
                Span::styled("Fleet DPS ", Style::default().fg(t.text_muted)),
                Span::styled(total_dps.to_string(), Style::default().fg(t.text_accent)),
            ]),
            Line::from(vec![
                Span::styled("IPC avg  ", Style::default().fg(t.text_muted)),
                Span::styled(
                    format!(
                        "{}ms ({:.0}% ok)",
                        stats.avg_latency_ms,
                        stats.success_rate()
                    ),
                    Style::default().fg(t.text_secondary),
                ),
            ]),
            Line::from(Span::styled(
                "Space pause/resume • X terminate",
                Style::default().fg(t.text_muted),
            )),
        ]
    } else {
        vec![Line::from(Span::styled(
            "No client selected",
            Style::default().fg(t.text_muted),
        ))]
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(" Session Detail ", t.border_primary, t))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_group_tab(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = split_main_aside(area);
    draw_group_table(frame, chunks[0], app);
    draw_group_sidebar(frame, chunks[1], app);
}

fn draw_group_table(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let header = themed_header_row(
        &["Group", "Leader", "Members", "Formation", "Spell Sync"],
        t,
    );
    let rows: Vec<Row<'_>> = dashboard_groups(app)
        .into_iter()
        .map(|group| {
            let formation = group_formation_label(&group.members);
            let spell_sync = group_spell_sync_label(&group.members);
            Row::new(vec![
                Cell::from(group.name).style(Style::default().fg(t.text_accent)),
                Cell::from(group.leader).style(Style::default().fg(t.text_normal)),
                Cell::from(group.members.len().to_string())
                    .style(Style::default().fg(t.text_normal)),
                Cell::from(formation.0).style(Style::default().fg(formation.1)),
                Cell::from(spell_sync.0).style(Style::default().fg(spell_sync.1)),
            ])
        })
        .collect();

    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Min(12),
                Constraint::Length(7),
                Constraint::Min(14),
                Constraint::Min(14),
            ],
        )
        .header(header)
        .block(panel(" Group Readiness ", t.border_primary, t)),
        area,
    );
}

fn draw_group_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Mode ", Style::default().fg(t.text_muted)),
            Span::styled(
                app.operating_mode.to_string(),
                Style::default().fg(t.text_highlight),
            ),
        ]),
        Line::from(vec![
            Span::styled("Main Assist ", Style::default().fg(t.text_muted)),
            Span::styled(
                app.main_assist.as_deref().unwrap_or("—"),
                Style::default().fg(t.text_normal),
            ),
        ]),
        Line::from(vec![
            Span::styled("Main Tank   ", Style::default().fg(t.text_muted)),
            Span::styled(
                app.main_tank.as_deref().unwrap_or("—"),
                Style::default().fg(t.text_normal),
            ),
        ]),
    ];

    if let Some(status) = &app.ch_chain_status {
        lines.push(Line::from(vec![
            Span::styled("Spell Sync ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!(
                    "{} clerics @ {:.1}s {}",
                    status.members,
                    status.interval_secs,
                    if status.is_adaptive {
                        "adaptive"
                    } else {
                        "fixed"
                    }
                ),
                Style::default().fg(t.text_accent),
            ),
        ]));
    }

    lines.push(Line::from(Span::styled(
        "Role snapshot",
        Style::default()
            .fg(t.text_secondary)
            .add_modifier(Modifier::BOLD),
    )));

    if let Some(client) = app.active_client() {
        lines.push(Line::from(vec![
            Span::styled(
                app.client_command_target(client),
                Style::default().fg(t.text_normal),
            ),
            Span::styled(" → ", Style::default().fg(t.text_muted)),
            Span::styled(role_label(client), Style::default().fg(t.text_highlight)),
        ]));
    }

    lines.push(Line::from(Span::styled(
        "E pull • C camp status • N nav ui",
        Style::default().fg(t.text_muted),
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(" Group Controls ", t.border_primary, t))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_navigation_tab(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = split_main_aside(area);
    draw_navigation_table(frame, chunks[0], app);
    draw_navigation_sidebar(frame, chunks[1], app);
}

fn draw_navigation_table(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let selected_pid = app.active_client().map(|client| client.pid);
    let header = themed_header_row(
        &["", "Client", "Route", "Status", "Zone FSM", "Recovery"],
        t,
    );

    let rows: Vec<Row<'_>> = app
        .visible_clients()
        .iter()
        .map(|client| {
            let is_selected = Some(client.pid) == selected_pid;
            let marker = if is_selected { "▶" } else { " " };
            let nav = app.nav_state.nav_statuses.get(&client.pid);
            let zone = app.zone_status_state.zone_statuses.get(&client.pid);
            let route = nav
                .map(|status| status.destination.as_str())
                .unwrap_or("No route");
            let status = nav.map(|status| status.status.label()).unwrap_or("Idle");
            let recovery = nav
                .and_then(|status| status.recovery_state.as_deref())
                .unwrap_or("—");
            let row_style = if is_selected {
                Style::default()
                    .bg(t.row_selected_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(marker).style(Style::default().fg(t.text_accent)),
                Cell::from(app.client_command_target(client))
                    .style(Style::default().fg(t.text_normal)),
                Cell::from(route).style(Style::default().fg(t.text_highlight)),
                Cell::from(status).style(Style::default().fg(nav_status_color(nav, t))),
                Cell::from(zone.map_or("Idle", |status| status.fsm_state.label()))
                    .style(Style::default().fg(zone_fsm_color(zone, t))),
                Cell::from(recovery).style(Style::default().fg(t.text_secondary)),
            ])
            .style(row_style)
        })
        .collect();

    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Min(12),
                Constraint::Min(14),
                Constraint::Length(10),
                Constraint::Length(11),
                Constraint::Min(14),
            ],
        )
        .header(header)
        .block(panel(" Navigation Fleet ", t.border_primary, t)),
        area,
    );
}

fn draw_navigation_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let lines = if let Some(client) = app.active_client() {
        let nav = app.nav_state.nav_statuses.get(&client.pid);
        let zone = app.zone_status_state.zone_statuses.get(&client.pid);
        vec![
            Line::from(vec![
                Span::styled("Route ", Style::default().fg(t.text_muted)),
                Span::styled(
                    nav.map(|status| status.route_state.as_str())
                        .unwrap_or("Standing by"),
                    Style::default().fg(t.text_highlight),
                ),
            ]),
            Line::from(vec![
                Span::styled("Waypoint ", Style::default().fg(t.text_muted)),
                Span::styled(
                    nav.map(|status| status.progress_summary())
                        .unwrap_or_else(|| String::from("No active path")),
                    Style::default().fg(t.text_secondary),
                ),
            ]),
            Line::from(vec![
                Span::styled("Blockers ", Style::default().fg(t.text_muted)),
                Span::styled(
                    nav.and_then(|status| status.blocker_summary())
                        .unwrap_or_else(|| String::from("None")),
                    Style::default().fg(t.hp_low),
                ),
            ]),
            Line::from(vec![
                Span::styled("Zone FSM ", Style::default().fg(t.text_muted)),
                Span::styled(
                    zone.map_or("Idle", |status| status.fsm_state.label()),
                    Style::default().fg(zone_fsm_color(zone, t)),
                ),
            ]),
            Line::from(vec![
                Span::styled("Timeout ", Style::default().fg(t.text_muted)),
                Span::styled(
                    zone.and_then(|status| status.timeout_secs)
                        .map(|secs| format!("{secs}s"))
                        .unwrap_or_else(|| String::from("—")),
                    Style::default().fg(t.text_secondary),
                ),
            ]),
            Line::from(Span::styled(
                "N nav ui • C camp status",
                Style::default().fg(t.text_muted),
            )),
        ]
    } else {
        vec![Line::from(Span::styled(
            "No client selected",
            Style::default().fg(t.text_muted),
        ))]
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(" Route Detail ", t.border_primary, t))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_economy_tab(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = split_main_aside(area);
    draw_economy_metrics(frame, chunks[0], app);
    draw_economy_sidebar(frame, chunks[1], app);
}

fn draw_economy_metrics(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(8)])
        .split(area);

    let vendor_lines = vec![
        Line::from(vec![
            Span::styled("Loot/hr ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{:.1}", items_per_hour(app)),
                Style::default().fg(t.text_highlight),
            ),
            Span::styled("  Plat/hr ", Style::default().fg(t.text_muted)),
            Span::styled(
                format_plat_rate(plat_per_hour(app)),
                Style::default().fg(t.text_accent),
            ),
        ]),
        Line::from(vec![
            Span::styled("Vendor ", Style::default().fg(t.text_muted)),
            Span::styled(
                app.economy_state.vendor_status.label(),
                Style::default().fg(vendor_status_color(app, t)),
            ),
            Span::styled("  Next ", Style::default().fg(t.text_muted)),
            Span::styled(
                format_countdown(app.economy_state.vendor_next_cycle_secs),
                Style::default().fg(t.text_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("Last sell ", Style::default().fg(t.text_muted)),
            Span::styled(
                app.economy_state.vendor_last_zone.as_deref().unwrap_or("—"),
                Style::default().fg(t.text_normal),
            ),
            Span::styled("  Banking ", Style::default().fg(t.text_muted)),
            Span::styled(
                app.economy_state.banking_status.label(),
                Style::default().fg(t.text_highlight),
            ),
        ]),
        Line::from(vec![
            Span::styled("Trend ", Style::default().fg(t.text_muted)),
            render_sparkline(
                &Sparkline::new(
                    app.orchestrator_state
                        .profit_samples
                        .iter()
                        .map(|value| *value as f64 / 10.0)
                        .collect(),
                )
                .with_max(max_sample(app.orchestrator_state.profit_samples.iter().copied()) / 10.0),
                t.text_accent,
            ),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(vendor_lines)
            .block(panel(" Economy Metrics ", t.border_primary, t))
            .wrap(Wrap { trim: true }),
        inner[0],
    );

    let wishlist_lines = wishlist_lines(app, inner[1].width as usize, t);
    frame.render_widget(
        Paragraph::new(wishlist_lines)
            .block(panel(" Wishlist / Loot Watch ", t.border_primary, t))
            .wrap(Wrap { trim: true }),
        inner[1],
    );
}

fn draw_economy_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let lines = vec![
        Line::from(vec![
            Span::styled("Items looted ", Style::default().fg(t.text_muted)),
            Span::styled(
                total_loot_items(app).to_string(),
                Style::default().fg(t.text_highlight),
            ),
        ]),
        Line::from(vec![
            Span::styled("Queue ", Style::default().fg(t.text_muted)),
            Span::styled(
                app.economy_state.loot_queue_size.to_string(),
                Style::default().fg(t.text_accent),
            ),
            Span::styled("  Pending ", Style::default().fg(t.text_muted)),
            Span::styled(
                app.economy_state.loot_pending_distribute.to_string(),
                Style::default().fg(t.text_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("Banking ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!(
                    "{}/{} chars",
                    app.economy_state.banking_chars_done, app.economy_state.banking_chars_total
                ),
                Style::default().fg(t.text_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("Consolidated ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{} pp", app.economy_state.banking_consolidated_plat),
                Style::default().fg(t.hp_high),
            ),
        ]),
        Line::from(Span::styled(
            "6 opens full economy controls",
            Style::default().fg(t.text_muted),
        )),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(" Economy Detail ", t.border_primary, t))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_combat_tab(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Min(8),
        ])
        .split(area);

    draw_combat_overview(frame, rows[0], app);
    draw_spell_usage(frame, rows[1], app);
    draw_death_log(frame, rows[2], app, t);
}

fn draw_combat_overview(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let chunks = split_main_aside(area);
    let total_dps = app
        .visible_clients()
        .iter()
        .map(|client| estimated_client_dps(client, app))
        .sum::<u64>();
    let target = app
        .active_client()
        .and_then(|client| client.target.as_ref())
        .map(|spawn| app.redact_name(&spawn.displayed_name).into_owned())
        .unwrap_or_else(|| String::from("No combat target"));

    let dps_lines = vec![
        Line::from(vec![
            Span::styled("Target ", Style::default().fg(t.text_muted)),
            Span::styled(target, Style::default().fg(t.text_highlight)),
        ]),
        Line::from(vec![
            Span::styled("Fleet DPS ", Style::default().fg(t.text_muted)),
            Span::styled(total_dps.to_string(), Style::default().fg(t.text_accent)),
        ]),
        Line::from(vec![
            Span::styled("60s ", Style::default().fg(t.text_muted)),
            render_sparkline(
                &Sparkline::new(
                    app.orchestrator_state
                        .total_dps_samples
                        .iter()
                        .map(|value| *value as f64)
                        .collect(),
                )
                .with_max(max_sample(
                    app.orchestrator_state.total_dps_samples.iter().copied(),
                )),
                t.text_accent,
            ),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(dps_lines)
            .block(panel(" DPS Trend ", t.border_primary, t))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    let header = themed_header_row(&["Client", "DPS*", "Status"], t);
    let rows: Vec<Row<'_>> = app
        .visible_clients()
        .iter()
        .take(6)
        .map(|client| {
            let (status_label, status_color) = client_status_label(app, client);
            Row::new(vec![
                Cell::from(app.client_command_target(client))
                    .style(Style::default().fg(t.text_normal)),
                Cell::from(estimated_client_dps(client, app).to_string())
                    .style(Style::default().fg(t.text_accent)),
                Cell::from(status_label).style(Style::default().fg(status_color)),
            ])
        })
        .collect();

    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Min(12),
                Constraint::Length(6),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(panel(" Combat Round ", t.border_primary, t)),
        chunks[1],
    );
}

fn draw_spell_usage(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let header = themed_header_row(&["Spell", "Seen"], t);
    let rows: Vec<Row<'_>> = app
        .orchestrator_state
        .top_spells(8)
        .into_iter()
        .map(|(spell, count)| {
            Row::new(vec![
                Cell::from(truncate_inline(
                    spell,
                    area.width.saturating_sub(12) as usize,
                )),
                Cell::from(count.to_string()),
            ])
        })
        .collect();

    frame.render_widget(
        Table::new(rows, [Constraint::Min(16), Constraint::Length(6)])
            .header(header)
            .block(panel(" Spell Usage Frequency ", t.border_primary, t)),
        area,
    );
}

fn draw_death_log(frame: &mut Frame, area: Rect, app: &App, t: &Theme) {
    let lines = if app.orchestrator_state.recent_deaths(8).is_empty() {
        vec![Line::from(Span::styled(
            "No death or recovery events yet",
            Style::default().fg(t.text_muted),
        ))]
    } else {
        app.orchestrator_state
            .recent_deaths(8)
            .into_iter()
            .map(|entry| {
                Line::from(vec![
                    Span::styled(
                        format!("T{:>4} ", entry.tick),
                        Style::default().fg(t.text_muted),
                    ),
                    Span::styled(entry.message.as_str(), Style::default().fg(t.text_normal)),
                ])
            })
            .collect()
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(" Death Log / Recovery ", t.border_primary, t))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_system_tab(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = split_main_aside(area);
    draw_system_table(frame, chunks[0], app);
    draw_system_sidebar(frame, chunks[1], app);
}

fn draw_system_table(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let header = themed_header_row(&["Client", "RSS", "Fast", "Spawn", "Health"], t);
    let rows: Vec<Row<'_>> = app
        .visible_clients()
        .iter()
        .map(|client| {
            let (status_label, status_color) = client_status_label(app, client);
            Row::new(vec![
                Cell::from(app.client_command_target(client))
                    .style(Style::default().fg(t.text_normal)),
                Cell::from(process_memory_label(client.pid))
                    .style(Style::default().fg(t.text_secondary)),
                Cell::from(refresh_age_label(client.last_fast_refresh))
                    .style(Style::default().fg(t.text_secondary)),
                Cell::from(refresh_age_label(client.last_spawn_refresh))
                    .style(Style::default().fg(t.text_secondary)),
                Cell::from(status_label).style(Style::default().fg(status_color)),
            ])
        })
        .collect();

    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Min(12),
                Constraint::Length(10),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .block(panel(" Client Health ", t.border_primary, t)),
        area,
    );
}

fn draw_system_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let latencies: Vec<u64> = app
        .orchestrator_state
        .command_history
        .iter()
        .map(|entry| entry.latency_ms)
        .collect();
    let (p50, p95, p99) = latency_percentiles(&latencies);
    let health = system_health_label(app);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Frame rate ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{:.1} fps", 1000.0 / app.refresh_rate_ms.max(1) as f64),
                Style::default().fg(t.text_accent),
            ),
        ]),
        Line::from(vec![
            Span::styled("IPC p50/p95/p99 ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{p50}/{p95}/{p99} ms"),
                Style::default().fg(t.text_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("System ", Style::default().fg(t.text_muted)),
            Span::styled(health.0, Style::default().fg(health.1)),
        ]),
        Line::from(Span::styled(
            "Recent errors",
            Style::default()
                .fg(t.text_secondary)
                .add_modifier(Modifier::BOLD),
        )),
    ];

    if app.orchestrator_state.recent_errors(6).is_empty() {
        lines.push(Line::from(Span::styled(
            "No recent errors",
            Style::default().fg(t.text_muted),
        )));
    } else {
        for entry in app.orchestrator_state.recent_errors(6) {
            lines.push(Line::from(Span::styled(
                truncate_inline(entry, area.width.saturating_sub(4) as usize),
                Style::default().fg(t.hp_low),
            )));
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(" System Detail ", t.border_primary, t))
            .wrap(Wrap { trim: true }),
        area,
    );
}

#[derive(Clone)]
struct DashboardGroup<'a> {
    name: String,
    leader: String,
    members: Vec<&'a ClientState>,
}

fn dashboard_groups(app: &App) -> Vec<DashboardGroup<'_>> {
    if app.has_live_group_data() {
        let (live_groups, _) = app.build_live_groups();
        return live_groups
            .into_iter()
            .map(|group| DashboardGroup {
                name: group.name(),
                leader: group.leader,
                members: group
                    .member_names
                    .into_iter()
                    .filter_map(|name| app.find_client_by_name(&name))
                    .collect(),
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
            members: app.clients_in_group_idx(idx),
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
