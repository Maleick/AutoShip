//! Debuff tracking system for monitoring active debuffs per character.
//!
//! The DebuffTracker maintains a record of all active debuffs on a character,
//! tracking timestamps, predicted expiry times, and priority rankings.
//! This enables the combat system to make informed decisions about dispel/cure priorities.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::combat::debuffs::{DebuffEntry, DebuffType};
use textquest_common::combat::BuffInfo;

#[cfg(test)]
use crate::combat::debuffs::lookup_debuff;

/// Priority ranking for debuff severity.
///
/// Lower numeric values = higher priority (should be removed first).
/// This ranking is used to sort debuffs by urgency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DebuffPriority {
    /// Stun effects: highest priority (priority = 1)
    Stun = 1,
    /// Root effects: second highest (priority = 2)
    Root = 2,
    /// Snare effects: third (priority = 3)
    Snare = 3,
    /// Slow effects: moderate (priority = 4)
    Slow = 4,
    /// Other crowd control (mez, charm): priority = 5
    OtherCC = 5,
    /// Curses: high priority (priority = 6)
    Curse = 6,
    /// Diseases: moderate priority (priority = 7)
    Disease = 7,
    /// Poisons: moderate priority (priority = 8)
    Poison = 8,
    /// Unknown or low-priority debuffs: lowest (priority = 9)
    Unknown = 9,
}

impl DebuffPriority {
    /// Get the numeric priority value (lower = more urgent).
    #[must_use]
    pub fn value(&self) -> u8 {
        *self as u8
    }

    /// Get a human-readable name.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            DebuffPriority::Stun => "Stun",
            DebuffPriority::Root => "Root",
            DebuffPriority::Snare => "Snare",
            DebuffPriority::Slow => "Slow",
            DebuffPriority::OtherCC => "Other CC",
            DebuffPriority::Curse => "Curse",
            DebuffPriority::Disease => "Disease",
            DebuffPriority::Poison => "Poison",
            DebuffPriority::Unknown => "Unknown",
        }
    }
}

/// A tracked debuff with timestamp and expiry prediction.
///
/// Combines debuff metadata from the static database with timestamp information
/// to track when the debuff was applied and when it will expire.
#[derive(Debug, Clone)]
pub struct TrackedDebuff {
    /// Reference to the static debuff entry
    pub entry: &'static DebuffEntry,
    /// Unix timestamp when the debuff was detected (seconds)
    pub applied_at: u64,
    /// Predicted expiry timestamp (seconds), or 0 if permanent
    pub expires_at: u64,
    /// Priority ranking for removal
    pub priority: DebuffPriority,
}

impl TrackedDebuff {
    /// Create a new tracked debuff from a BuffInfo.
    ///
    /// # Arguments
    /// * `entry` - Static debuff database entry
    /// * `buff_info` - Current buff info from EQ UI
    /// * `now` - Current Unix timestamp (seconds)
    pub fn new(entry: &'static DebuffEntry, buff_info: &BuffInfo, now: u64) -> Self {
        let priority = Self::rank_priority(entry);

        // Calculate expiry: add duration to current time
        // EQ ticks are typically 6 seconds per server tick
        let duration_seconds = if entry.duration_seconds > 0 {
            (buff_info.duration_ticks as u64) * 6
        } else {
            0 // permanent
        };

        let expires_at = if duration_seconds > 0 {
            now.saturating_add(duration_seconds)
        } else {
            0
        };

        TrackedDebuff {
            entry,
            applied_at: now,
            expires_at,
            priority,
        }
    }

    /// Determine priority ranking for a debuff.
    fn rank_priority(entry: &'static DebuffEntry) -> DebuffPriority {
        // Match based on debuff name patterns and type
        let name_lower = entry.name.to_lowercase();

        // Stun detection
        if name_lower.contains("stun") || name_lower.contains("paralyze") {
            return DebuffPriority::Stun;
        }

        // Root detection
        if name_lower.contains("root") || name_lower.contains("entangle") {
            return DebuffPriority::Root;
        }

        // Snare detection
        if name_lower.contains("snare")
            || name_lower.contains("hamstring")
            || name_lower.contains("ensnare")
        {
            return DebuffPriority::Snare;
        }

        // Slow detection
        if name_lower.contains("slow") {
            return DebuffPriority::Slow;
        }

        // Mez/Charm detection
        if name_lower.contains("mez") || name_lower.contains("charm") {
            return DebuffPriority::OtherCC;
        }

        // Type-based priority
        match entry.debuff_type {
            DebuffType::Curse => DebuffPriority::Curse,
            DebuffType::Disease => DebuffPriority::Disease,
            DebuffType::Poison => DebuffPriority::Poison,
            DebuffType::CrowdControl => DebuffPriority::OtherCC,
        }
    }

    /// Check if this debuff has expired.
    pub fn is_expired(&self, now: u64) -> bool {
        if self.expires_at == 0 {
            return false; // permanent, never expires
        }
        now >= self.expires_at
    }

    /// Get remaining duration in seconds, or None if expired/permanent.
    pub fn remaining_seconds(&self, now: u64) -> Option<u64> {
        if self.expires_at == 0 {
            return None; // permanent
        }
        if now >= self.expires_at {
            return Some(0); // expired
        }
        Some(self.expires_at - now)
    }
}

/// Tracks all active debuffs for a single character.
///
/// Manages a collection of debuffs, automatically removing expired ones,
/// and providing utilities for querying, filtering, and ranking debuffs.
#[derive(Debug, Clone)]
pub struct DebuffTracker {
    /// Active debuffs indexed by spell ID
    debuffs: HashMap<i32, TrackedDebuff>,
}

impl DebuffTracker {
    /// Create a new empty debuff tracker.
    pub fn new() -> Self {
        DebuffTracker {
            debuffs: HashMap::new(),
        }
    }

    /// Update the tracker with current buff info from EQ.
    ///
    /// Adds new debuffs, updates existing ones, and removes expired debuffs.
    /// This should be called every game tick or when buff data changes.
    pub fn update(&mut self, buffs: &[BuffInfo], now: u64) {
        // Mark all current debuffs as potentially removed
        let mut active_spell_ids: std::collections::HashSet<i32> = std::collections::HashSet::new();

        // Process all buffs and add/update known debuffs
        for buff in buffs {
            if let Some(entry) = crate::combat::debuffs::lookup_debuff(buff.spell_id) {
                active_spell_ids.insert(buff.spell_id);
                // Add or update this debuff
                self.debuffs
                    .entry(buff.spell_id)
                    .and_modify(|d| {
                        d.expires_at = if entry.duration_seconds > 0 {
                            now.saturating_add((buff.duration_ticks as u64) * 6)
                        } else {
                            0
                        };
                    })
                    .or_insert_with(|| TrackedDebuff::new(entry, buff, now));
            }
        }

        // Remove debuffs that are no longer in the active buff list
        self.debuffs
            .retain(|spell_id, _| active_spell_ids.contains(spell_id));

        // Also remove expired debuffs
        self.debuffs.retain(|_, d| !d.is_expired(now));
    }

    /// Get all active debuffs, sorted by priority (highest first).
    pub fn get_sorted_by_priority(&self, now: u64) -> Vec<&TrackedDebuff> {
        let mut debuffs: Vec<_> = self
            .debuffs
            .values()
            .filter(|d| !d.is_expired(now))
            .collect();
        debuffs.sort_by_key(|d| d.priority);
        debuffs
    }

    /// Get all debuffs of a specific type.
    pub fn get_by_type(&self, debuff_type: DebuffType, now: u64) -> Vec<&TrackedDebuff> {
        self.debuffs
            .values()
            .filter(|d| !d.is_expired(now) && d.entry.debuff_type == debuff_type)
            .collect()
    }

    /// Get all debuffs with a specific priority.
    pub fn get_by_priority(&self, priority: DebuffPriority, now: u64) -> Vec<&TrackedDebuff> {
        self.debuffs
            .values()
            .filter(|d| !d.is_expired(now) && d.priority == priority)
            .collect()
    }

    /// Get the most urgent debuff that needs removal.
    pub fn most_urgent(&self, now: u64) -> Option<&TrackedDebuff> {
        self.get_sorted_by_priority(now).first().copied()
    }

    /// Check if a specific debuff (by spell ID) is active.
    pub fn has_debuff(&self, spell_id: i32, now: u64) -> bool {
        self.debuffs
            .get(&spell_id)
            .is_some_and(|d| !d.is_expired(now))
    }

    /// Get the number of active debuffs.
    pub fn count(&self, now: u64) -> usize {
        self.debuffs.values().filter(|d| !d.is_expired(now)).count()
    }

    /// Clear all tracked debuffs.
    pub fn clear(&mut self) {
        self.debuffs.clear();
    }

    /// Get all active debuffs as a list.
    pub fn all(&self, now: u64) -> Vec<&TrackedDebuff> {
        self.debuffs
            .values()
            .filter(|d| !d.is_expired(now))
            .collect()
    }
}

impl Default for DebuffTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current Unix timestamp in seconds.
#[must_use]
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::combat::BuffCategory;

    // ─── Test Helper ────────────────────────────────────────────────────────

    fn make_buff(spell_id: i32, duration_ticks: i32) -> BuffInfo {
        BuffInfo {
            spell_id,
            duration_ticks,
            initial_duration: duration_ticks,
            hit_count: 0,
            category: BuffCategory::LongBuff,
            caster_level: 65,
            slot_index: 0,
        }
    }

    // ─── DebuffPriority Tests ───────────────────────────────────────────────

    #[test]
    fn priority_stun_has_lowest_value() {
        assert_eq!(DebuffPriority::Stun.value(), 1);
    }

    #[test]
    fn priority_root_second_lowest() {
        assert_eq!(DebuffPriority::Root.value(), 2);
    }

    #[test]
    fn priority_values_ordered() {
        assert!(DebuffPriority::Stun < DebuffPriority::Root);
        assert!(DebuffPriority::Root < DebuffPriority::Snare);
        assert!(DebuffPriority::Curse < DebuffPriority::Disease);
    }

    #[test]
    fn priority_stun_as_str() {
        assert_eq!(DebuffPriority::Stun.as_str(), "Stun");
    }

    #[test]
    fn priority_curse_as_str() {
        assert_eq!(DebuffPriority::Curse.as_str(), "Curse");
    }

    // ─── TrackedDebuff Tests ────────────────────────────────────────────────

    #[test]
    fn tracked_debuff_new_creates_with_timestamp() {
        let entry = lookup_debuff(1239).unwrap(); // Stun
        let buff = make_buff(1239, 6);
        let now = 1000;

        let tracked = TrackedDebuff::new(entry, &buff, now);
        assert_eq!(tracked.applied_at, 1000);
        assert_eq!(tracked.entry.spell_id, 1239);
    }

    #[test]
    fn tracked_debuff_rank_priority_stun() {
        let entry = lookup_debuff(1239).unwrap(); // "Stun"
        let priority = TrackedDebuff::rank_priority(entry);
        assert_eq!(priority, DebuffPriority::Stun);
    }

    #[test]
    fn tracked_debuff_rank_priority_hamstring() {
        let entry = lookup_debuff(1235).unwrap(); // "Hamstring"
        let priority = TrackedDebuff::rank_priority(entry);
        assert_eq!(priority, DebuffPriority::Snare);
    }

    #[test]
    fn tracked_debuff_rank_priority_curse() {
        let entry = lookup_debuff(1500).unwrap(); // "Curse"
        let priority = TrackedDebuff::rank_priority(entry);
        assert_eq!(priority, DebuffPriority::Curse);
    }

    #[test]
    fn tracked_debuff_permanent_never_expires() {
        let entry = lookup_debuff(1238).unwrap(); // Root (permanent)
        let buff = make_buff(1238, 0);
        let now = 1000;

        let tracked = TrackedDebuff::new(entry, &buff, now);
        assert!(!tracked.is_expired(1000));
        assert!(!tracked.is_expired(9999));
    }

    #[test]
    fn tracked_debuff_remaining_seconds_permanent_returns_none() {
        let entry = lookup_debuff(1238).unwrap(); // Root (permanent)
        let buff = make_buff(1238, 0);
        let now = 1000;

        let tracked = TrackedDebuff::new(entry, &buff, now);
        assert_eq!(tracked.remaining_seconds(now), None);
    }

    #[test]
    fn tracked_debuff_remaining_seconds_calculates_duration() {
        let entry = lookup_debuff(1239).unwrap(); // Stun (6 seconds)
        let buff = make_buff(1239, 1); // 1 tick = 6 seconds
        let now = 1000;

        let tracked = TrackedDebuff::new(entry, &buff, now);
        // expires_at should be 1000 + 6 = 1006
        assert_eq!(tracked.expires_at, 1006);
        assert_eq!(tracked.remaining_seconds(1000), Some(6));
        assert_eq!(tracked.remaining_seconds(1003), Some(3));
        assert_eq!(tracked.remaining_seconds(1006), Some(0));
    }

    #[test]
    fn tracked_debuff_is_expired_after_time() {
        let entry = lookup_debuff(1239).unwrap(); // Stun
        let buff = make_buff(1239, 1);
        let now = 1000;

        let tracked = TrackedDebuff::new(entry, &buff, now);
        assert!(!tracked.is_expired(1005)); // Before expiry
        assert!(tracked.is_expired(1006)); // At/after expiry
    }

    // ─── DebuffTracker Tests ────────────────────────────────────────────────

    #[test]
    fn tracker_new_is_empty() {
        let tracker = DebuffTracker::new();
        assert_eq!(tracker.count(1000), 0);
    }

    #[test]
    fn tracker_update_adds_debuff() {
        let mut tracker = DebuffTracker::new();
        let buffs = vec![make_buff(1239, 6)]; // Stun
        let now = 1000;

        tracker.update(&buffs, now);
        assert_eq!(tracker.count(now), 1);
        assert!(tracker.has_debuff(1239, now));
    }

    #[test]
    fn tracker_update_ignores_unknown_buffs() {
        let mut tracker = DebuffTracker::new();
        let buffs = vec![make_buff(99999, 10)]; // Unknown
        let now = 1000;

        tracker.update(&buffs, now);
        assert_eq!(tracker.count(now), 0);
    }

    #[test]
    fn tracker_update_removes_expired_debuffs() {
        let mut tracker = DebuffTracker::new();
        let buffs = vec![make_buff(1239, 1)];
        let now = 1000;

        tracker.update(&buffs, now);
        assert_eq!(tracker.count(now), 1);

        // After expiry, should remove it
        tracker.update(&[], now + 10);
        assert_eq!(tracker.count(now + 10), 0);
    }

    #[test]
    fn tracker_get_sorted_by_priority() {
        let mut tracker = DebuffTracker::new();
        let buffs = vec![
            make_buff(1235, 18),  // Hamstring (Snare)
            make_buff(1239, 6),   // Stun
            make_buff(1300, 120), // Plague (Disease)
        ];
        let now = 1000;

        tracker.update(&buffs, now);
        let sorted = tracker.get_sorted_by_priority(now);
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].entry.spell_id, 1239); // Stun first
        assert_eq!(sorted[1].entry.spell_id, 1235); // Snare second
        assert_eq!(sorted[2].entry.spell_id, 1300); // Disease last
    }

    #[test]
    fn tracker_get_by_type() {
        let mut tracker = DebuffTracker::new();
        let buffs = vec![
            make_buff(1239, 6),   // Stun (CrowdControl)
            make_buff(1235, 18),  // Hamstring (CrowdControl)
            make_buff(1300, 120), // Plague (Disease)
        ];
        let now = 1000;

        tracker.update(&buffs, now);
        let cc = tracker.get_by_type(DebuffType::CrowdControl, now);
        assert_eq!(cc.len(), 2);

        let disease = tracker.get_by_type(DebuffType::Disease, now);
        assert_eq!(disease.len(), 1);
    }

    #[test]
    fn tracker_most_urgent() {
        let mut tracker = DebuffTracker::new();
        let buffs = vec![
            make_buff(1235, 18), // Snare
            make_buff(1239, 6),  // Stun
        ];
        let now = 1000;

        tracker.update(&buffs, now);
        let urgent = tracker.most_urgent(now).unwrap();
        assert_eq!(urgent.entry.spell_id, 1239); // Stun is most urgent
    }

    #[test]
    fn tracker_has_debuff() {
        let mut tracker = DebuffTracker::new();
        let buffs = vec![make_buff(1239, 6)];
        let now = 1000;

        tracker.update(&buffs, now);
        assert!(tracker.has_debuff(1239, now));
        assert!(!tracker.has_debuff(1235, now));
    }

    #[test]
    fn tracker_clear() {
        let mut tracker = DebuffTracker::new();
        let buffs = vec![make_buff(1239, 6), make_buff(1235, 18)];
        let now = 1000;

        tracker.update(&buffs, now);
        assert_eq!(tracker.count(now), 2);

        tracker.clear();
        assert_eq!(tracker.count(now), 0);
    }
}
