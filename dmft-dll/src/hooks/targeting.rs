//! Targeting control -- set, clear, and query the current target.
//!
//! This is a function-call API rather than a detour hook -- we invoke
//! EQ targeting primitives instead of intercepting them.

/// Set the current target by spawn ID.
pub fn set_target(spawn_id: u32) {
    // TODO: Find spawn in spawn list by ID, write to pinstTarget pointer
    tracing::trace!(spawn_id, "SetTarget (not yet implemented)");
}

/// Clear the current target.
pub fn clear_target() {
    // TODO: Write null to pinstTarget pointer
    tracing::trace!("ClearTarget (not yet implemented)");
}

/// Target the nearest NPC within range.
pub fn target_nearest_npc(max_range: f32) -> Option<u32> {
    // TODO: Walk spawn list, find nearest NPC, set as target
    tracing::trace!(max_range, "TargetNearestNPC (not yet implemented)");
    None
}

/// Assist another character -- target their target.
pub fn assist(assist_spawn_id: u32) {
    // TODO: Read the target of the specified spawn, set as our target
    tracing::trace!(assist_spawn_id, "Assist (not yet implemented)");
}

/// Check if we have a target and return its spawn ID.
pub fn get_current_target_id() -> Option<u32> {
    // TODO: Read pinstTarget, return spawn_id if not null
    None
}
