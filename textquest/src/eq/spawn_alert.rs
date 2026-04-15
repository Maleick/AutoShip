//! Spawn alert feed — pattern-matched spawn notifications and named mob alerts.
//!
//! Provides a ring buffer of `SpawnAlertEvent`s generated from two sources:
//! 1. **Named tracker** — automatic alerts for named NPCs (via `NamedTracker`)
//! 2. **Watch patterns** — user-defined glob-style patterns (`:watch *moss*`)
//!
//! Also provides `RareSpawnTracker` for tracking spawn times and calculating
//! time since last pop.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime};

/// How a spawn alert was triggered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchSource {
    /// Matched because the spawn is a named NPC (no article prefix).
    Named,
    /// Matched a user-defined watch pattern.
    WatchPattern(String),
}

/// A single spawn up/down alert event.
#[derive(Debug, Clone)]
pub struct SpawnAlertEvent {
    /// Name of the spawn.
    pub spawn_name: String,
    /// Zone where the event occurred.
    pub zone: String,
    /// `true` = spawn appeared, `false` = spawn disappeared.
    pub is_up: bool,
    /// Wall-clock time of the event.
    pub timestamp: SystemTime,
    /// Tick counter when the event was generated.
    pub tick: u64,
    /// What triggered this alert.
    pub match_source: MatchSource,
    /// Duration since the last time this spawn was seen (for UP events).
    /// `None` if this is the first time seeing this spawn.
    pub time_since_last_pop: Option<Duration>,
}

/// Pattern mode for spawn watch matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchMode {
    /// Exact case-insensitive match.
    Exact(String),
    /// `*keyword*` — substring match.
    Contains(String),
    /// `prefix*` — starts-with match.
    StartsWith(String),
    /// `*suffix` — ends-with match.
    EndsWith(String),
}

/// A user-defined spawn watch pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnWatchPattern {
    /// Original pattern string as typed by the user.
    pub raw: String,
    /// Parsed match mode.
    pub mode: WatchMode,
}

impl SpawnWatchPattern {
    /// Parse a pattern string into a `SpawnWatchPattern`.
    ///
    /// Glob-style syntax:
    /// - `*foo*` → contains "foo"
    /// - `foo*`  → starts with "foo"
    /// - `*foo`  → ends with "foo"
    /// - `foo`   → exact match
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        let trimmed = raw.trim();
        let lower = trimmed.to_lowercase();
        let mode = if lower.starts_with('*') && lower.ends_with('*') && lower.len() > 2 {
            WatchMode::Contains(lower[1..lower.len() - 1].to_string())
        } else if lower.ends_with('*') && lower.len() > 1 {
            WatchMode::StartsWith(lower[..lower.len() - 1].to_string())
        } else if lower.starts_with('*') && lower.len() > 1 {
            WatchMode::EndsWith(lower[1..].to_string())
        } else {
            WatchMode::Exact(lower)
        };
        Self {
            raw: trimmed.to_string(),
            mode,
        }
    }

    /// Test whether a spawn name matches this pattern (case-insensitive).
    #[must_use]
    pub fn matches(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        match &self.mode {
            WatchMode::Exact(s) => lower == *s,
            WatchMode::Contains(s) => lower.contains(s.as_str()),
            WatchMode::StartsWith(s) => lower.starts_with(s.as_str()),
            WatchMode::EndsWith(s) => lower.ends_with(s.as_str()),
        }
    }
}

/// Ring-buffered feed of spawn alert events with user-defined watch patterns.
pub struct SpawnAlertFeed {
    events: VecDeque<SpawnAlertEvent>,
    capacity: usize,
    watch_patterns: Vec<SpawnWatchPattern>,
    /// Spawn names currently matched by watch patterns (dedup across ticks).
    watched_seen: std::collections::HashSet<String>,
}

impl SpawnAlertFeed {
    /// Create a new feed with the given maximum capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity.min(1024)),
            capacity,
            watch_patterns: Vec::new(),
            watched_seen: std::collections::HashSet::new(),
        }
    }

    /// Push an alert event, evicting the oldest if at capacity.
    pub fn push(&mut self, event: SpawnAlertEvent) {
        if self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// All events in chronological order (oldest first).
    #[must_use]
    pub fn events(&self) -> &VecDeque<SpawnAlertEvent> {
        &self.events
    }

    /// Number of events currently in the feed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the feed is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Clear all events from the feed.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Add a watch pattern (parsed from user input).
    pub fn add_watch(&mut self, pattern: &str) -> &SpawnWatchPattern {
        self.watch_patterns.push(SpawnWatchPattern::parse(pattern));
        self.watch_patterns.last().unwrap()
    }

    /// Remove a watch pattern by its raw string. Returns `true` if found.
    pub fn remove_watch(&mut self, pattern: &str) -> bool {
        let before = self.watch_patterns.len();
        self.watch_patterns
            .retain(|p| !p.raw.eq_ignore_ascii_case(pattern.trim()));
        self.watch_patterns.len() < before
    }

    /// Track which watch-matched spawns have been seen (for dedup).
    pub fn watched_seen(&self) -> &std::collections::HashSet<String> {
        &self.watched_seen
    }

    /// Mark a spawn name as seen by watch pattern matching.
    pub fn mark_watched_seen(&mut self, name: String) {
        self.watched_seen.insert(name);
    }

    /// Remove a name from the seen set (spawn despawned).
    pub fn unmark_watched_seen(&mut self, name: &str) {
        self.watched_seen.remove(name);
    }

    /// Current watch patterns.
    #[must_use]
    pub fn watch_patterns(&self) -> &[SpawnWatchPattern] {
        &self.watch_patterns
    }

    /// Check a spawn name against all watch patterns.
    /// Returns the first matching pattern's raw string, or `None`.
    #[must_use]
    pub fn matches_any_pattern(&self, name: &str) -> Option<&str> {
        self.watch_patterns
            .iter()
            .find(|p| p.matches(name))
            .map(|p| p.raw.as_str())
    }
}

// ─── RareSpawnTracker ───────────────────────────────────────────────────────────

/// Tracks rare spawn spawn times and calculates time since last pop.
///
/// This allows tracking when a rare spawn was last seen and how long it's been
/// since it appeared. Useful for farming efficiency and respawn timing.
#[derive(Debug, Clone)]
pub struct RareSpawnTracker {
    /// Last seen time for each spawn (keyed by lowercase name).
    last_seen: HashMap<String, SystemTime>,
}

impl RareSpawnTracker {
    /// Create a new rare spawn tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            last_seen: HashMap::new(),
        }
    }

    /// Record a spawn as being seen now and return the time since last pop.
    ///
    /// Returns `None` if this is the first time seeing this spawn.
    pub fn record_spawn(&mut self, name: &str) -> Option<Duration> {
        let key = name.to_lowercase();
        let now = SystemTime::now();
        let result = self.last_seen.get(&key).and_then(|last| now.duration_since(*last));
        self.last_seen.insert(key, now);
        result
    }

    /// Get the time since the last pop for a spawn.
    ///
    /// Returns `None` if the spawn has never been recorded.
    #[must_use]
    pub fn time_since_last_pop(&self, name: &str) -> Option<Duration> {
        let key = name.to_lowercase();
        self.last_seen.get(&key).and_then(|last| SystemTime::now().duration_since(*last))
    }

    /// Clear all tracked spawn times (e.g., on zone change).
    pub fn clear(&mut self) {
        self.last_seen.clear();
    }

    /// Number of tracked spawns.
    #[must_use]
    pub fn len(&self) -> usize {
        self.last_seen.len()
    }

    /// Whether no spawns are tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.last_seen.is_empty()
    }
}

impl Default for RareSpawnTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let p = SpawnWatchPattern::parse("Emperor Crush");
        assert!(p.matches("Emperor Crush"));
        assert!(p.matches("emperor crush"));
        assert!(!p.matches("Emperor Crushbone"));
    }

    #[test]
    fn test_contains_match() {
        let p = SpawnWatchPattern::parse("*moss*");
        assert!(p.matches("a moss snake"));
        assert!(p.matches("Mossman"));
        assert!(!p.matches("a bear"));
    }

    #[test]
    fn test_starts_with_match() {
        let p = SpawnWatchPattern::parse("Emperor*");
        assert!(p.matches("Emperor Crush"));
        assert!(p.matches("emperor ssra"));
        assert!(!p.matches("The Emperor"));
    }

    #[test]
    fn test_ends_with_match() {
        let p = SpawnWatchPattern::parse("*Crush");
        assert!(p.matches("Emperor Crush"));
        assert!(p.matches("crush"));
        assert!(!p.matches("Crushbone"));
    }

    #[test]
    fn test_feed_capacity() {
        let mut feed = SpawnAlertFeed::new(3);
        for i in 0..5 {
            feed.push(SpawnAlertEvent {
                spawn_name: format!("Mob{i}"),
                zone: "zone".into(),
                is_up: true,
                timestamp: SystemTime::now(),
                tick: i,
                match_source: MatchSource::Named,
                time_since_last_pop: None,
            });
        }
        assert_eq!(feed.len(), 3);
        // Oldest two should have been evicted
        assert_eq!(feed.events().front().unwrap().spawn_name, "Mob2");
    }

    #[test]
    fn test_watch_add_remove() {
        let mut feed = SpawnAlertFeed::new(100);
        feed.add_watch("*moss*");
        feed.add_watch("Emperor Crush");
        assert_eq!(feed.watch_patterns().len(), 2);

        assert!(feed.remove_watch("*moss*"));
        assert_eq!(feed.watch_patterns().len(), 1);
        assert!(!feed.remove_watch("nonexistent"));
    }

    #[test]
    fn test_matches_any_pattern() {
        let mut feed = SpawnAlertFeed::new(100);
        feed.add_watch("*moss*");
        feed.add_watch("Emperor Crush");

        assert_eq!(feed.matches_any_pattern("a moss snake"), Some("*moss*"));
        assert_eq!(
            feed.matches_any_pattern("Emperor Crush"),
            Some("Emperor Crush")
        );
        assert_eq!(feed.matches_any_pattern("a bear"), None);
    }

    // ─── RareSpawnTracker tests ───────────────────────────────────────────────

    #[test]
    fn test_rare_spawn_tracker_first_spawn_returns_none() {
        let mut tracker = RareSpawnTracker::new();
        let result = tracker.record_spawn("Emperor Crush");
        assert!(result.is_none());
    }

    #[test]
    fn test_rare_spawn_tracker_second_spawn_returns_duration() {
        let mut tracker = RareSpawnTracker::new();
        tracker.record_spawn("Emperor Crush");
        // Small delay
        std::thread::sleep(Duration::from_millis(10));
        let result = tracker.record_spawn("Emperor Crush");
        assert!(result.is_some());
        assert!(result.unwrap() >= Duration::from_millis(10));
    }

    #[test]
    fn test_rare_spawn_tracker_case_insensitive() {
        let mut tracker = RareSpawnTracker::new();
        tracker.record_spawn("Emperor Crush");
        let result = tracker.time_since_last_pop("EMPEROR CRUSH");
        assert!(result.is_some());
    }

    #[test]
    fn test_rare_spawn_tracker_clear() {
        let mut tracker = RareSpawnTracker::new();
        tracker.record_spawn("Emperor Crush");
        assert!(!tracker.is_empty());
        tracker.clear();
        assert!(tracker.is_empty());
        assert!(tracker.time_since_last_pop("Emperor Crush").is_none());
    }

    #[test]
    fn test_rare_spawn_tracker_len() {
        let mut tracker = RareSpawnTracker::new();
        assert_eq!(tracker.len(), 0);
        tracker.record_spawn("Emperor Crush");
        assert_eq!(tracker.len(), 1);
        tracker.record_spawn("Lord Nagafen");
        assert_eq!(tracker.len(), 2);
    }
}
