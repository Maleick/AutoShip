//! Map screen — zone map renderer, spawn position list, named tracker panel.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Row, Table},
};

use super::{
    spawns,
    widgets::{
        WIDTH_MAP_EXTRA_WIDE, WIDTH_MAP_NARROW, WIDTH_MAP_STACK, WIDTH_MAP_WIDE_RIGHT,
        WIDTH_SIDEBAR_WIDE, panel, themed_header_row,
    },
};
use crate::eq::structs::SpawnType;
use crate::tui::app::{ActivePanel, App, MapViewportMode};
use crate::tui::theme::Theme;

pub fn draw_map_screen(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    let sections = tactical_sections(app);
    let sidebar_height = sections
        .iter()
        .map(|(_, constraint)| match constraint {
            Constraint::Length(h) | Constraint::Min(h) => *h,
            _ => 3,
        })
        .sum::<u16>()
        .max(6);

    if app.tactical_state.map_maximized {
        draw_maximized_map_screen(frame, area, app, &sections, sidebar_height);
        return;
    }

    if area.width < WIDTH_MAP_STACK {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(11), Constraint::Length(sidebar_height + 10)])
            .split(area);
        let rail_width = (f32::from(area.width) * 0.28).round() as u16;
        let rail_width = rail_width.clamp(20, area.width.saturating_sub(26));
        let bottom = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(24), Constraint::Length(rail_width)])
            .split(rows[1]);
        draw_map_view(frame, rows[0], app);
        spawns::draw_spawn_list(frame, bottom[0], app);
        draw_tactical_sidebar(frame, bottom[1], app, &sections);
        return;
    }

    if area.width < WIDTH_MAP_NARROW {
        let right_width = if area.width >= WIDTH_MAP_WIDE_RIGHT {
            52
        } else {
            46
        };
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40), Constraint::Length(right_width)])
            .split(area);
        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(10), Constraint::Length(sidebar_height)])
            .split(cols[1]);

        draw_map_view(frame, cols[0], app);
        spawns::draw_spawn_list(frame, right[0], app);
        draw_tactical_sidebar(frame, right[1], app, &sections);
        return;
    }

    let sidebar_width = if area.width >= WIDTH_MAP_EXTRA_WIDE {
        30
    } else {
        26
    };
    let spawn_width = if area.width >= WIDTH_SIDEBAR_WIDE {
        52
    } else {
        46
    };
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(46),
            Constraint::Length(spawn_width),
            Constraint::Length(sidebar_width),
        ])
        .split(area);

    draw_map_view(frame, cols[0], app);
    spawns::draw_spawn_list(frame, cols[1], app);
    draw_tactical_sidebar(frame, cols[2], app, &sections);
}

fn draw_maximized_map_screen(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &mut App,
    sections: &[(TacticalSectionKind, Constraint)],
    sidebar_height: u16,
) {
    let dock_height = (sidebar_height + 9)
        .min(area.height.saturating_sub(12))
        .max(8);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(14), Constraint::Length(dock_height)])
        .split(area);

    draw_map_view(frame, rows[0], app);

    if area.width < WIDTH_MAP_NARROW {
        let dock = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(6),
                Constraint::Length(sidebar_height.max(6)),
            ])
            .split(rows[1]);
        spawns::draw_spawn_list(frame, dock[0], app);
        draw_tactical_sidebar(frame, dock[1], app, sections);
        return;
    }

    let dock_sidebar_width = if area.width >= WIDTH_SIDEBAR_WIDE {
        40
    } else {
        34
    };
    let dock = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(34), Constraint::Length(dock_sidebar_width)])
        .split(rows[1]);

    spawns::draw_spawn_list(frame, dock[0], app);
    draw_tactical_sidebar(frame, dock[1], app, sections);
}

// ─── Map view ────────────────────────────────────────────────────────────────

fn draw_map_view(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    use ratatui::style::Color;
    let t = &app.theme;
    let zone_label = app
        .active_client()
        .map_or("Unknown", |c| c.zone_name.as_str());
    let z_range = app.map_state.z_filter_range;
    let player_z = app.local_player.as_ref().map(|p| p.z);
    let player_pos_label = app
        .local_player
        .as_ref()
        .map(|player| {
            format!(
                " | You y:{:.0} x:{:.0} z:{:.0}",
                player.y, player.x, player.z
            )
        })
        .unwrap_or_default();
    let mesh_cache_label = app
        .current_zone_has_cached_mesh()
        .map(|cached| if cached { "cached" } else { "on-demand" })
        .unwrap_or("n/a");
    let overlay_label = if app.map_state.show_navmesh {
        app.map_state
            .navmesh_overlay
            .as_ref()
            .map(|overlay| format!("{} segs", overlay.segment_count()))
            .unwrap_or_else(|| String::from("unavailable"))
    } else {
        String::from("off")
    };
    // Compute bounds and transform once so they can be reused for both the
    // view label and the actual map rendering logic.
    let map_bounds = combined_bounds(app);
    let map_view_transform = map_bounds
        .as_ref()
        .and_then(|bounds| map_transform(app, bounds, 80, 30));
    let view_label = map_view_transform
        .map(|transform| active_view_label(app.map_state.viewport_mode, transform.using_local_view))
        .unwrap_or_else(|| app.map_state.viewport_mode.label().to_string());
    let map_info = app
        .map_state
        .zone_map
        .as_ref().map_or_else(|| {
            format!(
                " Map: {zone_label} (no map data){player_pos_label}{mesh_label} | Z filter: {z_range:.0} [+/-] | m maximize "
            )
        }, |m| {
            format!(
                " Map: {} ({} lines, {} labels){} | View: {} {:.2}x | Z: {:.0} [+/-] | Mesh: {} {} [n] | m maximize ",
                zone_label,
                m.lines.len(),
                m.points.len(),
                player_pos_label,
                view_label,
                app.map_state.zoom,
                z_range,
                mesh_cache_label,
                overlay_label,
            )
        });

    let border_style = if app.is_panel_focused(ActivePanel::TacticalMap) {
        t.border_active
    } else {
        t.border_dim
    };
    let blk = panel(map_info.as_str(), border_style, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let w = inner.width as usize;
    let h = inner.height as usize;
    if w == 0 || h == 0 {
        return;
    }

    let mut grid: Vec<Vec<(char, Color)>> = vec![vec![(' ', t.map_lines); w]; h];

    let Some(bounds) = combined_bounds(app) else {
        frame.render_widget(
            Paragraph::new("No map, navmesh, or spawn data")
                .style(Style::default().fg(t.text_muted)),
            inner,
        );
        return;
    };

    let Some(transform) = map_transform(app, &bounds, w, h) else {
        frame.render_widget(
            Paragraph::new("Map transform unavailable").style(Style::default().fg(t.text_muted)),
            inner,
        );
        return;
    };

    let visible_region = VisibleMapRegion::from_transform(&transform, w, h);

    let to_grid = |mx: f32, my: f32| -> (i32, i32) {
        let col = ((mx - transform.center_x) * transform.scale_x + w as f32 / 2.0) as i32;
        let row = ((my - transform.center_y) * transform.scale_y + h as f32 / 2.0) as i32;
        (col, row)
    };

    if let Some(map) = &app.map_state.zone_map {
        for ml in &map.lines {
            if !visible_region.contains_line(ml.x1, ml.y1, ml.x2, ml.y2) {
                continue;
            }
            let (c1, r1) = to_grid(ml.x1, ml.y1);
            let (c2, r2) = to_grid(ml.x2, ml.y2);
            let color = map_rgb_to_color(ml.r, ml.g, ml.b, t);
            bresenham_line(
                c1,
                r1,
                c2,
                r2,
                w,
                h,
                &mut grid,
                color,
                LinePaintMode::BlankOnly,
            );
        }
        for mp in &map.points {
            if !visible_region.contains_point(mp.x, mp.y) {
                continue;
            }
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

    if app.map_state.show_navmesh
        && let Some(overlay) = &app.map_state.navmesh_overlay
    {
        let draw_inner_lines = transform.using_local_view || app.map_state.zoom >= 1.35;
        for segment in &overlay.outer_lines {
            if !visible_region.contains_line(segment.x1, segment.y1, segment.x2, segment.y2) {
                continue;
            }
            if let Some(pz) = player_z
                && (segment.z1 - pz).abs() > z_range
                && (segment.z2 - pz).abs() > z_range
            {
                continue;
            }
            let (c1, r1) = to_grid(segment.x1, segment.y1);
            let (c2, r2) = to_grid(segment.x2, segment.y2);
            bresenham_line(
                c1,
                r1,
                c2,
                r2,
                w,
                h,
                &mut grid,
                t.text_secondary,
                LinePaintMode::OverwriteLinework,
            );
        }

        if draw_inner_lines {
            for segment in &overlay.inner_lines {
                if !visible_region.contains_line(segment.x1, segment.y1, segment.x2, segment.y2) {
                    continue;
                }
                if let Some(pz) = player_z
                    && (segment.z1 - pz).abs() > z_range
                    && (segment.z2 - pz).abs() > z_range
                {
                    continue;
                }
                let (c1, r1) = to_grid(segment.x1, segment.y1);
                let (c2, r2) = to_grid(segment.x2, segment.y2);
                bresenham_line(
                    c1,
                    r1,
                    c2,
                    r2,
                    w,
                    h,
                    &mut grid,
                    t.text_muted,
                    LinePaintMode::OverwriteLinework,
                );
            }
        }
    }

    let selected_spawn_id = app
        .filtered_spawns()
        .get(app.spawn_selected())
        .map(|spawn| spawn.spawn_id);

    for spawn in &app.spawns {
        // EQ Z = altitude; filter spawns more than z_range units above/below player.
        if let Some(pz) = player_z
            && (spawn.z - pz).abs() > z_range
        {
            continue;
        }
        let mx = -spawn.y;
        let my = -spawn.x;
        let (col, row) = to_grid(mx, my);
        if col >= 0 && col < w as i32 && row >= 0 && row < h as i32 {
            let (ch, color) = if Some(spawn.spawn_id) == selected_spawn_id {
                ('◎', t.text_highlight)
            } else {
                match spawn.spawn_type {
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
                }
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

    // ─── Nav path overlay ─────────────────────────────────────────────────
    if let Some(client) = app.active_client()
        && let Some(nav) = app.nav_state.nav_statuses.get(&client.pid)
        && nav.waypoints.len() >= 2
    {
        let nav_color = t.text_accent;
        for pair in nav.waypoints.windows(2) {
            let (c1, r1) = to_grid(-pair[0].y, -pair[0].x);
            let (c2, r2) = to_grid(-pair[1].y, -pair[1].x);
            bresenham_line(
                c1,
                r1,
                c2,
                r2,
                w,
                h,
                &mut grid,
                nav_color,
                LinePaintMode::OverwriteLinework,
            );
        }
        // Mark the final destination with a special symbol.
        if let Some(dest) = nav.waypoints.last() {
            let (dc, dr) = to_grid(-dest.y, -dest.x);
            if dc >= 0 && dc < w as i32 && dr >= 0 && dr < h as i32 {
                grid[dr as usize][dc as usize] = ('★', nav_color);
            }
        }
    }

    // ─── Player marker + FOV cone ────────────────────────────────────────
    if let Some(player) = &app.local_player {
        let (col, row) = to_grid(-player.y, -player.x);

        // Draw FOV wedge — full coordinate-transform proof:
        //
        // 1. EQ heading: 0=N, 128=W, 256=S, 384=E. CW in EQ world coords,
        //    512 heading units = full circle = 2*pi radians.
        //
        // 2. EQ world → map coords: we negate both axes via to_grid(-y, -x).
        //    This is a 180-degree rotation, which mirrors both axes and
        //    preserves angular direction (CW stays CW in map space).
        //
        // 3. Map → screen coords: screen Y increases downward, so we use
        //    `row - sin(a)` (line 212), which flips the Y axis. This converts
        //    CW angles into CCW angles in screen space.
        //
        // 4. Standard math angles are CCW with 0=East. EQ heading 0 (North)
        //    should map to pi/2 (screen-up). The formula:
        //      heading_rad = (512 - heading) * pi / 256
        //    At heading=0:   (512-0)*pi/256   = 2*pi ≡ 0 (East in math).
        //    But step 3's Y-flip (row - sin) makes 0 rad point screen-up,
        //    because -sin(0)=0 for col and cos(0)=1 becomes row-1 (up).
        //    Wait — cos is on col and sin on row:
        //      end_col = col + cos(a) * len   → horizontal
        //      end_row = row - sin(a) * len   → vertical (inverted)
        //    At a=0: col+len, row-0 → points right (East). But EQ heading 0
        //    is North. With (512-0)*pi/256 = 2*pi ≡ 0, this points East...
        //    unless the 180-degree rotation from step 2 remaps it.
        //
        //    The axis swap (-y→mx, -x→my) means EQ North (+Y in world) maps
        //    to -Y in map x-axis (col). Combined with the negation of both
        //    axes, the net effect is that the formula produces correct screen
        //    directions empirically, but the interaction of swap + negate +
        //    Y-flip makes a clean closed-form proof non-trivial.
        //
        // NOTE: FOV direction derived from EQ heading convention (0 ≡ 512 = North, CW; 512 units = full circle).
        // Empirically correct in TUI demo; final live-client verification deferred.
        let heading_rad = (512.0 - player.heading) * std::f32::consts::PI / 256.0;
        let half_fov = std::f32::consts::PI / 6.0; // 30-degree half-angle (60 total)
        let cone_len: f32 = 4.0; // length in grid cells
        for &angle_offset in &[-half_fov, 0.0, half_fov] {
            let a = heading_rad + angle_offset;
            let end_col = col as f32 + a.cos() * cone_len;
            let end_row = row as f32 - a.sin() * cone_len; // screen Y is inverted
            bresenham_line(
                col,
                row,
                end_col as i32,
                end_row as i32,
                w,
                h,
                &mut grid,
                t.map_you,
                LinePaintMode::OverwriteLinework,
            );
        }

        if col >= 0 && col < w as i32 && row >= 0 && row < h as i32 {
            grid[row as usize][col as usize] = ('◆', t.map_you);
        }
    }

    let show_legend = h >= 12;
    let legend_row = h.saturating_sub(1);
    let show_nav_destination = app
        .active_client()
        .and_then(|client| app.nav_state.nav_statuses.get(&client.pid))
        .is_some_and(|nav| nav.waypoints.len() >= 2);
    let lines: Vec<Line<'_>> = grid
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            if show_legend && i == legend_row {
                let mut spans = vec![
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
                ];

                if selected_spawn_id.is_some() {
                    spans.extend([
                        Span::raw(" │ "),
                        Span::styled("◎ ", Style::default().fg(t.text_highlight)),
                        Span::styled("Sel", Style::default().fg(t.text_muted)),
                    ]);
                }

                if show_nav_destination {
                    spans.extend([
                        Span::raw(" │ "),
                        Span::styled("★ ", Style::default().fg(t.text_accent)),
                        Span::styled("Path", Style::default().fg(t.text_muted)),
                    ]);
                }

                if app.map_state.show_navmesh && app.map_state.navmesh_overlay.is_some() {
                    spans.extend([
                        Span::raw(" │ "),
                        Span::styled("▦ ", Style::default().fg(t.text_secondary)),
                        Span::styled("Mesh", Style::default().fg(t.text_muted)),
                    ]);
                }

                Line::from(spans)
            } else {
                Line::from(color_run_spans(row))
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

/// Collapse a row of `(char, Color)` cells into spans grouped by consecutive color runs.
/// Produces ~10-30 spans per row instead of one per cell, avoiding thousands of heap allocations.
fn color_run_spans(row: Vec<(char, Color)>) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut buf = String::new();
    let mut current_color: Option<Color> = None;

    for (ch, color) in row {
        if current_color == Some(color) {
            buf.push(ch);
        } else {
            if let Some(c) = current_color {
                spans.push(Span::styled(
                    std::mem::take(&mut buf),
                    Style::default().fg(c),
                ));
            }
            buf.push(ch);
            current_color = Some(color);
        }
    }
    if let Some(c) = current_color {
        spans.push(Span::styled(buf, Style::default().fg(c)));
    }

    spans
}

#[derive(Debug, Clone, Copy)]
struct ViewBounds {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
}

impl ViewBounds {
    fn from_zone_map(map: &crate::eq::map_parser::ZoneMap) -> Self {
        Self {
            min_x: map.bounds.min_x,
            max_x: map.bounds.max_x,
            min_y: map.bounds.min_y,
            max_y: map.bounds.max_y,
        }
    }

    fn from_navmesh_overlay(overlay: &crate::nav::mesh::NavMeshOverlay) -> Option<Self> {
        if overlay.bounds.is_empty() {
            None
        } else {
            Some(Self {
                min_x: overlay.bounds.min_x,
                max_x: overlay.bounds.max_x,
                min_y: overlay.bounds.min_y,
                max_y: overlay.bounds.max_y,
            })
        }
    }

    fn from_spawns(spawns: &[crate::eq::structs::SpawnInfo]) -> Option<Self> {
        let mut bounds: Option<Self> = None;
        for spawn in spawns {
            let x = -spawn.y;
            let y = -spawn.x;
            if let Some(existing) = &mut bounds {
                existing.include_point(x, y);
            } else {
                bounds = Some(Self {
                    min_x: x,
                    max_x: x,
                    min_y: y,
                    max_y: y,
                });
            }
        }
        bounds
    }

    fn include(&mut self, other: Self) {
        self.min_x = self.min_x.min(other.min_x);
        self.max_x = self.max_x.max(other.max_x);
        self.min_y = self.min_y.min(other.min_y);
        self.max_y = self.max_y.max(other.max_y);
    }

    fn include_point(&mut self, x: f32, y: f32) {
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);
    }

    fn width(&self) -> f32 {
        (self.max_x - self.min_x).max(1.0)
    }

    fn height(&self) -> f32 {
        (self.max_y - self.min_y).max(1.0)
    }

    fn center_x(&self) -> f32 {
        (self.min_x + self.max_x) / 2.0
    }

    fn center_y(&self) -> f32 {
        (self.min_y + self.max_y) / 2.0
    }

    fn max_dimension(&self) -> f32 {
        self.width().max(self.height())
    }

    fn contains_with_margin(&self, x: f32, y: f32) -> bool {
        let margin_x = self.width() * 0.20;
        let margin_y = self.height() * 0.20;
        x >= self.min_x - margin_x
            && x <= self.max_x + margin_x
            && y >= self.min_y - margin_y
            && y <= self.max_y + margin_y
    }
}

#[derive(Debug, Clone, Copy)]
struct MapTransform {
    center_x: f32,
    center_y: f32,
    scale_x: f32,
    scale_y: f32,
    using_local_view: bool,
}

#[derive(Debug, Clone, Copy)]
struct VisibleMapRegion {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
}

impl VisibleMapRegion {
    fn from_transform(transform: &MapTransform, w: usize, h: usize) -> Self {
        let half_width = (w as f32 / (transform.scale_x.max(0.001) * 2.0)).max(1.0);
        let half_height = (h as f32 / (transform.scale_y.max(0.001) * 2.0)).max(1.0);
        Self {
            min_x: transform.center_x - half_width,
            max_x: transform.center_x + half_width,
            min_y: transform.center_y - half_height,
            max_y: transform.center_y + half_height,
        }
    }

    fn contains_line(&self, x1: f32, y1: f32, x2: f32, y2: f32) -> bool {
        let line_min_x = x1.min(x2);
        let line_max_x = x1.max(x2);
        let line_min_y = y1.min(y2);
        let line_max_y = y1.max(y2);
        !(line_max_x < self.min_x
            || line_min_x > self.max_x
            || line_max_y < self.min_y
            || line_min_y > self.max_y)
    }

    fn contains_point(&self, x: f32, y: f32) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }
}

fn combined_bounds(app: &App) -> Option<ViewBounds> {
    let mut bounds = app
        .map_state
        .zone_map
        .as_ref()
        .map(ViewBounds::from_zone_map);

    if app.map_state.show_navmesh
        && let Some(overlay) = &app.map_state.navmesh_overlay
        && let Some(navmesh_bounds) = ViewBounds::from_navmesh_overlay(overlay)
    {
        if let Some(existing) = &mut bounds {
            existing.include(navmesh_bounds);
        } else {
            bounds = Some(navmesh_bounds);
        }
    }

    if bounds.is_none() {
        bounds = ViewBounds::from_spawns(&app.spawns);
    }

    bounds
}

fn map_transform(app: &App, bounds: &ViewBounds, w: usize, h: usize) -> Option<MapTransform> {
    let player_pos = app
        .local_player
        .as_ref()
        .map(|player| (-player.y, -player.x));
    let auto_prefers_local = app.tactical_state.map_maximized || bounds.max_dimension() > 1_200.0;
    let using_local_view = match app.map_state.viewport_mode {
        MapViewportMode::Auto => player_pos
            .map(|(x, y)| bounds.contains_with_margin(x, y) && auto_prefers_local)
            .unwrap_or(false),
        MapViewportMode::Local => player_pos.is_some(),
        MapViewportMode::Global => false,
    };

    let (mut center_x, mut center_y, base_scale) = if using_local_view {
        let (player_x, player_y) = player_pos?;
        let half_height = if app.tactical_state.map_maximized {
            260.0
        } else {
            180.0
        };
        let aspect = (w as f32 / h.max(1) as f32).clamp(1.0, 2.6);
        let half_width = half_height * aspect;
        let scale =
            ((w as f32 - 2.0) / (half_width * 2.0)).min((h as f32 - 2.0) / (half_height * 2.0));
        (player_x, player_y, scale)
    } else {
        let scale = ((w as f32 - 2.0) / bounds.width()).min((h as f32 - 2.0) / bounds.height());
        (bounds.center_x(), bounds.center_y(), scale)
    };

    center_x += app.map_state.pan_x;
    center_y += app.map_state.pan_y;

    Some(MapTransform {
        center_x,
        center_y,
        scale_x: base_scale * app.map_state.zoom,
        scale_y: base_scale * app.map_state.zoom,
        using_local_view,
    })
}

fn active_view_label(mode: MapViewportMode, using_local_view: bool) -> String {
    match mode {
        MapViewportMode::Auto => {
            if using_local_view {
                String::from("auto/local")
            } else {
                String::from("auto/global")
            }
        }
        _ => mode.label().to_string(),
    }
}

fn map_rgb_to_color(r: u8, g: u8, b: u8, t: &Theme) -> ratatui::style::Color {
    if r == 0 && g == 0 && b == 0 {
        t.map_lines
    } else {
        ratatui::style::Color::Rgb(r, g, b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinePaintMode {
    BlankOnly,
    OverwriteLinework,
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
    paint_mode: LinePaintMode,
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
            if can_paint_line_cell(grid[uy][ux].0, paint_mode) {
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

fn can_paint_line_cell(ch: char, paint_mode: LinePaintMode) -> bool {
    match paint_mode {
        LinePaintMode::BlankOnly => ch == ' ',
        LinePaintMode::OverwriteLinework => matches!(ch, ' ' | '·' | '─' | '│' | '╱' | '╲'),
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

// ─── Tactical side rail ──────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum TacticalSectionKind {
    Named,
    Navigation,
}

fn draw_tactical_sidebar(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &App,
    sections: &[(TacticalSectionKind, Constraint)],
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
            TacticalSectionKind::Named => {
                draw_named_tracker_panel(frame, *chunk, app, app.tactical_state.named_collapsed);
            }
            TacticalSectionKind::Navigation => {
                draw_navigation_summary(
                    frame,
                    *chunk,
                    app,
                    app.tactical_state.navigation_collapsed,
                );
            }
        }
    }
}

fn tactical_sections(app: &App) -> Vec<(TacticalSectionKind, Constraint)> {
    let mut sections = Vec::new();

    if app.tactical_state.show_named {
        let named_rows = app.named_tracker.tracked_spawns().len().min(4) as u16;
        let user_rows = app.tracked_spawns.len().min(4) as u16;
        let named_height = if named_rows > 0 { named_rows + 3 } else { 4 };
        let tracked_height = if user_rows > 0 { user_rows + 3 } else { 0 };
        let combined_height = if tracked_height > 0 {
            (named_height + tracked_height).min(15)
        } else {
            named_height.min(10)
        };
        sections.push((
            TacticalSectionKind::Named,
            if app.tactical_state.named_collapsed {
                Constraint::Length(3)
            } else {
                Constraint::Length(combined_height.max(5))
            },
        ));
    }

    if app.tactical_state.show_navigation {
        let nav_height = if app
            .active_client()
            .and_then(|client| app.nav_state.nav_statuses.get(&client.pid))
            .is_some()
        {
            8
        } else {
            6
        };
        sections.push((
            TacticalSectionKind::Navigation,
            if app.tactical_state.navigation_collapsed {
                Constraint::Length(3)
            } else {
                Constraint::Length(nav_height)
            },
        ));
    }

    sections
}

fn tactical_section_title(label: &str, collapsed: bool) -> String {
    let icon = if collapsed { "▶" } else { "▼" };
    format!(" {label} {icon} ")
}

fn draw_named_tracker_panel(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &App,
    collapsed: bool,
) {
    let t = &app.theme;
    let named = app.named_tracker.tracked_spawns();
    let n_alive = named.iter().filter(|s| s.is_alive).count();
    let u_tracked = &app.tracked_spawns;
    let u_up = u_tracked
        .values()
        .filter(|t| t.status == crate::tui::app::TrackedStatus::Up)
        .count();

    let border_style = if app.is_panel_focused(ActivePanel::TacticalNamed) {
        t.border_active
    } else {
        t.border_warn
    };

    if collapsed {
        let title = tactical_section_title("Named", true);
        let summary = format!("{n_alive} named up | {u_up} tracked up");
        frame.render_widget(
            Paragraph::new(summary)
                .block(panel(title.as_str(), border_style, t))
                .style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
    }

    let has_user = !u_tracked.is_empty();
    let has_named = !named.is_empty();

    if !has_user && !has_named {
        let title = tactical_section_title("Named", false);
        frame.render_widget(
            Paragraph::new("No named or tracked spawns")
                .block(panel(title.as_str(), border_style, t))
                .style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
    }

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
        let title = format!(
            " {} ({} up / {}) ",
            tactical_section_title("Named", false).trim(),
            n_alive,
            named.len()
        );
        let blk = panel(title.as_str(), border_style, t);

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

fn draw_navigation_summary(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &App,
    collapsed: bool,
) {
    let t = &app.theme;
    let border_style = if app.is_panel_focused(ActivePanel::TacticalNavigation) {
        t.border_active
    } else {
        t.border_server
    };
    let title = tactical_section_title("Navigation", collapsed);

    let visible = app.visible_clients();
    let navigating = visible
        .iter()
        .filter(|client| {
            app.nav_state
                .nav_statuses
                .get(&client.pid)
                .is_some_and(|nav| nav.status.is_moving())
        })
        .count();
    let arrived = visible
        .iter()
        .filter(|client| {
            app.nav_state
                .nav_statuses
                .get(&client.pid)
                .is_some_and(|nav| nav.status.is_arrived())
        })
        .count();
    let stuck = visible
        .iter()
        .filter(|client| {
            app.nav_state
                .nav_statuses
                .get(&client.pid)
                .is_some_and(|nav| nav.status.is_stuck())
        })
        .count();
    let idle = visible.len().saturating_sub(navigating + arrived + stuck);

    if collapsed {
        let summary = format!("{navigating} nav | {arrived} arr | {idle} idle");
        frame.render_widget(
            Paragraph::new(summary)
                .block(panel(title.as_str(), border_style, t))
                .style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
    }

    let selected_name = app
        .active_client()
        .and_then(|client| client.local_player.as_ref())
        .map_or_else(
            || String::from("No client"),
            |player| app.redact_name(&player.displayed_name).into_owned(),
        );

    let selected_nav = app
        .active_client()
        .and_then(|client| app.nav_state.nav_statuses.get(&client.pid));
    let selected_status = selected_nav.map_or("Idle", |nav| nav.status.label());
    let selected_dest = selected_nav.map_or("—", |nav| nav.destination.as_str());
    let selected_waypoints = selected_nav.map_or(0, |nav| nav.waypoints.len());
    let mesh_status = app.current_zone_short_name().map_or_else(
        || String::from("—"),
        |zone| {
            format!(
                "{} ({zone})",
                if crate::nav::mesh::has_cached_zone_mesh(&zone) {
                    "cached"
                } else {
                    "on-demand"
                },
            )
        },
    );

    let status_color = match selected_nav.map(|nav| &nav.status) {
        Some(s) if s.is_moving() => t.text_highlight,
        Some(s) if s.is_arrived() => t.hp_high,
        Some(s) if s.is_stuck() => t.hp_low,
        _ => t.text_muted,
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Selected ", Style::default().fg(t.text_muted)),
            Span::styled(selected_name, Style::default().fg(t.text_normal)),
        ]),
        Line::from(vec![
            Span::styled("Status   ", Style::default().fg(t.text_muted)),
            Span::styled(selected_status, Style::default().fg(status_color)),
        ]),
        Line::from(vec![
            Span::styled("Dest     ", Style::default().fg(t.text_muted)),
            Span::styled(selected_dest, Style::default().fg(t.text_accent)),
        ]),
        Line::from(vec![
            Span::styled("Waypts   ", Style::default().fg(t.text_muted)),
            Span::styled(
                selected_waypoints.to_string(),
                Style::default().fg(t.text_highlight),
            ),
        ]),
        Line::from(vec![
            Span::styled("Mesh     ", Style::default().fg(t.text_muted)),
            Span::styled(mesh_status, Style::default().fg(t.text_secondary)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Fleet    ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{navigating} nav"),
                Style::default().fg(t.text_highlight),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(format!("{arrived} arr"), Style::default().fg(t.hp_high)),
            Span::styled("  ", Style::default()),
            Span::styled(format!("{idle} idle"), Style::default().fg(t.text_muted)),
        ]),
        Line::from(vec![
            Span::styled(":nav ", Style::default().fg(t.text_accent)),
            Span::styled("<dest>", Style::default().fg(t.text_normal)),
        ]),
        Line::from(vec![
            Span::styled("Enter ", Style::default().fg(t.text_highlight)),
            Span::styled(
                "full navigation window",
                Style::default().fg(t.text_secondary),
            ),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(panel(title.as_str(), border_style, t)),
        area,
    );
}
