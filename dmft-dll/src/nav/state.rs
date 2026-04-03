//! Navigation state machine — runs once per game tick.
//! Transitions: Idle -> Moving -> Arrived -> Idle
//! Stick mode: activated via `stick_to()`, runs until `stick_off()` or stop.
//!
//! Stuck detection and recovery are handled inline by `StuckDetector`
//! rather than via a separate FSM state.

use crate::hooks::movement::{self, ARRIVAL_DISTANCE, MovementController};
// Distance methods are on Waypoint directly (e.g., a.distance_2d(&b)).
use dmft_common::nav::{CampSpot, NavStatus, StickConfig, Waypoint};
use dmft_common::types::SpawnData;

use super::humanize::MovementPersonality;
use super::stick::StickEngine;
use super::stuck::StuckDetector;
use super::waypoint::WaypointQueue;

/// Internal state for the navigation FSM.
enum State {
    Idle,
    Moving,
    Arrived,
    Sticking,
}

/// The navigation engine, owned per-client in the DLL.
pub struct Navigator {
    state: State,
    queue: WaypointQueue,
    controller: MovementController,
    /// Camp spot to hold after arrival (optional).
    camp: Option<CampSpot>,
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
}

impl Navigator {
    pub fn new(player_base: usize, client_id: u32) -> Self {
        Self {
            state: State::Idle,
            queue: WaypointQueue::new(),
            controller: MovementController::new(player_base),
            camp: None,
            stuck: StuckDetector::new(),
            personality: MovementPersonality::from_client_id(client_id),
            cached_distance: 0.0,
            stick: StickEngine::new(),
            cached_stick_target_id: 0,
            cached_stick_distance: 0.0,
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
        self.stuck.reset();
        self.stick.stop();
        self.state = State::Moving;
    }

    /// Move to a camp spot and face the specified heading.
    pub fn set_camp(&mut self, spot: CampSpot) {
        tracing::info!(role = %spot.role, "Setting camp spot");
        self.queue.set_path(vec![spot.position]);
        self.camp = Some(spot);
        self.stuck.reset();
        self.stick.stop();
        self.state = State::Moving;
    }

    /// Stop navigation immediately.
    pub fn stop(&mut self) {
        self.controller.stop_forward();
        self.queue.clear();
        self.camp = None;
        self.stuck.reset();
        self.stick.stop();
        self.state = State::Idle;
        tracing::info!("Navigation stopped");
    }

    /// Begin a stick-to-target session.
    ///
    /// Cancels any in-progress waypoint navigation.
    /// `current_target_id` should be the spawn ID of the game's current target
    /// at the moment the command arrives (used for `hold` locking).
    pub fn stick_to(&mut self, config: StickConfig, current_target_id: Option<u32>) {
        self.controller.stop_forward();
        self.queue.clear();
        self.camp = None;
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
    pub fn tick(&mut self, current_target: Option<&SpawnData>, nearby: &[SpawnData]) {
        match self.state {
            State::Idle | State::Arrived => {}
            State::Moving => self.tick_moving(),
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
            State::Arrived => NavStatus::Arrived,
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
                // Keep cached values from last known contact.
                if !self.stick.is_active() {
                    self.state = State::Idle;
                }
            }
            StickTickResult::InRange { target_id, distance } => {
                self.cached_stick_target_id = target_id;
                self.cached_stick_distance = distance;
                self.controller.stop_forward();
            }
            StickTickResult::OutOfRange {
                target_id,
                distance,
                desired_pos,
            } => {
                self.cached_stick_target_id = target_id;
                self.cached_stick_distance = distance;
                // Face and move toward the desired stick position.
                let heading = movement::calc_heading(&player_pos, &desired_pos);
                let wobbled = self.personality.wobble_heading(heading);
                self.controller.write_heading(wobbled);
                self.controller.write_speed_heading(wobbled);
                self.controller.press_forward();
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
        self.state = State::Arrived;
    }
}
