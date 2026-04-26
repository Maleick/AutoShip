//! Runtime non-persistent setting overrides — rgmercs `tempset` parity.
//!
//! `TempsetStore` holds in-memory overrides that shadow the on-disk config
//! without writing to disk. Overrides are lost on restart (intentional).
//!
//! # CLI surface (orchestrator, gap #3)
//! ```text
//! textquest tempset <character> <knob> <value>
//! textquest cleartempset <character> <knob>
//! textquest cleartempall <character>
//! ```
//!
//! # IPC surface (DLL and orchestrator)
//! `Command::Tempset`, `Command::ClearTempset`, `Command::ClearTempsetAll` —
//! see `crate::ipc::Command`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single runtime override entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TempsetEntry {
    /// Setting key (e.g., `"pull_radius"`, `"rest_mana_pct"`).
    pub key: String,
    /// Value as a string — coerced to the target type on read.
    pub value: String,
}

/// In-memory store for runtime non-persistent setting overrides.
///
/// One store per character. Overrides shadow the on-disk config without
/// writing to disk. Lost on restart.
///
/// The web UI should display active overrides with a "RUNTIME OVERRIDE" banner.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TempsetStore {
    /// Active overrides keyed by setting name.
    overrides: HashMap<String, String>,
}

impl TempsetStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self {
            overrides: HashMap::new(),
        }
    }

    /// Set an override. Returns the previous value if one existed.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) -> Option<String> {
        self.overrides.insert(key.into(), value.into())
    }

    /// Get the current override value for `key`, if any.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.overrides.get(key).map(String::as_str)
    }

    /// Remove the override for `key`. Returns the removed value if present.
    pub fn clear(&mut self, key: &str) -> Option<String> {
        self.overrides.remove(key)
    }

    /// Remove all overrides.
    pub fn clear_all(&mut self) {
        self.overrides.clear();
    }

    /// Returns `true` if there are no active overrides.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    /// Returns the number of active overrides.
    #[must_use]
    pub fn len(&self) -> usize {
        self.overrides.len()
    }

    /// Iterate over all active overrides as `(key, value)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.overrides.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Return a snapshot of all active overrides sorted by key (for display).
    #[must_use]
    pub fn entries_sorted(&self) -> Vec<TempsetEntry> {
        let mut entries: Vec<TempsetEntry> = self
            .overrides
            .iter()
            .map(|(k, v)| TempsetEntry {
                key: k.clone(),
                value: v.clone(),
            })
            .collect();
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let mut store = TempsetStore::new();
        assert!(store.get("pull_radius").is_none());
        store.set("pull_radius", "200.0");
        assert_eq!(store.get("pull_radius"), Some("200.0"));
    }

    #[test]
    fn set_returns_previous_value() {
        let mut store = TempsetStore::new();
        assert_eq!(store.set("mana_floor", "20"), None);
        assert_eq!(store.set("mana_floor", "30"), Some("20".into()));
    }

    #[test]
    fn clear_removes_key() {
        let mut store = TempsetStore::new();
        store.set("rest_mana_pct", "60");
        assert_eq!(store.clear("rest_mana_pct"), Some("60".into()));
        assert!(store.get("rest_mana_pct").is_none());
    }

    #[test]
    fn clear_missing_key_returns_none() {
        let mut store = TempsetStore::new();
        assert_eq!(store.clear("nonexistent"), None);
    }

    #[test]
    fn clear_all_empties_store() {
        let mut store = TempsetStore::new();
        store.set("a", "1");
        store.set("b", "2");
        assert_eq!(store.len(), 2);
        store.clear_all();
        assert!(store.is_empty());
    }

    #[test]
    fn entries_sorted_returns_alphabetical_order() {
        let mut store = TempsetStore::new();
        store.set("pull_radius", "200.0");
        store.set("mana_floor", "20");
        store.set("rest_mana_pct", "60");
        let entries = store.entries_sorted();
        assert_eq!(entries[0].key, "mana_floor");
        assert_eq!(entries[1].key, "pull_radius");
        assert_eq!(entries[2].key, "rest_mana_pct");
    }

    #[test]
    fn serde_roundtrip() {
        let mut store = TempsetStore::new();
        store.set("leash_radius", "80.0");
        let json = serde_json::to_string(&store).unwrap();
        let restored: TempsetStore = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.get("leash_radius"), Some("80.0"));
    }
}
