//! Ownership model — assigns dropped items to characters and tracks lifecycle.
//!
//! # States
//!
//! ```text
//! Available ──► Reserved ──► Assigned ──► Collected ──► Distributed
//!     │                          │
//!     └──────────────────────────┴──► Vendor  (fallback: no eligible character)
//! ```
//!
//! Every state transition is appended to an in-memory [`AuditLog`] so that the
//! full lifecycle of every item drop is recoverable without a database.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::loot::queue::DroppedItem;

// ─── Ownership state ──────────────────────────────────────────────────────────

/// Lifecycle states for a single dropped item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OwnershipState {
    /// Item is in the queue; no character has claimed it yet.
    Available,
    /// A character has expressed intent to take this item (soft-lock).
    Reserved,
    /// The item has been assigned to a specific character.
    Assigned,
    /// The assigned character has picked the item up.
    Collected,
    /// The item has been distributed / traded to its final owner.
    Distributed,
    /// No eligible character wanted the item; queued for vendor sale.
    Vendor,
}

impl std::fmt::Display for OwnershipState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OwnershipState::Available => write!(f, "Available"),
            OwnershipState::Reserved => write!(f, "Reserved"),
            OwnershipState::Assigned => write!(f, "Assigned"),
            OwnershipState::Collected => write!(f, "Collected"),
            OwnershipState::Distributed => write!(f, "Distributed"),
            OwnershipState::Vendor => write!(f, "Vendor"),
        }
    }
}

// ─── Audit log ────────────────────────────────────────────────────────────────

/// A single recorded state transition.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// Drop event this transition belongs to.
    pub drop_id: u64,
    /// State before the transition.
    pub from: OwnershipState,
    /// State after the transition.
    pub to: OwnershipState,
    /// Character name involved, if any.
    pub character: Option<String>,
    /// Unix timestamp of the transition.
    pub timestamp: u64,
}

/// Append-only audit trail for all ownership state changes.
#[derive(Debug, Default)]
pub struct AuditLog {
    entries: Vec<AuditEntry>,
}

impl AuditLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a state transition.
    pub fn record(
        &mut self,
        drop_id: u64,
        from: OwnershipState,
        to: OwnershipState,
        character: Option<String>,
    ) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.entries.push(AuditEntry {
            drop_id,
            from,
            to,
            character,
            timestamp,
        });
    }

    /// All recorded entries in insertion order.
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// Entries for a specific drop.
    pub fn for_drop(&self, drop_id: u64) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.drop_id == drop_id)
            .collect()
    }
}

// ─── Character priority record ────────────────────────────────────────────────

/// A character that can receive loot.
#[derive(Debug, Clone)]
pub struct LootCandidate {
    /// Character name (must be unique).
    pub name: String,
    /// Numeric priority — higher value wins ties.  Typical range 0-100.
    pub priority: u32,
    /// Item ids on this character's wishlist (in priority order).
    pub wishlist: Vec<u32>,
}

impl LootCandidate {
    pub fn new(name: impl Into<String>, priority: u32, wishlist: Vec<u32>) -> Self {
        Self {
            name: name.into(),
            priority,
            wishlist,
        }
    }

    /// `true` if this character wants `item_id`.
    pub fn wants(&self, item_id: u32) -> bool {
        self.wishlist.contains(&item_id)
    }
}

// ─── Ownership record ─────────────────────────────────────────────────────────

/// Tracks the current ownership state of one dropped item.
#[derive(Debug, Clone)]
pub struct OwnershipRecord {
    pub drop: DroppedItem,
    pub state: OwnershipState,
    /// The character currently holding the claim (Reserved / Assigned / …).
    pub owner: Option<String>,
}

// ─── Ownership model ──────────────────────────────────────────────────────────

/// Assigns dropped items to characters based on wishlist priority rules.
///
/// All state is in-memory.  Use [`AuditLog`] for persistence if needed.
#[derive(Debug, Default)]
pub struct OwnershipModel {
    /// Characters that can receive loot, keyed by name.
    candidates: Vec<LootCandidate>,
    /// Live ownership records, keyed by drop_id.
    records: HashMap<u64, OwnershipRecord>,
    /// Append-only audit trail.
    pub audit: AuditLog,
}

impl OwnershipModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a character as a loot candidate.
    pub fn add_candidate(&mut self, candidate: LootCandidate) {
        self.candidates.push(candidate);
    }

    /// Ingest a dropped item and immediately run priority assignment.
    ///
    /// Returns the resulting [`OwnershipRecord`].
    pub fn intake(&mut self, drop: DroppedItem) -> &OwnershipRecord {
        let drop_id = drop.drop_id;
        let item_id = drop.item_id;

        // Start as Available
        let record = OwnershipRecord {
            drop,
            state: OwnershipState::Available,
            owner: None,
        };
        self.records.insert(drop_id, record);
        self.audit.record(
            drop_id,
            OwnershipState::Available,
            OwnershipState::Available,
            None,
        );

        // Find the highest-priority candidate that wants this item
        let winner = self
            .candidates
            .iter()
            .filter(|c| c.wants(item_id))
            .max_by_key(|c| c.priority)
            .map(|c| c.name.clone());

        if let Some(name) = winner {
            self.transition(drop_id, OwnershipState::Assigned, Some(name));
        } else {
            // No candidate — route to vendor
            self.transition(drop_id, OwnershipState::Vendor, None);
        }

        self.records.get(&drop_id).unwrap()
    }

    /// Advance a record to a new state.
    ///
    /// # Panics
    /// Panics if `drop_id` is unknown.
    pub fn transition(
        &mut self,
        drop_id: u64,
        new_state: OwnershipState,
        character: Option<String>,
    ) {
        let record = self.records.get_mut(&drop_id).expect("unknown drop_id");
        let old_state = record.state.clone();
        record.state = new_state.clone();
        record.owner = character.clone();
        self.audit.record(drop_id, old_state, new_state, character);
    }

    /// Look up a record by drop id.
    pub fn get(&self, drop_id: u64) -> Option<&OwnershipRecord> {
        self.records.get(&drop_id)
    }

    /// All current records.
    pub fn records(&self) -> impl Iterator<Item = &OwnershipRecord> {
        self.records.values()
    }

    /// Records in a specific state.
    pub fn in_state(&self, state: &OwnershipState) -> Vec<&OwnershipRecord> {
        self.records
            .values()
            .filter(|r| &r.state == state)
            .collect()
    }
}

// ─── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loot::queue::LootQueue;

    fn make_model() -> OwnershipModel {
        let mut m = OwnershipModel::new();
        // Ranger (priority 10) and Warrior (priority 50) both want item 100.
        // Cleric (priority 30) wants item 200 only.
        m.add_candidate(LootCandidate::new("Ranger", 10, vec![100, 200]));
        m.add_candidate(LootCandidate::new("Warrior", 50, vec![100]));
        m.add_candidate(LootCandidate::new("Cleric", 30, vec![200]));
        m
    }

    #[test]
    fn high_priority_char_wins() {
        let mut q = LootQueue::new();
        let drop = q.push(100, 1, 1001);
        let mut m = make_model();
        let record = m.intake(drop);
        // Warrior (50) beats Ranger (10)
        assert_eq!(record.state, OwnershipState::Assigned);
        assert_eq!(record.owner.as_deref(), Some("Warrior"));
    }

    #[test]
    fn item_goes_to_vendor_fallback() {
        let mut q = LootQueue::new();
        // item 999 — nobody wants it
        let drop = q.push(999, 1, 1001);
        let mut m = make_model();
        let record = m.intake(drop);
        assert_eq!(record.state, OwnershipState::Vendor);
        assert!(record.owner.is_none());
    }

    #[test]
    fn only_matching_candidate_gets_item() {
        let mut q = LootQueue::new();
        let drop = q.push(200, 1, 1001); // Ranger + Cleric want this; Cleric priority=30 > Ranger=10
        let mut m = make_model();
        let record = m.intake(drop);
        assert_eq!(record.state, OwnershipState::Assigned);
        assert_eq!(record.owner.as_deref(), Some("Cleric"));
    }

    #[test]
    fn audit_trail_logs_transitions() {
        let mut q = LootQueue::new();
        let drop = q.push(100, 1, 1001);
        let drop_id = drop.drop_id;
        let mut m = make_model();
        m.intake(drop);
        let entries = m.audit.for_drop(drop_id);
        // Expect at least 2 entries: Available→Available (initial) + Available→Assigned
        assert!(entries.len() >= 2);
        let last = entries.last().unwrap();
        assert_eq!(last.to, OwnershipState::Assigned);
    }

    #[test]
    fn manual_transition_to_collected() {
        let mut q = LootQueue::new();
        let drop = q.push(100, 1, 1001);
        let drop_id = drop.drop_id;
        let mut m = make_model();
        m.intake(drop);
        m.transition(drop_id, OwnershipState::Collected, Some("Warrior".into()));
        let r = m.get(drop_id).unwrap();
        assert_eq!(r.state, OwnershipState::Collected);
    }

    #[test]
    fn manual_transition_to_distributed() {
        let mut q = LootQueue::new();
        let drop = q.push(100, 1, 1001);
        let drop_id = drop.drop_id;
        let mut m = make_model();
        m.intake(drop);
        m.transition(drop_id, OwnershipState::Collected, Some("Warrior".into()));
        m.transition(drop_id, OwnershipState::Distributed, Some("Warrior".into()));
        let r = m.get(drop_id).unwrap();
        assert_eq!(r.state, OwnershipState::Distributed);
    }

    #[test]
    fn in_state_filter() {
        let mut q = LootQueue::new();
        let drop1 = q.push(100, 1, 1001); // → Assigned
        let drop2 = q.push(999, 1, 1001); // → Vendor
        let mut m = make_model();
        m.intake(drop1);
        m.intake(drop2);
        assert_eq!(m.in_state(&OwnershipState::Assigned).len(), 1);
        assert_eq!(m.in_state(&OwnershipState::Vendor).len(), 1);
        assert_eq!(m.in_state(&OwnershipState::Available).len(), 0);
    }

    #[test]
    fn audit_vendor_fallback_logged() {
        let mut q = LootQueue::new();
        let drop = q.push(999, 1, 1001);
        let drop_id = drop.drop_id;
        let mut m = make_model();
        m.intake(drop);
        let entries = m.audit.for_drop(drop_id);
        let last = entries.last().unwrap();
        assert_eq!(last.to, OwnershipState::Vendor);
    }

    // --- Additional ownership tests ---

    #[test]
    fn ownership_state_display() {
        assert_eq!(format!("{}", OwnershipState::Available), "Available");
        assert_eq!(format!("{}", OwnershipState::Reserved), "Reserved");
        assert_eq!(format!("{}", OwnershipState::Assigned), "Assigned");
        assert_eq!(format!("{}", OwnershipState::Collected), "Collected");
        assert_eq!(format!("{}", OwnershipState::Distributed), "Distributed");
        assert_eq!(format!("{}", OwnershipState::Vendor), "Vendor");
    }

    #[test]
    fn loot_candidate_wants_item() {
        let candidate = LootCandidate::new("Ranger", 10, vec![100, 200, 300]);
        assert!(candidate.wants(100));
        assert!(candidate.wants(200));
        assert!(!candidate.wants(999));
    }

    #[test]
    fn loot_candidate_empty_wishlist_wants_nothing() {
        let candidate = LootCandidate::new("Empty", 10, vec![]);
        assert!(!candidate.wants(100));
    }

    #[test]
    fn empty_model_vendor_all_items() {
        let mut q = LootQueue::new();
        let drop = q.push(100, 1, 1001);
        let mut m = OwnershipModel::new();
        let record = m.intake(drop);
        assert_eq!(record.state, OwnershipState::Vendor);
    }

    #[test]
    fn multiple_items_same_candidate() {
        let mut q = LootQueue::new();
        let drop1 = q.push(100, 1, 1001);
        let drop2 = q.push(100, 1, 1001);
        let mut m = make_model();
        let r1 = m.intake(drop1);
        assert_eq!(r1.owner.as_deref(), Some("Warrior"));
        let r2 = m.intake(drop2);
        assert_eq!(r2.owner.as_deref(), Some("Warrior"));
    }

    #[test]
    fn records_iterator() {
        let mut q = LootQueue::new();
        let d1 = q.push(100, 1, 1001);
        let d2 = q.push(999, 1, 1001);
        let mut m = make_model();
        m.intake(d1);
        m.intake(d2);
        let count = m.records().count();
        assert_eq!(count, 2);
    }

    #[test]
    fn get_unknown_drop_returns_none() {
        let m = OwnershipModel::new();
        assert!(m.get(999).is_none());
    }

    #[test]
    fn audit_log_for_unknown_drop_is_empty() {
        let log = AuditLog::new();
        assert!(log.for_drop(999).is_empty());
    }

    #[test]
    fn audit_log_entries_order() {
        let mut log = AuditLog::new();
        log.record(1, OwnershipState::Available, OwnershipState::Reserved, Some("Alice".into()));
        log.record(1, OwnershipState::Reserved, OwnershipState::Assigned, Some("Alice".into()));
        log.record(2, OwnershipState::Available, OwnershipState::Vendor, None);

        assert_eq!(log.entries().len(), 3);
        assert_eq!(log.for_drop(1).len(), 2);
        assert_eq!(log.for_drop(2).len(), 1);
    }

    #[test]
    fn full_lifecycle_available_to_distributed() {
        let mut q = LootQueue::new();
        let drop = q.push(100, 1, 1001);
        let drop_id = drop.drop_id;
        let mut m = make_model();
        m.intake(drop);
        m.transition(drop_id, OwnershipState::Collected, Some("Warrior".into()));
        m.transition(drop_id, OwnershipState::Distributed, Some("Warrior".into()));

        let entries = m.audit.for_drop(drop_id);
        // Available→Available, Available→Assigned, Assigned→Collected, Collected→Distributed
        assert_eq!(entries.len(), 4);
        assert_eq!(entries.last().unwrap().to, OwnershipState::Distributed);
    }

    #[test]
    fn ownership_state_hash_and_eq() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(OwnershipState::Available);
        set.insert(OwnershipState::Available);
        assert_eq!(set.len(), 1, "same state should deduplicate");
        set.insert(OwnershipState::Vendor);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn in_state_empty_model() {
        let m = OwnershipModel::new();
        assert!(m.in_state(&OwnershipState::Available).is_empty());
    }
}
