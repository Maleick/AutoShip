use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEntry {
    pub module: String,
    pub key: String,
    pub value: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct DbConfig {
    conn: Connection,
}

impl DbConfig {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open config DB: {}", path.display()))?;

        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS config (
                    id INTEGER PRIMARY KEY,
                    module TEXT NOT NULL,
                    key TEXT NOT NULL,
                    value TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    UNIQUE(module, key)
                );
                CREATE INDEX IF NOT EXISTS idx_module ON config(module);",
            )
            .context("Failed to initialize config schema")?;
        Ok(())
    }

    pub fn set(&self, module: &str, key: &str, value: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("Time error")?
            .as_secs() as i64;

        self.conn
            .execute(
                "INSERT INTO config (module, key, value, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(module, key) DO UPDATE SET
             value=excluded.value, updated_at=excluded.updated_at",
                params![module, key, value, now, now],
            )
            .context("Failed to set config entry")?;
        Ok(())
    }

    pub fn get(&self, module: &str, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM config WHERE module = ?1 AND key = ?2",
                params![module, key],
                |row| row.get(0),
            )
            .optional()
            .context("Failed to get config entry")
    }

    pub fn get_all(&self, module: &str) -> Result<Vec<ConfigEntry>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT module, key, value, created_at, updated_at FROM config WHERE module = ?1",
            )
            .context("Failed to prepare statement")?;

        let entries = stmt
            .query_map(params![module], |row| {
                Ok(ConfigEntry {
                    module: row.get(0)?,
                    key: row.get(1)?,
                    value: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .context("Failed to query config")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect config entries")?;

        Ok(entries)
    }

    pub fn delete(&self, module: &str, key: &str) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM config WHERE module = ?1 AND key = ?2",
                params![module, key],
            )
            .context("Failed to delete config entry")?;
        Ok(())
    }

    pub fn clear_module(&self, module: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM config WHERE module = ?1", params![module])
            .context("Failed to clear module config")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigShare {
    pub module: String,
    pub entries: Vec<(String, String)>,
}

impl ConfigShare {
    pub fn from_db(db: &DbConfig, module: &str) -> Result<Self> {
        let entries = db
            .get_all(module)?
            .into_iter()
            .map(|e| (e.key, e.value))
            .collect();

        Ok(Self {
            module: module.to_string(),
            entries,
        })
    }

    pub fn to_base64(&self) -> Result<String> {
        let json = serde_json::to_string(self).context("Failed to serialize config share")?;
        Ok(BASE64.encode(json))
    }

    pub fn from_base64(encoded: &str) -> Result<Self> {
        let json = String::from_utf8(BASE64.decode(encoded).context("Failed to decode base64")?)
            .context("Invalid UTF-8 in config share")?;
        serde_json::from_str(&json).context("Failed to deserialize config share")
    }

    pub fn apply_to_db(&self, db: &DbConfig) -> Result<()> {
        db.clear_module(&self.module)?;
        for (key, value) in &self.entries {
            db.set(&self.module, key, value)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::NamedTempFile;

    #[test]
    fn test_db_config_basic() -> Result<()> {
        let tmp = NamedTempFile::new()?;
        let db = DbConfig::open(tmp.path())?;

        db.set("combat", "pull_mode", "Normal")?;
        assert_eq!(db.get("combat", "pull_mode")?, Some("Normal".into()));

        db.set("combat", "pull_mode", "Chain")?;
        assert_eq!(db.get("combat", "pull_mode")?, Some("Chain".into()));

        Ok(())
    }

    #[test]
    fn test_config_share_round_trip() -> Result<()> {
        let share = ConfigShare {
            module: "combat".into(),
            entries: vec![
                ("pull_mode".into(), "Normal".into()),
                ("burn_rotation".into(), "enabled".into()),
            ],
        };

        let encoded = share.to_base64()?;
        let decoded = ConfigShare::from_base64(&encoded)?;

        assert_eq!(decoded.module, "combat");
        assert_eq!(decoded.entries.len(), 2);

        Ok(())
    }
}
