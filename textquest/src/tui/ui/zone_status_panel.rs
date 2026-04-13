//! Zone Status Panel — per-client zone FSM state display.
//!
//! Shows zone name, FSM state (Walking → Zoning → Recovering), stuck flag,
//! and timeout countdown with color coding: green (normal), yellow (stuck),
//! red (timeout).

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::items_after_statements,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::needless_pass_by_value,
    clippy::no_effect_underscore_binding,
    clippy::option_if_let_else,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unreadable_literal,
    clippy::unused_self,
    clippy::used_underscore_binding,
    clippy::trivially_copy_pass_by_ref,
    clippy::ref_option,
    clippy::or_fun_call,
    clippy::needless_pass_by_ref_mut,
    clippy::wildcard_imports,
    clippy::redundant_field_names
)]

use std::collections::HashMap;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Row, Table},
};

use super::widgets::{panel, themed_header_row};
use crate::tui::app::App;

// ── Zone FSM state ────────────────────────────────────────────────────────────

/// Current phase in the zoning FSM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneFsmState {
    /// Client is walking toward a zone line.
    Walking,
    /// Zone transition is in progress (loading screen).
    Zoning,
    /// Post-zone recovery — waiting for the game world to fully load.
    Recovering,
    /// Idle — no zoning activity.
    Idle,
}

impl ZoneFsmState {
    /// Returns the short display label for this FSM state.
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Walking => "Walking",
            Self::Zoning => "Zoning",
            Self::Recovering => "Recovering",
            Self::Idle => "Idle",
        }
    }
}

/// Per-client zone status entry for display.
#[derive(Debug, Clone)]
pub struct ZoneClientStatus {
    /// Current zone short name (e.g. "commons", "befallen").
    pub zone_name: String,
    /// Current FSM phase.
    pub fsm_state: ZoneFsmState,
    /// Whether this client is currently stuck during zoning.
    pub stuck: bool,
    /// Seconds remaining before a timeout triggers recovery action.
    /// `None` means no timeout is active.
    pub timeout_secs: Option<u32>,
}

impl ZoneClientStatus {
    /// Creates a new idle zone status for the given zone.
    #[must_use]
    pub fn idle(zone_name: impl Into<String>) -> Self {
        Self {
            zone_name: zone_name.into(),
            fsm_state: ZoneFsmState::Idle,
            stuck: false,
            timeout_secs: None,
        }
    }
}

/// Application-level zone status state — one entry per PID.
#[derive(Debug, Default)]
pub struct ZoneStatusState {
    /// Per-client zone statuses keyed by PID.
    pub zone_statuses: HashMap<u32, ZoneClientStatus>,
}

impl ZoneStatusState {
    /// Creates a new, empty zone status state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            zone_statuses: HashMap::new(),
        }
    }
}

// ── Color helpers ─────────────────────────────────────────────────────────────

/// Returns the status color for a zone client entry.
fn zone_status_color(status: &ZoneClientStatus, t: &crate::tui::theme::Theme) -> Color {
    if status.timeout_secs.is_some_and(|s| s == 0) {
        // Timeout expired — red alert
        t.hp_low
    } else if status.stuck {
        // Stuck — yellow warning (map to text_highlight which is gold/yellow)
        t.text_highlight
    } else {
        match status.fsm_state {
            ZoneFsmState::Idle => t.text_muted,
            ZoneFsmState::Walking => t.hp_high,
            ZoneFsmState::Zoning => t.text_accent,
            ZoneFsmState::Recovering => t.text_secondary,
        }
    }
}

// ── Widget ────────────────────────────────────────────────────────────────────

/// Draw the zone status panel inside `area`.
///
/// Shows a table of per-client zone name, FSM state, stuck flag, and timeout
/// countdown. Color coding: green (normal), yellow (stuck), red (timeout).
pub fn draw_zone_status_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(100)])
        .split(area);

    let blk = panel(" Zone Status ", t.border_primary, t);

    let visible = app.visible_clients();
    if visible.is_empty() {
        frame.render_widget(
            Paragraph::new("No characters connected")
                .block(blk)
                .style(Style::default().fg(t.text_muted)),
            cols[0],
        );
        return;
    }

    let selected_pid = app.active_client().map(|c| c.pid);

    let header = themed_header_row(
        &["", "Character", "Zone", "FSM State", "Stuck", "Timeout"],
        t,
    );

    let rows: Vec<Row> = visible
        .iter()
        .map(|client| {
            let is_sel = Some(client.pid) == selected_pid;
            let marker = if is_sel { "▶" } else { " " };

            let name = client.local_player.as_ref().map_or_else(
                || app.client_command_target(client),
                |p| app.redact_name(&p.displayed_name).into_owned(),
            );

            let zone_status = app.zone_status_state.zone_statuses.get(&client.pid);

            // Fall back to the zone_name on ClientState if no explicit zone status yet
            let zone_name = zone_status
                .map(|s| s.zone_name.as_str())
                .unwrap_or_else(|| client.zone_name.as_str());

            let fsm_label = zone_status.map_or("Idle", |s| s.fsm_state.label());
            let stuck_label = zone_status.map_or("—", |s| if s.stuck { "YES" } else { "—" });
            let timeout_label = zone_status
                .and_then(|s| s.timeout_secs)
                .map_or_else(|| String::from("—"), |secs| format!("{secs}s"));

            let status_color = zone_status
                .map(|s| zone_status_color(s, t))
                .unwrap_or(t.text_muted);

            let stuck_color = zone_status
                .map(|s| {
                    if s.stuck {
                        t.text_highlight
                    } else {
                        t.text_muted
                    }
                })
                .unwrap_or(t.text_muted);

            let row_style = if is_sel {
                Style::default()
                    .bg(t.row_selected_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                ratatui::widgets::Cell::from(marker).style(Style::default().fg(t.text_accent)),
                ratatui::widgets::Cell::from(name).style(Style::default().fg(t.text_normal)),
                ratatui::widgets::Cell::from(zone_name.to_owned())
                    .style(Style::default().fg(t.text_secondary)),
                ratatui::widgets::Cell::from(fsm_label).style(Style::default().fg(status_color)),
                ratatui::widgets::Cell::from(stuck_label).style(Style::default().fg(stuck_color)),
                ratatui::widgets::Cell::from(timeout_label)
                    .style(Style::default().fg(t.text_secondary)),
            ])
            .style(row_style)
        })
        .collect();

    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Min(14),
                Constraint::Min(14),
                Constraint::Length(12),
                Constraint::Length(7),
                Constraint::Length(9),
            ],
        )
        .header(header)
        .block(blk),
        cols[0],
    );
}

/// Draw a compact zone status legend below the main table.
///
/// Shows color key: Green=Normal, Yellow=Stuck, Red=Timeout.
pub fn draw_zone_status_legend(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let line = Line::from(vec![
        Span::styled("● ", Style::default().fg(t.hp_high)),
        Span::styled("Normal  ", Style::default().fg(t.text_secondary)),
        Span::styled("● ", Style::default().fg(t.text_highlight)),
        Span::styled("Stuck  ", Style::default().fg(t.text_secondary)),
        Span::styled("● ", Style::default().fg(t.hp_low)),
        Span::styled("Timeout", Style::default().fg(t.text_secondary)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

// ── Demo data helpers ─────────────────────────────────────────────────────────

/// Populate zone status state with demo data for TUI preview mode (macOS / no live EQ).
pub fn load_demo_zone_statuses(state: &mut ZoneStatusState, clients: &[(u32, &str)]) {
    let demo_entries = [
        (ZoneFsmState::Idle, false, None, "commons"),
        (ZoneFsmState::Walking, false, Some(30u32), "befallen"),
        (ZoneFsmState::Zoning, false, Some(12u32), "lavastorm"),
        (ZoneFsmState::Recovering, true, Some(5u32), "ecommons"),
        (ZoneFsmState::Recovering, false, Some(0u32), "nro"),
    ];

    for (i, &(pid, _name)) in clients.iter().enumerate() {
        let (fsm, stuck, timeout, zone) = demo_entries[i % demo_entries.len()].clone();
        state.zone_statuses.insert(
            pid,
            ZoneClientStatus {
                zone_name: zone.to_owned(),
                fsm_state: fsm,
                stuck,
                timeout_secs: timeout,
            },
        );
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zone_fsm_state_labels() {
        assert_eq!(ZoneFsmState::Walking.label(), "Walking");
        assert_eq!(ZoneFsmState::Zoning.label(), "Zoning");
        assert_eq!(ZoneFsmState::Recovering.label(), "Recovering");
        assert_eq!(ZoneFsmState::Idle.label(), "Idle");
    }

    #[test]
    fn test_zone_client_status_idle_constructor() {
        let s = ZoneClientStatus::idle("commons");
        assert_eq!(s.zone_name, "commons");
        assert_eq!(s.fsm_state, ZoneFsmState::Idle);
        assert!(!s.stuck);
        assert!(s.timeout_secs.is_none());
    }

    #[test]
    fn test_zone_status_state_new_is_empty() {
        let state = ZoneStatusState::new();
        assert!(state.zone_statuses.is_empty());
    }

    #[test]
    fn test_zone_status_state_insert_and_retrieve() {
        let mut state = ZoneStatusState::new();
        state.zone_statuses.insert(
            1001,
            ZoneClientStatus {
                zone_name: String::from("befallen"),
                fsm_state: ZoneFsmState::Zoning,
                stuck: false,
                timeout_secs: Some(15),
            },
        );
        let entry = state.zone_statuses.get(&1001).unwrap();
        assert_eq!(entry.zone_name, "befallen");
        assert_eq!(entry.fsm_state, ZoneFsmState::Zoning);
        assert_eq!(entry.timeout_secs, Some(15));
    }

    #[test]
    fn test_load_demo_zone_statuses_populates_all_clients() {
        let mut state = ZoneStatusState::new();
        let clients: Vec<(u32, &str)> = vec![(1001, "Warrior"), (1002, "Cleric"), (1003, "Wizard")];
        load_demo_zone_statuses(&mut state, &clients);
        assert_eq!(state.zone_statuses.len(), 3);
        assert!(state.zone_statuses.contains_key(&1001));
        assert!(state.zone_statuses.contains_key(&1002));
        assert!(state.zone_statuses.contains_key(&1003));
    }

    #[test]
    fn test_load_demo_zone_statuses_timeout_client() {
        let mut state = ZoneStatusState::new();
        // Index 4 maps to demo entry with timeout=0 (expired)
        let clients: Vec<(u32, &str)> = vec![
            (1001, "A"),
            (1002, "B"),
            (1003, "C"),
            (1004, "D"),
            (1005, "E"),
        ];
        load_demo_zone_statuses(&mut state, &clients);
        let entry = state.zone_statuses.get(&1005).unwrap();
        assert_eq!(entry.timeout_secs, Some(0));
    }

    #[test]
    fn test_zone_client_status_stuck_flag() {
        let s = ZoneClientStatus {
            zone_name: String::from("ecommons"),
            fsm_state: ZoneFsmState::Recovering,
            stuck: true,
            timeout_secs: Some(5),
        };
        assert!(s.stuck);
    }
}
