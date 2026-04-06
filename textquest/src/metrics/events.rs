//! Fleet event capture types for DLL → IPC → SQLite pipeline.
//!
//! Typed event variants that flow from the injected DLL through IPC
//! into the metrics SQLite store. Each variant captures the minimum
//! data needed for fleet intelligence queries.

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
}
