# Overlay GUI System

The overlay is a pure-Rust immediate-mode-style windowing layer injected into the EQ process. It provides draggable, resizable windows that sit above the EQ viewport, handles its own input routing, and persists layout to disk between sessions.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  EQ window hook surface  (hooks/  — Windows only)               │
│  Delivers raw WM_* / DirectInput events to the overlay stack    │
└────────────────────────┬────────────────────────────────────────┘
                         │ InputEvent
                         ▼
         ┌───────────────────────────┐
         │     InputDispatcher       │  input.rs
         │  routes events; decides   │
         │  Consumed vs PassThrough  │
         └──────┬─────────┬──────────┘
                │         │
         FocusManager  ContextMenu / Tooltip / TextInput
         (which window
          has kbd focus)
                │
                ▼
         ┌─────────────────┐
         │  WindowManager  │  manager.rs
         │  owns Vec<Window>│
         │  drag / resize  │
         └────────┬────────┘
                  │
         ┌────────┴────────┐
         │    Window       │  window.rs
         │  header, body   │
         │  widgets, theme │
         └────────┬────────┘
                  │
         ┌────────┴────────┐
         │    Widget       │  widget.rs
         │  Button / Text  │
         │  List / Dropdown│
         └─────────────────┘

State I/O (state.rs) ──▶ overlay.json  (atomic write via .tmp rename)
Theme (theme.rs) ──────▶ Dark / Light  (per-manager or per-window override)
Layout (layout.rs) ────▶ Grid / Flex   (widget placement inside body)
```

### Key types

| Type | File | Purpose |
|------|------|---------|
| `WindowManager` | `manager.rs` | Owns all windows; handles drag, resize, close, save/load |
| `Window` | `window.rs` | Draggable/resizable frame with header + widget body |
| `Widget` | `widget.rs` | Leaf UI element: `Button`, `Text`, `List`, `Dropdown` |
| `FocusManager` | `input.rs` | Tracks which window holds keyboard focus |
| `InputDispatcher` | `input.rs` | Routes `InputEvent` → overlay or EQ passthrough |
| `OverlayState` | `state.rs` | JSON-serializable snapshot of all window geometry + theme |
| `Theme` | `theme.rs` | `Dark` / `Light` color scheme; `ThemeColors` holds RGBA values |
| `LayoutKind` | `layout.rs` | `Grid` / `Flex` — controls how widgets are arranged |

## Input flow

```
Raw event (mouse/keyboard)
    │
    ▼
InputDispatcher::dispatch(event, &mut manager, &mut focus)
    │
    ├─ Mouse over a window? ──yes──► consume; update FocusManager
    │                                update WindowManager (drag/resize/close)
    │
    ├─ Keyboard event + overlay_has_focus()? ──yes──► consume
    │
    └─ Otherwise ──────────────────────────────────────► PassThrough → EQ
```

`DispatchResult::Consumed` means the overlay handled it — the hook must not forward to EQ.
`DispatchResult::PassThrough` means the event belongs to EQ.

**Mouse-move is always `PassThrough`** even when the overlay has focus, because EQ needs the cursor position for targeting and camera.

## How to add a new overlay window

1. **Create the `Window`** with a stable `id` string:

```rust
use textquest_dll::overlay::{manager::WindowManager, theme::Theme, window::Window, widget::Widget};

let mut win = Window::new("my_panel", "My Panel");
win.x = 200.0;
win.y = 150.0;
win.width = 320.0;
win.height = 240.0;
```

2. **Add widgets** to the window body:

```rust
win.widgets.push(Widget::Text { content: "Hello EQ".into() });
win.widgets.push(Widget::Button { label: "Click me".into(), id: "btn_ok".into() });
```

3. **Register with the manager**:

```rust
// mgr is a &mut WindowManager that lives for the session.
mgr.add_window(win);
```

4. **Handle events** in your per-frame tick:

```rust
// Hook layer delivers InputEvent values.
let result = dispatcher.dispatch(&event, &mut mgr, &mut focus);
if result == DispatchResult::PassThrough {
    send_to_eq(event);
}
```

5. **Persist layout** on session shutdown or periodically:

```rust
use textquest_dll::overlay::state;
use std::path::Path;

// Build an OverlayState from the live manager.
// (Or use manager.save_state() for a full JSON snapshot.)
let json = mgr.save_state()?;
std::fs::write("overlay_layout.json", &json)?;
```

## Theming

Two built-in themes: `Theme::Dark` (default) and `Theme::Light`. Each exposes a `ThemeColors` struct with RGBA float fields for `background`, `foreground`, `header`, `border`, `button`, `button_hover`, and `button_active`.

### Switch the global theme

```rust
mgr.theme = Theme::Light;
```

The new theme takes effect on the next render pass. The theme is included in `save_state` / `load_state` so it persists across sessions.

### Per-window theme override

Individual windows can override the manager-level theme:

```rust
win.theme = Some(Theme::Light); // this window stays light even in a dark session
```

`win.theme = None` (the default) inherits the manager theme.

### Adding a custom theme

`Theme` is a non-exhaustive-friendly enum — extend it and add a new arm to `Theme::colors()` in `theme.rs`. No other changes are required; the renderer calls `.colors()` each frame.

## State persistence

`state.rs` provides two free functions:

```rust
// Load from disk (returns default if file absent — first-run safe).
let state: OverlayState = state::load(Path::new("overlay.json"))?;

// Atomic save (writes .tmp, then renames — safe against mid-write crashes).
state::save(&state, Path::new("overlay.json"))?;
```

`OverlayState` holds:
- `windows: HashMap<String, PersistedWindow>` — keyed by window `id`
- `theme: Theme`

Each `PersistedWindow` stores `x`, `y`, `width`, `height`, and `minimized`.

**Merge pattern** — to apply saved geometry to a running `WindowManager`:

```rust
let saved = state::load(&path)?;
for (id, geom) in &saved.windows {
    if let Some(win) = mgr.get_window_mut(id) {
        win.x = geom.x;
        win.y = geom.y;
        win.width = geom.width;
        win.height = geom.height;
        win.minimized = geom.minimized;
    }
}
mgr.theme = saved.theme;
```

Or use `mgr.load_state(&json)` which does the full round-trip from a JSON string produced by `mgr.save_state()`.

## Drag system

Windows are dragged by clicking and holding on the **header strip** (the top 22 px of each window) and moving the mouse. The drag system is split across two types:

| Layer | Type | Responsibility |
|-------|------|---------------|
| `Window` | `window.rs` | `drag(dx, dy)` — translates position; clamps to `(0, 0)` |
| `WindowManager` | `manager.rs` | `DragState` — tracks which window is dragging and the cursor offset |

### Lifecycle

```
mouse_down(mx, my)
  └─ header_contains_point? → DragState { window_idx, offset_x, offset_y }

mouse_move(mx, my)   [called every frame]
  └─ DragState present?
       new_x = mx - offset_x
       new_y = my - offset_y
       win.drag(new_x - win.x, new_y - win.y)

mouse_up(mx, my)
  └─ drag = None          ← drag ends; window stays at last position
```

### Offset anchoring

The `offset_x` / `offset_y` stored in `DragState` is the cursor's position relative to the window's top-left corner at the moment the drag began:

```rust
offset_x = mx - win.x   // at mouse_down time
offset_y = my - win.y
```

On every `mouse_move` the new window position is:

```rust
win.x = mx - offset_x   // equivalently: win.x += (mx - prev_mx)
win.y = my - offset_y
```

This ensures the window doesn't jump on drag start regardless of where inside the header the user clicked.

### Screen-edge clamping

`Window::drag` clamps the final position to `x ≥ 0` and `y ≥ 0`, preventing windows from being dragged off the top or left edge of the screen. There is intentionally no right/bottom clamp — partially off-screen windows are allowed so the user can park them out of the way.

```rust
pub fn drag(&mut self, dx: f32, dy: f32) {
    self.x = (self.x + dx).max(0.0);
    self.y = (self.y + dy).max(0.0);
}
```

### Z-order and focus

`mouse_down` iterates windows in **reverse insertion order** so the topmost (most recently added) window wins when windows overlap. The winning window's header starts the drag; no other window moves.

### Drag cancellation

There is no explicit cancel gesture. The system resets `DragState` on:

- `mouse_up` — normal drag end.
- `load_state` — restores a saved snapshot; any in-progress drag is discarded.

`Key::Escape` releases **keyboard focus** (via `FocusManager::blur`) but does **not** cancel an active drag — the user must release the mouse button.

### Resize handle

The bottom-right 10×10 px corner is the **resize handle**. It uses `ResizeState` (parallel to `DragState`) and enforces minimum dimensions of 80 × 40 px. Resize takes priority over body clicks but yields to the close and minimize buttons. The drag and resize states are mutually exclusive — only one can be active at a time.

### Testing

Drag behaviour is covered by module-local tests in `manager.rs` (`#[cfg(test)] mod tests`):

| Test | What it verifies |
|------|-----------------|
| `drag_start_records_offset` | `DragState.offset_x/y` matches cursor-to-origin delta |
| `drag_continue_tracks_cursor` | Position follows cursor correctly across multiple `mouse_move` calls |
| `drag_end_stops_movement` | `mouse_up` clears `DragState`; further moves are no-ops |
| `drag_clamped_at_screen_top_left` | `x ≥ 0` and `y ≥ 0` after dragging past the top-left edge |
| `body_click_does_not_start_drag` | Body clicks do not initiate drag or resize |
| `drag_topmost_window_wins` | Last-added window wins when two headers overlap |
| `drag_does_not_move_other_windows` | Only the dragged window moves; siblings are unaffected |
| `load_state_cancels_drag` | `load_state` resets in-progress drag to `None` |

Run with:

```bash
cargo test --lib -p textquest-dll overlay::manager
```

## Layout

`LayoutKind` controls how widgets fill the window body:

| Variant | Behaviour |
|---------|-----------|
| `Grid { columns }` | Widgets laid out in a fixed-column grid |
| `Flex { direction }` | Widgets stacked horizontally or vertically |

Set on the `Window`:

```rust
use textquest_dll::overlay::layout::LayoutKind;

win.layout = LayoutKind::Grid { columns: 2 };
```

`window.widget_rects()` returns a `Vec<Rect>` aligned to the chosen layout. Returns empty when the window is minimized.

## Troubleshooting

### Overlay not appearing

- Confirm `WindowManager::add_window` was called before the first render tick.
- Check that `x`/`y` coordinates are within the EQ viewport (default 100, 100).
- Verify the hook surface (`hooks/overlay_hook.rs` on Windows) is delivering `InputEvent` values — add a `tracing::debug!` at the hook entry point.

### Clicks pass through to EQ unexpectedly

- Check `InputDispatcher::dispatch` returns `DispatchResult::Consumed`.
- Confirm the window `width`/`height` actually cover the click position: `window.contains_point(mx, my)`.
- If the window was closed mid-session, its id will be in `mgr.closed_ids` — drain that list after each frame so stale IDs don't confuse callers.

### Keyboard input not reaching the overlay

- Ensure `FocusManager::overlay_has_focus()` returns `true` after clicking the window.
- If focus was stolen by a mouse-down outside all windows, click the target window again.
- `Key::Escape` always releases focus; test that EQ isn't synthesizing spurious escape events.

### State file corruption

The atomic write (`write tmp → rename`) prevents partial writes. If the state file is corrupted anyway (e.g., disk full during rename), `state::load` will return a `StateError::Json`. Call `state::load` defensively and fall back to `OverlayState::default()`:

```rust
let overlay_state = state::load(&path).unwrap_or_default();
```

### Theme not persisting

`save_state` / `load_state` on `WindowManager` includes the theme. If you are using `state::save` / `state::load` instead, set `overlay_state.theme = mgr.theme` before saving.

### macOS / CI builds

The overlay modules compile on macOS as stubs. All overlay logic is cross-platform pure Rust — only the hook wiring (`hooks/`) is `#[cfg(windows)]`. Integration tests run on macOS CI without any stubs needed.
