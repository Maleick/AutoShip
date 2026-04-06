//! EQ Internals panel — offset browser for compiled EQ memory offsets.
//!
//! Lists all known offsets from `OffsetDatabase::from_compiled_offsets()` in a
//! scrollable table. Selecting an offset auto-scrolls the hex dump panel to that
//! address. Category filter cycles with `c`, search with `/`.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
};

use super::widgets::{panel, themed_header_row};
use crate::tui::app::{ActivePanel, App};
use crate::tui::state::OffsetCategory;

/// Draw the EQ Internals offset browser panel.
pub fn draw_eq_internals_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    let t = &app.theme;
    let is_active = app.active_panel == ActivePanel::DebugInternals;
    let border_style = if is_active {
        t.border_active
    } else {
        t.border_dim
    };

    let cat_label = app.eq_internals_state.category_filter.label();
    let filtered = app.eq_internals_state.filtered_entries.len();
    let total = app.eq_internals_state.all_entries.len();
    let search_mode = app.eq_internals_state.search_mode;
    let search_filter = app.eq_internals_state.search_filter.clone();

    let title = if search_mode {
        format!(
            " EQ Internals ({filtered}/{total}) [{cat_label}] search: \"{search_filter}\" [Esc] ",
        )
    } else if !search_filter.is_empty() {
        format!(" EQ Internals ({filtered}/{total}) [{cat_label}] filter: \"{search_filter}\" ",)
    } else {
        format!(" EQ Internals ({filtered}/{total}) [{cat_label}] ")
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(area);

    let header = themed_header_row(&["Category", "Name", "Value"], t);

    let text_accent = t.text_accent;
    let text_bright = t.text_bright;
    let text_secondary = t.text_secondary;
    let row_selected_bg = t.row_selected_bg;

    let rows: Vec<Row> = app
        .eq_internals_state
        .filtered_entries
        .iter()
        .map(|e| {
            let cat = e.category.label();
            let name = e.name.as_str();
            let value = match e.category {
                OffsetCategory::Globals | OffsetCategory::Functions => {
                    format!("0x{:X}", e.value)
                }
                _ => format!("+0x{:X}", e.value),
            };

            Row::new(vec![
                Cell::from(Span::styled(cat, Style::default().fg(text_secondary))),
                Cell::from(Span::styled(name, Style::default().fg(text_bright))),
                Cell::from(Span::styled(value, Style::default().fg(text_accent))),
            ])
        })
        .collect();

    let highlight_style = Style::default()
        .bg(row_selected_bg)
        .add_modifier(Modifier::BOLD);

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Min(20),
            Constraint::Length(18),
        ],
    )
    .header(header)
    .block(panel(title.as_str(), border_style, t))
    .row_highlight_style(highlight_style);

    frame.render_stateful_widget(table, chunks[0], &mut app.eq_internals_state.table_state);

    // Status bar hint.
    let hint = Line::from(vec![
        Span::styled(
            " ↑↓ ",
            Style::default()
                .fg(text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("scroll  ", Style::default().fg(text_secondary)),
        Span::styled(
            "Enter ",
            Style::default()
                .fg(text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("→ hex  ", Style::default().fg(text_secondary)),
        Span::styled(
            "c ",
            Style::default()
                .fg(text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("category  ", Style::default().fg(text_secondary)),
        Span::styled(
            "/ ",
            Style::default()
                .fg(text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("search", Style::default().fg(text_secondary)),
    ]);

    frame.render_widget(Paragraph::new(hint), chunks[1]);
}
