//! Navigation screen — per-character nav status + commands reference panel.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Row, Table},
};

use super::widgets::{panel, themed_header_row};
use crate::tui::app::App;

pub fn draw_navigation_screen(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // ── Left: per-character nav status ────────────────────────────────
    let blk = panel(" Navigation Status ", t.border_primary, t);
    let visible = app.visible_clients();

    if visible.is_empty() {
        frame.render_widget(
            Paragraph::new("No characters connected")
                .block(blk)
                .style(Style::default().fg(t.text_muted)),
            cols[0],
        );
    } else {
        let header = themed_header_row(vec!["", "Character", "Zone", "Status", "Destination"], t);

        let rows: Vec<Row> = visible
            .iter()
            .enumerate()
            .map(|(i, client)| {
                let is_sel = i == app.nav_state.nav_selected;
                let marker = if is_sel { "▶" } else { " " };
                let name = client
                    .local_player
                    .as_ref()
                    .map(|p| app.redact_name(&p.displayed_name).into_owned())
                    .unwrap_or_else(|| format!("PID {}", client.pid));

                let nav = app.nav_state.nav_statuses.get(&client.pid);
                let status = nav.map(|s| s.status.as_str()).unwrap_or("Idle");
                let dest = nav.map(|s| s.destination.as_str()).unwrap_or("—");

                let status_color = match status {
                    "Navigating" => t.text_highlight,
                    "Arrived" => t.hp_high,
                    "Stuck" => t.hp_low,
                    _ => t.text_muted,
                };

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
                    ratatui::widgets::Cell::from(client.zone_name.as_str())
                        .style(Style::default().fg(t.text_secondary)),
                    ratatui::widgets::Cell::from(status).style(Style::default().fg(status_color)),
                    ratatui::widgets::Cell::from(dest).style(Style::default().fg(t.text_accent)),
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
                    Constraint::Min(14),
                ],
            )
            .header(header)
            .block(blk),
            cols[0],
        );
    }

    // ── Right: commands reference ──────────────────────────────────────
    let mode_str = format!("{}", app.operating_mode);
    let mode_color = match mode_str.as_str() {
        "Camp" => t.mode_camp,
        "Hunt" => t.mode_hunt,
        _ => t.text_normal,
    };
    let cmd_s = Style::default().fg(t.text_highlight);
    let lbl_s = Style::default().fg(t.text_secondary);

    let mut lines: Vec<Line<'_>> = vec![
        Line::from(Span::styled(
            "Operating Mode",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Mode: ", Style::default().fg(t.text_muted)),
            Span::styled(
                &mode_str,
                Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    if let Some(ma) = &app.main_assist {
        lines.push(Line::from(vec![
            Span::styled("  MA:   ", Style::default().fg(t.text_muted)),
            Span::styled(ma.as_str(), Style::default().fg(t.text_highlight)),
        ]));
    }
    if let Some(mt) = &app.main_tank {
        lines.push(Line::from(vec![
            Span::styled("  MT:   ", Style::default().fg(t.text_muted)),
            Span::styled(mt.as_str(), Style::default().fg(t.hp_low)),
        ]));
    }

    // ── Group Nav summary ──────────────────────────────────────────────
    if app.has_live_group_data() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Group Nav",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        let (live_groups, _) = app.build_live_groups();
        for group in &live_groups {
            let mut navigating = 0u32;
            let mut arrived = 0u32;
            let mut idle = 0u32;
            let mut dest: Option<&str> = None;
            let mut all_same_dest = true;

            for member_name in &group.member_names {
                if let Some(client) = app.find_client_by_name(member_name) {
                    if let Some(nav) = app.nav_state.nav_statuses.get(&client.pid) {
                        match nav.status.as_str() {
                            "Navigating" => navigating += 1,
                            "Arrived" => arrived += 1,
                            _ => idle += 1,
                        }
                        if !nav.destination.is_empty() && nav.destination != "\u{2014}" {
                            match dest {
                                None => dest = Some(nav.destination.as_str()),
                                Some(d) if d != nav.destination => all_same_dest = false,
                                _ => {}
                            }
                        }
                    } else {
                        idle += 1;
                    }
                }
            }

            let dest_str = if all_same_dest {
                dest.unwrap_or("\u{2014}")
            } else {
                "mixed"
            };

            let leader_display = app.redact_name(&group.leader);
            let status_color = if navigating > 0 {
                t.text_highlight
            } else if arrived > 0 {
                t.hp_high
            } else {
                t.text_muted
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<12}", leader_display),
                    Style::default().fg(t.text_normal),
                ),
                Span::styled(
                    format!("{}nav {}arr {}idl", navigating, arrived, idle),
                    Style::default().fg(status_color),
                ),
            ]));
            if dest_str != "\u{2014}" {
                lines.push(Line::from(vec![
                    Span::styled("    -> ", Style::default().fg(t.text_muted)),
                    Span::styled(dest_str, Style::default().fg(t.text_accent)),
                ]));
            }
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Nav Commands",
        Style::default()
            .fg(t.text_accent)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (cmd, desc) in &[
        (":mode camp ", "Camp mode"),
        (":mode hunt ", "Hunt mode"),
        (":camp start", "Start camp"),
        (":camp stop ", "Stop camp"),
        (":camp next ", "Next waypoint"),
        (":camp prev ", "Prev waypoint"),
    ] {
        lines.push(Line::from(vec![
            Span::styled(*cmd, cmd_s),
            Span::raw("  "),
            Span::styled(*desc, lbl_s),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Group Commands",
        Style::default()
            .fg(t.text_accent)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (cmd, desc) in &[
        (":invite <n>", "Invite to group"),
        (":accept    ", "Accept invite"),
        (":ma <name> ", "Main Assist"),
        (":mt <name> ", "Main Tank"),
    ] {
        lines.push(Line::from(vec![
            Span::styled(*cmd, cmd_s),
            Span::raw("  "),
            Span::styled(*desc, lbl_s),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines).block(panel(" Commands & Mode ", t.border_warn, t)),
        cols[1],
    );
}
