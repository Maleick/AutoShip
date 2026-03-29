//! Combat positioning — keeps melee characters in range of their target.

/// Default melee range threshold in EQ units.
pub const DEFAULT_MELEE_RANGE: f32 = 15.0;

/// Calculate distance between two 2D points.
pub fn distance_2d(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    (dx * dx + dy * dy).sqrt()
}

/// Check if a melee character needs to reposition (too far from target).
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
/// Returns Vec<String> of slash commands to execute.
///
/// - If in melee range: just `/face` to stay oriented on the target.
/// - If out of range: `/face` then `/nav target` to close the gap.
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

/// Returns `/face` commands for all melee members (Tank, DPS) in a camp group.
/// Intended to be called periodically during the Fighting state.
pub fn fighting_face_commands(members: &[(u32, super::state::Role)]) -> Vec<(u32, String)> {
    members
        .iter()
        .filter(|(_, role)| matches!(role, super::state::Role::Tank | super::state::Role::DPS))
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
            (103, Role::DPS),
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
        let members = vec![
            (100, Role::Tank),
            (101, Role::DPS),
            (102, Role::DPS),
        ];
        let cmds = fighting_face_commands(&members);
        assert_eq!(cmds.len(), 3);
    }
}
