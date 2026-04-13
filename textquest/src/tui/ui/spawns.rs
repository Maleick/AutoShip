//! Spawns screen — filterable, searchable spawn list + hex dump viewer.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, Wrap},
};

use super::widgets::{
    cast_summary, cast_time_remaining_label, hp_color, panel, spawn_info_lines, spawn_row_style,
    themed_header_row,
};
use crate::eq::structs::SpawnInfo;
use crate::tui::app::{ActivePanel, App};

fn current_spellset_lines(spells: &[crate::eq::structs::SpellSlot]) -> Vec<String> {
    spells
        .chunks(3)
        .map(|chunk| {
            chunk
                .iter()
                .map(|spell| format!("G{} {}", spell.slot + 1, spell.display_name()))
                .collect::<Vec<_>>()
                .join("  ")
        })
        .collect()
}

/// Draw the full spawns screen (spawn list + details).
pub fn draw_spawns_screen(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    draw_spawn_list(frame, area, app);
}

/// Draw the filterable, sortable spawn list table.
pub fn draw_spawn_list(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    let filtered_indices = app.filtered_spawn_indices().to_vec();
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
    let player_level: Option<u8> = app
        .active_client()
        .and_then(|c| c.local_player.as_ref())
        .map(|p| p.level);

    let client_label = app
        .active_client()
        .and_then(|c| c.local_player.as_ref())
        .map_or_else(
            || "???".into(),
            |p| app.redact_name(&p.displayed_name).into_owned(),
        );

    let fl = app.spawns_state.spawn_type_filter.label();

    // Build compact indicators for sort and nav scope
    let sort_ind = if app.spawns_state.sort_column != crate::tui::app::SpawnSort::Default {
        let arrow = if app.spawns_state.sort_ascending {
            "\u{2191}"
        } else {
            "\u{2193}"
        };
        format!(" sort:{}{arrow}", app.spawns_state.sort_column.label())
    } else {
        String::new()
    };
    let nav_ind = format!(" nav:{}", app.spawns_state.nav_scope.label());

    let title = if app.spawns_state.search_mode {
        format!(
            " Spawns: {} ({}) [{}] search: \"{}\" [Esc to close] ",
            client_label,
            filtered_indices.len(),
            fl,
            app.spawns_state.spawn_filter
        )
    } else if !app.spawns_state.spawn_filter.is_empty() {
        format!(
            " Spawns: {} ({}) [{}] filter: \"{}\"{sort_ind}{nav_ind} ",
            client_label,
            filtered_indices.len(),
            fl,
            app.spawns_state.spawn_filter
        )
    } else if app.spawns_state.spawn_type_filter != crate::tui::app::SpawnFilter::All {
        format!(
            " Spawns: {} ({}) [{}]{sort_ind}{nav_ind} ",
            client_label,
            filtered_indices.len(),
            fl
        )
    } else {
        format!(
            " Spawns: {} ({}){sort_ind}{nav_ind} ",
            client_label,
            filtered_indices.len()
        )
    };

    // Get local player position for distance calculation
    let player_pos: Option<(f32, f32)> = app
        .active_client()
        .and_then(|c| c.local_player.as_ref())
        .map(|p| (p.x, p.y));

    // Show coordinate columns when terminal is wide enough (>= 120 chars)
    let show_coords = area.width >= 120;
    // Show live cast summaries only when there's room to keep names readable.
    let show_cast = area.width >= 145;

    let mut header_cells = vec!["Type", "Name", "Race", "Cls", "Lv", "HP%", "Dist2D", "ID"];
    if show_cast {
        header_cells.push("Cast");
    }
    if show_coords {
        header_cells.push("X");
        header_cells.push("Y");
        header_cells.push("Z");
    }
    let header = themed_header_row(header_cells.as_slice(), t);

    let highlight_style = Style::default()
        .bg(t.row_selected_bg)
        .add_modifier(Modifier::BOLD);

    let rows: Vec<Row> = filtered_indices
        .iter()
        .filter_map(|&index| app.spawns.get(index))
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
            if show_cast {
                cells.push(Cell::from(
                    spawn
                        .cast_state
                        .as_ref()
                        .filter(|cast| cast.is_casting())
                        .map_or_else(String::new, cast_summary),
                ));
            }
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
    if show_cast {
        constraints.push(Constraint::Min(14)); // Cast
    }
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

/// Draw the target info panel (used from debug screen).
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

/// Draw a single spawn info panel with the given title.
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

/// Annotation color palette — 6 distinct colors cycled across fields.
const ANNOTATION_COLORS: [Color; 6] = [
    Color::Cyan,
    Color::Yellow,
    Color::Green,
    Color::Magenta,
    Color::Blue,
    Color::Red,
];

/// Draw the hex dump panel for raw memory inspection.
///
/// When `show_annotations` is true, bytes within known struct fields are
/// color-coded and a field name label is shown at the right margin.
pub fn draw_hex_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let is_active = app.active_panel == ActivePanel::DebugHexDump;
    let border_style = if is_active {
        t.border_warn
    } else {
        t.border_dim
    };

    let ann_indicator = if app.hex_state.show_annotations {
        " [a:annotations ON]"
    } else {
        ""
    };
    let blk = panel(
        format!(" Hex — {} {ann_indicator}", app.hex_state.hex_label),
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

    let annotating = app.hex_state.show_annotations && !app.hex_state.annotations.is_empty();

    let inner_height = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line<'_>> = (0..inner_height)
        .filter_map(|row| {
            let row_offset = row * 16;
            if row_offset >= app.hex_state.hex_data.len() {
                return None;
            }
            let addr = app.hex_state.hex_address + row_offset;
            let end = (row_offset + 16).min(app.hex_state.hex_data.len());
            let chunk = &app.hex_state.hex_data[row_offset..end];

            let mut spans = Vec::with_capacity(8);
            spans.push(Span::styled(
                format!("{addr:08x}"),
                Style::default().fg(t.text_muted),
            ));
            spans.push(Span::raw("  "));

            if annotating {
                // Build per-byte colored hex spans.
                for (i, b) in chunk.iter().enumerate() {
                    let byte_offset = row_offset + i;
                    let style = if let Some(ann) = app.hex_state.annotation_at(byte_offset) {
                        Style::default().fg(ANNOTATION_COLORS[ann.color_idx as usize % 6])
                    } else {
                        Style::default().fg(t.text_normal)
                    };
                    spans.push(Span::styled(format!("{b:02x} "), style));
                }
                // Pad if row is short.
                let pad = 16usize.saturating_sub(chunk.len());
                if pad > 0 {
                    spans.push(Span::raw(" ".repeat(pad * 3)));
                }
            } else {
                let hex_str: String = chunk.iter().map(|b| format!("{b:02x} ")).collect();
                spans.push(Span::styled(
                    format!("{hex_str:<48}"),
                    Style::default().fg(t.text_normal),
                ));
            }

            spans.push(Span::raw(" "));

            // ASCII column.
            let ascii_str: String = chunk
                .iter()
                .map(|b| {
                    if b.is_ascii_graphic() || *b == b' ' {
                        *b as char
                    } else {
                        '·'
                    }
                })
                .collect();
            spans.push(Span::styled(
                ascii_str,
                Style::default().fg(t.text_highlight),
            ));

            // Annotation label: show field name if this row starts at or contains a field boundary.
            if annotating && let Some(ann) = app.hex_state.annotation_at(row_offset) {
                let color = ANNOTATION_COLORS[ann.color_idx as usize % 6];
                // Only show label on the first row of the field.
                if row_offset <= ann.offset + 15 {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("◀ {}", ann.name),
                        Style::default().fg(color),
                    ));
                }
            }

            Some(Line::from(spans))
        })
        .collect();

    frame.render_widget(Paragraph::new(lines).block(blk), area);
}

// ─── Debug screen layout ─────────────────────────────────────────────────────

/// Draw the debug screen layout (spawn list + target + hex dump + explorer).
///
/// Layout varies by `LayoutPreset`:
/// - **Default**: original 2-column (spawns + detail/hex)
/// - **Alternate**: 3-column with explorer on the right
/// - **Compact**: explorer-focused (explorer + hex, minimal spawns)
pub fn draw_debug_screen(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    use crate::tui::app::LayoutPreset;

    match app.current_layout() {
        LayoutPreset::Default => draw_debug_default(frame, area, app),
        LayoutPreset::Alternate => draw_debug_with_explorer(frame, area, app),
        LayoutPreset::Compact => draw_debug_explorer_focused(frame, area, app),
    }
}

/// Original debug layout: spawns + player/target/hex + hook rotation.
fn draw_debug_default(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    if area.width < 110 {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(18),
                Constraint::Min(10),
                Constraint::Length(8),
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
        draw_hook_rotation_panel(frame, rows[2], app);
        draw_spawn_list(frame, rows[3], app);
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
            Constraint::Length(8),
            Constraint::Min(10),
        ])
        .split(cols[0]);

    draw_player_detail(frame, left[0], app);
    draw_target_panel(frame, left[1], app);
    draw_hook_rotation_panel(frame, left[2], app);
    draw_hex_panel(frame, left[3], app);
    draw_spawn_list(frame, cols[1], app);
}

/// 3-column layout: detail/hex | spawns | EQ Internals + explorer stacked.
fn draw_debug_with_explorer(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    use super::eq_internals::draw_eq_internals_panel;
    use super::explorer::draw_explorer_panel;

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(35),
            Constraint::Percentage(40),
        ])
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

    // Right column: EQ Internals on top, hook rotation in middle, explorer below.
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(8),
            Constraint::Min(5),
        ])
        .split(cols[2]);

    draw_eq_internals_panel(frame, right[0], app);
    draw_hook_rotation_panel(frame, right[1], app);
    draw_explorer_panel(frame, right[2], app);
}

/// EQ Internals-focused layout: internals + hex, minimal spawns.
fn draw_debug_explorer_focused(frame: &mut Frame, area: ratatui::layout::Rect, app: &mut App) {
    use super::eq_internals::draw_eq_internals_panel;

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    draw_eq_internals_panel(frame, rows[0], app);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    draw_hex_panel(frame, bottom[0], app);
    draw_spawn_list(frame, bottom[1], app);
}

fn draw_player_detail(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    use crate::tui::sprites;
    let t = &app.theme;
    let blk = panel(" Selected Character ", t.border_primary, t);

    let Some(client) = app.active_client() else {
        frame.render_widget(
            Paragraph::new("No client selected")
                .block(blk)
                .style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
    };

    let Some(player) = &client.local_player else {
        frame.render_widget(
            Paragraph::new("Not logged in")
                .block(blk)
                .style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
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
            let mut spans = vec![
                Span::styled("Casting ", Style::default().fg(t.text_muted)),
                Span::styled(
                    cast_summary(cast),
                    Style::default()
                        .fg(t.text_highlight)
                        .add_modifier(Modifier::BOLD),
                ),
            ];
            if let Some(remaining) = cast_time_remaining_label(cast) {
                spans.push(Span::styled(
                    format!("  {remaining}"),
                    Style::default().fg(t.text_muted),
                ));
            }
            lines.push(Line::from(spans));
        }
        if let Some(gem_etas) = cast.gem_etas.as_ref() {
            let recast_strs: Vec<String> = gem_etas
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
    }

    if !player.memorized_spells.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Spellset", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("  {} gems", player.memorized_spells.len()),
                Style::default().fg(t.text_accent),
            ),
        ]));
        for spellset_line in current_spellset_lines(&player.memorized_spells) {
            lines.push(Line::from(vec![
                Span::styled("         ", Style::default().fg(t.text_muted)),
                Span::styled(spellset_line, Style::default().fg(t.text_normal)),
            ]));
        }
    }

    if !player.spellbook.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Book ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{} spells scribed", player.spellbook.len()),
                Style::default().fg(t.text_server),
            ),
        ]));
    }

    lines.push(Line::from(""));

    for sprite_line in
        sprites::class_sprite(player.class.as_ref(), &player.stand_state, app.tick_count)
    {
        lines.push(sprite_line);
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

// ─── Hook Rotation Status Panel ──────────────────────────────────────────────

/// Draw the hook rotation status panel in the Debug screen.
///
/// Shows a table with one row per hook slot: name, state (ACTIVE / UNHOOKED),
/// time since last rotation, and countdown to next rotation.
pub fn draw_hook_rotation_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    use crate::tui::app::HookSlotState;
    use crate::tui::ui::widgets::themed_header_row;

    let t = &app.theme;
    let state = &app.hook_rotation_state;

    let interval_label = if state.interval_ms >= 1_000 {
        format!("{:.1}s", state.interval_ms as f64 / 1_000.0)
    } else {
        format!("{}ms", state.interval_ms)
    };

    let title = format!(" Hook Rotation  interval: {interval_label} ");
    let blk = panel(title.as_str(), t.border_dim, t);

    let header = themed_header_row(&["Hook", "State", "Last Rotated", "Next In"], t);

    let text_bright = t.text_bright;
    let text_secondary = t.text_secondary;
    let text_muted = t.text_muted;

    let rows: Vec<Row> = state
        .entries
        .iter()
        .map(|entry| {
            let state_label = entry.state.label();
            let state_color = match entry.state {
                HookSlotState::Active => Color::Green,
                HookSlotState::Unhooked => Color::Yellow,
            };

            let last_rotated = entry
                .ms_since_last_rotation()
                .map(|ms| {
                    if ms >= 1_000 {
                        format!("{:.1}s ago", ms as f64 / 1_000.0)
                    } else {
                        format!("{ms}ms ago")
                    }
                })
                .unwrap_or_else(|| String::from("—"));

            let next_rotation = entry
                .ms_until_next_rotation()
                .map(|ms| {
                    if ms == 0 {
                        String::from("now")
                    } else if ms >= 1_000 {
                        format!("{:.1}s", ms as f64 / 1_000.0)
                    } else {
                        format!("{ms}ms")
                    }
                })
                .unwrap_or_else(|| String::from("—"));

            Row::new(vec![
                Cell::from(Span::styled(
                    entry.name.as_str(),
                    Style::default().fg(text_bright),
                )),
                Cell::from(Span::styled(
                    state_label,
                    Style::default().fg(state_color).add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::styled(
                    last_rotated,
                    Style::default().fg(text_secondary),
                )),
                Cell::from(Span::styled(
                    next_rotation,
                    Style::default().fg(text_muted),
                )),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(blk);

    frame.render_widget(table, area);
}

#[cfg(test)]
mod tests {
    use super::current_spellset_lines;
    use crate::eq::structs::SpellSlot;

    #[test]
    fn current_spellset_lines_groups_spells_by_three_slots() {
        let lines = current_spellset_lines(&[
            SpellSlot {
                slot: 0,
                spell_id: 1,
                spell_name: Some("Complete Heal".into()),
            },
            SpellSlot {
                slot: 1,
                spell_id: 2,
                spell_name: Some("Celestial Remedy".into()),
            },
            SpellSlot {
                slot: 2,
                spell_id: 3,
                spell_name: Some("Yaulp".into()),
            },
            SpellSlot {
                slot: 3,
                spell_id: 4,
                spell_name: Some("Symbol".into()),
            },
        ]);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "G1 Complete Heal  G2 Celestial Remedy  G3 Yaulp");
        assert_eq!(lines[1], "G4 Symbol");
    }

    #[test]
    fn current_spellset_lines_uses_spell_id_fallback_when_name_missing() {
        let lines = current_spellset_lines(&[SpellSlot {
            slot: 6,
            spell_id: 789,
            spell_name: None,
        }]);

        assert_eq!(lines, vec!["G7 Spell 789"]);
    }
}
