use std::collections::HashSet;

use super::strategy::CombatContext;
use textquest_common::{combat::PullMethod, nav::Waypoint, types::SpawnData};

/// World-space position used to plan directional pulls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PullPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl PullPosition {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn from_spawn(spawn: &SpawnData) -> Self {
        Self::new(spawn.x, spawn.y, spawn.z)
    }

    fn waypoint(self) -> Waypoint {
        Waypoint::new(self.x, self.y, self.z)
    }

    fn distance_2d(self, other: Self) -> f32 {
        self.waypoint().distance_2d(&other.waypoint())
    }
}

/// Position traversal pattern for a directional pull route.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PullPattern {
    /// Use the designated pull position, or the puller's current position.
    #[default]
    Direct,
    /// Walk waypoints in order and hold the final waypoint after the route ends.
    Line,
    /// Rotate through waypoints around camp.
    Circular,
}

/// Planned pull request mirroring the Lua-facing API shape.
#[derive(Debug, Clone, PartialEq)]
pub struct PullRequest {
    pub target_name: String,
    pub count: u32,
    pub position: Option<PullPosition>,
    pub pattern: PullPattern,
}

/// Feedback captured when a pull completes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PullRecord {
    pub target_id: u32,
    pub pull_position: PullPosition,
    pub spawn_position: PullPosition,
    pub spacing_from_previous: Option<f32>,
    pub spacing_ok: bool,
}

/// Computes the position a puller should use for a pull attempt.
pub trait PullPositionPlanner {
    fn position_for_pull(&self, pull_index: usize) -> Option<PullPosition>;
}

#[derive(Debug, Clone)]
struct DirectionalPullPlan {
    position: Option<PullPosition>,
    waypoints: Vec<PullPosition>,
    pattern: PullPattern,
}

impl Default for DirectionalPullPlan {
    fn default() -> Self {
        Self {
            position: None,
            waypoints: Vec::new(),
            pattern: PullPattern::Direct,
        }
    }
}

impl PullPositionPlanner for DirectionalPullPlan {
    fn position_for_pull(&self, pull_index: usize) -> Option<PullPosition> {
        match self.pattern {
            PullPattern::Direct => self.position.or_else(|| self.waypoints.first().copied()),
            PullPattern::Line => self
                .waypoints
                .get(pull_index.min(self.waypoints.len().saturating_sub(1)))
                .copied()
                .or(self.position),
            PullPattern::Circular => {
                if self.waypoints.is_empty() {
                    self.position
                } else {
                    self.waypoints
                        .get(pull_index % self.waypoints.len())
                        .copied()
                }
            }
        }
    }
}

/// States for the pulling sub-FSM.
#[derive(Debug)]
enum PullerState {
    /// Ready to pull next mob.
    Ready,
    /// Currently pulling a mob.
    Pulling { target_id: u32, started_tick: u32 },
    /// Waiting between pulls (chain pull cooldown).
    Waiting { ticks_remaining: u32 },
    /// Returning to camp after pull.
    Returning,
}

/// Manages the pulling cycle as a sub-state within the combat FSM.
pub struct Puller {
    state: PullerState,
    method: PullMethod,
    pull_range: f32,
    camp_range: f32,
    recently_pulled: HashSet<u32>,
    directional_plan: DirectionalPullPlan,
    active_pull_position: Option<PullPosition>,
    last_spawn_position: Option<PullPosition>,
    pull_log: Vec<PullRecord>,
    min_spawn_spacing: f32,
}

impl Puller {
    pub fn new(method: PullMethod) -> Self {
        Self {
            state: PullerState::Ready,
            method,
            pull_range: 200.0,
            camp_range: 100.0,
            recently_pulled: HashSet::new(),
            directional_plan: DirectionalPullPlan::default(),
            active_pull_position: None,
            last_spawn_position: None,
            pull_log: Vec::new(),
            min_spawn_spacing: 30.0,
        }
    }

    pub fn is_pulling(&self) -> bool {
        matches!(self.state, PullerState::Pulling { .. })
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.state, PullerState::Ready)
    }

    /// Designate a pull position before selecting targets.
    pub fn set_position(&mut self, position: PullPosition) {
        tracing::info!(
            x = position.x,
            y = position.y,
            z = position.z,
            "Directional pull position set"
        );
        self.directional_plan.position = Some(position);
    }

    /// Add a route waypoint used by line or circular pull patterns.
    pub fn add_waypoint(&mut self, position: PullPosition) {
        tracing::debug!(
            x = position.x,
            y = position.y,
            z = position.z,
            "Directional pull waypoint added"
        );
        self.directional_plan.waypoints.push(position);
    }

    /// Set how the puller traverses configured waypoints.
    pub fn set_pattern(&mut self, pattern: PullPattern) {
        self.directional_plan.pattern = pattern;
    }

    /// Set the minimum desired spacing between successive spawn positions.
    pub fn set_min_spawn_spacing(&mut self, spacing: f32) {
        self.min_spawn_spacing = spacing.max(0.0);
    }

    /// Build a pull request for higher-level command bindings.
    pub fn execute(&self, target_name: impl Into<String>, count: u32) -> PullRequest {
        PullRequest {
            target_name: target_name.into(),
            count,
            position: self.planned_pull_position(),
            pattern: self.directional_plan.pattern,
        }
    }

    /// Position the puller should move to before starting the next pull.
    pub fn planned_pull_position(&self) -> Option<PullPosition> {
        let skipped_index = usize::from(self.last_pull_spacing_bad());
        self.directional_plan
            .position_for_pull(self.pull_log.len() + skipped_index)
    }

    /// Completed pull feedback for analysis and spacing adjustments.
    pub fn pull_log(&self) -> &[PullRecord] {
        &self.pull_log
    }

    /// Select the next pull target: closest unengaged NPC within pull range
    /// that hasn't been pulled recently.
    pub fn next_target(&self, ctx: &CombatContext) -> Option<u32> {
        let origin = self
            .planned_pull_position()
            .unwrap_or_else(|| PullPosition::from_spawn(ctx.player));

        ctx.nearby_enemies
            .iter()
            .filter(|npc| !self.recently_pulled.contains(&npc.spawn_id))
            .map(|npc| {
                let dist = origin.distance_2d(PullPosition::from_spawn(npc));
                (npc.spawn_id, dist)
            })
            .filter(|(_, dist)| *dist < self.pull_range)
            .min_by(|(_, da), (_, db)| da.partial_cmp(db).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| id)
    }

    /// Advance the puller state machine one tick.
    pub fn tick(&mut self, ctx: &CombatContext) {
        let mut completed_target_id = None;

        match &mut self.state {
            PullerState::Ready => {
                // Caller should check next_target() and initiate pull
            }
            PullerState::Pulling {
                target_id,
                started_tick,
            } => {
                let elapsed = ctx.tick - *started_tick;
                let tid = *target_id;
                // After 60 ticks (~3 sec), assume pull landed or failed
                if elapsed > 60 {
                    self.recently_pulled.insert(tid);
                    completed_target_id = Some(tid);
                    self.state = PullerState::Returning;
                    tracing::debug!(target_id = tid, "Pull complete, returning");
                }
            }
            PullerState::Waiting { ticks_remaining } => {
                if *ticks_remaining == 0 {
                    self.state = PullerState::Ready;
                } else {
                    *ticks_remaining -= 1;
                }
            }
            PullerState::Returning => {
                // Once at camp (caller checks distance), transition to waiting
                self.state = PullerState::Waiting {
                    ticks_remaining: 20,
                };
            }
        }

        if let Some(target_id) = completed_target_id {
            self.record_pull_feedback(target_id, ctx);
        }
    }

    /// Start a pull on a specific target.
    pub fn start_pull(&mut self, target_id: u32, current_tick: u32) {
        self.active_pull_position = self.planned_pull_position();
        tracing::info!(
            target_id,
            pull_position = ?self.active_pull_position,
            "Starting pull"
        );
        self.state = PullerState::Pulling {
            target_id,
            started_tick: current_tick,
        };
    }

    pub fn method(&self) -> &PullMethod {
        &self.method
    }

    /// Clear recently pulled list (e.g., when camp changes).
    pub fn reset(&mut self) {
        self.recently_pulled.clear();
        self.state = PullerState::Ready;
        self.active_pull_position = None;
        self.last_spawn_position = None;
        self.pull_log.clear();
    }

    fn last_pull_spacing_bad(&self) -> bool {
        self.pull_log
            .last()
            .is_some_and(|record| !record.spacing_ok)
    }

    fn record_pull_feedback(&mut self, target_id: u32, ctx: &CombatContext) {
        let Some(target) = ctx
            .nearby_enemies
            .iter()
            .find(|npc| npc.spawn_id == target_id)
        else {
            self.active_pull_position = None;
            return;
        };

        let spawn_position = PullPosition::from_spawn(target);
        let pull_position = self
            .active_pull_position
            .unwrap_or_else(|| PullPosition::from_spawn(ctx.player));
        let spacing_from_previous = self
            .last_spawn_position
            .map(|previous| previous.distance_2d(spawn_position));
        let spacing_ok =
            spacing_from_previous.map_or(true, |spacing| spacing >= self.min_spawn_spacing);

        tracing::info!(
            target_id,
            pull_x = pull_position.x,
            pull_y = pull_position.y,
            spawn_x = spawn_position.x,
            spawn_y = spawn_position.y,
            spacing_from_previous,
            spacing_ok,
            "Directional pull feedback recorded"
        );

        self.pull_log.push(PullRecord {
            target_id,
            pull_position,
            spawn_position,
            spacing_from_previous,
            spacing_ok,
        });
        self.last_spawn_position = Some(spawn_position);
        self.active_pull_position = None;
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use textquest_common::combat::CombatConfig;
    use textquest_common::types::SpawnData;

    static DEFAULT_CONFIG: std::sync::LazyLock<CombatConfig> =
        std::sync::LazyLock::new(CombatConfig::default);

    fn make_ctx_with_enemies(
        player: &SpawnData,
        enemies: &[SpawnData],
        tick: u32,
    ) -> CombatContext<'_> {
        CombatContext {
            player,
            target: None,
            nearby_enemies: enemies,
            group_members: &[],
            config: &DEFAULT_CONFIG,
            tick,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        }
    }

    #[test]
    fn new_puller_is_ready() {
        let puller = Puller::new(PullMethod::BowPull);
        assert!(puller.is_ready());
        assert!(!puller.is_pulling());
    }

    #[test]
    fn start_pull_transitions_to_pulling() {
        let mut puller = Puller::new(PullMethod::BowPull);
        puller.start_pull(42, 100);
        assert!(puller.is_pulling());
        assert!(!puller.is_ready());
    }

    #[test]
    fn pull_completes_after_60_ticks() {
        let mut puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData::default();
        puller.start_pull(42, 100);

        // At tick 160 (60 ticks elapsed), should complete
        let ctx = make_ctx_with_enemies(&player, &[], 161);
        puller.tick(&ctx);
        assert!(!puller.is_pulling());
        // Should be in Returning state now
    }

    #[test]
    fn returning_transitions_to_waiting() {
        let mut puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData::default();
        puller.start_pull(42, 100);

        // Complete the pull
        let ctx = make_ctx_with_enemies(&player, &[], 161);
        puller.tick(&ctx);

        // Returning → Waiting
        let ctx2 = make_ctx_with_enemies(&player, &[], 162);
        puller.tick(&ctx2);
        assert!(!puller.is_ready());
        assert!(!puller.is_pulling());
    }

    #[test]
    fn waiting_transitions_to_ready() {
        let mut puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData::default();
        puller.start_pull(42, 100);

        // Complete pull → Returning
        let ctx = make_ctx_with_enemies(&player, &[], 161);
        puller.tick(&ctx);

        // Returning → Waiting(20)
        let ctx2 = make_ctx_with_enemies(&player, &[], 162);
        puller.tick(&ctx2);

        // Tick through waiting period (20 ticks)
        for i in 0..21 {
            let ctx3 = make_ctx_with_enemies(&player, &[], 163 + i);
            puller.tick(&ctx3);
        }
        assert!(puller.is_ready());
    }

    #[test]
    fn next_target_closest_unengaged() {
        let puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 100.0,
                y: 0.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 50.0,
                y: 0.0,
                ..SpawnData::default()
            },
        ];
        let ctx = make_ctx_with_enemies(&player, &enemies, 0);
        assert_eq!(puller.next_target(&ctx), Some(2)); // closest
    }

    #[test]
    fn next_target_filters_recently_pulled() {
        let mut puller = Puller::new(PullMethod::BowPull);
        puller.recently_pulled.insert(2);
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 100.0,
                y: 0.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 50.0,
                y: 0.0,
                ..SpawnData::default()
            },
        ];
        let ctx = make_ctx_with_enemies(&player, &enemies, 0);
        assert_eq!(puller.next_target(&ctx), Some(1)); // 2 is recently pulled
    }

    #[test]
    fn next_target_filters_out_of_range() {
        let puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData::default();
        let enemies = vec![SpawnData {
            spawn_id: 1,
            x: 500.0, // > pull_range (200)
            y: 0.0,
            ..SpawnData::default()
        }];
        let ctx = make_ctx_with_enemies(&player, &enemies, 0);
        assert!(puller.next_target(&ctx).is_none());
    }

    #[test]
    fn next_target_empty_enemies() {
        let puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData::default();
        let ctx = make_ctx_with_enemies(&player, &[], 0);
        assert!(puller.next_target(&ctx).is_none());
    }

    #[test]
    fn next_target_all_recently_pulled() {
        let mut puller = Puller::new(PullMethod::BowPull);
        puller.recently_pulled.insert(1);
        puller.recently_pulled.insert(2);
        let player = SpawnData::default();
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 50.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 60.0,
                ..SpawnData::default()
            },
        ];
        let ctx = make_ctx_with_enemies(&player, &enemies, 0);
        assert!(puller.next_target(&ctx).is_none());
    }

    #[test]
    fn reset_clears_state() {
        let mut puller = Puller::new(PullMethod::BowPull);
        puller.recently_pulled.insert(1);
        puller.start_pull(42, 100);
        assert!(puller.is_pulling());

        puller.reset();
        assert!(puller.is_ready());
        assert!(!puller.is_pulling());
    }

    #[test]
    fn method_returns_configured_method() {
        let puller = Puller::new(PullMethod::BowPull);
        assert!(matches!(puller.method(), PullMethod::BowPull));

        let puller2 = Puller::new(PullMethod::SpellPull { spell_slot: 3 });
        if let PullMethod::SpellPull { spell_slot } = puller2.method() {
            assert_eq!(*spell_slot, 3);
        } else {
            panic!("expected SpellPull");
        }
    }

    #[test]
    fn pulled_target_added_to_recently_pulled() {
        let mut puller = Puller::new(PullMethod::BowPull);
        let player = SpawnData::default();
        puller.start_pull(42, 100);

        // Complete pull
        let ctx = make_ctx_with_enemies(&player, &[], 161);
        puller.tick(&ctx);

        // Now 42 should be in recently_pulled
        let enemies = vec![SpawnData {
            spawn_id: 42,
            x: 50.0,
            ..SpawnData::default()
        }];
        let ctx2 = make_ctx_with_enemies(&player, &enemies, 162);
        assert!(puller.next_target(&ctx2).is_none());
    }

    #[test]
    fn directional_position_changes_target_origin() {
        let mut puller = Puller::new(PullMethod::BowPull);
        puller.set_position(PullPosition::new(180.0, 0.0, 0.0));
        let player = SpawnData::default();
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 20.0,
                y: 0.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 175.0,
                y: 0.0,
                ..SpawnData::default()
            },
        ];
        let ctx = make_ctx_with_enemies(&player, &enemies, 0);

        assert_eq!(puller.next_target(&ctx), Some(2));
    }

    #[test]
    fn execute_builds_directional_pull_request() {
        let mut puller = Puller::new(PullMethod::BowPull);
        let position = PullPosition::new(100.0, 200.0, 0.0);
        puller.set_position(position);

        let request = puller.execute("golems", 3);

        assert_eq!(request.target_name, "golems");
        assert_eq!(request.count, 3);
        assert_eq!(request.position, Some(position));
        assert_eq!(request.pattern, PullPattern::Direct);
    }

    #[test]
    fn circular_pattern_advances_waypoints() {
        let mut puller = Puller::new(PullMethod::BowPull);
        let first = PullPosition::new(10.0, 0.0, 0.0);
        let second = PullPosition::new(0.0, 10.0, 0.0);
        puller.add_waypoint(first);
        puller.add_waypoint(second);
        puller.set_pattern(PullPattern::Circular);

        assert_eq!(puller.planned_pull_position(), Some(first));

        let player = SpawnData::default();
        let enemies = vec![SpawnData {
            spawn_id: 42,
            x: 40.0,
            y: 0.0,
            ..SpawnData::default()
        }];
        puller.start_pull(42, 100);
        let ctx = make_ctx_with_enemies(&player, &enemies, 161);
        puller.tick(&ctx);

        assert_eq!(puller.planned_pull_position(), Some(second));
    }

    #[test]
    fn pull_feedback_tracks_bad_spawn_spacing() {
        let mut puller = Puller::new(PullMethod::BowPull);
        puller.set_min_spawn_spacing(25.0);
        let player = SpawnData::default();

        let first_enemy = vec![SpawnData {
            spawn_id: 1,
            x: 40.0,
            y: 0.0,
            ..SpawnData::default()
        }];
        puller.start_pull(1, 100);
        let first_ctx = make_ctx_with_enemies(&player, &first_enemy, 161);
        puller.tick(&first_ctx);

        let second_enemy = vec![SpawnData {
            spawn_id: 2,
            x: 50.0,
            y: 0.0,
            ..SpawnData::default()
        }];
        puller.start_pull(2, 200);
        let second_ctx = make_ctx_with_enemies(&player, &second_enemy, 261);
        puller.tick(&second_ctx);

        let log = puller.pull_log();
        assert_eq!(log.len(), 2);
        assert_eq!(log[1].spacing_from_previous, Some(10.0));
        assert!(!log[1].spacing_ok);
    }
}
