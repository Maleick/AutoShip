//! Targeting control -- set, clear, and query the current target.
//!
//! This is a function-call API rather than a detour hook -- we write
//! directly to EQ's target pointer (pinstTarget) in memory. On
//! non-Windows platforms all operations are no-ops that log a warning.
//!
//! Full functionality requires resolved EQ base address + offsets.
//! Once the offset database is wired up, these functions will be able
//! to walk the spawn list and resolve spawn IDs to memory addresses.

use textquest_common::offsets;

/// Error type for targeting operations.
#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("EQ base address not set -- cannot resolve pointers")]
    NoBaseAddress,
    #[error("spawn {0} not found in spawn list")]
    SpawnNotFound(u32),
    #[error("target pointer address could not be rebased")]
    RebaseFailed,
    #[error("targeting not available on this platform")]
    PlatformStub,
    #[error("assist target pointer is not a live spawn")]
    InvalidAssistTarget,
}

/// Holds the resolved runtime address of pinstTarget and the EQ module base.
/// Created once the DLL knows the actual EQ base address.
pub struct TargetingController {
    /// Runtime base address of eqgame.exe (actual, not preferred).
    eq_base: u64,
}

impl TargetingController {
    /// Create a new controller with the given EQ module base address.
    pub fn new(eq_base: u64) -> Self {
        Self { eq_base }
    }

    /// Update the EQ base address (e.g., if module is reloaded).
    pub fn set_eq_base(&mut self, eq_base: u64) {
        self.eq_base = eq_base;
    }

    /// Resolve the runtime address of pinstTarget.
    fn target_ptr_addr(&self) -> Result<usize, TargetError> {
        if self.eq_base == 0 {
            return Err(TargetError::NoBaseAddress);
        }
        offsets::rebase(offsets::PINST_TARGET, self.eq_base).ok_or(TargetError::RebaseFailed)
    }

    /// Set the current target by writing a spawn's address to pinstTarget.
    ///
    /// `spawn_addr` is the runtime address of the target `PlayerClient` struct.
    /// Use `set_target_by_id` once spawn-list walking is implemented.
    pub fn set_target_by_addr(&self, spawn_addr: usize) -> Result<(), TargetError> {
        let pinst_addr = self.target_ptr_addr()?;
        tracing::debug!(
            pinst_addr = format!("{:#x}", pinst_addr),
            spawn_addr = format!("{:#x}", spawn_addr),
            "setting target"
        );

        #[cfg(windows)]
        // SAFETY: pinst_addr is the rebased pinstTarget global pointer in
        // eqgame.exe. Writing a spawn address (or 0 for clear) to it sets the
        // current target. The caller is responsible for providing a valid
        // PlayerClient address. If spawn_addr points to a freed spawn, EQ may
        // crash on the next tick when it dereferences the target pointer.
        unsafe {
            std::ptr::write(pinst_addr as *mut usize, spawn_addr);
        }

        #[cfg(not(windows))]
        {
            tracing::warn!(
                pinst_addr = format!("{:#x}", pinst_addr),
                spawn_addr = format!("{:#x}", spawn_addr),
                "set_target_by_addr (stub -- non-Windows)"
            );
        }

        Ok(())
    }

    /// Set the current target by spawn ID.
    ///
    /// Walks the spawn linked list to find the `PlayerClient` with the matching
    /// spawn ID, then writes its address to pinstTarget.
    pub fn set_target(&self, spawn_id: u32) -> Result<(), TargetError> {
        let spawn_addr = self.find_spawn_addr(spawn_id)?;
        self.set_target_by_addr(spawn_addr)
    }

    /// Clear the current target by writing null to pinstTarget.
    pub fn clear_target(&self) -> Result<(), TargetError> {
        let pinst_addr = self.target_ptr_addr()?;
        tracing::debug!(pinst_addr = format!("{:#x}", pinst_addr), "clearing target");

        #[cfg(windows)]
        // SAFETY: pinst_addr is the rebased pinstTarget global. Writing 0
        // clears the current target. This is always safe — EQ null-checks
        // the target pointer before use.
        unsafe {
            std::ptr::write(pinst_addr as *mut usize, 0usize);
        }

        #[cfg(not(windows))]
        tracing::warn!(
            pinst_addr = format!("{:#x}", pinst_addr),
            "clear_target (stub -- non-Windows)"
        );

        Ok(())
    }

    /// Read the current target's spawn ID, if any target is set.
    pub fn get_current_target_id(&self) -> Result<Option<u32>, TargetError> {
        let _pinst_addr = self.target_ptr_addr()?;

        #[cfg(windows)]
        // SAFETY: _pinst_addr is the rebased pinstTarget global. Reading a
        // usize yields the target's PlayerClient* (null = no target). If
        // non-null, SPAWN_ID is a known u32 field within PlayerClient. If the
        // target was cleared between the two reads (race), spawn_id may be
        // stale but reading from recently-freed memory won't segfault because
        // the page remains committed.
        unsafe {
            let target_ptr = std::ptr::read(_pinst_addr as *const usize);
            if target_ptr == 0 {
                return Ok(None);
            }
            let spawn_id =
                std::ptr::read((target_ptr + offsets::player_base::SPAWN_ID) as *const u32);
            return Ok(Some(spawn_id));
        }

        #[cfg(not(windows))]
        {
            tracing::warn!("get_current_target_id (stub -- non-Windows)");
            Ok(None)
        }
    }

    /// Target the nearest NPC within range.
    ///
    /// Walks the spawn list, filters to NPCs, finds the nearest one within
    /// `max_range` game units, and sets it as the current target.
    /// Returns the spawn ID of the new target, or `None` if none found.
    pub fn target_nearest_npc(&self, max_range: f32) -> Result<Option<u32>, TargetError> {
        if self.eq_base == 0 {
            return Err(TargetError::NoBaseAddress);
        }

        #[cfg(windows)]
        // SAFETY: All pointer dereferences follow EQ's known struct layout.
        // We read the local player position from PINST_LOCAL_PLAYER, then walk
        // the TList<PlayerClient*> via SpawnManager::PLAYER_LIST. Each node's
        // TYPE, Y, X, Z, SPAWN_ID, and NEXT fields are at offsets from MQ2
        // headers. The MAX_SPAWNS cap prevents infinite loops on list corruption.
        // Null checks guard every pointer dereference.
        unsafe {
            // 1. Read local player position.
            let local_player_pinst = offsets::rebase(offsets::PINST_LOCAL_PLAYER, self.eq_base)
                .ok_or(TargetError::RebaseFailed)?;
            let local_player_ptr = std::ptr::read(local_player_pinst as *const usize);
            if local_player_ptr == 0 {
                tracing::debug!("target_nearest_npc: local player ptr is null");
                return Ok(None);
            }
            let player_y =
                std::ptr::read((local_player_ptr + offsets::player_base::Y) as *const f32);
            let player_x =
                std::ptr::read((local_player_ptr + offsets::player_base::X) as *const f32);
            let player_z =
                std::ptr::read((local_player_ptr + offsets::player_base::Z) as *const f32);

            // 2. Walk spawn list from SpawnManager.
            let mgr_pinst = offsets::rebase(offsets::PINST_SPAWN_MANAGER, self.eq_base)
                .ok_or(TargetError::RebaseFailed)?;
            let mgr_ptr = std::ptr::read(mgr_pinst as *const usize);
            if mgr_ptr == 0 {
                tracing::debug!("target_nearest_npc: spawn manager ptr is null");
                return Ok(None);
            }
            let list_head_ptr = mgr_ptr + offsets::spawn_manager::PLAYER_LIST;
            let mut current = std::ptr::read(list_head_ptr as *const usize);

            const MAX_SPAWNS: u32 = 5000;
            let mut count = 0u32;
            let mut best_addr: Option<usize> = None;
            let mut best_dist_sq = max_range * max_range;
            let mut best_id: u32 = 0;

            while current != 0 && count < MAX_SPAWNS {
                // 3. Filter to TYPE == 1 (NPC).
                let spawn_type =
                    std::ptr::read((current + offsets::player_base::TYPE) as *const u8);
                if spawn_type == 1 {
                    // 4. Compute distance, keep track of nearest within range.
                    let sy = std::ptr::read((current + offsets::player_base::Y) as *const f32);
                    let sx = std::ptr::read((current + offsets::player_base::X) as *const f32);
                    let sz = std::ptr::read((current + offsets::player_base::Z) as *const f32);
                    let dy = sy - player_y;
                    let dx = sx - player_x;
                    let dz = sz - player_z;
                    let dist_sq = dy * dy + dx * dx + dz * dz;
                    if dist_sq < best_dist_sq {
                        best_dist_sq = dist_sq;
                        best_addr = Some(current);
                        best_id = std::ptr::read(
                            (current + offsets::player_base::SPAWN_ID) as *const u32,
                        );
                    }
                }
                current = std::ptr::read((current + offsets::player_base::NEXT) as *const usize);
                count += 1;
            }

            if let Some(addr) = best_addr {
                // 5. Set as current target.
                tracing::debug!(
                    spawn_id = best_id,
                    dist = (best_dist_sq.sqrt()),
                    "target_nearest_npc: found target"
                );
                self.set_target_by_addr(addr)?;
                Ok(Some(best_id))
            } else {
                tracing::debug!(max_range, "target_nearest_npc: no NPC in range");
                Ok(None)
            }
        }

        #[cfg(not(windows))]
        {
            tracing::warn!(max_range, "target_nearest_npc (stub -- non-Windows)");
            Ok(None)
        }
    }

    /// Assist another character -- target their target.
    ///
    /// Finds the `PlayerClient` for `assist_spawn_id`, reads its `ManagedTarget`
    /// pointer (the spawn they are currently targeting), then writes that pointer
    /// to our own `pinstTarget`. If the assist target has no target, clears ours.
    pub fn assist(&self, assist_spawn_id: u32) -> Result<(), TargetError> {
        #[cfg(windows)]
        // SAFETY: assist_addr is a valid PlayerClient* returned by find_spawn_addr,
        // which already validated the pointer via the spawn list walk. Reading
        // MANAGED_TARGET at a known offset yields the spawn's current target
        // pointer (PlayerClient*) or 0 if they have no target. Writing this
        // value to pinstTarget mirrors EQ's own targeting write path.
        unsafe {
            // 1. Find the assist target's PlayerClient by spawn ID.
            let assist_addr = self.find_spawn_addr(assist_spawn_id)?;

            // 2. Read the assist target's ManagedTarget pointer.
            let their_target_ptr = std::ptr::read(
                (assist_addr + offsets::player_base::MANAGED_TARGET) as *const usize,
            );

            if their_target_ptr == 0 {
                // Assist target has no target — clear ours.
                tracing::debug!(
                    assist_spawn_id,
                    "assist: assist target has no target, clearing"
                );
                return self.clear_target();
            }

            // 3. Validate that the target-of-target is still present in the
            // spawn list before reading any fields or writing pinstTarget.
            let tot_id = self.find_spawn_id_by_addr(their_target_ptr)?;
            tracing::debug!(
                assist_spawn_id,
                target_of_target_id = tot_id,
                target_addr = format!("{:#x}", their_target_ptr),
                "assist: setting target to assist target's target"
            );

            // Write the target-of-target address to our pinstTarget.
            self.set_target_by_addr(their_target_ptr)
        }

        #[cfg(not(windows))]
        {
            tracing::warn!(assist_spawn_id, "assist (stub -- non-Windows)");
            Err(TargetError::PlatformStub)
        }
    }

    /// Walk the spawn linked list to find a spawn by ID. Returns its address.
    ///
    /// Traverses the `TList`<`PlayerClient`*> starting from `SpawnManager`'s player
    /// list, following NEXT pointers until a matching `SPAWN_ID` is found.
    fn find_spawn_addr(&self, spawn_id: u32) -> Result<usize, TargetError> {
        if self.eq_base == 0 {
            return Err(TargetError::NoBaseAddress);
        }

        #[cfg(windows)]
        // SAFETY: All pointer dereferences follow EQ's known struct layout:
        // PINST_SPAWN_MANAGER → SpawnManager* → PLAYER_LIST → linked list of
        // PlayerClient nodes. Each node's SPAWN_ID and NEXT fields are at known
        // offsets derived from MQ2 headers. The MAX_SPAWNS cap prevents infinite
        // loops on corrupted linked list data. If any pointer is null or invalid,
        // we return SpawnNotFound rather than crashing.
        unsafe {
            let mgr_pinst = offsets::rebase(offsets::PINST_SPAWN_MANAGER, self.eq_base)
                .ok_or(TargetError::RebaseFailed)?;
            let mgr_ptr = std::ptr::read(mgr_pinst as *const usize);
            if mgr_ptr == 0 {
                return Err(TargetError::SpawnNotFound(spawn_id));
            }

            // TList head is at SpawnManager + PLAYER_LIST, which contains a
            // pointer to the first node.
            let list_head_ptr = mgr_ptr + offsets::spawn_manager::PLAYER_LIST;
            let mut current = std::ptr::read(list_head_ptr as *const usize);

            // Safety cap to avoid infinite loops on corrupted data.
            const MAX_SPAWNS: u32 = 5000;
            let mut count = 0u32;

            while current != 0 && count < MAX_SPAWNS {
                let sid = std::ptr::read((current + offsets::player_base::SPAWN_ID) as *const u32);
                if sid == spawn_id {
                    return Ok(current);
                }
                current = std::ptr::read((current + offsets::player_base::NEXT) as *const usize);
                count += 1;
            }

            Err(TargetError::SpawnNotFound(spawn_id))
        }

        #[cfg(not(windows))]
        {
            tracing::warn!(spawn_id, "find_spawn_addr (stub -- non-Windows)");
            Err(TargetError::PlatformStub)
        }
    }

    /// Walk the spawn list to verify a spawn address is still live and get its ID.
    fn find_spawn_id_by_addr(&self, spawn_addr: usize) -> Result<u32, TargetError> {
        if self.eq_base == 0 {
            return Err(TargetError::NoBaseAddress);
        }

        #[cfg(windows)]
        unsafe {
            let is_aligned = |addr: usize, align: usize| -> bool {
                addr != 0 && addr % align == 0
            };

            let read_usize_checked = |addr: usize| -> Result<usize, TargetError> {
                if !is_aligned(addr, std::mem::align_of::<usize>())
                    || !crate::hooks::game_loop::is_readable(
                        addr,
                        std::mem::size_of::<usize>(),
                    )
                {
                    return Err(TargetError::InvalidAssistTarget);
                }

                Ok(std::ptr::read(addr as *const usize))
            };

            let read_u32_checked = |addr: usize| -> Result<u32, TargetError> {
                if !is_aligned(addr, std::mem::align_of::<u32>())
                    || !crate::hooks::game_loop::is_readable(
                        addr,
                        std::mem::size_of::<u32>(),
                    )
                {
                    return Err(TargetError::InvalidAssistTarget);
                }

                Ok(std::ptr::read(addr as *const u32))
            };

            let mgr_pinst = offsets::rebase(offsets::PINST_SPAWN_MANAGER, self.eq_base)
                .ok_or(TargetError::RebaseFailed)?;
            let mgr_ptr = read_usize_checked(mgr_pinst as usize)?;
            if mgr_ptr == 0 {
                return Err(TargetError::InvalidAssistTarget);
            }

            let list_head_ptr = mgr_ptr + offsets::spawn_manager::PLAYER_LIST;
            let mut current = read_usize_checked(list_head_ptr)?;

            const MAX_SPAWNS: u32 = 5000;
            let mut count = 0u32;

            while current != 0 && count < MAX_SPAWNS {
                if current == spawn_addr {
                    let sid = read_u32_checked(current + offsets::player_base::SPAWN_ID)?;
                    return Ok(sid);
                }
                current = read_usize_checked(current + offsets::player_base::NEXT)?;
                count += 1;
            }

            Err(TargetError::InvalidAssistTarget)
        }

        #[cfg(not(windows))]
        {
            let _ = spawn_addr;
            Err(TargetError::PlatformStub)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── TargetingController construction ────────────────────────────────────

    #[test]
    fn new_sets_eq_base() {
        let ctrl = TargetingController::new(0x1_4000_0000);
        assert_eq!(ctrl.eq_base, 0x1_4000_0000);
    }

    #[test]
    fn set_eq_base_updates_value() {
        let mut ctrl = TargetingController::new(0x1_4000_0000);
        ctrl.set_eq_base(0x2_0000_0000);
        assert_eq!(ctrl.eq_base, 0x2_0000_0000);
    }

    // ── target_ptr_addr: no base address ────────────────────────────────────

    #[test]
    fn target_ptr_addr_errors_on_zero_base() {
        let ctrl = TargetingController::new(0);
        match ctrl.target_ptr_addr() {
            Err(TargetError::NoBaseAddress) => {}
            other => panic!("expected NoBaseAddress, got {other:?}"),
        }
    }

    // ── find_spawn_addr: no base address ────────────────────────────────────

    #[test]
    fn find_spawn_addr_errors_on_zero_base() {
        let ctrl = TargetingController::new(0);
        match ctrl.find_spawn_addr(1) {
            Err(TargetError::NoBaseAddress) => {}
            other => panic!("expected NoBaseAddress, got {other:?}"),
        }
    }

    // ── target_nearest_npc: no base address ─────────────────────────────────

    #[test]
    fn target_nearest_npc_errors_on_zero_base() {
        let ctrl = TargetingController::new(0);
        match ctrl.target_nearest_npc(100.0) {
            Err(TargetError::NoBaseAddress) => {}
            other => panic!("expected NoBaseAddress, got {other:?}"),
        }
    }

    // ── assist: no base address ──────────────────────────────────────────────

    #[test]
    #[cfg(windows)]
    fn assist_errors_on_zero_base_windows() {
        let ctrl = TargetingController::new(0);
        // find_spawn_addr is called first inside the cfg(windows) block and
        // will return NoBaseAddress because eq_base == 0.
        match ctrl.assist(42) {
            Err(TargetError::NoBaseAddress) => {}
            other => panic!("expected NoBaseAddress, got {other:?}"),
        }
    }

    #[test]
    #[cfg(not(windows))]
    fn assist_errors_platform_stub_on_non_windows_any_base() {
        // On non-Windows, assist returns PlatformStub regardless of eq_base
        // because find_spawn_addr is only called inside #[cfg(windows)].
        let ctrl = TargetingController::new(0);
        match ctrl.assist(42) {
            Err(TargetError::PlatformStub) => {}
            other => panic!("expected PlatformStub, got {other:?}"),
        }
    }

    // ── spawn-list walking: stub returns PlatformStub on non-Windows ─────────

    #[test]
    #[cfg(not(windows))]
    fn find_spawn_addr_returns_platform_stub_on_non_windows() {
        let ctrl = TargetingController::new(0x1_4000_0000);
        match ctrl.find_spawn_addr(99) {
            Err(TargetError::PlatformStub) => {}
            other => panic!("expected PlatformStub, got {other:?}"),
        }
    }

    #[test]
    #[cfg(not(windows))]
    fn target_nearest_npc_returns_none_on_non_windows() {
        let ctrl = TargetingController::new(0x1_4000_0000);
        assert_eq!(ctrl.target_nearest_npc(200.0).unwrap(), None);
    }

    #[test]
    #[cfg(not(windows))]
    fn assist_returns_platform_stub_on_non_windows() {
        let ctrl = TargetingController::new(0x1_4000_0000);
        match ctrl.assist(1) {
            Err(TargetError::PlatformStub) => {}
            other => panic!("expected PlatformStub, got {other:?}"),
        }
    }

    // ── TargetError Display ──────────────────────────────────────────────────

    #[test]
    fn target_error_display_no_base() {
        let msg = TargetError::NoBaseAddress.to_string();
        assert!(msg.contains("base address"));
    }

    #[test]
    fn target_error_display_spawn_not_found() {
        let msg = TargetError::SpawnNotFound(42).to_string();
        assert!(msg.contains("42"));
    }

    #[test]
    fn target_error_display_platform_stub() {
        let msg = TargetError::PlatformStub.to_string();
        assert!(!msg.is_empty());
    }

    #[test]
    fn target_error_display_invalid_assist_target() {
        let msg = TargetError::InvalidAssistTarget.to_string();
        assert!(msg.contains("assist"));
    }

    // NOTE: Windows-only spawn-list integration tests (e.g., walking a real
    // TList in-process with fake spawn nodes) require access to the live EQ
    // process memory layout and belong in integration tests on Frostreaver.
    // The unit tests above cover all pure-logic paths:
    //   - zero-base address guards (NoBaseAddress)
    //   - non-Windows platform stubs (PlatformStub / None returns)
    //   - error Display formatting
    // Spawn-list walking correctness is validated by the integration test
    // suite on the Windows CI runner against a live EQ build.
}
