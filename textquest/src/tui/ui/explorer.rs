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

    // Extract state values before building widgets to avoid borrow conflicts.
    let cat_label = app.explorer_state.category_filter.label();
    let filtered = app.explorer_state.filtered_functions.len();
    let total = app.explorer_state.total_count;
    let search_mode = app.explorer_state.search_mode;
    let search_filter = app.explorer_state.search_filter.clone();
    let has_db = app.ghidra_db.is_some();

    let title = if search_mode {
        format!(" Explorer ({filtered}/{total}) [{cat_label}] search: \"{search_filter}\" [Esc] ",)
    } else if !search_filter.is_empty() {
        format!(" Explorer ({filtered}/{total}) [{cat_label}] filter: \"{search_filter}\" ",)
    } else {
        format!(" Explorer ({filtered}/{total}) [{cat_label}] ")
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(area);

    let header = themed_header_row(&["Address", "Name", "Category", "Usability", "Size"], t);

    let text_accent = t.text_accent;
    let text_bright = t.text_bright;
    let text_secondary = t.text_secondary;
    let text_muted = t.text_muted;
    let row_selected_bg = t.row_selected_bg;

    let rows: Vec<Row> = app
        .explorer_state
        .filtered_functions
        .iter()
        .map(|f| {
            let addr = format!("0x{:X}", f.address);
            let name = f.name.as_str();
            let cat = f.category.as_deref().unwrap_or("-");
            let usab = f.usability.as_deref();
            let size = f.size.map_or_else(|| "-".to_string(), |s| format!("{s}"));
            let color = usability_color(usab);

            Row::new(vec![
                Cell::from(Span::styled(addr, Style::default().fg(text_accent))),
                Cell::from(Span::styled(name, Style::default().fg(text_bright))),
                Cell::from(Span::styled(cat, Style::default().fg(text_secondary))),
                Cell::from(Span::styled(
                    usability_label(usab),
                    Style::default().fg(color),
                )),
                Cell::from(Span::styled(size, Style::default().fg(text_muted))),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(18),
        Constraint::Min(20),
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(8),
    ];

    let block = panel(title.as_str(), border_style, &app.theme);
    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(
            Style::default()
                .bg(row_selected_bg)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_stateful_widget(table, chunks[0], &mut app.explorer_state.table_state);

    let db_status = if has_db {
        "Local Ghidra DB loaded"
    } else {
        "No local Ghidra DB"
    };
    let footer = Line::from(vec![
        Span::styled(format!(" {db_status} | "), Style::default().fg(text_muted)),
        Span::styled(
            format!("{filtered} shown / {total} total"),
            Style::default().fg(text_secondary),
        ),
        Span::styled(
            " | / search  c cycle category  j/k scroll",
            Style::default().fg(text_muted),
        ),
    ]);
    frame.render_widget(Paragraph::new(footer).wrap(Wrap { trim: true }), chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── usability_color ─────────────────────────────────────────────

    #[test]
    fn usability_color_client_authoritative() {
        assert_eq!(usability_color(Some("client_authoritative")), Color::Green);
    }

    #[test]
    fn usability_color_hybrid() {
        assert_eq!(usability_color(Some("hybrid")), Color::Yellow);
    }

    #[test]
    fn usability_color_untested() {
        assert_eq!(usability_color(Some("untested")), Color::DarkGray);
    }

    #[test]
    fn usability_color_server_validated() {
        assert_eq!(usability_color(Some("server_validated")), Color::Red);
    }

    #[test]
    fn usability_color_not_applicable() {
        assert_eq!(usability_color(Some("not_applicable")), Color::DarkGray);
    }

    #[test]
    fn usability_color_none() {
        assert_eq!(usability_color(None), Color::DarkGray);
    }

    #[test]
    fn usability_color_unknown_string() {
        assert_eq!(usability_color(Some("something_else")), Color::DarkGray);
    }

    // ── usability_label ─────────────────────────────────────────────

    #[test]
    fn usability_label_client_authoritative() {
        assert_eq!(usability_label(Some("client_authoritative")), "client");
    }

    #[test]
    fn usability_label_hybrid() {
        assert_eq!(usability_label(Some("hybrid")), "hybrid");
    }

    #[test]
    fn usability_label_untested() {
        assert_eq!(usability_label(Some("untested")), "untested");
    }

    #[test]
    fn usability_label_server_validated() {
        assert_eq!(usability_label(Some("server_validated")), "server");
    }

    #[test]
    fn usability_label_not_applicable() {
        assert_eq!(usability_label(Some("not_applicable")), "n/a");
    }

    #[test]
    fn usability_label_none() {
        assert_eq!(usability_label(None), "?");
    }

    #[test]
    fn usability_label_unknown_string() {
        assert_eq!(usability_label(Some("random")), "?");
    }

    #[test]
    fn usability_label_empty_string() {
        assert_eq!(usability_label(Some("")), "?");
    }
}
