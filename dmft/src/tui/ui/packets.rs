//! Packet monitor panel — scrolling log of captured EQ network opcodes.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Row, Table, Wrap},
};

use super::widgets::{panel, themed_header_row};
use crate::tui::app::App;

/// Draw the packet monitor screen — scrolling opcode log with stats sidebar.
pub fn draw_packet_monitor(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let state = &app.packet_monitor_state;

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(60), Constraint::Length(30)])
        .split(area);

    // ── Left: packet log table ───────────────────────────────────────
    let border_style = if app.is_panel_focused(crate::tui::app::ActivePanel::PacketMonitorLog) {
        t.border_active
    } else {
        t.border_primary
    };

    let title = if state.paused {
        " Packet Monitor [PAUSED] "
    } else {
        " Packet Monitor "
    };
    let blk = panel(title, border_style, t);

    let filtered = state.filtered_packets();

    if filtered.is_empty() {
        let msg = if state.packets.is_empty() {
            "No packets captured yet — waiting for DLL hooks…"
        } else {
            "No packets match current filters"
        };
        frame.render_widget(
            Paragraph::new(msg)
                .style(Style::default().fg(t.text_muted))
                .block(blk),
            cols[0],
        );
    } else {
        let header = themed_header_row(&["Time", "Dir", "Opcode", "Size", "Client"], t);

        // Calculate visible rows (area height minus borders and header)
        let visible_rows = cols[0].height.saturating_sub(4) as usize;
        let total = filtered.len();
        let skip = if state.auto_scroll {
            total.saturating_sub(visible_rows)
        } else {
            total
                .saturating_sub(visible_rows)
                .saturating_sub(state.scroll_offset)
        };

        let rows: Vec<Row> = filtered
            .iter()
            .skip(skip)
            .take(visible_rows)
            .map(|pkt| {
                let ts = format_timestamp(pkt.timestamp_ms);
                let (dir_label, dir_style) = match pkt.direction {
                    dmft_common::ipc::PacketDirection::Outbound => {
                        ("→ OUT", Style::default().fg(t.text_highlight))
                    }
                    dmft_common::ipc::PacketDirection::Inbound => {
                        ("← IN ", Style::default().fg(t.text_accent))
                    }
                };
                Row::new(vec![
                    Span::styled(ts, Style::default().fg(t.text_secondary)),
                    Span::styled(dir_label, dir_style),
                    Span::styled(
                        format!("0x{:04X}", pkt.opcode),
                        Style::default().fg(t.text_bright),
                    ),
                    Span::styled(
                        format!("{}b", pkt.payload_size),
                        Style::default().fg(t.text_secondary),
                    ),
                    Span::styled(
                        format!("{}", pkt.client_id),
                        Style::default().fg(t.text_muted),
                    ),
                ])
            })
            .collect();

        let widths = [
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
        ];

        let table = Table::new(rows, widths).header(header).block(blk);

        frame.render_widget(table, cols[0]);
    }

    // ── Right: stats sidebar ─────────────────────────────────────────
    let stats_blk = panel(" Stats ", t.border_dim, t);

    let total_count = state.packets.len();
    let filtered_count = filtered.len();
    let inbound_count = state
        .packets
        .iter()
        .filter(|p| p.direction == dmft_common::ipc::PacketDirection::Inbound)
        .count();
    let outbound_count = total_count - inbound_count;

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Total: ", Style::default().fg(t.text_secondary)),
            Span::styled(
                format!("{total_count}"),
                Style::default()
                    .fg(t.text_bright)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("← IN:  ", Style::default().fg(t.text_accent)),
            Span::styled(
                format!("{inbound_count}"),
                Style::default().fg(t.text_bright),
            ),
        ]),
        Line::from(vec![
            Span::styled("→ OUT: ", Style::default().fg(t.text_highlight)),
            Span::styled(
                format!("{outbound_count}"),
                Style::default().fg(t.text_bright),
            ),
        ]),
        Line::raw(""),
    ];

    if let Some(op) = state.filter_opcode {
        lines.push(Line::from(vec![
            Span::styled("Filter: ", Style::default().fg(t.text_secondary)),
            Span::styled(
                format!("0x{op:04X}"),
                Style::default()
                    .fg(t.text_accent)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Shown: ", Style::default().fg(t.text_secondary)),
            Span::styled(
                format!("{filtered_count}"),
                Style::default().fg(t.text_bright),
            ),
        ]));
        lines.push(Line::raw(""));
    }

    lines.push(Line::from(vec![Span::styled(
        "─ Controls ─",
        Style::default()
            .fg(t.text_accent)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![
        Span::styled("Space", Style::default().fg(t.text_accent)),
        Span::styled(" Pause/Resume", Style::default().fg(t.text_secondary)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("↑/↓  ", Style::default().fg(t.text_accent)),
        Span::styled(" Scroll", Style::default().fg(t.text_secondary)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("c    ", Style::default().fg(t.text_accent)),
        Span::styled(" Clear log", Style::default().fg(t.text_secondary)),
    ]));

    frame.render_widget(
        Paragraph::new(lines)
            .block(stats_blk)
            .wrap(Wrap { trim: false }),
        cols[1],
    );
}

/// Format a millisecond timestamp as `HH:MM:SS`.
fn format_timestamp(ms: u64) -> String {
    let secs = (ms / 1000) % 86400;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_timestamp_wraps_at_day() {
        assert_eq!(format_timestamp(0), "00:00:00");
        assert_eq!(format_timestamp(3_661_000), "01:01:01");
        // 25 hours wraps to 01:00:00
        assert_eq!(format_timestamp(90_000_000), "01:00:00");
    }

    #[test]
    fn format_timestamp_seconds() {
        assert_eq!(format_timestamp(59_000), "00:00:59");
        assert_eq!(format_timestamp(60_000), "00:01:00");
    }
}
