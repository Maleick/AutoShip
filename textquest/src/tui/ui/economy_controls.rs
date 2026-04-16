//! Economy Controls panel — vendor cycle, banking status, loot queue, and
//! operator controls.
//!
//! Displays stub/demo economy state with color-coded status indicators and
//! a reference card of economy control keybindings.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::widgets::panel;
use crate::tui::app::{ActivePanel, App};

/// Color-code a vendor cycle status string.
fn vendor_cycle_color(
    status: &VendorCycleStatus,
    t: &crate::tui::theme::Theme,
) -> ratatui::style::Color {
    match status {
        VendorCycleStatus::Active => t.hp_high,
        VendorCycleStatus::Paused => t.text_highlight,
        VendorCycleStatus::Idle => t.text_muted,
        VendorCycleStatus::Aborted => t.hp_low,
    }
}

/// Color-code a banking status string.
fn banking_status_color(
    status: &BankingStatus,
    t: &crate::tui::theme::Theme,
) -> ratatui::style::Color {
    match status {
        BankingStatus::Consolidating => t.text_highlight,
        BankingStatus::Idle => t.text_muted,
        BankingStatus::Error => t.hp_low,
    }
}

/// Vendor cycle operational status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendorCycleStatus {
    /// Actively visiting vendors.
    Active,
    /// Paused by operator.
    Paused,
    /// Waiting for next scheduled run.
    Idle,
    /// Aborted due to error or operator request.
    Aborted,
}

impl VendorCycleStatus {
    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Paused => "Paused",
            Self::Idle => "Idle",
            Self::Aborted => "Aborted",
        }
    }
}

/// Banking consolidation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BankingStatus {
    /// Currently consolidating plat across characters.
    Consolidating,
    /// Standing by.
    Idle,
    /// Last operation failed.
    Error,
}

impl BankingStatus {
    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Consolidating => "Consolidating",
            Self::Idle => "Idle",
            Self::Error => "Error",
        }
    }
}

/// Draw the Economy Controls screen.
pub fn draw_economy_screen(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // ── Left: status panels ────────────────────────────────────────────
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // vendor cycle
            Constraint::Length(8),  // banking
            Constraint::Length(9),  // session tracker
            Constraint::Min(4),     // loot queue
        ])
        .split(cols[0]);

    draw_vendor_cycle_panel(frame, rows[0], app);
    draw_banking_panel(frame, rows[1], app);
    draw_session_tracker_panel(frame, rows[2], app);
    draw_loot_queue_panel(frame, rows[3], app);

    // ── Right: controls reference ──────────────────────────────────────
    draw_controls_panel(frame, cols[1], app);
}

/// Render the vendor cycle status panel.
fn draw_vendor_cycle_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let econ = &app.economy_state;

    let is_focused = app.is_panel_focused(ActivePanel::EconomyControls);
    let border_style = if is_focused {
        t.border_active
    } else {
        t.border_primary
    };
    let blk = panel(" Vendor Cycle ", border_style, t);

    let status_color = vendor_cycle_color(&econ.vendor_status, t);
    let countdown_color = if econ.vendor_next_cycle_secs < 60 {
        t.hp_low
    } else if econ.vendor_next_cycle_secs < 300 {
        t.text_highlight
    } else {
        t.hp_high
    };

    let countdown_str = format_countdown(econ.vendor_next_cycle_secs);

    let lines = vec![
        Line::from(vec![
            Span::styled("  Status:      ", Style::default().fg(t.text_muted)),
            Span::styled(
                econ.vendor_status.label(),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Next Cycle:  ", Style::default().fg(t.text_muted)),
            Span::styled(countdown_str, Style::default().fg(countdown_color)),
        ]),
        Line::from(vec![
            Span::styled("  Cycles Done: ", Style::default().fg(t.text_muted)),
            Span::styled(
                econ.vendor_cycles_completed.to_string(),
                Style::default().fg(t.text_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Last Zone:   ", Style::default().fg(t.text_muted)),
            Span::styled(
                econ.vendor_last_zone.as_deref().unwrap_or("\u{2014}"),
                Style::default().fg(t.text_normal),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Items Sold:  ", Style::default().fg(t.text_muted)),
            Span::styled(
                econ.vendor_items_sold.to_string(),
                Style::default().fg(t.text_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Plat Earned: ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{} pp", econ.vendor_plat_earned),
                Style::default().fg(t.hp_high),
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines).block(blk), area);
}

/// Render the banking status panel.
fn draw_banking_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let econ = &app.economy_state;

    let blk = panel(" Banking ", t.border_primary, t);
    let status_color = banking_status_color(&econ.banking_status, t);

    let lines = vec![
        Line::from(vec![
            Span::styled("  Status:          ", Style::default().fg(t.text_muted)),
            Span::styled(
                econ.banking_status.label(),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Consolidated pp: ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{} pp", econ.banking_consolidated_plat),
                Style::default().fg(t.hp_high),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Characters:      ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{}/{}", econ.banking_chars_done, econ.banking_chars_total),
                Style::default().fg(t.text_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Last Bank Run:   ", Style::default().fg(t.text_muted)),
            Span::styled(
                econ.banking_last_run.as_deref().unwrap_or("\u{2014}"),
                Style::default().fg(t.text_normal),
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines).block(blk), area);
}

/// Render the session plat tracker panel (MQ2PlatTracker parity).
fn draw_session_tracker_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let summary = app.economy_state.plat_tracker.summary();

    let net_plat = summary.net_change.plat;
    let net_color = if net_plat >= 0 { t.hp_high } else { t.hp_low };

    let rate = summary.plat_per_hour();
    let rate_color = if rate >= 0.0 { t.hp_high } else { t.hp_low };

    let gained = summary.total_gained.plat;
    let spent = summary.total_spent.plat.abs();

    let blk = panel(" Session Economy ", t.border_primary, t);

    let lines = vec![
        Line::from(vec![
            Span::styled("  Net:           ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{}p", net_plat),
                Style::default()
                    .fg(net_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Rate:          ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{:.1}p/h", rate),
                Style::default()
                    .fg(rate_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Gained:        ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{}p", gained),
                Style::default().fg(t.hp_high),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Spent:         ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{}p", spent),
                Style::default().fg(t.hp_low),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Duration:      ", Style::default().fg(t.text_muted)),
            Span::styled(
                summary.format_duration(),
                Style::default().fg(t.text_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Transactions:  ", Style::default().fg(t.text_muted)),
            Span::styled(
                summary.transaction_count.to_string(),
                Style::default().fg(t.text_normal),
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines).block(blk), area);
}

/// Render the loot queue panel.
fn draw_loot_queue_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let econ = &app.economy_state;

    let queue_color = if econ.loot_queue_size == 0 {
        t.text_muted
    } else if econ.loot_queue_size > 50 {
        t.hp_low
    } else if econ.loot_queue_size > 20 {
        t.text_highlight
    } else {
        t.hp_high
    };

    let blk = panel(" Loot Queue ", t.border_primary, t);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("  Queue Size:    ", Style::default().fg(t.text_muted)),
            Span::styled(
                econ.loot_queue_size.to_string(),
                Style::default()
                    .fg(queue_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Items Looted:  ", Style::default().fg(t.text_muted)),
            Span::styled(
                econ.loot_items_total.to_string(),
                Style::default().fg(t.text_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Pending:       ", Style::default().fg(t.text_muted)),
            Span::styled(
                econ.loot_pending_distribute.to_string(),
                Style::default().fg(if econ.loot_pending_distribute > 0 {
                    t.text_highlight
                } else {
                    t.text_muted
                }),
            ),
        ]),
    ];

    if !econ.loot_recent_items.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Recent Loot:",
            Style::default().fg(t.text_accent),
        )));
        for item in econ.loot_recent_items.iter().take(3) {
            lines.push(Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(item.as_str(), Style::default().fg(t.text_normal)),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(lines).block(blk), area);
}

/// Render the controls reference panel.
fn draw_controls_panel(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let t = &app.theme;
    let econ = &app.economy_state;

    let cmd_s = Style::default().fg(t.text_highlight);
    let lbl_s = Style::default().fg(t.text_secondary);
    let key_s = Style::default()
        .fg(t.text_accent)
        .add_modifier(Modifier::BOLD);

    let automation_label = if econ.automation_paused {
        "Paused"
    } else {
        "Running"
    };
    let automation_color = if econ.automation_paused {
        t.hp_low
    } else {
        t.hp_high
    };

    let mut lines = vec![
        Line::from(Span::styled(
            "Economy Operator Controls",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Automation: ", Style::default().fg(t.text_muted)),
            Span::styled(
                automation_label,
                Style::default()
                    .fg(automation_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Keybindings",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    for (key, desc) in &[
        ("P", "Pause vendor/bank cycle"),
        ("R", "Resume vendor/bank cycle"),
        ("A", "Abort current cycle"),
        ("S", "Skip current cycle"),
    ] {
        lines.push(Line::from(vec![
            Span::styled(format!("  [{key}] "), key_s),
            Span::styled(*desc, lbl_s),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Commands",
        Style::default()
            .fg(t.text_accent)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (cmd, desc) in &[
        (":econ status  ", "Show economy summary"),
        (":econ pause   ", "Pause all economy ops"),
        (":econ resume  ", "Resume economy ops"),
        (":econ vendor  ", "Trigger vendor cycle now"),
        (":econ bank    ", "Trigger bank consolidation"),
        (":econ loot    ", "Process loot queue"),
        (":econ skip    ", "Skip current cycle"),
        (":econ abort   ", "Abort current operation"),
    ] {
        lines.push(Line::from(vec![
            Span::styled(*cmd, cmd_s),
            Span::raw("  "),
            Span::styled(*desc, lbl_s),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines).block(panel(" Controls ", t.border_warn, t)),
        area,
    );
}

/// Format a countdown in seconds to a human-readable string.
fn format_countdown(secs: u64) -> String {
    if secs == 0 {
        return String::from("Now");
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m:02}m {s:02}s")
    } else if m > 0 {
        format!("{m}m {s:02}s")
    } else {
        format!("{s}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_countdown_zero() {
        assert_eq!(format_countdown(0), "Now");
    }

    #[test]
    fn format_countdown_seconds() {
        assert_eq!(format_countdown(45), "45s");
    }

    #[test]
    fn format_countdown_minutes() {
        assert_eq!(format_countdown(125), "2m 05s");
    }

    #[test]
    fn format_countdown_hours() {
        assert_eq!(format_countdown(3723), "1h 02m 03s");
    }

    #[test]
    fn vendor_status_labels() {
        assert_eq!(VendorCycleStatus::Active.label(), "Active");
        assert_eq!(VendorCycleStatus::Paused.label(), "Paused");
        assert_eq!(VendorCycleStatus::Idle.label(), "Idle");
        assert_eq!(VendorCycleStatus::Aborted.label(), "Aborted");
    }

    #[test]
    fn banking_status_labels() {
        assert_eq!(BankingStatus::Consolidating.label(), "Consolidating");
        assert_eq!(BankingStatus::Idle.label(), "Idle");
        assert_eq!(BankingStatus::Error.label(), "Error");
    }
}
