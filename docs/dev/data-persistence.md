# Data Persistence and Schema Migration Framework

This document describes the persistence framework used by TextQuest for storing and managing runtime data with schema versioning and automatic migration support.

## Overview

The data persistence framework provides:

- **Schema Versioning**: Semantic versioning (major.minor.patch) for data schemas
- **Automatic Migrations**: Define migration steps to transform data between schema versions
- **Pluggable Storage**: Abstract backend interface for different storage mechanisms
- **File-Based Backend**: Production JSON file storage implementation

## Schema Versioning Strategy

### Version Format

Schema versions follow semantic versioning: `MAJOR.MINOR.PATCH`

- **MAJOR**: Incremented for incompatible breaking changes (e.g., removing a required field)
- **MINOR**: Incremented for backward-compatible additions (e.g., adding an optional field)
- **PATCH**: Incremented for bug fixes that don't affect the data structure

### Version Metadata

The current schema version is stored in a special key `__schema_version__` in the persistence backend. This allows the system to determine which migrations need to be applied on startup.

Example metadata file:
```json
{
  "major": 1,
  "minor": 2,
  "patch": 0
}
```

## Writing Migrations

### Basic Migration Structure

Implement the `Migration` trait to define a schema transformation:

```rust
use textquest_common::persistence::{Migration, SchemaVersion};
use anyhow::Result;

struct AddPlayerNameFieldMigration;

impl Migration for AddPlayerNameFieldMigration {
    fn source_version(&self) -> SchemaVersion {
        SchemaVersion::new(1, 0, 0)
    }

    fn target_version(&self) -> SchemaVersion {
        SchemaVersion::new(1, 1, 0)
    }

    fn up(&self, data: &mut serde_json::Value) -> Result<()> {
        // Add the new 'player_name' field with a default value
        if let Some(obj) = data.as_object_mut() {
            if !obj.contains_key("player_name") {
                obj.insert("player_name".to_string(), serde_json::json!("Unknown"));
            }
        }
        Ok(())
    }

    fn down(&self, data: &mut serde_json::Value) -> Result<()> {
        // Remove the 'player_name' field for downgrade
        if let Some(obj) = data.as_object_mut() {
            obj.remove("player_name");
        }
        Ok(())
    }
}
```

### Migration Chain Example

For complex schema changes, create multiple migrations in sequence:

```rust
// Version 0.0.0 → 1.0.0: Add level field
// Version 1.0.0 → 1.1.0: Add player_name field
// Version 1.1.0 → 2.0.0: Restructure to nested format

let mut runner = MigrationRunner::new(backend);
runner.register(Box::new(AddLevelFieldMigration));
runner.register(Box::new(AddPlayerNameFieldMigration));
runner.register(Box::new(RestructureFormatMigration));

runner.run(None)?; // Apply all migrations to reach latest
```

## Using the Persistence Framework

### Basic Setup

```rust
use textquest_common::persistence::{JsonFileBackend, MigrationRunner};

// Create a file-based backend
let backend = Box::new(JsonFileBackend::new("./data")?);

// Create a migration runner
let mut runner = MigrationRunner::new(backend);

// Register migrations (typically done once at startup)
runner.register(Box::new(MyMigration));

// Apply pending migrations
runner.run(None)?;
```

### Specifying a Target Version

To apply migrations up to a specific version:

```rust
let target_version = SchemaVersion::new(1, 0, 0);
runner.run(Some(target_version))?;
```

### Checking Current Version

```rust
let current = runner.current_version()?;
println!("Current schema version: {}", current);
```

## Pluggable Backends

The `PersistenceBackend` trait allows implementing custom storage mechanisms:

```rust
pub trait PersistenceBackend: Send + Sync {
    fn load(&self, key: &str) -> Result<Option<serde_json::Value>>;
    fn save(&self, key: &str, data: &serde_json::Value) -> Result<()>;
    fn exists(&self, key: &str) -> Result<bool>;
}
```

### Implementing a Custom Backend

Example: SQLite-backed persistence:

```rust
use textquest_common::persistence::PersistenceBackend;
use anyhow::Result;

pub struct SqliteBackend {
    db: rusqlite::Connection,
}

impl PersistenceBackend for SqliteBackend {
    fn load(&self, key: &str) -> Result<Option<serde_json::Value>> {
        // Query database and deserialize JSON
        Ok(None)
    }

    fn save(&self, key: &str, data: &serde_json::Value) -> Result<()> {
        // Serialize and insert into database
        Ok(())
    }

    fn exists(&self, key: &str) -> Result<bool> {
        // Check if key exists in database
        Ok(false)
    }
}
```

## JsonFileBackend

The `JsonFileBackend` stores data as JSON files in a directory structure.

### Features

- **File per Key**: Each key is stored in a separate `.json` file
- **Pretty-Printed JSON**: Files are human-readable with indentation
- **Path Sanitization**: Prevents directory traversal attacks (no `..` or `/` in keys)
- **Automatic Directory Creation**: Creates the data directory if it doesn't exist

### Data Directory Layout

```
./data/
├── __schema_version__.json    # Version metadata
├── game_state.json            # Game state snapshot
├── config_cache.json          # Configuration cache
└── metrics.json               # Metrics data
```

### File Example

```json
{
  "major": 1,
  "minor": 0,
  "patch": 0
}
```

## Backup and Restore Procedures

### Backing Up Data

Since the `JsonFileBackend` uses regular files, backup is straightforward:

```bash
# Tar all data files
tar -czf data_backup_$(date +%Y%m%d_%H%M%S).tar.gz ./data/

# Copy to backup location
cp data_backup_*.tar.gz /mnt/backups/
```

### Restoring From Backup

```bash
# Stop the application
# Extract backup
tar -xzf data_backup_20260414_100000.tar.gz

# Restart the application
# Migrations will run automatically
```

### Version-Aware Restore

If restoring an older data version, migrations will automatically upgrade the schema:

```rust
// Restore old backup, then run
runner.run(None)?; // Automatically migrates to current version
```

## Best Practices

1. **Increment Version Numbers**: Never reuse a version number; always increment.

2. **Make Migrations Reversible**: Implement both `up()` and `down()` to allow rolling back.

3. **Test Migrations**: Write unit tests for migration transformations with edge cases.

4. **Document Migration Intent**: Add comments explaining why each migration was needed.

5. **Use Sensible Defaults**: When adding new fields, provide safe default values that don't break old code.

6. **Avoid Large Data Copies**: For large datasets, consider batch processing or streaming updates.

7. **Backup Before Migrations**: Run backups before applying migrations in production.

8. **Log Migration Activity**: Use structured logging to track migration execution.

## Future Enhancements

- **SQLite Backend**: Move to SQLite for better transaction support and querying
- **Compression**: Compress archived data files to save disk space
- **Replication**: Support cross-machine backup and replication
- **Incremental Backups**: Track changes and backup only modified files
- **Schema Introspection**: Query available migrations and their effects
- **Rollback Automation**: Automatic rollback on migration failure

## Testing

The persistence module includes comprehensive unit tests:

```bash
cargo test -p textquest-common persistence
```

Tests cover:

- Schema version parsing and formatting
- Migration runner chains
- JsonFileBackend file I/O
- Path sanitization and security
- Version metadata persistence
