//! Terminal UI — ratatui-based dashboard with spawn list, player panels, map, navigation.

/// Achievement system — milestones, raid firsts, and progression unlocks.
pub mod achievements;
/// Application state — tracks active screen, selections, and all runtime data.
pub mod app;
/// Shared cast presentation helpers for TUI surfaces.
pub mod cast;
/// TUI client wrapper — connects process reading to app state updates.
pub mod client;
/// Shared command metadata for help, hints, and suggestions.
pub mod command;
/// Configuration panel with tree view and inline editing.
pub mod config_panel;
/// Demo data generator — synthetic spawns and player data for macOS development.
pub mod demo_data;
/// DPS tracker — rolling-window damage-per-second calculation for group members.
pub mod dps;
/// Event handling — keyboard input mapping and command dispatch.
pub mod event;
/// Env-gated live cast capture helpers for validating real EQ clients.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) mod live_cast_capture;
/// Dropdown menu bar system for accessible command navigation.
pub mod menu;
/// TUI run loop — terminal setup, tick/render cycle, graceful shutdown.
pub mod run;
/// Session monitor — fleet overview and per-client drill-down tracking.
pub mod session_monitor;
/// ASCII sprite definitions for the map overlay.
pub mod sprites;
/// Per-screen UI state — scroll positions, selections, input buffers.
pub mod state;
/// Color theme system — multiple themes with per-element color definitions.
pub mod theme;
/// Toast notification system — ephemeral messages for achievements, warnings, and status.
pub mod toast;
/// UI renderers — per-panel drawing functions for each dashboard section.
pub mod ui;
/// Onboarding wizard for first-run setup.
pub mod wizard;
