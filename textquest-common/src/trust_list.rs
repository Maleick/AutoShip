//! Shared trust list for auto-acceptance and safety features.
//!
//! Used by AutoAccept, Rez, and Paranoid to determine whether a player
//! name should be trusted for automated interactions.

use serde::{Deserialize, Serialize};

/// Case-insensitive allowlist of trusted player names.
///
/// Shared across AutoAccept, Rez auto-acceptance, and Paranoid zone
/// monitoring so operators manage one list rather than three.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TrustList {
    entries: Vec<String>,
}

impl TrustList {
    /// Create a new trust list from raw name entries.
    #[must_use]
    pub fn new(entries: Vec<String>) -> Self {
        Self { entries }
    }

    /// Check if a player name is trusted (case-insensitive).
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        self.entries.iter().any(|e| e.to_ascii_lowercase() == lower)
    }

    /// Add a player to the trust list (no-op if already present).
    pub fn add(&mut self, name: String) {
        if !self.contains(&name) {
            self.entries.push(name);
        }
    }

    /// Remove a player from the trust list (case-insensitive).
    pub fn remove(&mut self, name: &str) {
        let lower = name.to_ascii_lowercase();
        self.entries.retain(|e| e.to_ascii_lowercase() != lower);
    }

    /// Number of entries in the trust list.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the trust list has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Raw name entries (preserves original casing for display).
    #[must_use]
    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}

impl From<Vec<String>> for TrustList {
    fn from(entries: Vec<String>) -> Self {
        Self::new(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_case_insensitive() {
        let list = TrustList::new(vec!["Soandso".into(), "Cleric".into()]);
        assert!(list.contains("soandso"));
        assert!(list.contains("CLERIC"));
        assert!(list.contains("Soandso"));
        assert!(!list.contains("Stranger"));
    }

    #[test]
    fn add_no_duplicate() {
        let mut list = TrustList::default();
        list.add("Healer".into());
        list.add("healer".into());
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn remove_case_insensitive() {
        let mut list = TrustList::new(vec!["Healer".into()]);
        list.remove("HEALER");
        assert!(list.is_empty());
    }

    #[test]
    fn empty_list_contains_nothing() {
        let list = TrustList::default();
        assert!(!list.contains("Anyone"));
        assert!(list.is_empty());
    }

    #[test]
    fn round_trip_serde() {
        let list = TrustList::new(vec!["Alpha".into(), "Beta".into()]);
        let json = serde_json::to_string(&list).expect("serialize");
        let decoded: TrustList = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, list);
    }
}
