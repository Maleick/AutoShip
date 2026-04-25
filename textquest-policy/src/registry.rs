use std::path::Path;

use rusqlite::{Connection, params};

use crate::error::PolicyError;

/// Registry row returned by queries.
#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub scope: String,
    pub version: String,
    pub artifact_path: String,
    pub promoted_at: String,
    pub active: bool,
    pub source_bundle: String,
}

/// SQLite-backed registry of policy versions.
pub struct PolicyRegistry {
    conn: Connection,
}

impl PolicyRegistry {
    /// Open (or create) the registry at `db_path`.
    pub fn open(db_path: &Path) -> Result<Self, PolicyError> {
        let conn = Connection::open(db_path)?;
        let registry = Self { conn };
        registry.init_schema()?;
        Ok(registry)
    }

    fn init_schema(&self) -> Result<(), PolicyError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS policies (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                scope        TEXT    NOT NULL,
                version      TEXT    NOT NULL,
                artifact_path TEXT   NOT NULL,
                promoted_at  TEXT    NOT NULL,
                active       INTEGER NOT NULL DEFAULT 0,
                source_bundle TEXT   NOT NULL DEFAULT '',
                UNIQUE(scope, version)
            );
            CREATE INDEX IF NOT EXISTS idx_policies_scope ON policies(scope);
            ",
        )?;
        Ok(())
    }

    /// Insert or replace a policy entry (marks active=false initially).
    pub fn upsert(
        &self,
        scope: &str,
        version: &str,
        artifact_path: &str,
        source_bundle: &str,
        promoted_at: &str,
    ) -> Result<(), PolicyError> {
        self.conn.execute(
            "INSERT INTO policies (scope, version, artifact_path, promoted_at, active, source_bundle)
             VALUES (?1, ?2, ?3, ?4, 0, ?5)
             ON CONFLICT(scope, version) DO UPDATE SET
               artifact_path = excluded.artifact_path,
               promoted_at   = excluded.promoted_at,
               source_bundle = excluded.source_bundle",
            params![scope, version, artifact_path, promoted_at, source_bundle],
        )?;
        Ok(())
    }

    /// Mark `version` of `scope` as active; deactivate all others for that scope.
    pub fn set_active(&self, scope: &str, version: &str) -> Result<(), PolicyError> {
        self.conn.execute(
            "UPDATE policies SET active = (version = ?2) WHERE scope = ?1",
            params![scope, version],
        )?;
        Ok(())
    }

    /// Return the currently active entry for `scope`, if any.
    pub fn active_entry(&self, scope: &str) -> Result<Option<RegistryEntry>, PolicyError> {
        let mut stmt = self.conn.prepare(
            "SELECT scope, version, artifact_path, promoted_at, active, source_bundle
             FROM policies WHERE scope = ?1 AND active = 1 LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![scope], |row| {
            Ok(RegistryEntry {
                scope: row.get(0)?,
                version: row.get(1)?,
                artifact_path: row.get(2)?,
                promoted_at: row.get(3)?,
                active: row.get::<_, i32>(4)? != 0,
                source_bundle: row.get(5)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    /// Return all entries for a scope ordered by id (insertion order).
    pub fn entries_for_scope(&self, scope: &str) -> Result<Vec<RegistryEntry>, PolicyError> {
        let mut stmt = self.conn.prepare(
            "SELECT scope, version, artifact_path, promoted_at, active, source_bundle
             FROM policies WHERE scope = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![scope], |row| {
            Ok(RegistryEntry {
                scope: row.get(0)?,
                version: row.get(1)?,
                artifact_path: row.get(2)?,
                promoted_at: row.get(3)?,
                active: row.get::<_, i32>(4)? != 0,
                source_bundle: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Return all active entries across all scopes.
    pub fn all_active(&self) -> Result<Vec<RegistryEntry>, PolicyError> {
        let mut stmt = self.conn.prepare(
            "SELECT scope, version, artifact_path, promoted_at, active, source_bundle
             FROM policies WHERE active = 1 ORDER BY scope ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(RegistryEntry {
                scope: row.get(0)?,
                version: row.get(1)?,
                artifact_path: row.get(2)?,
                promoted_at: row.get(3)?,
                active: row.get::<_, i32>(4)? != 0,
                source_bundle: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}
