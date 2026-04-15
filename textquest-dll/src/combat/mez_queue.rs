//! Multi-target crowd control queue for enchanters (and bards).
//! Tracks mez targets with duration, auto-refreshes before break.

use std::collections::VecDeque;

/// A mob being tracked for crowd control.
#[derive(Debug, Clone)]
pub struct MezTarget {
    pub spawn_id: u32,
    /// Tick when the mez expires (needs refresh before this).
    pub expires_at: u32,
    /// How many ticks before expiry to start refresh cast.
    pub refresh_buffer: u32,
    /// Number of resist retries remaining.
    pub retries_left: u8,
}

/// Manages a queue of mobs that need to be mezzed/CC'd.
pub struct MezQueue {
    targets: VecDeque<MezTarget>,
    /// Maximum number of CC targets to track.
    max_targets: u8,
}

impl MezQueue {
    pub fn new(max_targets: u8) -> Self {
        Self {
            targets: VecDeque::new(),
            max_targets,
        }
    }

    /// Add a new mob to the CC queue.
    pub fn add_target(&mut self, spawn_id: u32, duration_ticks: u32, current_tick: u32) {
        // Don't add duplicates
        if self.targets.iter().any(|t| t.spawn_id == spawn_id) {
            return;
        }
        // Evict oldest if at capacity
        if self.targets.len() >= self.max_targets as usize {
            self.targets.pop_back();
        }
        self.targets.push_front(MezTarget {
            spawn_id,
            expires_at: current_tick + duration_ticks,
            refresh_buffer: 40, // ~2 seconds before break, start recasting
            retries_left: 3,
        });
    }

    /// Get the next target that needs a mez refresh (or initial mez).
    /// Returns the `spawn_id` of the mob that needs CC most urgently.
    pub fn next_refresh_target(&self, current_tick: u32) -> Option<u32> {
        self.targets
            .iter()
            .filter(|t| t.retries_left > 0)
            .filter(|t| {
                // Needs refresh if within buffer window or already expired
                current_tick + t.refresh_buffer >= t.expires_at
            })
            .min_by_key(|t| t.expires_at) // Most urgent first
            .map(|t| t.spawn_id)
    }

    /// Record a successful mez cast -- refresh the timer.
    pub fn record_mez_success(&mut self, spawn_id: u32, duration_ticks: u32, current_tick: u32) {
        if let Some(target) = self.targets.iter_mut().find(|t| t.spawn_id == spawn_id) {
            target.expires_at = current_tick + duration_ticks;
            target.retries_left = 3; // Reset retries
        }
    }

    /// Record a mez resist -- decrement retries.
    pub fn record_mez_resist(&mut self, spawn_id: u32) {
        if let Some(target) = self.targets.iter_mut().find(|t| t.spawn_id == spawn_id) {
            target.retries_left = target.retries_left.saturating_sub(1);
        }
    }

    /// Remove a target (died, despawned, etc.).
    pub fn remove_target(&mut self, spawn_id: u32) {
        self.targets.retain(|t| t.spawn_id != spawn_id);
    }

    /// Prune expired targets with no retries left.
    pub fn prune_expired(&mut self, current_tick: u32) {
        self.targets
            .retain(|t| t.retries_left > 0 || current_tick < t.expires_at);
    }

    /// Number of tracked targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_target_basic() {
        let mut queue = MezQueue::new(5);
        queue.add_target(100, 600, 0);

        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());
    }

    #[test]
    fn no_duplicates() {
        let mut queue = MezQueue::new(5);
        queue.add_target(100, 600, 0);
        queue.add_target(100, 600, 10); // duplicate

        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn capacity_eviction() {
        let mut queue = MezQueue::new(2);
        queue.add_target(1, 600, 0);
        queue.add_target(2, 600, 10);
        // At capacity — adding a third should evict the oldest (1)
        queue.add_target(3, 600, 20);

        assert_eq!(queue.len(), 2);
        // Target 1 was evicted (oldest, at back of deque)
        // Targets 2 and 3 remain
        queue.remove_target(1);
        assert_eq!(queue.len(), 2); // removing 1 has no effect — already gone
        queue.remove_target(2);
        assert_eq!(queue.len(), 1); // 2 was still present
        queue.remove_target(3);
        assert_eq!(queue.len(), 0); // 3 was still present
    }

    #[test]
    fn next_refresh_target_timing() {
        let mut queue = MezQueue::new(5);
        // Target A: expires at tick 100
        queue.add_target(1, 100, 0);
        // Target B: expires at tick 200
        queue.add_target(2, 200, 0);

        // At tick 0, neither is in the refresh window (buffer=40)
        assert!(queue.next_refresh_target(0).is_none());

        // At tick 60, target 1 is in refresh window (60 + 40 >= 100)
        assert_eq!(queue.next_refresh_target(60), Some(1));

        // At tick 160, both are in refresh window — target 1 is more urgent
        assert_eq!(queue.next_refresh_target(160), Some(1));
    }

    #[test]
    fn next_refresh_returns_most_urgent() {
        let mut queue = MezQueue::new(5);
        queue.add_target(1, 200, 0); // expires at 200
        queue.add_target(2, 100, 0); // expires at 100

        // At tick 80, target 2 is in refresh window (80+40>=100), target 1 is not
        // (80+40<200)
        assert_eq!(queue.next_refresh_target(80), Some(2));
    }

    #[test]
    fn record_success_refreshes_timer() {
        let mut queue = MezQueue::new(5);
        queue.add_target(1, 100, 0); // expires at 100

        // Refresh at tick 70 with a 100-tick mez
        queue.record_mez_success(1, 100, 70); // now expires at 170

        // At tick 60, was in refresh window before refresh, now it should NOT be
        // (60 + 40 = 100 < 170)
        assert!(queue.next_refresh_target(60).is_none());

        // At tick 130, now in refresh window again (130 + 40 >= 170)
        assert_eq!(queue.next_refresh_target(130), Some(1));
    }

    #[test]
    fn record_resist_decrements_retries() {
        let mut queue = MezQueue::new(5);
        queue.add_target(1, 100, 0);

        queue.record_mez_resist(1); // 2 left
        queue.record_mez_resist(1); // 1 left
        queue.record_mez_resist(1); // 0 left

        // With 0 retries, should not be returned as a refresh target
        assert!(queue.next_refresh_target(90).is_none());
    }

    #[test]
    fn record_success_resets_retries() {
        let mut queue = MezQueue::new(5);
        queue.add_target(1, 100, 0);

        queue.record_mez_resist(1);
        queue.record_mez_resist(1); // 1 retry left

        // Successful cast resets retries to 3
        queue.record_mez_success(1, 100, 50);

        queue.record_mez_resist(1); // 2 left
        queue.record_mez_resist(1); // 1 left

        // Still has retries, should appear
        assert_eq!(queue.next_refresh_target(120), Some(1));
    }

    #[test]
    fn remove_target_works() {
        let mut queue = MezQueue::new(5);
        queue.add_target(1, 100, 0);
        queue.add_target(2, 100, 0);

        queue.remove_target(1);
        assert_eq!(queue.len(), 1);

        // Removing again is a no-op
        queue.remove_target(1);
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn prune_expired_with_no_retries() {
        let mut queue = MezQueue::new(5);
        queue.add_target(1, 50, 0); // expires at 50
        queue.add_target(2, 100, 0); // expires at 100

        // Exhaust retries on target 1
        queue.record_mez_resist(1);
        queue.record_mez_resist(1);
        queue.record_mez_resist(1);

        // At tick 60: target 1 is expired AND has no retries -> pruned
        // target 2 still has retries and is not expired -> kept
        queue.prune_expired(60);
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn prune_keeps_expired_with_retries() {
        let mut queue = MezQueue::new(5);
        queue.add_target(1, 50, 0); // expires at 50

        // At tick 60: target is expired but still has retries (needs re-mez)
        queue.prune_expired(60);
        assert_eq!(queue.len(), 1); // kept because retries > 0
    }

    #[test]
    fn empty_queue() {
        let queue = MezQueue::new(5);
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert!(queue.next_refresh_target(0).is_none());
    }
}
