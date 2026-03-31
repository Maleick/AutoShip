//! Melee positioning logic — tank facing, rogue backstab angle, camp range enforcement.
//!
//! EQ melee attacks have positional requirements:
//! - Tanks should face the mob (auto-attack misses from behind)
//! - Rogues must be behind the mob for backstab
//! - All melee should stay within ~15 unit range
//! - Characters should return to camp after combat if they've drifted

use dmft_common::nav::Waypoint;
use dmft_common::types::SpawnData;

/// Maximum melee range in EQ units. Beyond this, melee attacks won't connect.
const MELEE_RANGE: f32 = 15.0;

/// Distance threshold for "close enough" — don't micro-adjust within this range.
const CLOSE_ENOUGH: f32 = 5.0;

/// Maximum distance a character can drift from camp before being pulled back.
const MAX_CAMP_DRIFT: f32 = 100.0;

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
    let dist = distance_2d(player, target);
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
) -> PositionAction {
    let dist = distance_2d(player, target);

    // Priority 1: If too far from camp, return to camp (after combat)
    if let Some(camp) = camp_pos {
        let camp_dist = ((player.x - camp.x).powi(2) + (player.y - camp.y).powi(2)).sqrt();
        if camp_dist > MAX_CAMP_DRIFT {
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

/// Check for AoE avoidance — returns a position to move to if needed.
/// Requires integration with the navmesh for pathfinding away from AoE zones.
/// Currently checks nearby enemies for large AoE clusters.
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
    let dist_to_center = ((player.x - cx).powi(2) + (player.y - cy).powi(2)).sqrt();
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

/// 2D distance between two spawns (ignoring Z for melee range checks).
fn distance_2d(a: &SpawnData, b: &SpawnData) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
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
        let action = check_melee_position(&player, &target, false, None);
        assert!(matches!(action, PositionAction::MoveToward(_)));
    }

    #[test]
    fn check_position_none_when_close() {
        let player = make_player(0.0, 0.0);
        let target = make_target(10.0, 0.0, 0.0);
        let action = check_melee_position(&player, &target, false, None);
        assert_eq!(action, PositionAction::None);
    }

    #[test]
    fn rogue_backstab_positioning() {
        let player = make_player(0.0, 0.0);
        // Target at (10, 0) facing north (heading 0) — "behind" is south
        let target = make_target(10.0, 0.0, 0.0);
        let action = check_melee_position(&player, &target, true, None);
        // Rogue should want to move behind
        assert!(matches!(action, PositionAction::MoveBehind(_)));
    }

    #[test]
    fn return_to_camp_when_drifted() {
        let player = make_player(200.0, 0.0);
        let target = make_target(210.0, 0.0, 0.0);
        let camp = Waypoint::new(0.0, 0.0, 0.0);
        let action = check_melee_position(&player, &target, false, Some(&camp));
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
}
