use std::collections::HashMap;

use super::named_db::{NamedMobDatabase, NamedPriority};
use super::structs::{SpawnInfo, SpawnType};

/// Status of a tracked named spawn.
#[derive(Debug, Clone)]
pub struct NamedSpawnStatus {
    pub name: String,
    pub spawn_id: u32,
    pub zone: String,
    pub first_seen_tick: u64,
    pub is_alive: bool,
    pub death_tick: Option<u64>,
    pub estimated_respawn_tick: Option<u64>,
    /// End of the respawn window (max estimate). None if no database entry.
    pub respawn_window_end_tick: Option<u64>,
    /// Priority from the named mob database.
    pub priority: Option<NamedPriority>,
    /// Last known position for map rendering of dead named spawns.
    pub last_x: f32,
    pub last_y: f32,
    pub last_z: f32,
}

/// Alert events emitted by the tracker.
#[derive(Debug, Clone, PartialEq)]
pub enum NamedAlert {
    SpawnUp {
        name: String,
        zone: String,
    },
    SpawnDown {
        name: String,
        zone: String,
        respawn_estimate: u64,
    },
}

/// Default respawn estimate in ticks (1200 ticks ~= 5 min at 250ms tick rate;
/// real EQ named respawn is ~20 min but we use a shorter value for testing).
const DEFAULT_RESPAWN_TICKS: u64 = 1200;

/// Tracks named NPC spawns across ticks.
pub struct NamedTracker {
    /// Tracked named spawns keyed by lowercase name.
    tracked: HashMap<String, NamedSpawnStatus>,
    /// Current zone (used for alert context).
    zone: String,
    /// Named mob database for respawn estimates and priority.
    db: Option<NamedMobDatabase>,
}

impl NamedTracker {
    pub fn new() -> Self {
        Self {
            tracked: HashMap::new(),
            zone: String::new(),
            db: None,
        }
    }

    /// Create a tracker with a named mob database for respawn estimates.
    pub fn with_db(db: NamedMobDatabase) -> Self {
        Self {
            tracked: HashMap::new(),
            zone: String::new(),
            db: Some(db),
        }
    }

    /// Set the named mob database (can be called after construction).
    pub fn set_db(&mut self, db: NamedMobDatabase) {
        self.db = Some(db);
    }

    /// Set the current zone name (call when zone changes).
    pub fn set_zone(&mut self, zone: &str) {
        if self.zone != zone {
            // Zone changed — clear tracking state
            self.tracked.clear();
            self.zone = zone.to_string();
        }
    }

    /// Update tracking state from the current spawn list.
    /// Returns alerts for new up/down events this tick.
    pub fn update(&mut self, current_spawns: &[SpawnInfo], tick: u64) -> Vec<NamedAlert> {
        let mut alerts = Vec::new();

        // Build set of currently alive named NPCs
        let mut alive_names: HashMap<String, &SpawnInfo> = HashMap::new();
        for spawn in current_spawns {
            if spawn.spawn_type == SpawnType::Npc && is_named(&spawn.displayed_name) {
                alive_names.insert(spawn.displayed_name.to_lowercase(), spawn);
            }
        }

        // Check for new spawns
        for (key, spawn) in &alive_names {
            if let Some(existing) = self.tracked.get_mut(key) {
                if !existing.is_alive {
                    // Respawned!
                    existing.is_alive = true;
                    existing.spawn_id = spawn.spawn_id;
                    existing.death_tick = None;
                    existing.estimated_respawn_tick = None;
                    existing.respawn_window_end_tick = None;
                    existing.last_x = spawn.x;
                    existing.last_y = spawn.y;
                    existing.last_z = spawn.z;
                    alerts.push(NamedAlert::SpawnUp {
                        name: spawn.displayed_name.clone(),
                        zone: self.zone.clone(),
                    });
                } else {
                    // Still alive — update position
                    existing.last_x = spawn.x;
                    existing.last_y = spawn.y;
                    existing.last_z = spawn.z;
                }
            } else {
                // Brand new named spawn — look up priority from database
                let priority = self
                    .db
                    .as_ref()
                    .and_then(|db| db.get(&self.zone, &spawn.displayed_name))
                    .map(|entry| entry.priority);
                self.tracked.insert(
                    key.clone(),
                    NamedSpawnStatus {
                        name: spawn.displayed_name.clone(),
                        spawn_id: spawn.spawn_id,
                        zone: self.zone.clone(),
                        first_seen_tick: tick,
                        is_alive: true,
                        death_tick: None,
                        estimated_respawn_tick: None,
                        respawn_window_end_tick: None,
                        priority,
                        last_x: spawn.x,
                        last_y: spawn.y,
                        last_z: spawn.z,
                    },
                );
                alerts.push(NamedAlert::SpawnUp {
                    name: spawn.displayed_name.clone(),
                    zone: self.zone.clone(),
                });
            }
        }

        // Check for despawned named mobs (were alive, no longer in spawn list)
        for (key, status) in self.tracked.iter_mut() {
            if status.is_alive && !alive_names.contains_key(key) {
                status.is_alive = false;
                status.death_tick = Some(tick);

                // Use database respawn times if available, else default
                let (min_ticks, max_ticks) = self
                    .db
                    .as_ref()
                    .and_then(|db| db.get(&self.zone, &status.name))
                    .map(|entry| (entry.respawn_min_ticks(), entry.respawn_max_ticks()))
                    .unwrap_or((DEFAULT_RESPAWN_TICKS, DEFAULT_RESPAWN_TICKS));

                status.estimated_respawn_tick = Some(tick + min_ticks);
                status.respawn_window_end_tick = Some(tick + max_ticks);

                alerts.push(NamedAlert::SpawnDown {
                    name: status.name.clone(),
                    zone: self.zone.clone(),
                    respawn_estimate: tick + min_ticks,
                });
            }
        }

        alerts
    }

    /// Get all tracked named spawns (alive and dead).
    pub fn tracked_spawns(&self) -> Vec<&NamedSpawnStatus> {
        let mut result: Vec<&NamedSpawnStatus> = self.tracked.values().collect();
        // Alive first, then dead sorted by estimated respawn
        result.sort_by(|a, b| {
            b.is_alive
                .cmp(&a.is_alive)
                .then_with(|| a.estimated_respawn_tick.cmp(&b.estimated_respawn_tick))
                .then_with(|| a.name.cmp(&b.name))
        });
        result
    }

    /// Returns the highest-priority alive named mob, if any.
    /// Used by the camp loop to override normal pull targets.
    pub fn priority_target(&self) -> Option<&NamedSpawnStatus> {
        self.tracked
            .values()
            .filter(|s| s.is_alive)
            .filter(|s| s.priority.is_some())
            .min_by_key(|s| match s.priority {
                Some(NamedPriority::High) => 0,
                Some(NamedPriority::Medium) => 1,
                Some(NamedPriority::Low) => 2,
                None => 3,
            })
    }

    /// Check if a respawn window is currently active for any tracked named mob.
    /// Returns named mobs whose respawn window has opened (past min estimate).
    pub fn in_respawn_window(&self, current_tick: u64) -> Vec<&NamedSpawnStatus> {
        self.tracked
            .values()
            .filter(|s| !s.is_alive)
            .filter(|s| {
                s.estimated_respawn_tick
                    .is_some_and(|min| current_tick >= min)
                    && s.respawn_window_end_tick
                        .is_some_and(|max| current_tick <= max)
            })
            .collect()
    }

    /// Number of tracked named spawns.
    pub fn len(&self) -> usize {
        self.tracked.len()
    }

    /// Whether no named spawns are tracked.
    pub fn is_empty(&self) -> bool {
        self.tracked.is_empty()
    }
}

/// Returns true if the spawn name looks like a named mob (not a generic mob).
/// Generic mobs start with articles: "a ", "an ", "the " (case-insensitive).
pub fn is_named(name: &str) -> bool {
    let lower = name.to_lowercase();
    !lower.starts_with("a ")
        && !lower.starts_with("an ")
        && !lower.starts_with("the ")
        && !lower.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eq::structs::{EqClass, SpawnType, StandState};

    fn make_npc(name: &str, id: u32) -> SpawnInfo {
        SpawnInfo {
            name: name.to_string(),
            displayed_name: name.to_string(),
            lastname: String::new(),
            spawn_id: id,
            spawn_type: SpawnType::Npc,
            level: 50,
            class_id: 1,
            class: Some(EqClass::Warrior),
            stand_state: StandState::Standing,
            x: 100.0,
            y: 200.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 10000,
            hp_max: 10000,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 0,
            endurance_max: 0,
            is_gm: false,
        }
    }

    #[test]
    fn test_is_named() {
        assert!(is_named("Emperor Crush"));
        assert!(is_named("Lord Nagafen"));
        assert!(is_named("Phinigel Autropos"));
        assert!(!is_named("a moss snake"));
        assert!(!is_named("an orc pawn"));
        assert!(!is_named("the Tangrin")); // "the" prefix = generic
        assert!(!is_named(""));
    }

    #[test]
    fn test_new_spawn_generates_alert() {
        let mut tracker = NamedTracker::new();
        tracker.set_zone("crushbone");

        let spawns = vec![
            make_npc("Emperor Crush", 1001),
            make_npc("a legionnaire", 1002),
        ];

        let alerts = tracker.update(&spawns, 1);
        assert_eq!(alerts.len(), 1);
        assert_eq!(
            alerts[0],
            NamedAlert::SpawnUp {
                name: "Emperor Crush".into(),
                zone: "crushbone".into(),
            }
        );
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn test_despawn_generates_down_alert() {
        let mut tracker = NamedTracker::new();
        tracker.set_zone("crushbone");

        // Tick 1: named mob appears
        let spawns = vec![make_npc("Emperor Crush", 1001)];
        tracker.update(&spawns, 1);

        // Tick 2: named mob gone
        let alerts = tracker.update(&[], 100);
        assert_eq!(alerts.len(), 1);
        match &alerts[0] {
            NamedAlert::SpawnDown {
                name,
                zone,
                respawn_estimate,
            } => {
                assert_eq!(name, "Emperor Crush");
                assert_eq!(zone, "crushbone");
                assert_eq!(*respawn_estimate, 100 + 1200);
            }
            _ => panic!("Expected SpawnDown"),
        }

        // Check tracked state
        let tracked = tracker.tracked_spawns();
        assert_eq!(tracked.len(), 1);
        assert!(!tracked[0].is_alive);
    }

    #[test]
    fn test_respawn_generates_up_alert() {
        let mut tracker = NamedTracker::new();
        tracker.set_zone("crushbone");

        // Appear
        let spawns = vec![make_npc("Emperor Crush", 1001)];
        tracker.update(&spawns, 1);

        // Die
        tracker.update(&[], 100);

        // Respawn
        let spawns = vec![make_npc("Emperor Crush", 2001)];
        let alerts = tracker.update(&spawns, 1400);
        assert_eq!(alerts.len(), 1);
        assert_eq!(
            alerts[0],
            NamedAlert::SpawnUp {
                name: "Emperor Crush".into(),
                zone: "crushbone".into(),
            }
        );

        let tracked = tracker.tracked_spawns();
        assert!(tracked[0].is_alive);
        assert_eq!(tracked[0].spawn_id, 2001);
    }

    #[test]
    fn test_still_alive_no_alert() {
        let mut tracker = NamedTracker::new();
        tracker.set_zone("crushbone");

        let spawns = vec![make_npc("Emperor Crush", 1001)];
        tracker.update(&spawns, 1);

        // Same mob still alive next tick — no alerts
        let alerts = tracker.update(&spawns, 2);
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_zone_change_clears_tracking() {
        let mut tracker = NamedTracker::new();
        tracker.set_zone("crushbone");

        let spawns = vec![make_npc("Emperor Crush", 1001)];
        tracker.update(&spawns, 1);
        assert_eq!(tracker.len(), 1);

        tracker.set_zone("freportw");
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn test_tracked_spawns_sort_order() {
        let mut tracker = NamedTracker::new();
        tracker.set_zone("lowerguk");

        let spawns = vec![
            make_npc("Frenzied Ghoul", 100),
            make_npc("King Crush", 101),
        ];
        tracker.update(&spawns, 1);

        // Kill Frenzied Ghoul
        let spawns = vec![make_npc("King Crush", 101)];
        tracker.update(&spawns, 50);

        let tracked = tracker.tracked_spawns();
        assert_eq!(tracked.len(), 2);
        // Alive first
        assert!(tracked[0].is_alive);
        assert_eq!(tracked[0].name, "King Crush");
        // Dead second
        assert!(!tracked[1].is_alive);
        assert_eq!(tracked[1].name, "Frenzied Ghoul");
    }

    #[test]
    fn test_generic_mobs_ignored() {
        let mut tracker = NamedTracker::new();
        tracker.set_zone("crushbone");

        let spawns = vec![
            make_npc("a legionnaire", 1),
            make_npc("an orc centurion", 2),
        ];

        let alerts = tracker.update(&spawns, 1);
        assert!(alerts.is_empty());
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn test_db_respawn_times_used() {
        use crate::eq::named_db::NamedMobDatabase;
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let toml = r#"
zone = "crushbone"

[[named]]
name = "Emperor Crush"
level = 15
respawn_min_minutes = 28
respawn_max_minutes = 32
location = [-688.0, 118.0, 28.0]
drops = ["Crushbone Belt"]
priority = "high"
"#;
        let mut f = std::fs::File::create(dir.path().join("crushbone.toml")).unwrap();
        f.write_all(toml.as_bytes()).unwrap();

        let db = NamedMobDatabase::load(dir.path()).unwrap();
        let mut tracker = NamedTracker::with_db(db);
        tracker.set_zone("crushbone");

        // Spawn and kill
        let spawns = vec![make_npc("Emperor Crush", 1001)];
        tracker.update(&spawns, 1);
        tracker.update(&[], 100);

        let tracked = tracker.tracked_spawns();
        assert_eq!(tracked.len(), 1);
        assert!(!tracked[0].is_alive);
        // 28 min * 60 * 4 = 6720 ticks
        assert_eq!(tracked[0].estimated_respawn_tick, Some(100 + 6720));
        // 32 min * 60 * 4 = 7680 ticks
        assert_eq!(tracked[0].respawn_window_end_tick, Some(100 + 7680));
        assert_eq!(tracked[0].priority, Some(NamedPriority::High));
    }

    #[test]
    fn test_priority_target() {
        use crate::eq::named_db::NamedMobDatabase;
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let toml = r#"
zone = "crushbone"

[[named]]
name = "Emperor Crush"
level = 15
respawn_min_minutes = 28
respawn_max_minutes = 32
location = [-688.0, 118.0, 28.0]
drops = ["Crushbone Belt"]
priority = "high"

[[named]]
name = "Lord Darish"
level = 12
respawn_min_minutes = 16
respawn_max_minutes = 22
location = [-340.0, 370.0, 28.0]
drops = []
priority = "low"
"#;
        let mut f = std::fs::File::create(dir.path().join("crushbone.toml")).unwrap();
        f.write_all(toml.as_bytes()).unwrap();

        let db = NamedMobDatabase::load(dir.path()).unwrap();
        let mut tracker = NamedTracker::with_db(db);
        tracker.set_zone("crushbone");

        let spawns = vec![
            make_npc("Emperor Crush", 1001),
            make_npc("Lord Darish", 1002),
        ];
        tracker.update(&spawns, 1);

        let target = tracker.priority_target().unwrap();
        assert_eq!(target.name, "Emperor Crush");
    }

    #[test]
    fn test_in_respawn_window() {
        use crate::eq::named_db::NamedMobDatabase;
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let toml = r#"
zone = "crushbone"

[[named]]
name = "Emperor Crush"
level = 15
respawn_min_minutes = 1
respawn_max_minutes = 2
location = [-688.0, 118.0, 28.0]
drops = []
priority = "high"
"#;
        let mut f = std::fs::File::create(dir.path().join("crushbone.toml")).unwrap();
        f.write_all(toml.as_bytes()).unwrap();

        let db = NamedMobDatabase::load(dir.path()).unwrap();
        let mut tracker = NamedTracker::with_db(db);
        tracker.set_zone("crushbone");

        let spawns = vec![make_npc("Emperor Crush", 1001)];
        tracker.update(&spawns, 0);
        tracker.update(&[], 100); // Kill at tick 100

        // min = 1 min * 60 * 4 = 240 ticks -> respawn at 340
        // max = 2 min * 60 * 4 = 480 ticks -> window end at 580

        // Before window: tick 300 (before 340)
        assert!(tracker.in_respawn_window(300).is_empty());

        // In window: tick 400 (between 340 and 580)
        let in_window = tracker.in_respawn_window(400);
        assert_eq!(in_window.len(), 1);
        assert_eq!(in_window[0].name, "Emperor Crush");

        // After window: tick 600 (after 580)
        assert!(tracker.in_respawn_window(600).is_empty());
    }
}
