//! TOML theme loader for the TextQuest TUI.
//!
//! Loads theme definition files from `config/themes/*.toml` and converts them
//! into [`Theme`] values usable by the renderer.
//!
//! ## TOML format
//!
//! ```toml
//! [meta]
//! name = "Neriak"
//! version = "1.0"
//! border_style = "rounded"   # "rounded" | "plain"
//!
//! [colors]
//! # Borders
//! border_dim      = "#501c8e"
//! border_primary  = "#cc44ff"
//! border_active   = "#00e5ff"
//! border_warn     = "#fbbf24"
//! border_danger   = "#ef4444"
//! border_server   = "#ff00ff"
//!
//! # Tab bar (fg = text on active tab, bg = active tab background)
//! tab_active_fg   = "#0d0618"
//! tab_active_bg   = "#cc44ff"
//! tab_inactive    = "#501c8e"
//!
//! # Text
//! text_bright    = "#e2d7f4"
//! text_normal    = "#e2d7f4"
//! text_secondary = "#a096b4"
//! text_muted     = "#503c6e"
//! text_accent    = "#00e5ff"
//! text_highlight = "#cc44ff"
//! text_server    = "#ff00ff"
//!
//! # HP / resource bars
//! hp_high   = "#34d399"
//! hp_mid    = "#fbbf24"
//! hp_low    = "#ef4444"
//! mana      = "#60a5fa"
//! bar_empty = "#2d1e41"
//!
//! # Spawn type colors
//! spawn_pc      = "#34d399"
//! spawn_npc     = "#e2d7f4"
//! spawn_named   = "#cc44ff"
//! spawn_corpse  = "#503c6e"
//! spawn_unknown = "#ef4444"
//!
//! # Table / selection
//! table_header    = "#00e5ff"
//! row_selected_bg = "#1a0a2e"
//!
//! # Stand-state colors
//! state_dead    = "#ef4444"
//! state_sitting = "#fbbf24"
//! state_feigned = "#cc44ff"
//! state_frozen  = "#60a5fa"
//! state_normal  = "#34d399"
//!
//! # Operating-mode colors
//! mode_camp = "#00e5ff"
//! mode_hunt = "#cc44ff"
//!
//! # Map overlay
//! map_you       = "#00e5ff"
//! map_pc        = "#34d399"
//! map_group     = "#00e5ff"
//! map_npc       = "#e2d7f4"
//! map_named     = "#cc44ff"
//! map_dead_named = "#ef4444"
//! map_corpse    = "#503c6e"
//! map_lines     = "#2d1e41"
//! map_geometry  = "#46375f"
//!
//! # Help overlay background
//! help_bg = "#0d0618"
//!
//! # Con colors (level-relative mob difficulty)
//! con_red        = "#ef4444"
//! con_yellow     = "#fbbf24"
//! con_white      = "#e2d7f4"
//! con_light_blue = "#00e5ff"
//! con_blue       = "#60a5fa"
//! con_green      = "#34d399"
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ratatui::{
    style::{Color, Modifier, Style},
    widgets::BorderType,
};
use serde::Deserialize;

use crate::tui::theme::Theme;

// ─── Deserialization types ────────────────────────────────────────────────────

/// Raw TOML representation of a theme file.
#[derive(Debug, Deserialize)]
struct ThemeFile {
    meta: ThemeMeta,
    colors: ThemeColors,
}

#[derive(Debug, Deserialize)]
struct ThemeMeta {
    name: String,
    #[allow(dead_code)]
    version: Option<String>,
    /// `"rounded"` or `"plain"` (default: `"rounded"`).
    #[serde(default = "default_border_style")]
    border_style: String,
}

fn default_border_style() -> String {
    "rounded".into()
}

/// All color fields in the TOML `[colors]` section.  Every field is a hex
/// color string (`"#rrggbb"` or `"#rgb"`).  Named terminal colors are also
/// accepted via [`parse_hex_color`].
#[derive(Debug, Deserialize)]
struct ThemeColors {
    // Base colors
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    foreground: Option<String>,

    // Borders
    border_dim: String,
    border_primary: String,
    border_active: String,
    border_warn: String,
    border_danger: String,
    border_server: String,

    // Tab bar
    tab_active_fg: String,
    tab_active_bg: String,
    tab_inactive: String,

    // Text
    text_bright: String,
    text_normal: String,
    text_secondary: String,
    text_muted: String,
    text_accent: String,
    text_highlight: String,
    text_server: String,

    // HP / resource bars
    hp_high: String,
    hp_mid: String,
    hp_low: String,
    mana: String,
    bar_empty: String,

    // Spawn type colors
    spawn_pc: String,
    spawn_npc: String,
    spawn_named: String,
    spawn_corpse: String,
    spawn_unknown: String,

    // Table / selection
    table_header: String,
    row_selected_bg: String,

    // Stand-state colors
    state_dead: String,
    state_sitting: String,
    state_feigned: String,
    state_frozen: String,
    state_normal: String,

    // Operating-mode colors
    mode_camp: String,
    mode_hunt: String,

    // Map overlay
    map_you: String,
    map_pc: String,
    map_group: String,
    map_npc: String,
    map_named: String,
    map_dead_named: String,
    map_corpse: String,
    map_lines: String,
    map_geometry: String,

    // Help overlay
    help_bg: String,

    // Con colors
    con_red: String,
    con_yellow: String,
    con_white: String,
    con_light_blue: String,
    con_blue: String,
    con_green: String,
}

// ─── Error type ───────────────────────────────────────────────────────────────

/// Errors that can occur while loading a theme.
#[derive(Debug)]
pub enum ThemeLoadError {
    /// Could not read the TOML file from disk.
    Io(std::io::Error),
    /// TOML parse error.
    Parse(toml::de::Error),
    /// A color value could not be parsed.
    InvalidColor { field: String, value: String },
}

impl std::fmt::Display for ThemeLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Parse(e) => write!(f, "TOML parse error: {e}"),
            Self::InvalidColor { field, value } => {
                write!(f, "invalid color for field `{field}`: {value:?}")
            }
        }
    }
}

impl std::error::Error for ThemeLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
            Self::InvalidColor { .. } => None,
        }
    }
}

impl From<std::io::Error> for ThemeLoadError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for ThemeLoadError {
    fn from(e: toml::de::Error) -> Self {
        Self::Parse(e)
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Parse a hex color string (`#rrggbb`, `#rgb`, or a named terminal color) into
/// a ratatui [`Color`].
///
/// # Errors
///
/// Returns `None` if the string does not match a recognised format.
pub fn parse_hex_color(hex: &str) -> Option<Color> {
    let s = hex.trim();

    // Named terminal colors
    let named = match s.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" | "purple" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "dark_gray" | "dark gray" => Some(Color::DarkGray),
        "lightred" | "light_red" => Some(Color::LightRed),
        "lightgreen" | "light_green" => Some(Color::LightGreen),
        "lightyellow" | "light_yellow" => Some(Color::LightYellow),
        "lightblue" | "light_blue" => Some(Color::LightBlue),
        "lightmagenta" | "light_magenta" => Some(Color::LightMagenta),
        "lightcyan" | "light_cyan" => Some(Color::LightCyan),
        "white" => Some(Color::White),
        _ => None,
    };
    if named.is_some() {
        return named;
    }

    // Hex strings: must start with '#'
    let hex_digits = s.strip_prefix('#')?;

    match hex_digits.len() {
        // #rgb shorthand
        3 => {
            let r = u8::from_str_radix(&hex_digits[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex_digits[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex_digits[2..3].repeat(2), 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        // #rrggbb full
        6 => {
            let r = u8::from_str_radix(&hex_digits[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex_digits[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex_digits[4..6], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

/// Load a single theme from a TOML file.
///
/// # Errors
///
/// Returns [`ThemeLoadError`] if the file cannot be read, the TOML is invalid,
/// or any color value cannot be parsed.
pub fn load_theme(path: &Path) -> Result<Theme, ThemeLoadError> {
    let content = std::fs::read_to_string(path)?;
    let file: ThemeFile = toml::from_str(&content)?;
    theme_from_file(file)
}

/// Load all `.toml` files from `dir` as themes, keyed by file stem.
///
/// Files that fail to parse are skipped with a warning log rather than
/// aborting the whole load.  Returns an empty map if the directory does not
/// exist.
///
/// # Errors
///
/// Returns an error only if `dir` exists but cannot be read as a directory.
pub fn load_all_themes(dir: &Path) -> Result<HashMap<String, Theme>, ThemeLoadError> {
    if !dir.exists() {
        return Ok(HashMap::new());
    }

    let mut map = HashMap::new();
    let entries = std::fs::read_dir(dir).map_err(ThemeLoadError::Io)?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        let key = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_owned();

        match load_theme(&path) {
            Ok(theme) => {
                map.insert(key, theme);
            }
            Err(e) => {
                tracing::warn!("skipping theme {:?}: {e}", path.display());
            }
        }
    }

    Ok(map)
}

/// Default theme directory relative to the working directory.
pub fn default_theme_dir() -> PathBuf {
    PathBuf::from("config/themes")
}

// ─── Conversion helpers ───────────────────────────────────────────────────────

fn parse_field(field: &str, value: &str) -> Result<Color, ThemeLoadError> {
    parse_hex_color(value).ok_or_else(|| ThemeLoadError::InvalidColor {
        field: field.to_owned(),
        value: value.to_owned(),
    })
}

fn fg(field: &str, value: &str) -> Result<Style, ThemeLoadError> {
    Ok(Style::default().fg(parse_field(field, value)?))
}

fn fg_bold(field: &str, value: &str) -> Result<Style, ThemeLoadError> {
    Ok(Style::default()
        .fg(parse_field(field, value)?)
        .add_modifier(Modifier::BOLD))
}

fn theme_from_file(file: ThemeFile) -> Result<Theme, ThemeLoadError> {
    let c = &file.colors;
    let m = &file.meta;

    let border_type = match m.border_style.to_ascii_lowercase().as_str() {
        "plain" => BorderType::Plain,
        _ => BorderType::Rounded,
    };

    let accent = parse_field("colors.text_accent", &c.text_accent)?;
    let table_hdr = parse_field("colors.table_header", &c.table_header)?;
    let tab_fg = parse_field("colors.tab_active_fg", &c.tab_active_fg)?;
    let tab_bg = parse_field("colors.tab_active_bg", &c.tab_active_bg)?;
    let help_bg = parse_field("colors.help_bg", &c.help_bg)?;
    let background = match &c.background {
        Some(value) => parse_field("colors.background", value)?,
        None => help_bg,
    };
    let foreground = match &c.foreground {
        Some(value) => parse_field("colors.foreground", value)?,
        None => parse_field("colors.text_normal", &c.text_normal)?,
    };

    Ok(Theme {
        border_type,
        background,
        foreground,

        border_dim: fg("colors.border_dim", &c.border_dim)?,
        border_primary: fg("colors.border_primary", &c.border_primary)?,
        border_active: fg("colors.border_active", &c.border_active)?,
        border_warn: fg("colors.border_warn", &c.border_warn)?,
        border_danger: fg("colors.border_danger", &c.border_danger)?,
        border_server: fg("colors.border_server", &c.border_server)?,

        tab_active: Style::default()
            .fg(tab_fg)
            .bg(tab_bg)
            .add_modifier(Modifier::BOLD),
        tab_inactive: fg("colors.tab_inactive", &c.tab_inactive)?,

        text_bright: parse_field("colors.text_bright", &c.text_bright)?,
        text_normal: parse_field("colors.text_normal", &c.text_normal)?,
        text_secondary: parse_field("colors.text_secondary", &c.text_secondary)?,
        text_muted: parse_field("colors.text_muted", &c.text_muted)?,
        text_accent: accent,
        text_highlight: parse_field("colors.text_highlight", &c.text_highlight)?,
        text_server: parse_field("colors.text_server", &c.text_server)?,

        hp_high: parse_field("colors.hp_high", &c.hp_high)?,
        hp_mid: parse_field("colors.hp_mid", &c.hp_mid)?,
        hp_low: parse_field("colors.hp_low", &c.hp_low)?,
        mana_color: parse_field("colors.mana", &c.mana)?,
        bar_empty: parse_field("colors.bar_empty", &c.bar_empty)?,

        spawn_pc: parse_field("colors.spawn_pc", &c.spawn_pc)?,
        spawn_npc: parse_field("colors.spawn_npc", &c.spawn_npc)?,
        spawn_named: parse_field("colors.spawn_named", &c.spawn_named)?,
        spawn_corpse: parse_field("colors.spawn_corpse", &c.spawn_corpse)?,
        spawn_unknown: parse_field("colors.spawn_unknown", &c.spawn_unknown)?,

        table_header: Style::default().fg(table_hdr).add_modifier(Modifier::BOLD),
        row_selected_bg: parse_field("colors.row_selected_bg", &c.row_selected_bg)?,

        state_dead: parse_field("colors.state_dead", &c.state_dead)?,
        state_sitting: parse_field("colors.state_sitting", &c.state_sitting)?,
        state_feigned: parse_field("colors.state_feigned", &c.state_feigned)?,
        state_frozen: parse_field("colors.state_frozen", &c.state_frozen)?,
        state_normal: parse_field("colors.state_normal", &c.state_normal)?,

        mode_camp: parse_field("colors.mode_camp", &c.mode_camp)?,
        mode_hunt: parse_field("colors.mode_hunt", &c.mode_hunt)?,

        statusbar_message: fg_bold("colors.text_highlight", &c.text_highlight)?,
        statusbar_key: fg("colors.text_accent", &c.text_accent)?,
        statusbar_dim: fg("colors.text_muted", &c.text_muted)?,
        statusbar_cmd: fg_bold("colors.text_accent", &c.text_accent)?,
        statusbar_badge: Style::default()
            .fg(tab_fg)
            .bg(tab_bg)
            .add_modifier(Modifier::BOLD),

        map_you: parse_field("colors.map_you", &c.map_you)?,
        map_pc: parse_field("colors.map_pc", &c.map_pc)?,
        map_group: parse_field("colors.map_group", &c.map_group)?,
        map_npc: parse_field("colors.map_npc", &c.map_npc)?,
        map_named: parse_field("colors.map_named", &c.map_named)?,
        map_dead_named: parse_field("colors.map_dead_named", &c.map_dead_named)?,
        map_corpse: parse_field("colors.map_corpse", &c.map_corpse)?,
        map_lines: parse_field("colors.map_lines", &c.map_lines)?,
        map_geometry: parse_field("colors.map_geometry", &c.map_geometry)?,

        header_title: fg_bold("colors.text_accent", &c.text_accent)?,
        header_client_count: fg_bold("colors.text_highlight", &c.text_highlight)?,
        header_selected: fg_bold("colors.state_normal", &c.state_normal)?,
        header_zone: fg("colors.text_normal", &c.text_normal)?,
        header_group: fg("colors.text_muted", &c.text_muted)?,
        header_group_active: fg_bold("colors.text_accent", &c.text_accent)?,

        help_key: Style::default().fg(accent),
        help_desc: fg("colors.text_secondary", &c.text_secondary)?,
        help_heading: Style::default().fg(accent).add_modifier(Modifier::BOLD),
        help_dim: fg("colors.text_muted", &c.text_muted)?,
        help_bg,
        help_border: Style::default().fg(accent),

        con_red: parse_field("colors.con_red", &c.con_red)?,
        con_yellow: parse_field("colors.con_yellow", &c.con_yellow)?,
        con_white: parse_field("colors.con_white", &c.con_white)?,
        con_light_blue: parse_field("colors.con_light_blue", &c.con_light_blue)?,
        con_blue: parse_field("colors.con_blue", &c.con_blue)?,
        con_green: parse_field("colors.con_green", &c.con_green)?,
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_hex_color ───────────────────────────────────────────────────────

    #[test]
    fn parse_hex_rrggbb() {
        assert_eq!(
            parse_hex_color("#cc44ff"),
            Some(Color::Rgb(0xcc, 0x44, 0xff))
        );
    }

    #[test]
    fn parse_hex_rgb_shorthand() {
        // #f0f → #ff00ff
        assert_eq!(parse_hex_color("#f0f"), Some(Color::Rgb(0xff, 0x00, 0xff)));
    }

    #[test]
    fn parse_hex_named_colors() {
        assert_eq!(parse_hex_color("red"), Some(Color::Red));
        assert_eq!(parse_hex_color("cyan"), Some(Color::Cyan));
        assert_eq!(parse_hex_color("white"), Some(Color::White));
        assert_eq!(parse_hex_color("darkgray"), Some(Color::DarkGray));
        assert_eq!(parse_hex_color("purple"), Some(Color::Magenta));
    }

    #[test]
    fn parse_hex_case_insensitive() {
        assert_eq!(
            parse_hex_color("#CC44FF"),
            Some(Color::Rgb(0xcc, 0x44, 0xff))
        );
        assert_eq!(parse_hex_color("RED"), Some(Color::Red));
    }

    #[test]
    fn parse_hex_invalid_returns_none() {
        assert_eq!(parse_hex_color("notacolor"), None);
        assert_eq!(parse_hex_color("#zzzzzz"), None);
        assert_eq!(parse_hex_color("#12345"), None); // 5 digits — invalid
        assert_eq!(parse_hex_color(""), None);
    }

    #[test]
    fn parse_hex_strips_whitespace() {
        assert_eq!(
            parse_hex_color("  #cc44ff  "),
            Some(Color::Rgb(0xcc, 0x44, 0xff))
        );
    }

    // ── load_theme round-trip ─────────────────────────────────────────────────

    fn minimal_toml() -> &'static str {
        r##"
[meta]
name = "Test"
version = "1.0"
border_style = "rounded"

[colors]
border_dim      = "#111111"
border_primary  = "#222222"
border_active   = "#333333"
border_warn     = "#444444"
border_danger   = "#555555"
border_server   = "#666666"
tab_active_fg   = "#000000"
tab_active_bg   = "#777777"
tab_inactive    = "#888888"
text_bright    = "#ffffff"
text_normal    = "#eeeeee"
text_secondary = "#cccccc"
text_muted     = "#888888"
text_accent    = "#00ffff"
text_highlight = "#ffff00"
text_server    = "#ff00ff"
hp_high   = "#00ff00"
hp_mid    = "#ffff00"
hp_low    = "#ff0000"
mana      = "#0000ff"
bar_empty = "#111111"
spawn_pc      = "#00ff00"
spawn_npc     = "#ffffff"
spawn_named   = "#ffff00"
spawn_corpse  = "#555555"
spawn_unknown = "#ff0000"
table_header    = "#00ffff"
row_selected_bg = "#222222"
state_dead    = "#ff0000"
state_sitting = "#ffff00"
state_feigned = "#ff00ff"
state_frozen  = "#0000ff"
state_normal  = "#00ff00"
mode_camp = "#00ff00"
mode_hunt = "#ff8800"
map_you       = "#00ffff"
map_pc        = "#00ff00"
map_group     = "#00ffff"
map_npc       = "#ffffff"
map_named     = "#ffff00"
map_dead_named = "#ff0000"
map_corpse    = "#555555"
map_lines     = "#222222"
map_geometry  = "#333333"
help_bg = "#000000"
con_red        = "#ff0000"
con_yellow     = "#ffff00"
con_white      = "#ffffff"
con_light_blue = "#aaddff"
con_blue       = "#0000ff"
con_green      = "#00ff00"
"##
    }

    #[test]
    fn load_theme_parses_minimal_toml() {
        let file: ThemeFile = toml::from_str(minimal_toml()).expect("should parse");
        let theme = theme_from_file(file).expect("should convert");
        assert_eq!(theme.border_type, BorderType::Rounded);
        assert_eq!(theme.hp_high, Color::Rgb(0, 255, 0));
        assert_eq!(theme.hp_low, Color::Rgb(255, 0, 0));
        assert_eq!(theme.mana_color, Color::Rgb(0, 0, 255));
    }

    #[test]
    fn load_theme_plain_border() {
        let toml_str = minimal_toml().replace("rounded", "plain");
        let file: ThemeFile = toml::from_str(&toml_str).expect("should parse");
        let theme = theme_from_file(file).expect("should convert");
        assert_eq!(theme.border_type, BorderType::Plain);
    }

    #[test]
    fn load_theme_invalid_color_returns_error() {
        let bad =
            minimal_toml().replace(r##"hp_high   = "#00ff00""##, r##"hp_high   = "notacolor""##);
        let file: ThemeFile = toml::from_str(&bad).expect("toml parses ok");
        let err = theme_from_file(file).expect_err("should fail on bad color");
        assert!(matches!(err, ThemeLoadError::InvalidColor { .. }));
    }

    #[test]
    fn load_theme_missing_field_returns_parse_error() {
        // Remove a required field
        let bad = minimal_toml().replace(r##"hp_high   = "#00ff00""##, "");
        let result: Result<ThemeFile, _> = toml::from_str(&bad);
        assert!(result.is_err(), "missing required field should fail");
    }

    #[test]
    fn load_all_themes_empty_dir_returns_empty_map() {
        let tmp = std::env::temp_dir().join("textquest_test_themes_empty");
        std::fs::create_dir_all(&tmp).ok();
        let map = load_all_themes(&tmp).expect("should succeed");
        assert!(map.is_empty());
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn load_all_themes_nonexistent_dir_returns_empty_map() {
        let path = std::path::Path::new("/tmp/textquest_nonexistent_theme_dir_xyz");
        let map = load_all_themes(path).expect("should succeed");
        assert!(map.is_empty());
    }

    #[test]
    fn load_all_themes_loads_valid_files() {
        let tmp = std::env::temp_dir().join("textquest_test_themes_load");
        std::fs::create_dir_all(&tmp).ok();
        std::fs::write(tmp.join("test.toml"), minimal_toml()).ok();
        std::fs::write(tmp.join("not_a_theme.txt"), "ignored").ok();

        let map = load_all_themes(&tmp).expect("should succeed");
        assert!(map.contains_key("test"), "should contain 'test'");
        assert_eq!(map.len(), 1, "txt file should be ignored");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn load_all_themes_skips_invalid_files() {
        let tmp = std::env::temp_dir().join("textquest_test_themes_skip");
        std::fs::create_dir_all(&tmp).ok();
        std::fs::write(tmp.join("valid.toml"), minimal_toml()).ok();
        std::fs::write(tmp.join("broken.toml"), "not valid toml {{{{").ok();

        let map = load_all_themes(&tmp).expect("should succeed");
        assert!(map.contains_key("valid"));
        assert!(!map.contains_key("broken"), "broken file should be skipped");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn loaded_theme_hp_colors_are_distinct() {
        let file: ThemeFile = toml::from_str(minimal_toml()).unwrap();
        let theme = theme_from_file(file).unwrap();
        assert_ne!(theme.hp_high, theme.hp_mid);
        assert_ne!(theme.hp_mid, theme.hp_low);
    }

    #[test]
    fn loaded_theme_spawn_pc_differs_from_corpse() {
        let file: ThemeFile = toml::from_str(minimal_toml()).unwrap();
        let theme = theme_from_file(file).unwrap();
        assert_ne!(theme.spawn_pc, theme.spawn_corpse);
    }

    #[test]
    fn default_theme_dir_is_config_themes() {
        let dir = default_theme_dir();
        assert_eq!(dir, std::path::PathBuf::from("config/themes"));
    }
}
