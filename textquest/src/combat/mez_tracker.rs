//! Mez (crowd-control) immunity tracker with zone-change expiry.
//!
//! Tracks per-mob mez immunity keyed by `(mob_name, spell_id)`.  The immune
//! list is automatically cleared on zone change so that fresh-zone targets
//! get a clean slate.
//!
//! ## Chat patterns detected
//! - `"Your target is immune"` — EQ "hard immune" message.
//! - `"did not take hold"` — EQ "spell did not land" message (soft immune /
//!   existing buff block).

use std::collections::HashMap;
use std::time::Instant;

/// Spell ID type alias — matches the `u32` used throughout the combat module.
pub type SpellId = u32;

/// Tracks mez-immune mobs for the current zone session.
///
/// Immunity entries are keyed by `(mob_name, spell_id)` and timestamped so
/// callers can optionally age entries out.  All entries are purged on
/// [`on_zone_change`](MezTracker::on_zone_change).
pub struct MezTracker {
    /// Map of `(mob_name, spell_id)` → timestamp when immunity was recorded.
    pub immune_list: HashMap<(String, SpellId), Instant>,
}

impl MezTracker {
    /// Create a new, empty tracker.
    pub fn new() -> Self {
        Self {
            immune_list: HashMap::new(),
        }
    }

    /// Mark a mob as immune to a specific spell.
    ///
    /// Subsequent calls with the same key refresh the timestamp.
    pub fn mark_immune(&mut self, mob_name: &str, spell_id: SpellId) {
        self.immune_list
            .insert((mob_name.to_string(), spell_id), Instant::now());
    }

    /// Returns `true` if the mob is currently tracked as immune to this spell.
    pub fn is_immune(&self, mob_name: &str, spell_id: SpellId) -> bool {
        self.immune_list
            .contains_key(&(mob_name.to_string(), spell_id))
    }

    /// Clear all immunity entries on zone change.
    ///
    /// Zone transitions reset the encounter state; mobs in the new zone should
    /// be attempted normally.
    pub fn on_zone_change(&mut self) {
        self.immune_list.clear();
    }

    /// Parse a chat log line and, if it contains an immunity message, record
    /// the mob as immune to the given spell.
    ///
    /// Recognised patterns (case-insensitive):
    /// - `"Your target is immune"` — hard immune.
    /// - `"did not take hold"` — spell did not land (soft immune / resist).
    ///
    /// Returns `true` when an immunity was recorded.
    pub fn parse_chat_line(&mut self, line: &str, mob_name: &str, spell_id: SpellId) -> bool {
        let lower = line.to_lowercase();
        let is_immune_msg =
            lower.contains("your target is immune") || lower.contains("did not take hold");

        if is_immune_msg {
            self.mark_immune(mob_name, spell_id);
        }
        is_immune_msg
    }
}

impl Default for MezTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tracker_is_empty() {
        let tracker = MezTracker::new();
        assert!(tracker.immune_list.is_empty());
    }

    #[test]
    fn mark_and_check_immune() {
        let mut tracker = MezTracker::new();
        tracker.mark_immune("a_goblin", 1234);
        assert!(tracker.is_immune("a_goblin", 1234));
        assert!(!tracker.is_immune("a_goblin", 9999)); // different spell
        assert!(!tracker.is_immune("other_mob", 1234)); // different mob
    }

    #[test]
    fn immune_mob_is_skipped_on_second_mez_attempt() {
        let mut tracker = MezTracker::new();
        let mob = "Grizzlebeard";
        let spell: SpellId = 5432;

        // First attempt: not yet immune
        assert!(!tracker.is_immune(mob, spell));

        // Simulate receiving immunity message (e.g. from cast result parser)
        tracker.mark_immune(mob, spell);

        // Second attempt: immune — caller should skip this target
        assert!(tracker.is_immune(mob, spell));
    }

    #[test]
    fn zone_change_clears_immune_list() {
        let mut tracker = MezTracker::new();
        tracker.mark_immune("mob_a", 100);
        tracker.mark_immune("mob_b", 200);
        assert_eq!(tracker.immune_list.len(), 2);

        tracker.on_zone_change();

        assert!(tracker.immune_list.is_empty());
        assert!(!tracker.is_immune("mob_a", 100));
        assert!(!tracker.is_immune("mob_b", 200));
    }

    #[test]
    fn parse_immune_message_records_immunity() {
        let mut tracker = MezTracker::new();
        let recorded =
            tracker.parse_chat_line("Your target is immune to this effect.", "Grizzlebeard", 10);
        assert!(recorded);
        assert!(tracker.is_immune("Grizzlebeard", 10));
    }

    #[test]
    fn parse_did_not_take_hold_records_immunity() {
        let mut tracker = MezTracker::new();
        let recorded = tracker.parse_chat_line("Your spell did not take hold.", "Thicket Rat", 20);
        assert!(recorded);
        assert!(tracker.is_immune("Thicket Rat", 20));
    }

    #[test]
    fn parse_unrelated_chat_does_not_record() {
        let mut tracker = MezTracker::new();
        let recorded = tracker.parse_chat_line("You strike the goblin for 42 damage.", "Goblin", 1);
        assert!(!recorded);
        assert!(!tracker.is_immune("Goblin", 1));
    }

    #[test]
    fn parse_chat_is_case_insensitive() {
        let mut tracker = MezTracker::new();
        tracker.parse_chat_line("YOUR TARGET IS IMMUNE TO THIS EFFECT.", "Boss", 99);
        assert!(tracker.is_immune("Boss", 99));
    }

    #[test]
    fn multiple_spells_tracked_independently() {
        let mut tracker = MezTracker::new();
        tracker.mark_immune("Mob", 1);
        tracker.mark_immune("Mob", 2);
        assert!(tracker.is_immune("Mob", 1));
        assert!(tracker.is_immune("Mob", 2));
        assert!(!tracker.is_immune("Mob", 3));
    }
}
