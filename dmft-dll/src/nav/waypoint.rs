//! Waypoint queue — stores and advances through a path of waypoints.

use dmft_common::nav::{IndexedQueue, Waypoint};

/// A queue of waypoints to follow in order.
pub struct WaypointQueue {
    inner: IndexedQueue<Waypoint>,
}

impl WaypointQueue {
    /// Create an empty queue.
    pub fn new() -> Self {
        Self {
            inner: IndexedQueue::new(),
        }
    }

    /// Load a new path, resetting to the first waypoint.
    /// Reuses existing allocation when capacity is sufficient.
    pub fn set_path(&mut self, waypoints: Vec<Waypoint>) {
        self.inner.set_items(waypoints);
    }

    /// Get the current target waypoint, if any remain.
    pub fn current(&self) -> Option<&Waypoint> {
        self.inner.current()
    }

    /// Advance to the next waypoint. Returns true if there is a next one.
    pub fn advance(&mut self) -> bool {
        self.inner.advance()
    }

    /// Current index in the path.
    pub fn index(&self) -> usize {
        self.inner.index()
    }

    /// Total number of waypoints.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the queue is empty (no path loaded).
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Clear the path.
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}
