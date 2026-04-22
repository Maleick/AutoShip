//! Theme system for the TextQuest TUI.
//!
//! A `Theme` carries every semantic color and style used by the renderer.
//! All `draw_*` functions read colors from `app.theme` instead of using
//! inline `Color::*` literals, making it trivial to add new themes.
//!
//! Use [`loader`] to load themes from TOML files at runtime.

/// TOML theme loader — parse `config/themes/*.toml` into [`Theme`] values.
pub mod loader;

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
    /// Style for the active/selected screen tab.
    pub tab_active: Style,
    /// Style for inactive screen tabs.
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
    /// HP bar color when above 75%.
    pub hp_high: Color,
    /// HP bar color when between 25-75%.
    pub hp_mid: Color,
    /// HP bar color when below 25%.
    pub hp_low: Color,
    /// Mana bar color.
    pub mana_color: Color,
    /// Empty portion of a bar.
    pub bar_empty: Color,

    // ── Spawn type colors ────────────────────────────────────────────
    /// Color for player character spawns.
    pub spawn_pc: Color,
    /// Color for regular NPC spawns.
    pub spawn_npc: Color,
    /// Named NPC (non-trivial mob name).
    pub spawn_named: Color,
    /// Color for corpse spawns.
    pub spawn_corpse: Color,
    /// Color for unknown spawn types.
    pub spawn_unknown: Color,

    // ── Tables ───────────────────────────────────────────────────────
    /// Style for table column headers.
    pub table_header: Style,
    /// Selected / highlighted row background.
    pub row_selected_bg: Color,

    // ── Stand-state colors ───────────────────────────────────────────
    /// Color for dead characters.
    pub state_dead: Color,
    /// Color for sitting characters.
    pub state_sitting: Color,
    /// Color for feign-death characters.
    pub state_feigned: Color,
    /// Color for frozen/stunned characters.
    pub state_frozen: Color,
    /// Color for standing (normal) characters.
    pub state_normal: Color,

    // ── Operating-mode colors ────────────────────────────────────────
    /// Color for camp mode indicator.
    pub mode_camp: Color,
    /// Color for hunt mode indicator.
    pub mode_hunt: Color,

    // ── Status bar ───────────────────────────────────────────────────
    /// Style for status bar messages.
    pub statusbar_message: Style,
    /// Style for keyboard shortcut hints in the status bar.
    pub statusbar_key: Style,
    /// Dim style for low-priority status bar text.
    pub statusbar_dim: Style,
    /// Style for the command mode indicator.
    pub statusbar_cmd: Style,
    /// Badge style for highlighted status items.
    pub statusbar_badge: Style,

    // ── Map overlay ──────────────────────────────────────────────────
    /// Color for the player's own position on the map.
    pub map_you: Color,
    /// Color for other PCs on the map.
    pub map_pc: Color,
    /// Color for group members on the map.
    pub map_group: Color,
    /// Color for NPCs on the map.
    pub map_npc: Color,
    /// Color for named mobs on the map.
    pub map_named: Color,
    /// Color for dead named mobs on the map.
    pub map_dead_named: Color,
    /// Color for corpses on the map.
    pub map_corpse: Color,
    /// Color for zone geometry lines on the map.
    pub map_lines: Color,
    /// Brighter color for (0,0,0) map geometry (walls/terrain).
    pub map_geometry: Color,

    // ── Header ───────────────────────────────────────────────────────
    /// Style for the main title in the header bar.
    pub header_title: Style,
    /// Style for the connected client count display.
    pub header_client_count: Style,
    /// Style for the selected client indicator.
    pub header_selected: Style,
    /// Style for the zone name in the header.
    pub header_zone: Style,
    /// Style for inactive group labels in the header.
    pub header_group: Style,
    /// Style for the actively focused group label.
    pub header_group_active: Style,

    // ── Help overlay ─────────────────────────────────────────────────
    /// Style for keyboard shortcut keys in the help overlay.
    pub help_key: Style,
    /// Style for help description text.
    pub help_desc: Style,
    /// Style for section headings in the help overlay.
    pub help_heading: Style,
    /// Dim style for secondary help text.
    pub help_dim: Style,
    /// Background color for the help overlay.
    pub help_bg: Color,
    /// Border style for the help overlay panel.
    pub help_border: Style,

    // ── Con colors (level-relative mob difficulty) ────────────────────
    /// Con color for dangerous mobs (red con).
    pub con_red: Color,
    /// Con color for even-level mobs (yellow con).
    pub con_yellow: Color,
    /// Con color for slightly below-level mobs (white con).
    pub con_white: Color,
    /// Con color for below-level mobs (light blue con).
    pub con_light_blue: Color,
    /// Con color for trivial mobs (blue con).
    pub con_blue: Color,
    /// Con color for very low mobs (green con).
    pub con_green: Color,
}

// ─── Dark Modern ────────────────────────────────────────────────────────────

/// A polished dark theme using RGB colors and rounded borders.
#[must_use]
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
        map_group: Color::Cyan,
        map_npc: white,
        map_named: gold,
        map_dead_named: red,
        map_corpse: dim,
        map_lines: muted,
        map_geometry: Color::Rgb(90, 100, 115),

        header_title: Style::default().fg(accent).add_modifier(Modifier::BOLD),
        header_client_count: Style::default().fg(gold).add_modifier(Modifier::BOLD),
        header_selected: Style::default().fg(green).add_modifier(Modifier::BOLD),
        header_zone: Style::default().fg(white),
        header_group: Style::default().fg(dim),
        header_group_active: Style::default().fg(accent).add_modifier(Modifier::BOLD),

        help_key: Style::default().fg(accent),
        help_desc: Style::default().fg(Color::Rgb(180, 180, 190)),
        help_heading: Style::default().fg(accent).add_modifier(Modifier::BOLD),
        help_dim: Style::default().fg(dim),
        help_bg: Color::Rgb(15, 18, 24),
        help_border: Style::default().fg(accent),

        con_red: Color::Red,
        con_yellow: Color::Yellow,
        con_white: Color::White,
        con_light_blue: Color::LightCyan,
        con_blue: Color::Blue,
        con_green: Color::Green,
    }
}

// ─── Classic ────────────────────────────────────────────────────────────────

/// Classic terminal theme using named colors and plain borders.
#[must_use]
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
        map_group: Color::Cyan,
        map_npc: Color::White,
        map_named: Color::Yellow,
        map_dead_named: Color::Red,
        map_corpse: Color::DarkGray,
        map_lines: Color::DarkGray,
        map_geometry: Color::Gray,

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

        help_key: Style::default().fg(Color::Cyan),
        help_desc: Style::default().fg(Color::Gray),
        help_heading: Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        help_dim: Style::default().fg(Color::DarkGray),
        help_bg: Color::Black,
        help_border: Style::default().fg(Color::Cyan),

        con_red: Color::Red,
        con_yellow: Color::Yellow,
        con_white: Color::White,
        con_light_blue: Color::LightCyan,
        con_blue: Color::Blue,
        con_green: Color::Green,
    }
}

// ─── Dracula ────────────────────────────────────────────────────────────────

/// Dracula color scheme — dark purples, pinks, and vivid accents.
/// Based on <https://draculatheme.com/contribute#color-palette>.
#[must_use]
pub fn dracula() -> Theme {
    let bg = Color::Rgb(40, 42, 54); // #282a36
    let fg = Color::Rgb(248, 248, 242); // #f8f8f2
    let selection = Color::Rgb(68, 71, 90); // #44475a
    let comment = Color::Rgb(98, 114, 164); // #6272a4
    let cyan = Color::Rgb(139, 233, 253); // #8be9fd
    let green = Color::Rgb(80, 250, 123); // #50fa7b
    let orange = Color::Rgb(255, 184, 108); // #ffb86c
    let pink = Color::Rgb(255, 121, 198); // #ff79c6
    let purple = Color::Rgb(189, 147, 249); // #bd93f9
    let red = Color::Rgb(255, 85, 85); // #ff5555
    let yellow = Color::Rgb(241, 250, 140); // #f1fa8c

    Theme {
        border_type: BorderType::Rounded,

        border_dim: Style::default().fg(comment),
        border_primary: Style::default().fg(purple),
        border_active: Style::default().fg(pink),
        border_warn: Style::default().fg(orange),
        border_danger: Style::default().fg(red),
        border_server: Style::default().fg(cyan),

        tab_active: Style::default()
            .fg(bg)
            .bg(purple)
            .add_modifier(Modifier::BOLD),
        tab_inactive: Style::default().fg(comment),

        text_bright: fg,
        text_normal: fg,
        text_secondary: Color::Rgb(190, 190, 200),
        text_muted: comment,
        text_accent: purple,
        text_highlight: yellow,
        text_server: cyan,

        hp_high: green,
        hp_mid: orange,
        hp_low: red,
        mana_color: cyan,
        bar_empty: selection,

        spawn_pc: green,
        spawn_npc: fg,
        spawn_named: yellow,
        spawn_corpse: comment,
        spawn_unknown: red,

        table_header: Style::default().fg(pink).add_modifier(Modifier::BOLD),
        row_selected_bg: selection,

        state_dead: red,
        state_sitting: yellow,
        state_feigned: orange,
        state_frozen: cyan,
        state_normal: green,

        mode_camp: green,
        mode_hunt: orange,

        statusbar_message: Style::default().fg(yellow).add_modifier(Modifier::BOLD),
        statusbar_key: Style::default().fg(purple),
        statusbar_dim: Style::default().fg(comment),
        statusbar_cmd: Style::default().fg(pink).add_modifier(Modifier::BOLD),
        statusbar_badge: Style::default()
            .fg(bg)
            .bg(purple)
            .add_modifier(Modifier::BOLD),

        map_you: pink,
        map_pc: green,
        map_group: Color::Cyan,
        map_npc: fg,
        map_named: yellow,
        map_dead_named: red,
        map_corpse: comment,
        map_lines: selection,
        map_geometry: comment,

        header_title: Style::default().fg(purple).add_modifier(Modifier::BOLD),
        header_client_count: Style::default().fg(pink).add_modifier(Modifier::BOLD),
        header_selected: Style::default().fg(green).add_modifier(Modifier::BOLD),
        header_zone: Style::default().fg(fg),
        header_group: Style::default().fg(comment),
        header_group_active: Style::default().fg(cyan).add_modifier(Modifier::BOLD),

        help_key: Style::default().fg(pink),
        help_desc: Style::default().fg(fg),
        help_heading: Style::default().fg(pink).add_modifier(Modifier::BOLD),
        help_dim: Style::default().fg(comment),
        help_bg: bg,
        help_border: Style::default().fg(purple),

        con_red: red,
        con_yellow: yellow,
        con_white: fg,
        con_light_blue: cyan,
        con_blue: purple,
        con_green: green,
    }
}

// ─── Neriak ────────────────────────────────────────────────────────────────

/// Neriak Third Gate theme — cyber-arcanepunk dark fantasy.
///
/// Adapted from the Variant G web palette: deep void blacks, magenta/purple
/// accents, spectral cyan highlights, lavender text.
#[must_use]
pub fn neriak() -> Theme {
    let void = Color::Rgb(13, 6, 24); // #0d0618  deep void background
    let violet = Color::Rgb(26, 10, 46); // #1a0a2e  panel background
    let magenta = Color::Rgb(204, 68, 255); // #cc44ff  primary accent
    let magenta_bright = Color::Rgb(255, 0, 255); // #ff00ff  hot magenta glow
    let cyan = Color::Rgb(0, 229, 255); // #00e5ff  spectral cyan
    let lavender = Color::Rgb(226, 215, 244); // #e2d7f4  body text
    let lavender_dim = Color::Rgb(160, 150, 180); // muted lavender
    let shadow = Color::Rgb(80, 60, 110); // dim purple-gray
    let deep = Color::Rgb(45, 30, 65); // very dim violet
    let green = Color::Rgb(52, 211, 153); // #34d399  emerald
    let red = Color::Rgb(239, 68, 68); // #ef4444  danger red
    let amber = Color::Rgb(251, 191, 36); // #fbbf24  warning amber
    let blue = Color::Rgb(96, 165, 250); // #60a5fa  mana blue

    Theme {
        border_type: BorderType::Rounded,

        border_dim: Style::default().fg(shadow),
        border_primary: Style::default().fg(magenta),
        border_active: Style::default().fg(cyan),
        border_warn: Style::default().fg(amber),
        border_danger: Style::default().fg(red),
        border_server: Style::default().fg(magenta_bright),

        tab_active: Style::default()
            .fg(void)
            .bg(magenta)
            .add_modifier(Modifier::BOLD),
        tab_inactive: Style::default().fg(shadow),

        text_bright: lavender,
        text_normal: lavender,
        text_secondary: lavender_dim,
        text_muted: shadow,
        text_accent: cyan,
        text_highlight: magenta,
        text_server: magenta_bright,

        hp_high: green,
        hp_mid: amber,
        hp_low: red,
        mana_color: blue,
        bar_empty: deep,

        spawn_pc: green,
        spawn_npc: lavender,
        spawn_named: magenta,
        spawn_corpse: shadow,
        spawn_unknown: red,

        table_header: Style::default().fg(cyan).add_modifier(Modifier::BOLD),
        row_selected_bg: violet,

        state_dead: red,
        state_sitting: amber,
        state_feigned: magenta,
        state_frozen: blue,
        state_normal: green,

        mode_camp: cyan,
        mode_hunt: magenta,

        statusbar_message: Style::default().fg(magenta).add_modifier(Modifier::BOLD),
        statusbar_key: Style::default().fg(cyan),
        statusbar_dim: Style::default().fg(shadow),
        statusbar_cmd: Style::default().fg(cyan).add_modifier(Modifier::BOLD),
        statusbar_badge: Style::default()
            .fg(void)
            .bg(magenta)
            .add_modifier(Modifier::BOLD),

        map_you: cyan,
        map_pc: green,
        map_npc: lavender,
        map_named: magenta,
        map_dead_named: red,
        map_corpse: shadow,
        map_lines: deep,
        map_geometry: Color::Rgb(70, 55, 95),
        map_group: cyan,

        header_title: Style::default().fg(magenta).add_modifier(Modifier::BOLD),
        header_client_count: Style::default().fg(cyan).add_modifier(Modifier::BOLD),
        header_selected: Style::default().fg(green).add_modifier(Modifier::BOLD),
        header_zone: Style::default().fg(lavender),
        header_group: Style::default().fg(shadow),
        header_group_active: Style::default().fg(cyan).add_modifier(Modifier::BOLD),

        help_key: Style::default().fg(cyan),
        help_desc: Style::default().fg(lavender_dim),
        help_heading: Style::default().fg(magenta).add_modifier(Modifier::BOLD),
        help_dim: Style::default().fg(shadow),
        help_bg: void,
        help_border: Style::default().fg(magenta),

        con_red: red,
        con_yellow: amber,
        con_white: lavender,
        con_light_blue: cyan,
        con_blue: blue,
        con_green: green,
    }
}

/// Parse a user-provided color name into a ratatui `Color`.
pub fn parse_color_name(s: &str) -> Option<Color> {
    match s.to_ascii_lowercase().as_str() {
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "blue" => Some(Color::Blue),
        "yellow" => Some(Color::Yellow),
        "cyan" => Some(Color::Cyan),
        "magenta" | "purple" => Some(Color::Magenta),
        "white" => Some(Color::White),
        "orange" => Some(Color::Rgb(255, 165, 0)),
        _ => None,
    }
}

// ─── ThemeKind ───────────────────────────────────────────────────────────────

/// Enum so the app can store which theme is active and cycle through them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeKind {
    /// Polished dark theme with RGB colors and rounded borders.
    #[default]
    DarkModern,
    /// Classic terminal theme with named colors and plain borders.
    Classic,
    /// Dracula color scheme with dark purples and vivid accents.
    Dracula,
    /// Neriak Third Gate — cyber-arcanepunk dark fantasy.
    Neriak,
}

impl ThemeKind {
    /// Cycles to the next theme variant.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::DarkModern => Self::Neriak,
            Self::Neriak => Self::Dracula,
            Self::Dracula => Self::Classic,
            Self::Classic => Self::DarkModern,
        }
    }

    /// Returns a short display label for this theme.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::DarkModern => "Dark",
            Self::Classic => "Classic",
            Self::Dracula => "Dracula",
            Self::Neriak => "Neriak",
        }
    }

    /// Constructs the full `Theme` for this variant.
    #[must_use]
    pub fn build(self) -> Theme {
        match self {
            Self::DarkModern => dark_modern(),
            Self::Classic => classic(),
            Self::Dracula => dracula(),
            Self::Neriak => neriak(),
        }
    }

    /// Path to the TUI preferences file.
    fn prefs_path() -> std::path::PathBuf {
        std::path::PathBuf::from("config/tui-prefs.toml")
    }

    /// Save the current theme preference to disk.
    pub fn save(self) {
        #[derive(serde::Serialize)]
        struct Prefs {
            theme: ThemeKind,
        }
        let content = toml::to_string_pretty(&Prefs { theme: self }).unwrap_or_default();
        if let Err(e) = std::fs::write(Self::prefs_path(), content) {
            tracing::warn!("failed to save theme preference: {e}");
        }
    }

    /// Load the saved theme preference from disk, or return the default.
    #[must_use]
    pub fn load_saved() -> Self {
        #[derive(serde::Deserialize)]
        struct Prefs {
            #[serde(default)]
            theme: ThemeKind,
        }
        std::fs::read_to_string(Self::prefs_path())
            .ok()
            .and_then(|s| toml::from_str::<Prefs>(&s).ok())
            .map(|p| p.theme)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_kind_next_cycles_all_four() {
        let start = ThemeKind::DarkModern;
        let second = start.next();
        assert_eq!(second, ThemeKind::Neriak);
        let third = second.next();
        assert_eq!(third, ThemeKind::Dracula);
        let fourth = third.next();
        assert_eq!(fourth, ThemeKind::Classic);
        let back = fourth.next();
        assert_eq!(back, ThemeKind::DarkModern);
    }

    #[test]
    fn theme_kind_labels() {
        assert_eq!(ThemeKind::DarkModern.label(), "Dark");
        assert_eq!(ThemeKind::Classic.label(), "Classic");
        assert_eq!(ThemeKind::Dracula.label(), "Dracula");
        assert_eq!(ThemeKind::Neriak.label(), "Neriak");
    }

    #[test]
    fn theme_kind_default_is_dark_modern() {
        assert_eq!(ThemeKind::default(), ThemeKind::DarkModern);
    }

    #[test]
    fn dark_modern_theme_has_rounded_borders() {
        let theme = dark_modern();
        assert_eq!(theme.border_type, BorderType::Rounded);
    }

    #[test]
    fn classic_theme_has_plain_borders() {
        let theme = classic();
        assert_eq!(theme.border_type, BorderType::Plain);
    }

    #[test]
    fn dracula_theme_has_rounded_borders() {
        let theme = dracula();
        assert_eq!(theme.border_type, BorderType::Rounded);
    }

    #[test]
    fn neriak_theme_has_rounded_borders() {
        let theme = neriak();
        assert_eq!(theme.border_type, BorderType::Rounded);
    }

    #[test]
    fn neriak_uses_rgb_colors() {
        let theme = neriak();
        assert!(matches!(theme.hp_high, Color::Rgb(_, _, _)));
        assert!(matches!(theme.hp_mid, Color::Rgb(_, _, _)));
        assert!(matches!(theme.hp_low, Color::Rgb(_, _, _)));
    }

    #[test]
    fn neriak_text_accent_is_cyan_family() {
        let theme = neriak();
        if let Color::Rgb(r, _g, b) = theme.text_accent {
            assert!(b > r, "Neriak accent should be cyan-dominant");
        }
    }

    #[test]
    fn build_returns_correct_theme_variant() {
        let dm = ThemeKind::DarkModern.build();
        assert_eq!(dm.border_type, BorderType::Rounded);

        let cl = ThemeKind::Classic.build();
        assert_eq!(cl.border_type, BorderType::Plain);

        let dr = ThemeKind::Dracula.build();
        assert_eq!(dr.border_type, BorderType::Rounded);

        let nr = ThemeKind::Neriak.build();
        assert_eq!(nr.border_type, BorderType::Rounded);
    }

    #[test]
    fn dark_modern_hp_colors_are_distinct() {
        let theme = dark_modern();
        assert_ne!(theme.hp_high, theme.hp_mid);
        assert_ne!(theme.hp_mid, theme.hp_low);
        assert_ne!(theme.hp_high, theme.hp_low);
    }

    #[test]
    fn classic_uses_named_colors() {
        let theme = classic();
        assert_eq!(theme.hp_high, Color::Green);
        assert_eq!(theme.hp_mid, Color::Yellow);
        assert_eq!(theme.hp_low, Color::Red);
        assert_eq!(theme.mana_color, Color::Blue);
    }

    #[test]
    fn dracula_con_colors_set() {
        let theme = dracula();
        // Dracula uses its own palette colors for con
        assert_ne!(theme.con_red, theme.con_green);
        assert_ne!(theme.con_yellow, theme.con_blue);
    }

    #[test]
    fn all_themes_have_distinct_spawn_colors() {
        for kind in [
            ThemeKind::DarkModern,
            ThemeKind::Classic,
            ThemeKind::Dracula,
            ThemeKind::Neriak,
        ] {
            let theme = kind.build();
            // PC and corpse should always be visually distinct
            assert_ne!(
                theme.spawn_pc, theme.spawn_corpse,
                "{:?} spawn_pc == spawn_corpse",
                kind
            );
            assert_ne!(
                theme.spawn_npc, theme.spawn_corpse,
                "{:?} spawn_npc == spawn_corpse",
                kind
            );
        }
    }

    #[test]
    fn theme_kind_clone_and_copy() {
        let a = ThemeKind::Dracula;
        let b = a;
        let c = a;
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn theme_kind_debug_format() {
        let dbg = format!("{:?}", ThemeKind::DarkModern);
        assert!(dbg.contains("DarkModern"));
    }

    #[test]
    fn theme_struct_is_clone() {
        let theme = dark_modern();
        let cloned = theme.clone();
        assert_eq!(cloned.border_type, theme.border_type);
        assert_eq!(cloned.hp_high, theme.hp_high);
    }

    #[test]
    fn all_themes_have_distinct_state_colors() {
        for kind in [
            ThemeKind::DarkModern,
            ThemeKind::Classic,
            ThemeKind::Dracula,
            ThemeKind::Neriak,
        ] {
            let theme = kind.build();
            assert_ne!(
                theme.state_dead, theme.state_normal,
                "{:?} dead == normal",
                kind
            );
            assert_ne!(
                theme.state_sitting, theme.state_dead,
                "{:?} sitting == dead",
                kind
            );
            assert_ne!(
                theme.state_feigned, theme.state_normal,
                "{:?} feigned == normal",
                kind
            );
        }
    }

    #[test]
    fn all_themes_have_distinct_map_colors() {
        for kind in [
            ThemeKind::DarkModern,
            ThemeKind::Classic,
            ThemeKind::Dracula,
            ThemeKind::Neriak,
        ] {
            let theme = kind.build();
            assert_ne!(
                theme.map_you, theme.map_npc,
                "{:?} map_you == map_npc",
                kind
            );
            assert_ne!(
                theme.map_pc, theme.map_corpse,
                "{:?} map_pc == map_corpse",
                kind
            );
        }
    }

    #[test]
    fn all_themes_have_distinct_mode_colors() {
        for kind in [
            ThemeKind::DarkModern,
            ThemeKind::Classic,
            ThemeKind::Dracula,
            ThemeKind::Neriak,
        ] {
            let theme = kind.build();
            assert_ne!(
                theme.mode_camp, theme.mode_hunt,
                "{:?} mode_camp == mode_hunt",
                kind
            );
        }
    }

    #[test]
    fn all_themes_hp_colors_are_distinct() {
        for kind in [
            ThemeKind::DarkModern,
            ThemeKind::Classic,
            ThemeKind::Dracula,
            ThemeKind::Neriak,
        ] {
            let theme = kind.build();
            assert_ne!(theme.hp_high, theme.hp_mid, "{:?} hp_high == hp_mid", kind);
            assert_ne!(theme.hp_mid, theme.hp_low, "{:?} hp_mid == hp_low", kind);
            assert_ne!(theme.hp_high, theme.hp_low, "{:?} hp_high == hp_low", kind);
        }
    }

    #[test]
    fn all_themes_have_six_con_colors() {
        for kind in [
            ThemeKind::DarkModern,
            ThemeKind::Classic,
            ThemeKind::Dracula,
            ThemeKind::Neriak,
        ] {
            let theme = kind.build();
            let cons = [
                theme.con_red,
                theme.con_yellow,
                theme.con_white,
                theme.con_light_blue,
                theme.con_blue,
                theme.con_green,
            ];
            // Red and green should always differ
            assert_ne!(cons[0], cons[5], "{:?} con_red == con_green", kind);
            // White and blue should differ
            assert_ne!(cons[2], cons[4], "{:?} con_white == con_blue", kind);
        }
    }

    #[test]
    fn dark_modern_text_colors_descend_brightness() {
        let theme = dark_modern();
        // text_bright should be brighter than text_muted
        if let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) =
            (theme.text_bright, theme.text_muted)
        {
            let bright_sum = r1 as u32 + g1 as u32 + b1 as u32;
            let muted_sum = r2 as u32 + g2 as u32 + b2 as u32;
            assert!(
                bright_sum > muted_sum,
                "text_bright ({}) should be brighter than text_muted ({})",
                bright_sum,
                muted_sum
            );
        }
    }

    #[test]
    fn dark_modern_mana_is_blue_family() {
        let theme = dark_modern();
        if let Color::Rgb(r, _g, b) = theme.mana_color {
            assert!(b > r, "Mana color should be blue-dominant");
        }
    }

    #[test]
    fn classic_mana_is_named_blue() {
        let theme = classic();
        assert_eq!(theme.mana_color, Color::Blue);
    }

    #[test]
    fn dracula_uses_rgb_colors() {
        let theme = dracula();
        // Dracula theme should use RGB colors for HP
        assert!(matches!(theme.hp_high, Color::Rgb(_, _, _)));
        assert!(matches!(theme.hp_mid, Color::Rgb(_, _, _)));
        assert!(matches!(theme.hp_low, Color::Rgb(_, _, _)));
    }

    #[test]
    fn theme_kind_serde_round_trip() {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct W {
            theme: ThemeKind,
        }
        for kind in [
            ThemeKind::DarkModern,
            ThemeKind::Classic,
            ThemeKind::Dracula,
            ThemeKind::Neriak,
        ] {
            let serialized = toml::to_string(&W { theme: kind }).unwrap();
            let deserialized: W = toml::from_str(&serialized).unwrap();
            assert_eq!(kind, deserialized.theme, "round-trip failed for {:?}", kind);
        }
    }

    #[test]
    fn all_themes_text_accent_differs_from_normal() {
        for kind in [
            ThemeKind::DarkModern,
            ThemeKind::Classic,
            ThemeKind::Dracula,
            ThemeKind::Neriak,
        ] {
            let theme = kind.build();
            assert_ne!(
                theme.text_accent, theme.text_normal,
                "{:?} accent == normal",
                kind
            );
        }
    }
}
