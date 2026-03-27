//! Targeting control -- set, clear, and query the current target.
//!
//! This is a function-call API rather than a detour hook -- we write
//! directly to EQ's target pointer (pinstTarget) in memory. On
//! non-Windows platforms all operations are no-ops that log a warning.
//!
//! Full functionality requires resolved EQ base address + offsets.
//! Once the offset database is wired up, these functions will be able
//! to walk the spawn list and resolve spawn IDs to memory addresses.

use dmft_common::offsets;

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
    /// `spawn_addr` is the runtime address of the target PlayerClient struct.
    /// Use `set_target_by_id` once spawn-list walking is implemented.
    pub fn set_target_by_addr(&self, spawn_addr: usize) -> Result<(), TargetError> {
        let pinst_addr = self.target_ptr_addr()?;
        tracing::debug!(
            pinst_addr = format!("{:#x}", pinst_addr),
            spawn_addr = format!("{:#x}", spawn_addr),
            "setting target"
        );

        #[cfg(windows)]
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
    /// Walks the spawn linked list to find the PlayerClient with the matching
    /// spawn ID, then writes its address to pinstTarget.
    pub fn set_target(&self, spawn_id: u32) -> Result<(), TargetError> {
        let spawn_addr = self.find_spawn_addr(spawn_id)?;
        self.set_target_by_addr(spawn_addr)
    }

    /// Clear the current target by writing null to pinstTarget.
    pub fn clear_target(&self) -> Result<(), TargetError> {
        let pinst_addr = self.target_ptr_addr()?;
        tracing::debug!(
            pinst_addr = format!("{:#x}", pinst_addr),
            "clearing target"
        );

        #[cfg(windows)]
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
        unsafe {
            let target_ptr = std::ptr::read(_pinst_addr as *const usize);
            if target_ptr == 0 {
                return Ok(None);
            }
            let spawn_id = std::ptr::read(
                (target_ptr + offsets::player_base::SPAWN_ID) as *const u32,
            );
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
        // TODO: Once spawn-list walking is implemented:
        // 1. Read local player position from PINST_LOCAL_PLAYER
        // 2. Walk spawn list from PINST_SPAWN_MANAGER -> PLAYER_LIST
        // 3. Filter to TYPE == NPC (1)
        // 4. Find nearest by distance, within max_range
        // 5. Call set_target_by_addr with the winner
        tracing::debug!(max_range, "target_nearest_npc -- not yet implemented (needs spawn list walker)");
        Ok(None)
    }

    /// Assist another character -- target their target.
    ///
    /// Reads the target pointer of the specified spawn (the "assist target"),
    /// then sets our target to whatever they are targeting.
    pub fn assist(&self, assist_spawn_id: u32) -> Result<(), TargetError> {
        // TODO: Full assist implementation requires:
        // 1. Find the assist target's PlayerClient by spawn ID
        // 2. Read the assist target's target pointer (the PlayerClient has a
        //    field pointing to the spawn they are targeting -- this offset
        //    needs to be added to offsets.rs once identified from MQ2 headers)
        // 3. If non-null, write that pointer to our pinstTarget
        //
        // For now, we log and return an error since we need the target-of-target
        // offset which hasn't been extracted from MQ2 headers yet.
        tracing::debug!(
            assist_spawn_id,
            "assist -- not yet implemented (needs target-of-target offset from MQ2 headers)"
        );
        Err(TargetError::SpawnNotFound(assist_spawn_id))
    }

    /// Walk the spawn linked list to find a spawn by ID. Returns its address.
    ///
    /// Traverses the TList<PlayerClient*> starting from SpawnManager's player
    /// list, following NEXT pointers until a matching SPAWN_ID is found.
    fn find_spawn_addr(&self, spawn_id: u32) -> Result<usize, TargetError> {
        if self.eq_base == 0 {
            return Err(TargetError::NoBaseAddress);
        }

        #[cfg(windows)]
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
                let sid = std::ptr::read(
                    (current + offsets::player_base::SPAWN_ID) as *const u32,
                );
                if sid == spawn_id {
                    return Ok(current);
                }
                current = std::ptr::read(
                    (current + offsets::player_base::NEXT) as *const usize,
                );
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
}
