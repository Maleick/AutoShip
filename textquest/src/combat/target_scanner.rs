//! MA (Main Assist) target scanner with safe-targeting predicate.
//!
//! Scans nearby NPC spawns for viable combat candidates, applying the
//! `safe_targeting` predicate to exclude mobs whose current combat target is
//! not a member of the local group or raid.  This prevents the bot from
//! assisting on mobs that are already training a different group.
//!
//! # Safe Targeting
//!
//! The predicate [`is_safe_target`] checks `SpawnData::combat_target_id`:
//! - `None` → mob has no target; safe to engage (pulls are allowed).
//! - `Some(id)` where `id` is in the group → safe (it's already on us).
//! - `Some(id)` where `id` is NOT in the group → unsafe; skip to avoid training.
//!
//! [`TargetScanner::scan`] respects [`TargetScanConfig::safe_targeting`]: when
//! the flag is `false` the predicate is bypassed entirely.

use std::collections::HashSet;

use textquest_common::{
    combat::TargetScanConfig,
    types::{ClientId, GameState, SpawnData},
};

// ── GroupMembers ─────────────────────────────────────────────────────────────

/// Set of spawn IDs belonging to the local group or raid.
///
/// Build one from the local `GameState` (local player + nearby player spawns)
/// before calling [`TargetScanner::scan`].
#[derive(Debug, Clone, Default)]
pub struct GroupMembers {
    spawn_ids: HashSet<u32>,
}

impl GroupMembers {
    /// Construct from an explicit list of spawn IDs (e.g. from an XTarget list
    /// or a pre-computed group manifest).
    #[must_use]
    pub fn from_ids(ids: impl IntoIterator<Item = u32>) -> Self {
        Self {
            spawn_ids: ids.into_iter().collect(),
        }
    }

    /// Build from a live `GameState` — includes the local player and every
    /// nearby spawn whose `spawn_type == 0` (player).
    #[must_use]
    pub fn from_game_state(state: &GameState) -> Self {
        let mut ids = HashSet::new();
        if let Some(player) = &state.local_player {
            ids.insert(player.spawn_id);
        }
        for spawn in &state.nearby_spawns {
            if spawn.spawn_type == 0 {
                ids.insert(spawn.spawn_id);
            }
        }
        Self { spawn_ids: ids }
    }

    /// Returns `true` if `spawn_id` is a known group/raid member.
    #[must_use]
    pub fn contains(&self, spawn_id: u32) -> bool {
        self.spawn_ids.contains(&spawn_id)
    }

    /// Number of members tracked.
    #[must_use]
    pub fn len(&self) -> usize {
        self.spawn_ids.len()
    }

    /// Returns `true` if the group has no members.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spawn_ids.is_empty()
    }
}

// ── Safe-targeting predicate ──────────────────────────────────────────────────

/// Returns `true` if `mob` is safe to target given the local `group`.
///
/// A mob is considered **safe** when:
/// - It has no recorded `combat_target_id` (untargeted — may be freely engaged),
///   **or**
/// - Its `combat_target_id` resolves to a spawn ID that belongs to `group`.
///
/// A mob is considered **unsafe** (returns `false`) when its
/// `combat_target_id` is set and the target is **not** in `group` — meaning
/// the mob is actively fighting someone outside the group, so engaging it
/// would train them.
#[must_use]
pub fn is_safe_target(mob: &SpawnData, group: &GroupMembers) -> bool {
    match mob.combat_target_id {
        None => true,
        Some(target_id) => group.contains(target_id),
    }
}

// ── TargetScanner ────────────────────────────────────────────────────────────

/// A candidate mob returned by [`TargetScanner::scan`].
#[derive(Debug, Clone, PartialEq)]
pub struct TargetCandidate {
    /// Spawn ID of the mob.
    pub spawn_id: u32,
    /// Display name of the mob.
    pub name: String,
    /// HP as a percentage in [0.0, 100.0].
    pub hp_pct: f32,
    /// 2-D (XY) distance from the local player to this mob.
    pub distance: f32,
}

/// Scans nearby NPC spawns and returns viable MA candidates.
///
/// Wire this into the combat coordinator tick — call [`scan`][Self::scan]
/// each pulse from the MA's `GameState`, then pick the first (highest-priority)
/// result.
#[derive(Debug, Default)]
pub struct TargetScanner {
    config: TargetScanConfig,
}

impl TargetScanner {
    /// Create a scanner with the default [`TargetScanConfig`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scanner with a custom config.
    #[must_use]
    pub fn with_config(config: TargetScanConfig) -> Self {
        Self { config }
    }

    /// Return the current configuration.
    #[must_use]
    pub fn config(&self) -> &TargetScanConfig {
        &self.config
    }

    /// Update the configuration at runtime.
    pub fn set_config(&mut self, config: TargetScanConfig) {
        self.config = config;
    }

    /// Scan `state` for viable MA target candidates.
    ///
    /// Filtering pipeline (each filter may discard a spawn):
    /// 1. Must be an NPC (`spawn_type != 0`).
    /// 2. Must not be a corpse (`stand_state != 111`).
    /// 3. Must have HP > 0.
    /// 4. Must be within `scan_radius` (XY) and `scan_z_radius` (Z).
    /// 5. If `skip_mezzed`: skip mobs in frozen state (`stand_state == 1`).
    /// 6. If `safe_targeting`: skip mobs whose `combat_target_id` is set and
    ///    not in `group`.
    ///
    /// The returned list is not yet sorted — callers should apply HP/distance
    /// preference from [`TargetScanConfig`] after receiving results.
    #[must_use]
    pub fn scan(&self, state: &GameState, group: &GroupMembers) -> Vec<TargetCandidate> {
        let local = match &state.local_player {
            Some(p) => p,
            None => return Vec::new(),
        };

        let mut candidates = Vec::new();

        for spawn in &state.nearby_spawns {
            // 1. NPCs only.
            if spawn.spawn_type == 0 {
                continue;
            }

            // 2. Skip corpses.
            if spawn.stand_state == 111 {
                continue;
            }

            // 3. Must be alive.
            if spawn.hp_current <= 0 || spawn.hp_max <= 0 {
                continue;
            }

            // 4. Distance gate.
            let dx = spawn.x - local.x;
            let dy = spawn.y - local.y;
            let dz = (spawn.z - local.z).abs();
            let dist_2d = (dx * dx + dy * dy).sqrt();

            if dist_2d > self.config.scan_radius {
                continue;
            }
            if dz > self.config.scan_z_radius {
                continue;
            }

            // 5. Skip mezzed (frozen).
            if self.config.skip_mezzed && spawn.stand_state == 1 {
                continue;
            }

            // 6. Safe-targeting predicate.
            if self.config.safe_targeting && !is_safe_target(spawn, group) {
                continue;
            }

            let hp_pct =
                (spawn.hp_current as f32 / spawn.hp_max as f32 * 100.0).clamp(0.0, 100.0);

            candidates.push(TargetCandidate {
                spawn_id: spawn.spawn_id,
                name: spawn.displayed_name.clone(),
                hp_pct,
                distance: dist_2d,
            });
        }

        candidates
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::{combat::CombatStatus, nav::NavStatus, types::SpawnData};

    fn make_player(spawn_id: u32, name: &str) -> SpawnData {
        SpawnData {
            spawn_id,
            name: name.into(),
            displayed_name: name.into(),
            spawn_type: 0, // player
            level: 60,
            class_id: 1,
            race_id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 1000,
            hp_max: 1000,
            mana_current: 500,
            mana_max: 500,
            endurance_current: 100,
            endurance_max: 100,
            speed_run: 0.0,
            stand_state: 0,
            is_gm: false,
            combat_target_id: None,
        }
    }

    fn make_mob(spawn_id: u32, name: &str, combat_target_id: Option<u32>) -> SpawnData {
        SpawnData {
            spawn_id,
            name: name.into(),
            displayed_name: name.into(),
            spawn_type: 1, // NPC
            level: 50,
            class_id: 0,
            race_id: 2,
            x: 10.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 800,
            hp_max: 1000,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 0,
            endurance_max: 0,
            speed_run: 0.0,
            stand_state: 0,
            is_gm: false,
            combat_target_id,
        }
    }

    fn make_state(
        client_id: ClientId,
        local: SpawnData,
        nearby: Vec<SpawnData>,
    ) -> GameState {
        GameState {
            client_id,
            local_player: Some(local),
            target: None,
            nearby_spawns: nearby,
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "commonlands".into(),
            zone_long_name: "The Commonlands".into(),
            active_buffs: vec![],
            pet: None,
            actual_version: None,
            is_zone_changing: false,
        }
    }

    // ── GroupMembers ──────────────────────────────────────────────────────────

    #[test]
    fn group_members_from_ids() {
        let group = GroupMembers::from_ids([1, 2, 3]);
        assert!(group.contains(1));
        assert!(group.contains(3));
        assert!(!group.contains(99));
        assert_eq!(group.len(), 3);
    }

    #[test]
    fn group_members_from_game_state_includes_local_and_players() {
        let local = make_player(1, "Tank");
        let party_member = make_player(2, "Healer");
        let mut npc = make_mob(50, "a_goblin", None);
        npc.spawn_type = 1;

        let state = make_state(1, local, vec![party_member, npc]);
        let group = GroupMembers::from_game_state(&state);

        assert!(group.contains(1), "local player should be in group");
        assert!(group.contains(2), "nearby player should be in group");
        assert!(!group.contains(50), "NPC should not be in group");
        assert_eq!(group.len(), 2);
    }

    // ── is_safe_target ────────────────────────────────────────────────────────

    #[test]
    fn mob_with_no_target_is_safe() {
        let group = GroupMembers::from_ids([1, 2]);
        let mob = make_mob(100, "untargeted_mob", None);
        assert!(
            is_safe_target(&mob, &group),
            "mob with no target should be safe"
        );
    }

    #[test]
    fn mob_fighting_group_member_is_safe() {
        // Mob is fighting spawn_id 1 which is in our group.
        let group = GroupMembers::from_ids([1, 2]);
        let mob = make_mob(100, "on_tank", Some(1));
        assert!(
            is_safe_target(&mob, &group),
            "mob targeting our tank should be safe"
        );
    }

    #[test]
    fn mob_fighting_stranger_is_unsafe() {
        // Mob is fighting spawn_id 99 which is NOT in our group.
        let group = GroupMembers::from_ids([1, 2]);
        let mob = make_mob(100, "training_mob", Some(99));
        assert!(
            !is_safe_target(&mob, &group),
            "mob targeting a stranger should NOT be safe"
        );
    }

    // ── TargetScanner::scan ───────────────────────────────────────────────────

    #[test]
    fn scan_excludes_mob_fighting_stranger() {
        let scanner = TargetScanner::with_config(TargetScanConfig {
            safe_targeting: true,
            ..TargetScanConfig::default()
        });

        let local = make_player(1, "MyChar");
        // Mob is fighting spawn 99 — a stranger not in our group.
        let mob = make_mob(200, "training_mob", Some(99));
        let group = GroupMembers::from_ids([1]);

        let state = make_state(1, local, vec![mob]);
        let candidates = scanner.scan(&state, &group);
        assert!(
            candidates.is_empty(),
            "mob fighting a stranger should be excluded from candidate list"
        );
    }

    #[test]
    fn scan_includes_mob_fighting_group_member() {
        let scanner = TargetScanner::with_config(TargetScanConfig {
            safe_targeting: true,
            ..TargetScanConfig::default()
        });

        let local = make_player(1, "MyChar");
        // Mob is fighting spawn 1 — our local character (tank).
        let mob = make_mob(200, "on_our_tank", Some(1));
        let group = GroupMembers::from_ids([1]);

        let state = make_state(1, local, vec![mob]);
        let candidates = scanner.scan(&state, &group);
        assert_eq!(
            candidates.len(),
            1,
            "mob fighting our group member should be included"
        );
        assert_eq!(candidates[0].spawn_id, 200);
    }

    #[test]
    fn scan_includes_untargeted_mob() {
        let scanner = TargetScanner::with_config(TargetScanConfig {
            safe_targeting: true,
            ..TargetScanConfig::default()
        });

        let local = make_player(1, "MyChar");
        let mob = make_mob(300, "fresh_mob", None);
        let group = GroupMembers::from_ids([1]);

        let state = make_state(1, local, vec![mob]);
        let candidates = scanner.scan(&state, &group);
        assert_eq!(candidates.len(), 1, "untargeted mob should be included");
    }

    #[test]
    fn scan_safe_targeting_false_includes_all_mobs() {
        let scanner = TargetScanner::with_config(TargetScanConfig {
            safe_targeting: false,
            ..TargetScanConfig::default()
        });

        let local = make_player(1, "MyChar");
        // Mob fighting a stranger — would be excluded if safe_targeting=true.
        let mob = make_mob(200, "training_mob", Some(99));
        let group = GroupMembers::from_ids([1]);

        let state = make_state(1, local, vec![mob]);
        let candidates = scanner.scan(&state, &group);
        assert_eq!(
            candidates.len(),
            1,
            "with safe_targeting=false, all mobs should be included"
        );
    }

    #[test]
    fn scan_skips_players() {
        let scanner = TargetScanner::new();
        let local = make_player(1, "MyChar");
        let other_player = make_player(2, "OtherPlayer");
        let group = GroupMembers::from_ids([1, 2]);

        let state = make_state(1, local, vec![other_player]);
        let candidates = scanner.scan(&state, &group);
        assert!(candidates.is_empty(), "players should not be scanned as targets");
    }

    #[test]
    fn scan_skips_out_of_range_mobs() {
        let scanner = TargetScanner::with_config(TargetScanConfig {
            scan_radius: 50.0,
            ..TargetScanConfig::default()
        });

        let local = make_player(1, "MyChar");
        let mut mob = make_mob(200, "far_mob", None);
        mob.x = 200.0; // 200 units away — beyond 50.0 radius.

        let group = GroupMembers::from_ids([1]);
        let state = make_state(1, local, vec![mob]);
        let candidates = scanner.scan(&state, &group);
        assert!(candidates.is_empty(), "mob beyond scan_radius should be excluded");
    }

    #[test]
    fn scan_skips_dead_mobs() {
        let scanner = TargetScanner::new();
        let local = make_player(1, "MyChar");
        let mut mob = make_mob(200, "dead_mob", None);
        mob.stand_state = 111; // dead

        let group = GroupMembers::from_ids([1]);
        let state = make_state(1, local, vec![mob]);
        let candidates = scanner.scan(&state, &group);
        assert!(candidates.is_empty(), "dead mobs should be excluded");
    }

    #[test]
    fn scan_skips_mezzed_mobs_when_configured() {
        let scanner = TargetScanner::with_config(TargetScanConfig {
            skip_mezzed: true,
            ..TargetScanConfig::default()
        });

        let local = make_player(1, "MyChar");
        let mut mob = make_mob(200, "mezzed_mob", None);
        mob.stand_state = 1; // frozen/mezzed

        let group = GroupMembers::from_ids([1]);
        let state = make_state(1, local, vec![mob]);
        let candidates = scanner.scan(&state, &group);
        assert!(candidates.is_empty(), "mezzed mob should be excluded when skip_mezzed=true");
    }
}
