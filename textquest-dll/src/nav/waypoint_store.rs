use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
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

fn verify_not_symlink_path(path: &Path) -> Result<(), String> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(String::from(
                "Refusing to read/write waypoint store through a symlink path",
            ));
        }
    }
    Ok(())
}

/// RAII guard that removes a temp file on drop unless `commit()` has been called.
struct TempFileGuard {
    path: PathBuf,
    committed: bool,
}

impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            committed: false,
        }
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
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
        verify_not_symlink_path(&self.path)?;

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

        validate_store_path(&self.path)?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        verify_not_symlink_path(&self.path)?;

        let parent = self
            .path
            .parent()
            .ok_or_else(|| String::from("Waypoint store path has no parent directory"))?;

        // Generate a cryptographically random suffix to prevent prediction/collision.
        let mut random_bytes = [0u8; 8];
        getrandom::getrandom(&mut random_bytes).map_err(|e| e.to_string())?;
        let mut unique = String::with_capacity(16);
        for b in random_bytes {
            use std::fmt::Write as FmtWrite;
            let _ = write!(unique, "{b:02x}");
        }
        let temp_path = parent.join(format!(".waypoints-{unique}.tmp"));

        // Write serialized data to the temp file; guard removes it on all error paths.
        let mut temp_guard = TempFileGuard::new(temp_path.clone());
        let mut temp_file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|e| e.to_string())?;
        temp_file
            .write_all(serialized.as_bytes())
            .map_err(|e| e.to_string())?;
        temp_file.flush().map_err(|e| e.to_string())?;
        drop(temp_file);

        // Backup-and-restore: rename existing store to a backup first so the old
        // file is preserved until the new one is successfully in place.
        // Use a fixed backup name — at most one persist runs at a time (Mutex) and
        // keeping it distinct from the random temp name avoids any coupling.
        let backup_path = parent.join(".waypoints.bak");
        let had_existing = if self.path.exists() {
            verify_not_symlink_path(&self.path)?;
            fs::rename(&self.path, &backup_path).map_err(|e| e.to_string())?;
            true
        } else {
            false
        };

        if let Err(error) = fs::rename(&temp_path, &self.path) {
            // Restore backup so no data is lost.
            if had_existing {
                let _ = fs::rename(&backup_path, &self.path);
            }
            return Err(error.to_string());
        }

        // Rename succeeded — temp was moved, so guard must not delete it.
        temp_guard.commit();
        if had_existing {
            let _ = fs::remove_file(&backup_path);
        }

        Ok(())
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

fn validate_store_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err(String::from("Waypoint store path cannot be empty"));
    }

    // Walk every existing ancestor in the user-supplied path and reject any
    // symlink encountered. This must inspect the original path to avoid
    // symlink-bypass tricks where canonicalization changes path depth.
    for ancestor in path.ancestors() {
        let metadata = match fs::symlink_metadata(ancestor) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        };
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Waypoint store path is unsafe: {} is a symlink",
                ancestor.display()
            ));
        }
    }

    Ok(())
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
        if fs::symlink_metadata(&path).is_ok() {
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

    #[cfg(unix)]
    #[test]
    fn persist_rejects_symlink_store_path() {
        use std::os::unix::fs::symlink;

        let path = temp_path("symlink");
        let target = temp_path("symlink-target");
        fs::write(&target, "{}").expect("create target");
        symlink(&target, &path).expect("create symlink");

        let mut store = WaypointStore {
            path: path.clone(),
            waypoints: BTreeMap::new(),
        };
        let result = store.upsert(NamedWaypoint::new(
            "pull",
            Waypoint::new(1.0, 2.0, 3.0),
            "guk",
        ));
        assert!(result.is_err());

        // load_from_disk should also reject a symlinked store path.
        let mut load_store = WaypointStore {
            path: path.clone(),
            waypoints: BTreeMap::new(),
        };
        let load_result = load_store.load_from_disk();
        assert!(
            load_result.is_err(),
            "loading from a symlinked store path should be rejected"
        );
    }

    #[cfg(unix)]
    #[test]
    fn validate_store_path_rejects_symlinked_ancestor() {
        use std::os::unix::fs::symlink;

        let base = std::env::temp_dir().join("textquest-waypoints-test-symlink-ancestor");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).expect("create base dir");

        let target = base.join("target/deep/path");
        fs::create_dir_all(&target).expect("create target dir");

        let link = base.join("link");
        symlink(&target, &link).expect("create symlinked ancestor");
        assert!(
            fs::symlink_metadata(&link)
                .expect("symlink metadata")
                .file_type()
                .is_symlink()
        );

        let store_path = link.join("waypoints.json");
        let result = validate_store_path(&store_path);
        assert!(
            result.is_err(),
            "symlinked ancestor should be rejected, got: {result:?}"
        );
    }
}
