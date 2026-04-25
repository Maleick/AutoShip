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
    let filtered = filtered.as_slice();
    let captured_count = state.packets.len();
    let (current_rate, peak_rate) = state.packet_rates();

    let title = format!(
        " Packet Stream · {} captured · {}/s live · {}/s peak ",
        captured_count, current_rate, peak_rate
    );
    let blk = panel(title, border_style, t);

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

        // Outer border (2) + header row (1) + filter line (1) + inner padding (2) = 6 rows reserved.
        const PACKETS_CHROME_ROWS: u16 = 6;
        let visible_rows = area.height.saturating_sub(PACKETS_CHROME_ROWS) as usize;
        let total = filtered.len();
        let selected_idx = state.selected_index(total).unwrap_or(0);
        let skip = if state.auto_scroll {
            total.saturating_sub(visible_rows)
        } else {
            let selection_anchor = selected_idx.saturating_add(1).saturating_sub(visible_rows);
            selection_anchor.min(
                total
                    .saturating_sub(visible_rows)
                    .saturating_sub(state.scroll_offset),
            )
        };

        let rows: Vec<Row> = filtered
            .iter()
            .enumerate()
            .skip(skip)
            .take(visible_rows)
            .map(|(idx, pkt)| {
                let is_selected = idx == selected_idx;

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

                let opcode_name = opcode_hex_label(pkt.opcode);
                let opcode_style = if is_selected {
                    Style::default()
                        .fg(t.text_bright)
                        .bg(t.text_accent)
                        .add_modifier(Modifier::REVERSED)
                } else {
                    get_opcode_color_style(pkt.opcode, t)
                };

                let hex_preview = pkt.payload_preview();

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
        let filter_line = format_filter_line(state, filtered);
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

    let Some(pkt) = state.selected_packet() else {
        frame.render_widget(
            Paragraph::new("No packet selected")
                .style(Style::default().fg(t.text_muted))
                .block(blk),
            area,
        );
        return;
    };

    // Use the name resolved at capture time (stored in the record itself).
    let client_label = pkt.client_label();
    let (hex_lines, text_lines) = format_payload_dump(&pkt.payload, area.width.saturating_sub(4));

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
                opcode_hex_label(pkt.opcode),
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
            Span::styled(client_label, Style::default().fg(t.text_accent)),
            Span::raw(" "),
            Span::styled(
                format!("(pid {})", pkt.client_id),
                Style::default().fg(t.text_muted),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            "hex",
            Style::default().fg(t.text_secondary),
        )]),
    ];

    lines.extend(hex_lines.into_iter().map(|line| {
        let mut parts = line.splitn(2, ": ");
        let offset = parts.next().unwrap_or("0000");
        let bytes = parts.next().unwrap_or("");
        Line::from(vec![
            Span::styled(offset.to_string(), Style::default().fg(t.text_muted)),
            Span::raw("  "),
            Span::raw(bytes.to_string()),
        ])
    }));
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![Span::styled(
        "text",
        Style::default().fg(t.text_secondary),
    )]));
    if text_lines.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "  (empty payload)",
            Style::default().fg(t.text_muted),
        )]));
    } else {
        lines.extend(text_lines.into_iter().map(|line| {
            Line::from(vec![Span::styled(
                format!("  {line}"),
                Style::default().fg(t.text_bright),
            )])
        }));
    }

    frame.render_widget(
        Paragraph::new(lines).block(blk).wrap(Wrap { trim: false }),
        area,
    );
}

/// Get the color style for an opcode based on its type.
fn get_opcode_color_style(opcode: u16, t: &crate::tui::theme::Theme) -> Style {
    let name = opcode_hex_label(opcode);
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

/// EverQuest opcode name lookup table. Maps known opcodes to their canonical names.
const OPCODE_NAMES: &[(u16, &str)] = &[
    (0x0001, "OP_ZoneEntry"),
    (0x0002, "OP_ZoneSpawns"),
    (0x0004, "OP_ZoneDespawn"),
    (0x0006, "OP_PlayerProfile"),
    (0x0007, "OP_PlayerUpdate"),
    (0x0008, "OP_MobUpdate"),
    (0x0009, "OP_ClientUpdate"),
    (0x000A, "OP_PlayerMoveRequest"),
    (0x000B, "OP_NameRequest"),
    (0x000C, "OP_NameReply"),
    (0x0100, "OP_Attack"),
    (0x0101, "OP_Action"),
    (0x0102, "OP_CastSpell"),
    (0x0103, "OP_BeginCast"),
    (0x0104, "OP_SpellAction"),
    (0x0105, "OP_MeleeDamage"),
    (0x0106, "OP_SpellDamage"),
    (0x0107, "OP_HealSpell"),
    (0x0108, "OP_Death"),
    (0x0109, "OP_Skill"),
    (0x0200, "OP_Say"),
    (0x0201, "OP_Emote"),
    (0x0202, "OP_ChatMessage"),
    (0x0203, "OP_BroadcastMessage"),
    (0x0204, "OP_Shout"),
    (0x0205, "OP_AuctionMessage"),
    (0x0300, "OP_ItemLink"),
    (0x0301, "OP_ClickObject"),
    (0x0302, "OP_DropItem"),
    (0x0303, "OP_ItemActivity"),
    (0x0304, "OP_TradeRequest"),
    (0x0305, "OP_TradeAccept"),
    (0x0306, "OP_TradeLoot"),
    (0x0307, "OP_Loot"),
    (0x0308, "OP_LootAck"),
    (0x0400, "OP_Animation"),
    (0x0401, "OP_EmoteBroadcast"),
    (0x0402, "OP_Buff"),
    (0x0403, "OP_BuffDuration"),
    (0x0500, "OP_GroupInvite"),
    (0x0501, "OP_GroupFollow"),
    (0x0502, "OP_GroupDisband"),
    (0x0503, "OP_GroupUpdate"),
    (0x0504, "OP_GroupExpUpdate"),
    (0x0600, "OP_TargetRequest"),
    (0x0601, "OP_TargetUpdate"),
    (0x0602, "OP_HotkeysSet"),
    (0x0603, "OP_SysMessage"),
    (0x0700, "OP_ServerUpdate"),
    (0x0701, "OP_ServerTime"),
    (0x0702, "OP_ServerNotification"),
];

/// Format opcode as name or hex label. Looks up known EQ opcodes by code;
/// falls back to `0xNNNN` hex format for unknown opcodes.
fn opcode_hex_label(opcode: u16) -> String {
    OPCODE_NAMES
        .binary_search_by_key(&opcode, |&(code, _)| code)
        .ok()
        .and_then(|idx| OPCODE_NAMES.get(idx))
        .map(|&(_, name)| name.to_string())
        .unwrap_or_else(|| format!("0x{:04X}", opcode))
}

/// Format a one-line payload preview.
fn format_payload_preview(payload: &[u8]) -> String {
    if payload.is_empty() {
        return String::from("(empty)");
    }

    payload
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Format the filter echo line showing active filter state and capture stats.
fn format_filter_line<'a>(
    state: &'a crate::tui::app::PacketMonitorState,
    filtered: &'a [&'a crate::tui::state::PacketRecord],
) -> Line<'a> {
    let opcode_label = match state.filter_opcode {
        Some(op) => format!("op=0x{op:04X}"),
        None => String::from("op=*"),
    };
    let dir_label = match state.filter_direction {
        Some(textquest_common::ipc::PacketDirection::Outbound) => String::from("dir=S→C"),
        Some(textquest_common::ipc::PacketDirection::Inbound) => String::from("dir=C→S"),
        None => String::from("dir=any"),
    };
    let client_label = match state.filter_client_id {
        Some(pid) => {
            let label = state
                .resolved_client_label(pid)
                .unwrap_or("WIP")
                .to_owned();
            format!("client={label}")
        }
        None => String::from("client=*"),
    };
    let scroll_label = if state.auto_scroll {
        "▶ live"
    } else {
        "⏸ paused"
    };
    let scroll_color = if state.auto_scroll {
        ratatui::style::Color::Green
    } else {
        ratatui::style::Color::Yellow
    };

    // Show "N packets since capture started" using the first-packet timestamp.
    let since_label = if let Some(start_ms) = state.capture_start_ms {
        // Use the latest packet's timestamp to compute elapsed duration.
        let elapsed_ms = state
            .packets
            .last()
            .map(|p| p.timestamp_ms.saturating_sub(start_ms))
            .unwrap_or(0);
        let elapsed_secs = elapsed_ms / 1_000;
        if elapsed_secs >= 3600 {
            format!(
                "{} pkts in {:02}:{:02}:{:02}",
                state.packets.len(),
                elapsed_secs / 3600,
                (elapsed_secs % 3600) / 60,
                elapsed_secs % 60
            )
        } else {
            format!(
                "{} pkts in {:02}:{:02}",
                state.packets.len(),
                elapsed_secs / 60,
                elapsed_secs % 60
            )
        }
    } else {
        format!("{} pkts", state.packets.len())
    };

    let spans = vec![
        Span::styled("filter: ", Style::default().fg(ratatui::style::Color::Cyan)),
        Span::raw(opcode_label),
        Span::raw(" · "),
        Span::styled(dir_label, Style::default().fg(ratatui::style::Color::Cyan)),
        Span::raw(" · "),
        Span::styled(
            client_label,
            Style::default().fg(ratatui::style::Color::Cyan),
        ),
        Span::raw(" · "),
        Span::styled(scroll_label, Style::default().fg(scroll_color)),
        Span::raw("  "),
        Span::styled(
            format!("({} shown · {})", filtered.len(), since_label),
            Style::default().fg(ratatui::style::Color::DarkGray),
        ),
    ];
    Line::from(spans)
}

fn format_payload_dump(payload: &[u8], width: u16) -> (Vec<String>, Vec<String>) {
    let hex_bytes_per_line = usize::from((width.saturating_sub(10) / 3).max(4));
    let text_bytes_per_line = usize::from(width.saturating_sub(4).max(8));

    let hex_lines = if payload.is_empty() {
        vec![String::from("0000: (empty)")]
    } else {
        payload
            .chunks(hex_bytes_per_line)
            .enumerate()
            .map(|(idx, chunk)| {
                let bytes = chunk
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("{:04X}: {bytes}", idx * hex_bytes_per_line)
            })
            .collect()
    };

    let text_lines = payload
        .chunks(text_bytes_per_line)
        .map(|chunk| {
            chunk
                .iter()
                .map(|byte| match byte {
                    b' '..=b'~' => char::from(*byte),
                    _ => '.',
                })
                .collect::<String>()
        })
        .collect();

    (hex_lines, text_lines)
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

    #[test]
    fn format_payload_preview_uses_real_bytes() {
        assert_eq!(
            format_payload_preview(&[0x34, 0x12, 0xAA, 0xFF]),
            "34 12 AA FF"
        );
    }

    #[test]
    fn format_payload_dump_includes_hex_and_ascii() {
        let (hex, text) = format_payload_dump(b"AB\x01cd", 32);
        assert_eq!(hex[0], "0000: 41 42 01 63 64");
        assert_eq!(text[0], "AB.cd");
    }
}
