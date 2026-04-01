//! Configuration panel with tree view and inline editing.
//!
//! Provides a dedicated config screen showing configuration hierarchy
//! using a tree structure with inline value editing.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};

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
        }
    }

    /// Build the default configuration tree from current settings.
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

    /// Update config values from app state.
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
            " Configuration [modified] "
        } else {
            " Configuration "
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.border_color));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 || inner.width < 10 {
            return;
        }

        let visible = self.state.visible_indices();

        for (row_idx, &node_idx) in visible.iter().enumerate() {
            let y = inner.y + row_idx as u16;
            if y >= inner.y + inner.height {
                break;
            }

            let node = &self.state.nodes[node_idx];
            let indent = "  ".repeat(node.depth as usize);
            let is_selected = node_idx == self.state.selected;

            // Branch indicator
            let branch_char = if node.value.is_none() {
                if node.expanded { "▾ " } else { "▸ " }
            } else {
                "  "
            };

            let label_style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(self.accent_color)
                    .add_modifier(Modifier::BOLD)
            } else if node.value.is_none() {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let label_text = format!("{indent}{branch_char}{}", node.label);
            buf.set_string(inner.x, y, &label_text, label_style);

            // Render value
            if let Some(ref val) = node.value {
                let display_val = if self.state.editing && is_selected {
                    format!("{}_", self.state.edit_buffer)
                } else {
                    val.clone()
                };

                let val_style = if is_selected && self.state.editing {
                    Style::default().fg(Color::Yellow).bg(Color::DarkGray)
                } else if node.is_toggle {
                    if val == "On" {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Red)
                    }
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let val_x = inner.x + inner.width.saturating_sub(display_val.len() as u16 + 1);
                if val_x > inner.x + label_text.len() as u16 {
                    buf.set_string(val_x, y, &display_val, val_style);
                }
            }
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
}
