//! Stick-to-target engine — MQ2MoveUtils `/stick` equivalent.
//!
//! Maintains a configured distance from a resolved target spawn each game tick.
//! Supports the following MQ2MoveUtils modifiers:
//!
//! | Syntax             | Behaviour                                                   |
//! |--------------------|-------------------------------------------------------------|
//! | `/stick #`         | Absolute distance in EQ units                              |
//! | `/stick #%`        | Percentage of default melee range                           |
//! | `/stick mod #`     | Additive delta applied to the current distance              |
//! | `/stick hold`      | Lock onto the target that was active when stick was started |
//! | `/stick always`    | Keep the engine active; auto-resume on the next valid NPC  |
//! | `/stick id #`      | Stick to a specific spawn ID regardless of current target  |
//! | `/stick moveback`  | Back up when target walks closer than stick distance       |

use textquest_common::{
    nav::{NavStatus, StickConfig, StickDistance, StickMode, Waypoint},
    types::SpawnData,
};

/// Default stick distance in EQ units when no explicit distance is configured.
pub const DEFAULT_STICK_DISTANCE: f32 = 15.0;

/// Minimum effective stick distance — prevents jitter when distance_mod is very
/// negative.
const MIN_STICK_DISTANCE: f32 = 3.0;

/// How close the player must be to the desired stick position before movement
/// stops.
pub const STICK_ARRIVAL_THRESHOLD: f32 = 2.0;

/// Result of one [`StickEngine::tick`] call.
#[derive(Debug, Clone, PartialEq)]
pub enum StickTickResult {
    /// No stick session is active.
    Inactive,
    /// Target not found this tick (lost target, wrong id, etc.).
    /// Returned for `always` mode so the engine stays armed; otherwise the
    /// caller should treat this as a reason to stop.
    TargetLost,
    /// Within stick range — stop movement.
    InRange {
        target_id: u32,
        distance: f32,
        /// When healer mode is active, the position to face (target location).
        face_target: Option<Waypoint>,
    },
    /// Outside stick range — move toward `desired_pos`.
    OutOfRange {
        target_id: u32,
        distance: f32,
        desired_pos: Waypoint,
        /// When healer mode is active, the position to face (target location).
        face_target: Option<Waypoint>,
    },
    /// Too close to target — back up away from `target_pos`.
    /// Only returned when `moveback` is enabled and current distance drops
    /// below `effective_distance - backup_dist`.
    TooClose {
        target_id: u32,
        distance: f32,
        /// The position to retreat toward (away from target, at stick
        /// distance).
        retreat_pos: Waypoint,
    },
}

/// The stick-to-target engine for one EQ client.
pub struct StickEngine {
    config: StickConfig,
    /// Spawn ID locked at stick-start time (populated when `hold` or `id` is
    /// set).
    locked_id: Option<u32>,
    /// Whether a stick session is currently active.
    active: bool,
}

impl StickEngine {
    pub fn new() -> Self {
        Self {
            config: StickConfig::default(),
            locked_id: None,
            active: false,
        }
    }

    /// Begin a stick session.
    ///
    /// `current_target_id` is the spawn ID of the player's current target at
    /// the moment the command arrives.  It is used to populate `locked_id` when
    /// `hold` is set (and no explicit `id` was provided).
    pub fn start(&mut self, config: StickConfig, current_target_id: Option<u32>) {
        self.locked_id = if let Some(explicit_id) = config.id {
            Some(explicit_id)
        } else if config.hold {
            current_target_id
        } else {
            None
        };
        self.active = true;
        self.config = config;
        tracing::info!(
            hold = self.config.hold,
            always = self.config.always,
            locked_id = self.locked_id,
            distance = ?self.config.distance,
            distance_mod = self.config.distance_mod,
            "Stick started"
        );
    }

    /// Stop the stick session.
    pub fn stop(&mut self) {
        self.active = false;
        self.locked_id = None;
        tracing::info!("Stick stopped");
    }

    /// Apply a distance modifier delta (`/stick mod #`).
    ///
    /// Adds `delta` to `config.distance_mod`.  May be called while a stick
    /// session is active or before one starts (the mod persists until
    /// `stop()` is called).
    pub fn apply_mod(&mut self, delta: f32) {
        self.config.distance_mod += delta;
        tracing::debug!(
            distance_mod = self.config.distance_mod,
            "Stick distance_mod updated"
        );
    }

    /// Whether a stick session is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Compute one tick of stick logic.
    ///
    /// # Parameters
    /// - `player_pos`       — current player position (from
    ///   `MovementController::read_position`)
    /// - `current_target`   — game's current target (may be `None`)
    /// - `nearby`           — nearby spawn snapshot (used for `id` / `hold`
    ///   lookup)
    pub fn tick(
        &self,
        player_pos: &Waypoint,
        current_target: Option<&SpawnData>,
        nearby: &[SpawnData],
    ) -> StickTickResult {
        if !self.active {
            return StickTickResult::Inactive;
        }

        // Resolve the target for this tick.
        let Some(target) = self.resolve_target(current_target, nearby) else {
            tracing::debug!(
                hold = self.config.hold,
                always = self.config.always,
                locked_id = self.locked_id,
                "Stick: target not found this tick"
            );
            return StickTickResult::TargetLost;
        };

        let target_pos = Waypoint::new(target.x, target.y, target.z);
        let distance = player_pos.distance_2d(&target_pos);
        let effective_dist = self.effective_distance();

        // Compute arc-aware desired position.
        let desired_pos = arc_position(
            player_pos,
            &target_pos,
            target.heading,
            effective_dist,
            self.config.mode,
            self.config.behind_arc,
            self.config.not_front_arc,
        );

        let distance_to_desired = player_pos.distance_2d(&desired_pos);

        // Moveback check: if target walked into the player, back up.
        // Triggers when distance to target is below (effective_dist - backup_dist).
        if self.config.moveback
            && distance < (effective_dist - self.config.backup_dist).max(MIN_STICK_DISTANCE)
        {
            // Retreat position: a point at `effective_dist` from the target,
            // in the direction away from the target (toward the player).
            let retreat_pos = lerp_toward(player_pos, &target_pos, distance, effective_dist);
            return StickTickResult::TooClose {
                target_id: target.spawn_id,
                distance,
                retreat_pos,
            };
        }

        // In-range check: distance to the desired stick point is within threshold,
        // AND we're within effective stick distance of the target (with tolerance).
        // Healer mode: provide face_target so navigator can turn toward the target.
        let face_target = if self.config.healer {
            Some(target_pos)
        } else {
            None
        };

        if distance_to_desired <= STICK_ARRIVAL_THRESHOLD
            || (self.config.mode == StickMode::Any
                && distance <= effective_dist + STICK_ARRIVAL_THRESHOLD)
        {
            StickTickResult::InRange {
                target_id: target.spawn_id,
                distance,
                face_target,
            }
        } else {
            StickTickResult::OutOfRange {
                target_id: target.spawn_id,
                distance,
                desired_pos,
                face_target,
            }
        }
    }

    /// Resolve the current stick target and convert it into a warp-monitor
    /// sample.
    pub fn target_sample(
        &self,
        current_target: Option<&SpawnData>,
        nearby: &[SpawnData],
    ) -> Option<super::warp::TargetSample> {
        self.resolve_target(current_target, nearby)
            .map(|target| super::warp::TargetSample {
                id: target.spawn_id,
                position: Waypoint::new(target.x, target.y, target.z),
            })
    }

    /// Whether the current stick session should remain armed when the target is
    /// lost.
    pub fn keep_armed_on_target_loss(&self) -> bool {
        self.active && self.config.always
    }

    /// Build a [`NavStatus::Sticking`] variant for IPC reporting.
    pub fn nav_status(&self, target_id: u32, distance: f32) -> NavStatus {
        let effective_dist = self.effective_distance();
        NavStatus::Sticking {
            target_id,
            distance,
            in_range: distance <= effective_dist + STICK_ARRIVAL_THRESHOLD,
        }
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    /// Resolve which spawn we should stick to this tick.
    fn resolve_target<'a>(
        &self,
        current_target: Option<&'a SpawnData>,
        nearby: &'a [SpawnData],
    ) -> Option<&'a SpawnData> {
        // `id` overrides everything — always look up by explicit spawn ID.
        if let Some(id) = self.config.id {
            return nearby.iter().find(|s| s.spawn_id == id);
        }

        // `hold` — use the spawn ID that was locked at stick-start time.
        if self.config.hold {
            if let Some(locked) = self.locked_id {
                return nearby.iter().find(|s| s.spawn_id == locked);
            }
        }

        // Default: use the current target.
        // When `always` is true the caller will keep the engine armed even if
        // this returns `None`; a future tick will pick up the new target.
        current_target
    }

    /// Compute the effective stick distance (base + additive modifier).
    pub fn effective_distance(&self) -> f32 {
        let base = match self.config.distance {
            StickDistance::Default => DEFAULT_STICK_DISTANCE,
            StickDistance::Absolute(d) => d,
            StickDistance::Percent(p) => DEFAULT_STICK_DISTANCE * p / 100.0,
        };
        (base + self.config.distance_mod).max(MIN_STICK_DISTANCE)
    }
}

/// Compute the position that is `desired_dist` EQ units from `target` along
/// the line from `target` toward `player`.  Used to find the "stand here"
/// point.
fn lerp_toward(
    player: &Waypoint,
    target: &Waypoint,
    current_dist: f32,
    desired_dist: f32,
) -> Waypoint {
    if current_dist < 0.001 {
        // Player is on top of target — just return player position.
        return *player;
    }
    // Unit vector from target toward player.
    let dx = (player.x - target.x) / current_dist;
    let dy = (player.y - target.y) / current_dist;
    Waypoint::new(
        target.x + dx * desired_dist,
        target.y + dy * desired_dist,
        target.z,
    )
}

// ─── Arc positioning ────────────────────────────────────────────────────────

/// Convert EQ heading (512-unit circle: 0=N, 128=W, 256=S, 384=E) to radians.
/// Returns a standard math angle (0=East, CCW positive).
fn eq_heading_to_rad(heading: f32) -> f32 {
    // EQ: 0=N(+Y), 128=W(-X), 256=S(-Y), 384=E(+X)
    // Math: 0=E(+X), pi/2=N(+Y), pi=W(-X), 3pi/2=S(-Y)
    // Conversion: math_angle = pi/2 - heading * 2pi/512
    std::f32::consts::FRAC_PI_2 - heading * std::f32::consts::TAU / 512.0
}

/// Normalize an angle to [-pi, pi].
fn normalize_angle(mut a: f32) -> f32 {
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

/// Compute the angle from `target` toward `player` in standard math radians.
fn angle_from_target(player: &Waypoint, target: &Waypoint) -> f32 {
    (player.y - target.y).atan2(player.x - target.x)
}

/// Compute a position at `dist` units from `target` at a given angle (radians).
fn position_at_angle(target: &Waypoint, angle: f32, dist: f32) -> Waypoint {
    Waypoint::new(
        target.x + angle.cos() * dist,
        target.y + angle.sin() * dist,
        target.z,
    )
}

/// Compute the desired stick position with arc mode awareness.
///
/// For `StickMode::Any`, this is equivalent to `lerp_toward` (approach from
/// current direction). For arc modes, the character is steered into the target
/// arc defined by the target's heading.
fn arc_position(
    player: &Waypoint,
    target: &Waypoint,
    target_heading: f32,
    desired_dist: f32,
    mode: StickMode,
    behind_arc_deg: f32,
    not_front_arc_deg: f32,
) -> Waypoint {
    if mode == StickMode::Any {
        let current_dist = player.distance_2d(target);
        return lerp_toward(player, target, current_dist, desired_dist);
    }

    // Target's facing direction in standard radians.
    let face_rad = eq_heading_to_rad(target_heading);
    // Angle from target toward player.
    let player_angle = angle_from_target(player, target);
    // Behind direction is opposite of facing.
    let behind_rad = normalize_angle(face_rad + std::f32::consts::PI);

    let desired_angle = match mode {
        StickMode::Any => unreachable!(),

        StickMode::Behind => {
            // Clamp player's angle to within half-arc of the behind direction.
            let half_arc = (behind_arc_deg.clamp(5.1, 259.9) / 2.0).to_radians();
            clamp_to_arc(player_angle, behind_rad, half_arc)
        }

        StickMode::NotFront => {
            // Exclude the frontal cone. If player is in the front arc, steer to nearest
            // edge.
            let half_front = (not_front_arc_deg.clamp(5.1, 259.9) / 2.0).to_radians();
            let diff = normalize_angle(player_angle - face_rad);
            if diff.abs() < half_front {
                // Player is in the forbidden frontal arc — snap to nearest edge.
                if diff >= 0.0 {
                    normalize_angle(face_rad + half_front)
                } else {
                    normalize_angle(face_rad - half_front)
                }
            } else {
                // Already outside frontal arc — keep current angle.
                player_angle
            }
        }

        StickMode::Pin => {
            // Pick the closer flank (left or right, 90 degrees from facing).
            let left_flank = normalize_angle(face_rad + std::f32::consts::FRAC_PI_2);
            let right_flank = normalize_angle(face_rad - std::f32::consts::FRAC_PI_2);
            let diff_left = normalize_angle(player_angle - left_flank).abs();
            let diff_right = normalize_angle(player_angle - right_flank).abs();
            if diff_left <= diff_right {
                left_flank
            } else {
                right_flank
            }
        }

        StickMode::Front => {
            // Clamp player's angle to within half-arc of the facing direction.
            // Use behind_arc as the arc width for symmetry with Behind mode.
            let half_arc = (behind_arc_deg.clamp(5.1, 259.9) / 2.0).to_radians();
            clamp_to_arc(player_angle, face_rad, half_arc)
        }

        StickMode::SnapRoll => {
            // Opposite side of the target from the player's current position.
            normalize_angle(player_angle + std::f32::consts::PI)
        }
    };

    position_at_angle(target, desired_angle, desired_dist)
}

/// Clamp `angle` to be within `half_arc` radians of `center`.
/// Returns the clamped angle.
fn clamp_to_arc(angle: f32, center: f32, half_arc: f32) -> f32 {
    let diff = normalize_angle(angle - center);
    if diff.abs() <= half_arc {
        angle // already in arc
    } else if diff > 0.0 {
        normalize_angle(center + half_arc)
    } else {
        normalize_angle(center - half_arc)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use textquest_common::nav::StickConfig;

    fn make_spawn(id: u32, x: f32, y: f32) -> SpawnData {
        let mut s = SpawnData::default();
        s.spawn_id = id;
        s.x = x;
        s.y = y;
        s
    }

    fn player_at(x: f32, y: f32) -> Waypoint {
        Waypoint::new(x, y, 0.0)
    }

    // ── effective_distance ───────────────────────────────────────────────────

    #[test]
    fn effective_distance_default() {
        let mut engine = StickEngine::new();
        engine.start(StickConfig::default(), None);
        assert!((engine.effective_distance() - DEFAULT_STICK_DISTANCE).abs() < 0.01);
    }

    #[test]
    fn effective_distance_absolute() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.distance = StickDistance::Absolute(30.0);
        engine.start(config, None);
        assert!((engine.effective_distance() - 30.0).abs() < 0.01);
    }

    #[test]
    fn effective_distance_percent() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.distance = StickDistance::Percent(50.0);
        engine.start(config, None);
        // 50% of DEFAULT_STICK_DISTANCE
        assert!((engine.effective_distance() - DEFAULT_STICK_DISTANCE * 0.5).abs() < 0.01);
    }

    #[test]
    fn effective_distance_with_mod() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.distance = StickDistance::Absolute(20.0);
        config.distance_mod = 5.0;
        engine.start(config, None);
        assert!((engine.effective_distance() - 25.0).abs() < 0.01);
    }

    #[test]
    fn effective_distance_mod_clamped_to_min() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.distance = StickDistance::Absolute(5.0);
        config.distance_mod = -100.0; // would go negative
        engine.start(config, None);
        assert!(engine.effective_distance() >= MIN_STICK_DISTANCE);
    }

    // ── apply_mod ────────────────────────────────────────────────────────────

    #[test]
    fn apply_mod_increments_distance_mod() {
        let mut engine = StickEngine::new();
        engine.start(StickConfig::default(), None);
        engine.apply_mod(5.0);
        assert!((engine.config.distance_mod - 5.0).abs() < 0.01);
        engine.apply_mod(-2.0);
        assert!((engine.config.distance_mod - 3.0).abs() < 0.01);
    }

    // ── target resolution ────────────────────────────────────────────────────

    #[test]
    fn resolve_target_uses_current_by_default() {
        let mut engine = StickEngine::new();
        engine.start(StickConfig::default(), None);
        let target = make_spawn(42, 10.0, 0.0);
        let result = engine.resolve_target(Some(&target), &[]);
        assert!(result.is_some());
        assert_eq!(result.unwrap().spawn_id, 42);
    }

    #[test]
    fn resolve_target_no_target_no_always_is_none() {
        let mut engine = StickEngine::new();
        engine.start(StickConfig::default(), None);
        let result = engine.resolve_target(None, &[]);
        assert!(result.is_none());
    }

    #[test]
    fn resolve_target_id_overrides_current_target() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.id = Some(99);
        engine.start(config, Some(42));
        // current target has id=42, but we want id=99
        let current = make_spawn(42, 0.0, 0.0);
        let target_99 = make_spawn(99, 50.0, 0.0);
        let nearby = vec![current, target_99];
        let result = engine.resolve_target(None, &nearby);
        assert_eq!(result.unwrap().spawn_id, 99);
    }

    #[test]
    fn resolve_target_id_not_found_in_nearby() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.id = Some(999);
        engine.start(config, None);
        let nearby = vec![make_spawn(1, 0.0, 0.0)];
        assert!(engine.resolve_target(None, &nearby).is_none());
    }

    #[test]
    fn resolve_target_hold_uses_locked_id() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.hold = true;
        // Current target at stick-start is spawn 7.
        engine.start(config, Some(7));
        assert_eq!(engine.locked_id, Some(7));
        // Player later retargets to spawn 99 — but hold ignores current target.
        let original_target = make_spawn(7, 20.0, 0.0);
        let new_target = make_spawn(99, 5.0, 0.0);
        let nearby = vec![original_target, new_target];
        let current = make_spawn(99, 5.0, 0.0);
        let result = engine.resolve_target(Some(&current), &nearby);
        assert_eq!(result.unwrap().spawn_id, 7);
    }

    #[test]
    fn resolve_target_hold_no_current_at_start_returns_none() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.hold = true;
        engine.start(config, None); // no target at start
        assert!(engine.locked_id.is_none());
        let nearby = vec![make_spawn(1, 0.0, 0.0)];
        assert!(engine.resolve_target(None, &nearby).is_none());
    }

    // ── tick results ─────────────────────────────────────────────────────────

    #[test]
    fn tick_inactive_before_start() {
        let engine = StickEngine::new();
        let player = player_at(0.0, 0.0);
        assert_eq!(engine.tick(&player, None, &[]), StickTickResult::Inactive);
    }

    #[test]
    fn tick_target_lost_when_no_target_and_not_always() {
        let mut engine = StickEngine::new();
        engine.start(StickConfig::default(), None);
        let player = player_at(0.0, 0.0);
        assert_eq!(engine.tick(&player, None, &[]), StickTickResult::TargetLost);
    }

    #[test]
    fn tick_in_range_when_close_enough() {
        let mut engine = StickEngine::new();
        engine.start(StickConfig::default(), None);
        let player = player_at(0.0, 0.0);
        // Target is at distance 10, effective distance is ~15 → should be InRange.
        let target = make_spawn(1, 10.0, 0.0);
        match engine.tick(&player, Some(&target), &[]) {
            StickTickResult::InRange {
                target_id,
                distance,
                ..
            } => {
                assert_eq!(target_id, 1);
                assert!((distance - 10.0).abs() < 0.1);
            }
            other => panic!("expected InRange, got {other:?}"),
        }
    }

    #[test]
    fn tick_out_of_range_when_far() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.distance = StickDistance::Absolute(10.0);
        engine.start(config, None);
        // Player at (0,0), target at (50,0) → distance 50, effective dist 10 →
        // OutOfRange
        let player = player_at(0.0, 0.0);
        let target = make_spawn(5, 50.0, 0.0);
        match engine.tick(&player, Some(&target), &[]) {
            StickTickResult::OutOfRange {
                target_id,
                distance,
                desired_pos,
                ..
            } => {
                assert_eq!(target_id, 5);
                assert!((distance - 50.0).abs() < 0.1);
                // Desired position should be ~10 units from target toward player (x-axis).
                // Target at x=50, player at x=0 → desired x ≈ 50 - 10*(-1) = wait, let me
                // recalculate. unit vec from target toward player: dx =
                // (0-50)/50 = -1, dy = 0 desired = (50 + (-1)*10, 0) = (40, 0)
                assert!((desired_pos.x - 40.0).abs() < 0.2);
                assert!(desired_pos.y.abs() < 0.2);
            }
            other => panic!("expected OutOfRange, got {other:?}"),
        }
    }

    #[test]
    fn tick_always_returns_target_lost_not_inactive() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.always = true;
        engine.start(config, None);
        let player = player_at(0.0, 0.0);
        // With always=true and no target, we should get TargetLost (not Inactive).
        assert_eq!(engine.tick(&player, None, &[]), StickTickResult::TargetLost);
        // Engine stays active.
        assert!(engine.is_active());
    }

    #[test]
    fn stop_deactivates_engine() {
        let mut engine = StickEngine::new();
        engine.start(StickConfig::default(), None);
        assert!(engine.is_active());
        engine.stop();
        assert!(!engine.is_active());
        let player = player_at(0.0, 0.0);
        assert_eq!(engine.tick(&player, None, &[]), StickTickResult::Inactive);
    }

    // ── lerp_toward ──────────────────────────────────────────────────────────

    #[test]
    fn lerp_toward_places_point_at_correct_distance() {
        let player = Waypoint::new(0.0, 0.0, 0.0);
        let target = Waypoint::new(100.0, 0.0, 0.0);
        let result = lerp_toward(&player, &target, 100.0, 10.0);
        // Should be at x=90 (10 units from target toward player)
        assert!((result.x - 90.0).abs() < 0.1);
        assert!(result.y.abs() < 0.1);
    }

    // ── moveback ────────────────────────────────────────────────────────────

    #[test]
    fn moveback_disabled_returns_in_range_when_close() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.distance = StickDistance::Absolute(15.0);
        config.moveback = false; // moveback disabled
        engine.start(config, None);
        // Player at (0,0), target at (5,0) → distance 5 (much closer than 15)
        let player = player_at(0.0, 0.0);
        let target = make_spawn(1, 5.0, 0.0);
        match engine.tick(&player, Some(&target), &[]) {
            StickTickResult::InRange { .. } => {} // expected
            other => panic!("expected InRange with moveback disabled, got {other:?}"),
        }
    }

    #[test]
    fn moveback_enabled_returns_too_close() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.distance = StickDistance::Absolute(15.0);
        config.moveback = true;
        config.backup_dist = 5.0;
        engine.start(config, None);
        // Player at (0,0), target at (5,0) → distance 5
        // Moveback threshold = 15.0 - 5.0 = 10.0, and 5 < 10 → TooClose
        let player = player_at(0.0, 0.0);
        let target = make_spawn(1, 5.0, 0.0);
        match engine.tick(&player, Some(&target), &[]) {
            StickTickResult::TooClose {
                target_id,
                distance,
                retreat_pos,
            } => {
                assert_eq!(target_id, 1);
                assert!((distance - 5.0).abs() < 0.1);
                // Retreat pos should be 15 units from target toward player (x-axis).
                // Target at x=5, player at x=0 → direction is -x.
                // retreat = target + (-1) * 15 = 5 - 15 = -10
                assert!((retreat_pos.x - (-10.0)).abs() < 0.2);
            }
            other => panic!("expected TooClose, got {other:?}"),
        }
    }

    #[test]
    fn moveback_does_not_trigger_when_within_acceptable_range() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.distance = StickDistance::Absolute(15.0);
        config.moveback = true;
        config.backup_dist = 5.0;
        engine.start(config, None);
        // Player at (0,0), target at (12,0) → distance 12
        // Moveback threshold = 15.0 - 5.0 = 10.0, and 12 > 10 → NOT TooClose
        let player = player_at(0.0, 0.0);
        let target = make_spawn(1, 12.0, 0.0);
        if let StickTickResult::TooClose { .. } = engine.tick(&player, Some(&target), &[]) {
            panic!("should not trigger moveback when above threshold");
        }
    }

    #[test]
    fn moveback_threshold_clamped_to_min_stick_distance() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.distance = StickDistance::Absolute(5.0);
        config.moveback = true;
        config.backup_dist = 10.0; // larger than stick distance
        engine.start(config, None);
        // Threshold would be 5.0 - 10.0 = -5.0, clamped to MIN_STICK_DISTANCE (3.0).
        // Player at (0,0), target at (2,0) → distance 2 < 3.0 → TooClose
        let player = player_at(0.0, 0.0);
        let target = make_spawn(1, 2.0, 0.0);
        match engine.tick(&player, Some(&target), &[]) {
            StickTickResult::TooClose { .. } => {} // expected
            other => panic!("expected TooClose when below clamped threshold, got {other:?}"),
        }
    }

    #[test]
    fn moveback_threshold_clamped_does_not_trigger_above_min() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.distance = StickDistance::Absolute(5.0);
        config.moveback = true;
        config.backup_dist = 10.0; // larger than stick distance
        engine.start(config, None);
        // Threshold clamped to MIN_STICK_DISTANCE (3.0).
        // Player at (0,0), target at (4,0) → distance 4 > 3.0 → NOT TooClose
        let player = player_at(0.0, 0.0);
        let target = make_spawn(1, 4.0, 0.0);
        if let StickTickResult::TooClose { .. } = engine.tick(&player, Some(&target), &[]) {
            panic!("should not trigger when above clamped threshold");
        }
    }

    // ── healer mode (#163) ──────────────────────────────────────────────────

    #[test]
    fn healer_mode_sets_face_target() {
        let mut engine = StickEngine::new();
        let mut config = StickConfig::default();
        config.healer = true;
        engine.start(config, None);
        let player = player_at(0.0, 0.0);
        let target = make_spawn(1, 10.0, 0.0);
        match engine.tick(&player, Some(&target), &[]) {
            StickTickResult::InRange { face_target, .. } => {
                assert!(face_target.is_some(), "healer mode should set face_target");
                let ft = face_target.unwrap();
                assert!((ft.x - 10.0).abs() < 0.1);
            }
            other => panic!("expected InRange, got {other:?}"),
        }
    }

    #[test]
    fn non_healer_mode_no_face_target() {
        let mut engine = StickEngine::new();
        let config = StickConfig::default();
        engine.start(config, None);
        let player = player_at(0.0, 0.0);
        let target = make_spawn(1, 10.0, 0.0);
        match engine.tick(&player, Some(&target), &[]) {
            StickTickResult::InRange { face_target, .. } => {
                assert!(
                    face_target.is_none(),
                    "non-healer should not set face_target"
                );
            }
            other => panic!("expected InRange, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod arc_tests {
    use super::*;
    use textquest_common::nav::{StickConfig, StickDistance, StickMode};

    fn spawn_at(id: u32, x: f32, y: f32, heading: f32) -> SpawnData {
        SpawnData {
            spawn_id: id,
            x,
            y,
            heading,
            ..SpawnData::default()
        }
    }

    #[test]
    fn eq_heading_north_to_rad() {
        let rad = eq_heading_to_rad(0.0);
        assert!((rad - std::f32::consts::FRAC_PI_2).abs() < 0.01);
    }

    #[test]
    fn eq_heading_south_to_rad() {
        let rad = eq_heading_to_rad(256.0);
        assert!((rad - (-std::f32::consts::FRAC_PI_2)).abs() < 0.01);
    }

    #[test]
    fn normalize_angle_wraps() {
        assert!(normalize_angle(4.0).abs() <= std::f32::consts::PI);
        assert!(normalize_angle(-4.0).abs() <= std::f32::consts::PI);
        assert!((normalize_angle(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn clamp_to_arc_inside_unchanged() {
        assert!((clamp_to_arc(0.1, 0.0, 0.5) - 0.1).abs() < 0.001);
    }

    #[test]
    fn clamp_to_arc_outside_snaps() {
        assert!((clamp_to_arc(1.0, 0.0, 0.3) - 0.3).abs() < 0.01);
        assert!((clamp_to_arc(-1.0, 0.0, 0.3) - (-0.3)).abs() < 0.01);
    }

    #[test]
    fn arc_any_same_as_lerp() {
        let p = Waypoint::new(0.0, 0.0, 0.0);
        let t = Waypoint::new(50.0, 0.0, 0.0);
        let r = arc_position(&p, &t, 0.0, 10.0, StickMode::Any, 45.0, 90.0);
        assert!((r.x - 40.0).abs() < 0.5);
    }

    #[test]
    fn arc_behind_north_facing() {
        let p = Waypoint::new(0.0, -20.0, 0.0);
        let t = Waypoint::new(0.0, 0.0, 0.0);
        let r = arc_position(&p, &t, 0.0, 10.0, StickMode::Behind, 45.0, 90.0);
        assert!(r.y < -5.0, "Expected south, got y={}", r.y);
        assert!((t.distance_2d(&r) - 10.0).abs() < 0.5);
    }

    #[test]
    fn arc_behind_redirects_from_front() {
        let p = Waypoint::new(0.0, 20.0, 0.0);
        let t = Waypoint::new(0.0, 0.0, 0.0);
        let r = arc_position(&p, &t, 0.0, 10.0, StickMode::Behind, 45.0, 90.0);
        assert!(r.y < 0.0, "Expected redirect behind, got y={}", r.y);
    }

    #[test]
    fn arc_not_front_allows_side() {
        let p = Waypoint::new(20.0, 0.0, 0.0);
        let t = Waypoint::new(0.0, 0.0, 0.0);
        let r = arc_position(&p, &t, 0.0, 10.0, StickMode::NotFront, 45.0, 90.0);
        assert!(r.x > 5.0, "Should stay east, got x={}", r.x);
    }

    #[test]
    fn arc_not_front_redirects_from_front() {
        let p = Waypoint::new(0.0, 20.0, 0.0);
        let t = Waypoint::new(0.0, 0.0, 0.0);
        let r = arc_position(&p, &t, 0.0, 10.0, StickMode::NotFront, 45.0, 90.0);
        let angle = angle_from_target(&r, &t);
        let diff = normalize_angle(angle - eq_heading_to_rad(0.0)).abs();
        assert!(
            diff >= (45.0_f32).to_radians() - 0.1,
            "Should be outside front arc"
        );
    }

    #[test]
    fn arc_pin_picks_closer_flank() {
        let p = Waypoint::new(20.0, 0.0, 0.0);
        let t = Waypoint::new(0.0, 0.0, 0.0);
        let r = arc_position(&p, &t, 0.0, 10.0, StickMode::Pin, 45.0, 90.0);
        assert!(r.x.abs() > 5.0, "Should be on flank, x={}", r.x);
    }

    #[test]
    fn arc_pin_left_when_closer() {
        let p = Waypoint::new(-20.0, 0.0, 0.0);
        let t = Waypoint::new(0.0, 0.0, 0.0);
        let r = arc_position(&p, &t, 0.0, 10.0, StickMode::Pin, 45.0, 90.0);
        assert!(r.x < -5.0, "Should be west flank, x={}", r.x);
    }

    #[test]
    fn arc_front_places_in_front() {
        let p = Waypoint::new(0.0, 20.0, 0.0);
        let t = Waypoint::new(0.0, 0.0, 0.0);
        let r = arc_position(&p, &t, 0.0, 10.0, StickMode::Front, 45.0, 90.0);
        assert!(r.y > 5.0, "Expected front (north), got y={}", r.y);
    }

    #[test]
    fn arc_front_redirects_from_behind() {
        let p = Waypoint::new(0.0, -20.0, 0.0);
        let t = Waypoint::new(0.0, 0.0, 0.0);
        let r = arc_position(&p, &t, 0.0, 10.0, StickMode::Front, 45.0, 90.0);
        assert!((t.distance_2d(&r) - 10.0).abs() < 0.5);
    }

    #[test]
    fn arc_all_modes_correct_distance() {
        let p = Waypoint::new(15.0, -15.0, 0.0);
        let t = Waypoint::new(0.0, 0.0, 0.0);
        for mode in [
            StickMode::Behind,
            StickMode::NotFront,
            StickMode::Pin,
            StickMode::Front,
        ] {
            let r = arc_position(&p, &t, 0.0, 12.0, mode, 45.0, 90.0);
            assert!((t.distance_2d(&r) - 12.0).abs() < 0.5, "{mode:?}");
        }
    }

    #[test]
    fn arc_behind_wide_arc_allows_side() {
        let p = Waypoint::new(20.0, 0.0, 0.0);
        let t = Waypoint::new(0.0, 0.0, 0.0);
        let r = arc_position(&p, &t, 0.0, 10.0, StickMode::Behind, 180.0, 90.0);
        assert!(r.x > 0.0, "Wide arc should allow east");
    }

    #[test]
    fn tick_behind_mode_to_rear() {
        let mut engine = StickEngine::new();
        let cfg = StickConfig {
            distance: StickDistance::Absolute(10.0),
            mode: StickMode::Behind,
            ..StickConfig::default()
        };
        engine.start(cfg, None);
        let player = Waypoint::new(0.0, 30.0, 0.0);
        let target = spawn_at(1, 0.0, 0.0, 0.0);
        match engine.tick(&player, Some(&target), &[]) {
            StickTickResult::OutOfRange { desired_pos, .. } => {
                assert!(desired_pos.y < 0.0, "Expected behind, y={}", desired_pos.y);
            }
            other => panic!("expected OutOfRange, got {other:?}"),
        }
    }

    #[test]
    fn tick_front_mode_for_tank() {
        let mut engine = StickEngine::new();
        let cfg = StickConfig {
            distance: StickDistance::Absolute(10.0),
            mode: StickMode::Front,
            ..StickConfig::default()
        };
        engine.start(cfg, None);
        let player = Waypoint::new(0.0, -30.0, 0.0);
        let target = spawn_at(1, 0.0, 0.0, 0.0);
        match engine.tick(&player, Some(&target), &[]) {
            StickTickResult::OutOfRange { desired_pos, .. } => {
                assert!(desired_pos.y > 0.0, "Expected front, y={}", desired_pos.y);
            }
            other => panic!("expected OutOfRange, got {other:?}"),
        }
    }

    // ── snaproll (#183) ──

    #[test]
    fn snaproll_positions_opposite_side() {
        let player = Waypoint::new(0.0, 0.0, 0.0);
        let target = Waypoint::new(10.0, 0.0, 0.0);
        let result = arc_position(&player, &target, 0.0, 15.0, StickMode::SnapRoll, 45.0, 90.0);
        assert!(
            result.x > target.x,
            "snaproll should place player on opposite side: result.x={}, target.x={}",
            result.x,
            target.x
        );
    }
}
