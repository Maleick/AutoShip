//! SQLite-backed local cache for Ghidra binary analysis data.
//!
//! Stores functions, globals, strings, imports, opcodes, and call graphs
//! imported into TextQuest for runtime/debug exploration. Canonical immutable
//! evidence, manifests, and snapshot history live in the sibling
//! `Maleick/TextQuest-Ghidra` repository.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS functions (
    address     INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    size        INTEGER,
    category    TEXT,
    source      TEXT DEFAULT 'ghidra',
    description TEXT,
    usability   TEXT DEFAULT 'untested',
    notes       TEXT
);

CREATE TABLE IF NOT EXISTS function_calls (
    caller_addr INTEGER NOT NULL,
    callee_addr INTEGER NOT NULL,
    PRIMARY KEY (caller_addr, callee_addr),
    FOREIGN KEY (caller_addr) REFERENCES functions(address),
    FOREIGN KEY (callee_addr) REFERENCES functions(address)
);

CREATE TABLE IF NOT EXISTS globals (
    address     INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    size        INTEGER,
    data_type   TEXT,
    description TEXT
);

CREATE TABLE IF NOT EXISTS opcodes (
    code        INTEGER PRIMARY KEY,
    handler_addr INTEGER,
    direction   TEXT,
    description TEXT,
    FOREIGN KEY (handler_addr) REFERENCES functions(address)
);

CREATE TABLE IF NOT EXISTS strings (
    address     INTEGER PRIMARY KEY,
    value       TEXT NOT NULL,
    ref_function_addr INTEGER,
    FOREIGN KEY (ref_function_addr) REFERENCES functions(address)
);

CREATE TABLE IF NOT EXISTS imports (
    address     INTEGER PRIMARY KEY,
    dll_name    TEXT NOT NULL,
    func_name   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS metadata (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_functions_name ON functions(name);
CREATE INDEX IF NOT EXISTS idx_functions_category ON functions(category);
CREATE INDEX IF NOT EXISTS idx_globals_name ON globals(name);
CREATE INDEX IF NOT EXISTS idx_strings_value ON strings(value);
CREATE INDEX IF NOT EXISTS idx_imports_func ON imports(func_name);
";

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionEntry {
    pub address: u64,
    pub name: String,
    pub size: Option<u64>,
    pub category: Option<String>,
    pub source: Option<String>,
    pub description: Option<String>,
    /// Exploitability classification: 'client_authoritative', 'server_validated',
    /// 'hybrid', 'untested', or 'not_applicable'.
    pub usability: Option<String>,
    /// Free-form annotations added during testing.
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalEntry {
    pub address: u64,
    pub name: String,
    pub size: Option<u64>,
    pub data_type: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringEntry {
    pub address: u64,
    pub value: String,
    pub ref_function_addr: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportEntry {
    pub address: u64,
    pub dll_name: String,
    pub func_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpcodeEntry {
    pub code: u64,
    pub handler_addr: Option<u64>,
    pub direction: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbStats {
    pub functions: u64,
    pub function_calls: u64,
    pub globals: u64,
    pub opcodes: u64,
    pub strings: u64,
    pub imports: u64,
}

// ---------------------------------------------------------------------------
// Database
// ---------------------------------------------------------------------------

/// SQLite-backed local cache for imported Ghidra binary analysis data.
pub struct GhidraDatabase {
    conn: Connection,
}

impl GhidraDatabase {
    /// Open (or create) the Ghidra database at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open Ghidra DB at {}", path.display()))?;

        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
            .context("Failed to set PRAGMAs on Ghidra DB")?;

        conn.execute_batch(SCHEMA)
            .context("Failed to initialize Ghidra DB schema")?;

        Ok(Self { conn })
    }

    /// Bulk-insert function entries. Returns the number of rows inserted.
    pub fn import_functions(&self, data: &[FunctionEntry]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO functions (address, name, size, category, source, description, usability, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        let mut count = 0usize;
        for f in data {
            stmt.execute(params![
                f.address as i64,
                f.name,
                f.size.map(|v| v as i64),
                f.category,
                f.source,
                f.description,
                f.usability,
                f.notes,
            ])?;
            count += 1;
        }
        drop(stmt);
        tx.commit()?;
        Ok(count)
    }

    /// Bulk-insert global entries. Returns the number of rows inserted.
    pub fn import_globals(&self, data: &[GlobalEntry]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO globals (address, name, size, data_type, description)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut count = 0usize;
        for g in data {
            stmt.execute(params![
                g.address as i64,
                g.name,
                g.size.map(|v| v as i64),
                g.data_type,
                g.description,
            ])?;
            count += 1;
        }
        drop(stmt);
        tx.commit()?;
        Ok(count)
    }

    /// Bulk-insert string entries. Returns the number of rows inserted.
    pub fn import_strings(&self, data: &[StringEntry]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO strings (address, value, ref_function_addr)
             VALUES (?1, ?2, ?3)",
        )?;
        let mut count = 0usize;
        for s in data {
            stmt.execute(params![
                s.address as i64,
                s.value,
                s.ref_function_addr.map(|v| v as i64),
            ])?;
            count += 1;
        }
        drop(stmt);
        tx.commit()?;
        Ok(count)
    }

    /// Bulk-insert import entries. Returns the number of rows inserted.
    pub fn import_imports(&self, data: &[ImportEntry]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO imports (address, dll_name, func_name)
             VALUES (?1, ?2, ?3)",
        )?;
        let mut count = 0usize;
        for i in data {
            stmt.execute(params![i.address as i64, i.dll_name, i.func_name])?;
            count += 1;
        }
        drop(stmt);
        tx.commit()?;
        Ok(count)
    }

    /// Bulk-insert opcode entries. Returns the number of rows inserted.
    pub fn import_opcodes(&self, data: &[OpcodeEntry]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO opcodes (code, handler_addr, direction, description)
             VALUES (?1, ?2, ?3, ?4)",
        )?;
        let mut count = 0usize;
        for o in data {
            stmt.execute(params![
                o.code as i64,
                o.handler_addr.map(|v| v as i64),
                o.direction,
                o.description,
            ])?;
            count += 1;
        }
        drop(stmt);
        tx.commit()?;
        Ok(count)
    }

    /// Bulk-insert function call edges.
    pub fn import_function_calls(&self, calls: &[(u64, u64)]) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO function_calls (caller_addr, callee_addr)
             VALUES (?1, ?2)",
        )?;
        let mut count = 0usize;
        for &(caller, callee) in calls {
            stmt.execute(params![caller as i64, callee as i64])?;
            count += 1;
        }
        drop(stmt);
        tx.commit()?;
        Ok(count)
    }

    /// Search functions by name (LIKE query).
    pub fn search_functions(&self, query: &str) -> Result<Vec<FunctionEntry>> {
        let pattern = format!("%{query}%");
        let mut stmt = self.conn.prepare_cached(
            "SELECT address, name, size, category, source, description, usability, notes
             FROM functions WHERE name LIKE ?1",
        )?;
        let rows = stmt
            .query_map(params![pattern], |row| {
                Ok(FunctionEntry {
                    address: row.get::<_, i64>(0)? as u64,
                    name: row.get(1)?,
                    size: row.get::<_, Option<i64>>(2)?.map(|v| v as u64),
                    category: row.get(3)?,
                    source: row.get(4)?,
                    description: row.get(5)?,
                    usability: row.get(6)?,
                    notes: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Exact address lookup for a function.
    pub fn search_by_address(&self, addr: u64) -> Result<Option<FunctionEntry>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT address, name, size, category, source, description, usability, notes
             FROM functions WHERE address = ?1",
        )?;
        let mut rows = stmt.query_map(params![addr as i64], |row| {
            Ok(FunctionEntry {
                address: row.get::<_, i64>(0)? as u64,
                name: row.get(1)?,
                size: row.get::<_, Option<i64>>(2)?.map(|v| v as u64),
                category: row.get(3)?,
                source: row.get(4)?,
                description: row.get(5)?,
                usability: row.get(6)?,
                notes: row.get(7)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// Get all functions that call the function at `addr`.
    pub fn get_callers(&self, addr: u64) -> Result<Vec<FunctionEntry>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT f.address, f.name, f.size, f.category, f.source, f.description, f.usability, f.notes
             FROM functions f
             JOIN function_calls fc ON f.address = fc.caller_addr
             WHERE fc.callee_addr = ?1",
        )?;
        let rows = stmt
            .query_map(params![addr as i64], |row| {
                Ok(FunctionEntry {
                    address: row.get::<_, i64>(0)? as u64,
                    name: row.get(1)?,
                    size: row.get::<_, Option<i64>>(2)?.map(|v| v as u64),
                    category: row.get(3)?,
                    source: row.get(4)?,
                    description: row.get(5)?,
                    usability: row.get(6)?,
                    notes: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Get all functions called by the function at `addr`.
    pub fn get_callees(&self, addr: u64) -> Result<Vec<FunctionEntry>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT f.address, f.name, f.size, f.category, f.source, f.description, f.usability, f.notes
             FROM functions f
             JOIN function_calls fc ON f.address = fc.callee_addr
             WHERE fc.caller_addr = ?1",
        )?;
        let rows = stmt
            .query_map(params![addr as i64], |row| {
                Ok(FunctionEntry {
                    address: row.get::<_, i64>(0)? as u64,
                    name: row.get(1)?,
                    size: row.get::<_, Option<i64>>(2)?.map(|v| v as u64),
                    category: row.get(3)?,
                    source: row.get(4)?,
                    description: row.get(5)?,
                    usability: row.get(6)?,
                    notes: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Search strings by value (LIKE query).
    pub fn search_strings(&self, query: &str) -> Result<Vec<StringEntry>> {
        let pattern = format!("%{query}%");
        let mut stmt = self.conn.prepare_cached(
            "SELECT address, value, ref_function_addr
             FROM strings WHERE value LIKE ?1",
        )?;
        let rows = stmt
            .query_map(params![pattern], |row| {
                Ok(StringEntry {
                    address: row.get::<_, i64>(0)? as u64,
                    value: row.get(1)?,
                    ref_function_addr: row.get::<_, Option<i64>>(2)?.map(|v| v as u64),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Get a metadata value by key.
    pub fn get_metadata(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT value FROM metadata WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    /// Set a metadata key-value pair.
    pub fn set_metadata(&self, key: &str, value: &str) -> Result<()> {
        self.conn
            .prepare_cached("INSERT OR REPLACE INTO metadata (key, value) VALUES (?1, ?2)")?
            .execute(params![key, value])?;
        Ok(())
    }

    /// Get row counts for each table.
    pub fn stats(&self) -> Result<DbStats> {
        let count = |table: &str| -> Result<u64> {
            let sql = format!("SELECT COUNT(*) FROM {table}");
            Ok(self.conn.query_row(&sql, [], |row| row.get::<_, i64>(0))? as u64)
        };
        Ok(DbStats {
            functions: count("functions")?,
            function_calls: count("function_calls")?,
            globals: count("globals")?,
            opcodes: count("opcodes")?,
            strings: count("strings")?,
            imports: count("imports")?,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    fn temp_db() -> (GhidraDatabase, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_ghidra.db");
        let db = GhidraDatabase::open(&path).unwrap();
        (db, dir)
    }

    #[test]
    fn open_and_create() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ghidra.db");
        assert!(!path.exists());
        let _db = GhidraDatabase::open(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn import_and_search_functions() {
        let (db, _dir) = temp_db();
        let funcs = vec![
            FunctionEntry {
                address: 0x1400_1000,
                name: "ProcessGameEvents".into(),
                size: Some(256),
                category: Some("internal".into()),
                source: Some("ghidra".into()),
                description: Some("Main game loop".into()),
                usability: None,
                notes: None,
            },
            FunctionEntry {
                address: 0x1400_2000,
                name: "HandlePacket".into(),
                size: Some(128),
                category: Some("opcode_handler".into()),
                source: Some("ghidra".into()),
                description: None,
                usability: None,
                notes: None,
            },
        ];

        let count = db.import_functions(&funcs).unwrap();
        assert_eq!(count, 2);

        let results = db.search_functions("Process").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "ProcessGameEvents");

        let results = db.search_functions("Packet").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].address, 0x1400_2000);
    }

    #[test]
    fn search_by_address() {
        let (db, _dir) = temp_db();
        let funcs = vec![FunctionEntry {
            address: 0x1400_ABCD,
            name: "FUN_1400abcd".into(),
            size: None,
            category: None,
            source: None,
            description: None,
            usability: None,
            notes: None,
        }];
        db.import_functions(&funcs).unwrap();

        let found = db.search_by_address(0x1400_ABCD).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "FUN_1400abcd");

        let missing = db.search_by_address(0xDEAD).unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn callers_and_callees() {
        let (db, _dir) = temp_db();
        let funcs = vec![
            FunctionEntry {
                address: 0x1000,
                name: "main".into(),
                size: None,
                category: None,
                source: None,
                description: None,
                usability: None,
                notes: None,
            },
            FunctionEntry {
                address: 0x2000,
                name: "helper".into(),
                size: None,
                category: None,
                source: None,
                description: None,
                usability: None,
                notes: None,
            },
            FunctionEntry {
                address: 0x3000,
                name: "utility".into(),
                size: None,
                category: None,
                source: None,
                description: None,
                usability: None,
                notes: None,
            },
        ];
        db.import_functions(&funcs).unwrap();
        db.import_function_calls(&[(0x1000, 0x2000), (0x1000, 0x3000), (0x2000, 0x3000)])
            .unwrap();

        // main calls helper and utility
        let callees = db.get_callees(0x1000).unwrap();
        assert_eq!(callees.len(), 2);

        // utility is called by main and helper
        let callers = db.get_callers(0x3000).unwrap();
        assert_eq!(callers.len(), 2);

        // helper is called only by main
        let callers = db.get_callers(0x2000).unwrap();
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0].name, "main");
    }

    #[test]
    fn import_and_search_strings() {
        let (db, _dir) = temp_db();
        // Insert referenced function first (FK constraint)
        db.import_functions(&[FunctionEntry {
            address: 0x1000,
            name: "ref_func".into(),
            size: None,
            category: None,
            source: None,
            description: None,
            usability: None,
            notes: None,
        }])
        .unwrap();
        let strings = vec![
            StringEntry {
                address: 0x5000,
                value: "EverQuest".into(),
                ref_function_addr: Some(0x1000),
            },
            StringEntry {
                address: 0x5100,
                value: "LoginServerAddr".into(),
                ref_function_addr: None,
            },
        ];
        db.import_strings(&strings).unwrap();

        let results = db.search_strings("EverQuest").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].address, 0x5000);

        let results = db.search_strings("Login").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn stats_counts() {
        let (db, _dir) = temp_db();
        let stats = db.stats().unwrap();
        assert_eq!(stats.functions, 0);
        assert_eq!(stats.globals, 0);

        db.import_functions(&[FunctionEntry {
            address: 0x1000,
            name: "test".into(),
            size: None,
            category: None,
            source: None,
            description: None,
            usability: None,
            notes: None,
        }])
        .unwrap();

        db.import_globals(&[GlobalEntry {
            address: 0x9000,
            name: "g_player".into(),
            size: Some(8),
            data_type: Some("pointer".into()),
            description: None,
        }])
        .unwrap();

        let stats = db.stats().unwrap();
        assert_eq!(stats.functions, 1);
        assert_eq!(stats.globals, 1);
    }

    #[test]
    fn metadata_get_set() {
        let (db, _dir) = temp_db();

        assert!(db.get_metadata("version").unwrap().is_none());

        db.set_metadata("version", "1.0").unwrap();
        assert_eq!(db.get_metadata("version").unwrap().unwrap(), "1.0");

        db.set_metadata("version", "2.0").unwrap();
        assert_eq!(db.get_metadata("version").unwrap().unwrap(), "2.0");
    }

    #[test]
    fn import_opcodes_and_imports() {
        let (db, _dir) = temp_db();
        // Insert referenced function first (FK constraint)
        db.import_functions(&[FunctionEntry {
            address: 0x1000,
            name: "handler".into(),
            size: None,
            category: None,
            source: None,
            description: None,
            usability: None,
            notes: None,
        }])
        .unwrap();

        let opcodes = vec![OpcodeEntry {
            code: 0x42,
            handler_addr: Some(0x1000),
            direction: Some("inbound".into()),
            description: Some("OP_ZoneEntry".into()),
        }];
        assert_eq!(db.import_opcodes(&opcodes).unwrap(), 1);

        let imports = vec![ImportEntry {
            address: 0x7000,
            dll_name: "kernel32.dll".into(),
            func_name: "ReadProcessMemory".into(),
        }];
        assert_eq!(db.import_imports(&imports).unwrap(), 1);

        let stats = db.stats().unwrap();
        assert_eq!(stats.opcodes, 1);
        assert_eq!(stats.imports, 1);
    }
}
