//! Movement control -- provides functions to move the player character.
//! Uses EQ's internal movement functions called from our game loop hook.
//!
//! This is a function-call API rather than a detour hook -- we invoke
//! EQ movement primitives instead of intercepting them.

/// Move the character toward a target position.
/// Called from `on_game_tick()` when a MoveTo command is pending.
pub fn move_to(x: f32, y: f32, z: f32) {
    // TODO: Calculate heading to target
    // TODO: Set movement state (forward/backward/strafe)
    // TODO: Handle arrival detection (within threshold distance)
    tracing::trace!(x, y, z, "MoveTo command (not yet implemented)");
}

/// Stop all movement.
pub fn stop() {
    // TODO: Clear movement state
    tracing::trace!("Stop movement (not yet implemented)");
}

/// Face a specific heading (in EQ degrees).
pub fn face_heading(heading: f32) {
    // TODO: Set character heading via memory write
    tracing::trace!(heading, "Face heading (not yet implemented)");
}

/// Face a target position.
pub fn face_position(target_x: f32, target_y: f32, current_x: f32, current_y: f32) {
    let dx = target_x - current_x;
    let dy = target_y - current_y;
    let heading = dy.atan2(dx).to_degrees();
    face_heading(heading);
}

/// Calculate distance between two 3D points.
pub fn distance_3d(x1: f32, y1: f32, z1: f32, x2: f32, y2: f32, z2: f32) -> f32 {
    ((x2 - x1).powi(2) + (y2 - y1).powi(2) + (z2 - z1).powi(2)).sqrt()
}

/// Calculate 2D distance (ignoring Z/height).
pub fn distance_2d(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt()
}
