//! SQLite-backed item database, loot tables, wishlists, and loot history.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

/// Schema for the loot database.
///
/// Six tables:
/// - `items` — master item catalog (Lucy/Allakhazam data)
/// - `loot_tables` — TLP random loot table definitions (mob pool → table name)
/// - `drop_rates` — item drop rates within a loot table
/// - `wishlists` — per-character gear priority lists
/// - `loot_log` — historical record of drops and assignments
/// - `item_classes` — junction table for item class usability
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    lucy_id     INTEGER,
    slot        TEXT,
    item_type   TEXT,
    ac          INTEGER DEFAULT 0,
    hp          INTEGER DEFAULT 0,
    mana        INTEGER DEFAULT 0,
    damage      INTEGER DEFAULT 0,
    delay       INTEGER DEFAULT 0,
    level_req   INTEGER DEFAULT 0,
    weight      INTEGER DEFAULT 0,
    magic       INTEGER NOT NULL DEFAULT 0,
    lore        INTEGER NOT NULL DEFAULT 0,
    nodrop      INTEGER NOT NULL DEFAULT 0,
    expansion   TEXT,
    effect      TEXT,
    stats_json  TEXT,
    source_url  TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_items_name ON items(name);
CREATE INDEX IF NOT EXISTS idx_items_slot ON items(slot);
CREATE INDEX IF NOT EXISTS idx_items_type ON items(item_type);
CREATE INDEX IF NOT EXISTS idx_items_expansion ON items(expansion);
CREATE INDEX IF NOT EXISTS idx_items_level ON items(level_req);

CREATE TABLE IF NOT EXISTS item_classes (
    item_id     INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    class       TEXT NOT NULL,
    PRIMARY KEY (item_id, class)
);
CREATE INDEX IF NOT EXISTS idx_ic_class ON item_classes(class);

CREATE TABLE IF NOT EXISTS loot_tables (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    table_name  TEXT NOT NULL,
    mob_name    TEXT NOT NULL,
    zone        TEXT,
    min_level   INTEGER,
    max_level   INTEGER,
    expansion   TEXT,
    notes       TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(table_name, mob_name)
);
CREATE INDEX IF NOT EXISTS idx_lt_table ON loot_tables(table_name);
CREATE INDEX IF NOT EXISTS idx_lt_mob ON loot_tables(mob_name);
CREATE INDEX IF NOT EXISTS idx_lt_zone ON loot_tables(zone);

CREATE TABLE IF NOT EXISTS drop_rates (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    table_name  TEXT NOT NULL,
    item_id     INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    drop_chance REAL NOT NULL DEFAULT 0.0,
    min_qty     INTEGER NOT NULL DEFAULT 1,
    max_qty     INTEGER NOT NULL DEFAULT 1,
    notes       TEXT,
    UNIQUE(table_name, item_id)
);
CREATE INDEX IF NOT EXISTS idx_dr_table ON drop_rates(table_name);
CREATE INDEX IF NOT EXISTS idx_dr_item ON drop_rates(item_id);

CREATE TABLE IF NOT EXISTS wishlists (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    character   TEXT NOT NULL,
    item_id     INTEGER NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    priority    INTEGER NOT NULL DEFAULT 5,
    slot        TEXT,
    notes       TEXT,
    obtained    INTEGER NOT NULL DEFAULT 0,
    obtained_at TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(character, item_id)
);
CREATE INDEX IF NOT EXISTS idx_wl_char ON wishlists(character);
CREATE INDEX IF NOT EXISTS idx_wl_item ON wishlists(item_id);
CREATE INDEX IF NOT EXISTS idx_wl_prio ON wishlists(priority);

CREATE TABLE IF NOT EXISTS loot_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp   TEXT NOT NULL DEFAULT (datetime('now')),
    item_id     INTEGER REFERENCES items(id) ON DELETE SET NULL,
    item_name   TEXT NOT NULL,
    recipient   TEXT NOT NULL,
    source_mob  TEXT,
    zone        TEXT,
    table_name  TEXT,
    quantity    INTEGER NOT NULL DEFAULT 1,
    assigned_by TEXT
);
CREATE INDEX IF NOT EXISTS idx_ll_item ON loot_log(item_name);
CREATE INDEX IF NOT EXISTS idx_ll_char ON loot_log(recipient);
CREATE INDEX IF NOT EXISTS idx_ll_ts ON loot_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_ll_mob ON loot_log(source_mob);
";

/// Item data for bulk import (Lucy/Allakhazam pipeline).
#[derive(Debug, Clone)]
pub struct ImportItem {
    pub name: String,
    pub lucy_id: Option<i64>,
    pub slot: Option<String>,
    pub item_type: Option<String>,
    pub ac: i64,
    pub hp: i64,
    pub mana: i64,
    pub damage: i64,
    pub delay: i64,
    pub level_req: i64,
    pub weight: i64,
    pub magic: bool,
    pub lore: bool,
    pub nodrop: bool,
    pub expansion: Option<String>,
    pub effect: Option<String>,
    pub stats_json: Option<String>,
    pub source_url: Option<String>,
    pub classes: Vec<String>,
}

/// EQ item database and loot management store.
pub struct LootStore {
    conn: Mutex<Connection>,
}

impl LootStore {
    /// Open (or create) the loot database at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        let conn =
            Connection::open(path).with_context(|| format!("Failed to open loot DB: {path:?}"))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to set PRAGMA")?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize loot schema")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory loot database (for testing).
    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("Failed to open in-memory DB")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .context("Failed to set PRAGMA")?;
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize loot schema")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    // ── Items ──────────────────────────────────────────────────────────────

    /// Import an item into the database (upsert by name).
    ///
    /// Returns the item's row ID.
    pub fn upsert_item(&self, item: &ImportItem) -> Result<i64> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        conn.execute(
            "INSERT INTO items (name, lucy_id, slot, item_type, ac, hp, mana, damage, delay,
                level_req, weight, magic, lore, nodrop, expansion, effect, stats_json, source_url,
                updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, datetime('now'))
             ON CONFLICT(name) DO UPDATE SET
                lucy_id = excluded.lucy_id,
                slot = excluded.slot,
                item_type = excluded.item_type,
                ac = excluded.ac,
                hp = excluded.hp,
                mana = excluded.mana,
                damage = excluded.damage,
                delay = excluded.delay,
                level_req = excluded.level_req,
                weight = excluded.weight,
                magic = excluded.magic,
                lore = excluded.lore,
                nodrop = excluded.nodrop,
                expansion = excluded.expansion,
                effect = excluded.effect,
                stats_json = excluded.stats_json,
                source_url = excluded.source_url,
                updated_at = datetime('now')",
            params![
                item.name,
                item.lucy_id,
                item.slot,
                item.item_type,
                item.ac,
                item.hp,
                item.mana,
                item.damage,
                item.delay,
                item.level_req,
                item.weight,
                item.magic as i64,
                item.lore as i64,
                item.nodrop as i64,
                item.expansion,
                item.effect,
                item.stats_json,
                item.source_url,
            ],
        )
        .context("Failed to upsert item")?;

        // last_insert_rowid() returns 0 on conflict-update, so query the actual ID.
        let item_id: i64 = conn
            .query_row(
                "SELECT id FROM items WHERE name = ?1",
                params![item.name],
                |row| row.get(0),
            )
            .context("Failed to get item ID after upsert")?;

        // Refresh class usability entries.
        conn.execute(
            "DELETE FROM item_classes WHERE item_id = ?1",
            params![item_id],
        )
        .context("Failed to clear item classes")?;
        for class in &item.classes {
            conn.execute(
                "INSERT INTO item_classes (item_id, class) VALUES (?1, ?2)",
                params![item_id, class],
            )
            .context("Failed to insert item class")?;
        }

        Ok(item_id)
    }

    /// Bulk import items within a single transaction.
    pub fn import_items(&self, items: &[ImportItem]) -> Result<Vec<i64>> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        let tx = conn
            .unchecked_transaction()
            .context("Failed to begin transaction")?;
        let mut ids = Vec::with_capacity(items.len());

        for item in items {
            tx.execute(
                "INSERT INTO items (name, lucy_id, slot, item_type, ac, hp, mana, damage, delay,
                    level_req, weight, magic, lore, nodrop, expansion, effect, stats_json, source_url,
                    updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, datetime('now'))
                 ON CONFLICT(name) DO UPDATE SET
                    lucy_id = excluded.lucy_id,
                    slot = excluded.slot,
                    item_type = excluded.item_type,
                    ac = excluded.ac,
                    hp = excluded.hp,
                    mana = excluded.mana,
                    damage = excluded.damage,
                    delay = excluded.delay,
                    level_req = excluded.level_req,
                    weight = excluded.weight,
                    magic = excluded.magic,
                    lore = excluded.lore,
                    nodrop = excluded.nodrop,
                    expansion = excluded.expansion,
                    effect = excluded.effect,
                    stats_json = excluded.stats_json,
                    source_url = excluded.source_url,
                    updated_at = datetime('now')",
                params![
                    item.name,
                    item.lucy_id,
                    item.slot,
                    item.item_type,
                    item.ac,
                    item.hp,
                    item.mana,
                    item.damage,
                    item.delay,
                    item.level_req,
                    item.weight,
                    item.magic as i64,
                    item.lore as i64,
                    item.nodrop as i64,
                    item.expansion,
                    item.effect,
                    item.stats_json,
                    item.source_url,
                ],
            )
            .context("Failed to upsert item in batch")?;

            let item_id: i64 = tx
                .query_row(
                    "SELECT id FROM items WHERE name = ?1",
                    params![item.name],
                    |row| row.get(0),
                )
                .context("Failed to get item ID in batch")?;

            tx.execute(
                "DELETE FROM item_classes WHERE item_id = ?1",
                params![item_id],
            )
            .context("Failed to clear item classes")?;
            for class in &item.classes {
                tx.execute(
                    "INSERT INTO item_classes (item_id, class) VALUES (?1, ?2)",
                    params![item_id, class],
                )
                .context("Failed to insert item class")?;
            }

            ids.push(item_id);
        }

        tx.commit().context("Failed to commit item import")?;
        Ok(ids)
    }

    /// Get an item by its database ID.
    pub fn get_item(&self, item_id: i64) -> Result<Option<ItemRow>> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        let row = conn
            .query_row(
                "SELECT id, name, lucy_id, slot, item_type, ac, hp, mana, damage, delay,
                        level_req, weight, magic, lore, nodrop, expansion, effect, stats_json,
                        source_url, created_at, updated_at
                 FROM items WHERE id = ?1",
                params![item_id],
                map_item_row,
            )
            .optional()
            .context("Failed to query item")?;
        Ok(row)
    }

    /// Look up an item by exact name.
    pub fn get_item_by_name(&self, name: &str) -> Result<Option<ItemRow>> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        let row = conn
            .query_row(
                "SELECT id, name, lucy_id, slot, item_type, ac, hp, mana, damage, delay,
                        level_req, weight, magic, lore, nodrop, expansion, effect, stats_json,
                        source_url, created_at, updated_at
                 FROM items WHERE name = ?1",
                params![name],
                map_item_row,
            )
            .optional()
            .context("Failed to query item by name")?;
        Ok(row)
    }

    /// Get classes that can use an item.
    pub fn item_classes(&self, item_id: i64) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        let mut stmt = conn
            .prepare("SELECT class FROM item_classes WHERE item_id = ?1 ORDER BY class")
            .context("Failed to prepare class query")?;
        let rows = stmt
            .query_map(params![item_id], |row| row.get(0))
            .context("Failed to query item classes")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect item classes")
    }

    /// Search items with flexible filters.
    pub fn search_items(&self, filter: &ItemSearchFilter) -> Result<Vec<ItemRow>> {
        let mut sql = String::from(
            "SELECT DISTINCT i.id, i.name, i.lucy_id, i.slot, i.item_type, i.ac, i.hp, i.mana,
                    i.damage, i.delay, i.level_req, i.weight, i.magic, i.lore, i.nodrop,
                    i.expansion, i.effect, i.stats_json, i.source_url, i.created_at, i.updated_at
             FROM items i",
        );
        let mut conditions: Vec<String> = Vec::new();
        let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1u32;

        if filter.class.is_some() {
            sql.push_str(" JOIN item_classes ic ON ic.item_id = i.id");
        }

        if let Some(ref name) = filter.name {
            conditions.push(format!("i.name LIKE ?{param_idx}"));
            values.push(Box::new(format!("%{name}%")));
            param_idx += 1;
        }
        if let Some(ref slot) = filter.slot {
            conditions.push(format!("i.slot = ?{param_idx}"));
            values.push(Box::new(slot.clone()));
            param_idx += 1;
        }
        if let Some(ref class) = filter.class {
            conditions.push(format!("ic.class = ?{param_idx}"));
            values.push(Box::new(class.clone()));
            param_idx += 1;
        }
        if let Some(min_level) = filter.min_level {
            conditions.push(format!("i.level_req >= ?{param_idx}"));
            values.push(Box::new(min_level));
            param_idx += 1;
        }
        if let Some(max_level) = filter.max_level {
            conditions.push(format!("i.level_req <= ?{param_idx}"));
            values.push(Box::new(max_level));
            param_idx += 1;
        }
        if let Some(ref expansion) = filter.expansion {
            conditions.push(format!("i.expansion = ?{param_idx}"));
            values.push(Box::new(expansion.clone()));
            param_idx += 1;
        }
        if let Some(ref item_type) = filter.item_type {
            conditions.push(format!("i.item_type = ?{param_idx}"));
            values.push(Box::new(item_type.clone()));
            param_idx += 1;
        }
        if filter.magic_only {
            conditions.push(format!("i.magic = ?{param_idx}"));
            values.push(Box::new(1i64));
            param_idx += 1;
        }
        if filter.lore_only {
            conditions.push(format!("i.lore = ?{param_idx}"));
            values.push(Box::new(1i64));
            param_idx += 1;
        }
        if filter.nodrop_only {
            conditions.push(format!("i.nodrop = ?{param_idx}"));
            values.push(Box::new(1i64));
        }
        let _ = param_idx;

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY i.name LIMIT 200");

        let conn = self.conn.lock().expect("loot lock poisoned");
        let mut stmt = conn
            .prepare(&sql)
            .context("Failed to prepare search query")?;
        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            values.iter().map(|v| v.as_ref()).collect();
        let rows = stmt
            .query_map(params_ref.as_slice(), map_item_row)
            .context("Failed to execute search")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect search results")
    }

    /// Total item count in the database.
    pub fn item_count(&self) -> Result<i64> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        conn.query_row("SELECT COUNT(*) FROM items", [], |row| row.get(0))
            .context("Failed to count items")
    }

    // ── Loot Tables (Dave's TLP overlay) ───────────────────────────────────

    /// Define a loot table entry: which mob belongs to which table.
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_loot_table(
        &self,
        table_name: &str,
        mob_name: &str,
        zone: Option<&str>,
        min_level: Option<i64>,
        max_level: Option<i64>,
        expansion: Option<&str>,
        notes: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        conn.execute(
            "INSERT INTO loot_tables (table_name, mob_name, zone, min_level, max_level, expansion, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(table_name, mob_name) DO UPDATE SET
                zone = excluded.zone,
                min_level = excluded.min_level,
                max_level = excluded.max_level,
                expansion = excluded.expansion,
                notes = excluded.notes",
            params![table_name, mob_name, zone, min_level, max_level, expansion, notes],
        )
        .context("Failed to upsert loot table entry")?;
        Ok(conn.last_insert_rowid())
    }

    /// Set a drop rate for an item within a loot table.
    pub fn upsert_drop_rate(
        &self,
        table_name: &str,
        item_id: i64,
        drop_chance: f64,
        min_qty: i64,
        max_qty: i64,
        notes: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        conn.execute(
            "INSERT INTO drop_rates (table_name, item_id, drop_chance, min_qty, max_qty, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(table_name, item_id) DO UPDATE SET
                drop_chance = excluded.drop_chance,
                min_qty = excluded.min_qty,
                max_qty = excluded.max_qty,
                notes = excluded.notes",
            params![table_name, item_id, drop_chance, min_qty, max_qty, notes],
        )
        .context("Failed to upsert drop rate")?;
        Ok(())
    }

    /// Get all loot table entries by table name.
    pub fn loot_table_entries(&self, table_name: &str) -> Result<Vec<LootTableRow>> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT id, table_name, mob_name, zone, min_level, max_level, expansion, notes, created_at
                 FROM loot_tables WHERE table_name = ?1 ORDER BY mob_name",
            )
            .context("Failed to prepare loot table query")?;
        let rows = stmt
            .query_map(params![table_name], |row| {
                Ok(LootTableRow {
                    id: row.get(0)?,
                    table_name: row.get(1)?,
                    mob_name: row.get(2)?,
                    zone: row.get(3)?,
                    min_level: row.get(4)?,
                    max_level: row.get(5)?,
                    expansion: row.get(6)?,
                    notes: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .context("Failed to query loot tables")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect loot table rows")
    }

    /// Get all mobs that can drop a specific item (via loot tables).
    pub fn mobs_dropping_item(&self, item_id: i64) -> Result<Vec<LootTableRow>> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT lt.id, lt.table_name, lt.mob_name, lt.zone, lt.min_level, lt.max_level,
                        lt.expansion, lt.notes, lt.created_at
                 FROM loot_tables lt
                 JOIN drop_rates dr ON dr.table_name = lt.table_name
                 WHERE dr.item_id = ?1
                 ORDER BY lt.mob_name",
            )
            .context("Failed to prepare mob drop query")?;
        let rows = stmt
            .query_map(params![item_id], |row| {
                Ok(LootTableRow {
                    id: row.get(0)?,
                    table_name: row.get(1)?,
                    mob_name: row.get(2)?,
                    zone: row.get(3)?,
                    min_level: row.get(4)?,
                    max_level: row.get(5)?,
                    expansion: row.get(6)?,
                    notes: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })
            .context("Failed to query mobs dropping item")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect mob drop rows")
    }

    /// Drop rates for all items within a loot table.
    pub fn drop_rates(&self, table_name: &str) -> Result<Vec<DropRateRow>> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT dr.id, dr.table_name, dr.item_id, i.name, dr.drop_chance,
                        dr.min_qty, dr.max_qty, dr.notes
                 FROM drop_rates dr
                 JOIN items i ON i.id = dr.item_id
                 WHERE dr.table_name = ?1
                 ORDER BY dr.drop_chance DESC",
            )
            .context("Failed to prepare drop rate query")?;
        let rows = stmt
            .query_map(params![table_name], |row| {
                Ok(DropRateRow {
                    id: row.get(0)?,
                    table_name: row.get(1)?,
                    item_id: row.get(2)?,
                    item_name: row.get(3)?,
                    drop_chance: row.get(4)?,
                    min_qty: row.get(5)?,
                    max_qty: row.get(6)?,
                    notes: row.get(7)?,
                })
            })
            .context("Failed to query drop rates")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect drop rate rows")
    }

    // ── Wishlists ──────────────────────────────────────────────────────────

    /// Add or update a wishlist entry for a character.
    pub fn upsert_wishlist(
        &self,
        character: &str,
        item_id: i64,
        priority: i64,
        slot: Option<&str>,
        notes: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        conn.execute(
            "INSERT INTO wishlists (character, item_id, priority, slot, notes)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(character, item_id) DO UPDATE SET
                priority = excluded.priority,
                slot = excluded.slot,
                notes = excluded.notes",
            params![character, item_id, priority, slot, notes],
        )
        .context("Failed to upsert wishlist entry")?;
        Ok(())
    }

    /// Mark a wishlist item as obtained.
    pub fn mark_obtained(&self, character: &str, item_id: i64) -> Result<bool> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        let updated = conn
            .execute(
                "UPDATE wishlists SET obtained = 1, obtained_at = datetime('now')
                 WHERE character = ?1 AND item_id = ?2",
                params![character, item_id],
            )
            .context("Failed to mark item obtained")?;
        Ok(updated > 0)
    }

    /// Get a character's wishlist, sorted by priority (1 = highest).
    pub fn character_wishlist(
        &self,
        character: &str,
        include_obtained: bool,
    ) -> Result<Vec<WishlistRow>> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        let sql = if include_obtained {
            "SELECT w.id, w.character, w.item_id, i.name, w.priority, w.slot, w.notes,
                    w.obtained, w.obtained_at, w.created_at
             FROM wishlists w JOIN items i ON i.id = w.item_id
             WHERE w.character = ?1
             ORDER BY w.obtained ASC, w.priority ASC"
        } else {
            "SELECT w.id, w.character, w.item_id, i.name, w.priority, w.slot, w.notes,
                    w.obtained, w.obtained_at, w.created_at
             FROM wishlists w JOIN items i ON i.id = w.item_id
             WHERE w.character = ?1 AND w.obtained = 0
             ORDER BY w.priority ASC"
        };
        let mut stmt = conn
            .prepare(sql)
            .context("Failed to prepare wishlist query")?;
        let rows = stmt
            .query_map(params![character], |row| {
                Ok(WishlistRow {
                    id: row.get(0)?,
                    character: row.get(1)?,
                    item_id: row.get(2)?,
                    item_name: row.get(3)?,
                    priority: row.get(4)?,
                    slot: row.get(5)?,
                    notes: row.get(6)?,
                    obtained: row.get(7)?,
                    obtained_at: row.get(8)?,
                    created_at: row.get(9)?,
                })
            })
            .context("Failed to query wishlist")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect wishlist rows")
    }

    /// "Who needs this?" — find all characters who have this item on their wishlist
    /// and haven't obtained it yet.
    pub fn who_needs_item(&self, item_id: i64) -> Result<Vec<WishlistRow>> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT w.id, w.character, w.item_id, i.name, w.priority, w.slot, w.notes,
                        w.obtained, w.obtained_at, w.created_at
                 FROM wishlists w JOIN items i ON i.id = w.item_id
                 WHERE w.item_id = ?1 AND w.obtained = 0
                 ORDER BY w.priority ASC, w.character ASC",
            )
            .context("Failed to prepare 'who needs' query")?;
        let rows = stmt
            .query_map(params![item_id], |row| {
                Ok(WishlistRow {
                    id: row.get(0)?,
                    character: row.get(1)?,
                    item_id: row.get(2)?,
                    item_name: row.get(3)?,
                    priority: row.get(4)?,
                    slot: row.get(5)?,
                    notes: row.get(6)?,
                    obtained: row.get(7)?,
                    obtained_at: row.get(8)?,
                    created_at: row.get(9)?,
                })
            })
            .context("Failed to query who needs item")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect 'who needs' rows")
    }

    /// Remove a wishlist entry.
    pub fn remove_wishlist_entry(&self, character: &str, item_id: i64) -> Result<bool> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        let deleted = conn
            .execute(
                "DELETE FROM wishlists WHERE character = ?1 AND item_id = ?2",
                params![character, item_id],
            )
            .context("Failed to remove wishlist entry")?;
        Ok(deleted > 0)
    }

    // ── Loot History ───────────────────────────────────────────────────────

    /// Record a loot drop.
    #[allow(clippy::too_many_arguments)]
    pub fn record_loot(
        &self,
        item_id: Option<i64>,
        item_name: &str,
        recipient: &str,
        source_mob: Option<&str>,
        zone: Option<&str>,
        table_name: Option<&str>,
        quantity: i64,
        assigned_by: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        conn.execute(
            "INSERT INTO loot_log (item_id, item_name, recipient, source_mob, zone, table_name, quantity, assigned_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![item_id, item_name, recipient, source_mob, zone, table_name, quantity, assigned_by],
        )
        .context("Failed to record loot")?;
        Ok(conn.last_insert_rowid())
    }

    /// Recent loot history, newest first.
    pub fn recent_loot_history(&self, limit: u32) -> Result<Vec<LootHistoryRow>> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT id, timestamp, item_id, item_name, recipient, source_mob, zone,
                        table_name, quantity, assigned_by
                 FROM loot_log ORDER BY id DESC LIMIT ?1",
            )
            .context("Failed to prepare loot history query")?;
        let rows = stmt
            .query_map(params![limit], map_loot_history_row)
            .context("Failed to query loot history")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect loot history rows")
    }

    /// Loot history for a specific character.
    pub fn character_loot_history(
        &self,
        character: &str,
        limit: u32,
    ) -> Result<Vec<LootHistoryRow>> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        let mut stmt = conn
            .prepare(
                "SELECT id, timestamp, item_id, item_name, recipient, source_mob, zone,
                        table_name, quantity, assigned_by
                 FROM loot_log WHERE recipient = ?1 ORDER BY id DESC LIMIT ?2",
            )
            .context("Failed to prepare character loot query")?;
        let rows = stmt
            .query_map(params![character, limit], map_loot_history_row)
            .context("Failed to query character loot")?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .context("Failed to collect character loot rows")
    }

    /// How many times has a specific item dropped?
    pub fn item_drop_count(&self, item_name: &str) -> Result<i64> {
        let conn = self.conn.lock().expect("loot lock poisoned");
        conn.query_row(
            "SELECT COALESCE(SUM(quantity), 0) FROM loot_log WHERE item_name = ?1",
            params![item_name],
            |row| row.get(0),
        )
        .context("Failed to count item drops")
    }
}

// ── Row mapper helpers ─────────────────────────────────────────────────────

fn map_item_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ItemRow> {
    Ok(ItemRow {
        id: row.get(0)?,
        name: row.get(1)?,
        lucy_id: row.get(2)?,
        slot: row.get(3)?,
        item_type: row.get(4)?,
        ac: row.get(5)?,
        hp: row.get(6)?,
        mana: row.get(7)?,
        damage: row.get(8)?,
        delay: row.get(9)?,
        level_req: row.get(10)?,
        weight: row.get(11)?,
        magic: row.get(12)?,
        lore: row.get(13)?,
        nodrop: row.get(14)?,
        expansion: row.get(15)?,
        effect: row.get(16)?,
        stats_json: row.get(17)?,
        source_url: row.get(18)?,
        created_at: row.get(19)?,
        updated_at: row.get(20)?,
    })
}

fn map_loot_history_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LootHistoryRow> {
    Ok(LootHistoryRow {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        item_id: row.get(2)?,
        item_name: row.get(3)?,
        recipient: row.get(4)?,
        source_mob: row.get(5)?,
        zone: row.get(6)?,
        table_name: row.get(7)?,
        quantity: row.get(8)?,
        assigned_by: row.get(9)?,
    })
}

// ── Row types ──────────────────────────────────────────────────────────────

/// A row from the `items` table.
#[derive(Debug)]
pub struct ItemRow {
    pub id: i64,
    pub name: String,
    pub lucy_id: Option<i64>,
    pub slot: Option<String>,
    pub item_type: Option<String>,
    pub ac: i64,
    pub hp: i64,
    pub mana: i64,
    pub damage: i64,
    pub delay: i64,
    pub level_req: i64,
    pub weight: i64,
    pub magic: bool,
    pub lore: bool,
    pub nodrop: bool,
    pub expansion: Option<String>,
    pub effect: Option<String>,
    pub stats_json: Option<String>,
    pub source_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A row from the `loot_tables` table.
#[derive(Debug)]
pub struct LootTableRow {
    pub id: i64,
    pub table_name: String,
    pub mob_name: String,
    pub zone: Option<String>,
    pub min_level: Option<i64>,
    pub max_level: Option<i64>,
    pub expansion: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
}

/// A drop rate entry with joined item name.
#[derive(Debug)]
pub struct DropRateRow {
    pub id: i64,
    pub table_name: String,
    pub item_id: i64,
    pub item_name: String,
    pub drop_chance: f64,
    pub min_qty: i64,
    pub max_qty: i64,
    pub notes: Option<String>,
}

/// A wishlist entry with joined item name.
#[derive(Debug)]
pub struct WishlistRow {
    pub id: i64,
    pub character: String,
    pub item_id: i64,
    pub item_name: String,
    pub priority: i64,
    pub slot: Option<String>,
    pub notes: Option<String>,
    pub obtained: bool,
    pub obtained_at: Option<String>,
    pub created_at: String,
}

/// A row from the `loot_log` table.
#[derive(Debug)]
pub struct LootHistoryRow {
    pub id: i64,
    pub timestamp: String,
    pub item_id: Option<i64>,
    pub item_name: String,
    pub recipient: String,
    pub source_mob: Option<String>,
    pub zone: Option<String>,
    pub table_name: Option<String>,
    pub quantity: i64,
    pub assigned_by: Option<String>,
}

/// Flexible search filter for item queries.
#[derive(Debug, Default)]
pub struct ItemSearchFilter {
    pub name: Option<String>,
    pub slot: Option<String>,
    pub class: Option<String>,
    pub min_level: Option<i64>,
    pub max_level: Option<i64>,
    pub expansion: Option<String>,
    pub item_type: Option<String>,
    pub magic_only: bool,
    pub lore_only: bool,
    pub nodrop_only: bool,
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item(name: &str) -> ImportItem {
        ImportItem {
            name: name.to_string(),
            lucy_id: Some(12345),
            slot: Some("Primary".to_string()),
            item_type: Some("1HS".to_string()),
            ac: 0,
            hp: 25,
            mana: 0,
            damage: 12,
            delay: 20,
            level_req: 50,
            weight: 35,
            magic: true,
            lore: false,
            nodrop: true,
            expansion: Some("Classic".to_string()),
            effect: Some("Fiery Avenger Effect".to_string()),
            stats_json: None,
            source_url: Some("https://lucy.example.com/item/12345".to_string()),
            classes: vec!["Paladin".to_string(), "Shadowknight".to_string()],
        }
    }

    #[test]
    fn open_and_upsert_item() {
        let store = LootStore::open_memory().unwrap();
        let item = sample_item("Fiery Avenger");
        let id = store.upsert_item(&item).unwrap();
        assert!(id > 0);

        let fetched = store.get_item(id).unwrap().unwrap();
        assert_eq!(fetched.name, "Fiery Avenger");
        assert_eq!(fetched.damage, 12);
        assert_eq!(fetched.delay, 20);
        assert!(fetched.magic);
        assert!(fetched.nodrop);
    }

    #[test]
    fn upsert_item_updates_existing() {
        let store = LootStore::open_memory().unwrap();
        let mut item = sample_item("SoS");
        item.damage = 15;
        store.upsert_item(&item).unwrap();

        item.damage = 20;
        store.upsert_item(&item).unwrap();

        let fetched = store.get_item_by_name("SoS").unwrap().unwrap();
        assert_eq!(fetched.damage, 20);
        assert_eq!(store.item_count().unwrap(), 1);
    }

    #[test]
    fn item_class_usability() {
        let store = LootStore::open_memory().unwrap();
        let item = sample_item("Fiery Avenger");
        let id = store.upsert_item(&item).unwrap();

        let classes = store.item_classes(id).unwrap();
        assert_eq!(classes, vec!["Paladin", "Shadowknight"]);
    }

    #[test]
    fn search_by_slot() {
        let store = LootStore::open_memory().unwrap();
        store
            .upsert_item(&sample_item("Sword of Strategy"))
            .unwrap();

        let mut bow = sample_item("Bow of the Destroyer");
        bow.slot = Some("Range".to_string());
        store.upsert_item(&bow).unwrap();

        let filter = ItemSearchFilter {
            slot: Some("Primary".to_string()),
            ..Default::default()
        };
        let results = store.search_items(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Sword of Strategy");
    }

    #[test]
    fn search_by_class() {
        let store = LootStore::open_memory().unwrap();
        store.upsert_item(&sample_item("Fiery Avenger")).unwrap();

        let mut wiz_item = sample_item("Staff of Temperate Flux");
        wiz_item.classes = vec!["Wizard".to_string()];
        store.upsert_item(&wiz_item).unwrap();

        let filter = ItemSearchFilter {
            class: Some("Wizard".to_string()),
            ..Default::default()
        };
        let results = store.search_items(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Staff of Temperate Flux");
    }

    #[test]
    fn search_by_name_partial() {
        let store = LootStore::open_memory().unwrap();
        store.upsert_item(&sample_item("Cloak of Flames")).unwrap();
        store.upsert_item(&sample_item("Cloak of Shadows")).unwrap();
        store.upsert_item(&sample_item("Sword of Truth")).unwrap();

        let filter = ItemSearchFilter {
            name: Some("Cloak".to_string()),
            ..Default::default()
        };
        let results = store.search_items(&filter).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_by_expansion_and_level() {
        let store = LootStore::open_memory().unwrap();
        let mut item = sample_item("Primal Weapon");
        item.expansion = Some("Velious".to_string());
        item.level_req = 55;
        store.upsert_item(&item).unwrap();

        store.upsert_item(&sample_item("Classic Sword")).unwrap();

        let filter = ItemSearchFilter {
            expansion: Some("Velious".to_string()),
            min_level: Some(50),
            ..Default::default()
        };
        let results = store.search_items(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Primal Weapon");
    }

    #[test]
    fn search_magic_only() {
        let store = LootStore::open_memory().unwrap();
        store.upsert_item(&sample_item("Magic Blade")).unwrap();

        let mut mundane = sample_item("Rusty Sword");
        mundane.magic = false;
        store.upsert_item(&mundane).unwrap();

        let filter = ItemSearchFilter {
            magic_only: true,
            ..Default::default()
        };
        let results = store.search_items(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Magic Blade");
    }

    #[test]
    fn bulk_import() {
        let store = LootStore::open_memory().unwrap();
        let items = vec![
            sample_item("Item A"),
            sample_item("Item B"),
            sample_item("Item C"),
        ];
        let ids = store.import_items(&items).unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(store.item_count().unwrap(), 3);
    }

    #[test]
    fn loot_table_and_drop_rates() {
        let store = LootStore::open_memory().unwrap();
        let item_id = store.upsert_item(&sample_item("Cloak of Flames")).unwrap();

        store
            .upsert_loot_table(
                "nagafen_table",
                "Lord Nagafen",
                Some("soldungb"),
                Some(55),
                Some(55),
                Some("Classic"),
                None,
            )
            .unwrap();

        store
            .upsert_drop_rate("nagafen_table", item_id, 0.15, 1, 1, Some("rare"))
            .unwrap();

        let entries = store.loot_table_entries("nagafen_table").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].mob_name, "Lord Nagafen");

        let rates = store.drop_rates("nagafen_table").unwrap();
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[0].item_name, "Cloak of Flames");
        assert!((rates[0].drop_chance - 0.15).abs() < f64::EPSILON);
    }

    #[test]
    fn mobs_dropping_item_query() {
        let store = LootStore::open_memory().unwrap();
        let item_id = store.upsert_item(&sample_item("Cloak of Flames")).unwrap();

        store
            .upsert_loot_table(
                "nagafen_table",
                "Lord Nagafen",
                Some("soldungb"),
                None,
                None,
                None,
                None,
            )
            .unwrap();
        store
            .upsert_loot_table(
                "vox_table",
                "Lady Vox",
                Some("permafrost"),
                None,
                None,
                None,
                None,
            )
            .unwrap();

        store
            .upsert_drop_rate("nagafen_table", item_id, 0.15, 1, 1, None)
            .unwrap();
        store
            .upsert_drop_rate("vox_table", item_id, 0.10, 1, 1, None)
            .unwrap();

        let mobs = store.mobs_dropping_item(item_id).unwrap();
        assert_eq!(mobs.len(), 2);
        let mob_names: Vec<&str> = mobs.iter().map(|m| m.mob_name.as_str()).collect();
        assert!(mob_names.contains(&"Lady Vox"));
        assert!(mob_names.contains(&"Lord Nagafen"));
    }

    #[test]
    fn wishlist_crud() {
        let store = LootStore::open_memory().unwrap();
        let id = store.upsert_item(&sample_item("Fiery Avenger")).unwrap();

        store
            .upsert_wishlist("Paladin01", id, 1, Some("Primary"), Some("BiS"))
            .unwrap();
        store
            .upsert_wishlist("Paladin02", id, 3, Some("Primary"), None)
            .unwrap();

        let wl = store.character_wishlist("Paladin01", false).unwrap();
        assert_eq!(wl.len(), 1);
        assert_eq!(wl[0].priority, 1);
        assert_eq!(wl[0].item_name, "Fiery Avenger");
        assert!(!wl[0].obtained);
    }

    #[test]
    fn wishlist_mark_obtained() {
        let store = LootStore::open_memory().unwrap();
        let id = store.upsert_item(&sample_item("Epic 1.0")).unwrap();
        store
            .upsert_wishlist("Warrior01", id, 1, None, None)
            .unwrap();

        assert!(store.mark_obtained("Warrior01", id).unwrap());

        let active = store.character_wishlist("Warrior01", false).unwrap();
        assert!(active.is_empty());

        let all = store.character_wishlist("Warrior01", true).unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].obtained);
        assert!(all[0].obtained_at.is_some());
    }

    #[test]
    fn who_needs_item() {
        let store = LootStore::open_memory().unwrap();
        let id = store.upsert_item(&sample_item("Cloak of Flames")).unwrap();

        store
            .upsert_wishlist("Warrior01", id, 1, None, None)
            .unwrap();
        store.upsert_wishlist("Monk01", id, 3, None, None).unwrap();
        store.upsert_wishlist("Rogue01", id, 2, None, None).unwrap();

        store.mark_obtained("Warrior01", id).unwrap();

        let needing = store.who_needs_item(id).unwrap();
        assert_eq!(needing.len(), 2);
        assert_eq!(needing[0].character, "Rogue01");
        assert_eq!(needing[1].character, "Monk01");
    }

    #[test]
    fn remove_wishlist_entry() {
        let store = LootStore::open_memory().unwrap();
        let id = store.upsert_item(&sample_item("Sword")).unwrap();
        store.upsert_wishlist("Char01", id, 1, None, None).unwrap();

        assert!(store.remove_wishlist_entry("Char01", id).unwrap());
        assert!(!store.remove_wishlist_entry("Char01", id).unwrap());

        let wl = store.character_wishlist("Char01", true).unwrap();
        assert!(wl.is_empty());
    }

    #[test]
    fn loot_history_recording() {
        let store = LootStore::open_memory().unwrap();
        let item_id = store.upsert_item(&sample_item("Cloak of Flames")).unwrap();

        store
            .record_loot(
                Some(item_id),
                "Cloak of Flames",
                "Warrior01",
                Some("Lord Nagafen"),
                Some("soldungb"),
                Some("nagafen_table"),
                1,
                Some("DKP"),
            )
            .unwrap();
        store
            .record_loot(
                None,
                "Torn Cloth Sandal",
                "Monk01",
                Some("orc_pawn"),
                None,
                None,
                2,
                None,
            )
            .unwrap();

        let history = store.recent_loot_history(10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].item_name, "Torn Cloth Sandal");
        assert_eq!(history[0].quantity, 2);
        assert_eq!(history[1].item_name, "Cloak of Flames");
        assert_eq!(history[1].assigned_by.as_deref(), Some("DKP"));
    }

    #[test]
    fn character_loot_history() {
        let store = LootStore::open_memory().unwrap();
        store
            .record_loot(None, "Item A", "Char_A", None, None, None, 1, None)
            .unwrap();
        store
            .record_loot(None, "Item B", "Char_B", None, None, None, 1, None)
            .unwrap();
        store
            .record_loot(None, "Item C", "Char_A", None, None, None, 1, None)
            .unwrap();

        let history = store.character_loot_history("Char_A", 10).unwrap();
        assert_eq!(history.len(), 2);
        assert!(history.iter().all(|h| h.recipient == "Char_A"));
    }

    #[test]
    fn item_drop_count() {
        let store = LootStore::open_memory().unwrap();
        store
            .record_loot(
                None,
                "Cloak of Flames",
                "Warrior01",
                None,
                None,
                None,
                1,
                None,
            )
            .unwrap();
        store
            .record_loot(None, "Cloak of Flames", "Monk01", None, None, None, 1, None)
            .unwrap();
        store
            .record_loot(None, "Rusty Sword", "Warrior01", None, None, None, 3, None)
            .unwrap();

        assert_eq!(store.item_drop_count("Cloak of Flames").unwrap(), 2);
        assert_eq!(store.item_drop_count("Rusty Sword").unwrap(), 3);
        assert_eq!(store.item_drop_count("Nonexistent").unwrap(), 0);
    }

    #[test]
    fn empty_search_returns_all() {
        let store = LootStore::open_memory().unwrap();
        store.upsert_item(&sample_item("Item 1")).unwrap();
        store.upsert_item(&sample_item("Item 2")).unwrap();

        let filter = ItemSearchFilter::default();
        let results = store.search_items(&filter).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn loot_table_upsert_updates_existing() {
        let store = LootStore::open_memory().unwrap();
        store
            .upsert_loot_table("table_a", "MobX", Some("zone1"), None, None, None, None)
            .unwrap();
        store
            .upsert_loot_table(
                "table_a",
                "MobX",
                Some("zone2"),
                None,
                None,
                None,
                Some("updated"),
            )
            .unwrap();

        let entries = store.loot_table_entries("table_a").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].zone.as_deref(), Some("zone2"));
        assert_eq!(entries[0].notes.as_deref(), Some("updated"));
    }

    #[test]
    fn drop_rate_upsert_updates_existing() {
        let store = LootStore::open_memory().unwrap();
        let item_id = store.upsert_item(&sample_item("TestItem")).unwrap();

        store
            .upsert_drop_rate("table_a", item_id, 0.10, 1, 1, None)
            .unwrap();
        store
            .upsert_drop_rate("table_a", item_id, 0.25, 1, 2, Some("buffed"))
            .unwrap();

        let rates = store.drop_rates("table_a").unwrap();
        assert_eq!(rates.len(), 1);
        assert!((rates[0].drop_chance - 0.25).abs() < f64::EPSILON);
        assert_eq!(rates[0].max_qty, 2);
    }
}
