use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

use super::app::{ActivePanel, ActiveScreen, App};
use super::sprites;
use crate::eq::structs::{SpawnInfo, SpawnType};

/// Main render function — dispatches to the active screen.
pub fn draw(frame: &mut Frame, app: &App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header + tab bar
            Constraint::Min(10),   // Body
            Constraint::Length(3), // Status bar
        ])
        .split(frame.area());

    draw_header(frame, outer[0], app);

    match app.active_screen {
        ActiveScreen::Dashboard => draw_dashboard(frame, outer[1], app),
        ActiveScreen::Spawns => draw_spawns_screen(frame, outer[1], app),
        ActiveScreen::Character => draw_character_screen(frame, outer[1], app),
        ActiveScreen::Map => draw_map_screen(frame, outer[1], app),
        ActiveScreen::Groups => draw_groups_screen(frame, outer[1], app),
    }

    draw_status_bar(frame, outer[2], app);
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let client_count = app.clients.len();
    let client_str = if client_count > 0 {
        format!("{}x EQ", client_count)
    } else {
        "Not attached".into()
    };

    let selected_str = if let Some(client) = app.active_client() {
        let player_name = client
            .local_player
            .as_ref()
            .map(|p| app.redact_name(&p.displayed_name).into_owned())
            .unwrap_or_else(|| "???".into());
        format!(
            "[{}/{}] {}",
            app.selected_client + 1,
            client_count,
            player_name
        )
    } else {
        String::from("No client selected")
    };

    let server_str = format!(" {} ", app.display_server());

    let zone_str = app
        .active_client()
        .map(|c| {
            if c.zone_name.is_empty() {
                "Unknown Zone".to_string()
            } else {
                c.zone_name.clone()
            }
        })
        .unwrap_or_else(|| "No Zone".to_string());

    // Build tab bar spans
    let mut tabs: Vec<Span<'_>> = Vec::new();
    tabs.push(Span::raw(" "));
    for screen in &ActiveScreen::ALL {
        let is_active = *screen == app.active_screen;
        let label = format!(" {}:{} ", screen.key(), screen.label());
        if is_active {
            tabs.push(Span::styled(
                label,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            tabs.push(Span::styled(label, Style::default().fg(Color::DarkGray)));
        }
    }

    let mut spans = vec![
        Span::styled(
            &client_str,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(
            &selected_str,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(&server_str, Style::default().fg(Color::Magenta)),
        Span::raw("| "),
        Span::styled(
            format!(" {} ", zone_str),
            Style::default().fg(Color::White),
        ),
        Span::raw(" |"),
    ];
    spans.extend(tabs);

    let header = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" EQ Multibox Controller "),
    );

    frame.render_widget(header, area);
}

// ─── Screen 1: Dashboard ────────────────────────────────────────────

fn draw_dashboard(frame: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Character grid
            Constraint::Percentage(40), // Group health bars + server info
        ])
        .split(area);

    draw_dashboard_grid(frame, cols[0], app);
    draw_dashboard_sidebar(frame, cols[1], app);
}

fn draw_dashboard_grid(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Characters ({}) ", app.clients.len()))
        .border_style(Style::default().fg(Color::Green));

    if app.clients.is_empty() {
        let paragraph = Paragraph::new("No characters connected")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Cls").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Lv").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("HP%").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("MP%").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("State").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Zone").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .height(1);

    let rows: Vec<Row> = app
        .clients
        .iter()
        .enumerate()
        .map(|(i, client)| {
            let is_selected = i == app.selected_client;
            let marker = if is_selected { ">" } else { " " };

            if let Some(player) = &client.local_player {
                let hp_pct = player.hp_pct();
                let mana_pct = player.mana_pct();
                let display_name = app.redact_name(&player.displayed_name).into_owned();

                let style = if is_selected {
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    Cell::from(marker).style(Style::default().fg(Color::Cyan)),
                    Cell::from(display_name),
                    Cell::from(player.class_str()).style(Style::default().fg(Color::Cyan)),
                    Cell::from(player.level.to_string()),
                    Cell::from(format!("{:.0}%", hp_pct)).style(Style::default().fg(hp_color(hp_pct))),
                    Cell::from(if player.mana_max > 0 {
                        format!("{:.0}%", mana_pct)
                    } else {
                        "-".into()
                    })
                    .style(Style::default().fg(Color::Blue)),
                    Cell::from(player.stand_state.label()).style(
                        Style::default().fg(stand_state_color(&player.stand_state)),
                    ),
                    Cell::from(client.zone_name.as_str()).style(Style::default().fg(Color::DarkGray)),
                ])
                .style(style)
            } else {
                Row::new(vec![
                    Cell::from(marker).style(Style::default().fg(Color::Cyan)),
                    Cell::from(format!("PID {}", client.pid))
                        .style(Style::default().fg(Color::DarkGray)),
                    Cell::from("-"),
                    Cell::from("-"),
                    Cell::from("-"),
                    Cell::from("-"),
                    Cell::from("-"),
                    Cell::from(&*client.zone_name),
                ])
            }
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(1),  // marker
            Constraint::Min(14),    // Name
            Constraint::Length(4),  // Class
            Constraint::Length(3),  // Level
            Constraint::Length(5),  // HP%
            Constraint::Length(5),  // MP%
            Constraint::Length(6),  // State
            Constraint::Min(12),    // Zone
        ],
    )
    .header(header)
    .block(block);

    frame.render_widget(table, area);
}

fn draw_dashboard_sidebar(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),    // Group health bars
            Constraint::Length(6), // Server info
        ])
        .split(area);

    // Group health bars
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Group Health ")
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);

    let bar_width = inner.width.saturating_sub(16) as usize; // name(12) + space + bar + pct
    let mut lines: Vec<Line<'_>> = Vec::new();

    for (i, client) in app.clients.iter().enumerate() {
        if let Some(player) = &client.local_player {
            let hp_pct = player.hp_pct();
            let filled = ((hp_pct / 100.0) * bar_width as f64) as usize;
            let empty = bar_width.saturating_sub(filled);
            let color = hp_color(hp_pct);
            let is_selected = i == app.selected_client;

            let display_name = app.redact_name(&player.displayed_name).into_owned();
            let name_style = if is_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            lines.push(Line::from(vec![
                Span::styled(format!("{:<12} ", display_name), name_style),
                Span::styled(
                    "\u{2588}".repeat(filled),
                    Style::default().fg(color),
                ),
                Span::styled(
                    "\u{2591}".repeat(empty),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!(" {:>3.0}%", hp_pct),
                    Style::default().fg(color),
                ),
            ]));
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);

    // Server info
    let server_info = vec![
        Line::from(vec![
            Span::raw("Server: "),
            Span::styled(app.display_server(), Style::default().fg(Color::Magenta)),
        ]),
        Line::from(vec![
            Span::raw("Clients: "),
            Span::styled(
                app.clients.len().to_string(),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::raw("Refresh: "),
            Span::styled(
                format!("{}ms", app.refresh_rate_ms),
                Style::default().fg(Color::Cyan),
            ),
        ]),
    ];

    let info_block = Block::default()
        .borders(Borders::ALL)
        .title(" Server Info ")
        .border_style(Style::default().fg(Color::Magenta));
    let paragraph = Paragraph::new(server_info).block(info_block);
    frame.render_widget(paragraph, chunks[1]);
}

// ─── Screen 2: Spawns ───────────────────────────────────────────────

fn draw_spawns_screen(frame: &mut Frame, area: Rect, app: &App) {
    draw_spawn_list(frame, area, app);
}

// ─── Screen 3: Character ────────────────────────────────────────────

fn draw_character_screen(frame: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(45), // Left: player + target
            Constraint::Percentage(55), // Right: hex dump
        ])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),    // Player info + sprite
            Constraint::Length(8),  // Target info
        ])
        .split(cols[0]);

    draw_player_detail(frame, left[0], app);
    draw_target_panel(frame, left[1], app);
    draw_hex_panel(frame, cols[1], app);
}

fn draw_player_detail(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Selected Character ")
        .border_style(Style::default().fg(Color::Green));

    let client = match app.active_client() {
        Some(c) => c,
        None => {
            let paragraph = Paragraph::new("No client selected")
                .block(block)
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(paragraph, area);
            return;
        }
    };

    let player = match &client.local_player {
        Some(p) => p,
        None => {
            let paragraph = Paragraph::new("Not logged in")
                .block(block)
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(paragraph, area);
            return;
        }
    };

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let display_name = app.redact_name(&player.displayed_name).into_owned();
    let hp_pct = player.hp_pct();
    let mana_pct = player.mana_pct();

    let mut lines: Vec<Line<'_>> = Vec::new();

    // Name + class + level
    lines.push(Line::from(vec![
        Span::styled(
            &display_name,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{} Lv{}", player.class_str(), player.level),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(format!("  [{}]", player.stand_state)),
    ]));

    // HP / Mana / Endurance
    lines.push(Line::from(vec![
        Span::raw("HP: "),
        Span::styled(
            format!(
                "{}/{} ({:.0}%)",
                player.hp_current, player.hp_max, hp_pct
            ),
            Style::default().fg(hp_color(hp_pct)),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::raw("Mana: "),
        Span::styled(
            format!(
                "{}/{} ({:.0}%)",
                player.mana_current, player.mana_max, mana_pct
            ),
            Style::default().fg(Color::Blue),
        ),
        Span::raw(format!(
            "  End: {}/{}",
            player.endurance_current, player.endurance_max
        )),
    ]));

    // Position
    lines.push(Line::from(vec![
        Span::raw("Pos: "),
        Span::styled(
            format!("({:.1}, {:.1}, {:.1})", player.y, player.x, player.z),
            Style::default().fg(Color::Magenta),
        ),
        Span::raw(format!("  Hdg: {:.1}", player.heading)),
    ]));
    lines.push(Line::from(vec![
        Span::raw("Zone: "),
        Span::styled(&client.zone_name, Style::default().fg(Color::White)),
    ]));

    // Pixel art class sprite
    lines.push(Line::from(""));
    let sprite_lines = sprites::class_sprite(player.class.as_ref(), &player.stand_state, app.tick_count);
    for sprite_line in sprite_lines {
        lines.push(sprite_line);
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

// ─── Screen 4: Map ──────────────────────────────────────────────────

fn draw_map_screen(frame: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55), // Map view
            Constraint::Percentage(25), // Spawn position list
            Constraint::Percentage(20), // Named tracker panel
        ])
        .split(area);

    draw_map_view(frame, cols[0], app);
    draw_map_spawn_list(frame, cols[1], app);
    draw_named_tracker_panel(frame, cols[2], app);
}

fn draw_map_view(frame: &mut Frame, area: Rect, app: &App) {
    let zone_label = app
        .active_client()
        .map(|c| c.zone_name.as_str())
        .unwrap_or("Unknown");
    let map_info = app
        .zone_map
        .as_ref()
        .map(|m| format!(" Map: {} ({} lines, {} labels) ", zone_label, m.lines.len(), m.points.len()))
        .unwrap_or_else(|| format!(" Map: {} (no map data) ", zone_label));

    let block = Block::default()
        .borders(Borders::ALL)
        .title(map_info)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let w = inner.width as usize;
    let h = inner.height as usize;
    if w == 0 || h == 0 {
        return;
    }

    // Each cell: (char, Color)
    let mut grid: Vec<Vec<(char, Color)>> = vec![vec![(' ', Color::DarkGray); w]; h];

    // Determine center and scale from zone map or spawn data
    let (center_x, center_y, scale_x, scale_y) = if let Some(map) = &app.zone_map {
        // Center on player if available, otherwise center on map bounds
        let (cx, cy) = if let Some(player) = &app.local_player {
            // Map coords: (-locY, -locX)
            (-player.y, -player.x)
        } else {
            (map.bounds.center_x(), map.bounds.center_y())
        };

        // Scale to fit the map with some margin, using the larger dimension
        let map_w = map.bounds.width();
        let map_h = map.bounds.height();
        // Use uniform scale based on the dimension that needs more room
        let sx = (w as f32 - 2.0) / map_w;
        let sy = (h as f32 - 2.0) / map_h;
        let uniform_scale = sx.min(sy);
        (cx, cy, uniform_scale, uniform_scale)
    } else {
        // Fallback: use spawn bounding box (old behavior)
        let spawns = &app.spawns;
        if spawns.is_empty() {
            let paragraph = Paragraph::new("No map or spawn data")
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(paragraph, inner);
            return;
        }
        let (mut min_x, mut max_x, mut min_y, mut max_y) =
            (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for s in spawns {
            let mx = -s.y;
            let my = -s.x;
            if mx < min_x { min_x = mx; }
            if mx > max_x { max_x = mx; }
            if my < min_y { min_y = my; }
            if my > max_y { max_y = my; }
        }
        let range_x = (max_x - min_x).max(1.0);
        let range_y = (max_y - min_y).max(1.0);
        let cx = (min_x + max_x) / 2.0;
        let cy = (min_y + max_y) / 2.0;
        let sx = (w as f32 - 2.0) / range_x;
        let sy = (h as f32 - 2.0) / range_y;
        let uniform = sx.min(sy);
        (cx, cy, uniform, uniform)
    };

    // Helper: convert map coords to grid (col, row)
    let to_grid = |mx: f32, my: f32| -> (i32, i32) {
        let col = ((mx - center_x) * scale_x + w as f32 / 2.0) as i32;
        let row = ((my - center_y) * scale_y + h as f32 / 2.0) as i32;
        (col, row)
    };

    // Rasterize zone map lines using Bresenham's
    if let Some(map) = &app.zone_map {
        for ml in &map.lines {
            let (c1, r1) = to_grid(ml.x1, ml.y1);
            let (c2, r2) = to_grid(ml.x2, ml.y2);
            let color = map_rgb_to_color(ml.r, ml.g, ml.b);
            bresenham_line(c1, r1, c2, r2, w, h, &mut grid, color);
        }

        // Render point labels (just the first character as a marker)
        for mp in &map.points {
            let (col, row) = to_grid(mp.x, mp.y);
            if col >= 0 && col < w as i32 && row >= 0 && row < h as i32 {
                let color = map_rgb_to_color(mp.r, mp.g, mp.b);
                // Place a label marker and try to render abbreviated text
                let label_char = if mp.label.is_empty() { '*' } else { mp.label.chars().next().unwrap_or('*') };
                grid[row as usize][col as usize] = (label_char, color);

                // Render up to 12 chars of label text after the marker
                let label_text: String = mp.label.chars().take(12).collect();
                for (i, ch) in label_text.chars().enumerate() {
                    let lc = col as usize + 1 + i;
                    if lc < w && grid[row as usize][lc].0 == ' ' {
                        grid[row as usize][lc] = (ch, color);
                    }
                }
            }
        }
    }

    // Overlay spawns
    for spawn in &app.spawns {
        let mx = -spawn.y; // map X = -locY
        let my = -spawn.x; // map Y = -locX
        let (col, row) = to_grid(mx, my);
        if col >= 0 && col < w as i32 && row >= 0 && row < h as i32 {
            let (ch, color) = match spawn.spawn_type {
                SpawnType::Player => ('@', Color::Green),
                SpawnType::Npc => {
                    if !spawn.displayed_name.starts_with("a ")
                        && !spawn.displayed_name.starts_with("an ")
                    {
                        ('!', Color::Yellow)
                    } else {
                        ('*', Color::White)
                    }
                }
                SpawnType::Corpse => ('.', Color::DarkGray),
                SpawnType::Unknown(_) => ('?', Color::Red),
            };
            grid[row as usize][col as usize] = (ch, color);
        }
    }

    // Overlay dead named spawn positions from the tracker
    for status in app.named_tracker.tracked_spawns() {
        if !status.is_alive {
            let mx = -status.last_y;
            let my = -status.last_x;
            let (col, row) = to_grid(mx, my);
            if col >= 0 && col < w as i32 && row >= 0 && row < h as i32 {
                grid[row as usize][col as usize] = ('X', Color::Red);
            }
        }
    }

    // Mark local player on top
    if let Some(player) = &app.local_player {
        let mx = -player.y;
        let my = -player.x;
        let (col, row) = to_grid(mx, my);
        if col >= 0 && col < w as i32 && row >= 0 && row < h as i32 {
            grid[row as usize][col as usize] = ('+', Color::LightCyan);
        }
    }

    // Render grid to terminal — reserve last row for legend
    let legend_row = h.saturating_sub(1);
    let lines: Vec<Line<'_>> = grid
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            if i == legend_row {
                // Map legend
                Line::from(vec![
                    Span::styled("+ ", Style::default().fg(Color::LightCyan)),
                    Span::styled("You", Style::default().fg(Color::DarkGray)),
                    Span::raw(" | "),
                    Span::styled("@ ", Style::default().fg(Color::Green)),
                    Span::styled("PC", Style::default().fg(Color::DarkGray)),
                    Span::raw(" | "),
                    Span::styled("* ", Style::default().fg(Color::White)),
                    Span::styled("NPC", Style::default().fg(Color::DarkGray)),
                    Span::raw(" | "),
                    Span::styled("! ", Style::default().fg(Color::Yellow)),
                    Span::styled("Named", Style::default().fg(Color::DarkGray)),
                    Span::raw(" | "),
                    Span::styled("X ", Style::default().fg(Color::Red)),
                    Span::styled("Dead Named", Style::default().fg(Color::DarkGray)),
                    Span::raw(" | "),
                    Span::styled(". ", Style::default().fg(Color::DarkGray)),
                    Span::styled("Corpse", Style::default().fg(Color::DarkGray)),
                ])
            } else {
                Line::from(
                    row.into_iter()
                        .map(|(ch, color)| {
                            Span::styled(String::from(ch), Style::default().fg(color))
                        })
                        .collect::<Vec<_>>(),
                )
            }
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Map RGB from map file to a ratatui Color.
fn map_rgb_to_color(r: u8, g: u8, b: u8) -> Color {
    // Use true color for non-black colors; black lines become dark gray for visibility
    if r == 0 && g == 0 && b == 0 {
        Color::DarkGray
    } else {
        Color::Rgb(r, g, b)
    }
}

/// Bresenham's line algorithm — rasterize a line onto the character grid.
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

    // Safety limit to prevent runaway loops on huge off-screen lines
    let max_steps = (dx.unsigned_abs() + dy.unsigned_abs() + 1).min(10_000) as usize;

    for _ in 0..max_steps {
        // Plot if in bounds and cell is empty (don't overwrite spawns)
        if cx >= 0 && cx < w as i32 && cy >= 0 && cy < h as i32 {
            let ux = cx as usize;
            let uy = cy as usize;
            if grid[uy][ux].0 == ' ' {
                // Pick a line character based on slope direction
                let ch = line_char(x0, y0, x1, y1);
                grid[uy][ux] = (ch, color);
            }
        }

        if cx == x1 && cy == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            cx += sx;
        }
        if e2 <= dx {
            err += dx;
            cy += sy;
        }
    }
}

/// Choose a line-drawing character based on the line's overall direction.
fn line_char(x0: i32, y0: i32, x1: i32, y1: i32) -> char {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    if dx == 0 && dy == 0 {
        '·'
    } else if dy == 0 {
        '─'
    } else if dx == 0 {
        '│'
    } else if dx > dy * 2 {
        '─'
    } else if dy > dx * 2 {
        '│'
    } else {
        // Diagonal — pick / or \ based on slope direction
        let slope_positive = (x1 - x0).signum() != (y1 - y0).signum();
        if slope_positive { '/' } else { '\\' }
    }
}

fn draw_map_spawn_list(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Spawn Positions ")
        .border_style(Style::default().fg(Color::DarkGray));

    let header = Row::new(vec![
        Cell::from("T").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Y").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("X").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Z").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .height(1);

    let rows: Vec<Row> = app
        .spawns
        .iter()
        .map(|spawn| {
            let name = app.redact_name(&spawn.displayed_name).into_owned();
            let color = match spawn.spawn_type {
                SpawnType::Player => Color::Green,
                SpawnType::Npc => Color::White,
                SpawnType::Corpse => Color::DarkGray,
                SpawnType::Unknown(_) => Color::Red,
            };

            Row::new(vec![
                Cell::from(match spawn.spawn_type {
                    SpawnType::Player => "@",
                    SpawnType::Npc => "*",
                    SpawnType::Corpse => ".",
                    SpawnType::Unknown(_) => "?",
                }),
                Cell::from(name),
                Cell::from(format!("{:.0}", spawn.y)),
                Cell::from(format!("{:.0}", spawn.x)),
                Cell::from(format!("{:.0}", spawn.z)),
            ])
            .style(Style::default().fg(color))
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),  // Type marker
            Constraint::Min(14),    // Name
            Constraint::Length(7),  // Y
            Constraint::Length(7),  // X
            Constraint::Length(5),  // Z
        ],
    )
    .header(header)
    .block(block);

    frame.render_widget(table, area);
}

fn draw_named_tracker_panel(frame: &mut Frame, area: Rect, app: &App) {
    let tracked = app.named_tracker.tracked_spawns();
    let alive_count = tracked.iter().filter(|s| s.is_alive).count();
    let title = format!(" Named ({} up, {} tracked) ", alive_count, tracked.len());

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Yellow));

    if tracked.is_empty() {
        let paragraph = Paragraph::new("No named spawns detected")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Timer").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .height(1);

    let rows: Vec<Row> = tracked
        .iter()
        .map(|status| {
            let (status_str, color) = if status.is_alive {
                ("UP", Color::Green)
            } else {
                ("DEAD", Color::Red)
            };

            let timer_str = if status.is_alive {
                String::new()
            } else if let Some(respawn) = status.estimated_respawn_tick {
                let remaining = respawn.saturating_sub(app.tick_count);
                // Convert ticks to approximate seconds (250ms per tick)
                let secs = remaining / 4;
                let mins = secs / 60;
                let secs_rem = secs % 60;
                format!("~{}:{:02}", mins, secs_rem)
            } else {
                "???".into()
            };

            // Check if this is an HVT
            let is_hvt = app
                .hvt_watchlist
                .as_ref()
                .is_some_and(|wl| wl.is_hvt(&status.name).is_some());

            let name_style = if is_hvt {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            Row::new(vec![
                Cell::from(status.name.clone()).style(name_style),
                Cell::from(status_str).style(Style::default().fg(color)),
                Cell::from(timer_str).style(Style::default().fg(Color::Cyan)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(12),    // Name
            Constraint::Length(5),  // Status
            Constraint::Length(7),  // Timer
        ],
    )
    .header(header)
    .block(block);

    frame.render_widget(table, area);
}

// ─── Shared panels ──────────────────────────────────────────────────

fn draw_target_panel(frame: &mut Frame, area: Rect, app: &App) {
    let target = app.active_client().and_then(|c| c.target.as_ref());
    draw_spawn_panel(
        frame,
        area,
        target,
        " Current Target ",
        Color::Red,
        "No target",
        app,
    );
}

fn draw_spawn_panel(
    frame: &mut Frame,
    area: Rect,
    spawn: Option<&SpawnInfo>,
    title: &str,
    border_color: Color,
    empty_msg: &str,
    app: &App,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(border_color));

    if let Some(info) = spawn {
        let lines = spawn_info_lines(info, app);
        let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    } else {
        let paragraph = Paragraph::new(empty_msg)
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
    }
}

fn spawn_info_lines(spawn: &SpawnInfo, app: &App) -> Vec<Line<'static>> {
    let hp_pct = spawn.hp_pct();
    let hp_col = hp_color(hp_pct);
    let display_name = app.redact_name(&spawn.displayed_name).into_owned();
    let raw_name = app.redact_name(&spawn.name).into_owned();

    vec![
        Line::from(vec![
            Span::styled(
                display_name,
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} Lv{}", spawn.class_str(), spawn.level),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(format!("  [{}]", spawn.spawn_type)),
            Span::raw(format!("  {}", spawn.stand_state)),
        ]),
        Line::from(vec![
            Span::raw("HP: "),
            Span::styled(
                format!("{}/{} ({:.0}%)", spawn.hp_current, spawn.hp_max, hp_pct),
                Style::default().fg(hp_col),
            ),
        ]),
        Line::from(vec![
            Span::raw("Mana: "),
            Span::styled(
                format!("{}/{}", spawn.mana_current, spawn.mana_max),
                Style::default().fg(Color::Blue),
            ),
            Span::raw(format!(
                "  End: {}/{}",
                spawn.endurance_current, spawn.endurance_max
            )),
        ]),
        Line::from(vec![
            Span::raw("Pos: "),
            Span::styled(
                format!("({:.1}, {:.1}, {:.1})", spawn.y, spawn.x, spawn.z),
                Style::default().fg(Color::Magenta),
            ),
            Span::raw(format!("  Hdg: {:.1}", spawn.heading)),
        ]),
        Line::from(vec![Span::raw(format!(
            "ID: {}  Name: {}",
            spawn.spawn_id, raw_name
        ))]),
    ]
}

fn hp_color(hp_pct: f64) -> Color {
    if hp_pct > 75.0 {
        Color::Green
    } else if hp_pct > 25.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn stand_state_color(state: &crate::eq::structs::StandState) -> Color {
    use crate::eq::structs::StandState;
    match state {
        StandState::Dead => Color::Red,
        StandState::Sitting => Color::Yellow,
        StandState::Feigned => Color::Magenta,
        StandState::Frozen => Color::Blue,
        _ => Color::Green,
    }
}

fn draw_hex_panel(frame: &mut Frame, area: Rect, app: &App) {
    let is_active = app.active_panel == ActivePanel::HexDump;
    let border_color = if is_active {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Hex Dump — {} ", app.hex_label))
        .border_style(Style::default().fg(border_color));

    if app.hex_data.is_empty() {
        let paragraph = Paragraph::new("Select a spawn and press Enter to inspect memory")
            .block(block)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
        return;
    }

    let inner_height = area.height.saturating_sub(2) as usize;
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(inner_height);

    for row in 0..inner_height {
        let offset = row * 16;
        if offset >= app.hex_data.len() {
            break;
        }

        let addr = app.hex_address + offset;
        let end = (offset + 16).min(app.hex_data.len());
        let chunk = &app.hex_data[offset..end];

        let hex_str: String = chunk.iter().map(|b| format!("{:02x} ", b)).collect();
        let ascii_str: String = chunk
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();

        lines.push(Line::from(vec![
            Span::styled(
                format!("{:08x}", addr),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{:<48}", hex_str),
                Style::default().fg(Color::White),
            ),
            Span::raw(" "),
            Span::styled(ascii_str, Style::default().fg(Color::Yellow)),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_spawn_list(frame: &mut Frame, area: Rect, app: &App) {
    let is_active = app.active_panel == ActivePanel::SpawnList;
    let border_color = if is_active {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let filtered = app.filtered_spawns();

    let client_label = app
        .active_client()
        .and_then(|c| c.local_player.as_ref())
        .map(|p| app.redact_name(&p.displayed_name).into_owned())
        .unwrap_or_else(|| "???".into());

    let filter_label = app.spawn_type_filter.label();
    let title = if app.search_mode {
        format!(
            " Spawns: {} ({}) [{}] search: \"{}\" ",
            client_label,
            filtered.len(),
            filter_label,
            app.spawn_filter
        )
    } else if !app.spawn_filter.is_empty() {
        format!(
            " Spawns: {} ({}) [{}] filter: \"{}\" ",
            client_label,
            filtered.len(),
            filter_label,
            app.spawn_filter
        )
    } else if app.spawn_type_filter != super::app::SpawnFilter::All {
        format!(
            " Spawns: {} ({}) [{}] ",
            client_label,
            filtered.len(),
            filter_label
        )
    } else {
        format!(" Spawns: {} ({}) ", client_label, filtered.len())
    };

    let header = Row::new(vec![
        Cell::from("Type").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Cls").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Lv").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("HP%").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("ID").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .height(1);

    // Compute visible window: scroll so selected row stays on screen.
    // Available height = area height - 2 (borders) - 1 (header row).
    let visible_rows = area.height.saturating_sub(3) as usize;
    let scroll_offset = if visible_rows == 0 {
        0
    } else if app.spawn_selected >= visible_rows {
        app.spawn_selected - visible_rows + 1
    } else {
        0
    };

    let rows: Vec<Row> = filtered
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_rows)
        .map(|(i, spawn)| {
            let is_selected = i == app.spawn_selected;
            let style = if is_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                spawn_row_style(spawn)
            };

            let row_name = app.redact_name(&spawn.displayed_name);
            Row::new(vec![
                Cell::from(spawn.spawn_type.to_string()),
                Cell::from(row_name.into_owned()),
                Cell::from(spawn.class_str()),
                Cell::from(spawn.level.to_string()),
                Cell::from(format!("{:.0}%", spawn.hp_pct())),
                Cell::from(spawn.spawn_id.to_string()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(7), // Type
            Constraint::Min(20),   // Name
            Constraint::Length(4), // Class
            Constraint::Length(4), // Level
            Constraint::Length(6), // HP%
            Constraint::Length(8), // ID
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color)),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_widget(table, area);
}

fn spawn_row_style(spawn: &SpawnInfo) -> Style {
    match spawn.spawn_type {
        SpawnType::Player => Style::default().fg(Color::Green),
        SpawnType::Npc => Style::default().fg(Color::White),
        SpawnType::Corpse => Style::default().fg(Color::DarkGray),
        SpawnType::Unknown(_) => Style::default().fg(Color::Red),
    }
}

// ─── Screen 5: Groups ──────────────────────────────────────────────

fn draw_groups_screen(frame: &mut Frame, area: Rect, app: &App) {
    // 2 rows × 3 columns
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(rows[0]);

    let bot_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(rows[1]);

    let panels = [
        top_cols[0], top_cols[1], top_cols[2],
        bot_cols[0], bot_cols[1], bot_cols[2],
    ];

    for (i, group) in app.groups.iter().enumerate() {
        if i >= 6 {
            break;
        }
        draw_group_panel(frame, panels[i], app, group);
    }
}

/// Extract account number from a character name or window title.
/// Looks for trailing digits (e.g., "frostreaver05" → 5).
fn extract_account_number(name: &str) -> Option<u8> {
    let digits: String = name.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let digits: String = digits.chars().rev().collect();
    digits.parse().ok()
}

/// Get clients belonging to a group based on account number range.
fn clients_in_group<'a>(app: &'a App, group: &super::app::GroupDef) -> Vec<&'a super::app::ClientState> {
    let (lo, hi) = group.account_range;
    app.clients
        .iter()
        .filter(|c| {
            // Try character_name first, then player displayed_name
            let name = if !c.character_name.is_empty() {
                &c.character_name
            } else if let Some(p) = &c.local_player {
                &p.displayed_name
            } else {
                return false;
            };
            if let Some(num) = extract_account_number(name) {
                num >= lo && num <= hi
            } else {
                false
            }
        })
        .collect()
}

fn draw_group_panel(frame: &mut Frame, area: Rect, app: &App, group: &super::app::GroupDef) {
    let members = clients_in_group(app, group);
    let online_count = members.len();
    let (lo, hi) = group.account_range;
    let total_slots = (hi - lo + 1) as usize;

    let border_color = if online_count == total_slots {
        Color::Green
    } else if online_count > 0 {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let title = format!(" G{} {} ({}/{}) ", group.id, group.name, online_count, total_slots);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'_>> = Vec::new();

    // Build a map of account_num → client for quick lookup
    let mut slot_map: std::collections::HashMap<u8, &super::app::ClientState> =
        std::collections::HashMap::new();
    for client in &members {
        let name = if !client.character_name.is_empty() {
            &client.character_name
        } else if let Some(p) = &client.local_player {
            &p.displayed_name
        } else {
            continue;
        };
        if let Some(num) = extract_account_number(name) {
            slot_map.insert(num, client);
        }
    }

    for acct_num in lo..=hi {
        if let Some(client) = slot_map.get(&acct_num) {
            if let Some(player) = &client.local_player {
                let hp_pct = player.hp_pct();
                let display_name = app.redact_name(&player.displayed_name).into_owned();
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{:<12}", display_name),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        format!("{:<4}", player.class_str()),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(
                        format!("{:>3} ", player.level),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        format!("{:>3.0}%", hp_pct),
                        Style::default().fg(hp_color(hp_pct)),
                    ),
                ]));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("  PID {} (loading...)", client.pid),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        } else {
            lines.push(Line::from(Span::styled(
                format!("  #{:02} --- offline ---", acct_num),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    // Group status line
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Status: Idle",
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
    )));

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn draw_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    if app.command_mode {
        let cmd_line = format!(": {}_", app.command_buffer);
        let status = Paragraph::new(Line::from(vec![
            Span::styled(cmd_line, Style::default().fg(Color::Cyan)),
        ]))
        .block(Block::default().borders(Borders::ALL));
        frame.render_widget(status, area);
        return;
    }

    let privacy_indicator = if app.privacy_mode { " [PRIVATE]" } else { "" };
    let keybinds = format!(
        " 1-4:Screen | q:Quit | Tab:Panel | [/]:Client | Arrows:Nav | Enter:Inspect | Esc:Clear | /:Search | f:Filter({}) | p:Privacy{} | :Cmd",
        app.spawn_type_filter.label(),
        privacy_indicator
    );

    let status = Paragraph::new(Line::from(vec![
        Span::styled(&app.status_message, Style::default().fg(Color::Yellow)),
        Span::raw("  |  "),
        Span::styled(keybinds, Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::ALL));

    frame.render_widget(status, area);
}
