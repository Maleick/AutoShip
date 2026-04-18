//! Character screen — operator roster, group scope, and selected character
//! detail.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Color,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, Wrap},
};

use super::widgets::{
    WIDTH_OVERVIEW_STACK, WIDTH_SHOW_CLASS_COL, WIDTH_SHOW_GROUP_COL, WIDTH_SHOW_ZONE_COL,
    WIDTH_SIDEBAR_MEDIUM, WIDTH_SIDEBAR_WIDE, WidthClass, classify_width, hp_color, panel,
    render_cast_bar, stand_state_color, themed_header_row, truncate_inline,
};
use crate::{
    eq::structs::{EqClass, StandState},
    tui::app::{ActivePanel, App, ClientState},
};
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
pub fn draw_dashboard(frame: &mut Frame, area: Rect, app: &mut App) {
    let stacked = area.width < WIDTH_OVERVIEW_STACK;
    let sections = overview_sections(app, area, stacked);

    // If map is shown and not in stacked mode, split horizontally with map on the
    // right
    if app.overview_state.show_map && !stacked && area.width >= 100 {
        let map_width = (f32::from(area.width) * 0.4).round() as u16;
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(50), Constraint::Length(map_width)])
            .split(area);

        // Draw dashboard on the left
        let sidebar_width = if cols[0].width >= WIDTH_SIDEBAR_WIDE {
            46
        } else if cols[0].width >= WIDTH_SIDEBAR_MEDIUM {
            42
        } else {
            38
        };
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(30),
                Constraint::Length(sidebar_width.min(cols[0].width.saturating_sub(24))),
            ])
            .split(cols[0]);

        draw_dashboard_grid(frame, chunks[0], app);
        if !sections.is_empty() {
            draw_dashboard_sidebar(frame, chunks[1], app, &sections);
        }

        // Draw map on the right
        crate::tui::ui::map::draw_map_view(frame, cols[1], app);
        return;
    }

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
        " Ops Roster · {} clients · sorted by Group ",
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

    // Fixed column layout matching design mock
    let headers = vec!["", "Name", "Grp", "Cls", "Lvl", "Zone", "HP", "Mana", "Cond", "State", "Activity"];
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
            let marker_style = if is_sel {
                Style::default()
                    .fg(t.text_accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text_accent)
            };
            let marker = if is_sel { "▶" } else { " " };

            if let Some(player) = &client.local_player {
                let hp_pct = player.hp_pct();
                let mana_pct = player.mana_pct();
                let name = app.redact_name(&player.displayed_name).into_owned();
                let (condition_label, condition_style) = client_condition(client, t);
                let (activity_label, activity_style) = client_activity(app, client, false);

                let mut cells = vec![
                    Cell::from(marker).style(marker_style),
                    Cell::from(name).style(if is_sel {
                        Style::default()
                            .fg(t.text_accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(t.text_bright)
                    }),
                    Cell::from(app.client_group_label(client).unwrap_or("--"))
                        .style(Style::default().fg(t.text_accent)),
                    Cell::from(player.class_str()).style(Style::default().fg(t.text_highlight)),
                    Cell::from(format!("{:>3}", player.level))
                        .style(Style::default().fg(t.text_bright)),
                    Cell::from(truncate_inline(&client.zone_name, 22))
                        .style(Style::default().fg(t.text_secondary)),
                ];

                // HP bar: "96% ███▌···"
                let hp_bar = render_hp_bar(hp_pct, t);
                cells.push(Cell::from(hp_bar).style(Style::default().fg(hp_color(hp_pct, t))));

                // Mana bar (melee classes show --)
                let is_melee = matches!(player.class, EqClass::Warrior | EqClass::Monk | EqClass::Rogue | EqClass::Berserker);
                if is_melee {
                    cells.push(Cell::from("  --  ").style(Style::default().fg(t.text_muted)));
                } else {
                    let mana_bar = render_mana_bar(mana_pct, t);
                    cells.push(Cell::from(mana_bar).style(Style::default().fg(t.text_accent)));
                }

                cells.push(Cell::from(condition_label).style(condition_style));

                let state_label = match client.stand_state {
                    StandState::Stand => "Stand",
                    StandState::Sit => "Sit",
                    StandState::Feign => "FD",
                };
                let state_style = stand_state_color(&client.stand_state, t);
                cells.push(Cell::from(state_label).style(state_style));

                cells.push(Cell::from(activity_label).style(activity_style));

                Row::new(cells).style(if is_sel {
                    highlight_style
                } else {
                    Style::default()
                })
            } else {
                let mut cells = vec![
                    Cell::from(marker).style(marker_style),
                    Cell::from(if client.client_status.is_empty() {
                        format!("PID {}", client.pid)
                    } else {
                        client.client_status.clone()
                    })
                    .style(Style::default().fg(t.hp_low)),
                    Cell::from("--").style(Style::default().fg(t.text_muted)),
                    Cell::from("--").style(Style::default().fg(t.text_muted)),
                    Cell::from("--").style(Style::default().fg(t.text_muted)),
                    Cell::from("--").style(Style::default().fg(t.text_muted)),
                    Cell::from("--").style(Style::default().fg(t.hp_low)),
                    Cell::from("--").style(Style::default().fg(t.text_muted)),
                    Cell::from("Offline").style(Style::default().fg(t.hp_low)),
                    Cell::from("—").style(Style::default().fg(t.text_muted)),
                    Cell::from("• Wait").style(Style::default().fg(t.text_muted)),
                ];

                Row::new(cells).style(if is_sel {
                    highlight_style
                } else {
                    Style::default()
                })
            }
        })
        .collect();

    // Fixed column constraints matching design widths
    let constraints = vec![
        Constraint::Length(2),   // cursor
        Constraint::Length(14),  // Name
        Constraint::Length(3),   // Grp
        Constraint::Length(4),   // Cls
        Constraint::Length(3),   // Lvl
        Constraint::Length(22),  // Zone
        Constraint::Length(16),  // HP
        Constraint::Length(12),  // Mana
        Constraint::Length(9),   // Cond
        Constraint::Length(8),   // State
        Constraint::Min(10),     // Activity
    ];

    // Build activity legend footer
    let activity_legend_lines = build_activity_legend_lines(t);

    let inner = blk.inner(roster_area);
    frame.render_widget(
        Table::new(rows, constraints)
            .header(header)
            .block(blk)
            .row_highlight_style(highlight_style),
        roster_area,
    );

    // Render activity legend at bottom
    if inner.height > 2 {
        let legend_area = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(2),
            width: inner.width,
            height: 2,
        };
        frame.render_widget(
            Paragraph::new(activity_legend_lines),
            legend_area,
        );
    }
}

fn render_hp_bar(hp_pct: f64, t: &crate::tui::theme::Theme) -> String {
    let width = 10;
    let filled = (hp_pct * width as f64 / 100.0).round() as usize;
    let mut bar = String::new();
    bar.push_str(&format!("{:>3.0}% ", hp_pct));
    for i in 0..width {
        if i < filled {
            bar.push('█');
        } else {
            bar.push('·');
        }
    }
    bar
}

fn render_mana_bar(mana_pct: f64, t: &crate::tui::theme::Theme) -> String {
    let width = 6;
    let filled = (mana_pct * width as f64 / 100.0).round() as usize;
    let mut bar = String::new();
    bar.push_str(&format!("{:>3.0}% ", mana_pct));
    for i in 0..width {
        if i < filled {
            bar.push('█');
        } else {
            bar.push('·');
        }
    }
    bar
}

fn build_activity_legend_lines(t: &crate::tui::theme::Theme) -> Vec<Line<'static>> {
    let activity_glyphs = vec![
        ("➜", "Nav", t.text_accent),
        ("✓", "Arr", t.text_success),
        ("!", "Stk", t.text_warning),
        ("☠", "Ded", t.hp_low),
        ("⇣", "FD", t.text_info),
        ("☾", "Sit", t.text_secondary),
        ("⌕", "Lot", t.text_accent),
        ("✦", "Cst", t.text_accent),
        ("⚔", "Fgt", t.text_warning),
        ("●", "Rdy", t.text_success),
    ];

    let mut legend = vec![
        Span::styled(
            "Activity glyphs:  ",
            Style::default().fg(t.text_muted),
        ),
    ];

    for (glyph, label, color) in activity_glyphs {
        legend.push(Span::styled(glyph, Style::default().fg(color)));
        legend.push(Span::raw(" "));
        legend.push(Span::styled(label, Style::default().fg(t.text_secondary)));
        legend.push(Span::raw("  "));
    }

    vec![Line::from(legend)]
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
    let title = format!(" Group Focus · {} ", app.group_focus_label());
    let blk = panel(title.as_str(), t.border_active, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    if inner.height == 0 {
        return;
    }

    // Build the stats line with separators: Uptime │ Kills/Deaths │ XP │ Plat │ TopLoot
    let mut line_spans = Vec::new();

    // Uptime
    let uptime_str = if let Some(uptime) = &app.session_uptime {
        format!("{:02}:{:02}:{:02}", uptime.hours, uptime.minutes, uptime.seconds)
    } else {
        "00:00:00".to_string()
    };
    line_spans.push(Span::styled(
        "Uptime ",
        Style::default().fg(t.text_secondary),
    ));
    line_spans.push(Span::styled(
        uptime_str,
        Style::default().fg(t.text_bright),
    ));
    line_spans.push(Span::raw(" │ "));

    // Kills / Deaths
    let kills = app.combat_stats.kills.unwrap_or(0);
    let deaths = app.combat_stats.deaths.unwrap_or(0);
    line_spans.push(Span::styled(
        "Kills ",
        Style::default().fg(t.text_secondary),
    ));
    line_spans.push(Span::styled(
        format!("{} ", kills),
        Style::default().fg(t.text_bright),
    ));
    line_spans.push(Span::styled(
        format!("/ Deaths {}", deaths),
        Style::default().fg(t.text_bright),
    ));
    line_spans.push(Span::raw(" │ "));

    // XP info
    let xp_total = app.combat_stats.xp_total.unwrap_or(0) as f64 / 1_000_000.0;
    let xp_per_hour = app.combat_stats.xp_per_hour.unwrap_or(0.0);
    let xp_per_15m = app.combat_stats.xp_per_15m.unwrap_or(0.0);
    line_spans.push(Span::styled(
        "XP ",
        Style::default().fg(t.text_secondary),
    ));
    line_spans.push(Span::styled(
        format!("{:.2}M", xp_total),
        Style::default().fg(t.text_bright),
    ));
    line_spans.push(Span::styled(
        format!(" · +{:.0}k/h · +{:.0}k/15m", xp_per_hour / 1000.0, xp_per_15m / 1000.0),
        Style::default().fg(t.text_bright),
    ));
    line_spans.push(Span::raw(" │ "));

    // Platinum
    let plat = app.platinum_balance.unwrap_or(0.0);
    let plat_per_hour = app.platinum_per_hour.unwrap_or(0.0);
    line_spans.push(Span::styled(
        "Plat ",
        Style::default().fg(t.text_secondary),
    ));
    line_spans.push(Span::styled(
        format!("{:.1}", plat),
        Style::default().fg(t.text_bright),
    ));
    line_spans.push(Span::styled(
        format!(" · +{:.1}/h", plat_per_hour),
        Style::default().fg(t.text_bright),
    ));
    line_spans.push(Span::raw(" │ "));

    // Top Loot
    line_spans.push(Span::styled(
        "Top Loot ",
        Style::default().fg(t.text_secondary),
    ));
    let loot_preview = if app.loot_database.items.is_empty() {
        "—".to_string()
    } else {
        // Get top 2-3 items by count
        let mut items: Vec<_> = app.loot_database.items.iter().collect();
        items.sort_by(|a, b| b.1.count.cmp(&a.1.count));
        items
            .iter()
            .take(2)
            .map(|(name, info)| format!("{}×{}", name, info.count))
            .collect::<Vec<_>>()
            .join(" · ")
    };
    line_spans.push(Span::styled(
        loot_preview,
        Style::default().fg(t.text_bright),
    ));

    frame.render_widget(
        Paragraph::new(Line::from(line_spans)).wrap(Wrap { trim: true }),
        inner,
    );
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
            SlotLifecycle::CampingOut => ("Camping…", t.text_highlight),
            SlotLifecycle::Exited => ("Exited", t.hp_low),
            SlotLifecycle::Relaunching => ("Relaunching", t.text_accent),
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
    Target,
    Groups,
    Combat,
    Session,
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
                draw_character_summary(frame, *chunk, app);
            }
            OverviewSectionKind::Target => {
                draw_target_cast_summary(frame, *chunk, app);
            }
            OverviewSectionKind::Groups => {
                draw_group_ops_summary(frame, *chunk, app);
            }
            OverviewSectionKind::Combat => {
                draw_combat_status(frame, *chunk, app);
            }
            OverviewSectionKind::Session => {
                draw_session_stats(frame, *chunk, app);
            }
        }
    }
}

fn overview_sections(app: &App, area: Rect, stacked: bool) -> Vec<OverviewSectionLayout> {
    // Five fixed sidebar sections: Character, Target, Groups, Session, Combat
    // Heights are designed for 44-wide sidebar with balanced visibility
    let sections = vec![
        OverviewSectionLayout {
            kind: OverviewSectionKind::Character,
            height: 7,
            collapsed: false,
        },
        OverviewSectionLayout {
            kind: OverviewSectionKind::Target,
            height: 6,
            collapsed: false,
        },
        OverviewSectionLayout {
            kind: OverviewSectionKind::Groups,
            height: 7,
            collapsed: false,
        },
        OverviewSectionLayout {
            kind: OverviewSectionKind::Session,
            height: 8,
            collapsed: false,
        },
        OverviewSectionLayout {
            kind: OverviewSectionKind::Combat,
            height: 8,
            collapsed: false,
        },
    ];

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
        OverviewSectionKind::Target => 1,
        OverviewSectionKind::Combat if app.ch_chain_status.is_some() => 2,
        OverviewSectionKind::Groups => 3,
        OverviewSectionKind::Combat => 4,
        OverviewSectionKind::Session => 5,
    };
    (priority, order)
}

fn natural_section_order(kind: OverviewSectionKind) -> u8 {
    match kind {
        OverviewSectionKind::Character => 0,
        OverviewSectionKind::Target => 1,
        OverviewSectionKind::Groups => 2,
        OverviewSectionKind::Combat => 3,
        OverviewSectionKind::Session => 4,
    }
}

fn section_title(label: &str, key_hint: Option<&str>, collapsed: bool) -> String {
    let icon = if collapsed { "▶" } else { "▼" };
    match key_hint {
        Some(key) => format!(" {label} [{key}] {icon} "),
        None => format!(" {label} {icon} "),
    }
}

fn draw_character_summary(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let border_style = if app.is_panel_focused(ActivePanel::OverviewCharacter) {
        t.border_active
    } else {
        t.border_primary
    };

    let Some(client) = app.active_client() else {
        let blk = panel(" Character · — ", border_style, t);
        let inner = blk.inner(area);
        frame.render_widget(blk, area);
        frame.render_widget(
            Paragraph::new("No character selected").style(Style::default().fg(t.text_muted)),
            inner,
        );
        return;
    };
    let Some(player) = &client.local_player else {
        let blk = panel(" Character · — ", border_style, t);
        let inner = blk.inner(area);
        frame.render_widget(blk, area);
        frame.render_widget(
            Paragraph::new("Selected client has no player data")
                .style(Style::default().fg(t.text_muted)),
            inner,
        );
        return;
    };

    let name = app.redact_name(&player.displayed_name).into_owned();
    let title = format!(" Character · {} ", name);
    let blk = panel(&title, border_style, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

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

fn draw_group_ops_summary(frame: &mut Frame, area: Rect, app: &App) {
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

fn draw_target_cast_summary(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let border_style = if app.is_panel_focused(ActivePanel::OverviewCharacter) {
        t.border_active
    } else {
        t.border_primary
    };
    let title = " Target · Cast ";
    let blk = panel(title, border_style, t);
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    let Some(client) = app.active_client() else {
        frame.render_widget(
            Paragraph::new("No character selected").style(Style::default().fg(t.text_muted)),
            inner,
        );
        return;
    };

    let mut lines = Vec::new();

    // Target info
    if let Some(target) = &client.target {
        let target_name = app.redact_name(&target.displayed_name).into_owned();
        let target_type_color = match target.npc_type_id {
            0 => t.text_success, // PC
            _ => t.text_accent,  // NPC (assume named if special ID)
        };
        lines.push(Line::from(vec![
            Span::styled("Target   ", Style::default().fg(t.text_secondary)),
            Span::styled(target_name, Style::default().fg(target_type_color).add_modifier(Modifier::BOLD)),
        ]));

        let target_hp_pct = (target.cur_hp as f64 / target.max_hp.max(1) as f64) * 100.0;
        let hp_bar_str = render_hp_bar_long(target_hp_pct);
        lines.push(Line::from(vec![
            Span::styled("Target HP", Style::default().fg(t.text_secondary)),
            Span::raw(" "),
            Span::styled(hp_bar_str, Style::default().fg(hp_color(target_hp_pct, t))),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            "Target   —",
            Style::default().fg(t.text_muted),
        )));
    }

    lines.push(Line::raw(""));

    // Casting info
    if let Some(cast_info) = &client.casting_info {
        let spell_label = &cast_info.spell_name;
        lines.push(Line::from(vec![
            Span::styled("Casting  ", Style::default().fg(t.text_secondary)),
            Span::styled(spell_label, Style::default().fg(t.text_accent).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(&cast_info.gem_slot, Style::default().fg(t.text_muted)),
        ]));

        let total_time = cast_info.total_time;
        let elapsed = cast_info.elapsed;
        let progress = if total_time > 0.0 {
            (elapsed / total_time).min(1.0)
        } else {
            0.0
        };
        let remaining = (total_time - elapsed).max(0.0);

        let bar_width = 22;
        let filled = (progress * bar_width as f64).round() as usize;
        let mut progress_bar = String::new();
        for i in 0..bar_width {
            if i < filled {
                progress_bar.push('█');
            } else {
                progress_bar.push('·');
            }
        }

        lines.push(Line::from(vec![
            Span::styled("Progress ", Style::default().fg(t.text_secondary)),
            Span::styled(progress_bar, Style::default().fg(t.text_accent)),
            Span::raw(" "),
            Span::styled(format!("{:.0}%", progress * 100.0), Style::default().fg(t.text_bright)),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Remaining", Style::default().fg(t.text_secondary)),
            Span::raw(" "),
            Span::styled(format!("{:.1}s", remaining), Style::default().fg(t.text_bright)),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            "Casting  idle",
            Style::default().fg(t.text_muted),
        )));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_hp_bar_long(hp_pct: f64) -> String {
    let width = 18;
    let filled = (hp_pct * width as f64 / 100.0).round() as usize;
    let mut bar = String::new();
    for i in 0..width {
        if i < filled {
            bar.push('█');
        } else {
            bar.push('·');
        }
    }
    bar
}

fn draw_scope_summary(frame: &mut Frame, area: Rect, app: &App) {
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
fn draw_combat_status(frame: &mut Frame, area: Rect, app: &App) {
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

fn draw_session_stats(frame: &mut Frame, area: Rect, app: &App) {
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

// ─── Kill tracker panel
// ───────────────────────────────────────────────────────

fn draw_kill_stats(frame: &mut Frame, area: Rect, app: &App, collapsed: bool) {
    let t = &app.theme;
    let border_style = if app.is_panel_focused(ActivePanel::OverviewSession) {
        t.border_active
    } else {
        t.border_primary
    };
    let title = section_title("Kill Tracker", None, collapsed);

    let elapsed = app.session_start.elapsed();
    let hours = elapsed.as_secs() as f64 / 3600.0;
    let total_kills: u32 = app.loot_database.kills.values().sum();
    let kills_per_hour = if hours > 0.01 {
        total_kills as f64 / hours
    } else {
        0.0
    };

    let top_mobs = {
        let mut mobs: Vec<(&String, &u32)> = app.loot_database.kills.iter().collect();
        mobs.sort_by(|a, b| b.1.cmp(a.1));
        mobs.truncate(5);
        mobs
    };

    let lines = if collapsed {
        vec![Line::from(vec![
            Span::styled(total_kills.to_string(), Style::default().fg(t.hp_low)),
            Span::styled(" kills", Style::default().fg(t.text_muted)),
            Span::styled(
                " | ",
                Style::default().fg(t.border_dim.fg.unwrap_or(Color::DarkGray)),
            ),
            Span::styled(
                format!("{:.1} KPH", kills_per_hour),
                Style::default().fg(t.text_accent),
            ),
        ])]
    } else {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Total  ", Style::default().fg(t.text_muted)),
                Span::styled(
                    total_kills.to_string(),
                    Style::default().fg(t.hp_low).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" kills", Style::default().fg(t.text_secondary)),
            ]),
            Line::from(vec![
                Span::styled("KPH    ", Style::default().fg(t.text_muted)),
                Span::styled(
                    format!("{:.1}", kills_per_hour),
                    Style::default().fg(t.text_accent),
                ),
                Span::styled(" /hr", Style::default().fg(t.text_secondary)),
            ]),
            Line::from(vec![
                Span::styled("Deaths ", Style::default().fg(t.text_muted)),
                Span::styled(
                    app.loot_database.deaths.to_string(),
                    Style::default().fg(if app.loot_database.deaths > 0 {
                        t.hp_low
                    } else {
                        t.text_secondary
                    }),
                ),
            ]),
        ];

        if !top_mobs.is_empty() {
            lines.push(Line::from(Span::styled(
                "── Top Mobs ──",
                Style::default().fg(t.text_muted),
            )));
            for (name, count) in &top_mobs {
                let label: String = name.chars().take(18).collect();
                let pct = if total_kills > 0 {
                    (**count as f64 / total_kills as f64 * 100.0) as u32
                } else {
                    0
                };
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        format!("{}× ", count),
                        Style::default().fg(t.text_highlight),
                    ),
                    Span::styled(label, Style::default().fg(t.text_secondary)),
                    Span::styled(format!(" ({}%)", pct), Style::default().fg(t.text_muted)),
                ]));
            }
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

// ─── Slot profile panel
// ───────────────────────────────────────────────────────

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
        SlotLifecycle::CampingOut => t.text_highlight,
        SlotLifecycle::Exited => t.hp_low,
        SlotLifecycle::Relaunching => t.text_accent,
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_healer_class ──────────────────────────────────────────────

    #[test]
    fn healer_classes_are_recognized() {
        assert!(is_healer_class(Some(EqClass::Cleric)));
        assert!(is_healer_class(Some(EqClass::Druid)));
        assert!(is_healer_class(Some(EqClass::Shaman)));
        assert!(is_healer_class(Some(EqClass::Paladin)));
    }

    #[test]
    fn non_healer_classes_rejected() {
        assert!(!is_healer_class(Some(EqClass::Warrior)));
        assert!(!is_healer_class(Some(EqClass::Wizard)));
        assert!(!is_healer_class(Some(EqClass::Rogue)));
        assert!(!is_healer_class(Some(EqClass::Enchanter)));
        assert!(!is_healer_class(Some(EqClass::Necromancer)));
        assert!(!is_healer_class(Some(EqClass::Bard)));
        assert!(!is_healer_class(Some(EqClass::Ranger)));
        assert!(!is_healer_class(Some(EqClass::Monk)));
        assert!(!is_healer_class(Some(EqClass::Magician)));
        assert!(!is_healer_class(Some(EqClass::ShadowKnight)));
    }

    #[test]
    fn healer_class_none() {
        assert!(!is_healer_class(None));
    }

    // ── is_debuffer_class ────────────────────────────────────────────

    #[test]
    fn debuffer_classes_are_recognized() {
        assert!(is_debuffer_class(Some(EqClass::Enchanter)));
        assert!(is_debuffer_class(Some(EqClass::Shaman)));
        assert!(is_debuffer_class(Some(EqClass::Necromancer)));
        assert!(is_debuffer_class(Some(EqClass::Bard)));
    }

    #[test]
    fn non_debuffer_classes_rejected() {
        assert!(!is_debuffer_class(Some(EqClass::Warrior)));
        assert!(!is_debuffer_class(Some(EqClass::Cleric)));
        assert!(!is_debuffer_class(Some(EqClass::Wizard)));
        assert!(!is_debuffer_class(Some(EqClass::Rogue)));
        assert!(!is_debuffer_class(Some(EqClass::Druid)));
        assert!(!is_debuffer_class(Some(EqClass::Paladin)));
        assert!(!is_debuffer_class(Some(EqClass::Ranger)));
        assert!(!is_debuffer_class(Some(EqClass::Monk)));
        assert!(!is_debuffer_class(Some(EqClass::Magician)));
        assert!(!is_debuffer_class(Some(EqClass::ShadowKnight)));
    }

    #[test]
    fn debuffer_class_none() {
        assert!(!is_debuffer_class(None));
    }

    // ── group_focus_strip_height ─────────────────────────────────────

    #[test]
    fn strip_height_tall_terminal() {
        let area = Rect::new(0, 0, 120, 40);
        assert_eq!(group_focus_strip_height(area), 3);
    }

    #[test]
    fn strip_height_at_threshold() {
        let area = Rect::new(0, 0, 120, MIN_HEIGHT_FOR_FOCUS_STRIP);
        assert_eq!(group_focus_strip_height(area), 3);
    }

    #[test]
    fn strip_height_below_threshold() {
        let area = Rect::new(0, 0, 120, MIN_HEIGHT_FOR_FOCUS_STRIP - 1);
        assert_eq!(group_focus_strip_height(area), 0);
    }

    #[test]
    fn strip_height_zero_height() {
        let area = Rect::new(0, 0, 120, 0);
        assert_eq!(group_focus_strip_height(area), 0);
    }

    // ── stacked_roster_min_height ────────────────────────────────────

    #[test]
    fn roster_min_height_tall() {
        assert_eq!(
            stacked_roster_min_height(STACKED_ROSTER_TALL_HEIGHT_THRESHOLD),
            STACKED_ROSTER_MIN_TALL
        );
        assert_eq!(stacked_roster_min_height(50), STACKED_ROSTER_MIN_TALL);
    }

    #[test]
    fn roster_min_height_medium() {
        assert_eq!(
            stacked_roster_min_height(STACKED_ROSTER_MEDIUM_HEIGHT_THRESHOLD),
            STACKED_ROSTER_MIN_MEDIUM
        );
        assert_eq!(
            stacked_roster_min_height(STACKED_ROSTER_TALL_HEIGHT_THRESHOLD - 1),
            STACKED_ROSTER_MIN_MEDIUM
        );
    }

    #[test]
    fn roster_min_height_compact() {
        assert_eq!(
            stacked_roster_min_height(STACKED_ROSTER_COMPACT_HEIGHT_THRESHOLD),
            STACKED_ROSTER_MIN_COMPACT
        );
        assert_eq!(
            stacked_roster_min_height(STACKED_ROSTER_MEDIUM_HEIGHT_THRESHOLD - 1),
            STACKED_ROSTER_MIN_COMPACT
        );
    }

    #[test]
    fn roster_min_height_tiny() {
        assert_eq!(
            stacked_roster_min_height(STACKED_ROSTER_COMPACT_HEIGHT_THRESHOLD - 1),
            STACKED_ROSTER_MIN_TINY
        );
        assert_eq!(stacked_roster_min_height(0), STACKED_ROSTER_MIN_TINY);
    }

    // ── section_title ────────────────────────────────────────────────

    #[test]
    fn section_title_collapsed_with_key() {
        let t = section_title("Character", Some("1"), true);
        assert!(t.contains("Character"));
        assert!(t.contains("[1]"));
        assert!(t.contains('▶'));
        assert!(!t.contains('▼'));
    }

    #[test]
    fn section_title_expanded_with_key() {
        let t = section_title("Groups", Some("2"), false);
        assert!(t.contains("Groups"));
        assert!(t.contains("[2]"));
        assert!(t.contains('▼'));
        assert!(!t.contains('▶'));
    }

    #[test]
    fn section_title_no_key_hint() {
        let t = section_title("Combat", None, true);
        assert!(t.contains("Combat"));
        assert!(t.contains('▶'));
        assert!(!t.contains('['));
    }

    // ── natural_section_order ────────────────────────────────────────

    #[test]
    fn section_order_is_deterministic() {
        assert!(
            natural_section_order(OverviewSectionKind::Character)
                < natural_section_order(OverviewSectionKind::Target)
        );
        assert!(
            natural_section_order(OverviewSectionKind::Target)
                < natural_section_order(OverviewSectionKind::Groups)
        );
        assert!(
            natural_section_order(OverviewSectionKind::Groups)
                < natural_section_order(OverviewSectionKind::Combat)
        );
        assert!(
            natural_section_order(OverviewSectionKind::Combat)
                < natural_section_order(OverviewSectionKind::Session)
        );
    }

    #[test]
    fn section_order_unique_per_kind() {
        let orders: Vec<u8> = [
            OverviewSectionKind::Character,
            OverviewSectionKind::Target,
            OverviewSectionKind::Groups,
            OverviewSectionKind::Combat,
            OverviewSectionKind::Session,
        ]
        .iter()
        .map(|k| natural_section_order(*k))
        .collect();

        let mut unique = orders.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            orders.len(),
            unique.len(),
            "each section kind should map to a unique order"
        );
    }
}
