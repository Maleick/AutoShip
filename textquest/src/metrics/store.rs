//! SQLite-backed fleet metrics store.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

/// Schema for the fleet metrics database.
///
/// Five tables covering the core fleet intelligence domains:
/// - `events` — combat, zone transitions, login/logout, deaths
/// - `dps_snapshots` — per-tick damage data for DPS tracking
/// - `loot_history` — item drops with timestamps and recipients
/// - `lockouts` — DZ instance timers per character
/// - `plat_ledger` — currency transactions and balances
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp   TEXT NOT NULL DEFAULT (datetime('now')),
    event_type  TEXT NOT NULL,
    character   TEXT NOT NULL,
    zone        TEXT,
    details     TEXT,
    pid         INTEGER
);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_char ON events(character);
CREATE INDEX IF NOT EXISTS idx_events_ts   ON events(timestamp);

CREATE TABLE IF NOT EXISTS dps_snapshots (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp   TEXT NOT NULL DEFAULT (datetime('now')),
    character   TEXT NOT NULL,
    target      TEXT,
    damage      INTEGER NOT NULL,
    spell_name  TEXT,
    zone        TEXT,
    encounter   TEXT
);
CREATE INDEX IF NOT EXISTS idx_dps_char ON dps_snapshots(character);
CREATE INDEX IF NOT EXISTS idx_dps_enc  ON dps_snapshots(encounter);

CREATE TABLE IF NOT EXISTS loot_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp   TEXT NOT NULL DEFAULT (datetime('now')),
    item_name   TEXT NOT NULL,
    item_id     INTEGER,
    recipient   TEXT NOT NULL,
    source      TEXT,
    zone        TEXT,
    value_plat  INTEGER DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_loot_item ON loot_history(item_name);
CREATE INDEX IF NOT EXISTS idx_loot_char ON loot_history(recipient);

CREATE TABLE IF NOT EXISTS lockouts (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    character   TEXT NOT NULL,
    instance    TEXT NOT NULL,
    expires_at  TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(character, instance)
);
CREATE INDEX IF NOT EXISTS idx_lock_char ON lockouts(character);
CREATE INDEX IF NOT EXISTS idx_lock_exp  ON lockouts(expires_at);

CREATE TABLE IF NOT EXISTS plat_ledger (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp   TEXT NOT NULL DEFAULT (datetime('now')),
    character   TEXT NOT NULL,
    amount      INTEGER NOT NULL,
    balance     INTEGER NOT NULL DEFAULT 0,
    source      TEXT,
    note        TEXT
);
CREATE INDEX IF NOT EXISTS idx_plat_char ON plat_ledger(character);
";

/// Fleet metrics store backed by SQLite.
pub struct MetricsStore {
    conn: Mutex<Connection>,
}

impl MetricsStore {
    /// Open (or create) the metrics database at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open metrics DB: {path:?}"))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to set PRAGMA")?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize metrics schema")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory metrics database (for testing).
    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("Failed to open in-memory DB")?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize metrics schema")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // ── Events ──────────────────────────────────────────────────────────

    /// Record a fleet event (combat, zone, login, death, etc.).
    pub fn insert_event(
        &self,
        event_type: &str,
        character: &str,
        zone: Option<&str>,
        details: Option<&str>,
        pid: Option<u32>,
    ) -> Result<i64> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        conn.execute(
            "INSERT INTO events (event_type, character, zone, details, pid) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![event_type, character, zone, details, pid.map(|p| p as i64)],
        )
        .context("Failed to insert event")?;
        Ok(conn.last_insert_rowid())
    }

    /// Query recent events, newest first.
    pub fn recent_events(&self, limit: u32) -> Result<Vec<EventRow>> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        let mut stmt = conn
            .prepare("SELECT id, timestamp, event_type, character, zone, details, pid FROM events ORDER BY id DESC LIMIT ?1")
            .context("Failed to prepare events query")?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(EventRow {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    event_type: row.get(2)?,
                    character: row.get(3)?,
                    zone: row.get(4)?,
                    details: row.get(5)?,
                    pid: row.get(6)?,
                })
            })
            .context("Failed to query events")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect event rows")
    }

    // ── DPS Snapshots ───────────────────────────────────────────────────

    /// Record a damage tick.
    pub fn insert_dps(
        &self,
        character: &str,
        target: Option<&str>,
        damage: i64,
        spell_name: Option<&str>,
        zone: Option<&str>,
        encounter: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        conn.execute(
            "INSERT INTO dps_snapshots (character, target, damage, spell_name, zone, encounter) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![character, target, damage, spell_name, zone, encounter],
        )
        .context("Failed to insert DPS snapshot")?;
        Ok(conn.last_insert_rowid())
    }

    /// Total damage per character for an encounter.
    pub fn encounter_dps_summary(&self, encounter: &str) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        let mut stmt = conn
            .prepare("SELECT character, SUM(damage) FROM dps_snapshots WHERE encounter = ?1 GROUP BY character ORDER BY SUM(damage) DESC")
            .context("Failed to prepare DPS summary")?;
        let rows = stmt
            .query_map(params![encounter], |row| Ok((row.get(0)?, row.get(1)?)))
            .context("Failed to query DPS summary")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect DPS rows")
    }

    // ── Loot History ────────────────────────────────────────────────────

    /// Record a loot drop.
    pub fn insert_loot(
        &self,
        item_name: &str,
        item_id: Option<i64>,
        recipient: &str,
        source: Option<&str>,
        zone: Option<&str>,
        value_plat: Option<i64>,
    ) -> Result<i64> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        conn.execute(
            "INSERT INTO loot_history (item_name, item_id, recipient, source, zone, value_plat) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![item_name, item_id, recipient, source, zone, value_plat],
        )
        .context("Failed to insert loot")?;
        Ok(conn.last_insert_rowid())
    }

    /// Recent loot drops, newest first.
    pub fn recent_loot(&self, limit: u32) -> Result<Vec<LootRow>> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        let mut stmt = conn
            .prepare("SELECT id, timestamp, item_name, item_id, recipient, source, zone, value_plat FROM loot_history ORDER BY id DESC LIMIT ?1")
            .context("Failed to prepare loot query")?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(LootRow {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    item_name: row.get(2)?,
                    item_id: row.get(3)?,
                    recipient: row.get(4)?,
                    source: row.get(5)?,
                    zone: row.get(6)?,
                    value_plat: row.get(7)?,
                })
            })
            .context("Failed to query loot")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect loot rows")
    }

    // ── Lockouts ────────────────────────────────────��───────────────────

    /// Set or update a DZ lockout for a character.
    pub fn upsert_lockout(&self, character: &str, instance: &str, expires_at: &str) -> Result<()> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        conn.execute(
            "INSERT INTO lockouts (character, instance, expires_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT(character, instance) DO UPDATE SET expires_at = excluded.expires_at",
            params![character, instance, expires_at],
        )
        .context("Failed to upsert lockout")?;
        Ok(())
    }

    /// Active lockouts for a character (not yet expired).
    pub fn active_lockouts(&self, character: &str) -> Result<Vec<LockoutRow>> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT id, character, instance, expires_at, created_at FROM lockouts \
                 WHERE character = ?1 AND expires_at > datetime('now') ORDER BY expires_at",
            )
            .context("Failed to prepare lockout query")?;
        let rows = stmt
            .query_map(params![character], |row| {
                Ok(LockoutRow {
                    id: row.get(0)?,
                    character: row.get(1)?,
                    instance: row.get(2)?,
                    expires_at: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .context("Failed to query lockouts")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect lockout rows")
    }

    // ── Plat Ledger ─────────────────────────────────────────────────────

    /// Record a platinum transaction.
    pub fn insert_plat(
        &self,
        character: &str,
        amount: i64,
        balance: i64,
        source: Option<&str>,
        note: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        conn.execute(
            "INSERT INTO plat_ledger (character, amount, balance, source, note) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![character, amount, balance, source, note],
        )
        .context("Failed to insert plat transaction")?;
        Ok(conn.last_insert_rowid())
    }

    /// Current balance for a character (latest ledger entry).
    pub fn current_balance(&self, character: &str) -> Result<Option<i64>> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        conn.query_row(
            "SELECT balance FROM plat_ledger WHERE character = ?1 ORDER BY id DESC LIMIT 1",
            params![character],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to query balance")
    }
}

// ── Row types ───────────────────────────────────────────────────────────────

/// A row from the `events` table.
#[derive(Debug)]
pub struct EventRow {
    pub id: i64,
    pub timestamp: String,
    pub event_type: String,
    pub character: String,
    pub zone: Option<String>,
    pub details: Option<String>,
    pub pid: Option<i64>,
}

/// A row from the `loot_history` table.
#[derive(Debug)]
pub struct LootRow {
    pub id: i64,
    pub timestamp: String,
    pub item_name: String,
    pub item_id: Option<i64>,
    pub recipient: String,
    pub source: Option<String>,
    pub zone: Option<String>,
    pub value_plat: Option<i64>,
}

/// A row from the `lockouts` table.
#[derive(Debug)]
pub struct LockoutRow {
    pub id: i64,
    pub character: String,
    pub instance: String,
    pub expires_at: String,
    pub created_at: String,
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_insert_event() {
        let store = MetricsStore::open_memory().unwrap();
        let id = store
            .insert_event(
                "combat_start",
                "Warrior01",
                Some("fearplane"),
                None,
                Some(1234),
            )
            .unwrap();
        assert!(id > 0);

        let events = store.recent_events(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "combat_start");
        assert_eq!(events[0].character, "Warrior01");
        assert_eq!(events[0].zone.as_deref(), Some("fearplane"));
    }

    #[test]
    fn dps_snapshots_and_summary() {
        let store = MetricsStore::open_memory().unwrap();
        store
            .insert_dps(
                "Wizard01",
                Some("Nagafen"),
                5000,
                Some("Ice Comet"),
                None,
                Some("enc-001"),
            )
            .unwrap();
        store
            .insert_dps(
                "Wizard01",
                Some("Nagafen"),
                3000,
                Some("Ice Comet"),
                None,
                Some("enc-001"),
            )
            .unwrap();
        store
            .insert_dps("Monk01", Some("Nagafen"), 2000, None, None, Some("enc-001"))
            .unwrap();

        let summary = store.encounter_dps_summary("enc-001").unwrap();
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].0, "Wizard01");
        assert_eq!(summary[0].1, 8000);
        assert_eq!(summary[1].0, "Monk01");
        assert_eq!(summary[1].1, 2000);
    }

    #[test]
    fn loot_insert_and_query() {
        let store = MetricsStore::open_memory().unwrap();
        store
            .insert_loot(
                "Cloak of Flames",
                Some(12345),
                "Warrior01",
                Some("Nagafen"),
                Some("soldungb"),
                Some(50000),
            )
            .unwrap();
        store
            .insert_loot(
                "Torn Cloth Sandal",
                None,
                "Monk01",
                Some("orc_pawn"),
                None,
                None,
            )
            .unwrap();

        let loot = store.recent_loot(10).unwrap();
        assert_eq!(loot.len(), 2);
        assert_eq!(loot[0].item_name, "Torn Cloth Sandal"); // newest first
        assert_eq!(loot[1].item_name, "Cloak of Flames");
        assert_eq!(loot[1].value_plat, Some(50000));
    }

    #[test]
    fn lockout_upsert_and_query() {
        let store = MetricsStore::open_memory().unwrap();
        // Insert a lockout far in the future
        store
            .upsert_lockout("Cleric01", "Plane of Fear", "2030-01-01 00:00:00")
            .unwrap();
        // Insert an expired lockout
        store
            .upsert_lockout("Cleric01", "Nagafen's Lair", "2020-01-01 00:00:00")
            .unwrap();

        let active = store.active_lockouts("Cleric01").unwrap();
        // Only the future lockout should appear
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].instance, "Plane of Fear");
    }

    #[test]
    fn lockout_upsert_updates_existing() {
        let store = MetricsStore::open_memory().unwrap();
        store
            .upsert_lockout("War01", "NToV", "2030-06-01 00:00:00")
            .unwrap();
        store
            .upsert_lockout("War01", "NToV", "2030-12-01 00:00:00")
            .unwrap();

        let active = store.active_lockouts("War01").unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].expires_at, "2030-12-01 00:00:00");
    }

    #[test]
    fn plat_ledger_insert_and_balance() {
        let store = MetricsStore::open_memory().unwrap();
        store
            .insert_plat("Trader01", 1000, 1000, Some("vendor_sale"), None)
            .unwrap();
        store
            .insert_plat(
                "Trader01",
                -200,
                800,
                Some("spell_purchase"),
                Some("Gate spell"),
            )
            .unwrap();

        let balance = store.current_balance("Trader01").unwrap();
        assert_eq!(balance, Some(800));
    }

    #[test]
    fn current_balance_unknown_character() {
        let store = MetricsStore::open_memory().unwrap();
        let balance = store.current_balance("Nobody").unwrap();
        assert_eq!(balance, None);
    }

    #[test]
    fn multiple_characters_isolated() {
        let store = MetricsStore::open_memory().unwrap();
        store
            .insert_event("login", "Char_A", Some("commons"), None, None)
            .unwrap();
        store
            .insert_event("login", "Char_B", Some("gfay"), None, None)
            .unwrap();
        store.insert_plat("Char_A", 500, 500, None, None).unwrap();
        store.insert_plat("Char_B", 100, 100, None, None).unwrap();

        assert_eq!(store.current_balance("Char_A").unwrap(), Some(500));
        assert_eq!(store.current_balance("Char_B").unwrap(), Some(100));
    }
}
