//! Tracks active DoTs on targets to prevent wasteful recasts.

use std::collections::HashMap;

/// Tracks which spells are active on which targets, with expiration ticks.
pub struct DotTracker {
    /// (target_spawn_id, spell_name) -> expiration_tick
    active_dots: HashMap<(u32, String), u32>,
}

impl DotTracker {
    pub fn new() -> Self {
        Self {
            active_dots: HashMap::new(),
        }
    }

    /// Record that a DoT was applied to a target.
    pub fn record_dot(
        &mut self,
        target_id: u32,
        spell_name: &str,
        duration_ticks: u32,
        current_tick: u32,
    ) {
        self.active_dots.insert(
            (target_id, spell_name.to_string()),
            current_tick + duration_ticks,
        );
    }

    /// Check if a DoT is still active on a target.
    pub fn is_dot_active(&self, target_id: u32, spell_name: &str, current_tick: u32) -> bool {
        self.active_dots
            .get(&(target_id, spell_name.to_string()))
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
        tracker.record_dot(100, "Envenomed Bolt", 60, 10);

        assert!(tracker.is_dot_active(100, "Envenomed Bolt", 10));
        assert!(tracker.is_dot_active(100, "Envenomed Bolt", 50));
        assert!(tracker.is_dot_active(100, "Envenomed Bolt", 69));
        // Not active at or after expiry
        assert!(!tracker.is_dot_active(100, "Envenomed Bolt", 70));
        assert!(!tracker.is_dot_active(100, "Envenomed Bolt", 100));
    }

    #[test]
    fn expiry_boundary() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, "Poison", 10, 0);

        // Active at tick 9 (last tick before expiry)
        assert!(tracker.is_dot_active(1, "Poison", 9));
        // Not active at tick 10 (expiry tick)
        assert!(!tracker.is_dot_active(1, "Poison", 10));
    }

    #[test]
    fn prune_removes_expired_entries() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, "DoT A", 10, 0); // expires at 10
        tracker.record_dot(2, "DoT B", 50, 0); // expires at 50
        tracker.record_dot(3, "DoT C", 20, 0); // expires at 20

        assert_eq!(tracker.active_count(), 3);

        tracker.prune_expired(15);
        // DoT A expired (10 < 15), the other two remain
        assert_eq!(tracker.active_count(), 2);
        assert!(!tracker.is_dot_active(1, "DoT A", 15));
        assert!(tracker.is_dot_active(2, "DoT B", 15));
        assert!(tracker.is_dot_active(3, "DoT C", 15));
    }

    #[test]
    fn clear_target_removes_all_dots_for_target() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(100, "Poison", 60, 0);
        tracker.record_dot(100, "Disease", 60, 0);
        tracker.record_dot(200, "Poison", 60, 0);

        assert_eq!(tracker.active_count(), 3);

        tracker.clear_target(100);
        assert_eq!(tracker.active_count(), 1);
        assert!(!tracker.is_dot_active(100, "Poison", 0));
        assert!(!tracker.is_dot_active(100, "Disease", 0));
        assert!(tracker.is_dot_active(200, "Poison", 0));
    }

    #[test]
    fn different_targets_independent() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, "Poison", 30, 0);
        tracker.record_dot(2, "Poison", 60, 0);

        // Same spell name, different targets — tracked independently
        assert!(tracker.is_dot_active(1, "Poison", 25));
        assert!(tracker.is_dot_active(2, "Poison", 25));

        // Target 1's dot expires first
        assert!(!tracker.is_dot_active(1, "Poison", 35));
        assert!(tracker.is_dot_active(2, "Poison", 35));
    }

    #[test]
    fn unknown_dot_not_active() {
        let tracker = DotTracker::new();
        assert!(!tracker.is_dot_active(999, "NonExistent", 0));
    }

    #[test]
    fn record_overwrites_existing() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, "Poison", 10, 0); // expires at 10
        tracker.record_dot(1, "Poison", 10, 20); // refresh: expires at 30

        assert!(tracker.is_dot_active(1, "Poison", 15)); // would have expired without refresh
        assert!(tracker.is_dot_active(1, "Poison", 29));
        assert!(!tracker.is_dot_active(1, "Poison", 30));
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
        tracker.record_dot(1, "Poison", 30, 0);
        tracker.clear_target(999);
        assert_eq!(tracker.active_count(), 1);
    }

    #[test]
    fn zero_duration_dot_never_active() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, "Instant", 0, 5); // expires at 5
        assert!(!tracker.is_dot_active(1, "Instant", 5));
        assert!(!tracker.is_dot_active(1, "Instant", 6));
    }

    #[test]
    fn multiple_dots_on_same_target() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, "Poison", 30, 0);
        tracker.record_dot(1, "Disease", 60, 0);
        tracker.record_dot(1, "Fire", 10, 0);
        assert_eq!(tracker.active_count(), 3);

        // Fire expired
        assert!(!tracker.is_dot_active(1, "Fire", 15));
        assert!(tracker.is_dot_active(1, "Poison", 15));
        assert!(tracker.is_dot_active(1, "Disease", 15));
    }

    #[test]
    fn prune_removes_all_expired() {
        let mut tracker = DotTracker::new();
        tracker.record_dot(1, "A", 5, 0);
        tracker.record_dot(2, "B", 5, 0);
        tracker.record_dot(3, "C", 5, 0);
        assert_eq!(tracker.active_count(), 3);

        tracker.prune_expired(10);
        assert_eq!(tracker.active_count(), 0);
    }
}
