//! Navigation state machine — runs once per game tick.
//! Transitions: Idle -> Moving -> Arrived -> Idle
//! Stick mode: activated via `stick_to()`, runs until `stick_off()` or stop.
//!
//! Follow mode (player anchor): Idle -> Following -> (navigating back when
//! leash exceeded) -> Following -> ...
//!
//! Stuck detection and recovery are handled inline by `StuckDetector`
//! rather than via a separate FSM state.

use crate::hooks::movement::{self, ARRIVAL_DISTANCE, MovementController};
// Distance methods are on Waypoint directly (e.g., a.distance_2d(&b)).
use textquest_common::nav::{
    CampSpot, FollowConfig, NavCampConfig, NavStatus, PauseReason, StickConfig, Waypoint,
};
use textquest_common::types::SpawnData;

use super::humanize::MovementPersonality;
use super::stick::StickEngine;
use super::stuck::StuckDetector;
use super::warp::{TargetSample, WarpAction, WarpMonitor};
use super::waypoint::WaypointQueue;

/// Internal state for the navigation FSM.
enum State {
    Idle,
    Moving,
    Paused(PauseReason),
    Arrived,
    /// Player follow mode: anchor tracks a leader's position.
    Following {
        /// Current follow configuration (leader name, distances).
        config: FollowConfig,
        /// Last-known anchor position (leader's position).
        anchor: Waypoint,
        /// Whether we are currently navigating back toward the anchor.
        returning: bool,
    },
    Sticking,
}

/// The navigation engine, owned per-client in the DLL.
pub struct Navigator {
    state: State,
    queue: WaypointQueue,
    controller: MovementController,
    /// Camp spot to hold after arrival (optional).
    camp: Option<CampSpot>,
    camp_config: Option<NavCampConfig>,
    /// Stuck detection and recovery.
    stuck: StuckDetector,
    /// Per-character movement personality for humanization.
    personality: MovementPersonality,
    /// Cached distance to current waypoint (updated each tick, read by status()).
    cached_distance: f32,
    /// Stick-to-target engine.
    stick: StickEngine,
    /// Cached stick reporting fields (updated each Sticking tick).
    cached_stick_target_id: u32,
    cached_stick_distance: f32,
    /// Warp detection + pause gate.
    warp: WarpMonitor,
}

impl Navigator {
    pub fn new(player_base: usize, client_id: u32) -> Self {
        Self {
            state: State::Idle,
            queue: WaypointQueue::new(),
            controller: MovementController::new(player_base),
            camp: None,
            camp_config: None,
            stuck: StuckDetector::new(),
            personality: MovementPersonality::from_client_id(client_id),
            cached_distance: 0.0,
            stick: StickEngine::new(),
            cached_stick_target_id: 0,
            cached_stick_distance: 0.0,
            warp: WarpMonitor::new(),
        }
    }

    /// Update the player base address (call after zoning or pointer refresh).
    pub fn set_player_base(&mut self, addr: usize) {
        self.controller.set_player_base(addr);
    }

    /// Start navigating a path of waypoints.
    pub fn navigate(&mut self, waypoints: Vec<Waypoint>) {
        tracing::info!(count = waypoints.len(), "Starting navigation");
        self.queue.set_path(waypoints);
        self.camp = None;
        self.camp_config = None;
        self.stuck.reset();
        self.stick.stop();
        self.warp.reset();
        self.state = State::Moving;
    }

    /// Move to a camp spot and face the specified heading.
    pub fn set_camp(&mut self, spot: CampSpot) {
        tracing::info!(role = %spot.role, "Setting camp spot");
        self.queue.set_path(vec![spot.position]);
        self.camp = Some(spot);
        self.stuck.reset();
        self.stick.stop();
        self.warp.reset();
        self.state = State::Moving;
    }

    /// Set a full camp config with scatter positioning.
    pub fn set_camp_config(&mut self, config: NavCampConfig) {
        tracing::info!(role = %config.role, radius = config.radius, scatter = config.scatter.is_some(), "Setting camp config");
        let spot = config.to_camp_spot();
        self.queue.set_path(vec![spot.position]);
        self.camp = Some(spot);
        self.camp_config = Some(config);
        self.stuck.reset();
        self.stick.stop();
        self.warp.reset();
        self.state = State::Moving;
    }

    /// Stop navigation immediately.
    pub fn stop(&mut self) {
        self.controller.stop_forward();
        self.queue.clear();
        self.camp = None;
        self.camp_config = None;
        self.stuck.reset();
        self.stick.stop();
        self.warp.reset();
        self.state = State::Idle;
        tracing::info!("Navigation stopped");
    }

    /// Start MQ2MoveUtils-style `/makecamp player` follow mode.
    pub fn follow_player(&mut self, config: FollowConfig, anchor: Waypoint) {
        tracing::info!(
            leader = %config.leader_name,
            follow_dist = config.follow_distance,
            leash_dist = config.leash_distance,
            "Starting player follow mode"
        );
        self.controller.stop_forward();
        self.queue.clear();
        self.camp = None;
        self.camp_config = None;
        self.stuck.reset();
        self.state = State::Following {
            config,
            anchor,
            returning: false,
        };
    }

    /// Update the dynamic anchor position in an active follow mode.
    pub fn update_follow_anchor(&mut self, new_anchor: Waypoint) {
        if let State::Following { ref mut anchor, .. } = self.state {
            *anchor = new_anchor;
        }
    }

    /// Stop player follow mode and return to idle.
    pub fn stop_follow(&mut self) {
        if matches!(self.state, State::Following { .. }) {
            self.controller.stop_forward();
            self.queue.clear();
            self.stuck.reset();
            self.state = State::Idle;
            tracing::info!("Player follow mode stopped");
        }
    }

    /// Begin a stick-to-target session.
    pub fn stick_to(&mut self, config: StickConfig, current_target_id: Option<u32>) {
        self.controller.stop_forward();
        self.queue.clear();
        self.camp = None;
        self.camp_config = None;
        self.stuck.reset();
        self.stick.start(config, current_target_id);
        self.state = State::Sticking;
    }

    /// Stop sticking and return to `Idle`.
    pub fn stick_off(&mut self) {
        self.controller.stop_forward();
        self.stick.stop();
        self.state = State::Idle;
        tracing::info!("Stick off — returning to Idle");
    }

    /// Apply a distance modifier delta to the active stick session (`/stick mod #`).
    pub fn stick_mod(&mut self, delta: f32) {
        self.stick.apply_mod(delta);
    }

    /// Run one tick of the navigation state machine. Call from on_game_tick().
    ///
    /// `current_target` and `nearby` are used by the stick engine.
    /// `target_sample` is used by the warp monitor.
    pub fn tick(
        &mut self,
        current_target: Option<&SpawnData>,
        nearby: &[SpawnData],
        target_sample: Option<&TargetSample>,
    ) {
        let warp_action = match self.state {
            State::Idle | State::Arrived => WarpAction::None,
            _ => self.warp.update(target_sample),
        };

        match warp_action {
            WarpAction::Pause => {
                self.controller.stop_forward();
                self.state = State::Paused(PauseReason::Warp);
                return;
            }
            WarpAction::Resume => {
                if matches!(self.state, State::Paused(_)) {
                    tracing::info!("Warp pause cleared — resuming navigation");
                    self.state = State::Moving;
                    self.stuck.reset();
                }
            }
            WarpAction::None => {}
        }

        match self.state {
            State::Idle => {}
            State::Arrived => self.tick_arrived(),
            State::Paused(_) => self.tick_paused(),
            State::Moving => self.tick_moving(),
            State::Following { .. } => self.tick_following(),
            State::Sticking => self.tick_sticking(current_target, nearby),
        }
    }

    /// Get current navigation status for IPC reporting.
    pub fn status(&self) -> NavStatus {
        match &self.state {
            State::Idle => NavStatus::Idle,
            State::Moving => {
                // If we're in the middle of recovery, report Stuck status.
                if self.stuck.recovery_attempt() > 0 {
                    return NavStatus::Stuck {
                        recovery_attempt: self.stuck.recovery_attempt(),
                    };
                }
                NavStatus::Moving {
                    waypoint_index: self.queue.index(),
                    waypoint_count: self.queue.len(),
                    distance_remaining: self.cached_distance,
                }
            }
            State::Paused(reason) => NavStatus::Paused {
                reason: reason.clone(),
                waypoint_index: self.queue.index(),
                waypoint_count: self.queue.len(),
                distance_remaining: self.cached_distance,
            },
            State::Arrived => NavStatus::Arrived,
            State::Following {
                config,
                anchor,
                returning,
            } => {
                let current_pos = self.controller.read_position();
                NavStatus::Following {
                    leader_name: config.leader_name.clone(),
                    distance_to_anchor: current_pos.distance_2d(anchor),
                    returning: *returning,
                }
            }
            State::Sticking => {
                let effective_dist = self.stick.effective_distance();
                NavStatus::Sticking {
                    target_id: self.cached_stick_target_id,
                    distance: self.cached_stick_distance,
                    in_range: self.cached_stick_distance
                        <= effective_dist + super::stick::STICK_ARRIVAL_THRESHOLD,
                }
            }
        }
    }

    fn tick_paused(&mut self) {
        // Update cached distance for UI/telemetry while paused.
        if let Some(target) = self.queue.current() {
            let dist = self.controller.read_position().distance_2d(target);
            self.cached_distance = dist;
        }
        self.controller.stop_forward();
    }

    fn tick_moving(&mut self) {
        let current_pos = self.controller.read_position();

        // Check if we've arrived at the current waypoint.
        if let Some(target) = self.queue.current() {
            let dist = current_pos.distance_2d(target);
            self.cached_distance = dist;

            if dist < ARRIVAL_DISTANCE {
                // Arrived at this waypoint — advance or finish.
                if self.queue.advance() {
                    tracing::debug!(index = self.queue.index(), "Advanced to next waypoint");
                    self.stuck.reset();
                } else {
                    self.on_path_complete();
                    return;
                }
            }
        } else {
            // No waypoint — shouldn't happen, go idle.
            self.state = State::Idle;
            return;
        }

        // Stuck detection: check if we moved enough since last tick.
        if self.stuck.check(&current_pos) {
            // We're stuck — attempt recovery.
            if !self.stuck.recover(&self.controller) {
                // Recovery exhausted, stop navigation.
                self.stop();
                return;
            }
            // Recovery applied a turn; let the next tick try moving again.
            return;
        }

        // Face and move toward the current waypoint.
        if let Some(target) = self.queue.current() {
            let heading = movement::calc_heading(&current_pos, target);
            let wobbled = self.personality.wobble_heading(heading);
            self.controller.write_heading(wobbled);
            self.controller.write_speed_heading(wobbled);
            // Actually walk forward via ExecuteCmd.
            self.controller.press_forward();
        }
    }

    fn tick_sticking(&mut self, current_target: Option<&SpawnData>, nearby: &[SpawnData]) {
        use super::stick::StickTickResult;

        let player_pos = self.controller.read_position();
        let result = self.stick.tick(&player_pos, current_target, nearby);

        match result {
            StickTickResult::Inactive => {
                // Engine deactivated externally — go idle.
                self.state = State::Idle;
            }
            StickTickResult::TargetLost => {
                // `always` mode: stay armed, stop movement.
                self.controller.stop_forward();
                self.controller.stop_back();
                // Keep cached values from last known contact.
                if !self.stick.is_active() {
                    self.state = State::Idle;
                }
            }
            StickTickResult::InRange {
                target_id,
                distance,
            } => {
                self.cached_stick_target_id = target_id;
                self.cached_stick_distance = distance;
                self.controller.stop_forward();
                self.controller.stop_back();
            }
            StickTickResult::OutOfRange {
                target_id,
                distance,
                desired_pos,
            } => {
                self.cached_stick_target_id = target_id;
                self.cached_stick_distance = distance;
                // Ensure backward key is released before moving forward.
                self.controller.stop_back();
                // Face and move toward the desired stick position.
                let heading = movement::calc_heading(&player_pos, &desired_pos);
                let wobbled = self.personality.wobble_heading(heading);
                self.controller.write_heading(wobbled);
                self.controller.write_speed_heading(wobbled);
                self.controller.press_forward();
            }
            StickTickResult::TooClose {
                target_id,
                distance,
                retreat_pos,
            } => {
                self.cached_stick_target_id = target_id;
                self.cached_stick_distance = distance;
                // Face the retreat point and walk backward (away from target).
                // We set heading toward the retreat pos so the character faces
                // away from the mob, then press backward to move in that direction.
                self.controller.stop_forward();
                let heading = movement::calc_heading(&player_pos, &retreat_pos);
                let wobbled = self.personality.wobble_heading(heading);
                self.controller.write_heading(wobbled);
                self.controller.write_speed_heading(wobbled);
                self.controller.press_back();
            }
        }
    }

    fn on_path_complete(&mut self) {
        self.controller.stop_forward();
        if let Some(ref camp) = self.camp {
            self.controller.write_heading(camp.heading);
            tracing::info!(role = %camp.role, "Arrived at camp spot");
        } else {
            tracing::info!("Navigation path complete");
        }
        self.stuck.reset();
        self.warp.reset();
        self.state = State::Arrived;
    }

    /// One tick while at camp — checks if character has drifted beyond the
    /// leash boundary and triggers a return if so.
    fn tick_arrived(&mut self) {
        if let Some(ref config) = self.camp_config {
            let current_pos = self.controller.read_position();
            if config.is_beyond_leash(&current_pos) {
                let return_pos = config.return_position();
                tracing::debug!(role = %config.role, "Drifted beyond camp leash — returning");
                self.queue.set_path(vec![return_pos]);
                self.stuck.reset();
                self.state = State::Moving;
            }
        }
    }

    /// One tick for player follow mode.
    ///
    /// Implements the leash/return logic:
    /// - If distance to anchor > `leash_distance`: start (or continue) navigating back.
    /// - If currently returning and distance <= `follow_distance`: stop, hold position.
    fn tick_following(&mut self) {
        // Extract values without holding a mutable borrow on self.state.
        let (leash_distance, follow_distance, anchor, currently_returning) =
            if let State::Following {
                ref config,
                ref anchor,
                returning,
            } = self.state
            {
                (
                    config.leash_distance,
                    config.follow_distance,
                    *anchor,
                    returning,
                )
            } else {
                return;
            };

        let current_pos = self.controller.read_position();
        let dist = current_pos.distance_2d(&anchor);
        self.cached_distance = dist;

        if dist > leash_distance {
            // Beyond the leash — navigate back toward the anchor.
            if !currently_returning {
                tracing::debug!(
                    dist,
                    leash = leash_distance,
                    "Follow leash exceeded — returning to anchor"
                );
                if let State::Following {
                    ref mut returning, ..
                } = self.state
                {
                    *returning = true;
                }
                self.stuck.reset();
            }

            // Stuck detection while returning.
            if self.stuck.check(&current_pos) {
                if !self.stuck.recover(&self.controller) {
                    // Give up stuck recovery but stay in follow mode — the anchor
                    // may move closer on its own.
                    self.stuck.reset();
                }
                return;
            }

            // Face and step toward anchor.
            let heading = movement::calc_heading(&current_pos, &anchor);
            let wobbled = self.personality.wobble_heading(heading);
            self.controller.write_heading(wobbled);
            self.controller.write_speed_heading(wobbled);
            self.controller.press_forward();
        } else if currently_returning && dist <= follow_distance {
            // Arrived back within follow range — stop moving.
            tracing::debug!(dist, follow = follow_distance, "Returned to follow range");
            self.controller.stop_forward();
            self.stuck.reset();
            if let State::Following {
                ref mut returning, ..
            } = self.state
            {
                *returning = false;
            }
        } else {
            // Within acceptable range — ensure we're not still moving.
            self.controller.stop_forward();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pauses_and_resumes_on_warp() {
        let mut nav = Navigator::new(0, 1);
        nav.navigate(vec![Waypoint::new(100.0, 0.0, 0.0)]);

        let stable = TargetSample {
            id: 99,
            position: Waypoint::new(0.0, 0.0, 0.0),
        };
        nav.tick(None, &[], Some(&stable));
        assert!(matches!(nav.status(), NavStatus::Moving { .. }));

        let warped = TargetSample {
            id: 99,
            position: Waypoint::new(200.0, 0.0, 0.0),
        };
        nav.tick(None, &[], Some(&warped));
        assert!(matches!(nav.status(), NavStatus::Paused { .. }));

        // Feed stable samples to clear the pause
        let stable_again = TargetSample {
            id: 99,
            position: Waypoint::new(200.5, 0.5, 0.0),
        };
        for _ in 0..15 {
            nav.tick(None, &[], Some(&stable_again));
        }
        assert!(matches!(nav.status(), NavStatus::Moving { .. }));
    }
}
