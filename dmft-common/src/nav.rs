// dmft-common/src/nav.rs
use serde::{Deserialize, Serialize};

/// Knuth multiplicative hash constant for deterministic per-client randomness.
pub const KNUTH_HASH: u32 = 2_654_435_761;

/// Simple xorshift32 PRNG for deterministic per-client randomness.
pub struct Xorshift32 {
    state: u32,
}

impl Xorshift32 {
    /// Create a new PRNG with the given seed. Seed must not be 0 (use `from_client_id` for safe seeding).
    #[must_use]
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    /// Create a PRNG seeded deterministically from a client PID via Knuth hash.
    #[must_use]
    pub fn from_client_id(client_id: u32) -> Self {
        let seed = client_id.wrapping_mul(KNUTH_HASH);
        // Xorshift with seed 0 is a fixed point — every call returns 0 forever.
        Self::new(if seed == 0 { 1 } else { seed })
    }

    /// Generate the next pseudo-random `u32`.
    pub fn next_u32(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state
    }

    /// Generate a pseudo-random `f32` in [0.0, 1.0).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32) / (u32::MAX as f32)
    }
}

/// Reasons navigation can be paused without abandoning the path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PauseReason {
    /// Target or anchor warped unexpectedly — wait for stability.
    Warp,
}

/// A single point in 3D space with optional metadata.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Waypoint {
    /// World X coordinate.
    pub x: f32,
    /// World Y coordinate.
    pub y: f32,
    /// World Z coordinate (vertical).
    pub z: f32,
}

impl Waypoint {
    /// Create a waypoint at the given coordinates.
    #[must_use]
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// 2D distance (XY plane) to another waypoint.
    #[must_use]
    pub fn distance_2d(&self, other: &Waypoint) -> f32 {
        ((other.x - self.x).powi(2) + (other.y - self.y).powi(2)).sqrt()
    }

    /// 3D distance to another waypoint.
    #[must_use]
    pub fn distance_3d(&self, other: &Waypoint) -> f32 {
        ((other.x - self.x).powi(2) + (other.y - self.y).powi(2) + (other.z - self.z).powi(2))
            .sqrt()
    }
}

/// Configuration for MQ2MoveUtils-style `/makecamp player` follow mode.
///
/// Establishes a dynamic camp anchor that tracks another player's position.
/// Followers maintain `follow_distance` from the anchor and are forced back
/// within `leash_distance` when they stray too far.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FollowConfig {
    /// Name of the player being followed (for logging and status display).
    pub leader_name: String,
    /// Desired distance to maintain from the leader's position.
    pub follow_distance: f32,
    /// Maximum distance from the anchor before forcing a return.
    /// Mirrors MQ2MoveUtils `/makecamp leash` enforcement radius.
    pub leash_distance: f32,
}

impl FollowConfig {
    /// Create a new follow configuration.
    #[must_use]
    pub fn new(leader_name: impl Into<String>, follow_distance: f32, leash_distance: f32) -> Self {
        Self {
            leader_name: leader_name.into(),
            follow_distance,
            leash_distance,
        }
    }
}

/// Distance specification for a `/stick` command.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum StickDistance {
    /// Stick at the default melee range (~15 EQ units).
    #[default]
    Default,
    /// Stick at an explicit absolute distance in EQ units (`/stick #`).
    Absolute(f32),
    /// Stick at a percentage of the default range (`/stick #%`).
    Percent(f32),
}

/// Configuration for a `/stick` session (MQ2MoveUtils compatible).
///
/// Maps the MQ2MoveUtils command surface:
/// - `/stick #`      → `distance = StickDistance::Absolute(#)`
/// - `/stick #%`     → `distance = StickDistance::Percent(#)`
/// - `/stick mod #`  → `distance_mod += #` (applied via `StickMod` command)
/// - `/stick hold`   → `hold = true`
/// - `/stick always` → `always = true`
/// - `/stick id #`   → `id = Some(#)`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StickConfig {
    /// Base distance to maintain from the target.
    pub distance: StickDistance,
    /// Additive distance modifier applied on top of `distance` (from `/stick mod #`).
    pub distance_mod: f32,
    /// Lock onto the current target's spawn ID even if the player retargets (`hold`).
    pub hold: bool,
    /// Keep the stick engine active and auto-resume on the next valid NPC
    /// when the current target is lost (`always`).
    pub always: bool,
    /// Stick to a specific spawn ID regardless of current target (`id #`).
    pub id: Option<u32>,
}

impl Default for StickConfig {
    fn default() -> Self {
        Self {
            distance: StickDistance::Default,
            distance_mod: 0.0,
            hold: false,
            always: false,
            id: None,
        }
    }
}

/// Current navigation state reported from DLL to orchestrator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NavStatus {
    /// Not navigating.
    Idle,
    /// Actively moving toward a waypoint.
    Moving {
        /// Current waypoint index in the path.
        waypoint_index: usize,
        /// Total waypoints in path.
        waypoint_count: usize,
        /// Distance to current waypoint.
        distance_remaining: f32,
    },
    /// Paused (path retained) due to a safety condition.
    Paused {
        /// Why navigation was paused.
        reason: PauseReason,
        /// Current waypoint index in the path.
        waypoint_index: usize,
        /// Total waypoints in path.
        waypoint_count: usize,
        /// Distance to current waypoint.
        distance_remaining: f32,
    },
    /// Stuck and attempting recovery.
    Stuck {
        /// Which recovery attempt this is (1, 2, 3...).
        recovery_attempt: u32,
    },
    /// Arrived at final destination.
    Arrived,
    /// Player follow mode active (`/makecamp player` equivalent).
    Following {
        /// Name of the player being followed.
        leader_name: String,
        /// 2-D distance from self to the current anchor position.
        distance_to_anchor: f32,
        /// Whether the follower is currently navigating back to the anchor.
        returning: bool,
    },
    /// Actively sticking to a target spawn (MQ2MoveUtils `/stick` equivalent).
    Sticking {
        /// Spawn ID of the current stick target (0 when target is temporarily lost).
        target_id: u32,
        /// Current 2D distance to the stick target.
        distance: f32,
        /// `true` when within the desired stick range.
        in_range: bool,
    },
}

impl NavStatus {
    /// Human-readable label for the current navigation state.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Moving { .. } => "Navigating",
            Self::Paused { .. } => "Paused",
            Self::Stuck { .. } => "Stuck",
            Self::Arrived => "Arrived",
            Self::Following { .. } => "Following",
            Self::Sticking { .. } => "Sticking",
        }
    }

    /// Returns true if currently navigating toward a waypoint.
    #[must_use]
    pub fn is_moving(&self) -> bool {
        matches!(self, Self::Moving { .. })
    }

    /// Returns true if navigation is currently paused.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        matches!(self, Self::Paused { .. })
    }

    /// Returns true if stuck and attempting recovery.
    #[must_use]
    pub fn is_stuck(&self) -> bool {
        matches!(self, Self::Stuck { .. })
    }

    /// Returns true if arrived at the final waypoint.
    #[must_use]
    pub fn is_arrived(&self) -> bool {
        matches!(self, Self::Arrived)
    }

    /// Returns true if player follow mode is active.
    #[must_use]
    pub fn is_following(&self) -> bool {
        matches!(self, Self::Following { .. })
    }

    /// Returns true if actively sticking to a target.
    #[must_use]
    pub fn is_sticking(&self) -> bool {
        matches!(self, Self::Sticking { .. })
    }
}

/// Navigation path health metrics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NavPathMetrics {
    /// Whether a navmesh-backed path was successfully computed.
    pub path_exists: bool,
    /// Total distance of the planned path (in world units), if known.
    pub path_length: Option<f32>,
    /// Human-readable reason when no navmesh path could be found.
    pub failure_reason: Option<String>,
}

impl NavPathMetrics {
    /// Construct metrics for a successful navmesh query.
    #[must_use]
    pub fn success(path_length: Option<f32>) -> Self {
        Self {
            path_exists: true,
            path_length,
            failure_reason: None,
        }
    }

    /// Construct metrics for a failed navmesh query.
    #[must_use]
    pub fn failure(reason: impl Into<String>, path_length: Option<f32>) -> Self {
        Self {
            path_exists: false,
            path_length,
            failure_reason: Some(reason.into()),
        }
    }
}

/// A generic indexed cursor over a `Vec<T>`.
///
/// Provides sequential traversal with `current()` / `advance()` semantics.
/// Used to deduplicate the cursor-over-Vec pattern in `WaypointQueue` and `TravelPlan`.
pub struct IndexedQueue<T> {
    items: Vec<T>,
    index: usize,
}

impl<T> Default for IndexedQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> IndexedQueue<T> {
    /// Create an empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            index: 0,
        }
    }

    /// Load new items, resetting the cursor to the start.
    pub fn set_items(&mut self, items: Vec<T>) {
        self.items = items;
        self.index = 0;
    }

    /// Get the current item, if any remain.
    #[must_use]
    pub fn current(&self) -> Option<&T> {
        self.items.get(self.index)
    }

    /// Advance to the next item. Returns `true` if there is a next one.
    pub fn advance(&mut self) -> bool {
        if self.index + 1 < self.items.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    /// Current index in the queue.
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    /// Total number of items.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the queue is empty (no items loaded).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Clear all items and reset the cursor.
    pub fn clear(&mut self) {
        self.items.clear();
        self.index = 0;
    }
}

// ─── Zone Graph (zone-to-zone pathfinding) ───

/// A connection from one zone to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneConnection {
    /// Destination zone ID.
    pub dest_zone_id: u16,
    /// Transfer type (0=zone line, 1=translocator, etc.).
    pub transfer_type: u8,
    /// Whether this connection is disabled (impassable).
    pub disabled: bool,
}

/// A single zone node with its connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneNode {
    /// EQ zone ID.
    pub zone_id: u16,
    /// Zone short name (e.g. "qey2hh1").
    pub name: String,
    /// Minimum recommended level for this zone.
    pub min_level: i32,
    /// Maximum recommended level for this zone.
    pub max_level: i32,
    /// Outgoing connections to adjacent zones.
    pub connections: Vec<ZoneConnection>,
}

/// Complete zone adjacency graph read from EQ's `ZoneGuideManagerClient`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ZoneGraph {
    /// Map of zone ID to zone node.
    pub zones: std::collections::HashMap<u16, ZoneNode>,
}

impl ZoneGraph {
    /// BFS shortest path from one zone to another.
    /// Returns the sequence of zone IDs to traverse (including start and end),
    /// or `None` if no path exists.
    #[must_use]
    pub fn find_path(&self, from_zone_id: u16, to_zone_id: u16) -> Option<Vec<u16>> {
        use std::collections::{HashMap, VecDeque};

        if from_zone_id == to_zone_id {
            return Some(vec![from_zone_id]);
        }
        if !self.zones.contains_key(&from_zone_id) || !self.zones.contains_key(&to_zone_id) {
            return None;
        }

        let mut visited: HashMap<u16, u16> = HashMap::new(); // child -> parent
        let mut queue = VecDeque::new();
        queue.push_back(from_zone_id);
        visited.insert(from_zone_id, from_zone_id);

        while let Some(current) = queue.pop_front() {
            if let Some(node) = self.zones.get(&current) {
                for conn in &node.connections {
                    if conn.disabled {
                        continue;
                    }
                    if visited.contains_key(&conn.dest_zone_id) {
                        continue;
                    }
                    visited.insert(conn.dest_zone_id, current);
                    if conn.dest_zone_id == to_zone_id {
                        // Reconstruct path
                        let mut path = vec![to_zone_id];
                        let mut step = to_zone_id;
                        while step != from_zone_id {
                            step = visited[&step];
                            path.push(step);
                        }
                        path.reverse();
                        return Some(path);
                    }
                    queue.push_back(conn.dest_zone_id);
                }
            }
        }
        None
    }
}

/// A named camp position for a specific role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampSpot {
    /// World position for this camp spot.
    pub position: Waypoint,
    /// Heading to face (EQ degrees, 0-512).
    pub heading: f32,
    /// Role label (e.g., "tank", "healer1", "dps_ranged").
    pub role: String,
}

/// A complete camp definition with spots for each role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampDefinition {
    /// Human-readable camp name (e.g. "LGUK - Live Side").
    pub name: String,
    /// Zone short name where this camp is located.
    pub zone: String,
    /// Positions for each role in the camp.
    pub spots: Vec<CampSpot>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── FollowConfig tests ───

    #[test]
    fn follow_config_new_stores_all_fields() {
        let cfg = FollowConfig::new("Camrene", 15.0, 60.0);
        assert_eq!(cfg.leader_name, "Camrene");
        assert!((cfg.follow_distance - 15.0).abs() < f32::EPSILON);
        assert!((cfg.leash_distance - 60.0).abs() < f32::EPSILON);
    }

    #[test]
    fn follow_config_new_accepts_string_or_str() {
        let cfg1 = FollowConfig::new("Leader", 10.0, 50.0);
        let cfg2 = FollowConfig::new(String::from("Leader"), 10.0, 50.0);
        assert_eq!(cfg1.leader_name, cfg2.leader_name);
    }

    // ─── NavStatus tests ───

    #[test]
    fn nav_status_following_label() {
        let status = NavStatus::Following {
            leader_name: "Camrene".to_string(),
            distance_to_anchor: 25.0,
            returning: false,
        };
        assert_eq!(status.label(), "Following");
    }

    #[test]
    fn nav_status_is_following() {
        let following = NavStatus::Following {
            leader_name: "Test".to_string(),
            distance_to_anchor: 5.0,
            returning: false,
        };
        assert!(following.is_following());
        assert!(!NavStatus::Idle.is_following());
        assert!(!NavStatus::Arrived.is_following());
    }

    #[test]
    fn nav_status_following_is_not_moving_or_stuck() {
        let following = NavStatus::Following {
            leader_name: "Test".to_string(),
            distance_to_anchor: 5.0,
            returning: false,
        };
        assert!(!following.is_moving());
        assert!(!following.is_stuck());
        assert!(!following.is_arrived());
    }

    #[test]
    fn nav_status_following_serialization_roundtrip() {
        let original = NavStatus::Following {
            leader_name: "Leader".to_string(),
            distance_to_anchor: 42.5,
            returning: true,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: NavStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn xorshift32_from_client_id_zero_guards_against_zero_seed() {
        let mut rng = Xorshift32::from_client_id(0);
        // 0 * KNUTH_HASH wraps to 0, but the guard forces seed = 1
        let val = rng.next_u32();
        assert_ne!(val, 0, "zero-seed guard should prevent all-zeros output");
    }

    #[test]
    fn xorshift32_produces_different_values_each_call() {
        let mut rng = Xorshift32::from_client_id(1);
        let a = rng.next_u32();
        let b = rng.next_u32();
        let c = rng.next_u32();
        assert_ne!(a, b);
        assert_ne!(b, c);
    }

    #[test]
    fn xorshift32_next_f32_in_unit_range() {
        let mut rng = Xorshift32::from_client_id(42);
        for _ in 0..1000 {
            let v = rng.next_f32();
            assert!(v >= 0.0, "next_f32 returned {v}, expected >= 0.0");
            assert!(v < 1.0, "next_f32 returned {v}, expected < 1.0");
        }
    }

    #[test]
    fn indexed_queue_empty_by_default() {
        let q: IndexedQueue<i32> = IndexedQueue::default();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
        assert!(q.current().is_none());
    }

    #[test]
    fn indexed_queue_set_items_and_traverse() {
        let mut q = IndexedQueue::new();
        q.set_items(vec![10, 20, 30]);

        assert_eq!(q.len(), 3);
        assert!(!q.is_empty());
        assert_eq!(q.current(), Some(&10));
        assert_eq!(q.index(), 0);

        assert!(q.advance());
        assert_eq!(q.current(), Some(&20));

        assert!(q.advance());
        assert_eq!(q.current(), Some(&30));

        // Cannot advance past the last item
        assert!(!q.advance());
        assert_eq!(q.current(), Some(&30));
    }

    #[test]
    fn indexed_queue_clear_resets_state() {
        let mut q = IndexedQueue::new();
        q.set_items(vec![1, 2, 3]);
        q.advance();
        q.clear();

        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
        assert_eq!(q.index(), 0);
        assert!(q.current().is_none());
    }

    #[test]
    fn indexed_queue_set_items_resets_index() {
        let mut q = IndexedQueue::new();
        q.set_items(vec![1, 2, 3]);
        q.advance();
        q.advance();
        assert_eq!(q.index(), 2);

        q.set_items(vec![100, 200]);
        assert_eq!(q.index(), 0);
        assert_eq!(q.current(), Some(&100));
    }

    #[test]
    fn waypoint_distance_2d() {
        let a = Waypoint::new(0.0, 0.0, 0.0);
        let b = Waypoint::new(3.0, 4.0, 99.0);
        let dist = a.distance_2d(&b);
        assert!((dist - 5.0).abs() < 1e-5);
    }

    #[test]
    fn waypoint_distance_3d() {
        let a = Waypoint::new(0.0, 0.0, 0.0);
        let b = Waypoint::new(1.0, 2.0, 2.0);
        let dist = a.distance_3d(&b);
        assert!((dist - 3.0).abs() < 1e-5);
    }

    // ─── ZoneGraph tests ───

    fn make_test_graph() -> ZoneGraph {
        // A -> B -> C, A -> D -> C (two paths from A to C)
        let mut g = ZoneGraph::default();
        g.zones.insert(
            1,
            ZoneNode {
                zone_id: 1,
                name: "ZoneA".into(),
                min_level: 1,
                max_level: 10,
                connections: vec![
                    ZoneConnection {
                        dest_zone_id: 2,
                        transfer_type: 0,
                        disabled: false,
                    },
                    ZoneConnection {
                        dest_zone_id: 4,
                        transfer_type: 1,
                        disabled: false,
                    },
                ],
            },
        );
        g.zones.insert(
            2,
            ZoneNode {
                zone_id: 2,
                name: "ZoneB".into(),
                min_level: 10,
                max_level: 20,
                connections: vec![ZoneConnection {
                    dest_zone_id: 3,
                    transfer_type: 0,
                    disabled: false,
                }],
            },
        );
        g.zones.insert(
            3,
            ZoneNode {
                zone_id: 3,
                name: "ZoneC".into(),
                min_level: 20,
                max_level: 30,
                connections: vec![],
            },
        );
        g.zones.insert(
            4,
            ZoneNode {
                zone_id: 4,
                name: "ZoneD".into(),
                min_level: 15,
                max_level: 25,
                connections: vec![ZoneConnection {
                    dest_zone_id: 3,
                    transfer_type: 0,
                    disabled: false,
                }],
            },
        );
        g
    }

    #[test]
    fn zone_graph_find_path_same_zone() {
        let g = make_test_graph();
        assert_eq!(g.find_path(1, 1), Some(vec![1]));
    }

    #[test]
    fn zone_graph_find_path_direct() {
        let g = make_test_graph();
        let path = g.find_path(1, 2).unwrap();
        assert_eq!(path, vec![1, 2]);
    }

    #[test]
    fn zone_graph_find_path_multi_hop() {
        let g = make_test_graph();
        let path = g.find_path(1, 3).unwrap();
        // BFS finds shortest — both A->B->C and A->D->C are 2 hops
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], 1);
        assert_eq!(path[2], 3);
    }

    #[test]
    fn zone_graph_find_path_no_path() {
        let g = make_test_graph();
        // Zone C has no outgoing connections, can't reach A from C
        assert!(g.find_path(3, 1).is_none());
    }

    #[test]
    fn zone_graph_find_path_unknown_zone() {
        let g = make_test_graph();
        assert!(g.find_path(1, 999).is_none());
        assert!(g.find_path(999, 1).is_none());
    }

    #[test]
    fn zone_graph_find_path_skips_disabled() {
        let mut g = ZoneGraph::default();
        g.zones.insert(
            1,
            ZoneNode {
                zone_id: 1,
                name: "A".into(),
                min_level: 0,
                max_level: 0,
                connections: vec![ZoneConnection {
                    dest_zone_id: 2,
                    transfer_type: 0,
                    disabled: true,
                }],
            },
        );
        g.zones.insert(
            2,
            ZoneNode {
                zone_id: 2,
                name: "B".into(),
                min_level: 0,
                max_level: 0,
                connections: vec![],
            },
        );
        assert!(g.find_path(1, 2).is_none());
    }

    #[test]
    fn zone_graph_default_is_empty() {
        let g = ZoneGraph::default();
        assert!(g.zones.is_empty());
    }

    // ─── Additional Xorshift32 tests ───

    #[test]
    fn xorshift32_new_with_seed_is_deterministic() {
        let mut a = Xorshift32::new(42);
        let mut b = Xorshift32::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn xorshift32_different_client_ids_produce_different_sequences() {
        let mut a = Xorshift32::from_client_id(1);
        let mut b = Xorshift32::from_client_id(2);
        // At least one of the first 10 values should differ
        let differs = (0..10).any(|_| a.next_u32() != b.next_u32());
        assert!(
            differs,
            "different client IDs should produce different sequences"
        );
    }

    #[test]
    fn xorshift32_next_f32_never_returns_exactly_one() {
        // u32::MAX / u32::MAX as f32 should be < 1.0 due to floating point
        let mut rng = Xorshift32::new(1);
        for _ in 0..10_000 {
            let v = rng.next_f32();
            assert!(v < 1.0, "next_f32 should never return >= 1.0, got {v}");
        }
    }

    // ─── Additional Waypoint tests ───

    #[test]
    fn waypoint_new_sets_coordinates() {
        let wp = Waypoint::new(1.5, -2.5, 3.0);
        assert!((wp.x - 1.5).abs() < f32::EPSILON);
        assert!((wp.y - (-2.5)).abs() < f32::EPSILON);
        assert!((wp.z - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn waypoint_distance_to_self_is_zero() {
        let wp = Waypoint::new(10.0, 20.0, 30.0);
        assert!((wp.distance_2d(&wp)).abs() < f32::EPSILON);
        assert!((wp.distance_3d(&wp)).abs() < f32::EPSILON);
    }

    #[test]
    fn waypoint_distance_2d_ignores_z() {
        let a = Waypoint::new(0.0, 0.0, 0.0);
        let b = Waypoint::new(0.0, 0.0, 1000.0);
        assert!((a.distance_2d(&b)).abs() < f32::EPSILON);
    }

    #[test]
    fn waypoint_distance_3d_includes_z() {
        let a = Waypoint::new(0.0, 0.0, 0.0);
        let b = Waypoint::new(0.0, 0.0, 5.0);
        assert!((a.distance_3d(&b) - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn waypoint_distance_is_symmetric() {
        let a = Waypoint::new(1.0, 2.0, 3.0);
        let b = Waypoint::new(4.0, 5.0, 6.0);
        assert!((a.distance_2d(&b) - b.distance_2d(&a)).abs() < f32::EPSILON);
        assert!((a.distance_3d(&b) - b.distance_3d(&a)).abs() < f32::EPSILON);
    }

    #[test]
    fn waypoint_negative_coordinates() {
        let a = Waypoint::new(-3.0, -4.0, 0.0);
        let b = Waypoint::new(0.0, 0.0, 0.0);
        assert!((a.distance_2d(&b) - 5.0).abs() < f32::EPSILON);
    }

    // ─── Additional IndexedQueue tests ───

    #[test]
    fn indexed_queue_advance_on_empty_returns_false() {
        let mut q: IndexedQueue<i32> = IndexedQueue::new();
        assert!(!q.advance());
        assert_eq!(q.index(), 0);
    }

    #[test]
    fn indexed_queue_single_item() {
        let mut q = IndexedQueue::new();
        q.set_items(vec![42]);
        assert_eq!(q.current(), Some(&42));
        assert!(!q.advance()); // can't advance past single item
        assert_eq!(q.current(), Some(&42));
    }

    #[test]
    fn indexed_queue_set_items_on_non_empty_overwrites() {
        let mut q = IndexedQueue::new();
        q.set_items(vec![1, 2, 3]);
        q.advance();
        assert_eq!(q.current(), Some(&2));

        q.set_items(vec![10, 20]);
        assert_eq!(q.current(), Some(&10));
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn indexed_queue_current_returns_none_after_clear() {
        let mut q = IndexedQueue::new();
        q.set_items(vec![1, 2, 3]);
        q.advance();
        q.clear();
        assert!(q.current().is_none());
        assert!(q.is_empty());
    }

    #[test]
    fn indexed_queue_set_items_with_empty_vec() {
        let mut q = IndexedQueue::new();
        q.set_items(vec![1, 2]);
        q.set_items(Vec::<i32>::new());
        assert!(q.is_empty());
        assert!(q.current().is_none());
    }

    // ─── Additional ZoneGraph tests ───

    #[test]
    fn zone_graph_find_path_both_zones_unknown() {
        let g = ZoneGraph::default();
        assert!(g.find_path(100, 200).is_none());
    }

    #[test]
    fn zone_graph_find_path_same_zone_unknown_returns_self() {
        let g = ZoneGraph::default();
        // Same zone returns immediately even if not in graph (by design)
        assert_eq!(g.find_path(999, 999), Some(vec![999]));
    }

    #[test]
    fn zone_graph_find_path_prefers_shorter_route() {
        // A -> B (direct), A -> C -> D -> B (3 hops)
        let mut g = ZoneGraph::default();
        g.zones.insert(
            1,
            ZoneNode {
                zone_id: 1,
                name: "A".into(),
                min_level: 0,
                max_level: 0,
                connections: vec![
                    ZoneConnection {
                        dest_zone_id: 2,
                        transfer_type: 0,
                        disabled: false,
                    },
                    ZoneConnection {
                        dest_zone_id: 3,
                        transfer_type: 0,
                        disabled: false,
                    },
                ],
            },
        );
        g.zones.insert(
            2,
            ZoneNode {
                zone_id: 2,
                name: "B".into(),
                min_level: 0,
                max_level: 0,
                connections: vec![],
            },
        );
        g.zones.insert(
            3,
            ZoneNode {
                zone_id: 3,
                name: "C".into(),
                min_level: 0,
                max_level: 0,
                connections: vec![ZoneConnection {
                    dest_zone_id: 4,
                    transfer_type: 0,
                    disabled: false,
                }],
            },
        );
        g.zones.insert(
            4,
            ZoneNode {
                zone_id: 4,
                name: "D".into(),
                min_level: 0,
                max_level: 0,
                connections: vec![ZoneConnection {
                    dest_zone_id: 2,
                    transfer_type: 0,
                    disabled: false,
                }],
            },
        );

        let path = g.find_path(1, 2).unwrap();
        assert_eq!(path, vec![1, 2], "BFS should find shortest path");
    }

    #[test]
    fn zone_graph_find_path_routes_around_disabled() {
        // A -> B (disabled), A -> C -> B (enabled)
        let mut g = ZoneGraph::default();
        g.zones.insert(
            1,
            ZoneNode {
                zone_id: 1,
                name: "A".into(),
                min_level: 0,
                max_level: 0,
                connections: vec![
                    ZoneConnection {
                        dest_zone_id: 2,
                        transfer_type: 0,
                        disabled: true,
                    },
                    ZoneConnection {
                        dest_zone_id: 3,
                        transfer_type: 0,
                        disabled: false,
                    },
                ],
            },
        );
        g.zones.insert(
            2,
            ZoneNode {
                zone_id: 2,
                name: "B".into(),
                min_level: 0,
                max_level: 0,
                connections: vec![],
            },
        );
        g.zones.insert(
            3,
            ZoneNode {
                zone_id: 3,
                name: "C".into(),
                min_level: 0,
                max_level: 0,
                connections: vec![ZoneConnection {
                    dest_zone_id: 2,
                    transfer_type: 0,
                    disabled: false,
                }],
            },
        );

        let path = g.find_path(1, 2).unwrap();
        assert_eq!(
            path,
            vec![1, 3, 2],
            "should route around disabled connection"
        );
    }

    #[test]
    fn zone_graph_serialization_roundtrip() {
        let g = make_test_graph();
        let json = serde_json::to_string(&g).expect("serialize");
        let restored: ZoneGraph = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.zones.len(), g.zones.len());
        // Verify path still works
        assert_eq!(restored.find_path(1, 3).unwrap().len(), 3);
    }

    // ─── StickConfig / StickDistance tests ─────────────────────────────────

    #[test]
    fn stick_distance_default_is_default_variant() {
        let d = StickDistance::default();
        assert!(matches!(d, StickDistance::Default));
    }

    #[test]
    fn stick_config_default_has_sensible_values() {
        let cfg = StickConfig::default();
        assert!(matches!(cfg.distance, StickDistance::Default));
        assert!((cfg.distance_mod).abs() < f32::EPSILON);
        assert!(!cfg.hold);
        assert!(!cfg.always);
        assert!(cfg.id.is_none());
    }

    #[test]
    fn nav_status_is_sticking() {
        let s = NavStatus::Sticking {
            target_id: 7,
            distance: 10.0,
            in_range: true,
        };
        assert!(s.is_sticking());
        assert!(!s.is_moving());
        assert!(!s.is_stuck());
        assert!(!s.is_arrived());
        assert_eq!(s.label(), "Sticking");
    }

    #[test]
    fn nav_status_sticking_serialization_roundtrip() {
        let s = NavStatus::Sticking {
            target_id: 42,
            distance: 8.5,
            in_range: false,
        };
        let json = serde_json::to_string(&s).expect("serialize");
        let restored: NavStatus = serde_json::from_str(&json).expect("deserialize");
        if let NavStatus::Sticking {
            target_id,
            distance,
            in_range,
        } = restored
        {
            assert_eq!(target_id, 42);
            assert!((distance - 8.5).abs() < f32::EPSILON);
            assert!(!in_range);
        } else {
            panic!("expected Sticking variant");
        }
    }
}
