//! Melee positioning logic — tank facing, rogue backstab angle, camp range
//! enforcement.
//!
//! EQ melee attacks have positional requirements:
//! - Tanks should face the mob (auto-attack misses from behind)
//! - Rogues must be behind the mob for backstab
//! - All melee should stay within ~15 unit range
//! - Characters should return to camp after combat if they've drifted

use textquest_common::{nav::Waypoint, types::SpawnData};

/// Maximum melee range in EQ units. Beyond this, melee attacks won't connect.
const MELEE_RANGE: f32 = 15.0;

/// Distance threshold for "close enough" — don't micro-adjust within this
/// range.
const CLOSE_ENOUGH: f32 = 5.0;

/// Default maximum distance a character can drift from camp before being pulled
/// back.
pub const DEFAULT_CAMP_DRIFT: f32 = 100.0;

/// Result of a positioning check — tells the caller what movement is needed.
#[derive(Debug, Clone, PartialEq)]
pub enum PositionAction {
    /// No movement needed — we're in a good spot.
    None,
    /// Move toward a specific waypoint (target too far, need to close gap).
    MoveToward(Waypoint),
    /// Move to get behind the target (rogue backstab positioning).
    MoveBehind(Waypoint),
    /// Return to camp position (drifted too far).
    ReturnToCamp(Waypoint),
}

/// Check if we're within melee range of the target.
pub fn is_in_melee_range(player: &SpawnData, target: &SpawnData) -> bool {
    let dist =
        Waypoint::new(player.x, player.y, 0.0).distance_2d(&Waypoint::new(target.x, target.y, 0.0));
    dist <= MELEE_RANGE
}

/// Calculate the position behind a target (for backstab).
/// EQ heading is in 512-degree units. "Behind" = 180 degrees from mob's facing.
pub fn behind_target(target: &SpawnData) -> Waypoint {
    // EQ heading: 0=N, 128=E, 256=S, 384=W (512-unit circle)
    // "Behind" = heading + 256 (opposite direction)
    let behind_heading = (target.heading + 256.0) % 512.0;
    let angle_rad = behind_heading * std::f32::consts::PI * 2.0 / 512.0;

    // Position 8 units behind the mob
    let offset_dist = 8.0;
    Waypoint::new(
        target.x + angle_rad.sin() * offset_dist,
        target.y + angle_rad.cos() * offset_dist,
        target.z,
    )
}

/// Determine what positioning action a melee character should take.
///
/// `is_rogue`: true for backstab-dependent classes.
/// `camp_pos`: optional camp location to enforce drift limits.
pub fn check_melee_position(
    player: &SpawnData,
    target: &SpawnData,
    is_rogue: bool,
    camp_pos: Option<&Waypoint>,
    camp_radius: Option<f32>,
) -> PositionAction {
    let dist =
        Waypoint::new(player.x, player.y, 0.0).distance_2d(&Waypoint::new(target.x, target.y, 0.0));

    // Priority 1: If too far from camp, return to camp (after combat)
    if let Some(camp) = camp_pos {
        let max_drift = camp_radius.unwrap_or(DEFAULT_CAMP_DRIFT);
        let camp_dist = Waypoint::new(player.x, player.y, 0.0).distance_2d(camp);
        if camp_dist > max_drift {
            return PositionAction::ReturnToCamp(*camp);
        }
    }

    // Priority 2: Close distance if out of melee range
    if dist > MELEE_RANGE {
        let target_pos = Waypoint::new(target.x, target.y, target.z);
        return PositionAction::MoveToward(target_pos);
    }

    // Priority 3: Rogue backstab positioning
    if is_rogue && dist <= MELEE_RANGE {
        let behind = behind_target(target);
        let player_pos = Waypoint::new(player.x, player.y, player.z);
        let behind_dist = player_pos.distance_2d(&behind);
        if behind_dist > CLOSE_ENOUGH {
            return PositionAction::MoveBehind(behind);
        }
    }

    PositionAction::None
}

/// Check for `AoE` avoidance — returns a position to move to if needed.
/// Requires integration with the navmesh for pathfinding away from `AoE` zones.
/// Currently checks nearby enemies for large `AoE` clusters.
pub fn check_aoe_avoidance(
    player: &SpawnData,
    nearby_enemies: &[SpawnData],
    aoe_threshold: u8,
) -> Option<Waypoint> {
    // If too many enemies are clustered nearby, move away from the center of mass
    if nearby_enemies.len() < aoe_threshold as usize {
        return None;
    }

    // Calculate center of mass of enemy cluster
    let count = nearby_enemies.len() as f32;
    let cx: f32 = nearby_enemies.iter().map(|e| e.x).sum::<f32>() / count;
    let cy: f32 = nearby_enemies.iter().map(|e| e.y).sum::<f32>() / count;

    // Check if we're dangerously close to the cluster center
    let dist_to_center =
        Waypoint::new(player.x, player.y, 0.0).distance_2d(&Waypoint::new(cx, cy, 0.0));
    if dist_to_center < 30.0 {
        // Move 40 units away from the cluster center
        let dx = player.x - cx;
        let dy = player.y - cy;
        let mag = (dx * dx + dy * dy).sqrt().max(0.1);
        let escape_x = player.x + (dx / mag) * 40.0;
        let escape_y = player.y + (dy / mag) * 40.0;
        return Some(Waypoint::new(escape_x, escape_y, player.z));
    }

    None
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    fn make_player(x: f32, y: f32) -> SpawnData {
        let mut p = SpawnData::default();
        p.x = x;
        p.y = y;
        p
    }

    fn make_target(x: f32, y: f32, heading: f32) -> SpawnData {
        let mut t = SpawnData::default();
        t.x = x;
        t.y = y;
        t.heading = heading;
        t
    }

    #[test]
    fn in_melee_range_close() {
        let player = make_player(0.0, 0.0);
        let target = make_target(10.0, 0.0, 0.0);
        assert!(is_in_melee_range(&player, &target));
    }

    #[test]
    fn out_of_melee_range() {
        let player = make_player(0.0, 0.0);
        let target = make_target(50.0, 0.0, 0.0);
        assert!(!is_in_melee_range(&player, &target));
    }

    #[test]
    fn check_position_move_toward_when_far() {
        let player = make_player(0.0, 0.0);
        let target = make_target(50.0, 0.0, 0.0);
        let action = check_melee_position(&player, &target, false, None, None);
        assert!(matches!(action, PositionAction::MoveToward(_)));
    }

    #[test]
    fn check_position_none_when_close() {
        let player = make_player(0.0, 0.0);
        let target = make_target(10.0, 0.0, 0.0);
        let action = check_melee_position(&player, &target, false, None, None);
        assert_eq!(action, PositionAction::None);
    }

    #[test]
    fn rogue_backstab_positioning() {
        let player = make_player(0.0, 0.0);
        // Target at (10, 0) facing north (heading 0) — "behind" is south
        let target = make_target(10.0, 0.0, 0.0);
        let action = check_melee_position(&player, &target, true, None, None);
        // Rogue should want to move behind
        assert!(matches!(action, PositionAction::MoveBehind(_)));
    }

    #[test]
    fn return_to_camp_when_drifted() {
        let player = make_player(200.0, 0.0);
        let target = make_target(210.0, 0.0, 0.0);
        let camp = Waypoint::new(0.0, 0.0, 0.0);
        let action = check_melee_position(&player, &target, false, Some(&camp), None);
        assert!(matches!(action, PositionAction::ReturnToCamp(_)));
    }

    #[test]
    fn aoe_avoidance_below_threshold() {
        let player = make_player(0.0, 0.0);
        let enemies = vec![make_target(10.0, 0.0, 0.0)];
        assert!(check_aoe_avoidance(&player, &enemies, 3).is_none());
    }

    #[test]
    fn aoe_avoidance_triggers_when_clustered() {
        let player = make_player(5.0, 5.0);
        let enemies = vec![
            make_target(0.0, 0.0, 0.0),
            make_target(10.0, 0.0, 0.0),
            make_target(5.0, 10.0, 0.0),
        ];
        let result = check_aoe_avoidance(&player, &enemies, 3);
        assert!(result.is_some());
    }

    #[test]
    fn aoe_avoidance_safe_when_far_from_cluster() {
        let player = make_player(100.0, 100.0);
        let enemies = vec![
            make_target(0.0, 0.0, 0.0),
            make_target(10.0, 0.0, 0.0),
            make_target(5.0, 10.0, 0.0),
        ];
        assert!(check_aoe_avoidance(&player, &enemies, 3).is_none());
    }

    #[test]
    fn behind_target_produces_valid_waypoint() {
        let target = make_target(100.0, 100.0, 0.0); // facing north
        let behind = behind_target(&target);
        // Behind a north-facing mob should be south (higher y in EQ coords)
        // Just verify it produces a valid point near the target
        let dist = ((behind.x - target.x).powi(2) + (behind.y - target.y).powi(2)).sqrt();
        assert!((dist - 8.0).abs() < 0.1); // Should be ~8 units away
    }

    #[test]
    fn behind_target_east_facing() {
        let target = make_target(50.0, 50.0, 128.0); // facing east
        let behind = behind_target(&target);
        let dist = ((behind.x - target.x).powi(2) + (behind.y - target.y).powi(2)).sqrt();
        assert!((dist - 8.0).abs() < 0.1);
    }

    #[test]
    fn behind_target_south_facing() {
        let target = make_target(50.0, 50.0, 256.0); // facing south
        let behind = behind_target(&target);
        let dist = ((behind.x - target.x).powi(2) + (behind.y - target.y).powi(2)).sqrt();
        assert!((dist - 8.0).abs() < 0.1);
    }

    #[test]
    fn melee_range_exact_boundary() {
        let player = make_player(0.0, 0.0);
        let target = make_target(MELEE_RANGE, 0.0, 0.0);
        assert!(is_in_melee_range(&player, &target));
    }

    #[test]
    fn melee_range_just_outside() {
        let player = make_player(0.0, 0.0);
        let target = make_target(MELEE_RANGE + 0.1, 0.0, 0.0);
        assert!(!is_in_melee_range(&player, &target));
    }

    #[test]
    fn same_position_is_in_melee_range() {
        let player = make_player(50.0, 50.0);
        let target = make_target(50.0, 50.0, 0.0);
        assert!(is_in_melee_range(&player, &target));
    }

    #[test]
    fn non_rogue_in_melee_range_is_none() {
        let player = make_player(10.0, 0.0);
        let target = make_target(10.0, 5.0, 0.0);
        let action = check_melee_position(&player, &target, false, None, None);
        assert_eq!(action, PositionAction::None);
    }

    #[test]
    fn camp_drift_within_limit_not_returned() {
        let player = make_player(50.0, 0.0);
        let target = make_target(55.0, 0.0, 0.0);
        let camp = Waypoint::new(0.0, 0.0, 0.0);
        // 50 < DEFAULT_CAMP_DRIFT (100), should check melee not camp
        let action = check_melee_position(&player, &target, false, Some(&camp), None);
        assert_eq!(action, PositionAction::None);
    }

    #[test]
    fn aoe_avoidance_empty_enemies() {
        let player = make_player(0.0, 0.0);
        assert!(check_aoe_avoidance(&player, &[], 3).is_none());
    }

    #[test]
    fn aoe_avoidance_escape_direction_away_from_center() {
        let player = make_player(5.0, 0.0);
        let enemies = vec![
            make_target(0.0, 0.0, 0.0),
            make_target(0.0, 10.0, 0.0),
            make_target(0.0, -10.0, 0.0),
        ];
        let escape = check_aoe_avoidance(&player, &enemies, 3).unwrap();
        // Center of mass is (0, 0). Player at (5, 0). Escape should be further right.
        assert!(escape.x > player.x);
    }

    #[test]
    fn distance_2d_ignores_z() {
        let a = SpawnData {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            ..SpawnData::default()
        };
        let b = SpawnData {
            x: 3.0,
            y: 4.0,
            z: 100.0,
            ..SpawnData::default()
        };
        let dist = Waypoint::new(a.x, a.y, 0.0).distance_2d(&Waypoint::new(b.x, b.y, 0.0));
        assert!((dist - 5.0).abs() < 0.01);
    }

    #[test]
    fn custom_camp_radius_triggers_return() {
        let player = make_player(40.0, 0.0);
        let target = make_target(45.0, 0.0, 0.0);
        let camp = Waypoint::new(0.0, 0.0, 0.0);
        let action = check_melee_position(&player, &target, false, Some(&camp), Some(30.0));
        assert!(matches!(action, PositionAction::ReturnToCamp(_)));
    }

    #[test]
    fn custom_camp_radius_within_limit() {
        let player = make_player(40.0, 0.0);
        let target = make_target(45.0, 0.0, 0.0);
        let camp = Waypoint::new(0.0, 0.0, 0.0);
        let action = check_melee_position(&player, &target, false, Some(&camp), Some(50.0));
        assert_eq!(action, PositionAction::None);
    }
}
