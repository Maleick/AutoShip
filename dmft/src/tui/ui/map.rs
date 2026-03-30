//! Map screen — zone map renderer, spawn position list, named tracker panel.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Row, Table},
};

use super::widgets::{panel, spawn_type_color, themed_header_row};
use crate::eq::structs::SpawnType;
use crate::tui::app::App;
use crate::tui::theme::Theme;

pub fn draw_map_screen(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
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

// ─── Map view ────────────────────────────────────────────────────────────────

fn draw_map_view(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    use ratatui::style::Color;
    let t = &app.theme;
    let zone_label = app
        .active_client()
        .map(|c| c.zone_name.as_str())
        .unwrap_or("Unknown");
    let map_info = app
        .zone_map
        .as_ref()
        .map(|m| {
            format!(
                " Map: {} ({} lines, {} labels) ",
                zone_label,
                m.lines.len(),
                m.points.len()
            )
        })
        .unwrap_or_else(|| format!(" Map: {} (no map data) ", zone_label));

    let blk = panel(map_info.as_str(), t.border_active, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let w = inner.width as usize;
    let h = inner.height as usize;
    if w == 0 || h == 0 {
        return;
    }

    let mut grid: Vec<Vec<(char, Color)>> = vec![vec![(' ', t.map_lines); w]; h];

    let (center_x, center_y, scale_x, scale_y) = if let Some(map) = &app.zone_map {
        let (cx, cy) = if let Some(player) = &app.local_player {
            (-player.y, -player.x)
        } else {
            (map.bounds.center_x(), map.bounds.center_y())
        };
        let s = ((w as f32 - 2.0) / map.bounds.width()).min((h as f32 - 2.0) / map.bounds.height());
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
            let mx = -s.y;
            let my = -s.x;
            min_x = min_x.min(mx);
            max_x = max_x.max(mx);
            min_y = min_y.min(my);
            max_y = max_y.max(my);
        }
        let cx = (min_x + max_x) / 2.0;
        let cy = (min_y + max_y) / 2.0;
        let u = ((w as f32 - 2.0) / (max_x - min_x).max(1.0))
            .min((h as f32 - 2.0) / (max_y - min_y).max(1.0));
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
                let ch = if mp.label.is_empty() {
                    '*'
                } else {
                    mp.label.chars().next().unwrap_or('*')
                };
                grid[row as usize][col as usize] = (ch, color);
                for (i, c) in mp.label.chars().take(12).enumerate() {
                    let lc = col as usize + 1 + i;
                    if lc < w && grid[row as usize][lc].0 == ' ' {
                        grid[row as usize][lc] = (c, color);
                    }
                }
            }
        }
    }

    for spawn in &app.spawns {
        let mx = -spawn.y;
        let my = -spawn.x;
        let (col, row) = to_grid(mx, my);
        if col >= 0 && col < w as i32 && row >= 0 && row < h as i32 {
            let (ch, color) = match spawn.spawn_type {
                SpawnType::Player => ('@', t.map_pc),
                SpawnType::Npc => {
                    if !spawn.displayed_name.starts_with("a ")
                        && !spawn.displayed_name.starts_with("an ")
                    {
                        ('!', t.map_named)
                    } else {
                        ('·', t.map_npc)
                    }
                }
                SpawnType::Corpse => ('.', t.map_corpse),
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
    let lines: Vec<Line<'_>> = grid
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            if i == legend_row {
                Line::from(vec![
                    Span::styled("◆ ", Style::default().fg(t.map_you)),
                    Span::styled("You", Style::default().fg(t.text_muted)),
                    Span::raw(" │ "),
                    Span::styled("@ ", Style::default().fg(t.map_pc)),
                    Span::styled("PC", Style::default().fg(t.text_muted)),
                    Span::raw(" │ "),
                    Span::styled("· ", Style::default().fg(t.map_npc)),
                    Span::styled("NPC", Style::default().fg(t.text_muted)),
                    Span::raw(" │ "),
                    Span::styled("! ", Style::default().fg(t.map_named)),
                    Span::styled("Named", Style::default().fg(t.text_muted)),
                    Span::raw(" │ "),
                    Span::styled("✕ ", Style::default().fg(t.map_dead_named)),
                    Span::styled("Dead", Style::default().fg(t.text_muted)),
                    Span::raw(" │ "),
                    Span::styled(". ", Style::default().fg(t.map_corpse)),
                    Span::styled("Corpse", Style::default().fg(t.text_muted)),
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

    frame.render_widget(Paragraph::new(lines), inner);
}

fn map_rgb_to_color(r: u8, g: u8, b: u8, t: &Theme) -> ratatui::style::Color {
    if r == 0 && g == 0 && b == 0 {
        t.map_lines
    } else {
        ratatui::style::Color::Rgb(r, g, b)
    }
}

#[allow(clippy::too_many_arguments)]
fn bresenham_line(
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    w: usize,
    h: usize,
    grid: &mut [Vec<(char, ratatui::style::Color)>],
    color: ratatui::style::Color,
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
            let (ux, uy) = (cx as usize, cy as usize);
            if grid[uy][ux].0 == ' ' {
                grid[uy][ux] = (line_char(x0, y0, x1, y1), color);
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
    } else if (x1 - x0).signum() != (y1 - y0).signum() {
        '╱'
    } else {
        '╲'
    }
}

// ─── Spawn position list ─────────────────────────────────────────────────────

fn draw_map_spawn_list(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let blk = panel(" Spawn Positions ", t.border_dim, t);
    let header = themed_header_row(vec!["T", "Name", "Y", "X", "Z"], t);

    let rows: Vec<Row> = app
        .spawns
        .iter()
        .map(|spawn| {
            let name = app.redact_name(&spawn.displayed_name).into_owned();
            let color = spawn_type_color(&spawn.spawn_type, t);
            Row::new(vec![
                ratatui::widgets::Cell::from(match spawn.spawn_type {
                    SpawnType::Player => "@",
                    SpawnType::Npc => "·",
                    SpawnType::Corpse => ".",
                    SpawnType::Unknown(_) => "?",
                }),
                ratatui::widgets::Cell::from(name),
                ratatui::widgets::Cell::from(format!("{:.0}", spawn.y)),
                ratatui::widgets::Cell::from(format!("{:.0}", spawn.x)),
                ratatui::widgets::Cell::from(format!("{:.0}", spawn.z)),
            ])
            .style(Style::default().fg(color))
        })
        .collect();

    frame.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(2),
                Constraint::Min(14),
                Constraint::Length(7),
                Constraint::Length(7),
                Constraint::Length(5),
            ],
        )
        .header(header)
        .block(blk),
        area,
    );
}

// ─── Named tracker panel ─────────────────────────────────────────────────────

fn draw_named_tracker_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let named = app.named_tracker.tracked_spawns();
    let n_alive = named.iter().filter(|s| s.is_alive).count();
    let u_tracked = &app.tracked_spawns;
    let u_up = u_tracked
        .values()
        .filter(|t| t.status == crate::tui::app::TrackedStatus::Up)
        .count();

    let has_user = !u_tracked.is_empty();
    let has_named = !named.is_empty();

    let (named_area, user_area) = if has_user && has_named {
        let ch = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);
        (Some(ch[0]), Some(ch[1]))
    } else if has_user {
        (None, Some(area))
    } else {
        (Some(area), None)
    };

    if let Some(na) = named_area {
        let title = format!(" Named ({} up / {}) ", n_alive, named.len());
        let blk = panel(title.as_str(), t.border_warn, t);

        if named.is_empty() {
            frame.render_widget(
                Paragraph::new("No named spawns detected")
                    .block(blk)
                    .style(Style::default().fg(t.text_muted)),
                na,
            );
        } else {
            let header = themed_header_row(vec!["Name", "St", "Timer"], t);
            let rows: Vec<Row> = named
                .iter()
                .map(|s| {
                    let (status_str, color) = if s.is_alive {
                        ("UP", t.hp_high)
                    } else {
                        ("down", t.text_muted)
                    };
                    let timer_str = if let Some(death_tick) = s.death_tick {
                        let elapsed_secs =
                            app.tick_count.saturating_sub(death_tick) * app.refresh_rate_ms / 1000;
                        format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60)
                    } else {
                        "-".into()
                    };

                    let name_style = if s.is_alive {
                        Style::default()
                            .fg(t.spawn_named)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(t.text_muted)
                    };
                    Row::new(vec![
                        ratatui::widgets::Cell::from(s.name.clone()).style(name_style),
                        ratatui::widgets::Cell::from(status_str).style(Style::default().fg(color)),
                        ratatui::widgets::Cell::from(timer_str).style(Style::default().fg(
                            if s.is_alive {
                                t.text_highlight
                            } else {
                                t.text_accent
                            },
                        )),
                    ])
                })
                .collect();

            frame.render_widget(
                Table::new(
                    rows,
                    [
                        Constraint::Min(12),
                        Constraint::Length(5),
                        Constraint::Length(7),
                    ],
                )
                .header(header)
                .block(blk),
                na,
            );
        }
    }

    if let Some(ua) = user_area {
        let title = format!(" Tracked ({} up / {}) ", u_up, u_tracked.len());
        let blk = panel(title.as_str(), t.border_server, t);
        let header = themed_header_row(vec!["Name", "St"], t);

        let mut sorted: Vec<_> = u_tracked.values().collect();
        sorted.sort_by(|a, b| {
            let ord = |s: &crate::tui::app::TrackedStatus| match s {
                crate::tui::app::TrackedStatus::Up => 0,
                crate::tui::app::TrackedStatus::Down => 1,
                crate::tui::app::TrackedStatus::Unknown => 2,
            };
            ord(&a.status)
                .cmp(&ord(&b.status))
                .then_with(|| a.name.cmp(&b.name))
        });

        let rows: Vec<Row> = sorted
            .iter()
            .map(|tr| {
                Row::new(vec![
                    ratatui::widgets::Cell::from(tr.name.clone())
                        .style(Style::default().fg(t.text_normal)),
                    ratatui::widgets::Cell::from(tr.status.label())
                        .style(Style::default().fg(tr.status.color())),
                ])
            })
            .collect();

        frame.render_widget(
            Table::new(rows, [Constraint::Min(12), Constraint::Length(5)])
                .header(header)
                .block(blk),
            ua,
        );
    }
}
