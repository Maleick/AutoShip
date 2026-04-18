//! Spawn events panel — live feed of spawn/despawn events (MQ2Paranoid/MQ2Spawns parity).

use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Cell, Paragraph, Row, Table, Wrap},
};

use crate::eq::spawn_alert::{MatchSource, SpawnAlertEvent};
use crate::tui::app::App;

pub fn draw_spawn_events_panel(frame: &mut Frame, area: Rect, app: &mut App) {
    let t = &app.theme;
    let is_active = app.active_panel == crate::tui::app::ActivePanel::SpawnEvents;
    let border_style = if is_active {
        t.border_active
    } else {
        t.border_dim
    };

    let filter_label = match app.player_notification_filter {
        crate::config::PlayerFilterMode::All => "all",
        crate::config::PlayerFilterMode::StrangersOnly => "strangers",
        crate::config::PlayerFilterMode::FriendsOnly => "friends",
    };

    let sound_ind = if app.sound_on_player_zone_in {
        " [sound]"
    } else {
        ""
    };

    let title = format!(
        " Spawn Events [pf:{} sound:{}{}] ",
        filter_label,
        if app.sound_on_player_zone_in {
            "ON"
        } else {
            "OFF"
        },
        sound_ind
    );

    let blk = crate::tui::ui::widgets::panel(title.as_str(), border_style, t);

    let events = app.spawn_alert_feed.events();
    let feed_len = events.len();

    if feed_len == 0 {
        frame.render_widget(
            Paragraph::new("No spawn events yet. Use :watch <pattern> to add patterns.")
                .block(blk)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(t.text_muted)),
            area,
        );
        return;
    }

    let header = crate::tui::ui::widgets::themed_header_row(&["Time", "Zone", "Name", "Event"], t);

    let visible_rows = (area.height.saturating_sub(2) as usize).min(feed_len);

    let rows: Vec<Row> = events
        .iter()
        .rev()
        .take(visible_rows)
        .map(|event| {
            let timestamp = format_time(event);
            let zone = truncate(&event.zone, 12);
            let name = truncate(&event.spawn_name, 20);
            let event_label = if event.is_up { "UP" } else { "DOWN" };

            let _source_style = match &event.match_source {
                MatchSource::Named => Style::default().fg(Color::Yellow),
                MatchSource::WatchPattern(_) => Style::default().fg(t.text_accent),
            };

            let event_style = if event.is_up {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red)
            };

            let time_style = Style::default().fg(t.text_muted);
            let zone_style = Style::default().fg(t.text_secondary);

            Row::new(vec![
                Cell::from(Span::styled(timestamp, time_style)),
                Cell::from(Span::styled(zone, zone_style)),
                Cell::from(Span::styled(name, t.text_bright)),
                Cell::from(Span::styled(event_label, event_style)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(14),
            Constraint::Min(20),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(blk)
    .row_highlight_style(Style::default().bg(t.row_selected_bg));

    frame.render_widget(table, area);
}

fn format_time(event: &SpawnAlertEvent) -> String {
    use std::time::SystemTime;
    match event.timestamp.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => {
            let secs = duration.as_secs();
            let hours = (secs / 3600) % 24;
            let mins = (secs / 60) % 60;
            let secs = secs % 60;
            format!("{:02}:{:02}:{:02}", hours, mins, secs)
        }
        Err(_) => String::from("--:--:--"),
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if max_len <= 2 {
        let mut chars = s.chars();
        for _ in 0..max_len {
            if chars.next().is_none() {
                return s.to_string();
            }
        }
        if chars.next().is_some() {
            ".".repeat(max_len)
        } else {
            s.to_string()
        }
    } else {
        let keep = max_len - 2;
        let mut iter = s.char_indices();
        let mut cutoff = s.len();

        for _ in 0..keep {
            match iter.next() {
                Some((idx, ch)) => {
                    cutoff = idx + ch.len_utf8();
                }
                None => return s.to_string(),
            }
        }

        if iter.next().is_some() {
            format!("{}..", &s[..cutoff])
        } else {
            s.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncate_keeps_short_ascii_strings() {
        assert_eq!(truncate("West Freeport", 20), "West Freeport");
    }

    #[test]
    fn truncate_limits_ascii_strings() {
        assert_eq!(truncate("abcdefghijklmnop", 8), "abcdef..");
    }

    #[test]
    fn truncate_handles_multibyte_utf8_without_panicking() {
        assert_eq!(truncate("ééééé", 4), "éé..");
    }

    #[test]
    fn truncate_handles_max_len_zero() {
        assert_eq!(truncate("", 0), "");
        assert_eq!(truncate("a", 0), "");
        assert_eq!(truncate("West Freeport", 0), "");
    }

    #[test]
    fn truncate_handles_max_len_one() {
        assert_eq!(truncate("", 1), "");
        assert_eq!(truncate("a", 1), "a");
        assert_eq!(truncate("ab", 1), ".");
        assert_eq!(truncate("West Freeport", 1), ".");
    }

    #[test]
    fn truncate_handles_max_len_two() {
        assert_eq!(truncate("a", 2), "a");
        assert_eq!(truncate("ab", 2), "ab");
        assert_eq!(truncate("abc", 2), "..");
        assert_eq!(truncate("West Freeport", 2), "..");
    }
}
