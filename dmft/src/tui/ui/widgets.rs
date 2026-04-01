//! Shared widget-building helpers used across all screen modules.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row},
};

use crate::eq::structs::{SpawnInfo, SpawnType};
use crate::tui::theme::Theme;

// ─── Layout breakpoints ─────────────────────────────────────────────────────
// Named constants for width-based layout transitions so dashboard.rs and map.rs
// stay in sync when thresholds are tuned.

/// Below this width the overview switches from side-by-side to stacked layout.
pub const WIDTH_OVERVIEW_STACK: u16 = 118;
/// Above this width sidebars get medium expansion.
pub const WIDTH_SIDEBAR_MEDIUM: u16 = 145;
/// Above this width sidebars and spawn lists get full expansion.
pub const WIDTH_SIDEBAR_WIDE: u16 = 170;
/// Below this width the map screen uses a vertical stacked layout.
pub const WIDTH_MAP_STACK: u16 = 100;
/// Below this width the map uses a 2-column layout instead of 3.
pub const WIDTH_MAP_NARROW: u16 = 140;
/// Above this width the map's right panel gets expanded width.
pub const WIDTH_MAP_WIDE_RIGHT: u16 = 126;
/// Above this width the tactical sidebar gets extra room.
pub const WIDTH_MAP_EXTRA_WIDE: u16 = 180;

/// Minimum width to show the Group column in the overview roster.
pub const WIDTH_SHOW_GROUP_COL: u16 = 78;
/// Minimum width to show the Class column in the overview roster.
pub const WIDTH_SHOW_CLASS_COL: u16 = 88;
/// Minimum width to show the Zone column in the overview roster.
pub const WIDTH_SHOW_ZONE_COL: u16 = 104;

// ─── Block / panel helper ────────────────────────────────────────────────────

/// Build a `Block` with the project's standard chrome: border type + style + title.
/// Using this everywhere ensures every panel switches to rounded borders together.
pub fn panel<'a>(
    title: impl Into<ratatui::text::Line<'a>>,
    border_style: Style,
    t: &Theme,
) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(t.border_type)
        .title(title)
        .border_style(border_style)
}

// ─── Table helpers ───────────────────────────────────────────────────────────

/// Build a table header row with all cells styled using `theme.table_header`.
#[must_use]
pub fn themed_header_row<'a>(cells: Vec<&'a str>, t: &Theme) -> Row<'a> {
    Row::new(
        cells
            .into_iter()
            .map(|c| Cell::from(c).style(t.table_header))
            .collect::<Vec<_>>(),
    )
    .height(1)
    .bottom_margin(0)
}

// ─── Color helpers ───────────────────────────────────────────────────────────

#[must_use]
pub fn hp_color(hp_pct: f64, t: &Theme) -> Color {
    if hp_pct > 75.0 {
        t.hp_high
    } else if hp_pct > 25.0 {
        t.hp_mid
    } else {
        t.hp_low
    }
}

#[must_use]
pub fn stand_state_color(state: &crate::eq::structs::StandState, t: &Theme) -> Color {
    use crate::eq::structs::StandState;
    match state {
        StandState::Dead => t.state_dead,
        StandState::Sitting => t.state_sitting,
        StandState::Feigned => t.state_feigned,
        StandState::Frozen => t.state_frozen,
        _ => t.state_normal,
    }
}

#[must_use]
pub fn spawn_type_color(st: &SpawnType, t: &Theme) -> Color {
    match st {
        SpawnType::Player => t.spawn_pc,
        SpawnType::Npc => t.spawn_npc,
        SpawnType::Corpse => t.spawn_corpse,
        SpawnType::Unknown(_) => t.spawn_unknown,
    }
}

/// EQ con color — level delta from player perspective.
/// delta = `mob_level` - `player_level`
#[must_use]
pub fn con_color(player_level: u8, mob_level: u8, t: &Theme) -> Color {
    let delta = i16::from(mob_level) - i16::from(player_level);
    match delta {
        d if d >= 4 => t.con_red,
        1..=3 => t.con_yellow,
        0 => t.con_white,
        -3..=-1 => t.con_light_blue,
        -6..=-4 => t.con_blue,
        _ => t.con_green,
    }
}

#[must_use]
pub fn spawn_row_style(
    spawn: &SpawnInfo,
    player_level: Option<u8>,
    t: &Theme,
) -> ratatui::style::Style {
    match spawn.spawn_type {
        SpawnType::Player => Style::default().fg(t.spawn_pc),
        SpawnType::Npc => {
            let color = player_level.map_or(t.spawn_npc, |pl| con_color(pl, spawn.level, t));
            Style::default().fg(color)
        }
        SpawnType::Corpse => Style::default().fg(t.spawn_corpse),
        SpawnType::Unknown(_) => Style::default().fg(t.spawn_unknown),
    }
}

// ─── Spawn info lines ────────────────────────────────────────────────────────

/// Render a `SpawnInfo` as a list of styled lines (used by target panel and character screen).
pub fn spawn_info_lines(
    spawn: &SpawnInfo,
    redact: &dyn Fn(&str) -> std::borrow::Cow<str>,
    t: &Theme,
) -> Vec<Line<'static>> {
    let hp_pct = spawn.hp_pct();
    let hp_col = hp_color(hp_pct, t);
    let name = redact(&spawn.displayed_name).into_owned();
    let rawname = redact(&spawn.name).into_owned();

    vec![
        Line::from(vec![
            Span::styled(
                name,
                Style::default()
                    .fg(t.text_bright)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} Lv{}", spawn.class_str(), spawn.level),
                Style::default().fg(t.text_accent),
            ),
            Span::raw(format!("  [{}]  {}", spawn.spawn_type, spawn.stand_state)),
        ]),
        Line::from(vec![
            Span::styled("HP   ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{}/{} ({:.0}%)", spawn.hp_current, spawn.hp_max, hp_pct),
                Style::default().fg(hp_col),
            ),
        ]),
        Line::from(vec![
            Span::styled("Mana ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("{}/{}", spawn.mana_current, spawn.mana_max),
                Style::default().fg(t.mana_color),
            ),
            Span::styled(
                format!("  End {}/{}", spawn.endurance_current, spawn.endurance_max),
                Style::default().fg(t.text_muted),
            ),
        ]),
        Line::from(vec![
            Span::styled("Pos  ", Style::default().fg(t.text_muted)),
            Span::styled(
                format!("({:.1}, {:.1}, {:.1})", spawn.y, spawn.x, spawn.z),
                Style::default().fg(t.text_server),
            ),
            Span::styled(
                format!("  Hdg {:.1}", spawn.heading),
                Style::default().fg(t.text_muted),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("ID {} ", spawn.spawn_id),
                Style::default().fg(t.text_muted),
            ),
            Span::styled(rawname, Style::default().fg(t.text_secondary)),
        ]),
    ]
}

// ─── Confirm dialog ─────────────────────────────────────────────────────────

/// State for a confirmation dialog overlay (e.g., "Are you sure?").
pub struct ConfirmDialog {
    /// Title displayed at the top of the dialog.
    pub title: String,
    /// Message body — can be multi-line.
    pub message: String,
    /// Label for the confirm action (default: "Yes").
    pub confirm_label: String,
    /// Label for the cancel action (default: "No").
    pub cancel_label: String,
    /// Which button is currently focused (true = confirm, false = cancel).
    pub confirm_focused: bool,
}

impl ConfirmDialog {
    /// Create a new confirm dialog with the given title and message.
    #[must_use]
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            confirm_label: "Yes".into(),
            cancel_label: "No".into(),
            confirm_focused: false,
        }
    }

    /// Toggle focus between confirm and cancel buttons.
    pub fn toggle_focus(&mut self) {
        self.confirm_focused = !self.confirm_focused;
    }
}

/// Render a confirmation dialog as a centered popup overlay.
pub fn render_confirm_dialog(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    dialog: &ConfirmDialog,
    t: &Theme,
) {
    use ratatui::{
        layout::{Constraint, Direction, Layout},
        widgets::{Clear, Paragraph},
    };

    let popup_w = (area.width * 50 / 100).clamp(30.min(area.width), 50.min(area.width));
    let popup_h = 7u16.min(area.height);
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup_area = ratatui::layout::Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup_area);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(1)])
        .split(popup_area.inner(ratatui::layout::Margin {
            horizontal: 2,
            vertical: 1,
        }));

    let confirm_style = if dialog.confirm_focused {
        Style::default()
            .fg(Color::Black)
            .bg(t.text_accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(t.text_muted)
    };
    let cancel_style = if !dialog.confirm_focused {
        Style::default()
            .fg(Color::Black)
            .bg(t.text_accent)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(t.text_muted)
    };

    let msg = Paragraph::new(dialog.message.as_str()).style(Style::default().fg(t.text_normal));
    frame.render_widget(msg, inner[0]);

    let buttons = Line::from(vec![
        Span::styled(format!(" {} ", dialog.confirm_label), confirm_style),
        Span::raw("  "),
        Span::styled(format!(" {} ", dialog.cancel_label), cancel_style),
    ]);
    frame.render_widget(Paragraph::new(buttons), inner[1]);

    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(t.border_type)
            .title(Span::styled(
                format!(" {} ", dialog.title),
                Style::default()
                    .fg(t.text_bright)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(t.border_active)
            .style(Style::default().bg(t.help_bg)),
        popup_area,
    );
}

// ─── Progress indicator ─────────────────────────────────────────────────────

/// A progress indicator for long-running operations.
pub struct ProgressIndicator {
    /// Label describing the operation.
    pub label: String,
    /// Progress as a fraction 0.0..=1.0. `None` = indeterminate.
    pub progress: Option<f64>,
}

impl ProgressIndicator {
    /// Create a new determinate progress indicator.
    #[must_use]
    pub fn new(label: impl Into<String>, progress: f64) -> Self {
        Self {
            label: label.into(),
            progress: Some(progress.clamp(0.0, 1.0)),
        }
    }

    /// Create an indeterminate (spinning) progress indicator.
    #[must_use]
    pub fn indeterminate(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            progress: None,
        }
    }
}

/// Render a compact progress indicator suitable for the status bar.
pub fn render_progress_indicator(indicator: &ProgressIndicator, t: &Theme) -> Line<'static> {
    let label = indicator.label.clone();
    match indicator.progress {
        Some(pct) => {
            let filled = (pct * 10.0).round() as usize;
            let empty = 10 - filled;
            let bar: String = format!(
                "[{}{}]",
                "\u{2588}".repeat(filled),
                "\u{2591}".repeat(empty)
            );
            Line::from(vec![
                Span::styled(format!("{label} "), Style::default().fg(t.text_accent)),
                Span::styled(bar, Style::default().fg(t.text_highlight)),
                Span::styled(
                    format!(" {:.0}%", pct * 100.0),
                    Style::default().fg(t.text_muted),
                ),
            ])
        }
        None => {
            let frames = ["\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}", "\u{2826}", "\u{2827}"];
            // Use a static-ish frame; in real use the app tick counter drives this.
            let spinner = frames[0];
            Line::from(vec![
                Span::styled(format!("{spinner} "), Style::default().fg(t.text_accent)),
                Span::styled(label, Style::default().fg(t.text_normal)),
            ])
        }
    }
}

// ─── Status message history ─────────────────────────────────────────────────

/// Tracks the last N status messages for the notification area.
pub struct StatusHistory {
    /// Ring buffer of recent messages.
    messages: Vec<(String, std::time::Instant)>,
    /// Maximum number of messages to retain.
    capacity: usize,
}

impl StatusHistory {
    /// Create a new history with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            messages: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a new status message, evicting the oldest if at capacity.
    pub fn push(&mut self, message: impl Into<String>) {
        if self.messages.len() >= self.capacity {
            self.messages.remove(0);
        }
        self.messages.push((message.into(), std::time::Instant::now()));
    }

    /// Get all messages (oldest first).
    #[must_use]
    pub fn messages(&self) -> &[(String, std::time::Instant)] {
        &self.messages
    }

    /// Render the status history as styled lines.
    #[must_use]
    pub fn render_lines(&self, t: &Theme) -> Vec<Line<'static>> {
        self.messages
            .iter()
            .map(|(msg, _ts)| {
                Line::from(Span::styled(
                    msg.clone(),
                    Style::default().fg(t.text_muted),
                ))
            })
            .collect()
    }
}

// ─── Breadcrumb navigation ──────────────────────────────────────────────────

/// Render a breadcrumb trail showing the current navigation context.
/// Example: "Characters > Roster > Normal"
#[must_use]
pub fn render_breadcrumb(parts: &[&str], t: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(
                " \u{203a} ",
                Style::default().fg(t.text_muted),
            ));
        }
        let style = if i == parts.len() - 1 {
            // Last element is current/active — bright
            Style::default()
                .fg(t.text_accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(t.text_secondary)
        };
        spans.push(Span::styled((*part).to_string(), style));
    }
    Line::from(spans)
}

// ─── Command hint table ─────────────────────────────────────────────────────

/// A single command with its usage pattern and description.
pub struct CommandHint {
    /// The command prefix that triggers this hint (e.g., "nav").
    pub prefix: &'static str,
    /// Usage pattern shown inline (e.g., "nav <zone> [camp]").
    pub usage: &'static str,
    /// Short description for the command palette.
    pub description: &'static str,
}

/// Return the full list of available commands with usage hints and descriptions.
#[must_use]
pub fn command_hints() -> Vec<CommandHint> {
    vec![
        CommandHint { prefix: "nav", usage: "nav <zone> [camp]", description: "Navigate to a zone/camp" },
        CommandHint { prefix: "camp start", usage: "camp start <name>", description: "Start a camp by name" },
        CommandHint { prefix: "camp stop", usage: "camp stop", description: "Stop the current camp" },
        CommandHint { prefix: "camp list", usage: "camp list", description: "List available camps" },
        CommandHint { prefix: "camp add", usage: "camp add <name> <zone>", description: "Add a new camp" },
        CommandHint { prefix: "camp rm", usage: "camp rm <name>", description: "Remove a camp" },
        CommandHint { prefix: "ma", usage: "ma <name>", description: "Set main assist" },
        CommandHint { prefix: "mt", usage: "mt <name>", description: "Set main tank" },
        CommandHint { prefix: "engage", usage: "engage [target_id]", description: "Engage combat" },
        CommandHint { prefix: "disengage", usage: "disengage", description: "Stop combat" },
        CommandHint { prefix: "track", usage: "track <spawn_name>", description: "Track a spawn on the map" },
        CommandHint { prefix: "all", usage: "all /<command>", description: "Broadcast to all characters" },
        CommandHint { prefix: "invite", usage: "invite <name>", description: "Invite player to group" },
        CommandHint { prefix: "accept", usage: "accept", description: "Accept pending invite" },
        CommandHint { prefix: "mode", usage: "mode <camp|hunt>", description: "Switch operating mode" },
        CommandHint { prefix: "login", usage: "login <profile>", description: "Login a character profile" },
        CommandHint { prefix: "ch start", usage: "ch start <pids> <interval>", description: "Start CH chain" },
        CommandHint { prefix: "ch stop", usage: "ch stop", description: "Stop CH chain" },
        CommandHint { prefix: "ch add", usage: "ch add <pid>", description: "Add cleric to CH chain" },
        CommandHint { prefix: "ch rm", usage: "ch rm <pid>", description: "Remove cleric from CH chain" },
        CommandHint { prefix: "ch interval", usage: "ch interval <seconds>", description: "Set CH interval" },
        CommandHint { prefix: "ch adaptive", usage: "ch adaptive <on|off>", description: "Toggle adaptive CH timing" },
    ]
}

/// Find the best matching command hint for the current input buffer.
#[must_use]
pub fn find_command_hint(input: &str) -> Option<&'static str> {
    // Static storage so we can return references.
    // This is fine because the hints are all &'static str.
    static HINTS: std::sync::LazyLock<Vec<CommandHint>> =
        std::sync::LazyLock::new(command_hints);

    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Find the longest matching prefix
    let mut best: Option<&CommandHint> = None;
    for hint in HINTS.iter() {
        if trimmed.starts_with(hint.prefix)
            && (trimmed.len() == hint.prefix.len()
                || trimmed.as_bytes().get(hint.prefix.len()) == Some(&b' '))
        {
            if best.is_none() || hint.prefix.len() > best.unwrap().prefix.len() {
                best = Some(hint);
            }
        }
    }

    best.map(|h| h.usage)
}

// ─── Dropdown selector ─────────────────────────────────────────────────────

/// State for a dropdown/list selector widget.
pub struct DropdownSelector {
    /// Items available for selection.
    pub items: Vec<String>,
    /// Currently highlighted index.
    pub selected: usize,
    /// Whether the dropdown is open/visible.
    pub open: bool,
    /// Title shown above the dropdown.
    pub title: String,
}

impl DropdownSelector {
    /// Create a new dropdown selector.
    #[must_use]
    pub fn new(title: impl Into<String>, items: Vec<String>) -> Self {
        Self {
            items,
            selected: 0,
            open: false,
            title: title.into(),
        }
    }

    /// Move selection up.
    pub fn select_previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }

    /// Get the currently selected item.
    #[must_use]
    pub fn selected_item(&self) -> Option<&str> {
        self.items.get(self.selected).map(String::as_str)
    }

    /// Toggle the dropdown open/closed.
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
}

/// Render a dropdown selector as a popup list overlay.
pub fn render_dropdown(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    selector: &DropdownSelector,
    t: &Theme,
) {
    use ratatui::widgets::{Clear, List, ListItem, ListState};

    let item_count = selector.items.len() as u16;
    let popup_h = (item_count + 2).min(area.height.saturating_sub(4)).max(3);
    let popup_w = (area.width * 40 / 100).clamp(20.min(area.width), 45.min(area.width));
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup_area = ratatui::layout::Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = selector
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let style = if i == selector.selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(t.text_accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text_normal)
            };
            ListItem::new(Span::styled(format!(" {item} "), style))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(t.border_type)
            .title(Span::styled(
                format!(" {} ", selector.title),
                Style::default()
                    .fg(t.text_bright)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(t.border_active)
            .style(Style::default().bg(t.help_bg)),
    );

    let mut list_state = ListState::default();
    list_state.select(Some(selector.selected));
    frame.render_stateful_widget(list, popup_area, &mut list_state);
}

// ─── Command palette ────────────────────────────────────────────────────────

/// State for the command palette overlay (Ctrl+P or : with empty input).
pub struct CommandPalette {
    /// Filter text entered by the user.
    pub filter: String,
    /// Currently selected index in the filtered list.
    pub selected: usize,
    /// Whether the palette is visible.
    pub visible: bool,
}

impl CommandPalette {
    /// Create a new hidden command palette.
    #[must_use]
    pub fn new() -> Self {
        Self {
            filter: String::new(),
            selected: 0,
            visible: false,
        }
    }

    /// Show the command palette, resetting filter and selection.
    pub fn show(&mut self) {
        self.filter.clear();
        self.selected = 0;
        self.visible = true;
    }

    /// Hide the command palette.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Get filtered command hints matching the current filter text.
    #[must_use]
    pub fn filtered_commands(&self) -> Vec<&'static CommandHint> {
        static HINTS: std::sync::LazyLock<Vec<CommandHint>> =
            std::sync::LazyLock::new(command_hints);

        if self.filter.is_empty() {
            HINTS.iter().collect()
        } else {
            let lower = self.filter.to_lowercase();
            HINTS
                .iter()
                .filter(|h| {
                    h.prefix.contains(&lower)
                        || h.description.to_lowercase().contains(&lower)
                })
                .collect()
        }
    }

    /// Move selection up.
    pub fn select_previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        let count = self.filtered_commands().len();
        if self.selected + 1 < count {
            self.selected += 1;
        }
    }

    /// Get the currently selected command prefix.
    #[must_use]
    pub fn selected_command(&self) -> Option<&'static str> {
        let cmds = self.filtered_commands();
        cmds.get(self.selected).map(|h| h.prefix)
    }
}

/// Render the command palette as a centered overlay.
pub fn render_command_palette(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    palette: &CommandPalette,
    t: &Theme,
) {
    use ratatui::{
        layout::{Constraint, Direction, Layout},
        widgets::{Clear, List, ListItem, ListState, Paragraph},
    };

    let filtered = palette.filtered_commands();
    let item_count = filtered.len() as u16;
    let popup_h = (item_count + 4).min(area.height * 70 / 100).max(6);
    let popup_w = (area.width * 60 / 100).clamp(40.min(area.width), 60.min(area.width));
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup_area = ratatui::layout::Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup_area);

    let inner = popup_area.inner(ratatui::layout::Margin {
        horizontal: 1,
        vertical: 1,
    });

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    // Filter input
    let filter_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(t.text_accent)),
        Span::styled(
            format!("{}_", palette.filter),
            Style::default().fg(t.text_bright),
        ),
    ]);
    frame.render_widget(Paragraph::new(filter_line), sections[0]);

    // Separator
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "\u{2500}".repeat(sections[1].width as usize),
            Style::default().fg(t.text_muted),
        ))),
        sections[1],
    );

    // Command list
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, hint)| {
            let style = if i == palette.selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(t.text_accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text_normal)
            };
            let desc_style = if i == palette.selected {
                Style::default().fg(Color::Black).bg(t.text_accent)
            } else {
                Style::default().fg(t.text_muted)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {:<20}", hint.prefix), style),
                Span::styled(hint.description.to_string(), desc_style),
            ]))
        })
        .collect();

    let list = List::new(items);
    let mut list_state = ListState::default();
    list_state.select(Some(palette.selected));
    frame.render_stateful_widget(list, sections[2], &mut list_state);

    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(t.border_type)
            .title(Span::styled(
                " Command Palette ",
                Style::default()
                    .fg(t.text_bright)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(t.border_active)
            .style(Style::default().bg(t.help_bg)),
        popup_area,
    );
}

// ─── Notification area ──────────────────────────────────────────────────────

/// A single notification with severity level.
pub struct Notification {
    /// The notification message text.
    pub message: String,
    /// Severity level for styling.
    pub level: NotificationLevel,
    /// When the notification was created.
    pub created_at: std::time::Instant,
}

/// Severity levels for notifications — controls color/styling.
pub enum NotificationLevel {
    /// Informational message (accent color).
    Info,
    /// Warning (yellow/gold).
    Warning,
    /// Error (red).
    Error,
    /// Success (green).
    Success,
}

impl Notification {
    /// Create a new notification.
    #[must_use]
    pub fn new(message: impl Into<String>, level: NotificationLevel) -> Self {
        Self {
            message: message.into(),
            level,
            created_at: std::time::Instant::now(),
        }
    }
}

/// Render a notification area as a list of styled lines.
#[must_use]
pub fn render_notifications(notifications: &[Notification], t: &Theme) -> Vec<Line<'static>> {
    notifications
        .iter()
        .map(|n| {
            let (icon, color) = match n.level {
                NotificationLevel::Info => ("\u{2139}", t.text_accent),
                NotificationLevel::Warning => ("\u{26a0}", t.text_highlight),
                NotificationLevel::Error => ("\u{2717}", t.hp_low),
                NotificationLevel::Success => ("\u{2713}", t.hp_high),
            };
            Line::from(vec![
                Span::styled(format!("{icon} "), Style::default().fg(color)),
                Span::styled(n.message.clone(), Style::default().fg(t.text_normal)),
            ])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::dark_modern;

    #[test]
    fn con_color_red_when_much_higher() {
        let t = dark_modern();
        assert_eq!(con_color(30, 34, &t), t.con_red);
        assert_eq!(con_color(30, 40, &t), t.con_red);
    }

    #[test]
    fn con_color_yellow_when_slightly_higher() {
        let t = dark_modern();
        assert_eq!(con_color(30, 31, &t), t.con_yellow);
        assert_eq!(con_color(30, 33, &t), t.con_yellow);
    }

    #[test]
    fn con_color_white_when_same() {
        let t = dark_modern();
        assert_eq!(con_color(30, 30, &t), t.con_white);
    }

    #[test]
    fn con_color_lightcyan_when_slightly_lower() {
        let t = dark_modern();
        assert_eq!(con_color(30, 29, &t), t.con_light_blue);
        assert_eq!(con_color(30, 27, &t), t.con_light_blue);
    }

    #[test]
    fn con_color_blue_when_lower() {
        let t = dark_modern();
        assert_eq!(con_color(30, 26, &t), t.con_blue);
        assert_eq!(con_color(30, 24, &t), t.con_blue);
    }

    #[test]
    fn con_color_green_when_trivial() {
        let t = dark_modern();
        assert_eq!(con_color(30, 23, &t), t.con_green);
        assert_eq!(con_color(30, 1, &t), t.con_green);
    }

    #[test]
    fn confirm_dialog_creation() {
        let dialog = ConfirmDialog::new("Delete?", "Are you sure you want to delete?");
        assert_eq!(dialog.title, "Delete?");
        assert_eq!(dialog.message, "Are you sure you want to delete?");
        assert!(!dialog.confirm_focused);
    }

    #[test]
    fn confirm_dialog_toggle_focus() {
        let mut dialog = ConfirmDialog::new("Test", "msg");
        assert!(!dialog.confirm_focused);
        dialog.toggle_focus();
        assert!(dialog.confirm_focused);
        dialog.toggle_focus();
        assert!(!dialog.confirm_focused);
    }

    #[test]
    fn progress_indicator_determinate() {
        let p = ProgressIndicator::new("Loading", 0.5);
        assert_eq!(p.label, "Loading");
        assert!((p.progress.unwrap() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_indicator_clamps() {
        let p = ProgressIndicator::new("Test", 1.5);
        assert!((p.progress.unwrap() - 1.0).abs() < f64::EPSILON);
        let p2 = ProgressIndicator::new("Test", -0.5);
        assert!((p2.progress.unwrap() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_indicator_indeterminate() {
        let p = ProgressIndicator::indeterminate("Syncing");
        assert_eq!(p.label, "Syncing");
        assert!(p.progress.is_none());
    }

    #[test]
    fn render_progress_determinate_has_bar() {
        let t = dark_modern();
        let p = ProgressIndicator::new("Loading", 0.5);
        let line = render_progress_indicator(&p, &t);
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn status_history_push_and_capacity() {
        let mut h = StatusHistory::new(3);
        h.push("msg1");
        h.push("msg2");
        h.push("msg3");
        h.push("msg4");
        assert_eq!(h.messages().len(), 3);
        assert_eq!(h.messages()[0].0, "msg2");
    }

    #[test]
    fn status_history_render_lines() {
        let t = dark_modern();
        let mut h = StatusHistory::new(5);
        h.push("hello");
        h.push("world");
        let lines = h.render_lines(&t);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn breadcrumb_render() {
        let t = dark_modern();
        let line = render_breadcrumb(&["Characters", "Roster", "Normal"], &t);
        assert_eq!(line.spans.len(), 5); // 3 parts + 2 separators
    }

    #[test]
    fn command_hints_non_empty() {
        let hints = command_hints();
        assert!(!hints.is_empty());
    }

    #[test]
    fn find_command_hint_matches_nav() {
        let hint = find_command_hint("nav ");
        assert_eq!(hint, Some("nav <zone> [camp]"));
    }

    #[test]
    fn find_command_hint_matches_camp_start() {
        let hint = find_command_hint("camp start ");
        assert_eq!(hint, Some("camp start <name>"));
    }

    #[test]
    fn find_command_hint_no_match() {
        let hint = find_command_hint("xyz");
        assert!(hint.is_none());
    }

    #[test]
    fn find_command_hint_empty() {
        let hint = find_command_hint("");
        assert!(hint.is_none());
    }

    #[test]
    fn dropdown_selector_navigation() {
        let mut sel = DropdownSelector::new("Test", vec!["A".into(), "B".into(), "C".into()]);
        assert_eq!(sel.selected, 0);
        sel.select_next();
        assert_eq!(sel.selected, 1);
        sel.select_next();
        assert_eq!(sel.selected, 2);
        sel.select_next();
        assert_eq!(sel.selected, 2); // clamped
        sel.select_previous();
        assert_eq!(sel.selected, 1);
    }

    #[test]
    fn dropdown_selector_selected_item() {
        let sel = DropdownSelector::new("Test", vec!["Alpha".into(), "Beta".into()]);
        assert_eq!(sel.selected_item(), Some("Alpha"));
    }

    #[test]
    fn dropdown_selector_toggle() {
        let mut sel = DropdownSelector::new("Test", vec![]);
        assert!(!sel.open);
        sel.toggle();
        assert!(sel.open);
        sel.toggle();
        assert!(!sel.open);
    }

    #[test]
    fn command_palette_creation() {
        let p = CommandPalette::new();
        assert!(!p.visible);
        assert!(p.filter.is_empty());
    }

    #[test]
    fn command_palette_show_hide() {
        let mut p = CommandPalette::new();
        p.show();
        assert!(p.visible);
        p.hide();
        assert!(!p.visible);
    }

    #[test]
    fn command_palette_filtered_all() {
        let p = CommandPalette::new();
        let cmds = p.filtered_commands();
        assert!(!cmds.is_empty());
    }

    #[test]
    fn command_palette_filtered_by_text() {
        let mut p = CommandPalette::new();
        p.filter = "nav".into();
        let cmds = p.filtered_commands();
        assert!(cmds.iter().all(|c| c.prefix.contains("nav")
            || c.description.to_lowercase().contains("nav")));
    }

    #[test]
    fn notification_creation() {
        let n = Notification::new("Test msg", NotificationLevel::Info);
        assert_eq!(n.message, "Test msg");
    }

    #[test]
    fn render_notifications_styled() {
        let t = dark_modern();
        let notifs = vec![
            Notification::new("info", NotificationLevel::Info),
            Notification::new("warn", NotificationLevel::Warning),
            Notification::new("err", NotificationLevel::Error),
            Notification::new("ok", NotificationLevel::Success),
        ];
        let lines = render_notifications(&notifs, &t);
        assert_eq!(lines.len(), 4);
    }
}
