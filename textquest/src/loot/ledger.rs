//! Economy ledger — tracks loot drops, distributions, vendor sales, and plat
//! movements with per-day trend reporting.

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::items_after_statements,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::redundant_field_names
)]

use std::{collections::HashMap, sync::{Mutex, PoisonError}};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

/// Category of an economy ledger entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntrySource {
    /// Item obtained from a mob kill.
    Drop,
    /// Item purchased from an NPC vendor.
    Vendor,
    /// Item retrieved from bank storage.
    Bank,
    /// Item received from another character (trade/distribution).
    Distribution,
}

impl EntrySource {
    fn as_str(&self) -> &'static str {
        match self {
            EntrySource::Drop => "drop",
            EntrySource::Vendor => "vendor",
            EntrySource::Bank => "bank",
            EntrySource::Distribution => "distribution",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "vendor" => EntrySource::Vendor,
            "bank" => EntrySource::Bank,
            "distribution" => EntrySource::Distribution,
            _ => EntrySource::Drop,
        }
    }
}

/// A single economy ledger entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub id: i64,
    /// ISO-8601 timestamp (UTC).
    pub timestamp: String,
    /// Item identifier (0 if not applicable, e.g. plat-only transactions).
    pub item_id: i64,
    /// Human-readable item name.
    pub item_name: String,
    /// Number of items in this entry.
    pub quantity: i64,
    /// How the item was obtained or moved.
    pub source: EntrySource,
    /// Character that gained/lost this item or plat.
    pub character_id: String,
    /// Platinum delta (positive = gained, negative = spent). 0 for pure item
    /// entries.
    pub plat_delta: i64,
    /// Optional freeform note.
    pub note: Option<String>,
}

/// Per-day summary produced by [`EconomyLedger::trend_report`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaySummary {
    /// Date in `YYYY-MM-DD` format.
    pub date: String,
    /// Total items collected across all characters on this day.
    pub items_collected: i64,
    /// Net plat change (sum of all plat_delta values) on this day.
    pub plat_delta: i64,
}

/// Full trend report returned by [`EconomyLedger::trend_report`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendReport {
    /// One entry per calendar day that has activity, sorted ascending.
    pub days: Vec<DaySummary>,
}

impl TrendReport {
    /// Serialize the report to a JSON string.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).context("Failed to serialize TrendReport")
    }
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS economy_ledger (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp    TEXT NOT NULL DEFAULT (datetime('now')),
    item_id      INTEGER NOT NULL DEFAULT 0,
    item_name    TEXT NOT NULL DEFAULT '',
    quantity     INTEGER NOT NULL DEFAULT 1,
    source       TEXT NOT NULL,
    character_id TEXT NOT NULL,
    plat_delta   INTEGER NOT NULL DEFAULT 0,
    note         TEXT
);
CREATE INDEX IF NOT EXISTS idx_ledger_char   ON economy_ledger(character_id);
CREATE INDEX IF NOT EXISTS idx_ledger_ts     ON economy_ledger(timestamp);
CREATE INDEX IF NOT EXISTS idx_ledger_item   ON economy_ledger(item_id);
CREATE INDEX IF NOT EXISTS idx_ledger_source ON economy_ledger(source);
";

/// SQLite-backed (or in-memory) economy ledger.
pub struct EconomyLedger {
    conn: Mutex<Connection>,
}

impl EconomyLedger {
    /// Open (or create) the ledger database at the given path.
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open ledger DB: {path:?}"))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to set PRAGMA")?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize ledger schema")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory ledger (for testing and ephemeral sessions).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("Failed to open in-memory ledger DB")?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize in-memory ledger schema")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Record a loot drop received by a character.
    pub fn record_drop(
        &self,
        item_id: i64,
        item_name: &str,
        quantity: i64,
        character_id: &str,
    ) -> Result<i64> {
        self.record(
            item_id,
            item_name,
            quantity,
            EntrySource::Drop,
            character_id,
            0,
            None,
        )
    }

    /// Record a vendor sale (character sells an item — plat_delta is positive
    /// for selling, negative for buying).
    pub fn record_vendor_sale(
        &self,
        item_id: i64,
        item_name: &str,
        quantity: i64,
        character_id: &str,
        plat_delta: i64,
    ) -> Result<i64> {
        self.record(
            item_id,
            item_name,
            quantity,
            EntrySource::Vendor,
            character_id,
            plat_delta,
            None,
        )
    }

    /// Record a plat deposit (positive amount) or withdrawal (negative amount).
    pub fn record_plat_transaction(
        &self,
        character_id: &str,
        plat_delta: i64,
        note: Option<&str>,
    ) -> Result<i64> {
        self.record(0, "", 0, EntrySource::Bank, character_id, plat_delta, note)
    }

    /// Record a distribution (loot assigned from raid/group to a specific
    /// character).
    pub fn record_distribution(
        &self,
        item_id: i64,
        item_name: &str,
        quantity: i64,
        character_id: &str,
    ) -> Result<i64> {
        self.record(
            item_id,
            item_name,
            quantity,
            EntrySource::Distribution,
            character_id,
            0,
            None,
        )
    }

    /// Low-level insert — returns the new row id.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &self,
        item_id: i64,
        item_name: &str,
        quantity: i64,
        source: EntrySource,
        character_id: &str,
        plat_delta: i64,
        note: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap_or_else(PoisonError::into_inner);
        conn.execute(
            "INSERT INTO economy_ledger
                (item_id, item_name, quantity, source, character_id, plat_delta, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                item_id,
                item_name,
                quantity,
                source.as_str(),
                character_id,
                plat_delta,
                note
            ],
        )
        .context("Failed to insert ledger entry")?;
        Ok(conn.last_insert_rowid())
    }

    /// Retrieve all entries, optionally filtered to a single character.
    pub fn entries(&self, character_filter: Option<&str>) -> Result<Vec<LedgerEntry>> {
        let conn = self.conn.lock().unwrap_or_else(PoisonError::into_inner);
        let (sql, param) = if let Some(c) = character_filter {
            (
                "SELECT id, timestamp, item_id, item_name, quantity, source, character_id, \
                 plat_delta, note
                   FROM economy_ledger WHERE character_id = ?1 ORDER BY timestamp ASC",
                Some(c.to_string()),
            )
        } else {
            (
                "SELECT id, timestamp, item_id, item_name, quantity, source, character_id, \
                 plat_delta, note
                   FROM economy_ledger ORDER BY timestamp ASC",
                None,
            )
        };

        let mut stmt = conn
            .prepare(sql)
            .context("Failed to prepare entries query")?;
        let rows = if let Some(ref p) = param {
            stmt.query_map(params![p], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .context("Failed to fetch ledger entries")?
        } else {
            stmt.query_map([], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .context("Failed to fetch ledger entries")?
        };
        Ok(rows)
    }

    /// Generate a per-day trend report, optionally scoped to one character.
    pub fn trend_report(&self, character_filter: Option<&str>) -> Result<TrendReport> {
        let entries = self.entries(character_filter)?;

        // Accumulate per-day buckets.
        let mut day_items: HashMap<String, i64> = HashMap::new();
        let mut day_plat: HashMap<String, i64> = HashMap::new();

        for entry in &entries {
            // timestamp is "YYYY-MM-DD HH:MM:SS" — take the date portion.
            let date = entry
                .timestamp
                .get(..10)
                .unwrap_or(&entry.timestamp)
                .to_string();
            *day_items.entry(date.clone()).or_insert(0) += entry.quantity;
            *day_plat.entry(date).or_insert(0) += entry.plat_delta;
        }

        let mut days: Vec<DaySummary> = day_items
            .into_iter()
            .map(|(date, items_collected)| DaySummary {
                plat_delta: *day_plat.get(&date).unwrap_or(&0),
                date,
                items_collected,
            })
            .collect();
        days.sort_by(|a, b| a.date.cmp(&b.date));

        Ok(TrendReport { days })
    }

    /// Serialize all entries to a JSON string (in-memory export path).
    pub fn to_json(&self) -> Result<String> {
        let entries = self.entries(None)?;
        serde_json::to_string_pretty(&entries).context("Failed to serialize ledger entries")
    }
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LedgerEntry> {
    Ok(LedgerEntry {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        item_id: row.get(2)?,
        item_name: row.get(3)?,
        quantity: row.get(4)?,
        source: EntrySource::from_str(&row.get::<_, String>(5)?),
        character_id: row.get(6)?,
        plat_delta: row.get(7)?,
        note: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger() -> EconomyLedger {
        EconomyLedger::open_in_memory().expect("in-memory ledger")
    }

    #[test]
    fn test_record_drop_and_retrieve() {
        let db = ledger();
        let id = db.record_drop(42, "Lambent Armor", 1, "Warrior1").unwrap();
        assert!(id > 0);

        let entries = db.entries(None).unwrap();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.item_id, 42);
        assert_eq!(e.item_name, "Lambent Armor");
        assert_eq!(e.quantity, 1);
        assert_eq!(e.source, EntrySource::Drop);
        assert_eq!(e.character_id, "Warrior1");
        assert_eq!(e.plat_delta, 0);
    }

    #[test]
    fn test_record_vendor_sale() {
        let db = ledger();
        db.record_vendor_sale(10, "Fine Steel Sword", 2, "Warrior1", 50)
            .unwrap();

        let entries = db.entries(None).unwrap();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.source, EntrySource::Vendor);
        assert_eq!(e.plat_delta, 50);
        assert_eq!(e.quantity, 2);
    }

    #[test]
    fn test_record_plat_transaction() {
        let db = ledger();
        db.record_plat_transaction("Cleric1", 1000, Some("vendor sell"))
            .unwrap();
        db.record_plat_transaction("Cleric1", -200, Some("vendor buy"))
            .unwrap();

        let entries = db.entries(None).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].plat_delta, 1000);
        assert_eq!(entries[1].plat_delta, -200);
        assert!(entries[0].note.as_deref() == Some("vendor sell"));
    }

    #[test]
    fn test_record_distribution() {
        let db = ledger();
        db.record_distribution(99, "Manastone", 1, "Wizard1")
            .unwrap();

        let entries = db.entries(None).unwrap();
        assert_eq!(entries[0].source, EntrySource::Distribution);
        assert_eq!(entries[0].item_name, "Manastone");
    }

    #[test]
    fn test_filter_by_character() {
        let db = ledger();
        db.record_drop(1, "Item A", 1, "CharA").unwrap();
        db.record_drop(2, "Item B", 1, "CharB").unwrap();
        db.record_drop(3, "Item C", 1, "CharA").unwrap();

        let all = db.entries(None).unwrap();
        assert_eq!(all.len(), 3);

        let char_a = db.entries(Some("CharA")).unwrap();
        assert_eq!(char_a.len(), 2);
        assert!(char_a.iter().all(|e| e.character_id == "CharA"));

        let char_b = db.entries(Some("CharB")).unwrap();
        assert_eq!(char_b.len(), 1);
        assert_eq!(char_b[0].item_name, "Item B");
    }

    #[test]
    fn test_trend_report_basic() {
        let db = ledger();
        // Insert entries with explicit timestamps so we can group by day.
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO economy_ledger (timestamp, item_id, item_name, quantity, source, \
                 character_id, plat_delta)
                 VALUES ('2026-04-10 12:00:00', 1, 'Sword', 1, 'drop', 'Warrior1', 0)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO economy_ledger (timestamp, item_id, item_name, quantity, source, \
                 character_id, plat_delta)
                 VALUES ('2026-04-10 14:00:00', 2, 'Shield', 2, 'drop', 'Warrior1', 0)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO economy_ledger (timestamp, item_id, item_name, quantity, source, \
                 character_id, plat_delta)
                 VALUES ('2026-04-11 09:00:00', 0, '', 0, 'bank', 'Warrior1', 500)",
                [],
            )
            .unwrap();
        }

        let report = db.trend_report(None).unwrap();
        assert_eq!(report.days.len(), 2);

        let day1 = report.days.iter().find(|d| d.date == "2026-04-10").unwrap();
        assert_eq!(day1.items_collected, 3); // 1 + 2
        assert_eq!(day1.plat_delta, 0);

        let day2 = report.days.iter().find(|d| d.date == "2026-04-11").unwrap();
        assert_eq!(day2.items_collected, 0);
        assert_eq!(day2.plat_delta, 500);
    }

    #[test]
    fn test_trend_report_filtered_by_character() {
        let db = ledger();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO economy_ledger (timestamp, item_id, item_name, quantity, source, \
                 character_id, plat_delta)
                 VALUES ('2026-04-10 12:00:00', 1, 'Ring', 1, 'drop', 'Rogue1', 0)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO economy_ledger (timestamp, item_id, item_name, quantity, source, \
                 character_id, plat_delta)
                 VALUES ('2026-04-10 13:00:00', 2, 'Belt', 1, 'drop', 'Cleric1', 0)",
                [],
            )
            .unwrap();
        }

        let report = db.trend_report(Some("Rogue1")).unwrap();
        assert_eq!(report.days.len(), 1);
        assert_eq!(report.days[0].items_collected, 1);
    }

    #[test]
    fn test_trend_report_to_json() {
        let db = ledger();
        db.record_drop(5, "Flowing Black Silk Sash", 1, "Necro1")
            .unwrap();

        let report = db.trend_report(None).unwrap();
        let json = report.to_json().unwrap();
        assert!(json.contains("days"));
        assert!(json.contains("items_collected"));
        assert!(json.contains("plat_delta"));
    }

    #[test]
    fn test_to_json_export() {
        let db = ledger();
        db.record_drop(1, "Rusty Dagger", 1, "Rogue1").unwrap();
        db.record_plat_transaction("Rogue1", -10, None).unwrap();

        let json = db.to_json().unwrap();
        assert!(json.contains("Rusty Dagger"));
        assert!(json.contains("Rogue1"));
    }

    #[test]
    fn test_entry_source_roundtrip() {
        for src in [
            EntrySource::Drop,
            EntrySource::Vendor,
            EntrySource::Bank,
            EntrySource::Distribution,
        ] {
            assert_eq!(EntrySource::from_str(src.as_str()), src);
        }
    }
}
