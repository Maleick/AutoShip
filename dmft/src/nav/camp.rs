//! Camp position management — assign characters to role-based spots.

use dmft_common::nav::{CampDefinition, CampSpot, Waypoint};
use dmft_common::types::ClientId;
use std::collections::HashMap;

/// Manages camp assignments for a group.
pub struct CampManager {
    /// Current camp definition (if any).
    active_camp: Option<CampDefinition>,
    /// `client_id` -> assigned role.
    assignments: HashMap<ClientId, String>,
}

impl CampManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_camp: None,
            assignments: HashMap::new(),
        }
    }

    /// Set the active camp and assign characters to spots based on their roles.
    /// `role_map` maps `client_id` to their role string (e.g., "tank", "healer1").
    pub fn set_camp(
        &mut self,
        camp: CampDefinition,
        role_map: &HashMap<ClientId, String>,
    ) -> Vec<(ClientId, CampSpot)> {
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
    }

    /// Whether a camp is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active_camp.is_some()
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
}
