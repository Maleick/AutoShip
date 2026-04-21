use super::{
    layout::{LayoutKind, arrange},
    theme::Theme,
    widget::{Rect, Widget},
};

const HEADER_H: f32 = 22.0;
const RESIZE_HANDLE: f32 = 10.0;
const MIN_WIDTH: f32 = 80.0;
const MIN_HEIGHT: f32 = 40.0;
const CLOSE_BTN_W: f32 = 18.0;
const MINIMIZE_BTN_W: f32 = 18.0;

/// A draggable, resizable overlay window.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Window {
    /// Unique identifier — used to persist/restore state.
    pub id: String,
    pub title: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub minimized: bool,
    /// Whether the window shows a close [X] button.
    #[serde(default = "default_true")]
    pub closeable: bool,
    pub widgets: Vec<Widget>,
    pub layout: LayoutKind,
    /// `None` inherits the manager-level theme.
    pub theme: Option<Theme>,
}

fn default_true() -> bool {
    true
}

impl Window {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            x: 100.0,
            y: 100.0,
            width: 200.0,
            height: 150.0,
            minimized: false,
            closeable: true,
            widgets: Vec::new(),
            layout: LayoutKind::default(),
            theme: None,
        }
    }

    /// Full bounding rect (header + body).
    pub fn outer_rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.width, self.height)
    }

    /// Header strip at the top of the window.
    pub fn header_rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.width, HEADER_H)
    }

    /// Content area below the header (empty when minimized).
    pub fn body_rect(&self) -> Rect {
        Rect::new(
            self.x,
            self.y + HEADER_H,
            self.width,
            (self.height - HEADER_H).max(0.0),
        )
    }

    /// Resize grab area at the bottom-right corner.
    pub fn resize_handle_rect(&self) -> Rect {
        Rect::new(
            self.x + self.width - RESIZE_HANDLE,
            self.y + self.height - RESIZE_HANDLE,
            RESIZE_HANDLE,
            RESIZE_HANDLE,
        )
    }

    /// Close [X] button inside the header (right side).
    pub fn close_button_rect(&self) -> Rect {
        Rect::new(
            self.x + self.width - CLOSE_BTN_W - 2.0,
            self.y + 2.0,
            CLOSE_BTN_W,
            HEADER_H - 4.0,
        )
    }

    /// Minimize [-] button inside the header (left of close).
    pub fn minimize_button_rect(&self) -> Rect {
        Rect::new(
            self.x + self.width - CLOSE_BTN_W - MINIMIZE_BTN_W - 4.0,
            self.y + 2.0,
            MINIMIZE_BTN_W,
            HEADER_H - 4.0,
        )
    }

    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        self.outer_rect().contains(px, py)
    }

    pub fn header_contains_point(&self, px: f32, py: f32) -> bool {
        self.header_rect().contains(px, py)
    }

    pub fn resize_handle_contains_point(&self, px: f32, py: f32) -> bool {
        !self.minimized && self.resize_handle_rect().contains(px, py)
    }

    /// Translate the window by (dx, dy), clamping x/y to non-negative.
    pub fn drag(&mut self, dx: f32, dy: f32) {
        self.x = (self.x + dx).max(0.0);
        self.y = (self.y + dy).max(0.0);
    }

    /// Grow/shrink the window, enforcing minimum dimensions.
    pub fn resize(&mut self, dw: f32, dh: f32) {
        self.width = (self.width + dw).max(MIN_WIDTH);
        self.height = (self.height + dh).max(MIN_HEIGHT);
    }

    pub fn toggle_minimize(&mut self) {
        self.minimized = !self.minimized;
    }

    /// Compute layout rects for all widgets inside the body.
    ///
    /// Returns an empty vec when the window is minimized.
    pub fn widget_rects(&self) -> Vec<Rect> {
        if self.minimized {
            return vec![];
        }
        arrange(&self.widgets, self.body_rect(), self.layout)
    }

    /// Effective height — body collapses to 0 when minimized.
    pub fn visible_height(&self) -> f32 {
        if self.minimized {
            HEADER_H
        } else {
            self.height
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_window() -> Window {
        Window::new("test", "Test Window")
    }

    #[test]
    fn header_rect_is_at_top() {
        let w = make_window();
        let h = w.header_rect();
        assert_eq!(h.y, w.y);
        assert_eq!(h.h, HEADER_H);
    }

    #[test]
    fn body_rect_below_header() {
        let w = make_window();
        let b = w.body_rect();
        assert!((b.y - (w.y + HEADER_H)).abs() < f32::EPSILON);
    }

    #[test]
    fn resize_handle_in_bottom_right() {
        let w = make_window();
        let r = w.resize_handle_rect();
        assert!((r.x + r.w - (w.x + w.width)).abs() < f32::EPSILON);
        assert!((r.y + r.h - (w.y + w.height)).abs() < f32::EPSILON);
    }

    #[test]
    fn close_button_inside_header() {
        let w = make_window();
        let hdr = w.header_rect();
        let btn = w.close_button_rect();
        assert!(btn.x >= hdr.x);
        assert!(btn.x + btn.w <= hdr.x + hdr.w + 1.0); // allow 1px rounding
    }

    #[test]
    fn drag_moves_window() {
        let mut w = make_window();
        let orig_x = w.x;
        let orig_y = w.y;
        w.drag(20.0, 15.0);
        assert_eq!(w.x, orig_x + 20.0);
        assert_eq!(w.y, orig_y + 15.0);
    }

    #[test]
    fn drag_clamps_to_zero() {
        let mut w = make_window();
        w.drag(-999.0, -999.0);
        assert_eq!(w.x, 0.0);
        assert_eq!(w.y, 0.0);
    }

    #[test]
    fn resize_enforces_minimums() {
        let mut w = make_window();
        w.resize(-9999.0, -9999.0);
        assert_eq!(w.width, MIN_WIDTH);
        assert_eq!(w.height, MIN_HEIGHT);
    }

    #[test]
    fn toggle_minimize() {
        let mut w = make_window();
        assert!(!w.minimized);
        w.toggle_minimize();
        assert!(w.minimized);
        w.toggle_minimize();
        assert!(!w.minimized);
    }

    #[test]
    fn widget_rects_empty_when_minimized() {
        let mut w = make_window();
        w.widgets.push(super::super::widget::Widget::Text {
            content: "hello".into(),
        });
        w.minimized = true;
        assert!(w.widget_rects().is_empty());
    }

    #[test]
    fn widget_rects_count_matches() {
        let mut w = make_window();
        for i in 0..3 {
            w.widgets.push(super::super::widget::Widget::Text {
                content: format!("item {i}"),
            });
        }
        assert_eq!(w.widget_rects().len(), 3);
    }

    #[test]
    fn visible_height_collapses_when_minimized() {
        let mut w = make_window();
        let full = w.visible_height();
        w.toggle_minimize();
        assert!(w.visible_height() < full);
        assert_eq!(w.visible_height(), HEADER_H);
    }

    #[test]
    fn contains_point_basic() {
        let w = make_window();
        assert!(w.contains_point(w.x + 1.0, w.y + 1.0));
        assert!(!w.contains_point(w.x - 1.0, w.y));
    }

    #[test]
    fn resize_handle_not_hit_when_minimized() {
        let mut w = make_window();
        let r = w.resize_handle_rect();
        w.minimized = true;
        assert!(!w.resize_handle_contains_point(r.x + 1.0, r.y + 1.0));
    }
}
