use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

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
    /// eqmain.dll global pointer addresses keyed by name.
    #[serde(default)]
    pub eqmain_globals: HashMap<String, u64>,
    /// eqmain.dll function addresses keyed by name.
    #[serde(default)]
    pub eqmain_functions: HashMap<String, u64>,
    /// eqgraphicsdx9.dll global pointer addresses keyed by name.
    #[serde(default)]
    pub eqgraphics_globals: HashMap<String, u64>,
    /// eqgraphicsdx9.dll function addresses keyed by name.
    #[serde(default)]
    pub eqgraphics_functions: HashMap<String, u64>,
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
    /// Serialize and write this database to a JSON file.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
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

    /// Look up an eqmain.dll global pointer address by name.
    #[must_use]
    pub fn get_eqmain_global(&self, name: &str) -> Option<u64> {
        self.eqmain_globals.get(name).copied()
    }

    /// Look up an eqmain.dll function address by name.
    #[must_use]
    pub fn get_eqmain_function(&self, name: &str) -> Option<u64> {
        self.eqmain_functions.get(name).copied()
    }

    /// Look up an eqgraphicsdx9.dll global pointer address by name.
    #[must_use]
    pub fn get_eqgraphics_global(&self, name: &str) -> Option<u64> {
        self.eqgraphics_globals.get(name).copied()
    }

    /// Look up an eqgraphicsdx9.dll function address by name.
    #[must_use]
    pub fn get_eqgraphics_function(&self, name: &str) -> Option<u64> {
        self.eqgraphics_functions.get(name).copied()
    }

    /// Convert a preferred-base address to a runtime address using this
    /// database's preferred base.
    #[must_use]
    pub fn rebase(&self, preferred_addr: u64, actual_base: u64) -> Option<usize> {
        let offset = preferred_addr.checked_sub(self.eq_preferred_base)?;
        Some((actual_base + offset) as usize)
    }

    /// Look up a named function/global and convert it to a runtime address.
    ///
    /// Functions are checked before globals because function hooks are the
    /// highest-risk consumers during DLL startup; names are expected to match the
    /// keys from [`Self::from_compiled_offsets`].
    #[must_use]
    pub fn rebase_by_name(&self, name: &str, actual_base: u64) -> Option<usize> {
        self.get_function(name)
            .or_else(|| self.get_global(name))
            .and_then(|preferred_addr| self.rebase(preferred_addr, actual_base))
    }

    /// Merge scan results into this database.
    ///
    /// Overwrites matching keys in the `globals` and `functions` maps with
    /// addresses resolved by the scan engine. Entries not present in the report
    /// are left unchanged (preserving compiled constants or JSON overrides).
    pub fn merge_scan_results(&mut self, report: &crate::scan_engine::ScanReport) {
        crate::scan_engine::apply_to_offset_db(report, self);
    }

    /// Create from the current compile-time constants in offsets.rs
    #[must_use]
    pub fn from_compiled_offsets() -> Self {
        use crate::offsets::{
            CAN_USE_ITEM, CAST_SPELL, CCHAT_MGR_CREATE_CHAT_WINDOW, CCHAT_MGR_FREE_CHAT_WINDOW,
            CCHAT_MGR_GET_RGBA, CCHAT_MGR_INIT_CONTEXT_MENU, CCHAT_MGR_SET_LOCKED_ACTIVE_CHAT,
            CHANGE_HEIGHT, CHAR_LIST_ENTER_WORLD, CHAR_LIST_SELECT_CHAR, CLICKED_PLAYER,
            CONTEXT_MENU_MGR_HANDLE_MENU, DO_ATTACK, DO_COMBAT_ABILITY, DO_LOOT, DSP_CHAT,
            EQ_BEGIN_ZONE, EQ_BUFF_PLAYER, EQ_BUY_ITEM, EQ_COMPLETE_TRADE, EQ_DISBAND, EQ_END_ZONE,
            EQ_FINISH_ZONE, EQ_FOLLOW_PLAYER, EQ_INVITE_PLAYER, EQ_MAKE_LEADER, EQ_OPEN_TRADE,
            EQ_PREFERRED_BASE, EQ_REMOVE_BUFF, EQ_SELL_ITEM, EQ_ZONE_CHANGE,
            EQGRAPHICS_DEVICE_RESET, EQGRAPHICS_DX_PRESENT, EQGRAPHICS_INIT_RENDER,
            EQGRAPHICS_REALRENDER_WORLD, EQGRAPHICS_RENDER_FRAME, EXECUTE_CMD,
            FILE_INTEGRITY_DISPATCHER, FIX_HEADING, FREE_TARGET_CAST_SPELL, GET_BEARING,
            GET_CON_LEVEL, GET_PC_CLIENT, INBOUND_MSG_COUNTER, INTERPRET_CMD,
            INV_SLOT_GET_ITEM_BASE, INV_SLOT_MGR_FIND_SLOT, INV_SLOT_MGR_MOVE_ITEM,
            INV_SLOT_MGR_SELECT_SLOT, ISSUE_PET_COMMAND, MEMCHECK4_PROCESS_ENUM, NET_SEND,
            OUTBOUND_MSG_COUNTER, PINST_ACTIVE_CORPSE, PINST_CCHAT_WINDOW_MANAGER, PINST_CDISPLAY,
            PINST_CEVERQUEST, PINST_CINV_SLOT_MGR, PINST_CONTEXT_MENU_MANAGER,
            PINST_CONTROLLED_PLAYER, PINST_CXWND_MANAGER, PINST_LOCAL_PC, PINST_LOCAL_PLAYER,
            PINST_SGRAPHICSENGINE, PINST_SPAWN_MANAGER, PINST_SPELL_MANAGER, PINST_TARGET,
            PROCESS_GAME_EVENTS, REAL_RENDER_WORLD, RIGHT_CLICKED_ON_PLAYER,
            SERVER_MEMCHECK_HANDLER, SPELL_BOOK_WND_MEMORIZE_SET, SYSTEM_FINGERPRINT, USE_SKILL,
            WORLD_AUTHENTICATE, ZONE_GUIDE_MANAGER, context_menu_mgr, eqmain, player_base,
            player_zone, spawn_manager, zone_info,
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
            "pinstCChatWindowManager".to_string(),
            PINST_CCHAT_WINDOW_MANAGER,
        );
        globals.insert("pinstCInvSlotMgr".to_string(), PINST_CINV_SLOT_MGR);
        globals.insert("pinstCXWndManager".to_string(), PINST_CXWND_MANAGER);
        globals.insert("pinstActiveCorpse".to_string(), PINST_ACTIVE_CORPSE);
        globals.insert("pinstSGraphicsEngine".to_string(), PINST_SGRAPHICSENGINE);
        globals.insert(
            "pinstCContextMenuManager".to_string(),
            PINST_CONTEXT_MENU_MANAGER,
        );
        globals.insert("instEQZoneInfo".to_string(), zone_info::INST_EQ_ZONE_INFO);
        globals.insert("outboundMsgCounter".to_string(), OUTBOUND_MSG_COUNTER);
        globals.insert("inboundMsgCounter".to_string(), INBOUND_MSG_COUNTER);

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

        let player_base = [
            ("next", player_base::NEXT),
            ("prev", player_base::PREV),
            ("y", player_base::Y),
            ("x", player_base::X),
            ("z", player_base::Z),
            ("heading", player_base::HEADING),
            ("speedCurrent", player_base::SPEED_CURRENT),
            ("speedRun", player_base::SPEED_RUN),
            ("speedHeading", player_base::SPEED_HEADING),
            ("name", player_base::NAME),
            ("displayedName", player_base::DISPLAYED_NAME),
            ("type", player_base::TYPE),
            ("spawnId", player_base::SPAWN_ID),
            ("lastName", player_base::LASTNAME),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

        let player_zone = [
            ("hpMax", player_zone::HP_MAX),
            ("hpCurrent", player_zone::HP_CURRENT),
            ("manaMax", player_zone::MANA_MAX),
            ("manaCurrent", player_zone::MANA_CURRENT),
            ("level", player_zone::LEVEL),
            ("charClass", player_zone::CHAR_CLASS),
            ("enduranceCurrent", player_zone::ENDURANCE_CURRENT),
            ("enduranceMax", player_zone::ENDURANCE_MAX),
            ("standState", player_zone::STANDSTATE),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

        let spawn_manager = [("playerList", spawn_manager::PLAYER_LIST)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();

        let context_menu_manager = [
            ("menusArray", context_menu_mgr::MENUS_DATA),
            ("numMenus", context_menu_mgr::MENUS_COUNT),
            ("currMenu", context_menu_mgr::CUR_MENU),
            ("curItem", context_menu_mgr::CUR_ITEM),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

        // No compile-time `offsets::context_menu` fallback is currently defined.
        // Leave the map empty here; JSON-loaded offsets can still populate it.
        let context_menu = HashMap::new();

        let mut functions = HashMap::new();
        functions.insert("castSpell".into(), CAST_SPELL);
        functions.insert("doCombatAbility".into(), DO_COMBAT_ABILITY);
        functions.insert("useSkill".into(), USE_SKILL);
        functions.insert("canUseItem".into(), CAN_USE_ITEM);
        functions.insert("doAttack".into(), DO_ATTACK);
        functions.insert("executeCmd".into(), EXECUTE_CMD);
        functions.insert("interpretCmd".into(), INTERPRET_CMD);
        functions.insert("rightClickedOnPlayer".into(), RIGHT_CLICKED_ON_PLAYER);
        functions.insert("clickedPlayer".into(), CLICKED_PLAYER);
        functions.insert("issuePetCommand".into(), ISSUE_PET_COMMAND);
        functions.insert("getConLevel".into(), GET_CON_LEVEL);
        functions.insert("getPcClient".into(), GET_PC_CLIENT);
        functions.insert("doLoot".into(), DO_LOOT);
        functions.insert("processGameEvents".into(), PROCESS_GAME_EVENTS);
        functions.insert("dspChat".into(), DSP_CHAT);
        functions.insert("realRenderWorld".into(), REAL_RENDER_WORLD);
        functions.insert("fixHeading".into(), FIX_HEADING);
        functions.insert("getBearing".into(), GET_BEARING);
        functions.insert("freeTargetCastSpell".into(), FREE_TARGET_CAST_SPELL);
        functions.insert("changeHeight".into(), CHANGE_HEIGHT);
        functions.insert("zoneGuideManager".into(), ZONE_GUIDE_MANAGER);
        functions.insert("charListEnterWorld".into(), CHAR_LIST_ENTER_WORLD);
        functions.insert("charListSelectChar".into(), CHAR_LIST_SELECT_CHAR);
        functions.insert("cchatMgrGetRgba".into(), CCHAT_MGR_GET_RGBA);
        functions.insert(
            "cchatMgrInitContextMenu".into(),
            CCHAT_MGR_INIT_CONTEXT_MENU,
        );
        functions.insert("cchatMgrFreeChatWindow".into(), CCHAT_MGR_FREE_CHAT_WINDOW);
        functions.insert(
            "cchatMgrSetLockedActiveChat".into(),
            CCHAT_MGR_SET_LOCKED_ACTIVE_CHAT,
        );
        functions.insert(
            "cchatMgrCreateChatWindow".into(),
            CCHAT_MGR_CREATE_CHAT_WINDOW,
        );
        functions.insert("invSlotMgrFindSlot".into(), INV_SLOT_MGR_FIND_SLOT);
        functions.insert("invSlotMgrMoveItem".into(), INV_SLOT_MGR_MOVE_ITEM);
        functions.insert("invSlotMgrSelectSlot".into(), INV_SLOT_MGR_SELECT_SLOT);
        functions.insert("invSlotGetItemBase".into(), INV_SLOT_GET_ITEM_BASE);
        functions.insert(
            "spellBookWndMemorizeSet".into(),
            SPELL_BOOK_WND_MEMORIZE_SET,
        );
        functions.insert("netSend".into(), NET_SEND);
        functions.insert("fileIntegrityDispatcher".into(), FILE_INTEGRITY_DISPATCHER);
        functions.insert("serverMemcheckHandler".into(), SERVER_MEMCHECK_HANDLER);
        functions.insert("worldAuthenticate".into(), WORLD_AUTHENTICATE);
        functions.insert("systemFingerprint".into(), SYSTEM_FINGERPRINT);
        functions.insert("memcheck4ProcessEnum".into(), MEMCHECK4_PROCESS_ENUM);
        functions.insert(
            "contextMenuMgrHandleMenu".into(),
            CONTEXT_MENU_MGR_HANDLE_MENU,
        );
        functions.insert("eqBeginZone".into(), EQ_BEGIN_ZONE);
        functions.insert("eqEndZone".into(), EQ_END_ZONE);
        functions.insert("eqFinishZone".into(), EQ_FINISH_ZONE);
        functions.insert("eqZoneChange".into(), EQ_ZONE_CHANGE);
        functions.insert("eqInvitePlayer".into(), EQ_INVITE_PLAYER);
        functions.insert("eqDisband".into(), EQ_DISBAND);
        functions.insert("eqFollowPlayer".into(), EQ_FOLLOW_PLAYER);
        functions.insert("eqMakeLeader".into(), EQ_MAKE_LEADER);
        functions.insert("eqBuyItem".into(), EQ_BUY_ITEM);
        functions.insert("eqSellItem".into(), EQ_SELL_ITEM);
        functions.insert("eqOpenTrade".into(), EQ_OPEN_TRADE);
        functions.insert("eqCompleteTrade".into(), EQ_COMPLETE_TRADE);
        functions.insert("eqBuffPlayer".into(), EQ_BUFF_PLAYER);
        functions.insert("eqRemoveBuff".into(), EQ_REMOVE_BUFF);

        let eqmain_globals = [
            ("sidlManager", eqmain::SIDL_MANAGER),
            ("loginServerApi", eqmain::LOGIN_SERVER_API),
            ("cxwndManager", eqmain::CXWND_MANAGER),
            ("loginViewManager", eqmain::LOGIN_VIEW_MANAGER),
            ("pinstLoginClient", eqmain::PINST_LOGIN_CLIENT),
            ("pinstLoginController", eqmain::PINST_LOGIN_CONTROLLER),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

        let eqmain_functions = [
            ("joinServer", eqmain::JOIN_SERVER),
            ("charSelectEnterWorld", eqmain::CHAR_SELECT_ENTER_WORLD),
            ("serverSelect", eqmain::SERVER_SELECT),
            ("handleSplash", eqmain::HANDLE_SPLASH),
            (
                "charSelectSelectCharacter",
                eqmain::CHAR_SELECT_SELECT_CHARACTER,
            ),
            ("charSelectSetFocus", eqmain::CHAR_SELECT_SET_FOCUS),
            (
                "loginControllerGiveTime",
                eqmain::LOGIN_CONTROLLER_GIVE_TIME,
            ),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

        let eqgraphics_globals = HashMap::new();

        let eqgraphics_functions = [
            ("realRenderWorld", EQGRAPHICS_REALRENDER_WORLD),
            ("deviceReset", EQGRAPHICS_DEVICE_RESET),
            ("initRender", EQGRAPHICS_INIT_RENDER),
            ("renderFrame", EQGRAPHICS_RENDER_FRAME),
            ("dxPresent", EQGRAPHICS_DX_PRESENT),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

        Self {
            client_date: crate::offsets::CLIENT_DATE.to_string(),
            eq_preferred_base: EQ_PREFERRED_BASE,
            globals,
            player_base,
            player_zone,
            spawn_manager,
            context_menu_manager,
            context_menu,
            functions,
            eqmain_globals,
            eqmain_functions,
            eqgraphics_globals,
            eqgraphics_functions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pattern_db::{OffsetCategory, ScanModule},
        scan_engine::{ScanReport, ScanResult},
    };
    use tempfile::tempdir;

    #[test]
    fn from_compiled_offsets_has_expected_globals() {
        let db = OffsetDatabase::from_compiled_offsets();
        assert!(db.get_global("pinstLocalPlayer").is_some());
        assert!(db.get_global("pinstTarget").is_some());
        assert!(db.get_global("pinstCChatWindowManager").is_some());
        assert!(db.get_global("instEQZoneInfo").is_some());
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
    fn rebase_by_name_prefers_function_and_global_entries() {
        let mut db = OffsetDatabase::from_compiled_offsets();
        db.functions
            .insert("customFunc".to_string(), db.eq_preferred_base + 0x1234);
        db.globals
            .insert("customGlobal".to_string(), db.eq_preferred_base + 0x5678);

        assert_eq!(
            db.rebase_by_name("customFunc", 0x7FF6_0000_0000),
            Some(0x7FF6_0000_1234)
        );
        assert_eq!(
            db.rebase_by_name("customGlobal", 0x7FF6_0000_0000),
            Some(0x7FF6_0000_5678)
        );
        assert_eq!(db.rebase_by_name("missing", 0x7FF6_0000_0000), None);
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
    fn load_from_legacy_json_defaults_optional_maps() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("legacy-offsets.json");
        std::fs::write(
            &path,
            r#"{
                "client_date": "20260310",
                "eq_preferred_base": 5368709120,
                "globals": {"pinstLocalPlayer": 1234},
                "player_base": {"x": 120},
                "player_zone": {"level": 420},
                "spawn_manager": {"playerList": 16}
            }"#,
        )
        .unwrap();

        let loaded = OffsetDatabase::load_from_file(&path).unwrap();

        assert!(loaded.context_menu_manager.is_empty());
        assert!(loaded.context_menu.is_empty());
        assert!(loaded.functions.is_empty());
        assert_eq!(loaded.get_global("pinstLocalPlayer"), Some(1234));
        assert_eq!(loaded.get_player_base_offset("x"), Some(120));
        assert_eq!(loaded.get_player_zone_offset("level"), Some(420));
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
    fn merge_scan_results_updates_only_non_zero_results() {
        let mut db = OffsetDatabase::from_compiled_offsets();
        let original_global = db.get_global("pinstLocalPlayer").unwrap();
        let original_function = db.get_function("castSpell").unwrap();
        let original_other_function = db.get_function("useSkill").unwrap();
        let original_player_base = db.get_player_base_offset("x").unwrap();

        let report = ScanReport {
            module: ScanModule::EqGame,
            entries_scanned: 3,
            entries_found: 3,
            entries_validated: 0,
            entries_failed: vec![],
            entries_skipped: vec![],
            entries_moved: vec![],
            results: vec![
                ScanResult {
                    name: "pinstLocalPlayer".to_string(),
                    category: OffsetCategory::Global,
                    resolved_preferred: original_global + 0x20,
                    matched_at_offset: 0,
                    validated: false,
                },
                ScanResult {
                    name: "castSpell".to_string(),
                    category: OffsetCategory::Function,
                    resolved_preferred: original_function + 0x40,
                    matched_at_offset: 0,
                    validated: false,
                },
                ScanResult {
                    name: "useSkill".to_string(),
                    category: OffsetCategory::Function,
                    resolved_preferred: 0,
                    matched_at_offset: 0,
                    validated: false,
                },
            ],
        };

        db.merge_scan_results(&report);

        assert_eq!(
            db.get_global("pinstLocalPlayer"),
            Some(original_global + 0x20)
        );
        assert_eq!(db.get_function("castSpell"), Some(original_function + 0x40));
        assert_eq!(db.get_function("useSkill"), Some(original_other_function));
        assert_eq!(db.get_player_base_offset("x"), Some(original_player_base));
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
            "pinstCChatWindowManager",
            "pinstCInvSlotMgr",
            "pinstCXWndManager",
            "pinstActiveCorpse",
            "pinstSGraphicsEngine",
            "pinstCContextMenuManager",
            "instEQZoneInfo",
            "outboundMsgCounter",
            "inboundMsgCounter",
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
            "rightClickedOnPlayer",
            "clickedPlayer",
            "issuePetCommand",
            "getConLevel",
            "getPcClient",
            "doLoot",
            "processGameEvents",
            "dspChat",
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
            "invSlotGetItemBase",
            "spellBookWndMemorizeSet",
            "netSend",
            "fileIntegrityDispatcher",
            "serverMemcheckHandler",
            "worldAuthenticate",
            "systemFingerprint",
            "memcheck4ProcessEnum",
            "contextMenuMgrHandleMenu",
            "eqBeginZone",
            "eqEndZone",
            "eqFinishZone",
            "eqZoneChange",
            "eqInvitePlayer",
            "eqDisband",
            "eqFollowPlayer",
            "eqMakeLeader",
            "eqBuyItem",
            "eqSellItem",
            "eqOpenTrade",
            "eqCompleteTrade",
            "eqBuffPlayer",
            "eqRemoveBuff",
        ];
        for key in &expected_functions {
            assert!(db.get_function(key).is_some(), "missing function: {}", key);
        }
        assert_eq!(db.functions.len(), expected_functions.len());
    }

    #[test]
    fn from_compiled_offsets_new_functions_do_not_duplicate_existing_non_zero_addresses() {
        let db = OffsetDatabase::from_compiled_offsets();
        let mut existing_without_new = db.functions.clone();

        let new_constants = [
            ("eqBeginZone", crate::offsets::EQ_BEGIN_ZONE),
            ("eqEndZone", crate::offsets::EQ_END_ZONE),
            ("eqFinishZone", crate::offsets::EQ_FINISH_ZONE),
            ("eqZoneChange", crate::offsets::EQ_ZONE_CHANGE),
            ("eqInvitePlayer", crate::offsets::EQ_INVITE_PLAYER),
            ("eqDisband", crate::offsets::EQ_DISBAND),
            ("eqFollowPlayer", crate::offsets::EQ_FOLLOW_PLAYER),
            ("eqMakeLeader", crate::offsets::EQ_MAKE_LEADER),
            ("eqBuyItem", crate::offsets::EQ_BUY_ITEM),
            ("eqSellItem", crate::offsets::EQ_SELL_ITEM),
            ("eqOpenTrade", crate::offsets::EQ_OPEN_TRADE),
            ("eqCompleteTrade", crate::offsets::EQ_COMPLETE_TRADE),
            ("eqBuffPlayer", crate::offsets::EQ_BUFF_PLAYER),
            ("eqRemoveBuff", crate::offsets::EQ_REMOVE_BUFF),
        ];

        for (name, _) in new_constants {
            existing_without_new.remove(name);
        }

        let non_zero_existing: Vec<u64> = existing_without_new
            .values()
            .copied()
            .filter(|addr| *addr != 0)
            .collect();

        for (name, addr) in &new_constants {
            if *addr != 0 {
                assert!(
                    !non_zero_existing.contains(addr),
                    "new function {name} duplicates an existing non-zero address"
                );
            }
        }
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

    #[test]
    fn from_compiled_offsets_exposes_module_specific_offsets() {
        let db = OffsetDatabase::from_compiled_offsets();

        assert_eq!(
            db.get_eqmain_global("cxwndManager"),
            Some(crate::offsets::eqmain::CXWND_MANAGER)
        );
        assert_eq!(
            db.get_eqmain_function("joinServer"),
            Some(crate::offsets::eqmain::JOIN_SERVER)
        );
        assert_eq!(
            db.get_eqgraphics_function("realRenderWorld"),
            Some(crate::offsets::EQGRAPHICS_REALRENDER_WORLD)
        );
    }

    #[test]
    fn load_from_legacy_json_defaults_module_specific_offsets() {
        let loaded: OffsetDatabase = serde_json::from_str(
            r#"{
                "client_date": "20260415",
                "eq_preferred_base": 5368709120,
                "globals": {},
                "player_base": {},
                "player_zone": {},
                "spawn_manager": {}
            }"#,
        )
        .expect("deserialize legacy offsets");

        assert!(loaded.eqmain_globals.is_empty());
        assert!(loaded.eqmain_functions.is_empty());
        assert!(loaded.eqgraphics_globals.is_empty());
        assert!(loaded.eqgraphics_functions.is_empty());
    }

    // ─── load/save edge cases ───────────────────────────────────────────

    #[test]
    fn load_from_nonexistent_path_object_returns_error() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let result = OffsetDatabase::load_from_file(&dir.path().join("does_not_exist.json"));
        assert!(result.is_err());
    }

    #[test]
    fn load_from_invalid_json_via_tempfile_returns_error() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not valid json {{{").expect("write");
        let result = OffsetDatabase::load_from_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn save_and_load_roundtrip_preserves_custom_entries() {
        let mut db = OffsetDatabase::from_compiled_offsets();
        db.globals.insert("customGlobal".to_string(), 0xDEAD);
        db.functions.insert("customFunc".to_string(), 0xBEEF);
        db.context_menu.insert("numItems".to_string(), 0x42);

        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("roundtrip.json");
        db.save_to_file(&path).expect("save");
        let loaded = OffsetDatabase::load_from_file(&path).expect("load");
        assert_eq!(loaded.get_global("customGlobal"), Some(0xDEAD));
        assert_eq!(loaded.get_function("customFunc"), Some(0xBEEF));
        assert_eq!(loaded.get_context_menu_offset("numItems"), Some(0x42));
    }

    // ─── rebase tests ───────────────────────────────────────────────────

    #[test]
    fn rebase_returns_none_for_address_below_preferred_base() {
        let db = OffsetDatabase::from_compiled_offsets();
        // An address smaller than preferred base should underflow
        let result = db.rebase(0, db.eq_preferred_base + 1);
        // 0 < eq_preferred_base for any realistic base, so checked_sub returns None
        assert!(result.is_none());
    }

    #[test]
    fn rebase_computes_correct_runtime_address() {
        let db = OffsetDatabase::from_compiled_offsets();
        let preferred_addr = db.eq_preferred_base + 0x1000;
        let actual_base = 0x7FF0_0000_0000_u64;
        let result = db.rebase(preferred_addr, actual_base);
        assert_eq!(result, Some((actual_base + 0x1000) as usize));
    }

    #[test]
    fn rebase_at_preferred_base_returns_actual_base() {
        let db = OffsetDatabase::from_compiled_offsets();
        let result = db.rebase(db.eq_preferred_base, 0x1_0000);
        assert_eq!(result, Some(0x1_0000));
    }

    // ─── get_* accessors for missing keys ───────────────────────────────

    #[test]
    fn get_global_missing_returns_none() {
        let db = OffsetDatabase::from_compiled_offsets();
        assert!(db.get_global("nonExistentGlobal").is_none());
    }

    #[test]
    fn get_player_base_offset_missing_returns_none() {
        let db = OffsetDatabase::from_compiled_offsets();
        assert!(db.get_player_base_offset("nonExistent").is_none());
    }

    #[test]
    fn get_player_zone_offset_missing_returns_none() {
        let db = OffsetDatabase::from_compiled_offsets();
        assert!(db.get_player_zone_offset("nonExistent").is_none());
    }

    #[test]
    fn get_function_missing_returns_none() {
        let db = OffsetDatabase::from_compiled_offsets();
        assert!(db.get_function("nonExistentFunction").is_none());
    }

    // ─── merge_scan_results edge cases ──────────────────────────────────

    #[test]
    fn merge_scan_results_empty_report_is_noop() {
        let original = OffsetDatabase::from_compiled_offsets();
        let mut db = original.clone();
        let report = crate::scan_engine::ScanReport {
            module: crate::pattern_db::ScanModule::EqGame,
            entries_scanned: 0,
            entries_found: 0,
            entries_validated: 0,
            entries_failed: vec![],
            entries_skipped: vec![],
            entries_moved: vec![],
            results: vec![],
        };
        db.merge_scan_results(&report);
        assert_eq!(db, original);
    }
}
