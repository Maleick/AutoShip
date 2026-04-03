//! Navigation module — autonomous waypoint-based movement.

pub mod humanize;
pub mod state;
pub mod stuck;
pub mod waypoint;
pub mod zone_graph;

pub use state::Navigator;

use std::sync::Mutex;

use dmft_common::nav::{CampSpot, FollowConfig, NavStatus, Waypoint};

/// Global navigator instance, persists across game ticks.
/// `Mutex<Option<...>>` because the game loop is single-threaded but
/// commands arrive from the IPC thread.
static NAVIGATOR: Mutex<Option<Navigator>> = Mutex::new(None);

/// Initialize the global navigator with the player base address.
pub fn init(player_base: usize, client_id: u32) {
    let mut nav = NAVIGATOR
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *nav = Some(Navigator::new(player_base, client_id));
    tracing::info!(client_id, "Navigator initialized");
}

/// Run one navigation tick. Call from `on_game_tick()`.
pub fn tick() {
    if let Some(ref mut nav) = *NAVIGATOR
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        nav.tick();
    }
}

/// Get current navigation status for IPC reporting.
pub fn status() -> NavStatus {
    NAVIGATOR
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map_or(NavStatus::Idle, state::Navigator::status)
}

/// Handle a navigation command from IPC.
pub fn handle_command(cmd: NavCommand) {
    if let Some(ref mut nav) = *NAVIGATOR
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        match cmd {
            NavCommand::Navigate(waypoints) => nav.navigate(waypoints),
            NavCommand::SetCamp(spot) => nav.set_camp(spot),
            NavCommand::Stop => nav.stop(),
            NavCommand::FollowPlayer { config, anchor } => nav.follow_player(config, anchor),
            NavCommand::UpdateFollowAnchor(anchor) => nav.update_follow_anchor(anchor),
            NavCommand::StopFollow => nav.stop_follow(),
        }
    }
}

/// Commands that can be sent to the navigator.
pub enum NavCommand {
    Navigate(Vec<Waypoint>),
    SetCamp(CampSpot),
    Stop,
    /// Start MQ2MoveUtils-style player follow mode.
    FollowPlayer {
        /// Follow configuration (leader name, follow distance, leash distance).
        config: FollowConfig,
        /// Initial anchor position (leader's current location).
        anchor: Waypoint,
    },
    /// Update the dynamic anchor in an active follow mode.
    UpdateFollowAnchor(Waypoint),
    /// Stop player follow mode.
    StopFollow,
}
