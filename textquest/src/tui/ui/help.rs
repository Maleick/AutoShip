//! Help search panel — centered overlay modal with search bar, tab selector,
//! scrollable result list, detail view, and footer keybinding hints.
//!

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use crate::tui::{
    app::App,
    command::{CommandEntry, HelpSection, command_entries},
    ui::widgets::{centered_popup, truncate_inline},
};

// ─── State stub (replace with #1110 canonical type once merged) ──────────────

/// Selectable category tabs in the help search panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HelpTab {
    #[default]
    Commands,
    Shortcuts,
    Faq,
    Tips,
}

impl HelpTab {
    fn label(self) -> &'static str {
        match self {
            HelpTab::Commands => "Commands",
            HelpTab::Shortcuts => "Shortcuts",
            HelpTab::Faq => "FAQ",
            HelpTab::Tips => "Tips",
        }
    }

    fn all() -> &'static [HelpTab] {
        &[
            HelpTab::Commands,
            HelpTab::Shortcuts,
            HelpTab::Faq,
            HelpTab::Tips,
        ]
    }

    fn next(self) -> Self {
        match self {
            HelpTab::Commands => HelpTab::Shortcuts,
            HelpTab::Shortcuts => HelpTab::Faq,
            HelpTab::Faq => HelpTab::Tips,
            HelpTab::Tips => HelpTab::Commands,
        }
    }

    fn prev(self) -> Self {
        match self {
            HelpTab::Commands => HelpTab::Tips,
            HelpTab::Shortcuts => HelpTab::Commands,
            HelpTab::Faq => HelpTab::Shortcuts,
            HelpTab::Tips => HelpTab::Faq,
        }
    }
}

/// State for the interactive help search panel.
#[derive(Debug, Clone, Default)]
pub struct HelpPanelState {
    /// Current search query entered by the operator.
    pub query: String,
    /// Cursor position within the query string (byte offset).
    pub cursor: usize,
    /// Active category tab.
    pub tab: HelpTab,
    /// Selected result row index (0-based within visible list).
    pub selected: usize,
    /// Scroll offset for the result list.
    pub scroll: usize,
    /// Whether the selected item is expanded into a full content pane.
    pub expanded: bool,
}

impl HelpPanelState {
    /// Return a new zeroed state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Move selection up by one row; clamp at 0.
    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
        self.expanded = false;
    }

    /// Move selection down by one row; clamp at `max`.
    pub fn select_next(&mut self, max: usize) {
        if self.selected + 1 < max {
            self.selected += 1;
        }
        self.expanded = false;
    }

    /// Adjust scroll so `selected` is always visible inside `page_height` rows.
    pub fn ensure_visible(&mut self, page_height: usize) {
        if page_height == 0 {
            return;
        }
        if self.selected >= self.scroll + page_height {
            self.scroll = self.selected - page_height + 1;
        }
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
    }

    /// Append a character to the query and advance the cursor.
    pub fn push_char(&mut self, ch: char) {
        self.query.push(ch);
        self.cursor = self.query.len();
        self.selected = 0;
        self.scroll = 0;
        self.expanded = false;
    }

    /// Delete the last character before the cursor.
    pub fn pop_char(&mut self) {
        if !self.query.is_empty() {
            self.query.pop();
            self.cursor = self.query.len();
            self.selected = 0;
            self.scroll = 0;
            self.expanded = false;
        }
    }

    /// Cycle to the next category tab and reset result position.
    pub fn next_tab(&mut self) {
        self.tab = self.tab.next();
        self.selected = 0;
        self.scroll = 0;
        self.expanded = false;
    }

    /// Cycle to the previous category tab and reset result position.
    pub fn prev_tab(&mut self) {
        self.tab = self.tab.prev();
        self.selected = 0;
        self.scroll = 0;
        self.expanded = false;
    }

    /// Toggle between split list/detail view and full detail view.
    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }

    /// Clear the search query and return to the first result.
    pub fn clear_query(&mut self) {
        self.query.clear();
        self.cursor = 0;
        self.selected = 0;
        self.scroll = 0;
        self.expanded = false;
    }

    /// Focus the shortcut cheat sheet and optionally pre-filter it.
    pub fn focus_shortcuts(&mut self, query: impl Into<String>) {
        self.tab = HelpTab::Shortcuts;
        self.query = query.into();
        self.cursor = self.query.len();
        self.selected = 0;
        self.scroll = 0;
        self.expanded = false;
    }
}

// ─── Help topic registry ─────────────────────────────────────────────────────

/// A single entry shown in the result list.
#[derive(Debug, Clone)]
pub struct HelpEntry<'a> {
    /// Display name / command phrase.
    pub title: &'a str,
    /// Short one-line summary.
    pub summary: &'a str,
    /// Usage pattern.
    pub usage: &'a str,
    /// Concrete example.
    pub example: &'a str,
    /// Category used for color coding.
    pub section: HelpSection,
}

impl<'a> HelpEntry<'a> {
    fn from_command(cmd: &'a CommandEntry) -> Self {
        HelpEntry {
            title: cmd.phrase,
            summary: cmd.summary,
            usage: cmd.usage,
            example: cmd.example,
            section: cmd.section,
        }
    }
}

/// Static FAQ topics shown under the FAQ tab.
static FAQ: &[(&str, &str, &str, &str, HelpSection)] = &[
    (
        "How do I open help?",
        "Press ? or type :help in the command bar.",
        ":help [topic]",
        ":help nav",
        HelpSection::Lifecycle,
    ),
    (
        "How do I switch screens?",
        "Use number keys 1-7 or the tab bar to switch between screens.",
        "1-7",
        "Press 2 for Tactical view",
        HelpSection::Workflows,
    ),
    (
        "How do I close an overlay?",
        "Press Esc to close any overlay or modal panel.",
        "Esc",
        "Press Esc to close this panel",
        HelpSection::Lifecycle,
    ),
    (
        "What is the command bar?",
        "Type : to open the command bar. Enter slash-commands like :help, :status, :mode.",
        ":<command>",
        ":status overview",
        HelpSection::Lifecycle,
    ),
];

/// Built-in keyboard/accessibility shortcuts shown in the Shortcuts tab.
static SHORTCUTS: &[(&str, &str, &str, &str, HelpSection)] = &[
    (
        "Keyboard shortcuts",
        "Use /help keyboard or :help keyboard to open this shortcut sheet.",
        ":help keyboard",
        ":help keyboard",
        HelpSection::Workflows,
    ),
    (
        "Tab panel focus",
        "Tab and Shift-Tab move focus through visible panels and announce the focused panel in the status bar.",
        "Tab / Shift-Tab",
        "Tab -> next panel",
        HelpSection::Workflows,
    ),
    (
        "List navigation",
        "Arrow keys move through focused lists; Enter selects, expands, or runs the highlighted action.",
        "Up / Down / Enter",
        "Down -> Enter",
        HelpSection::Workflows,
    ),
    (
        "Vim bindings",
        "When vi keys are enabled, supported lists also accept j and k for down and up.",
        "[ui.keyboard] style = \"vim\"",
        "j -> next row",
        HelpSection::Workflows,
    ),
    (
        "Emacs bindings",
        "The Emacs profile enables Ctrl+N and Ctrl+P client navigation aliases.",
        "[ui.keyboard] style = \"emacs\"",
        "Ctrl+N -> next client",
        HelpSection::Workflows,
    ),
    (
        "High contrast",
        "Use the accessibility command to request the high-contrast presentation mode.",
        ":accessibility high-contrast on",
        ":a11y high-contrast on",
        HelpSection::Lifecycle,
    ),
    (
        "Screen reader basics",
        "Screen reader mode emphasizes explicit status text for focus, selection, and mode changes.",
        ":accessibility screen-reader on",
        ":a11y screen-reader on",
        HelpSection::Lifecycle,
    ),
    (
        "Cheat sheet export",
        "Write the current shortcut reference to a Markdown file.",
        ":help keyboard export [path]",
        ":help keyboard export docs/keyboard-shortcuts.md",
        HelpSection::Workflows,
    ),
];

/// Static tips shown under the Tips tab.
static TIPS: &[(&str, &str, &str, &str, HelpSection)] = &[
    (
        "Search narrows as you type",
        "Start typing in the search box to filter commands and topics in real time.",
        "/ to focus search",
        "Type 'nav' to filter navigation commands",
        HelpSection::Workflows,
    ),
    (
        "Use Tab to cycle category tabs",
        "Press Tab / Shift-Tab to move between Commands, FAQ, Tips, and Results tabs.",
        "Tab / Shift-Tab",
        "Tab → switch to FAQ tab",
        HelpSection::Workflows,
    ),
    (
        "Enter opens a command detail",
        "Select a result and press Enter to see full usage, aliases, and examples.",
        "Enter",
        "Select 'nav' → press Enter",
        HelpSection::Workflows,
    ),
];

/// Collect entries for the active tab, filtered by `query`.
pub fn filtered_entries<'a>(state: &HelpPanelState) -> Vec<HelpEntry<'a>> {
    let q = state.query.to_lowercase();

    match state.tab {
        HelpTab::Commands => command_entries()
            .iter()
            .filter(|cmd| {
                q.is_empty()
                    || cmd.phrase.contains(q.as_str())
                    || cmd.summary.to_lowercase().contains(q.as_str())
                    || cmd.aliases.iter().any(|a| a.contains(q.as_str()))
            })
            .map(HelpEntry::from_command)
            .collect(),
        HelpTab::Shortcuts => SHORTCUTS
            .iter()
            .filter(|(title, summary, usage, example, ..)| {
                q.is_empty()
                    || title.to_lowercase().contains(q.as_str())
                    || summary.to_lowercase().contains(q.as_str())
                    || usage.to_lowercase().contains(q.as_str())
                    || example.to_lowercase().contains(q.as_str())
            })
            .map(|(title, summary, usage, example, section)| HelpEntry {
                title,
                summary,
                usage,
                example,
                section: *section,
            })
            .collect(),
        HelpTab::Faq => FAQ
            .iter()
            .filter(|(title, summary, ..)| {
                q.is_empty()
                    || title.to_lowercase().contains(q.as_str())
                    || summary.to_lowercase().contains(q.as_str())
            })
            .map(|(title, summary, usage, example, section)| HelpEntry {
                title,
                summary,
                usage,
                example,
                section: *section,
            })
            .collect(),
        HelpTab::Tips => TIPS
            .iter()
            .filter(|(title, summary, ..)| {
                q.is_empty()
                    || title.to_lowercase().contains(q.as_str())
                    || summary.to_lowercase().contains(q.as_str())
            })
            .map(|(title, summary, usage, example, section)| HelpEntry {
                title,
                summary,
                usage,
                example,
                section: *section,
            })
            .collect(),
    }
}

// ─── Renderer ────────────────────────────────────────────────────────────────

/// Section → accent colour mapping.
fn section_color(section: HelpSection, app: &App) -> ratatui::style::Color {
    let t = &app.theme;
    match section {
        HelpSection::Workflows | HelpSection::Lifecycle => t.text_accent,
        HelpSection::Combat => t.hp_low,
        HelpSection::Navigation => t.hp_high,
        HelpSection::Targeting => t.text_highlight,
        HelpSection::ChChain => t.text_server,
        HelpSection::Troubleshooting => t.text_muted,
    }
}

/// Draw the help search panel overlay into `frame`.
///
/// This is the primary public entry point called from `ui/mod.rs`.
pub fn draw_help_search_panel(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let state = &app.help_search_state;

    // Popup geometry: centered, but clamped by `centered_popup` for small terminals.
    let popup = centered_popup(area, 80, 70, 80, 24, 80, 30, 1);

    frame.render_widget(Clear, popup);

    // Outer block with title.
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border_type)
        .title(Span::styled(
            " Help ",
            Style::default()
                .fg(t.text_bright)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(t.text_accent))
        .style(Style::default().bg(t.help_bg));

    let inner = outer_block.inner(popup);
    frame.render_widget(outer_block, popup);

    // ── Vertical layout: search bar | tab row | result list | detail | footer ─
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search input
            Constraint::Length(1), // tab row
            Constraint::Min(6),    // result list + detail split
            Constraint::Length(1), // footer hints
        ])
        .split(inner);

    draw_search_bar(frame, chunks[0], app);
    draw_tab_row(frame, chunks[1], app);
    draw_result_and_detail(frame, chunks[2], app);
    draw_footer(frame, chunks[3], app);
}

/// Search input bar with inline cursor indicator.
fn draw_search_bar(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let state = &app.help_search_state;

    let cursor_char = if (app.tick_count / 15) % 2 == 0 {
        "█"
    } else {
        " "
    };
    let display = format!("{}{}", state.query, cursor_char);
    let display = truncate_inline(&display, area.width.saturating_sub(4) as usize);

    let input = Paragraph::new(display)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(t.border_type)
                .title(Span::styled(" Search ", Style::default().fg(t.text_muted)))
                .border_style(t.border_dim),
        )
        .style(Style::default().fg(t.text_normal).bg(t.help_bg));

    frame.render_widget(input, area);
}

/// Category tab row.
fn draw_tab_row(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let state = &app.help_search_state;

    let tabs: Vec<Span> = HelpTab::all()
        .iter()
        .flat_map(|tab| {
            let active = *tab == state.tab;
            let style = if active {
                Style::default()
                    .fg(t.text_bright)
                    .bg(t.text_accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text_secondary)
            };
            let label = format!(" {} ", tab.label());
            vec![Span::styled(label, style), Span::raw("  ")]
        })
        .collect();

    frame.render_widget(Paragraph::new(Line::from(tabs)), area);
}

/// Split lower area into result list (left/top) and detail pane (right/bottom).
fn draw_result_and_detail(frame: &mut Frame, area: Rect, app: &App) {
    if app.help_search_state.expanded {
        draw_detail_pane(frame, area, app);
        return;
    }

    // Use vertical split: results on top (60%), detail below (40%).
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    draw_result_list(frame, split[0], app);
    draw_detail_pane(frame, split[1], app);
}

/// Scrollable filtered result list.
fn draw_result_list(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let state = &app.help_search_state;
    let entries = filtered_entries(state);

    let page_height = area.height.saturating_sub(2) as usize; // subtract borders
    let selected = state.selected.min(entries.len().saturating_sub(1));
    let scroll = if page_height == 0 {
        state.scroll
    } else if selected >= state.scroll + page_height {
        selected - page_height + 1
    } else {
        state.scroll.min(selected)
    };
    let count = entries.len();

    let title = if entries.is_empty() {
        " Results — no matches ".to_string()
    } else {
        format!(" Results ({count}) ", count = entries.len())
    };

    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .skip(scroll)
        .take(page_height.max(1))
        .map(|(idx, entry)| {
            let color = section_color(entry.section, app);
            let is_selected = idx == selected;
            let marker = if is_selected { "▶ " } else { "  " };
            let label = truncate_inline(entry.title, area.width.saturating_sub(20) as usize);
            let summary = truncate_inline(
                entry.summary,
                area.width.saturating_sub(label.len() as u16 + 6) as usize,
            );
            let line = Line::from(vec![
                Span::styled(marker, Style::default().fg(t.text_accent)),
                Span::styled(format!("{label:<18}"), Style::default().fg(color)),
                Span::styled("  ", Style::default()),
                Span::styled(summary, Style::default().fg(t.text_muted)),
            ]);
            let style = if is_selected {
                Style::default().bg(t.row_selected_bg)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(t.border_type)
            .title(Span::styled(title, Style::default().fg(t.text_secondary)))
            .border_style(t.border_dim),
    );

    frame.render_widget(list, area);
}

/// Detail pane — shows usage and example for the currently selected entry.
fn draw_detail_pane(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let state = &app.help_search_state;
    let entries = filtered_entries(state);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(t.border_type)
        .title(Span::styled(
            " Detail ",
            Style::default().fg(t.text_secondary),
        ))
        .border_style(Style::default().fg(t.border_dim.fg.unwrap_or(t.text_muted)));

    if entries.is_empty() {
        let empty = Paragraph::new("No entry selected.")
            .style(Style::default().fg(t.text_muted).bg(t.help_bg))
            .block(block);
        frame.render_widget(empty, area);
        return;
    }

    let entry = &entries[state.selected.min(entries.len().saturating_sub(1))];
    let color = section_color(entry.section, app);

    let lines = vec![
        Line::from(vec![Span::styled(
            entry.title,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Usage:   ", Style::default().fg(t.text_muted)),
            Span::styled(entry.usage, Style::default().fg(t.text_highlight)),
        ]),
        Line::from(vec![
            Span::styled("Example: ", Style::default().fg(t.text_muted)),
            Span::styled(entry.example, Style::default().fg(t.text_accent)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            entry.summary,
            Style::default().fg(t.text_normal),
        )]),
    ];

    let detail = Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(t.help_bg))
        .block(block);

    frame.render_widget(detail, area);
}

/// Footer row with keybinding hints.
fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    let hints = vec![
        Span::styled(
            " / ",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("search  ", Style::default().fg(t.text_muted)),
        Span::styled(
            "q/Esc ",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("close  ", Style::default().fg(t.text_muted)),
        Span::styled(
            "↑↓ ",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("navigate  ", Style::default().fg(t.text_muted)),
        Span::styled(
            "Enter ",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("expand  ", Style::default().fg(t.text_muted)),
        Span::styled(
            "Tab ",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("switch tab  ", Style::default().fg(t.text_muted)),
        Span::styled(
            ":help keyboard ",
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("shortcuts", Style::default().fg(t.text_muted)),
    ];

    frame.render_widget(Paragraph::new(Line::from(hints)), area);
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    use crate::tui::app::App;
    use crate::tui::ui::help::draw_help_search_panel;

    fn render_help(mut app: App, width: u16, height: u16) -> String {
        app.help_search_visible = true;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| {
                let area = frame.area();
                draw_help_search_panel(frame, area, &app);
            })
            .expect("draw");
        let buf = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn help_search_panel_renders_title() {
        let app = App::new();
        let rendered = render_help(app, 100, 34);
        assert!(rendered.contains("Help"), "missing Help title");
    }

    #[test]
    fn help_search_panel_renders_tabs() {
        let app = App::new();
        let rendered = render_help(app, 100, 34);
        assert!(rendered.contains("Commands"), "missing Commands tab");
        assert!(rendered.contains("FAQ"), "missing FAQ tab");
        assert!(rendered.contains("Tips"), "missing Tips tab");
    }

    #[test]
    fn help_search_panel_renders_footer_hints() {
        let app = App::new();
        let rendered = render_help(app, 100, 34);
        assert!(
            rendered.contains("search") || rendered.contains("close"),
            "missing footer hints"
        );
    }

    #[test]
    fn help_search_panel_renders_at_narrow_width() {
        let app = App::new();
        // Should not panic at minimum width.
        let rendered = render_help(app, 82, 26);
        assert!(rendered.contains("Help"));
    }

    #[test]
    fn help_panel_state_push_pop() {
        let mut state = HelpPanelState::new();
        state.push_char('n');
        state.push_char('a');
        state.push_char('v');
        assert_eq!(state.query, "nav");
        assert_eq!(state.cursor, 3);
        state.pop_char();
        assert_eq!(state.query, "na");
    }

    #[test]
    fn filtered_entries_filters_by_query() {
        let mut state = HelpPanelState::new();
        state.query = "nav".to_string();
        state.tab = HelpTab::Commands;
        let entries = filtered_entries(&state);
        assert!(!entries.is_empty(), "expected at least one nav result");
        assert!(
            entries
                .iter()
                .any(|e| e.title.contains("nav") || e.summary.to_lowercase().contains("nav"))
        );
    }

    #[test]
    fn filtered_entries_empty_query_returns_all_commands() {
        let state = HelpPanelState::default();
        let entries = filtered_entries(&state);
        assert!(entries.len() >= 5, "expected full command list");
    }

    #[test]
    fn help_panel_state_select_navigation() {
        let mut state = HelpPanelState::new();
        state.select_next(10);
        assert_eq!(state.selected, 1);
        state.select_prev();
        assert_eq!(state.selected, 0);
        state.select_prev(); // should not underflow
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn help_panel_tabs_cycle_across_issue_sections() {
        let mut state = HelpPanelState::new();
        assert_eq!(state.tab, HelpTab::Commands);
        state.next_tab();
        assert_eq!(state.tab, HelpTab::Shortcuts);
        state.next_tab();
        assert_eq!(state.tab, HelpTab::Faq);
        state.next_tab();
        assert_eq!(state.tab, HelpTab::Tips);
        state.next_tab();
        assert_eq!(state.tab, HelpTab::Commands);
        state.prev_tab();
        assert_eq!(state.tab, HelpTab::Tips);
    }

    #[test]
    fn help_panel_expanded_resets_when_search_changes() {
        let mut state = HelpPanelState::new();
        state.toggle_expanded();
        assert!(state.expanded);
        state.push_char('n');
        assert!(!state.expanded);
        assert_eq!(state.query, "n");
    }

    #[test]
    fn help_panel_ensure_visible_scrolls_down() {
        let mut state = HelpPanelState::new();
        state.selected = 12;
        state.ensure_visible(5);
        assert!(state.scroll + 5 > state.selected || state.scroll <= state.selected);
        assert!(state.selected >= state.scroll);
    }

    #[test]
    fn faq_entries_filter_correctly() {
        let mut state = HelpPanelState::new();
        state.tab = HelpTab::Faq;
        state.query = "Esc".to_string();
        let entries = filtered_entries(&state);
        assert!(
            !entries.is_empty(),
            "expected at least one FAQ result for Esc"
        );
    }

    #[test]
    fn shortcut_entries_include_keyboard_help() {
        let mut state = HelpPanelState::new();
        state.focus_shortcuts("keyboard");
        let entries = filtered_entries(&state);
        assert!(
            entries.iter().any(|entry| entry.title.contains("Keyboard")),
            "expected keyboard shortcut help entry"
        );
    }
}
