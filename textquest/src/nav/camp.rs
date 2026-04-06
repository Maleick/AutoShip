//! Camp position management — assign characters to role-based spots.

use std::collections::HashMap;
use textquest_common::nav::{CampDefinition, CampSpot, FollowConfig, Waypoint};
use textquest_common::types::ClientId;

/// Minimum leader movement distance (2-D) that triggers an `UpdateFollowAnchor`
/// broadcast. Below this threshold we skip the update to avoid churning IPC.
pub const FOLLOW_ANCHOR_UPDATE_THRESHOLD: f32 = 2.0;

/// Orchestrator-side state for MQ2MoveUtils `/makecamp player` follow mode.
///
/// Tracks the leader's last-known position and determines when to broadcast
/// `UpdateFollowAnchor` IPC commands to the followers.
#[derive(Debug, Clone)]
pub struct PlayerFollowMode {
    /// Follow configuration shared with each follower DLL.
    pub config: FollowConfig,
    /// Last anchor position broadcast to followers.
    last_anchor: Option<Waypoint>,
}

impl PlayerFollowMode {
    /// Create a new player follow mode tracker.
    #[must_use]
    pub fn new(config: FollowConfig) -> Self {
        Self {
            config,
            last_anchor: None,
        }
    }

    /// Check whether the leader has moved far enough to warrant an anchor update.
    ///
    /// Returns the new anchor if the leader has moved more than
    /// `FOLLOW_ANCHOR_UPDATE_THRESHOLD` units since the last broadcast, or if
    /// this is the initial position. Returns `None` if no update is needed.
    pub fn check_anchor_update(&mut self, leader_pos: Waypoint) -> Option<Waypoint> {
        match self.last_anchor {
            None => {
                // First position — always broadcast.
                self.last_anchor = Some(leader_pos);
                Some(leader_pos)
            }
            Some(prev) => {
                if prev.distance_2d(&leader_pos) >= FOLLOW_ANCHOR_UPDATE_THRESHOLD {
                    self.last_anchor = Some(leader_pos);
                    Some(leader_pos)
                } else {
                    None
                }
            }
        }
    }

    /// Force the last anchor to the given position without triggering an update.
    /// Used when follow mode is first set up to avoid a double-broadcast.
    pub fn set_last_anchor(&mut self, pos: Waypoint) {
        self.last_anchor = Some(pos);
    }

    /// Returns the last anchor position that was broadcast to followers.
    #[must_use]
    pub fn last_anchor(&self) -> Option<Waypoint> {
        self.last_anchor
    }
}

/// Manages camp assignments for a group.
pub struct CampManager {
    /// Current camp definition (if any).
    active_camp: Option<CampDefinition>,
    /// `client_id` -> assigned role.
    assignments: HashMap<ClientId, String>,
    /// Optional player follow mode (dynamic anchor).
    follow_mode: Option<PlayerFollowMode>,
}

impl CampManager {
    /// Create a new camp manager with no active camp.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_camp: None,
            assignments: HashMap::new(),
            follow_mode: None,
        }
    }

    /// Set the active camp and assign characters to spots based on their roles.
    /// `role_map` maps `client_id` to their role string (e.g., "tank", "healer1").
    pub fn set_camp(
        &mut self,
        camp: CampDefinition,
        role_map: &HashMap<ClientId, String>,
    ) -> Vec<(ClientId, CampSpot)> {
        // Starting a static camp clears any active follow mode.
        self.follow_mode = None;

        let mut result = Vec::new();

        for (client_id, role) in role_map {
            if let Some(spot) = camp.spots.iter().find(|s| s.role == *role) {
                self.assignments.insert(*client_id, role.clone());
                result.push((*client_id, spot.clone()));
            } else {
                tracing::warn!(client_id, role = %role, "No camp spot defined for role");
            }
        }

        self.active_camp = Some(camp);
        result
    }

    /// Start MQ2MoveUtils-style `/makecamp player` follow mode.
    ///
    /// Enables a dynamic anchor that tracks the named leader's position.
    /// Returns the initial `FollowPlayer` commands for each follower client
    /// (all client IDs in `follower_ids`) using `initial_anchor` as the
    /// starting position.
    ///
    /// Clears any active static camp; the two modes are mutually exclusive.
    pub fn set_player_follow(
        &mut self,
        config: FollowConfig,
        follower_ids: &[ClientId],
        initial_anchor: Waypoint,
    ) -> Vec<(ClientId, textquest_common::ipc::Command)> {
        // Player follow mode replaces any static camp.
        self.active_camp = None;
        self.assignments.clear();

        let mut mode = PlayerFollowMode::new(config.clone());
        mode.set_last_anchor(initial_anchor);
        self.follow_mode = Some(mode);

        follower_ids
            .iter()
            .map(|&id| {
                (
                    id,
                    textquest_common::ipc::Command::FollowPlayer {
                        config: config.clone(),
                        anchor_x: initial_anchor.x,
                        anchor_y: initial_anchor.y,
                        anchor_z: initial_anchor.z,
                    },
                )
            })
            .collect()
    }

    /// Stop player follow mode.
    ///
    /// Returns `StopFollow` commands for every client in `follower_ids`.
    pub fn clear_follow(
        &mut self,
        follower_ids: &[ClientId],
    ) -> Vec<(ClientId, textquest_common::ipc::Command)> {
        self.follow_mode = None;
        follower_ids
            .iter()
            .map(|&id| (id, textquest_common::ipc::Command::StopFollow))
            .collect()
    }

    /// Tick player follow mode: check whether the leader has moved and return
    /// `UpdateFollowAnchor` commands for all followers if so.
    ///
    /// Returns an empty `Vec` if follow mode is not active, or if the leader
    /// hasn't moved far enough to warrant a broadcast.
    pub fn tick_follow(
        &mut self,
        leader_pos: Waypoint,
        follower_ids: &[ClientId],
    ) -> Vec<(ClientId, textquest_common::ipc::Command)> {
        let follow = match self.follow_mode.as_mut() {
            Some(f) => f,
            None => return Vec::new(),
        };

        match follow.check_anchor_update(leader_pos) {
            None => Vec::new(),
            Some(anchor) => follower_ids
                .iter()
                .map(|&id| {
                    (
                        id,
                        textquest_common::ipc::Command::UpdateFollowAnchor {
                            x: anchor.x,
                            y: anchor.y,
                            z: anchor.z,
                        },
                    )
                })
                .collect(),
        }
    }

    /// Get the camp spot for a specific client.
    #[must_use]
    pub fn get_spot(&self, client_id: ClientId) -> Option<&CampSpot> {
        let role = self.assignments.get(&client_id)?;
        self.active_camp
            .as_ref()?
            .spots
            .iter()
            .find(|s| s.role == *role)
    }

    /// Clear the active camp.
    pub fn clear(&mut self) {
        self.active_camp = None;
        self.assignments.clear();
        self.follow_mode = None;
    }

    /// Whether a camp is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active_camp.is_some()
    }

    /// Whether player follow mode is active.
    #[must_use]
    pub fn is_following(&self) -> bool {
        self.follow_mode.is_some()
    }

    /// Returns the active follow configuration, if any.
    #[must_use]
    pub fn follow_config(&self) -> Option<&FollowConfig> {
        self.follow_mode.as_ref().map(|f| &f.config)
    }
}

/// Helper: create a basic group camp with standard EQ positioning.
/// Tank in front, healer behind, DPS spread in a semicircle.
#[must_use]
pub fn create_standard_camp(center: Waypoint, pull_heading: f32, num_dps: usize) -> CampDefinition {
    let mut spots = Vec::new();

    // Tank: 20 units in the pull direction.
    let pull_rad = pull_heading * std::f32::consts::PI * 2.0 / 512.0;
    spots.push(CampSpot {
        position: Waypoint::new(
            center.x + 20.0 * pull_rad.sin(),
            center.y + 20.0 * pull_rad.cos(),
            center.z,
        ),
        heading: pull_heading,
        role: "tank".to_string(),
    });

    // Healer: 15 units behind center (opposite pull direction).
    let back_heading = (pull_heading + 256.0) % 512.0;
    let back_rad = back_heading * std::f32::consts::PI * 2.0 / 512.0;
    spots.push(CampSpot {
        position: Waypoint::new(
            center.x + 15.0 * back_rad.sin(),
            center.y + 15.0 * back_rad.cos(),
            center.z,
        ),
        heading: pull_heading,
        role: "healer".to_string(),
    });

    // DPS: spread in a semicircle behind center.
    for i in 0..num_dps {
        let angle_offset = (i as f32 / num_dps as f32 - 0.5) * 128.0; // +/- 45 degrees
        let dps_heading = (back_heading + angle_offset + 512.0) % 512.0;
        let dps_rad = dps_heading * std::f32::consts::PI * 2.0 / 512.0;
        spots.push(CampSpot {
            position: Waypoint::new(
                center.x + 18.0 * dps_rad.sin(),
                center.y + 18.0 * dps_rad.cos(),
                center.z,
            ),
            heading: pull_heading,
            role: format!("dps{}", i + 1),
        });
    }

    CampDefinition {
        name: "standard".to_string(),
        zone: String::new(),
        spots,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_camp(num_dps: usize) -> CampDefinition {
        create_standard_camp(Waypoint::new(0.0, 0.0, 0.0), 128.0, num_dps)
    }

    #[test]
    fn camp_manager_starts_inactive() {
        let mgr = CampManager::new();
        assert!(!mgr.is_active());
    }

    #[test]
    fn set_camp_activates_and_assigns_spots() {
        let mut mgr = CampManager::new();
        let camp = make_camp(2);

        let mut role_map = HashMap::new();
        role_map.insert(1, "tank".to_string());
        role_map.insert(2, "healer".to_string());
        role_map.insert(3, "dps1".to_string());
        role_map.insert(4, "dps2".to_string());

        let assignments = mgr.set_camp(camp, &role_map);
        assert!(mgr.is_active());
        assert_eq!(assignments.len(), 4);
    }

    #[test]
    fn set_camp_skips_unmatched_roles() {
        let mut mgr = CampManager::new();
        let camp = make_camp(1); // has tank, healer, dps1

        let mut role_map = HashMap::new();
        role_map.insert(1, "tank".to_string());
        role_map.insert(2, "nonexistent_role".to_string());

        let assignments = mgr.set_camp(camp, &role_map);
        // Only "tank" matches, "nonexistent_role" has no spot
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].0, 1);
    }

    #[test]
    fn get_spot_returns_correct_spot() {
        let mut mgr = CampManager::new();
        let camp = make_camp(1);

        let mut role_map = HashMap::new();
        role_map.insert(1, "tank".to_string());
        mgr.set_camp(camp, &role_map);

        let spot = mgr.get_spot(1);
        assert!(spot.is_some());
        assert_eq!(spot.unwrap().role, "tank");
    }

    #[test]
    fn get_spot_returns_none_for_unassigned_client() {
        let mut mgr = CampManager::new();
        let camp = make_camp(1);
        let role_map = HashMap::new();
        mgr.set_camp(camp, &role_map);

        assert!(mgr.get_spot(99).is_none());
    }

    #[test]
    fn get_spot_returns_none_when_no_camp_active() {
        let mgr = CampManager::new();
        assert!(mgr.get_spot(1).is_none());
    }

    #[test]
    fn clear_deactivates_camp() {
        let mut mgr = CampManager::new();
        let camp = make_camp(1);
        let mut role_map = HashMap::new();
        role_map.insert(1, "tank".to_string());
        mgr.set_camp(camp, &role_map);
        assert!(mgr.is_active());

        mgr.clear();
        assert!(!mgr.is_active());
        assert!(mgr.get_spot(1).is_none());
    }

    #[test]
    fn create_standard_camp_has_correct_spot_count() {
        let camp = create_standard_camp(Waypoint::new(0.0, 0.0, 0.0), 0.0, 4);
        // 1 tank + 1 healer + 4 dps = 6
        assert_eq!(camp.spots.len(), 6);
        assert_eq!(camp.spots[0].role, "tank");
        assert_eq!(camp.spots[1].role, "healer");
        assert_eq!(camp.spots[2].role, "dps1");
        assert_eq!(camp.spots[3].role, "dps2");
        assert_eq!(camp.spots[4].role, "dps3");
        assert_eq!(camp.spots[5].role, "dps4");
    }

    #[test]
    fn create_standard_camp_zero_dps() {
        let camp = create_standard_camp(Waypoint::new(0.0, 0.0, 0.0), 0.0, 0);
        // 1 tank + 1 healer = 2
        assert_eq!(camp.spots.len(), 2);
    }

    #[test]
    fn create_standard_camp_name_and_zone() {
        let camp = create_standard_camp(Waypoint::new(0.0, 0.0, 0.0), 0.0, 0);
        assert_eq!(camp.name, "standard");
        assert_eq!(camp.zone, "");
    }

    #[test]
    fn create_standard_camp_preserves_z() {
        let camp = create_standard_camp(Waypoint::new(0.0, 0.0, 42.0), 0.0, 2);
        for spot in &camp.spots {
            assert!((spot.position.z - 42.0).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn create_standard_camp_dps_roles_numbered() {
        let camp = create_standard_camp(Waypoint::new(0.0, 0.0, 0.0), 0.0, 5);
        let dps_roles: Vec<&str> = camp
            .spots
            .iter()
            .filter(|s| s.role.starts_with("dps"))
            .map(|s| s.role.as_str())
            .collect();
        assert_eq!(dps_roles, vec!["dps1", "dps2", "dps3", "dps4", "dps5"]);
    }

    #[test]
    fn set_camp_replaces_previous_camp() {
        let mut mgr = CampManager::new();
        let camp1 = make_camp(1);
        let camp2 = make_camp(2);

        let mut role_map = HashMap::new();
        role_map.insert(1, "tank".to_string());
        mgr.set_camp(camp1, &role_map);
        assert!(mgr.is_active());

        let mut role_map2 = HashMap::new();
        role_map2.insert(2, "healer".to_string());
        mgr.set_camp(camp2, &role_map2);

        // Client 1's tank assignment should still exist from old assignments,
        // but the spot lookup uses the new camp definition
        assert!(mgr.get_spot(2).is_some());
    }

    #[test]
    fn all_spots_have_correct_heading() {
        let pull_heading = 200.0;
        let camp = create_standard_camp(Waypoint::new(0.0, 0.0, 0.0), pull_heading, 3);
        for spot in &camp.spots {
            assert!(
                (spot.heading - pull_heading).abs() < f32::EPSILON,
                "All spots should face the pull direction"
            );
        }
    }

    #[test]
    fn tank_is_closer_to_pull_direction() {
        let camp = create_standard_camp(Waypoint::new(0.0, 0.0, 0.0), 0.0, 1);
        let tank = &camp.spots[0];
        let healer = &camp.spots[1];
        // Tank should be 20 units from center, healer 15 units but in opposite direction
        let tank_dist = (tank.position.x.powi(2) + tank.position.y.powi(2)).sqrt();
        let healer_dist = (healer.position.x.powi(2) + healer.position.y.powi(2)).sqrt();
        assert!((tank_dist - 20.0).abs() < 0.1);
        assert!((healer_dist - 15.0).abs() < 0.1);
    }

    #[test]
    fn get_spot_after_clear_returns_none() {
        let mut mgr = CampManager::new();
        let camp = make_camp(1);
        let mut role_map = HashMap::new();
        role_map.insert(1, "tank".to_string());
        mgr.set_camp(camp, &role_map);
        assert!(mgr.get_spot(1).is_some());

        mgr.clear();
        assert!(mgr.get_spot(1).is_none());
    }

    // ─── Player follow mode tests ───

    fn make_follow_config() -> FollowConfig {
        FollowConfig::new("Leader", 10.0, 50.0)
    }

    #[test]
    fn follow_mode_starts_inactive() {
        let mgr = CampManager::new();
        assert!(!mgr.is_following());
        assert!(mgr.follow_config().is_none());
    }

    #[test]
    fn set_player_follow_activates_follow_mode() {
        let mut mgr = CampManager::new();
        let anchor = Waypoint::new(100.0, 200.0, 0.0);
        let cmds = mgr.set_player_follow(make_follow_config(), &[1, 2, 3], anchor);

        assert!(mgr.is_following());
        assert_eq!(cmds.len(), 3);
        // Each command should be a FollowPlayer with matching anchor coords.
        for (_, cmd) in &cmds {
            match cmd {
                textquest_common::ipc::Command::FollowPlayer {
                    anchor_x,
                    anchor_y,
                    anchor_z,
                    ..
                } => {
                    assert!((*anchor_x - 100.0).abs() < f32::EPSILON);
                    assert!((*anchor_y - 200.0).abs() < f32::EPSILON);
                    assert!((*anchor_z - 0.0).abs() < f32::EPSILON);
                }
                other => panic!("Expected FollowPlayer, got {other:?}"),
            }
        }
    }

    #[test]
    fn set_player_follow_clears_static_camp() {
        let mut mgr = CampManager::new();
        let camp = make_camp(1);
        let mut role_map = HashMap::new();
        role_map.insert(1, "tank".to_string());
        mgr.set_camp(camp, &role_map);
        assert!(mgr.is_active());

        // Activating follow mode should clear the static camp.
        let anchor = Waypoint::new(0.0, 0.0, 0.0);
        mgr.set_player_follow(make_follow_config(), &[1], anchor);
        assert!(
            !mgr.is_active(),
            "Static camp should be cleared by follow mode"
        );
    }

    #[test]
    fn set_camp_clears_follow_mode() {
        let mut mgr = CampManager::new();
        let anchor = Waypoint::new(0.0, 0.0, 0.0);
        mgr.set_player_follow(make_follow_config(), &[1], anchor);
        assert!(mgr.is_following());

        let camp = make_camp(1);
        let mut role_map = HashMap::new();
        role_map.insert(1, "tank".to_string());
        mgr.set_camp(camp, &role_map);
        assert!(
            !mgr.is_following(),
            "Follow mode should be cleared by set_camp"
        );
    }

    #[test]
    fn clear_follow_stops_follow_mode() {
        let mut mgr = CampManager::new();
        let anchor = Waypoint::new(0.0, 0.0, 0.0);
        mgr.set_player_follow(make_follow_config(), &[1, 2], anchor);

        let cmds = mgr.clear_follow(&[1, 2]);
        assert!(!mgr.is_following());
        assert_eq!(cmds.len(), 2);
        for (_, cmd) in &cmds {
            assert!(
                matches!(cmd, textquest_common::ipc::Command::StopFollow),
                "Expected StopFollow command"
            );
        }
    }

    #[test]
    fn tick_follow_no_update_when_not_following() {
        let mut mgr = CampManager::new();
        let cmds = mgr.tick_follow(Waypoint::new(10.0, 10.0, 0.0), &[1, 2]);
        assert!(cmds.is_empty());
    }

    #[test]
    fn tick_follow_broadcasts_initial_anchor() {
        let mut mgr = CampManager::new();
        let config = make_follow_config();
        let initial = Waypoint::new(0.0, 0.0, 0.0);

        // Manually create follow mode without setting last_anchor to test first tick.
        let mode = PlayerFollowMode::new(config.clone());
        mgr.follow_mode = Some(mode);

        // First tick should always broadcast.
        let cmds = mgr.tick_follow(initial, &[1, 2]);
        assert_eq!(cmds.len(), 2);
    }

    #[test]
    fn tick_follow_no_update_when_leader_stationary() {
        let mut mgr = CampManager::new();
        let anchor = Waypoint::new(100.0, 100.0, 0.0);
        mgr.set_player_follow(make_follow_config(), &[1], anchor);

        // Leader stays put — no update expected.
        let cmds = mgr.tick_follow(anchor, &[1]);
        assert!(
            cmds.is_empty(),
            "Should not broadcast if leader hasn't moved"
        );
    }

    #[test]
    fn tick_follow_broadcasts_when_leader_moves_past_threshold() {
        let mut mgr = CampManager::new();
        let anchor = Waypoint::new(0.0, 0.0, 0.0);
        mgr.set_player_follow(make_follow_config(), &[1], anchor);

        // Move leader past the threshold (2.0 units).
        let new_pos = Waypoint::new(5.0, 0.0, 0.0); // 5 > 2.0 threshold
        let cmds = mgr.tick_follow(new_pos, &[1]);
        assert_eq!(cmds.len(), 1);
        match &cmds[0].1 {
            textquest_common::ipc::Command::UpdateFollowAnchor { x, y, z } => {
                assert!((*x - 5.0).abs() < f32::EPSILON);
                assert!((*y - 0.0).abs() < f32::EPSILON);
                assert!((*z - 0.0).abs() < f32::EPSILON);
            }
            other => panic!("Expected UpdateFollowAnchor, got {other:?}"),
        }
    }

    #[test]
    fn tick_follow_no_update_when_leader_moves_below_threshold() {
        let mut mgr = CampManager::new();
        let anchor = Waypoint::new(0.0, 0.0, 0.0);
        mgr.set_player_follow(make_follow_config(), &[1], anchor);

        // Move leader less than threshold (2.0 units).
        let new_pos = Waypoint::new(1.0, 0.0, 0.0); // 1.0 < 2.0 threshold
        let cmds = mgr.tick_follow(new_pos, &[1]);
        assert!(cmds.is_empty(), "Should not broadcast minor movement");
    }

    #[test]
    fn follow_config_stored_and_accessible() {
        let mut mgr = CampManager::new();
        let config = FollowConfig::new("Camrene", 15.0, 75.0);
        let anchor = Waypoint::new(0.0, 0.0, 0.0);
        mgr.set_player_follow(config.clone(), &[1], anchor);

        let stored = mgr
            .follow_config()
            .expect("follow config should be present");
        assert_eq!(stored.leader_name, "Camrene");
        assert!((stored.follow_distance - 15.0).abs() < f32::EPSILON);
        assert!((stored.leash_distance - 75.0).abs() < f32::EPSILON);
    }

    #[test]
    fn player_follow_mode_check_anchor_first_call_always_broadcasts() {
        let config = make_follow_config();
        let mut mode = PlayerFollowMode::new(config);
        let pos = Waypoint::new(50.0, 50.0, 0.0);
        assert!(
            mode.check_anchor_update(pos).is_some(),
            "First call must broadcast regardless of position"
        );
    }

    #[test]
    fn player_follow_mode_check_anchor_no_update_below_threshold() {
        let config = make_follow_config();
        let mut mode = PlayerFollowMode::new(config);
        let pos = Waypoint::new(0.0, 0.0, 0.0);
        mode.set_last_anchor(pos);

        // Small movement below threshold.
        let minor = Waypoint::new(1.0, 0.0, 0.0);
        assert!(
            mode.check_anchor_update(minor).is_none(),
            "Should not broadcast for sub-threshold movement"
        );
    }

    #[test]
    fn player_follow_mode_check_anchor_update_above_threshold() {
        let config = make_follow_config();
        let mut mode = PlayerFollowMode::new(config);
        let origin = Waypoint::new(0.0, 0.0, 0.0);
        mode.set_last_anchor(origin);

        let far = Waypoint::new(10.0, 0.0, 0.0);
        let result = mode.check_anchor_update(far);
        assert!(
            result.is_some(),
            "Should broadcast for significant movement"
        );
        let new_anchor = result.unwrap();
        assert!((new_anchor.x - 10.0).abs() < f32::EPSILON);

        // Last anchor should now be updated.
        assert!(mode.last_anchor().is_some());
        assert!((mode.last_anchor().unwrap().x - 10.0).abs() < f32::EPSILON);
    }
}
