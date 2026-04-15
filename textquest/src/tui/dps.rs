//! DPS tracker — rolling-window damage-per-second calculation for group
//! members.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

/// A single damage sample recorded at a point in time.
#[derive(Debug, Clone)]
pub struct DpsSample {
    /// When the damage occurred.
    pub timestamp: Instant,
    /// Raw damage amount.
    pub damage: u64,
    /// Name of the damage source (character name).
    pub source_name: String,
}

/// Tracks DPS across multiple sources using a configurable rolling time window.
#[derive(Debug, Clone)]
pub struct DpsTracker {
    /// All damage samples within the current window.
    samples: Vec<DpsSample>,
    /// Rolling window duration (samples older than this are pruned).
    window: Duration,
}

impl DpsTracker {
    /// Create a new tracker with the given rolling window duration.
    #[must_use]
    pub fn new(window: Duration) -> Self {
        Self {
            samples: Vec::new(),
            window,
        }
    }

    /// Record a damage event from the named source.
    pub fn record(&mut self, source: &str, damage: u64) {
        self.samples.push(DpsSample {
            timestamp: Instant::now(),
            damage,
            source_name: source.to_string(),
        });
    }

    /// Prune samples that have fallen outside the rolling window.
    pub fn tick(&mut self) {
        let cutoff = Instant::now() - self.window;
        self.samples.retain(|s| s.timestamp >= cutoff);
    }

    /// Current DPS for a single source.
    #[must_use]
    pub fn dps_for(&self, source: &str) -> f64 {
        let now = Instant::now();
        let cutoff = now - self.window;
        let total: u64 = self
            .samples
            .iter()
            .filter(|s| s.source_name == source && s.timestamp >= cutoff)
            .map(|s| s.damage)
            .sum();
        total as f64 / self.window.as_secs_f64()
    }

    /// All sources and their current DPS, sorted descending.
    #[must_use]
    pub fn all_dps(&self) -> Vec<(String, f64)> {
        let now = Instant::now();
        let cutoff = now - self.window;
        let mut totals: HashMap<&str, u64> = HashMap::new();
        for s in &self.samples {
            if s.timestamp >= cutoff {
                *totals.entry(&s.source_name).or_default() += s.damage;
            }
        }
        let secs = self.window.as_secs_f64();
        let mut result: Vec<(String, f64)> = totals
            .into_iter()
            .map(|(name, dmg)| (name.to_string(), dmg as f64 / secs))
            .collect();
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        result
    }

    /// Total DPS across all sources.
    #[must_use]
    pub fn total_dps(&self) -> f64 {
        let now = Instant::now();
        let cutoff = now - self.window;
        let total: u64 = self
            .samples
            .iter()
            .filter(|s| s.timestamp >= cutoff)
            .map(|s| s.damage)
            .sum();
        total as f64 / self.window.as_secs_f64()
    }

    /// Number of active samples (for testing/debugging).
    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }
}

impl Default for DpsTracker {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

#[cfg(test)]
impl DpsTracker {
    /// Record a sample with an explicit timestamp (for deterministic tests).
    pub fn record_at(&mut self, source: &str, damage: u64, timestamp: Instant) {
        self.samples.push(DpsSample {
            timestamp,
            damage,
            source_name: source.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_window_is_30s() {
        let tracker = DpsTracker::default();
        assert_eq!(tracker.window, Duration::from_secs(30));
    }

    #[test]
    fn record_and_query_single_source() {
        let mut tracker = DpsTracker::new(Duration::from_secs(10));
        tracker.record("Warrior", 5000);
        let dps = tracker.dps_for("Warrior");
        assert!((dps - 500.0).abs() < 1.0);
    }

    #[test]
    fn unknown_source_returns_zero() {
        let tracker = DpsTracker::new(Duration::from_secs(10));
        assert_eq!(tracker.dps_for("Nobody"), 0.0);
    }

    #[test]
    fn multiple_sources_sorted_descending() {
        let mut tracker = DpsTracker::new(Duration::from_secs(10));
        tracker.record("Wizard", 10_000);
        tracker.record("Cleric", 1_000);
        tracker.record("Rogue", 7_000);
        let all = tracker.all_dps();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].0, "Wizard");
        assert_eq!(all[1].0, "Rogue");
        assert_eq!(all[2].0, "Cleric");
    }

    #[test]
    fn total_dps_sums_all_sources() {
        let mut tracker = DpsTracker::new(Duration::from_secs(10));
        tracker.record("A", 3000);
        tracker.record("B", 7000);
        let total = tracker.total_dps();
        assert!((total - 1000.0).abs() < 1.0);
    }

    #[test]
    fn expired_samples_pruned_by_tick() {
        let mut tracker = DpsTracker::new(Duration::from_secs(5));
        let old = Instant::now() - Duration::from_secs(10);
        tracker.record_at("Stale", 9999, old);
        tracker.record("Fresh", 100);
        assert_eq!(tracker.sample_count(), 2);
        tracker.tick();
        assert_eq!(tracker.sample_count(), 1);
        assert_eq!(tracker.dps_for("Stale"), 0.0);
    }

    #[test]
    fn expired_samples_excluded_from_dps() {
        let mut tracker = DpsTracker::new(Duration::from_secs(5));
        let old = Instant::now() - Duration::from_secs(10);
        tracker.record_at("Warrior", 50_000, old);
        tracker.record("Warrior", 1000);
        let dps = tracker.dps_for("Warrior");
        assert!((dps - 200.0).abs() < 1.0);
    }

    #[test]
    fn multiple_records_same_source_accumulate() {
        let mut tracker = DpsTracker::new(Duration::from_secs(10));
        tracker.record("Monk", 2000);
        tracker.record("Monk", 3000);
        let dps = tracker.dps_for("Monk");
        assert!((dps - 500.0).abs() < 1.0);
    }

    #[test]
    fn all_dps_empty_tracker() {
        let tracker = DpsTracker::new(Duration::from_secs(10));
        assert!(tracker.all_dps().is_empty());
        assert_eq!(tracker.total_dps(), 0.0);
    }

    #[test]
    fn sample_count_tracks_inserts() {
        let mut tracker = DpsTracker::default();
        assert_eq!(tracker.sample_count(), 0);
        tracker.record("A", 100);
        tracker.record("B", 200);
        assert_eq!(tracker.sample_count(), 2);
    }

    #[test]
    fn clone_is_independent() {
        let mut tracker = DpsTracker::default();
        tracker.record("X", 1000);
        let clone = tracker.clone();
        tracker.record("X", 2000);
        assert_eq!(clone.sample_count(), 1);
        assert_eq!(tracker.sample_count(), 2);
    }
}
