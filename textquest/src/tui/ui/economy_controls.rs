//! Economy Controls panel — vendor cycle, banking status, loot queue, and
//! operator controls.
//!
//! Displays stub/demo economy state with color-coded status indicators and
//! a reference card of economy control keybindings.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
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
pub fn draw_economy_screen(frame: &mut Frame, area: Rect, app: &App) {
    // Main layout: main area (~60%) + sidebar (44 cols)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(101), Constraint::Length(44)])
        .split(area);

    // ── Main area: Vendor/Bank + Roster ────────────────────────────────
    let main_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(15), Constraint::Min(8)])
        .split(cols[0]);

    draw_vendor_bank_panel(frame, main_rows[0], app);
    draw_roster_panel(frame, main_rows[1], app);

    // ── Sidebar: Rules + Ledger ────────────────────────────────────────
    let sidebar_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(8)])
        .split(cols[1]);

    draw_rules_panel(frame, sidebar_rows[0], app);
    draw_ledger_panel(frame, sidebar_rows[1], app);
}

fn draw_vendor_bank_panel(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let econ = &app.economy_state;

    let is_focused = app.is_panel_focused(ActivePanel::EconomyControls);
    let border_style = if is_focused {
        t.border_active
    } else {
        Style::default().fg(t.text_server) // magenta border
    };
    let blk = panel(" Vendor / Bank Cycle ", border_style, t);

    let status_color = vendor_cycle_color(&econ.vendor_status, t);
    let countdown_color = if econ.vendor_next_cycle_secs < 60 {
        t.hp_low
    } else if econ.vendor_next_cycle_secs < 300 {
        t.text_highlight
    } else {
        t.hp_high
    };
    let countdown_str = format_countdown(econ.vendor_next_cycle_secs);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Cycle State: ", Style::default().fg(t.text_muted)),
            Span::styled(
                econ.vendor_status.label(),
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" · "),
            Span::styled("—", Style::default().fg(t.text_muted)), // stage placeholder
        ]),
        Line::from(vec![
            Span::styled("Current Slot: ", Style::default().fg(t.text_muted)),
            Span::styled("—", Style::default().fg(t.text_normal)),
        ]),
        Line::from(vec![
            Span::styled("Stage Started: ", Style::default().fg(t.text_muted)),
            Span::styled("—", Style::default().fg(t.text_normal)),
        ]),
        Line::from(vec![
            Span::styled("Cycle Started: ", Style::default().fg(t.text_muted)),
            Span::styled("—", Style::default().fg(t.text_normal)),
        ]),
        Line::from(vec![
            Span::styled("Cycles Today: ", Style::default().fg(t.text_muted)),
            Span::styled(
                econ.vendor_cycles_completed.to_string(),
                Style::default().fg(t.text_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("Next Cycle In: ", Style::default().fg(t.text_muted)),
            Span::styled(countdown_str, Style::default().fg(countdown_color)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Vendor: ", Style::default().fg(t.text_muted)),
            Span::styled(
                econ.vendor_last_zone.as_deref().unwrap_or("—"),
                Style::default().fg(t.text_normal),
            ),
        ]),
        Line::from(vec![
            Span::styled("Bank: ", Style::default().fg(t.text_muted)),
            Span::styled("—", Style::default().fg(t.text_normal)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Plat (pocket): ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{} pp", econ.vendor_plat_earned),
                Style::default().fg(t.hp_high),
            ),
        ]),
        Line::from(vec![
            Span::styled("Plat (banked): ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{} pp", econ.banking_consolidated_plat),
                Style::default().fg(t.hp_high),
            ),
        ]),
    ];

    // Footer with control hints
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "s",
            Style::default()
                .fg(t.text_highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" start · "),
        Span::styled(
            "S",
            Style::default()
                .fg(t.text_highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" stop · "),
        Span::styled(
            "x",
            Style::default()
                .fg(t.text_highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" skip client · "),
        Span::styled(
            "r",
            Style::default()
                .fg(t.text_highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" reload rules"),
    ]));

    frame.render_widget(Paragraph::new(lines).block(blk), area);
}

/// Render the roster table with client status.
fn draw_roster_panel(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    let blk = panel(" Roster ", Style::default().fg(t.text_server), t); // magenta border

    // Column headers
    let header_cells = vec![
        Cell::from("Slot").style(
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Status").style(
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Plat").style(
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Bags").style(
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Reason/Notes").style(
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let header = Row::new(header_cells)
        .style(Style::default())
        .bottom_margin(1);

    // Build rows from app.clients
    let rows: Vec<Row> = app
        .clients
        .iter()
        .enumerate()
        .map(|(idx, client)| {
            let slot_color = t.text_normal;
            let slot = format!("{}", idx + 1);
            let status = if client.connected { "Online" } else { "Offline" };

            // Status coloring: Active=amber/inverse, Done=green, Queued=cyan, Skipped=muted
            let status_color = if client.connected {
                t.text_highlight // amber for connected
            } else {
                t.text_muted // muted for disconnected
            };

            let cells = vec![
                Cell::from(slot).style(Style::default().fg(slot_color)),
                Cell::from(status).style(Style::default().fg(status_color)),
                Cell::from("0").style(Style::default().fg(t.text_normal)),
                Cell::from("0").style(Style::default().fg(t.text_normal)),
                Cell::from("—").style(Style::default().fg(t.text_muted)),
            ];

            Row::new(cells)
        })
        .collect();

    let constraints = vec![
        Constraint::Length(16), // Slot
        Constraint::Length(10), // Status
        Constraint::Length(6),  // Plat (right-aligned)
        Constraint::Length(6),  // Bags
        Constraint::Min(34),    // Reason/Notes
    ];

    let inner = blk.inner(area);
    frame.render_widget(
        Table::new(rows, constraints).header(header).block(blk),
        area,
    );
}

/// Render the Rules panel (cyan border, sidebar).
fn draw_rules_panel(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    let border_style = Style::default().fg(t.text_accent); // cyan
    let blk = panel(" Rules ", border_style, t);

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "active rule set default.ron",
                Style::default().fg(t.text_normal),
            ),
            Span::raw(" "),
            Span::styled("(0 rules)", Style::default().fg(t.text_muted)),
        ]),
        Line::from(""),
    ];

    // Placeholder: show "No rules loaded"
    lines.push(Line::from(Span::styled(
        "No rules loaded",
        Style::default().fg(t.text_muted),
    )));

    // Footer hints
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "rules load from ~/.config/textquest/economy/",
        Style::default().fg(t.text_muted),
    )));
    lines.push(Line::from(vec![
        Span::styled(
            "e",
            Style::default()
                .fg(t.text_highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" edit · "),
        Span::styled(
            "r",
            Style::default()
                .fg(t.text_highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" reload · "),
        Span::styled(
            "t",
            Style::default()
                .fg(t.text_highlight)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" test"),
    ]));

    frame.render_widget(Paragraph::new(lines).block(blk), area);
}

/// Render the Ledger panel (magenta border, sidebar).
fn draw_ledger_panel(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let econ = &app.economy_state;

    let blk = panel(" Ledger ", Style::default().fg(t.text_server), t); // magenta border

    let mut lines = vec![
        Line::from(Span::styled(
            "Today",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Earned  ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!(
                    "+{} pp",
                    if econ.ledger_earned_today > 0 {
                        econ.ledger_earned_today
                    } else {
                        econ.vendor_plat_earned
                    }
                ),
                Style::default().fg(t.hp_high),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Vendor  ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!(
                    "+{} pp",
                    if econ.ledger_vendor_today > 0 {
                        econ.ledger_vendor_today
                    } else {
                        econ.vendor_plat_earned
                    }
                ),
                Style::default().fg(t.text_accent),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Loot    ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("+{} pp", econ.ledger_loot_today),
                Style::default().fg(t.text_highlight),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Last Cycle",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Sold    ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!(
                    "{} items",
                    if econ.ledger_last_sold > 0 {
                        econ.ledger_last_sold
                    } else {
                        econ.vendor_items_sold
                    }
                ),
                Style::default().fg(t.text_normal),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Banked  ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!(
                    "{} pp",
                    if econ.ledger_last_banked > 0 {
                        econ.ledger_last_banked
                    } else {
                        econ.banking_consolidated_plat
                    }
                ),
                Style::default().fg(t.text_normal),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Skipped ", Style::default().fg(t.text_muted)),
            Span::styled(
                if econ.ledger_last_skipped > 0 {
                    format!("{} (combat)", econ.ledger_last_skipped)
                } else {
                    String::from("0")
                },
                Style::default().fg(if econ.ledger_last_skipped > 0 {
                    t.text_highlight
                } else {
                    t.text_muted
                }),
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines).block(blk), area);
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
