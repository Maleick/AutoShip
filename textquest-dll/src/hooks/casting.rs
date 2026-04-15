//! Spell casting control -- invokes EQ's internal casting functions.
//!
//! This is a function-call API rather than a detour hook. The three main
//! operations delegate to `crate::eq` primitives which resolve and call
//! EQ's internal functions directly:
//!
//! - `cast_spell`   → `CharacterZoneClient::CastSpell`  (via
//!   `crate::eq::cast_spell`)
//! - `use_ability`  → `PcZoneClient::DoCombatAbility`   (via
//!   `crate::eq::do_combat_ability`)
//! - `use_item`     → `/useitem` slash command           (via
//!   `crate::eq::slash_command`)
//!
//! On non-Windows platforms all operations are no-ops that log a warning
//! (the underlying `crate::eq` functions handle the platform stub).

#[cfg(windows)]
use textquest_common::offsets::{self, character_zone, launch_spell_data, player_zone};

/// Error type for casting operations.
#[derive(Debug, thiserror::Error)]
pub enum CastError {
    #[error("EQ base address not set -- cannot resolve pointers")]
    NoBaseAddress,
    #[error("spell gem slot {0} is out of range (valid: 1-13)")]
    InvalidSlot(u8),
    #[error("a spell is already being cast")]
    AlreadyCasting,
    #[error("casting not available on this platform")]
    PlatformStub,
    #[error("invalid item slot or name: {0}")]
    InvalidItem(String),
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

    /// Cast a spell from the given spell gem slot (1-13).
    ///
    /// Calls `CharacterZoneClient::CastSpell` via `crate::eq::cast_spell`.
    /// The gem slot is converted from 1-based (EQ UI) to 0-based (internal).
    /// spell_id=0 means "use whatever is memorized in that gem".
    /// The target should be set via the `TargetingController` before calling
    /// this.
    pub fn cast_spell(&self, spell_slot: u8, _target_id: u32) -> Result<(), CastError> {
        if spell_slot == 0 || spell_slot > 13 {
            return Err(CastError::InvalidSlot(spell_slot));
        }
        if self.eq_base == 0 {
            return Err(CastError::NoBaseAddress);
        }

        // Convert 1-based UI gem slot to 0-based internal gem index.
        let gem_id = spell_slot - 1;
        tracing::info!(
            spell_slot,
            gem_id,
            "cast_spell: delegating to eq::cast_spell"
        );
        crate::eq::cast_spell(gem_id, 0);
        Ok(())
    }

    /// Interrupt the current spell cast (equivalent to pressing duck/stand).
    ///
    /// Writes STANDSTATE=4 (ducking) to PlayerClient to cancel a cast in
    /// progress. The game loop will restore standing on the next tick.
    pub fn interrupt_cast(&self) -> Result<(), CastError> {
        if self.eq_base == 0 {
            return Err(CastError::NoBaseAddress);
        }

        #[cfg(windows)]
        {
            use textquest_common::offsets;

            let Some(player_ptr_addr) = offsets::rebase(offsets::PINST_LOCAL_PLAYER, self.eq_base)
            else {
                return Err(CastError::NoBaseAddress);
            };

            // SAFETY: player_ptr_addr is the rebased PINST_LOCAL_PLAYER global.
            // Dereferencing yields the PlayerClient* singleton (null if not in world).
            let player_base = unsafe { std::ptr::read(player_ptr_addr as *const usize) };
            if player_base == 0 {
                tracing::warn!("interrupt_cast: local player not available");
                return Err(CastError::NoBaseAddress);
            }

            // STANDSTATE offset in PlayerZoneClient — 4 = ducking, which interrupts a cast.
            // Sourced from eqlib PlayerClient.h: /*0x14*/ EQStandState StandState
            const STANDSTATE: usize = 0x14;
            tracing::info!("interrupt_cast: writing duck state to cancel cast");
            // SAFETY: player_base is a validated non-null PlayerClient* inside eqgame.exe.
            // Writing 4 (duck) to StandState is the standard MQ2 approach to interrupt a
            // cast. If the offset is wrong, this will corrupt game state —
            // offset sourced from eqlib.
            unsafe {
                std::ptr::write((player_base + STANDSTATE) as *mut u8, 4);
            }
            Ok(())
        }

        #[cfg(not(windows))]
        {
            tracing::warn!("interrupt_cast (stub -- non-Windows)");
            Err(CastError::PlatformStub)
        }
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

    /// Use an ability or discipline by spell ID.
    ///
    /// Abilities include disciplines, AAs, and combat skills.
    /// Calls `PcZoneClient::DoCombatAbility` via
    /// `crate::eq::do_combat_ability`.
    pub fn use_ability(&self, ability_id: u32) -> Result<(), CastError> {
        if self.eq_base == 0 {
            return Err(CastError::NoBaseAddress);
        }

        tracing::info!(
            ability_id,
            "use_ability: delegating to eq::do_combat_ability"
        );
        crate::eq::do_combat_ability(ability_id as i32, true);
        Ok(())
    }

    /// Use an item (clicky) by inventory slot ID or item name.
    ///
    /// Activates a clickable item effect via the `/useitem` slash command.
    /// `slot_id` maps to an EQ inventory slot number (e.g., 13 = primary,
    /// 14 = secondary). Uses `crate::eq::slash_command` which calls
    /// `CEverQuest::InterpretCmd` internally.
    pub fn use_item(&self, slot_id: u32) -> Result<(), CastError> {
        if self.eq_base == 0 {
            return Err(CastError::NoBaseAddress);
        }

        let command = format!("/useitem {slot_id}");
        tracing::info!(slot_id, cmd = %command, "use_item: issuing slash command");
        crate::eq::slash_command(&command);
        Ok(())
    }

    /// Use an item by name (clicky with quoted item name).
    ///
    /// Activates a clickable item effect by name via `/useitem "name"`.
    /// Strips control characters and embedded quotes to prevent command
    /// injection.
    pub fn use_item_by_name(&self, item_name: &str) -> Result<(), CastError> {
        if self.eq_base == 0 {
            return Err(CastError::NoBaseAddress);
        }

        let sanitized: String = item_name
            .chars()
            .filter(|ch| !ch.is_control() && *ch != '"')
            .collect();
        let trimmed = sanitized.trim();
        if trimmed.is_empty() {
            return Err(CastError::InvalidItem(item_name.to_string()));
        }

        let command = format!("/useitem \"{trimmed}\"");
        tracing::info!(item_name, cmd = %command, "use_item_by_name: issuing slash command");
        crate::eq::slash_command(&command);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify CastSpell offset is present and non-zero in the offset table.
    #[test]
    fn cast_spell_offset_is_nonzero() {
        assert_ne!(
            textquest_common::offsets::CAST_SPELL,
            0,
            "CAST_SPELL offset must be set in offsets.rs"
        );
    }

    /// Verify DoCombatAbility offset is present and non-zero.
    #[test]
    fn do_combat_ability_offset_is_nonzero() {
        assert_ne!(
            textquest_common::offsets::DO_COMBAT_ABILITY,
            0,
            "DO_COMBAT_ABILITY offset must be set in offsets.rs"
        );
    }

    /// Verify that offsets are within the expected preferred-base range for
    /// eqgame.exe (0x140000000 to 0x150000000).
    #[test]
    fn function_offsets_are_in_preferred_base_range() {
        let base = textquest_common::offsets::EQ_PREFERRED_BASE;
        let limit = base + 0x1000_0000;

        assert!(
            textquest_common::offsets::CAST_SPELL >= base
                && textquest_common::offsets::CAST_SPELL < limit,
            "CAST_SPELL {:#x} is outside expected preferred-base range",
            textquest_common::offsets::CAST_SPELL
        );

        assert!(
            textquest_common::offsets::DO_COMBAT_ABILITY >= base
                && textquest_common::offsets::DO_COMBAT_ABILITY < limit,
            "DO_COMBAT_ABILITY {:#x} is outside expected preferred-base range",
            textquest_common::offsets::DO_COMBAT_ABILITY
        );
    }

    /// Verify slot validation: slot 0 is rejected.
    #[test]
    fn cast_spell_rejects_slot_zero() {
        let ctrl = CastingController::new(0x1400_0000);
        let result = ctrl.cast_spell(0, 0);
        assert!(matches!(result, Err(CastError::InvalidSlot(0))));
    }

    /// Verify slot validation: slot 14 is rejected.
    #[test]
    fn cast_spell_rejects_slot_fourteen() {
        let ctrl = CastingController::new(0x1400_0000);
        let result = ctrl.cast_spell(14, 0);
        assert!(matches!(result, Err(CastError::InvalidSlot(14))));
    }

    /// Verify slot validation: slot 13 is accepted (max valid).
    #[test]
    fn cast_spell_accepts_slot_thirteen() {
        // eq_base=0 triggers NoBaseAddress before the EQ call — safe on macOS.
        let ctrl = CastingController::new(0);
        let result = ctrl.cast_spell(13, 0);
        // Slot is valid (13), but eq_base=0 → NoBaseAddress
        assert!(matches!(result, Err(CastError::NoBaseAddress)));
    }

    /// Verify use_ability rejects zero base address.
    #[test]
    fn use_ability_rejects_zero_base() {
        let ctrl = CastingController::new(0);
        let result = ctrl.use_ability(100);
        assert!(matches!(result, Err(CastError::NoBaseAddress)));
    }

    /// Verify use_item rejects zero base address.
    #[test]
    fn use_item_rejects_zero_base() {
        let ctrl = CastingController::new(0);
        let result = ctrl.use_item(13);
        assert!(matches!(result, Err(CastError::NoBaseAddress)));
    }

    /// Verify use_item_by_name rejects empty name after sanitization.
    #[test]
    fn use_item_by_name_rejects_empty_sanitized() {
        let ctrl = CastingController::new(0x1400_0000);
        // String of only quotes and control chars → empty after sanitization.
        let result = ctrl.use_item_by_name("\"\"");
        assert!(matches!(result, Err(CastError::InvalidItem(_))));
    }

    /// Verify use_item_by_name strips control characters and quotes.
    #[test]
    fn use_item_by_name_rejects_zero_base_with_valid_name() {
        let ctrl = CastingController::new(0);
        let result = ctrl.use_item_by_name("Fungi Tunic");
        assert!(matches!(result, Err(CastError::NoBaseAddress)));
    }

    /// Verify rebase arithmetic round-trips for CAST_SPELL.
    #[test]
    fn cast_spell_rebase_roundtrip() {
        use textquest_common::offsets;
        // Simulate a module loaded at the preferred base — rebase should return
        // the same address since delta = 0.
        let preferred = offsets::EQ_PREFERRED_BASE;
        let rebased = offsets::rebase(offsets::CAST_SPELL, preferred);
        assert!(rebased.is_some(), "rebase with preferred base must succeed");
        assert_eq!(
            rebased.unwrap() as u64,
            offsets::CAST_SPELL,
            "rebase at preferred base must return the original address"
        );
    }
}
