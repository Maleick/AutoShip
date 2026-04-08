//! Onboarding wizard for first-run setup.
//!
//! Launches when no config file exists or via the `:wizard` command.
//! Guides the operator through EQ client detection, character assignment,
//! camp configuration, and class role setup.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

/// Wizard step identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardStep {
    Welcome,
    ClientDetection,
    CharacterAssignment,
    CampConfiguration,
    ClassRoles,
    ReviewConfirm,
}

impl WizardStep {
    pub const ALL: [WizardStep; 6] = [
        Self::Welcome,
        Self::ClientDetection,
        Self::CharacterAssignment,
        Self::CampConfiguration,
        Self::ClassRoles,
        Self::ReviewConfirm,
    ];

    /// 1-indexed step number.
    pub fn number(self) -> u8 {
        match self {
            Self::Welcome => 1,
            Self::ClientDetection => 2,
            Self::CharacterAssignment => 3,
            Self::CampConfiguration => 4,
            Self::ClassRoles => 5,
            Self::ReviewConfirm => 6,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Welcome => "Welcome",
            Self::ClientDetection => "EQ Client Detection",
            Self::CharacterAssignment => "Character Assignment",
            Self::CampConfiguration => "Camp Configuration",
            Self::ClassRoles => "Class Roles",
            Self::ReviewConfirm => "Review & Confirm",
        }
    }

    pub fn next(self) -> Option<Self> {
        match self {
            Self::Welcome => Some(Self::ClientDetection),
            Self::ClientDetection => Some(Self::CharacterAssignment),
            Self::CharacterAssignment => Some(Self::CampConfiguration),
            Self::CampConfiguration => Some(Self::ClassRoles),
            Self::ClassRoles => Some(Self::ReviewConfirm),
            Self::ReviewConfirm => None,
        }
    }

    pub fn prev(self) -> Option<Self> {
        match self {
            Self::Welcome => None,
            Self::ClientDetection => Some(Self::Welcome),
            Self::CharacterAssignment => Some(Self::ClientDetection),
            Self::CampConfiguration => Some(Self::CharacterAssignment),
            Self::ClassRoles => Some(Self::CampConfiguration),
            Self::ReviewConfirm => Some(Self::ClassRoles),
        }
    }
}

/// A character entry being configured in the wizard.
#[derive(Debug, Clone)]
pub struct WizardCharacter {
    pub name: String,
    pub class: String,
    pub level: u8,
    pub group: u8,
    pub roles: WizardRoles,
}

/// Role toggles for a character.
#[derive(Debug, Clone, Default)]
pub struct WizardRoles {
    pub tank: bool,
    pub healer: bool,
    pub dps: bool,
    pub puller: bool,
    pub cc: bool,
}

/// Camp template presets.
#[derive(Debug, Clone)]
pub struct CampTemplate {
    pub name: &'static str,
    pub zone: &'static str,
    pub description: &'static str,
}

pub const CAMP_TEMPLATES: &[CampTemplate] = &[
    CampTemplate {
        name: "Permafrost Entrance",
        zone: "permafrost",
        description: "Safe camp near the zone entrance with steady ice giant pulls",
    },
    CampTemplate {
        name: "Eastern Wastes - Coldain",
        zone: "eastwastes",
        description: "Camp near the Coldain settlement, good for faction and experience",
    },
    CampTemplate {
        name: "Great Divide - Spires",
        zone: "greatdivide",
        description: "Near the Wizard Spires, central location with diverse pulls",
    },
    CampTemplate {
        name: "Custom",
        zone: "",
        description: "Create a custom camp configuration from scratch",
    },
];

/// Full wizard state.
pub struct WizardState {
    /// Whether the wizard is currently active.
    pub active: bool,
    /// Current step in the wizard flow.
    pub step: WizardStep,
    /// Detected EQ client PIDs (populated during client detection step).
    pub detected_clients: Vec<u32>,
    /// Characters being configured.
    pub characters: Vec<WizardCharacter>,
    /// Selected camp template index.
    pub selected_camp: usize,
    /// Currently focused field index within the active step.
    pub field_index: usize,
    /// Input buffer for text fields.
    pub input_buffer: String,
    /// Whether the wizard completed successfully.
    pub completed: bool,
    /// Zone search filter text.
    pub zone_filter: String,
}

impl WizardState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            step: WizardStep::Welcome,
            detected_clients: Vec::new(),
            characters: Vec::new(),
            selected_camp: 0,
            field_index: 0,
            input_buffer: String::new(),
            completed: false,
            zone_filter: String::new(),
        }
    }

    /// Start the wizard.
    pub fn start(&mut self) {
        self.active = true;
        self.step = WizardStep::Welcome;
        self.field_index = 0;
        self.completed = false;
    }

    /// Advance to the next step.
    pub fn advance(&mut self) {
        if let Some(next) = self.step.next() {
            self.step = next;
            self.field_index = 0;
        } else {
            self.completed = true;
            self.active = false;
        }
    }

    /// Go back to the previous step.
    pub fn go_back(&mut self) {
        if let Some(prev) = self.step.prev() {
            self.step = prev;
            self.field_index = 0;
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
        // Center the wizard popup (70% width, 80% height)
        let w = (area.width as f32 * 0.7).max(40.0).min(area.width as f32) as u16;
        let h = (area.height as f32 * 0.8).max(20.0).min(area.height as f32) as u16;
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
                " TextQuest Setup Wizard — Step {} of {}",
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
            WizardStep::CharacterAssignment => self.render_character_assignment(layout[0], buf),
            WizardStep::CampConfiguration => self.render_camp_config(layout[0], buf),
            WizardStep::ClassRoles => self.render_class_roles(layout[0], buf),
            WizardStep::ReviewConfirm => self.render_review(layout[0], buf),
        }

        // Progress bar
        self.render_progress(layout[2], buf);

        // Navigation hints
        let nav_hint = match self.state.step {
            WizardStep::Welcome => " Enter: Start  │  Esc: Skip ",
            WizardStep::ReviewConfirm => " Enter: Confirm & Save  │  Esc: Back ",
            _ => " Enter: Next  │  Esc: Back  │  Tab: Next Field  │  F1: Help ",
        };
        let hint_style = Style::default().fg(Color::DarkGray);
        buf.set_string(layout[3].x, layout[3].y, nav_hint, hint_style);
    }
}

impl WizardWidget<'_> {
    fn render_welcome(&self, area: Rect, buf: &mut Buffer) {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "╔══════════════════════════════════════╗",
                Style::default().fg(self.accent_color),
            )),
            Line::from(Span::styled(
                "║            TextQuest                      ║",
                Style::default()
                    .fg(self.accent_color)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "║      EverQuest Multibox Controller     ║",
                Style::default().fg(self.accent_color),
            )),
            Line::from(Span::styled(
                "╚══════════════════════════════════════╝",
                Style::default().fg(self.accent_color),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Welcome to the TextQuest Setup Wizard!",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("This wizard will help you configure:"),
            Line::from(""),
            Line::from(Span::styled(
                "  1. Detect running EQ clients",
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                "  2. Assign characters to groups",
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                "  3. Set up camp configurations",
                Style::default().fg(Color::White),
            )),
            Line::from(Span::styled(
                "  4. Configure class roles and strategies",
                Style::default().fg(Color::White),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press Enter to begin or Esc to skip.",
                Style::default().fg(Color::DarkGray),
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
            lines.push(Line::from("Start EQ clients and press Enter to re-scan,"));
            lines.push(Line::from("or continue with demo mode."));
        } else {
            lines.push(Line::from(format!(
                "Found {} EQ client(s):",
                self.state.detected_clients.len()
            )));
            lines.push(Line::from(""));
            for (i, pid) in self.state.detected_clients.iter().enumerate() {
                let marker = if i == self.state.field_index {
                    "▸"
                } else {
                    " "
                };
                lines.push(Line::from(format!(" {marker} PID {pid}")));
            }
        }

        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        para.render(area, buf);
    }

    fn render_character_assignment(&self, area: Rect, buf: &mut Buffer) {
        let mut lines = vec![
            Line::from(Span::styled(
                "Character Assignment",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Assign characters to groups (G1-G6)."),
            Line::from("Tab to switch fields, Enter to confirm."),
            Line::from(""),
        ];

        if self.state.characters.is_empty() {
            lines.push(Line::from(Span::styled(
                "No characters configured yet.",
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from("Press Enter to add characters."));
        } else {
            lines.push(Line::from(format!(
                " {:12} {:6} {:3} {:5} Roles",
                "Name", "Class", "Lvl", "Group"
            )));
            lines.push(Line::from(Span::styled(
                " ────────────────────────────────────────",
                Style::default().fg(Color::DarkGray),
            )));
            for (i, ch) in self.state.characters.iter().enumerate() {
                let marker = if i == self.state.field_index {
                    "▸"
                } else {
                    " "
                };
                let roles = format!(
                    "{}{}{}{}{}",
                    if ch.roles.tank { "T" } else { "." },
                    if ch.roles.healer { "H" } else { "." },
                    if ch.roles.dps { "D" } else { "." },
                    if ch.roles.puller { "P" } else { "." },
                    if ch.roles.cc { "C" } else { "." },
                );
                lines.push(Line::from(format!(
                    "{marker} {:12} {:6} {:3} G{:1}    {roles}",
                    ch.name, ch.class, ch.level, ch.group
                )));
            }
        }

        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        para.render(area, buf);
    }

    fn render_camp_config(&self, area: Rect, buf: &mut Buffer) {
        let mut lines = vec![
            Line::from(Span::styled(
                "Camp Configuration",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Select a camp template or create a custom configuration."),
            Line::from(""),
        ];

        for (i, tpl) in CAMP_TEMPLATES.iter().enumerate() {
            let marker = if i == self.state.selected_camp {
                "◉"
            } else {
                "○"
            };
            let style = if i == self.state.selected_camp {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            lines.push(Line::from(Span::styled(
                format!(" {marker} {}", tpl.name),
                style,
            )));
            lines.push(Line::from(Span::styled(
                format!("     {}", tpl.description),
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));
        }

        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        para.render(area, buf);
    }

    fn render_class_roles(&self, area: Rect, buf: &mut Buffer) {
        let mut lines = vec![
            Line::from(Span::styled(
                "Class Roles & Strategies",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Configure the strategy for each class in your group."),
            Line::from(""),
        ];

        let class_strategies = [
            ("Warrior", &["Main Tank", "Off-Tank", "DPS"][..]),
            (
                "Cleric",
                &["Main Healer", "CH Chain Participant", "Battle Cleric"],
            ),
            ("Enchanter", &["CC Primary", "Buff Bot", "DPS Enchanter"]),
            ("Bard", &["Puller", "Melee DPS + Songs", "Kiter"]),
            ("Ranger", &["Puller", "Ranged DPS", "Off-Tank"]),
            ("Wizard", &["Nuke DPS", "Porter", "AE DPS"]),
        ];

        for (i, (class, strategies)) in class_strategies.iter().enumerate() {
            let marker = if i == self.state.field_index {
                "▸"
            } else {
                " "
            };
            lines.push(Line::from(Span::styled(
                format!(" {marker} {class}"),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )));
            for (j, strat) in strategies.iter().enumerate() {
                let sel = if j == 0 { "●" } else { "○" };
                lines.push(Line::from(format!("     {sel} {strat}")));
            }
            lines.push(Line::from(""));
        }

        let para = Paragraph::new(lines).wrap(Wrap { trim: false });
        para.render(area, buf);
    }

    fn render_review(&self, area: Rect, buf: &mut Buffer) {
        let mut lines = vec![
            Line::from(Span::styled(
                "Review & Confirm",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        lines.push(Line::from(format!(
            "  Clients detected: {}",
            self.state.detected_clients.len()
        )));
        lines.push(Line::from(format!(
            "  Characters configured: {}",
            self.state.characters.len()
        )));
        lines.push(Line::from(format!(
            "  Camp template: {}",
            CAMP_TEMPLATES
                .get(self.state.selected_camp)
                .map_or("None", |t| t.name)
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Press Enter to save configuration and start.",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            "Press Esc to go back and make changes.",
            Style::default().fg(Color::DarkGray),
        )));

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

        state.step = WizardStep::ReviewConfirm;
        assert!((state.progress() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn wizard_completes_on_final_advance() {
        let mut state = WizardState::new();
        state.step = WizardStep::ReviewConfirm;
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
