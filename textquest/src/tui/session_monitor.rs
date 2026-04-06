//! Session monitor — fleet overview and per-client drill-down.
//!
//! Tracks session events (connections, zone changes, deaths, level-ups, loot)
//! across all managed EQ clients and provides fleet-level summaries.

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Maximum number of events retained in the global event log.
const MAX_EVENT_LOG: usize = 1000;

// ─── Events ─────────────────────────────────────────────────────────────────

/// A discrete event observed for a specific client session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionEvent {
    /// A new client process connected.
    ClientConnected { pid: u32 },
    /// A client process disconnected.
    ClientDisconnected { pid: u32 },
    /// A client changed zones.
    ZoneChange { pid: u32, from: String, to: String },
    /// A character died.
    Death { pid: u32, name: String },
    /// A character leveled up.
    LevelUp { pid: u32, name: String, level: u32 },
    /// A loot item was acquired.
    LootDrop { pid: u32, item: String, value: u64 },
}

impl SessionEvent {
    /// Returns the PID associated with this event.
    #[must_use]
    pub fn pid(&self) -> u32 {
        match self {
            Self::ClientConnected { pid }
            | Self::ClientDisconnected { pid }
            | Self::ZoneChange { pid, .. }
            | Self::Death { pid, .. }
            | Self::LevelUp { pid, .. }
            | Self::LootDrop { pid, .. } => *pid,
        }
    }
}

// ─── Timestamped wrapper ────────────────────────────────────────────────────

/// An event paired with the instant it was recorded.
#[derive(Debug, Clone)]
pub struct TimestampedEvent {
    /// When the event was recorded.
    pub at: Instant,
    /// The event payload.
    pub event: SessionEvent,
}

// ─── Per-client session ─────────────────────────────────────────────────────

/// Tracks the session state for a single EQ client.
#[derive(Debug, Clone)]
pub struct ClientSession {
    /// OS process ID.
    pub pid: u32,
    /// Character name, if known.
    pub character_name: String,
    /// Current zone short name.
    pub zone: String,
    /// When this client first connected.
    pub connected_at: Instant,
    /// Total number of events recorded for this client.
    pub event_count: u64,
    /// Timestamp of the most recent event.
    pub last_event_at: Option<Instant>,
}

impl ClientSession {
    /// Creates a new session for the given PID.
    #[must_use]
    fn new(pid: u32, connected_at: Instant) -> Self {
        Self {
            pid,
            character_name: String::new(),
            zone: String::from("Unknown"),
            connected_at,
            event_count: 0,
            last_event_at: None,
        }
    }

    /// Returns the elapsed time since this client connected.
    #[must_use]
    pub fn uptime(&self) -> std::time::Duration {
        self.connected_at.elapsed()
    }
}

// ─── Fleet summary ──────────────────────────────────────────────────────────

/// Aggregate statistics across all active client sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetSummary {
    /// Number of currently connected clients.
    pub total_clients: usize,
    /// Average uptime across all clients, in seconds.
    pub avg_uptime_secs: f64,
    /// Total events recorded across all clients.
    pub total_events: u64,
}

// ─── Session monitor ────────────────────────────────────────────────────────

/// Central tracker for all client sessions and fleet-wide events.
#[derive(Debug)]
pub struct SessionMonitor {
    /// Per-client session state, keyed by PID.
    clients: HashMap<u32, ClientSession>,
    /// Global event log with bounded capacity.
    event_log: VecDeque<TimestampedEvent>,
    /// Maximum events to retain in the log.
    max_events: usize,
}

impl Default for SessionMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionMonitor {
    /// Creates a new session monitor with default capacity.
    #[must_use]
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            event_log: VecDeque::new(),
            max_events: MAX_EVENT_LOG,
        }
    }

    /// Creates a session monitor with a custom event log capacity.
    #[must_use]
    pub fn with_capacity(max_events: usize) -> Self {
        Self {
            clients: HashMap::new(),
            event_log: VecDeque::with_capacity(max_events),
            max_events,
        }
    }

    /// Records a session event, updating client state and the global log.
    pub fn record_event(&mut self, event: SessionEvent) {
        let now = Instant::now();
        let pid = event.pid();

        // Update client-specific state based on event type.
        match &event {
            SessionEvent::ClientConnected { pid } => {
                self.clients
                    .entry(*pid)
                    .or_insert_with(|| ClientSession::new(*pid, now));
            }
            SessionEvent::ClientDisconnected { pid } => {
                self.clients.remove(pid);
            }
            SessionEvent::ZoneChange { pid, to, .. } => {
                if let Some(session) = self.clients.get_mut(pid) {
                    session.zone = to.clone();
                }
            }
            SessionEvent::Death { pid, name } => {
                if let Some(session) = self.clients.get_mut(pid) {
                    session.character_name = name.clone();
                }
            }
            SessionEvent::LevelUp { pid, name, .. } => {
                if let Some(session) = self.clients.get_mut(pid) {
                    session.character_name = name.clone();
                }
            }
            SessionEvent::LootDrop { .. } => {}
        }

        // Bump per-client counters (for all events except disconnect).
        if let Some(session) = self.clients.get_mut(&pid) {
            session.event_count += 1;
            session.last_event_at = Some(now);
        }

        // Append to the global log, evicting oldest if at capacity.
        if self.event_log.len() >= self.max_events {
            self.event_log.pop_front();
        }
        self.event_log
            .push_back(TimestampedEvent { at: now, event });
    }

    /// Returns a reference to the session for the given PID, if it exists.
    #[must_use]
    pub fn client_summary(&self, pid: u32) -> Option<&ClientSession> {
        self.clients.get(&pid)
    }

    /// Computes an aggregate summary across all active client sessions.
    #[must_use]
    pub fn fleet_summary(&self) -> FleetSummary {
        let total_clients = self.clients.len();
        let total_events: u64 = self.clients.values().map(|c| c.event_count).sum();
        let avg_uptime_secs = if total_clients == 0 {
            0.0
        } else {
            let total_secs: f64 = self
                .clients
                .values()
                .map(|c| c.uptime().as_secs_f64())
                .sum();
            total_secs / total_clients as f64
        };

        FleetSummary {
            total_clients,
            avg_uptime_secs,
            total_events,
        }
    }

    /// Returns the N most recent events from the global log.
    #[must_use]
    pub fn recent_events(&self, n: usize) -> Vec<&TimestampedEvent> {
        self.event_log.iter().rev().take(n).collect()
    }

    /// Returns the number of currently connected clients.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Returns the total number of events in the log.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.event_log.len()
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_connect_and_summary() {
        let mut monitor = SessionMonitor::new();
        monitor.record_event(SessionEvent::ClientConnected { pid: 100 });

        let session = monitor.client_summary(100).expect("client should exist");
        assert_eq!(session.pid, 100);
        assert_eq!(session.event_count, 1);
        assert!(session.last_event_at.is_some());
        assert_eq!(monitor.client_count(), 1);
    }

    #[test]
    fn test_client_disconnect_removes_session() {
        let mut monitor = SessionMonitor::new();
        monitor.record_event(SessionEvent::ClientConnected { pid: 100 });
        monitor.record_event(SessionEvent::ClientDisconnected { pid: 100 });

        assert!(monitor.client_summary(100).is_none());
        assert_eq!(monitor.client_count(), 0);
    }

    #[test]
    fn test_zone_change_updates_zone() {
        let mut monitor = SessionMonitor::new();
        monitor.record_event(SessionEvent::ClientConnected { pid: 200 });
        monitor.record_event(SessionEvent::ZoneChange {
            pid: 200,
            from: "poknowledge".into(),
            to: "potranquility".into(),
        });

        let session = monitor.client_summary(200).unwrap();
        assert_eq!(session.zone, "potranquility");
        assert_eq!(session.event_count, 2);
    }

    #[test]
    fn test_death_sets_character_name() {
        let mut monitor = SessionMonitor::new();
        monitor.record_event(SessionEvent::ClientConnected { pid: 300 });
        monitor.record_event(SessionEvent::Death {
            pid: 300,
            name: "Xegony".into(),
        });

        let session = monitor.client_summary(300).unwrap();
        assert_eq!(session.character_name, "Xegony");
    }

    #[test]
    fn test_level_up_sets_name() {
        let mut monitor = SessionMonitor::new();
        monitor.record_event(SessionEvent::ClientConnected { pid: 400 });
        monitor.record_event(SessionEvent::LevelUp {
            pid: 400,
            name: "Bristle".into(),
            level: 65,
        });

        let session = monitor.client_summary(400).unwrap();
        assert_eq!(session.character_name, "Bristle");
    }

    #[test]
    fn test_fleet_summary_multiple_clients() {
        let mut monitor = SessionMonitor::new();
        monitor.record_event(SessionEvent::ClientConnected { pid: 1 });
        monitor.record_event(SessionEvent::ClientConnected { pid: 2 });
        monitor.record_event(SessionEvent::ClientConnected { pid: 3 });

        monitor.record_event(SessionEvent::LootDrop {
            pid: 1,
            item: "Kunark Gem".into(),
            value: 500,
        });
        monitor.record_event(SessionEvent::Death {
            pid: 2,
            name: "Tank".into(),
        });

        let summary = monitor.fleet_summary();
        assert_eq!(summary.total_clients, 3);
        assert_eq!(summary.total_events, 5);
        assert!(summary.avg_uptime_secs >= 0.0);
    }

    #[test]
    fn test_fleet_summary_empty() {
        let monitor = SessionMonitor::new();
        let summary = monitor.fleet_summary();
        assert_eq!(summary.total_clients, 0);
        assert_eq!(summary.total_events, 0);
        assert!((summary.avg_uptime_secs - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_recent_events_ordering() {
        let mut monitor = SessionMonitor::new();
        monitor.record_event(SessionEvent::ClientConnected { pid: 10 });
        monitor.record_event(SessionEvent::ClientConnected { pid: 20 });
        monitor.record_event(SessionEvent::ClientConnected { pid: 30 });

        let recent = monitor.recent_events(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].event.pid(), 30);
        assert_eq!(recent[1].event.pid(), 20);
    }

    #[test]
    fn test_recent_events_more_than_available() {
        let mut monitor = SessionMonitor::new();
        monitor.record_event(SessionEvent::ClientConnected { pid: 1 });

        let recent = monitor.recent_events(100);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn test_event_log_capacity_limit() {
        let mut monitor = SessionMonitor::with_capacity(3);
        monitor.record_event(SessionEvent::ClientConnected { pid: 1 });
        monitor.record_event(SessionEvent::ClientConnected { pid: 2 });
        monitor.record_event(SessionEvent::ClientConnected { pid: 3 });
        monitor.record_event(SessionEvent::ClientConnected { pid: 4 });

        assert_eq!(monitor.event_count(), 3);
        let recent = monitor.recent_events(10);
        assert_eq!(recent[0].event.pid(), 4);
        assert_eq!(recent[2].event.pid(), 2);
    }

    #[test]
    fn test_loot_drop_does_not_crash_without_session() {
        let mut monitor = SessionMonitor::new();
        monitor.record_event(SessionEvent::LootDrop {
            pid: 999,
            item: "Torn Page".into(),
            value: 100,
        });
        assert!(monitor.client_summary(999).is_none());
        assert_eq!(monitor.event_count(), 1);
    }

    #[test]
    fn test_session_event_pid_helper() {
        let event = SessionEvent::ZoneChange {
            pid: 42,
            from: "a".into(),
            to: "b".into(),
        };
        assert_eq!(event.pid(), 42);
    }

    #[test]
    fn test_uptime_is_nonnegative() {
        let mut monitor = SessionMonitor::new();
        monitor.record_event(SessionEvent::ClientConnected { pid: 1 });
        let session = monitor.client_summary(1).unwrap();
        assert!(session.uptime().as_secs_f64() >= 0.0);
    }
}
