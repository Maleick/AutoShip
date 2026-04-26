//! Sessions telemetry database for combat analysis, healing, spells, buffs, and events.
//!
//! Stores detailed event logs from EQ combat, organized by session with comprehensive
//! event tables covering damage, healing, spells, buffs, deaths, loot, and more.
//! Designed to support self-improvement analytics and raid reconstruction.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::Path;

pub mod migrations;

// ─── Modifiers Bitmask ────────────────────────────────────────────────────────

/// 12-bit damage modifier mask matching EQLogParser spec.
/// Each bit represents a modifier that can be applied to a damage event.
pub mod modifiers {
    pub const CRITICAL: u32 = 1 << 0; // Bit 0
    pub const LUCKY: u32 = 1 << 1; // Bit 1
    pub const TWINCAST: u32 = 1 << 2; // Bit 2
    pub const RAMPAGE: u32 = 1 << 3; // Bit 3
    pub const ASSASSINATE: u32 = 1 << 4; // Bit 4
    pub const HEADSHOT: u32 = 1 << 5; // Bit 5
    pub const FINISHING_BLOW: u32 = 1 << 6; // Bit 6
    pub const DOUBLE_BOW: u32 = 1 << 7; // Bit 7
    pub const FLURRY: u32 = 1 << 8; // Bit 8
    pub const STRIKETHROUGH: u32 = 1 << 9; // Bit 9
    pub const RIPOSTE: u32 = 1 << 10; // Bit 10
    pub const SLAY: u32 = 1 << 11; // Bit 11

    pub const MASK_ALL: u32 = 0xFFF; // All 12 bits

    pub fn has_flag(modifiers: u32, flag: u32) -> bool {
        (modifiers & flag) != 0
    }

    pub fn set_flag(modifiers: u32, flag: u32) -> u32 {
        modifiers | flag
    }

    pub fn clear_flag(modifiers: u32, flag: u32) -> u32 {
        modifiers & !flag
    }

    pub fn flag_name(flag: u32) -> Option<&'static str> {
        match flag {
            CRITICAL => Some("Critical"),
            LUCKY => Some("Lucky"),
            TWINCAST => Some("Twincast"),
            RAMPAGE => Some("Rampage"),
            ASSASSINATE => Some("Assassinate"),
            HEADSHOT => Some("Headshot"),
            FINISHING_BLOW => Some("FinishingBlow"),
            DOUBLE_BOW => Some("DoubleBow"),
            FLURRY => Some("Flurry"),
            STRIKETHROUGH => Some("Strikethrough"),
            RIPOSTE => Some("Riposte"),
            SLAY => Some("Slay"),
            _ => None,
        }
    }

    pub fn flags_from_mask(modifiers: u32) -> Vec<&'static str> {
        let mut flags = Vec::new();
        for bit in 0..12 {
            let flag = 1 << bit;
            if (modifiers & flag) != 0 {
                if let Some(name) = flag_name(flag) {
                    flags.push(name);
                }
            }
        }
        flags
    }
}

// ─── Database Initialization ──────────────────────────────────────────────────

pub struct SessionsDb {
    conn: Connection,
}

impl SessionsDb {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).context("Failed to open sessions database")?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .context("Failed to set WAL mode")?;

        let db = SessionsDb { conn };

        // Apply migrations instead of inline schema
        let runner = migrations::MigrationRunner::new(&db.conn)
            .context("Failed to create migration runner")?;
        runner.apply_pending()
            .context("Failed to apply pending migrations")?;
        runner.verify_schema()
            .context("Failed to verify schema after migrations")?;

        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                start_ts INTEGER NOT NULL,
                end_ts INTEGER,
                character TEXT NOT NULL,
                zone TEXT,
                server TEXT,
                client_version TEXT,
                tlp_ruleset TEXT,
                duration_secs INTEGER
            );

            CREATE TABLE IF NOT EXISTS combat_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                ts INTEGER NOT NULL,
                attacker TEXT NOT NULL,
                defender TEXT NOT NULL,
                attacker_owner TEXT,
                defender_owner TEXT,
                ability TEXT,
                dmg_type TEXT,
                damage INTEGER,
                outcome TEXT,
                modifiers INTEGER DEFAULT 0,
                resist TEXT,
                partial_resist_pct REAL,
                raw_line_offset INTEGER,
                hp_pct_attacker REAL,
                hp_pct_defender REAL,
                mana_pct_attacker REAL,
                target_distance REAL
            );

            CREATE TABLE IF NOT EXISTS heal_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                ts INTEGER NOT NULL,
                healer TEXT,
                healed TEXT,
                spell TEXT,
                type TEXT,
                amount INTEGER,
                overheal INTEGER,
                modifiers INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS spell_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                ts INTEGER NOT NULL,
                caster TEXT,
                target TEXT,
                spell TEXT,
                phase TEXT,
                interrupted_by TEXT,
                ambiguity_json TEXT
            );

            CREATE TABLE IF NOT EXISTS buff_uptime (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                target TEXT,
                spell TEXT,
                applied_ts INTEGER,
                faded_ts INTEGER,
                source TEXT
            );

            CREATE TABLE IF NOT EXISTS taunt_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                ts INTEGER NOT NULL,
                taunter TEXT,
                taunted TEXT,
                success INTEGER
            );

            CREATE TABLE IF NOT EXISTS mez_break_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                ts INTEGER NOT NULL,
                target TEXT,
                breaker TEXT,
                spell_broken TEXT
            );

            CREATE TABLE IF NOT EXISTS random_rolls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                ts INTEGER NOT NULL,
                item TEXT,
                roller TEXT,
                roll_value INTEGER,
                max_value INTEGER,
                winner TEXT
            );

            CREATE TABLE IF NOT EXISTS zone_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                ts INTEGER NOT NULL,
                zone TEXT,
                x REAL,
                y REAL,
                z REAL
            );

            CREATE TABLE IF NOT EXISTS xp_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                ts INTEGER NOT NULL,
                level_after INTEGER,
                aa_after INTEGER
            );

            CREATE TABLE IF NOT EXISTS faction_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                ts INTEGER NOT NULL,
                faction TEXT,
                delta INTEGER,
                zone TEXT
            );

            CREATE TABLE IF NOT EXISTS pulls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                ts_start INTEGER NOT NULL,
                ts_end INTEGER,
                mob_name TEXT,
                zone TEXT,
                difficulty TEXT,
                time_to_engage_ms INTEGER,
                success INTEGER,
                aggro_count INTEGER,
                unintended_adds INTEGER
            );

            CREATE TABLE IF NOT EXISTS deaths (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                ts INTEGER NOT NULL,
                character TEXT,
                zone TEXT,
                attacker TEXT,
                attacker_max_hit INTEGER,
                last_5_damage_json TEXT
            );

            CREATE TABLE IF NOT EXISTS rotation_ticks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                ts INTEGER NOT NULL,
                action TEXT,
                target TEXT,
                cooldown_remaining_ms INTEGER,
                reason_failed TEXT,
                hp_pct REAL,
                mana_pct REAL,
                target_distance REAL
            );

            CREATE TABLE IF NOT EXISTS loot (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id INTEGER NOT NULL REFERENCES sessions(id),
                ts INTEGER NOT NULL,
                item TEXT,
                looter TEXT,
                is_currency INTEGER,
                quantity INTEGER,
                source TEXT,
                zone TEXT
            );

            -- Indices for common queries
            CREATE INDEX IF NOT EXISTS idx_combat_session_defender_ts ON combat_events(session_id, defender, ts);
            CREATE INDEX IF NOT EXISTS idx_combat_session_attacker_ts ON combat_events(session_id, attacker, ts);
            CREATE INDEX IF NOT EXISTS idx_heal_session_healed_ts ON heal_events(session_id, healed, ts);
            CREATE INDEX IF NOT EXISTS idx_spell_session_caster_spell_ts ON spell_events(session_id, caster, spell, ts);
            CREATE INDEX IF NOT EXISTS idx_sessions_start_ts ON sessions(start_ts);
            "#
        ).context("Failed to create tables and indices")?;
        Ok(())
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_modifiers_critical_flag() {
        let mut m = 0u32;
        m = modifiers::set_flag(m, modifiers::CRITICAL);
        assert!(modifiers::has_flag(m, modifiers::CRITICAL));
        assert!(!modifiers::has_flag(m, modifiers::LUCKY));
    }

    #[test]
    fn test_modifiers_multiple_flags() {
        let mut m = 0u32;
        m = modifiers::set_flag(m, modifiers::CRITICAL);
        m = modifiers::set_flag(m, modifiers::LUCKY);
        m = modifiers::set_flag(m, modifiers::TWINCAST);

        assert!(modifiers::has_flag(m, modifiers::CRITICAL));
        assert!(modifiers::has_flag(m, modifiers::LUCKY));
        assert!(modifiers::has_flag(m, modifiers::TWINCAST));
        assert!(!modifiers::has_flag(m, modifiers::RAMPAGE));
    }

    #[test]
    fn test_modifiers_clear_flag() {
        let mut m = 0u32;
        m = modifiers::set_flag(m, modifiers::CRITICAL);
        m = modifiers::set_flag(m, modifiers::LUCKY);
        assert!(modifiers::has_flag(m, modifiers::CRITICAL));

        m = modifiers::clear_flag(m, modifiers::CRITICAL);
        assert!(!modifiers::has_flag(m, modifiers::CRITICAL));
        assert!(modifiers::has_flag(m, modifiers::LUCKY));
    }

    #[test]
    fn test_modifiers_all_12_bits() {
        let mut m = 0u32;
        m = modifiers::set_flag(m, modifiers::CRITICAL);
        m = modifiers::set_flag(m, modifiers::LUCKY);
        m = modifiers::set_flag(m, modifiers::TWINCAST);
        m = modifiers::set_flag(m, modifiers::RAMPAGE);
        m = modifiers::set_flag(m, modifiers::ASSASSINATE);
        m = modifiers::set_flag(m, modifiers::HEADSHOT);
        m = modifiers::set_flag(m, modifiers::FINISHING_BLOW);
        m = modifiers::set_flag(m, modifiers::DOUBLE_BOW);
        m = modifiers::set_flag(m, modifiers::FLURRY);
        m = modifiers::set_flag(m, modifiers::STRIKETHROUGH);
        m = modifiers::set_flag(m, modifiers::RIPOSTE);
        m = modifiers::set_flag(m, modifiers::SLAY);

        assert_eq!(m, modifiers::MASK_ALL);
    }

    #[test]
    fn test_modifiers_flag_name() {
        assert_eq!(modifiers::flag_name(modifiers::CRITICAL), Some("Critical"));
        assert_eq!(modifiers::flag_name(modifiers::LUCKY), Some("Lucky"));
        assert_eq!(modifiers::flag_name(0x1000), None);
    }

    #[test]
    fn test_modifiers_flags_from_mask() {
        let mut m = 0u32;
        m = modifiers::set_flag(m, modifiers::CRITICAL);
        m = modifiers::set_flag(m, modifiers::LUCKY);

        let flags = modifiers::flags_from_mask(m);
        assert!(flags.contains(&"Critical"));
        assert!(flags.contains(&"Lucky"));
        assert_eq!(flags.len(), 2);
    }

    #[test]
    fn test_database_creation() -> Result<()> {
        let temp = NamedTempFile::new()?;
        let db = SessionsDb::open(temp.path())?;

        // Verify tables exist
        let mut stmt = db
            .connection()
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        assert!(tables.contains(&"sessions".to_string()));
        assert!(tables.contains(&"combat_events".to_string()));
        assert!(tables.contains(&"heal_events".to_string()));
        assert!(tables.contains(&"spell_events".to_string()));
        assert!(tables.contains(&"buff_uptime".to_string()));
        assert!(tables.contains(&"taunt_events".to_string()));
        assert!(tables.contains(&"mez_break_events".to_string()));
        assert!(tables.contains(&"random_rolls".to_string()));
        assert!(tables.contains(&"zone_events".to_string()));
        assert!(tables.contains(&"xp_events".to_string()));
        assert!(tables.contains(&"faction_events".to_string()));
        assert!(tables.contains(&"pulls".to_string()));
        assert!(tables.contains(&"deaths".to_string()));
        assert!(tables.contains(&"rotation_ticks".to_string()));
        assert!(tables.contains(&"loot".to_string()));

        Ok(())
    }

    #[test]
    fn test_indices_created() -> Result<()> {
        let temp = NamedTempFile::new()?;
        let db = SessionsDb::open(temp.path())?;

        let mut stmt = db
            .connection()
            .prepare("SELECT name FROM sqlite_master WHERE type='index' ORDER BY name")?;
        let indices: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        assert!(
            indices
                .iter()
                .any(|i| i.contains("idx_combat_session_defender_ts"))
        );
        assert!(
            indices
                .iter()
                .any(|i| i.contains("idx_combat_session_attacker_ts"))
        );
        assert!(
            indices
                .iter()
                .any(|i| i.contains("idx_heal_session_healed_ts"))
        );
        assert!(
            indices
                .iter()
                .any(|i| i.contains("idx_spell_session_caster_spell_ts"))
        );

        Ok(())
    }
}
