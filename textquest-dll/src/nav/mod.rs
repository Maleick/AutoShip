//! Navigation module — autonomous waypoint-based movement.

pub mod humanize;
pub mod state;
pub mod stick;
pub mod stuck;
pub mod warp;
pub mod waypoint;
pub mod zone_graph;

pub use state::Navigator;
use warp::TargetSample;

use std::sync::Mutex;

use textquest_common::nav::{
    CampSpot, FollowConfig, HeadingMode, MoveToConfig, NavCampConfig, NavStatus, StickConfig,
    Waypoint,
};
use textquest_common::types::SpawnData;

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
///
/// `current_target` and `nearby` are used by the stick engine.
/// `target_sample` is used by the warp monitor.
pub fn tick(
    current_target: Option<&SpawnData>,
    nearby: &[SpawnData],
    target_sample: Option<&TargetSample>,
) {
    if let Some(ref mut nav) = *NAVIGATOR
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        nav.tick(current_target, nearby, target_sample);
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
            NavCommand::UpdateFollowConfig(config) => nav.update_follow_config(config),
            NavCommand::StickTo {
                config,
                current_target_id,
            } => {
                nav.stick_to(config, current_target_id);
            }
            NavCommand::StickOff => nav.stick_off(),
            NavCommand::StickMod(delta) => nav.stick_mod(delta),
            NavCommand::SetCampConfig(config) => nav.set_camp_config(config),
            NavCommand::Pause => nav.pause(),
            NavCommand::Resume => nav.resume(),
            NavCommand::SetMeshLoaded(loaded) => nav.set_mesh_loaded(loaded),
            NavCommand::MoveToAdvanced(config) => nav.move_to_advanced(config),
            NavCommand::SetAutopause(enabled) => nav.set_autopause(enabled),
            NavCommand::SetBreakOnGm(enabled) => nav.set_break_on_gm(enabled),
            NavCommand::SetHeadingMode(mode) => nav.set_heading_mode(mode),
        }
    }
}

/// Get navigation state signals for IPC queries (#176).
pub fn signals() -> textquest_common::nav::NavStateSignals {
    NAVIGATOR
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map_or_else(Default::default, state::Navigator::signals)
}

/// Get navigation diagnostics for debug overlay (#177).
pub fn diagnostics() -> textquest_common::nav::NavDiagnostics {
    NAVIGATOR
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map_or_else(Default::default, state::Navigator::diagnostics)
}

/// Apply a mutation to the active follow config.
///
/// Used by the `/makecamp` slash command handler to update individual
/// return-policy fields (mindelay, maxdelay, returnnoaggro, returnnotlooting)
/// on the live follow session without restarting it.
/// If no follow session is active the closure is never called.
pub fn update_follow_policy<F>(f: F)
where
    F: FnOnce(&mut FollowConfig),
{
    if let Some(ref mut nav) = *NAVIGATOR
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        nav.mutate_follow_config(f);
    }
}

/// Commands that can be sent to the navigator.
pub enum NavCommand {
    Navigate(Vec<Waypoint>),
    SetCamp(CampSpot),
    SetCampConfig(NavCampConfig),
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
    /// Update the return-policy options of an active follow session at runtime.
    /// See `Navigator::update_follow_config` for semantics.
    UpdateFollowConfig(FollowConfig),
    /// Begin a stick session with the given config.
    /// `current_target_id` is used for `hold` locking.
    StickTo {
        config: StickConfig,
        current_target_id: Option<u32>,
    },
    /// Stop sticking.
    StickOff,
    /// Adjust the active stick distance modifier.
    StickMod(f32),
    /// Pause navigation, retaining path (#168).
    Pause,
    /// Resume from user-initiated pause (#168).
    Resume,
    /// Set mesh loaded status (#174).
    SetMeshLoaded(bool),
    /// Advanced moveto with full MQ2MoveUtils options (#184).
    MoveToAdvanced(MoveToConfig),
    /// Enable or disable autopause globally (#164).
    SetAutopause(bool),
    /// Enable or disable break-on-GM safety halt.
    SetBreakOnGm(bool),
    /// Set the heading update mode (true / loose / fast).
    SetHeadingMode(HeadingMode),
}
