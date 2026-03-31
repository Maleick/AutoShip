//! Overview screen — fleet grid with collapsible operational sections.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph, Row, Table, Wrap},
};

use super::widgets::{hp_color, panel, stand_state_color, themed_header_row};
use crate::tui::app::{ActivePanel, App};

pub fn draw_dashboard(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = if area.width < 110 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
            .split(area)
    };

    draw_dashboard_grid(frame, chunks[0], app);
    draw_dashboard_sidebar(frame, chunks[1], app);
}

// ─── Character grid ──────────────────────────────────────────────────────────

fn draw_dashboard_grid(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let visible = app.visible_clients();
    let title = match app.active_group {
        Some(idx) => {
            if let Some(g) = app.groups.get(idx) {
                format!(" Fleet — G{} {} ({}) ", g.id, g.name, visible.len())
            } else {
                format!(" Fleet — Group {} ({}) ", idx + 1, visible.len())
            }
        }
        None => format!(" Fleet ({}) ", app.clients.len()),
    };

    let border_style = if app.is_panel_focused(ActivePanel::OverviewRoster) {
        t.border_active
    } else {
        t.border_primary
    };
    let blk = panel(title.as_str(), border_style, t);

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
                let status_label = if client.client_status.is_empty() {
                    format!("PID {}", client.pid)
                } else {
                    client.client_status.clone()
                };
                let status_color = if client.client_status.contains("error")
                    || client.client_status.contains("Lost")
                {
                    t.hp_low
                } else {
                    t.text_muted
                };
                Row::new(vec![
                    ratatui::widgets::Cell::from(marker).style(Style::default().fg(t.text_accent)),
                    ratatui::widgets::Cell::from(status_label)
                        .style(Style::default().fg(status_color)),
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
                Constraint::Length(2),
                Constraint::Min(14),
                Constraint::Length(4),
                Constraint::Length(3),
                Constraint::Length(5),
                Constraint::Length(5),
                Constraint::Length(8),
                Constraint::Min(12),
            ],
        )
        .header(header)
        .block(blk),
        area,
    );
}

// ─── Sidebar ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum OverviewSectionKind {
    Groups,
    Filters,
    Combat,
    Session,
}

fn draw_dashboard_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let sections = overview_sections(app);
    if sections.is_empty() {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            sections
                .iter()
                .map(|(_, constraint)| *constraint)
                .collect::<Vec<_>>(),
        )
        .split(area);

    for ((section, _), chunk) in sections.iter().zip(chunks.iter()) {
        match section {
            OverviewSectionKind::Groups => {
                draw_group_health_gauges(frame, *chunk, app, app.overview_state.groups_collapsed)
            }
            OverviewSectionKind::Filters => {
                draw_filter_summary(frame, *chunk, app, app.overview_state.filters_collapsed)
            }
            OverviewSectionKind::Combat => {
                draw_combat_status(frame, *chunk, app, app.overview_state.combat_collapsed)
            }
            OverviewSectionKind::Session => {
                draw_session_stats(frame, *chunk, app, app.overview_state.session_collapsed)
            }
        }
    }
}

fn overview_sections(app: &App) -> Vec<(OverviewSectionKind, Constraint)> {
    let mut sections = Vec::new();

    if app.overview_state.show_groups {
        sections.push((
            OverviewSectionKind::Groups,
            if app.overview_state.groups_collapsed {
                Constraint::Length(3)
            } else {
                Constraint::Min(7)
            },
        ));
    }

    if app.overview_state.show_filters {
        sections.push((
            OverviewSectionKind::Filters,
            if app.overview_state.filters_collapsed {
                Constraint::Length(3)
            } else {
                Constraint::Length(6)
            },
        ));
    }

    sections.push((
        OverviewSectionKind::Combat,
        if app.overview_state.combat_collapsed {
            Constraint::Length(3)
        } else {
            Constraint::Length(7)
        },
    ));
    sections.push((
        OverviewSectionKind::Session,
        if app.overview_state.session_collapsed {
            Constraint::Length(3)
        } else {
            Constraint::Min(9)
        },
    ));

    sections
}

fn section_title(label: &str, key_hint: Option<&str>, collapsed: bool) -> String {
    let icon = if collapsed { "▶" } else { "▼" };
    match key_hint {
        Some(key) => format!(" {} [{}] {} ", label, key, icon),
        None => format!(" {} {} ", label, icon),
    }
}

/// HP bars using ratatui's `Gauge` widget — one per visible character.
fn draw_group_health_gauges(frame: &mut Frame, area: Rect, app: &App, collapsed: bool) {
    let t = &app.theme;
    let border_style = if app.is_panel_focused(ActivePanel::OverviewGroups) {
        t.border_active
    } else {
        t.border_warn
    };
    let title = section_title("Groups", Some("g"), collapsed);
    let blk = panel(title.as_str(), border_style, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let visible = app.visible_clients();
    if collapsed {
        let summary = format!("{} visible | {}", visible.len(), app.group_focus_label());
        frame.render_widget(
            Paragraph::new(summary).style(Style::default().fg(t.text_muted)),
            inner,
        );
        return;
    }

    if visible.is_empty() || inner.height == 0 {
        frame.render_widget(
            Paragraph::new("No characters connected").style(Style::default().fg(t.text_muted)),
            inner,
        );
        return;
    }

    let n = visible.len().min(inner.height as usize);
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

            let marker_area = Rect {
                x: row.x,
                y: row.y,
                width: 2,
                height: 1,
            };
            let gauge_area = Rect {
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
            let label = if client.client_status.is_empty() {
                format!("  PID {} …", client.pid)
            } else {
                format!("  {}", client.client_status)
            };
            let color = if client.client_status.contains("error")
                || client.client_status.contains("Lost")
            {
                t.hp_low
            } else {
                t.text_muted
            };
            frame.render_widget(
                Paragraph::new(Span::styled(label, Style::default().fg(color))),
                row,
            );
        }
    }
}

fn draw_filter_summary(frame: &mut Frame, area: Rect, app: &App, collapsed: bool) {
    let t = &app.theme;
    let border_style = if app.is_panel_focused(ActivePanel::OverviewFilters) {
        t.border_active
    } else {
        t.border_dim
    };
    let title = section_title("Filters", Some("v"), collapsed);
    let blk = panel(title.as_str(), border_style, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let search = if app.spawns_state.spawn_filter.is_empty() {
        String::from("none")
    } else {
        app.spawns_state.spawn_filter.clone()
    };

    let lines = if collapsed {
        vec![Line::from(vec![
            Span::styled(
                format!("{} ", app.spawns_state.spawn_type_filter.label()),
                Style::default()
                    .fg(t.text_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("| {} | Z ±{:.0}", search, app.map_state.z_filter_range),
                t.text_muted,
            ),
        ])]
    } else {
        vec![
            Line::from(vec![
                Span::styled("Type   ", Style::default().fg(t.text_muted)),
                Span::styled(
                    app.spawns_state.spawn_type_filter.label(),
                    Style::default()
                        .fg(t.text_accent)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Search ", Style::default().fg(t.text_muted)),
                Span::styled(search, Style::default().fg(t.text_normal)),
            ]),
            Line::from(vec![
                Span::styled("Z slice", Style::default().fg(t.text_muted)),
                Span::styled(
                    format!(" ±{:.0}", app.map_state.z_filter_range),
                    Style::default().fg(t.text_highlight),
                ),
            ]),
            Line::from(vec![
                Span::styled("/", Style::default().fg(t.text_accent)),
                Span::styled(" search  ", Style::default().fg(t.text_muted)),
                Span::styled("f", Style::default().fg(t.text_accent)),
                Span::styled(" type  ", Style::default().fg(t.text_muted)),
                Span::styled("+/-", Style::default().fg(t.text_accent)),
                Span::styled(" depth", Style::default().fg(t.text_muted)),
            ]),
        ]
    };

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

/// Combat status summary — MA/MT, operating mode, CH chain status.
fn draw_combat_status(frame: &mut Frame, area: Rect, app: &App, collapsed: bool) {
    let t = &app.theme;
    let border_style = if app.is_panel_focused(ActivePanel::OverviewCombat) {
        t.border_active
    } else {
        t.border_server
    };
    let title = section_title("Combat", None, collapsed);

    let mode_str = format!("{}", app.operating_mode);
    let mode_color = match mode_str.as_str() {
        "Camp" => t.mode_camp,
        "Hunt" => t.mode_hunt,
        _ => t.text_muted,
    };

    let ma_str = app.main_assist.as_deref().unwrap_or("—");
    let mt_str = app.main_tank.as_deref().unwrap_or("—");

    let lines = if collapsed {
        vec![Line::from(vec![
            Span::styled(
                &mode_str,
                Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  |  MA ", Style::default().fg(t.text_muted)),
            Span::styled(ma_str, Style::default().fg(t.text_highlight)),
            Span::styled("  |  MT ", Style::default().fg(t.text_muted)),
            Span::styled(mt_str, Style::default().fg(t.text_highlight)),
        ])]
    } else {
        let mut lines = vec![
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
                Span::styled("  HlCx ", Style::default().fg(t.text_muted)),
                Span::styled(
                    if app.heal_cancel_enabled { "ON" } else { "off" },
                    Style::default().fg(if app.heal_cancel_enabled {
                        t.hp_high
                    } else {
                        t.text_muted
                    }),
                ),
            ]),
        ];

        if let Some(ch) = &app.ch_chain_status {
            let adaptive_str = if ch.is_adaptive { "adaptive" } else { "fixed" };
            lines.push(Line::from(vec![
                Span::styled("CH   ", Style::default().fg(t.text_muted)),
                Span::styled(
                    format!(
                        "{}× {:.1}s {} tgt={}",
                        ch.members, ch.interval_secs, adaptive_str, ch.target_id
                    ),
                    Style::default().fg(t.text_highlight),
                ),
            ]));
        }

        lines
    };

    frame.render_widget(
        Paragraph::new(lines).block(panel(title.as_str(), border_style, t)),
        area,
    );
}

fn draw_session_stats(frame: &mut Frame, area: Rect, app: &App, collapsed: bool) {
    let t = &app.theme;
    let border_style = if app.is_panel_focused(ActivePanel::OverviewSession) {
        t.border_active
    } else {
        t.border_primary
    };
    let title = section_title("Session", None, collapsed);

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
    top_items.truncate(3);

    let lines = if collapsed {
        vec![Line::from(vec![
            Span::styled(&duration_str, Style::default().fg(t.text_accent)),
            Span::styled("  |  XP ", Style::default().fg(t.text_muted)),
            Span::styled(
                db.total_xp_events.to_string(),
                Style::default().fg(t.hp_high),
            ),
            Span::styled("  |  P ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{:.0}", total_plat),
                Style::default().fg(t.text_highlight),
            ),
        ])]
    } else {
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

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Server  ", Style::default().fg(t.text_muted)),
            Span::styled(app.display_server(), Style::default().fg(t.text_server)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Theme   ", Style::default().fg(t.text_muted)),
            Span::styled(app.theme_kind.label(), Style::default().fg(t.text_accent)),
            Span::styled("  Refresh ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{}ms", app.refresh_rate_ms),
                Style::default().fg(t.text_accent),
            ),
        ]));

        lines
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(title.as_str(), border_style, t))
            .wrap(Wrap { trim: true }),
        area,
    );
}
