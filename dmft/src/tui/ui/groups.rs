//! Groups screen — dynamic grid of group panels, each showing per-slot HP/mana.
//!
//! Prefers live EQ group membership data (from `ClientState.group_info`) when
//! available, falling back to config-based account-range grouping otherwise.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::widgets::{
    WidthClass, classify_width, hp_color, panel, render_cast_bar, truncate_inline,
};
use crate::eq::structs::BuffSlot;
use crate::tui::app::extract_account_number;
use crate::tui::app::{App, ClientState, GroupDef, LiveGroup};
use crate::tui::theme::Theme;

struct MemberRenderEntry<'a> {
    primary: Line<'a>,
    cast: Option<Line<'a>>,
    buff: Option<Line<'a>>,
}

// ── Shared member-row helpers ───────────────────────────────────────────────

/// Build a member info line for a connected player.
fn member_line<'a>(
    player: &crate::eq::structs::SpawnInfo,
    is_leader: bool,
    display_name: String,
    t: &Theme,
) -> Line<'a> {
    let hp_pct = player.hp_pct();
    let mana_str = if player.mana_max > 0 {
        format!(" {:>3.0}%mp", player.mana_pct())
    } else {
        "     -".into()
    };

    let name_style = if is_leader {
        Style::default()
            .fg(t.text_highlight)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(t.text_normal)
    };

    let leader_marker = if is_leader { "*" } else { " " };
    let short_name = truncate_inline(&display_name, 12);

    Line::from(vec![
        Span::styled(leader_marker, Style::default().fg(t.text_accent)),
        Span::styled(format!("{short_name:<12}"), name_style),
        Span::styled(
            format!("{:<4}", player.class_str()),
            Style::default().fg(t.text_accent),
        ),
        Span::styled(
            format!("{:>3}", player.level),
            Style::default().fg(t.text_secondary),
        ),
        Span::styled(
            format!(" {hp_pct:>3.0}%"),
            Style::default().fg(hp_color(hp_pct, t)),
        ),
        Span::styled(mana_str, Style::default().fg(t.mana_color)),
    ])
}

/// Build a buff timer row for a player (returns None if no active buffs).
fn buff_line<'a>(player: &crate::eq::structs::SpawnInfo, t: &Theme) -> Option<Line<'a>> {
    let active_buffs: Vec<&BuffSlot> = player
        .buff_slots
        .iter()
        .filter(|b| !b.is_empty())
        .take(6)
        .collect();
    if active_buffs.is_empty() {
        return None;
    }
    let mut buff_spans: Vec<Span<'_>> = vec![Span::raw("  ")];
    for b in &active_buffs {
        buff_spans.push(Span::styled(
            format!("{:04X}", b.spell_id),
            Style::default().fg(t.text_highlight),
        ));
        buff_spans.push(Span::styled(
            format!("({}) ", b.duration_str()),
            Style::default().fg(t.text_muted),
        ));
    }
    Some(Line::from(buff_spans))
}

fn member_cast_line<'a>(
    app: &App,
    client: &ClientState,
    available_width: usize,
    t: &Theme,
) -> Option<Line<'a>> {
    app.client_cast_display(client).map(|cast_display| {
        render_cast_bar(
            &cast_display,
            available_width.saturating_sub(2),
            if cast_display.exact {
                t.hp_high
            } else {
                t.text_highlight
            },
            if cast_display.exact {
                t.hp_high
            } else {
                t.text_accent
            },
            t.text_secondary,
            t.text_muted,
        )
    })
}

/// Build the operating mode indicator line.
fn mode_line<'a>(app: &App) -> Line<'a> {
    let t = &app.theme;
    let mode_str = format!("{}", app.operating_mode);
    let mode_color = match mode_str.as_str() {
        "Camp" => t.mode_camp,
        "Hunt" => t.mode_hunt,
        _ => t.text_muted,
    };
    Line::from(vec![
        Span::styled("Mode ", Style::default().fg(t.text_muted)),
        Span::styled(
            mode_str,
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ),
    ])
}

fn assemble_member_lines<'a>(
    entries: Vec<MemberRenderEntry<'a>>,
    max_lines: usize,
    width_class: WidthClass,
    mode: Option<Line<'a>>,
) -> Vec<Line<'a>> {
    if max_lines == 0 {
        return Vec::new();
    }

    let visible_count = entries.len().min(max_lines);
    let detail_capacity = max_lines.saturating_sub(visible_count);

    let active_cast_total = entries
        .iter()
        .take(visible_count)
        .filter(|entry| entry.cast.is_some())
        .count();
    let cast_budget = detail_capacity.min(active_cast_total);
    let buff_budget =
        if width_class == WidthClass::Wide && cast_budget > 0 && detail_capacity > cast_budget {
            detail_capacity - cast_budget
        } else {
            0
        };

    let mut lines = Vec::with_capacity(max_lines);
    let mut remaining_cast = cast_budget;
    let mut remaining_buff = buff_budget;

    for entry in entries.into_iter().take(visible_count) {
        if lines.len() >= max_lines {
            break;
        }
        lines.push(entry.primary);

        if remaining_cast > 0
            && lines.len() < max_lines
            && let Some(detail) = entry.cast
        {
            lines.push(detail);
            remaining_cast -= 1;
        }

        if remaining_buff > 0
            && lines.len() < max_lines
            && let Some(detail) = entry.buff
        {
            lines.push(detail);
            remaining_buff -= 1;
        }
    }

    if let Some(mode_line) = mode
        && lines.len() < max_lines
    {
        lines.push(mode_line);
    }

    lines.truncate(max_lines);
    lines
}

/// Draw the groups overview screen showing all group members.
pub fn draw_groups_screen(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    if app.has_live_group_data() {
        draw_live_groups_screen(frame, area, app);
    } else {
        draw_config_groups_screen(frame, area, app);
    }
}

// ── Live group rendering (from EQ GroupInfo) ─────────────────────────────────

fn draw_live_groups_screen(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let (live_groups, ungrouped) = app.build_live_groups();

    let total_panels = live_groups.len() + usize::from(!ungrouped.is_empty());

    if total_panels == 0 {
        frame.render_widget(
            Paragraph::new("No group data available")
                .block(panel(" Groups ", t.border_dim, t))
                .style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
    }

    let (num_rows, num_cols) = grid_dims(total_panels);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            (0..num_rows)
                .map(|_| Constraint::Ratio(1, num_rows as u32))
                .collect::<Vec<_>>(),
        )
        .split(area);

    let col_constraints: Vec<Constraint> = (0..num_cols)
        .map(|_| Constraint::Ratio(1, num_cols as u32))
        .collect();

    let mut panel_idx = 0;
    for row in rows.iter() {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints.clone())
            .split(*row);

        for col in cols.iter() {
            if panel_idx < live_groups.len() {
                draw_live_group_panel(frame, *col, app, &live_groups[panel_idx], panel_idx);
            } else if panel_idx == live_groups.len() && !ungrouped.is_empty() {
                draw_ungrouped_panel(frame, *col, app, &ungrouped);
            }
            panel_idx += 1;
        }
    }
}

fn draw_live_group_panel(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &App,
    group: &LiveGroup,
    group_idx: usize,
) {
    let t = &app.theme;
    let focused = app.active_group == Some(group_idx);

    // Collect connected clients for each member
    let connected: Vec<(&str, Option<&ClientState>)> = group
        .member_names
        .iter()
        .map(|name| (name.as_str(), app.find_client_by_name(name)))
        .collect();

    let online = connected.iter().filter(|(_, c)| c.is_some()).count();
    let total = group.member_names.len();

    let has_dead = connected.iter().any(|(_, c)| {
        c.and_then(|c| c.local_player.as_ref())
            .is_some_and(|p| p.hp_current == 0)
    });

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

    let leader_display = app.redact_name(&group.leader);
    let title = format!(" {} ({}/{}) {} ", leader_display, online, total, group.zone);

    let effective_style = if focused {
        border_style.add_modifier(Modifier::BOLD)
    } else {
        border_style
    };
    let blk = panel(title.as_str(), effective_style, t);

    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let max_lines = inner.height as usize;
    let width_class = classify_width(inner.width);
    let mut entries: Vec<MemberRenderEntry<'_>> = Vec::new();

    for (name, client_opt) in &connected {
        if let Some(client) = client_opt {
            if let Some(player) = &client.local_player {
                let display_name = app.redact_name(&player.displayed_name).into_owned();
                let is_leader = *name == group.leader;
                entries.push(MemberRenderEntry {
                    primary: member_line(player, is_leader, display_name, t),
                    cast: member_cast_line(app, client, inner.width as usize, t),
                    buff: buff_line(player, t),
                });
            } else {
                entries.push(MemberRenderEntry {
                    primary: Line::from(Span::styled(
                        format!("  PID {} (loading…)", client.pid),
                        Style::default().fg(t.text_muted),
                    )),
                    cast: None,
                    buff: None,
                });
            }
        } else {
            // Member not connected
            let display_name = app.redact_name(name).into_owned();
            entries.push(MemberRenderEntry {
                primary: Line::from(vec![Span::styled(
                    format!("  {:<12} offline", truncate_inline(&display_name, 12)),
                    Style::default().fg(t.text_muted),
                )]),
                cast: None,
                buff: None,
            });
        }
    }

    let lines = assemble_member_lines(entries, max_lines, width_class, Some(mode_line(app)));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_ungrouped_panel(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &App,
    ungrouped_indices: &[usize],
) {
    let t = &app.theme;
    let ungrouped_title = format!(" Ungrouped ({}) ", ungrouped_indices.len());
    let blk = panel(ungrouped_title.as_str(), t.border_dim, t);

    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let mut lines: Vec<Line<'_>> = Vec::new();
    for &idx in ungrouped_indices {
        if let Some(client) = app.clients.get(idx) {
            let name = if let Some(p) = &client.local_player {
                app.redact_name(&p.displayed_name).into_owned()
            } else if !client.character_name.is_empty() {
                app.redact_name(&client.character_name).into_owned()
            } else {
                format!("PID {}", client.pid)
            };

            if let Some(player) = &client.local_player {
                let hp_pct = player.hp_pct();
                let mana_str = if player.mana_max > 0 {
                    format!(" {:>3.0}%mp", player.mana_pct())
                } else {
                    "     -".into()
                };
                lines.push(Line::from(vec![
                    Span::styled(format!(" {name:<12}"), Style::default().fg(t.text_normal)),
                    Span::styled(
                        format!("{:<4}", player.class_str()),
                        Style::default().fg(t.text_accent),
                    ),
                    Span::styled(
                        format!(" {hp_pct:>3.0}%"),
                        Style::default().fg(hp_color(hp_pct, t)),
                    ),
                    Span::styled(mana_str, Style::default().fg(t.mana_color)),
                ]));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("  {name} (loading…)"),
                    Style::default().fg(t.text_muted),
                )));
            }
        }
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

// ── Config-based group rendering (fallback) ──────────────────────────────────

fn draw_config_groups_screen(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
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

    let (num_rows, num_cols) = grid_dims(group_count);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            (0..num_rows)
                .map(|_| Constraint::Ratio(1, num_rows as u32))
                .collect::<Vec<_>>(),
        )
        .split(area);

    let col_constraints: Vec<Constraint> = (0..num_cols)
        .map(|_| Constraint::Ratio(1, num_cols as u32))
        .collect();

    let mut panel_idx = 0;
    for row in rows.iter() {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints.clone())
            .split(*row);

        for col in cols.iter() {
            if panel_idx < group_count {
                draw_config_group_panel(frame, *col, app, &app.groups[panel_idx], panel_idx);
            }
            panel_idx += 1;
        }
    }
}

/// Collect all client states that belong to a given group definition.
pub fn clients_in_group<'a>(
    app: &'a App,
    group: &GroupDef,
) -> Vec<&'a crate::tui::app::ClientState> {
    let (lo, hi) = group.account_range;
    app.clients
        .iter()
        .filter(|c| {
            let name = if !c.character_name.is_empty() {
                &c.character_name
            } else if let Some(p) = &c.local_player {
                &p.displayed_name
            } else {
                return false;
            };
            extract_account_number(name).is_some_and(|n| n >= lo && n <= hi)
        })
        .collect()
}

fn draw_config_group_panel(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &App,
    group: &GroupDef,
    group_idx: usize,
) {
    let t = &app.theme;
    let members = clients_in_group(app, group);
    let online = members.len();
    let (lo, hi) = group.account_range;
    let total = (hi - lo + 1) as usize;
    let focused = app.active_group == Some(group_idx);

    let has_dead = members
        .iter()
        .any(|c| c.local_player.as_ref().is_some_and(|p| p.hp_current == 0));

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

    let zone = members.first().map_or("---", |c| c.zone_name.as_str());
    let title = format!(
        " G{} {} ({}/{}) {} ",
        group.id, group.name, online, total, zone
    );

    let effective_style = if focused {
        border_style.add_modifier(Modifier::BOLD)
    } else {
        border_style
    };
    let blk = panel(title.as_str(), effective_style, t);

    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    // slot_map: account_num → client
    let mut slot_map: std::collections::HashMap<u8, &crate::tui::app::ClientState> =
        std::collections::HashMap::new();
    for client in &members {
        let name = if !client.character_name.is_empty() {
            &client.character_name
        } else if let Some(p) = &client.local_player {
            &p.displayed_name
        } else {
            continue;
        };
        if let Some(n) = extract_account_number(name) {
            slot_map.insert(n, client);
        }
    }

    let config_map: std::collections::HashMap<u8, &crate::config::AccountEntry> = app
        .accounts_config
        .as_ref()
        .map(|cfg| {
            cfg.accounts
                .iter()
                .filter(|a| a.group == u32::from(group.id))
                .filter_map(|a| extract_account_number(&a.name).map(|n| (n, a)))
                .collect()
        })
        .unwrap_or_default();

    let max_lines = inner.height as usize;
    let slot_count = (hi - lo + 1) as usize;
    let width_class = classify_width(inner.width);
    let mut entries: Vec<MemberRenderEntry<'_>> = Vec::new();

    for acct_num in lo..=hi {
        if let Some(client) = slot_map.get(&acct_num) {
            if let Some(player) = &client.local_player {
                let name = app.redact_name(&player.displayed_name).into_owned();
                // Config groups don't have a leader concept per se; no leader marker
                entries.push(MemberRenderEntry {
                    primary: member_line(player, false, name, t),
                    cast: member_cast_line(app, client, inner.width as usize, t),
                    buff: buff_line(player, t),
                });
            } else {
                entries.push(MemberRenderEntry {
                    primary: Line::from(Span::styled(
                        format!("  PID {} (loading…)", client.pid),
                        Style::default().fg(t.text_muted),
                    )),
                    cast: None,
                    buff: None,
                });
            }
        } else if let Some(acct) = config_map.get(&acct_num) {
            entries.push(MemberRenderEntry {
                primary: Line::from(vec![Span::styled(
                    format!("  #{:02} {:<4} offline", acct_num, acct.class),
                    Style::default().fg(t.text_muted),
                )]),
                cast: None,
                buff: None,
            });
        } else {
            entries.push(MemberRenderEntry {
                primary: Line::from(Span::styled(
                    format!("  #{acct_num:02} -- empty --"),
                    Style::default().fg(t.text_muted),
                )),
                cast: None,
                buff: None,
            });
        }
    }

    let lines = assemble_member_lines(
        entries.into_iter().take(slot_count).collect(),
        max_lines,
        width_class,
        Some(mode_line(app)),
    );

    frame.render_widget(Paragraph::new(lines), inner);
}

// ── Shared helpers ───────────────────────────────────────────────────────────

/// Choose grid rows/cols for a given number of panels.
fn grid_dims(panel_count: usize) -> (usize, usize) {
    match panel_count {
        0 | 1 => (1, 1),
        2 => (1, 2),
        3 => (1, 3),
        4 => (2, 2),
        5..=6 => (2, 3),
        7..=9 => (3, 3),
        10..=12 => (3, 4),
        _ => {
            // Dynamically calculate for >12 panels
            let cols = (panel_count as f64).sqrt().ceil() as usize;
            let rows = panel_count.div_ceil(cols);
            (rows, cols)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_dims_zero_groups() {
        assert_eq!(grid_dims(0), (1, 1));
    }

    #[test]
    fn grid_dims_one_group() {
        assert_eq!(grid_dims(1), (1, 1));
    }

    #[test]
    fn grid_dims_six_groups() {
        assert_eq!(grid_dims(6), (2, 3));
    }

    #[test]
    fn grid_dims_large_count_does_not_panic() {
        // 13+ groups use dynamic formula: cols=ceil(sqrt(n)), rows=ceil(n/cols)
        let (rows, cols) = grid_dims(13);
        assert!(rows > 0 && cols > 0);
        assert_eq!((rows, cols), (4, 4)); // ceil(sqrt(13))=4, ceil(13/4)=4

        // Even very large values should be fine.
        let (rows, cols) = grid_dims(100);
        assert!(rows > 0 && cols > 0);
        assert_eq!((rows, cols), (10, 10)); // ceil(sqrt(100))=10, ceil(100/10)=10
    }

    #[test]
    fn grid_dims_all_breakpoints() {
        assert_eq!(grid_dims(2), (1, 2));
        assert_eq!(grid_dims(3), (1, 3));
        assert_eq!(grid_dims(4), (2, 2));
        assert_eq!(grid_dims(5), (2, 3));
        assert_eq!(grid_dims(7), (3, 3));
        assert_eq!(grid_dims(8), (3, 3));
        assert_eq!(grid_dims(9), (3, 3));
        assert_eq!(grid_dims(10), (3, 4));
    }

    #[test]
    fn assemble_member_lines_keeps_idle_members_single_line() {
        let entries = vec![
            MemberRenderEntry {
                primary: Line::from("member-1"),
                cast: None,
                buff: Some(Line::from("buff-1")),
            },
            MemberRenderEntry {
                primary: Line::from("member-2"),
                cast: None,
                buff: Some(Line::from("buff-2")),
            },
            MemberRenderEntry {
                primary: Line::from("member-3"),
                cast: None,
                buff: None,
            },
        ];

        let lines = assemble_member_lines(entries, 4, WidthClass::Wide, Some(Line::from("mode")));
        let rendered: Vec<String> = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect();

        assert_eq!(rendered, vec!["member-1", "member-2", "member-3", "mode"]);
    }

    #[test]
    fn assemble_member_lines_prioritizes_cast_detail_before_buffs() {
        let entries = vec![
            MemberRenderEntry {
                primary: Line::from("member-1"),
                cast: Some(Line::from("cast-1")),
                buff: Some(Line::from("buff-1")),
            },
            MemberRenderEntry {
                primary: Line::from("member-2"),
                cast: None,
                buff: Some(Line::from("buff-2")),
            },
            MemberRenderEntry {
                primary: Line::from("member-3"),
                cast: None,
                buff: None,
            },
        ];

        let lines = assemble_member_lines(entries, 4, WidthClass::Wide, Some(Line::from("mode")));
        let rendered: Vec<String> = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect();

        assert_eq!(rendered, vec!["member-1", "cast-1", "member-2", "member-3"]);
        assert!(!rendered.iter().any(|line| line.contains("buff")));
        assert!(!rendered.iter().any(|line| line == "mode"));
    }
}
