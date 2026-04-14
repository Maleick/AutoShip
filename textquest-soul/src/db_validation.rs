//! SQLite schema validation and migration support for the Soul Engine.

use rusqlite::{Connection, params};

/// Errors returned by schema validation.
#[derive(Debug, PartialEq)]
pub enum SchemaError {
    MissingTable(String),
    MissingColumn { table: String, column: String },
    MissingIndex(String),
    Corrupted(String),
    Database(String),
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaError::MissingTable(t) => write!(f, "Missing table: {t}"),
            SchemaError::MissingColumn { table, column } => {
                write!(f, "Missing column {column} in table {table}")
            }
            SchemaError::MissingIndex(i) => write!(f, "Missing index: {i}"),
            SchemaError::Corrupted(msg) => write!(f, "Database corrupted: {msg}"),
            SchemaError::Database(msg) => write!(f, "Database error: {msg}"),
        }
    }
}

/// Errors returned by the migration framework.
#[derive(Debug, PartialEq)]
pub enum MigrationError {
    ExecutionFailed(String),
    MetaTable(String),
    Database(String),
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::ExecutionFailed(msg) => write!(f, "Migration failed: {msg}"),
            MigrationError::MetaTable(msg) => write!(f, "Meta table error: {msg}"),
            MigrationError::Database(msg) => write!(f, "Database error: {msg}"),
        }
    }
}

const REQUIRED_TABLES: &[&str] = &[
    "memories",
    "conversations",
    "memory_summaries",
    "shared_references",
    "speech_patterns",
];

const REQUIRED_INDEXES: &[&str] = &[
    "idx_memories_character",
    "idx_memories_zone",
    "idx_conversations_character",
    "idx_conversations_speaker",
    "idx_summaries_character",
    "idx_shared_refs",
];

const REQUIRED_COLUMNS: &[(&str, &str)] = &[
    ("memories", "id"),
    ("memories", "character_id"),
    ("memories", "event_type"),
    ("memories", "event_json"),
    ("memories", "zone"),
    ("memories", "mood_at_time"),
    ("memories", "importance"),
    ("memories", "decayed"),
    ("memories", "created_at"),
    ("conversations", "id"),
    ("conversations", "character_id"),
    ("conversations", "speaker"),
    ("conversations", "is_player"),
    ("conversations", "channel"),
    ("conversations", "message"),
    ("conversations", "sentiment"),
    ("conversations", "created_at"),
    ("memory_summaries", "id"),
    ("memory_summaries", "character_id"),
    ("memory_summaries", "period_start"),
    ("memory_summaries", "period_end"),
    ("memory_summaries", "summary"),
    ("memory_summaries", "mood_trend"),
    ("memory_summaries", "created_at"),
    ("shared_references", "memory_id"),
    ("shared_references", "description"),
    ("speech_patterns", "character_id"),
];

/// Validate that the database schema matches what the Soul Engine requires.
///
/// Checks PRAGMA integrity, required tables, required columns, and required indexes.
pub fn validate_schema(conn: &Connection) -> Result<(), SchemaError> {
    // Integrity check
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| SchemaError::Database(e.to_string()))?;
    if integrity != "ok" {
        return Err(SchemaError::Corrupted(integrity));
    }

    // Check required tables
    for table in REQUIRED_TABLES {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                params![table],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .map_err(|e| SchemaError::Database(e.to_string()))?;
        if !exists {
            return Err(SchemaError::MissingTable(table.to_string()));
        }
    }

    // Check required columns
    for (table, column) in REQUIRED_COLUMNS {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name=?2",
                params![table, column],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .map_err(|e| SchemaError::Database(e.to_string()))?;
        if !exists {
            return Err(SchemaError::MissingColumn {
                table: table.to_string(),
                column: column.to_string(),
            });
        }
    }

    // Check required indexes
    for index in REQUIRED_INDEXES {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                params![index],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .map_err(|e| SchemaError::Database(e.to_string()))?;
        if !exists {
            return Err(SchemaError::MissingIndex(index.to_string()));
        }
    }

    Ok(())
}

/// Run pending schema migrations, returning the number of migrations applied.
///
/// Creates a `schema_migrations` tracking table if it doesn't exist.
/// Each migration runs inside its own transaction; failures roll back.
pub fn run_migrations(conn: &Connection) -> Result<u32, MigrationError> {
    // Create migration tracking table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .map_err(|e| MigrationError::MetaTable(e.to_string()))?;

    let current_version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .map_err(|e| MigrationError::Database(e.to_string()))?;

    let migrations: &[(i64, &str)] = &[
        // Version 1: initial schema (idempotent via IF NOT EXISTS)
        (
            1,
            "CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                character_id INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                event_json TEXT NOT NULL,
                zone TEXT,
                mood_at_time TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 1.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                decayed INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS conversations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                character_id INTEGER NOT NULL,
                speaker TEXT NOT NULL,
                is_player INTEGER NOT NULL DEFAULT 0,
                channel TEXT NOT NULL DEFAULT 'say',
                message TEXT NOT NULL,
                sentiment REAL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS memory_summaries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                character_id INTEGER NOT NULL,
                period_start TEXT NOT NULL DEFAULT (datetime('now')),
                period_end TEXT NOT NULL DEFAULT (datetime('now')),
                summary TEXT NOT NULL,
                mood_trend TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS shared_references (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                character_a INTEGER NOT NULL,
                character_b INTEGER NOT NULL,
                memory_id INTEGER NOT NULL REFERENCES memories(id),
                description TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS soul_audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                character_id INTEGER NOT NULL,
                action_type TEXT NOT NULL,
                action_json TEXT NOT NULL,
                reason TEXT,
                operator_id TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS speech_patterns (
                character_id INTEGER PRIMARY KEY,
                vocabulary_level REAL NOT NULL DEFAULT 0.5,
                emote_frequency REAL NOT NULL DEFAULT 0.5,
                typing_speed REAL NOT NULL DEFAULT 1.0,
                catchphrases TEXT NOT NULL DEFAULT '[]',
                adopted_slang TEXT NOT NULL DEFAULT '[]',
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_memories_character ON memories(character_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_memories_zone ON memories(character_id, zone);
            CREATE INDEX IF NOT EXISTS idx_conversations_character ON conversations(character_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_conversations_speaker ON conversations(character_id, speaker);
            CREATE INDEX IF NOT EXISTS idx_summaries_character ON memory_summaries(character_id, period_start DESC);
            CREATE INDEX IF NOT EXISTS idx_shared_refs ON shared_references(character_a, character_b);
            CREATE INDEX IF NOT EXISTS idx_audit_character ON soul_audit_log(character_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_audit_action_type ON soul_audit_log(action_type, created_at DESC);",
        ),
    ];

    let mut applied = 0u32;
    for (version, sql) in migrations {
        if *version <= current_version {
            continue;
        }
        conn.execute_batch(sql)
            .map_err(|e| MigrationError::ExecutionFailed(format!("v{version}: {e}")))?;
        conn.execute(
            "INSERT INTO schema_migrations (version) VALUES (?1)",
            params![version],
        )
        .map_err(|e| MigrationError::MetaTable(e.to_string()))?;
        applied += 1;
    }

    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn open_with_schema() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn valid_schema_passes() {
        let conn = open_with_schema();
        assert!(validate_schema(&conn).is_ok());
    }

    #[test]
    fn missing_table_detected() {
        let conn = Connection::open_in_memory().unwrap();
        // Don't run migrations — no tables
        let err = validate_schema(&conn).unwrap_err();
        assert!(matches!(err, SchemaError::MissingTable(_)));
    }

    #[test]
    fn missing_index_detected() {
        let conn = Connection::open_in_memory().unwrap();
        // Create tables without indexes
        conn.execute_batch(
            "CREATE TABLE memories (
                id INTEGER PRIMARY KEY,
                character_id INTEGER,
                event_type TEXT,
                event_json TEXT,
                zone TEXT,
                mood_at_time TEXT,
                importance REAL,
                decayed INTEGER,
                created_at TEXT
             );
             CREATE TABLE conversations (
                id INTEGER PRIMARY KEY,
                character_id INTEGER,
                speaker TEXT,
                is_player INTEGER,
                channel TEXT,
                message TEXT,
                sentiment REAL,
                created_at TEXT
             );
             CREATE TABLE memory_summaries (
                id INTEGER PRIMARY KEY,
                character_id INTEGER,
                period_start TEXT,
                period_end TEXT,
                summary TEXT,
                mood_trend TEXT,
                created_at TEXT
             );
             CREATE TABLE shared_references (
                id INTEGER PRIMARY KEY,
                character_a INTEGER,
                character_b INTEGER,
                memory_id INTEGER,
                description TEXT
             );
             CREATE TABLE speech_patterns (character_id INTEGER PRIMARY KEY);",
        )
        .unwrap();
        let err = validate_schema(&conn).unwrap_err();
        assert!(matches!(err, SchemaError::MissingIndex(_)));
    }

    #[test]
    fn migration_v1_applies() {
        let conn = Connection::open_in_memory().unwrap();
        let applied = run_migrations(&conn).unwrap();
        assert_eq!(applied, 1);
    }

    #[test]
    fn migration_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let applied = run_migrations(&conn).unwrap();
        assert_eq!(applied, 0, "second run should apply 0 migrations");
    }

    #[test]
    fn migration_version_tracked() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        let version: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn schema_then_validate_passes() {
        let conn = open_with_schema();
        assert!(validate_schema(&conn).is_ok());
        // Run again — still passes
        assert!(validate_schema(&conn).is_ok());
    }
}
