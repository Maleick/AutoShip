use dmft_common::nav::Waypoint;

/// Heuristic aggro check: is the target NPC facing toward us?
/// EQ has no explicit aggro flag readable from memory, so we use
/// heading + distance as an approximation.
pub fn has_aggro_on_me(
    target_heading: f32,
    target_pos: &Waypoint,
    my_pos: &Waypoint,
    threshold_degrees: f32,
) -> bool {
    let dx = my_pos.x - target_pos.x;
    let dy = my_pos.y - target_pos.y;
    let heading_to_me = ((-dx).atan2(dy).to_degrees() * 512.0 / 360.0 + 512.0) % 512.0;
    let diff = ((target_heading - heading_to_me) + 512.0) % 512.0;
    let diff = if diff > 256.0 { 512.0 - diff } else { diff };
    let threshold_eq = threshold_degrees * 512.0 / 360.0;
    diff < threshold_eq
}

/// Check if snap aggro is needed (tank role, new mob without aggro).
pub fn is_snap_aggro_needed(
    _my_spawn_id: u32, // reserved for future aggro list lookup
    is_main_tank: bool,
    target_heading: f32,
    target_pos: &Waypoint,
    my_pos: &Waypoint,
) -> bool {
    if !is_main_tank {
        return false;
    }
    !has_aggro_on_me(target_heading, target_pos, my_pos, 45.0)
}
