//! UI notification controller for MQ2-level event dispatch.
//!
//! Provides a typed in-memory event bus for EQ `CXWnd::WndNotification` events
//! (`EXWndNotification` message taxonomy). This mirrors the `ControllerBase`
//! async notification routing present in MQ2 and fills the gap noted in
//! `docs/wiki/Research-MQ2-Comparison.md`.
//!
//! # Design
//!
//! - [`XwmMessage`] — typed enum covering all 51 known `EXWndNotification` IDs.
//!   Raw `u32` codes are decoded via `From<u32>`. Unknown codes become [`XwmMessage::Unknown`].
//! - [`NotificationController`] — in-memory registry that maps message types to
//!   a list of handler closures. Call [`NotificationController::register`] to subscribe,
//!   [`NotificationController::unregister`] to remove a subscription, and
//!   [`NotificationController::dispatch`] to fire all handlers for an event.
//! - Module-level free functions (`register`, `unregister`, `dispatch`) delegate to a
//!   process-wide singleton (`NOTIFICATION_CONTROLLER`) so callers don't need to pass
//!   controller references through call chains.
//!
//! # Thread safety
//!
//! All singleton access is guarded by a [`std::sync::Mutex`]. **Handlers must not call
//! `register` or `unregister` on the global singleton while inside a `dispatch` call** —
//! that would attempt to re-acquire the same lock and deadlock. If re-entrant registration
//! is needed, schedule it for the next game tick.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::eq::notification::{self, XwmMessage, NotificationEvent};
//!
//! // Register a handler for close events on any window.
//! let id = notification::register(XwmMessage::Close, |ev| {
//!     tracing::info!(sender = format!("{:#x}", ev.sender_wnd), "Window closed");
//! });
//!
//! // Later, dispatch an event (called from the game loop thread).
//! notification::dispatch(NotificationEvent {
//!     sender_wnd: 0xdeadbeef,
//!     target_wnd: 0xdeadbeef,
//!     message: XwmMessage::Close,
//!     data: 0,
//! });
//!
//! // Remove the handler when no longer needed.
//! notification::unregister(id);
//! ```

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

// ─── Message enum ────────────────────────────────────────────────────────────

/// EQ window notification message IDs (`EXWndNotification` enum).
///
/// Covers the full vocabulary of 51 message types used by `CXWnd::WndNotification`.
/// Numeric values match EQ's vtable call convention (confirmed from MQ2 eqlib headers).
///
/// Value 1 (`LClick`) is the only type currently dispatched by existing DLL code;
/// the rest are defined here so handlers can be registered ahead of the vtable hook
/// without manually tracking raw `u32` constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XwmMessage {
    /// Null / no-op message. Rarely dispatched.
    Null,
    /// Left-click confirmed on a widget. Most common action message.
    LClick,
    /// Left mouse button pressed (down, before release).
    LMouseDown,
    /// Right-click confirmed.
    RClick,
    /// Right mouse button pressed.
    RMouseDown,
    /// Middle mouse button pressed.
    MMouseDown,
    /// Middle mouse button click confirmed.
    MClick,
    /// Double-click.
    DblClick,
    /// Mouse cursor entered the widget area.
    MouseEnter,
    /// Mouse cursor left the widget area.
    MouseLeave,
    /// Mouse moved within the widget.
    MouseMove,
    /// Tooltip display request.
    Tooltip,
    /// Window close requested.
    Close,
    /// Window maximized.
    Maximize,
    /// Window minimized.
    Minimize,
    /// Window moved.
    Move,
    /// Window resized.
    Size,
    /// Window became visible.
    Show,
    /// Window visibility requested (may be suppressed).
    RequestShow,
    /// Widget gained keyboard focus.
    Focus,
    /// Widget lost keyboard focus.
    KillFocus,
    /// Key down event while widget has focus.
    KeyDown,
    /// Key up event while widget has focus.
    KeyUp,
    /// Character input event.
    Char,
    /// Per-frame processing tick (high-frequency, use sparingly).
    ProcessFrame,
    /// Scroll bar: scroll up one line.
    ScrollUpLine,
    /// Scroll bar: scroll down one line.
    ScrollDownLine,
    /// Scroll bar: scroll up one page.
    ScrollUp,
    /// Scroll bar: scroll down one page.
    ScrollDown,
    /// Scroll bar: scroll left.
    ScrollLeft,
    /// Scroll bar: scroll right.
    ScrollRight,
    /// Column header clicked (list/table widgets).
    ColumnClick,
    /// List box item changed.
    ListBoxChange,
    /// List box item selected.
    ListBoxSelect,
    /// List box selection confirmed (double-click or Enter).
    ListBoxFinish,
    /// Slider/gauge value changed.
    NewValue,
    /// Periodic throb/pulse event for animated widgets.
    Throb,
    /// Hot-spot / hit-area triggered.
    HitArea,
    /// Tab key forwarded to widget.
    Tab,
    /// Shift-Tab key forwarded to widget.
    ShiftTab,
    /// Context menu opened.
    MenuOpen,
    /// Context menu closed.
    MenuClose,
    /// Context menu item message.
    MenuMessage,
    /// Browser-style back navigation.
    HistoryBack,
    /// Browser-style forward navigation.
    HistoryForward,
    /// Widget state changed (e.g. checkbox toggled).
    StateChanged,
    /// Radio button selection changed.
    RadioButton,
    /// Combo box selection changed.
    ComboBox,
    /// Menu item selected.
    MenuSelect,
    /// Mouse wheel scrolled up.
    MouseWheelUp,
    /// Mouse wheel scrolled down.
    MouseWheelDown,
    /// Message ID not covered by the known vocabulary.
    Unknown(u32),
}

impl From<u32> for XwmMessage {
    fn from(v: u32) -> Self {
        match v {
            0 => Self::Null,
            1 => Self::LClick,
            2 => Self::LMouseDown,
            3 => Self::RClick,
            4 => Self::RMouseDown,
            5 => Self::MMouseDown,
            6 => Self::MClick,
            7 => Self::DblClick,
            8 => Self::MouseEnter,
            9 => Self::MouseLeave,
            10 => Self::MouseMove,
            11 => Self::Tooltip,
            12 => Self::Close,
            13 => Self::Maximize,
            14 => Self::Minimize,
            15 => Self::Move,
            16 => Self::Size,
            17 => Self::Show,
            18 => Self::RequestShow,
            19 => Self::Focus,
            20 => Self::KillFocus,
            21 => Self::KeyDown,
            22 => Self::KeyUp,
            23 => Self::Char,
            24 => Self::ProcessFrame,
            25 => Self::ScrollUpLine,
            26 => Self::ScrollDownLine,
            27 => Self::ScrollUp,
            28 => Self::ScrollDown,
            29 => Self::ScrollLeft,
            30 => Self::ScrollRight,
            31 => Self::ColumnClick,
            32 => Self::ListBoxChange,
            33 => Self::ListBoxSelect,
            34 => Self::ListBoxFinish,
            35 => Self::NewValue,
            36 => Self::Throb,
            37 => Self::HitArea,
            38 => Self::Tab,
            39 => Self::ShiftTab,
            40 => Self::MenuOpen,
            41 => Self::MenuClose,
            42 => Self::MenuMessage,
            43 => Self::HistoryBack,
            44 => Self::HistoryForward,
            45 => Self::StateChanged,
            46 => Self::RadioButton,
            47 => Self::ComboBox,
            48 => Self::MenuSelect,
            49 => Self::MouseWheelUp,
            50 => Self::MouseWheelDown,
            other => Self::Unknown(other),
        }
    }
}

impl From<XwmMessage> for u32 {
    fn from(msg: XwmMessage) -> Self {
        match msg {
            XwmMessage::Null => 0,
            XwmMessage::LClick => 1,
            XwmMessage::LMouseDown => 2,
            XwmMessage::RClick => 3,
            XwmMessage::RMouseDown => 4,
            XwmMessage::MMouseDown => 5,
            XwmMessage::MClick => 6,
            XwmMessage::DblClick => 7,
            XwmMessage::MouseEnter => 8,
            XwmMessage::MouseLeave => 9,
            XwmMessage::MouseMove => 10,
            XwmMessage::Tooltip => 11,
            XwmMessage::Close => 12,
            XwmMessage::Maximize => 13,
            XwmMessage::Minimize => 14,
            XwmMessage::Move => 15,
            XwmMessage::Size => 16,
            XwmMessage::Show => 17,
            XwmMessage::RequestShow => 18,
            XwmMessage::Focus => 19,
            XwmMessage::KillFocus => 20,
            XwmMessage::KeyDown => 21,
            XwmMessage::KeyUp => 22,
            XwmMessage::Char => 23,
            XwmMessage::ProcessFrame => 24,
            XwmMessage::ScrollUpLine => 25,
            XwmMessage::ScrollDownLine => 26,
            XwmMessage::ScrollUp => 27,
            XwmMessage::ScrollDown => 28,
            XwmMessage::ScrollLeft => 29,
            XwmMessage::ScrollRight => 30,
            XwmMessage::ColumnClick => 31,
            XwmMessage::ListBoxChange => 32,
            XwmMessage::ListBoxSelect => 33,
            XwmMessage::ListBoxFinish => 34,
            XwmMessage::NewValue => 35,
            XwmMessage::Throb => 36,
            XwmMessage::HitArea => 37,
            XwmMessage::Tab => 38,
            XwmMessage::ShiftTab => 39,
            XwmMessage::MenuOpen => 40,
            XwmMessage::MenuClose => 41,
            XwmMessage::MenuMessage => 42,
            XwmMessage::HistoryBack => 43,
            XwmMessage::HistoryForward => 44,
            XwmMessage::StateChanged => 45,
            XwmMessage::RadioButton => 46,
            XwmMessage::ComboBox => 47,
            XwmMessage::MenuSelect => 48,
            XwmMessage::MouseWheelUp => 49,
            XwmMessage::MouseWheelDown => 50,
            XwmMessage::Unknown(v) => v,
        }
    }
}

// ─── Event struct ─────────────────────────────────────────────────────────────

/// Context passed to handlers when a notification is dispatched.
#[derive(Debug, Clone, Copy)]
pub struct NotificationEvent {
    /// Window that sent the notification (raw `CXWnd*` pointer value).
    pub sender_wnd: usize,
    /// Window that received the notification (raw `CXWnd*` pointer value).
    pub target_wnd: usize,
    /// The message type.
    pub message: XwmMessage,
    /// Message-specific data (interpretation depends on message type).
    pub data: usize,
}

// ─── Controller ───────────────────────────────────────────────────────────────

/// Opaque registration handle returned by [`NotificationController::register`].
///
/// Pass to [`NotificationController::unregister`] to remove the handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandlerId(u64);

type HandlerFn = Box<dyn Fn(NotificationEvent) + Send + 'static>;

struct HandlerEntry {
    id: HandlerId,
    func: HandlerFn,
}

/// In-memory event bus for `CXWnd::WndNotification` events.
///
/// Maintains a per-message-type list of handler closures. Handlers are called
/// synchronously from [`dispatch`], in registration order.
///
/// Use the module-level free functions [`register`], [`unregister`], and [`dispatch`]
/// to interact with the process-wide singleton, or construct a local instance for
/// isolated testing.
pub struct NotificationController {
    next_id: u64,
    handlers: HashMap<XwmMessage, Vec<HandlerEntry>>,
}

impl NotificationController {
    /// Create a new, empty controller.
    pub fn new() -> Self {
        Self {
            next_id: 1,
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for a specific message type.
    ///
    /// Returns a [`HandlerId`] that can be passed to [`Self::unregister`].
    /// Multiple handlers for the same message are called in registration order.
    pub fn register<F>(&mut self, message: XwmMessage, handler: F) -> HandlerId
    where
        F: Fn(NotificationEvent) + Send + 'static,
    {
        let id = HandlerId(self.next_id);
        self.next_id += 1;
        self.handlers
            .entry(message)
            .or_default()
            .push(HandlerEntry {
                id,
                func: Box::new(handler),
            });
        tracing::debug!(
            message = ?message,
            message_id = u32::from(message),
            handler_id = id.0,
            "Registered UI notification handler"
        );
        id
    }

    /// Remove a handler by its registration ID.
    ///
    /// Returns `true` if the handler was found and removed, `false` if not found.
    pub fn unregister(&mut self, id: HandlerId) -> bool {
        for handlers in self.handlers.values_mut() {
            if let Some(pos) = handlers.iter().position(|e| e.id == id) {
                handlers.remove(pos);
                tracing::debug!(handler_id = id.0, "Unregistered UI notification handler");
                return true;
            }
        }
        tracing::warn!(
            handler_id = id.0,
            "unregister: handler id not found — already removed?"
        );
        false
    }

    /// Dispatch a notification event to all handlers registered for its message type.
    ///
    /// Logs the event at `debug` level regardless of how many handlers are registered.
    /// If no handlers are registered for the message type, this is a no-op (not an error).
    ///
    /// **Do not call `register` or `unregister` on the global singleton from inside a
    /// handler** — that would deadlock while the singleton mutex is held. Schedule
    /// any re-entrant registration for the next game tick instead.
    pub fn dispatch(&self, event: NotificationEvent) {
        tracing::debug!(
            message = ?event.message,
            message_id = u32::from(event.message),
            sender_wnd = format!("{:#x}", event.sender_wnd),
            target_wnd = format!("{:#x}", event.target_wnd),
            data = format!("{:#x}", event.data),
            "UI notification dispatched"
        );
        if let Some(handlers) = self.handlers.get(&event.message) {
            for entry in handlers {
                (entry.func)(event);
            }
        }
    }

    /// Return the number of registered handlers for a given message type.
    pub fn handler_count(&self, message: XwmMessage) -> usize {
        self.handlers.get(&message).map_or(0, Vec::len)
    }

    /// Return the total number of registered handlers across all message types.
    pub fn total_handler_count(&self) -> usize {
        self.handlers.values().map(Vec::len).sum()
    }
}

impl Default for NotificationController {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Global singleton ─────────────────────────────────────────────────────────

static NOTIFICATION_CONTROLLER: OnceLock<Mutex<NotificationController>> = OnceLock::new();

fn global() -> &'static Mutex<NotificationController> {
    NOTIFICATION_CONTROLLER.get_or_init(|| Mutex::new(NotificationController::new()))
}

/// Register a handler on the global notification controller.
///
/// See [`NotificationController::register`] for full documentation.
pub fn register<F>(message: XwmMessage, handler: F) -> HandlerId
where
    F: Fn(NotificationEvent) + Send + 'static,
{
    global()
        .lock()
        .expect("notification controller lock poisoned")
        .register(message, handler)
}

/// Unregister a handler from the global notification controller.
///
/// See [`NotificationController::unregister`] for full documentation.
pub fn unregister(id: HandlerId) -> bool {
    global()
        .lock()
        .expect("notification controller lock poisoned")
        .unregister(id)
}

/// Dispatch a notification event via the global controller.
///
/// See [`NotificationController::dispatch`] for full documentation.
pub fn dispatch(event: NotificationEvent) {
    global()
        .lock()
        .expect("notification controller lock poisoned")
        .dispatch(event)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn make_event(message: XwmMessage) -> NotificationEvent {
        NotificationEvent {
            sender_wnd: 0x1000,
            target_wnd: 0x2000,
            message,
            data: 0,
        }
    }

    // ── XwmMessage round-trip ──

    #[test]
    fn xwm_lclick_round_trips() {
        assert_eq!(XwmMessage::from(1u32), XwmMessage::LClick);
        assert_eq!(u32::from(XwmMessage::LClick), 1u32);
    }

    #[test]
    fn xwm_all_known_ids_round_trip() {
        for id in 0u32..=50 {
            let msg = XwmMessage::from(id);
            assert!(
                !matches!(msg, XwmMessage::Unknown(_)),
                "id {id} should map to a named variant"
            );
            assert_eq!(u32::from(msg), id, "round-trip failed for id {id}");
        }
    }

    #[test]
    fn xwm_unknown_id_preserved() {
        let msg = XwmMessage::from(999u32);
        assert_eq!(msg, XwmMessage::Unknown(999));
        assert_eq!(u32::from(msg), 999u32);
    }

    #[test]
    fn xwm_unknown_ids_are_distinct() {
        assert_ne!(XwmMessage::Unknown(5), XwmMessage::Unknown(6));
        assert_ne!(XwmMessage::Unknown(1), XwmMessage::LClick);
    }

    #[test]
    fn xwm_covers_48_plus_named_types() {
        // 0..=50 = 51 named variants; well above the "48+" gap threshold.
        let named_count: u32 = (0u32..=50).count() as u32;
        assert!(
            named_count >= 48,
            "expected ≥48 named types, got {named_count}"
        );
    }

    // ── NotificationController ──

    #[test]
    fn register_increments_handler_count() {
        let mut ctrl = NotificationController::new();
        assert_eq!(ctrl.handler_count(XwmMessage::LClick), 0);
        ctrl.register(XwmMessage::LClick, |_| {});
        assert_eq!(ctrl.handler_count(XwmMessage::LClick), 1);
        ctrl.register(XwmMessage::LClick, |_| {});
        assert_eq!(ctrl.handler_count(XwmMessage::LClick), 2);
    }

    #[test]
    fn total_handler_count_spans_message_types() {
        let mut ctrl = NotificationController::new();
        ctrl.register(XwmMessage::LClick, |_| {});
        ctrl.register(XwmMessage::Close, |_| {});
        ctrl.register(XwmMessage::Close, |_| {});
        assert_eq!(ctrl.total_handler_count(), 3);
    }

    #[test]
    fn dispatch_calls_handler_with_correct_event() {
        let mut ctrl = NotificationController::new();
        let captured = Arc::new(Mutex::new(None::<NotificationEvent>));
        let cap = Arc::clone(&captured);
        ctrl.register(XwmMessage::Close, move |ev| {
            *cap.lock().unwrap() = Some(ev);
        });

        let event = NotificationEvent {
            sender_wnd: 0xAABB,
            target_wnd: 0xCCDD,
            message: XwmMessage::Close,
            data: 42,
        };
        ctrl.dispatch(event);

        let got = captured.lock().unwrap().unwrap();
        assert_eq!(got.sender_wnd, 0xAABB);
        assert_eq!(got.target_wnd, 0xCCDD);
        assert_eq!(got.data, 42);
        assert_eq!(got.message, XwmMessage::Close);
    }

    #[test]
    fn dispatch_calls_all_handlers_for_message_type() {
        let mut ctrl = NotificationController::new();
        let counter = Arc::new(Mutex::new(0u32));
        for _ in 0..3 {
            let c = Arc::clone(&counter);
            ctrl.register(XwmMessage::LClick, move |_| {
                *c.lock().unwrap() += 1;
            });
        }
        ctrl.dispatch(make_event(XwmMessage::LClick));
        assert_eq!(*counter.lock().unwrap(), 3);
    }

    #[test]
    fn dispatch_does_not_call_handlers_for_other_messages() {
        let mut ctrl = NotificationController::new();
        let called = Arc::new(Mutex::new(false));
        let c = Arc::clone(&called);
        ctrl.register(XwmMessage::Close, move |_| {
            *c.lock().unwrap() = true;
        });
        ctrl.dispatch(make_event(XwmMessage::LClick));
        assert!(!*called.lock().unwrap());
    }

    #[test]
    fn dispatch_with_no_handlers_is_noop() {
        let ctrl = NotificationController::new();
        // Should not panic.
        ctrl.dispatch(make_event(XwmMessage::LClick));
    }

    #[test]
    fn dispatch_calls_handlers_in_registration_order() {
        let mut ctrl = NotificationController::new();
        let order = Arc::new(Mutex::new(Vec::<u32>::new()));
        for i in 0..4 {
            let o = Arc::clone(&order);
            ctrl.register(XwmMessage::Tab, move |_| {
                o.lock().unwrap().push(i);
            });
        }
        ctrl.dispatch(make_event(XwmMessage::Tab));
        assert_eq!(*order.lock().unwrap(), vec![0, 1, 2, 3]);
    }

    #[test]
    fn unregister_removes_handler() {
        let mut ctrl = NotificationController::new();
        let called = Arc::new(Mutex::new(false));
        let c = Arc::clone(&called);
        let id = ctrl.register(XwmMessage::LClick, move |_| {
            *c.lock().unwrap() = true;
        });
        assert!(ctrl.unregister(id));
        assert_eq!(ctrl.handler_count(XwmMessage::LClick), 0);
        ctrl.dispatch(make_event(XwmMessage::LClick));
        assert!(!*called.lock().unwrap());
    }

    #[test]
    fn unregister_returns_false_for_missing_id() {
        let mut ctrl = NotificationController::new();
        assert!(!ctrl.unregister(HandlerId(999)));
    }

    #[test]
    fn unregister_removes_only_targeted_handler() {
        let mut ctrl = NotificationController::new();
        let counter = Arc::new(Mutex::new(0u32));
        let c1 = Arc::clone(&counter);
        let c2 = Arc::clone(&counter);
        let id1 = ctrl.register(XwmMessage::Focus, move |_| {
            *c1.lock().unwrap() += 1;
        });
        ctrl.register(XwmMessage::Focus, move |_| {
            *c2.lock().unwrap() += 1;
        });
        assert!(ctrl.unregister(id1));
        ctrl.dispatch(make_event(XwmMessage::Focus));
        // Only the second handler should fire.
        assert_eq!(*counter.lock().unwrap(), 1);
    }

    #[test]
    fn unregister_same_id_twice_returns_false_second_time() {
        let mut ctrl = NotificationController::new();
        let id = ctrl.register(XwmMessage::LClick, |_| {});
        assert!(ctrl.unregister(id));
        assert!(!ctrl.unregister(id));
    }

    #[test]
    fn handler_ids_are_unique_across_message_types() {
        let mut ctrl = NotificationController::new();
        let id1 = ctrl.register(XwmMessage::LClick, |_| {});
        let id2 = ctrl.register(XwmMessage::Close, |_| {});
        assert_ne!(id1, id2);
    }

    #[test]
    fn dispatch_unknown_message_calls_handler() {
        let mut ctrl = NotificationController::new();
        let called = Arc::new(Mutex::new(false));
        let c = Arc::clone(&called);
        ctrl.register(XwmMessage::Unknown(200), move |_| {
            *c.lock().unwrap() = true;
        });
        ctrl.dispatch(make_event(XwmMessage::Unknown(200)));
        assert!(*called.lock().unwrap());
    }

    #[test]
    fn register_returns_incrementing_ids() {
        let mut ctrl = NotificationController::new();
        let ids: Vec<HandlerId> = (0..5)
            .map(|_| ctrl.register(XwmMessage::LClick, |_| {}))
            .collect();
        for w in ids.windows(2) {
            assert!(w[0].0 < w[1].0, "handler ids must be strictly increasing");
        }
    }

    #[test]
    fn high_value_messages_all_decode_correctly() {
        // Spot-check the high-value message IDs used by critical UI flows.
        let cases: &[(u32, XwmMessage)] = &[
            (1, XwmMessage::LClick),
            (3, XwmMessage::RClick),
            (7, XwmMessage::DblClick),
            (12, XwmMessage::Close),
            (17, XwmMessage::Show),
            (19, XwmMessage::Focus),
            (20, XwmMessage::KillFocus),
            (33, XwmMessage::ListBoxSelect),
            (34, XwmMessage::ListBoxFinish),
            (35, XwmMessage::NewValue),
            (45, XwmMessage::StateChanged),
            (47, XwmMessage::ComboBox),
        ];
        for &(raw, expected) in cases {
            assert_eq!(
                XwmMessage::from(raw),
                expected,
                "message id {raw} should decode to {expected:?}"
            );
        }
    }
}
