//! DPS bar widget — horizontal bar chart showing per-member DPS within a group.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::tui::dps::DpsTracker;
use crate::tui::theme::Theme;
use crate::tui::ui::widgets::panel;

/// Color palette for DPS bars — cycles through these for visual distinction.
const BAR_COLORS: [fn(&Theme) -> ratatui::style::Color; 6] = [
    |t| t.hp_high,        // green
    |t| t.text_accent,    // cyan/teal
    |t| t.text_highlight, // gold
    |t| t.mana_color,     // blue
    |t| t.spawn_named,    // yellow
    |t| t.text_server,    // purple
];

/// Format a DPS value for display (e.g. "1.2k", "45").
fn format_dps(dps: f64) -> String {
    if dps >= 1_000_000.0 {
        format!("{:.1}M", dps / 1_000_000.0)
    } else if dps >= 1_000.0 {
        format!("{:.1}k", dps / 1_000.0)
    } else {
        format!("{:.0}", dps)
    }
}

/// Draw horizontal DPS bars inside the given area.
///
/// Each row shows: `name  [████████░░░░░░] 1.2k`
///
/// Bars are proportional to the highest DPS member. If no data, renders a
/// "No DPS data" placeholder.
pub fn draw_dps_bars(frame: &mut Frame, area: Rect, tracker: &DpsTracker, theme: &Theme) {
    let block = panel("DPS", theme.border_dim, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let entries = tracker.all_dps();
    if entries.is_empty() {
        let placeholder = Paragraph::new(Line::from(Span::styled(
            "No DPS data",
            Style::default().fg(theme.text_muted),
        )));
        frame.render_widget(placeholder, inner);
        return;
    }

    let max_dps = entries.first().map(|(_, d)| *d).unwrap_or(1.0).max(1.0);

    // Each entry takes 1 row; render as many as fit
    let visible = (inner.height as usize).min(entries.len());

    for (i, (name, dps)) in entries.iter().take(visible).enumerate() {
        let y = inner.y + i as u16;
        if y >= inner.y + inner.height {
            break;
        }

        let row_area = Rect::new(inner.x, y, inner.width, 1);
        let color_fn = BAR_COLORS[i % BAR_COLORS.len()];
        let bar_color = color_fn(theme);

        // Layout: "name  " (14 chars) + bar + " dps_label"
        let name_width = 14usize;
        let dps_label = format_dps(*dps);
        let label_width = dps_label.len() + 1; // space + label
        let bar_max_width = (row_area.width as usize).saturating_sub(name_width + label_width);

        if bar_max_width == 0 {
            // Terminal too narrow for bars, just show name + DPS
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<width$}", truncate(name, name_width), width = name_width),
                    Style::default().fg(bar_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(dps_label.clone(), Style::default().fg(theme.text_secondary)),
            ]);
            frame.render_widget(Paragraph::new(line), row_area);
            continue;
        }

        let ratio = dps / max_dps;
        let filled =
            ((bar_max_width as f64 * ratio).round() as usize).max(if *dps > 0.0 { 1 } else { 0 });
        let empty = bar_max_width.saturating_sub(filled);

        let bar_filled: String = "█".repeat(filled);
        let bar_empty: String = "░".repeat(empty);

        let line = Line::from(vec![
            Span::styled(
                format!("{:<width$}", truncate(name, name_width), width = name_width),
                Style::default().fg(bar_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(bar_filled, Style::default().fg(bar_color)),
            Span::styled(bar_empty, Style::default().fg(theme.bar_empty)),
            Span::styled(
                format!(" {dps_label}"),
                Style::default()
                    .fg(theme.text_bright)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(Paragraph::new(line), row_area);
    }
}

/// Truncate a string to fit within `max_len` characters.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len.saturating_sub(1)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_dps_small() {
        assert_eq!(format_dps(0.0), "0");
        assert_eq!(format_dps(42.0), "42");
        assert_eq!(format_dps(999.0), "999");
    }

    #[test]
    fn format_dps_thousands() {
        assert_eq!(format_dps(1_000.0), "1.0k");
        assert_eq!(format_dps(1_500.0), "1.5k");
        assert_eq!(format_dps(999_999.0), "1000.0k");
    }

    #[test]
    fn format_dps_millions() {
        assert_eq!(format_dps(1_000_000.0), "1.0M");
        assert_eq!(format_dps(2_500_000.0), "2.5M");
    }

    #[test]
    fn truncate_short_string() {
        assert_eq!(truncate("Warrior", 14), "Warrior");
    }

    #[test]
    fn truncate_long_string() {
        let result = truncate("Verylongnamehere", 10);
        assert_eq!(result.chars().count(), 10);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn truncate_exact_length() {
        assert_eq!(truncate("Exact", 5), "Exact");
    }
}
