//! Advanced spawn filtering, sorting, and named-mob tracking.
//!
//! Provides [`SpawnFilter`] for multi-criteria spawn queries,
//! [`SpawnSorter`] for in-place ordering, and [`NamedSpawnTracker`]
//! for lightweight named-mob up/down state tracking — all
//! platform-independent pure logic with no Windows APIs.

use std::collections::HashMap;

use super::structs::{SpawnInfo, SpawnType};

// ---------------------------------------------------------------------------
// SpawnTypeFilter
// ---------------------------------------------------------------------------

/// Spawn type selector used by [`SpawnFilter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpawnTypeFilter {
    /// Accept any spawn type.
    #[default]
    All,
    /// Non-player characters only.
    Npc,
    /// Player characters only.
    Pc,
    /// Pet spawns — detected heuristically as NPCs whose name ends with "`'s
    /// pet`" or "`'s familiar`" (case-insensitive).
    Pet,
}

impl SpawnTypeFilter {
    fn matches(self, spawn: &SpawnInfo) -> bool {
        match self {
            Self::All => true,
            Self::Npc => spawn.spawn_type == SpawnType::Npc && !is_pet(spawn),
            Self::Pc => spawn.spawn_type == SpawnType::Player,
            Self::Pet => is_pet(spawn),
        }
    }
}

fn is_pet(spawn: &SpawnInfo) -> bool {
    let lower = spawn.displayed_name.to_lowercase();
    lower.ends_with("'s pet") || lower.ends_with("'s familiar") || lower.ends_with("'s warder")
}

// ---------------------------------------------------------------------------
// SpawnFilter
// ---------------------------------------------------------------------------

/// Multi-criteria spawn filter.
///
/// All set criteria must match (logical AND). Unset fields are wildcards.
///
/// # Example
/// ```rust
/// use textquest::eq::spawn_filter::{SpawnFilter, SpawnTypeFilter};
///
/// let filter = SpawnFilter {
///     name_contains: Some("crush".to_string()),
///     min_level: Some(10),
///     max_level: Some(20),
///     spawn_type: Some(SpawnTypeFilter::Npc),
/// };
/// ```
#[derive(Debug, Clone, Default)]
pub struct SpawnFilter {
    /// Only include spawns whose `displayed_name` contains this substring
    /// (case-insensitive). `None` matches all names.
    pub name_contains: Option<String>,
    /// Minimum level, inclusive. `None` = no lower bound.
    pub min_level: Option<u8>,
    /// Maximum level, inclusive. `None` = no upper bound.
    pub max_level: Option<u8>,
    /// Spawn type constraint. `None` accepts all types.
    pub spawn_type: Option<SpawnTypeFilter>,
}

impl SpawnFilter {
    /// Returns `true` if `spawn` satisfies every criterion set on this filter.
    #[must_use]
    pub fn matches(&self, spawn: &SpawnInfo) -> bool {
        if let Some(ref needle) = self.name_contains
            && !spawn
                .displayed_name
                .to_lowercase()
                .contains(&needle.to_lowercase())
        {
            return false;
        }

        if let Some(min) = self.min_level
            && spawn.level < min
        {
            return false;
        }

        if let Some(max) = self.max_level
            && spawn.level > max
        {
            return false;
        }

        if let Some(type_filter) = self.spawn_type
            && !type_filter.matches(spawn)
        {
            return false;
        }

        true
    }

    /// Apply this filter to a slice, returning matching spawns (cloned).
    #[must_use]
    pub fn apply<'a>(&self, spawns: &'a [SpawnInfo]) -> Vec<&'a SpawnInfo> {
        spawns.iter().filter(|s| self.matches(s)).collect()
    }
}

// ---------------------------------------------------------------------------
// SpawnSortKey
// ---------------------------------------------------------------------------

/// Ordering key used by [`SpawnSorter`].
#[derive(Debug, Clone)]
pub enum SpawnSortKey {
    /// Alphabetical by `displayed_name` (A → Z).
    Name,
    /// Ascending by level (lowest first).
    Level,
    /// Ascending by 3-D Euclidean distance from a reference point.
    Distance {
        /// Reference position `(x, y, z)`.
        from: (f32, f32, f32),
    },
    /// Ascending by `spawn_id`.
    Id,
}

// ---------------------------------------------------------------------------
// SpawnSorter
// ---------------------------------------------------------------------------

/// Sorts a `Vec<SpawnInfo>` in-place by a chosen key.
#[derive(Debug, Clone)]
pub struct SpawnSorter {
    /// The ordering criterion to apply.
    pub sort_by: SpawnSortKey,
}

impl SpawnSorter {
    /// Create a new sorter with the given key.
    #[must_use]
    pub fn new(sort_by: SpawnSortKey) -> Self {
        Self { sort_by }
    }

    /// Sort `spawns` in-place according to [`Self::sort_by`].
    pub fn sort(&self, spawns: &mut [SpawnInfo]) {
        match &self.sort_by {
            SpawnSortKey::Name => {
                spawns.sort_by(|a, b| a.displayed_name.cmp(&b.displayed_name));
            }
            SpawnSortKey::Level => {
                spawns.sort_by_key(|s| s.level);
            }
            SpawnSortKey::Distance { from } => {
                let (fx, fy, fz) = *from;
                spawns.sort_by(|a, b| {
                    let da = dist_sq(a.x, a.y, a.z, fx, fy, fz);
                    let db = dist_sq(b.x, b.y, b.z, fx, fy, fz);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            SpawnSortKey::Id => {
                spawns.sort_by_key(|s| s.spawn_id);
            }
        }
    }
}

fn dist_sq(ax: f32, ay: f32, az: f32, bx: f32, by: f32, bz: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    let dz = az - bz;
    dx * dx + dy * dy + dz * dz
}

// ---------------------------------------------------------------------------
// NamedSpawnTracker
// ---------------------------------------------------------------------------

/// Lightweight named-mob presence tracker.
///
/// Tracks whether named mobs (registered by name) are currently present
/// in the spawn list. Unlike [`super::named_tracker::NamedTracker`], this
/// struct does not manage respawn timers or priority databases — it is a
/// minimal "is it up?" layer suitable for scripting and unit tests.
///
/// # Usage
/// ```rust
/// use textquest::eq::spawn_filter::NamedSpawnTracker;
///
/// let mut tracker = NamedSpawnTracker::default();
/// tracker.track("Emperor Crush");
/// tracker.track("Lord Nagafen");
/// // ... call tracker.update(&spawns) each tick ...
/// // if tracker.is_up("Emperor Crush") { /* engage */ }
/// ```
#[derive(Debug, Clone, Default)]
pub struct NamedSpawnTracker {
    /// Map from lowercase tracked name → last seen `SpawnInfo`, if the mob is
    /// up.
    pub tracked: HashMap<String, Option<SpawnInfo>>,
}

impl NamedSpawnTracker {
    /// Create an empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a name to watch. The name comparison is case-insensitive.
    pub fn track(&mut self, name: &str) {
        self.tracked.entry(name.to_lowercase()).or_insert(None);
    }

    /// Unregister a name.
    pub fn untrack(&mut self, name: &str) {
        self.tracked.remove(&name.to_lowercase());
    }

    /// Refresh tracking state from the current live spawn list.
    ///
    /// For each registered name, sets the entry to `Some(spawn)` if a
    /// matching NPC is present, or `None` if it is absent.
    pub fn update(&mut self, spawns: &[SpawnInfo]) {
        // Build a lookup: lowercase name → first matching NPC spawn
        let mut live: HashMap<String, &SpawnInfo> = HashMap::new();
        for spawn in spawns {
            if spawn.spawn_type == SpawnType::Npc {
                live.entry(spawn.displayed_name.to_lowercase())
                    .or_insert(spawn);
            }
        }

        for (key, slot) in self.tracked.iter_mut() {
            *slot = live.get(key).map(|s| (*s).clone());
        }
    }

    /// Returns `true` if the named mob is currently present in the spawn list.
    #[must_use]
    pub fn is_up(&self, name: &str) -> bool {
        self.tracked
            .get(&name.to_lowercase())
            .is_some_and(|slot| slot.is_some())
    }

    /// Returns the last known `SpawnInfo` for a tracked mob, if it is up.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&SpawnInfo> {
        self.tracked
            .get(&name.to_lowercase())
            .and_then(|slot| slot.as_ref())
    }

    /// Number of tracked names (regardless of up/down state).
    #[must_use]
    pub fn len(&self) -> usize {
        self.tracked.len()
    }

    /// Whether no names are tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tracked.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::useless_vec)]

    use super::*;
    use crate::eq::structs::{EqClass, StandState};

    fn make_spawn(name: &str, level: u8, spawn_type: SpawnType, id: u32) -> SpawnInfo {
        SpawnInfo {
            name: name.to_string(),
            displayed_name: name.to_string(),
            lastname: String::new(),
            spawn_id: id,
            spawn_type,
            level,
            class_id: 1,
            class: Some(EqClass::Warrior),
            stand_state: StandState::Standing,
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
            is_gm: false,
            combat_target_id: None,
            race_id: 1,
            buff_slots: Vec::new(),
            spellbook: Vec::new(),
            memorized_spells: Vec::new(),
            cast_state: None,
        }
    }

    fn make_npc(name: &str, level: u8, id: u32) -> SpawnInfo {
        make_spawn(name, level, SpawnType::Npc, id)
    }

    fn make_pc(name: &str, level: u8, id: u32) -> SpawnInfo {
        make_spawn(name, level, SpawnType::Player, id)
    }

    // -----------------------------------------------------------------------
    // SpawnFilter tests
    // -----------------------------------------------------------------------

    #[test]
    fn filter_name_substring_case_insensitive() {
        let spawns = vec![
            make_npc("Emperor Crush", 15, 1),
            make_npc("a legionnaire", 5, 2),
            make_npc("Crushbone Crusher", 8, 3),
        ];

        let filter = SpawnFilter {
            name_contains: Some("crush".to_string()),
            ..Default::default()
        };

        let matched: Vec<_> = spawns.iter().filter(|s| filter.matches(s)).collect();
        assert_eq!(matched.len(), 2);
        assert_eq!(matched[0].displayed_name, "Emperor Crush");
        assert_eq!(matched[1].displayed_name, "Crushbone Crusher");
    }

    #[test]
    fn filter_level_range() {
        let spawns = vec![
            make_npc("Low Mob", 5, 1),
            make_npc("Mid Mob", 15, 2),
            make_npc("High Mob", 50, 3),
        ];

        let filter = SpawnFilter {
            min_level: Some(10),
            max_level: Some(20),
            ..Default::default()
        };

        let matched: Vec<_> = spawns.iter().filter(|s| filter.matches(s)).collect();
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].displayed_name, "Mid Mob");
    }

    #[test]
    fn filter_level_min_only() {
        let spawns = vec![make_npc("Mob A", 1, 1), make_npc("Mob B", 30, 2)];
        let filter = SpawnFilter {
            min_level: Some(10),
            ..Default::default()
        };
        let matched: Vec<_> = spawns.iter().filter(|s| filter.matches(s)).collect();
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].displayed_name, "Mob B");
    }

    #[test]
    fn filter_level_max_only() {
        let spawns = vec![make_npc("Mob A", 5, 1), make_npc("Mob B", 30, 2)];
        let filter = SpawnFilter {
            max_level: Some(10),
            ..Default::default()
        };
        let matched: Vec<_> = spawns.iter().filter(|s| filter.matches(s)).collect();
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].displayed_name, "Mob A");
    }

    #[test]
    fn filter_spawn_type_npc() {
        let spawns = vec![make_npc("a rat", 1, 1), make_pc("Maleick", 60, 2)];
        let filter = SpawnFilter {
            spawn_type: Some(SpawnTypeFilter::Npc),
            ..Default::default()
        };
        let matched: Vec<_> = spawns.iter().filter(|s| filter.matches(s)).collect();
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].displayed_name, "a rat");
    }

    #[test]
    fn filter_spawn_type_pc() {
        let spawns = vec![make_npc("a rat", 1, 1), make_pc("Maleick", 60, 2)];
        let filter = SpawnFilter {
            spawn_type: Some(SpawnTypeFilter::Pc),
            ..Default::default()
        };
        let matched: Vec<_> = spawns.iter().filter(|s| filter.matches(s)).collect();
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].displayed_name, "Maleick");
    }

    #[test]
    fn filter_spawn_type_pet() {
        let spawns = vec![
            make_npc("a rat", 1, 1),
            make_npc("Maleick's pet", 50, 2),
            make_npc("Druid's warder", 55, 3),
        ];
        let filter = SpawnFilter {
            spawn_type: Some(SpawnTypeFilter::Pet),
            ..Default::default()
        };
        let matched: Vec<_> = spawns.iter().filter(|s| filter.matches(s)).collect();
        assert_eq!(matched.len(), 2);
    }

    #[test]
    fn filter_no_criteria_matches_all() {
        let spawns = vec![make_npc("Mob A", 5, 1), make_pc("Player", 60, 2)];
        let filter = SpawnFilter::default();
        let matched: Vec<_> = spawns.iter().filter(|s| filter.matches(s)).collect();
        assert_eq!(matched.len(), 2);
    }

    #[test]
    fn filter_combined_name_and_level() {
        let spawns = vec![
            make_npc("Emperor Crush", 15, 1),
            make_npc("Emperor Snail", 2, 2),
        ];
        let filter = SpawnFilter {
            name_contains: Some("Emperor".to_string()),
            min_level: Some(10),
            ..Default::default()
        };
        let matched: Vec<_> = spawns.iter().filter(|s| filter.matches(s)).collect();
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].displayed_name, "Emperor Crush");
    }

    // -----------------------------------------------------------------------
    // SpawnSorter tests
    // -----------------------------------------------------------------------

    #[test]
    fn sort_by_level_ascending() {
        let mut spawns = vec![
            make_npc("High", 50, 3),
            make_npc("Low", 1, 1),
            make_npc("Mid", 20, 2),
        ];
        let sorter = SpawnSorter::new(SpawnSortKey::Level);
        sorter.sort(&mut spawns);
        assert_eq!(spawns[0].level, 1);
        assert_eq!(spawns[1].level, 20);
        assert_eq!(spawns[2].level, 50);
    }

    #[test]
    fn sort_by_name_alphabetical() {
        let mut spawns = vec![
            make_npc("Zebra", 10, 3),
            make_npc("Apple", 10, 1),
            make_npc("Mango", 10, 2),
        ];
        let sorter = SpawnSorter::new(SpawnSortKey::Name);
        sorter.sort(&mut spawns);
        assert_eq!(spawns[0].displayed_name, "Apple");
        assert_eq!(spawns[1].displayed_name, "Mango");
        assert_eq!(spawns[2].displayed_name, "Zebra");
    }

    #[test]
    fn sort_by_id() {
        let mut spawns = vec![
            make_npc("C", 1, 300),
            make_npc("A", 1, 100),
            make_npc("B", 1, 200),
        ];
        let sorter = SpawnSorter::new(SpawnSortKey::Id);
        sorter.sort(&mut spawns);
        assert_eq!(spawns[0].spawn_id, 100);
        assert_eq!(spawns[1].spawn_id, 200);
        assert_eq!(spawns[2].spawn_id, 300);
    }

    #[test]
    fn sort_by_distance() {
        let mut a = make_npc("Near", 1, 1);
        a.x = 1.0;
        a.y = 0.0;
        a.z = 0.0;

        let mut b = make_npc("Far", 1, 2);
        b.x = 100.0;
        b.y = 0.0;
        b.z = 0.0;

        let mut c = make_npc("Mid", 1, 3);
        c.x = 10.0;
        c.y = 0.0;
        c.z = 0.0;

        let mut spawns = vec![b.clone(), c.clone(), a.clone()];
        let sorter = SpawnSorter::new(SpawnSortKey::Distance {
            from: (0.0, 0.0, 0.0),
        });
        sorter.sort(&mut spawns);
        assert_eq!(spawns[0].displayed_name, "Near");
        assert_eq!(spawns[1].displayed_name, "Mid");
        assert_eq!(spawns[2].displayed_name, "Far");
    }

    // -----------------------------------------------------------------------
    // NamedSpawnTracker tests
    // -----------------------------------------------------------------------

    fn make_named_npc(name: &str) -> SpawnInfo {
        make_npc(name, 50, 999)
    }

    #[test]
    fn named_tracker_detects_up() {
        let mut tracker = NamedSpawnTracker::new();
        tracker.track("Emperor Crush");

        let spawns = vec![
            make_named_npc("Emperor Crush"),
            make_npc("a legionnaire", 5, 1),
        ];
        tracker.update(&spawns);

        assert!(tracker.is_up("Emperor Crush"));
        assert!(tracker.is_up("emperor crush")); // case-insensitive
    }

    #[test]
    fn named_tracker_detects_down() {
        let mut tracker = NamedSpawnTracker::new();
        tracker.track("Emperor Crush");

        // First: mob is up
        let spawns = vec![make_named_npc("Emperor Crush")];
        tracker.update(&spawns);
        assert!(tracker.is_up("Emperor Crush"));

        // Second: mob is gone
        tracker.update(&[]);
        assert!(!tracker.is_up("Emperor Crush"));
    }

    #[test]
    fn named_tracker_untracked_name_returns_false() {
        let tracker = NamedSpawnTracker::new();
        assert!(!tracker.is_up("Lord Nagafen"));
    }

    #[test]
    fn named_tracker_get_returns_spawn_info() {
        let mut tracker = NamedSpawnTracker::new();
        tracker.track("Emperor Crush");

        let mut spawn = make_named_npc("Emperor Crush");
        spawn.spawn_id = 4242;

        tracker.update(&[spawn]);

        let info = tracker.get("Emperor Crush").expect("should be up");
        assert_eq!(info.spawn_id, 4242);
    }

    #[test]
    fn named_tracker_len_and_empty() {
        let mut tracker = NamedSpawnTracker::new();
        assert!(tracker.is_empty());
        assert_eq!(tracker.len(), 0);

        tracker.track("Lord Nagafen");
        assert!(!tracker.is_empty());
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn named_tracker_untrack_removes_entry() {
        let mut tracker = NamedSpawnTracker::new();
        tracker.track("Boss");
        assert_eq!(tracker.len(), 1);
        tracker.untrack("Boss");
        assert_eq!(tracker.len(), 0);
        assert!(!tracker.is_up("Boss"));
    }

    #[test]
    fn named_tracker_multiple_names() {
        let mut tracker = NamedSpawnTracker::new();
        tracker.track("Emperor Crush");
        tracker.track("Lord Nagafen");
        tracker.track("Lady Vox");

        let spawns = vec![make_named_npc("Emperor Crush"), make_named_npc("Lady Vox")];
        tracker.update(&spawns);

        assert!(tracker.is_up("Emperor Crush"));
        assert!(!tracker.is_up("Lord Nagafen"));
        assert!(tracker.is_up("Lady Vox"));
    }

    #[test]
    fn named_tracker_only_tracks_npcs() {
        let mut tracker = NamedSpawnTracker::new();
        tracker.track("Maleick");

        // PC with same name as tracked mob — should NOT count as "up"
        let spawns = vec![make_pc("Maleick", 60, 1)];
        tracker.update(&spawns);

        assert!(!tracker.is_up("Maleick"));
    }
}
