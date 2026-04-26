//! Onboarding wizard stub — delegates config to web UI.
//!
//! Detects running EQ clients and opens web dashboard at /characters
//! for character assignment, camp configuration, and class roles setup.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

/// Wizard step identifier (simplified: detect clients, then open web).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardStep {
    Welcome,
    ClientDetection,
    OpenWeb,
}

impl WizardStep {
    pub const ALL: [WizardStep; 3] = [Self::Welcome, Self::ClientDetection, Self::OpenWeb];

    /// 1-indexed step number.
    pub fn number(self) -> u8 {
        match self {
            Self::Welcome => 1,
            Self::ClientDetection => 2,
            Self::OpenWeb => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Welcome => "Welcome",
            Self::ClientDetection => "EQ Client Detection",
            Self::OpenWeb => "Opening Web Dashboard",
        }
    }

    pub fn next(self) -> Option<Self> {
        match self {
            Self::Welcome => Some(Self::ClientDetection),
            Self::ClientDetection => Some(Self::OpenWeb),
            Self::OpenWeb => None,
        }
    }

    pub fn prev(self) -> Option<Self> {
        match self {
            Self::Welcome => None,
            Self::ClientDetection => Some(Self::Welcome),
            Self::OpenWeb => Some(Self::ClientDetection),
        }
    }
}

/// Full wizard state (stub: detect clients and open web).
pub struct WizardState {
    /// Whether the wizard is currently active.
    pub active: bool,
    /// Current step in the wizard flow.
    pub step: WizardStep,
    /// Detected EQ client PIDs.
    pub detected_clients: Vec<u32>,
    /// Whether the wizard completed successfully.
    pub completed: bool,
}

impl WizardState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            step: WizardStep::Welcome,
            detected_clients: Vec::new(),
            completed: false,
        }
    }

    /// Start the wizard.
    pub fn start(&mut self) {
        self.active = true;
        self.step = WizardStep::Welcome;
        self.completed = false;
    }

    /// Advance to the next step.
    pub fn advance(&mut self) {
        if let Some(next) = self.step.next() {
            self.step = next;
        } else {
            self.completed = true;
            self.active = false;
        }
    }

    /// Go back to the previous step.
    pub fn go_back(&mut self) {
        if let Some(prev) = self.step.prev() {
            self.step = prev;
        }
    }

    /// Progress ratio (0.0 to 1.0) for the progress bar.
    pub fn progress(&self) -> f64 {
        let total = WizardStep::ALL.len() as f64;
        let current = self.step.number() as f64;
        current / total
    }
}

/// Widget that draws the wizard overlay.
pub struct WizardWidget<'a> {
    state: &'a WizardState,
    accent_color: Color,
}

impl<'a> WizardWidget<'a> {
    pub fn new(state: &'a WizardState) -> Self {
        Self {
            state,
            accent_color: Color::Cyan,
        }
    }

    pub fn accent_color(mut self, color: Color) -> Self {
        self.accent_color = color;
        self
    }
}

impl Widget for WizardWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Center the wizard popup
        let w = 80_u16.min(area.width);
        let h = 20_u16.min(area.height);
        let x = area.x + (area.width.saturating_sub(w)) / 2;
        let y = area.y + (area.height.saturating_sub(h)) / 2;
        let popup = Rect::new(x, y, w, h);

        // Clear background
        for row in popup.y..popup.y + popup.height {
            for col in popup.x..popup.x + popup.width {
                if row < buf.area().height && col < buf.area().width {
                    buf.set_string(col, row, " ", Style::default().bg(Color::Black));
                }
            }
        }

        let block = Block::default()
            .title(format!(
                " TextQuest Setup Wizard · Step {} of {} ",
                self.state.step.number(),
                WizardStep::ALL.len()
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.accent_color))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(popup);
        block.render(popup, buf);

        if inner.height < 3 || inner.width < 10 {
            return;
        }

        // Layout: content area + progress bar at bottom
        let layout = Layout::vertical([
            Constraint::Min(3),    // Content
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Progress bar
            Constraint::Length(1), // Navigation hints
        ])
        .split(inner);

        // Render step content
        match self.state.step {
            WizardStep::Welcome => self.render_welcome(layout[0], buf),
            WizardStep::ClientDetection => self.render_client_detection(layout[0], buf),
            WizardStep::OpenWeb => self.render_open_web(layout[0], buf),
        }

        // Progress bar
        self.render_progress(layout[2], buf);

        // Navigation hints
        let nav_style = Style::default().fg(Color::DarkGray);
        let hint = match self.state.step {
            WizardStep::Welcome => "◀ Skip  │  Next ▶",
            WizardStep::OpenWeb => "Opening web dashboard...",
            _ => "◀ Back  │  Next ▶  │  esc cancel",
        };
        buf.set_string(layout[3].x, layout[3].y, hint, nav_style);
    }
}

impl WizardWidget<'_> {
    fn render_welcome(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Welcome to TextQuest Setup",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("This quick wizard will:"),
            Line::from(""),
            Line::from(Span::styled(
                "  1. Detect your running EQ clients",
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                "  2. Open the web dashboard at /characters",
                Style::default().fg(Color::White),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Configure characters, camps, and roles in the web UI.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press Enter to continue.",
                Style::default().fg(Color::Yellow),
            )),
        ];
        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        para.render(area, buf);
    }

    fn render_client_detection(&self, area: Rect, buf: &mut Buffer) {
        let mut lines = vec![
            Line::from(Span::styled(
                "EQ Client Detection",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if self.state.detected_clients.is_empty() {
            lines.push(Line::from(Span::styled(
                "No EQ clients detected.",
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from("Start EQ clients (eqgame.exe) and press Enter"));
            lines.push(Line::from("to re-scan, or continue anyway."));
        } else {
            lines.push(Line::from(format!(
                "Found {} EQ client(s):",
                self.state.detected_clients.len()
            )));
            lines.push(Line::from(""));
            for pid in &self.state.detected_clients {
                lines.push(Line::from(format!("  • PID {pid}")));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press Enter to continue to web dashboard.",
            Style::default().fg(Color::Yellow),
        )));

        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        para.render(area, buf);
    }

    fn render_open_web(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            Line::from(Span::styled(
                "Opening Web Dashboard",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Opening browser to:"),
            Line::from(""),
            Line::from(Span::styled(
                "  http://localhost:3001/characters",
                Style::default().fg(Color::Cyan),
            )),
            Line::from(""),
            Line::from("Complete your configuration in the web UI:"),
            Line::from(""),
            Line::from("  • Assign characters to groups"),
            Line::from("  • Select camp configurations"),
            Line::from("  • Set class roles and strategies"),
        ];

        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        para.render(area, buf);
    }

    fn render_progress(&self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 {
            return;
        }
        let progress = self.state.progress();
        let filled = ((area.width as f64 - 2.0) * progress) as u16;
        let empty = area.width.saturating_sub(2).saturating_sub(filled);

        let mut parts = String::from("[");
        for _ in 0..filled {
            parts.push('█');
        }
        for _ in 0..empty {
            parts.push('░');
        }
        parts.push(']');

        buf.set_string(
            area.x,
            area.y,
            &parts,
            Style::default().fg(self.accent_color),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wizard_step_navigation() {
        let mut state = WizardState::new();
        state.start();
        assert_eq!(state.step, WizardStep::Welcome);

        state.advance();
        assert_eq!(state.step, WizardStep::ClientDetection);

        state.advance();
        assert_eq!(state.step, WizardStep::OpenWeb);

        state.go_back();
        assert_eq!(state.step, WizardStep::ClientDetection);

        state.go_back();
        assert_eq!(state.step, WizardStep::Welcome);

        state.go_back(); // Can't go before welcome
        assert_eq!(state.step, WizardStep::Welcome);
    }

    #[test]
    fn wizard_progress_calculation() {
        let mut state = WizardState::new();
        state.start();
        assert!(state.progress() > 0.0);

        state.step = WizardStep::OpenWeb;
        assert!((state.progress() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn wizard_completes_on_final_advance() {
        let mut state = WizardState::new();
        state.step = WizardStep::OpenWeb;
        state.active = true;
        state.advance();
        assert!(state.completed);
        assert!(!state.active);
    }

    #[test]
    fn all_steps_have_labels() {
        for step in WizardStep::ALL {
            assert!(!step.label().is_empty());
        }
    }
}
