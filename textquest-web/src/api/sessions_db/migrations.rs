//! Migration loader and schema versioning for sessions.db.
//!
//! Provides idempotent migration application, forward migration tracking,
//! and schema version verification on database initialization.

use anyhow::{Context, Result};
use rusqlite::Connection;

/// Migration metadata
#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// All migrations in order
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "init_schema",
        sql: include_str!("../../../migrations/sessions/001_init_schema.sql"),
    },
    Migration {
        version: 2,
        name: "retention_helpers",
        sql: include_str!("../../../migrations/sessions/002_retention_helpers.sql"),
    },
];

/// Migration runner: applies pending migrations idempotently
pub struct MigrationRunner {
    conn: *const Connection,
}

impl MigrationRunner {
    /// Create a new migration runner bound to a connection
    pub fn new(conn: &Connection) -> Result<Self> {
        Ok(MigrationRunner { conn })
    }

    /// Initialize schema versions table (must exist before any migration check)
    fn ensure_schema_versions_table(&self) -> Result<()> {
        let conn = unsafe { &*self.conn };
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                version INTEGER NOT NULL UNIQUE,
                name TEXT NOT NULL,
                applied_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
            );
            "#,
        )
        .context("Failed to create schema_versions table")?;
        Ok(())
    }

    /// Get the current schema version
    pub fn current_version(&self) -> Result<i32> {
        let conn = unsafe { &*self.conn };
        self.ensure_schema_versions_table()?;

        let mut stmt = conn
            .prepare("SELECT MAX(version) FROM schema_versions")
            .context("Failed to prepare version query")?;

        let version = stmt
            .query_row([], |row| row.get::<_, Option<i32>>(0))
            .context("Failed to fetch current version")?
            .unwrap_or(0);

        Ok(version)
    }

    /// Apply all pending migrations
    pub fn apply_pending(&self) -> Result<()> {
        let _conn = unsafe { &*self.conn };
        self.ensure_schema_versions_table()?;

        let current = self.current_version()?;

        for migration in MIGRATIONS {
            if migration.version > current {
                self.apply_migration(migration)
                    .with_context(|| format!("Failed to apply migration {}", migration.version))?;
            }
        }

        Ok(())
    }

    /// Apply a single migration
    fn apply_migration(&self, migration: &Migration) -> Result<()> {
        let conn = unsafe { &*self.conn };

        // Wrap migration in transaction for atomicity. `unchecked_transaction`
        // is required because rusqlite's safe `Connection::transaction()` takes
        // `&mut self`, but we hold `&Connection` here intentionally.
        let tx = conn.unchecked_transaction()?;

        // Execute migration SQL
        tx.execute_batch(migration.sql)
            .with_context(|| format!("Failed to execute migration SQL for v{}", migration.version))?;

        // Record migration
        tx.execute(
            "INSERT OR IGNORE INTO schema_versions (version, name) VALUES (?, ?)",
            [&migration.version.to_string(), migration.name],
        )
        .with_context(|| format!("Failed to record migration {} in schema_versions", migration.version))?;

        tx.commit()
            .with_context(|| format!("Failed to commit migration {}", migration.version))?;

        Ok(())
    }

    /// Verify all expected tables and indices exist
    pub fn verify_schema(&self) -> Result<()> {
        let conn = unsafe { &*self.conn };

        // Check tables
        let expected_tables = &[
            "sessions",
            "combat_events",
            "deaths",
            "stuck_events",
            "pulls",
            "loot",
            "route_costs",
            "rotation_ticks",
            "session_aggregates",
        ];

        for table_name in expected_tables {
            let exists = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [*table_name],
                    |row| row.get::<_, i32>(0),
                )
                .context("Failed to query sqlite_master")?
                > 0;

            if !exists {
                anyhow::bail!("Required table '{}' does not exist", table_name);
            }
        }

        // Check indices
        let expected_indices = &[
            "idx_sessions_character",
            "idx_combat_session_ts",
            "idx_deaths_session",
            "idx_pulls_session",
        ];

        for index_name in expected_indices {
            let exists = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    [*index_name],
                    |row| row.get::<_, i32>(0),
                )
                .context("Failed to query sqlite_master for index")?
                > 0;

            if !exists {
                anyhow::bail!("Required index '{}' does not exist", index_name);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::NamedTempFile;

    #[test]
    fn test_migration_apply_idempotent() -> Result<()> {
        let temp = NamedTempFile::new()?;
        let conn = Connection::open(temp.path())?;
        let runner = MigrationRunner::new(&conn)?;

        // First apply
        runner.apply_pending()?;
        let v1 = runner.current_version()?;

        // Second apply (should be idempotent)
        runner.apply_pending()?;
        let v2 = runner.current_version()?;

        assert_eq!(v1, v2);
        assert_eq!(v2, 2); // Should be at latest migration

        Ok(())
    }

    #[test]
    fn test_migration_forward_only() -> Result<()> {
        let temp = NamedTempFile::new()?;
        let conn = Connection::open(temp.path())?;
        let runner = MigrationRunner::new(&conn)?;

        runner.apply_pending()?;
        let current = runner.current_version()?;

        // Verify version can only go forward
        assert_eq!(current, MIGRATIONS.len() as i32);

        Ok(())
    }

    #[test]
    fn test_schema_verification() -> Result<()> {
        let temp = NamedTempFile::new()?;
        let conn = Connection::open(temp.path())?;
        let runner = MigrationRunner::new(&conn)?;

        runner.apply_pending()?;
        runner.verify_schema()?; // Should not error

        Ok(())
    }

    #[test]
    fn test_all_tables_created() -> Result<()> {
        let temp = NamedTempFile::new()?;
        let conn = Connection::open(temp.path())?;
        let runner = MigrationRunner::new(&conn)?;

        runner.apply_pending()?;

        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
        )?;
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        assert!(tables.contains(&"sessions".to_string()));
        assert!(tables.contains(&"combat_events".to_string()));
        assert!(tables.contains(&"deaths".to_string()));
        assert!(tables.contains(&"stuck_events".to_string()));
        assert!(tables.contains(&"pulls".to_string()));
        assert!(tables.contains(&"loot".to_string()));
        assert!(tables.contains(&"route_costs".to_string()));
        assert!(tables.contains(&"rotation_ticks".to_string()));
        assert!(tables.contains(&"session_aggregates".to_string()));
        assert!(tables.contains(&"schema_versions".to_string()));

        Ok(())
    }

    #[test]
    fn test_indices_created() -> Result<()> {
        let temp = NamedTempFile::new()?;
        let conn = Connection::open(temp.path())?;
        let runner = MigrationRunner::new(&conn)?;

        runner.apply_pending()?;

        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' ORDER BY name")?;
        let indices: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        assert!(indices.iter().any(|i| i.contains("idx_sessions_character")));
        assert!(indices.iter().any(|i| i.contains("idx_combat_session_ts")));
        assert!(indices.iter().any(|i| i.contains("idx_deaths_session")));
        assert!(indices.iter().any(|i| i.contains("idx_pulls_session")));

        Ok(())
    }
}
