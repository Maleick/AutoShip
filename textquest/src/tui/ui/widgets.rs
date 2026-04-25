//! Shared widget-building helpers used across all screen modules.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row},
};

use crate::{
    combat::spell_db,
    eq::structs::{CastState as EqCastState, SpawnInfo, SpawnType},
    tui::ui::ch_chain::CastState as ChainCastState,
    tui::{cast::CastDisplay, command, theme::Theme},
};

// ─── Layout breakpoints ─────────────────────────────────────────────────────
// Named constants for width-based layout transitions so roster.rs and map.rs
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
/// Below this width global chrome uses aggressive compaction.
pub const WIDTH_CHROME_MEDIUM: u16 = 96;
/// Above this width global chrome can render in its full form.
pub const WIDTH_CHROME_WIDE: u16 = 130;

/// Shared width classes for header/footer/popups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidthClass {
    Narrow,
    Medium,
    Wide,
}

/// Classify a width into narrow, medium, or wide chrome modes.
#[must_use]
pub fn classify_width(width: u16) -> WidthClass {
    if width < WIDTH_CHROME_MEDIUM {
        WidthClass::Narrow
    } else if width < WIDTH_CHROME_WIDE {
        WidthClass::Medium
    } else {
        WidthClass::Wide
    }
}

/// Count the visible width of a span collection in terminal cells.
#[must_use]
pub fn spans_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(Span::width).sum()
}

/// Count the visible width of a line in terminal cells.
#[must_use]
pub fn line_width(line: &Line<'_>) -> usize {
    line.width()
}

/// Build a status pill widget (capsule-style badge) for header status indicators.
/// Returns a vector of spans with bold uppercase text and accent color.
/// Color is determined by the label type: magenta for HUNT, cyan for PALETTE, etc.
#[must_use]
pub fn status_pill(label: &str, t: &Theme) -> Vec<Span<'static>> {
    let fg_color = match label {
        "HUNT" => Color::Magenta,
        "CAMP" => Color::Cyan,
        "PALETTE" | "⌘K PALETTE" => Color::Cyan,
        "VISIBLE" => t.text_accent,
        _ => t.text_muted,
    };

    let uppercase = label.to_uppercase();
    vec![Span::styled(
        format!(" {} ", uppercase),
        Style::default()
            .fg(fg_color)
            .add_modifier(Modifier::BOLD),
    )]
}

/// Build a centered popup rect with bounded margins on small terminals.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn centered_popup(
    area: Rect,
    width_pct: u16,
    height_pct: u16,
    min_width: u16,
    min_height: u16,
    max_width: u16,
    max_height: u16,
    margin: u16,
) -> Rect {
    let max_popup_width = area.width.saturating_sub(margin.saturating_mul(2)).max(1);
    let max_popup_height = area.height.saturating_sub(margin.saturating_mul(2)).max(1);
    let width_cap = max_popup_width.min(max_width.max(1));
    let height_cap = max_popup_height.min(max_height.max(1));
    let requested_width = ((u32::from(area.width) * u32::from(width_pct)) / 100) as u16;
    let requested_height = ((u32::from(area.height) * u32::from(height_pct)) / 100) as u16;
    let popup_width = requested_width
        .max(min_width.min(width_cap))
        .min(width_cap)
        .max(1);
    let popup_height = requested_height
        .max(min_height.min(height_cap))
        .min(height_cap)
        .max(1);
    let x = area.x + area.width.saturating_sub(popup_width) / 2;
    let y = area.y + area.height.saturating_sub(popup_height) / 2;
    Rect::new(x, y, popup_width, popup_height)
}

// ─── Block / panel helper ────────────────────────────────────────────────────

/// Build a `Block` with the project's standard chrome: border type + style +
/// title. Using this everywhere ensures every panel switches to rounded borders
/// together.
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
pub fn themed_header_row<'a>(cells: &'a [&'a str], t: &Theme) -> Row<'a> {
    Row::new(cells.iter().map(|c| Cell::from(*c).style(t.table_header)))
        .height(1)
        .bottom_margin(0)
}

// ─── Color helpers ───────────────────────────────────────────────────────────

/// Map an HP percentage to a themed color (green/yellow/red).
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

/// Map a CH chain cast state to themed semantic colors.
#[must_use]
pub fn cast_state_color(state: &ChainCastState, t: &Theme) -> Color {
    match *state {
        ChainCastState::Idle => t.text_muted,
        ChainCastState::Casting(progress) if progress > 0.75 => t.hp_high,
        ChainCastState::Casting(progress) if progress > 0.25 => t.text_accent,
        ChainCastState::Casting(_) => t.text_muted,
        ChainCastState::Completed => t.hp_high,
        ChainCastState::Missed => t.hp_low,
    }
}

/// Map a stand state (dead, sitting, feigned, etc.) to a themed color.
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

/// Map a spawn type (PC, NPC, corpse) to a themed color.
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

/// Compute the row style for a spawn entry based on type and con color.
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

/// Human-readable cast label for a `LaunchSpellData` snapshot.
#[must_use]
pub fn cast_summary(cast: &EqCastState) -> String {
    let spell_name = cast
        .spell_name
        .clone()
        .or_else(|| {
            u32::try_from(cast.spell_id)
                .ok()
                .and_then(spell_db::get)
                .map(|spell| spell.name.to_string())
        })
        .unwrap_or_else(|| format!("Spell {}", cast.spell_id));

    match cast.spell_gem() {
        Some(gem) => format!("G{gem} {spell_name}"),
        None => spell_name,
    }
}

/// Format remaining cast time in a compact user-facing form.
#[must_use]
pub fn cast_time_remaining_label(cast: &EqCastState) -> Option<String> {
    let remaining_ms = cast.cast_time_remaining_ms()?;
    if remaining_ms >= 1_000 {
        Some(format!("{:.1}s", f64::from(remaining_ms) / 1_000.0))
    } else {
        Some(format!("{remaining_ms}ms"))
    }
}

// ─── Spawn info lines ────────────────────────────────────────────────────────

/// Render a `SpawnInfo` as a list of styled lines (used by target panel and
/// character screen).
pub fn spawn_info_lines(
    spawn: &SpawnInfo,
    redact: &dyn Fn(&str) -> std::borrow::Cow<str>,
    t: &Theme,
) -> Vec<Line<'static>> {
    let hp_pct = spawn.hp_pct();
    let hp_col = hp_color(hp_pct, t);
    let name = redact(&spawn.displayed_name).into_owned();
    let rawname = redact(&spawn.name).into_owned();

    let mut lines = vec![
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
    ];

    if let Some(cast) = spawn.cast_state.as_ref().filter(|cast| cast.is_casting()) {
        let mut spans = vec![
            Span::styled("Cast ", Style::default().fg(t.text_muted)),
            Span::styled(
                cast_summary(cast),
                Style::default()
                    .fg(t.text_highlight)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        if cast.target_id != 0 {
            spans.push(Span::styled(
                format!("  -> {}", cast.target_id),
                Style::default().fg(t.text_secondary),
            ));
        }
        if let Some(remaining) = cast_time_remaining_label(cast) {
            spans.push(Span::styled(
                format!("  {remaining}"),
                Style::default().fg(t.text_muted),
            ));
        }
        lines.push(Line::from(spans));
    }

    lines
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
        widgets::{Clear, Paragraph, Wrap},
    };

    let popup_area = centered_popup(area, 60, 42, 28, 6, 60, 9, 1);

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

    let msg = Paragraph::new(dialog.message.as_str())
        .style(Style::default().fg(t.text_normal))
        .wrap(Wrap { trim: true });
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
            let pct = pct.clamp(0.0, 1.0);
            let filled = ((pct * 10.0).round() as usize).min(10);
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
            let frames = [
                "\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}", "\u{2826}",
                "\u{2827}",
            ];
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
        // If capacity is zero, treat this as a no-op to avoid panicking on remove(0).
        if self.capacity == 0 {
            return;
        }
        if self.messages.len() >= self.capacity {
            self.messages.remove(0);
        }
        self.messages
            .push((message.into(), std::time::Instant::now()));
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
                Line::from(Span::styled(msg.clone(), Style::default().fg(t.text_muted)))
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

/// Return the full list of available commands with usage hints and
/// descriptions.
#[must_use]
pub fn command_hints() -> Vec<CommandHint> {
    command::command_entries()
        .iter()
        .map(|entry| CommandHint {
            prefix: entry.phrase,
            usage: entry.usage,
            description: entry.summary,
        })
        .collect()
}

/// Find the best matching command hint for the current input buffer.
#[must_use]
pub fn find_command_hint(input: &str) -> Option<&'static str> {
    command::find_command_hint(input).map(|hint| hint.usage)
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
    let popup_h = (item_count + 2)
        .min(area.height.saturating_sub(4))
        .max(3)
        .min(area.height);
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

    /// ASCII case-insensitive substring search: returns true if `needle`
    /// occurs in `haystack`, ignoring ASCII case.
    fn contains_ascii_case_insensitive(haystack: &str, needle: &str) -> bool {
        let haystack_bytes = haystack.as_bytes();
        let needle_bytes = needle.as_bytes();

        if needle_bytes.is_empty() {
            return true;
        }
        if needle_bytes.len() > haystack_bytes.len() {
            return false;
        }

        for window in haystack_bytes.windows(needle_bytes.len()) {
            let mut all_match = true;
            for (a, b) in window.iter().zip(needle_bytes.iter()) {
                if !a.eq_ignore_ascii_case(b) {
                    all_match = false;
                    break;
                }
            }
            if all_match {
                return true;
            }
        }

        false
    }

    /// Get filtered command hints matching the current filter text.
    #[must_use]
    pub fn filtered_commands(&self) -> Vec<&'static CommandHint> {
        static HINTS: std::sync::LazyLock<Vec<CommandHint>> =
            std::sync::LazyLock::new(command_hints);

        // Fast path: no filter, return all hints.
        if self.filter.is_empty() {
            HINTS.iter().collect()
        } else {
            // Since prefixes/descriptions are ASCII, we can safely use ASCII-only
            // lowercasing and a custom case-insensitive `contains` that avoids
            // per-item allocations.
            let needle = self.filter.to_ascii_lowercase();
            HINTS
                .iter()
                .filter(|h| {
                    Self::contains_ascii_case_insensitive(h.prefix, &needle)
                        || Self::contains_ascii_case_insensitive(h.description, &needle)
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
    let popup_h = (item_count + 4)
        .min(area.height * 72 / 100)
        .max(10)
        .min(area.height);
    let popup_w = 96_u16.min(area.width); // Fixed 96-wide
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
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

    // Filter input: `: ` prefix in magenta bold, text in bright, `█` cursor in magenta
    let filter_line = Line::from(vec![
        Span::styled(
            ": ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{}█", palette.filter),
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
            let is_selected = i == palette.selected;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(t.text_accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan) // cyan for commands
            };
            let desc_style = if is_selected {
                Style::default().fg(Color::Black).bg(t.text_accent)
            } else {
                Style::default().fg(t.text_muted)
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {:<20}", hint.prefix), style),
                Span::styled(hint.description, desc_style),
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
                " Command mode ",
                Style::default()
                    .fg(t.text_bright)
                    .add_modifier(Modifier::BOLD),
            ))
            .border_style(Style::default().fg(Color::Magenta)) // magenta border
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

// ─── Multi-option selector (checkbox list) ──────────────────────────────────

/// An item in a multi-option selector with a checked state.
pub struct MultiOptionItem {
    /// Display label.
    pub label: String,
    /// Whether this item is currently checked/selected.
    pub checked: bool,
}

impl MultiOptionItem {
    /// Create a new unchecked item.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            checked: false,
        }
    }
}

/// State for a multi-option selector (checkbox list).
pub struct MultiOptionSelector {
    /// Title for the selector.
    pub title: String,
    /// Available items with check state.
    pub items: Vec<MultiOptionItem>,
    /// Currently focused item index.
    pub focused: usize,
    /// Whether the selector is visible.
    pub visible: bool,
}

impl MultiOptionSelector {
    /// Create a new multi-option selector.
    #[must_use]
    pub fn new(title: impl Into<String>, labels: Vec<String>) -> Self {
        Self {
            title: title.into(),
            items: labels.into_iter().map(MultiOptionItem::new).collect(),
            focused: 0,
            visible: false,
        }
    }

    /// Move focus up.
    pub fn focus_previous(&mut self) {
        if self.focused > 0 {
            self.focused -= 1;
        }
    }

    /// Move focus down.
    pub fn focus_next(&mut self) {
        if self.focused + 1 < self.items.len() {
            self.focused += 1;
        }
    }

    /// Toggle the check state of the focused item.
    pub fn toggle_focused(&mut self) {
        if let Some(item) = self.items.get_mut(self.focused) {
            item.checked = !item.checked;
        }
    }

    /// Get labels of all checked items.
    #[must_use]
    pub fn checked_labels(&self) -> Vec<&str> {
        self.items
            .iter()
            .filter(|i| i.checked)
            .map(|i| i.label.as_str())
            .collect()
    }

    /// Select all items.
    pub fn select_all(&mut self) {
        for item in &mut self.items {
            item.checked = true;
        }
    }

    /// Deselect all items.
    pub fn deselect_all(&mut self) {
        for item in &mut self.items {
            item.checked = false;
        }
    }
}

/// Render a multi-option selector as a popup overlay.
pub fn render_multi_option(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    selector: &MultiOptionSelector,
    t: &Theme,
) {
    use ratatui::widgets::{Clear, List, ListItem, ListState};

    let item_count = selector.items.len() as u16;
    let popup_h = (item_count + 3)
        .min(area.height.saturating_sub(4))
        .max(5)
        .min(area.height);
    let popup_w = (area.width * 45 / 100).clamp(25.min(area.width), 50.min(area.width));
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup_area = ratatui::layout::Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup_area);

    let items: Vec<ListItem> = selector
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let check = if item.checked { "\u{2611}" } else { "\u{2610}" };
            let style = if i == selector.focused {
                Style::default()
                    .fg(Color::Black)
                    .bg(t.text_accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.text_normal)
            };
            ListItem::new(Span::styled(format!(" {check} {} ", item.label), style))
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
    list_state.select(Some(selector.focused));
    frame.render_stateful_widget(list, popup_area, &mut list_state);
}

// ─── Inline command hint ────────────────────────────────────────────────────

/// Render an inline usage hint for the command bar.
/// Returns a styled line showing the current command hint below the input.
#[must_use]
pub fn render_inline_hint(input: &str, t: &Theme) -> Option<Line<'static>> {
    find_command_hint(input).map(|usage| {
        Line::from(vec![
            Span::styled("  Usage: ", Style::default().fg(t.text_muted)),
            Span::styled(usage.to_string(), Style::default().fg(t.text_accent)),
        ])
    })
}

// ─── Notification area (managed queue) ──────────────────────────────────────

/// A managed notification queue with auto-expiry support.
pub struct NotificationArea {
    /// Active notifications.
    pub notifications: Vec<Notification>,
    /// Maximum number of visible notifications.
    pub max_visible: usize,
    /// How long notifications are kept (seconds).
    pub ttl_secs: u64,
}

impl NotificationArea {
    /// Create a new notification area.
    #[must_use]
    pub fn new(max_visible: usize, ttl_secs: u64) -> Self {
        Self {
            notifications: Vec::new(),
            max_visible,
            ttl_secs,
        }
    }

    /// Push a notification, evicting the oldest if at capacity.
    pub fn push(&mut self, notification: Notification) {
        if self.notifications.len() >= self.max_visible {
            self.notifications.remove(0);
        }
        self.notifications.push(notification);
    }

    /// Remove expired notifications.
    pub fn prune_expired(&mut self) {
        let ttl = std::time::Duration::from_secs(self.ttl_secs);
        self.notifications.retain(|n| n.created_at.elapsed() < ttl);
    }

    /// Push a convenience info notification.
    pub fn info(&mut self, msg: impl Into<String>) {
        self.push(Notification::new(msg, NotificationLevel::Info));
    }

    /// Push a convenience warning notification.
    pub fn warn(&mut self, msg: impl Into<String>) {
        self.push(Notification::new(msg, NotificationLevel::Warning));
    }

    /// Push a convenience error notification.
    pub fn error(&mut self, msg: impl Into<String>) {
        self.push(Notification::new(msg, NotificationLevel::Error));
    }

    /// Push a convenience success notification.
    pub fn success(&mut self, msg: impl Into<String>) {
        self.push(Notification::new(msg, NotificationLevel::Success));
    }

    /// Render the notification area.
    #[must_use]
    pub fn render(&self, t: &Theme) -> Vec<Line<'static>> {
        render_notifications(&self.notifications, t)
    }
}

// ─── Gauge bar ──────────────────────────────────────────────────────────────

/// A simple gauge bar widget for displaying resource levels inline.
pub struct GaugeBar {
    /// Current value.
    pub value: f64,
    /// Maximum value.
    pub max: f64,
    /// Width of the bar in characters.
    pub width: usize,
    /// Label shown to the left (optional).
    pub label: Option<String>,
    /// Whether to render an ASCII-safe bar.
    pub ascii_safe: bool,
}

impl GaugeBar {
    /// Create a new gauge bar.
    #[must_use]
    pub fn new(value: f64, max: f64, width: usize) -> Self {
        Self {
            value,
            max,
            width,
            label: None,
            ascii_safe: false,
        }
    }

    /// Set a label for the gauge.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Render with plain ASCII glyphs instead of Unicode block glyphs.
    #[must_use]
    pub fn ascii_safe(mut self, ascii_safe: bool) -> Self {
        self.ascii_safe = ascii_safe;
        self
    }

    /// Get the fill ratio (0.0..=1.0).
    #[must_use]
    pub fn ratio(&self) -> f64 {
        if self.max <= 0.0 {
            0.0
        } else {
            (self.value / self.max).clamp(0.0, 1.0)
        }
    }
}

/// Render a gauge bar as a styled line.
#[must_use]
pub fn render_gauge_bar(gauge: &GaugeBar, filled_color: Color, t: &Theme) -> Line<'static> {
    let ratio = gauge.ratio();
    let filled = (ratio * gauge.width as f64).round() as usize;
    let empty = gauge.width.saturating_sub(filled);
    let bar = gauge_bar_string(filled, empty, gauge.ascii_safe);

    let mut spans = Vec::new();
    if let Some(ref label) = gauge.label {
        spans.push(Span::styled(
            format!("{label} "),
            Style::default().fg(t.text_muted),
        ));
    }
    spans.push(Span::styled(bar, Style::default().fg(filled_color)));
    spans.push(Span::styled(
        format!(" {:.0}%", ratio * 100.0),
        Style::default().fg(t.text_secondary),
    ));

    Line::from(spans)
}

/// Render a compact cast strip with width-aware ASCII fallbacks.
#[must_use]
pub fn render_cast_bar(
    cast: &CastDisplay,
    available_width: usize,
    label_color: Color,
    filled_color: Color,
    meta_color: Color,
    dim_color: Color,
) -> Line<'static> {
    const MIN_LABEL_BUDGET_COMPACT: usize = 2;
    const MIN_LABEL_BUDGET_MEDIUM: usize = 4;
    const MIN_LABEL_BUDGET_WIDE: usize = 8;

    let compact = available_width < 34;
    let medium = (34..52).contains(&available_width);
    let wide = available_width >= 52;
    let ascii_safe = compact;
    let label_prefix = if compact { "Cast:" } else { "Cast" };
    let bar_width_target = if compact {
        8
    } else if medium {
        10
    } else {
        14
    };
    let minimum_bar_width = 4;
    let max_bar_width = available_width
        .saturating_sub(label_prefix.chars().count().saturating_add(5))
        .max(minimum_bar_width);
    let bar_width = bar_width_target.min(max_bar_width).max(minimum_bar_width);
    let suffix_source = if wide {
        match (
            cast.elapsed_secs,
            cast.remaining_secs,
            cast.status_text.as_deref(),
        ) {
            (Some(elapsed), Some(remaining), Some(status)) => {
                format!("{elapsed:.1}s/{remaining:.1}s {status}")
            }
            (Some(_), Some(remaining), None) => format!("{remaining:.1}s"),
            (_, _, Some(status)) => status.to_string(),
            _ => String::from("casting"),
        }
    } else {
        cast.remaining_secs
            .map(|remaining| format!("{remaining:.1}s"))
            .or_else(|| cast.status_text.clone())
            .unwrap_or_else(|| String::from("casting"))
    };
    let total_text_budget = available_width
        .saturating_sub(label_prefix.chars().count())
        .saturating_sub(bar_width)
        .saturating_sub(3);
    let label_hint = if compact {
        2
    } else if medium {
        6
    } else {
        12
    };
    let mut suffix_budget = if wide {
        total_text_budget.saturating_sub(label_hint + 1).min(22)
    } else if medium {
        total_text_budget.saturating_sub(label_hint + 1).min(10)
    } else {
        total_text_budget.saturating_sub(label_hint + 1).min(6)
    };
    let mut label_budget = total_text_budget
        .saturating_sub(if suffix_budget > 0 {
            suffix_budget + 1
        } else {
            0
        })
        .max(MIN_LABEL_BUDGET_COMPACT);
    let minimum_label_budget = if compact {
        MIN_LABEL_BUDGET_COMPACT
    } else if medium {
        MIN_LABEL_BUDGET_MEDIUM
    } else {
        MIN_LABEL_BUDGET_WIDE
    };
    if label_budget < minimum_label_budget && suffix_budget > 0 {
        let shift = (minimum_label_budget - label_budget).min(suffix_budget);
        suffix_budget = suffix_budget.saturating_sub(shift);
        label_budget += shift;
    }
    let preferred_label =
        cast.preferred_label(compact || (medium && cast.label.chars().count() > 12));
    let label = truncate_inline(preferred_label, label_budget);
    let suffix = truncate_inline(&suffix_source, suffix_budget);

    let gauge = GaugeBar::new(cast.progress * 100.0, 100.0, bar_width).ascii_safe(ascii_safe);
    let filled = (gauge.ratio() * gauge.width as f64).round() as usize;
    let empty = gauge.width.saturating_sub(filled);
    let bar = gauge_bar_string(filled, empty, gauge.ascii_safe);

    let mut spans = vec![
        Span::styled(label_prefix, Style::default().fg(dim_color)),
        Span::raw(" "),
        Span::styled(
            label,
            Style::default()
                .fg(label_color)
                .add_modifier(if cast.exact {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
        Span::raw(" "),
    ];
    spans.push(Span::styled(bar, Style::default().fg(filled_color)));
    if !suffix.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(suffix, Style::default().fg(meta_color)));
    }

    Line::from(spans)
}

fn gauge_bar_string(filled: usize, empty: usize, ascii_safe: bool) -> String {
    if ascii_safe {
        format!("|{}{}|", "#".repeat(filled), "-".repeat(empty))
    } else {
        format!(
            "\u{2502}{}{}\u{2502}",
            "\u{2588}".repeat(filled),
            "\u{2591}".repeat(empty)
        )
    }
}

pub(crate) fn truncate_inline(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }

    match max_chars {
        0 => String::new(),
        1 | 2 => text.chars().take(max_chars).collect(),
        _ => {
            let mut truncated: String = text.chars().take(max_chars - 2).collect();
            truncated.push_str("..");
            truncated
        }
    }
}

// ─── Tooltip ────────────────────────────────────────────────────────────────

/// A tooltip that can be positioned near the cursor or a specific area.
pub struct Tooltip {
    /// The tooltip text.
    pub text: String,
    /// Position hint — anchor coordinates (x, y).
    pub anchor_x: u16,
    pub anchor_y: u16,
}

impl Tooltip {
    /// Create a new tooltip at the given position.
    #[must_use]
    pub fn new(text: impl Into<String>, anchor_x: u16, anchor_y: u16) -> Self {
        Self {
            text: text.into(),
            anchor_x,
            anchor_y,
        }
    }
}

/// Render a tooltip near the specified anchor point.
pub fn render_tooltip(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    tooltip: &Tooltip,
    t: &Theme,
) {
    use ratatui::widgets::{Clear, Paragraph};

    let text_len = tooltip.text.len() as u16 + 2;
    let w = text_len.min(area.width.saturating_sub(2));
    let h = 3u16;

    // Position below and to the right of anchor, falling back if near edges
    let x = tooltip.anchor_x.min(area.x + area.width.saturating_sub(w));
    let y = if tooltip.anchor_y + h + 1 < area.y + area.height {
        tooltip.anchor_y + 1
    } else {
        tooltip.anchor_y.saturating_sub(h)
    };

    let tooltip_area = ratatui::layout::Rect::new(x, y, w, h);
    frame.render_widget(Clear, tooltip_area);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            tooltip.text.clone(),
            Style::default().fg(t.text_normal),
        )))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(t.border_type)
                .border_style(Style::default().fg(t.text_muted))
                .style(Style::default().bg(t.help_bg)),
        ),
        tooltip_area,
    );
}

// ─── Tab bar ────────────────────────────────────────────────────────────────

/// A reusable tab bar widget for switching between sections.
pub struct TabBar {
    /// Tab labels.
    pub tabs: Vec<String>,
    /// Currently active tab index.
    pub active: usize,
}

impl TabBar {
    /// Create a new tab bar with the given labels.
    #[must_use]
    pub fn new(tabs: Vec<String>) -> Self {
        Self { tabs, active: 0 }
    }

    /// Select the next tab (wraps around).
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
        }
    }

    /// Select the previous tab (wraps around).
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active = if self.active == 0 {
                self.tabs.len() - 1
            } else {
                self.active - 1
            };
        }
    }

    /// Get the active tab label.
    #[must_use]
    pub fn active_label(&self) -> Option<&str> {
        self.tabs.get(self.active).map(String::as_str)
    }
}

/// Render a tab bar as a styled line.
#[must_use]
pub fn render_tab_bar(tab_bar: &TabBar, t: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, label) in tab_bar.tabs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" ", Style::default().fg(t.text_muted)));
        }
        let style = if i == tab_bar.active {
            t.tab_active
        } else {
            t.tab_inactive
        };
        spans.push(Span::styled(format!(" {label} "), style));
    }
    Line::from(spans)
}

// ─── Filter input ───────────────────────────────────────────────────────────

/// State for a text filter input field with live feedback.
pub struct FilterInput {
    /// Current filter text.
    pub text: String,
    /// Whether the filter is active (focused for typing).
    pub active: bool,
    /// Placeholder text shown when empty.
    pub placeholder: String,
    /// Number of items matching the current filter (for feedback).
    pub match_count: Option<usize>,
}

impl FilterInput {
    /// Create a new filter input with the given placeholder.
    #[must_use]
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            text: String::new(),
            active: false,
            placeholder: placeholder.into(),
            match_count: None,
        }
    }

    /// Clear the filter text.
    pub fn clear(&mut self) {
        self.text.clear();
        self.match_count = None;
    }

    /// Toggle active state.
    pub fn toggle(&mut self) {
        self.active = !self.active;
    }

    /// Check if the filter has any text.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// Render a filter input as a styled line.
#[must_use]
pub fn render_filter_input(filter: &FilterInput, t: &Theme) -> Line<'static> {
    let mut spans = Vec::new();

    if filter.active {
        spans.push(Span::styled(
            "\u{1f50d} ",
            Style::default().fg(t.text_accent),
        ));
    } else {
        spans.push(Span::styled(
            "\u{1f50d} ",
            Style::default().fg(t.text_muted),
        ));
    }

    if filter.text.is_empty() {
        spans.push(Span::styled(
            filter.placeholder.clone(),
            Style::default().fg(t.text_muted),
        ));
    } else {
        spans.push(Span::styled(
            filter.text.clone(),
            Style::default().fg(t.text_bright),
        ));
        if filter.active {
            spans.push(Span::styled("_", Style::default().fg(t.text_accent)));
        }
    }

    if let Some(count) = filter.match_count {
        spans.push(Span::styled(
            format!("  ({count} matches)"),
            Style::default().fg(t.text_secondary),
        ));
    }

    Line::from(spans)
}

// ─── Keybinding hint ────────────────────────────────────────────────────────

/// Render a single keybinding hint as a pair of styled spans.
#[must_use]
pub fn keybinding_hint<'a>(key: &'a str, description: &'a str, t: &Theme) -> Vec<Span<'a>> {
    vec![
        Span::styled(key, t.statusbar_key),
        Span::styled(format!(" {description}  "), t.statusbar_dim),
    ]
}

/// Render a row of keybinding hints.
#[must_use]
pub fn keybinding_row<'a>(bindings: &[(&'a str, &'a str)], t: &Theme) -> Line<'a> {
    let mut spans = Vec::new();
    for (key, desc) in bindings {
        spans.extend(keybinding_hint(key, desc, t));
    }
    Line::from(spans)
}

// ─── Divider / separator ────────────────────────────────────────────────────

/// Render a horizontal divider line with an optional centered label.
#[must_use]
pub fn divider(width: usize, label: Option<&str>, t: &Theme) -> Line<'static> {
    match label {
        Some(text) => {
            let text_len = text.len() + 2; // space padding
            let side = width.saturating_sub(text_len) / 2;
            let right_side = width.saturating_sub(text_len).saturating_sub(side);
            Line::from(vec![
                Span::styled("\u{2500}".repeat(side), Style::default().fg(t.text_muted)),
                Span::styled(format!(" {text} "), Style::default().fg(t.text_secondary)),
                Span::styled(
                    "\u{2500}".repeat(right_side),
                    Style::default().fg(t.text_muted),
                ),
            ])
        }
        None => Line::from(Span::styled(
            "\u{2500}".repeat(width),
            Style::default().fg(t.text_muted),
        )),
    }
}

// ─── Badge ──────────────────────────────────────────────────────────────────

/// Render a small inline badge (colored label).
#[must_use]
pub fn badge(text: &str, fg: Color, bg: Color) -> Span<'static> {
    Span::styled(
        format!(" {text} "),
        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
    )
}

/// Render a status badge using theme colors based on variant.
#[must_use]
pub fn status_badge(label: &str, variant: BadgeVariant, t: &Theme) -> Span<'static> {
    let (fg, bg) = match variant {
        BadgeVariant::Primary => (Color::Black, t.text_accent),
        BadgeVariant::Success => (Color::Black, t.hp_high),
        BadgeVariant::Warning => (Color::Black, t.text_highlight),
        BadgeVariant::Danger => (Color::Black, t.hp_low),
        BadgeVariant::Info => (Color::Black, t.mana_color),
        BadgeVariant::Muted => (t.text_bright, t.bar_empty),
    };
    badge(label, fg, bg)
}

/// Badge style variants.
pub enum BadgeVariant {
    /// Accent/primary color.
    Primary,
    /// Green/success.
    Success,
    /// Yellow/warning.
    Warning,
    /// Red/danger.
    Danger,
    /// Blue/info.
    Info,
    /// Dim/muted.
    Muted,
}

// ─── Scrollable list ────────────────────────────────────────────────────────

/// A stateful scrollable list with position tracking and viewport management.
pub struct ScrollableList {
    /// Total number of items.
    pub total: usize,
    /// Index of the currently selected item.
    pub selected: usize,
    /// Index of the first visible item (scroll offset).
    pub offset: usize,
    /// Number of visible rows in the viewport.
    pub viewport_height: usize,
}

impl ScrollableList {
    /// Create a new scrollable list.
    #[must_use]
    pub fn new(total: usize, viewport_height: usize) -> Self {
        Self {
            total,
            selected: 0,
            offset: 0,
            viewport_height,
        }
    }

    /// Move selection down, adjusting scroll offset if needed.
    pub fn select_next(&mut self) {
        if self.selected + 1 < self.total {
            self.selected += 1;
            if self.selected >= self.offset + self.viewport_height {
                self.offset = self.selected + 1 - self.viewport_height;
            }
        }
    }

    /// Move selection up, adjusting scroll offset if needed.
    pub fn select_previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            if self.selected < self.offset {
                self.offset = self.selected;
            }
        }
    }

    /// Jump to the first item.
    pub fn select_first(&mut self) {
        self.selected = 0;
        self.offset = 0;
    }

    /// Jump to the last item.
    pub fn select_last(&mut self) {
        if self.total > 0 {
            self.selected = self.total - 1;
            self.offset = self.total.saturating_sub(self.viewport_height);
        }
    }

    /// Page down — move viewport_height items forward.
    pub fn page_down(&mut self) {
        self.selected = (self.selected + self.viewport_height).min(self.total.saturating_sub(1));
        self.offset = self.selected.saturating_sub(self.viewport_height / 2);
    }

    /// Page up — move viewport_height items backward.
    pub fn page_up(&mut self) {
        self.selected = self.selected.saturating_sub(self.viewport_height);
        self.offset = self.selected.saturating_sub(self.viewport_height / 2);
    }

    /// Get the visible range of indices.
    #[must_use]
    pub fn visible_range(&self) -> std::ops::Range<usize> {
        let end = (self.offset + self.viewport_height).min(self.total);
        self.offset..end
    }

    /// Whether a scrollbar should be shown (more items than viewport).
    #[must_use]
    pub fn needs_scrollbar(&self) -> bool {
        self.total > self.viewport_height
    }

    /// Get the scroll position as a ratio (0.0..=1.0) for scrollbar rendering.
    #[must_use]
    pub fn scroll_ratio(&self) -> f64 {
        if self.total <= self.viewport_height {
            0.0
        } else {
            self.offset as f64 / (self.total - self.viewport_height) as f64
        }
    }
}

/// Render a vertical scrollbar indicator.
#[must_use]
pub fn render_scrollbar(height: u16, scroll_ratio: f64, t: &Theme) -> Vec<Span<'static>> {
    let usable = height as usize;
    let thumb_pos = (scroll_ratio * (usable.saturating_sub(1)) as f64).round() as usize;

    (0..usable)
        .map(|i| {
            if i == thumb_pos {
                Span::styled("\u{2588}", Style::default().fg(t.text_accent))
            } else {
                Span::styled("\u{2502}", Style::default().fg(t.text_muted))
            }
        })
        .collect()
}

// ─── Info panel (key-value detail view) ─────────────────────────────────────

/// A structured info panel that displays key-value pairs.
pub struct InfoPanel {
    /// Title of the panel.
    pub title: String,
    /// Key-value entries.
    pub entries: Vec<(String, String)>,
}

impl InfoPanel {
    /// Create a new info panel.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            entries: Vec::new(),
        }
    }

    /// Add a key-value entry.
    pub fn add(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.entries.push((key.into(), value.into()));
    }

    /// Builder method to add a key-value entry.
    #[must_use]
    pub fn with_entry(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.add(key, value);
        self
    }
}

/// Render an info panel as styled lines.
#[must_use]
pub fn render_info_panel(panel_data: &InfoPanel, t: &Theme) -> Vec<Line<'static>> {
    let key_width = panel_data
        .entries
        .iter()
        .map(|(k, _)| k.len())
        .max()
        .unwrap_or(0);

    let mut lines = vec![Line::from(Span::styled(
        panel_data.title.clone(),
        Style::default()
            .fg(t.text_accent)
            .add_modifier(Modifier::BOLD),
    ))];

    for (key, value) in &panel_data.entries {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {key:<width$}  ", width = key_width),
                Style::default().fg(t.text_muted),
            ),
            Span::styled(value.clone(), Style::default().fg(t.text_normal)),
        ]));
    }

    lines
}

// ─── Sparkline (inline mini chart) ──────────────────────────────────────────

/// A mini inline sparkline for showing trends.
pub struct Sparkline {
    /// Data points.
    pub data: Vec<f64>,
    /// Maximum value for scaling (auto-detected if None).
    pub max_val: Option<f64>,
}

impl Sparkline {
    /// Create a sparkline from data points.
    #[must_use]
    pub fn new(data: Vec<f64>) -> Self {
        Self {
            data,
            max_val: None,
        }
    }

    /// Set an explicit maximum value.
    #[must_use]
    pub fn with_max(mut self, max: f64) -> Self {
        self.max_val = Some(max);
        self
    }
}

/// Render a sparkline as a styled span using Unicode block elements.
#[must_use]
pub fn render_sparkline(spark: &Sparkline, color: Color) -> Span<'static> {
    const BLOCKS: [char; 8] = [
        ' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
    ];

    let max = spark
        .max_val
        .unwrap_or_else(|| spark.data.iter().copied().fold(f64::NEG_INFINITY, f64::max));

    let chars: String = if max <= 0.0 {
        spark.data.iter().map(|_| BLOCKS[0]).collect()
    } else {
        spark
            .data
            .iter()
            .map(|&v| {
                let ratio = (v / max).clamp(0.0, 1.0);
                let idx = (ratio * 7.0).round() as usize;
                BLOCKS[idx]
            })
            .collect()
    };

    Span::styled(chars, Style::default().fg(color))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::dark_modern;
    use ratatui::text::{Line, Span};

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
    fn cast_state_color_uses_theme_semantics() {
        use crate::tui::ui::ch_chain::CastState as ChainCastState;

        let t = dark_modern();
        assert_eq!(cast_state_color(&ChainCastState::Idle, &t), t.text_muted);
        assert_eq!(
            cast_state_color(&ChainCastState::Casting(0.90), &t),
            t.hp_high
        );
        assert_eq!(
            cast_state_color(&ChainCastState::Casting(0.50), &t),
            t.text_accent
        );
        assert_eq!(
            cast_state_color(&ChainCastState::Casting(0.10), &t),
            t.text_muted
        );
        assert_eq!(cast_state_color(&ChainCastState::Completed, &t), t.hp_high);
        assert_eq!(cast_state_color(&ChainCastState::Missed, &t), t.hp_low);
    }

    #[test]
    fn spans_width_uses_terminal_cell_width() {
        let spans = vec![Span::raw("A"), Span::raw("界")];
        assert_eq!(spans_width(&spans), 3);
    }

    #[test]
    fn line_width_uses_terminal_cell_width() {
        let line = Line::from(vec![Span::raw("A"), Span::raw("界")]);
        assert_eq!(line_width(&line), 3);
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
        assert_eq!(hint, Some("nav <camp_name|x y z|zone|reload>"));
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
        assert!(
            cmds.iter()
                .all(|c| c.prefix.contains("nav") || c.description.to_lowercase().contains("nav"))
        );
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

    #[test]
    fn multi_option_selector_navigation() {
        let mut sel = MultiOptionSelector::new("Pick", vec!["A".into(), "B".into(), "C".into()]);
        assert_eq!(sel.focused, 0);
        sel.focus_next();
        assert_eq!(sel.focused, 1);
        sel.toggle_focused();
        assert!(sel.items[1].checked);
        sel.toggle_focused();
        assert!(!sel.items[1].checked);
    }

    #[test]
    fn multi_option_checked_labels() {
        let mut sel = MultiOptionSelector::new("Pick", vec!["A".into(), "B".into(), "C".into()]);
        sel.items[0].checked = true;
        sel.items[2].checked = true;
        let labels = sel.checked_labels();
        assert_eq!(labels, vec!["A", "C"]);
    }

    #[test]
    fn multi_option_select_deselect_all() {
        let mut sel = MultiOptionSelector::new("Pick", vec!["X".into(), "Y".into()]);
        sel.select_all();
        assert!(sel.items.iter().all(|i| i.checked));
        sel.deselect_all();
        assert!(sel.items.iter().all(|i| !i.checked));
    }

    #[test]
    fn render_inline_hint_found() {
        let t = dark_modern();
        let line = render_inline_hint("nav zone1", &t);
        assert!(line.is_some());
    }

    #[test]
    fn render_inline_hint_not_found() {
        let t = dark_modern();
        let line = render_inline_hint("unknown_cmd", &t);
        assert!(line.is_none());
    }

    #[test]
    fn notification_area_push_and_prune() {
        let mut area = NotificationArea::new(3, 60);
        area.info("msg1");
        area.warn("msg2");
        area.error("msg3");
        area.success("msg4");
        assert_eq!(area.notifications.len(), 3);
        assert_eq!(area.notifications[0].message, "msg2");
    }

    #[test]
    fn notification_area_render() {
        let t = dark_modern();
        let mut area = NotificationArea::new(5, 60);
        area.info("hello");
        let lines = area.render(&t);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn gauge_bar_ratio() {
        let g = GaugeBar::new(50.0, 100.0, 10);
        assert!((g.ratio() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_bar_ratio_zero_max() {
        let g = GaugeBar::new(50.0, 0.0, 10);
        assert!((g.ratio() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_bar_with_label() {
        let g = GaugeBar::new(75.0, 100.0, 10).with_label("HP");
        assert_eq!(g.label, Some("HP".into()));
    }

    #[test]
    fn render_gauge_bar_has_spans() {
        let t = dark_modern();
        let g = GaugeBar::new(30.0, 100.0, 10);
        let line = render_gauge_bar(&g, t.hp_high, &t);
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn tooltip_creation() {
        let tip = Tooltip::new("Hover info", 10, 20);
        assert_eq!(tip.text, "Hover info");
        assert_eq!(tip.anchor_x, 10);
        assert_eq!(tip.anchor_y, 20);
    }

    #[test]
    fn tab_bar_navigation() {
        let mut tb = TabBar::new(vec!["A".into(), "B".into(), "C".into()]);
        assert_eq!(tb.active, 0);
        tb.next_tab();
        assert_eq!(tb.active, 1);
        tb.next_tab();
        assert_eq!(tb.active, 2);
        tb.next_tab();
        assert_eq!(tb.active, 0); // wrap
        tb.prev_tab();
        assert_eq!(tb.active, 2); // wrap back
    }

    #[test]
    fn tab_bar_active_label() {
        let tb = TabBar::new(vec!["First".into(), "Second".into()]);
        assert_eq!(tb.active_label(), Some("First"));
    }

    #[test]
    fn render_tab_bar_has_spans() {
        let t = dark_modern();
        let tb = TabBar::new(vec!["A".into(), "B".into()]);
        let line = render_tab_bar(&tb, &t);
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn filter_input_clear() {
        let mut fi = FilterInput::new("Search...");
        fi.text = "hello".into();
        fi.match_count = Some(5);
        fi.clear();
        assert!(fi.text.is_empty());
        assert!(fi.match_count.is_none());
    }

    #[test]
    fn filter_input_toggle() {
        let mut fi = FilterInput::new("Search...");
        assert!(!fi.active);
        fi.toggle();
        assert!(fi.active);
    }

    #[test]
    fn render_filter_input_placeholder() {
        let t = dark_modern();
        let fi = FilterInput::new("Type to search...");
        let line = render_filter_input(&fi, &t);
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn render_filter_input_with_text_and_count() {
        let t = dark_modern();
        let mut fi = FilterInput::new("Search...");
        fi.text = "orc".into();
        fi.match_count = Some(12);
        fi.active = true;
        let line = render_filter_input(&fi, &t);
        assert!(line.spans.len() >= 3);
    }

    #[test]
    fn render_cast_bar_compact_uses_ascii_bar() {
        let t = dark_modern();
        let cast = crate::tui::cast::CastDisplay::exact_progress("Complete Heal", "CH", 0.4, 10.0);
        let line = render_cast_bar(
            &cast,
            28,
            t.hp_high,
            t.text_accent,
            t.text_secondary,
            t.text_muted,
        );
        let rendered = render_line(&line);

        assert!(rendered.contains("Cast:"));
        assert!(rendered.contains("|"));
        assert!(rendered.contains("CH"));
        assert_eq!(gauge_width(&rendered), Some(8));
    }

    #[test]
    fn render_cast_bar_medium_keeps_fixed_gauge_width() {
        let t = dark_modern();
        let cast = crate::tui::cast::CastDisplay::exact_progress("Complete Heal", "CH", 0.4, 10.0);
        let line = render_cast_bar(
            &cast,
            42,
            t.hp_high,
            t.text_accent,
            t.text_secondary,
            t.text_muted,
        );
        let rendered = render_line(&line);

        assert!(rendered.contains("CH"));
        assert!(rendered.contains("6.0s"));
        assert_eq!(gauge_width(&rendered), Some(10));
    }

    #[test]
    fn render_cast_bar_wide_shows_elapsed_and_remaining_time() {
        let t = dark_modern();
        let cast = crate::tui::cast::CastDisplay::exact_progress("Complete Heal", "CH", 0.25, 10.0);
        let line = render_cast_bar(
            &cast,
            72,
            t.hp_high,
            t.text_accent,
            t.text_secondary,
            t.text_muted,
        );
        let rendered = render_line(&line);

        assert!(rendered.contains("Complete Heal"));
        assert!(rendered.contains("2.5s/7.5s"));
        assert_eq!(gauge_width(&rendered), Some(14));
    }

    #[test]
    fn keybinding_hint_has_two_spans() {
        let t = dark_modern();
        let spans = keybinding_hint("Tab", "switch pane", &t);
        assert_eq!(spans.len(), 2);
    }

    fn render_line(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    fn gauge_width(rendered: &str) -> Option<usize> {
        let chars: Vec<char> = rendered.chars().collect();
        if let Some(start) = chars.iter().position(|ch| *ch == '|')
            && let Some(end) = chars[start + 1..].iter().position(|ch| *ch == '|')
        {
            return Some(end);
        }
        if let Some(start) = chars.iter().position(|ch| *ch == '│')
            && let Some(end) = chars[start + 1..].iter().position(|ch| *ch == '│')
        {
            return Some(end);
        }
        None
    }

    #[test]
    fn keybinding_row_renders() {
        let t = dark_modern();
        let line = keybinding_row(&[("Tab", "pane"), ("?", "help")], &t);
        assert_eq!(line.spans.len(), 4);
    }

    #[test]
    fn divider_without_label() {
        let t = dark_modern();
        let line = divider(40, None, &t);
        assert_eq!(line.spans.len(), 1);
    }

    #[test]
    fn divider_with_label() {
        let t = dark_modern();
        let line = divider(40, Some("Section"), &t);
        assert_eq!(line.spans.len(), 3);
    }

    #[test]
    fn badge_variant_primary() {
        let t = dark_modern();
        let b = status_badge("OK", BadgeVariant::Primary, &t);
        assert!(!b.content.is_empty());
    }

    #[test]
    fn badge_variant_danger() {
        let t = dark_modern();
        let b = status_badge("ERR", BadgeVariant::Danger, &t);
        assert!(!b.content.is_empty());
    }

    #[test]
    fn scrollable_list_navigation() {
        let mut sl = ScrollableList::new(20, 5);
        assert_eq!(sl.selected, 0);
        assert_eq!(sl.offset, 0);
        for _ in 0..6 {
            sl.select_next();
        }
        assert_eq!(sl.selected, 6);
        assert!(sl.offset > 0);
    }

    #[test]
    fn scrollable_list_select_first_last() {
        let mut sl = ScrollableList::new(20, 5);
        sl.select_last();
        assert_eq!(sl.selected, 19);
        sl.select_first();
        assert_eq!(sl.selected, 0);
        assert_eq!(sl.offset, 0);
    }

    #[test]
    fn scrollable_list_page_down_up() {
        let mut sl = ScrollableList::new(100, 10);
        sl.page_down();
        assert_eq!(sl.selected, 10);
        sl.page_up();
        assert_eq!(sl.selected, 0);
    }

    #[test]
    fn scrollable_list_visible_range() {
        let sl = ScrollableList::new(20, 5);
        assert_eq!(sl.visible_range(), 0..5);
    }

    #[test]
    fn scrollable_list_needs_scrollbar() {
        let sl = ScrollableList::new(20, 5);
        assert!(sl.needs_scrollbar());
        let sl2 = ScrollableList::new(3, 5);
        assert!(!sl2.needs_scrollbar());
    }

    #[test]
    fn scrollable_list_scroll_ratio() {
        let sl = ScrollableList::new(20, 5);
        assert!((sl.scroll_ratio() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn render_scrollbar_has_entries() {
        let t = dark_modern();
        let spans = render_scrollbar(10, 0.5, &t);
        assert_eq!(spans.len(), 10);
    }

    #[test]
    fn info_panel_builder() {
        let p = InfoPanel::new("Player")
            .with_entry("Name", "Warrior")
            .with_entry("Level", "50");
        assert_eq!(p.entries.len(), 2);
        assert_eq!(p.title, "Player");
    }

    #[test]
    fn render_info_panel_lines() {
        let t = dark_modern();
        let p = InfoPanel::new("Stats")
            .with_entry("HP", "1000")
            .with_entry("Mana", "500");
        let lines = render_info_panel(&p, &t);
        assert_eq!(lines.len(), 3); // title + 2 entries
    }

    #[test]
    fn sparkline_creation() {
        let s = Sparkline::new(vec![1.0, 2.0, 3.0, 2.0, 1.0]);
        assert_eq!(s.data.len(), 5);
        assert!(s.max_val.is_none());
    }

    #[test]
    fn sparkline_with_max() {
        let s = Sparkline::new(vec![1.0, 2.0]).with_max(10.0);
        assert_eq!(s.max_val, Some(10.0));
    }

    #[test]
    fn render_sparkline_chars() {
        let t = dark_modern();
        let s = Sparkline::new(vec![0.0, 0.5, 1.0, 0.5, 0.0]).with_max(1.0);
        let span = render_sparkline(&s, t.text_accent);
        assert_eq!(span.content.chars().count(), 5);
    }
}
