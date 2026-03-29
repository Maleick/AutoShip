use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, Wrap},
    Frame,
};

use super::app::{extract_account_number, ActivePanel, ActiveScreen, App};
use super::sprites;
use super::theme::Theme;
use crate::eq::structs::{SpawnInfo, SpawnType};

// ─── Entry point ────────────────────────────────────────────────────────────

/// Main render function — dispatches to the active screen.
pub fn draw(frame: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header + tab bar
            Constraint::Min(10),   // body
            Constraint::Length(3), // status bar
        ])
        .split(frame.area());

    draw_header(frame, outer[0], app);

    match app.active_screen {
        ActiveScreen::Dashboard  => draw_dashboard(frame, outer[1], app),
        ActiveScreen::Spawns     => draw_spawns_screen(frame, outer[1], app),
        ActiveScreen::Character  => draw_character_screen(frame, outer[1], app),
        ActiveScreen::Map        => draw_map_screen(frame, outer[1], app),
        ActiveScreen::Groups     => draw_groups_screen(frame, outer[1], app),
        ActiveScreen::Navigation => draw_navigation_screen(frame, outer[1], app),
    }

    draw_status_bar(frame, outer[2], app);

    if app.help_visible {
        draw_help_overlay(frame, frame.area());
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Build a `Block` with the project's standard chrome (border type + style + title).
fn panel<'a>(title: impl Into<ratatui::text::Line<'a>>, border_style: Style, t: &Theme) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(t.border_type)
        .title(title)
        .border_style(border_style)
}

fn hp_color(hp_pct: f64, t: &Theme) -> Color {
    if hp_pct > 75.0      { t.hp_high }
    else if hp_pct > 25.0 { t.hp_mid  }
    else                  { t.hp_low  }
}

fn stand_state_color(state: &crate::eq::structs::StandState, t: &Theme) -> Color {
    use crate::eq::structs::StandState;
    match state {
        StandState::Dead    => t.state_dead,
        StandState::Sitting => t.state_sitting,
        StandState::Feigned => t.state_feigned,
        StandState::Frozen  => t.state_frozen,
        _                   => t.state_normal,
    }
}

fn spawn_type_color(st: &SpawnType, t: &Theme) -> Color {
    match st {
        SpawnType::Player     => t.spawn_pc,
        SpawnType::Npc        => t.spawn_npc,
        SpawnType::Corpse     => t.spawn_corpse,
        SpawnType::Unknown(_) => t.spawn_unknown,
    }
}

fn themed_header_row<'a>(cells: Vec<&'a str>, t: &Theme) -> Row<'a> {
    Row::new(cells.into_iter().map(|c| Cell::from(c).style(t.table_header)).collect::<Vec<_>>())
        .height(1)
        .bottom_margin(0)
}

// ─── Header ─────────────────────────────────────────────────────────────────

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    let client_count = app.clients.len();
    let client_str = if client_count > 0 {
        format!(" {}✕ EQ", client_count)
    } else {
        " Not attached".into()
    };

    let selected_str = if let Some(client) = app.active_client() {
        let name = client
            .local_player
            .as_ref()
            .map(|p| app.redact_name(&p.displayed_name).into_owned())
            .unwrap_or_else(|| "???".into());
        format!(" [{}/{}] {} ", app.selected_client + 1, client_count, name)
    } else {
        " No client ".into()
    };

    let server_str = format!(" {} ", app.display_server());
    let zone_str = app
        .active_client()
        .map(|c| if c.zone_name.is_empty() { "Unknown Zone".into() } else { c.zone_name.clone() })
        .unwrap_or_else(|| "No Zone".into());

    // Tab bar
    let mut tabs: Vec<Span<'_>> = vec![Span::raw("  ")];
    for screen in &ActiveScreen::ALL {
        let is_active = *screen == app.active_screen;
        let label = format!(" {} ", screen.label());
        if is_active {
            tabs.push(Span::styled(label, t.tab_active));
            tabs.push(Span::raw(" "));
        } else {
            tabs.push(Span::styled(label, t.tab_inactive));
            tabs.push(Span::raw(" "));
        }
    }

    // Group indicator
    let group_label = app.group_focus_label();
    let group_style = if app.active_group.is_some() {
        t.header_group_active
    } else {
        t.header_group
    };

    let mut spans: Vec<Span<'_>> = vec![
        Span::styled(" FROST ", t.header_title),
        Span::styled("│", t.border_dim),
        Span::styled(&client_str, t.header_client_count),
        Span::styled(" │", t.border_dim),
        Span::styled(&selected_str, t.header_selected),
        Span::styled("│ ", t.border_dim),
        Span::styled(format!(" {} ", group_label), group_style),
        Span::styled(" │ ", t.border_dim),
        Span::styled(&server_str, Style::default().fg(t.text_server)),
        Span::styled("│ ", t.border_dim),
        Span::styled(format!(" {} ", zone_str), t.header_zone),
        Span::styled("│", t.border_dim),
        Span::styled("  ", Style::default()),
    ];
    spans.extend(tabs);

    let header = Paragraph::new(Line::from(spans))
        .block(panel(" Frostreaver ", t.border_dim, t));
    frame.render_widget(header, area);
}

// ─── Screen 1: Dashboard ────────────────────────────────────────────────────

fn draw_dashboard(frame: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    draw_dashboard_grid(frame, cols[0], app);
    draw_dashboard_sidebar(frame, cols[1], app);
}

fn draw_dashboard_grid(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let visible = app.visible_clients();
    let title = match app.active_group {
        Some(idx) => format!(" G{} {} ({}) ", app.groups[idx].id, app.groups[idx].name, visible.len()),
        None => format!(" Characters ({}) ", app.clients.len()),
    };

    let blk = panel(title.as_str(), t.border_primary, t);

    if visible.is_empty() {
        frame.render_widget(
            Paragraph::new("No characters connected").block(blk).style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
    }

    let header = themed_header_row(vec!["", "Name", "Cls", "Lv", "HP%", "MP%", "State", "Zone"], t);

    let rows: Vec<Row> = visible.iter().map(|client| {
        let global_idx = app.clients.iter().position(|c| c.pid == client.pid).unwrap_or(usize::MAX);
        let is_selected = global_idx == app.selected_client;
        let marker = if is_selected { "▶" } else { " " };

        if let Some(player) = &client.local_player {
            let hp_pct   = player.hp_pct();
            let mana_pct = player.mana_pct();
            let name     = app.redact_name(&player.displayed_name).into_owned();

            let row_style = if is_selected {
                Style::default().bg(t.row_selected_bg).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(marker).style(Style::default().fg(t.text_accent)),
                Cell::from(name).style(Style::default().fg(t.text_normal)),
                Cell::from(player.class_str()).style(Style::default().fg(t.text_accent)),
                Cell::from(player.level.to_string()).style(Style::default().fg(t.text_secondary)),
                Cell::from(format!("{:.0}%", hp_pct)).style(Style::default().fg(hp_color(hp_pct, t))),
                Cell::from(if player.mana_max > 0 { format!("{:.0}%", mana_pct) } else { "-".into() })
                    .style(Style::default().fg(t.mana_color)),
                Cell::from(player.stand_state.label())
                    .style(Style::default().fg(stand_state_color(&player.stand_state, t))),
                Cell::from(client.zone_name.as_str()).style(Style::default().fg(t.text_muted)),
            ])
            .style(row_style)
        } else {
            Row::new(vec![
                Cell::from(marker).style(Style::default().fg(t.text_accent)),
                Cell::from(format!("PID {}", client.pid)).style(Style::default().fg(t.text_muted)),
                Cell::from("-"), Cell::from("-"), Cell::from("-"),
                Cell::from("-"), Cell::from("-"),
                Cell::from(client.zone_name.as_str()),
            ])
        }
    }).collect();

    let table = Table::new(rows, [
        Constraint::Length(2),  // marker
        Constraint::Min(14),    // Name
        Constraint::Length(4),  // Class
        Constraint::Length(3),  // Level
        Constraint::Length(5),  // HP%
        Constraint::Length(5),  // MP%
        Constraint::Length(8),  // State
        Constraint::Min(12),    // Zone
    ])
    .header(header)
    .block(blk);

    frame.render_widget(table, area);
}

fn draw_dashboard_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(12), Constraint::Length(6)])
        .split(area);

    draw_group_health_bars(frame, chunks[0], app);
    draw_session_stats(frame, chunks[1], app);
    draw_server_info(frame, chunks[2], app);
}

fn draw_group_health_bars(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let blk = panel(" Group Health ", t.border_warn, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let bar_width = inner.width.saturating_sub(18) as usize;
    let visible = app.visible_clients();

    let lines: Vec<Line<'_>> = visible.iter().map(|client| {
        if let Some(player) = &client.local_player {
            let hp_pct  = player.hp_pct();
            let filled  = ((hp_pct / 100.0) * bar_width as f64) as usize;
            let empty   = bar_width.saturating_sub(filled);
            let color   = hp_color(hp_pct, t);
            let global_idx = app.clients.iter().position(|c| c.pid == client.pid).unwrap_or(usize::MAX);
            let is_sel  = global_idx == app.selected_client;
            let name    = app.redact_name(&player.displayed_name).into_owned();

            let name_style = if is_sel {
                Style::default().fg(t.text_bright).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text_normal)
            };

            Line::from(vec![
                Span::styled(format!("{:<12} ", name), name_style),
                Span::styled("█".repeat(filled), Style::default().fg(color)),
                Span::styled("░".repeat(empty),  Style::default().fg(t.bar_empty)),
                Span::styled(format!(" {:>3.0}%", hp_pct), Style::default().fg(color)),
            ])
        } else {
            Line::from(Span::styled(
                format!("  PID {} …", client.pid),
                Style::default().fg(t.text_muted),
            ))
        }
    }).collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_session_stats(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let db = &app.loot_database;
    let elapsed = app.session_start.elapsed();
    let hours = elapsed.as_secs() as f64 / 3600.0;

    let duration_str = {
        let secs = elapsed.as_secs();
        format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
    };

    let xp_per_hour = if hours > 0.01 {
        format!("{:.0}", db.total_xp_events as f64 / hours)
    } else { "-".into() };

    let total_plat = db.total_plat as f64
        + db.total_gold   as f64 / 10.0
        + db.total_silver as f64 / 100.0
        + db.total_copper as f64 / 1000.0;
    let plat_per_hour = if hours > 0.01 { format!("{:.1}", total_plat / hours) } else { "-".into() };

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
            Span::styled("XP ", Style::default().fg(t.text_muted)),
            Span::styled(format!("{} ({}/hr)", db.total_xp_events, xp_per_hour),
                         Style::default().fg(t.hp_high)),
        ]),
        Line::from(vec![
            Span::styled("Pp ", Style::default().fg(t.text_muted)),
            Span::styled(format!("{:.0} ({}/hr)", total_plat, plat_per_hour),
                         Style::default().fg(t.text_highlight)),
        ]),
        Line::from(vec![
            Span::styled("☠ ", Style::default().fg(t.text_muted)),
            Span::styled(total_kills.to_string(), Style::default().fg(t.hp_low)),
            Span::styled("  Deaths: ", Style::default().fg(t.text_muted)),
            Span::styled(db.deaths.to_string(),
                Style::default().fg(if db.deaths > 0 { t.hp_low } else { t.text_muted })),
        ]),
    ];

    if !top_items.is_empty() {
        lines.push(Line::from(Span::styled("── Loot ──", Style::default().fg(t.text_muted))));
        for (name, count) in &top_items {
            let truncated: String = name.chars().take(18).collect();
            lines.push(Line::from(vec![
                Span::raw(" "),
                Span::styled(format!("{}× ", count), Style::default().fg(t.text_highlight)),
                Span::styled(truncated, Style::default().fg(t.text_secondary)),
            ]));
        }
    }

    frame.render_widget(
        Paragraph::new(lines).block(panel(" Session ", t.border_primary, t)),
        area,
    );
}

fn draw_server_info(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let lines = vec![
        Line::from(vec![
            Span::styled("Server  ", Style::default().fg(t.text_muted)),
            Span::styled(app.display_server(), Style::default().fg(t.text_server)),
        ]),
        Line::from(vec![
            Span::styled("Clients ", Style::default().fg(t.text_muted)),
            Span::styled(app.clients.len().to_string(), Style::default().fg(t.text_highlight)),
        ]),
        Line::from(vec![
            Span::styled("Refresh ", Style::default().fg(t.text_muted)),
            Span::styled(format!("{}ms", app.refresh_rate_ms), Style::default().fg(t.text_accent)),
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

// ─── Screen 2: Spawns ───────────────────────────────────────────────────────

fn draw_spawns_screen(frame: &mut Frame, area: Rect, app: &App) {
    draw_spawn_list(frame, area, app);
}

// ─── Screen 3: Character ────────────────────────────────────────────────────

fn draw_character_screen(frame: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(8)])
        .split(cols[0]);

    draw_player_detail(frame, left[0], app);
    draw_target_panel(frame, left[1], app);
    draw_hex_panel(frame, cols[1], app);
}

fn draw_player_detail(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let blk = panel(" Selected Character ", t.border_primary, t);

    let client = match app.active_client() {
        Some(c) => c,
        None => {
            frame.render_widget(
                Paragraph::new("No client selected").block(blk).style(Style::default().fg(t.text_muted)),
                area,
            );
            return;
        }
    };

    let player = match &client.local_player {
        Some(p) => p,
        None => {
            frame.render_widget(
                Paragraph::new("Not logged in").block(blk).style(Style::default().fg(t.text_muted)),
                area,
            );
            return;
        }
    };

    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let name     = app.redact_name(&player.displayed_name).into_owned();
    let hp_pct   = player.hp_pct();
    let mana_pct = player.mana_pct();

    let mut lines: Vec<Line<'_>> = vec![
        Line::from(vec![
            Span::styled(&name, Style::default().fg(t.text_bright).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(format!("{} Lv{}", player.class_str(), player.level),
                         Style::default().fg(t.text_accent)),
            Span::raw("  "),
            Span::styled(format!("[{}]", player.stand_state),
                         Style::default().fg(stand_state_color(&player.stand_state, t))),
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
                format!("{}/{} ({:.0}%)", player.mana_current, player.mana_max, mana_pct),
                Style::default().fg(t.mana_color),
            ),
            Span::styled(format!("  End {}/{}", player.endurance_current, player.endurance_max),
                         Style::default().fg(t.text_muted)),
        ]),
        Line::from(vec![
            Span::styled("Pos  ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("({:.1}, {:.1}, {:.1})", player.y, player.x, player.z),
                Style::default().fg(t.text_server),
            ),
            Span::styled(format!("  Hdg {:.1}", player.heading), Style::default().fg(t.text_muted)),
        ]),
        Line::from(vec![
            Span::styled("Zone ", Style::default().fg(t.text_muted)),
            Span::styled(&client.zone_name, Style::default().fg(t.text_normal)),
        ]),
        Line::from(""),
    ];

    for sprite_line in sprites::class_sprite(player.class.as_ref(), &player.stand_state, app.tick_count) {
        lines.push(sprite_line);
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

// ─── Screen 4: Map ──────────────────────────────────────────────────────────

fn draw_map_screen(frame: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55),
            Constraint::Percentage(25),
            Constraint::Percentage(20),
        ])
        .split(area);

    draw_map_view(frame, cols[0], app);
    draw_map_spawn_list(frame, cols[1], app);
    draw_named_tracker_panel(frame, cols[2], app);
}

fn draw_map_view(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let zone_label = app.active_client().map(|c| c.zone_name.as_str()).unwrap_or("Unknown");
    let map_info = app.zone_map.as_ref()
        .map(|m| format!(" Map: {} ({} lines, {} labels) ", zone_label, m.lines.len(), m.points.len()))
        .unwrap_or_else(|| format!(" Map: {} (no map data) ", zone_label));

    let blk = panel(map_info.as_str(), t.border_active, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let w = inner.width as usize;
    let h = inner.height as usize;
    if w == 0 || h == 0 { return; }

    let mut grid: Vec<Vec<(char, Color)>> = vec![vec![(' ', t.map_lines); w]; h];

    let (center_x, center_y, scale_x, scale_y) = if let Some(map) = &app.zone_map {
        let (cx, cy) = if let Some(player) = &app.local_player {
            (-player.y, -player.x)
        } else {
            (map.bounds.center_x(), map.bounds.center_y())
        };
        let sx = (w as f32 - 2.0) / map.bounds.width();
        let sy = (h as f32 - 2.0) / map.bounds.height();
        let s  = sx.min(sy);
        (cx, cy, s, s)
    } else {
        let spawns = &app.spawns;
        if spawns.is_empty() {
            frame.render_widget(
                Paragraph::new("No map or spawn data").style(Style::default().fg(t.text_muted)),
                inner,
            );
            return;
        }
        let (mut min_x, mut max_x, mut min_y, mut max_y) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for s in spawns {
            let mx = -s.y; let my = -s.x;
            if mx < min_x { min_x = mx; } if mx > max_x { max_x = mx; }
            if my < min_y { min_y = my; } if my > max_y { max_y = my; }
        }
        let range_x = (max_x - min_x).max(1.0);
        let range_y = (max_y - min_y).max(1.0);
        let cx = (min_x + max_x) / 2.0;
        let cy = (min_y + max_y) / 2.0;
        let u  = ((w as f32 - 2.0) / range_x).min((h as f32 - 2.0) / range_y);
        (cx, cy, u, u)
    };

    let to_grid = |mx: f32, my: f32| -> (i32, i32) {
        let col = ((mx - center_x) * scale_x + w as f32 / 2.0) as i32;
        let row = ((my - center_y) * scale_y + h as f32 / 2.0) as i32;
        (col, row)
    };

    if let Some(map) = &app.zone_map {
        for ml in &map.lines {
            let (c1, r1) = to_grid(ml.x1, ml.y1);
            let (c2, r2) = to_grid(ml.x2, ml.y2);
            let color = map_rgb_to_color(ml.r, ml.g, ml.b, t);
            bresenham_line(c1, r1, c2, r2, w, h, &mut grid, color);
        }
        for mp in &map.points {
            let (col, row) = to_grid(mp.x, mp.y);
            if col >= 0 && col < w as i32 && row >= 0 && row < h as i32 {
                let color = map_rgb_to_color(mp.r, mp.g, mp.b, t);
                let ch = if mp.label.is_empty() { '*' } else { mp.label.chars().next().unwrap_or('*') };
                grid[row as usize][col as usize] = (ch, color);
                let label_text: String = mp.label.chars().take(12).collect();
                for (i, c) in label_text.chars().enumerate() {
                    let lc = col as usize + 1 + i;
                    if lc < w && grid[row as usize][lc].0 == ' ' {
                        grid[row as usize][lc] = (c, color);
                    }
                }
            }
        }
    }

    for spawn in &app.spawns {
        let mx = -spawn.y; let my = -spawn.x;
        let (col, row) = to_grid(mx, my);
        if col >= 0 && col < w as i32 && row >= 0 && row < h as i32 {
            let (ch, color) = match spawn.spawn_type {
                SpawnType::Player => ('@', t.map_pc),
                SpawnType::Npc => {
                    if !spawn.displayed_name.starts_with("a ")
                        && !spawn.displayed_name.starts_with("an ")
                    { ('!', t.map_named) } else { ('·', t.map_npc) }
                }
                SpawnType::Corpse     => ('.', t.map_corpse),
                SpawnType::Unknown(_) => ('?', t.spawn_unknown),
            };
            grid[row as usize][col as usize] = (ch, color);
        }
    }

    for status in app.named_tracker.tracked_spawns() {
        if !status.is_alive {
            let (col, row) = to_grid(-status.last_y, -status.last_x);
            if col >= 0 && col < w as i32 && row >= 0 && row < h as i32 {
                grid[row as usize][col as usize] = ('✕', t.map_dead_named);
            }
        }
    }

    if let Some(player) = &app.local_player {
        let (col, row) = to_grid(-player.y, -player.x);
        if col >= 0 && col < w as i32 && row >= 0 && row < h as i32 {
            grid[row as usize][col as usize] = ('◆', t.map_you);
        }
    }

    let legend_row = h.saturating_sub(1);
    let lines: Vec<Line<'_>> = grid.into_iter().enumerate().map(|(i, row)| {
        if i == legend_row {
            Line::from(vec![
                Span::styled("◆ ", Style::default().fg(t.map_you)),   Span::styled("You", Style::default().fg(t.text_muted)),
                Span::raw(" │ "),
                Span::styled("@ ", Style::default().fg(t.map_pc)),    Span::styled("PC", Style::default().fg(t.text_muted)),
                Span::raw(" │ "),
                Span::styled("· ", Style::default().fg(t.map_npc)),   Span::styled("NPC", Style::default().fg(t.text_muted)),
                Span::raw(" │ "),
                Span::styled("! ", Style::default().fg(t.map_named)), Span::styled("Named", Style::default().fg(t.text_muted)),
                Span::raw(" │ "),
                Span::styled("✕ ", Style::default().fg(t.map_dead_named)), Span::styled("Dead", Style::default().fg(t.text_muted)),
                Span::raw(" │ "),
                Span::styled(". ", Style::default().fg(t.map_corpse)), Span::styled("Corpse", Style::default().fg(t.text_muted)),
            ])
        } else {
            Line::from(row.into_iter().map(|(ch, color)| {
                Span::styled(String::from(ch), Style::default().fg(color))
            }).collect::<Vec<_>>())
        }
    }).collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

fn map_rgb_to_color(r: u8, g: u8, b: u8, t: &Theme) -> Color {
    if r == 0 && g == 0 && b == 0 { t.map_lines } else { Color::Rgb(r, g, b) }
}

#[allow(clippy::too_many_arguments)]
fn bresenham_line(
    x0: i32, y0: i32, x1: i32, y1: i32,
    w: usize, h: usize,
    grid: &mut [Vec<(char, Color)>],
    color: Color,
) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut cx = x0;
    let mut cy = y0;
    let max_steps = (dx.unsigned_abs() + dy.unsigned_abs() + 1).min(10_000) as usize;

    for _ in 0..max_steps {
        if cx >= 0 && cx < w as i32 && cy >= 0 && cy < h as i32 {
            let ux = cx as usize; let uy = cy as usize;
            if grid[uy][ux].0 == ' ' {
                grid[uy][ux] = (line_char(x0, y0, x1, y1), color);
            }
        }
        if cx == x1 && cy == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; cx += sx; }
        if e2 <= dx { err += dx; cy += sy; }
    }
}

fn line_char(x0: i32, y0: i32, x1: i32, y1: i32) -> char {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    if dx == 0 && dy == 0     { '·' }
    else if dy == 0            { '─' }
    else if dx == 0            { '│' }
    else if dx > dy * 2        { '─' }
    else if dy > dx * 2        { '│' }
    else if (x1 - x0).signum() != (y1 - y0).signum() { '╱' }
    else                       { '╲' }
}

fn draw_map_spawn_list(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let blk = panel(" Spawn Positions ", t.border_dim, t);
    let header = themed_header_row(vec!["T", "Name", "Y", "X", "Z"], t);

    let rows: Vec<Row> = app.spawns.iter().map(|spawn| {
        let name  = app.redact_name(&spawn.displayed_name).into_owned();
        let color = spawn_type_color(&spawn.spawn_type, t);
        Row::new(vec![
            Cell::from(match spawn.spawn_type {
                SpawnType::Player => "@", SpawnType::Npc => "·",
                SpawnType::Corpse => ".", SpawnType::Unknown(_) => "?",
            }),
            Cell::from(name),
            Cell::from(format!("{:.0}", spawn.y)),
            Cell::from(format!("{:.0}", spawn.x)),
            Cell::from(format!("{:.0}", spawn.z)),
        ]).style(Style::default().fg(color))
    }).collect();

    let table = Table::new(rows, [
        Constraint::Length(2), Constraint::Min(14),
        Constraint::Length(7), Constraint::Length(7), Constraint::Length(5),
    ])
    .header(header)
    .block(blk);

    frame.render_widget(table, area);
}

fn draw_named_tracker_panel(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let named = app.named_tracker.tracked_spawns();
    let named_alive = named.iter().filter(|s| s.is_alive).count();
    let user_tracked = &app.tracked_spawns;
    let user_up = user_tracked.values().filter(|t| t.status == super::app::TrackedStatus::Up).count();

    let has_user = !user_tracked.is_empty();
    let has_named = !named.is_empty();

    let (named_area, user_area) = if has_user && has_named {
        let ch = Layout::default().direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        (Some(ch[0]), Some(ch[1]))
    } else if has_user {
        (None, Some(area))
    } else {
        (Some(area), None)
    };

    if let Some(na) = named_area {
        let title = format!(" Named ({} up / {}) ", named_alive, named.len());
        let blk = panel(title.as_str(), t.border_warn, t);

        if named.is_empty() {
            frame.render_widget(
                Paragraph::new("No named spawns detected").block(blk).style(Style::default().fg(t.text_muted)),
                na,
            );
        } else {
            let header = themed_header_row(vec!["Name", "St", "Timer"], t);
            let rows: Vec<Row> = named.iter().map(|s| {
                let (status_str, color) = if s.is_alive {
                    ("UP", t.hp_high)
                } else {
                    ("down", t.text_muted)
                };
                let timer_str = if let Some(death_tick) = s.death_tick {
                    let elapsed_ticks = app.tick_count.saturating_sub(death_tick);
                    let elapsed_secs = elapsed_ticks * app.refresh_rate_ms / 1000;
                    let m = elapsed_secs / 60;
                    let sec = elapsed_secs % 60;
                    format!("{:02}:{:02}", m, sec)
                } else { "-".into() };

                let name_style = if s.is_alive {
                    Style::default().fg(t.spawn_named).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(t.text_muted)
                };

                Row::new(vec![
                    Cell::from(s.name.clone()).style(name_style),
                    Cell::from(status_str).style(Style::default().fg(color)),
                    Cell::from(timer_str).style(Style::default().fg(if s.is_alive { t.text_highlight } else { t.text_accent })),
                ])
            }).collect();

            frame.render_widget(
                Table::new(rows, [Constraint::Min(12), Constraint::Length(5), Constraint::Length(7)])
                    .header(header).block(blk),
                na,
            );
        }
    }

    if let Some(ua) = user_area {
        let title = format!(" Tracked ({} up / {}) ", user_up, user_tracked.len());
        let blk = panel(title.as_str(), t.border_server, t);
        let header = themed_header_row(vec!["Name", "St"], t);

        let mut sorted: Vec<_> = user_tracked.values().collect();
        sorted.sort_by(|a, b| {
            let ord = |s: &super::app::TrackedStatus| match s {
                super::app::TrackedStatus::Up      => 0,
                super::app::TrackedStatus::Down    => 1,
                super::app::TrackedStatus::Unknown => 2,
            };
            ord(&a.status).cmp(&ord(&b.status)).then_with(|| a.name.cmp(&b.name))
        });

        let rows: Vec<Row> = sorted.iter().map(|tr| {
            Row::new(vec![
                Cell::from(tr.name.clone()).style(Style::default().fg(t.text_normal)),
                Cell::from(tr.status.label()).style(Style::default().fg(tr.status.color())),
            ])
        }).collect();

        frame.render_widget(
            Table::new(rows, [Constraint::Min(12), Constraint::Length(5)])
                .header(header).block(blk),
            ua,
        );
    }
}

// ─── Screen 5: Groups ───────────────────────────────────────────────────────

fn draw_groups_screen(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let group_count = app.groups.len();

    if group_count == 0 {
        frame.render_widget(
            Paragraph::new("No groups configured. Add groups to config/accounts.toml")
                .block(panel(" Groups ", t.border_dim, t))
                .style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
    }

    let (num_rows, num_cols): (usize, usize) = match group_count {
        1 => (1, 1), 2 => (1, 2), 3 => (1, 3), 4 => (2, 2),
        5..=6 => (2, 3), 7..=9 => (3, 3), _ => (3, 4),
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints((0..num_rows).map(|_| Constraint::Ratio(1, num_rows as u32)).collect::<Vec<_>>())
        .split(area);

    let col_constraints: Vec<Constraint> = (0..num_cols).map(|_| Constraint::Ratio(1, num_cols as u32)).collect();

    let mut panel_idx = 0;
    for row in rows.iter() {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints.clone())
            .split(*row);

        for col in cols.iter() {
            if panel_idx < group_count {
                draw_group_panel(frame, *col, app, &app.groups[panel_idx], panel_idx);
            }
            panel_idx += 1;
        }
    }
}

fn clients_in_group<'a>(app: &'a App, group: &super::app::GroupDef) -> Vec<&'a super::app::ClientState> {
    let (lo, hi) = group.account_range;
    app.clients.iter().filter(|c| {
        let name = if !c.character_name.is_empty() { &c.character_name }
                   else if let Some(p) = &c.local_player { &p.displayed_name }
                   else { return false; };
        extract_account_number(name).map(|n| n >= lo && n <= hi).unwrap_or(false)
    }).collect()
}

fn draw_group_panel(frame: &mut Frame, area: Rect, app: &App, group: &super::app::GroupDef, group_idx: usize) {
    let t = &app.theme;
    let members = clients_in_group(app, group);
    let online  = members.len();
    let (lo, hi) = group.account_range;
    let total   = (hi - lo + 1) as usize;
    let focused = app.active_group == Some(group_idx);

    let has_dead = members.iter().any(|c| c.local_player.as_ref().is_some_and(|p| p.hp_current == 0));

    let border_style = if focused {
        t.border_active
    } else if has_dead {
        t.border_danger
    } else if online == total {
        t.border_primary
    } else if online > 0 {
        t.border_warn
    } else {
        t.border_dim
    };
    if focused {
        // add BOLD to border style
    }

    let zone = members.first().map(|c| c.zone_name.as_str()).unwrap_or("---");
    let title = format!(" G{} {} ({}/{}) {} ", group.id, group.name, online, total, zone);
    let blk = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border_type)
        .title(title.as_str())
        .border_style(if focused { border_style.add_modifier(Modifier::BOLD) } else { border_style });

    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    // Build slot → client map
    let mut slot_map: std::collections::HashMap<u8, &super::app::ClientState> = std::collections::HashMap::new();
    for client in &members {
        let name = if !client.character_name.is_empty() { &client.character_name }
                   else if let Some(p) = &client.local_player { &p.displayed_name }
                   else { continue; };
        if let Some(n) = extract_account_number(name) { slot_map.insert(n, client); }
    }

    let config_map: std::collections::HashMap<u8, &crate::config::AccountEntry> =
        app.accounts_config.as_ref()
            .map(|cfg| {
                cfg.accounts.iter()
                    .filter(|a| a.group == group.id as u32)
                    .filter_map(|a| extract_account_number(&a.name).map(|n| (n, a)))
                    .collect()
            })
            .unwrap_or_default();

    let mut lines: Vec<Line<'_>> = Vec::new();
    for acct_num in lo..=hi {
        if let Some(client) = slot_map.get(&acct_num) {
            if let Some(player) = &client.local_player {
                let hp_pct  = player.hp_pct();
                let name    = app.redact_name(&player.displayed_name).into_owned();
                let mana_str = if player.mana_max > 0 {
                    format!(" {:>3.0}%mp", player.mana_pct())
                } else { "   -  ".to_string() };

                // Buff timer placeholder — will be populated when buff timer
                // data is available from the DLL. Shows dots until then.
                let buff_timers = "  ··· ··· ···";

                lines.push(Line::from(vec![
                    Span::styled(format!("{:<12}", name), Style::default().fg(t.text_normal)),
                    Span::styled(format!("{:<4}", player.class_str()), Style::default().fg(t.text_accent)),
                    Span::styled(format!("{:>3}", player.level), Style::default().fg(t.text_secondary)),
                    Span::styled(format!(" {:>3.0}%", hp_pct), Style::default().fg(hp_color(hp_pct, t))),
                    Span::styled(mana_str, Style::default().fg(t.mana_color)),
                    Span::styled(buff_timers, Style::default().fg(t.text_muted)),
                ]));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("  PID {} (loading…)", client.pid),
                    Style::default().fg(t.text_muted),
                )));
            }
        } else if let Some(acct) = config_map.get(&acct_num) {
            lines.push(Line::from(vec![
                Span::styled(format!("  #{:02} {:<4}", acct_num, acct.class), Style::default().fg(t.text_muted)),
                Span::styled(" offline", Style::default().fg(t.text_muted)),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                format!("  #{:02} ── empty ──", acct_num),
                Style::default().fg(t.text_muted),
            )));
        }
    }

    // Mode indicator
    lines.push(Line::from(""));
    let mode_str = format!("{}", app.operating_mode);
    let mode_color = match mode_str.as_str() {
        "Camp" => t.mode_camp,
        "Hunt" => t.mode_hunt,
        _      => t.text_muted,
    };
    lines.push(Line::from(vec![
        Span::styled("  Mode: ", Style::default().fg(t.text_muted)),
        Span::styled(mode_str, Style::default().fg(mode_color).add_modifier(Modifier::BOLD)),
    ]));

    frame.render_widget(Paragraph::new(lines), inner);
}

// ─── Screen 6: Navigation ───────────────────────────────────────────────────

fn draw_navigation_screen(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Left: nav status per character
    let blk = panel(" Navigation Status ", t.border_primary, t);
    let visible = app.visible_clients();

    if visible.is_empty() {
        frame.render_widget(
            Paragraph::new("No characters connected").block(blk).style(Style::default().fg(t.text_muted)),
            cols[0],
        );
    } else {
        let header = themed_header_row(vec!["", "Character", "Zone", "Status", "Destination"], t);

        let rows: Vec<Row> = visible.iter().enumerate().map(|(i, client)| {
            let is_sel  = i == app.nav_selected;
            let marker  = if is_sel { "▶" } else { " " };
            let name    = client.local_player.as_ref()
                .map(|p| app.redact_name(&p.displayed_name).into_owned())
                .unwrap_or_else(|| format!("PID {}", client.pid));

            let nav = app.nav_statuses.get(&client.pid);
            let status_str = nav.map(|s| s.status.as_str()).unwrap_or("Idle");
            let dest_str   = nav.map(|s| s.destination.as_str()).unwrap_or("—");

            let status_color = match status_str {
                "Navigating" => t.text_highlight,
                "Arrived"    => t.hp_high,
                "Stuck"      => t.hp_low,
                _            => t.text_muted,
            };

            let row_style = if is_sel {
                Style::default().bg(t.row_selected_bg).add_modifier(Modifier::BOLD)
            } else { Style::default() };

            Row::new(vec![
                Cell::from(marker).style(Style::default().fg(t.text_accent)),
                Cell::from(name).style(Style::default().fg(t.text_normal)),
                Cell::from(client.zone_name.as_str()).style(Style::default().fg(t.text_secondary)),
                Cell::from(status_str).style(Style::default().fg(status_color)),
                Cell::from(dest_str).style(Style::default().fg(t.text_accent)),
            ]).style(row_style)
        }).collect();

        frame.render_widget(
            Table::new(rows, [
                Constraint::Length(2), Constraint::Min(14),
                Constraint::Min(14),   Constraint::Length(12), Constraint::Min(14),
            ])
            .header(header)
            .block(blk),
            cols[0],
        );
    }

    // Right: commands reference
    let mode_str = format!("{}", app.operating_mode);
    let mode_color = match mode_str.as_str() {
        "Camp" => t.mode_camp,
        "Hunt" => t.mode_hunt,
        _      => t.text_normal,
    };

    let key = |k: &'static str| Span::styled(k, Style::default().fg(t.text_accent));
    let sep = || Span::styled("  ", Style::default());
    let cmd_style = Style::default().fg(t.text_highlight);

    let mut info_lines = vec![
        Line::from(Span::styled("Operating Mode", Style::default().fg(t.text_accent).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Mode: ", Style::default().fg(t.text_muted)),
            Span::styled(&mode_str, Style::default().fg(mode_color).add_modifier(Modifier::BOLD)),
        ]),
    ];

    if let Some(ma) = &app.main_assist {
        info_lines.push(Line::from(vec![
            Span::styled("  MA:   ", Style::default().fg(t.text_muted)),
            Span::styled(ma.as_str(), Style::default().fg(t.text_highlight)),
        ]));
    }
    if let Some(mt) = &app.main_tank {
        info_lines.push(Line::from(vec![
            Span::styled("  MT:   ", Style::default().fg(t.text_muted)),
            Span::styled(mt.as_str(), Style::default().fg(t.hp_low)),
        ]));
    }

    info_lines.push(Line::from(""));
    info_lines.push(Line::from(Span::styled("Nav Commands", Style::default().fg(t.text_accent).add_modifier(Modifier::BOLD))));
    info_lines.push(Line::from(""));

    for (cmd, desc) in &[
        (":mode camp ",  "Camp mode"),
        (":mode hunt ",  "Hunt mode"),
        (":camp start",  "Start camp"),
        (":camp stop ",  "Stop camp"),
        (":camp next ",  "Next waypoint"),
        (":camp prev ",  "Prev waypoint"),
    ] {
        info_lines.push(Line::from(vec![
            Span::styled(*cmd, cmd_style),
            sep(),
            Span::styled(*desc, Style::default().fg(t.text_secondary)),
        ]));
    }

    info_lines.push(Line::from(""));
    info_lines.push(Line::from(Span::styled("Group Commands", Style::default().fg(t.text_accent).add_modifier(Modifier::BOLD))));
    info_lines.push(Line::from(""));

    for (cmd, desc) in &[
        (":invite <n>", "Invite to group"),
        (":accept    ", "Accept invite"),
        (":ma <name> ", "Main Assist"),
        (":mt <name> ", "Main Tank"),
    ] {
        info_lines.push(Line::from(vec![
            Span::styled(*cmd, cmd_style),
            sep(),
            Span::styled(*desc, Style::default().fg(t.text_secondary)),
        ]));
    }

    let _ = key; // suppress warning

    frame.render_widget(
        Paragraph::new(info_lines).block(panel(" Commands & Mode ", t.border_warn, t)),
        cols[1],
    );
}

// ─── Shared panels ──────────────────────────────────────────────────────────

fn draw_target_panel(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let target = app.active_client().and_then(|c| c.target.as_ref());
    draw_spawn_panel(frame, area, target, " Current Target ", t.border_danger, "No target", app);
}

fn draw_spawn_panel(
    frame: &mut Frame,
    area: Rect,
    spawn: Option<&SpawnInfo>,
    title: &str,
    border_style: Style,
    empty_msg: &str,
    app: &App,
) {
    let t = &app.theme;
    let blk = panel(title, border_style, t);

    if let Some(info) = spawn {
        let lines = spawn_info_lines(info, app);
        frame.render_widget(Paragraph::new(lines).block(blk).wrap(Wrap { trim: true }), area);
    } else {
        frame.render_widget(
            Paragraph::new(empty_msg).block(blk).style(Style::default().fg(t.text_muted)),
            area,
        );
    }
}

fn spawn_info_lines(spawn: &SpawnInfo, app: &App) -> Vec<Line<'static>> {
    let t = &app.theme;
    let hp_pct  = spawn.hp_pct();
    let hp_col  = hp_color(hp_pct, t);
    let name    = app.redact_name(&spawn.displayed_name).into_owned();
    let rawname = app.redact_name(&spawn.name).into_owned();

    vec![
        Line::from(vec![
            Span::styled(name, Style::default().fg(t.text_bright).add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(format!("{} Lv{}", spawn.class_str(), spawn.level), Style::default().fg(t.text_accent)),
            Span::raw(format!("  [{}]  {}", spawn.spawn_type, spawn.stand_state)),
        ]),
        Line::from(vec![
            Span::styled("HP   ", Style::default().fg(t.text_muted)),
            Span::styled(format!("{}/{} ({:.0}%)", spawn.hp_current, spawn.hp_max, hp_pct), Style::default().fg(hp_col)),
        ]),
        Line::from(vec![
            Span::styled("Mana ", Style::default().fg(t.text_muted)),
            Span::styled(format!("{}/{}", spawn.mana_current, spawn.mana_max), Style::default().fg(t.mana_color)),
            Span::styled(format!("  End {}/{}", spawn.endurance_current, spawn.endurance_max), Style::default().fg(t.text_muted)),
        ]),
        Line::from(vec![
            Span::styled("Pos  ", Style::default().fg(t.text_muted)),
            Span::styled(format!("({:.1}, {:.1}, {:.1})", spawn.y, spawn.x, spawn.z), Style::default().fg(t.text_server)),
            Span::styled(format!("  Hdg {:.1}", spawn.heading), Style::default().fg(t.text_muted)),
        ]),
        Line::from(vec![
            Span::styled(format!("ID {} ", spawn.spawn_id), Style::default().fg(t.text_muted)),
            Span::styled(rawname, Style::default().fg(t.text_secondary)),
        ]),
    ]
}

fn draw_hex_panel(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let is_active    = app.active_panel == ActivePanel::HexDump;
    let border_style = if is_active { t.border_warn } else { t.border_dim };
    let blk          = panel(format!(" Hex — {} ", app.hex_label), border_style, t);

    if app.hex_data.is_empty() {
        frame.render_widget(
            Paragraph::new("Select a spawn and press Enter to inspect memory")
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
            if offset >= app.hex_data.len() { return None; }
            let addr  = app.hex_address + offset;
            let end   = (offset + 16).min(app.hex_data.len());
            let chunk = &app.hex_data[offset..end];

            let hex_str: String   = chunk.iter().map(|b| format!("{:02x} ", b)).collect();
            let ascii_str: String = chunk.iter().map(|&b| {
                if b.is_ascii_graphic() || b == b' ' { b as char } else { '·' }
            }).collect();

            Some(Line::from(vec![
                Span::styled(format!("{:08x}", addr), Style::default().fg(t.text_muted)),
                Span::raw("  "),
                Span::styled(format!("{:<48}", hex_str), Style::default().fg(t.text_normal)),
                Span::raw(" "),
                Span::styled(ascii_str, Style::default().fg(t.text_highlight)),
            ]))
        })
        .collect();

    frame.render_widget(Paragraph::new(lines).block(blk), area);
}

fn draw_spawn_list(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let is_active    = app.active_panel == ActivePanel::SpawnList;
    let border_style = if is_active { t.border_active } else { t.border_dim };

    let filtered = app.filtered_spawns();
    let client_label = app.active_client()
        .and_then(|c| c.local_player.as_ref())
        .map(|p| app.redact_name(&p.displayed_name).into_owned())
        .unwrap_or_else(|| "???".into());

    let filter_label = app.spawn_type_filter.label();
    let title = if app.search_mode {
        format!(" Spawns: {} ({}) [{}] search: \"{}\" ", client_label, filtered.len(), filter_label, app.spawn_filter)
    } else if !app.spawn_filter.is_empty() {
        format!(" Spawns: {} ({}) [{}] filter: \"{}\" ", client_label, filtered.len(), filter_label, app.spawn_filter)
    } else if app.spawn_type_filter != super::app::SpawnFilter::All {
        format!(" Spawns: {} ({}) [{}] ", client_label, filtered.len(), filter_label)
    } else {
        format!(" Spawns: {} ({}) ", client_label, filtered.len())
    };

    let header = themed_header_row(vec!["Type", "Name", "Cls", "Lv", "HP%", "ID"], t);

    let visible_rows  = area.height.saturating_sub(3) as usize;
    let scroll_offset = if visible_rows > 0 && app.spawn_selected >= visible_rows {
        app.spawn_selected - visible_rows + 1
    } else { 0 };

    let rows: Vec<Row> = filtered.iter().enumerate()
        .skip(scroll_offset)
        .take(visible_rows)
        .map(|(i, spawn)| {
            let is_sel = i == app.spawn_selected;
            let style  = if is_sel {
                Style::default().bg(t.row_selected_bg).add_modifier(Modifier::BOLD)
            } else {
                spawn_row_style(spawn, t)
            };
            let name = app.redact_name(&spawn.displayed_name);
            Row::new(vec![
                Cell::from(spawn.spawn_type.to_string()),
                Cell::from(name.into_owned()),
                Cell::from(spawn.class_str()),
                Cell::from(spawn.level.to_string()),
                Cell::from(format!("{:.0}%", spawn.hp_pct())),
                Cell::from(spawn.spawn_id.to_string()),
            ]).style(style)
        }).collect();

    frame.render_widget(
        Table::new(rows, [
            Constraint::Length(7),  // Type
            Constraint::Min(20),    // Name
            Constraint::Length(4),  // Class
            Constraint::Length(4),  // Level
            Constraint::Length(6),  // HP%
            Constraint::Length(8),  // ID
        ])
        .header(header)
        .block(panel(title.as_str(), border_style, t))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        area,
    );
}

fn spawn_row_style(spawn: &SpawnInfo, t: &Theme) -> Style {
    Style::default().fg(spawn_type_color(&spawn.spawn_type, t))
}

// ─── Status bar ─────────────────────────────────────────────────────────────

fn draw_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    if app.command_mode {
        let cmd = format!(": {}_", app.command_buffer);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(cmd, t.statusbar_cmd)))
                .block(panel("", t.border_active, t)),
            area,
        );
        return;
    }

    // Badges for active indicators
    let mut right_spans: Vec<Span<'_>> = Vec::new();

    let mode_str = format!("{}", app.operating_mode);
    let mode_color = match mode_str.as_str() {
        "Camp" => t.mode_camp,
        "Hunt" => t.mode_hunt,
        _      => t.text_muted,
    };
    right_spans.push(Span::styled(format!(" {} ", mode_str), Style::default().fg(Color::Black).bg(mode_color).add_modifier(Modifier::BOLD)));
    right_spans.push(Span::raw(" "));

    let filter = app.spawn_type_filter.label();
    if filter != "All" {
        right_spans.push(Span::styled(
            format!(" {} ", filter),
            Style::default().fg(Color::Black).bg(t.text_accent).add_modifier(Modifier::BOLD),
        ));
        right_spans.push(Span::raw(" "));
    }

    if app.privacy_mode {
        right_spans.push(Span::styled(" PRIVATE ", t.statusbar_badge));
        right_spans.push(Span::raw(" "));
    }

    if let Some(idx) = app.active_group {
        right_spans.push(Span::styled(
            format!(" G{} ", idx + 1),
            Style::default().fg(Color::Black).bg(t.text_accent),
        ));
        right_spans.push(Span::raw(" "));
    }

    right_spans.push(Span::styled(format!(" {} ", app.theme_kind.label()), Style::default().fg(t.text_muted)));

    // Key hints (compact)
    let hints = vec![
        Span::styled("1-6", t.statusbar_key),
        Span::styled(" screen  ", t.statusbar_dim),
        Span::styled("⇧1-6", t.statusbar_key),
        Span::styled(" group  ", t.statusbar_dim),
        Span::styled("[ ]", t.statusbar_key),
        Span::styled(" client  ", t.statusbar_dim),
        Span::styled("/", t.statusbar_key),
        Span::styled(" search  ", t.statusbar_dim),
        Span::styled("f", t.statusbar_key),
        Span::styled(" filter  ", t.statusbar_dim),
        Span::styled("T", t.statusbar_key),
        Span::styled(" theme  ", t.statusbar_dim),
        Span::styled("?", t.statusbar_key),
        Span::styled(" help", t.statusbar_dim),
    ];

    // Left: status message | hints | right badges
    let left = Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(app.status_message.as_str(), t.statusbar_message),
        Span::styled("  │  ", t.statusbar_dim),
    ].into_iter().chain(hints).collect::<Vec<_>>());

    let right_line = Line::from(right_spans);

    // Split into two columns: left (hints) and right (badges)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(10), Constraint::Length(30)])
        .split(area);

    frame.render_widget(
        Paragraph::new(left).block(panel("", t.border_dim, t)),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(right_line).block(Block::default().borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM).border_type(t.border_type).border_style(t.border_dim)),
        cols[1],
    );
}

// ─── Help overlay ───────────────────────────────────────────────────────────

fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    let popup_w = 48u16;
    let popup_h = 36u16;
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup_area = Rect::new(x, y, popup_w.min(area.width), popup_h.min(area.height));

    frame.render_widget(Clear, popup_area);

    // Use plain colors so overlay is always readable regardless of theme
    let key_s  = Style::default().fg(Color::Rgb(0, 200, 210));
    let desc_s = Style::default().fg(Color::Rgb(180, 180, 190));
    let head_s = Style::default().fg(Color::Rgb(0, 200, 210)).add_modifier(Modifier::BOLD);
    let dim_s  = Style::default().fg(Color::Rgb(80, 85, 95));

    let kv = |k: &'static str, v: &'static str| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!(" {:<11}", k), key_s),
            Span::styled(v, desc_s),
        ])
    };

    let help_text = vec![
        Line::from(Span::styled(" Keybindings", head_s)),
        Line::from(""),
        kv("1-6",       "Switch screens"),
        kv("Shift+1-6", "Focus group G1–G6"),
        kv("Shift+0",   "All groups"),
        kv("[ ]",       "Cycle clients"),
        kv("/",         "Search spawns"),
        kv("f",         "Filter spawn type"),
        kv("p",         "Privacy mode"),
        kv("T",         "Cycle theme"),
        kv(":",         "Command mode"),
        kv("?",         "This help"),
        kv("q",         "Quit"),
        Line::from(""),
        Line::from(Span::styled(" Commands  (:cmd)", head_s)),
        Line::from(""),
        kv("<pid> /cmd", "Send to PID"),
        kv("G1-G6 /cmd", "Send to group"),
        kv("all /cmd",  "Broadcast"),
        kv("camp <sub>","start|stop|list|add|rm"),
        kv("track <n>", "Track spawn"),
        kv("ma <name>", "Set Main Assist"),
        kv("mt <name>", "Set Main Tank"),
        kv("engage",    "Start combat"),
        kv("disengage", "Stop combat"),
        kv("invite <n>","Group invite"),
        kv("accept",    "Accept invite"),
        kv("mode camp", "Camp mode"),
        kv("mode hunt", "Hunt mode"),
        Line::from(""),
        Line::from(Span::styled(" Press ? or Esc to close", dim_s)),
    ];

    let blk = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(" Help ", Style::default().fg(Color::Rgb(0, 200, 210)).add_modifier(Modifier::BOLD)))
        .border_style(Style::default().fg(Color::Rgb(0, 200, 210)))
        .style(Style::default().bg(Color::Rgb(15, 18, 24)));

    frame.render_widget(Paragraph::new(help_text).block(blk), popup_area);
}
