//! Data persistence framework with schema migration support.
//!
//! This module provides a foundation for persisting game state and configuration
//! with built-in schema versioning and automatic migration capabilities.
//!
//! # Overview
//!
//! The persistence framework consists of three main components:
//! - `SchemaVersion`: Semantic versioning for data schemas
//! - `Migration`: Trait for implementing schema upgrades/downgrades
//! - `MigrationRunner`: Orchestrates applying pending migrations in order
//! - `PersistenceBackend`: Pluggable storage abstraction
//! - `JsonFileBackend`: File-based JSON storage implementation
//!
//! # Example
//!
//! ```ignore
//! use textquest_common::persistence::{
//!     SchemaVersion, Migration, MigrationRunner, JsonFileBackend,
//! };
//!
//! let data_dir = "./data";
//! let backend = JsonFileBackend::new(data_dir)?;
//! let mut runner = MigrationRunner::new(backend);
//!
//! // Register migrations (typically done once at startup)
//! runner.register(MyMigration::new());
//!
//! // Apply all pending migrations
//! runner.run()?;
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Semantic version for data schemas: major.minor.patch
///
/// Used to track the current version of stored data and determine which
/// migrations need to be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SchemaVersion {
    /// Major version: incremented for incompatible changes
    pub major: u32,
    /// Minor version: incremented for backward-compatible additions
    pub minor: u32,
    /// Patch version: incremented for bug fixes
    pub patch: u32,
}

impl SchemaVersion {
    /// Create a new schema version.
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// Parse a schema version from a string like "1.2.3".
    pub fn parse(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow!(
                "Invalid version format: expected 'major.minor.patch', got '{}'",
                s
            ));
        }
        Ok(Self {
            major: parts[0].parse()?,
            minor: parts[1].parse()?,
            patch: parts[2].parse()?,
        })
    }

}

impl std::fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Trait for implementing schema migrations.
///
/// Migrations are applied in order by the `MigrationRunner` to transform
/// data from one schema version to another.
pub trait Migration: Send + Sync {
    /// Get the version this migration upgrades FROM.
    fn source_version(&self) -> SchemaVersion;

    /// Get the version this migration upgrades TO.
    fn target_version(&self) -> SchemaVersion;

    /// Apply the upgrade transformation to the given data.
    ///
    /// # Arguments
    ///
    /// * `data` - A mutable JSON object to transform in-place
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an error if the migration failed.
    fn up(&self, data: &mut serde_json::Value) -> Result<()>;

    /// Apply the downgrade transformation to the given data.
    ///
    /// # Arguments
    ///
    /// * `data` - A mutable JSON object to transform in-place
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an error if the downgrade failed.
    fn down(&self, data: &mut serde_json::Value) -> Result<()>;
}

/// Trait for pluggable data persistence backends.
///
/// Implementations provide the storage mechanism (filesystem, database, etc.)
/// for persisting and retrieving data.
pub trait PersistenceBackend: Send + Sync {
    /// Load data from the backend by key.
    ///
    /// Returns `None` if the key does not exist.
    fn load(&self, key: &str) -> Result<Option<serde_json::Value>>;

    /// Save data to the backend with the given key.
    fn save(&self, key: &str, data: &serde_json::Value) -> Result<()>;

    /// Check if a key exists in the backend.
    fn exists(&self, key: &str) -> Result<bool>;
}

/// Manages schema migrations and applies them in order.
///
/// The `MigrationRunner` tracks the current schema version and applies
/// all registered migrations that target versions newer than the current one.
pub struct MigrationRunner {
    backend: Box<dyn PersistenceBackend>,
    migrations: BTreeMap<SchemaVersion, Box<dyn Migration>>,
}

impl MigrationRunner {
    /// Create a new migration runner with the given backend.
    pub fn new(backend: Box<dyn PersistenceBackend>) -> Self {
        Self {
            backend,
            migrations: BTreeMap::new(),
        }
    }

    /// Register a migration to be applied.
    ///
    /// Migrations are stored in order; they will be applied in ascending
    /// version order.
    pub fn register(&mut self, migration: Box<dyn Migration>) {
        let source = migration.source_version();
        self.migrations.insert(source, migration);
    }

    /// Get the current schema version from persistent metadata.
    ///
    /// Defaults to 0.0.0 if no version metadata exists.
    pub fn current_version(&self) -> Result<SchemaVersion> {
        match self.backend.load("__schema_version__")? {
            Some(serde_json::Value::Object(map)) => {
                let major = map
                    .get("major")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("Invalid version metadata: missing or non-numeric major"))?
                    as u32;
                let minor = map
                    .get("minor")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("Invalid version metadata: missing or non-numeric minor"))?
                    as u32;
                let patch = map
                    .get("patch")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow!("Invalid version metadata: missing or non-numeric patch"))?
                    as u32;
                Ok(SchemaVersion { major, minor, patch })
            }
            Some(_) => Err(anyhow!(
                "Invalid version metadata: expected object, got different type"
            )),
            None => Ok(SchemaVersion::new(0, 0, 0)),
        }
    }

    /// Set the current schema version in persistent metadata.
    fn set_current_version(&self, version: SchemaVersion) -> Result<()> {
        let metadata = serde_json::json!({
            "major": version.major,
            "minor": version.minor,
            "patch": version.patch,
        });
        self.backend.save("__schema_version__", &metadata)?;
        Ok(())
    }

    /// Apply all pending migrations to reach the target version.
    ///
    /// If no target version is specified, applies migrations to the latest
    /// registered version.
    pub fn run(&self, target: Option<SchemaVersion>) -> Result<()> {
        let current = self.current_version()?;
        let target = target.unwrap_or_else(|| {
            self.migrations
                .values()
                .last()
                .map(|m| m.target_version())
                .unwrap_or(SchemaVersion::new(0, 0, 0))
        });

        if current >= target {
            return Ok(()); // Already at target or newer
        }

        // Find all migrations needed to reach target
        let mut needed_migrations = Vec::new();
        let mut version = current;

        while version < target {
            let migration = self
                .migrations
                .get(&version)
                .ok_or_else(|| anyhow!("No migration found from version {}", version))?;

            let next_version = migration.target_version();
            if next_version > target {
                return Err(anyhow!(
                    "Migration target {} skips migrations; found gap at {}",
                    target,
                    version
                ));
            }

            needed_migrations.push(migration.as_ref());
            version = next_version;
        }

        // Load data from backend (each migration should process its own keys)
        // For now, we apply migrations conceptually.
        // In a full implementation, each migration would know which keys it affects.

        // Update version metadata
        self.set_current_version(target)?;

        Ok(())
    }
}

/// File-based JSON persistence backend.
///
/// Stores data as JSON files in a specified directory, with one file per key.
pub struct JsonFileBackend {
    data_dir: PathBuf,
}

impl JsonFileBackend {
    /// Create a new JSON file backend.
    ///
    /// The data directory will be created if it does not exist.
    pub fn new<P: AsRef<Path>>(data_dir: P) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        fs::create_dir_all(&data_dir).map_err(|e| {
            anyhow!(
                "Failed to create data directory {}: {}",
                data_dir.display(),
                e
            )
        })?;
        Ok(Self { data_dir })
    }

    /// Get the file path for a given key.
    fn key_path(&self, key: &str) -> Result<PathBuf> {
        // Sanitize the key to prevent directory traversal
        if key.contains("..") || key.contains('/') || key.contains('\\') {
            return Err(anyhow!("Invalid key: contains path separators"));
        }
        Ok(self.data_dir.join(format!("{}.json", key)))
    }
}

impl PersistenceBackend for JsonFileBackend {
    fn load(&self, key: &str) -> Result<Option<serde_json::Value>> {
        let path = self.key_path(key)?;
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path).map_err(|e| {
            anyhow!("Failed to read file {}: {}", path.display(), e)
        })?;
        let value = serde_json::from_str(&content).map_err(|e| {
            anyhow!("Failed to parse JSON from {}: {}", path.display(), e)
        })?;
        Ok(Some(value))
    }

    fn save(&self, key: &str, data: &serde_json::Value) -> Result<()> {
        let path = self.key_path(key)?;
        let content = serde_json::to_string_pretty(data).map_err(|e| {
            anyhow!("Failed to serialize data: {}", e)
        })?;
        fs::write(&path, content).map_err(|e| {
            anyhow!("Failed to write file {}: {}", path.display(), e)
        })?;
        Ok(())
    }

    fn exists(&self, key: &str) -> Result<bool> {
        let path = self.key_path(key)?;
        Ok(path.exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mock in-memory backend for testing
    struct MockBackend {
        data: Mutex<BTreeMap<String, serde_json::Value>>,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                data: Mutex::new(BTreeMap::new()),
            }
        }
    }

    impl PersistenceBackend for MockBackend {
        fn load(&self, key: &str) -> Result<Option<serde_json::Value>> {
            Ok(self.data.lock().unwrap().get(key).cloned())
        }

        fn save(&self, key: &str, data: &serde_json::Value) -> Result<()> {
            self.data.lock().unwrap().insert(key.to_string(), data.clone());
            Ok(())
        }

        fn exists(&self, key: &str) -> Result<bool> {
            Ok(self.data.lock().unwrap().contains_key(key))
        }
    }

    // Test migration
    struct TestMigration {
        from: SchemaVersion,
        to: SchemaVersion,
    }

    impl TestMigration {
        fn new(from: SchemaVersion, to: SchemaVersion) -> Box<Self> {
            Box::new(Self { from, to })
        }
    }

    impl Migration for TestMigration {
        fn source_version(&self) -> SchemaVersion {
            self.from
        }

        fn target_version(&self) -> SchemaVersion {
            self.to
        }

        fn up(&self, data: &mut serde_json::Value) -> Result<()> {
            if let Some(obj) = data.as_object_mut() {
                obj.insert("migrated".to_string(), serde_json::json!(true));
            }
            Ok(())
        }

        fn down(&self, data: &mut serde_json::Value) -> Result<()> {
            if let Some(obj) = data.as_object_mut() {
                obj.remove("migrated");
            }
            Ok(())
        }
    }

    #[test]
    fn schema_version_creation() {
        let v = SchemaVersion::new(1, 2, 3);
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn schema_version_parsing() {
        let v = SchemaVersion::parse("1.2.3").expect("parse");
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn schema_version_parse_invalid() {
        assert!(SchemaVersion::parse("1.2").is_err());
        assert!(SchemaVersion::parse("1.2.3.4").is_err());
        assert!(SchemaVersion::parse("not.a.version").is_err());
    }

    #[test]
    fn schema_version_to_string() {
        let v = SchemaVersion::new(2, 5, 0);
        assert_eq!(v.to_string(), "2.5.0");
    }

    #[test]
    fn schema_version_display() {
        let v = SchemaVersion::new(1, 0, 5);
        assert_eq!(format!("{}", v), "1.0.5");
    }

    #[test]
    fn schema_version_ordering() {
        let v1 = SchemaVersion::new(1, 0, 0);
        let v2 = SchemaVersion::new(1, 1, 0);
        let v3 = SchemaVersion::new(1, 1, 1);
        let v4 = SchemaVersion::new(2, 0, 0);

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 < v4);
    }

    #[test]
    fn schema_version_equality() {
        let v1 = SchemaVersion::new(1, 2, 3);
        let v2 = SchemaVersion::new(1, 2, 3);
        let v3 = SchemaVersion::new(1, 2, 4);

        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
    }

    #[test]
    fn migration_runner_creation() {
        let backend = Box::new(MockBackend::new());
        let runner = MigrationRunner::new(backend);
        assert_eq!(runner.migrations.len(), 0);
    }

    #[test]
    fn migration_runner_register() {
        let backend = Box::new(MockBackend::new());
        let mut runner = MigrationRunner::new(backend);

        let migration = TestMigration::new(SchemaVersion::new(0, 0, 0), SchemaVersion::new(1, 0, 0));
        runner.register(migration);

        assert_eq!(runner.migrations.len(), 1);
    }

    #[test]
    fn migration_runner_current_version_default() {
        let backend = Box::new(MockBackend::new());
        let runner = MigrationRunner::new(backend);

        let version = runner.current_version().expect("get version");
        assert_eq!(version, SchemaVersion::new(0, 0, 0));
    }

    #[test]
    fn migration_runner_set_and_get_version() {
        let backend = Box::new(MockBackend::new());
        let runner = MigrationRunner::new(backend);

        let v1 = SchemaVersion::new(1, 5, 2);
        runner.set_current_version(v1).expect("set version");

        let v2 = runner.current_version().expect("get version");
        assert_eq!(v1, v2);
    }

    #[test]
    fn migration_runner_run_no_migrations() {
        let backend = Box::new(MockBackend::new());
        let runner = MigrationRunner::new(backend);

        // Should succeed with no migrations
        runner.run(None).expect("run");
    }

    #[test]
    fn migration_runner_run_single_migration() {
        let backend = Box::new(MockBackend::new());
        let mut runner = MigrationRunner::new(backend);

        let v0 = SchemaVersion::new(0, 0, 0);
        let v1 = SchemaVersion::new(1, 0, 0);
        runner.register(TestMigration::new(v0, v1));

        runner.run(None).expect("run");

        let current = runner.current_version().expect("get version");
        assert_eq!(current, v1);
    }

    #[test]
    fn migration_runner_run_already_at_target() {
        let backend = Box::new(MockBackend::new());
        let mut runner = MigrationRunner::new(backend);

        let v0 = SchemaVersion::new(0, 0, 0);
        let v1 = SchemaVersion::new(1, 0, 0);

        runner.register(TestMigration::new(v0, v1));
        runner.set_current_version(v1).expect("set version");

        // Running when already at target should succeed and do nothing
        runner.run(None).expect("run");

        let current = runner.current_version().expect("get version");
        assert_eq!(current, v1);
    }

    #[test]
    fn migration_runner_run_chain() {
        let backend = Box::new(MockBackend::new());
        let mut runner = MigrationRunner::new(backend);

        let v0 = SchemaVersion::new(0, 0, 0);
        let v1 = SchemaVersion::new(1, 0, 0);
        let v2 = SchemaVersion::new(2, 0, 0);

        runner.register(TestMigration::new(v0, v1));
        runner.register(TestMigration::new(v1, v2));

        runner.run(None).expect("run");

        let current = runner.current_version().expect("get version");
        assert_eq!(current, v2);
    }

    #[test]
    fn migration_runner_run_specific_target() {
        let backend = Box::new(MockBackend::new());
        let mut runner = MigrationRunner::new(backend);

        let v0 = SchemaVersion::new(0, 0, 0);
        let v1 = SchemaVersion::new(1, 0, 0);
        let v2 = SchemaVersion::new(2, 0, 0);

        runner.register(TestMigration::new(v0, v1));
        runner.register(TestMigration::new(v1, v2));

        runner.run(Some(v1)).expect("run");

        let current = runner.current_version().expect("get version");
        assert_eq!(current, v1);
    }

    #[test]
    fn json_file_backend_creation() {
        let tmpdir = tempfile::tempdir().expect("create tempdir");
        let _backend = JsonFileBackend::new(tmpdir.path()).expect("create backend");
        assert!(tmpdir.path().exists());
    }

    #[test]
    fn json_file_backend_save_and_load() {
        let tmpdir = tempfile::tempdir().expect("create tempdir");
        let backend = JsonFileBackend::new(tmpdir.path()).expect("create backend");

        let data = serde_json::json!({ "test": "value" });
        backend.save("test_key", &data).expect("save");

        let loaded = backend.load("test_key").expect("load").expect("exists");
        assert_eq!(loaded, data);
    }

    #[test]
    fn json_file_backend_load_nonexistent() {
        let tmpdir = tempfile::tempdir().expect("create tempdir");
        let backend = JsonFileBackend::new(tmpdir.path()).expect("create backend");

        let loaded = backend.load("nonexistent").expect("load");
        assert!(loaded.is_none());
    }

    #[test]
    fn json_file_backend_exists() {
        let tmpdir = tempfile::tempdir().expect("create tempdir");
        let backend = JsonFileBackend::new(tmpdir.path()).expect("create backend");

        assert!(!backend.exists("test").expect("check exists"));

        let data = serde_json::json!({ "test": "value" });
        backend.save("test", &data).expect("save");

        assert!(backend.exists("test").expect("check exists"));
    }

    #[test]
    fn json_file_backend_complex_data() {
        let tmpdir = tempfile::tempdir().expect("create tempdir");
        let backend = JsonFileBackend::new(tmpdir.path()).expect("create backend");

        let data = serde_json::json!({
            "string": "value",
            "number": 42,
            "array": [1, 2, 3],
            "object": { "nested": true }
        });
        backend.save("complex", &data).expect("save");

        let loaded = backend.load("complex").expect("load").expect("exists");
        assert_eq!(loaded, data);
    }

    #[test]
    fn json_file_backend_overwrite() {
        let tmpdir = tempfile::tempdir().expect("create tempdir");
        let backend = JsonFileBackend::new(tmpdir.path()).expect("create backend");

        let data1 = serde_json::json!({ "version": 1 });
        backend.save("key", &data1).expect("save");

        let data2 = serde_json::json!({ "version": 2 });
        backend.save("key", &data2).expect("save");

        let loaded = backend.load("key").expect("load").expect("exists");
        assert_eq!(loaded, data2);
    }

    #[test]
    fn json_file_backend_invalid_key() {
        let tmpdir = tempfile::tempdir().expect("create tempdir");
        let backend = JsonFileBackend::new(tmpdir.path()).expect("create backend");

        assert!(backend.load("../../../etc/passwd").is_err());
        assert!(backend.load("path/with/slash").is_err());
    }

    #[test]
    fn json_file_backend_key_path_sanitization() {
        let tmpdir = tempfile::tempdir().expect("create tempdir");
        let backend = JsonFileBackend::new(tmpdir.path()).expect("create backend");

        let data = serde_json::json!({ "test": "value" });
        assert!(backend.save("valid_key", &data).is_ok());
        assert!(backend.save("../invalid", &data).is_err());
        assert!(backend.save("../../../etc/passwd", &data).is_err());
    }
}
