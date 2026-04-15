//! Navigation screen — per-character nav status + commands reference panel.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Row, Table},
};

use super::widgets::{panel, themed_header_row};
use crate::tui::{app::App, ui::widgets::truncate_inline};

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

/// Draw the navigation screen with waypoint list and status.
pub fn draw_navigation_screen(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
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
    let selected_pid = app.active_client().map(|client| client.pid);

    if visible.is_empty() {
        frame.render_widget(
            Paragraph::new("No characters connected")
                .block(blk)
                .style(Style::default().fg(t.text_muted)),
            cols[0],
        );
    } else {
        let header = themed_header_row(
            &["", "Character", "Zone", "Status", "ZoneFSM", "Destination"],
            t,
        );

        let rows: Vec<Row> = visible
            .iter()
            .map(|client| {
                let is_sel = Some(client.pid) == selected_pid;
                let marker = if is_sel { "▶" } else { " " };
                let name = client.local_player.as_ref().map_or_else(
                    || app.client_command_target(client),
                    |p| app.redact_name(&p.displayed_name).into_owned(),
                );

                let nav = app.nav_state.nav_statuses.get(&client.pid);
                let status = nav.map_or("Idle", |s| s.status.label());
                let dest = nav.map_or("—", |s| s.destination.as_str());

                let status_color = nav.map_or(t.text_muted, |s| nav_status_color(&s.status, t));

                // Get zone FSM state
                let zone_status = app.zone_status_state.zone_statuses.get(&client.pid);
                let zone_fsm_label = zone_status.map_or("Idle", |s| s.fsm_state.label());
                let zone_fsm_color = zone_status.map_or(t.text_muted, |s| {
                    if s.stuck {
                        t.text_highlight // yellow/gold for stuck
                    } else {
                        match s.fsm_state {
                            crate::tui::ui::zone_status_panel::ZoneFsmState::Idle => t.text_muted,
                            crate::tui::ui::zone_status_panel::ZoneFsmState::Walking => t.hp_high,
                            crate::tui::ui::zone_status_panel::ZoneFsmState::Zoning => {
                                t.text_accent
                            }
                            crate::tui::ui::zone_status_panel::ZoneFsmState::Recovering => {
                                t.text_secondary
                            }
                        }
                    }
                });

                let row_style = if is_sel {
                    Style::default()
                        .bg(t.row_selected_bg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                Row::new(vec![
                    ratatui::widgets::Cell::from(marker).style(Style::default().fg(t.text_accent)),
                    ratatui::widgets::Cell::from(name).style(Style::default().fg(t.text_normal)),
                    ratatui::widgets::Cell::from(client.zone_name.as_str())
                        .style(Style::default().fg(t.text_secondary)),
                    ratatui::widgets::Cell::from(status).style(Style::default().fg(status_color)),
                    ratatui::widgets::Cell::from(zone_fsm_label)
                        .style(Style::default().fg(zone_fsm_color)),
                    ratatui::widgets::Cell::from(dest).style(Style::default().fg(t.text_accent)),
                ])
                .style(row_style)
            })
            .collect();

        frame.render_widget(
            Table::new(
                rows,
                [
                    Constraint::Length(2),
                    Constraint::Min(14),
                    Constraint::Min(14),
                    Constraint::Length(12),
                    Constraint::Length(11),
                    Constraint::Min(14),
                ],
            )
            .header(header)
            .block(blk),
            cols[0],
        );
    }

    // ── Right: selected detail + commands reference ───────────────────
    let mode_str = format!("{}", app.operating_mode);
    let mode_color = match mode_str.as_str() {
        "Camp" => t.mode_camp,
        "Hunt" => t.mode_hunt,
        _ => t.text_normal,
    };
    let cmd_s = Style::default().fg(t.text_highlight);
    let lbl_s = Style::default().fg(t.text_secondary);
    let mesh_status = app
        .current_zone_short_name()
        .map(|zone| {
            format!(
                "{} ({})",
                if crate::nav::mesh::has_cached_zone_mesh(&zone) {
                    "cached"
                } else {
                    "on-demand"
                },
                zone
            )
        })
        .unwrap_or_else(|| String::from("—"));

    let mut detail_lines: Vec<Line<'_>> = Vec::new();

    if let Some(client) = app.active_client() {
        let client_name = client.local_player.as_ref().map_or_else(
            || app.client_command_target(client),
            |player| app.redact_name(&player.displayed_name).into_owned(),
        );
        let zone_name = client.zone_name.clone();
        let nav = app.nav_state.nav_statuses.get(&client.pid);
        let route_state = nav.map_or("Standing by", |status| status.route_state.as_str());
        let progress = nav.map_or_else(String::new, |status| status.progress_summary());
        let recovery = nav
            .and_then(|status| status.recovery_state.as_deref())
            .unwrap_or("—");
        let path_exists = nav.map(|status| status.path_exists).unwrap_or(false);
        let path_length = nav
            .and_then(|status| status.path_length)
            .map(|len| format!("{len:.0}u"))
            .unwrap_or_else(|| String::from("—"));
        let failure_reason = nav
            .and_then(|status| status.failure_reason.as_deref())
            .unwrap_or("None");
        let blockers = nav
            .and_then(|status| status.blocker_summary())
            .unwrap_or_else(|| String::from("None"));

        // Get zone FSM state for detailed info
        let zone_status = app.zone_status_state.zone_statuses.get(&client.pid);
        let zone_fsm_label = zone_status.map_or("Idle", |s| s.fsm_state.label());
        let zone_fsm_color = zone_status.map_or(t.text_muted, |s| {
            if s.stuck {
                t.text_highlight // yellow for stuck
            } else {
                match s.fsm_state {
                    crate::tui::ui::zone_status_panel::ZoneFsmState::Idle => t.text_muted,
                    crate::tui::ui::zone_status_panel::ZoneFsmState::Walking => t.hp_high,
                    crate::tui::ui::zone_status_panel::ZoneFsmState::Zoning => t.text_accent,
                    crate::tui::ui::zone_status_panel::ZoneFsmState::Recovering => t.text_secondary,
                }
            }
        });
        let zone_stuck_label = zone_status.map_or("", |s| if s.stuck { "YES" } else { "" });
        let zone_timeout_label = zone_status
            .and_then(|s| s.timeout_secs)
            .map(|secs| format!("{}s", secs))
            .unwrap_or_else(|| String::from("—"));

        detail_lines.extend([
            Line::from(vec![
                Span::styled("  Toon: ", Style::default().fg(t.text_muted)),
                Span::styled(client_name, Style::default().fg(t.text_normal)),
            ]),
            Line::from(vec![
                Span::styled("  Zone: ", Style::default().fg(t.text_muted)),
                Span::styled(zone_name, Style::default().fg(t.text_secondary)),
            ]),
            Line::from(vec![
                Span::styled("  Route: ", Style::default().fg(t.text_muted)),
                Span::styled(route_state, Style::default().fg(t.text_highlight)),
            ]),
            Line::from(vec![
                Span::styled("  Path: ", Style::default().fg(t.text_muted)),
                Span::styled(
                    if path_exists { "Yes" } else { "No" },
                    Style::default().fg(if path_exists { t.hp_high } else { t.hp_low }),
                ),
                Span::styled("  Len: ", Style::default().fg(t.text_muted)),
                Span::styled(path_length, Style::default().fg(t.text_secondary)),
            ]),
        ]);

        if !progress.is_empty() {
            detail_lines.push(Line::from(vec![
                Span::styled("  Prog: ", Style::default().fg(t.text_muted)),
                Span::styled(progress, Style::default().fg(t.text_secondary)),
            ]));
        }

        detail_lines.push(Line::from(vec![
            Span::styled("  Recovery: ", Style::default().fg(t.text_muted)),
            Span::styled(recovery, Style::default().fg(t.hp_low)),
        ]));
        detail_lines.push(Line::from(vec![
            Span::styled("  Failure: ", Style::default().fg(t.text_muted)),
            Span::styled(
                truncate_inline(failure_reason, cols[1].width.saturating_sub(14) as usize),
                Style::default().fg(if failure_reason == "None" {
                    t.hp_high
                } else {
                    t.hp_low
                }),
            ),
        ]));
        detail_lines.push(Line::from(vec![
            Span::styled("  Blockers: ", Style::default().fg(t.text_muted)),
            Span::styled(
                truncate_inline(&blockers, cols[1].width.saturating_sub(14) as usize),
                Style::default().fg(if blockers == "None" {
                    t.hp_high
                } else {
                    t.hp_low
                }),
            ),
        ]));

        // Zone transition state section
        detail_lines.push(Line::from(""));
        detail_lines.push(Line::from(Span::styled(
            "Zone Transition",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        )));
        detail_lines.push(Line::from(""));
        detail_lines.push(Line::from(vec![
            Span::styled("  FSM: ", Style::default().fg(t.text_muted)),
            Span::styled(zone_fsm_label, Style::default().fg(zone_fsm_color)),
        ]));
        if !zone_stuck_label.is_empty() {
            detail_lines.push(Line::from(vec![
                Span::styled("  Stuck: ", Style::default().fg(t.text_muted)),
                Span::styled(
                    zone_stuck_label,
                    Style::default()
                        .fg(t.text_highlight)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }
        if zone_timeout_label != "—" {
            detail_lines.push(Line::from(vec![
                Span::styled("  Retry: ", Style::default().fg(t.text_muted)),
                Span::styled(
                    zone_timeout_label,
                    Style::default().fg(
                        if zone_status.is_some_and(|s| s.timeout_secs == Some(0)) {
                            t.hp_low
                        } else {
                            t.text_secondary
                        },
                    ),
                ),
            ]));
        }
        detail_lines.push(Line::from(""));
    }

    let mut command_lines: Vec<Line<'_>> = Vec::new();

    command_lines.extend([
        Line::from(Span::styled(
            "Operating Mode",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Mode: ", Style::default().fg(t.text_muted)),
            Span::styled(
                &mode_str,
                Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Mesh: ", Style::default().fg(t.text_muted)),
            Span::styled(mesh_status, Style::default().fg(t.text_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  Focus: ", Style::default().fg(t.text_muted)),
            Span::styled("[ ] / j k", Style::default().fg(t.text_highlight)),
            Span::styled("  Enter", Style::default().fg(t.text_muted)),
            Span::styled(" back to map", Style::default().fg(t.text_secondary)),
        ]),
    ]);

    if let Some(ma) = &app.main_assist {
        command_lines.push(Line::from(vec![
            Span::styled("  MA:   ", Style::default().fg(t.text_muted)),
            Span::styled(ma.as_str(), Style::default().fg(t.text_highlight)),
        ]));
    }
    if let Some(mt) = &app.main_tank {
        command_lines.push(Line::from(vec![
            Span::styled("  MT:   ", Style::default().fg(t.text_muted)),
            Span::styled(mt.as_str(), Style::default().fg(t.hp_low)),
        ]));
    }

    // ── Group Nav summary ──────────────────────────────────────────────
    if app.has_live_group_data() {
        command_lines.push(Line::from(""));
        command_lines.push(Line::from(Span::styled(
            "Group Nav",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        )));
        command_lines.push(Line::from(""));

        let (live_groups, _) = app.build_live_groups();
        for group in &live_groups {
            let mut navigating = 0u32;
            let mut arrived = 0u32;
            let mut idle = 0u32;
            let mut dest: Option<&str> = None;
            let mut all_same_dest = true;

            for member_name in &group.member_names {
                if let Some(client) = app.find_client_by_name(member_name) {
                    if let Some(nav) = app.nav_state.nav_statuses.get(&client.pid) {
                        if nav.status.is_moving() {
                            navigating += 1;
                        } else if nav.status.is_arrived() {
                            arrived += 1;
                        } else {
                            idle += 1;
                        }
                        if !nav.destination.is_empty() && nav.destination != "\u{2014}" {
                            match dest {
                                None => dest = Some(nav.destination.as_str()),
                                Some(d) if d != nav.destination => all_same_dest = false,
                                _ => {}
                            }
                        }
                    } else {
                        idle += 1;
                    }
                }
            }

            let dest_str = if all_same_dest {
                dest.unwrap_or("\u{2014}")
            } else {
                "mixed"
            };

            let leader_display = app.redact_name(&group.leader);
            let status_color = if navigating > 0 {
                t.text_highlight
            } else if arrived > 0 {
                t.hp_high
            } else {
                t.text_muted
            };

            command_lines.push(Line::from(vec![
                Span::styled(
                    format!("  {leader_display:<12}"),
                    Style::default().fg(t.text_normal),
                ),
                Span::styled(
                    format!("{navigating}nav {arrived}arr {idle}idl"),
                    Style::default().fg(status_color),
                ),
            ]));
            if dest_str != "\u{2014}" {
                command_lines.push(Line::from(vec![
                    Span::styled("    -> ", Style::default().fg(t.text_muted)),
                    Span::styled(dest_str, Style::default().fg(t.text_accent)),
                ]));
            }
        }
    }

    command_lines.push(Line::from(""));
    command_lines.push(Line::from(Span::styled(
        "Nav Commands",
        Style::default()
            .fg(t.text_accent)
            .add_modifier(Modifier::BOLD),
    )));
    command_lines.push(Line::from(""));

    for (cmd, desc) in &[
        (":nav <dest>", "Mesh route or slash fallback"),
        (":nav ui    ", "Toggle debug diagnostics overlay"),
        (":mode camp ", "Camp mode"),
        (":mode hunt ", "Hunt mode"),
        (":camp start", "Start camp"),
        (":camp stop ", "Stop camp"),
        (":camp next ", "Next waypoint"),
        (":camp prev ", "Prev waypoint"),
        (":circle on  ", "Start circle kite"),
        (":circle off ", "Stop circle kite"),
    ] {
        command_lines.push(Line::from(vec![
            Span::styled(*cmd, cmd_s),
            Span::raw("  "),
            Span::styled(*desc, lbl_s),
        ]));
    }

    // ── Nav Debug Diagnostics overlay ─────────────────────────────────
    if app.nav_state.show_nav_debug {
        command_lines.push(Line::from(""));
        command_lines.push(Line::from(Span::styled(
            "Nav Debug Diagnostics",
            Style::default()
                .fg(t.text_highlight)
                .add_modifier(Modifier::BOLD),
        )));
        command_lines.push(Line::from(""));

        if let Some((pid, ref diag)) = app.nav_state.nav_diagnostics {
            command_lines.push(Line::from(vec![
                Span::styled("  PID:   ", Style::default().fg(t.text_muted)),
                Span::styled(pid.to_string(), Style::default().fg(t.text_secondary)),
            ]));
            command_lines.push(Line::from(vec![
                Span::styled("  State: ", Style::default().fg(t.text_muted)),
                Span::styled(diag.state.as_str(), Style::default().fg(t.text_highlight)),
            ]));
            command_lines.push(Line::from(vec![
                Span::styled("  Mesh:  ", Style::default().fg(t.text_muted)),
                Span::styled(
                    if diag.mesh_loaded { "Loaded" } else { "None" },
                    Style::default().fg(if diag.mesh_loaded {
                        t.hp_high
                    } else {
                        t.hp_low
                    }),
                ),
            ]));
            command_lines.push(Line::from(vec![
                Span::styled("  Path:  ", Style::default().fg(t.text_muted)),
                Span::styled(
                    if diag.path_exists { "Yes" } else { "No" },
                    Style::default().fg(if diag.path_exists {
                        t.hp_high
                    } else {
                        t.hp_low
                    }),
                ),
            ]));
            if let Some(len) = diag.path_length {
                command_lines.push(Line::from(vec![
                    Span::styled("  Len:   ", Style::default().fg(t.text_muted)),
                    Span::styled(format!("{len:.0}u"), Style::default().fg(t.text_secondary)),
                ]));
            }
            command_lines.push(Line::from(vec![
                Span::styled("  WP:    ", Style::default().fg(t.text_muted)),
                Span::styled(
                    format!("{}/{}", diag.waypoint_index, diag.waypoint_count),
                    Style::default().fg(t.text_secondary),
                ),
            ]));
            command_lines.push(Line::from(vec![
                Span::styled("  Dist:  ", Style::default().fg(t.text_muted)),
                Span::styled(
                    format!("{:.0}u", diag.distance_remaining),
                    Style::default().fg(t.text_secondary),
                ),
            ]));
            command_lines.push(Line::from(vec![
                Span::styled("  Vel:   ", Style::default().fg(t.text_muted)),
                Span::styled(
                    format!("{:.1} u/s", diag.velocity),
                    Style::default().fg(t.text_secondary),
                ),
            ]));
            command_lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    "(run :nav ui again to refresh)",
                    Style::default().fg(t.text_muted),
                ),
            ]));
        } else {
            command_lines.push(Line::from(Span::styled(
                "  No live diagnostics — run :nav ui while a client is focused.",
                Style::default().fg(t.text_muted),
            )));
        }
    }

    command_lines.push(Line::from(""));
    command_lines.push(Line::from(Span::styled(
        "Combat Commands",
        Style::default()
            .fg(t.text_accent)
            .add_modifier(Modifier::BOLD),
    )));
    command_lines.push(Line::from(""));

    for (cmd, desc) in &[
        (":invite <n>", "Invite to group"),
        (":accept    ", "Accept invite"),
        (":assist <n>", "Main Assist"),
        (":tank <n>  ", "Main Tank"),
        (":pull      ", "Start combat"),
        (":combat status", "Scope summary"),
        (":disengage ", "Stop combat"),
        (":ch start  ", "Start CH chain"),
        (":ch stop   ", "Stop CH chain"),
        (":ch adaptive", "on/off"),
    ] {
        command_lines.push(Line::from(vec![
            Span::styled(*cmd, cmd_s),
            Span::raw("  "),
            Span::styled(*desc, lbl_s),
        ]));
    }

    if detail_lines.is_empty() {
        frame.render_widget(
            Paragraph::new(command_lines).block(panel(" Commands & Mode ", t.border_warn, t)),
            cols[1],
        );
    } else {
        let detail_panel_height = (detail_lines.len() as u16).saturating_add(2);
        let right_sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(detail_panel_height.min(cols[1].height.saturating_sub(8))),
                Constraint::Min(8),
            ])
            .split(cols[1]);

        frame.render_widget(
            Paragraph::new(detail_lines).block(panel(" Selected Route ", t.border_primary, t)),
            right_sections[0],
        );
        frame.render_widget(
            Paragraph::new(command_lines).block(panel(" Commands & Mode ", t.border_warn, t)),
            right_sections[1],
        );
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
