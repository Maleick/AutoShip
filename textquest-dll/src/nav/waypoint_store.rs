use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use textquest_common::nav::{NamedWaypoint, Waypoint};

/// Default path for persisted waypoints.
fn default_store_path() -> PathBuf {
    if let Ok(override_path) = std::env::var("TEXTQUEST_WAYPOINT_STORE") {
        return PathBuf::from(override_path);
    }

    std::env::temp_dir()
        .join("textquest")
        .join("waypoints.json")
}

/// Normalize waypoint names for lookups (trim + lowercase).
fn normalize_name(name: &str) -> Result<String, String> {
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

    fn get(&self, name: &str) -> Option<NamedWaypoint> {
        normalize_name(name)
            .ok()
            .and_then(|key| self.waypoints.get(&key).cloned())
    }

    fn list(&self) -> Vec<NamedWaypoint> {
        self.waypoints.values().cloned().collect()
    }
}

fn store() -> &'static Mutex<WaypointStore> {
    static STORE: OnceLock<Mutex<WaypointStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(WaypointStore::load(default_store_path())))
}

/// Save a waypoint at the given position and zone.
pub fn save(name: &str, position: Waypoint, zone: String) -> Result<NamedWaypoint, String> {
    if zone.trim().is_empty() {
        return Err(String::from(
            "Zone name is unavailable — cannot save waypoint",
        ));
    }

    let waypoint = NamedWaypoint::new(name.trim(), position, zone.trim());
    store().lock().map_err(|e| e.to_string())?.upsert(waypoint)
}

/// Recall a saved waypoint by name.
#[must_use]
pub fn recall(name: &str) -> Option<NamedWaypoint> {
    store().lock().ok().and_then(|guard| guard.get(name))
}

/// Delete a saved waypoint by name. Returns true if it existed.
pub fn delete(name: &str) -> Result<bool, String> {
    store().lock().map_err(|e| e.to_string())?.delete(name)
}

/// List all saved waypoints.
#[must_use]
pub fn list() -> Vec<NamedWaypoint> {
    store()
        .lock()
        .ok()
        .map(|guard| guard.list())
        .unwrap_or_default()
}

/// Path to the persisted waypoint store (for diagnostics).
#[must_use]
pub fn store_path() -> PathBuf {
    store()
        .lock()
        .ok()
        .map(|guard| guard.path.clone())
        .unwrap_or_else(default_store_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("textquest-waypoints-test-{name}.json"));
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
        path
    }

    #[test]
    fn save_and_load_roundtrip() {
        let path = temp_path("roundtrip");
        let position = Waypoint::new(10.0, 20.0, 5.0);
        let waypoint = NamedWaypoint::new("camp1", position, "qeynos");

        {
            let mut store = WaypointStore {
                path: path.clone(),
                waypoints: BTreeMap::new(),
            };
            let saved = store.upsert(waypoint.clone()).expect("save waypoint");
            assert_eq!(saved, waypoint);
        }

        let reloaded = WaypointStore::load(path);
        let listed = reloaded.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "camp1");
        assert_eq!(listed[0].zone, "qeynos");
        assert_eq!(listed[0].position, position);
    }

    #[test]
    fn delete_removes_waypoint() {
        let path = temp_path("delete");
        let mut store = WaypointStore {
            path: path.clone(),
            waypoints: BTreeMap::new(),
        };
        store
            .upsert(NamedWaypoint::new(
                "pull",
                Waypoint::new(1.0, 2.0, 3.0),
                "guk",
            ))
            .expect("save waypoint");
        assert!(store.delete("pull").expect("delete should succeed"));
        assert!(store.list().is_empty());

        let reloaded = WaypointStore::load(path);
        assert!(reloaded.list().is_empty());
    }

    #[test]
    fn normalize_enforces_length_and_trim() {
        assert_eq!(normalize_name("  Camp1  ").unwrap(), String::from("camp1"));
        assert!(normalize_name("").is_err());
        assert!(normalize_name("   ").is_err());
        let long_name = "a".repeat(65);
        assert!(normalize_name(&long_name).is_err());
    }
}
