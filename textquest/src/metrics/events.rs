//! Fleet event capture types for DLL → IPC → SQLite pipeline.
//!
//! Typed event variants that flow from the injected DLL through IPC
//! into the metrics SQLite store. Each variant captures the minimum
//! data needed for fleet intelligence queries.

use std::collections::HashMap;
use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// A discrete fleet event captured from a game client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum FleetEvent {
    Kill {
        source_pid: u32,
        target_name: String,
        target_level: u8,
        zone: String,
        timestamp: i64,
    },
    Death {
        pid: u32,
        character_name: String,
        zone: String,
        timestamp: i64,
    },
    LootDrop {
        pid: u32,
        item_name: String,
        item_id: u32,
        zone: String,
        timestamp: i64,
    },
    ZoneChange {
        pid: u32,
        from_zone: String,
        to_zone: String,
        timestamp: i64,
    },
    LevelUp {
        pid: u32,
        character_name: String,
        new_level: u8,
        timestamp: i64,
    },
    CombatRound {
        pid: u32,
        damage_dealt: u64,
        damage_taken: u64,
        duration_ms: u32,
        timestamp: i64,
    },
}

impl FleetEvent {
    /// Returns the PID associated with this event.
    pub fn pid(&self) -> u32 {
        match self {
            Self::Kill { source_pid, .. } => *source_pid,
            Self::Death { pid, .. }
            | Self::LootDrop { pid, .. }
            | Self::ZoneChange { pid, .. }
            | Self::LevelUp { pid, .. }
            | Self::CombatRound { pid, .. } => *pid,
        }
    }

    /// Returns the Unix epoch timestamp.
    pub fn timestamp(&self) -> i64 {
        match self {
            Self::Kill { timestamp, .. }
            | Self::Death { timestamp, .. }
            | Self::LootDrop { timestamp, .. }
            | Self::ZoneChange { timestamp, .. }
            | Self::LevelUp { timestamp, .. }
            | Self::CombatRound { timestamp, .. } => *timestamp,
        }
    }
}

/// A platinum change event recorded when a character's balance changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatEvent {
    /// Process ID of the game client.
    pub pid: u32,
    /// Character whose balance changed.
    pub character: String,
    /// Signed delta (positive = earned, negative = spent).
    pub delta: i64,
    /// New absolute balance after the change.
    pub balance: i64,
    /// Optional source label (e.g. "vendor_sale", "spell_purchase").
    pub source: Option<String>,
    /// Unix epoch timestamp of the observation.
    pub timestamp: i64,
}

impl PlatEvent {
    /// Constructs a new `PlatEvent`.
    pub fn new(
        pid: u32,
        character: impl Into<String>,
        delta: i64,
        balance: i64,
        source: Option<String>,
        timestamp: i64,
    ) -> Self {
        Self {
            pid,
            character: character.into(),
            delta,
            balance,
            source,
            timestamp,
        }
    }
}

/// Per-session platinum tracker.
///
/// Tracks the running balance per character, accumulates session earnings,
/// and computes plat-per-hour based on session elapsed time.
pub struct PlatTracker {
    /// Session start time as a Unix epoch timestamp.
    session_start: i64,
    /// Last known balance per character (from the most recent `PlatEvent`).
    last_balance: HashMap<String, i64>,
    /// Cumulative plat earned (sum of positive deltas) per character since session start.
    session_earned: HashMap<String, i64>,
}

impl PlatTracker {
    /// Create a new tracker anchored to `session_start` (Unix epoch seconds).
    pub fn new(session_start: i64) -> Self {
        Self {
            session_start,
            last_balance: HashMap::new(),
            session_earned: HashMap::new(),
        }
    }

    /// Record a platinum event, updating internal state.
    ///
    /// Returns the computed `PlatEvent` with delta filled in.
    pub fn record(
        &mut self,
        pid: u32,
        character: &str,
        new_balance: i64,
        timestamp: i64,
    ) -> PlatEvent {
        self.record_with_source(pid, character, new_balance, None, timestamp)
    }

    /// Record a platinum event with an optional source label, updating internal state.
    ///
    /// Returns the computed `PlatEvent` with delta filled in.
    pub fn record_with_source(
        &mut self,
        pid: u32,
        character: &str,
        new_balance: i64,
        source: Option<String>,
        timestamp: i64,
    ) -> PlatEvent {
        let prev = self
            .last_balance
            .get(character)
            .copied()
            .unwrap_or(new_balance);
        let delta = new_balance - prev;
        self.last_balance.insert(character.to_owned(), new_balance);
        if delta > 0 {
            *self.session_earned.entry(character.to_owned()).or_insert(0) += delta;
        }
        PlatEvent::new(pid, character, delta, new_balance, source, timestamp)
    }

    /// Cumulative plat earned by `character` since session start.
    pub fn session_earned(&self, character: &str) -> i64 {
        self.session_earned.get(character).copied().unwrap_or(0)
    }

    /// Total plat earned across all characters since session start.
    pub fn total_session_earned(&self) -> i64 {
        self.session_earned.values().sum()
    }

    /// Last known balance for `character`, or `None` if never seen.
    pub fn last_balance(&self, character: &str) -> Option<i64> {
        self.last_balance.get(character).copied()
    }

    /// Plat per hour for `character` based on session duration ending at `now`.
    ///
    /// Returns `0.0` if the session has just started (< 1 second elapsed).
    pub fn plat_per_hour(&self, character: &str, now: i64) -> f64 {
        self.to_per_hour(self.session_earned(character), now)
    }

    /// Fleet-wide plat per hour based on total session earnings and duration.
    pub fn fleet_plat_per_hour(&self, now: i64) -> f64 {
        self.to_per_hour(self.total_session_earned(), now)
    }

    /// Convert a raw earned amount to a per-hour rate given `now`.
    fn to_per_hour(&self, earned: i64, now: i64) -> f64 {
        let elapsed_secs = (now - self.session_start).max(0) as f64;
        if elapsed_secs < 1.0 {
            return 0.0;
        }
        earned as f64 / elapsed_secs * 3600.0
    }

    /// Session start timestamp (Unix epoch seconds).
    pub fn session_start(&self) -> i64 {
        self.session_start
    }
}

/// Bounded ring buffer of fleet events with query helpers.
pub struct FleetEventLog {
    events: VecDeque<FleetEvent>,
    capacity: usize,
}

impl FleetEventLog {
    /// Creates a new event log with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Pushes an event, evicting the oldest if at capacity.
    pub fn push(&mut self, event: FleetEvent) {
        if self.events.len() == self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// Returns the `n` most recent events (newest last).
    pub fn recent(&self, n: usize) -> Vec<&FleetEvent> {
        let len = self.events.len();
        let skip = len.saturating_sub(n);
        self.events.iter().skip(skip).collect()
    }

    /// Returns all events from the given PID.
    pub fn filter_by_pid(&self, pid: u32) -> Vec<&FleetEvent> {
        self.events.iter().filter(|e| e.pid() == pid).collect()
    }

    /// Counts kill events since the given timestamp (inclusive).
    pub fn kills_since(&self, timestamp: i64) -> usize {
        self.events
            .iter()
            .filter(|e| matches!(e, FleetEvent::Kill { .. }) && e.timestamp() >= timestamp)
            .count()
    }

    /// Counts death events since the given timestamp (inclusive).
    pub fn deaths_since(&self, timestamp: i64) -> usize {
        self.events
            .iter()
            .filter(|e| matches!(e, FleetEvent::Death { .. }) && e.timestamp() >= timestamp)
            .count()
    }

    /// Returns the total number of events in the log.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns true if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_kill(pid: u32, ts: i64) -> FleetEvent {
        FleetEvent::Kill {
            source_pid: pid,
            target_name: "mob".into(),
            target_level: 50,
            zone: "gfaydark".into(),
            timestamp: ts,
        }
    }

    fn make_death(pid: u32, ts: i64) -> FleetEvent {
        FleetEvent::Death {
            pid,
            character_name: "Toon".into(),
            zone: "gfaydark".into(),
            timestamp: ts,
        }
    }

    #[test]
    fn push_and_len() {
        let mut log = FleetEventLog::new(10);
        assert!(log.is_empty());
        log.push(make_kill(1, 100));
        assert_eq!(log.len(), 1);
        assert!(!log.is_empty());
    }

    #[test]
    fn capacity_eviction() {
        let mut log = FleetEventLog::new(3);
        for i in 0..5 {
            log.push(make_kill(1, i));
        }
        assert_eq!(log.len(), 3);
        // oldest two (ts=0, ts=1) evicted; remaining are ts=2,3,4
        let recent = log.recent(10);
        assert_eq!(recent[0].timestamp(), 2);
        assert_eq!(recent[2].timestamp(), 4);
    }

    #[test]
    fn recent_returns_newest() {
        let mut log = FleetEventLog::new(10);
        for i in 0..5 {
            log.push(make_kill(1, i));
        }
        let last_two = log.recent(2);
        assert_eq!(last_two.len(), 2);
        assert_eq!(last_two[0].timestamp(), 3);
        assert_eq!(last_two[1].timestamp(), 4);
    }

    #[test]
    fn recent_more_than_available() {
        let mut log = FleetEventLog::new(10);
        log.push(make_kill(1, 100));
        let all = log.recent(50);
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn filter_by_pid() {
        let mut log = FleetEventLog::new(10);
        log.push(make_kill(1, 100));
        log.push(make_kill(2, 101));
        log.push(make_death(1, 102));
        let pid1 = log.filter_by_pid(1);
        assert_eq!(pid1.len(), 2);
        let pid2 = log.filter_by_pid(2);
        assert_eq!(pid2.len(), 1);
    }

    #[test]
    fn kills_since() {
        let mut log = FleetEventLog::new(10);
        log.push(make_kill(1, 100));
        log.push(make_kill(1, 200));
        log.push(make_death(1, 250));
        log.push(make_kill(1, 300));
        assert_eq!(log.kills_since(200), 2);
        assert_eq!(log.kills_since(301), 0);
    }

    #[test]
    fn deaths_since() {
        let mut log = FleetEventLog::new(10);
        log.push(make_death(1, 100));
        log.push(make_kill(1, 150));
        log.push(make_death(2, 200));
        assert_eq!(log.deaths_since(100), 2);
        assert_eq!(log.deaths_since(150), 1);
        assert_eq!(log.deaths_since(201), 0);
    }

    #[test]
    fn pid_accessor() {
        assert_eq!(make_kill(42, 0).pid(), 42);
        assert_eq!(make_death(7, 0).pid(), 7);

        let zone = FleetEvent::ZoneChange {
            pid: 99,
            from_zone: "a".into(),
            to_zone: "b".into(),
            timestamp: 0,
        };
        assert_eq!(zone.pid(), 99);
    }

    #[test]
    fn serde_roundtrip() {
        let event = make_kill(1, 12345);
        let json = serde_json::to_string(&event).unwrap();
        let back: FleetEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    // ── PlatEvent tests ──────────────────────────────────────────────────

    #[test]
    fn plat_event_fields() {
        let ev = PlatEvent::new(42, "Warrior01", 500, 1500, Some("vendor_sale".into()), 1000);
        assert_eq!(ev.pid, 42);
        assert_eq!(ev.character, "Warrior01");
        assert_eq!(ev.delta, 500);
        assert_eq!(ev.balance, 1500);
        assert_eq!(ev.source.as_deref(), Some("vendor_sale"));
        assert_eq!(ev.timestamp, 1000);
    }

    #[test]
    fn plat_event_serde_roundtrip() {
        let ev = PlatEvent::new(1, "Cleric01", -200, 800, None, 9999);
        let json = serde_json::to_string(&ev).unwrap();
        let back: PlatEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(ev, back);
    }

    // ── PlatTracker tests ────────────────────────────────────────────────

    #[test]
    fn tracker_initial_record_zero_delta() {
        // First observation establishes the baseline — delta should be 0.
        let mut tracker = PlatTracker::new(1000);
        let ev = tracker.record(1, "Char_A", 5000, 1010);
        assert_eq!(ev.delta, 0);
        assert_eq!(ev.balance, 5000);
        assert_eq!(tracker.session_earned("Char_A"), 0);
    }

    #[test]
    fn tracker_positive_delta_accumulates() {
        let mut tracker = PlatTracker::new(0);
        tracker.record(1, "Char_A", 1000, 100);
        tracker.record(1, "Char_A", 1500, 200);
        let ev = tracker.record(1, "Char_A", 2000, 300);
        assert_eq!(ev.delta, 500);
        assert_eq!(tracker.session_earned("Char_A"), 1000); // +500 +500
    }

    #[test]
    fn tracker_negative_delta_not_counted_in_earned() {
        let mut tracker = PlatTracker::new(0);
        tracker.record(1, "Char_A", 1000, 100);
        tracker.record(1, "Char_A", 1500, 200); // +500 earned
        tracker.record(1, "Char_A", 800, 300); // -700 spent — should NOT reduce earned
        assert_eq!(tracker.session_earned("Char_A"), 500);
    }

    #[test]
    fn tracker_last_balance() {
        let mut tracker = PlatTracker::new(0);
        assert_eq!(tracker.last_balance("Char_A"), None);
        tracker.record(1, "Char_A", 1234, 0);
        assert_eq!(tracker.last_balance("Char_A"), Some(1234));
        tracker.record(1, "Char_A", 4321, 10);
        assert_eq!(tracker.last_balance("Char_A"), Some(4321));
    }

    #[test]
    fn tracker_multiple_characters_isolated() {
        let mut tracker = PlatTracker::new(0);
        tracker.record(1, "Char_A", 1000, 100);
        tracker.record(2, "Char_B", 2000, 100);
        tracker.record(1, "Char_A", 1500, 200); // Char_A earned 500
        tracker.record(2, "Char_B", 2300, 200); // Char_B earned 300

        assert_eq!(tracker.session_earned("Char_A"), 500);
        assert_eq!(tracker.session_earned("Char_B"), 300);
        assert_eq!(tracker.total_session_earned(), 800);
    }

    #[test]
    fn tracker_plat_per_hour() {
        // Verify plat/hour uses earned / elapsed * 3600.
        // Here, earned = 3600 and with session_start = 0 and now = 3600,
        // elapsed = 3600, so (3600 / 3600) * 3600 = 3600 plat/hr.
        let mut tracker = PlatTracker::new(0);
        tracker.record(1, "Char_A", 0, 0); // baseline
        tracker.record(1, "Char_A", 3600, 1); // sets total earned to 3600; rate is checked at now = 3600
        let pph = tracker.plat_per_hour("Char_A", 3600);
        assert!((pph - 3600.0).abs() < 0.01, "expected ~3600, got {pph}");
    }

    #[test]
    fn tracker_plat_per_hour_zero_elapsed() {
        let mut tracker = PlatTracker::new(1000);
        tracker.record(1, "Char_A", 0, 1000);
        tracker.record(1, "Char_A", 500, 1000);
        // now == session_start → 0 elapsed
        assert_eq!(tracker.plat_per_hour("Char_A", 1000), 0.0);
    }

    #[test]
    fn tracker_fleet_plat_per_hour() {
        let mut tracker = PlatTracker::new(0);
        tracker.record(1, "Char_A", 0, 0);
        tracker.record(2, "Char_B", 0, 0);
        tracker.record(1, "Char_A", 1800, 1); // +1800
        tracker.record(2, "Char_B", 1800, 1); // +1800, total earned = 3600
        // at now=3600s: 3600/3600*3600 = 3600 plat/hr fleet
        let fpph = tracker.fleet_plat_per_hour(3600);
        assert!((fpph - 3600.0).abs() < 0.01, "expected ~3600, got {fpph}");
    }

    #[test]
    fn tracker_session_start() {
        let tracker = PlatTracker::new(12345);
        assert_eq!(tracker.session_start(), 12345);
    }
}
