//! Persistent named-waypoint store for `/nav waypoint` save/recall/delete/list.
//!
//! Waypoints are serialized as a JSON array to
//! `%TEMP%/textquest/waypoints.json` (override via `TEXTQUEST_WAYPOINT_STORE`).
//! The store is loaded lazily on first use and held in a process-wide `Mutex`.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use textquest_common::nav::{NamedWaypoint, Waypoint};

// ── path resolution ──────────────────────────────────────────────────────────

fn default_store_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("TEXTQUEST_WAYPOINT_STORE") {
        return PathBuf::from(override_path);
    }

    std::env::temp_dir()
        .join("textquest")
        .join("waypoints.json")
}

// ── name normalization ────────────────────────────────────────────────────────

/// Normalize a waypoint name for storage and lookup (trim + lowercase).
pub fn normalize_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(String::from("Waypoint name cannot be empty"));
    }
    if trimmed.len() > 64 {
        return Err(String::from(
            "Waypoint name must be at most 64 characters long",
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

// ── store ─────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct WaypointStore {
    path: PathBuf,
    waypoints: BTreeMap<String, NamedWaypoint>,
}

impl WaypointStore {
    fn load(path: PathBuf) -> Self {
        let mut store = Self {
            path,
            waypoints: BTreeMap::new(),
        };

        if let Err(error) = store.load_from_disk() {
            tracing::warn!(%error, "Failed to load waypoint store — starting empty");
        }

        store
    }

    fn load_from_disk(&mut self) -> Result<(), String> {
        if !self.path.exists() {
            return Ok(());
        }

        let bytes = fs::read(&self.path).map_err(|e| e.to_string())?;
        let parsed: Vec<NamedWaypoint> =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;

        self.waypoints.clear();
        for wp in parsed {
            if let Ok(key) = normalize_name(&wp.name) {
                self.waypoints.insert(key, wp);
            }
        }

        Ok(())
    }

    fn persist(&self) -> Result<(), String> {
        let data: Vec<&NamedWaypoint> = self.waypoints.values().collect();
        let serialized = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        fs::write(&self.path, serialized).map_err(|e| e.to_string())
    }

    fn upsert(&mut self, waypoint: NamedWaypoint) -> Result<NamedWaypoint, String> {
        let key = normalize_name(&waypoint.name)?;
        self.waypoints.insert(key, waypoint.clone());
        self.persist().map(|_| waypoint)
    }

    fn delete(&mut self, name: &str) -> Result<bool, String> {
        let key = normalize_name(name)?;
        let removed = self.waypoints.remove(&key).is_some();
        if removed {
            self.persist()?;
        }
        Ok(removed)
    }

    fn recall(&self, name: &str) -> Option<NamedWaypoint> {
        let key = normalize_name(name).ok()?;
        self.waypoints.get(&key).cloned()
    }

    fn list(&self) -> Vec<NamedWaypoint> {
        self.waypoints.values().cloned().collect()
    }
}

// ── global store access ───────────────────────────────────────────────────────

static STORE: OnceLock<Mutex<WaypointStore>> = OnceLock::new();

fn store() -> &'static Mutex<WaypointStore> {
    STORE.get_or_init(|| Mutex::new(WaypointStore::load(default_store_path())))
}

// ── public API ────────────────────────────────────────────────────────────────

/// Save a named waypoint at `position` in `zone`, overwriting any existing
/// entry with the same normalized name.
pub fn save(name: &str, position: Waypoint, zone: String) -> Result<NamedWaypoint, String> {
    let wp = NamedWaypoint::new(name, position, zone);
    store()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .upsert(wp)
}

/// Look up a waypoint by name. Returns `None` if not found.
pub fn recall(name: &str) -> Option<NamedWaypoint> {
    store()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .recall(name)
}

/// Delete a waypoint by name. Returns `Ok(true)` if it existed, `Ok(false)`
/// if it was not found, or `Err` if normalization fails.
pub fn delete(name: &str) -> Result<bool, String> {
    store()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .delete(name)
}

/// List all stored waypoints in sorted-name order.
pub fn list() -> Vec<NamedWaypoint> {
    store()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .list()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_wp(name: &str, zone: &str) -> NamedWaypoint {
        NamedWaypoint::new(name, Waypoint::new(1.0, 2.0, 3.0), zone)
    }

    #[test]
    fn normalize_name_trims_and_lowercases() {
        assert_eq!(normalize_name("  Camp1  ").unwrap(), "camp1");
        assert_eq!(normalize_name("PULL_SPOT").unwrap(), "pull_spot");
    }

    #[test]
    fn normalize_name_rejects_empty() {
        assert!(normalize_name("").is_err());
        assert!(normalize_name("   ").is_err());
    }

    #[test]
    fn normalize_name_rejects_too_long() {
        let long = "a".repeat(65);
        assert!(normalize_name(&long).is_err());
    }

    #[test]
    fn waypoint_store_upsert_and_recall() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut store = WaypointStore::load(tmp.path().to_path_buf());

        let wp = make_wp("camp1", "gfaydark");
        store.upsert(wp.clone()).unwrap();

        let recalled = store.recall("camp1").unwrap();
        assert_eq!(recalled.name, "camp1");
        assert_eq!(recalled.zone, "gfaydark");
    }

    #[test]
    fn waypoint_store_delete_removes_entry() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut store = WaypointStore::load(tmp.path().to_path_buf());

        store.upsert(make_wp("spot", "highpass")).unwrap();
        assert!(store.delete("spot").unwrap());
        assert!(!store.delete("spot").unwrap());
        assert!(store.recall("spot").is_none());
    }

    #[test]
    fn waypoint_store_list_returns_sorted() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut store = WaypointStore::load(tmp.path().to_path_buf());

        store.upsert(make_wp("zzz", "zone")).unwrap();
        store.upsert(make_wp("aaa", "zone")).unwrap();
        store.upsert(make_wp("mmm", "zone")).unwrap();

        let names: Vec<_> = store.list().into_iter().map(|w| w.name).collect();
        assert_eq!(names, vec!["aaa", "mmm", "zzz"]);
    }

    #[test]
    fn waypoint_store_normalizes_on_upsert() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let mut store = WaypointStore::load(tmp.path().to_path_buf());

        store.upsert(make_wp("Camp1", "zone")).unwrap();
        // Lookup by lowercase should succeed; name field preserves original
        assert!(store.recall("camp1").is_some());
    }

    #[test]
    fn waypoint_store_persists_and_reloads() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        {
            let mut store = WaypointStore::load(path.clone());
            store.upsert(make_wp("base", "crushbone")).unwrap();
        }

        let reloaded = WaypointStore::load(path);
        let wp = reloaded.recall("base").unwrap();
        assert_eq!(wp.zone, "crushbone");
    }
}
