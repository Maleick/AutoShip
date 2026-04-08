//! Waypoint queue — stores and advances through a path of waypoints.

use textquest_common::nav::{IndexedQueue, Waypoint};

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

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::nav::Waypoint;

    #[test]
    fn new_queue_is_empty() {
        let q = WaypointQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
        assert!(q.current().is_none());
    }

    #[test]
    fn set_path_loads_waypoints() {
        let mut q = WaypointQueue::new();
        q.set_path(vec![
            Waypoint::new(0.0, 0.0, 0.0),
            Waypoint::new(10.0, 0.0, 0.0),
            Waypoint::new(20.0, 0.0, 0.0),
        ]);
        assert_eq!(q.len(), 3);
        assert!(!q.is_empty());
        assert_eq!(q.index(), 0);
    }

    #[test]
    fn current_returns_first_waypoint() {
        let mut q = WaypointQueue::new();
        q.set_path(vec![Waypoint::new(5.0, 10.0, 15.0)]);
        let wp = q.current().unwrap();
        assert!((wp.x - 5.0).abs() < f32::EPSILON);
        assert!((wp.y - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn advance_moves_to_next() {
        let mut q = WaypointQueue::new();
        q.set_path(vec![
            Waypoint::new(0.0, 0.0, 0.0),
            Waypoint::new(100.0, 0.0, 0.0),
        ]);
        assert!(q.advance());
        assert_eq!(q.index(), 1);
        let wp = q.current().unwrap();
        assert!((wp.x - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn advance_past_end_returns_false() {
        let mut q = WaypointQueue::new();
        q.set_path(vec![Waypoint::new(0.0, 0.0, 0.0)]);
        assert!(!q.advance()); // single item, can't advance
    }

    #[test]
    fn advance_on_empty_returns_false() {
        let mut q = WaypointQueue::new();
        assert!(!q.advance());
    }

    #[test]
    fn clear_resets_queue() {
        let mut q = WaypointQueue::new();
        q.set_path(vec![
            Waypoint::new(0.0, 0.0, 0.0),
            Waypoint::new(10.0, 0.0, 0.0),
        ]);
        q.advance();
        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
        assert_eq!(q.index(), 0);
        assert!(q.current().is_none());
    }

    #[test]
    fn set_path_resets_index() {
        let mut q = WaypointQueue::new();
        q.set_path(vec![
            Waypoint::new(0.0, 0.0, 0.0),
            Waypoint::new(10.0, 0.0, 0.0),
        ]);
        q.advance();
        assert_eq!(q.index(), 1);

        q.set_path(vec![Waypoint::new(50.0, 50.0, 0.0)]);
        assert_eq!(q.index(), 0);
        let wp = q.current().unwrap();
        assert!((wp.x - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn traverse_full_path() {
        let mut q = WaypointQueue::new();
        q.set_path(vec![
            Waypoint::new(0.0, 0.0, 0.0),
            Waypoint::new(10.0, 0.0, 0.0),
            Waypoint::new(20.0, 0.0, 0.0),
        ]);
        assert_eq!(q.current().unwrap().x, 0.0);
        assert!(q.advance());
        assert_eq!(q.current().unwrap().x, 10.0);
        assert!(q.advance());
        assert_eq!(q.current().unwrap().x, 20.0);
        assert!(!q.advance()); // end of path
    }
}
