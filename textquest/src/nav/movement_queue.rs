//! Movement queue tracking for the orchestrator.
//!
//! Tracks pending movement commands per client, provides queue state snapshots,
//! and validates queue flushing before zone transitions.

use std::collections::VecDeque;
use textquest_common::types::ClientId;
use textquest_common::nav::Waypoint;

/// A single pending movement command.
#[derive(Debug, Clone, PartialEq)]
pub enum MovementCommand {
    /// Move to an absolute position.
    MoveTo { x: f32, y: f32, z: f32 },
    /// Stop all movement.
    Stop,
}

impl MovementCommand {
    /// Create a MoveTo command.
    pub fn move_to(x: f32, y: f32, z: f32) -> Self {
        Self::MoveTo { x, y, z }
    }

    /// Create a Stop command.
    pub fn stop() -> Self {
        Self::Stop
    }
}

/// Tracks movement commands queued for a single client.
#[derive(Debug, Clone)]
pub struct ClientMovementQueue {
    /// Client ID.
    pub client_id: ClientId,
    /// Pending movement commands.
    commands: VecDeque<MovementCommand>,
    /// Total commands dropped since creation.
    pub dropped_count: u32,
}

impl ClientMovementQueue {
    /// Create a new empty queue for a client.
    pub fn new(client_id: ClientId) -> Self {
        Self {
            client_id,
            commands: VecDeque::new(),
            dropped_count: 0,
        }
    }

    /// Add a movement command to the queue.
    pub fn enqueue(&mut self, cmd: MovementCommand) {
        self.commands.push_back(cmd);
    }

    /// Get the number of pending commands.
    pub fn pending_count(&self) -> u32 {
        self.commands.len() as u32
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Peek at the next command without removing it.
    pub fn peek(&self) -> Option<&MovementCommand> {
        self.commands.front()
    }

    /// Dequeue and return the next command.
    pub fn dequeue(&mut self) -> Option<MovementCommand> {
        self.commands.pop_front()
    }

    /// Flush all pending commands and return the count.
    ///
    /// Records the dropped commands in the queue's history.
    pub fn flush(&mut self) -> u32 {
        let count = self.commands.len() as u32;
        self.commands.clear();
        self.dropped_count += count;
        count
    }

    /// Get a snapshot of all pending commands (for logging/debugging).
    pub fn pending_snapshot(&self) -> Vec<MovementCommand> {
        self.commands.iter().cloned().collect()
    }
}

/// Tracks movement queues for all active clients.
#[derive(Debug, Default)]
pub struct MovementQueueManager {
    /// Per-client movement queues.
    queues: std::collections::HashMap<ClientId, ClientMovementQueue>,
}

impl MovementQueueManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new client's movement queue.
    pub fn register_client(&mut self, client_id: ClientId) {
        self.queues.insert(client_id, ClientMovementQueue::new(client_id));
    }

    /// Unregister a client's movement queue.
    pub fn unregister_client(&mut self, client_id: ClientId) {
        self.queues.remove(&client_id);
    }

    /// Get mutable reference to a client's queue.
    pub fn get_mut(&mut self, client_id: ClientId) -> Option<&mut ClientMovementQueue> {
        self.queues.get_mut(&client_id)
    }

    /// Get immutable reference to a client's queue.
    pub fn get(&self, client_id: ClientId) -> Option<&ClientMovementQueue> {
        self.queues.get(&client_id)
    }

    /// Enqueue a movement command for a client.
    pub fn enqueue(&mut self, client_id: ClientId, cmd: MovementCommand) {
        self.queues
            .entry(client_id)
            .or_insert_with(|| ClientMovementQueue::new(client_id))
            .enqueue(cmd);
    }

    /// Flush all pending commands for a client.
    ///
    /// Returns the number of commands that were dropped.
    pub fn flush(&mut self, client_id: ClientId) -> u32 {
        self.queues
            .get_mut(&client_id)
            .map(|q| q.flush())
            .unwrap_or(0)
    }

    /// Get total pending commands across all clients.
    pub fn total_pending(&self) -> u32 {
        self.queues.values().map(|q| q.pending_count()).sum()
    }

    /// Get all client IDs with pending commands.
    pub fn clients_with_pending(&self) -> Vec<ClientId> {
        self.queues
            .iter()
            .filter(|(_, q)| !q.is_empty())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get a snapshot of all queues (for logging/diagnostics).
    pub fn snapshot(&self) -> Vec<(ClientId, u32, Vec<MovementCommand>)> {
        self.queues
            .iter()
            .map(|(id, q)| (*id, q.pending_count(), q.pending_snapshot()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_queue_is_empty() {
        let q = ClientMovementQueue::new(1);
        assert_eq!(q.pending_count(), 0);
        assert!(q.is_empty());
        assert_eq!(q.dropped_count, 0);
    }

    #[test]
    fn enqueue_increases_pending_count() {
        let mut q = ClientMovementQueue::new(1);
        q.enqueue(MovementCommand::move_to(100.0, 50.0, 0.0));
        assert_eq!(q.pending_count(), 1);
        assert!(!q.is_empty());
    }

    #[test]
    fn peek_does_not_remove() {
        let mut q = ClientMovementQueue::new(1);
        let cmd = MovementCommand::move_to(100.0, 50.0, 0.0);
        q.enqueue(cmd.clone());
        assert_eq!(q.peek(), Some(&cmd));
        assert_eq!(q.pending_count(), 1);
    }

    #[test]
    fn dequeue_removes_and_returns() {
        let mut q = ClientMovementQueue::new(1);
        let cmd = MovementCommand::move_to(100.0, 50.0, 0.0);
        q.enqueue(cmd.clone());
        assert_eq!(q.dequeue(), Some(cmd));
        assert_eq!(q.pending_count(), 0);
        assert!(q.is_empty());
    }

    #[test]
    fn flush_clears_queue_and_records_count() {
        let mut q = ClientMovementQueue::new(1);
        q.enqueue(MovementCommand::move_to(100.0, 50.0, 0.0));
        q.enqueue(MovementCommand::stop());
        q.enqueue(MovementCommand::move_to(200.0, 100.0, 0.0));

        assert_eq!(q.pending_count(), 3);
        assert_eq!(q.dropped_count, 0);

        let dropped = q.flush();
        assert_eq!(dropped, 3);
        assert_eq!(q.dropped_count, 3);
        assert!(q.is_empty());
    }

    #[test]
    fn multiple_flushes_accumulate_dropped_count() {
        let mut q = ClientMovementQueue::new(1);

        // First flush
        q.enqueue(MovementCommand::move_to(100.0, 50.0, 0.0));
        q.enqueue(MovementCommand::move_to(200.0, 100.0, 0.0));
        let dropped1 = q.flush();
        assert_eq!(dropped1, 2);
        assert_eq!(q.dropped_count, 2);

        // Second flush
        q.enqueue(MovementCommand::stop());
        let dropped2 = q.flush();
        assert_eq!(dropped2, 1);
        assert_eq!(q.dropped_count, 3);
    }

    #[test]
    fn manager_registers_and_unregisters_clients() {
        let mut mgr = MovementQueueManager::new();
        mgr.register_client(1);
        assert!(mgr.get(1).is_some());

        mgr.unregister_client(1);
        assert!(mgr.get(1).is_none());
    }

    #[test]
    fn manager_enqueue_creates_queue_if_needed() {
        let mut mgr = MovementQueueManager::new();
        mgr.enqueue(1, MovementCommand::move_to(100.0, 50.0, 0.0));
        assert!(mgr.get(1).is_some());
        assert_eq!(mgr.get(1).unwrap().pending_count(), 1);
    }

    #[test]
    fn manager_flush_drops_commands() {
        let mut mgr = MovementQueueManager::new();
        mgr.register_client(1);
        mgr.enqueue(1, MovementCommand::move_to(100.0, 50.0, 0.0));
        mgr.enqueue(1, MovementCommand::move_to(200.0, 100.0, 0.0));

        let dropped = mgr.flush(1);
        assert_eq!(dropped, 2);
        assert_eq!(mgr.get(1).unwrap().is_empty(), true);
    }

    #[test]
    fn manager_total_pending() {
        let mut mgr = MovementQueueManager::new();
        mgr.enqueue(1, MovementCommand::move_to(100.0, 50.0, 0.0));
        mgr.enqueue(1, MovementCommand::move_to(200.0, 100.0, 0.0));
        mgr.enqueue(2, MovementCommand::stop());
        mgr.enqueue(3, MovementCommand::move_to(50.0, 25.0, 0.0));
        mgr.enqueue(3, MovementCommand::move_to(75.0, 75.0, 0.0));

        assert_eq!(mgr.total_pending(), 5);
    }

    #[test]
    fn manager_clients_with_pending() {
        let mut mgr = MovementQueueManager::new();
        mgr.enqueue(1, MovementCommand::move_to(100.0, 50.0, 0.0));
        mgr.enqueue(2, MovementCommand::stop());
        mgr.register_client(3); // Register but don't enqueue

        let mut with_pending: Vec<_> = mgr.clients_with_pending();
        with_pending.sort();
        assert_eq!(with_pending, vec![1, 2]);
    }

    #[test]
    fn manager_snapshot() {
        let mut mgr = MovementQueueManager::new();
        mgr.enqueue(1, MovementCommand::move_to(100.0, 50.0, 0.0));
        mgr.enqueue(1, MovementCommand::stop());
        mgr.enqueue(2, MovementCommand::move_to(200.0, 100.0, 0.0));

        let snapshot = mgr.snapshot();
        assert_eq!(snapshot.len(), 2);

        // Find entry for client 1
        let entry1 = snapshot.iter().find(|(id, _, _)| *id == 1).unwrap();
        assert_eq!(entry1.1, 2); // pending count
        assert_eq!(entry1.2.len(), 2); // command count
    }

    #[test]
    fn movement_command_constructors() {
        let move_cmd = MovementCommand::move_to(100.0, 50.0, 0.0);
        assert!(matches!(move_cmd, MovementCommand::MoveTo { .. }));

        let stop_cmd = MovementCommand::stop();
        assert_eq!(stop_cmd, MovementCommand::Stop);
    }

    #[test]
    fn pending_snapshot_reflects_queue_state() {
        let mut q = ClientMovementQueue::new(1);
        q.enqueue(MovementCommand::move_to(100.0, 50.0, 0.0));
        q.enqueue(MovementCommand::stop());

        let snapshot = q.pending_snapshot();
        assert_eq!(snapshot.len(), 2);
        assert!(matches!(snapshot[0], MovementCommand::MoveTo { .. }));
        assert_eq!(snapshot[1], MovementCommand::Stop);
    }

    #[test]
    fn flush_empty_queue_returns_zero() {
        let mut q = ClientMovementQueue::new(1);
        let dropped = q.flush();
        assert_eq!(dropped, 0);
        assert_eq!(q.dropped_count, 0);
    }

    #[test]
    fn fifo_order_is_preserved() {
        let mut q = ClientMovementQueue::new(1);
        q.enqueue(MovementCommand::move_to(1.0, 1.0, 1.0));
        q.enqueue(MovementCommand::move_to(2.0, 2.0, 2.0));
        q.enqueue(MovementCommand::move_to(3.0, 3.0, 3.0));

        assert_eq!(
            q.dequeue(),
            Some(MovementCommand::move_to(1.0, 1.0, 1.0))
        );
        assert_eq!(
            q.dequeue(),
            Some(MovementCommand::move_to(2.0, 2.0, 2.0))
        );
        assert_eq!(
            q.dequeue(),
            Some(MovementCommand::move_to(3.0, 3.0, 3.0))
        );
    }
}
