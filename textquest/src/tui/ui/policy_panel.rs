//! Policy debug panel — active learned policy versions, sources, and rollback.
//!
//! Displays all scopes with active policy versions, their source canary bundle,
//! promotion time, and offers single-keystroke rollback per row (`r`).

use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
};

use super::widgets::{panel, themed_header_row};
use crate::tui::app::{ActivePanel, App};

/// Draw the policy debug screen.
pub fn draw_policy_screen(frame: &mut Frame, area: Rect, app: &mut App) {
    let t = &app.theme;
    let is_active = app.active_panel == ActivePanel::PolicyPanel;
    let border_style = if is_active {
        t.border_active
    } else {
        t.border_dim
    };

    let title = " Active Policies [r: rollback selected] ";
    let block = panel(title, border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let header = themed_header_row(&["Scope", "Version", "Source Bundle", "Promoted"], t);

    let rows: Vec<Row> = app
        .policy_panel_state
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_selected = app.policy_panel_state.selected == i;
            let base_style = if is_selected {
                Style::default()
                    .bg(t.text_accent)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text_primary)
            };

            Row::new(vec![
                Cell::from(entry.scope.as_str()).style(base_style),
                Cell::from(entry.version.as_str()).style(if entry.version == "rule-based" {
                    base_style.fg(t.text_secondary)
                } else {
                    base_style.fg(t.text_accent)
                }),
                Cell::from(entry.source_bundle.as_str()).style(base_style),
                Cell::from(entry.promoted_at.as_str()).style(Style::default().fg(t.text_secondary)),
            ])
        })
        .collect();

    if rows.is_empty() {
        let msg = Paragraph::from(vec![
            Line::from(vec![Span::styled(
                "No active policies — all scopes using rule-based strategy.",
                Style::default().fg(t.text_secondary),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Promote an L-7 artifact to activate a learned policy for a scope.",
                Style::default().fg(t.text_dim),
            )]),
        ]);
        frame.render_widget(msg, inner);
        return;
    }

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Percentage(20),
            Constraint::Percentage(35),
            Constraint::Percentage(15),
        ],
    )
    .header(header)
    .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_widget(table, inner);
}

/// Cached policy entry for TUI display.
#[derive(Debug, Clone, Default)]
pub struct PolicyPanelEntry {
    pub scope: String,
    pub version: String,
    pub source_bundle: String,
    pub promoted_at: String,
}

/// State for the policy panel widget.
#[derive(Debug, Default)]
pub struct PolicyPanelState {
    pub entries: Vec<PolicyPanelEntry>,
    pub selected: usize,
}

impl PolicyPanelState {
    pub fn select_next(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = (self.selected + 1).min(self.entries.len() - 1);
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn selected_scope(&self) -> Option<&str> {
        self.entries.get(self.selected).map(|e| e.scope.as_str())
    }
}
