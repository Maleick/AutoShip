//! Theme system for the Frostreaver TUI.
//!
//! A `Theme` carries every semantic color and style used by the renderer.
//! All `draw_*` functions read colors from `app.theme` instead of using
//! inline `Color::*` literals, making it trivial to add new themes.

use ratatui::{
    style::{Color, Modifier, Style},
    widgets::BorderType,
};

/// All semantic colors/styles consumed by the UI renderer.
#[derive(Debug, Clone)]
pub struct Theme {
    // ── Border chrome ────────────────────────────────────────────────
    /// Unicode border character set (Rounded vs Plain).
    pub border_type: BorderType,
    /// Default panel borders — inactive, low-emphasis.
    pub border_dim: Style,
    /// Primary panel borders — main content areas (e.g., character grid).
    pub border_primary: Style,
    /// Active / focused panel border.
    pub border_active: Style,
    /// Warning/combat emphasis border.
    pub border_warn: Style,
    /// Danger/target border (red).
    pub border_danger: Style,
    /// Magenta/server-info border.
    pub border_server: Style,

    // ── Tab bar ──────────────────────────────────────────────────────
    pub tab_active: Style,
    pub tab_inactive: Style,

    // ── Text ─────────────────────────────────────────────────────────
    /// Bright foreground text.
    pub text_bright: Color,
    /// Normal body text.
    pub text_normal: Color,
    /// Secondary / label text.
    pub text_secondary: Color,
    /// Muted / dim text.
    pub text_muted: Color,
    /// Primary accent text (cyan family).
    pub text_accent: Color,
    /// Secondary accent text (gold/yellow family).
    pub text_highlight: Color,
    /// Server name color.
    pub text_server: Color,

    // ── HP / resource bars ───────────────────────────────────────────
    pub hp_high: Color,
    pub hp_mid: Color,
    pub hp_low: Color,
    pub mana_color: Color,
    /// Empty portion of a bar.
    pub bar_empty: Color,

    // ── Spawn type colors ────────────────────────────────────────────
    pub spawn_pc: Color,
    pub spawn_npc: Color,
    /// Named NPC (non-trivial mob name).
    pub spawn_named: Color,
    pub spawn_corpse: Color,
    pub spawn_unknown: Color,

    // ── Tables ───────────────────────────────────────────────────────
    pub table_header: Style,
    /// Selected / highlighted row background.
    pub row_selected_bg: Color,

    // ── Stand-state colors ───────────────────────────────────────────
    pub state_dead: Color,
    pub state_sitting: Color,
    pub state_feigned: Color,
    pub state_frozen: Color,
    pub state_normal: Color,

    // ── Operating-mode colors ────────────────────────────────────────
    pub mode_camp: Color,
    pub mode_hunt: Color,

    // ── Status bar ───────────────────────────────────────────────────
    pub statusbar_message: Style,
    pub statusbar_key: Style,
    pub statusbar_dim: Style,
    pub statusbar_cmd: Style,
    pub statusbar_badge: Style,

    // ── Map overlay ──────────────────────────────────────────────────
    pub map_you: Color,
    pub map_pc: Color,
    pub map_npc: Color,
    pub map_named: Color,
    pub map_dead_named: Color,
    pub map_corpse: Color,
    pub map_lines: Color,

    // ── Header ───────────────────────────────────────────────────────
    pub header_title: Style,
    pub header_client_count: Style,
    pub header_selected: Style,
    pub header_zone: Style,
    pub header_group: Style,
    pub header_group_active: Style,
}

// ─── Dark Modern ────────────────────────────────────────────────────────────

/// A polished dark theme using RGB colors and rounded borders.
pub fn dark_modern() -> Theme {
    let accent = Color::Rgb(0, 200, 210); // teal-cyan
    let gold = Color::Rgb(240, 185, 40); // warm gold
    let green = Color::Rgb(80, 210, 100); // vivid green
    let red = Color::Rgb(220, 60, 60); // clear red
    let orange = Color::Rgb(240, 130, 30); // warning orange
    let purple = Color::Rgb(180, 100, 240); // soft purple / server
    let white = Color::Rgb(230, 230, 230); // near-white
    let mid = Color::Rgb(160, 160, 170); // mid gray
    let dim = Color::Rgb(85, 90, 100); // dim gray
    let muted = Color::Rgb(55, 60, 70); // very dim
    let blue = Color::Rgb(80, 130, 220); // mana blue

    Theme {
        border_type: BorderType::Rounded,

        border_dim: Style::default().fg(dim),
        border_primary: Style::default().fg(green),
        border_active: Style::default().fg(accent),
        border_warn: Style::default().fg(gold),
        border_danger: Style::default().fg(red),
        border_server: Style::default().fg(purple),

        tab_active: Style::default()
            .fg(Color::Black)
            .bg(accent)
            .add_modifier(Modifier::BOLD),
        tab_inactive: Style::default().fg(dim),

        text_bright: white,
        text_normal: white,
        text_secondary: mid,
        text_muted: dim,
        text_accent: accent,
        text_highlight: gold,
        text_server: purple,

        hp_high: green,
        hp_mid: gold,
        hp_low: red,
        mana_color: blue,
        bar_empty: muted,

        spawn_pc: green,
        spawn_npc: white,
        spawn_named: gold,
        spawn_corpse: dim,
        spawn_unknown: red,

        table_header: Style::default().fg(accent).add_modifier(Modifier::BOLD),
        row_selected_bg: Color::Rgb(35, 45, 55),

        state_dead: red,
        state_sitting: gold,
        state_feigned: purple,
        state_frozen: blue,
        state_normal: green,

        mode_camp: green,
        mode_hunt: orange,

        statusbar_message: Style::default().fg(gold).add_modifier(Modifier::BOLD),
        statusbar_key: Style::default().fg(accent),
        statusbar_dim: Style::default().fg(dim),
        statusbar_cmd: Style::default().fg(accent).add_modifier(Modifier::BOLD),
        statusbar_badge: Style::default()
            .fg(Color::Black)
            .bg(gold)
            .add_modifier(Modifier::BOLD),

        map_you: accent,
        map_pc: green,
        map_npc: white,
        map_named: gold,
        map_dead_named: red,
        map_corpse: dim,
        map_lines: muted,

        header_title: Style::default().fg(accent).add_modifier(Modifier::BOLD),
        header_client_count: Style::default().fg(gold).add_modifier(Modifier::BOLD),
        header_selected: Style::default().fg(green).add_modifier(Modifier::BOLD),
        header_zone: Style::default().fg(white),
        header_group: Style::default().fg(dim),
        header_group_active: Style::default().fg(accent).add_modifier(Modifier::BOLD),
    }
}

// ─── Classic ────────────────────────────────────────────────────────────────

/// Classic terminal theme using named colors and plain borders.
pub fn classic() -> Theme {
    Theme {
        border_type: BorderType::Plain,

        border_dim: Style::default().fg(Color::DarkGray),
        border_primary: Style::default().fg(Color::Green),
        border_active: Style::default().fg(Color::Cyan),
        border_warn: Style::default().fg(Color::Yellow),
        border_danger: Style::default().fg(Color::Red),
        border_server: Style::default().fg(Color::Magenta),

        tab_active: Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        tab_inactive: Style::default().fg(Color::DarkGray),

        text_bright: Color::White,
        text_normal: Color::White,
        text_secondary: Color::Gray,
        text_muted: Color::DarkGray,
        text_accent: Color::Cyan,
        text_highlight: Color::Yellow,
        text_server: Color::Magenta,

        hp_high: Color::Green,
        hp_mid: Color::Yellow,
        hp_low: Color::Red,
        mana_color: Color::Blue,
        bar_empty: Color::DarkGray,

        spawn_pc: Color::Green,
        spawn_npc: Color::White,
        spawn_named: Color::Yellow,
        spawn_corpse: Color::DarkGray,
        spawn_unknown: Color::Red,

        table_header: Style::default().add_modifier(Modifier::BOLD),
        row_selected_bg: Color::DarkGray,

        state_dead: Color::Red,
        state_sitting: Color::Yellow,
        state_feigned: Color::Magenta,
        state_frozen: Color::Blue,
        state_normal: Color::Green,

        mode_camp: Color::Green,
        mode_hunt: Color::Yellow,

        statusbar_message: Style::default().fg(Color::Yellow),
        statusbar_key: Style::default().fg(Color::Cyan),
        statusbar_dim: Style::default().fg(Color::DarkGray),
        statusbar_cmd: Style::default().fg(Color::Cyan),
        statusbar_badge: Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD),

        map_you: Color::LightCyan,
        map_pc: Color::Green,
        map_npc: Color::White,
        map_named: Color::Yellow,
        map_dead_named: Color::Red,
        map_corpse: Color::DarkGray,
        map_lines: Color::DarkGray,

        header_title: Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        header_client_count: Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        header_selected: Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        header_zone: Style::default().fg(Color::White),
        header_group: Style::default().fg(Color::DarkGray),
        header_group_active: Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    }
}

// ─── ThemeKind ───────────────────────────────────────────────────────────────

/// Enum so the app can store which theme is active and cycle through them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeKind {
    #[default]
    DarkModern,
    Classic,
}

impl ThemeKind {
    pub fn next(self) -> Self {
        match self {
            Self::DarkModern => Self::Classic,
            Self::Classic => Self::DarkModern,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::DarkModern => "Dark",
            Self::Classic => "Classic",
        }
    }

    pub fn build(self) -> Theme {
        match self {
            Self::DarkModern => dark_modern(),
            Self::Classic => classic(),
        }
    }
}
