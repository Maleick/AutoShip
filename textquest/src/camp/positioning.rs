//! Combat positioning — keeps melee characters in range of their target.

/// Default melee range threshold in EQ units.
pub const DEFAULT_MELEE_RANGE: f32 = 15.0;

/// Calculate distance between two 2D points.
#[must_use]
pub fn distance_2d(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    ((x1 - x2).powi(2) + (y1 - y2).powi(2)).sqrt()
}

/// Check if a melee character needs to reposition (too far from target).
#[must_use]
pub fn needs_reposition(
    player_x: f32,
    player_y: f32,
    target_x: f32,
    target_y: f32,
    max_melee_range: f32,
) -> bool {
    distance_2d(player_x, player_y, target_x, target_y) > max_melee_range
}

/// Generate movement commands to get a melee character to their target.
/// Returns a list of slash commands to execute.
///
/// - If in melee range: just `/face` to stay oriented on the target.
/// - If out of range: `/face` then `/nav target` to close the gap.
#[must_use]
pub fn melee_positioning_commands(
    player_pos: (f32, f32),
    target_pos: (f32, f32),
    is_in_range: bool,
) -> Vec<String> {
    let _ = (player_pos, target_pos); // positions reserved for future use
    let mut cmds = vec!["/face".to_string()];
    if !is_in_range {
        cmds.push("/nav target".to_string());
    }
    cmds
}

/// EQ heading units per full rotation (0-512, not 0-360).
const EQ_HEADING_UNITS: f32 = 512.0;

/// Default backstab offset distance in EQ units.
const BACKSTAB_OFFSET: f32 = 5.0;

/// Calculate the position directly behind a target based on EQ heading.
///
/// EQ heading is 0-512. "Behind" = heading + 256 (mod 512).
/// Heading-to-radians: `radians = heading * (2π / 512)`.
/// Position: `(target_x + offset * sin(behind_heading_rad), target_y + offset *
/// cos(behind_heading_rad))`.
#[must_use]
pub fn behind_target_position(
    target_x: f32,
    target_y: f32,
    target_heading: f32,
    offset_distance: f32,
) -> (f32, f32) {
    let behind_heading = (target_heading + EQ_HEADING_UNITS / 2.0) % EQ_HEADING_UNITS;
    let radians = behind_heading * (std::f32::consts::TAU / EQ_HEADING_UNITS);
    (
        target_x + offset_distance * radians.sin(),
        target_y + offset_distance * radians.cos(),
    )
}

/// Generate movement commands for a rogue to get behind their target for
/// backstab.
///
/// If already behind the target (within backstab offset range), returns just
/// `/face`. Otherwise returns `/face` + movement to the behind position.
#[must_use]
pub fn rogue_positioning_commands(
    player_pos: (f32, f32),
    target_pos: (f32, f32),
    target_heading: f32,
) -> Vec<String> {
    let (behind_x, behind_y) =
        behind_target_position(target_pos.0, target_pos.1, target_heading, BACKSTAB_OFFSET);
    let dist_to_behind = distance_2d(player_pos.0, player_pos.1, behind_x, behind_y);

    if dist_to_behind <= BACKSTAB_OFFSET {
        // Already behind — just face the mob
        vec!["/face".to_string()]
    } else {
        // Move to behind position, then face
        vec!["/face".to_string(), "/nav target".to_string()]
    }
}

/// Returns `/face` commands for all melee members (Tank, DPS) in a camp group.
/// Intended to be called periodically during the Fighting state.
///
/// Rogues (identified by `is_rogue` closure) get backstab positioning instead
/// of plain `/face`.
#[must_use]
pub fn fighting_face_commands(members: &[(u32, super::state::Role)]) -> Vec<(u32, String)> {
    members
        .iter()
        .filter(|(_, role)| matches!(role, super::state::Role::Tank | super::state::Role::Dps))
        .map(|(pid, _)| (*pid, "/face".to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camp::state::Role;

    #[test]
    fn test_distance_2d_zero() {
        assert!((distance_2d(0.0, 0.0, 0.0, 0.0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_distance_2d_known() {
        // 3-4-5 triangle
        let d = distance_2d(0.0, 0.0, 3.0, 4.0);
        assert!((d - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_distance_2d_negative_coords() {
        let d = distance_2d(-3.0, -4.0, 0.0, 0.0);
        assert!((d - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_needs_reposition_in_range() {
        assert!(!needs_reposition(0.0, 0.0, 5.0, 0.0, 15.0));
    }

    #[test]
    fn test_needs_reposition_out_of_range() {
        assert!(needs_reposition(0.0, 0.0, 20.0, 0.0, 15.0));
    }

    #[test]
    fn test_needs_reposition_exact_boundary() {
        // At exactly max range, not greater — should NOT need reposition
        assert!(!needs_reposition(0.0, 0.0, 15.0, 0.0, 15.0));
    }

    #[test]
    fn test_melee_commands_in_range() {
        let cmds = melee_positioning_commands((0.0, 0.0), (5.0, 5.0), true);
        assert_eq!(cmds, vec!["/face"]);
    }

    #[test]
    fn test_melee_commands_out_of_range() {
        let cmds = melee_positioning_commands((0.0, 0.0), (50.0, 50.0), false);
        assert_eq!(cmds, vec!["/face", "/nav target"]);
    }

    #[test]
    fn test_fighting_face_commands_filters_roles() {
        let members = vec![
            (100, Role::Tank),
            (101, Role::Healer),
            (102, Role::CC),
            (103, Role::Dps),
            (104, Role::Puller),
            (105, Role::Bard),
        ];
        let cmds = fighting_face_commands(&members);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0], (100, "/face".to_string()));
        assert_eq!(cmds[1], (103, "/face".to_string()));
    }

    #[test]
    fn test_fighting_face_commands_empty() {
        let members: Vec<(u32, Role)> = vec![];
        let cmds = fighting_face_commands(&members);
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_fighting_face_commands_all_melee() {
        let members = vec![(100, Role::Tank), (101, Role::Dps), (102, Role::Dps)];
        let cmds = fighting_face_commands(&members);
        assert_eq!(cmds.len(), 3);
    }

    // -- Backstab positioning tests --

    #[test]
    fn test_behind_target_heading_zero() {
        // Heading 0 → behind = 256 (south in EQ)
        let (bx, by) = behind_target_position(100.0, 200.0, 0.0, 5.0);
        // behind_heading = 256, radians = 256 * 2π/512 = π
        // sin(π) ≈ 0, cos(π) = -1
        assert!((bx - 100.0).abs() < 0.1, "x should be ~100, got {bx}");
        assert!((by - 195.0).abs() < 0.1, "y should be ~195, got {by}");
    }

    #[test]
    fn test_behind_target_heading_256() {
        // Heading 256 → behind = 0 (north in EQ)
        let (bx, by) = behind_target_position(100.0, 200.0, 256.0, 5.0);
        // behind_heading = 0, radians = 0
        // sin(0) = 0, cos(0) = 1
        assert!((bx - 100.0).abs() < 0.1, "x should be ~100, got {bx}");
        assert!((by - 205.0).abs() < 0.1, "y should be ~205, got {by}");
    }

    #[test]
    fn test_behind_target_heading_wraps() {
        // Heading 400 → behind = (400 + 256) % 512 = 144
        let (bx, by) = behind_target_position(0.0, 0.0, 400.0, 5.0);
        let behind_heading = 144.0_f32;
        let radians = behind_heading * (std::f32::consts::TAU / 512.0);
        let expected_x = 5.0 * radians.sin();
        let expected_y = 5.0 * radians.cos();
        assert!((bx - expected_x).abs() < 0.01);
        assert!((by - expected_y).abs() < 0.01);
    }

    #[test]
    fn test_behind_target_offset_distance() {
        let (bx, by) = behind_target_position(0.0, 0.0, 0.0, 10.0);
        let dist = distance_2d(0.0, 0.0, bx, by);
        assert!(
            (dist - 10.0).abs() < 0.1,
            "distance should be ~10, got {dist}"
        );
    }

    #[test]
    fn test_rogue_positioning_already_behind() {
        // Player is already at the behind position
        let (behind_x, behind_y) = behind_target_position(100.0, 200.0, 0.0, BACKSTAB_OFFSET);
        let cmds = rogue_positioning_commands((behind_x, behind_y), (100.0, 200.0), 0.0);
        assert_eq!(cmds, vec!["/face"]);
    }

    #[test]
    fn test_rogue_positioning_not_behind() {
        // Player is far from the behind position
        let cmds = rogue_positioning_commands(
            (200.0, 200.0), // far away
            (100.0, 200.0),
            0.0,
        );
        assert_eq!(cmds, vec!["/face", "/nav target"]);
    }
}
