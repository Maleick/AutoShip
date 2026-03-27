//! Movement control -- provides functions to move the player character.
//! Writes directly to PlayerClient struct fields in EQ memory.
//! The game engine reads these values each tick to process movement.

use dmft_common::nav::Waypoint;

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
            let addr = self.player_base + dmft_common::offsets::player_base::HEADING;
            std::ptr::write(addr as *mut f32, heading);
        }
        #[cfg(not(windows))]
        tracing::trace!(heading, "write_heading (stub)");
    }

    /// Write speed heading (direction of actual movement).
    pub fn write_speed_heading(&self, heading: f32) {
        #[cfg(windows)]
        unsafe {
            let addr = self.player_base + dmft_common::offsets::player_base::SPEED_HEADING;
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
            let y = std::ptr::read((base + dmft_common::offsets::player_base::Y) as *const f32);
            let x = std::ptr::read((base + dmft_common::offsets::player_base::X) as *const f32);
            let z = std::ptr::read((base + dmft_common::offsets::player_base::Z) as *const f32);
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
            std::ptr::read(
                (self.player_base + dmft_common::offsets::player_base::HEADING) as *const f32,
            )
        }
        #[cfg(not(windows))]
        {
            tracing::trace!("read_heading (stub)");
            0.0
        }
    }
}
