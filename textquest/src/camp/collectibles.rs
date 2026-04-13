//! Collectible and tribute management — OpenVanilla MQ2Collectible / MQ2TributeManager parity.
//!
//! Tracks collection quest progress and automates tribute system interactions.

/// A single collectible item within a collection set.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectibleItem {
    /// Display name of the collectible item.
    pub name: String,
    /// Whether this item has been collected.
    pub collected: bool,
    /// EQ item ID for this collectible.
    pub item_id: u32,
}

/// A named set of collectible items (one collection quest).
#[derive(Debug, Clone, PartialEq)]
pub struct CollectibleSet {
    /// Name of the collection set (e.g. "Crescent Reach Collections").
    pub name: String,
    /// All items belonging to this set.
    pub items: Vec<CollectibleItem>,
    /// Whether every item in the set has been collected.
    pub completed: bool,
}

impl CollectibleSet {
    /// Create a new, empty collectible set.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            items: Vec::new(),
            completed: false,
        }
    }

    /// Recompute `completed` based on whether all items are collected.
    fn recompute_completed(&mut self) {
        self.completed = !self.items.is_empty() && self.items.iter().all(|i| i.collected);
    }
}

/// Tracks all collection quest sets for a character.
#[derive(Debug, Default, Clone)]
pub struct CollectibleTracker {
    sets: Vec<CollectibleSet>,
}

impl CollectibleTracker {
    /// Create a new, empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new collection set.
    pub fn add_set(&mut self, set: CollectibleSet) {
        self.sets.push(set);
    }

    /// Mark a collectible item (by `item_id`) as collected across all sets.
    ///
    /// Returns `true` if at least one item was updated.
    pub fn mark_collected(&mut self, item_id: u32) -> bool {
        let mut changed = false;
        for set in &mut self.sets {
            for item in &mut set.items {
                if item.item_id == item_id && !item.collected {
                    item.collected = true;
                    changed = true;
                }
            }
            set.recompute_completed();
        }
        changed
    }

    /// Return the fraction of all items collected across all sets, in `[0.0, 1.0]`.
    ///
    /// Returns `0.0` if there are no items.
    #[must_use]
    pub fn completion_pct(&self) -> f32 {
        let total: usize = self.sets.iter().map(|s| s.items.len()).sum();
        if total == 0 {
            return 0.0;
        }
        let collected: usize = self
            .sets
            .iter()
            .flat_map(|s| s.items.iter())
            .filter(|i| i.collected)
            .count();
        collected as f32 / total as f32
    }

    /// Return references to all sets that are not yet completed.
    #[must_use]
    pub fn pending_sets(&self) -> Vec<&CollectibleSet> {
        self.sets.iter().filter(|s| !s.completed).collect()
    }

    /// Return a reference to all sets (for inspection / display).
    #[must_use]
    pub fn sets(&self) -> &[CollectibleSet] {
        &self.sets
    }
}

/// A single tribute item with its associated point value.
#[derive(Debug, Clone, PartialEq)]
pub struct TributeEntry {
    /// EQ item ID of the tribute item.
    pub item_id: u32,
    /// Number of tribute points granted by this item.
    pub points: u32,
}

/// Tracks active tribute items and accumulated points for a character.
#[derive(Debug, Default, Clone)]
pub struct TributeTracker {
    /// All tribute items currently tracked.
    pub active_tribute_items: Vec<u32>,
    /// Total tribute points accumulated.
    pub tribute_points: u32,
    /// Detailed entries (item_id → points).
    entries: Vec<TributeEntry>,
}

impl TributeTracker {
    /// Create a new, empty tracker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a tribute item and its point value.
    ///
    /// Duplicate `item_id` entries are allowed (each add accumulates points independently).
    pub fn add_tribute(&mut self, item_id: u32, points: u32) {
        self.active_tribute_items.push(item_id);
        self.tribute_points += points;
        self.entries.push(TributeEntry { item_id, points });
    }

    /// Return the total accumulated tribute points.
    #[must_use]
    pub fn total_points(&self) -> u32 {
        self.tribute_points
    }

    /// Return the detailed tribute entries.
    #[must_use]
    pub fn entries(&self) -> &[TributeEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_set(name: &str, items: &[(&str, u32)]) -> CollectibleSet {
        let mut set = CollectibleSet::new(name);
        for (item_name, item_id) in items {
            set.items.push(CollectibleItem {
                name: (*item_name).to_string(),
                collected: false,
                item_id: *item_id,
            });
        }
        set
    }

    // --- CollectibleTracker tests ---

    #[test]
    fn test_mark_collected_flips_state() {
        let mut tracker = CollectibleTracker::new();
        tracker.add_set(make_set("Set A", &[("Item 1", 100), ("Item 2", 101)]));

        let changed = tracker.mark_collected(100);
        assert!(changed, "mark_collected should return true when item is updated");

        let set = &tracker.sets()[0];
        assert!(set.items[0].collected, "item_id 100 should be marked collected");
        assert!(!set.items[1].collected, "item_id 101 should not be collected yet");
    }

    #[test]
    fn test_mark_collected_already_collected_returns_false() {
        let mut tracker = CollectibleTracker::new();
        tracker.add_set(make_set("Set A", &[("Item 1", 100)]));

        tracker.mark_collected(100);
        let changed = tracker.mark_collected(100);
        assert!(!changed, "mark_collected on already-collected item should return false");
    }

    #[test]
    fn test_mark_collected_unknown_id_returns_false() {
        let mut tracker = CollectibleTracker::new();
        tracker.add_set(make_set("Set A", &[("Item 1", 100)]));

        let changed = tracker.mark_collected(999);
        assert!(!changed, "mark_collected with unknown item_id should return false");
    }

    #[test]
    fn test_completion_pct_empty_tracker() {
        let tracker = CollectibleTracker::new();
        assert_eq!(tracker.completion_pct(), 0.0);
    }

    #[test]
    fn test_completion_pct_none_collected() {
        let mut tracker = CollectibleTracker::new();
        tracker.add_set(make_set("Set A", &[("Item 1", 100), ("Item 2", 101)]));
        assert_eq!(tracker.completion_pct(), 0.0);
    }

    #[test]
    fn test_completion_pct_partial() {
        let mut tracker = CollectibleTracker::new();
        tracker.add_set(make_set("Set A", &[("Item 1", 100), ("Item 2", 101), ("Item 3", 102), ("Item 4", 103)]));

        tracker.mark_collected(100);
        tracker.mark_collected(101);

        let pct = tracker.completion_pct();
        assert!((pct - 0.5).abs() < f32::EPSILON, "expected 0.5, got {pct}");
    }

    #[test]
    fn test_completion_pct_all_collected() {
        let mut tracker = CollectibleTracker::new();
        tracker.add_set(make_set("Set A", &[("Item 1", 100), ("Item 2", 101)]));

        tracker.mark_collected(100);
        tracker.mark_collected(101);

        assert_eq!(tracker.completion_pct(), 1.0);
    }

    #[test]
    fn test_completion_pct_across_multiple_sets() {
        let mut tracker = CollectibleTracker::new();
        tracker.add_set(make_set("Set A", &[("Item 1", 100), ("Item 2", 101)]));
        tracker.add_set(make_set("Set B", &[("Item 3", 200), ("Item 4", 201)]));

        // Collect 1 out of 4
        tracker.mark_collected(100);

        let pct = tracker.completion_pct();
        assert!((pct - 0.25).abs() < f32::EPSILON, "expected 0.25, got {pct}");
    }

    #[test]
    fn test_set_completed_flag_flips_when_all_collected() {
        let mut tracker = CollectibleTracker::new();
        tracker.add_set(make_set("Set A", &[("Item 1", 100), ("Item 2", 101)]));

        assert!(!tracker.sets()[0].completed);

        tracker.mark_collected(100);
        assert!(!tracker.sets()[0].completed, "not complete yet — only 1 of 2");

        tracker.mark_collected(101);
        assert!(tracker.sets()[0].completed, "should be complete after both items collected");
    }

    #[test]
    fn test_pending_sets_excludes_completed() {
        let mut tracker = CollectibleTracker::new();
        tracker.add_set(make_set("Set A", &[("Item 1", 100)]));
        tracker.add_set(make_set("Set B", &[("Item 2", 200), ("Item 3", 201)]));

        // Complete Set A
        tracker.mark_collected(100);

        let pending = tracker.pending_sets();
        assert_eq!(pending.len(), 1, "only Set B should be pending");
        assert_eq!(pending[0].name, "Set B");
    }

    #[test]
    fn test_pending_sets_all_pending() {
        let mut tracker = CollectibleTracker::new();
        tracker.add_set(make_set("Set A", &[("Item 1", 100)]));
        tracker.add_set(make_set("Set B", &[("Item 2", 200)]));

        let pending = tracker.pending_sets();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_pending_sets_none_pending() {
        let mut tracker = CollectibleTracker::new();
        tracker.add_set(make_set("Set A", &[("Item 1", 100)]));

        tracker.mark_collected(100);

        let pending = tracker.pending_sets();
        assert!(pending.is_empty());
    }

    #[test]
    fn test_empty_set_is_not_completed() {
        let mut tracker = CollectibleTracker::new();
        tracker.add_set(CollectibleSet::new("Empty Set"));

        // An empty set should not be marked completed
        assert!(!tracker.sets()[0].completed);
        let pending = tracker.pending_sets();
        assert_eq!(pending.len(), 1, "empty set counts as pending");
    }

    // --- TributeTracker tests ---

    #[test]
    fn test_add_tribute_accumulates_points() {
        let mut tracker = TributeTracker::new();
        tracker.add_tribute(1001, 100);
        tracker.add_tribute(1002, 250);
        tracker.add_tribute(1003, 50);

        assert_eq!(tracker.total_points(), 400);
    }

    #[test]
    fn test_add_tribute_populates_active_items() {
        let mut tracker = TributeTracker::new();
        tracker.add_tribute(1001, 100);
        tracker.add_tribute(1002, 200);

        assert_eq!(tracker.active_tribute_items, vec![1001, 1002]);
    }

    #[test]
    fn test_total_points_empty() {
        let tracker = TributeTracker::new();
        assert_eq!(tracker.total_points(), 0);
    }

    #[test]
    fn test_add_tribute_single_item() {
        let mut tracker = TributeTracker::new();
        tracker.add_tribute(5000, 999);

        assert_eq!(tracker.total_points(), 999);
        assert_eq!(tracker.active_tribute_items.len(), 1);
    }

    #[test]
    fn test_add_tribute_duplicate_item_ids() {
        let mut tracker = TributeTracker::new();
        tracker.add_tribute(1001, 100);
        tracker.add_tribute(1001, 100);

        // Both contributions count
        assert_eq!(tracker.total_points(), 200);
        assert_eq!(tracker.active_tribute_items.len(), 2);
    }

    #[test]
    fn test_tribute_entries_recorded() {
        let mut tracker = TributeTracker::new();
        tracker.add_tribute(1001, 100);
        tracker.add_tribute(1002, 250);

        let entries = tracker.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], TributeEntry { item_id: 1001, points: 100 });
        assert_eq!(entries[1], TributeEntry { item_id: 1002, points: 250 });
    }
}
