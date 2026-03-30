// dmft-common/src/nav.rs
use serde::{Deserialize, Serialize};

/// Knuth multiplicative hash constant for deterministic per-client randomness.
pub const KNUTH_HASH: u32 = 2654435761;

/// Simple xorshift32 PRNG for deterministic per-client randomness.
pub struct Xorshift32 {
    state: u32,
}

impl Xorshift32 {
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    pub fn from_client_id(client_id: u32) -> Self {
        let seed = client_id.wrapping_mul(KNUTH_HASH);
        // Xorshift with seed 0 is a fixed point — every call returns 0 forever.
        Self::new(if seed == 0 { 1 } else { seed })
    }

    pub fn next_u32(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state
    }

    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32) / (u32::MAX as f32)
    }
}

/// A single point in 3D space with optional metadata.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Waypoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Waypoint {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// 2D distance (XY plane) to another waypoint.
    pub fn distance_2d(&self, other: &Waypoint) -> f32 {
        ((other.x - self.x).powi(2) + (other.y - self.y).powi(2)).sqrt()
    }

    /// 3D distance to another waypoint.
    pub fn distance_3d(&self, other: &Waypoint) -> f32 {
        ((other.x - self.x).powi(2) + (other.y - self.y).powi(2) + (other.z - self.z).powi(2))
            .sqrt()
    }
}

/// Current navigation state reported from DLL to orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Stuck and attempting recovery.
    Stuck {
        /// Which recovery attempt this is (1, 2, 3...).
        recovery_attempt: u32,
    },
    /// Arrived at final destination.
    Arrived,
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
    pub fn index(&self) -> usize {
        self.index
    }

    /// Total number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the queue is empty (no items loaded).
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
    pub dest_zone_id: u16,
    pub transfer_type: u8,
    pub disabled: bool,
}

/// A single zone node with its connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneNode {
    pub zone_id: u16,
    pub name: String,
    pub min_level: i32,
    pub max_level: i32,
    pub connections: Vec<ZoneConnection>,
}

/// Complete zone adjacency graph read from EQ's ZoneGuideManagerClient.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ZoneGraph {
    pub zones: std::collections::HashMap<u16, ZoneNode>,
}

impl ZoneGraph {
    /// BFS shortest path from one zone to another.
    /// Returns the sequence of zone IDs to traverse (including start and end),
    /// or `None` if no path exists.
    pub fn find_path(&self, from_zone_id: u16, to_zone_id: u16) -> Option<Vec<u16>> {
        if from_zone_id == to_zone_id {
            return Some(vec![from_zone_id]);
        }
        if !self.zones.contains_key(&from_zone_id) || !self.zones.contains_key(&to_zone_id) {
            return None;
        }

        use std::collections::{HashMap, VecDeque};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampSpot {
    pub position: Waypoint,
    /// Heading to face (EQ degrees, 0-512).
    pub heading: f32,
    /// Role label (e.g., "tank", "healer1", "dps_ranged").
    pub role: String,
}

/// A complete camp definition with spots for each role.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampDefinition {
    pub name: String,
    pub zone: String,
    pub spots: Vec<CampSpot>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
        g.zones.insert(1, ZoneNode {
            zone_id: 1, name: "ZoneA".into(), min_level: 1, max_level: 10,
            connections: vec![
                ZoneConnection { dest_zone_id: 2, transfer_type: 0, disabled: false },
                ZoneConnection { dest_zone_id: 4, transfer_type: 1, disabled: false },
            ],
        });
        g.zones.insert(2, ZoneNode {
            zone_id: 2, name: "ZoneB".into(), min_level: 10, max_level: 20,
            connections: vec![
                ZoneConnection { dest_zone_id: 3, transfer_type: 0, disabled: false },
            ],
        });
        g.zones.insert(3, ZoneNode {
            zone_id: 3, name: "ZoneC".into(), min_level: 20, max_level: 30,
            connections: vec![],
        });
        g.zones.insert(4, ZoneNode {
            zone_id: 4, name: "ZoneD".into(), min_level: 15, max_level: 25,
            connections: vec![
                ZoneConnection { dest_zone_id: 3, transfer_type: 0, disabled: false },
            ],
        });
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
        g.zones.insert(1, ZoneNode {
            zone_id: 1, name: "A".into(), min_level: 0, max_level: 0,
            connections: vec![
                ZoneConnection { dest_zone_id: 2, transfer_type: 0, disabled: true },
            ],
        });
        g.zones.insert(2, ZoneNode {
            zone_id: 2, name: "B".into(), min_level: 0, max_level: 0,
            connections: vec![],
        });
        assert!(g.find_path(1, 2).is_none());
    }

    #[test]
    fn zone_graph_default_is_empty() {
        let g = ZoneGraph::default();
        assert!(g.zones.is_empty());
    }
}
