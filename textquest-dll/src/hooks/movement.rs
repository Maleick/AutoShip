//! Movement control -- provides functions to move the player character.
//! Writes directly to `PlayerClient` struct fields in EQ memory.
//! The game engine reads these values each tick to process movement.

use textquest_common::nav::Waypoint;
pub use textquest_common::nav::calc_heading;

use textquest_common::eq_fn;

eq_fn!(execute_cmd_fn(command: u32, key_down: i32, data: usize, target: usize) -> () = textquest_common::offsets::EXECUTE_CMD);

/// Arrival threshold in game units (close enough to "be there").
pub const ARRIVAL_DISTANCE: f32 = 15.0;

/// EQ command IDs for `ExecuteCmd`.
pub const CMD_AUTORUN: u32 = 0;
pub const CMD_JUMP: u32 = 1;
pub const CMD_FORWARD: u32 = 2;
pub const CMD_BACK: u32 = 3;

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
            let addr = self.player_base + textquest_common::offsets::player_base::HEADING;
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
            let addr = self.player_base + textquest_common::offsets::player_base::SPEED_HEADING;
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
            let y =
                std::ptr::read((base + textquest_common::offsets::player_base::Y) as *const f32);
            let x =
                std::ptr::read((base + textquest_common::offsets::player_base::X) as *const f32);
            let z =
                std::ptr::read((base + textquest_common::offsets::player_base::Z) as *const f32);
            Waypoint::new(x, y, z)
        }
        #[cfg(not(windows))]
        {
            tracing::trace!("read_position (stub)");
            Waypoint::new(0.0, 0.0, 0.0)
        }
    }

    /// Read current player HP from the local player struct.
    pub fn read_hp_current(&self) -> Option<i64> {
        #[cfg(any(windows, test))]
        // SAFETY: player_base is a validated PlayerClient*. HP_CURRENT is a known
        // i64 field offset within the local player zone data. Reads are naturally
        // aligned and within committed EQ memory. Null-checked below. Tests may
        // provide a synthetic backing buffer at the same offset.
        unsafe {
            if self.player_base == 0 {
                return None;
            }
            Some(std::ptr::read(
                (self.player_base + textquest_common::offsets::player_zone::HP_CURRENT)
                    as *const i64,
            ))
        }
        #[cfg(all(not(windows), not(test)))]
        {
            tracing::trace!("read_hp_current (stub)");
            None
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
            // SAFETY: addr resolution and transmute are performed in the shared
            // binding layer. If EXECUTE_CMD is stale or wrong, behavior is still
            // best-effort via offset_db fallback; if both paths resolve invalid,
            // this call is expected to be guarded by EQ process correctness checks.
            unsafe {
                execute_cmd_fn.call(eq_base, command, i32::from(key_down), 0, 0);
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
                (self.player_base + textquest_common::offsets::player_base::HEADING) as *const f32,
            )
        }
        #[cfg(not(windows))]
        {
            tracing::trace!("read_heading (stub)");
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::nav::Waypoint;

    fn wp(x: f32, y: f32) -> Waypoint {
        Waypoint { x, y, z: 0.0 }
    }

    // --- calc_heading ---
    // EQ headings wrap: 0 and 512 represent the same direction (north).
    // Tests accept both ~0 and ~512 for north-facing headings.

    #[test]
    fn heading_north() {
        // Target directly north (+Y) from origin → heading 0 (or 512, equivalent)
        let heading = calc_heading(&wp(0.0, 0.0), &wp(0.0, 100.0));
        assert!(
            heading.abs() < 1.0 || (heading - 512.0).abs() < 1.0,
            "expected ~0 (north), got {heading}"
        );
    }

    #[test]
    fn heading_south() {
        // Target directly south (-Y) → heading 256 (half circle)
        let heading = calc_heading(&wp(0.0, 0.0), &wp(0.0, -100.0));
        assert!(
            (heading - 256.0).abs() < 1.0,
            "expected ~256 (south), got {heading}"
        );
    }

    #[test]
    fn heading_east() {
        // Target directly east (+X) → heading 128 (quarter turn CW)
        let heading = calc_heading(&wp(0.0, 0.0), &wp(100.0, 0.0));
        assert!(
            (heading - 128.0).abs() < 1.0,
            "expected ~128 (east), got {heading}"
        );
    }

    #[test]
    fn heading_west() {
        // Target directly west (-X) → heading 384 (three-quarter turn CW)
        let heading = calc_heading(&wp(0.0, 0.0), &wp(-100.0, 0.0));
        assert!(
            (heading - 384.0).abs() < 1.0,
            "expected ~384 (west), got {heading}"
        );
    }

    #[test]
    fn heading_northeast() {
        // 45 degrees NE → heading ~64 (128/2)
        let heading = calc_heading(&wp(0.0, 0.0), &wp(100.0, 100.0));
        assert!(
            (heading - 64.0).abs() < 1.0,
            "expected ~64 (NE), got {heading}"
        );
    }

    #[test]
    fn heading_same_position_returns_zero() {
        // Same position → atan2(0,0) = 0 → heading 0
        let heading = calc_heading(&wp(5.0, 5.0), &wp(5.0, 5.0));
        // atan2(0,0) is 0 in Rust, so (0*256/PI + 512) % 512 = 0
        assert!(
            heading.abs() < 1.0 || (heading - 512.0).abs() < 1.0,
            "expected ~0 for same position, got {heading}"
        );
    }

    #[test]
    fn heading_always_in_range() {
        // Verify heading is always in [0, 512) for various directions
        let directions = [
            (1.0, 0.0),
            (0.0, 1.0),
            (-1.0, 0.0),
            (0.0, -1.0),
            (1.0, 1.0),
            (-1.0, 1.0),
            (1.0, -1.0),
            (-1.0, -1.0),
            (0.001, 1000.0),
            (1000.0, 0.001),
        ];
        for (dx, dy) in &directions {
            let heading = calc_heading(&wp(0.0, 0.0), &wp(*dx, *dy));
            assert!(
                (0.0..512.0).contains(&heading),
                "heading {heading} out of range for dx={dx}, dy={dy}"
            );
        }
    }

    #[test]
    fn heading_offset_origin() {
        // Same relative direction from a non-zero origin should give same heading
        let h1 = calc_heading(&wp(0.0, 0.0), &wp(100.0, 0.0));
        let h2 = calc_heading(&wp(500.0, 500.0), &wp(600.0, 500.0));
        assert!(
            (h1 - h2).abs() < 0.01,
            "heading should be the same regardless of origin: {h1} vs {h2}"
        );
    }

    #[test]
    fn heading_opposite_directions_differ_by_256() {
        let h_east = calc_heading(&wp(0.0, 0.0), &wp(100.0, 0.0));
        let h_west = calc_heading(&wp(0.0, 0.0), &wp(-100.0, 0.0));
        let diff = (h_west - h_east).abs();
        assert!(
            (diff - 256.0).abs() < 1.0,
            "opposite headings should differ by ~256, got {diff}"
        );
    }

    // --- MovementController ---

    #[test]
    fn controller_new_and_valid() {
        let ctrl = MovementController::new(0x1000);
        assert!(ctrl.is_valid());

        let ctrl_null = MovementController::new(0);
        assert!(!ctrl_null.is_valid());
    }

    #[test]
    fn controller_set_player_base() {
        let mut ctrl = MovementController::new(0);
        assert!(!ctrl.is_valid());
        ctrl.set_player_base(0xDEAD);
        assert!(ctrl.is_valid());
    }

    #[test]
    fn controller_face_toward_does_not_panic() {
        // On non-Windows this is a no-op stub, just ensure it doesn't crash
        let ctrl = MovementController::new(0x1000);
        let target = wp(100.0, 200.0);
        let current = wp(50.0, 50.0);
        ctrl.face_toward(&target, &current);
    }

    #[test]
    fn controller_read_position_stub() {
        // On non-Windows the stub returns (0, 0, 0)
        let ctrl = MovementController::new(0x1000);
        let pos = ctrl.read_position();
        #[cfg(not(windows))]
        {
            assert_eq!(pos.x, 0.0);
            assert_eq!(pos.y, 0.0);
            assert_eq!(pos.z, 0.0);
        }
        let _ = pos; // suppress unused on Windows
    }

    #[test]
    fn controller_read_heading_stub() {
        let ctrl = MovementController::new(0x1000);
        let heading = ctrl.read_heading();
        #[cfg(not(windows))]
        assert_eq!(heading, 0.0);
        let _ = heading;
    }
}
