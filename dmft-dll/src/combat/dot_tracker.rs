//! Tracks active `DoTs` on targets to prevent wasteful recasts.

use std::collections::HashMap;

/// Tracks which spells are active on which targets, with expiration ticks.
pub struct DotTracker {
    /// (`target_spawn_id`, `spell_id`) -> `expiration_tick`
    active_dots: HashMap<(u32, i32), u32>,
}

impl DotTracker {
    pub fn new() -> Self {
        Self {
            active_dots: HashMap::new(),
        }
    }

    /// Record that a `DoT` was applied to a target.
    ///
    /// Non-positive `spell_id` values (0, -1, etc.) are ignored — they could
    /// alias unrelated `DoTs` across different targets.
    pub fn record_dot(
        &mut self,
        target_id: u32,
        spell_id: i32,
        duration_ticks: u32,
        current_tick: u32,
    ) {
        if spell_id <= 0 {
            return;
        }
        self.active_dots
            .insert((target_id, spell_id), current_tick + duration_ticks);
    }

    /// Check if a `DoT` is still active on a target.
    ///
    /// Always returns `false` for non-positive `spell_id` values.
    pub fn is_dot_active(&self, target_id: u32, spell_id: i32, current_tick: u32) -> bool {
        if spell_id <= 0 {
            return false;
        }
        self.active_dots
            .get(&(target_id, spell_id))
            .is_some_and(|&expiry| current_tick < expiry)
    }

    /// Prune expired entries to prevent unbounded growth.
    pub fn prune_expired(&mut self, current_tick: u32) {
        self.active_dots
            .retain(|_, &mut expiry| current_tick < expiry);
    }

    /// Clear all tracking for a specific target (e.g., target died).
    pub fn clear_target(&mut self, target_id: u32) {
        self.active_dots.retain(|(tid, _), _| *tid != target_id);
    }

    /// Number of active entries (for testing).
    pub fn active_count(&self) -> usize {
        self.active_dots.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_check() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(100, 1001, 60, 10);

        assert!(tracker.is_dot_active(100, 1001, 10));
        assert!(tracker.is_dot_active(100, 1001, 50));
        assert!(tracker.is_dot_active(100, 1001, 69));
        // Not active at or after expiry
        assert!(!tracker.is_dot_active(100, 1001, 70));
        assert!(!tracker.is_dot_active(100, 1001, 100));
    }

    #[test]
    fn expiry_boundary() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, 100, 10, 0);

        // Active at tick 9 (last tick before expiry)
        assert!(tracker.is_dot_active(1, 100, 9));
        // Not active at tick 10 (expiry tick)
        assert!(!tracker.is_dot_active(1, 100, 10));
    }

    #[test]
    fn prune_removes_expired_entries() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, 101, 10, 0); // expires at 10
        tracker.record_dot(2, 102, 50, 0); // expires at 50
        tracker.record_dot(3, 103, 20, 0); // expires at 20

        assert_eq!(tracker.active_count(), 3);

        tracker.prune_expired(15);
        // spell 101 expired (10 < 15), the other two remain
        assert_eq!(tracker.active_count(), 2);
        assert!(!tracker.is_dot_active(1, 101, 15));
        assert!(tracker.is_dot_active(2, 102, 15));
        assert!(tracker.is_dot_active(3, 103, 15));
    }

    #[test]
    fn clear_target_removes_all_dots_for_target() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(100, 201, 60, 0);
        tracker.record_dot(100, 202, 60, 0);
        tracker.record_dot(200, 201, 60, 0);

        assert_eq!(tracker.active_count(), 3);

        tracker.clear_target(100);
        assert_eq!(tracker.active_count(), 1);
        assert!(!tracker.is_dot_active(100, 201, 0));
        assert!(!tracker.is_dot_active(100, 202, 0));
        assert!(tracker.is_dot_active(200, 201, 0));
    }

    #[test]
    fn different_targets_independent() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, 300, 30, 0);
        tracker.record_dot(2, 300, 60, 0);

        // Same spell_id, different targets — tracked independently
        assert!(tracker.is_dot_active(1, 300, 25));
        assert!(tracker.is_dot_active(2, 300, 25));

        // Target 1's dot expires first
        assert!(!tracker.is_dot_active(1, 300, 35));
        assert!(tracker.is_dot_active(2, 300, 35));
    }

    #[test]
    fn unknown_dot_not_active() {
        let tracker = DotTracker::new();
        assert!(!tracker.is_dot_active(999, 999, 0));
    }

    #[test]
    fn record_overwrites_existing() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, 400, 10, 0); // expires at 10
        tracker.record_dot(1, 400, 10, 20); // refresh: expires at 30

        assert!(tracker.is_dot_active(1, 400, 15)); // would have expired without refresh
        assert!(tracker.is_dot_active(1, 400, 29));
        assert!(!tracker.is_dot_active(1, 400, 30));
        assert_eq!(tracker.active_count(), 1); // still only one entry
    }

    #[test]
    fn new_tracker_is_empty() {
        let tracker = DotTracker::new();
        assert_eq!(tracker.active_count(), 0);
    }

    #[test]
    fn prune_on_empty_is_noop() {
        let mut tracker = DotTracker::new();
        tracker.prune_expired(100);
        assert_eq!(tracker.active_count(), 0);
    }

    #[test]
    fn clear_nonexistent_target_is_noop() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, 500, 30, 0);
        tracker.clear_target(999);
        assert_eq!(tracker.active_count(), 1);
    }

    #[test]
    fn record_dot_ignores_zero_spell_id() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, 0, 60, 0);
        assert_eq!(tracker.active_count(), 0);
    }

    #[test]
    fn record_dot_ignores_negative_spell_id() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, -1, 60, 0);
        assert_eq!(tracker.active_count(), 0);
    }

    #[test]
    fn is_dot_active_returns_false_for_zero_spell_id() {
        let mut tracker = DotTracker::new();
        // Even if somehow an entry existed, the guard should reject it
        tracker.active_dots.insert((1, 0), 100);
        assert!(!tracker.is_dot_active(1, 0, 0));
    }

    #[test]
    fn is_dot_active_returns_false_for_negative_spell_id() {
        let mut tracker = DotTracker::new();
        tracker.active_dots.insert((1, -1), 100);
        assert!(!tracker.is_dot_active(1, -1, 0));
    }

    #[test]
    fn zero_duration_dot_never_active() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, 600, 0, 5); // expires at 5
        assert!(!tracker.is_dot_active(1, 600, 5));
        assert!(!tracker.is_dot_active(1, 600, 6));
    }

    #[test]
    fn multiple_dots_on_same_target() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, 701, 30, 0);
        tracker.record_dot(1, 702, 60, 0);
        tracker.record_dot(1, 703, 10, 0);
        assert_eq!(tracker.active_count(), 3);

        // spell 703 expired
        assert!(!tracker.is_dot_active(1, 703, 15));
        assert!(tracker.is_dot_active(1, 701, 15));
        assert!(tracker.is_dot_active(1, 702, 15));
    }

    #[test]
    fn prune_removes_all_expired() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, 801, 5, 0);
        tracker.record_dot(2, 802, 5, 0);
        tracker.record_dot(3, 803, 5, 0);
        assert_eq!(tracker.active_count(), 3);

        tracker.prune_expired(10);
        assert_eq!(tracker.active_count(), 0);
    }
}
