//! Navigation screen — nav blockers panel + per-client route cards.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use super::widgets::panel;
use crate::tui::{app::App, theme::Theme};

fn client_class_abbr(client: &crate::tui::client::ClientState) -> &'static str {
    client
        .local_player
        .as_ref()
        .and_then(|p| p.class)
        .map(|c| c.short_name())
        .unwrap_or("?")
}

fn nav_status_color(
    status: &textquest_common::nav::NavStatus,
    t: &crate::tui::theme::Theme,
) -> ratatui::style::Color {
    if status.is_moving() {
        t.text_highlight
    } else if status.is_paused() {
        t.text_secondary
    } else if status.is_arrived() {
        t.hp_high
    } else if status.is_stuck() {
        t.hp_low
    } else {
        t.text_muted
    }
}

/// Draw a progress bar string with filled (█) and empty (·) characters.
fn draw_progress_bar(progress: f64, width: usize, color: ratatui::style::Color) -> Span<'static> {
    let filled = ((progress.clamp(0.0, 1.0)) * width as f64) as usize;
    let empty = width.saturating_sub(filled);
    let bar = format!("{}{}", "█".repeat(filled), "·".repeat(empty));
    Span::styled(bar, Style::default().fg(color))
}

/// Draw the blocker summary panel (full-width, top).
fn draw_blocker_panel(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    // Check if there are active blockers - if so, show blocker panel instead of commands panel
    let has_active_blockers = super::zone_blocker_panel::count_active_blockers(app) > 0;

    // Adaptive: narrow terminals get more space for nav status
    let (left_pct, right_pct) = if area.width < 100 { (65, 35) } else { (60, 40) };
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(left_pct),
            Constraint::Percentage(right_pct),
        ])
        .split(area);

    // ── Left: per-character nav status ────────────────────────────────
    let blk = panel(
        " Navigation Status ",
        if app.is_panel_focused(crate::tui::app::ActivePanel::TacticalNavigation) {
            t.border_active
        } else {
            t.border_primary
        },
        t,
    );
    let visible = app.visible_clients();

    // Count blockers and stuck clients
    let mut blocker_count = 0;
    let mut stuck_count = 0;
    let mut nominal_count = 0;
    let mut blockers_detail: Vec<(String, u32)> = Vec::new();

    for client in &visible {
        if let Some(nav) = app.nav_state.nav_statuses.get(&client.pid) {
            if nav.status.is_stuck() {
                stuck_count += 1;
                blocker_count += 1;
                let name = client.local_player.as_ref().map_or_else(
                    || app.client_command_target(client),
                    |p| app.redact_name(&p.displayed_name).into_owned(),
                );
                blockers_detail.push((name, client.pid));
            } else {
                nominal_count += 1;
            }
        } else {
            nominal_count += 1;
        }
    }

    let mut lines: Vec<Line> = Vec::new();

    // Count line with colored status indicators
    let count_line = vec![
        Span::styled(
            format!("{} blocker", blocker_count),
            Style::default().fg(t.hp_low).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" · "),
        Span::styled(
            format!("{} stuck", stuck_count),
            Style::default().fg(t.text_accent),
        ),
        Span::raw(" · "),
        Span::styled(
            format!("{} routing nominal", nominal_count),
            Style::default().fg(t.hp_high),
        ),
    ];
    lines.push(Line::from(count_line));

    if blocker_count > 0 {
        lines.push(Line::from(""));

        for (name, pid) in &blockers_detail {
            if let Some(nav) = app.nav_state.nav_statuses.get(pid) {
                let slot = app
                    .visible_clients()
                    .iter()
                    .position(|c| c.pid == *pid)
                    .map(|i| format!("S{:02}", i + 1))
                    .unwrap_or_else(|| "—".to_string());

                let zone = app
                    .visible_clients()
                    .iter()
                    .find(|c| c.pid == *pid)
                    .map(|c| c.zone_name.as_str())
                    .unwrap_or("?");

                // Header line with warning marker
                lines.push(Line::from(vec![
                    Span::styled("⚠ ", Style::default().fg(t.hp_low)),
                    Span::styled(name.clone(), Style::default().fg(t.text_bright)),
                    Span::raw("  "),
                    Span::styled(format!("slot {}", slot), Style::default().fg(t.text_muted)),
                    Span::raw(" · "),
                    Span::styled(zone.to_string(), Style::default().fg(t.text_muted)),
                ]));

                // Detail lines
                let blocker_desc = nav
                    .blockers
                    .first()
                    .cloned()
                    .or_else(|| nav.blocker_description.clone());
                if let Some(desc) = blocker_desc {
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::styled("blocker   ", Style::default().fg(t.text_secondary)),
                        Span::styled(desc, Style::default().fg(t.text_bright)),
                    ]));
                }

                let retry_label = format!("{}/5", nav.retry_count);
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled("retries   ", Style::default().fg(t.text_secondary)),
                    Span::styled(
                        retry_label,
                        Style::default().fg(if nav.retry_count >= 4 {
                            t.hp_low
                        } else {
                            t.text_accent
                        }),
                    ),
                ]));

                let fallback_label = nav.fallback_route.as_deref().unwrap_or("none");
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled("fallback  ", Style::default().fg(t.text_secondary)),
                    Span::styled(
                        fallback_label.to_string(),
                        Style::default().fg(if nav.fallback_route.is_some() {
                            t.text_highlight
                        } else {
                            t.text_muted
                        }),
                    ),
                ]));

                lines.push(Line::from(vec![Span::styled(
                    "    resolution options: :nav unstick · :nav reroute · :nav force_tp",
                    Style::default().fg(t.text_muted),
                )]));
            }
        }
    }

    let blk = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border_type)
        .title(" NAV BLOCKERS ")
        .border_style(Style::default().fg(t.hp_low));

    frame.render_widget(Paragraph::new(lines).block(blk), area);
}

/// Draw a single client navigation card.
fn draw_nav_card(
    client: &crate::tui::client::ClientState,
    nav_status: Option<&crate::tui::app::NavClientStatus>,
    t: &Theme,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();

    if let Some(nav) = nav_status {
        // Status color based on nav state
        let status_color = if nav.status.is_stuck() {
            t.hp_low
        } else if nav.status.is_paused() {
            t.text_accent
        } else if nav.status.is_moving() {
            t.text_highlight
        } else {
            t.text_muted
        };

        let status_label = nav.status.label();
        let eta = if nav.status.is_moving() {
            "12.4s"
        } else if nav.status.is_paused() {
            "at anchor"
        } else {
            "—"
        };

        // Class, level, group
        let class_abbr = client_class_abbr(client);
        lines.push(Line::from(vec![
            Span::styled("Class    ", Style::default().fg(t.text_secondary)),
            Span::styled(
                format!("{}", class_abbr),
                Style::default().fg(t.text_highlight),
            ),
            Span::raw("  "),
            Span::styled(
                format!(
                    "L{}",
                    client.local_player.as_ref().map(|p| p.level).unwrap_or(0)
                ),
                Style::default().fg(t.text_muted),
            ),
            Span::raw("   "),
            Span::styled("Group ", Style::default().fg(t.text_secondary)),
            Span::styled(
                client
                    .group_info
                    .as_ref()
                    .map(|g| g.member_count.to_string())
                    .unwrap_or_else(|| "—".to_string()),
                Style::default().fg(t.text_highlight),
            ),
        ]));

        // Zone
        lines.push(Line::from(vec![
            Span::styled("Zone     ", Style::default().fg(t.text_secondary)),
            Span::styled(client.zone_name.clone(), Style::default().fg(t.text_bright)),
        ]));

        // Position
        let x = if let Some(p) = &client.local_player {
            p.x
        } else {
            0.0
        };
        let y = if let Some(p) = &client.local_player {
            p.y
        } else {
            0.0
        };
        let z = if let Some(p) = &client.local_player {
            p.z
        } else {
            0.0
        };
        let h = if let Some(p) = &client.local_player {
            p.heading
        } else {
            0.0
        };

        lines.push(Line::from(vec![
            Span::styled("Position ", Style::default().fg(t.text_secondary)),
            Span::styled(
                format!("{:.1}, {:.1}, {:.1}", y, x, z),
                Style::default().fg(t.text_bright),
            ),
            Span::raw("  "),
            Span::styled(format!("h{}", h), Style::default().fg(t.text_muted)),
        ]));

        lines.push(Line::from(""));

        // Status and ETA
        lines.push(Line::from(vec![
            Span::styled("Status   ", Style::default().fg(t.text_secondary)),
            Span::styled(
                status_label,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled("ETA ", Style::default().fg(t.text_secondary)),
            Span::styled(eta, Style::default().fg(t.text_bright)),
        ]));

        // Destination
        lines.push(Line::from(vec![
            Span::styled("Destination", Style::default().fg(t.text_secondary)),
            Span::styled(
                nav.destination.clone(),
                Style::default().fg(t.text_highlight),
            ),
        ]));

        // Route state
        lines.push(Line::from(vec![
            Span::styled("Route    ", Style::default().fg(t.text_secondary)),
            Span::styled(
                nav.route_state.clone(),
                Style::default().fg(t.text_secondary),
            ),
        ]));

        // Progress bar or blocker info
        if nav.status.is_stuck() {
            lines.push(Line::from(""));
            if !nav.blockers.is_empty() {
                let blocker = &nav.blockers[0];
                lines.push(Line::from(vec![
                    Span::styled(
                        "⚠ Blocker: ",
                        Style::default().fg(t.hp_low).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(blocker.clone(), Style::default().fg(t.text_bright)),
                ]));
                lines.push(Line::from(vec![Span::styled(
                    "  retries 3/5 · fallback route queued · :nav unstick",
                    Style::default().fg(t.text_muted),
                )]));
            }
        } else {
            lines.push(Line::from(""));
            let progress = if nav.status.is_paused() {
                1.0
            } else if nav.progress_pct > 0.0 {
                nav.progress_pct as f64 / 100.0
            } else {
                0.0
            };
            let percent = format!("{:.0}%", progress * 100.0);
            lines.push(Line::from(vec![
                Span::styled("Progress ", Style::default().fg(t.text_secondary)),
                draw_progress_bar(progress, 28, t.text_highlight),
                Span::raw(" "),
                Span::styled(percent.clone(), Style::default().fg(t.text_bright)),
            ]));
        }
    }

    lines
}

/// Build the zone graph panel content lines for display.
///
/// Renders up to `max_nodes` zones sorted by zone ID, showing:
/// - Each zone as `[zone_name]` (highlighted in yellow when on the active path,
///   bracketed with `>` when it is the current zone).
/// - Outgoing connections as `  -> dest_name` (highlighted when the edge is on
///   the active path).
///
/// This is a text-mode graph — no canvas or Unicode box-drawing required, so
/// it works on any terminal.
fn build_zone_graph_lines<'a>(
    state: &crate::tui::app::ZoneGraphPanelState,
    t: &'a Theme,
    max_nodes: usize,
) -> Vec<Line<'a>> {
    let Some(graph) = &state.zone_graph else {
        return vec![Line::from(Span::styled(
            "  Zone graph unavailable — connect a client to load.",
            Style::default().fg(t.text_muted),
        ))];
    };

    if graph.zones.is_empty() {
        return vec![Line::from(Span::styled(
            "  No zones in graph.",
            Style::default().fg(t.text_muted),
        ))];
    }

    // Build active-path set for O(1) lookup.
    let path_set: std::collections::HashSet<u16> = state
        .active_path
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .copied()
        .collect();

    // Build active-edge set: pairs (a, b) where both consecutive in path.
    let path_edges: std::collections::HashSet<(u16, u16)> = state
        .active_path
        .as_deref()
        .unwrap_or(&[])
        .windows(2)
        .map(|w| (w[0], w[1]))
        .collect();

    // Sort zones by ID for stable display.
    let mut zone_ids: Vec<u16> = graph.zones.keys().copied().collect();
    zone_ids.sort_unstable();

    let mut lines: Vec<Line> = Vec::new();

    // Header: show path summary if active.
    if let Some(path) = &state.active_path {
        let names: Vec<&str> = path
            .iter()
            .filter_map(|id| graph.zones.get(id).map(|z| z.name.as_str()))
            .collect();
        let path_str = names.join(" → ");
        lines.push(Line::from(vec![
            Span::styled("Active path: ", Style::default().fg(t.text_secondary)),
            Span::styled(path_str, Style::default().fg(t.text_highlight).add_modifier(Modifier::BOLD)),
        ]));
        lines.push(Line::from(""));
    }

    let shown = zone_ids.len().min(max_nodes);
    let truncated = zone_ids.len() > max_nodes;

    for &zone_id in &zone_ids[..shown] {
        let Some(node) = graph.zones.get(&zone_id) else {
            continue;
        };

        let is_current = state.current_zone_id == Some(zone_id);
        let on_path = path_set.contains(&zone_id);

        // Node label: >[name]< for current zone, [name] otherwise.
        let node_label = if is_current {
            format!(">[{}]<", node.name)
        } else {
            format!("[{}]", node.name)
        };

        let node_color = if is_current {
            t.text_accent
        } else if on_path {
            t.text_highlight
        } else {
            t.text_bright
        };

        lines.push(Line::from(Span::styled(
            node_label,
            Style::default().fg(node_color).add_modifier(if on_path {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
        )));

        // Edges: only show up to 4 connections per node to keep density manageable.
        let conn_limit = 4;
        let connections = &node.connections;
        let shown_conns = connections.len().min(conn_limit);
        for conn in &connections[..shown_conns] {
            if conn.disabled {
                continue;
            }
            let dest_name = graph
                .zones
                .get(&conn.dest_zone_id)
                .map_or_else(|| conn.dest_zone_id.to_string(), |z| z.name.clone());

            let edge_on_path =
                path_edges.contains(&(zone_id, conn.dest_zone_id));

            let edge_color = if edge_on_path {
                t.text_highlight
            } else {
                t.text_muted
            };

            lines.push(Line::from(vec![
                Span::styled("  -> ", Style::default().fg(edge_color)),
                Span::styled(
                    dest_name,
                    Style::default().fg(edge_color).add_modifier(if edge_on_path {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
                ),
            ]));
        }
        if connections.len() > conn_limit {
            lines.push(Line::from(Span::styled(
                format!("  ... +{} more", connections.len() - conn_limit),
                Style::default().fg(t.text_muted),
            )));
        }
    }

    if truncated {
        lines.push(Line::from(Span::styled(
            format!(
                "  ... showing {shown}/{} zones — scroll with :nav graph for full view",
                zone_ids.len()
            ),
            Style::default().fg(t.text_muted),
        )));
    }

    lines
}

/// Draw the zone graph panel.
pub fn draw_zone_graph_panel(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let focused = app.is_panel_focused(crate::tui::app::ActivePanel::ZoneGraph);

    let border_color = if focused {
        t.border_active
    } else {
        t.border_primary
    };

    let blk = panel(" Zone Graph ", border_color, t);

    // Reserve space inside the block for text.
    let inner = blk.inner(area);
    frame.render_widget(blk, area);

    // Approximate how many lines fit — subtract 1 for potential path header.
    let max_nodes = (inner.height as usize).saturating_sub(3).max(1);

    let lines = build_zone_graph_lines(&app.nav_state.zone_graph_panel, t, max_nodes);
    frame.render_widget(Paragraph::new(lines), inner);
}

/// Draw the navigation screen with nav blockers panel + zone graph + per-client card grid.
pub fn draw_navigation_screen(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let visible = app.visible_clients();

    if visible.is_empty() {
        let blk = Block::default()
            .borders(Borders::ALL)
            .border_type(t.border_type)
            .title(" NAVIGATION ")
            .border_style(t.border_primary);
        frame.render_widget(
            Paragraph::new("No characters connected")
                .block(blk)
                .style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
    }

    // Split vertically: [blocker panel] | [zone graph] | [client cards]
    let blocker_height = 7u16;
    let zone_graph_height = 14u16; // ~10 zones visible
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(blocker_height),
            Constraint::Length(zone_graph_height),
            Constraint::Min(8),
        ])
        .split(area);

    // Draw blocker/status panel (top)
    draw_blocker_panel(frame, sections[0], app);

    // Draw zone graph panel (middle)
    draw_zone_graph_panel(frame, sections[1], app);

    // Draw 2-column grid of client cards (bottom)
    let card_width = (sections[2].width.saturating_sub(1)) / 2;

    // Arrange cards in 2-column layout
    for (idx, client) in visible.iter().enumerate() {
        let nav = app.nav_state.nav_statuses.get(&client.pid);
        let lines = draw_nav_card(client, nav, t);

        let border_color = if let Some(nav) = nav {
            if nav.status.is_stuck() {
                t.hp_low
            } else {
                t.text_accent
            }
        } else {
            t.text_accent
        };

        let client_name = client.local_player.as_ref().map_or_else(
            || app.client_command_target(client),
            |p| app.redact_name(&p.displayed_name).into_owned(),
        );
        let class_abbr = client_class_abbr(client);
        let title = format!("{} · {}", client_name, class_abbr);

        let blk = Block::default()
            .borders(Borders::ALL)
            .border_type(t.border_type)
            .title(title)
            .border_style(Style::default().fg(border_color));

        // Calculate position for this card
        let card_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(card_width),
                Constraint::Length(1), // gap
                Constraint::Length(card_width),
            ])
            .split(sections[2]);

        // Determine which column this card goes in
        let col_idx = idx % 2;
        let card_area = if col_idx == 0 {
            card_cols[0]
        } else {
            card_cols[2]
        };

        // Only render if there's space
        if card_area.height > 0 && card_area.width > 0 {
            frame.render_widget(Paragraph::new(lines).block(blk), card_area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::nav::{NavStatus, PauseReason};

    fn test_theme() -> crate::tui::theme::Theme {
        crate::tui::theme::dark_modern()
    }

    #[test]
    fn nav_color_moving() {
        let t = test_theme();
        let status = NavStatus::Moving {
            waypoint_index: 1,
            waypoint_count: 5,
            distance_remaining: 100.0,
        };
        assert_eq!(nav_status_color(&status, &t), t.text_highlight);
    }

    #[test]
    fn nav_color_paused() {
        let t = test_theme();
        let status = NavStatus::Paused {
            reason: PauseReason::UserPause,
            waypoint_index: 0,
            waypoint_count: 3,
            distance_remaining: 50.0,
        };
        assert_eq!(nav_status_color(&status, &t), t.text_secondary);
    }

    #[test]
    fn nav_color_arrived() {
        let t = test_theme();
        let status = NavStatus::Arrived;
        assert_eq!(nav_status_color(&status, &t), t.hp_high);
    }

    #[test]
    fn nav_color_stuck() {
        let t = test_theme();
        let status = NavStatus::Stuck {
            recovery_attempt: 1,
        };
        assert_eq!(nav_status_color(&status, &t), t.hp_low);
    }

    #[test]
    fn nav_color_idle_falls_through() {
        let t = test_theme();
        let status = NavStatus::Idle;
        assert_eq!(nav_status_color(&status, &t), t.text_muted);
    }

    #[test]
    fn nav_color_following_falls_through() {
        let t = test_theme();
        let status = NavStatus::Following {
            leader_name: "Testchar".to_string(),
            distance_to_anchor: 10.0,
            returning: false,
        };
        // Following is not moving/paused/arrived/stuck, so falls to text_muted
        assert_eq!(nav_status_color(&status, &t), t.text_muted);
    }
}
