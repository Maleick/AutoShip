use textquest_common::nav::Waypoint;

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
    let heading_to_me = (dx.atan2(dy).to_degrees() * 512.0 / 360.0 + 512.0) % 512.0;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_facing_directly_at_me_has_aggro() {
        // Target at origin, facing north (heading 0), player is north of target
        let target_pos = Waypoint::new(0.0, 0.0, 0.0);
        let my_pos = Waypoint::new(0.0, 10.0, 0.0); // north of target
        assert!(has_aggro_on_me(0.0, &target_pos, &my_pos, 45.0));
    }

    #[test]
    fn target_facing_away_no_aggro() {
        // Target at origin, facing south (heading 256), player is north
        let target_pos = Waypoint::new(0.0, 0.0, 0.0);
        let my_pos = Waypoint::new(0.0, 10.0, 0.0);
        assert!(!has_aggro_on_me(256.0, &target_pos, &my_pos, 45.0));
    }

    #[test]
    fn target_facing_90_degrees_off_no_aggro_with_small_threshold() {
        // Target facing east (heading 128), player is north
        let target_pos = Waypoint::new(0.0, 0.0, 0.0);
        let my_pos = Waypoint::new(0.0, 10.0, 0.0);
        assert!(!has_aggro_on_me(128.0, &target_pos, &my_pos, 45.0));
    }

    #[test]
    fn target_facing_90_degrees_off_has_aggro_with_large_threshold() {
        // 90 degrees = 128 EQ degrees. Threshold of 100 degrees should catch it.
        let target_pos = Waypoint::new(0.0, 0.0, 0.0);
        let my_pos = Waypoint::new(0.0, 10.0, 0.0);
        assert!(has_aggro_on_me(128.0, &target_pos, &my_pos, 100.0));
    }

    #[test]
    fn heading_wraps_around_512() {
        // Heading 510 is just slightly west of north. Player north should have aggro.
        let target_pos = Waypoint::new(0.0, 0.0, 0.0);
        let my_pos = Waypoint::new(0.0, 10.0, 0.0);
        assert!(has_aggro_on_me(510.0, &target_pos, &my_pos, 45.0));
    }

    #[test]
    fn same_position_is_aggro() {
        // When at the same position, atan2(0,0)=0, heading_to_me=0
        // Any heading close to 0 should be aggro
        let pos = Waypoint::new(5.0, 5.0, 0.0);
        // This tests the degenerate case - both at same spot
        let result = has_aggro_on_me(0.0, &pos, &pos, 45.0);
        // atan2(0,0) = 0.0, heading_to_me = 0, diff = 0 => true
        assert!(result);
    }

    #[test]
    fn snap_aggro_not_needed_if_not_tank() {
        let target_pos = Waypoint::new(0.0, 0.0, 0.0);
        let my_pos = Waypoint::new(0.0, 10.0, 0.0);
        // Not main tank => never needs snap aggro
        assert!(!is_snap_aggro_needed(1, false, 256.0, &target_pos, &my_pos));
    }

    #[test]
    fn snap_aggro_needed_when_tank_and_no_aggro() {
        let target_pos = Waypoint::new(0.0, 0.0, 0.0);
        let my_pos = Waypoint::new(0.0, 10.0, 0.0);
        // Tank + target facing away (heading 256) => needs snap
        assert!(is_snap_aggro_needed(1, true, 256.0, &target_pos, &my_pos));
    }

    #[test]
    fn snap_aggro_not_needed_when_tank_has_aggro() {
        let target_pos = Waypoint::new(0.0, 0.0, 0.0);
        let my_pos = Waypoint::new(0.0, 10.0, 0.0);
        // Tank + target facing toward us => no snap needed
        assert!(!is_snap_aggro_needed(1, true, 0.0, &target_pos, &my_pos));
    }

    #[test]
    fn cardinal_directions() {
        // EQ axes: +X=West, -X=East, +Y=North, -Y=South
        // EQ heading: 0=North, 128=West(+X), 256=South, 384=East(-X)
        let center = Waypoint::new(0.0, 0.0, 0.0);

        // West (+X): heading_to_me=128, target facing 128 = facing west toward me
        let west = Waypoint::new(10.0, 0.0, 0.0);
        assert!(has_aggro_on_me(128.0, &center, &west, 45.0));

        // South (-Y): heading_to_me=256
        let south = Waypoint::new(0.0, -10.0, 0.0);
        assert!(has_aggro_on_me(256.0, &center, &south, 45.0));

        // East (-X): heading_to_me=384
        let east = Waypoint::new(-10.0, 0.0, 0.0);
        assert!(has_aggro_on_me(384.0, &center, &east, 45.0));
    }
}
