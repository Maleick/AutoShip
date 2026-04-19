//! Packet monitor panel — scrolling log of captured EQ network opcodes with detail sidebar.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Row, Table, Wrap},
};

use super::widgets::{panel, themed_header_row};
use crate::tui::app::App;

const SIDEBAR_WIDTH: u16 = 44;

/// Draw the packet monitor screen — scrolling opcode log with detail sidebar.
pub fn draw_packet_monitor(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    // Determine layout: main pane + sidebar or stacked
    let use_sidebar = area.width > 100;

    if use_sidebar {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(50),
                Constraint::Length(1),
                Constraint::Length(SIDEBAR_WIDTH),
            ])
            .split(area);

        draw_packet_stream(frame, cols[0], app);
        draw_packet_detail(frame, cols[2], app);
    } else {
        // Vertical stacking for narrow terminals
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(15)])
            .split(area);

        draw_packet_stream(frame, rows[0], app);
        draw_packet_detail(frame, rows[1], app);
    }
}

/// Draw the main packet stream table.
fn draw_packet_stream(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let state = &app.packet_monitor_state;

    let border_style = if app.is_panel_focused(crate::tui::app::ActivePanel::PacketMonitorLog) {
        t.border_active
    } else {
        t.border_primary
    };

    let filtered = state.filtered_packets();
    let captured_count = state.packets.len();
    let peak_rate = 12482; // TODO: Calculate from timestamps

    let title = format!(
        " Packet Stream · {} captured · {}/s peak ",
        captured_count, peak_rate
    );
    let footer = " ↑↓ select  ·  / filter  ·  p pause  ·  space mark  ·  e export ";

    let blk = panel(title, border_style, t).footer(
        Line::from(Span::styled(footer, Style::default().fg(t.text_secondary))).right_aligned(),
    );

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
            area,
        );
    } else {
        let header = themed_header_row(&["", "Time", "Dir", "Opcode", "Size", "Payload"], t);

        // Calculate visible rows (area height minus borders, header, and filter line)
        let visible_rows = area.height.saturating_sub(6) as usize;
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
                let is_selected = false; // TODO: Track selected index in state

                let cursor = if is_selected {
                    Span::styled(
                        "▶",
                        Style::default()
                            .fg(t.text_accent)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::raw(" ")
                };

                let ts = format_timestamp(pkt.timestamp_ms);
                let (dir_label, dir_style) = match pkt.direction {
                    textquest_common::ipc::PacketDirection::Outbound => {
                        ("S→C", Style::default().fg(t.text_accent)) // cyan
                    }
                    textquest_common::ipc::PacketDirection::Inbound => {
                        ("C→S", Style::default().fg(ratatui::style::Color::Green)) // green
                    }
                };

                let opcode_name = format_opcode_name(pkt.opcode);
                let opcode_style = if is_selected {
                    Style::default()
                        .fg(t.text_bright)
                        .bg(t.text_accent)
                        .add_modifier(Modifier::REVERSED)
                } else {
                    get_opcode_color_style(pkt.opcode, t)
                };

                let hex_preview = format_hex_preview(pkt.opcode);

                Row::new(vec![
                    cursor,
                    Span::styled(ts, Style::default().fg(t.text_secondary)),
                    Span::styled(dir_label, dir_style),
                    Span::styled(opcode_name, opcode_style),
                    Span::styled(
                        format!("{}", pkt.payload_size),
                        Style::default().fg(t.text_bright),
                    ),
                    Span::styled(hex_preview, Style::default().fg(t.text_muted)),
                ])
            })
            .collect();

        let widths = [
            Constraint::Length(2),
            Constraint::Length(13),
            Constraint::Length(4),
            Constraint::Length(22),
            Constraint::Length(5),
            Constraint::Length(50),
        ];

        let table = Table::new(rows, widths).header(header).block(blk);

        frame.render_widget(table, area);

        // Draw filter echo line below the table
        let filter_line = format_filter_line(state, &filtered);
        let filter_area = Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(2),
            width: area.width,
            height: 1,
        };
        frame.render_widget(Paragraph::new(filter_line), filter_area);
    }
}

/// Draw the packet detail sidebar.
fn draw_packet_detail(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let state = &app.packet_monitor_state;
    let border_style = t.border_active;

    let title = " Packet · detail ";
    let blk = panel(title, border_style, t);

    // Get the selected packet (or first if none selected)
    let filtered = state.filtered_packets();
    if filtered.is_empty() {
        frame.render_widget(
            Paragraph::new("No packet selected")
                .style(Style::default().fg(t.text_muted))
                .block(blk),
            area,
        );
        return;
    }

    // TODO: Get actual selected index from state
    let pkt = &filtered[0];

    let mut lines = vec![
        Line::from(vec![
            Span::styled("captured ", Style::default().fg(t.text_secondary)),
            Span::styled(
                format_timestamp(pkt.timestamp_ms),
                Style::default().fg(t.text_bright),
            ),
            Span::raw("   "),
            Span::styled("dir ", Style::default().fg(t.text_secondary)),
            Span::styled(
                match pkt.direction {
                    textquest_common::ipc::PacketDirection::Outbound => "S→C",
                    textquest_common::ipc::PacketDirection::Inbound => "C→S",
                },
                Style::default().fg(t.text_accent),
            ),
        ]),
        Line::from(vec![
            Span::styled("opcode   ", Style::default().fg(t.text_secondary)),
            Span::styled(
                format_opcode_name(pkt.opcode),
                Style::default()
                    .fg(t.text_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                format!("(0x{:04X})", pkt.opcode),
                Style::default().fg(t.text_muted),
            ),
        ]),
        Line::from(vec![
            Span::styled("size     ", Style::default().fg(t.text_secondary)),
            Span::styled(
                format!("{} bytes", pkt.payload_size),
                Style::default().fg(t.text_bright),
            ),
        ]),
        Line::from(vec![
            Span::styled("client   ", Style::default().fg(t.text_secondary)),
            Span::styled(
                format_client_name(pkt.client_id),
                Style::default().fg(t.text_accent),
            ),
            Span::raw(" "),
            Span::styled(
                format!("(pid {})", pkt.client_id),
                Style::default().fg(t.text_muted),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "decoded",
            Style::default().fg(t.text_secondary),
        )]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("caster_id", Style::default().fg(t.text_accent)),
            Span::raw("    = "),
            Span::styled("4826", Style::default().fg(t.text_bright)),
            Span::raw("  "),
            Span::styled("// Sylunariel", Style::default().fg(t.text_muted)),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("target_id", Style::default().fg(t.text_accent)),
            Span::raw("    = "),
            Span::styled("4829", Style::default().fg(t.text_bright)),
            Span::raw("  "),
            Span::styled("// Thurgrek", Style::default().fg(t.text_muted)),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("spell_id", Style::default().fg(t.text_accent)),
            Span::raw("     = "),
            Span::styled("1000", Style::default().fg(t.text_bright)),
            Span::raw("  "),
            Span::styled("// Complete Healing", Style::default().fg(t.text_muted)),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("cast_time_ms", Style::default().fg(t.text_accent)),
            Span::raw(" = "),
            Span::styled("10000", Style::default().fg(t.text_bright)),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("gem_slot", Style::default().fg(t.text_accent)),
            Span::raw("     = "),
            Span::styled("7", Style::default().fg(t.text_bright)),
        ]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "hex",
            Style::default().fg(t.text_secondary),
        )]),
        Line::from(vec![
            Span::styled("0000", Style::default().fg(t.text_muted)),
            Span::raw("  "),
            Span::raw(format_hex_preview(pkt.opcode)),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(blk).wrap(Wrap { trim: false }),
        area,
    );
}

/// Get the color style for an opcode based on its type.
fn get_opcode_color_style(opcode: u16, t: &ratatui::style::Theme) -> Style {
    let name = format_opcode_name(opcode);
    if name.contains("HP") || name.contains("Mana") {
        Style::default().fg(ratatui::style::Color::Yellow) // amber
    } else if name.contains("Cast") || name.contains("MemorizeSpell") {
        Style::default().fg(t.text_accent)
    } else if name.contains("Damage") {
        Style::default().fg(ratatui::style::Color::Red)
    } else {
        Style::default().fg(t.text_bright)
    }
}

/// Format opcode as name (currently hex, TODO: resolve to actual names).
fn format_opcode_name(opcode: u16) -> String {
    // TODO: Resolve opcode to actual spell/packet names
    format!("0x{:04X}", opcode)
}

/// Format hex preview (first 16 bytes, currently placeholder).
fn format_hex_preview(opcode: u16) -> String {
    // TODO: Get actual payload bytes
    format!(
        "{:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}",
        (opcode >> 8) & 0xFF,
        opcode & 0xFF,
        0x00,
        0x01,
        0x02,
        0x03,
        0x04,
        0x05,
        0x06,
        0x07,
        0x08,
        0x09,
        0x0A,
        0x0B,
        0x0C,
        0x0D
    )
}

/// Format client name (currently uses PID, TODO: resolve to character names).
fn format_client_name(client_id: u32) -> String {
    // TODO: Resolve client_id to character name
    format!("Client{}", client_id)
}

/// Format the filter echo line.
fn format_filter_line<'a>(
    state: &'a crate::tui::app::PacketMonitorState,
    filtered: &'a [crate::tui::state::PacketRecord],
) -> Line<'a> {
    let spans = vec![
        Span::styled("filter: ", Style::default().fg(ratatui::style::Color::Cyan)),
        Span::raw("op=* · "),
        Span::styled("dir=any", Style::default().fg(ratatui::style::Color::Cyan)),
        Span::raw(" · "),
        Span::styled("client=*", Style::default().fg(ratatui::style::Color::Cyan)),
        Span::raw(" · "),
        Span::styled("▶ live", Style::default().fg(ratatui::style::Color::Green)),
        Span::raw("  "),
        Span::styled(
            format!(
                "({} rows · {} since 11:42)",
                filtered.len(),
                state.packets.len()
            ),
            Style::default().fg(ratatui::style::Color::DarkGray),
        ),
    ];
    Line::from(spans)
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
