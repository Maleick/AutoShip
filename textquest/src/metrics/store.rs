//! SQLite-backed fleet metrics store.

use std::{path::Path, sync::Mutex};

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

CREATE TABLE IF NOT EXISTS xp_sessions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    character       TEXT NOT NULL,
    session_start   TEXT NOT NULL DEFAULT (datetime('now')),
    session_end     TEXT,
    start_xp_pct    REAL NOT NULL DEFAULT 0.0,
    end_xp_pct      REAL,
    start_aa_pct    REAL NOT NULL DEFAULT 0.0,
    end_aa_pct      REAL,
    start_level     INTEGER NOT NULL DEFAULT 0,
    end_level       INTEGER,
    level_ups       INTEGER NOT NULL DEFAULT 0,
    duration_secs   INTEGER,
    xp_per_hour     REAL,
    aa_per_hour     REAL
);
CREATE INDEX IF NOT EXISTS idx_xp_char ON xp_sessions(character);
CREATE INDEX IF NOT EXISTS idx_xp_start ON xp_sessions(session_start);

CREATE TABLE IF NOT EXISTS kill_sessions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    character       TEXT NOT NULL,
    zone            TEXT,
    session_start   TEXT NOT NULL DEFAULT (datetime('now')),
    session_end     TEXT,
    total_kills     INTEGER NOT NULL DEFAULT 0,
    total_deaths    INTEGER NOT NULL DEFAULT 0,
    kills_per_hour  REAL,
    duration_secs   INTEGER,
    top_mob         TEXT
);
CREATE INDEX IF NOT EXISTS idx_kill_char ON kill_sessions(character);
CREATE INDEX IF NOT EXISTS idx_kill_start ON kill_sessions(session_start);
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
            "INSERT INTO events (event_type, character, zone, details, pid) VALUES (?1, ?2, ?3, \
             ?4, ?5)",
            params![event_type, character, zone, details, pid.map(|p| p as i64)],
        )
        .context("Failed to insert event")?;
        Ok(conn.last_insert_rowid())
    }

    /// Query recent events, newest first.
    pub fn recent_events(&self, limit: u32) -> Result<Vec<EventRow>> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT id, timestamp, event_type, character, zone, details, pid FROM events \
                 ORDER BY id DESC LIMIT ?1",
            )
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
            "INSERT INTO dps_snapshots (character, target, damage, spell_name, zone, encounter) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![character, target, damage, spell_name, zone, encounter],
        )
        .context("Failed to insert DPS snapshot")?;
        Ok(conn.last_insert_rowid())
    }

    /// Total damage per character for an encounter.
    pub fn encounter_dps_summary(&self, encounter: &str) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT character, SUM(damage) FROM dps_snapshots WHERE encounter = ?1 GROUP BY \
                 character ORDER BY SUM(damage) DESC",
            )
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
            "INSERT INTO loot_history (item_name, item_id, recipient, source, zone, value_plat) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![item_name, item_id, recipient, source, zone, value_plat],
        )
        .context("Failed to insert loot")?;
        Ok(conn.last_insert_rowid())
    }

    /// Recent loot drops, newest first.
    pub fn recent_loot(&self, limit: u32) -> Result<Vec<LootRow>> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT id, timestamp, item_name, item_id, recipient, source, zone, value_plat \
                 FROM loot_history ORDER BY id DESC LIMIT ?1",
            )
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
            "INSERT INTO lockouts (character, instance, expires_at) VALUES (?1, ?2, ?3) ON \
             CONFLICT(character, instance) DO UPDATE SET expires_at = excluded.expires_at",
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
                "SELECT id, character, instance, expires_at, created_at FROM lockouts WHERE \
                 character = ?1 AND expires_at > datetime('now') ORDER BY expires_at",
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
            "INSERT INTO plat_ledger (character, amount, balance, source, note) VALUES (?1, ?2, \
             ?3, ?4, ?5)",
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

    /// Total plat earned (sum of positive `amount` entries) by `character`
    /// since `since_timestamp` (ISO datetime string, e.g. "2026-04-12
    /// 00:00:00").
    pub fn plat_earned_since(&self, character: &str, since_timestamp: &str) -> Result<i64> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        let total: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(amount), 0) FROM plat_ledger WHERE character = ?1 AND amount \
                 > 0 AND timestamp >= ?2",
                params![character, since_timestamp],
                |row| row.get(0),
            )
            .context("Failed to query plat earned since")?;
        Ok(total)
    }

    /// Fleet-wide total plat earned (all characters, all positive entries)
    /// since `since_timestamp`.
    pub fn fleet_plat_earned_since(&self, since_timestamp: &str) -> Result<i64> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        let total: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(amount), 0) FROM plat_ledger WHERE amount > 0 AND timestamp \
                 >= ?1",
                params![since_timestamp],
                |row| row.get(0),
            )
            .context("Failed to query fleet plat earned since")?;
        Ok(total)
    }

    /// Recent platinum ledger entries for a character, newest first.
    pub fn recent_plat(&self, character: &str, limit: u32) -> Result<Vec<PlatRow>> {
        let conn = self.conn.lock().expect("metrics lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT id, timestamp, character, amount, balance, source, note FROM plat_ledger \
                 WHERE character = ?1 ORDER BY id DESC LIMIT ?2",
            )
            .context("Failed to prepare plat query")?;
        let rows = stmt
            .query_map(params![character, limit], |row| {
                Ok(PlatRow {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    character: row.get(2)?,
                    amount: row.get(3)?,
                    balance: row.get(4)?,
                    source: row.get(5)?,
                    note: row.get(6)?,
                })
            })
            .context("Failed to query plat ledger")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect plat rows")
    }

    /// Persist a completed XP session (MQ2XPTracker parity).
    ///
    /// Call on session end / shutdown so rates survive restarts.
    pub fn save_xp_session(
        &self,
        character: &str,
        start_xp: f32,
        end_xp: f32,
        start_aa: f32,
        end_aa: f32,
        start_level: u8,
        end_level: u8,
        level_ups: u32,
        duration_secs: u64,
        xp_per_hour: f32,
        aa_per_hour: f32,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO xp_sessions \
             (character, start_xp_pct, end_xp_pct, start_aa_pct, end_aa_pct, \
              start_level, end_level, level_ups, duration_secs, xp_per_hour, aa_per_hour, \
              session_end) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))",
            params![
                character,
                start_xp,
                end_xp,
                start_aa,
                end_aa,
                start_level as i64,
                end_level as i64,
                level_ups as i64,
                duration_secs as i64,
                xp_per_hour,
                aa_per_hour,
            ],
        )
        .context("Failed to save XP session")?;
        Ok(())
    }

    /// Load recent XP sessions for a character (newest first).
    pub fn recent_xp_sessions(
        &self,
        character: &str,
        limit: u32,
    ) -> Result<Vec<XpSessionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, character, session_start, session_end, \
                  start_xp_pct, end_xp_pct, start_aa_pct, end_aa_pct, \
                  start_level, end_level, level_ups, duration_secs, \
                  xp_per_hour, aa_per_hour \
                 FROM xp_sessions WHERE character = ?1 \
                 ORDER BY session_start DESC LIMIT ?2",
            )
            .context("Failed to prepare xp_sessions query")?;
        let rows = stmt
            .query_map(params![character, limit], |row| {
                Ok(XpSessionRow {
                    id: row.get(0)?,
                    character: row.get(1)?,
                    session_start: row.get(2)?,
                    session_end: row.get(3)?,
                    start_xp_pct: row.get(4)?,
                    end_xp_pct: row.get(5)?,
                    start_aa_pct: row.get(6)?,
                    end_aa_pct: row.get(7)?,
                    start_level: row.get(8)?,
                    end_level: row.get(9)?,
                    level_ups: row.get(10)?,
                    duration_secs: row.get(11)?,
                    xp_per_hour: row.get(12)?,
                    aa_per_hour: row.get(13)?,
                })
            })
            .context("Failed to query xp_sessions")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect xp session rows")
    }

    /// Persist a completed kill session (MQ2KillTracker parity).
    pub fn save_kill_session(
        &self,
        character: &str,
        zone: Option<&str>,
        total_kills: u32,
        total_deaths: u32,
        kills_per_hour: f64,
        duration_secs: u64,
        top_mob: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO kill_sessions \
             (character, zone, total_kills, total_deaths, kills_per_hour, \
              duration_secs, top_mob, session_end) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
            params![
                character,
                zone,
                total_kills as i64,
                total_deaths as i64,
                kills_per_hour,
                duration_secs as i64,
                top_mob,
            ],
        )
        .context("Failed to save kill session")?;
        Ok(())
    }

    /// Load recent kill sessions for a character (newest first).
    pub fn recent_kill_sessions(
        &self,
        character: &str,
        limit: u32,
    ) -> Result<Vec<KillSessionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, character, zone, session_start, session_end, \
                  total_kills, total_deaths, kills_per_hour, duration_secs, top_mob \
                 FROM kill_sessions WHERE character = ?1 \
                 ORDER BY session_start DESC LIMIT ?2",
            )
            .context("Failed to prepare kill_sessions query")?;
        let rows = stmt
            .query_map(params![character, limit], |row| {
                Ok(KillSessionRow {
                    id: row.get(0)?,
                    character: row.get(1)?,
                    zone: row.get(2)?,
                    session_start: row.get(3)?,
                    session_end: row.get(4)?,
                    total_kills: row.get(5)?,
                    total_deaths: row.get(6)?,
                    kills_per_hour: row.get(7)?,
                    duration_secs: row.get(8)?,
                    top_mob: row.get(9)?,
                })
            })
            .context("Failed to query kill_sessions")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect kill session rows")
    }

    /// Aggregate kills-per-hour and plat-per-hour over recent sessions for the self-improvement
    /// loop (feeds #2598 aggregate tables).
    ///
    /// Returns `(avg_kph, avg_pph)` across the last `window_sessions` sessions for `character`.
    pub fn session_aggregate_rates(
        &self,
        character: &str,
        window_sessions: u32,
    ) -> Result<SessionAggregateRates> {
        let conn = self.conn.lock().unwrap();

        let avg_kph: Option<f64> = conn
            .query_row(
                "SELECT AVG(kills_per_hour) FROM \
                 (SELECT kills_per_hour FROM kill_sessions WHERE character = ?1 \
                  AND kills_per_hour IS NOT NULL ORDER BY session_start DESC LIMIT ?2)",
                params![character, window_sessions],
                |row| row.get(0),
            )
            .optional()
            .context("Failed to query avg KPH")?
            .flatten();

        let avg_xph: Option<f64> = conn
            .query_row(
                "SELECT AVG(xp_per_hour) FROM \
                 (SELECT xp_per_hour FROM xp_sessions WHERE character = ?1 \
                  AND xp_per_hour IS NOT NULL ORDER BY session_start DESC LIMIT ?2)",
                params![character, window_sessions],
                |row| row.get(0),
            )
            .optional()
            .context("Failed to query avg XPH")?
            .flatten();

        let avg_pph: Option<f64> = conn
            .query_row(
                "SELECT AVG(CAST(amount AS REAL) / NULLIF(duration_secs, 0) * 3600.0) FROM \
                 (SELECT amount, \
                   CAST(strftime('%s', session_end) - strftime('%s', session_start) AS INTEGER) \
                   AS duration_secs \
                  FROM plat_ledger \
                  WHERE character = ?1 AND amount > 0 \
                  ORDER BY timestamp DESC LIMIT ?2)",
                params![character, window_sessions * 10],
                |row| row.get(0),
            )
            .optional()
            .context("Failed to query avg PPH")?
            .flatten();

        Ok(SessionAggregateRates {
            avg_kills_per_hour: avg_kph.unwrap_or(0.0),
            avg_xp_per_hour: avg_xph.unwrap_or(0.0),
            avg_plat_per_hour: avg_pph.unwrap_or(0.0),
        })
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

/// A row from the `plat_ledger` table.
#[derive(Debug)]
pub struct PlatRow {
    pub id: i64,
    pub timestamp: String,
    pub character: String,
    pub amount: i64,
    pub balance: i64,
    pub source: Option<String>,
    pub note: Option<String>,
}

/// A row from the `xp_sessions` table.
#[derive(Debug)]
pub struct XpSessionRow {
    pub id: i64,
    pub character: String,
    pub session_start: String,
    pub session_end: Option<String>,
    pub start_xp_pct: f64,
    pub end_xp_pct: Option<f64>,
    pub start_aa_pct: f64,
    pub end_aa_pct: Option<f64>,
    pub start_level: i64,
    pub end_level: Option<i64>,
    pub level_ups: i64,
    pub duration_secs: Option<i64>,
    pub xp_per_hour: Option<f64>,
    pub aa_per_hour: Option<f64>,
}

/// A row from the `kill_sessions` table.
#[derive(Debug)]
pub struct KillSessionRow {
    pub id: i64,
    pub character: String,
    pub zone: Option<String>,
    pub session_start: String,
    pub session_end: Option<String>,
    pub total_kills: i64,
    pub total_deaths: i64,
    pub kills_per_hour: Option<f64>,
    pub duration_secs: Option<i64>,
    pub top_mob: Option<String>,
}

/// Aggregate session rates for the self-improvement loop (#2598).
#[derive(Debug, Default, Clone)]
pub struct SessionAggregateRates {
    /// Average kills per hour over recent sessions.
    pub avg_kills_per_hour: f64,
    /// Average XP percent per hour over recent sessions.
    pub avg_xp_per_hour: f64,
    /// Average platinum per hour over recent plat ledger entries.
    pub avg_plat_per_hour: f64,
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

    // ── Plat economy query tests ─────────────────────────────────────────

    #[test]
    fn plat_earned_since_sums_positive_only() {
        let store = MetricsStore::open_memory().unwrap();
        // Insert some transactions — the timestamp column defaults to datetime('now')
        // which is within the "since" window we'll use.
        store
            .insert_plat("Trader01", 1000, 1000, Some("vendor_sale"), None)
            .unwrap();
        store
            .insert_plat("Trader01", 500, 1500, Some("vendor_sale"), None)
            .unwrap();
        store
            .insert_plat("Trader01", -200, 1300, Some("spell_purchase"), None)
            .unwrap();

        // Use a timestamp well in the past so all rows qualify.
        let earned = store
            .plat_earned_since("Trader01", "2000-01-01 00:00:00")
            .unwrap();
        // Only positive amounts: 1000 + 500 = 1500
        assert_eq!(earned, 1500);
    }

    #[test]
    fn plat_earned_since_unknown_character() {
        let store = MetricsStore::open_memory().unwrap();
        let earned = store
            .plat_earned_since("Nobody", "2000-01-01 00:00:00")
            .unwrap();
        assert_eq!(earned, 0);
    }

    #[test]
    fn plat_earned_since_future_timestamp_returns_zero() {
        let store = MetricsStore::open_memory().unwrap();
        store
            .insert_plat("Trader01", 1000, 1000, None, None)
            .unwrap();
        // A timestamp far in the future excludes all rows.
        let earned = store
            .plat_earned_since("Trader01", "9999-12-31 23:59:59")
            .unwrap();
        assert_eq!(earned, 0);
    }

    #[test]
    fn fleet_plat_earned_since_aggregates_all_characters() {
        let store = MetricsStore::open_memory().unwrap();
        store.insert_plat("Char_A", 400, 400, None, None).unwrap();
        store.insert_plat("Char_B", 600, 600, None, None).unwrap();
        store.insert_plat("Char_A", -100, 300, None, None).unwrap(); // negative — not counted

        let fleet = store
            .fleet_plat_earned_since("2000-01-01 00:00:00")
            .unwrap();
        assert_eq!(fleet, 1000); // 400 + 600 = 1000 positive
    }

    #[test]
    fn recent_plat_newest_first() {
        let store = MetricsStore::open_memory().unwrap();
        store
            .insert_plat("Char_A", 100, 100, Some("a"), None)
            .unwrap();
        store
            .insert_plat("Char_A", 200, 300, Some("b"), None)
            .unwrap();
        store
            .insert_plat("Char_A", -50, 250, Some("c"), None)
            .unwrap();

        let rows = store.recent_plat("Char_A", 10).unwrap();
        assert_eq!(rows.len(), 3);
        // Newest (id=3) first
        assert_eq!(rows[0].source.as_deref(), Some("c"));
        assert_eq!(rows[0].amount, -50);
        assert_eq!(rows[0].balance, 250);
        assert_eq!(rows[1].source.as_deref(), Some("b"));
        assert_eq!(rows[2].source.as_deref(), Some("a"));
    }

    #[test]
    fn recent_plat_limit_respected() {
        let store = MetricsStore::open_memory().unwrap();
        for i in 0..5 {
            store
                .insert_plat("Char_A", i * 10, i * 10, None, None)
                .unwrap();
        }
        let rows = store.recent_plat("Char_A", 3).unwrap();
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn recent_plat_empty_for_unknown_character() {
        let store = MetricsStore::open_memory().unwrap();
        let rows = store.recent_plat("Ghost", 10).unwrap();
        assert!(rows.is_empty());
    }
}
