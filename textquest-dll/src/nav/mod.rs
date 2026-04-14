//! Navigation module — autonomous waypoint-based movement.

pub mod humanize;
pub mod state;
pub mod stick;
pub mod stuck;
pub mod warp;
pub mod waypoint;
pub mod waypoint_store;
pub mod zone_graph;

pub use state::Navigator;
use warp::TargetSample;

use std::sync::Mutex;

use textquest_common::nav::{
    CampSpot, CircleConfig, FollowConfig, MoveToConfig, NavCampConfig, NavStatus, StickConfig,
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
            NavCommand::CircleKite { config, center } => nav.circle_kite(config, center),
            NavCommand::CircleOff => nav.circle_off(),
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
    /// Start circle-kiting mode around a center point.
    CircleKite {
        /// Circle kiting configuration (radius, mode, target_id, etc.).
        config: CircleConfig,
        /// Center of the circle.  `None` means use the player's current position.
        center: Option<Waypoint>,
    },
    /// Stop circle-kiting and return to Idle.
    CircleOff,
    /// Set the heading update mode.
    SetHeadingMode(textquest_common::nav::HeadingMode),
}

#[cfg(test)]
mod tests {
    use textquest_common::nav::{
        CampSpot, CircleConfig, FollowConfig, HeadingMode, NavDiagnostics, NavStateSignals,
        NavStatus, StickConfig, Waypoint,
    };

    use super::*;

    // ─── NavCommand enum: variant construction ─────────────────────────────────

    #[test]
    fn nav_command_navigate_is_constructible() {
        let cmd = NavCommand::Navigate(vec![]);
        assert!(matches!(cmd, NavCommand::Navigate(_)));
    }

    #[test]
    fn nav_command_set_camp_is_constructible() {
        let cmd = NavCommand::SetCamp(CampSpot {
            position: Waypoint::new(0.0, 0.0, 0.0),
            heading: 0.0,
            role: String::new(),
        });
        assert!(matches!(cmd, NavCommand::SetCamp(_)));
    }

    #[test]
    fn nav_command_stop_is_constructible() {
        let cmd = NavCommand::Stop;
        assert!(matches!(cmd, NavCommand::Stop));
    }

    #[test]
    fn nav_command_pause_resume_are_constructible() {
        assert!(matches!(NavCommand::Pause, NavCommand::Pause));
        assert!(matches!(NavCommand::Resume, NavCommand::Resume));
    }

    #[test]
    fn nav_command_set_mesh_loaded_stores_bool() {
        let cmd = NavCommand::SetMeshLoaded(true);
        if let NavCommand::SetMeshLoaded(v) = cmd {
            assert!(v);
        } else {
            panic!("expected SetMeshLoaded");
        }
    }

    #[test]
    fn nav_command_set_autopause_stores_bool() {
        let cmd = NavCommand::SetAutopause(false);
        if let NavCommand::SetAutopause(v) = cmd {
            assert!(!v);
        } else {
            panic!("expected SetAutopause");
        }
    }

    #[test]
    fn nav_command_set_break_on_gm_stores_bool() {
        let cmd = NavCommand::SetBreakOnGm(true);
        if let NavCommand::SetBreakOnGm(v) = cmd {
            assert!(v);
        } else {
            panic!("expected SetBreakOnGm");
        }
    }

    #[test]
    fn nav_command_stick_to_stores_fields() {
        let cmd = NavCommand::StickTo {
            config: StickConfig::default(),
            current_target_id: Some(42),
        };
        if let NavCommand::StickTo {
            current_target_id, ..
        } = cmd
        {
            assert_eq!(current_target_id, Some(42));
        } else {
            panic!("expected StickTo");
        }
    }

    #[test]
    fn nav_command_stick_mod_stores_delta() {
        let cmd = NavCommand::StickMod(3.5);
        if let NavCommand::StickMod(d) = cmd {
            assert!((d - 3.5).abs() < f32::EPSILON);
        } else {
            panic!("expected StickMod");
        }
    }

    #[test]
    fn nav_command_follow_player_is_constructible() {
        let cmd = NavCommand::FollowPlayer {
            config: FollowConfig::default(),
            anchor: Waypoint::new(0.0, 0.0, 0.0),
        };
        assert!(matches!(cmd, NavCommand::FollowPlayer { .. }));
    }

    #[test]
    fn nav_command_circle_off_is_constructible() {
        let cmd = NavCommand::CircleOff;
        assert!(matches!(cmd, NavCommand::CircleOff));
    }

    #[test]
    fn nav_command_circle_kite_is_constructible() {
        let cmd = NavCommand::CircleKite {
            config: CircleConfig::default(),
            center: None,
        };
        assert!(matches!(cmd, NavCommand::CircleKite { .. }));
    }

    #[test]
    fn nav_command_set_heading_mode_is_constructible() {
        let cmd = NavCommand::SetHeadingMode(HeadingMode::default());
        assert!(matches!(cmd, NavCommand::SetHeadingMode(_)));
    }

    // ─── Module-level functions: pre-init (None) behavior ─────────────────────
    //
    // The NAVIGATOR global starts as None. These tests verify that each public
    // function handles the uninitialized case gracefully.

    fn take_navigator() -> Option<Navigator> {
        NAVIGATOR
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    fn restore_navigator(nav: Option<Navigator>) {
        *NAVIGATOR
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = nav;
    }

    #[test]
    fn status_returns_idle_when_no_navigator() {
        let prev = take_navigator();
        let s = status();
        restore_navigator(prev);
        assert!(
            matches!(s, NavStatus::Idle),
            "expected Idle before init, got {s:?}"
        );
    }

    #[test]
    fn signals_returns_default_when_no_navigator() {
        let prev = take_navigator();
        let s = signals();
        restore_navigator(prev);
        assert_eq!(s, NavStateSignals::default());
    }

    #[test]
    fn diagnostics_returns_default_when_no_navigator() {
        let prev = take_navigator();
        let d = diagnostics();
        restore_navigator(prev);
        assert_eq!(d, NavDiagnostics::default());
    }

    #[test]
    fn tick_is_safe_with_no_navigator() {
        let prev = take_navigator();
        // Should not panic.
        tick(None, &[], None);
        restore_navigator(prev);
    }

    #[test]
    fn handle_command_stop_is_safe_with_no_navigator() {
        let prev = take_navigator();
        // Should not panic.
        handle_command(NavCommand::Stop);
        restore_navigator(prev);
    }

    #[test]
    fn update_follow_policy_is_safe_with_no_navigator() {
        let prev = take_navigator();
        // Closure should never be called — no panic.
        update_follow_policy(|config| {
            config.follow_distance = 10.0;
        });
        restore_navigator(prev);
    }
}
