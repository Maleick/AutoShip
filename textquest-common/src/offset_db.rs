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
    /// `CContextMenuManager` struct field offsets keyed by name.
    #[serde(default)]
    pub context_menu_manager: HashMap<String, usize>,
    /// `CContextMenu` struct field offsets keyed by name.
    #[serde(default)]
    pub context_menu: HashMap<String, usize>,
    /// Internal function addresses keyed by name (e.g. "castSpell").
    #[serde(default)]
    pub functions: HashMap<String, u64>,
}

impl OffsetDatabase {
    /// Load an offset database from a JSON file on disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub fn load_from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let db: Self = serde_json::from_str(&content)?;
        Ok(db)
    }
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    /// Serialize and write this database to a JSON file.
    pub fn save_to_file(&self, path: &Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Look up a global pointer address by name.
    #[must_use]
    pub fn get_global(&self, name: &str) -> Option<u64> {
        self.globals.get(name).copied()
    }

    /// Look up a PlayerBase field offset by name.
    #[must_use]
    pub fn get_player_base_offset(&self, name: &str) -> Option<usize> {
        self.player_base.get(name).copied()
    }

    /// Look up a PlayerZoneClient field offset by name.
    #[must_use]
    pub fn get_player_zone_offset(&self, name: &str) -> Option<usize> {
        self.player_zone.get(name).copied()
    }

    /// Look up a function address by name.
    #[must_use]
    pub fn get_function(&self, name: &str) -> Option<u64> {
        self.functions.get(name).copied()
    }

    /// Look up a `CContextMenuManager` field offset by name.
    #[must_use]
    pub fn get_context_menu_manager_offset(&self, name: &str) -> Option<usize> {
        self.context_menu_manager.get(name).copied()
    }

    /// Look up a `CContextMenu` field offset by name.
    #[must_use]
    pub fn get_context_menu_offset(&self, name: &str) -> Option<usize> {
        self.context_menu.get(name).copied()
    }

    /// Convert a preferred-base address to a runtime address using this database's preferred base.
    #[must_use]
    pub fn rebase(&self, preferred_addr: u64, actual_base: u64) -> Option<usize> {
        let offset = preferred_addr.checked_sub(self.eq_preferred_base)?;
        Some((actual_base + offset) as usize)
    }

    /// Create from the current compile-time constants in offsets.rs
    #[must_use]
    pub fn from_compiled_offsets() -> Self {
        use crate::offsets::{
            CAN_USE_ITEM, CAST_SPELL, CCHAT_MGR_CREATE_CHAT_WINDOW, CCHAT_MGR_FREE_CHAT_WINDOW,
            CCHAT_MGR_GET_RGBA, CCHAT_MGR_INIT_CONTEXT_MENU, CCHAT_MGR_SET_LOCKED_ACTIVE_CHAT,
            CHANGE_HEIGHT, CHAR_LIST_ENTER_WORLD, CHAR_LIST_SELECT_CHAR, CLICKED_PLAYER,
            CONTEXT_MENU_MGR_HANDLE_MENU, DO_ATTACK, DO_COMBAT_ABILITY, DO_LOOT, EQ_PREFERRED_BASE,
            EXECUTE_CMD, FILE_INTEGRITY_DISPATCHER, FIX_HEADING, FREE_TARGET_CAST_SPELL,
            GET_BEARING, GET_CON_LEVEL, GET_PC_CLIENT, INBOUND_MSG_COUNTER, INTERPRET_CMD,
            INV_SLOT_MGR_FIND_SLOT, INV_SLOT_MGR_MOVE_ITEM, INV_SLOT_MGR_SELECT_SLOT,
            ISSUE_PET_COMMAND, NET_SEND, OUTBOUND_MSG_COUNTER, PINST_CDISPLAY, PINST_CEVERQUEST,
            PINST_CONTEXT_MENU_MANAGER, PINST_CONTROLLED_PLAYER, PINST_LOCAL_PC,
            PINST_LOCAL_PLAYER, PINST_SPAWN_MANAGER, PINST_SPELL_MANAGER, PINST_TARGET,
            PROCESS_GAME_EVENTS, REAL_RENDER_WORLD, SERVER_MEMCHECK_HANDLER,
            SPELL_BOOK_WND_MEMORIZE_SET, SYSTEM_FINGERPRINT, USE_SKILL, WORLD_AUTHENTICATE,
            ZONE_GUIDE_MANAGER, context_menu_mgr, player_base, player_zone, spawn_manager,
        };
        let mut globals = HashMap::new();
        globals.insert("pinstLocalPlayer".to_string(), PINST_LOCAL_PLAYER);
        globals.insert("pinstControlledPlayer".to_string(), PINST_CONTROLLED_PLAYER);
        globals.insert("pinstTarget".to_string(), PINST_TARGET);
        globals.insert("pinstSpawnManager".to_string(), PINST_SPAWN_MANAGER);
        globals.insert("pinstLocalPC".to_string(), PINST_LOCAL_PC);
        globals.insert("pinstSpellManager".to_string(), PINST_SPELL_MANAGER);
        globals.insert("pinstCDisplay".to_string(), PINST_CDISPLAY);
        globals.insert("pinstCEverQuest".to_string(), PINST_CEVERQUEST);
        globals.insert(
            "pinstCContextMenuManager".to_string(),
            PINST_CONTEXT_MENU_MANAGER,
        );

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

        let mut cmm = HashMap::new();
        cmm.insert("menusArray".to_string(), context_menu_mgr::MENUS_DATA);
        cmm.insert("numMenus".to_string(), context_menu_mgr::MENUS_COUNT);
        cmm.insert("currMenu".to_string(), context_menu_mgr::CUR_MENU);
        cmm.insert("curItem".to_string(), context_menu_mgr::CUR_ITEM);

        // No compile-time `offsets::context_menu` fallback is currently defined.
        // Leave the map empty here; JSON-loaded offsets can still populate it.
        let cm = HashMap::new();

        let mut funcs = HashMap::new();
        funcs.insert("castSpell".into(), CAST_SPELL);
        funcs.insert("doCombatAbility".into(), DO_COMBAT_ABILITY);
        funcs.insert("useSkill".into(), USE_SKILL);
        funcs.insert("canUseItem".into(), CAN_USE_ITEM);
        funcs.insert("doAttack".into(), DO_ATTACK);
        funcs.insert("executeCmd".into(), EXECUTE_CMD);
        funcs.insert("interpretCmd".into(), INTERPRET_CMD);
        funcs.insert("clickedPlayer".into(), CLICKED_PLAYER);
        funcs.insert("issuePetCommand".into(), ISSUE_PET_COMMAND);
        funcs.insert("getConLevel".into(), GET_CON_LEVEL);
        funcs.insert("getPcClient".into(), GET_PC_CLIENT);
        funcs.insert("doLoot".into(), DO_LOOT);
        funcs.insert("processGameEvents".into(), PROCESS_GAME_EVENTS);
        funcs.insert("realRenderWorld".into(), REAL_RENDER_WORLD);
        funcs.insert("fixHeading".into(), FIX_HEADING);
        funcs.insert("getBearing".into(), GET_BEARING);
        funcs.insert("freeTargetCastSpell".into(), FREE_TARGET_CAST_SPELL);
        funcs.insert("changeHeight".into(), CHANGE_HEIGHT);
        funcs.insert("zoneGuideManager".into(), ZONE_GUIDE_MANAGER);
        funcs.insert("charListEnterWorld".into(), CHAR_LIST_ENTER_WORLD);
        funcs.insert("charListSelectChar".into(), CHAR_LIST_SELECT_CHAR);
        funcs.insert("cchatMgrGetRgba".into(), CCHAT_MGR_GET_RGBA);
        funcs.insert(
            "cchatMgrInitContextMenu".into(),
            CCHAT_MGR_INIT_CONTEXT_MENU,
        );
        funcs.insert("cchatMgrFreeChatWindow".into(), CCHAT_MGR_FREE_CHAT_WINDOW);
        funcs.insert(
            "cchatMgrSetLockedActiveChat".into(),
            CCHAT_MGR_SET_LOCKED_ACTIVE_CHAT,
        );
        funcs.insert(
            "cchatMgrCreateChatWindow".into(),
            CCHAT_MGR_CREATE_CHAT_WINDOW,
        );
        funcs.insert("invSlotMgrFindSlot".into(), INV_SLOT_MGR_FIND_SLOT);
        funcs.insert("invSlotMgrMoveItem".into(), INV_SLOT_MGR_MOVE_ITEM);
        funcs.insert("invSlotMgrSelectSlot".into(), INV_SLOT_MGR_SELECT_SLOT);
        funcs.insert(
            "spellBookWndMemorizeSet".into(),
            SPELL_BOOK_WND_MEMORIZE_SET,
        );
        funcs.insert("netSend".into(), NET_SEND);
        funcs.insert("outboundMsgCounter".into(), OUTBOUND_MSG_COUNTER);
        funcs.insert("inboundMsgCounter".into(), INBOUND_MSG_COUNTER);
        funcs.insert("fileIntegrityDispatcher".into(), FILE_INTEGRITY_DISPATCHER);
        funcs.insert("serverMemcheckHandler".into(), SERVER_MEMCHECK_HANDLER);
        funcs.insert("worldAuthenticate".into(), WORLD_AUTHENTICATE);
        funcs.insert("systemFingerprint".into(), SYSTEM_FINGERPRINT);
        funcs.insert(
            "contextMenuMgrHandleMenu".into(),
            CONTEXT_MENU_MGR_HANDLE_MENU,
        );

        Self {
            client_date: "20260310".to_string(),
            eq_preferred_base: EQ_PREFERRED_BASE,
            globals,
            player_base: pb,
            player_zone: pz,
            spawn_manager: sm,
            context_menu_manager: cmm,
            context_menu: cm,
            functions: funcs,
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
        assert_eq!(
            restored.get_function("castSpell"),
            db.get_function("castSpell")
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
        let dir = std::env::temp_dir().join("textquest_test_offset_db");
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
            "textquest_nonexistent_{}.json",
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
        let dir = std::env::temp_dir().join("textquest_test_invalid_json");
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
            "pinstCContextMenuManager",
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

    #[test]
    fn from_compiled_offsets_has_all_expected_functions() {
        let db = OffsetDatabase::from_compiled_offsets();
        let expected_functions = [
            "castSpell",
            "doCombatAbility",
            "useSkill",
            "canUseItem",
            "doAttack",
            "executeCmd",
            "interpretCmd",
            "clickedPlayer",
            "issuePetCommand",
            "getConLevel",
            "getPcClient",
            "doLoot",
            "processGameEvents",
            "realRenderWorld",
            "fixHeading",
            "getBearing",
            "freeTargetCastSpell",
            "changeHeight",
            "zoneGuideManager",
            "charListEnterWorld",
            "charListSelectChar",
            "cchatMgrGetRgba",
            "cchatMgrInitContextMenu",
            "cchatMgrFreeChatWindow",
            "cchatMgrSetLockedActiveChat",
            "cchatMgrCreateChatWindow",
            "invSlotMgrFindSlot",
            "invSlotMgrMoveItem",
            "invSlotMgrSelectSlot",
            "spellBookWndMemorizeSet",
            "netSend",
            "outboundMsgCounter",
            "inboundMsgCounter",
            "fileIntegrityDispatcher",
            "serverMemcheckHandler",
            "worldAuthenticate",
            "systemFingerprint",
            "contextMenuMgrHandleMenu",
        ];
        for key in &expected_functions {
            assert!(db.get_function(key).is_some(), "missing function: {}", key);
        }
        assert_eq!(db.functions.len(), expected_functions.len());
    }

    #[test]
    fn get_function_returns_correct_value() {
        let db = OffsetDatabase::from_compiled_offsets();
        assert_eq!(
            db.get_function("castSpell"),
            Some(crate::offsets::CAST_SPELL)
        );
        assert_eq!(db.get_function("netSend"), Some(crate::offsets::NET_SEND));
        assert_eq!(
            db.get_function("systemFingerprint"),
            Some(crate::offsets::SYSTEM_FINGERPRINT)
        );
    }

    #[test]
    fn get_function_returns_none_for_missing() {
        let db = OffsetDatabase::from_compiled_offsets();
        assert!(db.get_function("does_not_exist").is_none());
    }

    #[test]
    fn context_menu_manager_offsets_exposed_in_db() {
        let db = OffsetDatabase::from_compiled_offsets();
        assert_eq!(
            db.get_context_menu_manager_offset("currMenu"),
            Some(crate::offsets::context_menu_mgr::CUR_MENU)
        );
        assert_eq!(
            db.get_context_menu_manager_offset("numMenus"),
            Some(crate::offsets::context_menu_mgr::MENUS_COUNT)
        );
        assert_eq!(
            db.get_context_menu_manager_offset("curItem"),
            Some(crate::offsets::context_menu_mgr::CUR_ITEM)
        );
        assert!(db.get_context_menu_manager_offset("nonexistent").is_none());
    }

    #[test]
    fn context_menu_offsets_empty_until_json_loaded() {
        // context_menu offsets have no compile-time fallback yet (see
        // from_compiled_offsets comment).  They are populated via JSON at runtime.
        let db = OffsetDatabase::from_compiled_offsets();
        assert!(db.context_menu.is_empty());
        assert!(db.get_context_menu_offset("numItems").is_none());
        assert!(db.get_context_menu_offset("nonexistent").is_none());
    }

    #[test]
    fn context_menu_db_roundtrips_through_json() {
        let db = OffsetDatabase::from_compiled_offsets();
        let json = serde_json::to_string(&db).expect("serialize");
        let restored: OffsetDatabase = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            restored.get_context_menu_manager_offset("numMenus"),
            db.get_context_menu_manager_offset("numMenus")
        );
        assert_eq!(
            restored.get_context_menu_offset("numItems"),
            db.get_context_menu_offset("numItems")
        );
    }
}
