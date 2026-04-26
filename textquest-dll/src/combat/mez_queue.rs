//! Multi-target crowd control queue for enchanters (and bards).
//! Tracks mez targets with duration, auto-refreshes before break.
//!
//! [`MezTracker`] wraps [`MezQueue`] and additionally records which spawn IDs
//! have proven immune to mesmerization so rotations can permanently skip them.

use std::collections::{HashSet, VecDeque};

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

/// Combines a [`MezQueue`] with a permanent immune-mob registry.
///
/// When a mob resists mez via `CastResult::Immune`, it is added to the immune
/// set and will never be offered as a mez target again — even across resets of
/// the underlying queue. Non-immune resists are handled by the queue's normal
/// retry-decrement logic.
pub struct MezTracker {
    queue: MezQueue,
    immune: HashSet<u32>,
}

impl MezTracker {
    /// Create a new tracker backed by a queue with the given capacity.
    pub fn new(max_targets: u8) -> Self {
        Self {
            queue: MezQueue::new(max_targets),
            immune: HashSet::new(),
        }
    }

    /// Returns `true` if this spawn has been confirmed immune to mez.
    pub fn is_immune(&self, spawn_id: u32) -> bool {
        self.immune.contains(&spawn_id)
    }

    /// Add a mob to the CC queue, skipping it silently if it is immune.
    pub fn add_target(&mut self, spawn_id: u32, duration_ticks: u32, current_tick: u32) {
        if self.immune.contains(&spawn_id) {
            return;
        }
        self.queue.add_target(spawn_id, duration_ticks, current_tick);
    }

    /// Return the most-urgent refresh target, skipping any that are immune.
    pub fn next_refresh_target(&self, current_tick: u32) -> Option<u32> {
        // The queue already filters by retry count and timing.
        // Immune mobs are never added to the queue, so this is a pass-through.
        self.queue.next_refresh_target(current_tick)
    }

    /// Record a successful mez cast and refresh the timer.
    pub fn record_mez_success(&mut self, spawn_id: u32, duration_ticks: u32, current_tick: u32) {
        self.queue
            .record_mez_success(spawn_id, duration_ticks, current_tick);
    }

    /// Record a normal resist — decrement retries.
    pub fn record_mez_resist(&mut self, spawn_id: u32) {
        self.queue.record_mez_resist(spawn_id);
    }

    /// Record an immunity result: permanently remove from the queue and mark
    /// the mob so it is never targeted again.
    pub fn record_mez_immune(&mut self, spawn_id: u32) {
        self.queue.remove_target(spawn_id);
        self.immune.insert(spawn_id);
    }

    /// Remove a target (died, despawned, etc.). Does NOT clear the immune flag.
    pub fn remove_target(&mut self, spawn_id: u32) {
        self.queue.remove_target(spawn_id);
    }

    /// Prune expired targets with no retries left.
    pub fn prune_expired(&mut self, current_tick: u32) {
        self.queue.prune_expired(current_tick);
    }

    /// Reset the queue and immune set (e.g. when leaving combat or changing zone).
    pub fn reset(&mut self, max_targets: u8) {
        self.queue = MezQueue::new(max_targets);
        self.immune.clear();
    }

    /// Number of actively-tracked targets (excludes immune mobs).
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Number of permanently-immune mobs recorded this session.
    pub fn immune_count(&self) -> usize {
        self.immune.len()
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

    // ─── MezTracker tests ──────────────────────────────────────────────────────

    #[test]
    fn tracker_is_immune_false_by_default() {
        let tracker = MezTracker::new(5);
        assert!(!tracker.is_immune(1));
    }

    #[test]
    fn tracker_add_target_skips_immune() {
        let mut tracker = MezTracker::new(5);
        tracker.record_mez_immune(42);
        tracker.add_target(42, 600, 0);
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn tracker_record_immune_removes_from_queue() {
        let mut tracker = MezTracker::new(5);
        tracker.add_target(10, 600, 0);
        assert_eq!(tracker.len(), 1);
        tracker.record_mez_immune(10);
        assert_eq!(tracker.len(), 0);
        assert!(tracker.is_immune(10));
    }

    #[test]
    fn tracker_immune_mob_never_returned_as_refresh_target() {
        let mut tracker = MezTracker::new(5);
        tracker.add_target(7, 100, 0);
        tracker.record_mez_immune(7);
        // Even at tick 90 (inside refresh window), immune mob not returned
        assert!(tracker.next_refresh_target(90).is_none());
    }

    #[test]
    fn tracker_non_immune_mob_still_returned() {
        let mut tracker = MezTracker::new(5);
        tracker.add_target(1, 100, 0);
        tracker.add_target(2, 100, 0);
        tracker.record_mez_immune(1);
        // Mob 2 is still in the queue and within refresh window at tick 90
        assert_eq!(tracker.next_refresh_target(90), Some(2));
    }

    #[test]
    fn tracker_resist_still_decrements_retries() {
        let mut tracker = MezTracker::new(5);
        tracker.add_target(5, 100, 0);
        tracker.record_mez_resist(5);
        tracker.record_mez_resist(5);
        tracker.record_mez_resist(5); // 0 retries left
        // No longer returned — but NOT immune
        assert!(!tracker.is_immune(5));
        assert!(tracker.next_refresh_target(90).is_none());
    }

    #[test]
    fn tracker_immune_count() {
        let mut tracker = MezTracker::new(5);
        assert_eq!(tracker.immune_count(), 0);
        tracker.record_mez_immune(1);
        tracker.record_mez_immune(2);
        assert_eq!(tracker.immune_count(), 2);
    }

    #[test]
    fn tracker_reset_clears_queue_and_immune_set() {
        let mut tracker = MezTracker::new(5);
        tracker.add_target(1, 100, 0);
        tracker.record_mez_immune(2);
        tracker.reset(5);
        assert_eq!(tracker.len(), 0);
        assert_eq!(tracker.immune_count(), 0);
        assert!(!tracker.is_immune(2));
    }
}
