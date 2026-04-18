//! Intent-based item decision tracking for M10 economy.
//!
//! This module provides a structured way to track operator intent for looted items:
//! what the operator wants to do with items as they are looted. Each item can be
//! tracked with a specific intent (keep, sell, bank, distribute to a role, or salvage).
//!
//! The `WishlistEntry` represents a single item with its intent and metadata.
//! The `IntentTracker` provides storage and CRUD operations for managing entries.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use textquest_common::types::ClientId;

/// The intent (decision) for what to do with a looted item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemIntent {
    /// Keep the item for personal use on this character.
    Keep,
    /// Sell the item to a vendor for profit.
    Sell,
    /// Move the item to the shared bank for later use.
    Bank,
    /// Distribute the item to a character in a specific role.
    DistributeToRole { role: u8 },
    /// Disassemble or salvage the item for materials.
    Salvage,
}

impl ItemIntent {
    /// Returns a human-readable label for this intent.
    pub fn label(&self) -> &'static str {
        match self {
            ItemIntent::Keep => "keep",
            ItemIntent::Sell => "sell",
            ItemIntent::Bank => "bank",
            ItemIntent::DistributeToRole { .. } => "distribute-to-role",
            ItemIntent::Salvage => "salvage",
        }
    }
}

/// A single wishlist entry tracking operator intent for a specific item.
///
/// Each entry records:
/// - What item (by item_id)
/// - What the operator intends to do with it
/// - Who (if any) the item is reserved for
/// - When it was last updated
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WishlistEntry {
    /// EQ item ID.
    pub item_id: u32,
    /// Operator's intent for this item.
    pub intent: ItemIntent,
    /// If intent is DistributeToRole, the role it should go to.
    /// If intent is other, this may be the character to keep/bank for.
    pub reserved_for: Option<ClientId>,
    /// Human-readable note (e.g., "needed for archetype quest").
    pub note: String,
    /// Timestamp when this entry was created or last modified (ISO 8601 string).
    pub updated_at: String,
}

impl WishlistEntry {
    /// Create a new wishlist entry with the given item_id and intent.
    pub fn new(item_id: u32, intent: ItemIntent) -> Self {
        Self {
            item_id,
            intent,
            reserved_for: None,
            note: String::new(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Builder method: set the character this item is reserved for.
    pub fn with_reserved_for(mut self, char_id: ClientId) -> Self {
        self.reserved_for = Some(char_id);
        self
    }

    /// Builder method: add a note explaining the intent.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }

    /// Update the timestamp to now.
    pub fn touch(&mut self) {
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }
}

/// Manages operator intent tracking for looted items.
///
/// Provides in-memory storage of wishlist entries with CRUD operations:
/// - Create: add a new entry
/// - Read: retrieve by item_id
/// - Update: modify an existing entry's intent or note
/// - Delete: remove an entry
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct IntentTracker {
    /// Item ID → WishlistEntry mapping.
    entries: HashMap<u32, WishlistEntry>,
}

impl IntentTracker {
    /// Create an empty intent tracker.
    pub fn new() -> Self {
        Self::default()
    }

    // ── Create (Add) ──────────────────────────────────────────────────────

    /// Add a new wishlist entry, replacing any existing entry for this item_id.
    pub fn add_entry(&mut self, entry: WishlistEntry) {
        self.entries.insert(entry.item_id, entry);
    }

    /// Convenience: create and add an entry with just item_id and intent.
    pub fn add_item(&mut self, item_id: u32, intent: ItemIntent) -> &mut WishlistEntry {
        let entry = WishlistEntry::new(item_id, intent);
        self.entries.insert(item_id, entry);
        self.entries.get_mut(&item_id).expect("just inserted")
    }

    // ── Read ──────────────────────────────────────────────────────────────

    /// Get the wishlist entry for a specific item_id, if it exists.
    pub fn get(&self, item_id: u32) -> Option<&WishlistEntry> {
        self.entries.get(&item_id)
    }

    /// Get mutable reference to a wishlist entry (for in-place updates).
    pub fn get_mut(&mut self, item_id: u32) -> Option<&mut WishlistEntry> {
        self.entries.get_mut(&item_id)
    }

    /// Get all entries for items with a specific intent.
    pub fn get_by_intent(&self, intent: ItemIntent) -> Vec<&WishlistEntry> {
        self.entries
            .values()
            .filter(|e| e.intent == intent)
            .collect()
    }

    /// Check if an item has a wishlist entry.
    pub fn contains(&self, item_id: u32) -> bool {
        self.entries.contains_key(&item_id)
    }

    /// Get a count of all tracked items.
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = &WishlistEntry> {
        self.entries.values()
    }

    // ── Update ────────────────────────────────────────────────────────────

    /// Update the intent for an existing entry.
    /// Returns `true` if updated, `false` if item_id not found.
    pub fn update_intent(&mut self, item_id: u32, new_intent: ItemIntent) -> bool {
        if let Some(entry) = self.entries.get_mut(&item_id) {
            entry.intent = new_intent;
            entry.touch();
            true
        } else {
            false
        }
    }

    /// Update the note for an existing entry.
    /// Returns `Ok(())` if updated, `Err(())` if item_id not found.
    pub fn update_note(&mut self, item_id: u32, note: impl Into<String>) -> bool {
        if let Some(entry) = self.entries.get_mut(&item_id) {
            entry.note = note.into();
            entry.touch();
            true
        } else {
            false
        }
    }

    /// Update the reserved_for character for an existing entry.
    /// Returns `true` if updated, `false` if item_id not found.
    pub fn update_reserved_for(&mut self, item_id: u32, char_id: Option<ClientId>) -> bool {
        if let Some(entry) = self.entries.get_mut(&item_id) {
            entry.reserved_for = char_id;
            entry.touch();
            true
        } else {
            false
        }
    }

    // ── Delete ────────────────────────────────────────────────────────────

    /// Remove an entry by item_id.
    /// Returns the removed entry if it existed, `None` otherwise.
    pub fn remove(&mut self, item_id: u32) -> Option<WishlistEntry> {
        self.entries.remove(&item_id)
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Import entries from another tracker, overwriting any existing entries.
    pub fn merge(&mut self, other: IntentTracker) {
        self.entries.extend(other.entries);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const ITEM_SWORD: u32 = 1001;
    const ITEM_RING: u32 = 2002;
    const ITEM_ROBE: u32 = 3003;
    const CHAR_WARRIOR: ClientId = 10;
    const CHAR_MAGE: ClientId = 11;

    // ─── Test 1: Create and retrieve a basic entry ────────────────────────

    #[test]
    fn test_add_and_get_entry() {
        let mut tracker = IntentTracker::new();

        let entry = WishlistEntry::new(ITEM_SWORD, ItemIntent::Keep);
        tracker.add_entry(entry);

        let retrieved = tracker.get(ITEM_SWORD).expect("entry should exist");
        assert_eq!(retrieved.item_id, ITEM_SWORD);
        assert_eq!(retrieved.intent, ItemIntent::Keep);
        assert_eq!(retrieved.reserved_for, None);
    }

    // ─── Test 2: Update intent ────────────────────────────────────────────

    #[test]
    fn test_update_intent() {
        let mut tracker = IntentTracker::new();
        tracker.add_item(ITEM_RING, ItemIntent::Keep);

        // Update from Keep to Sell.
        let result = tracker.update_intent(ITEM_RING, ItemIntent::Sell);
        assert!(result);

        let entry = tracker.get(ITEM_RING).expect("entry should exist");
        assert_eq!(entry.intent, ItemIntent::Sell);
    }

    // ─── Test 3: CRUD operations (Create, Read, Update, Delete) ──────────

    #[test]
    fn test_full_crud_cycle() {
        let mut tracker = IntentTracker::new();

        // Create
        let entry =
            WishlistEntry::new(ITEM_ROBE, ItemIntent::Bank).with_note("healer robe for group");
        tracker.add_entry(entry);
        assert!(tracker.contains(ITEM_ROBE));

        // Read
        let entry = tracker.get(ITEM_ROBE).expect("should exist");
        assert_eq!(entry.note, "healer robe for group");

        // Update
        tracker.update_note(ITEM_ROBE, "updated healer robe note");
        let entry = tracker.get(ITEM_ROBE).expect("should exist");
        assert_eq!(entry.note, "updated healer robe note");

        // Delete
        let removed = tracker.remove(ITEM_ROBE);
        assert!(removed.is_some());
        assert!(!tracker.contains(ITEM_ROBE));
    }

    // ─── Test 4: Query by intent ──────────────────────────────────────────

    #[test]
    fn test_get_by_intent() {
        let mut tracker = IntentTracker::new();

        tracker.add_item(ITEM_SWORD, ItemIntent::Keep);
        tracker.add_item(ITEM_RING, ItemIntent::Sell);
        tracker.add_item(ITEM_ROBE, ItemIntent::Bank);

        // Query for "Keep" intent.
        let keep_items = tracker.get_by_intent(ItemIntent::Keep);
        assert_eq!(keep_items.len(), 1);
        assert_eq!(keep_items[0].item_id, ITEM_SWORD);

        // Query for "Sell" intent.
        let sell_items = tracker.get_by_intent(ItemIntent::Sell);
        assert_eq!(sell_items.len(), 1);
        assert_eq!(sell_items[0].item_id, ITEM_RING);
    }

    // ─── Test 5: Reserved character tracking ──────────────────────────────

    #[test]
    fn test_reserved_for_tracking() {
        let mut tracker = IntentTracker::new();

        let entry =
            WishlistEntry::new(ITEM_SWORD, ItemIntent::Keep).with_reserved_for(CHAR_WARRIOR);
        tracker.add_entry(entry);

        let retrieved = tracker.get(ITEM_SWORD).expect("should exist");
        assert_eq!(retrieved.reserved_for, Some(CHAR_WARRIOR));

        // Update reserved_for.
        tracker.update_reserved_for(ITEM_SWORD, Some(CHAR_MAGE));
        let entry = tracker.get(ITEM_SWORD).expect("should exist");
        assert_eq!(entry.reserved_for, Some(CHAR_MAGE));
    }

    // ─── Test 6: DistributeToRole intent with role ────────────────────────

    #[test]
    fn test_distribute_to_role_intent() {
        let mut tracker = IntentTracker::new();

        let distribute_intent = ItemIntent::DistributeToRole { role: 5 };
        tracker.add_item(ITEM_ROBE, distribute_intent);

        let entry = tracker.get(ITEM_ROBE).expect("should exist");
        assert!(matches!(
            entry.intent,
            ItemIntent::DistributeToRole { role: 5 }
        ));
    }

    // ─── Test 7: Salvage intent ──────────────────────────────────────────

    #[test]
    fn test_salvage_intent() {
        let mut tracker = IntentTracker::new();

        tracker.add_item(ITEM_RING, ItemIntent::Salvage);

        let entry = tracker.get(ITEM_RING).expect("should exist");
        assert_eq!(entry.intent, ItemIntent::Salvage);
        assert_eq!(entry.intent.label(), "salvage");
    }

    // ─── Test 8: Replace existing entry ──────────────────────────────────

    #[test]
    fn test_replace_entry() {
        let mut tracker = IntentTracker::new();

        tracker.add_item(ITEM_SWORD, ItemIntent::Keep);
        assert_eq!(tracker.count(), 1);

        // Add a new entry with the same item_id (should replace).
        let new_entry =
            WishlistEntry::new(ITEM_SWORD, ItemIntent::Sell).with_note("sell this instead");
        tracker.add_entry(new_entry);
        assert_eq!(tracker.count(), 1);

        let entry = tracker.get(ITEM_SWORD).expect("should exist");
        assert_eq!(entry.intent, ItemIntent::Sell);
        assert_eq!(entry.note, "sell this instead");
    }

    // ─── Test 9: Clear all entries ────────────────────────────────────────

    #[test]
    fn test_clear_all() {
        let mut tracker = IntentTracker::new();

        tracker.add_item(ITEM_SWORD, ItemIntent::Keep);
        tracker.add_item(ITEM_RING, ItemIntent::Bank);
        tracker.add_item(ITEM_ROBE, ItemIntent::Sell);

        assert_eq!(tracker.count(), 3);
        tracker.clear();
        assert_eq!(tracker.count(), 0);
    }

    // ─── Test 10: Iteration ───────────────────────────────────────────────

    #[test]
    fn test_iteration() {
        let mut tracker = IntentTracker::new();

        tracker.add_item(ITEM_SWORD, ItemIntent::Keep);
        tracker.add_item(ITEM_RING, ItemIntent::Bank);
        tracker.add_item(ITEM_ROBE, ItemIntent::Sell);

        let entries: Vec<_> = tracker.iter().collect();
        assert_eq!(entries.len(), 3);

        // Collect all item_ids from iteration.
        let ids: Vec<_> = entries.iter().map(|e| e.item_id).collect();
        assert!(ids.contains(&ITEM_SWORD));
        assert!(ids.contains(&ITEM_RING));
        assert!(ids.contains(&ITEM_ROBE));
    }

    // ─── Test 11: Merge trackers ──────────────────────────────────────────

    #[test]
    fn test_merge_trackers() {
        let mut tracker1 = IntentTracker::new();
        tracker1.add_item(ITEM_SWORD, ItemIntent::Keep);

        let mut tracker2 = IntentTracker::new();
        tracker2.add_item(ITEM_RING, ItemIntent::Bank);
        tracker2.add_item(ITEM_ROBE, ItemIntent::Sell);

        tracker1.merge(tracker2);
        assert_eq!(tracker1.count(), 3);
        assert!(tracker1.contains(ITEM_SWORD));
        assert!(tracker1.contains(ITEM_RING));
        assert!(tracker1.contains(ITEM_ROBE));
    }

    // ─── Test 12: Serialization round-trip ────────────────────────────────

    #[test]
    fn test_serde_round_trip() {
        let mut tracker = IntentTracker::new();

        let entry = WishlistEntry::new(ITEM_SWORD, ItemIntent::Keep)
            .with_reserved_for(CHAR_WARRIOR)
            .with_note("BiS weapon");
        tracker.add_entry(entry);

        // Serialize
        let json = serde_json::to_string(&tracker).expect("should serialize");

        // Deserialize
        let restored: IntentTracker = serde_json::from_str(&json).expect("should deserialize");

        let restored_entry = restored.get(ITEM_SWORD).expect("should exist");
        assert_eq!(restored_entry.item_id, ITEM_SWORD);
        assert_eq!(restored_entry.intent, ItemIntent::Keep);
        assert_eq!(restored_entry.reserved_for, Some(CHAR_WARRIOR));
        assert_eq!(restored_entry.note, "BiS weapon");
    }
}
