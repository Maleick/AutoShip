//! Input event dispatch and focus management for the overlay.
//!
//! This module handles:
//! - [`InputEvent`] — the unified input event enum (mouse + keyboard)
//! - [`FocusManager`] — tracks which window owns keyboard focus
//! - [`InputDispatcher`] — routes events to the focused/hovered window
//!
//! The actual EQ hook wiring lives in the platform layer (`hooks/`).  When the
//! hook surface is not present the dispatcher still runs as a pure state-machine
//! that can be exercised in unit tests.
//!
//! # EQ input passthrough
//!
//! The dispatcher is "passive by default": it only claims an event when a
//! window is under the cursor (mouse) or holds keyboard focus.  Otherwise the
//! event is passed through to EQ unchanged.

use std::collections::VecDeque;

use super::manager::WindowManager;
use super::widget::{Rect, Widget};

// ── Input event types ────────────────────────────────────────────────────────

/// A mouse or keyboard input event delivered to the overlay.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    // Mouse events
    /// Left-button press at screen position.
    MouseDown { x: f32, y: f32 },
    /// Left-button release at screen position.
    MouseUp { x: f32, y: f32 },
    /// Mouse moved to screen position (button may or may not be held).
    MouseMove { x: f32, y: f32 },
    /// Right-button press — used to open context menus.
    RightClick { x: f32, y: f32 },
    /// Mouse wheel delta (positive = scroll up).
    MouseWheel { x: f32, y: f32, delta: f32 },

    // Keyboard events
    /// A printable character was typed (after OS key repeat / dead-key compose).
    CharInput(char),
    /// A non-printable key was pressed.
    KeyDown(Key),
    /// A non-printable key was released.
    KeyUp(Key),
}

/// Non-printable keys relevant to overlay navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Escape,
    Enter,
    Tab,
    Backspace,
    Delete,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    /// F1-F12 function keys.
    F(u8),
    /// Any other key (platform scancode).
    Other(u32),
}

// ── Dispatch result ──────────────────────────────────────────────────────────

/// Whether the overlay consumed the event (should not reach EQ) or passed it
/// through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchResult {
    /// Event was handled by the overlay — do not forward to EQ.
    Consumed,
    /// Event was not handled — pass through to EQ.
    PassThrough,
}

// ── Focus manager ────────────────────────────────────────────────────────────

/// Tracks which overlay window currently holds keyboard focus.
///
/// When `focused_window` is `None` all keyboard events pass through to EQ.
#[derive(Debug, Default)]
pub struct FocusManager {
    /// ID of the window that currently owns keyboard focus, if any.
    pub focused_window: Option<String>,
    /// ID of the window the cursor is currently hovering over, if any.
    pub hovered_window: Option<String>,
}

impl FocusManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Grant focus to the window with the given id.
    pub fn focus(&mut self, id: impl Into<String>) {
        self.focused_window = Some(id.into());
    }

    /// Remove focus from all windows.
    pub fn blur(&mut self) {
        self.focused_window = None;
    }

    /// Returns `true` when the overlay holds keyboard focus (and should consume
    /// keyboard events before they reach EQ).
    pub fn overlay_has_focus(&self) -> bool {
        self.focused_window.is_some()
    }

    /// Update hover state based on current cursor position.
    ///
    /// Returns the id of the newly hovered window (if any).
    pub fn update_hover<'a>(
        &mut self,
        mgr: &'a WindowManager,
        mx: f32,
        my: f32,
    ) -> Option<&'a str> {
        let hit = mgr
            .windows()
            .iter()
            .rev()
            .find(|w| w.contains_point(mx, my))
            .map(|w| w.id.as_str());
        self.hovered_window = hit.map(|s| s.to_owned());
        hit
    }

    /// Focus the topmost window under the cursor, or blur if none.
    pub fn click_focus(&mut self, mgr: &WindowManager, mx: f32, my: f32) {
        let hit = mgr
            .windows()
            .iter()
            .rev()
            .find(|w| w.contains_point(mx, my))
            .map(|w| w.id.clone());
        self.focused_window = hit;
    }
}

// ── Text input field ─────────────────────────────────────────────────────────

/// An editable single-line text field.
///
/// Stored separately from [`Widget`] so it can live outside the serializable
/// window tree when needed, or be embedded as a widget variant in the future.
#[derive(Debug, Clone, Default)]
pub struct TextInput {
    /// Current text content.
    pub value: String,
    /// Cursor byte offset (clamped to `value.len()`).
    pub cursor: usize,
    /// Maximum allowed character count (0 = unlimited).
    pub max_len: usize,
}

impl TextInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_len(mut self, n: usize) -> Self {
        self.max_len = n;
        self
    }

    /// Insert a character at the cursor position.
    pub fn insert(&mut self, ch: char) {
        if self.max_len > 0 && self.value.chars().count() >= self.max_len {
            return;
        }
        self.value.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    /// Delete the character before the cursor (backspace behaviour).
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        // Find the previous char boundary.
        let prev = self.value[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.value.drain(prev..self.cursor);
        self.cursor = prev;
    }

    /// Delete the character at the cursor position (delete-key behaviour).
    pub fn delete_forward(&mut self) {
        if self.cursor >= self.value.len() {
            return;
        }
        let next_boundary = self.value[self.cursor..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| self.cursor + i)
            .unwrap_or(self.value.len());
        self.value.drain(self.cursor..next_boundary);
    }

    /// Move cursor left by one character.
    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor = self.value[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
    }

    /// Move cursor right by one character.
    pub fn move_right(&mut self) {
        if self.cursor >= self.value.len() {
            return;
        }
        let ch = self.value[self.cursor..].chars().next().unwrap();
        self.cursor += ch.len_utf8();
    }

    /// Move cursor to start.
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end.
    pub fn move_end(&mut self) {
        self.cursor = self.value.len();
    }

    /// Handle a [`Key`] event; returns `true` if consumed.
    pub fn handle_key(&mut self, key: Key) -> bool {
        match key {
            Key::Backspace => {
                self.backspace();
                true
            }
            Key::Delete => {
                self.delete_forward();
                true
            }
            Key::ArrowLeft => {
                self.move_left();
                true
            }
            Key::ArrowRight => {
                self.move_right();
                true
            }
            Key::Home => {
                self.move_home();
                true
            }
            Key::End => {
                self.move_end();
                true
            }
            _ => false,
        }
    }

    /// Handle a [`InputEvent::CharInput`] event.
    pub fn handle_char(&mut self, ch: char) {
        // Ignore control characters.
        if !ch.is_control() {
            self.insert(ch);
        }
    }
}

// ── Tooltip ──────────────────────────────────────────────────────────────────

/// Hover tooltip state managed by the input dispatcher.
#[derive(Debug, Clone, Default)]
pub struct TooltipState {
    /// Text to display (empty = no tooltip visible).
    pub text: String,
    /// Anchor position in screen space.
    pub x: f32,
    pub y: f32,
    /// How long (in frames) the cursor has been over the trigger area.
    pub hover_frames: u32,
    /// Show after this many hover frames.
    pub show_after: u32,
}

impl TooltipState {
    pub fn new(show_after: u32) -> Self {
        Self {
            show_after,
            ..Default::default()
        }
    }

    /// Call each frame the cursor remains over a tooltip target.
    pub fn tick(&mut self, text: impl Into<String>, x: f32, y: f32) {
        self.hover_frames += 1;
        self.text = text.into();
        self.x = x;
        self.y = y;
    }

    /// Call when the cursor leaves the tooltip target.
    pub fn clear(&mut self) {
        self.hover_frames = 0;
        self.text.clear();
    }

    /// Whether the tooltip should be displayed this frame.
    pub fn visible(&self) -> bool {
        !self.text.is_empty() && self.hover_frames >= self.show_after
    }
}

// ── Context menu ─────────────────────────────────────────────────────────────

/// A single item in a context menu.
#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    pub label: String,
    pub id: String,
    pub enabled: bool,
}

impl ContextMenuItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Context menu state (opened by right-click).
#[derive(Debug, Clone, Default)]
pub struct ContextMenu {
    /// Whether the context menu is currently open.
    pub open: bool,
    pub x: f32,
    pub y: f32,
    pub items: Vec<ContextMenuItem>,
    /// ID of the item the user clicked, drained by the caller.
    pub clicked_id: Option<String>,
    /// Index of the currently hovered item.
    pub hovered_idx: Option<usize>,
}

impl ContextMenu {
    /// Item row height in pixels.
    const ITEM_H: f32 = 22.0;
    /// Menu width in pixels.
    const MENU_W: f32 = 160.0;
    /// Top/bottom padding inside the menu.
    const PADDING: f32 = 4.0;

    pub fn new() -> Self {
        Self::default()
    }

    /// Open the context menu at the given screen position with the given items.
    pub fn open(&mut self, x: f32, y: f32, items: Vec<ContextMenuItem>) {
        self.open = true;
        self.x = x;
        self.y = y;
        self.items = items;
        self.clicked_id = None;
        self.hovered_idx = None;
    }

    /// Close the context menu without selecting an item.
    pub fn close(&mut self) {
        self.open = false;
        self.hovered_idx = None;
    }

    /// Bounding rect of the entire menu.
    pub fn menu_rect(&self) -> Rect {
        let h = Self::PADDING * 2.0 + self.items.len() as f32 * Self::ITEM_H;
        Rect::new(self.x, self.y, Self::MENU_W, h)
    }

    /// Bounding rect of a single item row.
    pub fn item_rect(&self, idx: usize) -> Rect {
        Rect::new(
            self.x,
            self.y + Self::PADDING + idx as f32 * Self::ITEM_H,
            Self::MENU_W,
            Self::ITEM_H,
        )
    }

    /// Handle a mouse-move event; updates `hovered_idx`.
    pub fn on_mouse_move(&mut self, mx: f32, my: f32) {
        if !self.open {
            return;
        }
        self.hovered_idx = self
            .items
            .iter()
            .enumerate()
            .find(|(i, item)| item.enabled && self.item_rect(*i).contains(mx, my))
            .map(|(i, _)| i);
    }

    /// Handle a left-click; closes the menu and records `clicked_id` when an
    /// enabled item was hit.  Returns `true` if the click was inside the menu.
    pub fn on_click(&mut self, mx: f32, my: f32) -> bool {
        if !self.open {
            return false;
        }
        if !self.menu_rect().contains(mx, my) {
            self.close();
            return false;
        }
        if let Some(idx) = self.hovered_idx {
            if let Some(item) = self.items.get(idx) {
                if item.enabled {
                    self.clicked_id = Some(item.id.clone());
                }
            }
        }
        self.close();
        true
    }
}

// ── Input dispatcher ─────────────────────────────────────────────────────────

/// Routes input events to the correct overlay subsystem and decides whether the
/// event should pass through to EQ.
///
/// Typical per-frame usage:
/// ```text
/// for event in raw_events {
///     let result = dispatcher.dispatch(event, &mut manager, &mut focus);
///     if result == DispatchResult::PassThrough {
///         send_to_eq(event);
///     }
/// }
/// ```
#[derive(Debug, Default)]
pub struct InputDispatcher {
    /// Pending events to re-process next frame (not currently used, reserved).
    _pending: VecDeque<InputEvent>,
    pub tooltip: TooltipState,
    pub context_menu: ContextMenu,
}

impl InputDispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a single [`InputEvent`].
    ///
    /// Returns [`DispatchResult::Consumed`] if the overlay handled the event and
    /// [`DispatchResult::PassThrough`] if EQ should receive it.
    pub fn dispatch(
        &mut self,
        event: &InputEvent,
        mgr: &mut WindowManager,
        focus: &mut FocusManager,
    ) -> DispatchResult {
        match event {
            InputEvent::MouseDown { x, y } => self.on_mouse_down(*x, *y, mgr, focus),
            InputEvent::MouseUp { x, y } => self.on_mouse_up(*x, *y, mgr, focus),
            InputEvent::MouseMove { x, y } => self.on_mouse_move(*x, *y, mgr, focus),
            InputEvent::RightClick { x, y } => self.on_right_click(*x, *y, mgr, focus),
            InputEvent::MouseWheel { x, y, .. } => self.on_mouse_wheel(*x, *y, mgr, focus),
            InputEvent::CharInput(ch) => self.on_char(*ch, focus),
            InputEvent::KeyDown(key) => self.on_key_down(*key, mgr, focus),
            InputEvent::KeyUp(_) => {
                // Key-up events are pass-through unless overlay has focus.
                if focus.overlay_has_focus() {
                    DispatchResult::Consumed
                } else {
                    DispatchResult::PassThrough
                }
            }
        }
    }

    fn on_mouse_down(
        &mut self,
        mx: f32,
        my: f32,
        mgr: &mut WindowManager,
        focus: &mut FocusManager,
    ) -> DispatchResult {
        // Context menu eats the click if it's open.
        if self.context_menu.open {
            self.context_menu.on_click(mx, my);
            return DispatchResult::Consumed;
        }

        let over_overlay = mgr.windows().iter().any(|w| w.contains_point(mx, my));
        if over_overlay {
            focus.click_focus(mgr, mx, my);
            mgr.mouse_down(mx, my);
            DispatchResult::Consumed
        } else {
            // Click outside overlay — blur and pass through.
            focus.blur();
            DispatchResult::PassThrough
        }
    }

    fn on_mouse_up(
        &mut self,
        mx: f32,
        my: f32,
        mgr: &mut WindowManager,
        focus: &mut FocusManager,
    ) -> DispatchResult {
        mgr.mouse_up(mx, my);
        // If the overlay had focus the up-event belongs to it.
        if focus.overlay_has_focus() {
            DispatchResult::Consumed
        } else {
            DispatchResult::PassThrough
        }
    }

    fn on_mouse_move(
        &mut self,
        mx: f32,
        my: f32,
        mgr: &mut WindowManager,
        focus: &mut FocusManager,
    ) -> DispatchResult {
        mgr.mouse_move(mx, my);
        focus.update_hover(mgr, mx, my);

        if self.context_menu.open {
            self.context_menu.on_mouse_move(mx, my);
        }

        // Mouse-move never blocks EQ — EQ still needs cursor position.
        DispatchResult::PassThrough
    }

    fn on_right_click(
        &mut self,
        mx: f32,
        my: f32,
        mgr: &WindowManager,
        focus: &mut FocusManager,
    ) -> DispatchResult {
        let over_overlay = mgr.windows().iter().any(|w| w.contains_point(mx, my));
        if over_overlay {
            focus.click_focus(mgr, mx, my);
            // Open a default context menu — callers can replace items before
            // the next render pass.
            self.context_menu.open(
                mx,
                my,
                vec![
                    ContextMenuItem::new("close", "Close window"),
                    ContextMenuItem::new("minimize", "Minimize"),
                    ContextMenuItem::new("sep", "─────────────").disabled(),
                    ContextMenuItem::new("theme", "Toggle theme"),
                ],
            );
            DispatchResult::Consumed
        } else {
            DispatchResult::PassThrough
        }
    }

    fn on_mouse_wheel(
        &mut self,
        mx: f32,
        my: f32,
        mgr: &WindowManager,
        _focus: &mut FocusManager,
    ) -> DispatchResult {
        let over_overlay = mgr.windows().iter().any(|w| w.contains_point(mx, my));
        if over_overlay {
            DispatchResult::Consumed
        } else {
            DispatchResult::PassThrough
        }
    }

    fn on_char(&mut self, _ch: char, focus: &FocusManager) -> DispatchResult {
        if focus.overlay_has_focus() {
            DispatchResult::Consumed
        } else {
            DispatchResult::PassThrough
        }
    }

    fn on_key_down(
        &mut self,
        key: Key,
        _mgr: &WindowManager,
        focus: &mut FocusManager,
    ) -> DispatchResult {
        if !focus.overlay_has_focus() {
            return DispatchResult::PassThrough;
        }
        match key {
            // Escape always releases focus back to EQ.
            Key::Escape => {
                focus.blur();
                DispatchResult::Consumed
            }
            _ => DispatchResult::Consumed,
        }
    }
}

// ── Widget-level input helpers ────────────────────────────────────────────────

/// Hit-test a click against the widgets of a specific window and return the
/// index of the widget that was clicked (if any).
pub fn hit_test_widget(window: &super::window::Window, mx: f32, my: f32) -> Option<usize> {
    let rects = window.widget_rects();
    rects
        .iter()
        .enumerate()
        .find(|(_, r)| r.contains(mx, my))
        .map(|(i, _)| i)
}

/// Apply a click to a window's widget at the given index.
///
/// Returns `true` if the widget reacted (e.g. button clicked).
pub fn click_widget(window: &mut super::window::Window, widget_idx: usize) -> bool {
    match window.widgets.get_mut(widget_idx) {
        Some(Widget::Button { clicked, .. }) => {
            *clicked = true;
            true
        }
        Some(Widget::List { selected, items }) => {
            // Lists select by index — caller must map screen y to row.
            // Here we just treat any click as selecting the first item when
            // nothing is yet selected.
            if selected.is_none() && !items.is_empty() {
                *selected = Some(0);
            }
            true
        }
        Some(Widget::Dropdown { open, .. }) => {
            *open = !*open;
            true
        }
        _ => false,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::{manager::WindowManager, theme::Theme, window::Window};

    fn make_mgr_with_win(id: &str) -> WindowManager {
        let mut mgr = WindowManager::new(Theme::Dark);
        let win = Window::new(id, "Test");
        mgr.add_window(win);
        mgr
    }

    // ── FocusManager ─────────────────────────────────────────────────────────

    #[test]
    fn focus_and_blur() {
        let mut fm = FocusManager::new();
        assert!(!fm.overlay_has_focus());
        fm.focus("win1");
        assert!(fm.overlay_has_focus());
        assert_eq!(fm.focused_window.as_deref(), Some("win1"));
        fm.blur();
        assert!(!fm.overlay_has_focus());
    }

    #[test]
    fn click_focus_sets_focused_window() {
        let mgr = make_mgr_with_win("w1");
        let mut fm = FocusManager::new();
        let win = mgr.windows().first().unwrap();
        // Click inside the window.
        fm.click_focus(&mgr, win.x + 5.0, win.y + 5.0);
        assert_eq!(fm.focused_window.as_deref(), Some("w1"));
    }

    #[test]
    fn click_focus_outside_blurs() {
        let mgr = make_mgr_with_win("w1");
        let mut fm = FocusManager::new();
        fm.focus("w1");
        // Click far outside any window.
        fm.click_focus(&mgr, 9999.0, 9999.0);
        assert!(!fm.overlay_has_focus());
    }

    #[test]
    fn hover_update_tracks_window() {
        let mgr = make_mgr_with_win("hw");
        let mut fm = FocusManager::new();
        let win = mgr.windows().first().unwrap();
        let hit = fm.update_hover(&mgr, win.x + 1.0, win.y + 1.0);
        assert_eq!(hit, Some("hw"));
        assert_eq!(fm.hovered_window.as_deref(), Some("hw"));
    }

    #[test]
    fn hover_update_no_hit() {
        let mgr = make_mgr_with_win("hw");
        let mut fm = FocusManager::new();
        let hit = fm.update_hover(&mgr, 9999.0, 9999.0);
        assert!(hit.is_none());
        assert!(fm.hovered_window.is_none());
    }

    // ── Input dispatch ────────────────────────────────────────────────────────

    #[test]
    fn mouse_down_inside_overlay_consumed() {
        let mut mgr = make_mgr_with_win("d");
        let mut fm = FocusManager::new();
        let mut disp = InputDispatcher::new();
        let win = mgr.windows().first().unwrap();
        let ev = InputEvent::MouseDown {
            x: win.x + 5.0,
            y: win.y + 5.0,
        };
        let result = disp.dispatch(&ev, &mut mgr, &mut fm);
        assert_eq!(result, DispatchResult::Consumed);
        assert!(fm.overlay_has_focus());
    }

    #[test]
    fn mouse_down_outside_overlay_passes_through() {
        let mut mgr = make_mgr_with_win("d");
        let mut fm = FocusManager::new();
        let mut disp = InputDispatcher::new();
        let ev = InputEvent::MouseDown {
            x: 9999.0,
            y: 9999.0,
        };
        let result = disp.dispatch(&ev, &mut mgr, &mut fm);
        assert_eq!(result, DispatchResult::PassThrough);
        assert!(!fm.overlay_has_focus());
    }

    #[test]
    fn keyboard_passes_through_when_not_focused() {
        let mut mgr = make_mgr_with_win("k");
        let mut fm = FocusManager::new();
        let mut disp = InputDispatcher::new();
        let ev = InputEvent::CharInput('a');
        assert_eq!(
            disp.dispatch(&ev, &mut mgr, &mut fm),
            DispatchResult::PassThrough
        );
    }

    #[test]
    fn keyboard_consumed_when_focused() {
        let mut mgr = make_mgr_with_win("k");
        let mut fm = FocusManager::new();
        fm.focus("k");
        let mut disp = InputDispatcher::new();
        let ev = InputEvent::CharInput('a');
        assert_eq!(
            disp.dispatch(&ev, &mut mgr, &mut fm),
            DispatchResult::Consumed
        );
    }

    #[test]
    fn escape_releases_focus() {
        let mut mgr = make_mgr_with_win("e");
        let mut fm = FocusManager::new();
        fm.focus("e");
        let mut disp = InputDispatcher::new();
        let ev = InputEvent::KeyDown(Key::Escape);
        assert_eq!(
            disp.dispatch(&ev, &mut mgr, &mut fm),
            DispatchResult::Consumed
        );
        assert!(!fm.overlay_has_focus());
    }

    #[test]
    fn focus_switch_between_windows() {
        let mut mgr = WindowManager::new(Theme::Dark);
        let w1 = Window::new("w1", "Window 1");
        let mut w2 = Window::new("w2", "Window 2");
        // Position w2 to the right so it doesn't overlap w1.
        w2.x = 500.0;
        w2.y = 100.0;
        mgr.add_window(w1);
        mgr.add_window(w2);

        let mut fm = FocusManager::new();
        let mut disp = InputDispatcher::new();

        // Click w1.
        let ev1 = InputEvent::MouseDown { x: 105.0, y: 105.0 };
        disp.dispatch(&ev1, &mut mgr, &mut fm);
        assert_eq!(fm.focused_window.as_deref(), Some("w1"));

        // Click w2.
        let ev2 = InputEvent::MouseDown { x: 505.0, y: 105.0 };
        disp.dispatch(&ev2, &mut mgr, &mut fm);
        assert_eq!(fm.focused_window.as_deref(), Some("w2"));
    }

    // ── Tooltip ──────────────────────────────────────────────────────────────

    #[test]
    fn tooltip_not_visible_before_threshold() {
        let mut tt = TooltipState::new(30);
        for _ in 0..29 {
            tt.tick("Help text", 0.0, 0.0);
        }
        assert!(!tt.visible());
    }

    #[test]
    fn tooltip_visible_after_threshold() {
        let mut tt = TooltipState::new(5);
        for _ in 0..5 {
            tt.tick("Help", 10.0, 20.0);
        }
        assert!(tt.visible());
        assert_eq!(tt.text, "Help");
    }

    #[test]
    fn tooltip_clear_hides() {
        let mut tt = TooltipState::new(1);
        tt.tick("Hello", 0.0, 0.0);
        assert!(tt.visible());
        tt.clear();
        assert!(!tt.visible());
    }

    // ── Context menu ─────────────────────────────────────────────────────────

    #[test]
    fn right_click_opens_context_menu() {
        let mut mgr = make_mgr_with_win("cm");
        let mut fm = FocusManager::new();
        let mut disp = InputDispatcher::new();
        let win = mgr.windows().first().unwrap();
        let ev = InputEvent::RightClick {
            x: win.x + 10.0,
            y: win.y + 10.0,
        };
        let result = disp.dispatch(&ev, &mut mgr, &mut fm);
        assert_eq!(result, DispatchResult::Consumed);
        assert!(disp.context_menu.open);
    }

    #[test]
    fn context_menu_click_selects_item() {
        let mut cm = ContextMenu::new();
        cm.open(
            100.0,
            100.0,
            vec![
                ContextMenuItem::new("copy", "Copy"),
                ContextMenuItem::new("paste", "Paste"),
            ],
        );
        // Hover and click the first item.
        cm.on_mouse_move(100.0 + 5.0, 100.0 + ContextMenu::PADDING + 2.0);
        let consumed = cm.on_click(100.0 + 5.0, 100.0 + ContextMenu::PADDING + 2.0);
        assert!(consumed);
        assert_eq!(cm.clicked_id.as_deref(), Some("copy"));
        assert!(!cm.open);
    }

    #[test]
    fn context_menu_outside_click_closes() {
        let mut cm = ContextMenu::new();
        cm.open(100.0, 100.0, vec![ContextMenuItem::new("x", "X")]);
        let consumed = cm.on_click(9999.0, 9999.0);
        assert!(!consumed);
        assert!(!cm.open);
        assert!(cm.clicked_id.is_none());
    }

    #[test]
    fn context_menu_disabled_item_not_selectable() {
        let mut cm = ContextMenu::new();
        cm.open(
            0.0,
            0.0,
            vec![ContextMenuItem::new("sep", "---").disabled()],
        );
        cm.on_mouse_move(5.0, ContextMenu::PADDING + 2.0);
        // hovered_idx should be None because item is disabled.
        assert!(cm.hovered_idx.is_none());
        cm.on_click(5.0, ContextMenu::PADDING + 2.0);
        assert!(cm.clicked_id.is_none());
    }

    // ── TextInput ─────────────────────────────────────────────────────────────

    #[test]
    fn text_input_insert_and_backspace() {
        let mut ti = TextInput::new();
        ti.insert('H');
        ti.insert('i');
        assert_eq!(ti.value, "Hi");
        assert_eq!(ti.cursor, 2);
        ti.backspace();
        assert_eq!(ti.value, "H");
        assert_eq!(ti.cursor, 1);
    }

    #[test]
    fn text_input_cursor_move() {
        let mut ti = TextInput::new();
        for ch in "Hello".chars() {
            ti.insert(ch);
        }
        ti.move_left();
        ti.move_left();
        assert_eq!(ti.cursor, 3);
        ti.move_right();
        assert_eq!(ti.cursor, 4);
        ti.move_home();
        assert_eq!(ti.cursor, 0);
        ti.move_end();
        assert_eq!(ti.cursor, 5);
    }

    #[test]
    fn text_input_delete_forward() {
        let mut ti = TextInput::new();
        for ch in "abc".chars() {
            ti.insert(ch);
        }
        ti.move_home();
        ti.delete_forward();
        assert_eq!(ti.value, "bc");
        assert_eq!(ti.cursor, 0);
    }

    #[test]
    fn text_input_max_len() {
        let mut ti = TextInput::new().with_max_len(3);
        for ch in "abcdefgh".chars() {
            ti.insert(ch);
        }
        assert_eq!(ti.value, "abc");
    }

    #[test]
    fn text_input_handle_key_backspace() {
        let mut ti = TextInput::new();
        ti.insert('X');
        let consumed = ti.handle_key(Key::Backspace);
        assert!(consumed);
        assert!(ti.value.is_empty());
    }

    #[test]
    fn text_input_control_chars_ignored() {
        let mut ti = TextInput::new();
        ti.handle_char('\x01'); // SOH
        ti.handle_char('\n');
        assert!(ti.value.is_empty());
    }
}
