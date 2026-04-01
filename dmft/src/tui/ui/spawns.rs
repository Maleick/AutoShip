//! Spawns screen — filterable, searchable spawn list + hex dump viewer.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, Wrap},
};

use super::widgets::{hp_color, panel, spawn_info_lines, spawn_row_style, themed_header_row};
use crate::eq::structs::SpawnInfo;
use crate::tui::app::{ActivePanel, App};

pub fn draw_spawns_screen(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    draw_spawn_list(frame, area, app);
}

pub fn draw_spawn_list(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    let t = &app.theme;
    let is_active = matches!(
        app.active_panel,
        ActivePanel::TacticalSpawns | ActivePanel::DebugSpawns
    );
    let border_style = if is_active {
        t.border_active
    } else {
        t.border_dim
    };

    let filtered = app.filtered_spawns();

    let player_level: Option<u8> = app
        .active_client()
        .and_then(|c| c.local_player.as_ref())
        .map(|p| p.level);

    let client_label = app
        .active_client()
        .and_then(|c| c.local_player.as_ref()).map_or_else(|| "???".into(), |p| app.redact_name(&p.displayed_name).into_owned());

    let fl = app.spawns_state.spawn_type_filter.label();
    let title = if app.spawns_state.search_mode {
        format!(
            " Spawns: {} ({}) [{}] search: \"{}\" [Esc to close] ",
            client_label,
            filtered.len(),
            fl,
            app.spawns_state.spawn_filter
        )
    } else if !app.spawns_state.spawn_filter.is_empty() {
        format!(
            " Spawns: {} ({}) [{}] filter: \"{}\" ",
            client_label,
            filtered.len(),
            fl,
            app.spawns_state.spawn_filter
        )
    } else if app.spawns_state.spawn_type_filter != crate::tui::app::SpawnFilter::All {
        format!(" Spawns: {} ({}) [{}] ", client_label, filtered.len(), fl)
    } else {
        format!(" Spawns: {} ({}) ", client_label, filtered.len())
    };

    // Get local player position for distance calculation
    let player_pos: Option<(f32, f32)> = app
        .active_client()
        .and_then(|c| c.local_player.as_ref())
        .map(|p| (p.x, p.y));

    // Show coordinate columns when terminal is wide enough (>= 120 chars)
    let show_coords = area.width >= 120;

    let mut header_cells = vec!["Type", "Name", "Race", "Cls", "Lv", "HP%", "Dist2D", "ID"];
    if show_coords {
        header_cells.push("X");
        header_cells.push("Y");
        header_cells.push("Z");
    }
    let header = themed_header_row(header_cells, t);

    let highlight_style = Style::default()
        .bg(t.row_selected_bg)
        .add_modifier(Modifier::BOLD);

    let rows: Vec<Row> = filtered
        .iter()
        .map(|spawn| {
            let style = spawn_row_style(spawn, player_level, t);
            let name = app.redact_name(&spawn.displayed_name);

            // 2D Euclidean distance — Z (altitude) intentionally excluded for tactical range
            let dist_str = match player_pos {
                Some((px, py)) => {
                    let dist = ((spawn.x - px).powi(2) + (spawn.y - py).powi(2)).sqrt();
                    format!("{dist:.0}")
                }
                None => String::from("-"),
            };

            let mut cells = vec![
                Cell::from(spawn.spawn_type.to_string()),
                Cell::from(name.into_owned()),
                Cell::from(spawn.race_name()),
                Cell::from(spawn.class_str()),
                Cell::from(spawn.level.to_string()),
                Cell::from(format!("{:.0}%", spawn.hp_pct())),
                Cell::from(dist_str),
                Cell::from(spawn.spawn_id.to_string()),
            ];
            if show_coords {
                cells.push(Cell::from(format!("{:.0}", spawn.x)));
                cells.push(Cell::from(format!("{:.0}", spawn.y)));
                cells.push(Cell::from(format!("{:.0}", spawn.z)));
            }
            Row::new(cells).style(style)
        })
        .collect();

    let mut constraints = vec![
        Constraint::Length(7),  // Type
        Constraint::Min(16),    // Name
        Constraint::Length(10), // Race
        Constraint::Length(4),  // Cls
        Constraint::Length(4),  // Lv
        Constraint::Length(6),  // HP%
        Constraint::Length(6),  // Dist
        Constraint::Length(8),  // ID
    ];
    if show_coords {
        constraints.push(Constraint::Length(7)); // X
        constraints.push(Constraint::Length(7)); // Y
        constraints.push(Constraint::Length(6)); // Z
    }

    let table = Table::new(rows, constraints)
        .header(header)
        .block(panel(title.as_str(), border_style, t))
        .row_highlight_style(highlight_style);

    frame.render_stateful_widget(table, area, &mut app.spawns_state.table_state);
}

// ─── Target / spawn panels (used from debug screen) ─────────────────────────

pub fn draw_target_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let target = app.active_client().and_then(|c| c.target.as_ref());
    draw_spawn_panel(
        frame,
        area,
        target,
        " Current Target ",
        t.border_danger,
        "No target",
        app,
    );
}

pub fn draw_spawn_panel(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    spawn: Option<&SpawnInfo>,
    title: &str,
    border_style: Style,
    empty_msg: &str,
    app: &App,
) {
    let t = &app.theme;
    let blk = panel(title, border_style, t);

    if let Some(info) = spawn {
        let lines = spawn_info_lines(info, &|s| app.redact_name(s), t);
        frame.render_widget(
            Paragraph::new(lines).block(blk).wrap(Wrap { trim: true }),
            area,
        );
    } else {
        frame.render_widget(
            Paragraph::new(empty_msg)
                .block(blk)
                .style(Style::default().fg(t.text_muted)),
            area,
        );
    }
}

// ─── Hex dump panel ──────────────────────────────────────────────────────────

pub fn draw_hex_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let is_active = app.active_panel == ActivePanel::DebugHexDump;
    let border_style = if is_active {
        t.border_warn
    } else {
        t.border_dim
    };
    let blk = panel(
        format!(" Hex — {} ", app.hex_state.hex_label),
        border_style,
        t,
    );

    if app.hex_state.hex_data.is_empty() {
        frame.render_widget(
            Paragraph::new("Select a spawn and press Enter to open Debug memory")
                .block(blk)
                .style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
    }

    let inner_height = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line<'_>> = (0..inner_height)
        .filter_map(|row| {
            let offset = row * 16;
            if offset >= app.hex_state.hex_data.len() {
                return None;
            }
            let addr = app.hex_state.hex_address + offset;
            let end = (offset + 16).min(app.hex_state.hex_data.len());
            let chunk = &app.hex_state.hex_data[offset..end];

            let hex_str: String = chunk.iter().map(|b| format!("{b:02x} ")).collect();
            let ascii_str: String = chunk
                .iter()
                .map(|&b| {
                    if b.is_ascii_graphic() || b == b' ' {
                        b as char
                    } else {
                        '·'
                    }
                })
                .collect();

            Some(Line::from(vec![
                Span::styled(format!("{addr:08x}"), Style::default().fg(t.text_muted)),
                Span::raw("  "),
                Span::styled(
                    format!("{hex_str:<48}"),
                    Style::default().fg(t.text_normal),
                ),
                Span::raw(" "),
                Span::styled(ascii_str, Style::default().fg(t.text_highlight)),
            ]))
        })
        .collect();

    frame.render_widget(Paragraph::new(lines).block(blk), area);
}

// ─── Debug screen layout ─────────────────────────────────────────────────────

pub fn draw_debug_screen(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    if area.width < 110 {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(18),
                Constraint::Min(10),
                Constraint::Min(10),
            ])
            .split(area);

        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(rows[0]);

        draw_player_detail(frame, top[0], app);
        draw_target_panel(frame, top[1], app);
        draw_hex_panel(frame, rows[1], app);
        draw_spawn_list(frame, rows[2], app);
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Min(10),
        ])
        .split(cols[0]);

    draw_player_detail(frame, left[0], app);
    draw_target_panel(frame, left[1], app);
    draw_hex_panel(frame, left[2], app);
    draw_spawn_list(frame, cols[1], app);
}

fn draw_player_detail(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    use crate::tui::sprites;
    let t = &app.theme;
    let blk = panel(" Selected Character ", t.border_primary, t);

    let client = match app.active_client() {
        Some(c) => c,
        None => {
            frame.render_widget(
                Paragraph::new("No client selected")
                    .block(blk)
                    .style(Style::default().fg(t.text_muted)),
                area,
            );
            return;
        }
    };

    let player = match &client.local_player {
        Some(p) => p,
        None => {
            frame.render_widget(
                Paragraph::new("Not logged in")
                    .block(blk)
                    .style(Style::default().fg(t.text_muted)),
                area,
            );
            return;
        }
    };

    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let name = app.redact_name(&player.displayed_name).into_owned();
    let hp_pct = player.hp_pct();
    let mana_pct = player.mana_pct();
    use super::widgets::stand_state_color;

    let mut lines: Vec<Line<'_>> = vec![
        Line::from(vec![
            Span::styled(
                &name,
                Style::default()
                    .fg(t.text_bright)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} Lv{}", player.class_str(), player.level),
                Style::default().fg(t.text_accent),
            ),
            Span::raw("  "),
            Span::styled(
                format!("[{}]", player.stand_state),
                Style::default().fg(stand_state_color(&player.stand_state, t)),
            ),
        ]),
        Line::from(vec![
            Span::styled("HP   ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{}/{} ({:.0}%)", player.hp_current, player.hp_max, hp_pct),
                Style::default().fg(hp_color(hp_pct, t)),
            ),
        ]),
        Line::from(vec![
            Span::styled("Mana ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!(
                    "{}/{} ({:.0}%)",
                    player.mana_current, player.mana_max, mana_pct
                ),
                Style::default().fg(t.mana_color),
            ),
            Span::styled(
                format!(
                    "  End {}/{}",
                    player.endurance_current, player.endurance_max
                ),
                Style::default().fg(t.text_muted),
            ),
        ]),
        Line::from(vec![
            Span::styled("Pos  ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("({:.1}, {:.1}, {:.1})", player.y, player.x, player.z),
                Style::default().fg(t.text_server),
            ),
            Span::styled(
                format!("  Hdg {:.1}", player.heading),
                Style::default().fg(t.text_muted),
            ),
        ]),
        Line::from(vec![
            Span::styled("Zone ", Style::default().fg(t.text_muted)),
            Span::styled(&client.zone_name, Style::default().fg(t.text_normal)),
        ]),
    ];

    // Cast state
    if let Some(cast) = &player.cast_state {
        if cast.is_casting() {
            lines.push(Line::from(vec![
                Span::styled("Casting ", Style::default().fg(t.text_muted)),
                Span::styled(
                    format!("gem {} (ETA: {})", cast.spell_slot + 1, cast.spell_eta),
                    Style::default()
                        .fg(t.text_highlight)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }
        let recast_strs: Vec<String> = cast
            .gem_etas
            .iter()
            .enumerate()
            .filter(|(_, eta)| **eta != 0)
            .map(|(i, eta)| format!("G{}:{}", i + 1, eta))
            .collect();
        if !recast_strs.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Recast  ", Style::default().fg(t.text_muted)),
                Span::styled(recast_strs.join(" "), Style::default().fg(t.text_accent)),
            ]));
        }
    }

    lines.push(Line::from(""));

    for sprite_line in
        sprites::class_sprite(player.class.as_ref(), &player.stand_state, app.tick_count)
    {
        lines.push(sprite_line);
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}
