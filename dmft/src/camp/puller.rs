//! Pull target selection — picks the best mob to pull from nearby spawns.

use crate::camp::cc::CcTracker;
use crate::camp::config::CampConfig;
use crate::camp::positioning::distance_2d;
use crate::eq::named_tracker::NamedTracker;

/// Spawn type discriminator matching EQ's internal spawn types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnType {
    Player,
    NPC,
    Corpse,
    Other,
}

/// A nearby spawn visible to the puller.
#[derive(Debug, Clone)]
pub struct NearbySpawn {
    pub spawn_id: u32,
    pub name: String,
    pub spawn_type: SpawnType,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Select the best pull target from nearby spawns.
///
/// Filters:
/// - Must be an NPC (not player, corpse, or other)
/// - Must be within `pull_radius` of the camp's `pull_point`
/// - Must not already be tracked by the CC system (already pulled/CC'd)
///
/// Preference order:
/// 1. Mobs matching `pull_mob_names` from camp config (if configured)
/// 2. Mobs on the HVT (high-value target) watchlist
/// 3. Closest NPC to the pull point
pub fn select_pull_target(
    nearby_spawns: &[NearbySpawn],
    camp_config: &CampConfig,
    cc_tracker: &CcTracker,
    hvt_watchlist: &[String],
) -> Option<String> {
    let pull_x = camp_config.pull_point[0];
    let pull_y = camp_config.pull_point[1];

    // IDs already tracked by CC (already in camp, being CC'd or killed)
    let cc_ids: Vec<u32> = cc_tracker.targets.iter().map(|t| t.spawn_id).collect();

    // Filter to valid pull candidates
    let candidates: Vec<&NearbySpawn> = nearby_spawns
        .iter()
        .filter(|s| s.spawn_type == SpawnType::NPC)
        .filter(|s| distance_2d(s.x, s.y, pull_x, pull_y) <= camp_config.pull_radius)
        .filter(|s| !cc_ids.contains(&s.spawn_id))
        .collect();

    if candidates.is_empty() {
        return None;
    }

    // Prefer configured pull mob names
    if !camp_config.pull_mob_names.is_empty() {
        let config_match = candidates
            .iter()
            .filter(|s| camp_config.pull_mob_names.contains(&s.name))
            .min_by(|a, b| {
                let da = distance_2d(a.x, a.y, pull_x, pull_y);
                let db = distance_2d(b.x, b.y, pull_x, pull_y);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(target) = config_match {
            return Some(target.name.clone());
        }
    }

    // Prefer HVT watchlist targets
    if !hvt_watchlist.is_empty() {
        let hvt_match = candidates
            .iter()
            .filter(|s| hvt_watchlist.contains(&s.name))
            .min_by(|a, b| {
                let da = distance_2d(a.x, a.y, pull_x, pull_y);
                let db = distance_2d(b.x, b.y, pull_x, pull_y);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            });
        if let Some(target) = hvt_match {
            return Some(target.name.clone());
        }
    }

    // Fall back to closest NPC
    candidates
        .iter()
        .min_by(|a, b| {
            let da = distance_2d(a.x, a.y, pull_x, pull_y);
            let db = distance_2d(b.x, b.y, pull_x, pull_y);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|s| s.name.clone())
}

/// Extended pull target selection that checks the named tracker first.
/// If a named mob from the database is alive and in range, it takes priority
/// over all other targets.
pub fn select_pull_target_with_named(
    nearby_spawns: &[NearbySpawn],
    camp_config: &CampConfig,
    cc_tracker: &CcTracker,
    hvt_watchlist: &[String],
    named_tracker: &NamedTracker,
) -> Option<String> {
    // Check for a high-priority named mob override
    if let Some(priority_named) = named_tracker.priority_target() {
        // Verify the named mob is actually in our spawn list and in range
        let pull_x = camp_config.pull_point[0];
        let pull_y = camp_config.pull_point[1];
        let cc_ids: Vec<u32> = cc_tracker.targets.iter().map(|t| t.spawn_id).collect();

        let in_range = nearby_spawns.iter().any(|s| {
            s.spawn_type == SpawnType::NPC
                && s.name == priority_named.name
                && distance_2d(s.x, s.y, pull_x, pull_y) <= camp_config.pull_radius
                && !cc_ids.contains(&s.spawn_id)
        });

        if in_range {
            return Some(priority_named.name.clone());
        }
    }

    // Fall back to normal selection
    select_pull_target(nearby_spawns, camp_config, cc_tracker, hvt_watchlist)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CampConfig {
        CampConfig {
            name: "test_camp".into(),
            zone: "crushbone".into(),
            camp_center: [100.0, 200.0, 0.0],
            pull_point: [150.0, 250.0, 0.0],
            pull_radius: 200.0,
            camp_radius: 30.0,
            leash_radius: 100.0,
            rest_mana_pct: 60,
            pull_mana_pct: 30,
            level_range: [5, 12],
            pull_mob_names: vec!["an orc pawn".into()],
            ignore_mob_names: Vec::new(),
            burn_mob_names: Vec::new(),
            next_camp: None,
            prev_camp: None,
        }
    }

    fn make_spawn(id: u32, name: &str, spawn_type: SpawnType, x: f32, y: f32) -> NearbySpawn {
        NearbySpawn {
            spawn_id: id,
            name: name.into(),
            spawn_type,
            x,
            y,
            z: 0.0,
        }
    }

    #[test]
    fn test_selects_configured_mob_name() {
        let spawns = vec![
            make_spawn(1, "an orc centurion", SpawnType::NPC, 160.0, 260.0),
            make_spawn(2, "an orc pawn", SpawnType::NPC, 170.0, 270.0),
        ];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &test_config(), &cc, &[]);
        assert_eq!(result, Some("an orc pawn".into()));
    }

    #[test]
    fn test_selects_hvt_when_no_config_match() {
        let mut config = test_config();
        config.pull_mob_names.clear();
        let spawns = vec![
            make_spawn(1, "an orc centurion", SpawnType::NPC, 160.0, 260.0),
            make_spawn(2, "a named mob", SpawnType::NPC, 170.0, 270.0),
        ];
        let cc = CcTracker::new();
        let hvt = vec!["a named mob".to_string()];
        let result = select_pull_target(&spawns, &config, &cc, &hvt);
        assert_eq!(result, Some("a named mob".into()));
    }

    #[test]
    fn test_selects_closest_fallback() {
        let mut config = test_config();
        config.pull_mob_names.clear();
        let spawns = vec![
            make_spawn(1, "far orc", SpawnType::NPC, 300.0, 400.0),
            make_spawn(2, "close orc", SpawnType::NPC, 155.0, 255.0),
        ];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &config, &cc, &[]);
        assert_eq!(result, Some("close orc".into()));
    }

    #[test]
    fn test_filters_out_players() {
        let spawns = vec![
            make_spawn(1, "PlayerChar", SpawnType::Player, 155.0, 255.0),
            make_spawn(2, "an orc pawn", SpawnType::NPC, 170.0, 270.0),
        ];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &test_config(), &cc, &[]);
        assert_eq!(result, Some("an orc pawn".into()));
    }

    #[test]
    fn test_filters_out_corpses() {
        let spawns = vec![
            make_spawn(1, "an orc pawn", SpawnType::Corpse, 155.0, 255.0),
        ];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &test_config(), &cc, &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn test_filters_out_cc_tracked_mobs() {
        let spawns = vec![
            make_spawn(1, "an orc pawn", SpawnType::NPC, 155.0, 255.0),
            make_spawn(2, "an orc centurion", SpawnType::NPC, 160.0, 260.0),
        ];
        let mut cc = CcTracker::new();
        cc.update(&[(1, "an orc pawn".into())], None, 0);
        let result = select_pull_target(&spawns, &test_config(), &cc, &[]);
        // Spawn 1 is tracked by CC, so should pick spawn 2
        assert_eq!(result, Some("an orc centurion".into()));
    }

    #[test]
    fn test_filters_out_of_range() {
        let spawns = vec![
            make_spawn(1, "an orc pawn", SpawnType::NPC, 9999.0, 9999.0),
        ];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &test_config(), &cc, &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn test_empty_spawns() {
        let cc = CcTracker::new();
        let result = select_pull_target(&[], &test_config(), &cc, &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn test_config_match_prefers_closest() {
        let config = CampConfig {
            pull_mob_names: vec!["an orc pawn".into()],
            ..test_config()
        };
        let spawns = vec![
            make_spawn(1, "an orc pawn", SpawnType::NPC, 300.0, 350.0),
            make_spawn(2, "an orc pawn", SpawnType::NPC, 155.0, 255.0),
        ];
        let cc = CcTracker::new();
        let result = select_pull_target(&spawns, &config, &cc, &[]);
        // Should pick the closer orc pawn (spawn 2)
        assert_eq!(result, Some("an orc pawn".into()));
    }
}
