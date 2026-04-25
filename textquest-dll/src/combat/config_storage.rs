/// Configuration storage — base64 sharing + SQLite DB-backed persistence.
///
/// Provides compact base64-encoded strings for easy config sharing via chat,
/// and SQLite-backed storage for per-character and per-server configurations.

use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json;
use std::path::Path;

use super::rotation::RotationGroup;

/// Base64-encode a rotation group for sharing.
///
/// Serializes RotationGroup to JSON, then base64-encodes for compact transport.
pub fn encode_rotation_group(group: &RotationGroup) -> Result<String> {
    let json = serde_json::to_string(group)?;
    Ok(general_purpose::STANDARD.encode(json))
}

/// Decode a base64-encoded rotation group.
pub fn decode_rotation_group(encoded: &str) -> Result<RotationGroup> {
    let json = general_purpose::STANDARD
        .decode(encoded)
        .context("Failed to decode base64")?;
    let json_str = String::from_utf8(json)?;
    serde_json::from_str(&json_str).context("Failed to deserialize rotation group")
}

/// SQLite-backed config store for per-character, per-server rotation configs.
pub struct SqliteConfigStore {
    conn: Connection,
}

impl SqliteConfigStore {
    /// Open or create config database at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .context("Failed to create config directory")?;
            }
        }

        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS rotation_configs (
                id INTEGER PRIMARY KEY,
                character_name TEXT NOT NULL,
                server_name TEXT NOT NULL,
                group_name TEXT NOT NULL,
                config_data TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(character_name, server_name, group_name)
            );
            CREATE INDEX IF NOT EXISTS idx_char_server ON rotation_configs(character_name, server_name);",
        )?;

        Ok(Self { conn })
    }

    /// Save rotation group to DB (overwrites if exists).
    pub fn save_rotation_group(
        &self,
        character_name: &str,
        server_name: &str,
        group: &RotationGroup,
    ) -> Result<()> {
        let encoded = encode_rotation_group(group)?;

        self.conn.execute(
            "INSERT OR REPLACE INTO rotation_configs (character_name, server_name, group_name, config_data, updated_at)
             VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)",
            params![character_name, server_name, &group.name, &encoded],
        )?;

        Ok(())
    }

    /// Load rotation group from DB.
    pub fn load_rotation_group(
        &self,
        character_name: &str,
        server_name: &str,
        group_name: &str,
    ) -> Result<Option<RotationGroup>> {
        let mut stmt = self.conn.prepare(
            "SELECT config_data FROM rotation_configs
             WHERE character_name = ? AND server_name = ? AND group_name = ?",
        )?;

        let config = stmt
            .query_row(params![character_name, server_name, group_name], |row| {
                Ok(row.get::<_, String>(0)?)
            })
            .optional()?;

        match config {
            Some(encoded) => Ok(Some(decode_rotation_group(&encoded)?)),
            None => Ok(None),
        }
    }

    /// Load all rotation groups for a character/server pair.
    pub fn load_all_rotation_groups(
        &self,
        character_name: &str,
        server_name: &str,
    ) -> Result<Vec<RotationGroup>> {
        let mut stmt = self.conn.prepare(
            "SELECT config_data FROM rotation_configs
             WHERE character_name = ? AND server_name = ?",
        )?;

        let groups = stmt
            .query_map(params![character_name, server_name], |row| {
                Ok(row.get::<_, String>(0)?)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut result = Vec::new();
        for encoded in groups {
            result.push(decode_rotation_group(&encoded)?);
        }

        Ok(result)
    }

    /// Delete rotation group from DB.
    pub fn delete_rotation_group(
        &self,
        character_name: &str,
        server_name: &str,
        group_name: &str,
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM rotation_configs
             WHERE character_name = ? AND server_name = ? AND group_name = ?",
            params![character_name, server_name, group_name],
        )?;

        Ok(())
    }

    /// Export config for sharing — returns base64-encoded JSON with metadata.
    pub fn export_config_string(
        &self,
        character_name: &str,
        server_name: &str,
        group_name: &str,
    ) -> Result<Option<String>> {
        match self.load_rotation_group(character_name, server_name, group_name)? {
            Some(group) => {
                let metadata = serde_json::json!({
                    "character": character_name,
                    "server": server_name,
                    "rotation": group.name,
                });
                let wrapped = serde_json::json!({
                    "metadata": metadata,
                    "data": serde_json::to_value(&group)?,
                });
                let json = serde_json::to_string(&wrapped)?;
                Ok(Some(general_purpose::STANDARD.encode(json)))
            }
            None => Ok(None),
        }
    }

    /// Import config from sharing string.
    pub fn import_config_string(
        &self,
        character_name: &str,
        server_name: &str,
        encoded: &str,
    ) -> Result<RotationGroup> {
        let json = general_purpose::STANDARD
            .decode(encoded)
            .context("Failed to decode base64")?;
        let json_str = String::from_utf8(json)?;
        let wrapped: serde_json::Value = serde_json::from_str(&json_str)?;

        let group: RotationGroup = serde_json::from_value(wrapped["data"].clone())?;
        self.save_rotation_group(character_name, server_name, &group)?;

        Ok(group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::combat::{CombatStateReq, TargetSelector};

    fn create_test_rotation() -> RotationGroup {
        RotationGroup {
            name: "TestRotation".to_string(),
            target_selector: TargetSelector::SelfOnly,
            combat_state_req: CombatStateReq::Downtime,
            steps_per_frame: 1,
            full_rotation: false,
            hp_threshold: None,
            entries: vec![],
            current_step: 0,
        }
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let rotation = create_test_rotation();
        let encoded = encode_rotation_group(&rotation).unwrap();
        let decoded = decode_rotation_group(&encoded).unwrap();

        assert_eq!(decoded.name, rotation.name);
        assert_eq!(decoded.entries.len(), rotation.entries.len());
    }

    #[test]
    fn test_sqlite_storage() {
        let db_path = Path::new(":memory:");
        let store = SqliteConfigStore::open(db_path).unwrap();
        let rotation = create_test_rotation();

        // Save
        store
            .save_rotation_group("Wizard", "Teek", &rotation)
            .unwrap();

        // Load
        let loaded = store
            .load_rotation_group("Wizard", "Teek", "TestRotation")
            .unwrap();
        assert!(loaded.is_some());

        // Delete
        store
            .delete_rotation_group("Wizard", "Teek", "TestRotation")
            .unwrap();
        let deleted = store
            .load_rotation_group("Wizard", "Teek", "TestRotation")
            .unwrap();
        assert!(deleted.is_none());
    }
}
