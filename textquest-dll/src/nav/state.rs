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
    CampSpot, FollowConfig, MoveToConfig, NavCampConfig, NavDiagnostics, NavStateSignals,
    NavStatus, PauseReason, StickConfig, Waypoint,
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
    /// Advanced moveto — tracking a destination with break conditions (#184).
    MovingTo,
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
    /// State before user-initiated pause (for resume).
    pre_pause_state: Option<State>,
    /// Previous position for velocity calculation.
    prev_position: Option<Waypoint>,
    /// Cached velocity in world units per tick (updated each tick).
    cached_velocity: f32,
    /// Whether a navmesh is loaded (set externally via `set_mesh_loaded`).
    mesh_loaded: bool,
    /// Active moveto configuration (#184).
    moveto_config: Option<MoveToConfig>,
    /// Last observed HP while running moveto break-on-hit checks.
    last_moveto_hp: Option<i64>,
    /// Global autopause flag (#164).
    autopause: bool,
    /// Break-on-GM flag — pause navigation when a GM is detected nearby.
    break_on_gm: bool,
}

/// Radius for hostile NPC proximity checks (aggro detection), in EQ world units.
const AGGRO_CHECK_RADIUS: f32 = 50.0;
/// Distance delta that counts as an unexpected player displacement (e.g. summon).
const SUMMON_DISTANCE_THRESHOLD: f32 = 60.0;

/// Radius for GM proximity check (break-on-GM detection), in EQ world units.
const GM_CHECK_RADIUS: f32 = 500.0;

/// Returns true if any hostile NPC (type=1, moving) is within aggro radius.
fn has_hostile_nearby(nearby: &[SpawnData], pos: &Waypoint) -> bool {
    nearby.iter().any(|s| {
        s.spawn_type == 1 && s.speed_run > 0.0 && {
            let sp = Waypoint::new(s.x, s.y, s.z);
            pos.distance_2d(&sp) < AGGRO_CHECK_RADIUS
        }
    })
}

/// Returns true if any GM-flagged spawn is within the GM check radius.
fn has_gm_nearby(nearby: &[SpawnData], pos: &Waypoint) -> bool {
    nearby.iter().any(|s| {
        s.is_gm && {
            let sp = Waypoint::new(s.x, s.y, s.z);
            pos.distance_2d(&sp) < GM_CHECK_RADIUS
        }
    })
}

fn break_on_hit_triggered(last_hp_current: &mut Option<i64>, current_hp: Option<i64>) -> bool {
    let Some(current_hp) = current_hp else {
        return false;
    };
    let took_damage = last_hp_current.is_some_and(|previous_hp| current_hp < previous_hp);
    *last_hp_current = Some(current_hp);
    took_damage
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
            pre_pause_state: None,
            prev_position: None,
            cached_velocity: 0.0,
            mesh_loaded: false,
            moveto_config: None,
            last_moveto_hp: None,
            autopause: false,
            break_on_gm: false,
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
        self.controller.stop_back();
        self.queue.clear();
        self.camp = None;
        self.camp_config = None;
        self.stuck.reset();
        self.stick.stop();
        self.warp.reset();
        self.moveto_config = None;
        self.last_moveto_hp = None;
        self.pre_pause_state = None;
        self.state = State::Idle;
        tracing::info!("Navigation stopped");
    }

    /// Pause navigation, retaining path and state for later resume (#168).
    pub fn pause(&mut self) {
        match self.state {
            State::Moving | State::Following { .. } | State::Sticking => {
                self.controller.stop_forward();
                self.controller.stop_back();
                let old_state = std::mem::replace(&mut self.state, State::Idle);
                self.pre_pause_state = Some(old_state);
                self.state = State::Paused(PauseReason::UserPause);
                tracing::info!("Navigation paused by user");
            }
            _ => {
                tracing::debug!("Pause requested but not in a pauseable state");
            }
        }
    }

    /// Resume navigation from a user-initiated pause (#168).
    pub fn resume(&mut self) {
        if matches!(self.state, State::Paused(PauseReason::UserPause)) {
            if let Some(saved) = self.pre_pause_state.take() {
                self.state = saved;
                self.stuck.reset();
                tracing::info!("Navigation resumed by user");
            } else {
                self.state = State::Idle;
                tracing::warn!("Resume called but no pre-pause state saved");
            }
        } else {
            tracing::debug!("Resume requested but not in user-paused state");
        }
    }

    /// Set whether a navmesh is loaded for the current zone (#174).
    pub fn set_mesh_loaded(&mut self, loaded: bool) {
        self.mesh_loaded = loaded;
    }

    /// Get navigation state signals for TLO-style queries (#176).
    pub fn signals(&self) -> NavStateSignals {
        NavStateSignals {
            active: matches!(
                self.state,
                State::Moving | State::Following { .. } | State::Sticking
            ),
            mesh_loaded: self.mesh_loaded,
            path_exists: !self.queue.is_empty(),
            path_length: if self.queue.is_empty() {
                None
            } else {
                Some(self.cached_distance)
            },
            velocity: self.cached_velocity,
            paused: matches!(self.state, State::Paused(_)),
        }
    }

    /// Get navigation diagnostics snapshot (#177).
    pub fn diagnostics(&self) -> NavDiagnostics {
        NavDiagnostics {
            state: self.status().label().to_string(),
            mesh_loaded: self.mesh_loaded,
            path_exists: !self.queue.is_empty(),
            path_length: if self.queue.is_empty() {
                None
            } else {
                Some(self.cached_distance)
            },
            velocity: self.cached_velocity,
            waypoint_index: self.queue.index(),
            waypoint_count: self.queue.len(),
            distance_remaining: self.cached_distance,
        }
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
    /// Start an advanced moveto session (#184).
    pub fn move_to_advanced(&mut self, config: MoveToConfig) {
        tracing::info!(
            target_id = config.target_id,
            use_walk = config.use_walk,
            use_back = config.use_back,
            break_on_aggro = config.break_on_aggro,
            break_on_hit = config.break_on_hit,
            "Starting advanced moveto"
        );
        self.controller.stop_forward();
        self.queue.clear();
        self.camp = None;
        self.camp_config = None;
        self.stuck.reset();
        self.stick.stop();
        self.warp.reset();
        self.last_moveto_hp = self.controller.read_hp_current();
        self.moveto_config = Some(config);
        self.state = State::MovingTo;
    }

    /// Enable or disable autopause globally (#164).
    pub fn set_autopause(&mut self, enabled: bool) {
        self.autopause = enabled;
        tracing::info!(enabled, "Autopause set");
    }

    /// Enable or disable break-on-GM safety halt.
    ///
    /// When enabled, navigation pauses (path retained) whenever a GM-flagged
    /// spawn is detected within GM_CHECK_RADIUS, mirroring MQ2MoveUtils breakongm.
    pub fn set_break_on_gm(&mut self, enabled: bool) {
        self.break_on_gm = enabled;
        tracing::info!(enabled, "BreakOnGm set");
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
        let summon_displacement = if self.should_break_on_summon() {
            self.current_tick_displacement()
        } else {
            None
        };

        // Only update velocity when actively navigating (skip Idle/Arrived).
        if !matches!(self.state, State::Idle | State::Arrived) {
            self.update_velocity();
        }

        if summon_displacement.is_some_and(|distance| distance > SUMMON_DISTANCE_THRESHOLD) {
            match self.state {
                State::Following { .. } => {
                    self.disengage_follow(
                        "Follow disengaged — summon/warp guard triggered on local player",
                    );
                }
                State::Sticking => {
                    self.disengage_stick(
                        "Stick disengaged — summon/warp guard triggered on local player",
                    );
                }
                _ => {
                    tracing::info!(
                        displacement = summon_displacement,
                        "MoveToAdvanced: break_on_summon triggered"
                    );
                    self.stop_moveto();
                }
            }
            return;
        }

        let moveto_target_sample = self.moveto_target_sample(nearby, target_sample);
        let warp_action = if self.should_track_moveto_warp() {
            self.warp.update(moveto_target_sample.as_ref())
        } else {
            match self.state {
                State::Idle | State::Arrived | State::MovingTo => WarpAction::None,
                State::Following { ref config, .. } => self
                    .warp
                    .update(find_follow_target_sample(nearby, &config.leader_name).as_ref()),
                State::Sticking => {
                    let stick_sample = self.stick.target_sample(current_target, nearby);
                    self.warp.update(target_sample.or(stick_sample.as_ref()))
                }
                _ => self.warp.update(target_sample),
            }
        };

        match warp_action {
            WarpAction::Pause => {
                if matches!(self.state, State::Following { .. }) {
                    self.disengage_follow("Follow disengaged — warp guard triggered");
                    return;
                }
                if matches!(self.state, State::Sticking) {
                    self.disengage_stick("Stick disengaged — warp guard triggered");
                    return;
                }
                if self.should_break_moveto_on_warp() {
                    tracing::info!("MoveToAdvanced: break_on_warp triggered");
                    self.stop_moveto();
                    return;
                }
                if matches!(
                    self.state,
                    State::Paused(PauseReason::UserPause | PauseReason::UserInput)
                ) {
                    return;
                }
                self.controller.stop_forward();
                self.controller.stop_back();
                let old_state =
                    std::mem::replace(&mut self.state, State::Paused(PauseReason::Warp));
                self.pre_pause_state = Some(old_state);
                return;
            }
            WarpAction::Resume => {
                // Only auto-resume from warp pauses, not user-initiated pauses.
                if matches!(self.state, State::Paused(PauseReason::Warp)) {
                    tracing::info!("Warp pause cleared — resuming navigation");
                    self.state = self.pre_pause_state.take().unwrap_or(State::Moving);
                    self.stuck.reset();
                }
            }
            WarpAction::None => {}
        }

        // Break-on-GM: pause all movement when a GM-flagged spawn is detected nearby.
        if self.break_on_gm {
            let current_pos = self.controller.read_position();
            let gm_present = has_gm_nearby(nearby, &current_pos);

            if gm_present
                && matches!(
                    self.state,
                    State::Moving | State::Following { .. } | State::Sticking | State::MovingTo
                )
            {
                tracing::warn!("GM detected nearby — pausing navigation (break_on_gm)");
                self.controller.stop_forward();
                self.controller.stop_back();
                let old_state = std::mem::replace(&mut self.state, State::Idle);
                self.pre_pause_state = Some(old_state);
                self.state = State::Paused(PauseReason::GmNearby);
                return;
            }

            // Auto-resume from a GM pause once the GM is no longer nearby.
            if !gm_present && matches!(self.state, State::Paused(PauseReason::GmNearby)) {
                tracing::info!("GM no longer nearby — resuming navigation");
                if let Some(saved) = self.pre_pause_state.take() {
                    self.state = saved;
                } else {
                    self.state = State::Moving;
                }
                self.stuck.reset();
            }
        }

        match self.state {
            State::Idle => {}
            State::Arrived => self.tick_arrived(nearby),
            State::Paused(_) => self.tick_paused(),
            State::Moving => self.tick_moving(),
            State::Following { .. } => self.tick_following(nearby),
            State::Sticking => self.tick_sticking(current_target, nearby),
            State::MovingTo => self.tick_moveto(nearby),
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
            State::MovingTo => NavStatus::Moving {
                waypoint_index: 0,
                waypoint_count: 1,
                distance_remaining: self.cached_distance,
            },
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
                self.controller.stop_forward();
                self.controller.stop_back();
                if self.stick.keep_armed_on_target_loss() {
                    tracing::debug!("Stick target lost — waiting for next valid target");
                } else {
                    self.disengage_stick("Stick disengaged — target lost");
                }
            }
            StickTickResult::InRange {
                target_id,
                distance,
                face_target,
            } => {
                self.cached_stick_target_id = target_id;
                self.cached_stick_distance = distance;
                self.controller.stop_forward();
                self.controller.stop_back();
                // Healer mode: face target even when in range (for casting).
                if let Some(ref ft) = face_target {
                    let heading = movement::calc_heading(&player_pos, ft);
                    self.controller.write_heading(heading);
                }
            }
            StickTickResult::OutOfRange {
                target_id,
                distance,
                desired_pos,
                face_target,
            } => {
                self.cached_stick_target_id = target_id;
                self.cached_stick_distance = distance;
                // Ensure backward key is released before moving forward.
                self.controller.stop_back();
                // Healer mode: face target for casting, otherwise face movement direction.
                let heading = if let Some(ref ft) = face_target {
                    movement::calc_heading(&player_pos, ft)
                } else {
                    let h = movement::calc_heading(&player_pos, &desired_pos);
                    self.personality.wobble_heading(h)
                };
                self.controller.write_heading(heading);
                self.controller.write_speed_heading(heading);
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
    fn tick_arrived(&mut self, nearby: &[SpawnData]) {
        if let Some(ref config) = self.camp_config {
            let current_pos = self.controller.read_position();
            if config.is_beyond_leash(&current_pos) {
                // #182: return_no_aggro — don't return if hostile NPCs are nearby.
                if config.return_no_aggro && has_hostile_nearby(nearby, &current_pos) {
                    return;
                }
                let return_pos = config.return_position();
                tracing::debug!(role = %config.role, "Drifted beyond camp leash — returning");
                self.queue.set_path(vec![return_pos]);
                self.stuck.reset();
                self.state = State::Moving;
            }
        }
    }

    /// One tick for advanced moveto (#184).
    fn tick_moveto(&mut self, nearby: &[SpawnData]) {
        let config = match self.moveto_config {
            Some(ref c) => c,
            None => {
                self.state = State::Idle;
                return;
            }
        };

        let current_pos = self.controller.read_position();

        // Update destination from tracked spawn if target_id is set.
        let destination = if let Some(tid) = config.target_id {
            if let Some(s) = nearby.iter().find(|s| s.spawn_id == tid) {
                Waypoint::new(s.x, s.y, s.z)
            } else {
                config.destination
            }
        } else {
            config.destination
        };

        let dist = current_pos.distance_2d(&destination);
        self.cached_distance = dist;

        if config.break_on_hit
            && break_on_hit_triggered(&mut self.last_moveto_hp, self.controller.read_hp_current())
        {
            tracing::info!("MoveToAdvanced: break_on_hit triggered");
            self.stop_moveto();
            return;
        }

        // Break-on-aggro: hostile NPC moving toward player within aggro radius.
        if config.break_on_aggro && has_hostile_nearby(nearby, &current_pos) {
            tracing::info!("MoveToAdvanced: break_on_aggro triggered");
            self.stop_moveto();
            return;
        }

        // Arrival check.
        if dist < ARRIVAL_DISTANCE {
            tracing::info!("MoveToAdvanced: arrived at destination");
            self.stop_moveto();
            return;
        }

        // Stuck detection.
        if self.stuck.check(&current_pos) {
            if !self.stuck.recover(&self.controller) {
                tracing::warn!("MoveToAdvanced: stuck recovery exhausted");
                self.stop_moveto();
                return;
            }
            return;
        }

        // Move toward destination.
        let heading = movement::calc_heading(&current_pos, &destination);
        let wobbled = self.personality.wobble_heading(heading);

        if config.use_back {
            // Move backward: face away from destination, press back.
            let reverse = (wobbled + 256.0) % 512.0;
            self.controller.write_heading(reverse);
            self.controller.write_speed_heading(reverse);
            self.controller.stop_forward();
            self.controller.press_back();
        } else {
            self.controller.write_heading(wobbled);
            self.controller.write_speed_heading(wobbled);
            self.controller.stop_back();
            self.controller.press_forward();
        }
    }

    /// Stop an active moveto and return to idle.
    fn stop_moveto(&mut self) {
        self.controller.stop_forward();
        self.controller.stop_back();
        self.moveto_config = None;
        self.last_moveto_hp = None;
        self.stuck.reset();
        self.warp.reset();
        self.pre_pause_state = None;
        self.state = State::Idle;
    }

    fn should_track_moveto_warp(&self) -> bool {
        match self.state {
            State::MovingTo => {
                self.should_break_moveto_on_warp() || self.should_pause_moveto_on_warp()
            }
            State::Paused(PauseReason::Warp) => {
                matches!(self.pre_pause_state, Some(State::MovingTo))
                    && (self.should_break_moveto_on_warp() || self.should_pause_moveto_on_warp())
            }
            _ => false,
        }
    }

    fn should_break_moveto_on_warp(&self) -> bool {
        self.moveto_config
            .as_ref()
            .is_some_and(|config| config.break_on_warp)
    }

    fn should_pause_moveto_on_warp(&self) -> bool {
        self.moveto_config
            .as_ref()
            .is_some_and(|config| config.pause_on_warp)
    }

    fn should_break_on_summon(&self) -> bool {
        match self.state {
            State::Following { .. } | State::Sticking => true,
            State::MovingTo => self
                .moveto_config
                .as_ref()
                .is_some_and(|config| config.break_on_summon),
            State::Paused(PauseReason::Warp) => {
                matches!(
                    self.pre_pause_state,
                    Some(State::Following { .. } | State::Sticking)
                ) || (matches!(self.pre_pause_state, Some(State::MovingTo))
                    && self
                        .moveto_config
                        .as_ref()
                        .is_some_and(|config| config.break_on_summon))
            }
            _ => false,
        }
    }

    fn moveto_target_sample(
        &self,
        nearby: &[SpawnData],
        fallback: Option<&TargetSample>,
    ) -> Option<TargetSample> {
        let target_id = self.moveto_config.as_ref()?.target_id?;
        if let Some(spawn) = nearby.iter().find(|spawn| spawn.spawn_id == target_id) {
            return Some(TargetSample {
                id: spawn.spawn_id,
                position: Waypoint::new(spawn.x, spawn.y, spawn.z),
            });
        }

        fallback
            .filter(|sample| sample.id == target_id)
            .map(|sample| TargetSample {
                id: sample.id,
                position: sample.position,
            })
    }

    fn current_tick_displacement(&self) -> Option<f32> {
        self.prev_position
            .map(|previous| self.controller.read_position().distance_2d(&previous))
    }

    /// One tick for player follow mode.
    ///
    /// Implements the leash/return logic:
    /// - If distance to anchor > `leash_distance`: start (or continue) navigating back.
    /// - If currently returning and distance <= `follow_distance`: stop, hold position.
    fn tick_following(&mut self, nearby: &[SpawnData]) {
        if let State::Following {
            ref config,
            ref mut anchor,
            ..
        } = self.state
            && let Some(sample) = find_follow_target_sample(nearby, &config.leader_name)
        {
            *anchor = sample.position;
        }

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

    fn disengage_follow(&mut self, message: &str) {
        self.controller.stop_forward();
        self.controller.stop_back();
        self.queue.clear();
        self.stuck.reset();
        self.warp.reset();
        self.pre_pause_state = None;
        self.state = State::Idle;
        tracing::warn!(%message);
        crate::ipc::send_response(textquest_common::ipc::Response::CommandResult {
            success: false,
            message: message.to_string(),
        });
    }

    fn disengage_stick(&mut self, message: &str) {
        self.controller.stop_forward();
        self.controller.stop_back();
        self.stick.stop();
        self.warp.reset();
        self.pre_pause_state = None;
        self.state = State::Idle;
        tracing::warn!(%message);
        crate::ipc::send_response(textquest_common::ipc::Response::CommandResult {
            success: false,
            message: message.to_string(),
        });
    }

    /// Update velocity cache based on position delta.
    fn update_velocity(&mut self) {
        let current_pos = self.controller.read_position();
        if let Some(prev) = self.prev_position {
            self.cached_velocity = current_pos.distance_2d(&prev);
        }
        self.prev_position = Some(current_pos);
    }
}

fn find_follow_target_sample(nearby: &[SpawnData], leader_name: &str) -> Option<TargetSample> {
    nearby
        .iter()
        .find(|spawn| {
            spawn.displayed_name.eq_ignore_ascii_case(leader_name)
                || spawn.name.eq_ignore_ascii_case(leader_name)
        })
        .map(|spawn| TargetSample {
            id: spawn.spawn_id,
            position: Waypoint::new(spawn.x, spawn.y, spawn.z),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::nav::StickConfig;

    fn follow_spawn(id: u32, name: &str, x: f32, y: f32) -> SpawnData {
        SpawnData {
            spawn_id: id,
            name: name.to_string(),
            displayed_name: name.to_string(),
            x,
            y,
            ..SpawnData::default()
        }
    }

    #[test]
    fn break_on_hit_triggers_on_hp_drop() {
        let mut last_hp_current = Some(100);
        assert!(break_on_hit_triggered(&mut last_hp_current, Some(90)));
        assert_eq!(last_hp_current, Some(90));
    }

    #[test]
    fn break_on_hit_ignores_stable_or_rising_hp() {
        let mut last_hp_current = Some(100);
        assert!(!break_on_hit_triggered(&mut last_hp_current, Some(100)));
        assert_eq!(last_hp_current, Some(100));

        assert!(!break_on_hit_triggered(&mut last_hp_current, Some(110)));
        assert_eq!(last_hp_current, Some(110));
    }

    #[test]
    fn break_on_hit_ignores_missing_hp_sample() {
        let mut last_hp_current = Some(100);
        assert!(!break_on_hit_triggered(&mut last_hp_current, None));
        assert_eq!(last_hp_current, Some(100));
    }

    #[test]
    fn break_on_hit_records_initial_sample_without_triggering() {
        let mut last_hp_current = None;
        assert!(!break_on_hit_triggered(&mut last_hp_current, Some(100)));
        assert_eq!(last_hp_current, Some(100));
    }

    #[test]
    fn stick_target_loss_disengages_without_always() {
        let mut nav = Navigator::new(0, 1);
        nav.stick_to(StickConfig::default(), Some(42));

        nav.tick(None, &[], None);

        assert!(matches!(nav.status(), NavStatus::Idle));
    }

    #[test]
    fn stick_target_loss_stays_active_with_always() {
        let mut nav = Navigator::new(0, 1);
        let config = StickConfig {
            always: true,
            ..StickConfig::default()
        };
        nav.stick_to(config, Some(42));

        nav.tick(None, &[], None);

        assert!(matches!(nav.status(), NavStatus::Sticking { .. }));
    }

    #[test]
    fn follow_updates_anchor_from_matching_spawn() {
        let mut nav = Navigator::new(0, 1);
        nav.follow_player(
            FollowConfig::new("Leader", 20.0, 60.0),
            Waypoint::new(0.0, 0.0, 0.0),
        );

        nav.tick(None, &[follow_spawn(7, "Leader", 25.0, 0.0)], None);

        match nav.status() {
            NavStatus::Following {
                distance_to_anchor, ..
            } => assert!((distance_to_anchor - 25.0).abs() < f32::EPSILON),
            other => panic!("expected Following status, got {other:?}"),
        }
    }

    #[test]
    fn stick_disengages_on_warp_guard() {
        let mut nav = Navigator::new(0, 1);
        let target = follow_spawn(9, "a_mob", 10.0, 0.0);
        nav.stick_to(StickConfig::default(), Some(target.spawn_id));

        let stable = TargetSample {
            id: target.spawn_id,
            position: Waypoint::new(10.0, 0.0, 0.0),
        };
        nav.tick(Some(&target), std::slice::from_ref(&target), Some(&stable));
        assert!(matches!(nav.status(), NavStatus::Sticking { .. }));

        let warped_target = follow_spawn(9, "a_mob", 200.0, 0.0);
        let warped = TargetSample {
            id: warped_target.spawn_id,
            position: Waypoint::new(200.0, 0.0, 0.0),
        };
        nav.tick(
            Some(&warped_target),
            std::slice::from_ref(&warped_target),
            Some(&warped),
        );
        assert!(matches!(nav.status(), NavStatus::Idle));
    }

    #[test]
    fn follow_disengages_on_summon_guard() {
        let mut nav = Navigator::new(0, 1);
        nav.follow_player(
            FollowConfig::new("Leader", 20.0, 60.0),
            Waypoint::new(0.0, 0.0, 0.0),
        );
        nav.prev_position = Some(Waypoint::new(100.0, 0.0, 0.0));
        nav.tick(None, &[], None);

        assert!(matches!(nav.status(), NavStatus::Idle));
    }

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

    #[test]
    fn user_pause_and_resume() {
        let mut nav = Navigator::new(0, 1);
        nav.navigate(vec![Waypoint::new(100.0, 0.0, 0.0)]);
        assert!(matches!(nav.status(), NavStatus::Moving { .. }));

        nav.pause();
        assert!(matches!(
            nav.status(),
            NavStatus::Paused {
                reason: PauseReason::UserPause,
                ..
            }
        ));

        nav.resume();
        assert!(matches!(nav.status(), NavStatus::Moving { .. }));
    }

    #[test]
    fn pause_idle_is_noop() {
        let mut nav = Navigator::new(0, 1);
        assert!(matches!(nav.status(), NavStatus::Idle));
        nav.pause();
        assert!(matches!(nav.status(), NavStatus::Idle));
    }

    #[test]
    fn resume_without_pause_is_noop() {
        let mut nav = Navigator::new(0, 1);
        nav.navigate(vec![Waypoint::new(100.0, 0.0, 0.0)]);
        nav.resume(); // not paused
        assert!(matches!(nav.status(), NavStatus::Moving { .. }));
    }

    #[test]
    fn stop_clears_pause_state() {
        let mut nav = Navigator::new(0, 1);
        nav.navigate(vec![Waypoint::new(100.0, 0.0, 0.0)]);
        nav.pause();
        nav.stop();
        assert!(matches!(nav.status(), NavStatus::Idle));
        nav.resume(); // should be noop now
        assert!(matches!(nav.status(), NavStatus::Idle));
    }

    #[test]
    fn warp_resume_does_not_override_user_pause() {
        let mut nav = Navigator::new(0, 1);
        nav.navigate(vec![Waypoint::new(100.0, 0.0, 0.0)]);
        nav.pause();

        // Feed a warp sequence — should NOT affect user pause.
        let warped = TargetSample {
            id: 99,
            position: Waypoint::new(200.0, 0.0, 0.0),
        };
        nav.tick(None, &[], Some(&warped));
        // Still user-paused, not warp-paused.
        assert!(matches!(
            nav.status(),
            NavStatus::Paused {
                reason: PauseReason::UserPause,
                ..
            }
        ));
    }

    #[test]
    fn signals_reflect_state() {
        let mut nav = Navigator::new(0, 1);
        let sig = nav.signals();
        assert!(!sig.active);
        assert!(!sig.paused);
        assert!(!sig.path_exists);

        nav.navigate(vec![Waypoint::new(100.0, 0.0, 0.0)]);
        let sig = nav.signals();
        assert!(sig.active);
        assert!(sig.path_exists);
        assert!(!sig.paused);
    }

    #[test]
    fn diagnostics_reflect_state() {
        let mut nav = Navigator::new(0, 1);
        let diag = nav.diagnostics();
        assert_eq!(diag.state, "Idle");
        assert_eq!(diag.waypoint_count, 0);

        nav.navigate(vec![
            Waypoint::new(10.0, 0.0, 0.0),
            Waypoint::new(20.0, 0.0, 0.0),
        ]);
        let diag = nav.diagnostics();
        assert_eq!(diag.state, "Navigating");
        assert_eq!(diag.waypoint_count, 2);
    }

    #[test]
    fn mesh_loaded_flag() {
        let mut nav = Navigator::new(0, 1);
        assert!(!nav.signals().mesh_loaded);
        nav.set_mesh_loaded(true);
        assert!(nav.signals().mesh_loaded);
    }

    /// Build a nearby-spawn list with a single GM spawn at the given position.
    fn gm_spawn_at(x: f32, y: f32) -> SpawnData {
        SpawnData {
            spawn_id: 9999,
            name: "GM_Zordak".into(),
            displayed_name: "GM Zordak".into(),
            spawn_type: 0, // player
            level: 255,
            class_id: 0,
            x,
            y,
            z: 0.0,
            heading: 0.0,
            hp_current: 100,
            hp_max: 100,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 0,
            endurance_max: 0,
            speed_run: 0.0,
            stand_state: 0,
            is_gm: true,
        }
    }

    fn hostile_spawn_at(x: f32, y: f32) -> SpawnData {
        SpawnData {
            spawn_id: 1111,
            name: "a_goblin".into(),
            displayed_name: "a goblin".into(),
            spawn_type: 1,
            level: 10,
            class_id: 0,
            x,
            y,
            z: 0.0,
            heading: 0.0,
            hp_current: 100,
            hp_max: 100,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 0,
            endurance_max: 0,
            speed_run: 1.0,
            stand_state: 0,
            is_gm: false,
        }
    }

    fn player_hp_storage(hp: i64) -> Vec<u64> {
        let word_count = (textquest_common::offsets::player_zone::HP_CURRENT
            + std::mem::size_of::<i64>())
            / std::mem::size_of::<u64>();
        let mut storage = vec![0u64; word_count];
        write_player_hp(&mut storage, hp);
        storage
    }

    fn write_player_hp(storage: &mut [u64], hp: i64) {
        let base = storage.as_mut_ptr() as usize;
        // SAFETY: the backing buffer is sized so the HP_CURRENT offset lands within
        // the allocation, and HP_CURRENT is 8-byte aligned.
        unsafe {
            std::ptr::write(
                (base + textquest_common::offsets::player_zone::HP_CURRENT) as *mut i64,
                hp,
            );
        }
    }

    #[test]
    fn moveto_break_on_aggro_stops_navigation() {
        let mut nav = Navigator::new(0, 1);
        let mut config = MoveToConfig::to_position(100.0, 0.0, 0.0);
        config.break_on_aggro = true;
        nav.move_to_advanced(config);

        nav.tick(None, &[hostile_spawn_at(10.0, 0.0)], None);

        assert!(matches!(nav.status(), NavStatus::Idle));
    }

    #[test]
    fn moveto_break_on_hit_stops_after_damage() {
        let mut storage = player_hp_storage(100);
        let mut nav = Navigator::new(storage.as_mut_ptr() as usize, 1);
        let mut config = MoveToConfig::to_position(100.0, 0.0, 0.0);
        config.break_on_hit = true;
        nav.move_to_advanced(config);

        nav.tick(None, &[], None);
        assert!(matches!(nav.status(), NavStatus::Moving { .. }));

        write_player_hp(&mut storage, 90);
        nav.tick(None, &[], None);

        assert!(matches!(nav.status(), NavStatus::Idle));
    }

    #[test]
    fn moveto_without_break_on_hit_ignores_damage() {
        let mut storage = player_hp_storage(100);
        let mut nav = Navigator::new(storage.as_mut_ptr() as usize, 1);
        let config = MoveToConfig::to_position(100.0, 0.0, 0.0);
        nav.move_to_advanced(config);

        write_player_hp(&mut storage, 90);
        nav.tick(None, &[], None);

        assert!(matches!(nav.status(), NavStatus::Moving { .. }));
    }

    #[test]
    fn break_on_gm_pauses_navigation_when_gm_nearby() {
        let mut nav = Navigator::new(0, 1);
        nav.navigate(vec![Waypoint::new(100.0, 0.0, 0.0)]);
        nav.set_break_on_gm(true);

        // No GM nearby — should remain Moving.
        nav.tick(None, &[], None);
        assert!(matches!(nav.status(), NavStatus::Moving { .. }));

        // GM within check radius (player is at origin, GM at 10,0).
        let gm = gm_spawn_at(10.0, 0.0);
        nav.tick(None, &[gm], None);
        assert!(matches!(
            nav.status(),
            NavStatus::Paused {
                reason: PauseReason::GmNearby,
                ..
            }
        ));
    }

    #[test]
    fn break_on_gm_auto_resumes_when_gm_leaves() {
        let mut nav = Navigator::new(0, 1);
        nav.navigate(vec![Waypoint::new(100.0, 0.0, 0.0)]);
        nav.set_break_on_gm(true);

        // Trigger pause with a nearby GM.
        let gm = gm_spawn_at(10.0, 0.0);
        nav.tick(None, &[gm], None);
        assert!(matches!(
            nav.status(),
            NavStatus::Paused {
                reason: PauseReason::GmNearby,
                ..
            }
        ));

        // GM leaves — auto-resume expected.
        nav.tick(None, &[], None);
        assert!(matches!(nav.status(), NavStatus::Moving { .. }));
    }

    #[test]
    fn break_on_gm_disabled_does_not_pause() {
        let mut nav = Navigator::new(0, 1);
        nav.navigate(vec![Waypoint::new(100.0, 0.0, 0.0)]);
        // break_on_gm is false by default.

        let gm = gm_spawn_at(10.0, 0.0);
        nav.tick(None, &[gm], None);
        // Should still be moving — break_on_gm is off.
        assert!(matches!(nav.status(), NavStatus::Moving { .. }));
    }

    #[test]
    fn break_on_gm_far_gm_does_not_pause() {
        let mut nav = Navigator::new(0, 1);
        nav.navigate(vec![Waypoint::new(100.0, 0.0, 0.0)]);
        nav.set_break_on_gm(true);

        // GM is beyond GM_CHECK_RADIUS (600 EQ world units away; radius is 500).
        let gm = gm_spawn_at(600.0, 0.0);
        nav.tick(None, &[gm], None);
        assert!(matches!(nav.status(), NavStatus::Moving { .. }));
    }

    #[test]
    fn break_on_gm_does_not_override_user_pause() {
        let mut nav = Navigator::new(0, 1);
        nav.navigate(vec![Waypoint::new(100.0, 0.0, 0.0)]);
        nav.set_break_on_gm(true);
        nav.pause(); // user-initiated pause

        let gm = gm_spawn_at(10.0, 0.0);
        nav.tick(None, &[gm], None);
        // Still user-paused, not overwritten by GM detection.
        assert!(matches!(
            nav.status(),
            NavStatus::Paused {
                reason: PauseReason::UserPause,
                ..
            }
        ));
    }
}
