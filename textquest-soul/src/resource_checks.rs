//! Resource leak detection and cleanup verification for the Soul Engine.
//!
//! Provides `ResourceGuard` which captures resource counts at construction,
//! and can verify that resources (SQLite connections, personality caches,
//! event queues) are properly cleaned up on `SoulCoordinator` shutdown.

/// A description of a detected resource leak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLeak {
    /// Human-readable name of the leaked resource.
    pub resource: String,
    /// Count at guard creation.
    pub initial_count: usize,
    /// Count at check time (should be 0 after cleanup).
    pub current_count: usize,
    /// Brief description of what the leak means.
    pub description: String,
}

impl ResourceLeak {
    fn new(resource: &str, initial: usize, current: usize, description: &str) -> Self {
        Self {
            resource: resource.to_string(),
            initial_count: initial,
            current_count: current,
            description: description.to_string(),
        }
    }
}

/// Snapshot of resource counts taken at guard creation.
#[derive(Debug, Clone)]
struct ResourceSnapshot {
    db_connection_count: usize,
    personality_cache_size: usize,
    event_queue_depth: usize,
    registered_character_count: usize,
}

/// Captures resource state at creation and verifies cleanup on drop or on
/// demand via [`ResourceGuard::check_leaks`].
///
/// # Example
///
/// ```rust
/// use textquest_soul::resource_checks::{ResourceGuard, ResourceState};
///
/// let state = ResourceState {
///     db_connection_count: 1,
///     personality_cache_size: 3,
///     event_queue_depth: 0,
///     registered_character_count: 3,
/// };
/// let guard = ResourceGuard::new(state);
/// // ... shutdown coordinator ...
/// let after = ResourceState::zeroed();
/// let leaks = guard.check_leaks(&after);
/// assert!(leaks.is_empty());
/// ```
pub struct ResourceGuard {
    initial: ResourceSnapshot,
}

/// Live resource state to be checked against the initial snapshot.
#[derive(Debug, Clone, Default)]
pub struct ResourceState {
    /// Number of open SQLite connections (1 = normal, 0 = closed).
    pub db_connection_count: usize,
    /// Number of entries in the personality cache (should be 0 after clearing).
    pub personality_cache_size: usize,
    /// Number of pending events in the soul event queue.
    pub event_queue_depth: usize,
    /// Number of characters still registered in the coordinator.
    pub registered_character_count: usize,
}

impl ResourceState {
    /// Return a zeroed state representing fully cleaned-up resources.
    pub fn zeroed() -> Self {
        Self::default()
    }
}

impl ResourceGuard {
    /// Capture initial resource counts.
    pub fn new(state: ResourceState) -> Self {
        Self {
            initial: ResourceSnapshot {
                db_connection_count: state.db_connection_count,
                personality_cache_size: state.personality_cache_size,
                event_queue_depth: state.event_queue_depth,
                registered_character_count: state.registered_character_count,
            },
        }
    }

    /// Check for resource leaks by comparing `current` state against zero
    /// (expected post-shutdown state). Returns one [`ResourceLeak`] per
    /// resource that is not fully cleaned up.
    pub fn check_leaks(&self, current: &ResourceState) -> Vec<ResourceLeak> {
        let mut leaks = Vec::new();

        if current.db_connection_count > 0 {
            leaks.push(ResourceLeak::new(
                "db_connection",
                self.initial.db_connection_count,
                current.db_connection_count,
                "SQLite connection was not closed during shutdown",
            ));
        }

        if current.personality_cache_size > 0 {
            leaks.push(ResourceLeak::new(
                "personality_cache",
                self.initial.personality_cache_size,
                current.personality_cache_size,
                "Personality cache was not cleared during shutdown",
            ));
        }

        if current.event_queue_depth > 0 {
            leaks.push(ResourceLeak::new(
                "event_queue",
                self.initial.event_queue_depth,
                current.event_queue_depth,
                "Soul event queue was not drained during shutdown",
            ));
        }

        if current.registered_character_count > 0 {
            leaks.push(ResourceLeak::new(
                "registered_characters",
                self.initial.registered_character_count,
                current.registered_character_count,
                "Character registrations were not removed during shutdown",
            ));
        }

        leaks
    }

    /// Return the initial snapshot (for diagnostics).
    pub fn initial_snapshot(&self) -> (usize, usize, usize, usize) {
        (
            self.initial.db_connection_count,
            self.initial.personality_cache_size,
            self.initial.event_queue_depth,
            self.initial.registered_character_count,
        )
    }
}

impl Drop for ResourceGuard {
    fn drop(&mut self) {
        // On drop we emit a tracing warning for each resource that was
        // non-zero at guard creation and is still expected to be leaked,
        // since we cannot call check_leaks here (no current state).
        // Callers should call check_leaks() explicitly before dropping.
        if self.initial.db_connection_count > 0 {
            tracing::debug!(
                "ResourceGuard dropped: initial db_connection_count={}",
                self.initial.db_connection_count
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running_state(chars: usize) -> ResourceState {
        ResourceState {
            db_connection_count: 1,
            personality_cache_size: chars,
            event_queue_depth: 0,
            registered_character_count: chars,
        }
    }

    #[test]
    fn test_no_leaks_after_clean_shutdown() {
        let guard = ResourceGuard::new(running_state(3));
        let after = ResourceState::zeroed();
        let leaks = guard.check_leaks(&after);
        assert!(leaks.is_empty(), "expected no leaks, got: {leaks:?}");
    }

    #[test]
    fn test_detects_open_db_connection() {
        let guard = ResourceGuard::new(running_state(2));
        let after = ResourceState {
            db_connection_count: 1,
            ..ResourceState::zeroed()
        };
        let leaks = guard.check_leaks(&after);
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].resource, "db_connection");
        assert_eq!(leaks[0].current_count, 1);
    }

    #[test]
    fn test_detects_personality_cache_not_cleared() {
        let guard = ResourceGuard::new(running_state(4));
        let after = ResourceState {
            personality_cache_size: 4,
            ..ResourceState::zeroed()
        };
        let leaks = guard.check_leaks(&after);
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].resource, "personality_cache");
        assert_eq!(leaks[0].initial_count, 4);
        assert_eq!(leaks[0].current_count, 4);
    }

    #[test]
    fn test_detects_event_queue_not_drained() {
        let guard = ResourceGuard::new(ResourceState {
            db_connection_count: 1,
            personality_cache_size: 0,
            event_queue_depth: 5,
            registered_character_count: 0,
        });
        let after = ResourceState {
            event_queue_depth: 5,
            ..ResourceState::zeroed()
        };
        let leaks = guard.check_leaks(&after);
        assert_eq!(leaks.len(), 1);
        assert_eq!(leaks[0].resource, "event_queue");
        assert_eq!(leaks[0].current_count, 5);
    }

    #[test]
    fn test_detects_multiple_leaks_simultaneously() {
        let guard = ResourceGuard::new(running_state(3));
        let after = ResourceState {
            db_connection_count: 1,
            personality_cache_size: 3,
            event_queue_depth: 2,
            registered_character_count: 3,
        };
        let leaks = guard.check_leaks(&after);
        assert_eq!(leaks.len(), 4, "expected 4 leaks, got: {leaks:?}");
        let names: Vec<&str> = leaks.iter().map(|l| l.resource.as_str()).collect();
        assert!(names.contains(&"db_connection"));
        assert!(names.contains(&"personality_cache"));
        assert!(names.contains(&"event_queue"));
        assert!(names.contains(&"registered_characters"));
    }

    #[test]
    fn test_initial_snapshot_captures_state() {
        let state = ResourceState {
            db_connection_count: 2,
            personality_cache_size: 7,
            event_queue_depth: 3,
            registered_character_count: 7,
        };
        let guard = ResourceGuard::new(state);
        let (db, cache, queue, chars) = guard.initial_snapshot();
        assert_eq!(db, 2);
        assert_eq!(cache, 7);
        assert_eq!(queue, 3);
        assert_eq!(chars, 7);
    }

    #[test]
    fn test_resource_leak_fields_populated_correctly() {
        let guard = ResourceGuard::new(running_state(5));
        let after = ResourceState {
            registered_character_count: 5,
            ..ResourceState::zeroed()
        };
        let leaks = guard.check_leaks(&after);
        assert_eq!(leaks.len(), 1);
        let leak = &leaks[0];
        assert_eq!(leak.resource, "registered_characters");
        assert_eq!(leak.initial_count, 5);
        assert_eq!(leak.current_count, 5);
        assert!(!leak.description.is_empty());
    }
}
