//! Integration tests for the overlay GUI system.
//!
//! These tests exercise the full overlay stack as an external consumer would:
//! `WindowManager` + `FocusManager` + `InputDispatcher` + `OverlayState` I/O.
//! They complement the unit tests that live inside each module.

use textquest_dll::overlay::{
    input::{DispatchResult, FocusManager, InputDispatcher, InputEvent, Key},
    manager::WindowManager,
    state::{self, OverlayState, PersistedWindow},
    theme::Theme,
    window::Window,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_window(id: &str, x: f32, y: f32) -> Window {
    let mut w = Window::new(id, format!("Window {id}"));
    w.x = x;
    w.y = y;
    w
}

fn make_mgr_two_windows() -> (WindowManager, FocusManager, InputDispatcher) {
    let mut mgr = WindowManager::new(Theme::Dark);
    mgr.add_window(make_window("w1", 100.0, 100.0));
    mgr.add_window(make_window("w2", 500.0, 100.0));
    let fm = FocusManager::new();
    let disp = InputDispatcher::new();
    (mgr, fm, disp)
}

// ── Lifecycle: init, render, dispose ─────────────────────────────────────────

#[test]
fn lifecycle_init_empty_manager() {
    let mgr = WindowManager::new(Theme::Dark);
    assert!(mgr.windows().is_empty());
}

#[test]
fn lifecycle_add_window_and_query() {
    let mut mgr = WindowManager::new(Theme::Dark);
    mgr.add_window(make_window("overlay_hud", 50.0, 50.0));
    assert_eq!(mgr.windows().len(), 1);
    assert!(mgr.get_window("overlay_hud").is_some());
}

#[test]
fn lifecycle_remove_window() {
    let mut mgr = WindowManager::new(Theme::Dark);
    mgr.add_window(make_window("tmp", 0.0, 0.0));
    assert!(mgr.remove_window("tmp"));
    assert!(mgr.windows().is_empty());
}

#[test]
fn lifecycle_remove_nonexistent_returns_false() {
    let mut mgr = WindowManager::new(Theme::Dark);
    assert!(!mgr.remove_window("ghost"));
}

#[test]
fn lifecycle_multiple_windows_ordered() {
    let mut mgr = WindowManager::new(Theme::Dark);
    for i in 0..5u32 {
        mgr.add_window(make_window(&format!("w{i}"), i as f32 * 50.0, 0.0));
    }
    assert_eq!(mgr.windows().len(), 5);
}

// ── Lifecycle: close via mouse click ─────────────────────────────────────────

#[test]
fn lifecycle_close_button_disposes_window() {
    let mut mgr = WindowManager::new(Theme::Dark);
    let w = make_window("closeable", 100.0, 100.0);
    // The close button is inside the header near the right edge.
    let close_x = w.x + w.width - 2.0 - 9.0; // center of CLOSE_BTN_W=18
    let close_y = w.y + 11.0; // center of HEADER_H=22
    mgr.add_window(w);

    mgr.mouse_down(close_x, close_y);
    // After a mouse_down on the close button, the ID should be queued in closed_ids.
    assert!(
        mgr.closed_ids.contains(&"closeable".to_string()),
        "close button press should queue window for removal"
    );
}

// ── State persistence (save / load roundtrip) ────────────────────────────────

#[test]
fn state_persists_window_geometry_and_theme() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("overlay.json");

    let mut s = OverlayState {
        theme: Theme::Light,
        ..Default::default()
    };
    s.windows.insert(
        "hud".into(),
        PersistedWindow {
            x: 10.0,
            y: 20.0,
            width: 300.0,
            height: 200.0,
            minimized: false,
        },
    );
    state::save(&s, &path).unwrap();

    let loaded = state::load(&path).unwrap();
    assert_eq!(loaded.theme, Theme::Light);
    let win = loaded.windows.get("hud").unwrap();
    assert_eq!(win.x, 10.0);
    assert_eq!(win.y, 20.0);
    assert!(!win.minimized);
}

#[test]
fn state_load_missing_file_returns_default() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("does_not_exist.json");
    let s = state::load(&path).unwrap();
    assert_eq!(s.theme, Theme::default());
    assert!(s.windows.is_empty());
}

#[test]
fn state_minimized_flag_persists() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("overlay.json");

    let mut s = OverlayState::default();
    s.windows.insert(
        "mini".into(),
        PersistedWindow {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 150.0,
            minimized: true,
        },
    );
    state::save(&s, &path).unwrap();
    let loaded = state::load(&path).unwrap();
    assert!(loaded.windows["mini"].minimized);
}

#[test]
fn manager_save_load_preserves_positions_and_theme() {
    let mut mgr = WindowManager::new(Theme::Light);
    let mut w = make_window("panel", 250.0, 300.0);
    w.width = 400.0;
    w.height = 250.0;
    mgr.add_window(w);

    let json = mgr.save_state().unwrap();
    let mut mgr2 = WindowManager::new(Theme::Dark);
    mgr2.load_state(&json).unwrap();

    let win = mgr2.get_window("panel").unwrap();
    assert_eq!(win.x, 250.0);
    assert_eq!(win.y, 300.0);
    assert_eq!(mgr2.theme, Theme::Light);
}

// ── Input dispatch: focus switching between windows ──────────────────────────

#[test]
fn focus_click_w1_then_w2() {
    let (mut mgr, mut fm, mut disp) = make_mgr_two_windows();

    // Click inside w1 (default pos 100,100, size 200x150 → interior at 150,130).
    let ev1 = InputEvent::MouseDown { x: 150.0, y: 130.0 };
    let r1 = disp.dispatch(&ev1, &mut mgr, &mut fm);
    assert_eq!(r1, DispatchResult::Consumed);
    assert_eq!(fm.focused_window.as_deref(), Some("w1"));

    // Click inside w2 (pos 500,100 → interior at 550,130).
    let ev2 = InputEvent::MouseDown { x: 550.0, y: 130.0 };
    let r2 = disp.dispatch(&ev2, &mut mgr, &mut fm);
    assert_eq!(r2, DispatchResult::Consumed);
    assert_eq!(fm.focused_window.as_deref(), Some("w2"));
}

#[test]
fn focus_click_outside_blurs() {
    let (mut mgr, mut fm, mut disp) = make_mgr_two_windows();

    // Give w1 focus.
    fm.focus("w1");
    assert!(fm.overlay_has_focus());

    // Click in empty space.
    let ev = InputEvent::MouseDown { x: 5.0, y: 5.0 };
    disp.dispatch(&ev, &mut mgr, &mut fm);
    assert!(!fm.overlay_has_focus());
}

#[test]
fn focus_escape_blurs() {
    let (mut mgr, mut fm, mut disp) = make_mgr_two_windows();
    fm.focus("w1");

    let ev = InputEvent::KeyDown(Key::Escape);
    let result = disp.dispatch(&ev, &mut mgr, &mut fm);
    assert_eq!(result, DispatchResult::Consumed);
    assert!(!fm.overlay_has_focus());
}

// ── Input dispatch: keyboard+mouse combo ─────────────────────────────────────

#[test]
fn keyboard_consumed_after_mouse_focuses() {
    let (mut mgr, mut fm, mut disp) = make_mgr_two_windows();

    // Mouse click grants focus.
    disp.dispatch(
        &InputEvent::MouseDown { x: 150.0, y: 130.0 },
        &mut mgr,
        &mut fm,
    );
    assert!(fm.overlay_has_focus());

    // Keyboard input is now consumed by overlay.
    let key = InputEvent::CharInput('q');
    assert_eq!(
        disp.dispatch(&key, &mut mgr, &mut fm),
        DispatchResult::Consumed
    );
}

#[test]
fn keyboard_passes_through_before_focus() {
    let (mut mgr, mut fm, mut disp) = make_mgr_two_windows();
    // No focus yet — keyboard should pass through.
    let key = InputEvent::CharInput('a');
    assert_eq!(
        disp.dispatch(&key, &mut mgr, &mut fm),
        DispatchResult::PassThrough
    );
}

#[test]
fn mouse_move_always_passes_through() {
    let (mut mgr, mut fm, mut disp) = make_mgr_two_windows();
    // Even when overlay has focus, mouse-move is pass-through (EQ needs cursor).
    fm.focus("w1");
    let ev = InputEvent::MouseMove { x: 150.0, y: 130.0 };
    assert_eq!(
        disp.dispatch(&ev, &mut mgr, &mut fm),
        DispatchResult::PassThrough
    );
}

#[test]
fn mouse_wheel_consumed_over_window() {
    let (mut mgr, mut fm, mut disp) = make_mgr_two_windows();
    // Wheel over w1.
    let ev = InputEvent::MouseWheel {
        x: 150.0,
        y: 130.0,
        delta: 1.0,
    };
    assert_eq!(
        disp.dispatch(&ev, &mut mgr, &mut fm),
        DispatchResult::Consumed
    );
}

#[test]
fn mouse_wheel_passes_through_outside() {
    let (mut mgr, mut fm, mut disp) = make_mgr_two_windows();
    let ev = InputEvent::MouseWheel {
        x: 1.0,
        y: 1.0,
        delta: -1.0,
    };
    assert_eq!(
        disp.dispatch(&ev, &mut mgr, &mut fm),
        DispatchResult::PassThrough
    );
}

#[test]
fn tab_key_consumed_when_focused() {
    let (mut mgr, mut fm, mut disp) = make_mgr_two_windows();
    fm.focus("w1");
    let ev = InputEvent::KeyDown(Key::Tab);
    assert_eq!(
        disp.dispatch(&ev, &mut mgr, &mut fm),
        DispatchResult::Consumed
    );
}

#[test]
fn function_key_passes_through_when_not_focused() {
    let (mut mgr, mut fm, mut disp) = make_mgr_two_windows();
    // F1 without focus → EQ gets it (EQ uses F-keys for targeting etc.)
    let ev = InputEvent::KeyDown(Key::F(1));
    assert_eq!(
        disp.dispatch(&ev, &mut mgr, &mut fm),
        DispatchResult::PassThrough
    );
}

// ── Theme switching ───────────────────────────────────────────────────────────

#[test]
fn theme_dark_and_light_differ() {
    let dark = Theme::Dark.colors();
    let light = Theme::Light.colors();
    assert_ne!(
        dark.background, light.background,
        "dark/light backgrounds should differ"
    );
    assert_ne!(dark.header, light.header);
}

#[test]
fn theme_switch_reflected_in_manager() {
    let mut mgr = WindowManager::new(Theme::Dark);
    assert_eq!(mgr.theme, Theme::Dark);
    mgr.theme = Theme::Light;
    assert_eq!(mgr.theme, Theme::Light);
}

#[test]
fn theme_switch_serializes_and_deserializes() {
    let mut mgr = WindowManager::new(Theme::Light);
    mgr.add_window(make_window("t", 0.0, 0.0));
    let json = mgr.save_state().unwrap();
    let mut mgr2 = WindowManager::new(Theme::Dark);
    mgr2.load_state(&json).unwrap();
    assert_eq!(mgr2.theme, Theme::Light);
}

#[test]
fn theme_per_window_override() {
    let mut w = make_window("custom", 0.0, 0.0);
    w.theme = Some(Theme::Light);
    assert_eq!(w.theme, Some(Theme::Light));
    // Manager can be dark while one window opts into light.
    let mut mgr = WindowManager::new(Theme::Dark);
    mgr.add_window(w);
    let win = mgr.get_window("custom").unwrap();
    assert_eq!(win.theme, Some(Theme::Light));
    assert_eq!(mgr.theme, Theme::Dark);
}

// ── Draggable window ─────────────────────────────────────────────────────────

#[test]
fn window_drag_sequence_moves_position() {
    let mut mgr = WindowManager::new(Theme::Dark);
    mgr.add_window(make_window("drag_me", 100.0, 100.0));

    // Simulate drag: press header, move, release.
    mgr.mouse_down(150.0, 105.0); // inside header
    mgr.mouse_move(170.0, 120.0); // drag 20,15
    mgr.mouse_up(170.0, 120.0);

    let win = mgr.get_window("drag_me").unwrap();
    assert!(win.x > 100.0, "window should have moved right: x={}", win.x);
}

#[test]
fn window_drag_clamps_to_screen_boundary() {
    let mut mgr = WindowManager::new(Theme::Dark);
    mgr.add_window(make_window("clamped", 100.0, 100.0));

    mgr.mouse_down(150.0, 105.0);
    mgr.mouse_move(-9999.0, -9999.0); // try to drag off screen
    mgr.mouse_up(0.0, 0.0);

    let win = mgr.get_window("clamped").unwrap();
    assert!(win.x >= 0.0, "x should not go negative");
    assert!(win.y >= 0.0, "y should not go negative");
}

// ── Right-click context menu ──────────────────────────────────────────────────

#[test]
fn right_click_over_window_opens_context_menu() {
    let (mut mgr, mut fm, mut disp) = make_mgr_two_windows();
    let ev = InputEvent::RightClick { x: 150.0, y: 130.0 };
    let result = disp.dispatch(&ev, &mut mgr, &mut fm);
    assert_eq!(result, DispatchResult::Consumed);
    assert!(disp.context_menu.open);
}

#[test]
fn right_click_outside_passes_through() {
    let (mut mgr, mut fm, mut disp) = make_mgr_two_windows();
    let ev = InputEvent::RightClick { x: 1.0, y: 1.0 };
    let result = disp.dispatch(&ev, &mut mgr, &mut fm);
    assert_eq!(result, DispatchResult::PassThrough);
    assert!(!disp.context_menu.open);
}
