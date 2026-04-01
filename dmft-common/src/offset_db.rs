use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Hot-updatable offset database backed by JSON.
///
/// Allows updating EQ memory offsets without recompiling by loading a JSON file
/// at runtime. Falls back to compile-time constants from `offsets.rs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OffsetDatabase {
    /// EQ client build date (e.g. "20260310").
    pub client_date: String,
    /// Preferred base address of eqgame.exe.
    pub eq_preferred_base: u64,
    /// Global pointer addresses keyed by name (e.g. "pinstLocalPlayer").
    pub globals: HashMap<String, u64>,
    /// PlayerBase struct field offsets keyed by name.
    pub player_base: HashMap<String, usize>,
    /// PlayerZoneClient struct field offsets keyed by name.
    pub player_zone: HashMap<String, usize>,
    /// SpawnManager struct offsets keyed by name.
    pub spawn_manager: HashMap<String, usize>,
}

impl OffsetDatabase {
    /// Load an offset database from a JSON file on disk.
    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let db: Self = serde_json::from_str(&content)?;
        Ok(db)
    }

    /// Serialize and write this database to a JSON file.
    pub fn save_to_file(&self, path: &Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Look up a global pointer address by name.
    pub fn get_global(&self, name: &str) -> Option<u64> {
        self.globals.get(name).copied()
    }

    /// Look up a PlayerBase field offset by name.
    pub fn get_player_base_offset(&self, name: &str) -> Option<usize> {
        self.player_base.get(name).copied()
    }

    /// Look up a PlayerZoneClient field offset by name.
    pub fn get_player_zone_offset(&self, name: &str) -> Option<usize> {
        self.player_zone.get(name).copied()
    }

    /// Convert a preferred-base address to a runtime address using this database's preferred base.
    pub fn rebase(&self, preferred_addr: u64, actual_base: u64) -> Option<usize> {
        let offset = preferred_addr.checked_sub(self.eq_preferred_base)?;
        Some((actual_base + offset) as usize)
    }

    /// Create from the current compile-time constants in offsets.rs
    pub fn from_compiled_offsets() -> Self {
        use crate::offsets::{PINST_LOCAL_PLAYER, PINST_CONTROLLED_PLAYER, PINST_TARGET, PINST_SPAWN_MANAGER, PINST_LOCAL_PC, PINST_SPELL_MANAGER, PINST_CDISPLAY, PINST_CEVERQUEST, player_base, player_zone, spawn_manager, EQ_PREFERRED_BASE};
        let mut globals = HashMap::new();
        globals.insert("pinstLocalPlayer".to_string(), PINST_LOCAL_PLAYER);
        globals.insert("pinstControlledPlayer".to_string(), PINST_CONTROLLED_PLAYER);
        globals.insert("pinstTarget".to_string(), PINST_TARGET);
        globals.insert("pinstSpawnManager".to_string(), PINST_SPAWN_MANAGER);
        globals.insert("pinstLocalPC".to_string(), PINST_LOCAL_PC);
        globals.insert("pinstSpellManager".to_string(), PINST_SPELL_MANAGER);
        globals.insert("pinstCDisplay".to_string(), PINST_CDISPLAY);
        globals.insert("pinstCEverQuest".to_string(), PINST_CEVERQUEST);

        let mut pb = HashMap::new();
        pb.insert("next".to_string(), player_base::NEXT);
        pb.insert("prev".to_string(), player_base::PREV);
        pb.insert("y".to_string(), player_base::Y);
        pb.insert("x".to_string(), player_base::X);
        pb.insert("z".to_string(), player_base::Z);
        pb.insert("heading".to_string(), player_base::HEADING);
        pb.insert("speedCurrent".to_string(), player_base::SPEED_CURRENT);
        pb.insert("speedRun".to_string(), player_base::SPEED_RUN);
        pb.insert("speedHeading".to_string(), player_base::SPEED_HEADING);
        pb.insert("name".to_string(), player_base::NAME);
        pb.insert("displayedName".to_string(), player_base::DISPLAYED_NAME);
        pb.insert("type".to_string(), player_base::TYPE);
        pb.insert("spawnId".to_string(), player_base::SPAWN_ID);
        pb.insert("lastName".to_string(), player_base::LASTNAME);

        let mut pz = HashMap::new();
        pz.insert("hpMax".to_string(), player_zone::HP_MAX);
        pz.insert("hpCurrent".to_string(), player_zone::HP_CURRENT);
        pz.insert("manaMax".to_string(), player_zone::MANA_MAX);
        pz.insert("manaCurrent".to_string(), player_zone::MANA_CURRENT);
        pz.insert("level".to_string(), player_zone::LEVEL);
        pz.insert("charClass".to_string(), player_zone::CHAR_CLASS);
        pz.insert(
            "enduranceCurrent".to_string(),
            player_zone::ENDURANCE_CURRENT,
        );
        pz.insert("enduranceMax".to_string(), player_zone::ENDURANCE_MAX);
        pz.insert("standState".to_string(), player_zone::STANDSTATE);

        let mut sm = HashMap::new();
        sm.insert("playerList".to_string(), spawn_manager::PLAYER_LIST);

        Self {
            client_date: "20260310".to_string(),
            eq_preferred_base: EQ_PREFERRED_BASE,
            globals,
            player_base: pb,
            player_zone: pz,
            spawn_manager: sm,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_compiled_offsets_has_expected_globals() {
        let db = OffsetDatabase::from_compiled_offsets();
        assert!(db.get_global("pinstLocalPlayer").is_some());
        assert!(db.get_global("pinstTarget").is_some());
        assert!(db.get_global("nonexistent").is_none());
    }

    #[test]
    fn rebase_normal_case() {
        let db = OffsetDatabase::from_compiled_offsets();
        let actual_base: u64 = 0x7FF600000000;
        let addr = db.get_global("pinstLocalPlayer").unwrap();
        let result = db.rebase(addr, actual_base);
        let expected_offset = addr - db.eq_preferred_base;
        assert_eq!(result, Some((actual_base + expected_offset) as usize));
    }

    #[test]
    fn rebase_underflow_returns_none() {
        let db = OffsetDatabase::from_compiled_offsets();
        let result = db.rebase(0x100, 0x7FF600000000);
        assert_eq!(result, None);
    }

    #[test]
    fn json_serialization_roundtrip() {
        let db = OffsetDatabase::from_compiled_offsets();
        let json = serde_json::to_string(&db).expect("serialize failed");
        let restored: OffsetDatabase = serde_json::from_str(&json).expect("deserialize failed");

        assert_eq!(restored.client_date, db.client_date);
        assert_eq!(restored.eq_preferred_base, db.eq_preferred_base);
        assert_eq!(
            restored.get_global("pinstLocalPlayer"),
            db.get_global("pinstLocalPlayer")
        );
        assert_eq!(
            restored.get_player_base_offset("x"),
            db.get_player_base_offset("x")
        );
        assert_eq!(
            restored.get_player_zone_offset("hpMax"),
            db.get_player_zone_offset("hpMax")
        );
    }

    #[test]
    fn field_offset_lookups() {
        let db = OffsetDatabase::from_compiled_offsets();
        assert_eq!(
            db.get_player_base_offset("next"),
            Some(crate::offsets::player_base::NEXT)
        );
        assert_eq!(
            db.get_player_zone_offset("level"),
            Some(crate::offsets::player_zone::LEVEL)
        );
        assert!(db.get_player_base_offset("nonexistent").is_none());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let db = OffsetDatabase::from_compiled_offsets();
        let dir = std::env::temp_dir().join("dmft_test_offset_db");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_offsets.json");

        db.save_to_file(&path).expect("save failed");
        let loaded = OffsetDatabase::load_from_file(&path).expect("load failed");

        assert_eq!(loaded, db);

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_nonexistent_file_returns_error() {
        // Construct a path in the system temp directory that should not exist.
        let mut path = std::env::temp_dir();
        path.push(format!(
            "dmft_nonexistent_{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        // Ensure the file does not exist at this path.
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let result = OffsetDatabase::load_from_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn load_from_invalid_json_returns_error() {
        let dir = std::env::temp_dir().join("dmft_test_invalid_json");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.json");
        std::fs::write(&path, "not valid json{{{").unwrap();

        let result = OffsetDatabase::load_from_file(&path);
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn get_global_returns_none_for_missing_key() {
        let db = OffsetDatabase::from_compiled_offsets();
        assert!(db.get_global("does_not_exist").is_none());
    }

    #[test]
    fn get_player_zone_offset_returns_none_for_missing_key() {
        let db = OffsetDatabase::from_compiled_offsets();
        assert!(db.get_player_zone_offset("does_not_exist").is_none());
    }

    #[test]
    fn rebase_with_zero_actual_base() {
        let db = OffsetDatabase::from_compiled_offsets();
        let addr = db.get_global("pinstLocalPlayer").unwrap();
        let result = db.rebase(addr, 0);
        let expected_offset = addr - db.eq_preferred_base;
        assert_eq!(result, Some(expected_offset as usize));
    }

    #[test]
    fn rebase_preferred_base_itself_returns_actual_base() {
        let db = OffsetDatabase::from_compiled_offsets();
        let actual_base: u64 = 0x7FF600000000;
        let result = db.rebase(db.eq_preferred_base, actual_base);
        assert_eq!(result, Some(actual_base as usize));
    }

    #[test]
    fn from_compiled_offsets_has_all_expected_globals() {
        let db = OffsetDatabase::from_compiled_offsets();
        let expected_globals = [
            "pinstLocalPlayer",
            "pinstControlledPlayer",
            "pinstTarget",
            "pinstSpawnManager",
            "pinstLocalPC",
            "pinstSpellManager",
            "pinstCDisplay",
            "pinstCEverQuest",
        ];
        for key in &expected_globals {
            assert!(db.get_global(key).is_some(), "missing global: {}", key);
        }
        assert_eq!(db.globals.len(), expected_globals.len());
    }

    #[test]
    fn from_compiled_offsets_has_all_player_base_keys() {
        let db = OffsetDatabase::from_compiled_offsets();
        let expected = [
            "next",
            "prev",
            "y",
            "x",
            "z",
            "heading",
            "speedCurrent",
            "speedRun",
            "speedHeading",
            "name",
            "displayedName",
            "type",
            "spawnId",
            "lastName",
        ];
        for key in &expected {
            assert!(
                db.get_player_base_offset(key).is_some(),
                "missing player_base: {}",
                key
            );
        }
        assert_eq!(db.player_base.len(), expected.len());
    }

    #[test]
    fn from_compiled_offsets_has_all_player_zone_keys() {
        let db = OffsetDatabase::from_compiled_offsets();
        let expected = [
            "hpMax",
            "hpCurrent",
            "manaMax",
            "manaCurrent",
            "level",
            "charClass",
            "enduranceCurrent",
            "enduranceMax",
            "standState",
        ];
        for key in &expected {
            assert!(
                db.get_player_zone_offset(key).is_some(),
                "missing player_zone: {}",
                key
            );
        }
        assert_eq!(db.player_zone.len(), expected.len());
    }
}
