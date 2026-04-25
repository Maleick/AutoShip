//! Extended XP tracking and leveling analytics.
//!
//! Maintains a rolling window of XP samples per character and derives
//! XP/hour rate, time-to-level estimates, and level-up history.

use std::{collections::VecDeque, time::Instant};

const MAX_SAMPLES: usize = 1000;
const WINDOW_SECS: f32 = 3600.0; // 60 minutes

/// A single XP observation for one character.
pub struct XpSample {
    pub character: String,
    pub xp_pct: f32,
    pub aa_xp_pct: f32,
    pub level: u8,
    pub timestamp: Instant,
}

/// Point-in-time snapshot of a character's XP session — serializable for DB persistence.
#[derive(Debug, Clone)]
pub struct XpSessionSnapshot {
    pub character: String,
    pub start_xp_pct: f32,
    pub end_xp_pct: f32,
    pub start_aa_pct: f32,
    pub end_aa_pct: f32,
    pub start_level: u8,
    pub end_level: u8,
    /// Number of level-ups recorded in `level_history` during this session.
    pub level_ups: u32,
}

/// Tracks XP samples and computes leveling analytics.
pub struct XpTracker {
    /// Rolling window of raw samples (capped at 1000).
    pub samples: VecDeque<XpSample>,
    /// (level, time) pairs recorded each time a character gains a level.
    pub level_history: Vec<(u8, Instant)>,
}

impl XpTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self {
            samples: VecDeque::new(),
            level_history: Vec::new(),
        }
    }

    /// Record a new XP observation.
    ///
    /// If the character's level increased compared to their most recent sample,
    /// the new level is appended to `level_history`.
    pub fn record(
        &mut self,
        character: impl Into<String>,
        xp_pct: f32,
        aa_xp_pct: f32,
        level: u8,
        now: Instant,
    ) {
        let character = character.into();

        // Detect level-up by comparing against the latest sample for this character.
        let prev_level = self
            .samples
            .iter()
            .rev()
            .find(|s| s.character == character)
            .map(|s| s.level);

        if let Some(prev) = prev_level
            && level > prev
        {
            self.level_history.push((level, now));
        }

        // Enforce capacity cap.
        if self.samples.len() >= MAX_SAMPLES {
            self.samples.pop_front();
        }

        self.samples.push_back(XpSample {
            character,
            xp_pct,
            aa_xp_pct,
            level,
            timestamp: now,
        });
    }

    /// Compute XP gained per hour over the last 60 minutes for `character`.
    ///
    /// Only samples within the 60-minute window are considered.  Returns 0.0
    /// if fewer than two qualifying samples exist.
    pub fn xp_per_hour(&self, character: &str, now: Instant) -> f32 {
        let window = std::time::Duration::from_secs_f32(WINDOW_SECS);

        // Collect samples within the window, oldest first.
        let qualifying: Vec<&XpSample> = self
            .samples
            .iter()
            .filter(|s| s.character == character && now.duration_since(s.timestamp) <= window)
            .collect();

        if qualifying.len() < 2 {
            return 0.0;
        }

        let oldest = qualifying.first().expect("len >= 2");
        let newest = qualifying.last().expect("len >= 2");

        let elapsed_secs = newest
            .timestamp
            .duration_since(oldest.timestamp)
            .as_secs_f32();

        if elapsed_secs <= 0.0 {
            return 0.0;
        }

        let xp_gained = newest.xp_pct - oldest.xp_pct;
        xp_gained / elapsed_secs * WINDOW_SECS
    }

    /// Estimate seconds remaining until 100% XP at the current rate.
    ///
    /// Returns `None` when the XP/hour rate is zero (no progress observable).
    pub fn time_to_level_secs(&self, character: &str, now: Instant) -> Option<f64> {
        let rate = self.xp_per_hour(character, now);
        if rate <= 0.0 {
            return None;
        }

        // Current XP percent from the most recent sample.
        let current_xp = self
            .samples
            .iter()
            .rev()
            .find(|s| s.character == character)
            .map(|s| s.xp_pct)
            .unwrap_or(0.0);

        let remaining_pct = (100.0_f32 - current_xp).max(0.0);

        // rate is pct/hour; convert to pct/sec then divide remaining.
        let rate_per_sec = rate as f64 / WINDOW_SECS as f64;
        Some(remaining_pct as f64 / rate_per_sec)
    }

    /// Compute AA XP gained per hour over the last 60 minutes for `character`.
    pub fn aa_per_hour(&self, character: &str, now: Instant) -> f32 {
        let window = std::time::Duration::from_secs_f32(WINDOW_SECS);

        let qualifying: Vec<&XpSample> = self
            .samples
            .iter()
            .filter(|s| s.character == character && now.duration_since(s.timestamp) <= window)
            .collect();

        if qualifying.len() < 2 {
            return 0.0;
        }

        let oldest = qualifying.first().expect("len >= 2");
        let newest = qualifying.last().expect("len >= 2");

        let elapsed_secs = newest
            .timestamp
            .duration_since(oldest.timestamp)
            .as_secs_f32();

        if elapsed_secs <= 0.0 {
            return 0.0;
        }

        (newest.aa_xp_pct - oldest.aa_xp_pct) / elapsed_secs * WINDOW_SECS
    }

    /// Session snapshot from oldest→newest samples for `character`.
    ///
    /// Used for DB persistence across restarts.
    pub fn session_snapshot(&self, character: &str) -> Option<XpSessionSnapshot> {
        let char_samples: Vec<&XpSample> = self
            .samples
            .iter()
            .filter(|s| s.character == character)
            .collect();

        if char_samples.is_empty() {
            return None;
        }

        let oldest = char_samples.first().expect("non-empty");
        let newest = char_samples.last().expect("non-empty");

        Some(XpSessionSnapshot {
            character: character.to_string(),
            start_xp_pct: oldest.xp_pct,
            end_xp_pct: newest.xp_pct,
            start_aa_pct: oldest.aa_xp_pct,
            end_aa_pct: newest.aa_xp_pct,
            start_level: oldest.level,
            end_level: newest.level,
            level_ups: self.level_history.len() as u32,
        })
    }

    /// Return up to `limit` of the most recent samples for `character`.
    pub fn recent_samples(&self, character: &str, limit: usize) -> Vec<&XpSample> {
        let mut result: Vec<&XpSample> = self
            .samples
            .iter()
            .filter(|s| s.character == character)
            .collect();

        // Keep only the tail (most recent).
        if result.len() > limit {
            let skip = result.len() - limit;
            result.drain(..skip);
        }

        result
    }
}

impl Default for XpTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn make_instant_offset(base: Instant, secs: u64) -> Instant {
        base + Duration::from_secs(secs)
    }

    // ---------------------------------------------------------------------------
    // xp_per_hour
    // ---------------------------------------------------------------------------

    #[test]
    fn test_xp_per_hour_basic() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();

        // Record 10 % XP gain over 30 minutes → 20 %/hour expected.
        tracker.record("Warrior", 50.0, 0.0, 50, make_instant_offset(base, 0));
        tracker.record("Warrior", 60.0, 0.0, 50, make_instant_offset(base, 30 * 60));

        let now = make_instant_offset(base, 30 * 60);
        let rate = tracker.xp_per_hour("Warrior", now);
        // 10 % in 30 min = 20 %/hour
        let diff = (rate - 20.0_f32).abs();
        assert!(diff < 0.01, "expected ~20.0, got {rate}");
    }

    #[test]
    fn test_xp_per_hour_returns_zero_with_one_sample() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();
        tracker.record("Cleric", 25.0, 0.0, 30, base);
        let rate = tracker.xp_per_hour("Cleric", base);
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn test_xp_per_hour_ignores_samples_outside_window() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();

        // Old sample (90 minutes ago) — should be excluded.
        tracker.record("Rogue", 10.0, 0.0, 40, make_instant_offset(base, 0));
        // Recent sample (10 minutes ago).
        tracker.record("Rogue", 50.0, 0.0, 40, make_instant_offset(base, 50 * 60));
        // "now" is 90 minutes after base — first sample falls outside window.
        let now = make_instant_offset(base, 90 * 60);
        let rate = tracker.xp_per_hour("Rogue", now);
        // Only one sample in window → 0.0
        assert_eq!(rate, 0.0);
    }

    // ---------------------------------------------------------------------------
    // time_to_level_secs
    // ---------------------------------------------------------------------------

    #[test]
    fn test_time_to_level_with_positive_rate() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();

        // 50 % → 60 % over 30 minutes. Rate = 20 %/hour.
        // Remaining at second sample = 40 %. At 20 %/hour that is 2 hours = 7200 s.
        tracker.record("Wizard", 50.0, 0.0, 55, make_instant_offset(base, 0));
        tracker.record("Wizard", 60.0, 0.0, 55, make_instant_offset(base, 30 * 60));

        let now = make_instant_offset(base, 30 * 60);
        let ttl = tracker
            .time_to_level_secs("Wizard", now)
            .expect("should be Some");
        let expected = 7200.0_f64;
        let diff = (ttl - expected).abs();
        assert!(diff < 1.0, "expected ~7200 s, got {ttl}");
    }

    #[test]
    fn test_time_to_level_returns_none_at_zero_rate() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();
        // Only one sample → rate = 0 → None.
        tracker.record("Monk", 75.0, 0.0, 60, base);
        let ttl = tracker.time_to_level_secs("Monk", base);
        assert!(ttl.is_none());
    }

    // ---------------------------------------------------------------------------
    // level_history
    // ---------------------------------------------------------------------------

    #[test]
    fn test_level_history_appended_on_level_up() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();

        tracker.record("Paladin", 90.0, 0.0, 49, base);
        tracker.record("Paladin", 5.0, 0.0, 50, make_instant_offset(base, 60));

        assert_eq!(tracker.level_history.len(), 1);
        assert_eq!(tracker.level_history[0].0, 50);
    }

    #[test]
    fn test_level_history_not_appended_on_same_level() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();

        tracker.record("Bard", 10.0, 0.0, 35, base);
        tracker.record("Bard", 20.0, 0.0, 35, make_instant_offset(base, 60));

        assert!(tracker.level_history.is_empty());
    }

    // ---------------------------------------------------------------------------
    // recent_samples
    // ---------------------------------------------------------------------------

    #[test]
    fn test_recent_samples_limit_respected() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();

        for i in 0_u64..10 {
            tracker.record(
                "Shaman",
                i as f32 * 5.0,
                0.0,
                60,
                make_instant_offset(base, i * 60),
            );
        }

        let result = tracker.recent_samples("Shaman", 3);
        assert_eq!(result.len(), 3);
        // Most recent sample should have xp_pct = 45.0
        assert!((result.last().expect("non-empty").xp_pct - 45.0).abs() < 0.01);
    }

    #[test]
    fn test_recent_samples_filters_by_character() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();

        tracker.record("Enchanter", 10.0, 0.0, 50, base);
        tracker.record("Necromancer", 20.0, 0.0, 50, make_instant_offset(base, 10));

        let result = tracker.recent_samples("Enchanter", 10);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].character, "Enchanter");
    }

    #[test]
    fn test_recent_samples_fewer_than_limit() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();
        tracker.record("Druid", 50.0, 0.0, 55, base);
        let result = tracker.recent_samples("Druid", 100);
        assert_eq!(result.len(), 1);
    }

    // --- additional edge-case tests ---

    #[test]
    fn test_cap_at_max_samples() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();

        // Record more than MAX_SAMPLES (1000)
        for i in 0..1050_u64 {
            tracker.record(
                "Warrior",
                (i % 100) as f32,
                0.0,
                50,
                make_instant_offset(base, i),
            );
        }

        assert_eq!(tracker.samples.len(), 1000, "should cap at MAX_SAMPLES");
    }

    #[test]
    fn test_xp_per_hour_zero_elapsed_time() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();

        // Two samples at exact same time → elapsed = 0 → rate = 0
        tracker.record("Wizard", 10.0, 0.0, 50, base);
        tracker.record("Wizard", 20.0, 0.0, 50, base);

        let rate = tracker.xp_per_hour("Wizard", base);
        assert_eq!(rate, 0.0, "zero elapsed should return 0");
    }

    #[test]
    fn test_xp_per_hour_unknown_character() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();
        tracker.record("Warrior", 50.0, 0.0, 50, base);
        let rate = tracker.xp_per_hour("Unknown", base);
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn test_time_to_level_at_99_pct() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();

        // 98% → 99% over 30 minutes = 2%/hour rate
        // Remaining = 1% at 2%/hour = 30 minutes = 1800 seconds
        tracker.record("Ranger", 98.0, 0.0, 50, make_instant_offset(base, 0));
        tracker.record("Ranger", 99.0, 0.0, 50, make_instant_offset(base, 30 * 60));

        let now = make_instant_offset(base, 30 * 60);
        let ttl = tracker
            .time_to_level_secs("Ranger", now)
            .expect("should have TTL");
        let expected = 1800.0_f64;
        let diff = (ttl - expected).abs();
        assert!(diff < 1.0, "expected ~1800 s, got {ttl}");
    }

    #[test]
    fn test_time_to_level_at_100_pct() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();

        // 90% → 100% over 30 minutes → remaining = 0
        tracker.record("Monk", 90.0, 0.0, 50, make_instant_offset(base, 0));
        tracker.record("Monk", 100.0, 0.0, 50, make_instant_offset(base, 30 * 60));

        let now = make_instant_offset(base, 30 * 60);
        let ttl = tracker
            .time_to_level_secs("Monk", now)
            .expect("should be Some");
        assert!(ttl < 1.0, "at 100% XP, TTL should be near zero, got {ttl}");
    }

    #[test]
    fn test_level_history_multiple_level_ups() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();

        tracker.record("Paladin", 90.0, 0.0, 49, base);
        tracker.record("Paladin", 5.0, 0.0, 50, make_instant_offset(base, 60));
        tracker.record("Paladin", 95.0, 0.0, 50, make_instant_offset(base, 3600));
        tracker.record("Paladin", 5.0, 0.0, 51, make_instant_offset(base, 7200));

        assert_eq!(tracker.level_history.len(), 2);
        assert_eq!(tracker.level_history[0].0, 50);
        assert_eq!(tracker.level_history[1].0, 51);
    }

    #[test]
    fn test_level_history_not_triggered_on_delevel() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();

        tracker.record("Warrior", 10.0, 0.0, 50, base);
        // Level goes down — no level_history entry
        tracker.record("Warrior", 99.0, 0.0, 49, make_instant_offset(base, 60));

        assert!(tracker.level_history.is_empty());
    }

    #[test]
    fn test_recent_samples_empty_tracker() {
        let tracker = XpTracker::new();
        let result = tracker.recent_samples("Anyone", 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_default_trait() {
        let tracker = XpTracker::default();
        assert!(tracker.samples.is_empty());
        assert!(tracker.level_history.is_empty());
    }

    #[test]
    fn test_aa_xp_tracked() {
        let mut tracker = XpTracker::new();
        let base = Instant::now();
        tracker.record("Cleric", 50.0, 25.0, 60, base);
        let samples = tracker.recent_samples("Cleric", 1);
        assert_eq!(samples.len(), 1);
        assert!((samples[0].aa_xp_pct - 25.0).abs() < f32::EPSILON);
    }
}
