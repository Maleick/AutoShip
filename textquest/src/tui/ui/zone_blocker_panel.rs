//! Zone Blocker Panel — detailed display of navigation and zone obstacles.
//!
//! Shows per-client blocker reasons that explain why movement is delayed, stuck,
//! or blocked by walls/geometry/NPCs. Real-time updates as status changes.

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::items_after_statements,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::needless_pass_by_value,
    clippy::no_effect_underscore_binding,
    clippy::option_if_let_else,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unreadable_literal,
    clippy::unused_self,
    clippy::used_underscore_binding,
    clippy::trivially_copy_pass_by_ref,
    clippy::ref_option,
    clippy::or_fun_call,
    clippy::needless_pass_by_ref_mut,
    clippy::wildcard_imports,
    clippy::redundant_field_names
)]

use ratatui::{
    Frame,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
};

use super::widgets::panel;
use crate::tui::app::App;

/// Draw the zone blocker panel inside `area`.
///
/// Shows a detailed list of current blockers per client explaining why movement
/// is paused, stuck, or blocked. Updates in real-time as navigation status changes.
pub fn draw_zone_blocker_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;

    let blk = panel(" ZONE BLOCKERS ", t.border_primary, t);

    let visible = app.visible_clients();
    if visible.is_empty() {
        frame.render_widget(
            Paragraph::new("No characters connected")
                .block(blk)
                .style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
    }

    let selected_pid = app.active_client().map(|c| c.pid);

    // Collect all blockers from all visible clients
    let mut blocker_items: Vec<ListItem> = Vec::new();

    for client in visible.iter() {
        // Get navigation blockers
        if let Some(nav) = app.nav_state.nav_statuses.get(&client.pid) {
            if !nav.blockers.is_empty() {
                // Add client header if there are blockers
                let client_name = client.local_player.as_ref().map_or_else(
                    || app.client_command_target(client),
                    |p| app.redact_name(&p.displayed_name).into_owned(),
                );
                let is_selected = Some(client.pid) == selected_pid;
                let marker = if is_selected { "▶" } else { " " };

                let header_style = if is_selected {
                    Style::default()
                        .fg(t.text_accent)
                        .add_modifier(Modifier::BOLD)
                        .bg(t.row_selected_bg)
                } else {
                    Style::default().fg(t.text_accent)
                };

                blocker_items.push(ListItem::new(Line::from(vec![Span::styled(
                    format!("{} {}: ", marker, client_name),
                    header_style,
                )])));

                // Add each blocker with indentation
                for blocker in &nav.blockers {
                    let blocker_style = if is_selected {
                        Style::default().fg(t.hp_low).bg(t.row_selected_bg)
                    } else {
                        Style::default().fg(t.hp_low)
                    };

                    blocker_items.push(ListItem::new(Line::from(vec![
                        Span::styled("  • ", Style::default().fg(t.text_highlight)),
                        Span::styled(blocker.as_str(), blocker_style),
                    ])));
                }
            }
        }

        // Get zone transition stuck status as a blocker
        if let Some(zone_status) = app.zone_status_state.zone_statuses.get(&client.pid) {
            if zone_status.stuck {
                let client_name = client.local_player.as_ref().map_or_else(
                    || app.client_command_target(client),
                    |p| app.redact_name(&p.displayed_name).into_owned(),
                );
                let is_selected = Some(client.pid) == selected_pid;
                let marker = if is_selected { "▶" } else { " " };

                // Only add header if we haven't already added it above
                let has_nav_blockers = app
                    .nav_state
                    .nav_statuses
                    .get(&client.pid)
                    .map(|n| !n.blockers.is_empty())
                    .unwrap_or(false);

                if !has_nav_blockers {
                    let header_style = if is_selected {
                        Style::default()
                            .fg(t.text_accent)
                            .add_modifier(Modifier::BOLD)
                            .bg(t.row_selected_bg)
                    } else {
                        Style::default().fg(t.text_accent)
                    };

                    blocker_items.push(ListItem::new(Line::from(vec![Span::styled(
                        format!("{} {}: ", marker, client_name),
                        header_style,
                    )])));
                }

                let blocker_style = if is_selected {
                    Style::default().fg(t.text_highlight).bg(t.row_selected_bg)
                } else {
                    Style::default().fg(t.text_highlight)
                };

                blocker_items.push(ListItem::new(Line::from(vec![
                    Span::styled("  • ", Style::default().fg(t.hp_low)),
                    Span::styled(
                        "Zone transition stuck (recovery attempt may be in progress)",
                        blocker_style,
                    ),
                ])));
            }
        }
    }

    if blocker_items.is_empty() {
        frame.render_widget(
            Paragraph::new("No active blockers")
                .block(blk)
                .style(Style::default().fg(t.text_muted)),
            area,
        );
    } else {
        let list = List::new(blocker_items).block(blk);
        frame.render_widget(list, area);
    }
}

/// Draw a compact blocker count indicator suitable for status bars.
///
/// Shows the count of clients with active blockers.
pub fn count_active_blockers(app: &App) -> usize {
    let mut count = 0;

    for client in app.visible_clients().iter() {
        // Count clients with navigation blockers
        if let Some(nav) = app.nav_state.nav_statuses.get(&client.pid) {
            if !nav.blockers.is_empty() {
                count += 1;
                continue;
            }
        }

        // Count clients with zone stuck status
        if let Some(zone_status) = app.zone_status_state.zone_statuses.get(&client.pid) {
            if zone_status.stuck {
                count += 1;
            }
        }
    }

    count
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "placeholder only: requires a minimal App fixture to meaningfully assert count_active_blockers behavior"]
    fn test_count_active_blockers_none() {
        // Intentionally ignored: this was previously a no-op placeholder.
        // Replace with a real unit test once a minimal `App` test fixture exists,
        // or rely on integration tests that exercise this path end-to-end.
    }
}
