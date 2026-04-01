//! Overview screen — fleet roster with adaptive operational sections.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Gauge, Paragraph, Row, Table, Wrap},
};

use super::widgets::{hp_color, panel, stand_state_color, themed_header_row};
use crate::eq::structs::{EqClass, StandState};
use crate::tui::app::{ActivePanel, App, ClientState};

pub fn draw_dashboard(frame: &mut Frame, area: Rect, app: &App) {
    let sections = overview_sections(app);
    let natural_sidebar_height = sections
        .iter()
        .map(|(_, constraint)| preferred_height(*constraint))
        .sum::<u16>();

    let chunks = if area.width < 118 {
        let sidebar_height = natural_sidebar_height
            .min(area.height.saturating_sub(10))
            .max(3);
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(sidebar_height)])
            .split(area)
    } else {
        let sidebar_width = if area.width >= 170 {
            46
        } else if area.width >= 145 {
            42
        } else {
            38
        };
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(54),
                Constraint::Length(sidebar_width.min(area.width.saturating_sub(24))),
            ])
            .split(area)
    };

    draw_dashboard_grid(frame, chunks[0], app);
    draw_dashboard_sidebar(frame, chunks[1], app, &sections);
}

// ─── Character grid ──────────────────────────────────────────────────────────

fn draw_dashboard_grid(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let visible = app.visible_clients();
    let title = match app.active_group {
        Some(idx) => {
            if let Some(group) = app.groups.get(idx) {
                format!(
                    " Command Center — G{} {} ({}) ",
                    group.id,
                    group.name,
                    visible.len()
                )
            } else {
                format!(" Command Center — Group {} ({}) ", idx + 1, visible.len())
            }
        }
        None => format!(" Command Center — All Groups ({}) ", visible.len()),
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

    let show_group = area.width >= 78;
    let show_class = area.width >= 88;
    let show_zone = area.width >= 104;

    let mut headers = vec!["", "Name"];
    if show_group {
        headers.push("Grp");
    }
    if show_class {
        headers.push("Cls");
    }
    if show_zone {
        headers.push("Zone");
    }
    headers.extend(["HP", "Cond", "State"]);

    let header = themed_header_row(headers, t);
    let highlight_style = Style::default()
        .bg(t.row_selected_bg)
        .add_modifier(Modifier::BOLD);

    let rows: Vec<Row> = visible
        .iter()
        .map(|client| {
            let global_idx = app
                .clients
                .iter()
                .position(|candidate| candidate.pid == client.pid)
                .unwrap_or(usize::MAX);
            let is_sel = global_idx == app.selected_client;
            let marker = if is_sel { "▶" } else { " " };

            if let Some(player) = &client.local_player {
                let hp_pct = player.hp_pct();
                let name = app.redact_name(&player.displayed_name).into_owned();
                let (condition_label, condition_style) = client_condition(client, t);
                let (activity_label, activity_style) = client_activity(app, client);

                let mut cells = vec![
                    Cell::from(marker).style(Style::default().fg(t.text_accent)),
                    Cell::from(name).style(Style::default().fg(t.text_normal)),
                ];
                if show_group {
                    cells.push(
                        Cell::from(app.client_group_label(client).unwrap_or("--"))
                            .style(Style::default().fg(t.text_secondary)),
                    );
                }
                if show_class {
                    cells.push(
                        Cell::from(player.class_str()).style(Style::default().fg(t.text_accent)),
                    );
                }
                if show_zone {
                    cells.push(
                        Cell::from(client.zone_name.as_str())
                            .style(Style::default().fg(t.text_muted)),
                    );
                }
                cells.push(
                    Cell::from(format!("{:>3.0}%", hp_pct))
                        .style(Style::default().fg(hp_color(hp_pct, t))),
                );
                cells.push(Cell::from(condition_label).style(condition_style));
                cells.push(Cell::from(activity_label).style(activity_style));

                Row::new(cells).style(if is_sel {
                    highlight_style
                } else {
                    Style::default()
                })
            } else {
                let mut cells = vec![
                    Cell::from(marker).style(Style::default().fg(t.text_accent)),
                    Cell::from(if client.client_status.is_empty() {
                        format!("PID {}", client.pid)
                    } else {
                        client.client_status.clone()
                    })
                    .style(Style::default().fg(t.hp_low)),
                ];
                if show_group {
                    cells.push(Cell::from("--"));
                }
                if show_class {
                    cells.push(Cell::from("--"));
                }
                if show_zone {
                    cells.push(Cell::from(client.zone_name.as_str()));
                }
                cells.push(Cell::from(" --"));
                cells.push(Cell::from("Offline").style(Style::default().fg(t.hp_low)));
                cells.push(Cell::from("• Waiting").style(Style::default().fg(t.text_muted)));
                Row::new(cells).style(if is_sel {
                    highlight_style
                } else {
                    Style::default()
                })
            }
        })
        .collect();

    let mut constraints = vec![Constraint::Length(2), Constraint::Min(14)];
    if show_group {
        constraints.push(Constraint::Length(4));
    }
    if show_class {
        constraints.push(Constraint::Length(4));
    }
    if show_zone {
        constraints.push(Constraint::Min(12));
    }
    constraints.extend([
        Constraint::Length(5),
        Constraint::Length(8),
        Constraint::Min(12),
    ]);

    frame.render_widget(
        Table::new(rows, constraints)
            .header(header)
            .block(blk)
            .row_highlight_style(highlight_style),
        area,
    );
}

fn client_condition(client: &ClientState, t: &crate::tui::theme::Theme) -> (&'static str, Style) {
    let Some(player) = &client.local_player else {
        return ("Offline", Style::default().fg(t.hp_low));
    };

    if matches!(player.stand_state, StandState::Dead) {
        return ("Dead", Style::default().fg(t.hp_low));
    }

    let hp_pct = player.hp_pct();
    if hp_pct < 25.0 {
        (
            "Critical",
            Style::default().fg(t.hp_low).add_modifier(Modifier::BOLD),
        )
    } else if hp_pct < 60.0 {
        ("Hurt", Style::default().fg(hp_color(hp_pct, t)))
    } else if matches!(player.stand_state, StandState::Sitting) || hp_pct < 90.0 {
        ("Recover", Style::default().fg(t.mana_color))
    } else {
        ("Stable", Style::default().fg(t.hp_high))
    }
}

fn client_activity(app: &App, client: &ClientState) -> (&'static str, Style) {
    let t = &app.theme;

    if let Some(nav) = app.nav_state.nav_statuses.get(&client.pid) {
        match nav.status.as_str() {
            "Navigating" => {
                return (
                    "➜ Nav",
                    Style::default()
                        .fg(t.text_highlight)
                        .add_modifier(Modifier::BOLD),
                );
            }
            "Arrived" => {
                return ("✓ Arr", Style::default().fg(t.hp_high));
            }
            "Stuck" => {
                return (
                    "! Stuck",
                    Style::default().fg(t.hp_low).add_modifier(Modifier::BOLD),
                );
            }
            _ => {}
        }
    }

    let Some(player) = &client.local_player else {
        return ("• Idle", Style::default().fg(t.text_muted));
    };

    if matches!(player.stand_state, StandState::Dead) {
        return ("☠ Dead", Style::default().fg(t.hp_low));
    }
    if matches!(player.stand_state, StandState::Feigned) {
        return ("⇣ FD", Style::default().fg(t.text_secondary));
    }
    if matches!(player.stand_state, StandState::Sitting) {
        return ("☾ Sit", Style::default().fg(t.state_sitting));
    }
    if matches!(player.stand_state, StandState::Looting) {
        return ("⌕ Loot", Style::default().fg(t.text_highlight));
    }

    if let Some(cast) = &player.cast_state
        && cast.is_casting()
    {
        if is_healer_class(player.class) {
            return ("✚ Heal", Style::default().fg(t.hp_high));
        }
        if is_debuffer_class(player.class) {
            return ("≈ Debuff", Style::default().fg(t.text_accent));
        }
        return ("✦ Cast", Style::default().fg(t.text_highlight));
    }

    if client.target.is_some() {
        return (
            "⚔ Fight",
            Style::default()
                .fg(t.text_server)
                .add_modifier(Modifier::BOLD),
        );
    }

    match player.stand_state {
        StandState::Ducking => ("↧ Duck", Style::default().fg(t.text_secondary)),
        StandState::Frozen => ("■ Hold", Style::default().fg(t.text_muted)),
        _ => ("• Ready", Style::default().fg(t.text_muted)),
    }
}

fn is_healer_class(class: Option<EqClass>) -> bool {
    matches!(
        class,
        Some(EqClass::Cleric | EqClass::Druid | EqClass::Shaman | EqClass::Paladin)
    )
}

fn is_debuffer_class(class: Option<EqClass>) -> bool {
    matches!(
        class,
        Some(EqClass::Enchanter | EqClass::Shaman | EqClass::Necromancer | EqClass::Bard)
    )
}

// ─── Sidebar ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum OverviewSectionKind {
    Character,
    Groups,
    Filters,
    Combat,
    Session,
}

fn draw_dashboard_sidebar(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    sections: &[(OverviewSectionKind, Constraint)],
) {
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
            OverviewSectionKind::Character => {
                draw_character_summary(frame, *chunk, app, app.overview_state.character_collapsed)
            }
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
    let mut sections = vec![(
        OverviewSectionKind::Character,
        if app.overview_state.character_collapsed {
            Constraint::Length(3)
        } else {
            Constraint::Length(7)
        },
    )];

    if app.overview_state.show_groups {
        let group_rows = app.visible_clients().len().min(5) as u16;
        sections.push((
            OverviewSectionKind::Groups,
            if app.overview_state.groups_collapsed {
                Constraint::Length(3)
            } else {
                Constraint::Length(group_rows.saturating_add(2).max(4))
            },
        ));
    }

    if app.overview_state.show_filters {
        sections.push((
            OverviewSectionKind::Filters,
            if app.overview_state.filters_collapsed {
                Constraint::Length(3)
            } else {
                Constraint::Length(5)
            },
        ));
    }

    sections.push((
        OverviewSectionKind::Combat,
        if app.overview_state.combat_collapsed {
            Constraint::Length(3)
        } else if app.ch_chain_status.is_some() {
            Constraint::Length(5)
        } else {
            Constraint::Length(4)
        },
    ));
    sections.push((
        OverviewSectionKind::Session,
        if app.overview_state.session_collapsed {
            Constraint::Length(3)
        } else if app.loot_database.items.is_empty() {
            Constraint::Min(7)
        } else {
            Constraint::Min(10)
        },
    ));

    sections
}

fn preferred_height(constraint: Constraint) -> u16 {
    match constraint {
        Constraint::Length(height) | Constraint::Min(height) => height,
        _ => 3,
    }
}

fn section_title(label: &str, key_hint: Option<&str>, collapsed: bool) -> String {
    let icon = if collapsed { "▶" } else { "▼" };
    match key_hint {
        Some(key) => format!(" {} [{}] {} ", label, key, icon),
        None => format!(" {} {} ", label, icon),
    }
}

fn draw_character_summary(frame: &mut Frame, area: Rect, app: &App, collapsed: bool) {
    let t = &app.theme;
    let border_style = if app.is_panel_focused(ActivePanel::OverviewCharacter) {
        t.border_active
    } else {
        t.border_primary
    };
    let title = section_title("Character", None, collapsed);
    let blk = panel(title.as_str(), border_style, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let Some(client) = app.active_client() else {
        frame.render_widget(
            Paragraph::new("No character selected").style(Style::default().fg(t.text_muted)),
            inner,
        );
        return;
    };
    let Some(player) = &client.local_player else {
        frame.render_widget(
            Paragraph::new("Selected client has no player data")
                .style(Style::default().fg(t.text_muted)),
            inner,
        );
        return;
    };

    let name = app.redact_name(&player.displayed_name).into_owned();
    let group_label = app.client_group_label(client).unwrap_or("--");
    let (condition_label, condition_style) = client_condition(client, t);
    let (activity_label, activity_style) = client_activity(app, client);
    let target_name = client
        .target
        .as_ref()
        .map(|target| app.redact_name(&target.displayed_name).into_owned())
        .unwrap_or_else(|| String::from("—"));

    let lines = if collapsed {
        vec![Line::from(vec![
            Span::styled(name, Style::default().fg(t.text_bright)),
            Span::styled("  ", Style::default()),
            Span::styled(group_label, Style::default().fg(t.text_secondary)),
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("{:.0}%", player.hp_pct()),
                Style::default().fg(hp_color(player.hp_pct(), t)),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(activity_label, activity_style),
        ])]
    } else {
        vec![
            Line::from(vec![
                Span::styled(
                    name,
                    Style::default()
                        .fg(t.text_bright)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(player.class_str(), Style::default().fg(t.text_accent)),
                Span::styled(
                    format!(" Lv{}", player.level),
                    Style::default().fg(t.text_secondary),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(group_label, Style::default().fg(t.text_secondary)),
            ]),
            Line::from(vec![
                Span::styled("Zone ", Style::default().fg(t.text_muted)),
                Span::styled(
                    client.zone_name.as_str(),
                    Style::default().fg(t.text_normal),
                ),
                Span::styled("  HP ", Style::default().fg(t.text_muted)),
                Span::styled(
                    format!("{:.0}%", player.hp_pct()),
                    Style::default().fg(hp_color(player.hp_pct(), t)),
                ),
                Span::styled("  MP ", Style::default().fg(t.text_muted)),
                Span::styled(
                    if player.mana_max > 0 {
                        format!("{:.0}%", player.mana_pct())
                    } else {
                        String::from("—")
                    },
                    Style::default().fg(t.mana_color),
                ),
            ]),
            Line::from(vec![
                Span::styled("Cond ", Style::default().fg(t.text_muted)),
                Span::styled(condition_label, condition_style),
                Span::styled("  State ", Style::default().fg(t.text_muted)),
                Span::styled(
                    player.stand_state.label(),
                    Style::default().fg(stand_state_color(&player.stand_state, t)),
                ),
            ]),
            Line::from(vec![
                Span::styled("Act  ", Style::default().fg(t.text_muted)),
                Span::styled(activity_label, activity_style),
                Span::styled("  Tgt ", Style::default().fg(t.text_muted)),
                Span::styled(target_name, Style::default().fg(t.text_highlight)),
            ]),
            Line::from(vec![
                Span::styled("Pos  ", Style::default().fg(t.text_muted)),
                Span::styled(
                    format!("y:{:.0} x:{:.0} z:{:.0}", player.y, player.x, player.z),
                    Style::default().fg(t.text_secondary),
                ),
            ]),
        ]
    };

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
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
