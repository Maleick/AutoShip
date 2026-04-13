//! Map screen — zone map renderer, spawn position list, named tracker panel.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::time::Instant;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Row, Table},
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
use crate::tui::state::{MapSpawnPresentationCell, MapSpawnPresentationKey};
use crate::tui::theme::Theme;

static PERF_TRACE_ENABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var(textquest_common::ipc::PERF_TRACE_ENV)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
});

/// Draw the zone map screen with spawn positions and navigation overlay.
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
        let rail_max_width = area.width.saturating_sub(26);
        let rail_width = if rail_max_width < 20 {
            rail_max_width
        } else {
            rail_width.clamp(20, rail_max_width)
        };
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

#[derive(Default)]
struct PendingSpawnCell {
    count: u16,
    last_glyph: Option<(char, Color)>,
    selected_glyph: Option<(char, Color)>,
}

fn map_spawn_cache_key(
    app: &App,
    transform: &MapTransform,
    w: usize,
    h: usize,
    player_z: Option<f32>,
    selected_spawn_id: Option<u32>,
) -> MapSpawnPresentationKey {
    MapSpawnPresentationKey {
        client_pid: app.active_client().map(|client| client.pid),
        spawn_revision: app.selected_client_spawn_revision(),
        selected_spawn_id,
        width: w as u16,
        height: h as u16,
        z_filter_bits: app.map_state.z_filter_range.to_bits(),
        player_z_bits: player_z.map(f32::to_bits),
        show_spawns: app.map_state.show_spawns,
        map_filter_bits: app.map_state.filters.cache_key_bits(),
        theme_kind: app.theme_kind,
        center_x_bits: transform.center_x.to_bits(),
        center_y_bits: transform.center_y.to_bits(),
        scale_bits: transform.scale_x.to_bits(),
    }
}

fn rebuild_map_spawn_cache<F>(
    app: &mut App,
    key: MapSpawnPresentationKey,
    player_z: Option<f32>,
    selected_spawn: Option<&crate::eq::structs::SpawnInfo>,
    to_grid: F,
) -> bool
where
    F: Fn(f32, f32) -> (i32, i32),
{
    if app.map_spawn_cache.key.as_ref() == Some(&key) {
        return false;
    }

    let perf_start = if *PERF_TRACE_ENABLED {
        Some(Instant::now())
    } else {
        None
    };
    let mut pending: HashMap<(usize, usize), PendingSpawnCell> = HashMap::new();
    let use_clustering = app.map_state.zoom <= 0.95;
    let z_range = app.map_state.z_filter_range;
    let selected_spawn_id = selected_spawn.map(|spawn| spawn.spawn_id);
    let filters = &app.map_state.filters;

    app.map_spawn_cache.cells.clear();
    app.map_spawn_cache.selected_spawn =
        selected_spawn.map(|spawn| (-spawn.y, -spawn.x, spawn.spawn_id));

    if key.show_spawns {
        // Build set of connected client character names for group member detection.
        let group_names: HashSet<&str> = app
            .clients
            .iter()
            .filter_map(|c| c.local_player.as_ref().map(|p| p.displayed_name.as_str()))
            .collect();

        for spawn in &app.spawns {
            if !filters.allows_spawn(spawn) {
                continue;
            }
            if let Some(pz) = player_z
                && (spawn.z - pz).abs() > z_range
            {
                continue;
            }

            let (col, row) = to_grid(-spawn.y, -spawn.x);
            if col < 0 || row < 0 || col >= i32::from(key.width) || row >= i32::from(key.height) {
                continue;
            }

            let entry = pending.entry((row as usize, col as usize)).or_default();
            entry.count = entry.count.saturating_add(1);

            // Determine spawn glyph with visual hierarchy:
            // selected (◍) > group (⊕) > named (!) > PC (@) > NPC (·) > corpse (.)
            let glyph = spawn_marker_glyph(app, spawn, &group_names, selected_spawn_id);

            if Some(spawn.spawn_id) == selected_spawn_id {
                entry.selected_glyph = Some(glyph);
            } else {
                entry.last_glyph = Some(glyph);
            }
        }

        app.map_spawn_cache.cells = pending
            .into_iter()
            .filter_map(|((row, col), entry)| {
                let (ch, color) = if use_clustering && entry.count > 2 {
                    let digit = if entry.count > 9 {
                        '+'
                    } else {
                        char::from_digit(entry.count as u32, 10).unwrap_or('+')
                    };
                    (digit, app.theme.text_highlight)
                } else if let Some(selected) = entry.selected_glyph {
                    selected
                } else {
                    entry.last_glyph?
                };

                Some(MapSpawnPresentationCell {
                    row: row as u16,
                    col: col as u16,
                    ch,
                    color,
                })
            })
            .collect();
    }

    app.map_spawn_cache.key = Some(key.clone());

    if let Some(start) = perf_start {
        tracing::info!(
            target: "textquest::perf",
            client_pid = key.client_pid,
            spawn_revision = key.spawn_revision,
            cell_count = app.map_spawn_cache.cells.len(),
            show_spawns = key.show_spawns,
            elapsed_ms = start.elapsed().as_secs_f64() * 1000.0,
            "Tactical map spawn cache rebuilt"
        );
    }

    true
}

fn spawn_marker_glyph(
    app: &App,
    spawn: &crate::eq::structs::SpawnInfo,
    group_names: &HashSet<&str>,
    selected_spawn_id: Option<u32>,
) -> (char, Color) {
    if Some(spawn.spawn_id) == selected_spawn_id {
        ('◍', app.theme.text_highlight)
    } else {
        match spawn.spawn_type {
            SpawnType::Player => {
                if group_names.contains(spawn.displayed_name.as_str()) {
                    ('⊕', app.theme.map_group)
                } else {
                    ('@', app.theme.map_pc)
                }
            }
            SpawnType::Npc => {
                let is_named = !spawn.displayed_name.starts_with("a ")
                    && !spawn.displayed_name.starts_with("an ");
                let base_color = if is_named {
                    app.theme.map_named
                } else {
                    app.theme.map_npc
                };
                let marker = if is_named { '!' } else { '·' };
                let color = if spawn.hp_max > 0 && spawn.hp_current < spawn.hp_max / 2 {
                    Color::DarkGray
                } else {
                    base_color
                };
                (marker, color)
            }
            SpawnType::Corpse => ('.', app.theme.map_corpse),
            SpawnType::Unknown(_) => ('?', app.theme.spawn_unknown),
        }
    }
}


/// Convert an EQ heading value (0–512, where 0=North, 128=West, 256=South, 384=East)
/// to an 8-direction Unicode arrow character indicating the player's facing direction.
///
/// The EQ heading range is 0–512 (full circle). We map it to 8 octants of 64 units each:
///   0/512=N(↑), 64=NW(↖), 128=W(←), 192=SW(↙), 256=S(↓), 320=SE(↘), 384=E(→), 448=NE(↗)
fn heading_arrow_char(heading: f32) -> char {
    // Normalise to [0, 512)
    let h = ((heading % 512.0) + 512.0) % 512.0;
    // Each octant spans 64 units; centre at multiples of 64, offset by 32 for rounding.
    let octant = ((h + 32.0) % 512.0) as u32 / 64;
    match octant {
        0 => '↑', // N
        1 => '↖', // NW
        2 => '←', // W
        3 => '↙', // SW
        4 => '↓', // S
        5 => '↘', // SE
        6 => '→', // E
        7 => '↗', // NE
        _ => '↑', // fallback
    }
}
/// Place a directional heading arrow in the grid cell adjacent to position (`col`, `row`)
/// in the direction the player is facing. No-op if the target cell is out of bounds.
fn place_heading_arrow(
    heading: f32,
    heading_rad: f32,
    col: i32,
    row: i32,
    width: i32,
    height: i32,
    grid: &mut Vec<Vec<(char, ratatui::style::Color)>>,
    color: ratatui::style::Color,
) {
    let arrow = heading_arrow_char(heading);
    let arrow_col = col + heading_rad.cos().round() as i32;
    let arrow_row = row - heading_rad.sin().round() as i32; // screen Y inverted
    if arrow_col >= 0
        && arrow_col < width
        && arrow_row >= 0
        && arrow_row < height
        && (arrow_col != col || arrow_row != row)
    {
        grid[arrow_row as usize][arrow_col as usize] = (arrow, color);
    }
}

fn draw_map_view(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    use ratatui::style::Color;
    let theme = app.theme.clone();
    let t = &theme;
    let zone_label = app
        .active_client()
        .map_or_else(|| String::from("Unknown"), |c| c.zone_name.clone());
    let z_range = app.map_state.z_filter_range;
    let player_z = app.local_player.as_ref().map(|p| p.z);
    let selected_spawn = app.selected_filtered_spawn().cloned();
    let selected_spawn_id = selected_spawn.as_ref().map(|spawn| spawn.spawn_id);
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
    let selected_spawn_label = selected_spawn
        .as_ref()
        .map(|spawn| {
            format!(
                " | Sel {} y:{:.0} x:{:.0} z:{:.0}",
                app.redact_name(&spawn.displayed_name),
                spawn.y,
                spawn.x,
                spawn.z
            )
        })
        .unwrap_or_else(|| String::from(" | Sel none"));
    let filter_label = app.map_state.filters.inline_flags();
    let layer_label = format!(
        " [{}{}{}{}{}{}]",
        if app.map_state.show_geometry {
            "G"
        } else {
            "-"
        },
        if app.map_state.show_spawns { "S" } else { "-" },
        if app.map_state.show_nav_paths {
            "P"
        } else {
            "-"
        },
        if app.map_state.show_navmesh { "M" } else { "-" },
        if app.map_state.show_labels { "L" } else { "-" },
        if app.map_state.show_annotations {
            "A"
        } else {
            "-"
        },
    );

    let border_style = if app.is_panel_focused(ActivePanel::TacticalMap) {
        t.border_active
    } else {
        t.border_dim
    };
    let map_bounds = combined_bounds(app);
    let provisional_block = panel("", border_style, t);
    let provisional_inner = provisional_block.inner(area);
    let provisional_view_transform = map_bounds.as_ref().and_then(|bounds| {
        map_transform(
            app,
            bounds,
            provisional_inner.width.max(1).into(),
            provisional_inner.height.max(1).into(),
        )
    });
    let provisional_view_label = provisional_view_transform
        .map(|transform| active_view_label(app.map_state.viewport_mode, transform.using_local_view))
        .unwrap_or_else(|| app.map_state.viewport_mode.label().to_string());
    let provisional_view_center = provisional_view_transform
        .map(|transform| {
            format!(
                " center:{:.0},{:.0}",
                transform.center_x, transform.center_y
            )
        })
        .unwrap_or_default();
    let provisional_map_info = app
        .map_state
        .zone_map
        .as_ref()
        .map_or_else(
            || {
                format!(
                    " {zone_label} (no data){player_pos_label}{selected_spawn_label} |{layer_label} | {filter_label} | Z:{z_range:.0} "
                )
            },
            |_| {
                format!(
                    " {zone_label}{player_pos_label}{selected_spawn_label} | {provisional_view_label} {:.2}x{provisional_view_center} |{layer_label} | {filter_label} | Z:{z_range:.0} ",
                    app.map_state.zoom,
                )
            },
        );

    let blk_for_size = panel(provisional_map_info.as_str(), border_style, t);
    let inner = blk_for_size.inner(area);
    let w = inner.width as usize;
    let h = inner.height as usize;
    if w == 0 || h == 0 {
        return;
    }
    let map_view_transform = map_bounds
        .as_ref()
        .and_then(|bounds| map_transform(app, bounds, w.max(1), h.max(1)));
    let view_label = map_view_transform
        .map(|transform| active_view_label(app.map_state.viewport_mode, transform.using_local_view))
        .unwrap_or_else(|| app.map_state.viewport_mode.label().to_string());
    let view_center = map_view_transform
        .map(|transform| {
            format!(
                " center:{:.0},{:.0}",
                transform.center_x, transform.center_y
            )
        })
        .unwrap_or_default();
    let map_info = app
        .map_state
        .zone_map
        .as_ref()
        .map_or_else(
            || {
                format!(
                    " {zone_label} (no data){player_pos_label}{selected_spawn_label} |{layer_label} | {filter_label} | Z:{z_range:.0} "
                )
            },
            |_| {
                format!(
                    " {zone_label}{player_pos_label}{selected_spawn_label} | {view_label} {:.2}x{view_center} |{layer_label} | {filter_label} | Z:{z_range:.0} ",
                    app.map_state.zoom,
                )
            },
        );
    let blk = panel(map_info.as_str(), border_style, t);
    frame.render_widget(blk, area);

    let mut grid: Vec<Vec<(char, Color)>> = vec![vec![(' ', t.map_lines); w]; h];

    let Some(transform) = map_view_transform else {
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

    if app.map_state.show_geometry
        && let Some(map) = &app.map_state.zone_map
    {
        let hide_annotations = !app.map_state.show_annotations;
        for ml in &map.lines {
            if hide_annotations && ml.layer == 2 {
                continue;
            }
            if !visible_region.contains_line(ml.x1, ml.y1, ml.x2, ml.y2) {
                continue;
            }
            let color = map_rgb_to_color(ml.r, ml.g, ml.b, t);
            clip_project_draw_line(
                ml.x1, ml.y1, ml.z1, ml.x2, ml.y2, ml.z2,
                player_z, z_range, &to_grid, w, h, &mut grid,
                color, LinePaintMode::BlankOnly,
            );
        }
    }

    if app.map_state.show_labels
        && let Some(map) = &app.map_state.zone_map
    {
        let hide_annotations = !app.map_state.show_annotations;
        let show_labels = app.map_state.zoom >= 0.8;
        for mp in &map.points {
            if hide_annotations && mp.layer == 2 {
                continue;
            }
            if !visible_region.contains_point(mp.x, mp.y) {
                continue;
            }
            let (col, row) = to_grid(mp.x, mp.y);
            if grid_in_bounds(col, row, w, h) {
                let marker = if mp.label.is_empty() {
                    '*'
                } else {
                    mp.label.chars().next().unwrap_or('*')
                };
                let point_color = map_rgb_to_color(mp.r, mp.g, mp.b, t);
                grid[row as usize][col as usize] = (marker, point_color);

                if show_labels {
                    let label_budget = if app.map_state.zoom > 1.8 {
                        20
                    } else if app.map_state.zoom > 1.1 {
                        16
                    } else if w > 120 {
                        12
                    } else {
                        8
                    };
                    let max_label_len = w.saturating_sub(col as usize + 1);
                    for (i, c) in mp
                        .label
                        .chars()
                        .take(max_label_len.min(label_budget))
                        .enumerate()
                    {
                        let lc = col as usize + 1 + i;
                        if lc < w && grid[row as usize][lc].0 == ' ' {
                            grid[row as usize][lc] = (c, point_color);
                        }
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
            clip_project_draw_line(
                segment.x1, segment.y1, segment.z1, segment.x2, segment.y2, segment.z2,
                player_z, z_range, &to_grid, w, h, &mut grid,
                t.text_secondary, LinePaintMode::OverwriteLinework,
            );
        }

        if draw_inner_lines {
            for segment in &overlay.inner_lines {
                if !visible_region.contains_line(segment.x1, segment.y1, segment.x2, segment.y2) {
                    continue;
                }
                clip_project_draw_line(
                    segment.x1, segment.y1, segment.z1, segment.x2, segment.y2, segment.z2,
                    player_z, z_range, &to_grid, w, h, &mut grid,
                    t.text_muted, LinePaintMode::OverwriteLinework,
                );
            }
        }
    }

    let spawn_cache_key = map_spawn_cache_key(app, &transform, w, h, player_z, selected_spawn_id);
    rebuild_map_spawn_cache(
        app,
        spawn_cache_key,
        player_z,
        selected_spawn.as_ref(),
        to_grid,
    );
    for cell in &app.map_spawn_cache.cells {
        let row = cell.row as usize;
        let col = cell.col as usize;
        if row < h && col < w {
            grid[row][col] = (cell.ch, cell.color);
        }
    }

    for status in app.named_tracker.tracked_spawns() {
        if !status.is_alive {
            let (col, row) = to_grid(-status.last_y, -status.last_x);
            if grid_in_bounds(col, row, w, h) {
                grid[row as usize][col as usize] = ('✕', t.map_dead_named);
            }
        }
    }

    // ─── Nav path overlay ─────────────────────────────────────────────────
    if app.map_state.show_nav_paths
        && app.map_state.show_target_path
        && let Some(client) = app.active_client()
        && let Some(nav) = app.nav_state.nav_statuses.get(&client.pid)
        && nav.waypoints.len() >= 2
    {
        let nav_color = t.text_accent;
        // Draw path lines with directional arrows at segment midpoints.
        for (seg_idx, pair) in nav.waypoints.windows(2).enumerate() {
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
            // Draw directional arrow at midpoint of each segment.
            let mid_c = (c1 + c2) / 2;
            let mid_r = (r1 + r2) / 2;
            if grid_in_bounds(mid_c, mid_r, w, h) {
                let arrow = direction_arrow(c2 - c1, r2 - r1);
                grid[mid_r as usize][mid_c as usize] = (arrow, nav_color);
            }
            // Draw numbered marker at the start of each segment (intermediate waypoints).
            // Skip marking the very first waypoint (it's the player's current location).
            if seg_idx > 0 && grid_in_bounds(c1, r1, w, h) {
                let label = if seg_idx <= 9 {
                    char::from_digit(seg_idx as u32, 10).unwrap_or('+')
                } else {
                    '+'
                };
                grid[r1 as usize][c1 as usize] = (label, nav_color);
            }
        }
        // Mark the final destination with a special symbol.
        if let Some(dest) = nav.waypoints.last() {
            let (dc, dr) = to_grid(-dest.y, -dest.x);
            if grid_in_bounds(dc, dr, w, h) {
                grid[dr as usize][dc as usize] = ('★', nav_color);
            }
        }
    }

    // ─── Target line overlay ─────────────────────────────────────────────────
    if app.map_state.show_target_line
        && let (Some(player), Some(target)) = (&app.local_player, &app.target)
    {
        let (pc, pr) = to_grid(-player.y, -player.x);
        let (tc, tr) = to_grid(-target.y, -target.x);
        bresenham_line(
            pc,
            pr,
            tc,
            tr,
            w,
            h,
            &mut grid,
            t.text_highlight,
            LinePaintMode::OverwriteLinework,
        );
        if grid_in_bounds(tc, tr, w, h) {
            grid[tr as usize][tc as usize] = ('✚', t.text_highlight);
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

        if grid_in_bounds(col, row, w, h) {
            grid[row as usize][col as usize] = ('◆', t.map_you);
        }

        // ─── Heading arrow adjacent to player marker ─────────────────────────
        place_heading_arrow(
            player.heading,
            heading_rad,
            col,
            row,
            w as i32,
            h as i32,
            &mut grid,
            t.map_you,
        );

        // ─── Radius circle overlays ─────────────────────────────────────────
        draw_radius_overlays(app, &to_grid, w as u16, h as u16, &mut grid);
    }

    // ─── Loc marker overlay ──────────────────────────────────────────────
    if let Some(loc) = &app.map_state.loc_marker {
        let (lc, lr) = to_grid(-loc.y, -loc.x);
        if grid_in_bounds(lc, lr, w, h) {
            grid[lr as usize][lc as usize] = ('⊗', Color::Yellow);
            for (i, ch) in loc.label.chars().take(12).enumerate() {
                let col = lc + 2 + i as i32;
                if col >= 0 && col < w as i32 {
                    grid[lr as usize][col as usize] = (ch, Color::Yellow);
                }
            }
        }
    }

    // ─── Named persistent markers ────────────────────────────────────────
    draw_named_markers(app, &to_grid, w as i32, h as i32, &mut grid);

    // ─── Camp location overlay ────────────────────────────────────────────
    draw_camp_overlays(app, &to_grid, w as u16, h as u16, &mut grid);

    // ─── Spawn highlights overlay ────────────────────────────────────────
    if !app.map_state.highlights.is_empty() {
        for spawn in &app.spawns {
            let lower_name = spawn.name.to_ascii_lowercase();
            for hl in &app.map_state.highlights {
                if lower_name.contains(&hl.pattern_lower) {
                    let (sc, sr) = to_grid(-spawn.y, -spawn.x);
                    if grid_in_bounds(sc, sr, w, h) {
                        let color = hl.color.unwrap_or(Color::Magenta);
                        if hl.pulse && (app.tick_count / 5).is_multiple_of(2) {
                            continue;
                        }
                        let ch = match hl.size {
                            1 => '●',
                            2 => '◉',
                            _ => '◈',
                        };
                        grid[sr as usize][sc as usize] = (ch, color);
                        if hl.size >= 2 {
                            for &(dx, dy) in &[(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                                let nc = sc + dx;
                                let nr = sr + dy;
                                if grid_in_bounds(nc, nr, w, h) {
                                    grid[nr as usize][nc as usize] = ('·', color);
                                }
                            }
                        }
                    }
                    break;
                }
            }
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
                    Span::styled("↑ ", Style::default().fg(t.map_you)),
                    Span::styled("Hdg", Style::default().fg(t.text_muted)),
                    Span::raw(" │ "),
                    Span::styled("⊕ ", Style::default().fg(t.map_group)),
                    Span::styled("Grp", Style::default().fg(t.text_muted)),
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
                        Span::styled("1→★ ", Style::default().fg(t.text_accent)),
                        Span::styled("Wpts", Style::default().fg(t.text_muted)),
                    ]);
                }

                if app.target.is_some() && app.map_state.show_target_line {
                    spans.extend([
                        Span::raw(" │ "),
                        Span::styled("✚ ", Style::default().fg(t.text_highlight)),
                        Span::styled("Target", Style::default().fg(t.text_muted)),
                    ]);
                }

                if app.map_state.show_navmesh && app.map_state.navmesh_overlay.is_some() {
                    spans.extend([
                        Span::raw(" │ "),
                        Span::styled("▦ ", Style::default().fg(t.text_secondary)),
                        Span::styled("Mesh", Style::default().fg(t.text_muted)),
                    ]);
                }

                if !app.map_state.named_markers.is_empty() {
                    spans.extend([
                        Span::raw(" │ "),
                        Span::styled("◆ ", Style::default().fg(Color::Cyan)),
                        Span::styled("Mkr", Style::default().fg(t.text_muted)),
                    ]);
                }

                if app.map_state.camp_overlay.is_some() {
                    spans.extend([
                        Span::raw(" │ "),
                        Span::styled("⊕ ", Style::default().fg(Color::Green)),
                        Span::styled("Camp", Style::default().fg(t.text_muted)),
                        Span::raw(" "),
                        Span::styled("⊗ ", Style::default().fg(Color::Red)),
                        Span::styled("Pull", Style::default().fg(t.text_muted)),
                    ]);
                }

                Line::from(spans)
            } else {
                Line::from(color_run_spans(row))
            }
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);

    if let Some(mini_bounds) = minimap_area(inner, w, h)
        && let Some(bounds) = map_bounds.as_ref()
    {
        let (mini_title, mini_lines) = draw_minimap_widget(
            bounds,
            mini_bounds,
            app,
            app.map_spawn_cache.selected_spawn,
            &transform,
        );
        frame.render_widget(Clear, mini_bounds);
        frame.render_widget(
            Paragraph::new(mini_lines).block(panel(mini_title.as_str(), t.border_dim, t)),
            mini_bounds,
        );
    }
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

    let scale = base_scale * app.map_state.zoom;
    center_x += app.map_state.pan_x;
    center_y += app.map_state.pan_y;
    center_x = clamp_view_center(center_x, bounds.min_x, bounds.max_x, scale, w);
    center_y = clamp_view_center(center_y, bounds.min_y, bounds.max_y, scale, h);

    Some(MapTransform {
        center_x,
        center_y,
        scale_x: scale,
        scale_y: scale,
        using_local_view,
    })
}

fn minimap_area(
    outer: ratatui::layout::Rect,
    map_w: usize,
    map_h: usize,
) -> Option<ratatui::layout::Rect> {
    if map_w < 32 || map_h < 16 {
        return None;
    }
    let width = (map_w / 4).clamp(16, 34) as u16;
    let height = ((width as f32 * 0.58).round() as u16).max(8);
    if outer.width < width.saturating_add(2) || outer.height < height.saturating_add(2) {
        return None;
    }
    Some(ratatui::layout::Rect {
        x: outer.x.saturating_add(outer.width - width - 1),
        y: outer.y.saturating_add(1),
        width,
        height,
    })
}

fn clamp_view_center(center: f32, min: f32, max: f32, scale: f32, view_size: usize) -> f32 {
    let half = if view_size == 0 {
        0.0
    } else {
        (view_size as f32) / (2.0 * scale.max(0.001))
    };
    let allowed_min = min + half;
    let allowed_max = max - half;
    if allowed_min <= allowed_max {
        center.clamp(allowed_min, allowed_max)
    } else {
        (min + max) / 2.0
    }
}

fn draw_minimap_widget(
    bounds: &ViewBounds,
    mini_area: ratatui::layout::Rect,
    app: &App,
    selected_spawn: Option<(f32, f32, u32)>,
    main_transform: &MapTransform,
) -> (String, Vec<Line<'static>>) {
    let t = &app.theme;
    let mini_width = mini_area.width.saturating_sub(2).max(1) as usize;
    let mini_height = mini_area.height.saturating_sub(2).max(1) as usize;
    let mini_width = mini_width.clamp(10, 28);
    let mini_height = mini_height.clamp(6, 16);
    let mut mini_grid: Vec<Vec<(char, Color)>> =
        vec![vec![(' ', t.map_lines); mini_width]; mini_height];

    let span_x = bounds.width().max(1.0);
    let span_y = bounds.height().max(1.0);
    let scale =
        ((mini_width as f32 - 1.0) / span_x).min((mini_height as f32 - 1.0) / span_y) * 0.95;
    let to_mini = |mx: f32, my: f32| -> Option<(usize, usize)> {
        if scale <= 0.0 {
            return None;
        }
        let col = ((mx - bounds.min_x) * scale) as i32;
        let row = ((my - bounds.min_y) * scale) as i32;
        if col < 0 || row < 0 {
            return None;
        }
        let (col, row) = (col as usize, row as usize);
        if col >= mini_width || row >= mini_height {
            None
        } else {
            Some((col, row))
        }
    };

    if app.map_state.show_geometry
        && let Some(map) = &app.map_state.zone_map
    {
        let hide_annotations = !app.map_state.show_annotations;
        for segment in &map.lines {
            if hide_annotations && segment.layer == 2 {
                continue;
            }
            if let Some((x1, y1)) = to_mini(segment.x1, segment.y1)
                && let Some((x2, y2)) = to_mini(segment.x2, segment.y2)
            {
                let line_color = map_rgb_to_color(segment.r, segment.g, segment.b, t);
                bresenham_line(
                    x1 as i32,
                    y1 as i32,
                    x2 as i32,
                    y2 as i32,
                    mini_width,
                    mini_height,
                    &mut mini_grid,
                    line_color,
                    LinePaintMode::OverwriteLinework,
                );
            }
        }
    }

    if app.map_state.show_labels
        && let Some(map) = &app.map_state.zone_map
    {
        let hide_annotations = !app.map_state.show_annotations;
        for point in &map.points {
            if hide_annotations && point.layer == 2 {
                continue;
            }
            if let Some((col, row)) = to_mini(point.x, point.y) {
                let marker = point.label.chars().next().unwrap_or('*');
                let point_color = map_rgb_to_color(point.r, point.g, point.b, t);
                mini_grid[row][col] = (marker, point_color);
            }
        }
    }

    if let Some(player) = &app.local_player
        && let Some((col, row)) = to_mini(-player.y, -player.x)
    {
        mini_grid[row][col] = ('◆', t.map_you);
        // Heading arrow in adjacent minimap cell
        let heading_rad = (512.0 - player.heading) * std::f32::consts::PI / 256.0;
        place_heading_arrow(
            player.heading,
            heading_rad,
            col as i32,
            row as i32,
            mini_width as i32,
            mini_height as i32,
            &mut mini_grid,
            t.map_you,
        );
    }

    if app.map_state.show_spawns {
        let group_names: HashSet<&str> = app
            .clients
            .iter()
            .filter_map(|c| c.local_player.as_ref().map(|p| p.displayed_name.as_str()))
            .collect();
        let player_z = app.local_player.as_ref().map(|player| player.z);
        let selected_spawn_id = selected_spawn.map(|(_, _, spawn_id)| spawn_id);
        let z_range = app.map_state.z_filter_range;

        for spawn in &app.spawns {
            if !app.map_state.filters.allows_spawn(spawn) {
                continue;
            }
            if let Some(pz) = player_z
                && (spawn.z - pz).abs() > z_range
            {
                continue;
            }
            if let Some((col, row)) = to_mini(-spawn.y, -spawn.x) {
                mini_grid[row][col] =
                    spawn_marker_glyph(app, spawn, &group_names, selected_spawn_id);
            }
        }
    }

    if let Some((spawn_x, spawn_y, _spawn_id)) = selected_spawn
        && let Some((col, row)) = to_mini(spawn_x, spawn_y)
    {
        mini_grid[row][col] = ('◎', t.text_highlight);
    }

    if app.map_state.show_target_line
        && let (Some(player), Some(target)) = (&app.local_player, &app.target)
        && let (Some((pc, pr)), Some((tc, tr))) =
            (to_mini(-player.y, -player.x), to_mini(-target.y, -target.x))
    {
        bresenham_line(
            pc as i32,
            pr as i32,
            tc as i32,
            tr as i32,
            mini_width,
            mini_height,
            &mut mini_grid,
            t.text_highlight,
            LinePaintMode::OverwriteLinework,
        );
        mini_grid[tr][tc] = ('✚', t.text_highlight);
    }

    if let Some(client) = app.active_client()
        && let Some(nav) = app.nav_state.nav_statuses.get(&client.pid)
    {
        if app.map_state.show_nav_paths && nav.waypoints.len() >= 2 {
            for pair in nav.waypoints.windows(2) {
                if let (Some((c1, r1)), Some((c2, r2))) = (
                    to_mini(-pair[0].y, -pair[0].x),
                    to_mini(-pair[1].y, -pair[1].x),
                ) {
                    bresenham_line(
                        c1 as i32,
                        r1 as i32,
                        c2 as i32,
                        r2 as i32,
                        mini_width,
                        mini_height,
                        &mut mini_grid,
                        t.text_accent,
                        LinePaintMode::OverwriteLinework,
                    );
                }
            }
        }

        if let Some(dest) = nav.waypoints.last()
            && let Some((col, row)) = to_mini(-dest.y, -dest.x)
        {
            mini_grid[row][col] = ('★', t.text_accent);
        }
    }

    let header = format!(
        "Mini ({:.0},{:.0} | z:{:.2}x)",
        main_transform.center_x, main_transform.center_y, app.map_state.zoom
    );
    let mini_lines: Vec<Line<'static>> = mini_grid
        .into_iter()
        .map(|row| Line::from(color_run_spans(row)))
        .collect();
    (header, mini_lines)
}

fn active_view_label(mode: MapViewportMode, using_local_view: bool) -> String {
    if mode == MapViewportMode::Auto {
        if using_local_view { "auto/local" } else { "auto/global" }.into()
    } else {
        mode.label().to_string()
    }
}

/// Clip a line segment against a Z-height range centered on `center_z`.
///
/// Returns `None` if both endpoints are outside the range (line fully culled).
/// Otherwise returns the (possibly clipped) `(x1, y1, x2, y2)` coordinates,
/// interpolating XY at the height boundary when one endpoint is outside.
#[allow(clippy::too_many_arguments)]
fn clip_line_z(
    x1: f32,
    y1: f32,
    z1: f32,
    x2: f32,
    y2: f32,
    z2: f32,
    center_z: f32,
    z_range: f32,
) -> Option<(f32, f32, f32, f32)> {
    let min_z = center_z - z_range;
    let max_z = center_z + z_range;

    let p1_inside = z1 >= min_z && z1 <= max_z;
    let p2_inside = z2 >= min_z && z2 <= max_z;

    if p1_inside && p2_inside {
        return Some((x1, y1, x2, y2));
    }
    if !p1_inside && !p2_inside {
        // Both outside — but check if the segment crosses through the range
        // (e.g., one below min and one above max). If both are on the same side, cull.
        if (z1 < min_z && z2 < min_z) || (z1 > max_z && z2 > max_z) {
            return None;
        }
    }

    let dz = z2 - z1;
    // Avoid division by zero (horizontal line in Z — already handled above)
    if dz.abs() < f32::EPSILON {
        return if p1_inside || p2_inside {
            Some((x1, y1, x2, y2))
        } else {
            None
        };
    }

    let mut cx1 = x1;
    let mut cy1 = y1;
    let mut cz1 = z1;
    let mut cx2 = x2;
    let mut cy2 = y2;
    let mut cz2 = z2;

    // Clip p1 against min
    if cz1 < min_z {
        let t = (min_z - cz1) / (cz2 - cz1);
        cx1 = cx1 + t * (cx2 - cx1);
        cy1 = cy1 + t * (cy2 - cy1);
        cz1 = min_z;
    }
    // Clip p2 against min
    if cz2 < min_z {
        let t = (min_z - cz2) / (cz1 - cz2);
        cx2 = cx2 + t * (cx1 - cx2);
        cy2 = cy2 + t * (cy1 - cy2);
        cz2 = min_z;
    }
    // Clip p1 against max
    if cz1 > max_z {
        let t = (max_z - cz1) / (cz2 - cz1);
        cx1 = cx1 + t * (cx2 - cx1);
        cy1 = cy1 + t * (cy2 - cy1);
        cz1 = max_z;
    }
    // Clip p2 against max
    if cz2 > max_z {
        let t = (max_z - cz2) / (cz1 - cz2);
        cx2 = cx2 + t * (cx1 - cx2);
        cy2 = cy2 + t * (cy1 - cy2);
        #[allow(unused_assignments)]
        {
            cz2 = max_z;
        }
    }

    // Final sanity: both clipped Z values should be in range
    if cz1 < min_z || cz1 > max_z {
        return None;
    }

    Some((cx1, cy1, cx2, cy2))
}

/// Returns true if `(col, row)` is within a `w × h` grid (both non-negative and in-bounds).
fn grid_in_bounds(col: i32, row: i32, w: usize, h: usize) -> bool {
    col >= 0 && (col as usize) < w && row >= 0 && (row as usize) < h
}

/// Z-clip a line segment, project both endpoints via `to_grid`, and draw with Bresenham.
#[allow(clippy::too_many_arguments)]
fn clip_project_draw_line(
    x1: f32, y1: f32, z1: f32,
    x2: f32, y2: f32, z2: f32,
    player_z: Option<f32>,
    z_range: f32,
    to_grid: &impl Fn(f32, f32) -> (i32, i32),
    w: usize, h: usize,
    grid: &mut [Vec<(char, Color)>],
    color: Color,
    paint_mode: LinePaintMode,
) {
    let (lx1, ly1, lx2, ly2) = if let Some(pz) = player_z {
        match clip_line_z(x1, y1, z1, x2, y2, z2, pz, z_range) {
            Some(coords) => coords,
            None => return,
        }
    } else {
        (x1, y1, x2, y2)
    };
    let (c1, r1) = to_grid(lx1, ly1);
    let (c2, r2) = to_grid(lx2, ly2);
    bresenham_line(c1, r1, c2, r2, w, h, grid, color, paint_mode);
}

fn map_rgb_to_color(r: u8, g: u8, b: u8, t: &Theme) -> ratatui::style::Color {
    if r == 0 && g == 0 && b == 0 {
        t.map_geometry
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
        if grid_in_bounds(cx, cy, w, h) {
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
        .constraints(sections.iter().map(|(_, constraint)| *constraint))
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
            let header = themed_header_row(&["Name", "St", "Timer"], t);
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
                        ratatui::widgets::Cell::from(s.name.as_str()).style(name_style),
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
        let header = themed_header_row(&["Name", "St"], t);

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
                    ratatui::widgets::Cell::from(tr.name.as_str())
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
    let (mut navigating, mut arrived, mut stuck) = (0usize, 0usize, 0usize);
    for client in &visible {
        if let Some(nav) = app.nav_state.nav_statuses.get(&client.pid) {
            if nav.status.is_moving() {
                navigating += 1;
            } else if nav.status.is_arrived() {
                arrived += 1;
            } else if nav.status.is_stuck() {
                stuck += 1;
            }
        }
    }
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

/// Draw a radius circle around a world position on the map grid.
#[allow(clippy::too_many_arguments)]
fn draw_radius_circle(
    to_grid: &impl Fn(f32, f32) -> (i32, i32),
    center_x: f32,
    center_y: f32,
    radius: f32,
    color: Color,
    w: u16,
    h: u16,
    grid: &mut [Vec<(char, Color)>],
) {
    let steps = (radius * 0.5).clamp(24.0, 120.0) as usize;
    for i in 0..steps {
        let angle = 2.0 * std::f32::consts::PI * (i as f32) / (steps as f32);
        let wx = center_x + radius * angle.cos();
        let wy = center_y + radius * angle.sin();
        let (c, r) = to_grid(-wy, -wx);
        if grid_in_bounds(c, r, w as usize, h as usize) {
            grid[r as usize][c as usize] = ('·', color);
        }
    }
}

fn draw_named_markers(
    app: &App,
    to_grid: &impl Fn(f32, f32) -> (i32, i32),
    w: i32,
    h: i32,
    grid: &mut [Vec<(char, Color)>],
) {
    for marker in &app.map_state.named_markers {
        let (mc, mr) = to_grid(-marker.y, -marker.x);
        if mc < 0 || mc >= w || mr < 0 || mr >= h {
            continue;
        }
        grid[mr as usize][mc as usize] = ('◆', Color::Cyan);
        let label = marker.display_label();
        for (i, ch) in label.chars().take(12).enumerate() {
            let col = mc + 2 + i as i32;
            if col >= 0 && col < w {
                grid[mr as usize][col as usize] = (ch, Color::Cyan);
            }
        }
    }
}

/// Draw camp location overlays: camp center marker (⊕, green), pull point marker
/// (⊗, red), camp radius circle (green dots), and pull radius circle (red dots).
fn draw_camp_overlays(
    app: &App,
    to_grid: &impl Fn(f32, f32) -> (i32, i32),
    w: u16,
    h: u16,
    grid: &mut [Vec<(char, Color)>],
) {
    let Some(camp) = &app.map_state.camp_overlay else {
        return;
    };

    let [cx, cy] = camp.camp_center;
    let [px, py] = camp.pull_point;

    // Camp radius circle (green dots)
    if camp.camp_radius > 0.0 {
        draw_radius_circle(to_grid, cx, cy, camp.camp_radius, Color::Green, w, h, grid);
    }

    // Pull radius circle (red dots)
    if camp.pull_radius > 0.0 {
        draw_radius_circle(to_grid, px, py, camp.pull_radius, Color::Red, w, h, grid);
    }

    // Camp center marker (⊕, green) — drawn after circles so it's always visible
    let (cc, cr) = to_grid(-cy, -cx);
    if cc >= 0 && cc < w as i32 && cr >= 0 && cr < h as i32 {
        grid[cr as usize][cc as usize] = ('⊕', Color::Green);
    }

    // Pull point marker (⊗, red)
    let (pc, pr) = to_grid(-py, -px);
    if pc >= 0 && pc < w as i32 && pr >= 0 && pr < h as i32 {
        grid[pr as usize][pc as usize] = ('⊗', Color::Red);
    }
}

fn draw_radius_overlays(
    app: &App,
    to_grid: &impl Fn(f32, f32) -> (i32, i32),
    w: u16,
    h: u16,
    grid: &mut [Vec<(char, Color)>],
) {
    let Some(player) = app.local_player.as_ref() else {
        return;
    };

    for (center_x, center_y) in std::iter::once((player.x, player.y))
        .chain(app.target.iter().map(|target| (target.x, target.y)))
    {
        for overlay in app
            .map_state
            .cast_radius
            .iter()
            .chain(app.map_state.spell_radius.iter())
        {
            draw_radius_circle(
                to_grid,
                center_x,
                center_y,
                overlay.radius,
                overlay.color,
                w,
                h,
                grid,
            );
        }
    }

    if let Some(aggro) = &app.map_state.aggro_radius
        && app.map_state.show_spawns
    {
        let player_z = player.z;
        let z_range = app.map_state.z_filter_range;
        let filters = &app.map_state.filters;
        for spawn in &app.spawns {
            if spawn.spawn_type != crate::eq::structs::SpawnType::Npc {
                continue;
            }
            if !filters.allows_spawn(spawn) {
                continue;
            }
            if (spawn.z - player_z).abs() > z_range {
                continue;
            }
            draw_radius_circle(
                to_grid,
                spawn.x,
                spawn.y,
                aggro.radius,
                aggro.color,
                w,
                h,
                grid,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eq::structs::{SpawnInfo, SpawnType, StandState};
    use crate::tui::app::ClientState;
    use crate::tui::state::MapRadiusOverlay;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};

    fn test_spawn(id: u32, name: &str, x: f32, y: f32) -> SpawnInfo {
        SpawnInfo {
            name: name.into(),
            displayed_name: name.into(),
            lastname: String::new(),
            spawn_id: id,
            spawn_type: SpawnType::Npc,
            level: 60,
            class_id: 1,
            class: None,
            stand_state: StandState::Standing,
            x,
            y,
            z: 0.0,
            heading: 0.0,
            hp_current: 100,
            hp_max: 100,
            mana_current: 100,
            mana_max: 100,
            endurance_current: 100,
            endurance_max: 100,
            is_gm: false,
            race_id: 1,
            buff_slots: Vec::new(),
            spellbook: Vec::new(),
            memorized_spells: Vec::new(),
            cast_state: None,
        }
    }

    fn test_app_with_spawns() -> App {
        let mut app = App::new();
        let mut client = ClientState::new(77, 0);
        client.spawn_revision = 1;
        client.spawns = vec![
            test_spawn(1, "orc pawn", 4.0, 4.0),
            test_spawn(2, "orc centurion", 5.0, 4.0),
        ];
        client.local_player = Some(test_spawn(99, "Player", 0.0, 0.0));
        app.clients.push(client);
        app.sync_from_selected_client();
        app
    }

    fn spawn_with_type(id: u32, name: &str, x: f32, y: f32, spawn_type: SpawnType) -> SpawnInfo {
        let mut spawn = test_spawn(id, name, x, y);
        spawn.spawn_type = spawn_type;
        spawn
    }

    fn test_transform() -> MapTransform {
        MapTransform {
            center_x: 0.0,
            center_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            using_local_view: false,
        }
    }

    fn minimap_text(app: &App, selected_spawn: Option<(f32, f32, u32)>) -> String {
        let bounds = ViewBounds {
            min_x: -10.0,
            max_x: 10.0,
            min_y: -10.0,
            max_y: 10.0,
        };
        let (_, lines) = draw_minimap_widget(
            &bounds,
            Rect::new(0, 0, 20, 10),
            app,
            selected_spawn,
            &test_transform(),
        );
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_map_view_text(mut app: App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| draw_map_view(frame, Rect::new(0, 0, width, height), &mut app))
            .expect("render map view");
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn rebuild_map_spawn_cache_reuses_cached_cells_for_unchanged_inputs() {
        let mut app = test_app_with_spawns();
        let transform = test_transform();
        let key = map_spawn_cache_key(&app, &transform, 40, 20, Some(0.0), None);

        assert!(rebuild_map_spawn_cache(
            &mut app,
            key.clone(),
            Some(0.0),
            None,
            |x, y| (x as i32, y as i32),
        ));

        let cells = app.map_spawn_cache.cells.clone();
        assert!(!rebuild_map_spawn_cache(
            &mut app,
            key,
            Some(0.0),
            None,
            |x, y| (x as i32, y as i32),
        ));
        assert_eq!(app.map_spawn_cache.cells, cells);
    }

    #[test]
    fn map_spawn_cache_key_tracks_selection_zoom_and_z_filter() {
        let mut app = test_app_with_spawns();
        let transform = test_transform();

        let base_key = map_spawn_cache_key(&app, &transform, 40, 20, Some(0.0), None);
        let selected_key = map_spawn_cache_key(
            &app,
            &transform,
            40,
            20,
            Some(0.0),
            Some(app.spawns[0].spawn_id),
        );
        assert_ne!(selected_key, base_key);

        app.map_state.zoom_in();
        let zoomed_transform = MapTransform {
            scale_x: 1.25,
            scale_y: 1.25,
            ..transform
        };
        let zoomed_key = map_spawn_cache_key(&app, &zoomed_transform, 40, 20, Some(0.0), None);
        assert_ne!(zoomed_key, base_key);

        app.map_state.increase_z_filter();
        let z_changed_key = map_spawn_cache_key(&app, &zoomed_transform, 40, 20, Some(0.0), None);
        assert_ne!(z_changed_key, zoomed_key);
    }

    #[test]
    fn map_spawn_cache_key_only_changes_for_spawn_layer_visibility() {
        let mut app = test_app_with_spawns();
        let transform = test_transform();

        let base_key = map_spawn_cache_key(&app, &transform, 40, 20, Some(0.0), None);
        app.map_state.toggle_layer(1);
        let geometry_toggled_key = map_spawn_cache_key(&app, &transform, 40, 20, Some(0.0), None);
        assert_eq!(geometry_toggled_key, base_key);

        app.map_state.toggle_layer(2);
        let spawns_toggled_key = map_spawn_cache_key(&app, &transform, 40, 20, Some(0.0), None);
        assert_ne!(spawns_toggled_key, base_key);
    }

    #[test]
    fn map_spawn_cache_key_changes_when_filters_toggle() {
        let mut app = test_app_with_spawns();
        let transform = test_transform();
        let base_key = map_spawn_cache_key(&app, &transform, 40, 20, Some(0.0), None);
        app.map_state
            .filters
            .set(crate::tui::state::MapFilterKind::Npc, false);
        let toggled_key = map_spawn_cache_key(&app, &transform, 40, 20, Some(0.0), None);
        assert_ne!(base_key, toggled_key);
    }

    #[test]
    fn map_filters_hide_disabled_categories() {
        let mut app = App::new();
        let mut client = ClientState::new(77, 0);
        client.spawn_revision = 1;
        let pc = spawn_with_type(1, "Dmft01", -1.0, -1.0, SpawnType::Player);
        let npc = spawn_with_type(2, "a skeleton", -2.0, -2.0, SpawnType::Npc);
        client.spawns = vec![pc.clone(), npc];
        client.local_player = Some(pc);
        app.clients.push(client);
        app.sync_from_selected_client();

        let transform = test_transform();
        let key = map_spawn_cache_key(&app, &transform, 40, 20, Some(0.0), None);
        rebuild_map_spawn_cache(&mut app, key, Some(0.0), None, |x, y| (x as i32, y as i32));
        let all_cells = app.map_spawn_cache.cells.len();
        assert!(all_cells >= 2);

        app.map_state
            .filters
            .set(crate::tui::state::MapFilterKind::Npc, false);
        let key = map_spawn_cache_key(&app, &transform, 40, 20, Some(0.0), None);
        rebuild_map_spawn_cache(&mut app, key, Some(0.0), None, |x, y| (x as i32, y as i32));
        let filtered_cells = app.map_spawn_cache.cells.len();
        assert_eq!(filtered_cells, 1);
    }

    #[test]
    fn map_filters_hide_named_when_disabled() {
        let mut app = App::new();
        let mut client = ClientState::new(77, 0);
        client.spawn_revision = 1;
        client.spawns = vec![
            spawn_with_type(1, "Emperor Crush", -3.0, -3.0, SpawnType::Npc),
            spawn_with_type(2, "a legionnaire", -4.0, -4.0, SpawnType::Npc),
        ];
        client.local_player = Some(spawn_with_type(99, "You", -0.5, -0.5, SpawnType::Player));
        app.clients.push(client);
        app.sync_from_selected_client();

        let transform = test_transform();
        let key = map_spawn_cache_key(&app, &transform, 40, 20, Some(0.0), None);
        rebuild_map_spawn_cache(&mut app, key, Some(0.0), None, |x, y| (x as i32, y as i32));
        let with_named = app.map_spawn_cache.cells.len();
        assert!(with_named >= 2);

        app.map_state
            .filters
            .set(crate::tui::state::MapFilterKind::Named, false);
        let key = map_spawn_cache_key(&app, &transform, 40, 20, Some(0.0), None);
        rebuild_map_spawn_cache(&mut app, key, Some(0.0), None, |x, y| (x as i32, y as i32));
        let without_named = app.map_spawn_cache.cells.len();
        assert_eq!(without_named, 1);
    }

    #[test]
    fn minimap_renders_spawn_markers_for_visible_spawns() {
        let mut app = App::new();
        let mut client = ClientState::new(77, 0);
        client.spawn_revision = 1;
        client.spawns = vec![
            spawn_with_type(1, "Emperor Crush", -4.0, -4.0, SpawnType::Npc),
            spawn_with_type(2, "Wanderer", 4.0, 4.0, SpawnType::Player),
        ];
        client.local_player = Some(spawn_with_type(99, "You", 0.0, 0.0, SpawnType::Player));
        app.clients.push(client);
        app.sync_from_selected_client();

        let rendered = minimap_text(&app, None);

        assert!(rendered.contains('!'));
        assert!(rendered.contains('@'));
        assert!(rendered.contains('◆'));
    }

    #[test]
    fn minimap_target_marker_uses_target_line_instead_of_nav_paths() {
        let mut app = test_app_with_spawns();
        // Keep nav paths disabled to prove the target marker follows show_target_line directly.
        app.map_state.show_nav_paths = false;
        app.map_state.show_target_line = true;
        app.target = Some(test_spawn(77, "target", 6.0, 0.0));

        let with_target = minimap_text(&app, None);
        assert!(with_target.contains('✚'));

        app.map_state.show_target_line = false;
        let without_target = minimap_text(&app, None);
        assert!(!without_target.contains('✚'));
    }

    #[test]
    fn map_legend_target_entry_uses_target_line_instead_of_nav_paths() {
        let mut app = test_app_with_spawns();
        // Keep nav paths disabled to prove the legend entry follows show_target_line directly.
        app.map_state.show_nav_paths = false;
        app.map_state.show_target_line = true;
        app.target = Some(test_spawn(77, "target", 6.0, 0.0));

        // Use a wide terminal so the full legend (including the new Hdg entry) fits.
        let with_target = render_map_view_text(app, 140, 16);
        assert!(with_target.contains("Target"));

        let mut without_target_line = test_app_with_spawns();
        without_target_line.map_state.show_nav_paths = false;
        without_target_line.map_state.show_target_line = false;
        without_target_line.target = Some(test_spawn(77, "target", 6.0, 0.0));
        let without_target = render_map_view_text(without_target_line, 140, 16);
        assert!(!without_target.contains("Target"));
    }

    // ── clip_line_z tests ──────────────────────────────────────────────

    #[test]
    fn clip_line_z_both_inside() {
        let result = clip_line_z(0.0, 0.0, 50.0, 100.0, 100.0, 60.0, 55.0, 20.0);
        assert_eq!(result, Some((0.0, 0.0, 100.0, 100.0)));
    }

    #[test]
    fn clip_line_z_both_outside_same_side() {
        // Both below min
        assert_eq!(
            clip_line_z(0.0, 0.0, 10.0, 100.0, 100.0, 20.0, 100.0, 10.0),
            None
        );
        // Both above max
        assert_eq!(
            clip_line_z(0.0, 0.0, 200.0, 100.0, 100.0, 300.0, 100.0, 10.0),
            None
        );
    }

    #[test]
    fn clip_line_z_p1_below_min() {
        // p1 at z=0, p2 at z=100, center=50, range=50 → min=0, max=100
        // Narrow: center=80, range=20 → min=60, max=100
        // p1 z=0 < 60, p2 z=100 inside. t = (60-0)/(100-0) = 0.6
        let result = clip_line_z(0.0, 0.0, 0.0, 100.0, 200.0, 100.0, 80.0, 20.0);
        let (x1, y1, x2, y2) = result.unwrap();
        assert!((x1 - 60.0).abs() < 0.01, "x1={x1}");
        assert!((y1 - 120.0).abs() < 0.01, "y1={y1}");
        assert!((x2 - 100.0).abs() < 0.01);
        assert!((y2 - 200.0).abs() < 0.01);
    }

    #[test]
    fn clip_line_z_p2_above_max() {
        // center=50, range=10 → min=40, max=60
        // p1 z=50 (inside), p2 z=100 (above max)
        // t = (60-100)/(50-100) = -40/-50 = 0.8
        let result = clip_line_z(0.0, 0.0, 50.0, 100.0, 200.0, 100.0, 50.0, 10.0);
        let (x1, y1, x2, y2) = result.unwrap();
        assert!((x1 - 0.0).abs() < 0.01);
        assert!((y1 - 0.0).abs() < 0.01);
        assert!((x2 - 20.0).abs() < 0.01, "x2={x2}");
        assert!((y2 - 40.0).abs() < 0.01, "y2={y2}");
    }

    #[test]
    fn clip_line_z_both_outside_crossing() {
        // p1 below min, p2 above max — line crosses through the range
        // center=50, range=10 → min=40, max=60
        // p1 z=0, p2 z=100
        let result = clip_line_z(0.0, 0.0, 0.0, 100.0, 100.0, 100.0, 50.0, 10.0);
        let (x1, y1, x2, y2) = result.unwrap();
        // Clip p1 to min=40: t=(40-0)/100=0.4 → x=40, y=40
        assert!((x1 - 40.0).abs() < 0.01, "x1={x1}");
        assert!((y1 - 40.0).abs() < 0.01, "y1={y1}");
        // Clip p2 to max=60: t=(60-100)/(0-100)=0.4 → x2=100+0.4*(0-100)=60, y2=60
        assert!((x2 - 60.0).abs() < 0.01, "x2={x2}");
        assert!((y2 - 60.0).abs() < 0.01, "y2={y2}");
    }

    #[test]
    fn clip_line_z_no_player_z_passthrough() {
        // When there's no player_z, clip_line_z isn't called — but verify it handles
        // equal z endpoints gracefully (dz ≈ 0, both inside)
        let result = clip_line_z(10.0, 20.0, 50.0, 30.0, 40.0, 50.0, 50.0, 10.0);
        assert_eq!(result, Some((10.0, 20.0, 30.0, 40.0)));
    }

    #[test]
    fn radius_overlays_render_around_player_and_target() {
        let mut app = test_app_with_spawns();
        app.map_state.cast_radius = Some(MapRadiusOverlay {
            radius: 3.0,
            color: Color::Cyan,
            label: String::from("Cast 3"),
        });
        app.target = Some(test_spawn(500, "target", 20.0, 10.0));

        let mut grid = vec![vec![(' ', Color::Reset); 32]; 24];
        let to_grid = |map_x: f32, map_y: f32| ((-map_y).round() as i32, (-map_x).round() as i32);

        draw_radius_overlays(&app, &to_grid, 32, 24, &mut grid);

        assert_eq!(grid[0][3], ('·', Color::Cyan));
        assert_eq!(grid[10][23], ('·', Color::Cyan));
    }

    #[test]
    fn aggro_radius_renders_around_npc_spawns() {
        let mut app = test_app_with_spawns();
        // test_app_with_spawns adds NPC spawns at (4,4) and (5,4)
        app.map_state.aggro_radius = Some(MapRadiusOverlay {
            radius: 3.0,
            color: Color::Red,
            label: String::from("Aggro 3"),
        });

        let mut grid = vec![vec![(' ', Color::Reset); 32]; 24];
        let to_grid = |map_x: f32, map_y: f32| ((-map_y).round() as i32, (-map_x).round() as i32);

        draw_radius_overlays(&app, &to_grid, 32, 24, &mut grid);

        // spawn at x=4,y=4: at angle=0, wx=4+3=7, wy=4 => to_grid(-4,-7)=(7,4)
        assert_eq!(grid[4][7], ('·', Color::Red));
        // spawn at x=5,y=4: at angle=0, wx=5+3=8, wy=4 => to_grid(-4,-8)=(8,4)
        assert_eq!(grid[4][8], ('·', Color::Red));
    }

    #[test]
    fn aggro_radius_not_drawn_around_players() {
        let mut app = test_app_with_spawns();
        // Add a player-type spawn
        let mut player_spawn = test_spawn(200, "other_player", 10.0, 0.0);
        player_spawn.spawn_type = SpawnType::Player;
        app.spawns.push(player_spawn);

        app.map_state.aggro_radius = Some(MapRadiusOverlay {
            radius: 3.0,
            color: Color::Red,
            label: String::from("Aggro 3"),
        });

        let mut grid = vec![vec![(' ', Color::Reset); 32]; 24];
        let to_grid = |map_x: f32, map_y: f32| ((-map_y).round() as i32, (-map_x).round() as i32);

        draw_radius_overlays(&app, &to_grid, 32, 24, &mut grid);

        // player_spawn at x=10,y=0: at angle=0, wx=13, wy=0 => to_grid(0,-13)=(13,0)
        // Should NOT have aggro circle dot (player type excluded)
        assert_ne!(grid[0][13], ('·', Color::Red));
    }

    #[test]
    fn aggro_radius_not_drawn_when_show_spawns_off() {
        let mut app = test_app_with_spawns();
        app.map_state.show_spawns = false;
        app.map_state.aggro_radius = Some(MapRadiusOverlay {
            radius: 3.0,
            color: Color::Red,
            label: String::from("Aggro 3"),
        });

        let mut grid = vec![vec![(' ', Color::Reset); 32]; 24];
        let to_grid = |map_x: f32, map_y: f32| ((-map_y).round() as i32, (-map_x).round() as i32);

        draw_radius_overlays(&app, &to_grid, 32, 24, &mut grid);

        // spawn at x=4,y=4: circle point at (7,4) should not be drawn when spawns hidden
        assert_ne!(grid[4][7], ('·', Color::Red));
    }

    #[test]
    fn aggro_radius_respects_z_filter() {
        let mut app = test_app_with_spawns();
        // Move all existing spawns far above player (z=0) by setting z_filter_range tight
        for spawn in &mut app.spawns {
            spawn.z = 200.0; // 200 units above player z=0
        }
        app.map_state.z_filter_range = 10.0; // only show spawns within 10 units
        app.map_state.aggro_radius = Some(MapRadiusOverlay {
            radius: 3.0,
            color: Color::Red,
            label: String::from("Aggro 3"),
        });

        let mut grid = vec![vec![(' ', Color::Reset); 32]; 24];
        let to_grid = |map_x: f32, map_y: f32| ((-map_y).round() as i32, (-map_x).round() as i32);

        draw_radius_overlays(&app, &to_grid, 32, 24, &mut grid);

        // Spawns are z-filtered out, so aggro circles should not appear
        assert_ne!(grid[4][7], ('·', Color::Red));
    }

    // ── Player position and heading tests ─────────────────────────────────

    #[test]
    fn player_marker_renders_on_map_with_distinct_glyph() {
        // Player at (0,0) with default heading should render ◆ on the map.
        let app = test_app_with_spawns();
        let rendered = render_map_view_text(app, 80, 20);
        assert!(
            rendered.contains('◆'),
            "Player marker ◆ should appear on the map"
        );
    }

    #[test]
    fn player_marker_color_is_map_you() {
        // Verify that heading_arrow_char returns a non-space arrow for any heading.
        // We test the function directly via a full render and confirm the arrow glyphs appear.
        let app = test_app_with_spawns(); // player heading=0 → North → ↑
        let rendered = render_map_view_text(app, 80, 20);
        // The ↑ heading arrow should appear near the player marker.
        assert!(
            rendered.contains('↑')
                || rendered.contains('↗')
                || rendered.contains('→')
                || rendered.contains('↘')
                || rendered.contains('↓')
                || rendered.contains('↙')
                || rendered.contains('←')
                || rendered.contains('↖'),
            "A heading arrow character should appear on the map"
        );
    }

    #[test]
    fn heading_arrow_char_cardinal_directions() {
        // EQ heading: 0=North, 128=West, 256=South, 384=East
        assert_eq!(heading_arrow_char(0.0), '↑', "heading=0 (N) should be ↑");
        assert_eq!(heading_arrow_char(128.0), '←', "heading=128 (W) should be ←");
        assert_eq!(heading_arrow_char(256.0), '↓', "heading=256 (S) should be ↓");
        assert_eq!(heading_arrow_char(384.0), '→', "heading=384 (E) should be →");
    }

    #[test]
    fn heading_arrow_char_diagonal_directions() {
        // NE=448, NW=64, SW=192, SE=320
        assert_eq!(heading_arrow_char(448.0), '↗', "heading=448 (NE) should be ↗");
        assert_eq!(heading_arrow_char(64.0), '↖', "heading=64 (NW) should be ↖");
        assert_eq!(heading_arrow_char(192.0), '↙', "heading=192 (SW) should be ↙");
        assert_eq!(heading_arrow_char(320.0), '↘', "heading=320 (SE) should be ↘");
    }

    #[test]
    fn heading_arrow_char_wraps_512() {
        // 512 should wrap to 0 (North → ↑)
        assert_eq!(heading_arrow_char(512.0), '↑', "heading=512 should wrap to North ↑");
        // 768 % 512 = 256 → South (↓)
        assert_eq!(heading_arrow_char(768.0), '↓', "heading=768 should wrap to heading=256 (S) ↓");
    }

    #[test]
    fn map_legend_contains_player_marker_and_heading() {
        let app = test_app_with_spawns();
        let rendered = render_map_view_text(app, 120, 20);
        assert!(rendered.contains("You"), "Legend should contain 'You' label");
        assert!(rendered.contains("Hdg"), "Legend should contain 'Hdg' heading label");
    }

    #[test]
    fn minimap_renders_player_with_heading_arrow() {
        let mut app = App::new();
        let mut client = ClientState::new(77, 0);
        client.spawn_revision = 1;
        let mut player = test_spawn(99, "Player", 0.0, 0.0);
        player.heading = 0.0; // North
        client.local_player = Some(player);
        app.clients.push(client);
        app.sync_from_selected_client();

        let rendered = minimap_text(&app, None);
        // Player ◆ marker should be present
        assert!(rendered.contains('◆'), "Minimap should show player ◆ marker");
        // Heading arrow ↑ (North at heading=0) should also appear
        assert!(
            rendered.contains('↑')
                || rendered.contains('↗')
                || rendered.contains('→')
                || rendered.contains('↘')
                || rendered.contains('↓')
                || rendered.contains('↙')
                || rendered.contains('←')
                || rendered.contains('↖'),
            "Minimap should show a heading arrow adjacent to the player marker"
        );
    }
}
