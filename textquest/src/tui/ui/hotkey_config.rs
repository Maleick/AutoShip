//! Hotkey configuration UI — interactive rebind panel with conflict detection,
//! save/revert, and import/export profile management.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap},
};
use std::collections::HashMap;

use crate::tui::{
    app::App,
    ui::widgets::{WidthClass, truncate_inline},
};

// ─── Hotkey Configuration State ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeysTab {
    Available,
    Bindings,
    Profiles,
}

impl HotkeysTab {
    fn label(self) -> &'static str {
        match self {
            HotkeysTab::Available => "Commands",
            HotkeysTab::Bindings => "Bindings",
            HotkeysTab::Profiles => "Profiles",
        }
    }

    fn all() -> &'static [HotkeysTab] {
        &[
            HotkeysTab::Available,
            HotkeysTab::Bindings,
            HotkeysTab::Profiles,
        ]
    }

    fn next(self) -> Self {
        match self {
            HotkeysTab::Available => HotkeysTab::Bindings,
            HotkeysTab::Bindings => HotkeysTab::Profiles,
            HotkeysTab::Profiles => HotkeysTab::Available,
        }
    }

    fn prev(self) -> Self {
        match self {
            HotkeysTab::Available => HotkeysTab::Profiles,
            HotkeysTab::Bindings => HotkeysTab::Available,
            HotkeysTab::Profiles => HotkeysTab::Bindings,
        }
    }
}

/// Represents a single command that can be bound to a hotkey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandDef {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
}

/// A hotkey binding with conflict detection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HotkeyBinding {
    pub command_id: String,
    pub key_sequence: String, // e.g., "Ctrl+A", "F1", "Shift+Enter"
}

/// Current hotkey configuration state.
#[derive(Debug, Clone)]
pub struct HotkeysConfigState {
    pub tab: HotkeysTab,
    pub selected_command: usize,
    pub selected_binding: usize,
    pub selected_profile: usize,
    pub scroll_offset: usize,
    pub edit_mode: bool,
    pub edit_buffer: String,
    pub command_list: Vec<CommandDef>,
    pub current_bindings: HashMap<String, HotkeyBinding>,
    pub saved_bindings: HashMap<String, HotkeyBinding>,
    pub available_profiles: Vec<String>,
    pub conflict_message: Option<String>,
    pub status_message: Option<String>,
}

impl Default for HotkeysConfigState {
    fn default() -> Self {
        Self {
            tab: HotkeysTab::Available,
            selected_command: 0,
            selected_binding: 0,
            selected_profile: 0,
            scroll_offset: 0,
            edit_mode: false,
            edit_buffer: String::new(),
            command_list: vec![
                CommandDef {
                    id: "help".to_string(),
                    name: "Help".to_string(),
                    category: "Lifecycle".to_string(),
                    description: "Open help overlay".to_string(),
                },
                CommandDef {
                    id: "mode".to_string(),
                    name: "Toggle Mode".to_string(),
                    category: "Combat".to_string(),
                    description: "Switch between camp and hunt mode".to_string(),
                },
                CommandDef {
                    id: "assist".to_string(),
                    name: "Assist".to_string(),
                    category: "Combat".to_string(),
                    description: "Set main assist target".to_string(),
                },
                CommandDef {
                    id: "tank".to_string(),
                    name: "Tank".to_string(),
                    category: "Combat".to_string(),
                    description: "Set tank target".to_string(),
                },
                CommandDef {
                    id: "recall".to_string(),
                    name: "Recall".to_string(),
                    category: "Navigation".to_string(),
                    description: "Recall group to camp".to_string(),
                },
                CommandDef {
                    id: "waypoint".to_string(),
                    name: "Waypoint".to_string(),
                    category: "Navigation".to_string(),
                    description: "Toggle waypoint display".to_string(),
                },
            ],
            current_bindings: vec![
                (
                    "help".to_string(),
                    HotkeyBinding {
                        command_id: "help".to_string(),
                        key_sequence: "F1".to_string(),
                    },
                ),
                (
                    "mode".to_string(),
                    HotkeyBinding {
                        command_id: "mode".to_string(),
                        key_sequence: "F2".to_string(),
                    },
                ),
                (
                    "assist".to_string(),
                    HotkeyBinding {
                        command_id: "assist".to_string(),
                        key_sequence: "F3".to_string(),
                    },
                ),
            ]
            .into_iter()
            .collect(),
            saved_bindings: vec![
                (
                    "help".to_string(),
                    HotkeyBinding {
                        command_id: "help".to_string(),
                        key_sequence: "F1".to_string(),
                    },
                ),
                (
                    "mode".to_string(),
                    HotkeyBinding {
                        command_id: "mode".to_string(),
                        key_sequence: "F2".to_string(),
                    },
                ),
                (
                    "assist".to_string(),
                    HotkeyBinding {
                        command_id: "assist".to_string(),
                        key_sequence: "F3".to_string(),
                    },
                ),
            ]
            .into_iter()
            .collect(),
            available_profiles: vec![
                "default".to_string(),
                "gaming".to_string(),
                "emacs".to_string(),
            ],
            conflict_message: None,
            status_message: None,
        }
    }
}

impl HotkeysConfigState {
    /// Check if two bindings conflict (same key sequence).
    pub fn check_conflict(&self, new_key: &str, exclude_id: &str) -> Option<String> {
        for (cmd_id, binding) in &self.current_bindings {
            if cmd_id != exclude_id && binding.key_sequence == new_key {
                return Some(format!("Conflicts with: {}", cmd_id));
            }
        }
        None
    }

    /// Save current bindings as the active profile.
    pub fn save_bindings(&mut self) {
        self.saved_bindings.clone_from(&self.current_bindings);
        self.status_message = Some("Bindings saved.".to_string());
    }

    /// Revert to saved bindings.
    pub fn revert_bindings(&mut self) {
        self.current_bindings.clone_from(&self.saved_bindings);
        self.status_message = Some("Bindings reverted.".to_string());
    }
}

// ─── Rendering ──────────────────────────────────────────────────────────────

pub fn draw_hotkey_config(frame: &mut Frame, area: Rect, _app: &mut App) {
    let state = &mut _app.hotkeys_config_state;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(8),    // content
            Constraint::Length(2), // footer
        ])
        .split(area);

    // ── Tab bar ──
    let tab_names: Vec<&str> = HotkeysTab::all().iter().map(|t| t.label()).collect();
    let tab_widget = Tabs::new(tab_names)
        .block(Block::default().borders(Borders::BOTTOM))
        .select(match state.tab {
            HotkeysTab::Available => 0,
            HotkeysTab::Bindings => 1,
            HotkeysTab::Profiles => 2,
        })
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tab_widget, layout[0]);

    // ── Content area ──
    match state.tab {
        HotkeysTab::Available => draw_available_commands(frame, layout[1], state),
        HotkeysTab::Bindings => draw_bindings_editor(frame, layout[1], state),
        HotkeysTab::Profiles => draw_profiles_manager(frame, layout[1], state),
    }

    // ── Footer ──
    draw_footer(frame, layout[2], state);
}

fn draw_available_commands(frame: &mut Frame, area: Rect, state: &HotkeysConfigState) {
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    // Left: command list
    let items: Vec<ListItem> = state
        .command_list
        .iter()
        .enumerate()
        .map(|(idx, cmd)| {
            let selected = idx == state.selected_command;
            let marker = if selected { "▶ " } else { "  " };
            let style = if selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("{}{}", marker, cmd.name)).style(style)
        })
        .collect();

    let list = List::new(items).block(Block::default().title("Commands").borders(Borders::ALL));

    frame.render_widget(list, split[0]);

    // Right: command detail
    if state.selected_command < state.command_list.len() {
        let cmd = &state.command_list[state.selected_command];
        let detail = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&cmd.name),
            ]),
            Line::from(vec![
                Span::styled("Category: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&cmd.category),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Description:",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from(&cmd.description),
        ])
        .block(Block::default().title("Details").borders(Borders::ALL))
        .wrap(Wrap { trim: true });

        frame.render_widget(detail, split[1]);
    }
}

fn draw_bindings_editor(frame: &mut Frame, area: Rect, state: &HotkeysConfigState) {
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left: current bindings list
    let binding_items: Vec<ListItem> = state
        .command_list
        .iter()
        .enumerate()
        .map(|(idx, cmd)| {
            let binding = state.current_bindings.get(&cmd.id);
            let key_str = binding
                .map(|b| b.key_sequence.clone())
                .unwrap_or_else(|| "[unbound]".to_string());
            let selected = idx == state.selected_binding;
            let marker = if selected { "▶ " } else { "  " };
            let style = if selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("{}{:20} → {}", marker, cmd.name, key_str)).style(style)
        })
        .collect();

    let bindings_list = List::new(binding_items).block(
        Block::default()
            .title("Current Bindings")
            .borders(Borders::ALL),
    );

    frame.render_widget(bindings_list, split[0]);

    // Right: edit panel or details
    let edit_block = if state.edit_mode {
        Block::default()
            .title("Press keys to bind (ESC to cancel)")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Cyan))
    } else {
        Block::default()
            .title("Press ENTER to rebind")
            .borders(Borders::ALL)
    };

    let conflict_text = if let Some(ref msg) = state.conflict_message {
        Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![Span::styled(msg, Style::default().fg(Color::Red))]),
        ])
    } else {
        Paragraph::new("")
    };

    let edit_paragraph = conflict_text.block(edit_block).alignment(Alignment::Center);

    frame.render_widget(edit_paragraph, split[1]);
}

fn draw_profiles_manager(frame: &mut Frame, area: Rect, state: &HotkeysConfigState) {
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Available profiles
    let profile_items: Vec<ListItem> = state
        .available_profiles
        .iter()
        .enumerate()
        .map(|(idx, name)| {
            let selected = idx == state.selected_profile;
            let marker = if selected { "▶ " } else { "  " };
            let style = if selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(format!("{}{}", marker, name)).style(style)
        })
        .collect();

    let profiles_list =
        List::new(profile_items).block(Block::default().title("Profiles").borders(Borders::ALL));

    frame.render_widget(profiles_list, split[0]);

    // Actions
    let actions = Paragraph::new(vec![
        Line::from("ENTER — Load Profile"),
        Line::from("S — Save Current"),
        Line::from("E — Export to File"),
        Line::from("I — Import from File"),
        Line::from(""),
        if let Some(ref msg) = state.status_message {
            Line::from(vec![Span::styled(msg, Style::default().fg(Color::Green))])
        } else {
            Line::from("")
        },
    ])
    .block(Block::default().title("Actions").borders(Borders::ALL));

    frame.render_widget(actions, split[1]);
}

fn draw_footer(frame: &mut Frame, area: Rect, state: &HotkeysConfigState) {
    let hints = Paragraph::new(vec![Line::from(vec![
        Span::styled("TAB", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" next  "),
        Span::styled("Shift+TAB", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" prev  "),
        Span::styled("↑↓", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" select  "),
        Span::styled("S", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" save  "),
        Span::styled("R", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" revert  "),
        Span::styled("Q", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" close"),
    ])])
    .block(Block::default().borders(Borders::TOP))
    .alignment(Alignment::Left);

    frame.render_widget(hints, area);
}
