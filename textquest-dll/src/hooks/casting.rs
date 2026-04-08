//! Spell casting control -- invokes EQ's internal casting functions.
//!
//! This is a function-call API rather than a detour hook. Eventually
//! these functions will call EQ's internal `CastSpell` / `UseAbility` /
//! `UseItem` functions once we have their resolved addresses from the
//! offset database. Until then, they log intent and return an error.
//!
//! On non-Windows platforms all operations are no-ops that log a warning.

#[cfg(windows)]
use textquest_common::offsets::{self, character_zone, launch_spell_data, player_zone};

/// Error type for casting operations.
#[derive(Debug, thiserror::Error)]
pub enum CastError {
    #[error("EQ base address not set -- cannot resolve pointers")]
    NoBaseAddress,
    #[error("spell gem slot {0} is out of range (valid: 1-13)")]
    InvalidSlot(u8),
    #[error("cast function address not yet resolved from offset database")]
    FunctionNotResolved,
    #[error("a spell is already being cast")]
    AlreadyCasting,
    #[error("casting not available on this platform")]
    PlatformStub,
}

/// Manages spell casting, ability usage, and item activation.
///
/// Holds the EQ module base address needed to resolve function pointers
/// and read casting state from memory.
pub struct CastingController {
    /// Runtime base address of eqgame.exe (actual, not preferred).
    eq_base: u64,
}

impl CastingController {
    /// Create a new controller with the given EQ module base address.
    pub fn new(eq_base: u64) -> Self {
        Self { eq_base }
    }

    /// Update the EQ base address (e.g., if module is reloaded).
    pub fn set_eq_base(&mut self, eq_base: u64) {
        self.eq_base = eq_base;
    }

    /// Cast a spell from the given spell gem slot (1-13) on a target.
    ///
    /// Once we have the resolved address for EQ's `CastSpell` function,
    /// this will call it directly. The target should be set via the
    /// `TargetingController` before calling this.
    pub fn cast_spell(&self, spell_slot: u8, target_id: u32) -> Result<(), CastError> {
        if spell_slot == 0 || spell_slot > 13 {
            return Err(CastError::InvalidSlot(spell_slot));
        }
        if self.eq_base == 0 {
            return Err(CastError::NoBaseAddress);
        }

        tracing::info!(
            spell_slot,
            target_id,
            "cast_spell -- function address not yet resolved"
        );

        // TODO: Once the CastSpell function address is in the offset database:
        // 1. Resolve the address via rebase()
        // 2. Transmute to the correct function signature:
        //    type CastSpellFn = unsafe extern "system" fn(
        //        this: *mut c_void,  // PcClient*
        //        gem_index: i32,     // 0-based spell gem index
        //        spell_id: i32,      // spell ID
        //        item_ptr: *mut c_void,
        //        item_guid: u64,
        //    );
        // 3. Call the function with the correct arguments
        //
        // For now, return an error indicating the function isn't wired up yet.
        Err(CastError::FunctionNotResolved)
    }

    /// Interrupt the current spell cast (equivalent to pressing duck/stand).
    ///
    /// This will eventually call EQ's internal interrupt function or
    /// toggle the duck state to cancel a cast in progress.
    pub fn interrupt_cast(&self) -> Result<(), CastError> {
        if self.eq_base == 0 {
            return Err(CastError::NoBaseAddress);
        }

        tracing::info!("interrupt_cast -- function address not yet resolved");

        // TODO: Two possible approaches:
        // A) Call EQ's interrupt/duck internal function
        // B) Write STANDSTATE = 4 (ducking) to PlayerClient, then
        //    write STANDSTATE = 0 (standing) on the next tick
        //
        // Approach B is simpler and doesn't require a resolved function
        // address. Once we have a local player base address, we can do:
        //   ptr::write((player_base + STANDSTATE) as *mut u8, 4);
        Err(CastError::FunctionNotResolved)
    }

    /// Check if the local player is currently casting a spell.
    ///
    /// Reads `PlayerZoneClient::CastingData.SpellID` from the local spawn.
    /// Returns `true` if a cast is in progress.
    pub fn is_casting(&self) -> Result<bool, CastError> {
        if self.eq_base == 0 {
            return Err(CastError::NoBaseAddress);
        }

        #[cfg(windows)]
        {
            let pc_pinst = offsets::rebase(offsets::PINST_LOCAL_PC, self.eq_base)
                .ok_or(CastError::NoBaseAddress)?;
            let pc_ptr = unsafe { std::ptr::read(pc_pinst as *const usize) };
            if pc_ptr == 0 {
                return Ok(false);
            }

            let player_ptr =
                unsafe { std::ptr::read((pc_ptr + character_zone::ME) as *const usize) };
            if player_ptr == 0 {
                return Ok(false);
            }

            let cast_addr = player_ptr + player_zone::CASTING_DATA;
            let spell_id =
                unsafe { std::ptr::read((cast_addr + launch_spell_data::SPELL_ID) as *const i32) };
            Ok(spell_id != launch_spell_data::NOT_CASTING_SPELL_ID)
        }

        #[cfg(not(windows))]
        {
            tracing::warn!("is_casting (stub -- non-Windows)");
            Ok(false)
        }
    }

    /// Use an ability or discipline by ID.
    ///
    /// Abilities include skills like Kick, Bash, Taunt, and disciplines.
    /// Once we have the `DoAbility` function address from the offset database,
    /// this will call it directly.
    pub fn use_ability(&self, ability_id: u32) -> Result<(), CastError> {
        if self.eq_base == 0 {
            return Err(CastError::NoBaseAddress);
        }

        tracing::info!(
            ability_id,
            "use_ability -- function address not yet resolved"
        );

        // TODO: Once the DoAbility function address is in the offset database:
        // 1. Resolve the address via rebase()
        // 2. Transmute to the correct function signature
        // 3. Call with the ability_id
        Err(CastError::FunctionNotResolved)
    }

    /// Use an item (clicky) in an inventory slot.
    ///
    /// Activates a clickable item effect. Once we have the `UseItem`
    /// function address from the offset database, this will call it.
    pub fn use_item(&self, slot_id: u32) -> Result<(), CastError> {
        if self.eq_base == 0 {
            return Err(CastError::NoBaseAddress);
        }

        tracing::info!(slot_id, "use_item -- function address not yet resolved");

        // TODO: Once the UseItem function address is in the offset database:
        // 1. Resolve the address via rebase()
        // 2. Transmute to the correct function signature
        // 3. Call with slot_id to activate the item's click effect
        Err(CastError::FunctionNotResolved)
    }
}
