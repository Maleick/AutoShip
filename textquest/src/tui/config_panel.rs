//! Configuration panel with tree view and inline editing.
//!
//! Provides a dedicated config screen showing configuration hierarchy
//! using a tree structure with inline value editing. Supports three scopes:
//! Global (default), per-Group overrides, and per-Toon overrides.
//! Settings inherit: toon < group < global.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};

/// The scope for which config is being viewed/edited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigScope {
    /// Global settings (default).
    Global,
    /// Per-group overrides.
    Group(String),
    /// Per-toon overrides.
    Toon(String),
}

impl ConfigScope {
    /// Display label for the scope selector.
    pub fn label(&self) -> String {
        match self {
            Self::Global => "Global".into(),
            Self::Group(name) => format!("Group: {name}"),
            Self::Toon(name) => format!("Toon: {name}"),
        }
    }
}

/// A node in the configuration tree.
#[derive(Debug, Clone)]
pub struct ConfigNode {
    /// Display label.
    pub label: String,
    /// Current value (if leaf node).
    pub value: Option<String>,
    /// Whether this node is expanded (for branch nodes).
    pub expanded: bool,
    /// Depth in the tree (0 = root).
    pub depth: u8,
    /// Whether this is a boolean toggle.
    pub is_toggle: bool,
    /// Category key for identifying the setting.
    pub key: String,
}

/// State for the configuration panel.
pub struct ConfigPanelState {
    /// Whether the config panel is active.
    pub active: bool,
    /// Flattened tree of config nodes.
    pub nodes: Vec<ConfigNode>,
    /// Currently selected node index.
    pub selected: usize,
    /// Whether currently editing a value.
    pub editing: bool,
    /// Edit buffer for the selected value.
    pub edit_buffer: String,
    /// Whether there are unsaved changes.
    pub has_pending_changes: bool,
    /// Current config scope.
    pub scope: ConfigScope,
    /// Available group names for scope switching.
    pub available_groups: Vec<String>,
    /// Available toon names for scope switching.
    pub available_toons: Vec<String>,
    /// Whether the scope selector bar is focused (Tab toggles this).
    pub scope_selector_focused: bool,
    /// Index within the scope selector items.
    pub scope_selector_index: usize,
}

impl ConfigPanelState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            nodes: Self::build_default_tree(),
            selected: 0,
            editing: false,
            edit_buffer: String::new(),
            has_pending_changes: false,
            scope: ConfigScope::Global,
            available_groups: Vec::new(),
            available_toons: Vec::new(),
            scope_selector_focused: false,
            scope_selector_index: 0,
        }
    }

    /// All scope options: Global, then groups, then toons.
    fn scope_options(&self) -> Vec<ConfigScope> {
        let mut opts = vec![ConfigScope::Global];
        for g in &self.available_groups {
            opts.push(ConfigScope::Group(g.clone()));
        }
        for t in &self.available_toons {
            opts.push(ConfigScope::Toon(t.clone()));
        }
        opts
    }

    /// Move scope selector left.
    pub fn scope_prev(&mut self) {
        let opts = self.scope_options();
        if opts.is_empty() {
            return;
        }
        if self.scope_selector_index > 0 {
            self.scope_selector_index -= 1;
        } else {
            self.scope_selector_index = opts.len() - 1;
        }
        self.apply_scope_selection();
    }

    /// Move scope selector right.
    pub fn scope_next(&mut self) {
        let opts = self.scope_options();
        if opts.is_empty() {
            return;
        }
        if self.scope_selector_index + 1 < opts.len() {
            self.scope_selector_index += 1;
        } else {
            self.scope_selector_index = 0;
        }
        self.apply_scope_selection();
    }

    /// Apply the currently selected scope and rebuild the tree.
    fn apply_scope_selection(&mut self) {
        let opts = self.scope_options();
        if let Some(new_scope) = opts.get(self.scope_selector_index).cloned() {
            self.scope = new_scope;
        }
        self.rebuild_tree();
    }

    /// Rebuild the tree for the current scope.
    pub fn rebuild_tree(&mut self) {
        self.nodes = match &self.scope {
            ConfigScope::Global => Self::build_default_tree(),
            ConfigScope::Group(name) => Self::build_group_tree(name),
            ConfigScope::Toon(name) => Self::build_toon_tree(name),
        };
        self.selected = 0;
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Reset current scope's overrides back to global defaults.
    pub fn reset_to_defaults(&mut self) {
        if self.scope != ConfigScope::Global {
            self.rebuild_tree();
            self.has_pending_changes = true;
        }
    }

    /// Build the default (global) configuration tree.
    fn build_default_tree() -> Vec<ConfigNode> {
        vec![
            // General
            ConfigNode {
                label: "General".into(),
                value: None,
                expanded: true,
                depth: 0,
                is_toggle: false,
                key: "general".into(),
            },
            ConfigNode {
                label: "Theme".into(),
                value: Some("Dark Modern".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "general.theme".into(),
            },
            ConfigNode {
                label: "Refresh Rate (ms)".into(),
                value: Some("250".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "general.refresh_rate".into(),
            },
            ConfigNode {
                label: "Log Level".into(),
                value: Some("info".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "general.log_level".into(),
            },
            ConfigNode {
                label: "Privacy Mode".into(),
                value: Some("Off".into()),
                expanded: false,
                depth: 1,
                is_toggle: true,
                key: "general.privacy".into(),
            },
            // Combat
            ConfigNode {
                label: "Combat".into(),
                value: None,
                expanded: true,
                depth: 0,
                is_toggle: false,
                key: "combat".into(),
            },
            ConfigNode {
                label: "Main Assist".into(),
                value: Some("(not set)".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "combat.main_assist".into(),
            },
            ConfigNode {
                label: "Main Tank".into(),
                value: Some("(not set)".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "combat.main_tank".into(),
            },
            ConfigNode {
                label: "Heal Cancel".into(),
                value: Some("On".into()),
                expanded: false,
                depth: 1,
                is_toggle: true,
                key: "combat.heal_cancel".into(),
            },
            ConfigNode {
                label: "CH Chain".into(),
                value: None,
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "combat.ch_chain".into(),
            },
            // Camp
            ConfigNode {
                label: "Camp".into(),
                value: None,
                expanded: true,
                depth: 0,
                is_toggle: false,
                key: "camp".into(),
            },
            ConfigNode {
                label: "Active Camp".into(),
                value: Some("(none)".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "camp.active".into(),
            },
            ConfigNode {
                label: "Pull Radius".into(),
                value: Some("200".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "camp.pull_radius".into(),
            },
            ConfigNode {
                label: "Assist Delay (ms)".into(),
                value: Some("500".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "camp.assist_delay".into(),
            },
            ConfigNode {
                label: "Operating Mode".into(),
                value: Some("Camp".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "camp.mode".into(),
            },
            // Clients
            ConfigNode {
                label: "Clients".into(),
                value: None,
                expanded: false,
                depth: 0,
                is_toggle: false,
                key: "clients".into(),
            },
        ]
    }

    /// Build per-group config tree with overridable settings.
    fn build_group_tree(group_name: &str) -> Vec<ConfigNode> {
        vec![
            // Group Info
            ConfigNode {
                label: "Group Info".into(),
                value: None,
                expanded: true,
                depth: 0,
                is_toggle: false,
                key: "group.info".into(),
            },
            ConfigNode {
                label: "Group Name".into(),
                value: Some(group_name.to_string()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "group.name".into(),
            },
            ConfigNode {
                label: "Camp Name".into(),
                value: Some("(inherit global)".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "group.camp_name".into(),
            },
            // Overrides
            ConfigNode {
                label: "Overrides".into(),
                value: None,
                expanded: true,
                depth: 0,
                is_toggle: false,
                key: "group.overrides".into(),
            },
            ConfigNode {
                label: "Pull Radius".into(),
                value: Some("(inherit global)".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "group.pull_radius".into(),
            },
            ConfigNode {
                label: "Assist Delay (ms)".into(),
                value: Some("(inherit global)".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "group.assist_delay".into(),
            },
            ConfigNode {
                label: "Operating Mode".into(),
                value: Some("(inherit global)".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "group.mode".into(),
            },
            ConfigNode {
                label: "Main Assist".into(),
                value: Some("(inherit global)".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "group.main_assist".into(),
            },
            ConfigNode {
                label: "Main Tank".into(),
                value: Some("(inherit global)".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "group.main_tank".into(),
            },
        ]
    }

    /// Build per-toon config tree with role, behavior, and threshold overrides.
    fn build_toon_tree(toon_name: &str) -> Vec<ConfigNode> {
        vec![
            // Identity
            ConfigNode {
                label: "Character".into(),
                value: None,
                expanded: true,
                depth: 0,
                is_toggle: false,
                key: "toon.character".into(),
            },
            ConfigNode {
                label: "Name".into(),
                value: Some(toon_name.to_string()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "toon.name".into(),
            },
            ConfigNode {
                label: "Role".into(),
                value: Some("DPS".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "toon.role".into(),
            },
            ConfigNode {
                label: "Behavior Mode".into(),
                value: Some("Camp".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "toon.behavior_mode".into(),
            },
            // Thresholds
            ConfigNode {
                label: "Thresholds".into(),
                value: None,
                expanded: true,
                depth: 0,
                is_toggle: false,
                key: "toon.thresholds".into(),
            },
            ConfigNode {
                label: "Mana Sit %".into(),
                value: Some("30".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "toon.mana_sit_pct".into(),
            },
            ConfigNode {
                label: "Heal At %".into(),
                value: Some("70".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "toon.heal_at_pct".into(),
            },
            ConfigNode {
                label: "Nuke At %".into(),
                value: Some("95".into()),
                expanded: false,
                depth: 1,
                is_toggle: false,
                key: "toon.nuke_at_pct".into(),
            },
            // Auto-Accept
            ConfigNode {
                label: "Auto-Accept".into(),
                value: None,
                expanded: true,
                depth: 0,
                is_toggle: false,
                key: "toon.auto_accept".into(),
            },
            ConfigNode {
                label: "Group Invite".into(),
                value: Some("On".into()),
                expanded: false,
                depth: 1,
                is_toggle: true,
                key: "toon.accept_group".into(),
            },
            ConfigNode {
                label: "Trade".into(),
                value: Some("On".into()),
                expanded: false,
                depth: 1,
                is_toggle: true,
                key: "toon.accept_trade".into(),
            },
            ConfigNode {
                label: "Resurrect".into(),
                value: Some("On".into()),
                expanded: false,
                depth: 1,
                is_toggle: true,
                key: "toon.accept_rez".into(),
            },
        ]
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        let visible = self.visible_indices();
        if let Some(pos) = visible.iter().position(|&i| i == self.selected)
            && pos > 0
        {
            self.selected = visible[pos - 1];
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        let visible = self.visible_indices();
        if let Some(pos) = visible.iter().position(|&i| i == self.selected)
            && pos + 1 < visible.len()
        {
            self.selected = visible[pos + 1];
        }
    }

    /// Toggle expand/collapse on the selected branch node.
    pub fn toggle_expand(&mut self) {
        if self.selected < self.nodes.len() {
            let node = &self.nodes[self.selected];
            if node.value.is_none() {
                let new_val = !node.expanded;
                self.nodes[self.selected].expanded = new_val;
            }
        }
    }

    /// Toggle a boolean value.
    pub fn toggle_value(&mut self) {
        if self.selected < self.nodes.len() && self.nodes[self.selected].is_toggle {
            let current = self.nodes[self.selected].value.as_deref().unwrap_or("Off");
            let new_val = if current == "On" { "Off" } else { "On" };
            self.nodes[self.selected].value = Some(new_val.to_string());
            self.has_pending_changes = true;
        }
    }

    /// Start editing the selected node's value.
    pub fn start_edit(&mut self) {
        if self.selected < self.nodes.len() {
            let node = &self.nodes[self.selected];
            if let Some(ref val) = node.value
                && !node.is_toggle
            {
                self.editing = true;
                self.edit_buffer = val.clone();
            }
        }
    }

    /// Commit the edit buffer to the selected node.
    pub fn commit_edit(&mut self) {
        if self.editing && self.selected < self.nodes.len() {
            self.nodes[self.selected].value = Some(self.edit_buffer.clone());
            self.editing = false;
            self.has_pending_changes = true;
        }
    }

    /// Cancel editing.
    pub fn cancel_edit(&mut self) {
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Get indices of visible nodes (respecting collapse state).
    fn visible_indices(&self) -> Vec<usize> {
        let mut result = Vec::new();
        let mut skip_depth: Option<u8> = None;

        for (i, node) in self.nodes.iter().enumerate() {
            if let Some(sd) = skip_depth {
                if node.depth > sd {
                    continue;
                }
                skip_depth = None;
            }

            result.push(i);

            if node.value.is_none() && !node.expanded {
                skip_depth = Some(node.depth);
            }
        }
        result
    }

    /// Update available groups and toons from app state.
    pub fn set_available_scopes(&mut self, groups: Vec<String>, toons: Vec<String>) {
        self.available_groups = groups;
        self.available_toons = toons;
    }

    /// Update config values from app state (global scope).
    pub fn sync_from_app(
        &mut self,
        theme_label: &str,
        privacy: bool,
        main_assist: Option<&str>,
        main_tank: Option<&str>,
        heal_cancel: bool,
        mode: &str,
    ) {
        for node in &mut self.nodes {
            match node.key.as_str() {
                "general.theme" => node.value = Some(theme_label.to_string()),
                "general.privacy" => {
                    node.value = Some(if privacy { "On" } else { "Off" }.to_string());
                }
                "combat.main_assist" => {
                    node.value = Some(main_assist.unwrap_or("(not set)").to_string());
                }
                "combat.main_tank" => {
                    node.value = Some(main_tank.unwrap_or("(not set)").to_string());
                }
                "combat.heal_cancel" => {
                    node.value = Some(if heal_cancel { "On" } else { "Off" }.to_string());
                }
                "camp.mode" => node.value = Some(mode.to_string()),
                _ => {}
            }
        }
    }
}

/// Widget that renders the config panel.
pub struct ConfigPanelWidget<'a> {
    state: &'a ConfigPanelState,
    accent_color: Color,
    border_color: Color,
}

impl<'a> ConfigPanelWidget<'a> {
    pub fn new(state: &'a ConfigPanelState) -> Self {
        Self {
            state,
            accent_color: Color::Cyan,
            border_color: Color::Gray,
        }
    }

    pub fn accent_color(mut self, color: Color) -> Self {
        self.accent_color = color;
        self
    }

    pub fn border_color(mut self, color: Color) -> Self {
        self.border_color = color;
        self
    }
}

impl Widget for ConfigPanelWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = if self.state.has_pending_changes {
            " Config · ~/.config/textquest/config.ron [modified] "
        } else {
            " Config · ~/.config/textquest/config.ron "
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.accent_color)); // cyan border

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 || inner.width < 10 {
            return;
        }

        // ── Scope selector bar (row 0) ──
        let scope_y = inner.y;
        let scope_opts = self.state.scope_options();
        let mut x = inner.x;
        for (i, scope) in scope_opts.iter().enumerate() {
            let label = format!(" {} ", scope.label());
            let is_active = i == self.state.scope_selector_index;
            let style = if is_active {
                Style::default()
                    .fg(Color::Black)
                    .bg(self.accent_color)
                    .add_modifier(Modifier::BOLD)
            } else if self.state.scope_selector_focused {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Gray)
            };
            let len = label.len() as u16;
            if x + len <= inner.x + inner.width {
                buf.set_string(x, scope_y, &label, style);
                x += len + 1;
            }
        }

        // Hint for scope switching
        let hint = if self.state.scope_selector_focused {
            "←/→ scope  Tab tree  R reset"
        } else {
            "Tab scope"
        };
        let hint_x = inner.x + inner.width.saturating_sub(hint.len() as u16);
        buf.set_string(hint_x, scope_y, hint, Style::default().fg(Color::DarkGray));

        // ── Separator ──
        let sep_y = inner.y + 1;
        if sep_y >= inner.y + inner.height {
            return;
        }
        let separator = "─".repeat(inner.width as usize);
        buf.set_string(
            inner.x,
            sep_y,
            &separator,
            Style::default().fg(Color::DarkGray),
        );

        // ── Tree view (starts at row 2) ──
        let tree_start_y = inner.y + 2;
        let tree_height = inner.height.saturating_sub(2);

        let visible = self.state.visible_indices();

        for (row_idx, &node_idx) in visible.iter().enumerate() {
            let y = tree_start_y + row_idx as u16;
            if y >= tree_start_y + tree_height {
                break;
            }

            let node = &self.state.nodes[node_idx];
            let indent = "  ".repeat(node.depth as usize);
            let is_selected = node_idx == self.state.selected && !self.state.scope_selector_focused;

            // Branch indicator: ▾ (expanded), ▸ (collapsed), · (leaf) — in magenta
            let (branch_char, branch_style) = if node.value.is_none() {
                let glyph = if node.expanded { "▾" } else { "▸" };
                (
                    glyph,
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ("·", Style::default().fg(Color::Magenta))
            };

            // Label styling: section names in cyan, leaf items in secondary
            let label_style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(self.accent_color)
                    .add_modifier(Modifier::BOLD)
            } else if node.value.is_none() {
                // Section header (branch node)
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                // Leaf item
                Style::default().fg(Color::DarkGray)
            };

            // Render indent + branch glyph + label
            let indent_text = format!("{}{} ", indent, branch_char);
            buf.set_string(inner.x, y, &indent_text, branch_style);
            buf.set_string(
                inner.x + indent_text.len() as u16,
                y,
                &node.label,
                label_style,
            );

            // Render value on the right: {key} = {value}
            if let Some(ref val) = node.value {
                let display_val = if self.state.editing && is_selected {
                    format!("{}_", self.state.edit_buffer)
                } else {
                    val.clone()
                };

                let val_style = if is_selected && self.state.editing {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
                } else if node.is_toggle {
                    if val == "On" {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Red)
                    }
                } else if val.starts_with("(inherit") {
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC)
                } else {
                    Style::default().fg(Color::White) // bright
                };

                let val_x = inner.x + inner.width.saturating_sub(display_val.len() as u16 + 1);
                if val_x > inner.x + 20 {
                    // Render {key} = {value}
                    buf.set_string(val_x, y, " = ", Style::default().fg(Color::DarkGray));
                    buf.set_string(val_x + 3, y, &display_val, val_style);
                }
            }
        }

        // Footer with keybinds
        let footer_y = inner.y + inner.height.saturating_sub(1);
        if footer_y < buf.area().height {
            let footer_text = "↑↓ navigate · enter edit · s save · r reload";
            let footer_style = Style::default().fg(Color::DarkGray);
            buf.set_string(inner.x, footer_y, footer_text, footer_style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_panel_navigation() {
        let mut state = ConfigPanelState::new();
        assert_eq!(state.selected, 0);

        state.select_next();
        assert!(state.selected > 0);

        state.select_prev();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn config_panel_toggle() {
        let mut state = ConfigPanelState::new();
        // Find the privacy toggle
        let privacy_idx = state
            .nodes
            .iter()
            .position(|n| n.key == "general.privacy")
            .unwrap();
        state.selected = privacy_idx;

        assert_eq!(state.nodes[privacy_idx].value.as_deref(), Some("Off"));
        state.toggle_value();
        assert_eq!(state.nodes[privacy_idx].value.as_deref(), Some("On"));
        assert!(state.has_pending_changes);
    }

    #[test]
    fn config_panel_collapse() {
        let mut state = ConfigPanelState::new();
        let visible_before = state.visible_indices().len();

        // Collapse "General" (index 0)
        state.selected = 0;
        state.toggle_expand();

        let visible_after = state.visible_indices().len();
        assert!(visible_after < visible_before);
    }

    #[test]
    fn config_panel_editing() {
        let mut state = ConfigPanelState::new();
        // Select refresh rate
        let idx = state
            .nodes
            .iter()
            .position(|n| n.key == "general.refresh_rate")
            .unwrap();
        state.selected = idx;

        state.start_edit();
        assert!(state.editing);
        assert_eq!(state.edit_buffer, "250");

        state.edit_buffer = "500".to_string();
        state.commit_edit();
        assert!(!state.editing);
        assert_eq!(state.nodes[idx].value.as_deref(), Some("500"));
        assert!(state.has_pending_changes);
    }

    #[test]
    fn sync_from_app_updates_values() {
        let mut state = ConfigPanelState::new();
        state.sync_from_app("Dracula", true, Some("Warrior"), None, false, "Hunt");

        let theme_val = state
            .nodes
            .iter()
            .find(|n| n.key == "general.theme")
            .and_then(|n| n.value.as_deref());
        assert_eq!(theme_val, Some("Dracula"));
    }

    #[test]
    fn scope_defaults_to_global() {
        let state = ConfigPanelState::new();
        assert_eq!(state.scope, ConfigScope::Global);
        assert_eq!(state.scope_selector_index, 0);
        assert!(!state.scope_selector_focused);
    }

    #[test]
    fn scope_switching_rebuilds_tree() {
        let mut state = ConfigPanelState::new();
        state.set_available_scopes(
            vec!["Alpha".into(), "Bravo".into()],
            vec!["Cleric01".into(), "Warrior01".into()],
        );

        // Switch to group scope
        state.scope_next(); // index 1 = Group: Alpha
        assert_eq!(state.scope, ConfigScope::Group("Alpha".into()));
        assert!(state.nodes.iter().any(|n| n.key == "group.pull_radius"));

        // Switch to toon scope
        state.scope_selector_index = 3; // Toon: Cleric01
        state.apply_scope_selection();
        assert_eq!(state.scope, ConfigScope::Toon("Cleric01".into()));
        assert!(state.nodes.iter().any(|n| n.key == "toon.role"));
    }

    #[test]
    fn scope_wraps_around() {
        let mut state = ConfigPanelState::new();
        state.set_available_scopes(vec!["Alpha".into()], vec![]);

        // At index 0 (Global), go prev → wraps to last
        state.scope_prev();
        assert_eq!(state.scope_selector_index, 1);
        assert_eq!(state.scope, ConfigScope::Group("Alpha".into()));

        // Go next → wraps to 0
        state.scope_next();
        assert_eq!(state.scope_selector_index, 0);
        assert_eq!(state.scope, ConfigScope::Global);
    }

    #[test]
    fn toon_tree_has_role_and_thresholds() {
        let nodes = ConfigPanelState::build_toon_tree("TestToon");
        let keys: Vec<&str> = nodes.iter().map(|n| n.key.as_str()).collect();
        assert!(keys.contains(&"toon.role"));
        assert!(keys.contains(&"toon.behavior_mode"));
        assert!(keys.contains(&"toon.mana_sit_pct"));
        assert!(keys.contains(&"toon.heal_at_pct"));
        assert!(keys.contains(&"toon.nuke_at_pct"));
        assert!(keys.contains(&"toon.accept_group"));
        assert!(keys.contains(&"toon.accept_trade"));
        assert!(keys.contains(&"toon.accept_rez"));
    }

    #[test]
    fn group_tree_has_overrides() {
        let nodes = ConfigPanelState::build_group_tree("Alpha");
        let keys: Vec<&str> = nodes.iter().map(|n| n.key.as_str()).collect();
        assert!(keys.contains(&"group.name"));
        assert!(keys.contains(&"group.pull_radius"));
        assert!(keys.contains(&"group.assist_delay"));
        assert!(keys.contains(&"group.mode"));
        assert!(keys.contains(&"group.camp_name"));
        assert!(keys.contains(&"group.main_assist"));
        assert!(keys.contains(&"group.main_tank"));
    }

    #[test]
    fn group_tree_defaults_show_inherit() {
        let nodes = ConfigPanelState::build_group_tree("Alpha");
        let pull = nodes.iter().find(|n| n.key == "group.pull_radius").unwrap();
        assert_eq!(pull.value.as_deref(), Some("(inherit global)"));
    }

    #[test]
    fn reset_to_defaults_rebuilds() {
        let mut state = ConfigPanelState::new();
        state.set_available_scopes(vec!["Alpha".into()], vec![]);
        state.scope_next(); // Group: Alpha

        // Modify a value
        let idx = state
            .nodes
            .iter()
            .position(|n| n.key == "group.pull_radius")
            .unwrap();
        state.selected = idx;
        state.start_edit();
        state.edit_buffer = "300".to_string();
        state.commit_edit();
        assert_eq!(state.nodes[idx].value.as_deref(), Some("300"));

        // Reset
        state.reset_to_defaults();
        let idx = state
            .nodes
            .iter()
            .position(|n| n.key == "group.pull_radius")
            .unwrap();
        assert_eq!(state.nodes[idx].value.as_deref(), Some("(inherit global)"));
        assert!(state.has_pending_changes);
    }

    #[test]
    fn scope_label_formatting() {
        assert_eq!(ConfigScope::Global.label(), "Global");
        assert_eq!(ConfigScope::Group("Alpha".into()).label(), "Group: Alpha");
        assert_eq!(
            ConfigScope::Toon("Cleric01".into()).label(),
            "Toon: Cleric01"
        );
    }
}
