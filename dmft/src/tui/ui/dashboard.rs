//! Dashboard screen — character grid + sidebar with HP gauges, session stats, server info.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph, Row, Table},
};

use super::widgets::{hp_color, panel, stand_state_color, themed_header_row};
use crate::tui::app::App;

pub fn draw_dashboard(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    // Adaptive split: narrow terminals (< 100 cols) get 70/30, wide terminals get 60/40
    let (grid_pct, sidebar_pct) = if area.width < 100 { (70, 30) } else { (60, 40) };
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(grid_pct),
            Constraint::Percentage(sidebar_pct),
        ])
        .split(area);

    draw_dashboard_grid(frame, cols[0], app);
    draw_dashboard_sidebar(frame, cols[1], app);
}

// ─── Character grid ──────────────────────────────────────────────────────────

fn draw_dashboard_grid(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let visible = app.visible_clients();
    let title = match app.active_group {
        Some(idx) => format!(
            " G{} {} ({}) ",
            app.groups[idx].id,
            app.groups[idx].name,
            visible.len()
        ),
        None => format!(" Characters ({}) ", app.clients.len()),
    };

    let blk = panel(title.as_str(), t.border_primary, t);

    if visible.is_empty() {
        frame.render_widget(
            Paragraph::new("No characters connected")
                .block(blk)
                .style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
    }

    let header = themed_header_row(
        vec!["", "Name", "Cls", "Lv", "HP%", "MP%", "State", "Zone"],
        t,
    );

    let rows: Vec<Row> = visible
        .iter()
        .map(|client| {
            let global_idx = app
                .clients
                .iter()
                .position(|c| c.pid == client.pid)
                .unwrap_or(usize::MAX);
            let is_sel = global_idx == app.selected_client;
            let marker = if is_sel { "▶" } else { " " };

            if let Some(player) = &client.local_player {
                let hp_pct = player.hp_pct();
                let mana_pct = player.mana_pct();
                let name = app.redact_name(&player.displayed_name).into_owned();

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
                    ratatui::widgets::Cell::from(player.class_str())
                        .style(Style::default().fg(t.text_accent)),
                    ratatui::widgets::Cell::from(player.level.to_string())
                        .style(Style::default().fg(t.text_secondary)),
                    ratatui::widgets::Cell::from(format!("{:.0}%", hp_pct))
                        .style(Style::default().fg(hp_color(hp_pct, t))),
                    ratatui::widgets::Cell::from(if player.mana_max > 0 {
                        format!("{:.0}%", mana_pct)
                    } else {
                        "-".into()
                    })
                    .style(Style::default().fg(t.mana_color)),
                    ratatui::widgets::Cell::from(player.stand_state.label())
                        .style(Style::default().fg(stand_state_color(&player.stand_state, t))),
                    ratatui::widgets::Cell::from(client.zone_name.as_str())
                        .style(Style::default().fg(t.text_muted)),
                ])
                .style(row_style)
            } else {
                Row::new(vec![
                    ratatui::widgets::Cell::from(marker).style(Style::default().fg(t.text_accent)),
                    ratatui::widgets::Cell::from(format!("PID {}", client.pid))
                        .style(Style::default().fg(t.text_muted)),
                    ratatui::widgets::Cell::from("-"),
                    ratatui::widgets::Cell::from("-"),
                    ratatui::widgets::Cell::from("-"),
                    ratatui::widgets::Cell::from("-"),
                    ratatui::widgets::Cell::from("-"),
                    ratatui::widgets::Cell::from(client.zone_name.as_str()),
                ])
            }
        })
        .collect();

    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(2), // marker
                Constraint::Min(14),   // Name
                Constraint::Length(4), // Class
                Constraint::Length(3), // Level
                Constraint::Length(5), // HP%
                Constraint::Length(5), // MP%
                Constraint::Length(8), // State
                Constraint::Min(12),   // Zone
            ],
        )
        .header(header)
        .block(blk),
        area,
    );
}

// ─── Sidebar ─────────────────────────────────────────────────────────────────

fn draw_dashboard_sidebar(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    // Adaptive sidebar: tall terminals show 4 panels, short terminals collapse server info
    let show_combat = area.height >= 24;
    let chunks = if show_combat {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(6),     // Group health (flexible, gets leftover)
                Constraint::Length(5),   // Combat status
                Constraint::Min(8),     // Session stats (flexible)
                Constraint::Length(6),   // Server info
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(6),
                Constraint::Min(8),
                Constraint::Length(4),
            ])
            .split(area)
    };

    if show_combat {
        draw_group_health_gauges(frame, chunks[0], app);
        draw_combat_status(frame, chunks[1], app);
        draw_session_stats(frame, chunks[2], app);
        draw_server_info(frame, chunks[3], app);
    } else {
        draw_group_health_gauges(frame, chunks[0], app);
        draw_session_stats(frame, chunks[1], app);
        draw_server_info(frame, chunks[2], app);
    }
}

/// HP bars using ratatui's `Gauge` widget — one per visible character.
fn draw_group_health_gauges(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let blk = panel(" Group Health ", t.border_warn, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let visible = app.visible_clients();
    if visible.is_empty() {
        return;
    }

    let n = visible.len().min(inner.height as usize);
    if n == 0 {
        return;
    }

    let row_heights: Vec<Constraint> = (0..n).map(|_| Constraint::Length(1)).collect();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_heights)
        .split(inner);

    for (i, client) in visible.iter().enumerate() {
        if i >= rows.len() {
            break;
        }
        let row = rows[i];

        let global_idx = app
            .clients
            .iter()
            .position(|c| c.pid == client.pid)
            .unwrap_or(usize::MAX);
        let is_sel = global_idx == app.selected_client;

        if let Some(player) = &client.local_player {
            let hp_pct = player.hp_pct().clamp(0.0, 100.0);
            let name = app.redact_name(&player.displayed_name).into_owned();
            let color = hp_color(hp_pct, t);

            // Selection indicator prefix (1 char)
            let marker_area = ratatui::layout::Rect {
                x: row.x,
                y: row.y,
                width: 2,
                height: 1,
            };
            let gauge_area = ratatui::layout::Rect {
                x: row.x + 2,
                y: row.y,
                width: row.width.saturating_sub(2),
                height: 1,
            };

            let marker_style = if is_sel {
                Style::default()
                    .fg(t.text_accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text_muted)
            };
            frame.render_widget(
                Paragraph::new(if is_sel { "▶ " } else { "  " }).style(marker_style),
                marker_area,
            );

            let label_str = format!("{:<12} {:>3.0}%", name, hp_pct);
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(color).bg(t.bar_empty))
                .percent(hp_pct as u16)
                .label(label_str)
                .style(Style::default().fg(color));

            frame.render_widget(gauge, gauge_area);
        } else {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    format!("  PID {} …", client.pid),
                    Style::default().fg(t.text_muted),
                )),
                row,
            );
        }
    }
}

/// Combat status summary — MA/MT, operating mode, CH chain status.
fn draw_combat_status(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;

    let mode_str = format!("{}", app.operating_mode);
    let mode_color = match mode_str.as_str() {
        "Camp" => t.mode_camp,
        "Hunt" => t.mode_hunt,
        _ => t.text_muted,
    };

    let ma_str = app
        .main_assist
        .as_deref()
        .unwrap_or("—");
    let mt_str = app
        .main_tank
        .as_deref()
        .unwrap_or("—");

    let lines = vec![
        Line::from(vec![
            Span::styled("Mode ", Style::default().fg(t.text_muted)),
            Span::styled(
                &mode_str,
                Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  MA ", Style::default().fg(t.text_muted)),
            Span::styled(ma_str, Style::default().fg(t.text_highlight)),
        ]),
        Line::from(vec![
            Span::styled("MT   ", Style::default().fg(t.text_muted)),
            Span::styled(mt_str, Style::default().fg(t.text_highlight)),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(panel(" Combat ", t.border_active, t)),
        area,
    );
}

fn draw_session_stats(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let db = &app.loot_database;
    let elapsed = app.session_start.elapsed();
    let hours = elapsed.as_secs() as f64 / 3600.0;

    let secs = elapsed.as_secs();
    let duration_str = format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    );

    let xp_per_hour = if hours > 0.01 {
        format!("{:.0}", db.total_xp_events as f64 / hours)
    } else {
        "-".into()
    };
    let xp_15min = {
        let rate = db.xp_rate_windowed(std::time::Duration::from_secs(900));
        if rate > 0.01 {
            format!("{:.0}/hr", rate)
        } else {
            "-".into()
        }
    };

    let total_plat = db.total_plat as f64
        + db.total_gold as f64 / 10.0
        + db.total_silver as f64 / 100.0
        + db.total_copper as f64 / 1000.0;
    let plat_per_hour = if hours > 0.01 {
        format!("{:.1}", total_plat / hours)
    } else {
        "-".into()
    };
    let total_kills: u32 = db.kills.values().sum();

    let mut top_items: Vec<(&String, &u32)> = db.items.iter().collect();
    top_items.sort_by(|a, b| b.1.cmp(a.1));
    top_items.truncate(5);

    let mut lines: Vec<Line<'_>> = vec![
        Line::from(vec![
            Span::styled("⏱ ", Style::default().fg(t.text_accent)),
            Span::styled(&duration_str, Style::default().fg(t.text_accent)),
        ]),
        Line::from(vec![
            Span::styled("XP  ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!(
                    "{} ({}/hr  15m:{})",
                    db.total_xp_events, xp_per_hour, xp_15min
                ),
                Style::default().fg(t.hp_high),
            ),
        ]),
        Line::from(vec![
            Span::styled("Pp  ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{:.0} ({}/hr)", total_plat, plat_per_hour),
                Style::default().fg(t.text_highlight),
            ),
        ]),
        Line::from(vec![
            Span::styled("☠   ", Style::default().fg(t.text_muted)),
            Span::styled(total_kills.to_string(), Style::default().fg(t.hp_low)),
            Span::styled("  Deaths: ", Style::default().fg(t.text_muted)),
            Span::styled(
                db.deaths.to_string(),
                Style::default().fg(if db.deaths > 0 {
                    t.hp_low
                } else {
                    t.text_muted
                }),
            ),
        ]),
    ];

    if !top_items.is_empty() {
        lines.push(Line::from(Span::styled(
            "── Loot ──",
            Style::default().fg(t.text_muted),
        )));
        for (name, count) in &top_items {
            let label: String = name.chars().take(18).collect();
            lines.push(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    format!("{}× ", count),
                    Style::default().fg(t.text_highlight),
                ),
                Span::styled(label, Style::default().fg(t.text_secondary)),
            ]));
        }
    }

    frame.render_widget(
        Paragraph::new(lines).block(panel(" Session ", t.border_primary, t)),
        area,
    );
}

fn draw_server_info(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let lines = vec![
        Line::from(vec![
            Span::styled("Server  ", Style::default().fg(t.text_muted)),
            Span::styled(app.display_server(), Style::default().fg(t.text_server)),
        ]),
        Line::from(vec![
            Span::styled("Clients ", Style::default().fg(t.text_muted)),
            Span::styled(
                app.clients.len().to_string(),
                Style::default().fg(t.text_highlight),
            ),
        ]),
        Line::from(vec![
            Span::styled("Refresh ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{}ms", app.refresh_rate_ms),
                Style::default().fg(t.text_accent),
            ),
        ]),
        Line::from(vec![
            Span::styled("Theme   ", Style::default().fg(t.text_muted)),
            Span::styled(app.theme_kind.label(), Style::default().fg(t.text_accent)),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(panel(" Server ", t.border_server, t)),
        area,
    );
}
