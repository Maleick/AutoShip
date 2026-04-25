//! Dropdown menu bar system for the TUI.
//!
//! Provides a top-level menu bar with categorized commands that can be
//! navigated with arrow keys. Toggled with F10 or Alt.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

/// A single item in a dropdown menu.
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Display label for the item.
    pub label: &'static str,
    /// The command to execute when selected (same as `:` command bar input).
    pub command: &'static str,
    /// Whether selecting this item should prefill command mode instead of
    /// executing immediately.
    pub requires_input: bool,
    /// Keyboard shortcut hint displayed on the right.
    pub shortcut: &'static str,
}

/// A top-level menu category containing items.
#[derive(Debug, Clone)]
pub struct MenuCategory {
    /// Category name displayed in the menu bar.
    pub name: &'static str,
    /// Items within this category.
    pub items: &'static [MenuItem],
}

/// All menu categories.
pub const MENU_CATEGORIES: &[MenuCategory] = &[
    MenuCategory {
        name: "Session",
        items: &[
            MenuItem {
                label: "Help",
                command: "help",
                requires_input: false,
                shortcut: "?",
            },
            MenuItem {
                label: "Commands",
                command: "commands",
                requires_input: false,
                shortcut: "",
            },
            MenuItem {
                label: "Status",
                command: "status",
                requires_input: false,
                shortcut: "",
            },
            MenuItem {
                label: "Inject DLL",
                command: "inject",
                requires_input: false,
                shortcut: "",
            },
            MenuItem {
                label: "Login",
                command: "login",
                requires_input: true,
                shortcut: "",
            },
            MenuItem {
                label: "Launch All",
                command: "login all",
                requires_input: false,
                shortcut: "",
            },
            MenuItem {
                label: "Stop All",
                command: "stop all",
                requires_input: false,
                shortcut: "",
            },
            MenuItem {
                label: "Restart All",
                command: "restart all",
                requires_input: false,
                shortcut: "",
            },
        ],
    },
    MenuCategory {
        name: "Camp",
        items: &[
            MenuItem {
                label: "Start Camp",
                command: "camp start",
                requires_input: true,
                shortcut: "",
            },
            MenuItem {
                label: "Stop Camp",
                command: "camp stop",
                requires_input: false,
                shortcut: "",
            },
            MenuItem {
                label: "Camp List",
                command: "camp list",
                requires_input: false,
                shortcut: "",
            },
            MenuItem {
                label: "Add Camp",
                command: "camp add",
                requires_input: true,
                shortcut: "",
            },
            MenuItem {
                label: "Remove Camp",
                command: "camp remove",
                requires_input: true,
                shortcut: "",
            },
            MenuItem {
                label: "Next Camp",
                command: "camp next",
                requires_input: false,
                shortcut: "",
            },
            MenuItem {
                label: "Prev Camp",
                command: "camp prev",
                requires_input: false,
                shortcut: "",
            },
            MenuItem {
                label: "Camp Mode",
                command: "mode camp",
                requires_input: false,
                shortcut: "",
            },
            MenuItem {
                label: "Hunt Mode",
                command: "mode hunt",
                requires_input: false,
                shortcut: "",
            },
        ],
    },
    MenuCategory {
        name: "Navigation",
        items: &[
            MenuItem {
                label: "Nav to Camp",
                command: "nav",
                requires_input: true,
                shortcut: "",
            },
            MenuItem {
                label: "Nav to Coords",
                command: "nav",
                requires_input: true,
                shortcut: "",
            },
            MenuItem {
                label: "Nav to Zone",
                command: "nav",
                requires_input: true,
                shortcut: "",
            },
        ],
    },
    MenuCategory {
        name: "Combat",
        items: &[
            MenuItem {
                label: "Engage",
                command: "engage",
                requires_input: false,
                shortcut: "e",
            },
            MenuItem {
                label: "Disengage",
                command: "disengage",
                requires_input: false,
                shortcut: "d",
            },
            MenuItem {
                label: "Main Assist",
                command: "ma",
                requires_input: true,
                shortcut: "",
            },
            MenuItem {
                label: "Main Tank",
                command: "mt",
                requires_input: true,
                shortcut: "",
            },
            MenuItem {
                label: "Heal Cancel",
                command: "heal cancel",
                requires_input: false,
                shortcut: "",
            },
            MenuItem {
                label: "CH Chain",
                command: "chui",
                requires_input: false,
                shortcut: "",
            },
            MenuItem {
                label: "Loot",
                command: "loot",
                requires_input: false,
                shortcut: "l",
            },
        ],
    },
    MenuCategory {
        name: "Groups",
        items: &[
            MenuItem {
                label: "Invite",
                command: "invite",
                requires_input: true,
                shortcut: "",
            },
            MenuItem {
                label: "Accept",
                command: "accept",
                requires_input: false,
                shortcut: "",
            },
            MenuItem {
                label: "G1 Focus",
                command: "G1",
                requires_input: false,
                shortcut: "Shift+1",
            },
            MenuItem {
                label: "G2 Focus",
                command: "G2",
                requires_input: false,
                shortcut: "Shift+2",
            },
            MenuItem {
                label: "G3 Focus",
                command: "G3",
                requires_input: false,
                shortcut: "Shift+3",
            },
            MenuItem {
                label: "G4 Focus",
                command: "G4",
                requires_input: false,
                shortcut: "Shift+4",
            },
            MenuItem {
                label: "G5 Focus",
                command: "G5",
                requires_input: false,
                shortcut: "Shift+5",
            },
            MenuItem {
                label: "G6 Focus",
                command: "G6",
                requires_input: false,
                shortcut: "Shift+6",
            },
        ],
    },
    MenuCategory {
        name: "Config",
        items: &[
            MenuItem {
                label: "Theme",
                command: "theme",
                requires_input: false,
                shortcut: "T",
            },
            MenuItem {
                label: "Privacy Toggle",
                command: "privacy",
                requires_input: false,
                shortcut: "p",
            },
        ],
    },
];

/// State for the dropdown menu system.
pub struct MenuState {
    /// Whether the menu bar is active (dropdown visible).
    pub active: bool,
    /// Currently selected category index.
    pub selected_category: usize,
    /// Currently selected item index within the active category.
    pub selected_item: usize,
}

impl MenuState {
    /// Create a new menu state (inactive by default).
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            selected_category: 0,
            selected_item: 0,
        }
    }

    /// Toggle the menu active state.
    pub fn toggle(&mut self) {
        self.active = !self.active;
        if self.active {
            self.selected_item = 0;
        }
    }

    /// Move to the next category (wrapping).
    pub fn next_category(&mut self) {
        self.selected_category = (self.selected_category + 1) % MENU_CATEGORIES.len();
        self.selected_item = 0;
    }

    /// Move to the previous category (wrapping).
    pub fn prev_category(&mut self) {
        if self.selected_category == 0 {
            self.selected_category = MENU_CATEGORIES.len() - 1;
        } else {
            self.selected_category -= 1;
        }
        self.selected_item = 0;
    }

    /// Move to the next item within the current category (wrapping).
    pub fn next_item(&mut self) {
        let cat = &MENU_CATEGORIES[self.selected_category];
        self.selected_item = (self.selected_item + 1) % cat.items.len();
    }

    /// Move to the previous item within the current category (wrapping).
    pub fn prev_item(&mut self) {
        let cat = &MENU_CATEGORIES[self.selected_category];
        if self.selected_item == 0 {
            self.selected_item = cat.items.len() - 1;
        } else {
            self.selected_item -= 1;
        }
    }

    /// Get the command for the currently selected item.
    #[must_use]
    pub fn selected_command(&self) -> &'static str {
        self.selected_item().command
    }

    /// Get the currently selected menu item.
    #[must_use]
    pub fn selected_item(&self) -> &'static MenuItem {
        let cat = &MENU_CATEGORIES[self.selected_category];
        &cat.items[self.selected_item]
    }
}

/// Widget that renders the menu bar (single line with category names).
pub struct MenuBar<'a> {
    state: &'a MenuState,
    highlight_style: Style,
    normal_style: Style,
}

impl<'a> MenuBar<'a> {
    pub fn new(state: &'a MenuState) -> Self {
        Self {
            state,
            highlight_style: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            normal_style: Style::default().fg(Color::Gray),
        }
    }

    pub fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = style;
        self
    }

    pub fn normal_style(mut self, style: Style) -> Self {
        self.normal_style = style;
        self
    }
}

impl Widget for MenuBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let mut x = area.x;
        for (i, cat) in MENU_CATEGORIES.iter().enumerate() {
            let label = format!(" {} ", cat.name);
            let style = if self.state.active && i == self.state.selected_category {
                self.highlight_style // active = inverse magenta
            } else {
                self.normal_style // inactive = secondary
            };

            let width = label.len() as u16;
            if x + width > area.x + area.width {
                break;
            }

            buf.set_string(x, area.y, &label, style);
            x += width;
        }

        // Fill remaining space and add separator line
        for col in x..area.x + area.width {
            buf.set_string(col, area.y, " ", self.normal_style);
        }

        // Add separator line below menu bar if we have space
        if area.height > 1 {
            let sep_y = area.y + 1;
            for col in area.x..area.x + area.width {
                buf.set_string(col, sep_y, "─", self.normal_style);
            }
        }
    }
}

/// Widget that renders the dropdown panel for the active category.
pub struct MenuDropdown<'a> {
    state: &'a MenuState,
    highlight_style: Style,
    normal_style: Style,
    shortcut_style: Style,
}

impl<'a> MenuDropdown<'a> {
    pub fn new(state: &'a MenuState) -> Self {
        Self {
            state,
            highlight_style: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            normal_style: Style::default().fg(Color::White).bg(Color::DarkGray),
            shortcut_style: Style::default().fg(Color::Yellow).bg(Color::DarkGray),
        }
    }

    pub fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = style;
        self
    }

    pub fn normal_style(mut self, style: Style) -> Self {
        self.normal_style = style;
        self
    }

    pub fn shortcut_style(mut self, style: Style) -> Self {
        self.shortcut_style = style;
        self
    }

    /// Calculate the position and size for the dropdown given the menu bar
    /// area.
    pub fn dropdown_rect(&self, menu_bar_area: Rect) -> Rect {
        let cat = &MENU_CATEGORIES[self.state.selected_category];

        // Calculate x offset by summing widths of preceding categories
        let mut x_offset = menu_bar_area.x;
        for c in MENU_CATEGORIES.iter().take(self.state.selected_category) {
            x_offset += (c.name.len() + 2) as u16;
        }

        // Width = max item label + shortcut + padding
        let max_label = cat.items.iter().map(|i| i.label.len()).max().unwrap_or(10);
        let max_shortcut = cat
            .items
            .iter()
            .map(|i| i.shortcut.len())
            .max()
            .unwrap_or(0);
        let width = (max_label + max_shortcut + 6).max(20) as u16;
        let height = cat.items.len() as u16 + 2; // +2 for border

        Rect::new(
            x_offset.min(menu_bar_area.x + menu_bar_area.width.saturating_sub(width)),
            menu_bar_area.y + 1,
            width.min(menu_bar_area.width),
            height,
        )
    }
}

impl Widget for MenuDropdown<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 || area.width < 4 {
            return;
        }

        let cat = &MENU_CATEGORIES[self.state.selected_category];

        // Draw border
        let border_style = self.normal_style;
        buf.set_string(area.x, area.y, "┌", border_style);
        buf.set_string(area.x + area.width - 1, area.y, "┐", border_style);
        for col in (area.x + 1)..area.x + area.width - 1 {
            buf.set_string(col, area.y, "─", border_style);
        }
        let bottom_y = area.y + area.height - 1;
        if bottom_y < buf.area().height {
            buf.set_string(area.x, bottom_y, "└", border_style);
            buf.set_string(area.x + area.width - 1, bottom_y, "┘", border_style);
            for col in (area.x + 1)..area.x + area.width - 1 {
                buf.set_string(col, bottom_y, "─", border_style);
            }
        }

        // Side borders
        for row in (area.y + 1)..bottom_y.min(buf.area().height) {
            buf.set_string(area.x, row, "│", border_style);
            buf.set_string(area.x + area.width - 1, row, "│", border_style);
        }

        // Render items
        let inner_width = area.width.saturating_sub(2) as usize;
        for (i, item) in cat.items.iter().enumerate() {
            let y = area.y + 1 + i as u16;
            if y >= area.y + area.height - 1 || y >= buf.area().height {
                break;
            }

            let is_selected = i == self.state.selected_item;
            let style = if is_selected {
                self.highlight_style
            } else {
                self.normal_style
            };

            // Clear the row
            let blank = " ".repeat(inner_width);
            buf.set_string(area.x + 1, y, &blank, style);

            // Label on the left
            let label = if item.label.len() > inner_width.saturating_sub(8) {
                &item.label[..inner_width.saturating_sub(8)]
            } else {
                item.label
            };
            buf.set_string(area.x + 2, y, label, style);

            // Shortcut on the right
            if !item.shortcut.is_empty() {
                let sc_style = if is_selected {
                    self.highlight_style
                } else {
                    self.shortcut_style
                };
                let sc_x = area.x + area.width - 2 - item.shortcut.len() as u16;
                buf.set_string(sc_x, y, item.shortcut, sc_style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_state_navigation() {
        let mut state = MenuState::new();
        assert!(!state.active);

        state.toggle();
        assert!(state.active);
        assert_eq!(state.selected_category, 0);
        assert_eq!(state.selected_item, 0);

        state.next_category();
        assert_eq!(state.selected_category, 1);

        state.prev_category();
        assert_eq!(state.selected_category, 0);

        state.prev_category();
        assert_eq!(state.selected_category, MENU_CATEGORIES.len() - 1);
    }

    #[test]
    fn menu_item_navigation_wraps() {
        let mut state = MenuState::new();
        state.active = true;

        let items_count = MENU_CATEGORIES[0].items.len();
        state.prev_item();
        assert_eq!(state.selected_item, items_count - 1);

        state.next_item();
        assert_eq!(state.selected_item, 0);
    }

    #[test]
    fn selected_command_returns_correct_item() {
        let mut state = MenuState::new();
        state.active = true;
        assert_eq!(state.selected_command(), "help");

        state.next_item();
        assert_eq!(state.selected_command(), "commands");
    }

    #[test]
    fn all_categories_have_items() {
        for cat in MENU_CATEGORIES {
            assert!(!cat.items.is_empty(), "{} has no items", cat.name);
        }
    }

    #[test]
    fn menu_bar_renders_without_panic() {
        let state = MenuState::new();
        let widget = MenuBar::new(&state);
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
    }
}
