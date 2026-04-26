//! Color theme system — multiple themes with per-element color definitions.
//!
//! This module provides theme-based color management for the TUI. Themes are loaded
//! from TOML configuration files in `config/themes/` and provide consistent colors
//! across all TUI components without hardcoding color values.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A loaded color theme with all UI element colors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    /// Theme metadata (name, version, etc.)
    pub meta: ThemeMeta,
    /// All color definitions for this theme
    pub colors: ThemeColors,
}

/// Theme metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeMeta {
    pub name: String,
    pub version: String,
    pub border_style: String,
}

/// All color definitions in a theme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeColors {
    // ── Borders ──────────────────────────────────────────────────────────────
    pub border_dim: String,
    pub border_primary: String,
    pub border_active: String,
    pub border_warn: String,
    pub border_danger: String,
    pub border_server: String,

    // ── Tab bar ───────────────────────────────────────────────────────────────
    pub tab_active_fg: String,
    pub tab_active_bg: String,
    pub tab_inactive: String,

    // ── Text ──────────────────────────────────────────────────────────────────
    pub text_bright: String,
    pub text_normal: String,
    pub text_secondary: String,
    pub text_muted: String,
    pub text_accent: String,
    pub text_highlight: String,
    pub text_server: String,

    // ── HP / resource bars ────────────────────────────────────────────────────
    pub hp_high: String,
    pub hp_mid: String,
    pub hp_low: String,
    pub mana: String,
    pub bar_empty: String,

    // ── Spawn type colors ─────────────────────────────────────────────────────
    pub spawn_pc: String,
    pub spawn_npc: String,
    pub spawn_named: String,
    pub spawn_corpse: String,
    pub spawn_unknown: String,

    // ── Table / selection ─────────────────────────────────────────────────────
    pub table_header: String,
    pub row_selected_bg: String,

    // ── Stand-state colors ────────────────────────────────────────────────────
    pub state_dead: String,
    pub state_sitting: String,
    pub state_feigned: String,
    pub state_frozen: String,
    pub state_normal: String,

    // ── Operating-mode colors ─────────────────────────────────────────────────
    pub mode_camp: String,
    pub mode_hunt: String,

    // ── Map overlay ───────────────────────────────────────────────────────────
    pub map_you: String,
    pub map_pc: String,
    pub map_group: String,
    pub map_npc: String,
    pub map_named: String,
    pub map_dead_named: String,
    pub map_corpse: String,
    pub map_lines: String,
    pub map_geometry: String,

    // ── Help overlay ──────────────────────────────────────────────────────────
    pub help_bg: String,

    // ── Con colors (level-relative mob difficulty) ────────────────────────────
    pub con_red: String,
    pub con_yellow: String,
    pub con_white: String,
    pub con_light_blue: String,
    pub con_blue: String,
    pub con_green: String,
}

impl Theme {
    /// Convert a hex color string to a ratatui Color.
    /// Supports formats: "#RRGGBB"
    fn hex_to_color(hex: &str) -> Color {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return Color::Reset;
        }

        if let Ok(num) = u32::from_str_radix(hex, 16) {
            let r = ((num >> 16) & 0xFF) as u8;
            let g = ((num >> 8) & 0xFF) as u8;
            let b = (num & 0xFF) as u8;
            Color::Rgb(r, g, b)
        } else {
            Color::Reset
        }
    }

    // ── Border colors ────────────────────────────────────────────────────────
    pub fn border_dim(&self) -> Color {
        Self::hex_to_color(&self.colors.border_dim)
    }

    pub fn border_primary(&self) -> Color {
        Self::hex_to_color(&self.colors.border_primary)
    }

    pub fn border_active(&self) -> Color {
        Self::hex_to_color(&self.colors.border_active)
    }

    pub fn border_warn(&self) -> Color {
        Self::hex_to_color(&self.colors.border_warn)
    }

    pub fn border_danger(&self) -> Color {
        Self::hex_to_color(&self.colors.border_danger)
    }

    pub fn border_server(&self) -> Color {
        Self::hex_to_color(&self.colors.border_server)
    }

    // ── Tab bar ──────────────────────────────────────────────────────────────
    pub fn tab_active_fg(&self) -> Color {
        Self::hex_to_color(&self.colors.tab_active_fg)
    }

    pub fn tab_active_bg(&self) -> Color {
        Self::hex_to_color(&self.colors.tab_active_bg)
    }

    pub fn tab_inactive(&self) -> Color {
        Self::hex_to_color(&self.colors.tab_inactive)
    }

    // ── Text ──────────────────────────────────────────────────────────────────
    pub fn text_bright(&self) -> Color {
        Self::hex_to_color(&self.colors.text_bright)
    }

    pub fn text_normal(&self) -> Color {
        Self::hex_to_color(&self.colors.text_normal)
    }

    pub fn text_secondary(&self) -> Color {
        Self::hex_to_color(&self.colors.text_secondary)
    }

    pub fn text_muted(&self) -> Color {
        Self::hex_to_color(&self.colors.text_muted)
    }

    pub fn text_accent(&self) -> Color {
        Self::hex_to_color(&self.colors.text_accent)
    }

    pub fn text_highlight(&self) -> Color {
        Self::hex_to_color(&self.colors.text_highlight)
    }

    pub fn text_server(&self) -> Color {
        Self::hex_to_color(&self.colors.text_server)
    }

    // ── HP / resource bars ────────────────────────────────────────────────────
    pub fn hp_high(&self) -> Color {
        Self::hex_to_color(&self.colors.hp_high)
    }

    pub fn hp_mid(&self) -> Color {
        Self::hex_to_color(&self.colors.hp_mid)
    }

    pub fn hp_low(&self) -> Color {
        Self::hex_to_color(&self.colors.hp_low)
    }

    pub fn mana(&self) -> Color {
        Self::hex_to_color(&self.colors.mana)
    }

    pub fn bar_empty(&self) -> Color {
        Self::hex_to_color(&self.colors.bar_empty)
    }

    // ── Spawn type colors ─────────────────────────────────────────────────────
    pub fn spawn_pc(&self) -> Color {
        Self::hex_to_color(&self.colors.spawn_pc)
    }

    pub fn spawn_npc(&self) -> Color {
        Self::hex_to_color(&self.colors.spawn_npc)
    }

    pub fn spawn_named(&self) -> Color {
        Self::hex_to_color(&self.colors.spawn_named)
    }

    pub fn spawn_corpse(&self) -> Color {
        Self::hex_to_color(&self.colors.spawn_corpse)
    }

    pub fn spawn_unknown(&self) -> Color {
        Self::hex_to_color(&self.colors.spawn_unknown)
    }

    // ── Table / selection ─────────────────────────────────────────────────────
    pub fn table_header(&self) -> Color {
        Self::hex_to_color(&self.colors.table_header)
    }

    pub fn row_selected_bg(&self) -> Color {
        Self::hex_to_color(&self.colors.row_selected_bg)
    }

    // ── Stand-state colors ────────────────────────────────────────────────────
    pub fn state_dead(&self) -> Color {
        Self::hex_to_color(&self.colors.state_dead)
    }

    pub fn state_sitting(&self) -> Color {
        Self::hex_to_color(&self.colors.state_sitting)
    }

    pub fn state_feigned(&self) -> Color {
        Self::hex_to_color(&self.colors.state_feigned)
    }

    pub fn state_frozen(&self) -> Color {
        Self::hex_to_color(&self.colors.state_frozen)
    }

    pub fn state_normal(&self) -> Color {
        Self::hex_to_color(&self.colors.state_normal)
    }

    // ── Operating-mode colors ─────────────────────────────────────────────────
    pub fn mode_camp(&self) -> Color {
        Self::hex_to_color(&self.colors.mode_camp)
    }

    pub fn mode_hunt(&self) -> Color {
        Self::hex_to_color(&self.colors.mode_hunt)
    }

    // ── Map overlay ───────────────────────────────────────────────────────────
    pub fn map_you(&self) -> Color {
        Self::hex_to_color(&self.colors.map_you)
    }

    pub fn map_pc(&self) -> Color {
        Self::hex_to_color(&self.colors.map_pc)
    }

    pub fn map_group(&self) -> Color {
        Self::hex_to_color(&self.colors.map_group)
    }

    pub fn map_npc(&self) -> Color {
        Self::hex_to_color(&self.colors.map_npc)
    }

    pub fn map_named(&self) -> Color {
        Self::hex_to_color(&self.colors.map_named)
    }

    pub fn map_dead_named(&self) -> Color {
        Self::hex_to_color(&self.colors.map_dead_named)
    }

    pub fn map_corpse(&self) -> Color {
        Self::hex_to_color(&self.colors.map_corpse)
    }

    pub fn map_lines(&self) -> Color {
        Self::hex_to_color(&self.colors.map_lines)
    }

    pub fn map_geometry(&self) -> Color {
        Self::hex_to_color(&self.colors.map_geometry)
    }

    // ── Help overlay ──────────────────────────────────────────────────────────
    pub fn help_bg(&self) -> Color {
        Self::hex_to_color(&self.colors.help_bg)
    }

    // ── Con colors ────────────────────────────────────────────────────────────
    pub fn con_red(&self) -> Color {
        Self::hex_to_color(&self.colors.con_red)
    }

    pub fn con_yellow(&self) -> Color {
        Self::hex_to_color(&self.colors.con_yellow)
    }

    pub fn con_white(&self) -> Color {
        Self::hex_to_color(&self.colors.con_white)
    }

    pub fn con_light_blue(&self) -> Color {
        Self::hex_to_color(&self.colors.con_light_blue)
    }

    pub fn con_blue(&self) -> Color {
        Self::hex_to_color(&self.colors.con_blue)
    }

    pub fn con_green(&self) -> Color {
        Self::hex_to_color(&self.colors.con_green)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_to_color_valid() {
        let color = Theme::hex_to_color("#FF0000");
        assert_eq!(color, Color::Rgb(255, 0, 0));
    }

    #[test]
    fn hex_to_color_with_hash() {
        let color = Theme::hex_to_color("#00FF00");
        assert_eq!(color, Color::Rgb(0, 255, 0));
    }

    #[test]
    fn hex_to_color_invalid() {
        let color = Theme::hex_to_color("INVALID");
        assert_eq!(color, Color::Reset);
    }
}
