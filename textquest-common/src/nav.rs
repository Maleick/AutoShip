// textquest-common/src/nav.rs
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
    /// User-initiated pause via `/nav pause`.
    UserPause,
    /// Player keyboard input detected — autopause engaged (#164).
    UserInput,
    /// GM detected nearby — break-on-GM safety halt.
    GmNearby,
}

/// Controls how the navigator writes heading updates to the player.
///
/// Mirrors the MQ2MoveUtils `/nav headsetting` surface:
/// - `True`  — instant memory write to the heading field only.
/// - `Loose` — smooth interpolation toward the target heading, capped to a
///   maximum turn rate per game tick (most human-looking).
/// - `Fast`  — instant memory write to both heading and speed-heading fields
///   (current default; fastest alignment, most responsive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HeadingMode {
    /// Instant heading snap — writes only the facing field.
    True,
    /// Smooth interpolated turn, capped at [`LOOSE_MAX_TURN_PER_TICK`] EQ
    /// heading units per game tick.
    Loose,
    /// Instant heading snap — writes both the facing and speed-heading fields.
    #[default]
    Fast,
}

/// Maximum per-tick heading change applied in [`HeadingMode::Loose`].
///
/// EQ uses a 0–512 heading scale (512 units = 360°).  This constant limits
/// the turn to ≈ 11.25° per tick, giving a visually smooth rotation while
/// still converging within a few ticks for most angles.
pub const LOOSE_MAX_TURN_PER_TICK: f32 = 16.0;

/// Arrival threshold in game units (close enough to "be there").
pub const ARRIVAL_DISTANCE: f32 = 15.0;

/// Calculate heading from current position to target (EQ heading: 0-512, 0=north, increases CW).
///
/// Matches MQ2's formula from MQCommands.cpp `/face`:
///   `atan2(target.x - player.x, target.y - player.y) * 256 / PI`
/// which is equivalent to `atan2(dx, dy) * 256 / PI` mapped to 0..512.
#[must_use]
pub fn calc_heading(from: &Waypoint, to: &Waypoint) -> f32 {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let rad = dx.atan2(dy);
    // 256/PI converts radians to EQ heading units (512 = full circle)
    let heading = rad * 256.0 / std::f32::consts::PI;
    // Normalize to 0..512
    (heading + 512.0) % 512.0
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
    /// Minimum delay (ms) before returning toward the anchor after the leash
    /// is exceeded.  Mirrors MQ2MoveUtils `/makecamp mindelay`.
    #[serde(default)]
    pub min_delay_ms: u32,
    /// Maximum delay (ms) before returning toward the anchor after the leash
    /// is exceeded.  When greater than `min_delay_ms` a random value in
    /// `[min_delay_ms, max_delay_ms]` is chosen.
    /// Mirrors MQ2MoveUtils `/makecamp maxdelay`.
    #[serde(default)]
    pub max_delay_ms: u32,
    /// Suppress camp-return navigation while hostile NPCs are in aggro range.
    /// Mirrors MQ2MoveUtils `/makecamp returnnoaggro`.
    #[serde(default)]
    pub return_no_aggro: bool,
    /// Suppress camp-return navigation while the character is actively looting.
    /// Mirrors MQ2MoveUtils `/makecamp returnnotlooting`.
    #[serde(default)]
    pub return_not_looting: bool,
}

impl FollowConfig {
    /// Create a new follow configuration with the given core distances.
    /// Return policy fields default to disabled (MQ2 defaults).
    #[must_use]
    pub fn new(leader_name: impl Into<String>, follow_distance: f32, leash_distance: f32) -> Self {
        Self {
            leader_name: leader_name.into(),
            follow_distance,
            leash_distance,
            min_delay_ms: 0,
            max_delay_ms: 0,
            return_no_aggro: false,
            return_not_looting: false,
        }
    }
}

impl Default for FollowConfig {
    /// Returns a follow config with MQ2MoveUtils-parity defaults:
    /// follow distance 15 EQ units, leash 75 EQ units, no delays, no aggro/loot gates.
    fn default() -> Self {
        Self::new("", 15.0, 75.0)
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

/// Positional arc mode for `/stick` — controls where the character stands
/// relative to the target's facing direction.
///
/// MQ2MoveUtils equivalents: `behind`, `!front`, `pin`, `front`.
/// Default arc widths match MQ2 defaults; overridable via `behind_arc` / `not_front_arc`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum StickMode {
    /// No arc constraint — stick from any direction (default MQ2 behaviour).
    #[default]
    Any,
    /// Position behind the target (`/stick behind`).
    /// Default arc = 45 degrees (22.5 each side of target's rear).
    Behind,
    /// Stay anywhere except the frontal arc (`/stick !front`).
    /// Default arc = 90 degrees (the excluded frontal cone).
    NotFront,
    /// Strafe to the target's side (`/stick pin`).
    /// Picks left or right flank, whichever is closer.
    Pin,
    /// Position in the frontal arc (`/stick front`).
    /// For tanks who need to face the mob head-on.
    Front,
    /// Snap to the opposite side of the target from the current position (`/stick snaproll`).
    /// Used for instant repositioning during combat (#183).
    SnapRoll,
}

/// Rotation direction for `/circle` kiting mode.
///
/// Maps the MQ2MoveUtils `/circle` modifier syntax:
/// - `clockwise` / `cw`         → `CircleMode::Cw` (default)
/// - `counterclockwise` / `ccw` → `CircleMode::Ccw`
/// - `drunken`                  → `CircleMode::Drunken`
/// - `backward`                 → `CircleMode::Backward`
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum CircleMode {
    /// Clockwise rotation around the center point (default).
    #[default]
    Cw,
    /// Counter-clockwise rotation around the center point.
    Ccw,
    /// Random direction changes at a configurable interval (drunken kiting).
    Drunken,
    /// Move backward while circling (character faces toward center).
    Backward,
}

/// Configuration for a `/circle` kiting session.
///
/// Mirrors the MQ2MoveUtils `/circle` command surface:
/// - `/circle on [radius]`           → start with optional radius
/// - `/circle off`                    → stop circling
/// - `/circle loc Y X`               → circle around specified coordinates
/// - `clockwise` / `cw`              → `mode = CircleMode::Cw`
/// - `counterclockwise` / `ccw`      → `mode = CircleMode::Ccw`
/// - `drunken`                        → `mode = CircleMode::Drunken`
/// - `backward`                       → `mode = CircleMode::Backward`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CircleConfig {
    /// Orbit radius in EQ world units (default: 20.0).
    pub radius: f32,
    /// Rotation direction / style.
    pub mode: CircleMode,
    /// Optional explicit center point.
    ///
    /// When `None`, the player's position at the time `/circle on` is issued
    /// becomes the center.  When `Some`, the character circles that fixed point.
    pub center: Option<Waypoint>,
    /// Optional spawn ID to orbit around.
    ///
    /// When set, the center tracks the target's live position each tick,
    /// enabling active kiting of a moving mob.
    pub target_id: Option<u32>,
    /// Ticks between direction reversals in `Drunken` mode (default: 20).
    pub drunken_interval: u32,
}

impl Default for CircleConfig {
    fn default() -> Self {
        Self {
            radius: 20.0,
            mode: CircleMode::Cw,
            center: None,
            target_id: None,
            drunken_interval: 20,
        }
    }
}

impl CircleConfig {
    /// Create a default config that starts circling the player's current
    /// position at the given radius.
    #[must_use]
    pub fn with_radius(radius: f32) -> Self {
        Self {
            radius,
            ..Self::default()
        }
    }

    /// Create a config that circles a fixed map location.
    #[must_use]
    pub fn at_loc(y: f32, x: f32, z: f32, radius: f32) -> Self {
        Self {
            radius,
            center: Some(Waypoint::new(x, y, z)),
            ..Self::default()
        }
    }
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
/// - `/stick behind` → `mode = StickMode::Behind`
/// - `/stick !front` → `mode = StickMode::NotFront`
/// - `/stick pin`    → `mode = StickMode::Pin`
/// - `/stick front`  → `mode = StickMode::Front`
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
    /// Positional arc mode — controls where to stand relative to target's facing.
    pub mode: StickMode,
    /// Custom arc width for `Behind` mode in degrees (MQ2 default: 45.0).
    /// Valid range: 5.1 to 259.9.
    pub behind_arc: f32,
    /// Custom arc width for `NotFront` mode in degrees (MQ2 default: 90.0).
    /// Defines the frontal cone to exclude. Valid range: 5.1 to 259.9.
    pub not_front_arc: f32,
    /// Back up when the target moves closer than the desired stick distance.
    /// MQ2MoveUtils equivalent: `/stick moveback`.
    pub moveback: bool,
    /// Distance below `effective_distance` at which moveback engages (EQ units).
    /// For example, if `backup_dist` is 5.0 and effective stick distance is 15.0,
    /// moveback triggers when the player is closer than 10.0 units.
    /// MQ2MoveUtils equivalent: `backupdist #`. Default: 5.0.
    pub backup_dist: f32,
    /// Healer stick mode — maintain distance and face target for ranged casting (#163).
    pub healer: bool,
    /// Autopause — pause stick movement on player keyboard input (#164).
    pub autopause: bool,
}

impl Default for StickConfig {
    fn default() -> Self {
        Self {
            distance: StickDistance::Default,
            distance_mod: 0.0,
            hold: false,
            always: false,
            id: None,
            mode: StickMode::Any,
            behind_arc: 45.0,
            not_front_arc: 90.0,
            moveback: false,
            backup_dist: 5.0,
            healer: false,
            autopause: false,
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
    /// Circle-kiting around a fixed or mob-tracked center point.
    Circling {
        /// Orbit radius in EQ world units.
        radius: f32,
        /// Current angle in radians (0 = north, increases clockwise).
        angle: f32,
        /// The active rotation mode.
        mode: CircleMode,
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
            Self::Circling { .. } => "Circling",
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

    /// Returns true if circle-kiting mode is active.
    #[must_use]
    pub fn is_circling(&self) -> bool {
        matches!(self, Self::Circling { .. })
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
    /// Stable classification for route-planning failures.
    pub failure_kind: Option<NavPathFailureKind>,
    /// Whether the caller should attempt a fresh navmesh query later.
    pub replan_recommended: bool,
}

/// Stable route-planning failure classes for operator diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavPathFailureKind {
    /// Required navmesh coverage/data is unavailable for the query.
    DataGap,
    /// A navmesh exists, but the route is temporarily blocked or disconnected.
    TransientBlockage,
}

impl NavPathFailureKind {
    /// Short operator-facing label for the failure class.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DataGap => "data gap",
            Self::TransientBlockage => "transient blockage",
        }
    }
}

impl NavPathMetrics {
    /// Construct metrics for a successful navmesh query.
    #[must_use]
    pub fn success(path_length: Option<f32>) -> Self {
        Self {
            path_exists: true,
            path_length,
            failure_reason: None,
            failure_kind: None,
            replan_recommended: false,
        }
    }

    /// Construct metrics for a failed navmesh query.
    #[must_use]
    pub fn failure(
        reason: impl Into<String>,
        failure_kind: NavPathFailureKind,
        path_length: Option<f32>,
        replan_recommended: bool,
    ) -> Self {
        Self {
            path_exists: false,
            path_length,
            failure_reason: Some(reason.into()),
            failure_kind: Some(failure_kind),
            replan_recommended,
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

/// Per-character scatter offset within a camp — MQ2MoveUtils `/makecamp` scatter parity.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScatterConfig {
    /// Compass bearing from camp center (degrees, 0=N, 90=E, 180=S, 270=W).
    pub bearing: f32,
    /// Base distance from camp center along the bearing.
    pub scatdist: f32,
    /// Randomization radius (uniform disk sampling).
    pub scatsize: f32,
}

impl ScatterConfig {
    #[must_use]
    pub fn new(bearing: f32, scatdist: f32, scatsize: f32) -> Self {
        Self {
            bearing,
            scatdist,
            scatsize,
        }
    }

    #[must_use]
    pub fn offset(&self, center: &Waypoint) -> Waypoint {
        let bearing_rad = self.bearing.to_radians();
        let math_rad = std::f32::consts::FRAC_PI_2 - bearing_rad;
        Waypoint::new(
            center.x + math_rad.cos() * self.scatdist,
            center.y + math_rad.sin() * self.scatdist,
            center.z,
        )
    }

    #[must_use]
    pub fn resolve(&self, center: &Waypoint, angle_seed: f32, dist_seed: f32) -> Waypoint {
        let base = self.offset(center);
        if self.scatsize <= 0.0 {
            return base;
        }
        let r = self.scatsize * dist_seed.sqrt();
        let theta = angle_seed * std::f32::consts::TAU;
        Waypoint::new(base.x + r * theta.cos(), base.y + r * theta.sin(), base.z)
    }
}

/// Steps a heading toward a target value by at most `max_step` units.
///
/// Correctlty handles the 0-512 EQ heading wrap-around, always taking the
/// shortest path.
#[must_use]
pub fn step_toward_heading(current: f32, target: f32, max_step: f32) -> f32 {
    let mut diff = target - current;
    // Normalize diff to [-256, 256] to find the shortest turn
    while diff > 256.0 {
        diff -= 512.0;
    }
    while diff <= -256.0 {
        diff += 512.0;
    }

    if diff.abs() <= max_step {
        target
    } else {
        let step = if diff > 0.0 { max_step } else { -max_step };
        (current + step + 512.0) % 512.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NavCampConfig {
    pub center: Waypoint,
    pub heading: f32,
    pub radius: f32,
    pub scatter: Option<ScatterConfig>,
    pub role: String,
    /// How far beyond the camp radius a character can drift before being
    /// returned.  A value of 1.2 means the leash triggers at 120% of `radius`.
    pub leash_factor: f32,
    /// Minimum delay (ms) before returning to camp after arrival (#182).
    pub min_delay_ms: u32,
    /// Maximum delay (ms) before returning to camp after arrival (#182).
    pub max_delay_ms: u32,
    /// Only return to camp when no aggro is detected (#182).
    pub return_no_aggro: bool,
    /// Only return to camp when not looting (#182).
    pub return_not_looting: bool,
    /// Autopause — pause camp return on player keyboard input (#164).
    pub autopause: bool,
}

impl NavCampConfig {
    #[must_use]
    pub fn return_position(&self) -> Waypoint {
        match self.scatter {
            Some(ref scatter) => scatter.offset(&self.center),
            None => self.center,
        }
    }

    #[must_use]
    pub fn return_position_randomized(&self, angle_seed: f32, dist_seed: f32) -> Waypoint {
        match self.scatter {
            Some(ref scatter) => scatter.resolve(&self.center, angle_seed, dist_seed),
            None => self.center,
        }
    }

    #[must_use]
    pub fn is_outside_radius(&self, pos: &Waypoint) -> bool {
        self.center.distance_2d(pos) > self.radius
    }

    /// Returns `true` when the position exceeds the leash boundary
    /// (`radius * leash_factor`).
    #[must_use]
    pub fn is_beyond_leash(&self, pos: &Waypoint) -> bool {
        self.center.distance_2d(pos) > self.radius * self.leash_factor
    }

    #[must_use]
    pub fn to_camp_spot(&self) -> CampSpot {
        CampSpot {
            position: self.return_position(),
            heading: self.heading,
            role: self.role.clone(),
        }
    }
}

/// A named waypoint that can be saved and recalled via `/nav waypoint`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedWaypoint {
    /// User-assigned name (e.g. "camp1", "puller_spot").
    pub name: String,
    /// World position.
    pub position: Waypoint,
    /// Zone short name where this waypoint was recorded.
    pub zone: String,
}

impl NamedWaypoint {
    #[must_use]
    pub fn new(name: impl Into<String>, position: Waypoint, zone: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            position,
            zone: zone.into(),
        }
    }
}

/// Axis constraint for `/moveto` arrival detection (#135).
///
/// Controls which spatial axes are used when measuring distance to the
/// destination for the arrival check.
///
/// - `Both`  — 2D XY distance (default, same as MQ2MoveUtils standard).
/// - `X`     — only the X-axis separation matters for arrival.
/// - `Y`     — only the Y-axis separation matters for arrival.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ArrivalAxis {
    /// Arrive when the 2D (XY) distance is within the threshold (default).
    #[default]
    Both,
    /// Arrive when only the X-axis separation is within the threshold.
    X,
    /// Arrive when only the Y-axis separation is within the threshold.
    Y,
}

/// Configuration for advanced `/moveto` commands (#184).
///
/// Supports MQ2MoveUtils options: moveto by spawn ID, xloc/yloc,
/// break-on-aggro, break-on-hit, use-walk, use-back.
/// Also supports distance and axis arrival controls (#135).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveToConfig {
    /// Destination waypoint (from xloc/yloc or spawn position).
    pub destination: Waypoint,
    /// Optional spawn ID to track (moveto id #). When set, destination updates
    /// each tick to the target's current position.
    pub target_id: Option<u32>,
    /// Stop navigation if aggro is detected (nearby hostile NPC moving toward player).
    pub break_on_aggro: bool,
    /// Stop navigation if the tracked target warps unexpectedly.
    #[serde(default)]
    pub break_on_warp: bool,
    /// Pause navigation until the tracked target stabilizes after a warp.
    #[serde(default)]
    pub pause_on_warp: bool,
    /// Stop navigation if the player is summoned or otherwise displaced unexpectedly.
    #[serde(default)]
    pub break_on_summon: bool,
    /// Stop navigation if the player takes damage.
    pub break_on_hit: bool,
    /// Use walk speed instead of run.
    pub use_walk: bool,
    /// Move backward toward the destination instead of turning and running forward.
    pub use_back: bool,
    /// Autopause — pause movement on player keyboard input.
    pub autopause: bool,
    /// Custom arrival distance threshold in EQ units (`/moveto dist #`).
    ///
    /// When `None`, the DLL's default `ARRIVAL_DISTANCE` constant is used.
    #[serde(default)]
    pub dist: Option<f32>,
    /// Axis constraint for arrival detection (`/moveto xloc`/`yloc` beeline mode).
    ///
    /// Defaults to `ArrivalAxis::Both` (standard 2D distance check).
    #[serde(default)]
    pub axis: ArrivalAxis,
}

impl Default for MoveToConfig {
    fn default() -> Self {
        Self {
            destination: Waypoint::new(0.0, 0.0, 0.0),
            target_id: None,
            break_on_aggro: false,
            break_on_warp: false,
            pause_on_warp: false,
            break_on_summon: false,
            break_on_hit: false,
            use_walk: false,
            use_back: false,
            autopause: false,
            dist: None,
            axis: ArrivalAxis::Both,
        }
    }
}

/// Navigation diagnostics snapshot — returned by `/nav ui` for debug overlay.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NavDiagnostics {
    /// Current navigation state label (e.g. "Moving", "Idle").
    pub state: String,
    /// Whether a navmesh is loaded for the current zone.
    pub mesh_loaded: bool,
    /// Whether a valid path exists to the current destination.
    pub path_exists: bool,
    /// Total path length in world units, if computed.
    pub path_length: Option<f32>,
    /// Current velocity in world units per second.
    pub velocity: f32,
    /// Current waypoint index / total.
    pub waypoint_index: usize,
    /// Total waypoints in the current path.
    pub waypoint_count: usize,
    /// Distance remaining to the current waypoint.
    pub distance_remaining: f32,
}

/// Navigation state signals — a compact boolean/metric summary for TLO-style queries.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NavStateSignals {
    /// True if the navigator is actively processing (Moving, Following, Sticking).
    pub active: bool,
    /// True if a navmesh is loaded for the current zone.
    pub mesh_loaded: bool,
    /// True if a valid path exists to the current destination.
    pub path_exists: bool,
    /// Total path length in world units, if computed.
    pub path_length: Option<f32>,
    /// Current velocity in world units per second.
    pub velocity: f32,
    /// True if navigation is paused (user or warp).
    pub paused: bool,
}

impl MoveToConfig {
    /// Create a moveto config for a static position.
    #[must_use]
    pub fn to_position(x: f32, y: f32, z: f32) -> Self {
        Self {
            destination: Waypoint::new(x, y, z),
            ..Self::default()
        }
    }

    /// Create a moveto config for a spawn ID.
    #[must_use]
    pub fn to_spawn(spawn_id: u32, current_pos: Waypoint) -> Self {
        Self {
            destination: current_pos,
            target_id: Some(spawn_id),
            ..Self::default()
        }
    }

    /// Compute the distance from `current` to `destination` according to `self.axis`.
    ///
    /// - `ArrivalAxis::Both` — standard 2D XY distance.
    /// - `ArrivalAxis::X`   — absolute X-axis separation only.
    /// - `ArrivalAxis::Y`   — absolute Y-axis separation only.
    #[must_use]
    pub fn axis_distance(&self, current: &Waypoint, destination: &Waypoint) -> f32 {
        match self.axis {
            ArrivalAxis::Both => current.distance_2d(destination),
            ArrivalAxis::X => (current.x - destination.x).abs(),
            ArrivalAxis::Y => (current.y - destination.y).abs(),
        }
    }

    /// Return the arrival distance threshold, applying the `dist` override when set.
    ///
    /// Falls back to `default_arrival_distance` (the DLL's `ARRIVAL_DISTANCE` constant)
    /// when no explicit `dist` was configured.
    #[must_use]
    pub fn effective_arrival_distance(&self, default_arrival_distance: f32) -> f32 {
        self.dist.unwrap_or(default_arrival_distance)
    }
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
        assert_eq!(cfg.min_delay_ms, 0);
        assert_eq!(cfg.max_delay_ms, 0);
        assert!(!cfg.return_no_aggro);
        assert!(!cfg.return_not_looting);
    }

    #[test]
    fn follow_config_new_accepts_string_or_str() {
        let cfg1 = FollowConfig::new("Leader", 10.0, 50.0);
        let cfg2 = FollowConfig::new(String::from("Leader"), 10.0, 50.0);
        assert_eq!(cfg1.leader_name, cfg2.leader_name);
    }

    #[test]
    fn follow_config_default_has_mq2_parity_distances() {
        let cfg = FollowConfig::default();
        assert!((cfg.follow_distance - 15.0).abs() < f32::EPSILON);
        assert!((cfg.leash_distance - 75.0).abs() < f32::EPSILON);
        assert!(!cfg.return_no_aggro);
        assert!(!cfg.return_not_looting);
    }

    #[test]
    fn follow_config_return_policy_fields_roundtrip() {
        let mut cfg = FollowConfig::new("Tank", 20.0, 100.0);
        cfg.min_delay_ms = 500;
        cfg.max_delay_ms = 2000;
        cfg.return_no_aggro = true;
        cfg.return_not_looting = true;
        let json = serde_json::to_string(&cfg).expect("serialize");
        let restored: FollowConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.min_delay_ms, 500);
        assert_eq!(restored.max_delay_ms, 2000);
        assert!(restored.return_no_aggro);
        assert!(restored.return_not_looting);
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
        assert!(!cfg.moveback);
        assert!((cfg.backup_dist - 5.0).abs() < f32::EPSILON);
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

    #[test]
    fn scatter_offset_north() {
        let center = Waypoint::new(100.0, 100.0, 0.0);
        let scatter = ScatterConfig::new(0.0, 20.0, 0.0);
        let pos = scatter.offset(&center);
        assert!((pos.x - 100.0).abs() < 0.1);
        assert!((pos.y - 120.0).abs() < 0.1);
    }

    #[test]
    fn scatter_offset_east() {
        let center = Waypoint::new(100.0, 100.0, 0.0);
        let scatter = ScatterConfig::new(90.0, 20.0, 0.0);
        let pos = scatter.offset(&center);
        assert!((pos.x - 120.0).abs() < 0.1);
        assert!((pos.y - 100.0).abs() < 0.1);
    }

    #[test]
    fn scatter_offset_south() {
        let center = Waypoint::new(100.0, 100.0, 0.0);
        let scatter = ScatterConfig::new(180.0, 20.0, 0.0);
        let pos = scatter.offset(&center);
        assert!((pos.x - 100.0).abs() < 0.1);
        assert!((pos.y - 80.0).abs() < 0.1);
    }

    #[test]
    fn scatter_offset_west() {
        let center = Waypoint::new(100.0, 100.0, 0.0);
        let scatter = ScatterConfig::new(270.0, 20.0, 0.0);
        let pos = scatter.offset(&center);
        assert!((pos.x - 80.0).abs() < 0.1);
        assert!((pos.y - 100.0).abs() < 0.1);
    }

    #[test]
    fn scatter_resolve_no_size_equals_offset() {
        let center = Waypoint::new(50.0, 50.0, 0.0);
        let scatter = ScatterConfig::new(45.0, 10.0, 0.0);
        let offset = scatter.offset(&center);
        let resolved = scatter.resolve(&center, 0.5, 0.5);
        assert!((offset.x - resolved.x).abs() < f32::EPSILON);
        assert!((offset.y - resolved.y).abs() < f32::EPSILON);
    }

    #[test]
    fn scatter_resolve_within_scatsize() {
        let center = Waypoint::new(0.0, 0.0, 0.0);
        let scatter = ScatterConfig::new(0.0, 20.0, 5.0);
        let base = scatter.offset(&center);
        for i in 0..10 {
            let a = (i as f32) / 10.0;
            let d = (i as f32) / 10.0;
            let pos = scatter.resolve(&center, a, d);
            let dist = ((pos.x - base.x).powi(2) + (pos.y - base.y).powi(2)).sqrt();
            assert!(dist <= 5.0 + 0.01, "dist={dist} exceeded scatsize");
        }
    }

    #[test]
    fn scatter_uniform_disk_avoids_center_bias() {
        let center = Waypoint::new(0.0, 0.0, 0.0);
        let scatter = ScatterConfig::new(90.0, 10.0, 5.0);
        let base = scatter.offset(&center);
        let pos = scatter.resolve(&center, 0.0, 0.25);
        let dist = ((pos.x - base.x).powi(2) + (pos.y - base.y).powi(2)).sqrt();
        assert!((dist - 2.5).abs() < 0.1);
    }

    #[test]
    fn camp_config_return_position_without_scatter() {
        let config = NavCampConfig {
            center: Waypoint::new(100.0, 200.0, 0.0),
            heading: 128.0,
            radius: 50.0,
            scatter: None,
            role: "tank".to_string(),
            leash_factor: 1.2,
            min_delay_ms: 0,
            max_delay_ms: 0,
            return_no_aggro: false,
            return_not_looting: false,
            autopause: false,
        };
        let pos = config.return_position();
        assert!((pos.x - 100.0).abs() < f32::EPSILON);
        assert!((pos.y - 200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn camp_config_return_position_with_scatter() {
        let config = NavCampConfig {
            center: Waypoint::new(100.0, 200.0, 0.0),
            heading: 128.0,
            radius: 50.0,
            scatter: Some(ScatterConfig::new(90.0, 15.0, 0.0)),
            role: "dps".to_string(),
            leash_factor: 1.2,
            min_delay_ms: 0,
            max_delay_ms: 0,
            return_no_aggro: false,
            return_not_looting: false,
            autopause: false,
        };
        let pos = config.return_position();
        assert!((pos.x - 115.0).abs() < 0.1);
        assert!((pos.y - 200.0).abs() < 0.1);
    }

    #[test]
    fn camp_config_is_outside_radius() {
        let config = NavCampConfig {
            center: Waypoint::new(0.0, 0.0, 0.0),
            heading: 0.0,
            radius: 50.0,
            scatter: None,
            role: "healer".to_string(),
            leash_factor: 1.2,
            min_delay_ms: 0,
            max_delay_ms: 0,
            return_no_aggro: false,
            return_not_looting: false,
            autopause: false,
        };
        assert!(!config.is_outside_radius(&Waypoint::new(30.0, 30.0, 0.0)));
        assert!(config.is_outside_radius(&Waypoint::new(40.0, 40.0, 0.0)));
    }

    #[test]
    fn camp_config_to_camp_spot_uses_scatter() {
        let config = NavCampConfig {
            center: Waypoint::new(100.0, 100.0, 0.0),
            heading: 256.0,
            radius: 60.0,
            scatter: Some(ScatterConfig::new(0.0, 10.0, 0.0)),
            role: "bard".to_string(),
            leash_factor: 1.2,
            min_delay_ms: 0,
            max_delay_ms: 0,
            return_no_aggro: false,
            return_not_looting: false,
            autopause: false,
        };
        let spot = config.to_camp_spot();
        assert_eq!(spot.role, "bard");
        assert!((spot.heading - 256.0).abs() < f32::EPSILON);
        assert!((spot.position.y - 110.0).abs() < 0.1);
    }

    #[test]
    fn camp_config_serde_roundtrip() {
        let config = NavCampConfig {
            center: Waypoint::new(50.0, 75.0, 10.0),
            heading: 384.0,
            radius: 80.0,
            scatter: Some(ScatterConfig::new(45.0, 12.0, 3.0)),
            role: "monk".to_string(),
            leash_factor: 1.2,
            min_delay_ms: 0,
            max_delay_ms: 0,
            return_no_aggro: false,
            return_not_looting: false,
            autopause: false,
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: NavCampConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, restored);
    }

    // ─── Leash boundary tests ───

    #[test]
    fn leash_within_boundary_returns_false() {
        let config = NavCampConfig {
            center: Waypoint::new(0.0, 0.0, 0.0),
            heading: 0.0,
            radius: 50.0,
            scatter: None,
            role: "tank".to_string(),
            leash_factor: 1.2,
            min_delay_ms: 0,
            max_delay_ms: 0,
            return_no_aggro: false,
            return_not_looting: false,
            autopause: false,
        };
        assert!(!config.is_beyond_leash(&Waypoint::new(55.0, 0.0, 0.0)));
    }

    #[test]
    fn leash_beyond_boundary_returns_true() {
        let config = NavCampConfig {
            center: Waypoint::new(0.0, 0.0, 0.0),
            heading: 0.0,
            radius: 50.0,
            scatter: None,
            role: "tank".to_string(),
            leash_factor: 1.2,
            min_delay_ms: 0,
            max_delay_ms: 0,
            return_no_aggro: false,
            return_not_looting: false,
            autopause: false,
        };
        assert!(config.is_beyond_leash(&Waypoint::new(61.0, 0.0, 0.0)));
    }

    #[test]
    fn leash_at_exact_boundary_returns_false() {
        let config = NavCampConfig {
            center: Waypoint::new(0.0, 0.0, 0.0),
            heading: 0.0,
            radius: 50.0,
            scatter: None,
            role: "healer".to_string(),
            leash_factor: 1.2,
            min_delay_ms: 0,
            max_delay_ms: 0,
            return_no_aggro: false,
            return_not_looting: false,
            autopause: false,
        };
        assert!(!config.is_beyond_leash(&Waypoint::new(60.0, 0.0, 0.0)));
    }

    #[test]
    fn leash_factor_one_equals_radius() {
        let config = NavCampConfig {
            center: Waypoint::new(0.0, 0.0, 0.0),
            heading: 0.0,
            radius: 50.0,
            scatter: None,
            role: "dps".to_string(),
            leash_factor: 1.0,
            min_delay_ms: 0,
            max_delay_ms: 0,
            return_no_aggro: false,
            return_not_looting: false,
            autopause: false,
        };
        assert!(config.is_beyond_leash(&Waypoint::new(50.1, 0.0, 0.0)));
        assert!(!config.is_beyond_leash(&Waypoint::new(49.9, 0.0, 0.0)));
    }

    // ─── PauseReason tests (#168) ───

    #[test]
    fn pause_reason_user_pause_serialization_roundtrip() {
        let reason = PauseReason::UserPause;
        let json = serde_json::to_string(&reason).expect("serialize");
        let restored: PauseReason = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, PauseReason::UserPause);
    }

    // ─── NamedWaypoint tests (#175) ───

    #[test]
    fn named_waypoint_new() {
        let wp = NamedWaypoint::new("camp1", Waypoint::new(1.0, 2.0, 3.0), "qey2hh1");
        assert_eq!(wp.name, "camp1");
        assert_eq!(wp.zone, "qey2hh1");
        assert!((wp.position.x - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn named_waypoint_serde_roundtrip() {
        let wp = NamedWaypoint::new("puller", Waypoint::new(10.0, 20.0, 30.0), "gukbottom");
        let json = serde_json::to_string(&wp).expect("serialize");
        let restored: NamedWaypoint = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(wp, restored);
    }

    // ─── NavDiagnostics tests (#177) ───

    #[test]
    fn nav_diagnostics_default() {
        let diag = NavDiagnostics::default();
        assert_eq!(diag.state, "");
        assert!(!diag.mesh_loaded);
        assert!(!diag.path_exists);
        assert!(diag.path_length.is_none());
        assert!(diag.velocity.abs() < f32::EPSILON);
    }

    #[test]
    fn nav_path_metrics_success_clears_failure_metadata() {
        let metrics = NavPathMetrics::success(Some(42.0));
        assert!(metrics.path_exists);
        assert_eq!(metrics.path_length, Some(42.0));
        assert_eq!(metrics.failure_reason, None);
        assert_eq!(metrics.failure_kind, None);
        assert!(!metrics.replan_recommended);
    }

    #[test]
    fn nav_path_metrics_failure_tracks_kind_and_replan_state() {
        let metrics = NavPathMetrics::failure(
            "No path found between start and end",
            NavPathFailureKind::TransientBlockage,
            None,
            true,
        );
        assert!(!metrics.path_exists);
        assert_eq!(metrics.path_length, None);
        assert_eq!(
            metrics.failure_reason.as_deref(),
            Some("No path found between start and end")
        );
        assert_eq!(
            metrics.failure_kind,
            Some(NavPathFailureKind::TransientBlockage)
        );
        assert!(metrics.replan_recommended);
    }

    // ─── NavStateSignals tests (#176) ───

    #[test]
    fn nav_state_signals_default() {
        let signals = NavStateSignals::default();
        assert!(!signals.active);
        assert!(!signals.mesh_loaded);
        assert!(!signals.path_exists);
        assert!(signals.path_length.is_none());
        assert!(signals.velocity.abs() < f32::EPSILON);
        assert!(!signals.paused);
    }

    #[test]
    fn nav_state_signals_serde_roundtrip() {
        let signals = NavStateSignals {
            active: true,
            mesh_loaded: true,
            path_exists: true,
            path_length: Some(150.0),
            velocity: 12.5,
            paused: false,
        };
        let json = serde_json::to_string(&signals).expect("serialize");
        let restored: NavStateSignals = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(signals, restored);
    }

    // ─── MoveToConfig tests (#184) ───

    #[test]
    fn moveto_config_default() {
        let config = MoveToConfig::default();
        assert!(config.target_id.is_none());
        assert!(!config.break_on_aggro);
        assert!(!config.break_on_warp);
        assert!(!config.pause_on_warp);
        assert!(!config.break_on_summon);
        assert!(!config.break_on_hit);
        assert!(!config.use_walk);
        assert!(!config.use_back);
        assert!(!config.autopause);
        assert!(config.dist.is_none());
        assert_eq!(config.axis, ArrivalAxis::Both);
    }

    #[test]
    fn moveto_config_to_position() {
        let config = MoveToConfig::to_position(100.0, 200.0, 0.0);
        assert!((config.destination.x - 100.0).abs() < f32::EPSILON);
        assert!((config.destination.y - 200.0).abs() < f32::EPSILON);
        assert!(config.target_id.is_none());
    }

    #[test]
    fn moveto_config_to_spawn() {
        let config = MoveToConfig::to_spawn(42, Waypoint::new(10.0, 20.0, 0.0));
        assert_eq!(config.target_id, Some(42));
        assert!((config.destination.x - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn moveto_config_serde_roundtrip() {
        let config = MoveToConfig {
            destination: Waypoint::new(50.0, 75.0, 10.0),
            target_id: Some(99),
            break_on_aggro: true,
            break_on_warp: true,
            pause_on_warp: false,
            break_on_summon: true,
            break_on_hit: false,
            use_walk: true,
            use_back: false,
            autopause: true,
            dist: Some(8.0),
            axis: ArrivalAxis::X,
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: MoveToConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, restored);
    }

    #[test]
    fn moveto_config_axis_distance_both() {
        let config = MoveToConfig {
            axis: ArrivalAxis::Both,
            ..MoveToConfig::default()
        };
        let a = Waypoint::new(0.0, 0.0, 0.0);
        let b = Waypoint::new(3.0, 4.0, 0.0);
        let d = config.axis_distance(&a, &b);
        assert!((d - 5.0).abs() < 0.001, "expected 5.0, got {d}");
    }

    #[test]
    fn moveto_config_axis_distance_x_only() {
        let config = MoveToConfig {
            axis: ArrivalAxis::X,
            ..MoveToConfig::default()
        };
        let a = Waypoint::new(10.0, 0.0, 0.0);
        let b = Waypoint::new(13.0, 999.0, 0.0);
        let d = config.axis_distance(&a, &b);
        assert!((d - 3.0).abs() < f32::EPSILON, "expected 3.0, got {d}");
    }

    #[test]
    fn moveto_config_axis_distance_y_only() {
        let config = MoveToConfig {
            axis: ArrivalAxis::Y,
            ..MoveToConfig::default()
        };
        let a = Waypoint::new(999.0, 5.0, 0.0);
        let b = Waypoint::new(0.0, 12.0, 0.0);
        let d = config.axis_distance(&a, &b);
        assert!((d - 7.0).abs() < f32::EPSILON, "expected 7.0, got {d}");
    }

    #[test]
    fn moveto_config_effective_arrival_distance_uses_override() {
        let config = MoveToConfig {
            dist: Some(5.0),
            ..MoveToConfig::default()
        };
        assert!((config.effective_arrival_distance(15.0) - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn moveto_config_effective_arrival_distance_falls_back_to_default() {
        let config = MoveToConfig::default();
        assert!((config.effective_arrival_distance(15.0) - 15.0).abs() < f32::EPSILON);
    }

    #[test]
    fn moveto_config_serde_backward_compat_missing_dist_axis() {
        // JSON without dist/axis fields should deserialize without error using serde defaults.
        let json = r#"{"destination":{"x":1.0,"y":2.0,"z":0.0},"target_id":null,"break_on_aggro":false,"break_on_warp":false,"pause_on_warp":false,"break_on_summon":false,"break_on_hit":false,"use_walk":false,"use_back":false,"autopause":false}"#;
        let config: MoveToConfig = serde_json::from_str(json).expect("deserialize legacy JSON");
        assert!(config.dist.is_none());
        assert_eq!(config.axis, ArrivalAxis::Both);
    }

    // ─── New variant tests ───

    #[test]
    fn camp_config_with_return_conditions() {
        let config = NavCampConfig {
            center: Waypoint::new(0.0, 0.0, 0.0),
            heading: 0.0,
            radius: 50.0,
            scatter: None,
            role: "tank".to_string(),
            leash_factor: 1.2,
            min_delay_ms: 500,
            max_delay_ms: 2000,
            return_no_aggro: true,
            return_not_looting: true,
            autopause: false,
        };
        assert_eq!(config.min_delay_ms, 500);
        assert_eq!(config.max_delay_ms, 2000);
        assert!(config.return_no_aggro);
        assert!(config.return_not_looting);
    }

    #[test]
    fn stick_config_healer_default_false() {
        let config = StickConfig::default();
        assert!(!config.healer);
        assert!(!config.autopause);
    }

    #[test]
    fn stick_mode_snaproll_serde() {
        let mode = StickMode::SnapRoll;
        let json = serde_json::to_string(&mode).expect("serialize");
        let restored: StickMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(mode, restored);
    }

    #[test]
    fn pause_reason_user_input_serde() {
        let reason = PauseReason::UserInput;
        let json = serde_json::to_string(&reason).expect("serialize");
        let restored: PauseReason = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(reason, restored);
    }

    #[test]
    fn pause_reason_gm_nearby_serde() {
        let reason = PauseReason::GmNearby;
        let json = serde_json::to_string(&reason).expect("serialize");
        let restored: PauseReason = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(reason, restored);
    }

    // ─── calc_heading tests ──────────────────────────────────────────────

    #[test]
    fn calc_heading_north() {
        // Target is directly "north" (+Y from origin)
        let from = Waypoint::new(0.0, 0.0, 0.0);
        let to = Waypoint::new(0.0, 100.0, 0.0);
        let heading = calc_heading(&from, &to);
        assert!(
            heading.abs() < 0.1 || (heading - 512.0).abs() < 0.1,
            "north heading should be ~0, got {heading}"
        );
    }

    #[test]
    fn calc_heading_east() {
        // Target is directly "east" (+X from origin)
        let from = Waypoint::new(0.0, 0.0, 0.0);
        let to = Waypoint::new(100.0, 0.0, 0.0);
        let heading = calc_heading(&from, &to);
        assert!(
            (heading - 128.0).abs() < 0.1,
            "east heading should be ~128, got {heading}"
        );
    }

    #[test]
    fn calc_heading_south() {
        // Target is directly "south" (-Y from origin)
        let from = Waypoint::new(0.0, 0.0, 0.0);
        let to = Waypoint::new(0.0, -100.0, 0.0);
        let heading = calc_heading(&from, &to);
        assert!(
            (heading - 256.0).abs() < 0.1,
            "south heading should be ~256, got {heading}"
        );
    }

    #[test]
    fn calc_heading_west() {
        // Target is directly "west" (-X from origin)
        let from = Waypoint::new(0.0, 0.0, 0.0);
        let to = Waypoint::new(-100.0, 0.0, 0.0);
        let heading = calc_heading(&from, &to);
        assert!(
            (heading - 384.0).abs() < 0.1,
            "west heading should be ~384, got {heading}"
        );
    }

    #[test]
    fn calc_heading_northeast() {
        let from = Waypoint::new(0.0, 0.0, 0.0);
        let to = Waypoint::new(100.0, 100.0, 0.0);
        let heading = calc_heading(&from, &to);
        assert!(
            (heading - 64.0).abs() < 0.1,
            "NE heading should be ~64, got {heading}"
        );
    }

    #[test]
    fn calc_heading_southeast() {
        let from = Waypoint::new(0.0, 0.0, 0.0);
        let to = Waypoint::new(100.0, -100.0, 0.0);
        let heading = calc_heading(&from, &to);
        assert!(
            (heading - 192.0).abs() < 0.1,
            "SE heading should be ~192, got {heading}"
        );
    }

    #[test]
    fn calc_heading_southwest() {
        let from = Waypoint::new(0.0, 0.0, 0.0);
        let to = Waypoint::new(-100.0, -100.0, 0.0);
        let heading = calc_heading(&from, &to);
        assert!(
            (heading - 320.0).abs() < 0.1,
            "SW heading should be ~320, got {heading}"
        );
    }

    #[test]
    fn calc_heading_northwest() {
        let from = Waypoint::new(0.0, 0.0, 0.0);
        let to = Waypoint::new(-100.0, 100.0, 0.0);
        let heading = calc_heading(&from, &to);
        assert!(
            (heading - 448.0).abs() < 0.1,
            "NW heading should be ~448, got {heading}"
        );
    }

    #[test]
    fn calc_heading_result_always_in_range() {
        // Test many angles to ensure result is always in [0, 512)
        let from = Waypoint::new(100.0, 200.0, 0.0);
        for angle_deg in (0..360).step_by(5) {
            let rad = (angle_deg as f32).to_radians();
            let to = Waypoint::new(from.x + rad.cos() * 50.0, from.y + rad.sin() * 50.0, 0.0);
            let heading = calc_heading(&from, &to);
            assert!(
                (0.0..512.0).contains(&heading),
                "heading {heading} out of range for angle {angle_deg}°"
            );
        }
    }

    #[test]
    fn calc_heading_same_position() {
        // Edge case: from == to. atan2(0,0) is 0 on most platforms.
        let pos = Waypoint::new(10.0, 20.0, 30.0);
        let heading = calc_heading(&pos, &pos);
        assert!(
            heading.is_finite(),
            "heading should be finite for zero-distance"
        );
    }

    #[test]
    fn calc_heading_ignores_z() {
        // Z difference should not affect heading
        let from = Waypoint::new(0.0, 0.0, 0.0);
        let to_flat = Waypoint::new(100.0, 0.0, 0.0);
        let to_elevated = Waypoint::new(100.0, 0.0, 500.0);
        let h1 = calc_heading(&from, &to_flat);
        let h2 = calc_heading(&from, &to_elevated);
        assert!(
            (h1 - h2).abs() < f32::EPSILON,
            "Z should not affect heading"
        );
    }

    #[test]
    fn calc_heading_symmetry() {
        // Heading from A→B and B→A should differ by ~256 (opposite directions)
        let a = Waypoint::new(0.0, 0.0, 0.0);
        let b = Waypoint::new(100.0, 50.0, 0.0);
        let h_ab = calc_heading(&a, &b);
        let h_ba = calc_heading(&b, &a);
        let diff = (h_ab - h_ba).abs();
        assert!(
            (diff - 256.0).abs() < 0.1,
            "opposite headings should differ by ~256, got diff={diff}"
        );
    }

    // ─── ARRIVAL_DISTANCE const ──────────────────────────────────────────

    #[test]
    fn arrival_distance_is_positive() {
        const _: () = assert!(ARRIVAL_DISTANCE > 0.0);
    }

    // ─── step_toward_heading tests ──────────────────────────────────────

    #[test]
    fn step_toward_heading_small_positive_delta() {
        // target slightly ahead — should arrive in one step
        let result = step_toward_heading(100.0, 110.0, 16.0);
        assert!((result - 110.0).abs() < f32::EPSILON);
    }

    #[test]
    fn step_toward_heading_small_negative_delta() {
        let result = step_toward_heading(100.0, 90.0, 16.0);
        assert!((result - 90.0).abs() < f32::EPSILON);
    }

    #[test]
    fn step_toward_heading_large_positive_delta_clamped() {
        // 60 degrees apart, max step 16 → should only step 16
        let result = step_toward_heading(100.0, 160.0, 16.0);
        assert!((result - 116.0).abs() < f32::EPSILON);
    }

    #[test]
    fn step_toward_heading_large_negative_delta_clamped() {
        let result = step_toward_heading(160.0, 100.0, 16.0);
        assert!((result - 144.0).abs() < f32::EPSILON);
    }

    #[test]
    fn step_toward_heading_wraps_around_zero_boundary() {
        // From 500 toward 10 — shortest path crosses the 0/512 boundary
        let result = step_toward_heading(500.0, 10.0, 16.0);
        // diff = 10 - 500 = -490 → normalize → -490 + 512 = 22 → step = +16
        assert!((result - 4.0).abs() < f32::EPSILON, "got {result}");
    }

    #[test]
    fn step_toward_heading_wraps_around_reverse() {
        // From 10 toward 500 — shortest path is CCW
        let result = step_toward_heading(10.0, 500.0, 16.0);
        // diff = 500 - 10 = 490 → normalize → 490 - 512 = -22 → step = -16
        // (10 - 16 + 512) % 512 = 506
        assert!((result - 506.0).abs() < f32::EPSILON, "got {result}");
    }

    #[test]
    fn step_toward_heading_exact_max_step_snaps() {
        let result = step_toward_heading(100.0, 116.0, 16.0);
        assert!((result - 116.0).abs() < f32::EPSILON);
    }

    #[test]
    fn step_toward_heading_zero_step() {
        // max_step = 0 and diff > 0: steps 0, so stays at current
        let result = step_toward_heading(100.0, 200.0, 0.0);
        assert!((result - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn step_toward_heading_same_heading() {
        let result = step_toward_heading(256.0, 256.0, 16.0);
        assert!((result - 256.0).abs() < f32::EPSILON);
    }

    #[test]
    fn step_toward_heading_opposite_direction() {
        // Exactly 256 apart — tests the boundary condition of the shortest path
        let result = step_toward_heading(0.0, 256.0, 16.0);
        // diff = 256 → not strictly > 256, so doesn't trigger normalization.
        // 256 > 16 → step = +16
        assert!((result - 16.0).abs() < f32::EPSILON, "got {result}");
    }

    // ─── CircleConfig tests ─────────────────────────────────────────────

    #[test]
    fn circle_config_with_radius_sets_radius() {
        let cfg = CircleConfig::with_radius(35.0);
        assert!((cfg.radius - 35.0).abs() < f32::EPSILON);
        assert_eq!(cfg.mode, CircleMode::Cw);
        assert!(cfg.center.is_none());
        assert!(cfg.target_id.is_none());
        assert_eq!(cfg.drunken_interval, 20);
    }

    #[test]
    fn circle_config_at_loc_sets_center_and_radius() {
        let cfg = CircleConfig::at_loc(100.0, 200.0, 50.0, 25.0);
        assert!((cfg.radius - 25.0).abs() < f32::EPSILON);
        let center = cfg.center.expect("center should be set");
        // Note: at_loc takes (y, x, z, radius) but constructs Waypoint::new(x, y, z)
        assert!((center.x - 200.0).abs() < f32::EPSILON);
        assert!((center.y - 100.0).abs() < f32::EPSILON);
        assert!((center.z - 50.0).abs() < f32::EPSILON);
        assert_eq!(cfg.mode, CircleMode::Cw);
    }

    #[test]
    fn circle_config_default() {
        let cfg = CircleConfig::default();
        assert!((cfg.radius - 20.0).abs() < f32::EPSILON);
        assert!(cfg.center.is_none());
        assert!(cfg.target_id.is_none());
    }

    #[test]
    fn circle_config_serde_roundtrip() {
        let cfg = CircleConfig::at_loc(50.0, 60.0, 10.0, 30.0);
        let json = serde_json::to_string(&cfg).expect("serialize");
        let restored: CircleConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cfg, restored);
    }

    // ─── NavStatus label and predicate coverage ─────────────────────────

    #[test]
    fn nav_status_label_all_variants() {
        assert_eq!(NavStatus::Idle.label(), "Idle");
        assert_eq!(
            NavStatus::Moving {
                waypoint_index: 0,
                waypoint_count: 1,
                distance_remaining: 10.0
            }
            .label(),
            "Navigating"
        );
        assert_eq!(
            NavStatus::Paused {
                reason: PauseReason::Warp,
                waypoint_index: 0,
                waypoint_count: 1,
                distance_remaining: 5.0,
            }
            .label(),
            "Paused"
        );
        assert_eq!(
            NavStatus::Stuck {
                recovery_attempt: 1
            }
            .label(),
            "Stuck"
        );
        assert_eq!(NavStatus::Arrived.label(), "Arrived");
        assert_eq!(
            NavStatus::Following {
                leader_name: "Tank".to_string(),
                distance_to_anchor: 10.0,
                returning: false,
            }
            .label(),
            "Following"
        );
        assert_eq!(
            NavStatus::Sticking {
                target_id: 1,
                distance: 5.0,
                in_range: true,
            }
            .label(),
            "Sticking"
        );
        assert_eq!(
            NavStatus::Circling {
                radius: 20.0,
                angle: 0.0,
                mode: CircleMode::Cw,
            }
            .label(),
            "Circling"
        );
    }

    #[test]
    fn nav_status_is_moving_true_only_for_moving() {
        assert!(
            NavStatus::Moving {
                waypoint_index: 0,
                waypoint_count: 2,
                distance_remaining: 10.0
            }
            .is_moving()
        );
        assert!(!NavStatus::Idle.is_moving());
        assert!(!NavStatus::Arrived.is_moving());
    }

    #[test]
    fn nav_status_is_paused_true_only_for_paused() {
        assert!(
            NavStatus::Paused {
                reason: PauseReason::UserPause,
                waypoint_index: 0,
                waypoint_count: 1,
                distance_remaining: 5.0,
            }
            .is_paused()
        );
        assert!(!NavStatus::Idle.is_paused());
        assert!(
            !NavStatus::Moving {
                waypoint_index: 0,
                waypoint_count: 1,
                distance_remaining: 10.0
            }
            .is_paused()
        );
    }

    #[test]
    fn nav_status_is_stuck_true_only_for_stuck() {
        assert!(
            NavStatus::Stuck {
                recovery_attempt: 3
            }
            .is_stuck()
        );
        assert!(!NavStatus::Idle.is_stuck());
    }

    #[test]
    fn nav_status_is_arrived_true_only_for_arrived() {
        assert!(NavStatus::Arrived.is_arrived());
        assert!(!NavStatus::Idle.is_arrived());
    }

    #[test]
    fn nav_status_is_circling_true_only_for_circling() {
        assert!(
            NavStatus::Circling {
                radius: 20.0,
                angle: 0.0,
                mode: CircleMode::Cw,
            }
            .is_circling()
        );
        assert!(!NavStatus::Idle.is_circling());
    }

    // ─── NavPathMetrics factory tests ───────────────────────────────────

    #[test]
    fn nav_path_metrics_success_with_length() {
        let m = NavPathMetrics::success(Some(100.0));
        assert!(m.path_exists);
        assert!((m.path_length.unwrap() - 100.0).abs() < f32::EPSILON);
        assert!(m.failure_reason.is_none());
        assert!(m.failure_kind.is_none());
        assert!(!m.replan_recommended);
    }

    #[test]
    fn nav_path_metrics_success_without_length() {
        let m = NavPathMetrics::success(None);
        assert!(m.path_exists);
        assert!(m.path_length.is_none());
    }

    #[test]
    fn nav_path_metrics_failure_data_gap() {
        let m = NavPathMetrics::failure("no mesh", NavPathFailureKind::DataGap, None, true);
        assert!(!m.path_exists);
        assert_eq!(m.failure_reason.as_deref(), Some("no mesh"));
        assert_eq!(m.failure_kind, Some(NavPathFailureKind::DataGap));
        assert!(m.replan_recommended);
    }

    #[test]
    fn nav_path_metrics_failure_transient() {
        let m = NavPathMetrics::failure(
            "blocked",
            NavPathFailureKind::TransientBlockage,
            Some(50.0),
            false,
        );
        assert!(!m.path_exists);
        assert!((m.path_length.unwrap() - 50.0).abs() < f32::EPSILON);
        assert!(!m.replan_recommended);
    }

    #[test]
    fn nav_path_failure_kind_labels() {
        assert_eq!(NavPathFailureKind::DataGap.label(), "data gap");
        assert_eq!(
            NavPathFailureKind::TransientBlockage.label(),
            "transient blockage"
        );
    }

    #[test]
    fn nav_path_metrics_serde_roundtrip() {
        let m = NavPathMetrics::failure("test", NavPathFailureKind::DataGap, Some(10.0), true);
        let json = serde_json::to_string(&m).expect("serialize");
        let restored: NavPathMetrics = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(m, restored);
    }

    // ─── NavCampConfig::return_position_randomized tests ────────────────

    #[test]
    fn camp_config_return_position_randomized_without_scatter() {
        let config = NavCampConfig {
            center: Waypoint::new(50.0, 50.0, 0.0),
            heading: 0.0,
            radius: 30.0,
            scatter: None,
            role: "tank".to_string(),
            leash_factor: 1.5,
            min_delay_ms: 0,
            max_delay_ms: 0,
            return_no_aggro: false,
            return_not_looting: false,
            autopause: false,
        };
        let pos = config.return_position_randomized(0.5, 0.5);
        assert!((pos.x - 50.0).abs() < f32::EPSILON);
        assert!((pos.y - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn camp_config_return_position_randomized_with_scatter() {
        let config = NavCampConfig {
            center: Waypoint::new(50.0, 50.0, 0.0),
            heading: 0.0,
            radius: 30.0,
            scatter: Some(ScatterConfig::new(0.0, 20.0, 10.0)),
            role: "healer".to_string(),
            leash_factor: 1.5,
            min_delay_ms: 0,
            max_delay_ms: 0,
            return_no_aggro: false,
            return_not_looting: false,
            autopause: false,
        };
        // With non-zero scatsize and seeds, position should differ from center
        let pos = config.return_position_randomized(0.5, 0.5);
        // The scatter should produce a point within scatdist + scatsize of center
        let dist = config.center.distance_2d(&pos);
        assert!(
            dist <= 30.0,
            "randomized position should be within scatdist+scatsize: dist={dist}"
        );
    }

    #[test]
    fn camp_config_return_position_randomized_zero_seeds() {
        let config = NavCampConfig {
            center: Waypoint::new(100.0, 200.0, 0.0),
            heading: 0.0,
            radius: 30.0,
            scatter: Some(ScatterConfig::new(90.0, 15.0, 5.0)),
            role: "dps".to_string(),
            leash_factor: 1.5,
            min_delay_ms: 0,
            max_delay_ms: 0,
            return_no_aggro: false,
            return_not_looting: false,
            autopause: false,
        };
        // angle_seed=0 → theta=0, dist_seed=0 → r=0 → scatter offset is zero
        let pos = config.return_position_randomized(0.0, 0.0);
        // With dist_seed=0, the scatter resolve reduces to the fixed offset
        let expected = config.scatter.as_ref().unwrap().offset(&config.center);
        assert!((pos.x - expected.x).abs() < f32::EPSILON);
        assert!((pos.y - expected.y).abs() < f32::EPSILON);
    }

    // ─── HeadingMode coverage ───────────────────────────────────────────

    #[test]
    fn heading_mode_serde_roundtrip() {
        for mode in [HeadingMode::True, HeadingMode::Loose] {
            let json = serde_json::to_string(&mode).expect("serialize");
            let restored: HeadingMode = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(mode, restored);
        }
    }

    // ─── PauseReason coverage ───────────────────────────────────────────

    #[test]
    fn pause_reason_serde_roundtrip() {
        let reasons = [
            PauseReason::Warp,
            PauseReason::UserPause,
            PauseReason::UserInput,
            PauseReason::GmNearby,
        ];
        for reason in reasons {
            let json = serde_json::to_string(&reason).expect("serialize");
            let restored: PauseReason = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(reason, restored);
        }
    }
}
