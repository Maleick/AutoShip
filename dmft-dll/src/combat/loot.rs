//! Loot automation — detect nearby corpses and loot them.
//!
//! Uses EQ's `__do_loot` function to open the loot window on a targeted corpse,
//! then loots all items. For now this is a stub that issues the /loot slash command
//! since direct function calls require more reverse engineering of the loot window.

/// Attempt to loot the nearest corpse via /loot slash command.
///
/// The /loot command targets the nearest corpse within range and opens the loot window.
/// This is the simplest approach — direct `__do_loot` calls would be faster but require
/// understanding the loot window's internal state machine.
pub fn loot_nearest_corpse() {
    tracing::info!("Loot: attempting to loot nearest corpse via /loot");
    crate::hooks::game_loop::queue_slash_command("/loot".to_string());
}

/// Loot all items from the currently open loot window via /lootall.
///
/// Requires the loot window to already be open (from a prior /loot or corpse click).
pub fn loot_all_items() {
    tracing::info!("Loot: looting all items via /lootall");
    crate::hooks::game_loop::queue_slash_command("/lootall".to_string());
}

/// Check if there are nearby corpses that belong to us (by checking spawn type).
///
/// Corpses have spawn_type == 2 in the spawn list. We look for corpses
/// with names matching our group members (player corpses) or any NPC corpses
/// within loot range.
#[cfg(windows)]
pub fn has_lootable_corpses() -> bool {
    // TODO: Walk spawn list, check for spawn_type == 2 within range
    // For now, always return false (loot is manually triggered)
    false
}

#[cfg(not(windows))]
pub fn has_lootable_corpses() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_lootable_corpses_returns_false_on_macos() {
        assert!(!has_lootable_corpses());
    }
}
