//! Real-time metrics dashboard panel — displays DPS, movement speed, loot value/hour.
//!
//! Metrics update in real-time from combat/movement/loot events. Panel is
//! resizable, movable, and supports reset via command.

use std::time::{Duration, Instant};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::tui::{theme::Theme, ui::widgets::panel};

/// Time window for metrics aggregation (rolling window).
const METRICS_WINDOW: Duration = Duration::from_secs(60);

/// Tracks real-time metrics from combat, movement, and loot events.
#[derive(Debug, Clone)]
pub struct MetricsTracker {
    /// DPS samples with timestamps (damage, time).
    dps_samples: Vec<(f64, Instant)>,
    /// Movement events with speed and timestamp.
    movement_samples: Vec<(f64, Instant)>,
    /// Loot events with value and timestamp.
    loot_samples: Vec<(f64, Instant)>,
    /// Last time metrics were reset.
    last_reset: Instant,
}

impl MetricsTracker {
    /// Create a new metrics tracker.
    pub fn new() -> Self {
        Self {
            dps_samples: Vec::new(),
            movement_samples: Vec::new(),
            loot_samples: Vec::new(),
            last_reset: Instant::now(),
        }
    }

    /// Record a damage event (in DPS units).
    pub fn record_damage(&mut self, damage: f64) {
        self.dps_samples.push((damage, Instant::now()));
    }

    /// Record a movement event (speed in units/sec).
    pub fn record_movement(&mut self, speed: f64) {
        self.movement_samples.push((speed, Instant::now()));
    }

    /// Record a loot event (value in platinum or normalized units).
    pub fn record_loot(&mut self, value: f64) {
        self.loot_samples.push((value, Instant::now()));
    }

    /// Reset all metrics and restart the window.
    pub fn reset(&mut self) {
        self.dps_samples.clear();
        self.movement_samples.clear();
        self.loot_samples.clear();
        self.last_reset = Instant::now();
    }

    /// Prune samples older than the metrics window.
    fn prune_old(&mut self) {
        let cutoff = Instant::now() - METRICS_WINDOW;
        self.dps_samples.retain(|(_, t)| *t > cutoff);
        self.movement_samples.retain(|(_, t)| *t > cutoff);
        self.loot_samples.retain(|(_, t)| *t > cutoff);
    }

    /// Calculate current DPS (damage per second over the window).
    pub fn current_dps(&mut self) -> f64 {
        self.prune_old();
        if self.dps_samples.is_empty() {
            return 0.0;
        }
        let total_damage: f64 = self.dps_samples.iter().map(|(d, _)| d).sum();
        total_damage / METRICS_WINDOW.as_secs_f64()
    }

    /// Calculate average movement speed.
    pub fn avg_speed(&mut self) -> f64 {
        self.prune_old();
        if self.movement_samples.is_empty() {
            return 0.0;
        }
        let total: f64 = self.movement_samples.iter().map(|(s, _)| s).sum();
        total / self.movement_samples.len() as f64
    }

    /// Calculate loot value per hour.
    pub fn loot_per_hour(&mut self) -> f64 {
        self.prune_old();
        if self.loot_samples.is_empty() {
            return 0.0;
        }
        let total_loot: f64 = self.loot_samples.iter().map(|(v, _)| v).sum();
        // Normalize to hourly rate based on window duration
        let hours = METRICS_WINDOW.as_secs_f64() / 3600.0;
        if hours > 0.0 { total_loot / hours } else { 0.0 }
    }

    /// Time elapsed since last reset (formatted as mm:ss).
    pub fn time_since_reset(&self) -> String {
        let elapsed = self.last_reset.elapsed();
        let secs = elapsed.as_secs();
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }
}

impl Default for MetricsTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a number with K/M suffix (e.g. "1.2k", "3.5M").
fn format_metric(value: f64) -> String {
    if value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{:.0}", value)
    }
}

/// Render the metrics dashboard panel.
///
/// Displays:
/// - DPS (damage per second)
/// - Movement Speed (avg units/sec)
/// - Loot Value/Hour (normalized to hourly rate)
/// - Time elapsed since reset
pub fn draw_metrics_panel(
    frame: &mut Frame,
    area: Rect,
    tracker: &mut MetricsTracker,
    theme: &Theme,
) {
    let block = panel(" Metrics ", theme.border_dim, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split area into rows: DPS, Movement, Loot, Time
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let dps = tracker.current_dps();
    let speed = tracker.avg_speed();
    let loot = tracker.loot_per_hour();
    let time_str = tracker.time_since_reset();

    // DPS row
    let dps_line = Line::from(vec![
        Span::styled("DPS: ", Style::default().fg(theme.text_muted)),
        Span::styled(
            format_metric(dps),
            Style::default()
                .fg(theme.hp_high)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let dps_para = Paragraph::new(dps_line);
    frame.render_widget(dps_para, rows[0]);

    // Movement row
    let speed_line = Line::from(vec![
        Span::styled("Spd: ", Style::default().fg(theme.text_muted)),
        Span::styled(
            format!("{:.1}", speed),
            Style::default()
                .fg(theme.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" u/s"),
    ]);
    let speed_para = Paragraph::new(speed_line);
    frame.render_widget(speed_para, rows[1]);

    // Loot row
    let loot_line = Line::from(vec![
        Span::styled("Loot/hr: ", Style::default().fg(theme.text_muted)),
        Span::styled(
            format_metric(loot),
            Style::default()
                .fg(theme.text_highlight)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let loot_para = Paragraph::new(loot_line);
    frame.render_widget(loot_para, rows[2]);

    // Time row
    let time_line = Line::from(vec![
        Span::styled("Time: ", Style::default().fg(theme.text_muted)),
        Span::styled(time_str, Style::default().fg(theme.mana_color)),
    ]);
    let time_para = Paragraph::new(time_line);
    frame.render_widget(time_para, rows[3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_tracker_init() {
        let tracker = MetricsTracker::new();
        assert!(tracker.dps_samples.is_empty());
        assert!(tracker.movement_samples.is_empty());
        assert!(tracker.loot_samples.is_empty());
    }

    #[test]
    fn test_record_damage() {
        let mut tracker = MetricsTracker::new();
        tracker.record_damage(100.0);
        tracker.record_damage(200.0);
        assert_eq!(tracker.dps_samples.len(), 2);
    }

    #[test]
    fn test_reset() {
        let mut tracker = MetricsTracker::new();
        tracker.record_damage(100.0);
        tracker.record_movement(10.0);
        tracker.record_loot(500.0);
        tracker.reset();
        assert!(tracker.dps_samples.is_empty());
        assert!(tracker.movement_samples.is_empty());
        assert!(tracker.loot_samples.is_empty());
    }

    #[test]
    fn test_format_metric() {
        assert_eq!(format_metric(500.0), "500");
        assert_eq!(format_metric(1_200.0), "1.2k");
        assert_eq!(format_metric(3_500_000.0), "3.5M");
    }
}
