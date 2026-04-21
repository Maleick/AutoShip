//! [`WindowManager`] — owns all overlay windows and processes mouse interaction.

use serde::{Deserialize, Serialize};

use super::{theme::Theme, window::Window};

/// Transient drag state — not serialized.
#[derive(Debug)]
struct DragState {
    window_idx: usize,
    /// Cursor offset inside the header at drag-start.
    offset_x: f32,
    offset_y: f32,
}

/// Transient resize state — not serialized.
#[derive(Debug)]
struct ResizeState {
    window_idx: usize,
    start_w: f32,
    start_h: f32,
    start_mx: f32,
    start_my: f32,
}

/// Serializable snapshot of the manager state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagerState {
    pub theme: Theme,
    pub windows: Vec<Window>,
}

/// Manages all overlay windows — creation, interaction, and state I/O.
#[derive(Debug)]
pub struct WindowManager {
    pub theme: Theme,
    windows: Vec<Window>,
    drag: Option<DragState>,
    resize: Option<ResizeState>,
    /// IDs of windows closed this frame (cleared after the caller drains them).
    pub closed_ids: Vec<String>,
}

impl Default for WindowManager {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            windows: Vec::new(),
            drag: None,
            resize: None,
            closed_ids: Vec::new(),
        }
    }
}

impl WindowManager {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            ..Self::default()
        }
    }

    /// Add a window; returns its index.
    pub fn add_window(&mut self, window: Window) -> usize {
        self.windows.push(window);
        self.windows.len() - 1
    }

    /// Remove a window by id. Returns `true` if found and removed.
    pub fn remove_window(&mut self, id: &str) -> bool {
        if let Some(idx) = self.windows.iter().position(|w| w.id == id) {
            self.windows.remove(idx);
            true
        } else {
            false
        }
    }

    pub fn windows(&self) -> &[Window] {
        &self.windows
    }

    pub fn windows_mut(&mut self) -> &mut Vec<Window> {
        &mut self.windows
    }

    pub fn get_window(&self, id: &str) -> Option<&Window> {
        self.windows.iter().find(|w| w.id == id)
    }

    pub fn get_window_mut(&mut self, id: &str) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.id == id)
    }

    // ── Mouse interaction ────────────────────────────────────────────────────

    /// Call when the primary mouse button is pressed.
    pub fn mouse_down(&mut self, mx: f32, my: f32) {
        self.closed_ids.clear();

        // Iterate windows in reverse so top-most (last drawn) wins.
        for (idx, win) in self.windows.iter_mut().enumerate().rev() {
            if !win.outer_rect().contains(mx, my) {
                continue;
            }

            // Close button hit.
            if win.close_button_rect().contains(mx, my) {
                // Defer removal — mark for the caller to drain via closed_ids.
                // We record the id here and remove after the loop.
                self.closed_ids.push(win.id.clone());
                break;
            }

            // Minimize button hit.
            if win.minimize_button_rect().contains(mx, my) {
                win.toggle_minimize();
                break;
            }

            // Resize handle (only when not minimized).
            if win.resize_handle_contains_point(mx, my) {
                self.resize = Some(ResizeState {
                    window_idx: idx,
                    start_w: win.width,
                    start_h: win.height,
                    start_mx: mx,
                    start_my: my,
                });
                break;
            }

            // Header drag.
            if win.header_contains_point(mx, my) {
                self.drag = Some(DragState {
                    window_idx: idx,
                    offset_x: mx - win.x,
                    offset_y: my - win.y,
                });
                break;
            }

            // Click inside body — handled by widget layer.
            break;
        }

        // Remove closed windows — clone ids to release the immutable borrow first.
        let to_remove: Vec<String> = self.closed_ids.clone();
        for id in &to_remove {
            self.remove_window(id);
        }
    }

    /// Call each frame when the mouse moves (button may or may not be held).
    pub fn mouse_move(&mut self, mx: f32, my: f32) {
        if let Some(ref d) = self.drag {
            let idx = d.window_idx;
            if let Some(win) = self.windows.get_mut(idx) {
                let new_x = mx - d.offset_x;
                let new_y = my - d.offset_y;
                win.drag(new_x - win.x, new_y - win.y);
            }
        }
        if let Some(ref r) = self.resize {
            let idx = r.window_idx;
            let dw = mx - r.start_mx;
            let dh = my - r.start_my;
            if let Some(win) = self.windows.get_mut(idx) {
                win.width = (r.start_w + dw).max(80.0);
                win.height = (r.start_h + dh).max(40.0);
            }
        }
    }

    /// Call when the primary mouse button is released.
    pub fn mouse_up(&mut self, _mx: f32, _my: f32) {
        self.drag = None;
        self.resize = None;
    }

    // ── State persistence ────────────────────────────────────────────────────

    /// Serialize window positions and state to JSON.
    pub fn save_state(&self) -> Result<String, serde_json::Error> {
        let snap = ManagerState {
            theme: self.theme,
            windows: self.windows.clone(),
        };
        serde_json::to_string(&snap)
    }

    /// Restore window positions and state from JSON produced by [`save_state`].
    ///
    /// Replaces all windows; preserves the current manager-level theme unless
    /// the snapshot contains a different one.
    pub fn load_state(&mut self, json: &str) -> Result<(), serde_json::Error> {
        let snap: ManagerState = serde_json::from_str(json)?;
        self.theme = snap.theme;
        self.windows = snap.windows;
        self.drag = None;
        self.resize = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::widget::Widget;

    fn make_mgr() -> WindowManager {
        WindowManager::new(Theme::Dark)
    }

    fn add_win(mgr: &mut WindowManager, id: &str) -> usize {
        mgr.add_window(Window::new(id, "Title"))
    }

    #[test]
    fn add_and_get_window() {
        let mut mgr = make_mgr();
        add_win(&mut mgr, "w1");
        assert!(mgr.get_window("w1").is_some());
    }

    #[test]
    fn remove_window() {
        let mut mgr = make_mgr();
        add_win(&mut mgr, "w1");
        assert!(mgr.remove_window("w1"));
        assert!(mgr.get_window("w1").is_none());
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut mgr = make_mgr();
        assert!(!mgr.remove_window("ghost"));
    }

    #[test]
    fn drag_moves_window() {
        let mut mgr = make_mgr();
        add_win(&mut mgr, "d");
        let win = mgr.get_window("d").unwrap();
        let (wx, wy) = (win.x, win.y);
        let hdr = win.header_rect();
        let (mx, my) = (hdr.x + 10.0, hdr.y + 5.0);

        mgr.mouse_down(mx, my);
        assert!(mgr.drag.is_some(), "drag should start");

        mgr.mouse_move(mx + 30.0, my + 20.0);
        let w = mgr.get_window("d").unwrap();
        assert!((w.x - (wx + 30.0)).abs() < 1.0);
        assert!((w.y - (wy + 20.0)).abs() < 1.0);

        mgr.mouse_up(mx + 30.0, my + 20.0);
        assert!(mgr.drag.is_none());
    }

    #[test]
    fn resize_changes_dimensions() {
        let mut mgr = make_mgr();
        add_win(&mut mgr, "r");
        let rh = mgr.get_window("r").unwrap().resize_handle_rect();
        let (mx, my) = (rh.x + 2.0, rh.y + 2.0);

        mgr.mouse_down(mx, my);
        assert!(mgr.resize.is_some(), "resize should start");

        mgr.mouse_move(mx + 40.0, my + 30.0);
        let w = mgr.get_window("r").unwrap();
        assert!(w.width > 200.0);
        assert!(w.height > 150.0);

        mgr.mouse_up(mx + 40.0, my + 30.0);
        assert!(mgr.resize.is_none());
    }

    #[test]
    fn minimize_button_toggles_state() {
        let mut mgr = make_mgr();
        add_win(&mut mgr, "m");
        let mb = mgr.get_window("m").unwrap().minimize_button_rect();

        mgr.mouse_down(mb.x + 2.0, mb.y + 2.0);
        assert!(mgr.get_window("m").unwrap().minimized);

        mgr.mouse_down(mb.x + 2.0, mb.y + 2.0);
        assert!(!mgr.get_window("m").unwrap().minimized);
    }

    #[test]
    fn close_button_removes_window() {
        let mut mgr = make_mgr();
        add_win(&mut mgr, "c");
        let cb = mgr.get_window("c").unwrap().close_button_rect();

        mgr.mouse_down(cb.x + 2.0, cb.y + 2.0);
        assert!(mgr.get_window("c").is_none(), "window should be removed");
        assert!(mgr.closed_ids.contains(&"c".to_string()));
    }

    #[test]
    fn save_load_roundtrip() {
        let mut mgr = make_mgr();
        let mut win = Window::new("persist", "Persist");
        win.widgets.push(Widget::Text {
            content: "hello".into(),
        });
        win.x = 42.0;
        win.y = 77.0;
        mgr.add_window(win);

        let json = mgr.save_state().expect("save failed");
        let mut mgr2 = WindowManager::new(Theme::Light);
        mgr2.load_state(&json).expect("load failed");

        assert_eq!(mgr2.theme, Theme::Dark);
        let w = mgr2
            .get_window("persist")
            .expect("window missing after load");
        assert!((w.x - 42.0).abs() < 0.001);
        assert!((w.y - 77.0).abs() < 0.001);
        assert_eq!(w.widgets.len(), 1);
    }

    #[test]
    fn load_invalid_json_returns_error() {
        let mut mgr = make_mgr();
        assert!(mgr.load_state("not json at all").is_err());
    }

    #[test]
    fn no_drag_outside_window() {
        let mut mgr = make_mgr();
        add_win(&mut mgr, "x");
        // Click far away.
        mgr.mouse_down(9999.0, 9999.0);
        assert!(mgr.drag.is_none());
        assert!(mgr.resize.is_none());
    }

    #[test]
    fn mouse_up_clears_all_states() {
        let mut mgr = make_mgr();
        add_win(&mut mgr, "s");
        let hdr = mgr.get_window("s").unwrap().header_rect();
        mgr.mouse_down(hdr.x + 5.0, hdr.y + 5.0);
        assert!(mgr.drag.is_some());
        mgr.mouse_up(0.0, 0.0);
        assert!(mgr.drag.is_none());
        assert!(mgr.resize.is_none());
    }
}
