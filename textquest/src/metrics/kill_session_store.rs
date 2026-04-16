use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{KillTracker, KillRecord};

/// A multi-session store that keeps separate KillTracker instances per character.
/// This enables per-character historical tracking while allowing a single
/// global store to persist across sessions.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct KillSessionStore {
    /// Keyed by character name (or unique per-user identifier).
    sessions: HashMap<String, KillTracker>,
}

impl KillSessionStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Start a new session for a character if not already present.
    pub fn start_session(&mut self, character: String, session_start: i64) {
        self.sessions
            .entry(character)
            .or_insert_with(|| KillTracker::new(session_start));
    }

    /// Get mutable reference to a character's session, if exists.
    pub fn get_session_mut(&mut self, character: &str) -> Option<&mut KillTracker> {
        self.sessions.get_mut(character)
    }

    /// End and remove a character's session from the store, returning it if present.
    pub fn end_session(&mut self, character: &str) -> Option<KillTracker> {
        self.sessions.remove(character)
    }

    /// Merge another store into this one (shallow merge of sessions).
    pub fn merge(&mut self, other: KillSessionStore) {
        for (k, v) in other.sessions {
            self.sessions
                .entry(k)
                .or_insert_with(|| KillTracker::new(0))
                .clone_from(&v);
        }
    }

    /// Historical snapshot: map character -> all kills recorded in their sessions.
    pub fn history(&self) -> HashMap<String, Vec<KillRecord>> {
        self.sessions
            .iter()
            .map(|(k, v)| (k.clone(), v.all_kills().to_vec()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record() -> KillRecord {
        KillRecord {
            mob_name: "goblin".to_string(),
            mob_level: 3,
            zone: "testzone".to_string(),
            killer_pid: 1,
            kill_time_ms: 1000,
            total_damage: 100,
            timestamp: 1,
        }
    }

    #[test]
    fn start_and_store_sessions_isolated() {
        let mut store = KillSessionStore::new();
        store.start_session("Alice".to_string(), 0);
        store.start_session("Bob".to_string(), 0);

        // Add kills to Alice
        if let Some(alice) = store.get_session_mut("Alice") {
            alice.record_kill(sample_record());
        }
        // Add a different kill to Bob
        if let Some(bob) = store.get_session_mut("Bob") {
            bob.record_kill(sample_record());
            bob.record_kill(sample_record());
        }

        // Verify isolation
        assert_eq!(store.history().get("Alice").unwrap().len(), 1);
        assert_eq!(store.history().get("Bob").unwrap().len(), 2);
    }
}
