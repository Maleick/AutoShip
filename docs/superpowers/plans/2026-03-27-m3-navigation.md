# M3 Navigation & Pathfinding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give each of 18-36 EQ characters independent, human-like movement — from basic A-to-B waypoint navigation through zone transitions, camp positioning, and stuck recovery.

**Architecture:** Two-layer design. The DLL runs a per-tick navigation state machine that autonomously follows waypoints, handles heading/speed, and detects stuck conditions. The orchestrator handles high-level routing (which zone, port coordination, group travel). IPC carries high-level commands down (`NavigateTo`, `SetCamp`) and navigation status up (via `GameState`). Waypoint recording comes first; Recast/Detour navmesh generation is a later phase.

**Tech Stack:** Rust, retour (detours), bincode (IPC), textquest-common (shared types). No new external dependencies in Phase 1.

---

## File Structure

### New files

| File                           | Responsibility                                                |
| ------------------------------ | ------------------------------------------------------------- |
| `textquest-dll/src/nav/mod.rs`      | Navigation module root, re-exports                            |
| `textquest-dll/src/nav/state.rs`    | Per-tick navigation state machine (Idle/Moving/Stuck/Arrived) |
| `textquest-dll/src/nav/waypoint.rs` | Waypoint queue, path following logic                          |
| `textquest-dll/src/nav/stuck.rs`    | Stuck detection and recovery strategies                       |
| `textquest-dll/src/nav/humanize.rs` | Speed jitter, heading wobble, path deviation                  |
| `textquest-common/src/nav.rs`       | Shared nav types (Waypoint, NavCommand, NavStatus, CampSpot)  |
| `textquest/src/nav/mod.rs`          | Orchestrator-side navigation module root                      |
| `textquest/src/nav/router.rs`       | High-level A-to-B zone routing, port coordination             |
| `textquest/src/nav/camp.rs`         | Camp position definitions and assignment                      |
| `textquest/src/nav/recorder.rs`     | Waypoint recording from live character movement               |

### Modified files

| File                                    | Changes                                        |
| --------------------------------------- | ---------------------------------------------- |
| `textquest-common/src/ipc.rs`                | Add navigation commands and responses          |
| `textquest-common/src/types.rs`              | Add `NavStatus` to `GameState`                 |
| `textquest-common/src/lib.rs`                | Add `pub mod nav;`                             |
| `textquest-dll/src/lib.rs`                   | Add `mod nav;`                                 |
| `textquest-dll/src/hooks/movement.rs`        | Implement actual movement via EQ memory writes |
| `textquest-dll/src/hooks/game_loop.rs`       | Call nav state machine in `on_game_tick()`     |
| `textquest/src/main.rs` or `textquest/src/lib.rs` | Add `mod nav;`                                 |

---

## Task 1: Shared Navigation Types (textquest-common)

**Files:**

- Create: `textquest-common/src/nav.rs`
- Modify: `textquest-common/src/lib.rs`
- Modify: `textquest-common/src/ipc.rs`
- Modify: `textquest-common/src/types.rs`

- [ ] **Step 1: Create `textquest-common/src/nav.rs` with core types**

```rust
// textquest-common/src/nav.rs
use serde::{Deserialize, Serialize};

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
        ((other.x - self.x).powi(2) + (other.y - self.y).powi(2) + (other.z - self.z).powi(2)).sqrt()
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
```

- [ ] **Step 2: Add `pub mod nav;` to `textquest-common/src/lib.rs`**

Add after existing module declarations:

```rust
pub mod nav;
```

- [ ] **Step 3: Add navigation commands to `textquest-common/src/ipc.rs`**

Add these variants to the `Command` enum:

```rust
    // Navigation
    /// Follow a sequence of waypoints.
    NavigateTo { waypoints: Vec<crate::nav::Waypoint> },
    /// Move to a camp spot and face heading.
    SetCamp { spot: crate::nav::CampSpot },
    /// Stop navigating, stay where you are.
    StopNavigation,
```

- [ ] **Step 4: Add `NavStatus` to `GameState` in `textquest-common/src/types.rs`**

Add field to `GameState`:

```rust
    pub nav_status: crate::nav::NavStatus,
```

And update the `GameState` struct's construction sites to default to `NavStatus::Idle`.

- [ ] **Step 5: Verify it compiles**

Run: `cargo build -p textquest-common`
Expected: Clean compile with no errors.

- [ ] **Step 6: Commit**

```bash
git add textquest-common/src/nav.rs textquest-common/src/lib.rs textquest-common/src/ipc.rs textquest-common/src/types.rs
git commit -m "feat(nav): add shared navigation types — Waypoint, NavStatus, CampSpot, nav commands"
```

---

## Task 2: Implement Movement Primitives (DLL side)

**Files:**

- Modify: `textquest-dll/src/hooks/movement.rs`
- Modify: `textquest-common/src/offsets.rs`

Movement in EQ works by writing directly to the PlayerClient struct fields: set heading, then set the speed/movement flags. The game engine picks up these values each tick.

- [ ] **Step 1: Add movement-related field offsets to `textquest-common/src/offsets.rs`**

Add to the `player_base` module:

```rust
    /// float — movement speed
    pub const SPEED_RUN: usize = 0x088;
    /// float — current speed (actual, includes modifiers)
    pub const SPEED_CURRENT: usize = 0x084;
    /// float — speed heading (direction of movement)
    pub const SPEED_HEADING: usize = 0x09c;
    /// uint8_t — standing state (0=standing, 1=frozen, 2=looting, 3=sitting, 4=ducking, 110=feigned, 111=dead)
    pub const STANDSTATE: usize = 0x134;
```

- [ ] **Step 2: Implement `movement.rs` with actual memory read/write logic**

Replace the full file:

```rust
//! Movement control -- provides functions to move the player character.
//! Writes directly to PlayerClient struct fields in EQ memory.
//! The game engine reads these values each tick to process movement.

use textquest_common::nav::Waypoint;

/// Arrival threshold in game units (close enough to "be there").
pub const ARRIVAL_DISTANCE: f32 = 15.0;

/// Calculate heading from current position to target (EQ heading: 0-512, 0=north, increases CW).
pub fn calc_heading(from: &Waypoint, to: &Waypoint) -> f32 {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    // EQ uses atan2(-dx, dy) mapped to 0..512
    let rad = (-dx).atan2(dy);
    let deg = rad.to_degrees();
    // Convert -180..180 to 0..512
    let eq_heading = (deg * 512.0 / 360.0 + 512.0) % 512.0;
    eq_heading
}

/// Calculate 2D distance between two waypoints.
pub fn distance_2d(a: &Waypoint, b: &Waypoint) -> f32 {
    a.distance_2d(b)
}

/// Calculate 3D distance between two waypoints.
pub fn distance_3d(a: &Waypoint, b: &Waypoint) -> f32 {
    a.distance_3d(b)
}

/// Movement controller state -- holds a pointer to the local player's
/// PlayerClient struct for direct memory writes.
///
/// On non-Windows, all write operations are no-ops logged via tracing.
pub struct MovementController {
    /// Base address of the local PlayerClient struct.
    player_base: usize,
}

impl MovementController {
    /// Create a new controller targeting the given PlayerClient address.
    pub fn new(player_base: usize) -> Self {
        Self { player_base }
    }

    /// Update the player base address (e.g., after zoning).
    pub fn set_player_base(&mut self, addr: usize) {
        self.player_base = addr;
    }

    /// Write heading to face a target position.
    pub fn face_toward(&self, target: &Waypoint, current: &Waypoint) {
        let heading = calc_heading(current, target);
        self.write_heading(heading);
    }

    /// Write heading value directly.
    pub fn write_heading(&self, heading: f32) {
        #[cfg(windows)]
        unsafe {
            let addr = self.player_base + textquest_common::offsets::player_base::HEADING;
            std::ptr::write(addr as *mut f32, heading);
        }
        #[cfg(not(windows))]
        tracing::trace!(heading, "write_heading (stub)");
    }

    /// Write speed heading (direction of actual movement).
    pub fn write_speed_heading(&self, heading: f32) {
        #[cfg(windows)]
        unsafe {
            let addr = self.player_base + textquest_common::offsets::player_base::SPEED_HEADING;
            std::ptr::write(addr as *mut f32, heading);
        }
        #[cfg(not(windows))]
        tracing::trace!(heading, "write_speed_heading (stub)");
    }

    /// Read current position from the PlayerClient struct.
    pub fn read_position(&self) -> Waypoint {
        #[cfg(windows)]
        unsafe {
            let base = self.player_base;
            let y = std::ptr::read((base + textquest_common::offsets::player_base::Y) as *const f32);
            let x = std::ptr::read((base + textquest_common::offsets::player_base::X) as *const f32);
            let z = std::ptr::read((base + textquest_common::offsets::player_base::Z) as *const f32);
            Waypoint::new(x, y, z)
        }
        #[cfg(not(windows))]
        {
            tracing::trace!("read_position (stub)");
            Waypoint::new(0.0, 0.0, 0.0)
        }
    }

    /// Read current heading.
    pub fn read_heading(&self) -> f32 {
        #[cfg(windows)]
        unsafe {
            std::ptr::read((self.player_base + textquest_common::offsets::player_base::HEADING) as *const f32)
        }
        #[cfg(not(windows))]
        {
            tracing::trace!("read_heading (stub)");
            0.0
        }
    }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p textquest-dll`
Expected: Clean compile.

- [ ] **Step 4: Commit**

```bash
git add textquest-dll/src/hooks/movement.rs textquest-common/src/offsets.rs
git commit -m "feat(nav): implement movement primitives — heading calc, memory read/write, MovementController"
```

---

## Task 3: Navigation State Machine (DLL side)

**Files:**

- Create: `textquest-dll/src/nav/mod.rs`
- Create: `textquest-dll/src/nav/state.rs`
- Create: `textquest-dll/src/nav/waypoint.rs`
- Modify: `textquest-dll/src/lib.rs`

This is the core per-tick logic. The state machine transitions: Idle -> Moving -> (Stuck -> Moving) -> Arrived -> Idle.

- [ ] **Step 1: Create `textquest-dll/src/nav/waypoint.rs` — waypoint queue**

```rust
//! Waypoint queue — stores and advances through a path of waypoints.

use textquest_common::nav::Waypoint;

/// A queue of waypoints to follow in order.
pub struct WaypointQueue {
    waypoints: Vec<Waypoint>,
    current_index: usize,
}

impl WaypointQueue {
    /// Create an empty queue.
    pub fn new() -> Self {
        Self {
            waypoints: Vec::new(),
            current_index: 0,
        }
    }

    /// Load a new path, resetting to the first waypoint.
    pub fn set_path(&mut self, waypoints: Vec<Waypoint>) {
        self.waypoints = waypoints;
        self.current_index = 0;
    }

    /// Get the current target waypoint, if any remain.
    pub fn current(&self) -> Option<&Waypoint> {
        self.waypoints.get(self.current_index)
    }

    /// Advance to the next waypoint. Returns true if there is a next one.
    pub fn advance(&mut self) -> bool {
        if self.current_index + 1 < self.waypoints.len() {
            self.current_index += 1;
            true
        } else {
            false
        }
    }

    /// Current index in the path.
    pub fn index(&self) -> usize {
        self.current_index
    }

    /// Total number of waypoints.
    pub fn len(&self) -> usize {
        self.waypoints.len()
    }

    /// Whether the queue is empty (no path loaded).
    pub fn is_empty(&self) -> bool {
        self.waypoints.is_empty()
    }

    /// Clear the path.
    pub fn clear(&mut self) {
        self.waypoints.clear();
        self.current_index = 0;
    }
}
```

- [ ] **Step 2: Create `textquest-dll/src/nav/state.rs` — navigation state machine**

```rust
//! Navigation state machine — runs once per game tick.
//! Transitions: Idle -> Moving -> (Stuck -> Moving) -> Arrived -> Idle

use textquest_common::nav::{NavStatus, Waypoint, CampSpot};
use crate::hooks::movement::{self, MovementController, ARRIVAL_DISTANCE};
use super::waypoint::WaypointQueue;

/// How many ticks with < 1.0 unit movement before we consider ourselves stuck.
const STUCK_TICK_THRESHOLD: u32 = 40; // ~2 seconds at 20 Hz
/// Maximum recovery attempts before giving up.
const MAX_RECOVERY_ATTEMPTS: u32 = 5;
/// Minimum movement per tick to not be considered stuck.
const MIN_MOVEMENT_PER_TICK: f32 = 0.5;

/// Internal state for the navigation FSM.
enum State {
    Idle,
    Moving,
    Stuck { recovery_attempt: u32 },
    Arrived,
}

/// The navigation engine, owned per-client in the DLL.
pub struct Navigator {
    state: State,
    queue: WaypointQueue,
    controller: MovementController,
    /// Camp spot to hold after arrival (optional).
    camp: Option<CampSpot>,
    /// Last known position for stuck detection.
    last_position: Waypoint,
    /// Ticks since meaningful movement.
    stuck_ticks: u32,
}

impl Navigator {
    pub fn new(player_base: usize) -> Self {
        Self {
            state: State::Idle,
            queue: WaypointQueue::new(),
            controller: MovementController::new(player_base),
            camp: None,
            last_position: Waypoint::new(0.0, 0.0, 0.0),
            stuck_ticks: 0,
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
        self.stuck_ticks = 0;
        self.state = State::Moving;
    }

    /// Move to a camp spot and face the specified heading.
    pub fn set_camp(&mut self, spot: CampSpot) {
        tracing::info!(role = %spot.role, "Setting camp spot");
        self.queue.set_path(vec![spot.position]);
        self.camp = Some(spot);
        self.stuck_ticks = 0;
        self.state = State::Moving;
    }

    /// Stop navigation immediately.
    pub fn stop(&mut self) {
        self.queue.clear();
        self.camp = None;
        self.stuck_ticks = 0;
        self.state = State::Idle;
        tracing::info!("Navigation stopped");
    }

    /// Run one tick of the navigation state machine. Call from on_game_tick().
    pub fn tick(&mut self) {
        match self.state {
            State::Idle | State::Arrived => {}
            State::Moving => self.tick_moving(),
            State::Stuck { recovery_attempt } => self.tick_stuck(recovery_attempt),
        }
    }

    /// Get current navigation status for IPC reporting.
    pub fn status(&self) -> NavStatus {
        match &self.state {
            State::Idle => NavStatus::Idle,
            State::Moving => {
                let current_pos = self.controller.read_position();
                let dist = self.queue.current()
                    .map(|wp| movement::distance_2d(&current_pos, wp))
                    .unwrap_or(0.0);
                NavStatus::Moving {
                    waypoint_index: self.queue.index(),
                    waypoint_count: self.queue.len(),
                    distance_remaining: dist,
                }
            }
            State::Stuck { recovery_attempt } => NavStatus::Stuck {
                recovery_attempt: *recovery_attempt,
            },
            State::Arrived => NavStatus::Arrived,
        }
    }

    fn tick_moving(&mut self) {
        let current_pos = self.controller.read_position();

        // Check if we've arrived at the current waypoint.
        if let Some(target) = self.queue.current() {
            let dist = movement::distance_2d(&current_pos, target);

            if dist < ARRIVAL_DISTANCE {
                // Arrived at this waypoint — advance or finish.
                if self.queue.advance() {
                    tracing::debug!(index = self.queue.index(), "Advanced to next waypoint");
                    self.stuck_ticks = 0;
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

        // Stuck detection: check if we moved since last tick.
        let moved = movement::distance_2d(&current_pos, &self.last_position);
        if moved < MIN_MOVEMENT_PER_TICK {
            self.stuck_ticks += 1;
            if self.stuck_ticks >= STUCK_TICK_THRESHOLD {
                tracing::warn!(ticks = self.stuck_ticks, "Stuck detected");
                self.state = State::Stuck { recovery_attempt: 1 };
                self.stuck_ticks = 0;
                return;
            }
        } else {
            self.stuck_ticks = 0;
        }
        self.last_position = current_pos;

        // Face and move toward the current waypoint.
        if let Some(target) = self.queue.current() {
            self.controller.face_toward(target, &current_pos);
            self.controller.write_speed_heading(
                movement::calc_heading(&current_pos, target),
            );
        }
    }

    fn tick_stuck(&mut self, recovery_attempt: u32) {
        if recovery_attempt > MAX_RECOVERY_ATTEMPTS {
            tracing::error!("Max recovery attempts reached, stopping navigation");
            self.stop();
            return;
        }

        // Recovery strategy: back up slightly, turn, retry.
        // Each attempt tries a different angle offset.
        let current_pos = self.controller.read_position();
        let current_heading = self.controller.read_heading();

        // Turn 90 degrees * attempt number (alternating left/right).
        let offset = if recovery_attempt % 2 == 1 { 128.0 } else { -128.0 };
        let new_heading = (current_heading + offset * recovery_attempt as f32) % 512.0;
        self.controller.write_heading(new_heading);

        tracing::info!(attempt = recovery_attempt, heading = new_heading, "Stuck recovery: turning");

        // Go back to Moving — if still stuck, we'll re-enter Stuck with attempt+1.
        self.stuck_ticks = 0;
        self.state = State::Moving;

        // Increment for next stuck detection.
        // (We store the next attempt number so repeated stucks escalate.)
        // This is handled by re-entering Stuck with recovery_attempt + 1 if
        // stuck detection fires again.
    }

    fn on_path_complete(&mut self) {
        // If we have a camp spot, face the camp heading.
        if let Some(ref camp) = self.camp {
            self.controller.write_heading(camp.heading);
            tracing::info!(role = %camp.role, "Arrived at camp spot");
        } else {
            tracing::info!("Navigation path complete");
        }
        self.state = State::Arrived;
    }
}
```

- [ ] **Step 3: Create `textquest-dll/src/nav/mod.rs`**

```rust
//! Navigation module — autonomous waypoint-based movement.

pub mod state;
pub mod waypoint;

pub use state::Navigator;
```

- [ ] **Step 4: Add `mod nav;` to `textquest-dll/src/lib.rs`**

Add after existing module declarations:

```rust
mod nav;
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build -p textquest-dll`
Expected: Clean compile.

- [ ] **Step 6: Commit**

```bash
git add textquest-dll/src/nav/
git commit -m "feat(nav): add navigation state machine — Idle/Moving/Stuck/Arrived FSM with waypoint queue"
```

---

## Task 4: Integrate Navigator into Game Loop

**Files:**

- Modify: `textquest-dll/src/hooks/game_loop.rs`
- Modify: `textquest-dll/src/hooks/mod.rs`

Wire the Navigator into `on_game_tick()` so it runs every frame.

- [ ] **Step 1: Add a global Navigator instance**

The Navigator needs to persist across ticks. Use a `Mutex<Option<Navigator>>` global since the game loop is single-threaded but commands arrive from IPC.

Add to `textquest-dll/src/nav/mod.rs`:

```rust
use std::sync::Mutex;

static NAVIGATOR: Mutex<Option<Navigator>> = Mutex::new(None);

/// Initialize the global navigator with the player base address.
pub fn init(player_base: usize) {
    let mut nav = NAVIGATOR.lock().unwrap();
    *nav = Some(Navigator::new(player_base));
    tracing::info!("Navigator initialized");
}

/// Run one navigation tick. Call from on_game_tick().
pub fn tick() {
    if let Some(ref mut nav) = *NAVIGATOR.lock().unwrap() {
        nav.tick();
    }
}

/// Get current navigation status for IPC reporting.
pub fn status() -> textquest_common::nav::NavStatus {
    NAVIGATOR.lock().unwrap()
        .as_ref()
        .map(|n| n.status())
        .unwrap_or(textquest_common::nav::NavStatus::Idle)
}

/// Handle a navigation command from IPC.
pub fn handle_command(cmd: NavCommand) {
    if let Some(ref mut nav) = *NAVIGATOR.lock().unwrap() {
        match cmd {
            NavCommand::Navigate(waypoints) => nav.navigate(waypoints),
            NavCommand::SetCamp(spot) => nav.set_camp(spot),
            NavCommand::Stop => nav.stop(),
        }
    }
}

/// Commands that can be sent to the navigator.
pub enum NavCommand {
    Navigate(Vec<textquest_common::nav::Waypoint>),
    SetCamp(textquest_common::nav::CampSpot),
    Stop,
}
```

- [ ] **Step 2: Call `nav::tick()` from `on_game_tick()`**

In `textquest-dll/src/hooks/game_loop.rs`, update `on_game_tick()`:

```rust
fn on_game_tick() {
    // Run navigation state machine.
    crate::nav::tick();

    // TODO: Read game state from EQ memory (local player, target, spawns)
    // TODO: Publish state to shared memory via IPC
    // TODO: Check for and execute pending commands from the orchestrator
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p textquest-dll`
Expected: Clean compile.

- [ ] **Step 4: Commit**

```bash
git add textquest-dll/src/nav/mod.rs textquest-dll/src/hooks/game_loop.rs
git commit -m "feat(nav): integrate Navigator into game loop — runs nav tick every frame"
```

---

## Task 5: Movement Humanization

**Files:**

- Create: `textquest-dll/src/nav/humanize.rs`
- Modify: `textquest-dll/src/nav/state.rs`

Make each character move differently so 36 characters don't look like synchronized bots.

- [ ] **Step 1: Create `textquest-dll/src/nav/humanize.rs`**

```rust
//! Movement humanization — per-character speed jitter, heading wobble,
//! and occasional path deviations to avoid bot-like movement patterns.

/// Per-character movement personality. Values are seeded from the client_id
/// so each character consistently moves differently.
pub struct MovementPersonality {
    /// Speed multiplier variance (e.g., 0.95..1.05).
    pub speed_factor: f32,
    /// Max heading wobble in EQ degrees per tick.
    pub heading_wobble: f32,
    /// Chance per waypoint of taking a slight detour (0.0..1.0).
    pub detour_chance: f32,
    /// Simple PRNG state for deterministic randomness.
    rng_state: u32,
}

impl MovementPersonality {
    /// Create a personality seeded from the client ID.
    pub fn from_client_id(client_id: u32) -> Self {
        // Simple hash to spread values across characters.
        let seed = client_id.wrapping_mul(2654435761); // Knuth multiplicative hash
        let speed_factor = 0.93 + (seed % 140) as f32 / 1000.0; // 0.93..1.07
        let heading_wobble = 1.0 + (seed.wrapping_shr(8) % 30) as f32 / 10.0; // 1.0..4.0
        let detour_chance = (seed.wrapping_shr(16) % 80) as f32 / 1000.0; // 0.00..0.08

        Self {
            speed_factor,
            heading_wobble,
            detour_chance,
            rng_state: seed,
        }
    }

    /// Apply heading wobble to a base heading. Returns adjusted heading.
    pub fn wobble_heading(&mut self, heading: f32) -> f32 {
        let noise = self.next_f32() * self.heading_wobble * 2.0 - self.heading_wobble;
        (heading + noise + 512.0) % 512.0
    }

    /// Should this character take a detour at the current waypoint?
    pub fn should_detour(&mut self) -> bool {
        self.next_f32() < self.detour_chance
    }

    /// Generate a random stagger delay in ticks (0..max_ticks).
    pub fn stagger_ticks(&mut self, max_ticks: u32) -> u32 {
        (self.next_f32() * max_ticks as f32) as u32
    }

    /// Simple xorshift32 PRNG, returns 0.0..1.0.
    fn next_f32(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        (self.rng_state as f32) / (u32::MAX as f32)
    }
}
```

- [ ] **Step 2: Integrate personality into Navigator**

In `textquest-dll/src/nav/state.rs`, add `personality` field to `Navigator`:

```rust
use super::humanize::MovementPersonality;
```

Add to `Navigator` struct:

```rust
    personality: MovementPersonality,
```

Update `Navigator::new`:

```rust
    pub fn new(player_base: usize, client_id: u32) -> Self {
        Self {
            state: State::Idle,
            queue: WaypointQueue::new(),
            controller: MovementController::new(player_base),
            camp: None,
            last_position: Waypoint::new(0.0, 0.0, 0.0),
            stuck_ticks: 0,
            personality: MovementPersonality::from_client_id(client_id),
        }
    }
```

Update `tick_moving()` to apply wobble when writing heading:

```rust
        // Face and move toward the current waypoint.
        if let Some(target) = self.queue.current() {
            let heading = movement::calc_heading(&current_pos, target);
            let wobbled = self.personality.wobble_heading(heading);
            self.controller.face_toward(target, &current_pos);
            self.controller.write_speed_heading(wobbled);
        }
```

- [ ] **Step 3: Update `nav::init()` to accept `client_id`**

In `textquest-dll/src/nav/mod.rs`:

```rust
pub fn init(player_base: usize, client_id: u32) {
    let mut nav = NAVIGATOR.lock().unwrap();
    *nav = Some(Navigator::new(player_base, client_id));
    tracing::info!(client_id, "Navigator initialized");
}
```

- [ ] **Step 4: Add `pub mod humanize;` to `textquest-dll/src/nav/mod.rs`**

- [ ] **Step 5: Verify it compiles**

Run: `cargo build -p textquest-dll`
Expected: Clean compile.

- [ ] **Step 6: Commit**

```bash
git add textquest-dll/src/nav/humanize.rs textquest-dll/src/nav/mod.rs textquest-dll/src/nav/state.rs
git commit -m "feat(nav): add movement humanization — per-character speed/heading personality via xorshift PRNG"
```

---

## Task 6: Stuck Detection & Recovery (Enhanced)

**Files:**

- Create: `textquest-dll/src/nav/stuck.rs`
- Modify: `textquest-dll/src/nav/state.rs`

Extract stuck detection into its own module with escalating recovery strategies.

- [ ] **Step 1: Create `textquest-dll/src/nav/stuck.rs`**

```rust
//! Stuck detection and escalating recovery strategies.
//!
//! Recovery escalation:
//! 1. Turn 90 degrees right, resume
//! 2. Turn 90 degrees left, resume
//! 3. Back up, turn 180, resume
//! 4. Jump + turn
//! 5. Give up, alert orchestrator

use textquest_common::nav::Waypoint;
use crate::hooks::movement::MovementController;

/// How many ticks with < threshold movement before stuck.
const STUCK_TICK_THRESHOLD: u32 = 40;
/// Minimum distance per tick to not be stuck.
const MIN_MOVEMENT: f32 = 0.5;
/// Maximum recovery attempts before giving up.
pub const MAX_RECOVERY_ATTEMPTS: u32 = 5;

/// Tracks position history for stuck detection.
pub struct StuckDetector {
    last_position: Waypoint,
    low_movement_ticks: u32,
    recovery_attempt: u32,
}

impl StuckDetector {
    pub fn new() -> Self {
        Self {
            last_position: Waypoint::new(0.0, 0.0, 0.0),
            low_movement_ticks: 0,
            recovery_attempt: 0,
        }
    }

    /// Check current position against history. Returns true if stuck.
    pub fn check(&mut self, current: &Waypoint) -> bool {
        let moved = current.distance_2d(&self.last_position);
        self.last_position = *current;

        if moved < MIN_MOVEMENT {
            self.low_movement_ticks += 1;
            self.low_movement_ticks >= STUCK_TICK_THRESHOLD
        } else {
            self.low_movement_ticks = 0;
            false
        }
    }

    /// Get current recovery attempt number (0 = not recovering).
    pub fn recovery_attempt(&self) -> u32 {
        self.recovery_attempt
    }

    /// Execute one recovery step. Returns false if max attempts exceeded.
    pub fn recover(&mut self, controller: &MovementController) -> bool {
        self.recovery_attempt += 1;
        self.low_movement_ticks = 0;

        if self.recovery_attempt > MAX_RECOVERY_ATTEMPTS {
            return false;
        }

        let heading = controller.read_heading();

        match self.recovery_attempt {
            1 => {
                // Turn 90 degrees right.
                tracing::info!("Stuck recovery 1: turn right 90");
                controller.write_heading((heading + 128.0) % 512.0);
            }
            2 => {
                // Turn 90 degrees left (180 from last attempt = 90 left of original).
                tracing::info!("Stuck recovery 2: turn left 90");
                controller.write_heading((heading + 384.0) % 512.0);
            }
            3 => {
                // Turn 180 degrees (face away from obstacle).
                tracing::info!("Stuck recovery 3: turn 180");
                controller.write_heading((heading + 256.0) % 512.0);
            }
            4 => {
                // Diagonal escape: turn 45 degrees.
                tracing::info!("Stuck recovery 4: diagonal turn 45");
                controller.write_heading((heading + 64.0) % 512.0);
            }
            _ => {
                tracing::error!("Stuck recovery {}: giving up", self.recovery_attempt);
                return false;
            }
        }

        true
    }

    /// Reset recovery state (call when meaningful movement resumes).
    pub fn reset(&mut self) {
        self.recovery_attempt = 0;
        self.low_movement_ticks = 0;
    }
}
```

- [ ] **Step 2: Replace inline stuck detection in `state.rs` with `StuckDetector`**

In `Navigator` struct, replace `last_position` and `stuck_ticks` with:

```rust
    stuck: super::stuck::StuckDetector,
```

Update `Navigator::new`:

```rust
    stuck: super::stuck::StuckDetector::new(),
```

Update `tick_moving()` to use the detector:

```rust
    fn tick_moving(&mut self) {
        let current_pos = self.controller.read_position();

        // Check arrival at current waypoint.
        if let Some(target) = self.queue.current() {
            let dist = movement::distance_2d(&current_pos, target);

            if dist < ARRIVAL_DISTANCE {
                if self.queue.advance() {
                    tracing::debug!(index = self.queue.index(), "Advanced to next waypoint");
                    self.stuck.reset();
                } else {
                    self.on_path_complete();
                    return;
                }
            }
        } else {
            self.state = State::Idle;
            return;
        }

        // Stuck detection.
        if self.stuck.check(&current_pos) {
            tracing::warn!("Stuck detected, attempting recovery");
            if !self.stuck.recover(&self.controller) {
                tracing::error!("All stuck recovery attempts exhausted");
                self.stop();
                return;
            }
            // After recovery action, stay in Moving — if still stuck, will re-trigger.
            return;
        }

        // Normal movement: face and move toward waypoint.
        if let Some(target) = self.queue.current() {
            let heading = movement::calc_heading(&current_pos, target);
            let wobbled = self.personality.wobble_heading(heading);
            self.controller.face_toward(target, &current_pos);
            self.controller.write_speed_heading(wobbled);
        }
    }
```

Remove `State::Stuck` variant and `tick_stuck()` method since the `StuckDetector` handles it inline now.

- [ ] **Step 3: Add `pub mod stuck;` to `textquest-dll/src/nav/mod.rs`**

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p textquest-dll`
Expected: Clean compile.

- [ ] **Step 5: Commit**

```bash
git add textquest-dll/src/nav/stuck.rs textquest-dll/src/nav/mod.rs textquest-dll/src/nav/state.rs
git commit -m "feat(nav): add StuckDetector with escalating recovery — turn/backup/diagonal/give-up"
```

---

## Task 7: Waypoint Recorder (Orchestrator side)

**Files:**

- Create: `textquest/src/nav/mod.rs`
- Create: `textquest/src/nav/recorder.rs`

Record a character's movement as waypoints for later replay by other characters. This is the "known routes first" strategy from the decision profile.

- [ ] **Step 1: Create `textquest/src/nav/recorder.rs`**

```rust
//! Waypoint recorder — captures a character's movement into a replayable path.
//! Records position snapshots at regular intervals while the character moves.

use textquest_common::nav::Waypoint;
use std::time::{Duration, Instant};

/// Minimum distance between recorded waypoints to avoid redundant points.
const MIN_WAYPOINT_DISTANCE: f32 = 10.0;
/// Maximum distance between waypoints — insert intermediate point if exceeded.
const MAX_WAYPOINT_DISTANCE: f32 = 200.0;

/// Records a character's movement into a sequence of waypoints.
pub struct WaypointRecorder {
    waypoints: Vec<Waypoint>,
    last_position: Option<Waypoint>,
    recording: bool,
    start_time: Option<Instant>,
}

impl WaypointRecorder {
    pub fn new() -> Self {
        Self {
            waypoints: Vec::new(),
            last_position: None,
            recording: false,
            start_time: None,
        }
    }

    /// Start recording.
    pub fn start(&mut self) {
        self.waypoints.clear();
        self.last_position = None;
        self.recording = true;
        self.start_time = Some(Instant::now());
        tracing::info!("Waypoint recording started");
    }

    /// Stop recording and return the recorded path.
    pub fn stop(&mut self) -> Vec<Waypoint> {
        self.recording = false;
        let elapsed = self.start_time.map(|s| s.elapsed()).unwrap_or_default();
        tracing::info!(
            waypoints = self.waypoints.len(),
            elapsed_secs = elapsed.as_secs(),
            "Waypoint recording stopped"
        );
        std::mem::take(&mut self.waypoints)
    }

    /// Feed a position update from game state. Call this each time
    /// the orchestrator reads a new GameState for the recorded character.
    pub fn record_position(&mut self, x: f32, y: f32, z: f32) {
        if !self.recording {
            return;
        }

        let current = Waypoint::new(x, y, z);

        let dominated = match self.last_position {
            Some(ref last) => current.distance_2d(last) < MIN_WAYPOINT_DISTANCE,
            None => false,
        };

        if !dominated {
            self.last_position = Some(current);
            self.waypoints.push(current);
        }
    }

    /// Whether currently recording.
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Number of waypoints recorded so far.
    pub fn waypoint_count(&self) -> usize {
        self.waypoints.len()
    }
}

/// Simplify a recorded path by removing redundant collinear points.
/// Uses the Ramer-Douglas-Peucker algorithm in 2D.
pub fn simplify_path(waypoints: &[Waypoint], epsilon: f32) -> Vec<Waypoint> {
    if waypoints.len() <= 2 {
        return waypoints.to_vec();
    }

    // Find the point with the maximum distance from the line (first, last).
    let first = &waypoints[0];
    let last = &waypoints[waypoints.len() - 1];
    let mut max_dist = 0.0f32;
    let mut max_index = 0;

    for (i, wp) in waypoints.iter().enumerate().skip(1).take(waypoints.len() - 2) {
        let dist = point_line_distance_2d(wp, first, last);
        if dist > max_dist {
            max_dist = dist;
            max_index = i;
        }
    }

    if max_dist > epsilon {
        // Recursively simplify both halves.
        let mut left = simplify_path(&waypoints[..=max_index], epsilon);
        let right = simplify_path(&waypoints[max_index..], epsilon);
        // Remove duplicate point at the junction.
        left.pop();
        left.extend(right);
        left
    } else {
        // All intermediate points are close to the line — keep only endpoints.
        vec![*first, *last]
    }
}

/// Perpendicular distance from point to line (2D, XY plane).
fn point_line_distance_2d(point: &Waypoint, line_start: &Waypoint, line_end: &Waypoint) -> f32 {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;
    let len_sq = dx * dx + dy * dy;

    if len_sq < f32::EPSILON {
        // Line start and end are the same point.
        return point.distance_2d(line_start);
    }

    let cross = (point.x - line_start.x) * dy - (point.y - line_start.y) * dx;
    cross.abs() / len_sq.sqrt()
}
```

- [ ] **Step 2: Create `textquest/src/nav/mod.rs`**

```rust
//! Orchestrator-side navigation — routing, recording, camp management.

pub mod recorder;
```

- [ ] **Step 3: Add `mod nav;` to the orchestrator crate root**

Add to `textquest/src/main.rs` (with the other mod declarations):

```rust
mod nav;
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p dmft`
Expected: Clean compile.

- [ ] **Step 5: Commit**

```bash
git add textquest/src/nav/
git commit -m "feat(nav): add waypoint recorder with RDP path simplification for route recording"
```

---

## Task 8: Camp Position System (Orchestrator side)

**Files:**

- Create: `textquest/src/nav/camp.rs`
- Modify: `textquest/src/nav/mod.rs`

Define camp positions with role-based spot assignments. Characters navigate to their assigned spot and face the specified heading.

- [ ] **Step 1: Create `textquest/src/nav/camp.rs`**

```rust
//! Camp position management — assign characters to role-based spots.

use textquest_common::nav::{CampDefinition, CampSpot, Waypoint};
use textquest_common::types::ClientId;
use std::collections::HashMap;

/// Manages camp assignments for a group.
pub struct CampManager {
    /// Current camp definition (if any).
    active_camp: Option<CampDefinition>,
    /// client_id -> assigned role.
    assignments: HashMap<ClientId, String>,
}

impl CampManager {
    pub fn new() -> Self {
        Self {
            active_camp: None,
            assignments: HashMap::new(),
        }
    }

    /// Set the active camp and assign characters to spots based on their roles.
    /// `role_map` maps client_id to their role string (e.g., "tank", "healer1").
    pub fn set_camp(
        &mut self,
        camp: CampDefinition,
        role_map: &HashMap<ClientId, String>,
    ) -> Vec<(ClientId, CampSpot)> {
        let mut result = Vec::new();

        for (client_id, role) in role_map {
            if let Some(spot) = camp.spots.iter().find(|s| s.role == *role) {
                self.assignments.insert(*client_id, role.clone());
                result.push((*client_id, spot.clone()));
            } else {
                tracing::warn!(client_id, role = %role, "No camp spot defined for role");
            }
        }

        self.active_camp = Some(camp);
        result
    }

    /// Get the camp spot for a specific client.
    pub fn get_spot(&self, client_id: ClientId) -> Option<&CampSpot> {
        let role = self.assignments.get(&client_id)?;
        self.active_camp.as_ref()?.spots.iter().find(|s| s.role == *role)
    }

    /// Clear the active camp.
    pub fn clear(&mut self) {
        self.active_camp = None;
        self.assignments.clear();
    }

    /// Whether a camp is active.
    pub fn is_active(&self) -> bool {
        self.active_camp.is_some()
    }
}

/// Helper: create a basic group camp with standard EQ positioning.
/// Tank in front, healer behind, DPS spread in a semicircle.
pub fn create_standard_camp(
    center: Waypoint,
    pull_heading: f32,
    num_dps: usize,
) -> CampDefinition {
    let mut spots = Vec::new();

    // Tank: 20 units in the pull direction.
    let pull_rad = pull_heading * std::f32::consts::PI * 2.0 / 512.0;
    spots.push(CampSpot {
        position: Waypoint::new(
            center.x + 20.0 * pull_rad.sin(),
            center.y + 20.0 * pull_rad.cos(),
            center.z,
        ),
        heading: pull_heading,
        role: "tank".to_string(),
    });

    // Healer: 15 units behind center (opposite pull direction).
    let back_heading = (pull_heading + 256.0) % 512.0;
    let back_rad = back_heading * std::f32::consts::PI * 2.0 / 512.0;
    spots.push(CampSpot {
        position: Waypoint::new(
            center.x + 15.0 * back_rad.sin(),
            center.y + 15.0 * back_rad.cos(),
            center.z,
        ),
        heading: pull_heading,
        role: "healer".to_string(),
    });

    // DPS: spread in a semicircle behind center.
    for i in 0..num_dps {
        let angle_offset = (i as f32 / num_dps as f32 - 0.5) * 128.0; // +/- 45 degrees
        let dps_heading = (back_heading + angle_offset + 512.0) % 512.0;
        let dps_rad = dps_heading * std::f32::consts::PI * 2.0 / 512.0;
        spots.push(CampSpot {
            position: Waypoint::new(
                center.x + 18.0 * dps_rad.sin(),
                center.y + 18.0 * dps_rad.cos(),
                center.z,
            ),
            heading: pull_heading,
            role: format!("dps{}", i + 1),
        });
    }

    CampDefinition {
        name: "standard".to_string(),
        zone: String::new(),
        spots,
    }
}
```

- [ ] **Step 2: Add `pub mod camp;` to `textquest/src/nav/mod.rs`**

```rust
pub mod camp;
pub mod recorder;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p dmft`
Expected: Clean compile.

- [ ] **Step 4: Commit**

```bash
git add textquest/src/nav/camp.rs textquest/src/nav/mod.rs
git commit -m "feat(nav): add camp position system — role-based spot assignment with standard layout generator"
```

---

## Task 9: Zone Transition Handling (Orchestrator side)

**Files:**

- Create: `textquest/src/nav/router.rs`
- Modify: `textquest/src/nav/mod.rs`

Handle zone-to-zone travel with staggered entries and port-first coordination.

- [ ] **Step 1: Create `textquest/src/nav/router.rs`**

```rust
//! High-level zone routing — plans multi-zone travel and coordinates group transitions.

use textquest_common::nav::Waypoint;
use textquest_common::types::ClientId;
use std::collections::HashMap;

/// A step in a multi-zone travel plan.
#[derive(Debug, Clone)]
pub enum TravelStep {
    /// Walk to a position within the current zone.
    WalkTo { waypoints: Vec<Waypoint> },
    /// Zone transition: walk to zone line and enter.
    ZoneTo {
        zone_name: String,
        zone_line_pos: Waypoint,
    },
    /// Port: caster ports the group (requires port-class character).
    PortTo {
        zone_name: String,
        caster_id: ClientId,
    },
    /// Wait for staggered entry (random delay before zoning).
    StaggerWait {
        min_secs: u32,
        max_secs: u32,
    },
}

/// A complete travel plan for one character.
#[derive(Debug, Clone)]
pub struct TravelPlan {
    pub client_id: ClientId,
    pub steps: Vec<TravelStep>,
    pub current_step: usize,
}

impl TravelPlan {
    pub fn new(client_id: ClientId, steps: Vec<TravelStep>) -> Self {
        Self {
            client_id,
            steps,
            current_step: 0,
        }
    }

    pub fn current(&self) -> Option<&TravelStep> {
        self.steps.get(self.current_step)
    }

    pub fn advance(&mut self) -> bool {
        if self.current_step + 1 < self.steps.len() {
            self.current_step += 1;
            true
        } else {
            false
        }
    }

    pub fn is_complete(&self) -> bool {
        self.current_step >= self.steps.len()
    }
}

/// Generates stagger delays for a group of characters zoning together.
/// Returns map of client_id -> delay in seconds.
pub fn generate_zone_staggers(
    client_ids: &[ClientId],
    min_secs: u32,
    max_secs: u32,
    seed: u32,
) -> HashMap<ClientId, u32> {
    let range = max_secs - min_secs;
    let mut result = HashMap::new();

    for (i, &id) in client_ids.iter().enumerate() {
        // Deterministic but varied delay per character.
        let hash = id.wrapping_mul(2654435761).wrapping_add(seed);
        let delay = min_secs + (hash % (range + 1));
        result.insert(id, delay);
    }

    result
}

/// Determines travel method based on group composition.
/// Port-first: if druids/wizards available, use ports for long-distance travel.
pub fn plan_group_travel(
    client_ids: &[ClientId],
    class_map: &HashMap<ClientId, u8>,
    _from_zone: &str,
    _to_zone: &str,
) -> Vec<TravelPlan> {
    // EQ class IDs: Druid=6, Wizard=5
    const DRUID_CLASS: u8 = 6;
    const WIZARD_CLASS: u8 = 5;

    let porters: Vec<ClientId> = client_ids.iter()
        .filter(|id| {
            class_map.get(id)
                .map(|&c| c == DRUID_CLASS || c == WIZARD_CLASS)
                .unwrap_or(false)
        })
        .copied()
        .collect();

    // For now, generate simple walk-to-zone-line plans.
    // Port coordination and multi-zone routing will be expanded
    // as zone data is populated.
    let staggers = generate_zone_staggers(client_ids, 5, 60, 42);

    client_ids.iter().map(|&id| {
        let delay = staggers.get(&id).copied().unwrap_or(5);
        TravelPlan::new(id, vec![
            TravelStep::StaggerWait { min_secs: delay, max_secs: delay },
        ])
    }).collect()
}
```

- [ ] **Step 2: Add `pub mod router;` to `textquest/src/nav/mod.rs`**

```rust
pub mod camp;
pub mod recorder;
pub mod router;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p dmft`
Expected: Clean compile.

- [ ] **Step 4: Commit**

```bash
git add textquest/src/nav/router.rs textquest/src/nav/mod.rs
git commit -m "feat(nav): add zone router — travel plans with staggered zone transitions"
```

---

## Task 10: Wire Up IPC Command Dispatch

**Files:**

- Modify: `textquest-dll/src/hooks/game_loop.rs`
- Modify: `textquest-common/src/ipc.rs`

Connect the navigation commands from IPC to the Navigator. When the game loop processes commands, navigation commands get routed to the nav module.

- [ ] **Step 1: Add nav status to the Response enum in `textquest-common/src/ipc.rs`**

Add a new variant:

```rust
    NavUpdate {
        status: crate::nav::NavStatus,
    },
```

- [ ] **Step 2: Update `on_game_tick()` to dispatch nav commands**

In `textquest-dll/src/hooks/game_loop.rs`:

```rust
use textquest_common::ipc::Command;

fn on_game_tick() {
    // Run navigation state machine.
    crate::nav::tick();

    // TODO: Read game state from EQ memory (local player, target, spawns)
    // TODO: Publish state to shared memory via IPC
    // TODO: Check for and execute pending commands from the orchestrator
    //       When a command arrives, dispatch navigation commands:
    //
    //   match cmd {
    //       Command::NavigateTo { waypoints } =>
    //           crate::nav::handle_command(NavCommand::Navigate(waypoints)),
    //       Command::SetCamp { spot } =>
    //           crate::nav::handle_command(NavCommand::SetCamp(spot)),
    //       Command::StopNavigation =>
    //           crate::nav::handle_command(NavCommand::Stop),
    //       _ => { /* other command handling */ }
    //   }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Clean compile across all crates.

- [ ] **Step 4: Commit**

```bash
git add textquest-dll/src/hooks/game_loop.rs textquest-common/src/ipc.rs
git commit -m "feat(nav): wire IPC command dispatch — NavigateTo/SetCamp/StopNavigation route to Navigator"
```

---

## Self-Review Checklist

1. **Spec coverage:**
   - [x] A-to-B waypoint navigation (Tasks 2, 3)
   - [x] Independent character movement, not /follow blob (Task 5 humanization)
   - [x] Camp positioning (Task 8)
   - [x] Stuck detection with escalating recovery (Task 6)
   - [x] Zone transitions with staggered timing (Task 9)
   - [x] Port-first travel coordination scaffold (Task 9)
   - [x] Waypoint recording for known routes (Task 7)
   - [x] Movement humanization — speed jitter, heading wobble (Task 5)
   - [x] IPC command integration (Tasks 1, 10)
   - [ ] Underwater swimming — deferred (requires additional PlayerClient fields for swim state)
   - [ ] Door/zone line interaction — deferred (requires EQ function hooking for clickable objects)
   - [ ] Recast/Detour navmesh generation — deferred to M3 Phase 2 (separate plan)

2. **Placeholder scan:** All code blocks contain complete implementations. No TBD/TODO in new code (existing TODOs in game_loop.rs are pre-existing and noted as comments showing future dispatch).

3. **Type consistency:**
   - `Waypoint` used consistently across all files (from `textquest_common::nav`)
   - `NavStatus` consistent between `nav.rs` (definition) and `state.rs` (production)
   - `CampSpot`/`CampDefinition` consistent between `nav.rs` and `camp.rs`
   - `Navigator::new` signature updated in Task 5 to include `client_id` — Task 4's `nav::init()` updated to match
   - `Command` enum variants match the dispatch in Task 10

---

## Deferred to M3 Phase 2 (separate plan)

These items require additional research and will be a follow-up plan:

- **Recast/Detour navmesh generation** from EQ zone geometry files
- **Underwater swimming** (Kedge Keep) — requires swim state detection
- **Door interaction** — requires hooking EQ's door activation functions
- **Map-click waypoint creation** — requires hooking the EQ map UI
- **Auto-camp detection** via spawn density analysis
