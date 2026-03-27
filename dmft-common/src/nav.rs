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
        Self::new(client_id.wrapping_mul(KNUTH_HASH))
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

impl<T> IndexedQueue<T> {
    /// Create an empty queue.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            index: 0,
        }
    }

    /// Load new items, resetting the cursor to the start.
    /// Reuses existing allocation when capacity is sufficient.
    pub fn set_items(&mut self, items: Vec<T>) {
        self.items.clear();
        self.items.extend(items);
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
