//! Terminal UI — ratatui-based dashboard with spawn list, player panels, map, navigation.

/// Application state — tracks active screen, selections, and all runtime data.
pub mod app;
/// TUI client wrapper — connects process reading to app state updates.
pub mod client;
/// Demo data generator — synthetic spawns and player data for macOS development.
pub mod demo_data;
/// Event handling — keyboard input mapping and command dispatch.
pub mod event;
/// TUI run loop — terminal setup, tick/render cycle, graceful shutdown.
pub mod run;
/// ASCII sprite definitions for the map overlay.
pub mod sprites;
/// Per-screen UI state — scroll positions, selections, input buffers.
pub mod state;
/// Color theme system — multiple themes with per-element color definitions.
pub mod theme;
/// UI renderers — per-panel drawing functions for each dashboard section.
pub mod ui;
