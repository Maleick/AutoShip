//! Character screen — operator roster, group scope, and selected character detail.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, Wrap},
};

use super::widgets::{
    WIDTH_OVERVIEW_STACK, WIDTH_SHOW_CLASS_COL, WIDTH_SHOW_GROUP_COL, WIDTH_SHOW_ZONE_COL,
    WIDTH_SIDEBAR_MEDIUM, WIDTH_SIDEBAR_WIDE, WidthClass, classify_width, hp_color, panel,
    render_cast_bar, stand_state_color, themed_header_row, truncate_inline,
};
use crate::eq::structs::{EqClass, StandState};
use crate::tui::app::{ActivePanel, App, ClientState};
use textquest_common::types::SlotLifecycle;

const MIN_HEIGHT_FOR_FOCUS_STRIP: u16 = 28;
const STACKED_ROSTER_TALL_HEIGHT_THRESHOLD: u16 = 28;
const STACKED_ROSTER_MEDIUM_HEIGHT_THRESHOLD: u16 = 24;
const STACKED_ROSTER_COMPACT_HEIGHT_THRESHOLD: u16 = 18;
const STACKED_ROSTER_MIN_TALL: u16 = 15;
const STACKED_ROSTER_MIN_MEDIUM: u16 = 13;
const STACKED_ROSTER_MIN_COMPACT: u16 = 11;
const STACKED_ROSTER_MIN_TINY: u16 = 8;

/// Draw the main overview dashboard with roster and status panels.
pub fn draw_dashboard(frame: &mut Frame, area: Rect, app: &App) {
    let stacked = area.width < WIDTH_OVERVIEW_STACK;
    let sections = overview_sections(app, area, stacked);

    let chunks = if stacked {
        let sidebar_height = sections.iter().map(|section| section.height).sum::<u16>();
        let roster_min = stacked_roster_min_height(area.height);
        let roster_height = area.height.saturating_sub(sidebar_height).max(roster_min);
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(roster_height.max(5)),
                Constraint::Length(sidebar_height),
            ])
            .split(area)
    } else {
        let sidebar_width = if area.width >= WIDTH_SIDEBAR_WIDE {
            46
        } else if area.width >= WIDTH_SIDEBAR_MEDIUM {
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
    if !sections.is_empty() {
        draw_dashboard_sidebar(frame, chunks[1], app, &sections);
    }
}

// ─── Character grid ──────────────────────────────────────────────────────────

fn draw_dashboard_grid(frame: &mut Frame, area: Rect, app: &App) {
    let focus_height = group_focus_strip_height(area);
    let roster_area = if focus_height > 0 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(focus_height), Constraint::Min(5)])
            .split(area);
        draw_group_focus_strip(frame, chunks[0], app);
        chunks[1]
    } else {
        area
    };

    let t = &app.theme;
    let visible = app.visible_clients();
    let title = format!(
        " Ops Roster — {} ({}) ",
        app.group_focus_label(),
        visible.len()
    );

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
            roster_area,
        );
        return;
    }

    let show_group = app.active_group.is_none() && roster_area.width >= WIDTH_SHOW_GROUP_COL;
    let show_class = roster_area.width >= WIDTH_SHOW_CLASS_COL;
    let show_zone = roster_area.width >= WIDTH_SHOW_ZONE_COL;

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

    let header = themed_header_row(headers.as_slice(), t);
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
                let compact_activity =
                    roster_area.width < WIDTH_SHOW_ZONE_COL || area.width < WIDTH_OVERVIEW_STACK;
                let (activity_label, activity_style) =
                    client_activity(app, client, compact_activity);

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
                    Cell::from(format!("{hp_pct:>3.0}%"))
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
        roster_area,
    );
}

struct GroupScopeEntry {
    label: String,
    zone: String,
    connected: usize,
    members: usize,
    active: bool,
}

fn group_scope_entries(app: &App) -> Vec<GroupScopeEntry> {
    if app.has_live_group_data() {
        let (live_groups, _) = app.build_live_groups();
        return live_groups
            .iter()
            .enumerate()
            .map(|(idx, group)| GroupScopeEntry {
                label: format!(
                    "G{} {}",
                    idx + 1,
                    app.redact_name(&group.leader).into_owned()
                ),
                zone: group.zone.clone(),
                connected: group
                    .member_names
                    .iter()
                    .filter(|name| app.find_client_by_name(name).is_some())
                    .count(),
                members: group.member_names.len(),
                active: app.active_group == Some(idx),
            })
            .collect();
    }

    app.groups
        .iter()
        .enumerate()
        .map(|(idx, group)| {
            let members = app.clients_in_group_idx(idx);
            let (lo, hi) = group.account_range;
            GroupScopeEntry {
                label: format!("G{} {}", group.id, group.name),
                zone: members
                    .first()
                    .map_or_else(|| String::from("—"), |client| client.zone_name.clone()),
                connected: members.len(),
                members: usize::from(hi.saturating_sub(lo).saturating_add(1)),
                active: app.active_group == Some(idx),
            }
        })
        .collect()
}

fn draw_group_focus_strip(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let blk = panel(" Group Scope [Shift+0-6] ", t.border_dim, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    if inner.height == 0 {
        return;
    }

    let mut cards = vec![Span::styled(
        format!("[All {}]", app.clients.len()),
        if app.active_group.is_none() {
            Style::default()
                .fg(t.text_bright)
                .bg(t.row_selected_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(t.text_secondary)
        },
    )];

    for entry in group_scope_entries(app) {
        cards.push(Span::raw(" "));
        cards.push(Span::styled(
            format!("[{} {}/{}]", entry.label, entry.connected, entry.members),
            if entry.active {
                Style::default()
                    .fg(t.text_bright)
                    .bg(t.row_selected_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text_secondary)
            },
        ));
    }

    let selected = app
        .active_client()
        .and_then(|client| {
            client
                .local_player
                .as_ref()
                .map(|player| app.redact_name(&player.displayed_name).into_owned())
                .or_else(|| Some(app.client_command_target(client)))
        })
        .unwrap_or_else(|| String::from("none"));

    let compact = inner.width < 60;
    let mut lines = vec![Line::from(cards)];
    if inner.height > 1 && !compact {
        lines.push(Line::from(vec![
            Span::styled("Focus ", Style::default().fg(t.text_muted)),
            Span::styled(
                app.group_focus_label(),
                Style::default()
                    .fg(t.text_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  Selected ", Style::default().fg(t.text_muted)),
            Span::styled(selected, Style::default().fg(t.text_normal)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn group_focus_strip_height(area: Rect) -> u16 {
    if area.height >= MIN_HEIGHT_FOR_FOCUS_STRIP {
        3
    } else {
        0
    }
}

fn client_condition(client: &ClientState, t: &crate::tui::theme::Theme) -> (&'static str, Style) {
    let Some(player) = &client.local_player else {
        // Use lifecycle label when there is no in-world player data yet.
        let (label, color) = match client.slot_lifecycle {
            SlotLifecycle::Launching => ("Launching", t.text_accent),
            SlotLifecycle::WaitingForLogin => ("Login…", t.text_accent),
            SlotLifecycle::EnteringWorld => ("Zoning…", t.text_accent),
            SlotLifecycle::Recovering => ("Recovering", t.text_highlight),
            SlotLifecycle::Blocked => ("Blocked", t.hp_low),
            SlotLifecycle::Configured => ("Configured", t.text_muted),
            SlotLifecycle::Live => ("Offline", t.hp_low),
        };
        return (label, Style::default().fg(color));
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

fn client_activity(app: &App, client: &ClientState, compact: bool) -> (&'static str, Style) {
    let t = &app.theme;

    if let Some(nav) = app.nav_state.nav_statuses.get(&client.pid) {
        if nav.status.is_moving() {
            return (
                if compact { "NAV" } else { "➜ Nav" },
                Style::default()
                    .fg(t.text_highlight)
                    .add_modifier(Modifier::BOLD),
            );
        }
        if nav.status.is_arrived() {
            return (
                if compact { "ARR" } else { "✓ Arr" },
                Style::default().fg(t.hp_high),
            );
        }
        if nav.status.is_stuck() {
            return (
                if compact { "STK" } else { "! Stuck" },
                Style::default().fg(t.hp_low).add_modifier(Modifier::BOLD),
            );
        }
    }

    let Some(player) = &client.local_player else {
        return ("RDY", Style::default().fg(t.text_muted));
    };

    if matches!(player.stand_state, StandState::Dead) {
        return (
            if compact { "DEAD" } else { "☠ Dead" },
            Style::default().fg(t.hp_low),
        );
    }
    if matches!(player.stand_state, StandState::Feigned) {
        return (
            if compact { "FD" } else { "⇣ FD" },
            Style::default().fg(t.text_secondary),
        );
    }
    if matches!(player.stand_state, StandState::Sitting) {
        return (
            if compact { "SIT" } else { "☾ Sit" },
            Style::default().fg(t.state_sitting),
        );
    }
    if matches!(player.stand_state, StandState::Looting) {
        return (
            if compact { "LOOT" } else { "⌕ Loot" },
            Style::default().fg(t.text_highlight),
        );
    }

    if let Some(cast) = &player.cast_state
        && cast.is_casting()
    {
        if is_healer_class(player.class) {
            return (
                if compact { "HEAL" } else { "✚ Heal" },
                Style::default().fg(t.hp_high),
            );
        }
        return (
            if compact { "CAST" } else { "✦ Cast" },
            Style::default().fg(if is_debuffer_class(player.class) {
                t.text_accent
            } else {
                t.text_highlight
            }),
        );
    }

    if client.target.is_some() {
        return (
            if compact { "FGT" } else { "⚔ Fight" },
            Style::default()
                .fg(t.text_server)
                .add_modifier(Modifier::BOLD),
        );
    }

    match player.stand_state {
        StandState::Ducking => (
            if compact { "DUCK" } else { "↧ Duck" },
            Style::default().fg(t.text_secondary),
        ),
        StandState::Frozen => (
            if compact { "HOLD" } else { "■ Hold" },
            Style::default().fg(t.text_muted),
        ),
        _ => (
            if compact { "RDY" } else { "• Ready" },
            Style::default().fg(t.text_muted),
        ),
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum OverviewSectionKind {
    Character,
    Groups,
    Filters,
    Combat,
    Session,
    /// Launch profile, session preset, and slot lifecycle for the selected client.
    SlotProfile,
}

#[derive(Clone, Copy)]
struct OverviewSectionLayout {
    kind: OverviewSectionKind,
    height: u16,
    collapsed: bool,
}

fn draw_dashboard_sidebar(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    sections: &[OverviewSectionLayout],
) {
    if sections.is_empty() {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            sections
                .iter()
                .map(|section| Constraint::Length(section.height)),
        )
        .split(area);

    for (section, chunk) in sections.iter().zip(chunks.iter()) {
        match section.kind {
            OverviewSectionKind::Character => {
                draw_character_summary(frame, *chunk, app, section.collapsed);
            }
            OverviewSectionKind::Groups => {
                draw_group_ops_summary(frame, *chunk, app, section.collapsed);
            }
            OverviewSectionKind::Filters => {
                draw_scope_summary(frame, *chunk, app, section.collapsed);
            }
            OverviewSectionKind::Combat => {
                draw_combat_status(frame, *chunk, app, section.collapsed);
            }
            OverviewSectionKind::Session => {
                draw_session_stats(frame, *chunk, app, section.collapsed);
            }
            OverviewSectionKind::SlotProfile => {
                draw_slot_profile(frame, *chunk, app, section.collapsed);
            }
        }
    }
}

fn overview_sections(app: &App, area: Rect, stacked: bool) -> Vec<OverviewSectionLayout> {
    let mut sections = vec![OverviewSectionLayout {
        kind: OverviewSectionKind::Character,
        height: if app.overview_state.character_collapsed {
            3
        } else {
            7
        },
        collapsed: app.overview_state.character_collapsed,
    }];

    if app.overview_state.show_groups {
        let group_rows = group_scope_entries(app).len().min(6) as u16;
        sections.push(OverviewSectionLayout {
            kind: OverviewSectionKind::Groups,
            height: if app.overview_state.groups_collapsed {
                3
            } else {
                group_rows.saturating_add(2).max(5)
            },
            collapsed: app.overview_state.groups_collapsed,
        });
    }

    if app.overview_state.show_filters {
        sections.push(OverviewSectionLayout {
            kind: OverviewSectionKind::Filters,
            height: if app.overview_state.filters_collapsed {
                3
            } else {
                5
            },
            collapsed: app.overview_state.filters_collapsed,
        });
    }

    sections.push(OverviewSectionLayout {
        kind: OverviewSectionKind::Combat,
        height: if app.overview_state.combat_collapsed {
            3
        } else if app.ch_chain_status.is_some() {
            6
        } else {
            5
        },
        collapsed: app.overview_state.combat_collapsed,
    });
    sections.push(OverviewSectionLayout {
        kind: OverviewSectionKind::Session,
        height: if app.overview_state.session_collapsed {
            3
        } else if app.loot_database.items.is_empty() {
            6
        } else {
            8
        },
        collapsed: app.overview_state.session_collapsed,
    });

    if app.overview_state.show_profile {
        sections.push(OverviewSectionLayout {
            kind: OverviewSectionKind::SlotProfile,
            height: if app.overview_state.profile_collapsed {
                3
            } else {
                6
            },
            collapsed: app.overview_state.profile_collapsed,
        });
    }

    if stacked {
        stacked_overview_sections(app, area, &sections)
    } else {
        sections
    }
}

fn stacked_roster_min_height(total_height: u16) -> u16 {
    if total_height >= STACKED_ROSTER_TALL_HEIGHT_THRESHOLD {
        STACKED_ROSTER_MIN_TALL
    } else if total_height >= STACKED_ROSTER_MEDIUM_HEIGHT_THRESHOLD {
        STACKED_ROSTER_MIN_MEDIUM
    } else if total_height >= STACKED_ROSTER_COMPACT_HEIGHT_THRESHOLD {
        STACKED_ROSTER_MIN_COMPACT
    } else {
        STACKED_ROSTER_MIN_TINY
    }
}

fn stacked_overview_sections(
    app: &App,
    area: Rect,
    sections: &[OverviewSectionLayout],
) -> Vec<OverviewSectionLayout> {
    let available = area
        .height
        .saturating_sub(stacked_roster_min_height(area.height));
    if available < 3 {
        return Vec::new();
    }

    let mut indexed: Vec<(usize, OverviewSectionLayout)> =
        sections.iter().copied().enumerate().collect();
    indexed.sort_by_key(|(order, section)| stacked_priority(app, section.kind, *order));

    let mut selected = Vec::new();
    let mut used = 0;
    for (_, section) in indexed {
        if used + 3 > available {
            continue;
        }
        used += 3;
        selected.push(OverviewSectionLayout {
            collapsed: true,
            height: 3,
            ..section
        });
    }

    let mut extra = available.saturating_sub(used);
    for section in &mut selected {
        if let Some(base) = sections
            .iter()
            .find(|candidate| candidate.kind == section.kind)
        {
            let desired = base.height.saturating_sub(3);
            if desired > 0 {
                let grant = desired.min(extra);
                section.height += grant;
                if grant == desired {
                    section.collapsed = base.collapsed;
                }
                extra = extra.saturating_sub(grant);
            }
        }
    }

    selected.sort_by_key(|section| natural_section_order(section.kind));
    selected
}

fn stacked_priority(app: &App, kind: OverviewSectionKind, order: usize) -> (u8, usize) {
    let priority = match kind {
        OverviewSectionKind::Character => 0,
        OverviewSectionKind::Combat if app.ch_chain_status.is_some() => 1,
        OverviewSectionKind::SlotProfile => 2,
        OverviewSectionKind::Groups => 3,
        OverviewSectionKind::Filters => 4,
        OverviewSectionKind::Combat => 5,
        OverviewSectionKind::Session => 6,
    };
    (priority, order)
}

fn natural_section_order(kind: OverviewSectionKind) -> u8 {
    match kind {
        OverviewSectionKind::Character => 0,
        OverviewSectionKind::SlotProfile => 1,
        OverviewSectionKind::Groups => 2,
        OverviewSectionKind::Filters => 3,
        OverviewSectionKind::Combat => 4,
        OverviewSectionKind::Session => 5,
    }
}

fn section_title(label: &str, key_hint: Option<&str>, collapsed: bool) -> String {
    let icon = if collapsed { "▶" } else { "▼" };
    match key_hint {
        Some(key) => format!(" {label} [{key}] {icon} "),
        None => format!(" {label} {icon} "),
    }
}

fn draw_character_summary(frame: &mut Frame, area: Rect, app: &App, collapsed: bool) {
    let t = &app.theme;
    let border_style = if app.is_panel_focused(ActivePanel::OverviewCharacter) {
        t.border_active
    } else {
        t.border_primary
    };
    let title = section_title("Character", Some("Enter"), collapsed);
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
    let width_class = classify_width(inner.width);
    let compact_activity = width_class != WidthClass::Wide;
    let (activity_label, activity_style) = client_activity(app, client, compact_activity);
    let mode_str = format!("{}", app.operating_mode);
    let mode_style = match mode_str.as_str() {
        "Camp" => Style::default()
            .fg(t.mode_camp)
            .add_modifier(Modifier::BOLD),
        "Hunt" => Style::default()
            .fg(t.mode_hunt)
            .add_modifier(Modifier::BOLD),
        _ => Style::default().fg(t.text_muted),
    };
    let target_name = client.target.as_ref().map_or_else(
        || String::from("—"),
        |target| app.redact_name(&target.displayed_name).into_owned(),
    );
    let (nav_label, nav_style, nav_destination, nav_route_state, nav_blocker_summary) =
        app.nav_state.nav_statuses.get(&client.pid).map_or_else(
            || {
                (
                    String::from("Idle"),
                    Style::default().fg(t.text_muted),
                    String::from("—"),
                    String::new(),
                    None,
                )
            },
            |nav| {
                let style = if nav.status.is_moving() {
                    Style::default().fg(t.text_highlight)
                } else if nav.status.is_arrived() {
                    Style::default().fg(t.hp_high)
                } else if nav.status.is_stuck() {
                    Style::default().fg(t.hp_low).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(t.text_muted)
                };
                let destination = if nav.destination.is_empty() {
                    String::from("—")
                } else {
                    nav.destination.clone()
                };
                (
                    nav.status.label().to_string(),
                    style,
                    destination,
                    nav.route_state.clone(),
                    nav.blocker_summary(),
                )
            },
        );

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
        let mut lines = vec![
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
                    truncate_inline(
                        client.zone_name.as_str(),
                        inner.width.saturating_sub(26) as usize,
                    ),
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
                Span::styled("  Act ", Style::default().fg(t.text_muted)),
                Span::styled(activity_label, activity_style),
            ]),
            Line::from(vec![
                Span::styled("Focus ", Style::default().fg(t.text_muted)),
                Span::styled(app.group_focus_label(), Style::default().fg(t.text_accent)),
                Span::styled("  Mode ", Style::default().fg(t.text_muted)),
                Span::styled(mode_str, mode_style),
                if width_class == WidthClass::Wide {
                    Span::styled("  Tgt ", Style::default().fg(t.text_muted))
                } else {
                    Span::styled("  Nav ", Style::default().fg(t.text_muted))
                },
                if width_class == WidthClass::Wide {
                    Span::styled(
                        truncate_inline(&target_name, 14),
                        Style::default().fg(t.text_highlight),
                    )
                } else {
                    Span::styled(truncate_inline(&nav_label, 10), nav_style)
                },
            ]),
        ];

        if let Some(cast_display) = app.client_cast_display(client) {
            lines.push(render_cast_bar(
                &cast_display,
                inner.width as usize,
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
            ));
        } else if width_class == WidthClass::Wide || nav_label != "Idle" {
            lines.push(Line::from(vec![
                Span::styled(
                    if nav_label != "Idle" {
                        "To   "
                    } else {
                        "Pos  "
                    },
                    Style::default().fg(t.text_muted),
                ),
                if nav_label != "Idle" {
                    Span::styled(
                        truncate_inline(&nav_destination, inner.width.saturating_sub(6) as usize),
                        Style::default().fg(t.text_secondary),
                    )
                } else {
                    Span::styled(
                        format!("y:{:.0} x:{:.0} z:{:.0}", player.y, player.x, player.z),
                        Style::default().fg(t.text_secondary),
                    )
                },
            ]));
        }

        if !nav_route_state.is_empty() || nav_blocker_summary.is_some() {
            lines.push(Line::from(vec![
                Span::styled("Route ", Style::default().fg(t.text_muted)),
                Span::styled(
                    truncate_inline(&nav_route_state, 20),
                    Style::default().fg(t.text_highlight),
                ),
                Span::styled("  Hold ", Style::default().fg(t.text_muted)),
                Span::styled(
                    truncate_inline(
                        nav_blocker_summary.as_deref().unwrap_or("clear"),
                        inner.width.saturating_sub(33) as usize,
                    ),
                    Style::default().fg(if nav_blocker_summary.is_some() {
                        t.hp_low
                    } else {
                        t.hp_high
                    }),
                ),
            ]));
        }

        lines
    };

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn draw_group_ops_summary(frame: &mut Frame, area: Rect, app: &App, collapsed: bool) {
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

    let entries = group_scope_entries(app);
    if collapsed {
        let summary = format!(
            "{} | {} chars",
            app.group_focus_label(),
            app.focused_pid_count()
        );
        frame.render_widget(
            Paragraph::new(summary).style(Style::default().fg(t.text_muted)),
            inner,
        );
        return;
    }

    if entries.is_empty() || inner.height == 0 {
        frame.render_widget(
            Paragraph::new("No group data").style(Style::default().fg(t.text_muted)),
            inner,
        );
        return;
    }

    let label_budget = inner.width.saturating_sub(16).clamp(8, 18) as usize;
    let zone_budget = inner.width.saturating_sub(24).clamp(8, 20) as usize;
    let lines: Vec<Line<'_>> = entries
        .into_iter()
        .take(inner.height as usize)
        .map(|entry| {
            let count_label = format!("{}/{} up", entry.connected, entry.members.max(1));
            let marker_style = if entry.active {
                Style::default()
                    .fg(t.text_accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text_muted)
            };
            let label_style = if entry.active {
                Style::default()
                    .fg(t.text_bright)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text_normal)
            };
            let count_style = if entry.connected > 0 {
                Style::default().fg(t.hp_high)
            } else {
                Style::default().fg(t.text_muted)
            };

            Line::from(vec![
                Span::styled(if entry.active { "▶ " } else { "  " }, marker_style),
                Span::styled(truncate_inline(&entry.label, label_budget), label_style),
                Span::styled(" ", Style::default()),
                Span::styled(count_label, count_style),
                Span::styled("  ", Style::default()),
                Span::styled(
                    truncate_inline(&entry.zone, zone_budget),
                    Style::default().fg(t.text_secondary),
                ),
            ])
        })
        .collect();

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn draw_scope_summary(frame: &mut Frame, area: Rect, app: &App, collapsed: bool) {
    let t = &app.theme;
    let border_style = if app.is_panel_focused(ActivePanel::OverviewFilters) {
        t.border_active
    } else {
        t.border_dim
    };
    let title = section_title("Scope", Some("v"), collapsed);
    let blk = panel(title.as_str(), border_style, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let focus = app.group_focus_label();
    let selected_cmd = if app.active_client().is_some() {
        String::from("Selected /cmd")
    } else {
        String::from("Select a character")
    };
    let mode_str = format!("{}", app.operating_mode);
    let mode_color = match mode_str.as_str() {
        "Camp" => t.mode_camp,
        "Hunt" => t.mode_hunt,
        _ => t.text_muted,
    };
    let focused_count = app.focused_pid_count();

    let lines = if collapsed {
        vec![Line::from(vec![
            Span::styled(
                focus,
                Style::default()
                    .fg(t.text_accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" | {mode_str} | {focused_count} chars"),
                t.text_muted,
            ),
        ])]
    } else {
        vec![
            Line::from(vec![
                Span::styled("Focus ", Style::default().fg(t.text_muted)),
                Span::styled(
                    focus,
                    Style::default()
                        .fg(t.text_accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  Mode ", Style::default().fg(t.text_muted)),
                Span::styled(&mode_str, Style::default().fg(mode_color)),
            ]),
            Line::from(vec![
                Span::styled("Char  ", Style::default().fg(t.text_muted)),
                Span::styled(selected_cmd, Style::default().fg(t.text_highlight)),
                Span::styled("  Count ", Style::default().fg(t.text_muted)),
                Span::styled(
                    focused_count.to_string(),
                    Style::default().fg(t.text_normal),
                ),
            ]),
            Line::from(vec![
                Span::styled("Ops   ", Style::default().fg(t.text_muted)),
                Span::styled("all /cmd", Style::default().fg(t.text_highlight)),
                Span::styled("  nav <zone>", Style::default().fg(t.text_accent)),
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
    let scope_str = app.group_focus_label();
    let focused_count = app.focused_pid_count();

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
            Span::styled("  |  Scope ", Style::default().fg(t.text_muted)),
            Span::styled(scope_str, Style::default().fg(t.text_accent)),
            Span::styled("  |  F ", Style::default().fg(t.text_muted)),
            Span::styled(
                focused_count.to_string(),
                Style::default().fg(t.text_normal),
            ),
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
            Line::from(vec![
                Span::styled("Scope ", Style::default().fg(t.text_muted)),
                Span::styled(scope_str, Style::default().fg(t.text_accent)),
                Span::styled("  Focused ", Style::default().fg(t.text_muted)),
                Span::styled(
                    focused_count.to_string(),
                    Style::default().fg(t.text_normal),
                ),
            ]),
            Line::from(vec![
                Span::styled("Ops   ", Style::default().fg(t.text_muted)),
                Span::styled(":assist", Style::default().fg(t.text_highlight)),
                Span::styled(" / :pull", Style::default().fg(t.text_highlight)),
                Span::styled(" / :combat status", Style::default().fg(t.text_accent)),
            ]),
        ];

        if let Some(ch) = &app.ch_chain_status {
            let adaptive_str = if ch.is_adaptive { "adaptive" } else { "fixed" };
            lines.push(Line::from(vec![
                Span::styled("CH   ", Style::default().fg(t.text_muted)),
                Span::styled(
                    format!(
                        "{}x {:.1}s {} tgt={}",
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
            format!("{rate:.0}/hr")
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
                format!("{total_plat:.0}"),
                Style::default().fg(t.text_highlight),
            ),
        ])]
    } else {
        let mut lines: Vec<Line<'_>> = vec![
            Line::from(vec![
                Span::styled("Time ", Style::default().fg(t.text_accent)),
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
                    format!("{total_plat:.0} ({plat_per_hour}/hr)"),
                    Style::default().fg(t.text_highlight),
                ),
            ]),
            Line::from(vec![
                Span::styled("Kill ", Style::default().fg(t.text_muted)),
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
                    Span::styled(format!("{count}× "), Style::default().fg(t.text_highlight)),
                    Span::styled(label, Style::default().fg(t.text_secondary)),
                ]));
            }
        }

        if area.height >= 8 {
            lines.push(Line::from(vec![
                Span::styled("Server ", Style::default().fg(t.text_muted)),
                Span::styled(app.display_server(), Style::default().fg(t.text_server)),
                Span::styled("  Theme ", Style::default().fg(t.text_muted)),
                Span::styled(app.theme_kind.label(), Style::default().fg(t.text_accent)),
            ]));
        }

        lines
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(title.as_str(), border_style, t))
            .wrap(Wrap { trim: true }),
        area,
    );
}

// ─── Slot profile panel ───────────────────────────────────────────────────────

/// Return the theme colour for a given `SlotLifecycle` state.
fn lifecycle_color(state: SlotLifecycle, t: &crate::tui::theme::Theme) -> ratatui::style::Color {
    match state {
        SlotLifecycle::Live => t.hp_high,
        SlotLifecycle::Recovering => t.text_highlight,
        SlotLifecycle::Blocked => t.hp_low,
        SlotLifecycle::Launching
        | SlotLifecycle::WaitingForLogin
        | SlotLifecycle::EnteringWorld => t.text_accent,
        SlotLifecycle::Configured => t.text_muted,
    }
}

/// Sidebar panel: launch profile, session preset, and slot lifecycle.
fn draw_slot_profile(frame: &mut Frame, area: Rect, app: &App, collapsed: bool) {
    let t = &app.theme;
    let border_style = t.border_primary;
    let title = section_title("Slot Profile", None, collapsed);

    let Some(client) = app.active_client() else {
        frame.render_widget(
            Paragraph::new("No client selected")
                .block(panel(title.as_str(), border_style, t))
                .style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
    };

    let lifecycle = client.slot_lifecycle;
    let lifecycle_style = Style::default()
        .fg(lifecycle_color(lifecycle, t))
        .add_modifier(if lifecycle.is_degraded() {
            Modifier::BOLD
        } else {
            Modifier::empty()
        });

    let lines = if collapsed {
        let profile_label = client.launch_profile.as_deref().unwrap_or("—");
        vec![Line::from(vec![
            Span::styled(lifecycle.label(), lifecycle_style),
            Span::styled("  ", Style::default()),
            Span::styled(
                truncate_inline(profile_label, 20),
                Style::default().fg(t.text_secondary),
            ),
        ])]
    } else {
        let profile_label = client.launch_profile.as_deref().unwrap_or("—");
        let preset_label = client.session_preset.as_deref().unwrap_or("—");
        vec![
            Line::from(vec![
                Span::styled("State  ", Style::default().fg(t.text_muted)),
                Span::styled(lifecycle.description(), lifecycle_style),
            ]),
            Line::from(vec![
                Span::styled("Profile", Style::default().fg(t.text_muted)),
                Span::styled(" ", Style::default()),
                Span::styled(
                    truncate_inline(profile_label, area.width.saturating_sub(9) as usize),
                    Style::default().fg(t.text_secondary),
                ),
            ]),
            Line::from(vec![
                Span::styled("Preset ", Style::default().fg(t.text_muted)),
                Span::styled(" ", Style::default()),
                Span::styled(
                    truncate_inline(preset_label, area.width.saturating_sub(9) as usize),
                    Style::default().fg(t.text_accent),
                ),
            ]),
        ]
    };

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel(title.as_str(), border_style, t))
            .wrap(Wrap { trim: true }),
        area,
    );
}
