//! Overlay window-manager and widget system.
//!
//! Entry points:
//! - [`manager::WindowManager`] — owns windows, routes mouse events
//! - [`window::Window`] — draggable/resizable overlay window
//! - [`widget::Widget`] — leaf UI elements (Button, Text, List, Dropdown)
//! - [`layout`] — grid/flex layout for widget placement
//! - [`theme::Theme`] — dark/light color scheme

pub mod layout;
pub mod manager;
pub mod state;
pub mod theme;
pub mod widget;
pub mod window;
