//! Named mob database — loads per-zone TOML files from config/named_mobs/.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Priority level for a named mob.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NamedPriority {
    High,
    Medium,
    Low,
}

/// A single named mob entry from a zone TOML file.
#[derive(Debug, Clone, Deserialize)]
pub struct NamedMobEntry {
    pub name: String,
    pub level: u8,
    pub respawn_min_minutes: u32,
    pub respawn_max_minutes: u32,
    pub location: [f32; 3],
    #[serde(default)]
    pub drops: Vec<String>,
    pub priority: NamedPriority,
}

impl NamedMobEntry {
    /// Convert `respawn_min_minutes` to ticks (at 250ms per tick = 4 ticks/sec).
    pub fn respawn_min_ticks(&self) -> u64 {
        self.respawn_min_minutes as u64 * 60 * 4
    }

    /// Convert `respawn_max_minutes` to ticks.
    pub fn respawn_max_ticks(&self) -> u64 {
        self.respawn_max_minutes as u64 * 60 * 4
    }
}

/// Per-zone TOML file structure.
#[derive(Debug, Deserialize)]
struct ZoneFile {
    zone: String,
    named: Vec<NamedMobEntry>,
}

/// Database of all named mobs across all zones.
#[derive(Debug, Clone)]
pub struct NamedMobDatabase {
    /// Keyed by (`zone_lowercase`, `name_lowercase`).
    entries: HashMap<(String, String), NamedMobEntry>,
    /// All entries for a given zone.
    by_zone: HashMap<String, Vec<NamedMobEntry>>,
}

impl NamedMobDatabase {
    /// Load all zone TOML files from a directory.
    pub fn load(dir: &Path) -> Result<Self> {
        let mut entries = HashMap::new();
        let mut by_zone: HashMap<String, Vec<NamedMobEntry>> = HashMap::new();

        if !dir.exists() {
            return Ok(Self { entries, by_zone });
        }

        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read named_mobs dir: {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toml") {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                let zone_file: ZoneFile = toml::from_str(&content)
                    .with_context(|| format!("Failed to parse {}", path.display()))?;

                let zone_key = zone_file.zone.to_ascii_lowercase();
                for mob in zone_file.named {
                    let name_key = mob.name.to_ascii_lowercase();
                    by_zone
                        .entry(zone_key.clone())
                        .or_default()
                        .push(mob.clone());
                    entries.insert((zone_key.clone(), name_key), mob);
                }
            }
        }

        Ok(Self { entries, by_zone })
    }

    /// Look up a named mob by zone and name (case-insensitive).
    pub fn get(&self, zone: &str, name: &str) -> Option<&NamedMobEntry> {
        self.entries
            .get(&(zone.to_ascii_lowercase(), name.to_ascii_lowercase()))
    }

    /// Get all named mobs for a zone.
    pub fn for_zone(&self, zone: &str) -> &[NamedMobEntry] {
        self.by_zone
            .get(&zone.to_ascii_lowercase())
            .map(std::vec::Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the database is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of zones loaded.
    pub fn zone_count(&self) -> usize {
        self.by_zone.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_test_toml(dir: &Path) {
        let crushbone = r#"
zone = "crushbone"

[[named]]
name = "Emperor Crush"
level = 15
respawn_min_minutes = 28
respawn_max_minutes = 32
location = [-688.0, 118.0, 28.0]
drops = ["Crushbone Belt"]
priority = "high"

[[named]]
name = "Ambassador Dvinn"
level = 13
respawn_min_minutes = 22
respawn_max_minutes = 28
location = [-450.0, 250.0, 50.0]
drops = ["Elven Chainmail"]
priority = "medium"
"#;
        let mut f = std::fs::File::create(dir.join("crushbone.toml")).unwrap();
        f.write_all(crushbone.as_bytes()).unwrap();
    }

    #[test]
    fn test_load_database() {
        let dir = tempfile::tempdir().unwrap();
        write_test_toml(dir.path());

        let db = NamedMobDatabase::load(dir.path()).unwrap();
        assert_eq!(db.len(), 2);
        assert_eq!(db.zone_count(), 1);
    }

    #[test]
    fn test_get_by_name() {
        let dir = tempfile::tempdir().unwrap();
        write_test_toml(dir.path());

        let db = NamedMobDatabase::load(dir.path()).unwrap();
        let crush = db.get("crushbone", "Emperor Crush").unwrap();
        assert_eq!(crush.level, 15);
        assert_eq!(crush.priority, NamedPriority::High);
        assert_eq!(crush.respawn_min_minutes, 28);
    }

    #[test]
    fn test_case_insensitive_lookup() {
        let dir = tempfile::tempdir().unwrap();
        write_test_toml(dir.path());

        let db = NamedMobDatabase::load(dir.path()).unwrap();
        assert!(db.get("CRUSHBONE", "emperor crush").is_some());
        assert!(db.get("Crushbone", "EMPEROR CRUSH").is_some());
    }

    #[test]
    fn test_for_zone() {
        let dir = tempfile::tempdir().unwrap();
        write_test_toml(dir.path());

        let db = NamedMobDatabase::load(dir.path()).unwrap();
        let mobs = db.for_zone("crushbone");
        assert_eq!(mobs.len(), 2);
    }

    #[test]
    fn test_missing_zone() {
        let dir = tempfile::tempdir().unwrap();
        write_test_toml(dir.path());

        let db = NamedMobDatabase::load(dir.path()).unwrap();
        assert!(db.get("fakezone", "Nobody").is_none());
        assert!(db.for_zone("fakezone").is_empty());
    }

    #[test]
    fn test_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let db = NamedMobDatabase::load(dir.path()).unwrap();
        assert!(db.is_empty());
    }

    #[test]
    fn test_nonexistent_dir() {
        let db = NamedMobDatabase::load(Path::new("/tmp/does_not_exist_named_mobs")).unwrap();
        assert!(db.is_empty());
    }

    #[test]
    fn test_respawn_ticks() {
        let dir = tempfile::tempdir().unwrap();
        write_test_toml(dir.path());

        let db = NamedMobDatabase::load(dir.path()).unwrap();
        let crush = db.get("crushbone", "Emperor Crush").unwrap();
        // 28 min * 60 sec * 4 ticks/sec = 6720
        assert_eq!(crush.respawn_min_ticks(), 6720);
        // 32 min * 60 sec * 4 ticks/sec = 7680
        assert_eq!(crush.respawn_max_ticks(), 7680);
    }
}
