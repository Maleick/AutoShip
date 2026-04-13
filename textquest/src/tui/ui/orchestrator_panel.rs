//! Orchestrator panel — live session visibility for the multibox fleet.
//!
//! Displays active sessions with status, relay stats (latency, success/fail
//! rates), and a ring buffer of the last 10 relay commands.  Color coding:
//! - Green  = healthy session
//! - Yellow = slow / degraded (latency > 100 ms)
//! - Red    = error / offline

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};
use std::collections::VecDeque;

use super::widgets::panel;
use crate::tui::theme::Theme;

// ── Domain types ──────────────────────────────────────────────────────────────

/// Health status for a session or the relay overall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionHealth {
    /// Session is responding normally.
    Healthy,
    /// Session is slow (high latency) or partially degraded.
    Slow,
    /// Session has errors or is offline.
    Error,
}

impl SessionHealth {
    /// Returns the theme colour for this health status.
    #[must_use]
    pub fn color(self, t: &Theme) -> Color {
        match self {
            Self::Healthy => t.hp_high,
            Self::Slow => t.text_highlight,
            Self::Error => t.hp_low,
        }
    }

    /// Returns the short status label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Healthy => "OK",
            Self::Slow => "SLOW",
            Self::Error => "ERR",
        }
    }
}

/// A single EQ client session visible to the orchestrator.
#[derive(Debug, Clone)]
pub struct OrchestratorSession {
    /// Unique session identifier (usually PID-based).
    pub session_id: String,
    /// Group this session belongs to (e.g., "G1", "Alpha").
    pub group: String,
    /// OS process ID of the EQ client.
    pub client_pid: u32,
    /// Human-readable session status.
    pub status: String,
    /// Health computed from latency / error state.
    pub health: SessionHealth,
}

/// Rolling relay statistics (computed from recent command history).
#[derive(Debug, Clone, Default)]
pub struct RelayStats {
    /// Average round-trip latency over the last 10 commands (ms).
    pub avg_latency_ms: u64,
    /// Number of successful commands in the tracked window.
    pub success_count: usize,
    /// Number of failed commands in the tracked window.
    pub fail_count: usize,
}

impl RelayStats {
    /// Success rate as a percentage (0–100).
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

/// A single relay command entry kept in the history ring.
#[derive(Debug, Clone)]
pub struct RelayCommandEntry {
    /// Which session received this command.
    pub session_id: String,
    /// The command text that was relayed.
    pub command: String,
    /// Whether the relay succeeded.
    pub success: bool,
    /// Round-trip latency in milliseconds.
    pub latency_ms: u64,
}

// ── Panel state ───────────────────────────────────────────────────────────────

/// Mutable state for the orchestrator visibility panel.
#[derive(Debug, Clone)]
pub struct OrchestratorPanelState {
    /// Known sessions, rebuilt from orchestrator state each tick.
    pub sessions: Vec<OrchestratorSession>,
    /// Relay statistics over the history window.
    pub relay_stats: RelayStats,
    /// Ring buffer of the last 10 relay commands.
    pub command_history: VecDeque<RelayCommandEntry>,
    /// Currently selected session index (for Up/Down navigation).
    pub selected_session: usize,
    /// Currently selected group tab index (for Tab navigation).
    pub selected_group_tab: usize,
    /// Whether to show group membership popup overlay.
    pub show_group_membership: bool,
    /// ratatui list state for the session list.
    pub list_state: ListState,
}

impl OrchestratorPanelState {
    /// Create a new state pre-populated with demo data so the panel renders
    /// on macOS without a live EQ client.
    #[must_use]
    pub fn new() -> Self {
        let sessions = demo_sessions();
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            sessions,
            relay_stats: demo_relay_stats(),
            command_history: demo_command_history(),
            selected_session: 0,
            selected_group_tab: 0,
            show_group_membership: false,
            list_state,
        }
    }

    /// Select the next session in the list.
    pub fn select_next(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let next = (self.selected_session + 1) % self.sessions.len();
        self.selected_session = next;
        self.list_state.select(Some(next));
    }

    /// Select the previous session in the list.
    pub fn select_prev(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let prev = if self.selected_session == 0 {
            self.sessions.len() - 1
        } else {
            self.selected_session - 1
        };
        self.selected_session = prev;
        self.list_state.select(Some(prev));
    }

    /// Cycle to the next group tab.
    pub fn next_group_tab(&mut self, group_count: usize) {
        if group_count == 0 {
            return;
        }
        self.selected_group_tab = (self.selected_group_tab + 1) % group_count;
    }

    /// Toggle the group membership overlay.
    pub fn toggle_group_membership(&mut self) {
        self.show_group_membership = !self.show_group_membership;
    }

    /// Push a new relay command result to the history ring (cap at 10).
    pub fn push_command(&mut self, entry: RelayCommandEntry) {
        if self.command_history.len() >= 10 {
            self.command_history.pop_front();
        }
        self.command_history.push_back(entry);
        self.rebuild_stats();
    }

    /// Recompute relay stats from current command history.
    fn rebuild_stats(&mut self) {
        let success_count = self.command_history.iter().filter(|e| e.success).count();
        let fail_count = self.command_history.len() - success_count;
        let avg_latency_ms = if self.command_history.is_empty() {
            0
        } else {
            let sum: u64 = self.command_history.iter().map(|e| e.latency_ms).sum();
            sum / self.command_history.len() as u64
        };
        self.relay_stats = RelayStats {
            avg_latency_ms,
            success_count,
            fail_count,
        };
    }

    /// Return the unique group names present in the current session list.
    #[must_use]
    pub fn group_names(&self) -> Vec<String> {
        let mut groups: Vec<String> = self
            .sessions
            .iter()
            .map(|s| s.group.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        groups.sort();
        groups
    }

    /// Sessions that belong to the currently selected group tab.
    #[must_use]
    pub fn sessions_for_selected_group(&self) -> Vec<&OrchestratorSession> {
        let groups = self.group_names();
        if groups.is_empty() {
            return self.sessions.iter().collect();
        }
        let group = match groups.get(self.selected_group_tab % groups.len()) {
            Some(g) => g.clone(),
            None => return self.sessions.iter().collect(),
        };
        self.sessions.iter().filter(|s| s.group == group).collect()
    }
}

impl Default for OrchestratorPanelState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Demo / stub data ──────────────────────────────────────────────────────────

fn demo_sessions() -> Vec<OrchestratorSession> {
    vec![
        OrchestratorSession {
            session_id: "S-001".into(),
            group: "G1".into(),
            client_pid: 1001,
            status: "In Combat".into(),
            health: SessionHealth::Healthy,
        },
        OrchestratorSession {
            session_id: "S-002".into(),
            group: "G1".into(),
            client_pid: 1002,
            status: "Medding".into(),
            health: SessionHealth::Healthy,
        },
        OrchestratorSession {
            session_id: "S-003".into(),
            group: "G2".into(),
            client_pid: 1003,
            status: "High Latency".into(),
            health: SessionHealth::Slow,
        },
        OrchestratorSession {
            session_id: "S-004".into(),
            group: "G2".into(),
            client_pid: 1004,
            status: "Offline".into(),
            health: SessionHealth::Error,
        },
        OrchestratorSession {
            session_id: "S-005".into(),
            group: "G3".into(),
            client_pid: 1005,
            status: "Idle".into(),
            health: SessionHealth::Healthy,
        },
    ]
}

fn demo_relay_stats() -> RelayStats {
    RelayStats {
        avg_latency_ms: 42,
        success_count: 47,
        fail_count: 3,
    }
}

fn demo_command_history() -> VecDeque<RelayCommandEntry> {
    let entries = vec![
        ("S-001", "/camp", true, 38),
        ("S-002", "/follow", true, 41),
        ("S-003", "/cast 5", false, 180),
        ("S-001", "/target mob", true, 35),
        ("S-004", "/follow", false, 220),
        ("S-002", "/cast 3", true, 45),
        ("S-005", "/camp", true, 39),
        ("S-001", "/cast 1", true, 36),
        ("S-003", "/target", true, 95),
        ("S-005", "/follow", true, 40),
    ];
    entries
        .into_iter()
        .map(
            |(session_id, command, success, latency_ms)| RelayCommandEntry {
                session_id: session_id.into(),
                command: command.into(),
                success,
                latency_ms,
            },
        )
        .collect()
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Draw the orchestrator overview screen.
pub fn draw_orchestrator_screen(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    state: &mut OrchestratorPanelState,
    t: &Theme,
) {
    // Split into top (sessions + stats) and bottom (command history).
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(14)])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(chunks[0]);

    draw_session_list(frame, top[0], state, t);
    draw_relay_stats(frame, top[1], state, t);
    draw_command_history(frame, chunks[1], state, t);

    // Group membership popup overlay (if active)
    if state.show_group_membership {
        draw_group_membership_overlay(frame, area, state, t);
    }
}

fn draw_session_list(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    state: &mut OrchestratorPanelState,
    t: &Theme,
) {
    let groups = state.group_names();
    let current_group = if groups.is_empty() {
        String::from("All")
    } else {
        groups
            .get(state.selected_group_tab % groups.len())
            .cloned()
            .unwrap_or_else(|| String::from("All"))
    };

    // Build group tab bar line
    let tab_spans: Vec<Span<'_>> = if groups.is_empty() {
        vec![Span::styled(" All ", t.tab_active)]
    } else {
        let mut spans = Vec::new();
        for (i, g) in groups.iter().enumerate() {
            let is_active = i == state.selected_group_tab % groups.len();
            let style = if is_active {
                t.tab_active
            } else {
                t.tab_inactive
            };
            spans.push(Span::styled(format!(" {g} "), style));
        }
        spans
    };

    let total = state.sessions.len();
    let healthy = state
        .sessions
        .iter()
        .filter(|s| s.health == SessionHealth::Healthy)
        .count();
    let title = format!(" Orchestrator Sessions ({healthy}/{total}) [Tab=group, G=membership] ");
    let blk = panel(title.as_str(), t.border_active, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    // Split inner: tab bar (1 line) + list
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    frame.render_widget(Paragraph::new(Line::from(tab_spans)), inner_chunks[0]);

    // Session list filtered to current group tab
    let visible_sessions: Vec<&OrchestratorSession> = if groups.is_empty() {
        state.sessions.iter().collect()
    } else {
        let group_name = groups
            .get(state.selected_group_tab % groups.len())
            .cloned()
            .unwrap_or_default();
        state
            .sessions
            .iter()
            .filter(|s| s.group == group_name)
            .collect()
    };

    let items: Vec<ListItem<'_>> = visible_sessions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let is_selected = state.list_state.selected().is_some_and(|sel| {
                // Map the global selected_session index back to local
                state
                    .sessions
                    .get(sel)
                    .is_some_and(|gs| gs.session_id == s.session_id)
            });

            let health_color = s.health.color(t);
            let status_style = Style::default().fg(health_color);
            let row_style = if is_selected {
                Style::default()
                    .fg(t.text_normal)
                    .add_modifier(Modifier::REVERSED)
            } else {
                Style::default().fg(t.text_normal)
            };

            let _ = i; // suppress warning — index used for visual alignment
            let line = Line::from(vec![
                Span::styled(
                    format!(" {:<6}", s.session_id),
                    Style::default().fg(t.text_accent),
                ),
                Span::styled(
                    format!(" {:>6}", s.client_pid),
                    Style::default().fg(t.text_secondary),
                ),
                Span::styled(format!(" {:<4}", s.health.label()), status_style),
                Span::styled(format!(" {}", s.status), row_style),
            ]);
            ListItem::new(line)
        })
        .collect();

    // Column header
    let header = Line::from(vec![
        Span::styled(
            " ID    ",
            Style::default()
                .fg(t.text_muted)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "    PID",
            Style::default()
                .fg(t.text_muted)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " HLTH",
            Style::default()
                .fg(t.text_muted)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " STATUS",
            Style::default()
                .fg(t.text_muted)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    // Split again for header + list
    let list_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner_chunks[1]);

    frame.render_widget(Paragraph::new(header), list_chunks[0]);

    let list = List::new(items).highlight_style(
        Style::default()
            .fg(t.text_normal)
            .add_modifier(Modifier::REVERSED),
    );

    // Render with mutable list state for cursor tracking
    frame.render_stateful_widget(list, list_chunks[1], &mut state.list_state);

    let _ = current_group; // used implicitly via filtering above
}

fn draw_relay_stats(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    state: &OrchestratorPanelState,
    t: &Theme,
) {
    let blk = panel(" Relay Stats ", t.border_primary, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let stats = &state.relay_stats;
    let latency_color = if stats.avg_latency_ms < 60 {
        t.hp_high
    } else if stats.avg_latency_ms < 150 {
        t.text_highlight
    } else {
        t.hp_low
    };

    let rate = stats.success_rate();
    let rate_color = if rate >= 95.0 {
        t.hp_high
    } else if rate >= 80.0 {
        t.text_highlight
    } else {
        t.hp_low
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Avg Latency ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{} ms", stats.avg_latency_ms),
                Style::default()
                    .fg(latency_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Success     ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{}", stats.success_count),
                Style::default().fg(t.hp_high),
            ),
        ]),
        Line::from(vec![
            Span::styled("Failed      ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{}", stats.fail_count),
                Style::default().fg(t.hp_low),
            ),
        ]),
        Line::from(vec![
            Span::styled("Rate        ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{rate:.1}%"),
                Style::default().fg(rate_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        Line::from(Span::styled(
            " Up/Down: select session",
            Style::default().fg(t.text_muted),
        )),
        Line::from(Span::styled(
            " Tab: cycle group",
            Style::default().fg(t.text_muted),
        )),
        Line::from(Span::styled(
            " G: group membership",
            Style::default().fg(t.text_muted),
        )),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_command_history(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    state: &OrchestratorPanelState,
    t: &Theme,
) {
    let blk = panel(" Relay Command History (last 10) ", t.border_dim, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let header = Line::from(vec![
        Span::styled(
            " SESSION ",
            Style::default()
                .fg(t.text_muted)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " COMMAND          ",
            Style::default()
                .fg(t.text_muted)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " STATUS",
            Style::default()
                .fg(t.text_muted)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  LAT",
            Style::default()
                .fg(t.text_muted)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let mut lines = vec![header];

    // Render newest-first
    for entry in state.command_history.iter().rev() {
        let status_span = if entry.success {
            Span::styled("  OK   ", Style::default().fg(t.hp_high))
        } else {
            Span::styled("  FAIL ", Style::default().fg(t.hp_low))
        };

        let latency_color = if entry.latency_ms < 60 {
            t.hp_high
        } else if entry.latency_ms < 150 {
            t.text_highlight
        } else {
            t.hp_low
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<7} ", entry.session_id),
                Style::default().fg(t.text_accent),
            ),
            Span::styled(
                format!(" {:<16} ", entry.command),
                Style::default().fg(t.text_normal),
            ),
            status_span,
            Span::styled(
                format!(" {:>4}ms", entry.latency_ms),
                Style::default().fg(latency_color),
            ),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_group_membership_overlay(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    state: &OrchestratorPanelState,
    t: &Theme,
) {
    use super::widgets::centered_popup;
    use ratatui::widgets::Clear;

    let popup_area = centered_popup(area, 60, 70, 40, 10, 80, 28, 2);
    frame.render_widget(Clear, popup_area);

    let blk = panel(" Group Membership [G to close] ", t.border_active, t);
    let inner = blk.inner(popup_area);
    frame.render_widget(blk, popup_area);

    let groups = state.group_names();
    let mut lines: Vec<Line<'_>> = Vec::new();

    for group_name in &groups {
        lines.push(Line::from(vec![Span::styled(
            format!("  {group_name}"),
            Style::default()
                .fg(t.text_highlight)
                .add_modifier(Modifier::BOLD),
        )]));

        for s in state.sessions.iter().filter(|s| s.group == *group_name) {
            let health_color = s.health.color(t);
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(
                    format!("{:<6}", s.session_id),
                    Style::default().fg(t.text_accent),
                ),
                Span::styled(
                    format!(" pid:{:<6}", s.client_pid),
                    Style::default().fg(t.text_secondary),
                ),
                Span::styled(
                    format!(" {:>4}", s.health.label()),
                    Style::default().fg(health_color),
                ),
                Span::styled(format!(" {}", s.status), Style::default().fg(t.text_normal)),
            ]));
        }

        lines.push(Line::raw(""));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No sessions registered",
            Style::default().fg(t.text_muted),
        )));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_has_demo_sessions() {
        let state = OrchestratorPanelState::new();
        assert!(
            !state.sessions.is_empty(),
            "demo sessions must be populated"
        );
    }

    #[test]
    fn new_state_has_command_history() {
        let state = OrchestratorPanelState::new();
        assert_eq!(
            state.command_history.len(),
            10,
            "demo history should have exactly 10 entries"
        );
    }

    #[test]
    fn session_health_labels() {
        assert_eq!(SessionHealth::Healthy.label(), "OK");
        assert_eq!(SessionHealth::Slow.label(), "SLOW");
        assert_eq!(SessionHealth::Error.label(), "ERR");
    }

    #[test]
    fn relay_stats_success_rate_empty() {
        let stats = RelayStats::default();
        assert!(
            (stats.success_rate() - 100.0).abs() < f32::EPSILON,
            "empty history should give 100% success rate"
        );
    }

    #[test]
    fn relay_stats_success_rate_partial() {
        let stats = RelayStats {
            avg_latency_ms: 50,
            success_count: 3,
            fail_count: 1,
        };
        assert!(
            (stats.success_rate() - 75.0).abs() < f32::EPSILON,
            "3 of 4 = 75%"
        );
    }

    #[test]
    fn select_next_wraps_around() {
        let mut state = OrchestratorPanelState::new();
        let count = state.sessions.len();
        state.selected_session = count - 1;
        state.select_next();
        assert_eq!(state.selected_session, 0);
    }

    #[test]
    fn select_prev_wraps_around() {
        let mut state = OrchestratorPanelState::new();
        state.selected_session = 0;
        state.select_prev();
        assert_eq!(state.selected_session, state.sessions.len() - 1);
    }

    #[test]
    fn push_command_caps_at_10() {
        let mut state = OrchestratorPanelState::new();
        assert_eq!(state.command_history.len(), 10);
        state.push_command(RelayCommandEntry {
            session_id: "S-999".into(),
            command: "/test".into(),
            success: true,
            latency_ms: 10,
        });
        assert_eq!(
            state.command_history.len(),
            10,
            "history must not exceed 10"
        );
        assert_eq!(state.command_history.back().unwrap().session_id, "S-999");
    }

    #[test]
    fn push_command_rebuilds_stats() {
        let mut state = OrchestratorPanelState::new();
        // Clear existing history for a clean baseline.
        state.command_history.clear();
        state.push_command(RelayCommandEntry {
            session_id: "S-001".into(),
            command: "/test".into(),
            success: true,
            latency_ms: 80,
        });
        state.push_command(RelayCommandEntry {
            session_id: "S-002".into(),
            command: "/test".into(),
            success: false,
            latency_ms: 20,
        });
        assert_eq!(state.relay_stats.success_count, 1);
        assert_eq!(state.relay_stats.fail_count, 1);
        assert_eq!(state.relay_stats.avg_latency_ms, 50);
    }

    #[test]
    fn group_names_are_sorted_and_unique() {
        let state = OrchestratorPanelState::new();
        let names = state.group_names();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(names, sorted, "group names must be sorted and deduplicated");
    }

    #[test]
    fn next_group_tab_cycles() {
        let mut state = OrchestratorPanelState::new();
        let count = state.group_names().len();
        state.selected_group_tab = count - 1;
        state.next_group_tab(count);
        assert_eq!(state.selected_group_tab, 0);
    }

    #[test]
    fn sessions_for_selected_group_filters_correctly() {
        let state = OrchestratorPanelState::new();
        let group_sessions = state.sessions_for_selected_group();
        // All returned sessions must belong to the same group.
        let groups: std::collections::HashSet<_> =
            group_sessions.iter().map(|s| s.group.as_str()).collect();
        assert_eq!(
            groups.len(),
            1,
            "filtered sessions should all be in one group"
        );
    }

    #[test]
    fn toggle_group_membership_flips_bool() {
        let mut state = OrchestratorPanelState::new();
        assert!(!state.show_group_membership);
        state.toggle_group_membership();
        assert!(state.show_group_membership);
        state.toggle_group_membership();
        assert!(!state.show_group_membership);
    }
}
