//! Named NPC and boss encounter tracking.
//!
//! This module keeps the first pass intentionally pure: it owns the named
//! encounter database, detects visible named spawns from game-state snapshots,
//! records spawn/death/loot history, and returns the highest-priority named
//! assist target for the combat coordinator.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use textquest_common::types::{ClientId, GameState, SpawnData};

/// Optional configured spawn point for prediction and operator display.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NamedSpawnPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    #[serde(default)]
    pub radius: f32,
}

/// Static configuration for a high-value named NPC or boss.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedEncounterDefinition {
    pub id: String,
    pub display_name: String,
    pub zone: String,
    #[serde(default, rename = "respawn_timer")]
    pub respawn_timer_secs: Option<u64>,
    #[serde(default)]
    pub loot_priority: u8,
    #[serde(default)]
    pub special_strategy: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub spawn_points: Vec<NamedSpawnPoint>,
}

impl NamedEncounterDefinition {
    /// Estimate the next spawn time from a known death timestamp in
    /// milliseconds.
    #[must_use]
    pub fn predict_next_spawn_ms(&self, last_death_at_ms: u64) -> Option<u64> {
        self.respawn_timer_secs
            .map(|seconds| last_death_at_ms.saturating_add(seconds.saturating_mul(1000)))
    }

    fn matches_spawn(&self, spawn_name: &str) -> bool {
        let spawn_key = normalize_key(spawn_name);
        normalize_key(&self.display_name) == spawn_key
            || normalize_key(&self.id) == spawn_key
            || self
                .aliases
                .iter()
                .any(|alias| normalize_key(alias) == spawn_key)
    }
}

/// In-memory named NPC database.
#[derive(Clone, Debug, Default)]
pub struct NamedEncounterDatabase {
    entries: Vec<NamedEncounterDefinition>,
    zone_index: HashMap<String, Vec<usize>>,
}

impl NamedEncounterDatabase {
    #[must_use]
    pub fn new(entries: Vec<NamedEncounterDefinition>) -> Self {
        let mut database = Self {
            entries,
            zone_index: HashMap::new(),
        };
        database.rebuild_index();
        database
    }

    #[must_use]
    pub fn builtin_high_value_targets() -> Self {
        Self::new(vec![
            named(
                "lord_nagafen",
                "Lord Nagafen",
                "soldungb",
                25_200,
                10,
                "named_burn",
            ),
            named(
                "lady_vox",
                "Lady Vox",
                "permafrost",
                25_200,
                10,
                "named_burn",
            ),
            named(
                "venril_sathir",
                "Venril Sathir",
                "karnor",
                25_200,
                9,
                "named_burn",
            ),
            named(
                "trakanon",
                "Trakanon",
                "oldsebilis",
                25_200,
                10,
                "named_burn",
            ),
            named(
                "emperor_ssraeshza",
                "Emperor Ssraeshza",
                "ssratemple",
                3600,
                10,
                "named_burn",
            ),
        ])
    }

    pub fn from_toml_str(input: &str) -> Result<Self, toml::de::Error> {
        let config: NamedDatabaseConfig = toml::from_str(input)?;
        let mut entries = Vec::new();
        for (zone_key, zone_entries) in config.named {
            for (id, entry) in zone_entries {
                entries.push(NamedEncounterDefinition {
                    display_name: entry.name.unwrap_or_else(|| humanize_id(&id)),
                    zone: entry.zone.unwrap_or_else(|| zone_key.clone()),
                    id,
                    respawn_timer_secs: entry.respawn_timer_secs,
                    loot_priority: entry.loot_priority.unwrap_or(1),
                    special_strategy: entry.special_strategy,
                    aliases: entry.aliases,
                    spawn_points: entry.spawn_points,
                });
            }
        }

        Ok(Self::new(entries))
    }

    #[must_use]
    pub fn get_named_list(&self) -> &[NamedEncounterDefinition] {
        &self.entries
    }

    #[must_use]
    pub fn zone_named(&self, zone: &str) -> Vec<&NamedEncounterDefinition> {
        self.zone_index
            .get(&normalize_key(zone))
            .into_iter()
            .flatten()
            .filter_map(|&index| self.entries.get(index))
            .collect()
    }

    #[must_use]
    pub fn match_spawn(&self, zone: &str, spawn: &SpawnData) -> Option<&NamedEncounterDefinition> {
        if spawn.spawn_type != 1 {
            return None;
        }

        self.zone_index
            .get(&normalize_key(zone))?
            .iter()
            .filter_map(|&index| self.entries.get(index))
            .find(|entry| {
                entry.matches_spawn(&spawn.name) || entry.matches_spawn(&spawn.displayed_name)
            })
    }

    fn by_id(&self, id: &str) -> Option<&NamedEncounterDefinition> {
        let id_key = normalize_key(id);
        self.entries
            .iter()
            .find(|entry| normalize_key(&entry.id) == id_key)
    }

    fn rebuild_index(&mut self) {
        self.zone_index.clear();
        for (index, entry) in self.entries.iter().enumerate() {
            self.zone_index
                .entry(normalize_key(&entry.zone))
                .or_default()
                .push(index);
        }
    }
}

/// Combat-priority target surfaced to the coordinator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedAssistTarget {
    pub spawn_id: u32,
    pub name: String,
    pub zone: String,
    pub encounter_id: String,
    pub loot_priority: u8,
    pub special_strategy: Option<String>,
}

/// Type of named encounter history event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NamedEventKind {
    Spawned,
    Died,
    LootDropped,
}

/// Spawn, death, or loot history for a named encounter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedEvent {
    pub kind: NamedEventKind,
    pub encounter_id: String,
    pub spawn_id: Option<u32>,
    pub name: String,
    pub zone: String,
    pub observed_at_ms: u64,
    pub loot_priority: u8,
    pub special_strategy: Option<String>,
    pub loot_item_name: Option<String>,
    pub loot_recipient: Option<String>,
}

/// Result of scanning the current group state for named encounters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NamedScanResult {
    pub alerts: Vec<NamedEvent>,
    pub priority_target: Option<NamedAssistTarget>,
}

/// Tracks visible named spawns and encounter history.
#[derive(Clone, Debug)]
pub struct NamedEncounterTracker {
    database: NamedEncounterDatabase,
    active_spawns: HashMap<u32, String>,
    history: Vec<NamedEvent>,
}

impl Default for NamedEncounterTracker {
    fn default() -> Self {
        Self::new(NamedEncounterDatabase::builtin_high_value_targets())
    }
}

impl NamedEncounterTracker {
    #[must_use]
    pub fn new(database: NamedEncounterDatabase) -> Self {
        Self {
            database,
            active_spawns: HashMap::new(),
            history: Vec::new(),
        }
    }

    pub fn replace_database(&mut self, database: NamedEncounterDatabase) {
        self.database = database;
        self.active_spawns.clear();
        self.history.clear();
    }

    #[must_use]
    pub fn database(&self) -> &NamedEncounterDatabase {
        &self.database
    }

    #[must_use]
    pub fn history(&self) -> &[NamedEvent] {
        &self.history
    }

    pub fn scan_states(&mut self, states: &HashMap<ClientId, GameState>) -> NamedScanResult {
        let mut result = NamedScanResult::default();
        let mut seen_spawn_ids = HashSet::new();

        for state in states.values() {
            let observed_at_ms = state.timestamp_ms;
            if let Some(target) = state.target.as_ref() {
                self.observe_candidate(
                    &state.zone_short_name,
                    target,
                    observed_at_ms,
                    &mut seen_spawn_ids,
                    &mut result,
                );
            }

            for spawn in &state.nearby_spawns {
                self.observe_candidate(
                    &state.zone_short_name,
                    spawn,
                    observed_at_ms,
                    &mut seen_spawn_ids,
                    &mut result,
                );
            }
        }

        result
    }

    pub fn record_death(&mut self, spawn_id: u32, observed_at_ms: u64) -> Option<NamedEvent> {
        let encounter_id = self.active_spawns.remove(&spawn_id)?;
        let definition = self.database.by_id(&encounter_id)?;
        let event = NamedEvent {
            kind: NamedEventKind::Died,
            encounter_id: definition.id.clone(),
            spawn_id: Some(spawn_id),
            name: definition.display_name.clone(),
            zone: definition.zone.clone(),
            observed_at_ms,
            loot_priority: definition.loot_priority,
            special_strategy: definition.special_strategy.clone(),
            loot_item_name: None,
            loot_recipient: None,
        };
        self.history.push(event.clone());
        Some(event)
    }

    pub fn record_loot_drop(
        &mut self,
        encounter_id: &str,
        item_name: &str,
        recipient: Option<&str>,
        observed_at_ms: u64,
    ) -> Option<NamedEvent> {
        let definition = self.database.by_id(encounter_id)?;
        let event = NamedEvent {
            kind: NamedEventKind::LootDropped,
            encounter_id: definition.id.clone(),
            spawn_id: None,
            name: definition.display_name.clone(),
            zone: definition.zone.clone(),
            observed_at_ms,
            loot_priority: definition.loot_priority,
            special_strategy: definition.special_strategy.clone(),
            loot_item_name: Some(item_name.to_string()),
            loot_recipient: recipient.map(str::to_string),
        };
        self.history.push(event.clone());
        Some(event)
    }

    fn observe_candidate(
        &mut self,
        zone: &str,
        spawn: &SpawnData,
        observed_at_ms: u64,
        seen_spawn_ids: &mut HashSet<u32>,
        result: &mut NamedScanResult,
    ) {
        if !seen_spawn_ids.insert(spawn.spawn_id) {
            return;
        }

        let Some(definition) = self.database.match_spawn(zone, spawn) else {
            return;
        };

        let encounter_id = definition.id.clone();
        let loot_priority = definition.loot_priority;
        let special_strategy = definition.special_strategy.clone();

        let target = NamedAssistTarget {
            spawn_id: spawn.spawn_id,
            name: spawn.name.clone(),
            zone: zone.to_string(),
            encounter_id: encounter_id.clone(),
            loot_priority,
            special_strategy: special_strategy.clone(),
        };

        if should_replace_priority_target(result.priority_target.as_ref(), &target) {
            result.priority_target = Some(target);
        }

        if !self.active_spawns.contains_key(&spawn.spawn_id) {
            let event = NamedEvent {
                kind: NamedEventKind::Spawned,
                encounter_id: encounter_id.clone(),
                spawn_id: Some(spawn.spawn_id),
                name: spawn.name.clone(),
                zone: zone.to_string(),
                observed_at_ms,
                loot_priority,
                special_strategy,
                loot_item_name: None,
                loot_recipient: None,
            };
            self.active_spawns.insert(spawn.spawn_id, encounter_id);
            self.history.push(event.clone());
            result.alerts.push(event);
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct NamedDatabaseConfig {
    #[serde(default)]
    named: HashMap<String, HashMap<String, NamedConfigEntry>>,
}

#[derive(Debug, Default, Deserialize)]
struct NamedConfigEntry {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    zone: Option<String>,
    #[serde(default, rename = "respawn_timer")]
    respawn_timer_secs: Option<u64>,
    #[serde(default)]
    loot_priority: Option<u8>,
    #[serde(default)]
    special_strategy: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    spawn_points: Vec<NamedSpawnPoint>,
}

fn named(
    id: &str,
    display_name: &str,
    zone: &str,
    respawn_timer_secs: u64,
    loot_priority: u8,
    special_strategy: &str,
) -> NamedEncounterDefinition {
    NamedEncounterDefinition {
        id: id.to_string(),
        display_name: display_name.to_string(),
        zone: zone.to_string(),
        respawn_timer_secs: Some(respawn_timer_secs),
        loot_priority,
        special_strategy: Some(special_strategy.to_string()),
        aliases: Vec::new(),
        spawn_points: Vec::new(),
    }
}

fn should_replace_priority_target(
    current: Option<&NamedAssistTarget>,
    candidate: &NamedAssistTarget,
) -> bool {
    match current {
        Some(current) => {
            candidate.loot_priority > current.loot_priority
                || (candidate.loot_priority == current.loot_priority
                    && candidate.spawn_id < current.spawn_id)
        }
        None => true,
    }
}

fn humanize_id(id: &str) -> String {
    id.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::{combat::CombatStatus, nav::NavStatus};

    fn spawn(spawn_id: u32, name: &str) -> SpawnData {
        SpawnData {
            spawn_id,
            name: name.to_string(),
            displayed_name: name.to_string(),
            spawn_type: 1,
            level: 60,
            class_id: 1,
            race_id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 1000,
            hp_max: 1000,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 0,
            endurance_max: 0,
            speed_run: 0.0,
            stand_state: 0,
            is_gm: false,
        }
    }

    #[test]
    fn loads_toml_named_database() {
        let db = NamedEncounterDatabase::from_toml_str(
            r#"
            [named.ssratemple.emperor_rallos]
            zone = 'ssratemple'
            respawn_timer = 3600
            loot_priority = 10
            special_strategy = 'named_burn'
            "#,
        )
        .unwrap();

        assert_eq!(db.get_named_list().len(), 1);
        assert!(
            db.match_spawn("ssratemple", &spawn(1, "Emperor Rallos"))
                .is_some()
        );
    }

    #[test]
    fn records_spawn_death_and_loot_history() {
        let db = NamedEncounterDatabase::from_toml_str(
            r#"
            [named.permafrost.lady_vox]
            name = 'Lady Vox'
            respawn_timer = 25200
            loot_priority = 10
            "#,
        )
        .unwrap();
        let mut tracker = NamedEncounterTracker::new(db);
        let mut states = HashMap::new();
        let game_state = GameState {
            client_id: 1,
            local_player: None,
            target: None,
            nearby_spawns: vec![spawn(99, "Lady Vox")],
            timestamp_ms: 1_000,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "permafrost".to_string(),
            zone_long_name: String::new(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
        };
        states.insert(1, game_state);

        let scan = tracker.scan_states(&states);
        assert_eq!(scan.priority_target.unwrap().spawn_id, 99);
        assert_eq!(scan.alerts.len(), 1);
        assert_eq!(
            tracker.record_death(99, 2_000).unwrap().kind,
            NamedEventKind::Died
        );
        assert!(
            tracker
                .record_loot_drop("lady_vox", "White Dragon Scale", Some("Cleric01"), 3_000)
                .is_some()
        );
        assert_eq!(tracker.history().len(), 3);
    }
}
