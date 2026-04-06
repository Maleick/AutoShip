//! Movement control -- provides functions to move the player character.
//! Writes directly to `PlayerClient` struct fields in EQ memory.
//! The game engine reads these values each tick to process movement.

use dmft_common::nav::Waypoint;

/// Arrival threshold in game units (close enough to "be there").
pub const ARRIVAL_DISTANCE: f32 = 15.0;

/// EQ command IDs for `ExecuteCmd`.
pub const CMD_AUTORUN: u32 = 0;
pub const CMD_JUMP: u32 = 1;
pub const CMD_FORWARD: u32 = 2;
pub const CMD_BACK: u32 = 3;

/// Calculate heading from current position to target (EQ heading: 0-512, 0=north, increases CW).
///
/// Matches MQ2's formula from MQCommands.cpp `/face`:
///   `atan2(target.x - player.x, target.y - player.y) * 256 / PI`
/// which is equivalent to `atan2(dx, dy) * 256 / PI` mapped to 0..512.
pub fn calc_heading(from: &Waypoint, to: &Waypoint) -> f32 {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let rad = dx.atan2(dy);
    // 256/PI converts radians to EQ heading units (512 = full circle)
    let heading = rad * 256.0 / std::f32::consts::PI;
    // Normalize to 0..512
    (heading + 512.0) % 512.0
}

/// Movement controller state -- holds a pointer to the local player's
/// `PlayerClient` struct for direct memory writes.
///
/// On non-Windows, all write operations are no-ops logged via tracing.
pub struct MovementController {
    /// Base address of the local `PlayerClient` struct.
    player_base: usize,
}

impl MovementController {
    /// Create a new controller targeting the given `PlayerClient` address.
    pub fn new(player_base: usize) -> Self {
        Self { player_base }
    }

    /// Check if the player base pointer is likely valid (non-null).
    pub fn is_valid(&self) -> bool {
        self.player_base != 0
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
        // SAFETY: player_base is a PlayerClient* obtained from PINST_LOCAL_PLAYER.
        // Null-checked above. HEADING is a known f32 field offset within
        // PlayerClient. Writing a f32 to an aligned address within committed
        // memory is safe. The game reads this value each tick for facing direction.
        unsafe {
            if self.player_base == 0 {
                return;
            }
            let addr = self.player_base + dmft_common::offsets::player_base::HEADING;
            std::ptr::write(addr as *mut f32, heading);
        }
        #[cfg(not(windows))]
        tracing::trace!(heading, "write_heading (stub)");
    }

    /// Write speed heading (direction of actual movement).
    pub fn write_speed_heading(&self, heading: f32) {
        #[cfg(windows)]
        // SAFETY: Same invariants as write_heading — player_base is a validated
        // PlayerClient*, SPEED_HEADING is a known f32 field offset.
        unsafe {
            if self.player_base == 0 {
                return;
            }
            let addr = self.player_base + dmft_common::offsets::player_base::SPEED_HEADING;
            std::ptr::write(addr as *mut f32, heading);
        }
        #[cfg(not(windows))]
        tracing::trace!(heading, "write_speed_heading (stub)");
    }

    /// Read current position from the `PlayerClient` struct.
    pub fn read_position(&self) -> Waypoint {
        #[cfg(windows)]
        // SAFETY: player_base is a validated PlayerClient*. X, Y, Z are known
        // f32 field offsets within the struct. Reads are naturally aligned and
        // within committed eqgame process memory. Null-checked above.
        unsafe {
            if self.player_base == 0 {
                return Waypoint::new(0.0, 0.0, 0.0);
            }
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

    /// Press forward key (start walking).
    pub fn press_forward(&self) {
        self.execute_cmd(CMD_FORWARD, true);
    }

    /// Release forward key (stop walking).
    pub fn stop_forward(&self) {
        self.execute_cmd(CMD_FORWARD, false);
    }

    /// Press backward key (start backing up).
    pub fn press_back(&self) {
        self.execute_cmd(CMD_BACK, true);
    }

    /// Release backward key (stop backing up).
    pub fn stop_back(&self) {
        self.execute_cmd(CMD_BACK, false);
    }

    /// Call EQ's __ExecuteCmd to simulate key presses.
    /// Signature: void __ExecuteCmd(uint32_t command, bool keyDown, void* data, void* pTarget)
    pub fn execute_cmd(&self, command: u32, key_down: bool) {
        #[cfg(windows)]
        {
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
            if eq_base == 0 {
                return;
            }
            if let Some(addr) =
                dmft_common::offsets::rebase(dmft_common::offsets::EXECUTE_CMD, eq_base)
            {
                type ExecuteCmdFn =
                    unsafe extern "C" fn(command: u32, key_down: i32, data: usize, target: usize);
                // SAFETY: addr was rebased from EXECUTE_CMD — a known function
                // address in eqgame.exe. The transmute converts it to match
                // __ExecuteCmd's calling convention. If the offset is wrong,
                // this will crash EQ. data=0 and target=0 are valid (no item,
                // no specific target).
                unsafe {
                    let func: ExecuteCmdFn = std::mem::transmute(addr);
                    func(command, i32::from(key_down), 0, 0);
                }
            }
        }
        #[cfg(not(windows))]
        tracing::trace!(command, key_down, "execute_cmd (stub)");
    }

    /// Read current heading.
    pub fn read_heading(&self) -> f32 {
        #[cfg(windows)]
        // SAFETY: Same invariants as read_position — player_base is a validated
        // PlayerClient*, HEADING is a known f32 field offset. Null-checked below.
        unsafe {
            if self.player_base == 0 {
                return 0.0;
            }
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
