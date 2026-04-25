//! Runtime non-persistent setting overrides — rgmercs `tempset` parity (gap #3).
//!
//! `TempSetStore` holds per-character key/value overrides that survive until
//! script restart. Overrides layer on top of TOML config without writing to
//! disk. The web UI surfaces them with a "RUNTIME OVERRIDE" banner.
//!
//! # CLI surface
//! ```text
//! textquest tempset   <character> <knob> <value>   # set override
//! textquest cleartempset <character> <knob>         # remove one override
//! textquest cleartempall <character>                # remove all overrides
//! ```

use std::collections::HashMap;

/// All runtime overrides for a single character, keyed by setting name.
pub type CharacterOverrides = HashMap<String, String>;

/// Global store of per-character runtime overrides.
///
/// Each entry maps `character_name → (knob → value)`. Overrides are
/// ephemeral: they exist only in memory and are lost on process restart.
#[derive(Debug, Default, Clone)]
pub struct TempSetStore {
    inner: HashMap<String, CharacterOverrides>,
}

impl TempSetStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set an override for `character`/`knob`. Returns the previous value if
    /// one existed.
    pub fn set(&mut self, character: &str, knob: &str, value: String) -> Option<String> {
        self.inner
            .entry(character.to_owned())
            .or_default()
            .insert(knob.to_owned(), value)
    }

    /// Get the current override value for `character`/`knob`, if any.
    #[must_use]
    pub fn get(&self, character: &str, knob: &str) -> Option<&str> {
        self.inner
            .get(character)
            .and_then(|m| m.get(knob))
            .map(String::as_str)
    }

    /// Remove a single override. Returns the removed value if it existed.
    pub fn clear_one(&mut self, character: &str, knob: &str) -> Option<String> {
        self.inner
            .get_mut(character)
            .and_then(|m| m.remove(knob))
    }

    /// Remove all overrides for `character`. Returns how many were cleared.
    pub fn clear_all(&mut self, character: &str) -> usize {
        self.inner
            .remove(character)
            .map(|m| m.len())
            .unwrap_or(0)
    }

    /// Return all overrides for `character` (empty if none).
    #[must_use]
    pub fn list(&self, character: &str) -> &CharacterOverrides {
        static EMPTY: std::sync::OnceLock<CharacterOverrides> = std::sync::OnceLock::new();
        self.inner
            .get(character)
            .unwrap_or_else(|| EMPTY.get_or_init(CharacterOverrides::new))
    }

    /// Iterate over every character that has at least one override.
    pub fn iter_characters(&self) -> impl Iterator<Item = (&str, &CharacterOverrides)> {
        self.inner.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// True when no overrides exist for any character.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let mut store = TempSetStore::new();
        store.set("Warrior1", "rest_mana_pct", "40".into());
        assert_eq!(store.get("Warrior1", "rest_mana_pct"), Some("40"));
        assert_eq!(store.get("Cleric1", "rest_mana_pct"), None);
    }

    #[test]
    fn clear_one_returns_value() {
        let mut store = TempSetStore::new();
        store.set("Enc1", "pull_radius", "300".into());
        let removed = store.clear_one("Enc1", "pull_radius");
        assert_eq!(removed, Some("300".into()));
        assert!(store.get("Enc1", "pull_radius").is_none());
    }

    #[test]
    fn clear_all_removes_character() {
        let mut store = TempSetStore::new();
        store.set("Mag1", "a", "1".into());
        store.set("Mag1", "b", "2".into());
        let count = store.clear_all("Mag1");
        assert_eq!(count, 2);
        assert!(store.list("Mag1").is_empty());
    }

    #[test]
    fn is_empty_initially() {
        assert!(TempSetStore::new().is_empty());
    }
}
