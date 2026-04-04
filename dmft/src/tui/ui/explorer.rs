//! Ghidra function/offset explorer panel for the Debug screen.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, Wrap},
};

use super::widgets::{panel, themed_header_row};
use crate::tui::app::{ActivePanel, App};

/// Color for usability classification.
fn usability_color(usability: Option<&str>) -> Color {
    match usability {
        Some("client_authoritative") => Color::Green,
        Some("hybrid") => Color::Yellow,
        Some("untested") => Color::DarkGray,
        Some("server_validated") => Color::Red,
        Some("not_applicable") => Color::DarkGray,
        _ => Color::DarkGray,
    }
}

/// Short usability label for the table.
fn usability_label(usability: Option<&str>) -> &str {
    match usability {
        Some("client_authoritative") => "client",
        Some("hybrid") => "hybrid",
        Some("untested") => "untested",
        Some("server_validated") => "server",
        Some("not_applicable") => "n/a",
        _ => "?",
    }
}

/// Draw the Ghidra function explorer panel.
pub fn draw_explorer_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    let t = &app.theme;
    let is_active = app.active_panel == ActivePanel::DebugExplorer;
    let border_style = if is_active {
        t.border_active
    } else {
        t.border_dim
    };

    let state = &app.explorer_state;
    let cat_label = state.category_filter.label();
    let filtered = state.filtered_functions.len();
    let total = state.total_count;

    let title = if state.search_mode {
        format!(
            " Explorer ({filtered}/{total}) [{cat_label}] search: \"{}\" [Esc] ",
            state.search_filter,
        )
    } else if !state.search_filter.is_empty() {
        format!(
            " Explorer ({filtered}/{total}) [{cat_label}] filter: \"{}\" ",
            state.search_filter,
        )
    } else {
        format!(" Explorer ({filtered}/{total}) [{cat_label}] ")
    };

    // Split into table + stats footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(area);

    let header = themed_header_row(&["Address", "Name", "Category", "Usability", "Size"], t);

    let highlight_style = Style::default()
        .bg(t.row_selected_bg)
        .add_modifier(Modifier::BOLD);

    let rows: Vec<Row> = state
        .filtered_functions
        .iter()
        .map(|f| {
            let addr = format!("0x{:X}", f.address);
            let name = f.name.as_str();
            let cat = f.category.as_deref().unwrap_or("-");
            let usab = f.usability.as_deref();
            let size = f
                .size
                .map_or_else(|| "-".to_string(), |s| format!("{s}"));
            let color = usability_color(usab);

            Row::new(vec![
                Cell::from(Span::styled(
                    addr,
                    Style::default().fg(t.text_accent),
                )),
                Cell::from(Span::styled(name, Style::default().fg(t.text_bright))),
                Cell::from(Span::styled(
                    cat,
                    Style::default().fg(t.text_secondary),
                )),
                Cell::from(Span::styled(
                    usability_label(usab),
                    Style::default().fg(color),
                )),
                Cell::from(Span::styled(
                    size,
                    Style::default().fg(t.text_muted),
                )),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(18),  // Address
        Constraint::Min(20),     // Name (flex)
        Constraint::Length(14),  // Category
        Constraint::Length(10),  // Usability
        Constraint::Length(8),   // Size
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(panel(title.as_str(), border_style, t))
        .row_highlight_style(highlight_style);

    frame.render_stateful_widget(table, chunks[0], &mut app.explorer_state.table_state);

    // Stats footer
    let db_status = if app.ghidra_db.is_some() {
        "DB loaded"
    } else {
        "No DB"
    };
    let footer = Line::from(vec![
        Span::styled(
            format!(" {db_status} | "),
            Style::default().fg(t.text_muted),
        ),
        Span::styled(
            format!("{filtered} shown / {total} total"),
            Style::default().fg(t.text_secondary),
        ),
        Span::styled(
            " | / search  c cycle category  j/k scroll",
            Style::default().fg(t.text_muted),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(footer).wrap(Wrap { trim: true }),
        chunks[1],
    );
}
