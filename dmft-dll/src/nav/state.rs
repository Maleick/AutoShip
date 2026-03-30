//! Navigation state machine — runs once per game tick.
//! Transitions: Idle -> Moving -> Arrived -> Idle
//!
//! Stuck detection and recovery are handled inline by `StuckDetector`
//! rather than via a separate FSM state.

use crate::hooks::movement::{self, ARRIVAL_DISTANCE, MovementController};
// Distance methods are on Waypoint directly (e.g., a.distance_2d(&b)).
use dmft_common::nav::{CampSpot, NavStatus, Waypoint};

use super::humanize::MovementPersonality;
use super::stuck::StuckDetector;
use super::waypoint::WaypointQueue;

/// Internal state for the navigation FSM.
enum State {
    Idle,
    Moving,
    Arrived,
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
        self.state = State::Moving;
    }

    /// Move to a camp spot and face the specified heading.
    pub fn set_camp(&mut self, spot: CampSpot) {
        tracing::info!(role = %spot.role, "Setting camp spot");
        self.queue.set_path(vec![spot.position]);
        self.camp = Some(spot);
        self.stuck.reset();
        self.state = State::Moving;
    }

    /// Stop navigation immediately.
    pub fn stop(&mut self) {
        self.controller.stop_forward();
        self.queue.clear();
        self.camp = None;
        self.stuck.reset();
        self.state = State::Idle;
        tracing::info!("Navigation stopped");
    }

    /// Run one tick of the navigation state machine. Call from on_game_tick().
    pub fn tick(&mut self) {
        match self.state {
            State::Idle | State::Arrived => {}
            State::Moving => self.tick_moving(),
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
