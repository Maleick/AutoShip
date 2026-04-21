/// Axis-aligned bounding rectangle in screen space (pixels).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }
}

/// Widgets are the leaf elements rendered inside windows.
///
/// Each variant carries its display state. Click/hover state is managed by the
/// [`WindowManager`](super::manager::WindowManager) interaction layer and stored
/// here for the render pass to consume.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Widget {
    Button {
        label: String,
        /// Set to true for one frame when the button is released over it.
        #[serde(default)]
        clicked: bool,
        #[serde(default)]
        hovered: bool,
    },
    Text {
        content: String,
    },
    List {
        items: Vec<String>,
        selected: Option<usize>,
    },
    Dropdown {
        label: String,
        options: Vec<String>,
        selected: Option<usize>,
        #[serde(default)]
        open: bool,
    },
}

impl Widget {
    /// Preferred (min) size in pixels — used by the layout manager.
    pub fn preferred_size(&self) -> (f32, f32) {
        match self {
            Widget::Button { label, .. } => {
                let w = (label.len() as f32 * 8.0 + 20.0).max(60.0);
                (w, 24.0)
            }
            Widget::Text { content } => {
                let w = (content.len() as f32 * 7.0).max(40.0);
                (w, 18.0)
            }
            Widget::List { items, .. } => {
                let max_w = items.iter().map(|s| s.len()).max().unwrap_or(4);
                (max_w as f32 * 7.0 + 16.0, items.len() as f32 * 20.0 + 4.0)
            }
            Widget::Dropdown {
                label,
                options,
                open,
                ..
            } => {
                let base_w = (label.len() as f32 * 7.0 + 30.0).max(80.0);
                let base_h = 24.0;
                if *open {
                    let rows = options.len().min(8) as f32;
                    (base_w, base_h + rows * 20.0)
                } else {
                    (base_w, base_h)
                }
            }
        }
    }

    /// Clear one-shot interaction flags after the render pass consumes them.
    pub fn clear_frame_flags(&mut self) {
        if let Widget::Button {
            clicked, hovered, ..
        } = self
        {
            *clicked = false;
            *hovered = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains() {
        let r = Rect::new(10.0, 10.0, 50.0, 30.0);
        assert!(r.contains(10.0, 10.0));
        assert!(r.contains(59.0, 39.0));
        assert!(!r.contains(60.0, 10.0));
        assert!(!r.contains(10.0, 40.0));
        assert!(!r.contains(9.9, 10.0));
    }

    #[test]
    fn button_preferred_size_min_width() {
        let w = Widget::Button {
            label: "X".into(),
            clicked: false,
            hovered: false,
        };
        let (pw, ph) = w.preferred_size();
        assert!(pw >= 60.0);
        assert_eq!(ph, 24.0);
    }

    #[test]
    fn list_height_proportional_to_items() {
        let w = Widget::List {
            items: vec!["a".into(), "b".into(), "c".into()],
            selected: None,
        };
        let (_, h) = w.preferred_size();
        assert!(h >= 60.0, "expected at least 60px for 3 items, got {h}");
    }

    #[test]
    fn dropdown_expands_when_open() {
        let closed = Widget::Dropdown {
            label: "Select".into(),
            options: vec!["A".into(), "B".into()],
            selected: None,
            open: false,
        };
        let open = Widget::Dropdown {
            label: "Select".into(),
            options: vec!["A".into(), "B".into()],
            selected: None,
            open: true,
        };
        let (_, h_closed) = closed.preferred_size();
        let (_, h_open) = open.preferred_size();
        assert!(h_open > h_closed);
    }

    #[test]
    fn clear_frame_flags_resets_button() {
        let mut w = Widget::Button {
            label: "Go".into(),
            clicked: true,
            hovered: true,
        };
        w.clear_frame_flags();
        if let Widget::Button {
            clicked, hovered, ..
        } = w
        {
            assert!(!clicked);
            assert!(!hovered);
        }
    }

    #[test]
    fn widget_serde_roundtrip() {
        let w = Widget::List {
            items: vec!["Alpha".into(), "Beta".into()],
            selected: Some(0),
        };
        let json = serde_json::to_string(&w).unwrap();
        let parsed: Widget = serde_json::from_str(&json).unwrap();
        if let Widget::List { selected, .. } = parsed {
            assert_eq!(selected, Some(0));
        } else {
            panic!("wrong variant");
        }
    }
}
