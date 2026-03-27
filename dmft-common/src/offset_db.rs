use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffsetDatabase {
    pub client_date: String,
    pub eq_preferred_base: u64,
    pub globals: HashMap<String, u64>,
    pub player_base: HashMap<String, usize>,
    pub player_zone: HashMap<String, usize>,
    pub spawn_manager: HashMap<String, usize>,
}

impl OffsetDatabase {
    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let db: Self = serde_json::from_str(&content)?;
        Ok(db)
    }

    pub fn save_to_file(&self, path: &Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn get_global(&self, name: &str) -> Option<u64> {
        self.globals.get(name).copied()
    }

    pub fn get_player_base_offset(&self, name: &str) -> Option<usize> {
        self.player_base.get(name).copied()
    }

    pub fn get_player_zone_offset(&self, name: &str) -> Option<usize> {
        self.player_zone.get(name).copied()
    }

    pub fn rebase(&self, preferred_addr: u64, actual_base: u64) -> Option<usize> {
        let offset = preferred_addr.checked_sub(self.eq_preferred_base)?;
        Some((actual_base + offset) as usize)
    }

    /// Create from the current compile-time constants in offsets.rs
    pub fn from_compiled_offsets() -> Self {
        use crate::offsets::*;
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
        pb.insert("standState".to_string(), player_base::STANDSTATE);
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
        pz.insert("enduranceCurrent".to_string(), player_zone::ENDURANCE_CURRENT);
        pz.insert("enduranceMax".to_string(), player_zone::ENDURANCE_MAX);

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
