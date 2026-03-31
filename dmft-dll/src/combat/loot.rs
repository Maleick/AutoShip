//! Loot automation — detect nearby corpses and loot them.
//!
//! Uses EQ's slash commands (`/loot`, `/lootall`) to interact with corpses.
//! Corpse detection walks the spawn list looking for spawn_type == 2 (corpse)
//! within loot range.

use dmft_common::nav::Waypoint;
use dmft_common::types::SpawnData;

/// Maximum range to detect lootable corpses (EQ units).
const LOOT_RANGE: f32 = 50.0;

/// Spawn type constant for corpses in EQ.
const SPAWN_TYPE_CORPSE: u8 = 2;

/// Attempt to loot the nearest corpse via /loot slash command.
///
/// The /loot command targets the nearest corpse within range and opens the loot window.
pub fn loot_nearest_corpse() {
    tracing::info!("Loot: attempting to loot nearest corpse via /loot");
    crate::hooks::game_loop::queue_slash_command("/loot".to_string());
}

/// Loot all items from the currently open loot window via /lootall.
///
/// Requires the loot window to already be open (from a prior /loot or corpse click).
pub fn loot_all_items() {
    tracing::info!("Loot: looting all items via /lootall");
    crate::hooks::game_loop::queue_slash_command("/lootall".to_string());
}

/// Find lootable corpses within range from the spawn list.
///
/// Returns spawn IDs of corpses within LOOT_RANGE of the player.
/// Corpses have spawn_type == 2 in EQ's spawn list.
pub fn find_lootable_corpses(player: &SpawnData, spawns: &[SpawnData]) -> Vec<u32> {
    spawns
        .iter()
        .filter(|s| s.spawn_type == SPAWN_TYPE_CORPSE)
        .filter(|s| {
            Waypoint::new(s.x, s.y, 0.0).distance_2d(&Waypoint::new(player.x, player.y, 0.0))
                <= LOOT_RANGE
        })
        .map(|s| s.spawn_id)
        .collect()
}

/// Check if there are nearby corpses that we could loot.
pub fn has_lootable_corpses(player: &SpawnData, spawns: &[SpawnData]) -> bool {
    spawns.iter().any(|s| {
        s.spawn_type == SPAWN_TYPE_CORPSE
            && Waypoint::new(s.x, s.y, 0.0).distance_2d(&Waypoint::new(player.x, player.y, 0.0))
                <= LOOT_RANGE
    })
}

/// Loot sequence: target corpse by ID, then /loot, then /lootall.
/// This is a multi-step sequence that should be called over several ticks:
/// 1. First tick: /target id {corpse_id}
/// 2. Wait ~10 ticks for target to register
/// 3. Second tick: /loot
/// 4. Wait ~20 ticks for loot window to open
/// 5. Third tick: /lootall
///
/// The caller (combat FSM or camp loop) manages the timing between steps.
pub fn begin_loot_sequence(corpse_id: u32) {
    tracing::info!(corpse_id, "Loot: beginning loot sequence");
    crate::hooks::game_loop::queue_slash_command(format!("/target id {corpse_id}"));
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

    fn make_corpse(id: u32, x: f32, y: f32) -> SpawnData {
        let mut s = SpawnData::default();
        s.spawn_id = id;
        s.spawn_type = SPAWN_TYPE_CORPSE;
        s.x = x;
        s.y = y;
        s
    }

    fn make_npc(id: u32, x: f32, y: f32) -> SpawnData {
        let mut s = SpawnData::default();
        s.spawn_id = id;
        s.spawn_type = 1; // NPC
        s.x = x;
        s.y = y;
        s
    }

    #[test]
    fn find_corpses_within_range() {
        let player = make_player(0.0, 0.0);
        let spawns = vec![
            make_corpse(1, 10.0, 0.0),  // in range
            make_corpse(2, 100.0, 0.0), // out of range
            make_npc(3, 5.0, 0.0),      // not a corpse
        ];
        let corpses = find_lootable_corpses(&player, &spawns);
        assert_eq!(corpses, vec![1]);
    }

    #[test]
    fn no_corpses_returns_empty() {
        let player = make_player(0.0, 0.0);
        let spawns = vec![make_npc(1, 10.0, 0.0)];
        let corpses = find_lootable_corpses(&player, &spawns);
        assert!(corpses.is_empty());
    }

    #[test]
    fn has_lootable_detects_nearby_corpse() {
        let player = make_player(0.0, 0.0);
        let spawns = vec![make_corpse(1, 10.0, 0.0)];
        assert!(has_lootable_corpses(&player, &spawns));
    }

    #[test]
    fn has_lootable_false_when_no_corpses() {
        let player = make_player(0.0, 0.0);
        let spawns = vec![make_npc(1, 10.0, 0.0)];
        assert!(!has_lootable_corpses(&player, &spawns));
    }

    #[test]
    fn has_lootable_false_when_corpse_out_of_range() {
        let player = make_player(0.0, 0.0);
        let spawns = vec![make_corpse(1, 200.0, 0.0)];
        assert!(!has_lootable_corpses(&player, &spawns));
    }

    #[test]
    fn multiple_corpses_in_range() {
        let player = make_player(0.0, 0.0);
        let spawns = vec![
            make_corpse(1, 10.0, 0.0),
            make_corpse(2, 20.0, 0.0),
            make_corpse(3, 30.0, 0.0),
        ];
        let corpses = find_lootable_corpses(&player, &spawns);
        assert_eq!(corpses.len(), 3);
    }

    #[test]
    fn corpse_at_exact_loot_range() {
        let player = make_player(0.0, 0.0);
        let spawns = vec![make_corpse(1, LOOT_RANGE, 0.0)];
        assert!(has_lootable_corpses(&player, &spawns));
        assert_eq!(find_lootable_corpses(&player, &spawns).len(), 1);
    }

    #[test]
    fn corpse_just_outside_loot_range() {
        let player = make_player(0.0, 0.0);
        let spawns = vec![make_corpse(1, LOOT_RANGE + 0.1, 0.0)];
        assert!(!has_lootable_corpses(&player, &spawns));
        assert!(find_lootable_corpses(&player, &spawns).is_empty());
    }

    #[test]
    fn empty_spawn_list() {
        let player = make_player(0.0, 0.0);
        assert!(!has_lootable_corpses(&player, &[]));
        assert!(find_lootable_corpses(&player, &[]).is_empty());
    }

    #[test]
    fn corpse_at_same_position() {
        let player = make_player(50.0, 50.0);
        let spawns = vec![make_corpse(1, 50.0, 50.0)];
        assert!(has_lootable_corpses(&player, &spawns));
    }

    #[test]
    fn mixed_spawn_types_only_corpses() {
        let player = make_player(0.0, 0.0);
        let spawns = vec![
            make_npc(1, 5.0, 0.0),
            make_corpse(2, 5.0, 0.0),
            make_npc(3, 5.0, 0.0),
            make_corpse(4, 5.0, 0.0),
        ];
        let corpses = find_lootable_corpses(&player, &spawns);
        assert_eq!(corpses, vec![2, 4]);
    }
}
