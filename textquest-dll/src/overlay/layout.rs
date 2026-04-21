use super::widget::{Rect, Widget};

/// Direction for flex layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum FlexDirection {
    #[default]
    Column,
    Row,
}

/// Layout algorithm applied to widgets inside a window body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LayoutKind {
    /// Flow along one axis with equal spacing.
    Flex(FlexDirection),
    /// Divide available width into fixed columns; rows wrap.
    Grid { columns: u32 },
}

impl Default for LayoutKind {
    fn default() -> Self {
        LayoutKind::Flex(FlexDirection::Column)
    }
}

const PADDING: f32 = 4.0;

/// Compute screen-space rects for each widget inside `bounds`.
///
/// Returns one rect per widget in the same order as `widgets`.
pub fn arrange(widgets: &[Widget], bounds: Rect, kind: LayoutKind) -> Vec<Rect> {
    if widgets.is_empty() {
        return vec![];
    }

    match kind {
        LayoutKind::Flex(dir) => arrange_flex(widgets, bounds, dir),
        LayoutKind::Grid { columns } => arrange_grid(widgets, bounds, columns.max(1)),
    }
}

fn arrange_flex(widgets: &[Widget], bounds: Rect, dir: FlexDirection) -> Vec<Rect> {
    let count = widgets.len() as f32;
    match dir {
        FlexDirection::Column => {
            let total_h = bounds.h - PADDING * (count + 1.0);
            let row_h = (total_h / count).max(0.0);
            widgets
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let y = bounds.y + PADDING + i as f32 * (row_h + PADDING);
                    Rect::new(bounds.x + PADDING, y, bounds.w - PADDING * 2.0, row_h)
                })
                .collect()
        }
        FlexDirection::Row => {
            let total_w = bounds.w - PADDING * (count + 1.0);
            let col_w = (total_w / count).max(0.0);
            widgets
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let x = bounds.x + PADDING + i as f32 * (col_w + PADDING);
                    Rect::new(x, bounds.y + PADDING, col_w, bounds.h - PADDING * 2.0)
                })
                .collect()
        }
    }
}

fn arrange_grid(widgets: &[Widget], bounds: Rect, columns: u32) -> Vec<Rect> {
    let cols = columns as usize;
    let rows = widgets.len().div_ceil(cols);

    let cell_w = (bounds.w - PADDING * (cols as f32 + 1.0)) / cols as f32;
    let cell_h = (bounds.h - PADDING * (rows as f32 + 1.0)) / rows as f32;
    let cell_w = cell_w.max(0.0);
    let cell_h = cell_h.max(0.0);

    widgets
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let col = i % cols;
            let row = i / cols;
            let x = bounds.x + PADDING + col as f32 * (cell_w + PADDING);
            let y = bounds.y + PADDING + row as f32 * (cell_h + PADDING);
            Rect::new(x, y, cell_w, cell_h)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::widget::Widget;

    fn make_buttons(n: usize) -> Vec<Widget> {
        (0..n)
            .map(|i| Widget::Button {
                label: format!("Btn{i}"),
                clicked: false,
                hovered: false,
            })
            .collect()
    }

    #[test]
    fn empty_widgets_returns_empty() {
        let rects = arrange(
            &[],
            Rect::new(0.0, 0.0, 200.0, 200.0),
            LayoutKind::default(),
        );
        assert!(rects.is_empty());
    }

    #[test]
    fn flex_column_count_matches() {
        let widgets = make_buttons(4);
        let rects = arrange(
            &widgets,
            Rect::new(0.0, 0.0, 200.0, 200.0),
            LayoutKind::Flex(FlexDirection::Column),
        );
        assert_eq!(rects.len(), 4);
    }

    #[test]
    fn flex_row_count_matches() {
        let widgets = make_buttons(3);
        let rects = arrange(
            &widgets,
            Rect::new(0.0, 0.0, 300.0, 60.0),
            LayoutKind::Flex(FlexDirection::Row),
        );
        assert_eq!(rects.len(), 3);
    }

    #[test]
    fn flex_column_rects_are_stacked() {
        let widgets = make_buttons(2);
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let rects = arrange(&widgets, bounds, LayoutKind::Flex(FlexDirection::Column));
        // Second rect should be below the first.
        assert!(rects[1].y > rects[0].y);
    }

    #[test]
    fn flex_row_rects_are_side_by_side() {
        let widgets = make_buttons(2);
        let bounds = Rect::new(0.0, 0.0, 200.0, 50.0);
        let rects = arrange(&widgets, bounds, LayoutKind::Flex(FlexDirection::Row));
        assert!(rects[1].x > rects[0].x);
    }

    #[test]
    fn grid_two_columns() {
        let widgets = make_buttons(4);
        let bounds = Rect::new(0.0, 0.0, 200.0, 200.0);
        let rects = arrange(&widgets, bounds, LayoutKind::Grid { columns: 2 });
        assert_eq!(rects.len(), 4);
        // First two should share the same y.
        assert_eq!(rects[0].y, rects[1].y);
        // Third should be on the second row.
        assert!(rects[2].y > rects[0].y);
    }

    #[test]
    fn grid_columns_zero_clamped() {
        let widgets = make_buttons(3);
        let bounds = Rect::new(0.0, 0.0, 200.0, 200.0);
        // columns=0 must not panic (clamped to 1).
        let rects = arrange(&widgets, bounds, LayoutKind::Grid { columns: 0 });
        assert_eq!(rects.len(), 3);
    }

    #[test]
    fn rects_stay_within_bounds() {
        let widgets = make_buttons(5);
        let bounds = Rect::new(10.0, 10.0, 200.0, 300.0);
        for kind in [
            LayoutKind::Flex(FlexDirection::Column),
            LayoutKind::Flex(FlexDirection::Row),
            LayoutKind::Grid { columns: 2 },
        ] {
            let rects = arrange(&widgets, bounds, kind);
            for r in &rects {
                assert!(
                    r.x >= bounds.x,
                    "rect.x={} < bounds.x={} for kind={kind:?}",
                    r.x,
                    bounds.x
                );
                assert!(
                    r.y >= bounds.y,
                    "rect.y={} < bounds.y={} for kind={kind:?}",
                    r.y,
                    bounds.y
                );
            }
        }
    }
}
