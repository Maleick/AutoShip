//! Spell casting control -- invokes EQ's internal casting functions.
//!
//! This is a function-call API rather than a detour hook -- we invoke
//! EQ casting primitives instead of intercepting them.

/// Cast a spell from the given spell gem slot (1-13) on a target.
pub fn cast_spell(spell_slot: u8, target_id: u32) {
    // TODO: Call EQ's internal CastSpell function
    // Need to: set target first, then invoke cast
    tracing::trace!(spell_slot, target_id, "CastSpell (not yet implemented)");
}

/// Interrupt the current spell cast.
pub fn interrupt_cast() {
    // TODO: Call EQ's interrupt/duck function
    tracing::trace!("Interrupt cast (not yet implemented)");
}

/// Check if currently casting (reading cast timer from memory).
pub fn is_casting() -> bool {
    // TODO: Read cast timer from local player data
    false
}

/// Use an ability/discipline by ID.
pub fn use_ability(ability_id: u32) {
    // TODO: Call EQ's ability activation function
    tracing::trace!(ability_id, "UseAbility (not yet implemented)");
}

/// Use an item (clicky) in an inventory slot.
pub fn use_item(slot_id: u32) {
    // TODO: Call EQ's item use function
    tracing::trace!(slot_id, "UseItem (not yet implemented)");
}
