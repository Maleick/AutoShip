//! Most-injured target finder — MQ2WorstHurt parity.
//!
//! Scans group members and pets to find the character with the lowest HP
//! percentage.  The result integrates with the heal rotation as a targeting
//! helper — the heal coordinator polls this each pulse to pick the
//! highest-priority healing target.
//!
//! Extended-target (XTarget) slot scanning requires a future `GameState`
//! extension to expose the XTarget list; the scope flag is wired up and
//! validated but the scan path is a no-op until that field is available.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use textquest_common::types::{ClientId, GameState};

/// Which member categories to include in the worst-hurt scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanScope {
    /// Include group members (nearby `spawn_type == 0` spawns plus local
    /// player).
    pub group: bool,
    /// Include the local character's pet.
    pub pet: bool,
    /// Include spawns from EQ's extended target list.
    /// Requires GameState.extended_targets — currently a no-op stub.
    pub xtarget: bool,
}

impl Default for ScanScope {
    fn default() -> Self {
        Self {
            group: true,
            pet: true,
            xtarget: false,
        }
    }
}

/// Per-character worst-hurt scanner configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorstHurtConfig {
    pub enabled: bool,
    pub scope: ScanScope,
    /// Skip targets above this HP percent to avoid unnecessary heals on
    /// near-full members.
    pub ignore_above_pct: u8,
}

impl Default for WorstHurtConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scope: ScanScope::default(),
            ignore_above_pct: 95,
        }
    }
}

/// A single scan result representing the most-injured target found.
#[derive(Debug, Clone, PartialEq)]
pub struct HurtTarget {
    pub spawn_id: u32,
    pub name: String,
    /// HP as a percentage in [0.0, 100.0].
    pub hp_pct: f32,
}

/// Scanner that holds per-character configuration and runs on each pulse.
pub struct WorstHurtScanner {
    configs: HashMap<ClientId, WorstHurtConfig>,
}

impl WorstHurtScanner {
    #[must_use]
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
        }
    }

    pub fn set_config(&mut self, client_id: ClientId, config: WorstHurtConfig) {
        self.configs.insert(client_id, config);
    }

    pub fn get_config(&self, client_id: ClientId) -> Option<&WorstHurtConfig> {
        self.configs.get(&client_id)
    }

    pub fn remove_client(&mut self, client_id: ClientId) {
        self.configs.remove(&client_id);
    }

    /// Scan `game_states` from the perspective of `healer_id` and return the
    /// most-injured target within the configured scope, or `None` if all
    /// targets are at or above `ignore_above_pct`.
    ///
    /// Completes within one pulse — O(n) scan over nearby_spawns.
    pub fn scan(
        &self,
        healer_id: ClientId,
        game_states: &HashMap<ClientId, GameState>,
    ) -> Option<HurtTarget> {
        let config = self.configs.get(&healer_id)?;
        if !config.enabled {
            return None;
        }

        let healer_state = game_states.get(&healer_id)?;
        let threshold = config.ignore_above_pct as f32;
        let mut worst: Option<HurtTarget> = None;

        let mut consider = |spawn_id: u32, name: &str, hp_current, hp_max| {
            if hp_max == 0 {
                return;
            }
            let pct = calc_hp_pct(hp_current, hp_max);
            if pct >= threshold {
                return;
            }
            let is_worse = worst.as_ref().map_or(true, |w| pct < w.hp_pct);
            if is_worse {
                worst = Some(HurtTarget {
                    spawn_id,
                    name: name.to_owned(),
                    hp_pct: pct,
                });
            }
        };

        // Local player (healer itself).
        if config.scope.group {
            if let Some(player) = &healer_state.local_player {
                consider(
                    player.spawn_id,
                    &player.name,
                    player.hp_current,
                    player.hp_max,
                );
            }
        }

        // Nearby player spawns (group members visible in same zone).
        if config.scope.group {
            for spawn in &healer_state.nearby_spawns {
                if spawn.spawn_type != 0 {
                    continue; // skip NPCs
                }
                consider(spawn.spawn_id, &spawn.name, spawn.hp_current, spawn.hp_max);
            }
        }

        // Pet.
        if config.scope.pet {
            if let Some(pet) = &healer_state.pet {
                consider(pet.spawn_id, &pet.name, pet.hp_current, pet.hp_max);
            }
        }

        // XTarget slots — stub; wired when GameState exposes xtarget list.
        // if config.scope.xtarget { ... }

        worst
    }
}

impl Default for WorstHurtScanner {
    fn default() -> Self {
        Self::new()
    }
}

fn calc_hp_pct(current: i64, max: i64) -> f32 {
    (current as f32 / max as f32 * 100.0).clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::{combat::CombatStatus, nav::NavStatus, types::SpawnData};

    fn make_spawn(id: u32, name: &str, hp_current: i64, hp_max: i64) -> SpawnData {
        SpawnData {
            spawn_id: id,
            name: name.into(),
            displayed_name: name.into(),
            spawn_type: 0,
            level: 60,
            class_id: 2,
            race_id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current,
            hp_max,
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

    fn make_state(client_id: u32, local: SpawnData, nearby: Vec<SpawnData>) -> GameState {
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
        }
    }

    #[test]
    fn finds_lowest_hp_group_member() {
        let mut scanner = WorstHurtScanner::new();
        scanner.set_config(
            1,
            WorstHurtConfig {
                enabled: true,
                scope: ScanScope {
                    group: true,
                    pet: false,
                    xtarget: false,
                },
                ignore_above_pct: 95,
            },
        );

        let full_health = make_spawn(10, "TankFull", 1000, 1000);
        let half_health = make_spawn(11, "DpsHalf", 500, 1000);
        let low_health = make_spawn(12, "HealerLow", 100, 1000);

        let state = make_state(1, full_health, vec![half_health, low_health]);
        let states: HashMap<ClientId, GameState> = [(1, state)].into_iter().collect();

        let result = scanner.scan(1, &states).expect("should find hurt target");
        assert_eq!(result.spawn_id, 12, "HealerLow (10%) should be worst hurt");
        assert!((result.hp_pct - 10.0).abs() < 0.1);
    }

    #[test]
    fn ignores_full_health_targets() {
        let mut scanner = WorstHurtScanner::new();
        scanner.set_config(1, WorstHurtConfig::default());

        let full = make_spawn(10, "FullPlayer", 1000, 1000);
        let state = make_state(1, full, vec![]);
        let states: HashMap<ClientId, GameState> = [(1, state)].into_iter().collect();

        // 100% >= ignore_above_pct (95) → no result
        assert!(scanner.scan(1, &states).is_none());
    }

    #[test]
    fn disabled_config_returns_none() {
        let mut scanner = WorstHurtScanner::new();
        scanner.set_config(
            1,
            WorstHurtConfig {
                enabled: false,
                ..WorstHurtConfig::default()
            },
        );
        let spawn = make_spawn(10, "Player", 1, 1000);
        let state = make_state(1, spawn, vec![]);
        let states = [(1, state)].into_iter().collect();
        assert!(scanner.scan(1, &states).is_none());
    }

    #[test]
    fn includes_pet_when_scope_enabled() {
        let mut scanner = WorstHurtScanner::new();
        scanner.set_config(
            1,
            WorstHurtConfig {
                enabled: true,
                scope: ScanScope {
                    group: false,
                    pet: true,
                    xtarget: false,
                },
                ignore_above_pct: 95,
            },
        );

        let local = make_spawn(1, "Mage", 1000, 1000);
        let pet_spawn = make_spawn(99, "PetWeak", 100, 1000);

        let mut state = make_state(1, local, vec![]);
        state.pet = Some(pet_spawn);

        let states = [(1, state)].into_iter().collect();
        let result = scanner.scan(1, &states).expect("pet should be found");
        assert_eq!(result.spawn_id, 99);
    }

    #[test]
    fn skips_npcs_in_group_scan() {
        let mut scanner = WorstHurtScanner::new();
        scanner.set_config(1, WorstHurtConfig::default());

        let local = make_spawn(1, "Healer", 1000, 1000);
        let mut npc = make_spawn(50, "a_goblin", 1, 1000);
        npc.spawn_type = 1; // NPC

        let state = make_state(1, local, vec![npc]);
        let states = [(1, state)].into_iter().collect();
        // NPC should be excluded from group scan
        assert!(scanner.scan(1, &states).is_none());
    }

    #[test]
    fn no_config_returns_none() {
        let scanner = WorstHurtScanner::new();
        let spawn = make_spawn(1, "Player", 1, 1000);
        let state = make_state(1, spawn, vec![]);
        let states = [(1, state)].into_iter().collect();
        assert!(scanner.scan(1, &states).is_none());
    }
}
