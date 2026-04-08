//! Structured combat event tracking — DPS meters, kill counts, damage aggregation.
//!
//! Captures individual combat events (melee, spell, DoT, etc.) into a bounded ring
//! buffer and provides time-windowed queries for per-source DPS and kill tracking.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Default maximum events retained in the buffer before oldest are evicted.
const DEFAULT_MAX_CAPACITY: usize = 10_000;

/// Classification of damage sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DamageType {
    /// Standard melee hit (slash, pierce, crush, etc.).
    Melee,
    /// Direct-damage spell.
    Spell,
    /// Damage-over-time tick.
    DoT,
    /// Weapon proc or augment proc.
    Proc,
    /// Riposte damage (defensive counter-attack).
    Riposte,
    /// Strikethrough damage (bypasses avoidance).
    Strikethrough,
}

/// A single combat damage event.
#[derive(Debug, Clone)]
pub struct CombatEvent {
    /// When the event occurred.
    pub timestamp: Instant,
    /// Spawn ID of the damage source.
    pub source_id: u32,
    /// Spawn ID of the damage target.
    pub target_id: u32,
    /// Damage amount.
    pub damage: u32,
    /// How the damage was dealt.
    pub damage_type: DamageType,
    /// Spell or ability name, if applicable.
    pub spell_name: Option<String>,
    /// Whether this was a critical hit.
    pub is_critical: bool,
    /// Whether this event resulted in a kill.
    pub is_kill: bool,
}

/// Bounded ring buffer of combat events with time-windowed aggregation queries.
#[derive(Debug)]
pub struct CombatEventBuffer {
    events: VecDeque<CombatEvent>,
    max_capacity: usize,
}

impl CombatEventBuffer {
    /// Create a new buffer with the default capacity (10,000 events).
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
            max_capacity: DEFAULT_MAX_CAPACITY,
        }
    }

    /// Create a new buffer with a custom maximum capacity.
    #[must_use]
    pub fn with_capacity(max_capacity: usize) -> Self {
        Self {
            events: VecDeque::new(),
            max_capacity,
        }
    }

    /// Push a combat event, evicting the oldest if at capacity.
    pub fn push(&mut self, event: CombatEvent) {
        if self.events.len() >= self.max_capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// Iterate over events that occurred at or after `since`.
    pub fn events_since(&self, since: Instant) -> impl Iterator<Item = &CombatEvent> {
        self.events.iter().filter(move |e| e.timestamp >= since)
    }

    /// Total damage dealt by each source within the given time window.
    #[must_use]
    pub fn damage_by_source(&self, window: Duration) -> HashMap<u32, u64> {
        let cutoff = Instant::now() - window;
        let mut totals: HashMap<u32, u64> = HashMap::new();
        for event in self.events_since(cutoff) {
            *totals.entry(event.source_id).or_default() += u64::from(event.damage);
        }
        totals
    }

    /// DPS (damage per second) for each source within the given time window.
    #[must_use]
    pub fn dps_by_source(&self, window: Duration) -> HashMap<u32, f64> {
        let seconds = window.as_secs_f64();
        if seconds <= 0.0 {
            return HashMap::new();
        }
        self.damage_by_source(window)
            .into_iter()
            .map(|(id, dmg)| (id, dmg as f64 / seconds))
            .collect()
    }

    /// Count kills attributed to `source_id` within the given time window.
    #[must_use]
    pub fn kill_count(&self, source_id: u32, window: Duration) -> u32 {
        let cutoff = Instant::now() - window;
        self.events_since(cutoff)
            .filter(|e| e.source_id == source_id && e.is_kill)
            .count() as u32
    }

    /// Remove all events from the buffer.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Number of events currently in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the buffer contains no events.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Default for CombatEventBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(source_id: u32, damage: u32, damage_type: DamageType) -> CombatEvent {
        CombatEvent {
            timestamp: Instant::now(),
            source_id,
            target_id: 999,
            damage,
            damage_type,
            spell_name: None,
            is_critical: false,
            is_kill: false,
        }
    }

    fn make_event_at(
        timestamp: Instant,
        source_id: u32,
        damage: u32,
        is_kill: bool,
    ) -> CombatEvent {
        CombatEvent {
            timestamp,
            source_id,
            target_id: 999,
            damage,
            damage_type: DamageType::Melee,
            spell_name: None,
            is_critical: false,
            is_kill,
        }
    }

    #[test]
    fn push_and_len() {
        let mut buf = CombatEventBuffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);

        buf.push(make_event(1, 100, DamageType::Melee));
        assert_eq!(buf.len(), 1);
        assert!(!buf.is_empty());
    }

    #[test]
    fn capacity_eviction() {
        let mut buf = CombatEventBuffer::with_capacity(3);
        buf.push(make_event(1, 10, DamageType::Melee));
        buf.push(make_event(2, 20, DamageType::Spell));
        buf.push(make_event(3, 30, DamageType::DoT));
        assert_eq!(buf.len(), 3);

        buf.push(make_event(4, 40, DamageType::Proc));
        assert_eq!(buf.len(), 3);

        let first = buf.events.front().unwrap();
        assert_eq!(first.source_id, 2);
    }

    #[test]
    fn events_since_filters_correctly() {
        let mut buf = CombatEventBuffer::new();
        let old = Instant::now() - Duration::from_secs(10);
        let recent = Instant::now() - Duration::from_secs(1);

        buf.push(make_event_at(old, 1, 50, false));
        buf.push(make_event_at(recent, 2, 75, false));

        let cutoff = Instant::now() - Duration::from_secs(5);
        let events: Vec<_> = buf.events_since(cutoff).collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].source_id, 2);
    }

    #[test]
    fn damage_by_source_aggregation() {
        let mut buf = CombatEventBuffer::new();
        let now = Instant::now();

        buf.push(make_event_at(now, 1, 100, false));
        buf.push(make_event_at(now, 1, 200, false));
        buf.push(make_event_at(now, 2, 150, false));

        let totals = buf.damage_by_source(Duration::from_secs(5));
        assert_eq!(totals[&1], 300);
        assert_eq!(totals[&2], 150);
    }

    #[test]
    fn dps_by_source_divides_by_window() {
        let mut buf = CombatEventBuffer::new();
        let now = Instant::now();

        buf.push(make_event_at(now, 1, 600, false));

        let dps = buf.dps_by_source(Duration::from_secs(6));
        let source_dps = dps[&1];
        assert!((source_dps - 100.0).abs() < 0.01);
    }

    #[test]
    fn dps_zero_window_returns_empty() {
        let mut buf = CombatEventBuffer::new();
        buf.push(make_event(1, 100, DamageType::Melee));

        let dps = buf.dps_by_source(Duration::ZERO);
        assert!(dps.is_empty());
    }

    #[test]
    fn kill_count_tracks_kills() {
        let mut buf = CombatEventBuffer::new();
        let now = Instant::now();

        buf.push(make_event_at(now, 1, 100, true));
        buf.push(make_event_at(now, 1, 200, false));
        buf.push(make_event_at(now, 1, 300, true));
        buf.push(make_event_at(now, 2, 400, true));

        assert_eq!(buf.kill_count(1, Duration::from_secs(5)), 2);
        assert_eq!(buf.kill_count(2, Duration::from_secs(5)), 1);
        assert_eq!(buf.kill_count(3, Duration::from_secs(5)), 0);
    }

    #[test]
    fn clear_empties_buffer() {
        let mut buf = CombatEventBuffer::new();
        buf.push(make_event(1, 100, DamageType::Melee));
        buf.push(make_event(2, 200, DamageType::Spell));
        assert_eq!(buf.len(), 2);

        buf.clear();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn default_impl_matches_new() {
        let buf = CombatEventBuffer::default();
        assert!(buf.is_empty());
        assert_eq!(buf.max_capacity, DEFAULT_MAX_CAPACITY);
    }

    #[test]
    fn spell_name_and_critical() {
        let event = CombatEvent {
            timestamp: Instant::now(),
            source_id: 1,
            target_id: 2,
            damage: 5000,
            damage_type: DamageType::Spell,
            spell_name: Some("Ice Comet".to_string()),
            is_critical: true,
            is_kill: false,
        };
        assert_eq!(event.spell_name.as_deref(), Some("Ice Comet"));
        assert!(event.is_critical);
        assert_eq!(event.damage_type, DamageType::Spell);
    }

    #[test]
    fn damage_type_variants() {
        let types = [
            DamageType::Melee,
            DamageType::Spell,
            DamageType::DoT,
            DamageType::Proc,
            DamageType::Riposte,
            DamageType::Strikethrough,
        ];
        let mut set = std::collections::HashSet::new();
        for t in &types {
            assert!(set.insert(t));
        }
        assert_eq!(set.len(), 6);
    }

    #[test]
    fn old_events_excluded_from_aggregation() {
        let mut buf = CombatEventBuffer::new();
        let old = Instant::now() - Duration::from_secs(60);
        let recent = Instant::now();

        buf.push(make_event_at(old, 1, 1000, true));
        buf.push(make_event_at(recent, 1, 100, false));

        let totals = buf.damage_by_source(Duration::from_secs(10));
        assert_eq!(totals[&1], 100);
        assert_eq!(buf.kill_count(1, Duration::from_secs(10)), 0);
    }
}
