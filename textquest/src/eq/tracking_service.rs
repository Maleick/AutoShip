//! MQ2Tracking parity — auto-track for Ranger/Bard with watchlist
//! cross-reference.
//!
//! `TrackingService` mirrors MQ2Tracking behaviour:
//! - Issues `/track` on zone-in when a tracker class (RNG/BRD) is active.
//! - Parses the track-window chat output for both Ranger and Bard formats.
//! - Cross-references results against a watchlist and emits `SpawnOnTrack`
//!   events for hits.
//! - Persists first-sighting timestamps per zone.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use super::structs::EqClass;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single entry from the EQ track list.
#[derive(Debug, Clone)]
pub struct TrackEntry {
    /// Display name as parsed from the track window.
    pub name: String,
    /// Zone-relative distance from the track string (yards, approximate).
    pub distance: f32,
    /// Direction from the track string (degrees, 0 = North).
    pub direction: f32,
}

/// Event fired when a watchlist name appears in the track list.
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnOnTrack {
    /// Name that matched the watchlist.
    pub name: String,
    /// Zone where the sighting occurred.
    pub zone: String,
    /// Wall-clock time of the first sighting in this zone.
    pub first_seen: SystemTime,
    /// Whether this is the first time this name has been seen in this zone.
    pub is_first_sighting: bool,
    /// Distance from track string (yards).
    pub distance: f32,
}

/// Cadence control — how often to re-issue `/track`.
#[derive(Debug, Clone)]
pub struct TrackCadence {
    /// Minimum time between `/track` commands.
    pub interval: Duration,
}

impl Default for TrackCadence {
    fn default() -> Self {
        // 30 s is a reasonable default — track list updates slowly.
        Self {
            interval: Duration::from_secs(30),
        }
    }
}

// ---------------------------------------------------------------------------
// TrackingService
// ---------------------------------------------------------------------------

/// Manages auto-tracking for Ranger and Bard clients.
pub struct TrackingService {
    /// Current zone.
    zone: String,
    /// Class of the character using this service.
    character_class: EqClass,
    /// Watchlist patterns (case-insensitive substring matches).
    watchlist: Vec<String>,
    /// First-sighting timestamp per (zone, name_lowercase).
    first_sightings: HashMap<(String, String), SystemTime>,
    /// When we last issued a `/track` command.
    last_track_time: Option<SystemTime>,
    /// How often to re-issue `/track`.
    cadence: TrackCadence,
    /// Most recent parsed track list (name → entry).
    current_track_list: Vec<TrackEntry>,
}

impl TrackingService {
    /// Create a new service for a character of the given class.
    ///
    /// Only `EqClass::Ranger` and `EqClass::Bard` will actually issue `/track`.
    #[must_use]
    pub fn new(character_class: EqClass) -> Self {
        Self {
            zone: String::new(),
            character_class,
            watchlist: Vec::new(),
            first_sightings: HashMap::new(),
            last_track_time: None,
            cadence: TrackCadence::default(),
            current_track_list: Vec::new(),
        }
    }

    /// Override the track cadence.
    pub fn set_cadence(&mut self, cadence: TrackCadence) {
        self.cadence = cadence;
    }

    /// Set the watchlist.  Each entry is a case-insensitive substring pattern.
    pub fn set_watchlist(&mut self, patterns: Vec<String>) {
        self.watchlist = patterns.into_iter().map(|p| p.to_lowercase()).collect();
    }

    /// Returns `true` if this class can use the `/track` skill.
    #[must_use]
    pub fn is_tracker_class(&self) -> bool {
        matches!(self.character_class, EqClass::Ranger | EqClass::Bard)
    }

    /// Call on zone-in.  Returns a `/track` command string if this client
    /// should issue it immediately.
    pub fn on_zone_in(&mut self, zone: &str) -> Option<String> {
        if zone != self.zone {
            self.zone = zone.to_string();
            self.current_track_list.clear();
            self.last_track_time = None;
        }
        if self.is_tracker_class() {
            self.last_track_time = Some(SystemTime::now());
            Some("/track".to_string())
        } else {
            None
        }
    }

    /// Call each game tick.  Returns a `/track` command if enough time has
    /// elapsed since the last one.
    pub fn tick(&mut self) -> Option<String> {
        if !self.is_tracker_class() {
            return None;
        }
        let now = SystemTime::now();
        let should_track = match self.last_track_time {
            None => true,
            Some(last) => now
                .duration_since(last)
                .unwrap_or(Duration::ZERO)
                >= self.cadence.interval,
        };
        if should_track {
            self.last_track_time = Some(now);
            Some("/track".to_string())
        } else {
            None
        }
    }

    /// Feed raw chat lines from the track window.  Returns watchlist hits as
    /// `SpawnOnTrack` events.
    ///
    /// The EQ track window outputs one line per tracked mob.  Ranger format:
    /// ```text
    /// Ignis the Undying is to the North, 123 yards away.
    /// ```
    /// Bard format (slightly different phrasing):
    /// ```text
    /// Ignis the Undying (North, 123 yards)
    /// ```
    /// Both are handled here.
    pub fn ingest_track_lines(&mut self, lines: &[&str]) -> Vec<SpawnOnTrack> {
        let mut new_list = Vec::new();
        for &line in lines {
            if let Some(entry) = parse_track_line(line) {
                new_list.push(entry);
            }
        }
        self.current_track_list = new_list;
        self.cross_reference_watchlist()
    }

    /// Cross-reference the current track list against the watchlist, emitting
    /// `SpawnOnTrack` events for any hits.
    fn cross_reference_watchlist(&mut self) -> Vec<SpawnOnTrack> {
        let mut events = Vec::new();
        let zone = self.zone.clone();
        for entry in &self.current_track_list {
            let name_lower = entry.name.to_lowercase();
            if self
                .watchlist
                .iter()
                .any(|pat| name_lower.contains(pat.as_str()))
            {
                let key = (zone.clone(), name_lower);
                let now = SystemTime::now();
                let is_first = !self.first_sightings.contains_key(&key);
                let first_seen = *self.first_sightings.entry(key).or_insert(now);
                events.push(SpawnOnTrack {
                    name: entry.name.clone(),
                    zone: zone.clone(),
                    first_seen,
                    is_first_sighting: is_first,
                    distance: entry.distance,
                });
            }
        }
        events
    }

    /// Returns the current parsed track list (last ingested).
    #[must_use]
    pub fn current_track_list(&self) -> &[TrackEntry] {
        &self.current_track_list
    }

    /// Returns the first-sighting timestamp for a given zone + name, if any.
    #[must_use]
    pub fn first_sighting(&self, zone: &str, name: &str) -> Option<SystemTime> {
        self.first_sightings
            .get(&(zone.to_string(), name.to_lowercase()))
            .copied()
    }

    /// Returns the current zone.
    #[must_use]
    pub fn zone(&self) -> &str {
        &self.zone
    }
}

// ---------------------------------------------------------------------------
// Track-line parsers
// ---------------------------------------------------------------------------

/// Parse a single track-window line into a `TrackEntry`.
///
/// Handles both Ranger format:
///   `<Name> is to the <Dir>, <N> yards away.`
/// and Bard format:
///   `<Name> (<Dir>, <N> yards)`
#[must_use]
fn parse_track_line(line: &str) -> Option<TrackEntry> {
    let line = line.trim();
    // Try Ranger format first.
    if let Some(entry) = parse_ranger_format(line) {
        return Some(entry);
    }
    parse_bard_format(line)
}

/// Ranger: `Ignis the Undying is to the North, 123 yards away.`
fn parse_ranger_format(line: &str) -> Option<TrackEntry> {
    // Split on " is to the "
    let (name_part, rest) = line.split_once(" is to the ")?;
    // rest: "North, 123 yards away."
    let (dir_part, yard_part) = rest.split_once(", ")?;
    // yard_part: "123 yards away."
    let yards_str = yard_part
        .trim_end_matches('.')
        .trim_end_matches(" away")
        .trim_end_matches(" yards")
        .trim();
    let distance = yards_str.parse::<f32>().ok()?;
    Some(TrackEntry {
        name: name_part.trim().to_string(),
        distance,
        direction: direction_degrees(dir_part.trim()),
    })
}

/// Bard: `Ignis the Undying (North, 123 yards)`
fn parse_bard_format(line: &str) -> Option<TrackEntry> {
    // Find the last `(` to handle names that might contain parentheses.
    let paren_open = line.rfind('(')?;
    let paren_close = line.rfind(')')?;
    if paren_close < paren_open {
        return None;
    }
    let name_part = line[..paren_open].trim();
    let inner = &line[paren_open + 1..paren_close];
    // inner: "North, 123 yards"
    let (dir_part, yard_part) = inner.split_once(", ")?;
    let yards_str = yard_part
        .trim_end_matches(" yards")
        .trim();
    let distance = yards_str.parse::<f32>().ok()?;
    Some(TrackEntry {
        name: name_part.to_string(),
        distance,
        direction: direction_degrees(dir_part.trim()),
    })
}

/// Convert a cardinal/intercardinal direction string to degrees (0 = North).
fn direction_degrees(dir: &str) -> f32 {
    match dir {
        "North" => 0.0,
        "NorthEast" | "Northeast" => 45.0,
        "East" => 90.0,
        "SouthEast" | "Southeast" => 135.0,
        "South" => 180.0,
        "SouthWest" | "Southwest" => 225.0,
        "West" => 270.0,
        "NorthWest" | "Northwest" => 315.0,
        _ => 0.0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ranger_format_basic() {
        let entry = parse_ranger_format("Ignis the Undying is to the North, 123 yards away.")
            .expect("should parse");
        assert_eq!(entry.name, "Ignis the Undying");
        assert!((entry.distance - 123.0).abs() < f32::EPSILON);
        assert!((entry.direction - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_bard_format_basic() {
        let entry = parse_bard_format("Ignis the Undying (South, 456 yards)")
            .expect("should parse");
        assert_eq!(entry.name, "Ignis the Undying");
        assert!((entry.distance - 456.0).abs() < f32::EPSILON);
        assert!((entry.direction - 180.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_track_line_dispatches_both_formats() {
        let ranger = parse_track_line("Phinigel Autropos is to the East, 88 yards away.");
        let bard = parse_track_line("Phinigel Autropos (West, 99 yards)");
        assert!(ranger.is_some());
        assert!(bard.is_some());
    }

    #[test]
    fn is_tracker_class_ranger_bard_only() {
        let rng = TrackingService::new(EqClass::Ranger);
        let brd = TrackingService::new(EqClass::Bard);
        let war = TrackingService::new(EqClass::Warrior);
        assert!(rng.is_tracker_class());
        assert!(brd.is_tracker_class());
        assert!(!war.is_tracker_class());
    }

    #[test]
    fn on_zone_in_returns_track_command_for_tracker() {
        let mut svc = TrackingService::new(EqClass::Ranger);
        let cmd = svc.on_zone_in("crushbone");
        assert_eq!(cmd, Some("/track".to_string()));
    }

    #[test]
    fn on_zone_in_returns_none_for_non_tracker() {
        let mut svc = TrackingService::new(EqClass::Warrior);
        let cmd = svc.on_zone_in("crushbone");
        assert!(cmd.is_none());
    }

    #[test]
    fn watchlist_hit_emits_spawn_on_track_event() {
        let mut svc = TrackingService::new(EqClass::Ranger);
        svc.on_zone_in("western_wastes");
        svc.set_watchlist(vec!["ignis".to_string()]);
        let lines = [
            "Ignis the Undying is to the North, 200 yards away.",
            "some random mob is to the South, 50 yards away.",
        ];
        let events = svc.ingest_track_lines(&lines);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "Ignis the Undying");
        assert!(events[0].is_first_sighting);
    }

    #[test]
    fn first_sighting_persisted_across_ingestions() {
        let mut svc = TrackingService::new(EqClass::Bard);
        svc.on_zone_in("plane_of_fear");
        svc.set_watchlist(vec!["cazic".to_string()]);
        let lines = ["Cazic Thule (North, 300 yards)"];
        let ev1 = svc.ingest_track_lines(&lines);
        let ev2 = svc.ingest_track_lines(&lines);
        assert!(ev1[0].is_first_sighting);
        assert!(!ev2[0].is_first_sighting);
        assert_eq!(ev1[0].first_seen, ev2[0].first_seen);
    }

    #[test]
    fn tick_returns_track_command_after_interval() {
        let mut svc = TrackingService::new(EqClass::Ranger);
        svc.on_zone_in("west_commonlands");
        // Override cadence to zero so next tick fires immediately.
        svc.set_cadence(TrackCadence {
            interval: Duration::ZERO,
        });
        let cmd = svc.tick();
        assert_eq!(cmd, Some("/track".to_string()));
    }

    #[test]
    fn tick_returns_none_for_non_tracker() {
        let mut svc = TrackingService::new(EqClass::Cleric);
        svc.on_zone_in("qeynos");
        let cmd = svc.tick();
        assert!(cmd.is_none());
    }
}
