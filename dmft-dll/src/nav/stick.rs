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

use dmft_common::nav::{NavStatus, StickConfig, StickDistance, Waypoint};
use dmft_common::types::SpawnData;

/// Default stick distance in EQ units when no explicit distance is configured.
pub const DEFAULT_STICK_DISTANCE: f32 = 15.0;

/// Minimum effective stick distance — prevents jitter when distance_mod is very negative.
const MIN_STICK_DISTANCE: f32 = 3.0;

/// How close the player must be to the desired stick position before movement stops.
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
    },
    /// Outside stick range — move toward `desired_pos`.
    OutOfRange {
        target_id: u32,
        distance: f32,
        desired_pos: Waypoint,
    },
}

/// The stick-to-target engine for one EQ client.
pub struct StickEngine {
    config: StickConfig,
    /// Spawn ID locked at stick-start time (populated when `hold` or `id` is set).
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
    /// Adds `delta` to `config.distance_mod`.  May be called while a stick session
    /// is active or before one starts (the mod persists until `stop()` is called).
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
    /// - `player_pos`       — current player position (from `MovementController::read_position`)
    /// - `current_target`   — game's current target (may be `None`)
    /// - `nearby`           — nearby spawn snapshot (used for `id` / `hold` lookup)
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

        if distance <= effective_dist + STICK_ARRIVAL_THRESHOLD {
            StickTickResult::InRange {
                target_id: target.spawn_id,
                distance,
            }
        } else {
            // Compute desired position: `effective_dist` units from target toward player.
            let desired_pos = lerp_toward(player_pos, &target_pos, distance, effective_dist);
            StickTickResult::OutOfRange {
                target_id: target.spawn_id,
                distance,
                desired_pos,
            }
        }
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
/// the line from `target` toward `player`.  Used to find the "stand here" point.
fn lerp_toward(player: &Waypoint, target: &Waypoint, current_dist: f32, desired_dist: f32) -> Waypoint {
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

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use dmft_common::nav::StickConfig;

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
        assert_eq!(
            engine.tick(&player, None, &[]),
            StickTickResult::Inactive
        );
    }

    #[test]
    fn tick_target_lost_when_no_target_and_not_always() {
        let mut engine = StickEngine::new();
        engine.start(StickConfig::default(), None);
        let player = player_at(0.0, 0.0);
        assert_eq!(
            engine.tick(&player, None, &[]),
            StickTickResult::TargetLost
        );
    }

    #[test]
    fn tick_in_range_when_close_enough() {
        let mut engine = StickEngine::new();
        engine.start(StickConfig::default(), None);
        let player = player_at(0.0, 0.0);
        // Target is at distance 10, effective distance is ~15 → should be InRange.
        let target = make_spawn(1, 10.0, 0.0);
        match engine.tick(&player, Some(&target), &[]) {
            StickTickResult::InRange { target_id, distance } => {
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
        // Player at (0,0), target at (50,0) → distance 50, effective dist 10 → OutOfRange
        let player = player_at(0.0, 0.0);
        let target = make_spawn(5, 50.0, 0.0);
        match engine.tick(&player, Some(&target), &[]) {
            StickTickResult::OutOfRange {
                target_id,
                distance,
                desired_pos,
            } => {
                assert_eq!(target_id, 5);
                assert!((distance - 50.0).abs() < 0.1);
                // Desired position should be ~10 units from target toward player (x-axis).
                // Target at x=50, player at x=0 → desired x ≈ 50 - 10*(-1) = wait, let me recalculate.
                // unit vec from target toward player: dx = (0-50)/50 = -1, dy = 0
                // desired = (50 + (-1)*10, 0) = (40, 0)
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
        assert_eq!(
            engine.tick(&player, None, &[]),
            StickTickResult::TargetLost
        );
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
}
