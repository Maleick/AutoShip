//! Camp progression engine — auto-advances camps based on average group level.

use crate::camp::config::CampConfig;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Event emitted when the group should move to a new camp.
#[derive(Debug, Clone, PartialEq)]
pub enum CampProgressionEvent {
    /// Group has outleveled the current camp — advance to next.
    AdvanceToNext {
        from_camp: String,
        to_camp: String,
        avg_level: f32,
    },
    /// Group is underleveled for current camp — fall back to previous.
    FallbackToPrev {
        from_camp: String,
        to_camp: String,
        avg_level: f32,
    },
    /// No next/prev camp configured — end of progression chain.
    EndOfChain { camp: String, avg_level: f32 },
}

/// Database of all known camp configurations, loaded from `config/camps/`.
pub struct CampDatabase {
    /// Camp configs keyed by their file name (without `.toml`).
    camps: HashMap<String, CampConfig>,
    /// Ordered list of camp names by `level_range`[0] (ascending).
    by_level: Vec<String>,
}

impl CampDatabase {
    /// Load all `.toml` files from `config/camps/` into memory.
    pub fn load() -> Result<Self> {
        Self::load_from(Path::new("config/camps"))
    }

    /// Load from a specific directory (for testing).
    pub fn load_from(dir: &Path) -> Result<Self> {
        let mut camps = HashMap::new();

        if !dir.exists() {
            return Ok(Self {
                camps,
                by_level: Vec::new(),
            });
        }

        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read camps directory: {}", dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                let contents = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                let config: CampConfig = toml::from_str(&contents)
                    .with_context(|| format!("Failed to parse {}", path.display()))?;
                let key = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                camps.insert(key, config);
            }
        }

        let mut by_level: Vec<String> = camps.keys().cloned().collect();
        by_level.sort_by_key(|name| camps[name].level_range[0]);

        Ok(Self { camps, by_level })
    }

    /// Get a camp config by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&CampConfig> {
        self.camps.get(name)
    }

    /// List all camp names, sorted by minimum level.
    #[must_use]
    pub fn list_by_level(&self) -> &[String] {
        &self.by_level
    }

    /// Number of loaded camps.
    #[must_use]
    pub fn len(&self) -> usize {
        self.camps.len()
    }

    /// Whether the database is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.camps.is_empty()
    }

    /// Find the best camp for a given average level.
    #[must_use]
    pub fn best_camp_for_level(&self, avg_level: f32) -> Option<&CampConfig> {
        let level = avg_level as u8;
        // Find the highest-min-level camp whose range contains the level.
        self.by_level
            .iter()
            .rev()
            .filter_map(|name| self.camps.get(name))
            .find(|c| level >= c.level_range[0] && level <= c.level_range[1])
    }

    /// Get the next camp in the progression chain from a given camp.
    #[must_use]
    pub fn next_camp(&self, current: &str) -> Option<&CampConfig> {
        let config = self.camps.get(current)?;
        let next_name = config.next_camp.as_deref()?;
        self.camps.get(next_name)
    }

    /// Get the previous camp in the progression chain from a given camp.
    #[must_use]
    pub fn prev_camp(&self, current: &str) -> Option<&CampConfig> {
        let config = self.camps.get(current)?;
        let prev_name = config.prev_camp.as_deref()?;
        self.camps.get(prev_name)
    }
}

/// Checks whether a camp progression event should fire based on average group level.
#[must_use]
pub fn check_progression(
    current_camp: &CampConfig,
    avg_level: f32,
    db: &CampDatabase,
) -> Option<CampProgressionEvent> {
    let level = avg_level as u8;

    // Outleveled — advance
    if level > current_camp.level_range[1] {
        return match &current_camp.next_camp {
            Some(next) if db.get(next).is_some() => Some(CampProgressionEvent::AdvanceToNext {
                from_camp: current_camp.name.clone(),
                to_camp: next.clone(),
                avg_level,
            }),
            _ => Some(CampProgressionEvent::EndOfChain {
                camp: current_camp.name.clone(),
                avg_level,
            }),
        };
    }

    // Underleveled — fall back
    if level < current_camp.level_range[0] {
        return match &current_camp.prev_camp {
            Some(prev) if db.get(prev).is_some() => Some(CampProgressionEvent::FallbackToPrev {
                from_camp: current_camp.name.clone(),
                to_camp: prev.clone(),
                avg_level,
            }),
            _ => Some(CampProgressionEvent::EndOfChain {
                camp: current_camp.name.clone(),
                avg_level,
            }),
        };
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(
        name: &str,
        zone: &str,
        min: u8,
        max: u8,
        next: Option<&str>,
        prev: Option<&str>,
    ) -> CampConfig {
        CampConfig {
            name: name.into(),
            zone: zone.into(),
            camp_center: [0.0, 0.0, 0.0],
            pull_point: [10.0, 10.0, 0.0],
            pull_radius: 200.0,
            camp_radius: 30.0,
            leash_radius: 100.0,
            rest_mana_pct: 60,
            pull_mana_pct: 30,
            level_range: [min, max],
            pull_mob_names: Vec::new(),
            ignore_mob_names: Vec::new(),
            burn_mob_names: Vec::new(),
            next_camp: next.map(String::from),
            prev_camp: prev.map(String::from),
        }
    }

    fn write_camp_toml(dir: &Path, name: &str, config: &CampConfig) {
        let path = dir.join(format!("{name}.toml"));
        let toml_str = toml::to_string_pretty(config).unwrap();
        std::fs::write(path, toml_str).unwrap();
    }

    fn test_db() -> (tempfile::TempDir, CampDatabase) {
        let dir = tempfile::tempdir().unwrap();
        let camps_dir = dir.path().to_path_buf();

        let c1 = make_config(
            "crescent_reach",
            "crescent",
            1,
            10,
            Some("crushbone_entrance"),
            None,
        );
        let c2 = make_config(
            "crushbone_entrance",
            "crushbone",
            5,
            15,
            Some("unrest_yard"),
            Some("crescent_reach"),
        );
        let c3 = make_config(
            "unrest_yard",
            "unrest",
            15,
            25,
            None,
            Some("crushbone_entrance"),
        );

        write_camp_toml(&camps_dir, "crescent_reach", &c1);
        write_camp_toml(&camps_dir, "crushbone_entrance", &c2);
        write_camp_toml(&camps_dir, "unrest_yard", &c3);

        let db = CampDatabase::load_from(&camps_dir).unwrap();
        (dir, db)
    }

    #[test]
    fn test_load_camp_database() {
        let (_dir, db) = test_db();
        assert_eq!(db.len(), 3);
        assert!(!db.is_empty());
        assert!(db.get("crushbone_entrance").is_some());
        assert!(db.get("nonexistent").is_none());
    }

    #[test]
    fn test_list_by_level_sorted() {
        let (_dir, db) = test_db();
        let names = db.list_by_level();
        assert_eq!(names[0], "crescent_reach"); // min=1
        assert_eq!(names[1], "crushbone_entrance"); // min=5
        assert_eq!(names[2], "unrest_yard"); // min=15
    }

    #[test]
    fn test_best_camp_for_level() {
        let (_dir, db) = test_db();

        // Level 3 → crescent_reach (1-10)
        let camp = db.best_camp_for_level(3.0).unwrap();
        assert_eq!(camp.name, "crescent_reach");

        // Level 8 → crushbone_entrance (5-15, higher min wins in overlap)
        let camp = db.best_camp_for_level(8.0).unwrap();
        assert_eq!(camp.name, "crushbone_entrance");

        // Level 20 → unrest_yard (15-25)
        let camp = db.best_camp_for_level(20.0).unwrap();
        assert_eq!(camp.name, "unrest_yard");

        // Level 50 → nothing in range
        assert!(db.best_camp_for_level(50.0).is_none());
    }

    #[test]
    fn test_next_and_prev_camp() {
        let (_dir, db) = test_db();
        let next = db.next_camp("crushbone_entrance").unwrap();
        assert_eq!(next.name, "unrest_yard");

        let prev = db.prev_camp("crushbone_entrance").unwrap();
        assert_eq!(prev.name, "crescent_reach");

        // End of chain
        assert!(db.next_camp("unrest_yard").is_none());
        assert!(db.prev_camp("crescent_reach").is_none());
    }

    #[test]
    fn test_check_progression_advance() {
        let (_dir, db) = test_db();
        let camp = db.get("crushbone_entrance").unwrap();

        // Level 16 > max 15 → advance
        let event = check_progression(camp, 16.0, &db).unwrap();
        assert_eq!(
            event,
            CampProgressionEvent::AdvanceToNext {
                from_camp: "crushbone_entrance".into(),
                to_camp: "unrest_yard".into(),
                avg_level: 16.0,
            }
        );
    }

    #[test]
    fn test_check_progression_fallback() {
        let (_dir, db) = test_db();
        let camp = db.get("crushbone_entrance").unwrap();

        // Level 4 < min 5 → fall back
        let event = check_progression(camp, 4.0, &db).unwrap();
        assert_eq!(
            event,
            CampProgressionEvent::FallbackToPrev {
                from_camp: "crushbone_entrance".into(),
                to_camp: "crescent_reach".into(),
                avg_level: 4.0,
            }
        );
    }

    #[test]
    fn test_check_progression_in_range() {
        let (_dir, db) = test_db();
        let camp = db.get("crushbone_entrance").unwrap();

        // Level 10 is in range (5-15) → no event
        assert!(check_progression(camp, 10.0, &db).is_none());
    }

    #[test]
    fn test_check_progression_end_of_chain() {
        let (_dir, db) = test_db();
        let camp = db.get("unrest_yard").unwrap();

        // Level 26 > max 25, no next_camp → end of chain
        let event = check_progression(camp, 26.0, &db).unwrap();
        assert_eq!(
            event,
            CampProgressionEvent::EndOfChain {
                camp: "unrest_yard".into(),
                avg_level: 26.0,
            }
        );
    }

    #[test]
    fn test_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let db = CampDatabase::load_from(dir.path()).unwrap();
        assert!(db.is_empty());
        assert_eq!(db.len(), 0);
        assert!(db.best_camp_for_level(10.0).is_none());
    }

    #[test]
    fn test_nonexistent_directory() {
        let db = CampDatabase::load_from(Path::new("/tmp/nonexistent_camps_dir_xyz")).unwrap();
        assert!(db.is_empty());
    }
}
