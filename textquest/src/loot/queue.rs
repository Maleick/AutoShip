//! Loot intake queue — buffers dropped items until the ownership model assigns them.
//!
//! [`LootQueue`] is a pure in-memory data structure; no live EQ connection is
//! required.  The queue stores every dropped item with its full provenance
//! (item id, quantity, dropper pid, timestamp) and exposes a simple FIFO drain
//! that feeds into [`crate::loot::ownership::OwnershipModel`].

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single item drop recorded in the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedItem {
    /// Unique identifier for this drop event (monotonically increasing).
    pub drop_id: u64,
    /// EQ item id (Lucy / in-game numeric id).
    pub item_id: u32,
    /// Number of items in this stack.
    pub quantity: u32,
    /// Process-id of the EQ client whose character produced the drop.
    pub dropper_pid: u32,
    /// Unix timestamp (seconds) when the drop was recorded.
    pub timestamp: u64,
}

impl DroppedItem {
    fn new(drop_id: u64, item_id: u32, quantity: u32, dropper_pid: u32) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            drop_id,
            item_id,
            quantity,
            dropper_pid,
            timestamp,
        }
    }
}

/// FIFO queue for items that have dropped but not yet been assigned.
#[derive(Debug, Default)]
pub struct LootQueue {
    queue: VecDeque<DroppedItem>,
    next_id: u64,
}

impl LootQueue {
    /// Create an empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a new item drop and push it onto the back of the queue.
    ///
    /// Returns the generated [`DroppedItem`] (with its assigned `drop_id`).
    pub fn push(&mut self, item_id: u32, quantity: u32, dropper_pid: u32) -> DroppedItem {
        let id = self.next_id;
        self.next_id += 1;
        let item = DroppedItem::new(id, item_id, quantity, dropper_pid);
        self.queue.push_back(item.clone());
        item
    }

    /// Take the oldest unprocessed item from the front of the queue.
    pub fn pop(&mut self) -> Option<DroppedItem> {
        self.queue.pop_front()
    }

    /// Peek at the oldest item without removing it.
    pub fn peek(&self) -> Option<&DroppedItem> {
        self.queue.front()
    }

    /// Number of items waiting for assignment.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// `true` if no items are waiting.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Drain all items from the queue, returning them in FIFO order.
    pub fn drain_all(&mut self) -> Vec<DroppedItem> {
        self.queue.drain(..).collect()
    }
}

// ─── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_increments_drop_id() {
        let mut q = LootQueue::new();
        let a = q.push(100, 1, 1001);
        let b = q.push(101, 2, 1001);
        assert_eq!(a.drop_id, 0);
        assert_eq!(b.drop_id, 1);
    }

    #[test]
    fn pop_is_fifo() {
        let mut q = LootQueue::new();
        q.push(10, 1, 1001);
        q.push(20, 1, 1002);
        assert_eq!(q.pop().unwrap().item_id, 10);
        assert_eq!(q.pop().unwrap().item_id, 20);
        assert!(q.pop().is_none());
    }

    #[test]
    fn is_empty_and_len() {
        let mut q = LootQueue::new();
        assert!(q.is_empty());
        q.push(1, 1, 1);
        assert_eq!(q.len(), 1);
        assert!(!q.is_empty());
        q.pop();
        assert!(q.is_empty());
    }

    #[test]
    fn drain_all_clears_queue() {
        let mut q = LootQueue::new();
        q.push(1, 1, 100);
        q.push(2, 1, 100);
        q.push(3, 1, 100);
        let drained = q.drain_all();
        assert_eq!(drained.len(), 3);
        assert!(q.is_empty());
    }

    #[test]
    fn peek_does_not_consume() {
        let mut q = LootQueue::new();
        q.push(42, 5, 999);
        assert_eq!(q.peek().unwrap().item_id, 42);
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn dropped_item_fields() {
        let mut q = LootQueue::new();
        let item = q.push(777, 3, 2048);
        assert_eq!(item.item_id, 777);
        assert_eq!(item.quantity, 3);
        assert_eq!(item.dropper_pid, 2048);
    }
}
