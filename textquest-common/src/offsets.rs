// EQ global pointer addresses from eqlib/offsets/eqgame.h
// These are PREFERRED 64-bit addresses (base 0x0001_4000_0000).
// At runtime, subtract the preferred base and add the actual base
// (obtained via GetModuleInformation or EnumProcessModules).
//
// Source: third_party/eqlib/include/eqlib/offsets/eqgame.h
// Client date: 20260310 (March 10, 2026)

/// Preferred base address of eqgame.exe (64-bit)
pub const EQ_PREFERRED_BASE: u64 = 0x0001_4000_0000;

/// Pointer to local player (`PlayerClient`*)
pub const PINST_LOCAL_PLAYER: u64 = 0x0001_40E8_E380;

/// Pointer to controlled player (`PlayerClient`*)
pub const PINST_CONTROLLED_PLAYER: u64 = 0x0001_40E8_E430;

/// Pointer to current target (`PlayerClient`*)
pub const PINST_TARGET: u64 = 0x0001_40E8_E428;

/// Pointer to spawn manager (`PlayerManagerClient`*)
pub const PINST_SPAWN_MANAGER: u64 = 0x0001_40F0_CD90;

/// Pointer to local PC data (`PcClient`*)
pub const PINST_LOCAL_PC: u64 = 0x0001_40E9_09A8;

/// Pointer to spell manager
pub const PINST_SPELL_MANAGER: u64 = 0x0001_40F0_E6F0;

/// Pointer to `CDisplay`
pub const PINST_CDISPLAY: u64 = 0x0001_40E8_E450;

/// Pointer to `CEverQuest`
pub const PINST_CEVERQUEST: u64 = 0x0001_40F1_1758;

/// Pointer to `CChatWindowManager` (in-game chat window manager)
/// Source: eqgame.h `pinstCChatWindowManager_x`
pub const PINST_CCHAT_WINDOW_MANAGER: u64 = 0x0001_40F2_2B20;

/// Pointer to `CInvSlotMgr` (inventory slot manager)
/// Source: eqgame.h `pinstCInvSlotMgr_x`
pub const PINST_CINV_SLOT_MGR: u64 = 0x0001_40DD_D5F0;

// ─── EQ Internal Function Addresses ───
// These are preferred-base addresses for EQ's internal functions.
// Used for calling game functions directly from the injected DLL.
// Source: macroquest/eqlib live branch, client date 20260310
// Calling convention: x64 MSVC (this in RCX for member functions)

/// `CharacterZoneClient::CastSpell` — cast a spell by gem ID
/// Signature: unsigned char CastSpell(unsigned char gemid, int spellid, ...)
pub const CAST_SPELL: u64 = 0x0001_400D_9F20;

/// `PcZoneClient::DoCombatAbility` — use a combat ability
/// Signature: bool DoCombatAbility(int spellID, bool allowLowerRank)
pub const DO_COMBAT_ABILITY: u64 = 0x0001_402E_D490;

/// `CharacterZoneClient::UseSkill` — use a skill on a target
/// Signature: void UseSkill(unsigned char skill, `PlayerZoneClient`* Target, bool bAuto)
pub const USE_SKILL: u64 = 0x0001_4010_52A0;

/// `CharacterZoneClient::CanUseItem` — check if an item is usable
pub const CAN_USE_ITEM: u64 = 0x0001_400E_DDB0;

/// `PlayerZoneClient::DoAttack` — perform a melee attack
/// Signature: bool DoAttack(BYTE slot, BYTE skill, `PlayerZoneClient`* Target, ...)
pub const DO_ATTACK: u64 = 0x0001_4031_B890;

/// __ExecuteCmd — execute any EQ command by command ID (most versatile)
/// Can do: follow, stopcast, face, sit, stand, attack, etc.
pub const EXECUTE_CMD: u64 = 0x0001_4022_35B0;

/// `CEverQuest::InterpretCmd` — interpret a slash command string
/// Signature: void InterpretCmd(PlayerClient*, const char*)
pub const INTERPRET_CMD: u64 = 0x0001_4028_3FB0;

/// `CEverQuest::RightClickedOnPlayer` — open NPC interaction window (merchant, bank, quest)
/// Signature: void RightClickedOnPlayer(PlayerClient* target, int unknown)
/// Source: eqlib/offsets/eqgame.h `CEverQuest__RightClickedOnPlayer_x`
pub const RIGHT_CLICKED_ON_PLAYER: u64 = 0x0001_4029_6D30;

/// `pinstCEverQuest` — pointer to the global CEverQuest instance
/// Source: eqlib/offsets/eqgame.h `pinstCEverQuest_x`
pub const PINST_EVERQUEST: u64 = 0x0001_40F1_1758;

// ─── PcClient struct field offsets ───
// Source: eqlib PcClient.h, client date 20260310

/// Offset of `pExtendedTargetList` within PcClient (ExtendedTargetList*).
/// Source: PcClient.h line 1690: `/*0x2e98*/ ExtendedTargetList* pExtendedTargetList`
pub const PCCLIENT_EXTENDED_TARGET_LIST: u64 = 0x2e98;

/// Offset of `InCombat` within PcClient (bool).
/// Source: PcClient.h line 1693: `/*0x2eac*/ bool InCombat`
pub const PCCLIENT_IN_COMBAT: u64 = 0x2eac;

// ─── ExtendedTargetList internal layout ───
// Source: PcClient.h, ArrayClass in Containers.h
// ExtendedTargetList has a vtable at 0x00, then:
//   ArrayClass<ExtendedTargetSlot> m_targetSlots at 0x08
//   bool m_autoAddHaters at 0x20

/// Offset of m_targetSlots (ArrayClass) within ExtendedTargetList.
/// ArrayClass layout (inherits CDynamicArrayBase):
///   0x00: int m_length (from CDynamicArrayBase)
///   0x08: T* m_array
///   0x10: int m_alloc
pub const XTARGET_LIST_SLOTS_OFFSET: u64 = 0x08;
/// Offset of m_autoAddHaters within ExtendedTargetList.
pub const XTARGET_LIST_AUTO_ADD_HATERS: u64 = 0x20;

// ─── ArrayClass internal layout (CDynamicArrayBase + ArrayClass<T>) ───
/// Offset of m_length within ArrayClass (from CDynamicArrayBase base).
pub const ARRAY_CLASS_LENGTH: u64 = 0x00;
/// Offset of m_array (T*) within ArrayClass.
pub const ARRAY_CLASS_ARRAY_PTR: u64 = 0x08;

// ─── ExtendedTargetSlot layout (size 0x4c) ───
/// Size of a single ExtendedTargetSlot.
pub const XTARGET_SLOT_SIZE: u64 = 0x4c;
/// Offset of xTargetType (DWORD) within ExtendedTargetSlot.
pub const XTARGET_SLOT_TYPE: u64 = 0x00;
/// Offset of XTargetSlotStatus (DWORD enum) within ExtendedTargetSlot.
pub const XTARGET_SLOT_STATUS: u64 = 0x04;
/// Offset of SpawnID (uint32_t) within ExtendedTargetSlot.
pub const XTARGET_SLOT_SPAWN_ID: u64 = 0x08;
/// Offset of Name (char[64]) within ExtendedTargetSlot.
pub const XTARGET_SLOT_NAME: u64 = 0x0c;

/// pinstCXWndManager — eqgame.exe's UI window manager (not eqmain.dll's)
pub const PINST_CXWND_MANAGER: u64 = 0x0001_40F3_7B28;

/// `CCharacterListWnd::EnterWorld` — enter world from character select
/// Signature: void EnterWorld() (member function, takes this only)
pub const CHAR_LIST_ENTER_WORLD: u64 = 0x0001_400D_4B20;

/// `CCharacterListWnd::SelectCharacter` — select a character by index
/// Signature: void SelectCharacter(int index)
pub const CHAR_LIST_SELECT_CHAR: u64 = 0x0001_400D_5D20;

/// `CEverQuest::ClickedPlayer` — click-target a player
pub const CLICKED_PLAYER: u64 = 0x0001_4027_24F0;

/// `CEverQuest::IssuePetCommand` — issue a pet command
/// Signature: void IssuePetCommand(ePetCommandType, int `TargetID`, bool bQuiet, ...)
pub const ISSUE_PET_COMMAND: u64 = 0x0001_4028_56A0;

/// `PcClient::GetConLevel` — get consider level of target
pub const GET_CON_LEVEL: u64 = 0x0001_402E_3C10;

/// `PlayerClient`::GetPcClient — get `PcClient` from `PlayerClient`
pub const GET_PC_CLIENT: u64 = 0x0001_4030_7970;

/// __do_loot — main loot function (opens loot window on targeted corpse)
pub const DO_LOOT: u64 = 0x0001_4022_C0D0;

/// pinstActiveCorpse — pointer to the active corpse (current loot target)
pub const PINST_ACTIVE_CORPSE: u64 = 0x0001_40E8_E390;

/// __ProcessGameEvents — game event processing (hook point for game loop)
pub const PROCESS_GAME_EVENTS: u64 = 0x0001_4028_E0F0;

/// `CEverQuest::dsp_chat` — chat message display function.
/// Signature: `void dsp_chat(const char* text, int color, bool log, bool percent_convert)`
/// Source: eqgame.h `CEverQuest__dsp_chat_x` (ChatManagerClient__DisplayChatText)
pub const DSP_CHAT: u64 = 0x0001_4010_CFC0;

/// `CDisplay::RealRender_World` — render loop (alternative hook point)
pub const REAL_RENDER_WORLD: u64 = 0x0001_401A_4320;

/// `pinstSGraphicsEngine` — pointer to SGraphicsEngine struct.
/// SGraphicsEngine+0x18 = CRender*. CRender+0x0F00 = DeviceImpl* (DX9 wrapper over DX11).
/// DeviceImpl+0x28 = Device*. Device+0x18 = SwapChain (inline). SwapChain+0x00 = ID3D11Device*.
/// Source: eqgame.h `pinstSGraphicsEngine_x`
pub const PINST_SGRAPHICSENGINE: u64 = 0x0001_40F3_6B68;

/// __FixHeading — normalize heading value
pub const FIX_HEADING: u64 = 0x0001_4066_1520;

/// __get_bearing — calculate bearing between two points
pub const GET_BEARING: u64 = 0x0001_4025_8850;

// ─── Anti-Cheat / MemCheck addresses ───
// EQ's internal anti-cheat functions that scan for known cheat tools.
// These enumerate running processes, check memory regions, and validate
// module integrity. Critical to hook/bypass for M5 stealth.

/// `memcheck4` — Enumerates running processes looking for known cheat tools
/// (MQ2, WinEQ, etc.). Part of EQ's anti-cheat process scanner.
/// Source: blownt (2026-04-04)
pub const MEMCHECK4_PROCESS_ENUM: u64 = 0x0001_4029_9120;

/// `FreeTargetTracker::CastSpell` — ground-targeted spell casting
pub const FREE_TARGET_CAST_SPELL: u64 = 0x0001_402B_5740;

/// `PlayerZoneClient::ChangeHeight` — change character height
pub const CHANGE_HEIGHT: u64 = 0x0001_4031_AB80;

/// `ZoneGuideManagerClient` singleton (preferred base)
/// Source: eqgame.h `ZoneGuideManagerClient__Instance_x`
pub const ZONE_GUIDE_MANAGER: u64 = 0x0001_4035_71F0;

// ─── CChatWindowManager function addresses ───
// Source: eqgame.h, client date 20260310
// Calling convention: x64 MSVC (this in RCX for member functions)

/// `CChatWindowManager::GetRGBAFromIndex` — get an RGBA color from a chat color index
/// Signature: COLORREF GetRGBAFromIndex(int index)
pub const CCHAT_MGR_GET_RGBA: u64 = 0x0001_403B_2D40;

/// `CChatWindowManager::InitContextMenu` — initialize the chat window context menu
pub const CCHAT_MGR_INIT_CONTEXT_MENU: u64 = 0x0001_403B_2ED0;

/// `CChatWindowManager::FreeChatWindow` — free/destroy a chat window
/// Signature: void FreeChatWindow(CChatWindow* pWnd)
pub const CCHAT_MGR_FREE_CHAT_WINDOW: u64 = 0x0001_403B_1D40;

/// `CChatWindowManager::SetLockedActiveChatWindow` — lock the active chat window
/// Signature: void SetLockedActiveChatWindow(CChatWindow* pWnd)
pub const CCHAT_MGR_SET_LOCKED_ACTIVE_CHAT: u64 = 0x0001_403B_B240;

/// `CChatWindowManager::CreateChatWindow` — create a new chat window
/// Signature: CChatWindow* CreateChatWindow(CTabWnd* pTabs, int, int, CXStr name, int, int, int, int, int)
pub const CCHAT_MGR_CREATE_CHAT_WINDOW: u64 = 0x0001_403B_1780;

// ─── Anti-Cheat / Network Internals (Ghidra-verified) ───
// Source: Ghidra analysis of eqgame.exe, 2026-04-03
// These addresses were discovered via binary analysis, not eqlib headers.

/// Main network packet send function (247 bytes, 29+ callers)
pub const NET_SEND: u64 = 0x0001_4056_3130;

/// Global outbound message counter (decremented by every opcode handler)
pub const OUTBOUND_MSG_COUNTER: u64 = 0x0001_40F6_0ED8;

/// Global inbound message counter
pub const INBOUND_MSG_COUNTER: u64 = 0x0001_40F6_0ED4;

/// File integrity check dispatcher (EXE self-hash + data files)
pub const FILE_INTEGRITY_DISPATCHER: u64 = 0x0001_4021_D730;

/// Server memcheck opcode 0x4f27 handler (returns region hashes)
pub const SERVER_MEMCHECK_HANDLER: u64 = 0x0001_400B_5720;

/// World authentication function
pub const WORLD_AUTHENTICATE: u64 = 0x0001_402C_9C80;

/// SystemFingerprint: sends VideoCardId, NetworkCardId, HardriveId, ComputerName
pub const SYSTEM_FINGERPRINT: u64 = 0x0001_4059_4840;

// ─── CInvSlotMgr function addresses ───
// Source: eqgame.h, client date 20260310

/// `CInvSlotMgr::FindInvSlot` — find an inventory slot by location
/// Signature: CInvSlot* FindInvSlot(int, int, ItemContainerInstance, int, bool)
pub const INV_SLOT_MGR_FIND_SLOT: u64 = 0x0001_4042_1100;

/// `CInvSlotMgr::MoveItem` — move an item between inventory slots
/// Signature: bool MoveItem(ItemGlobalIndex const&, ItemGlobalIndex const&, bool, bool, bool, bool)
pub const INV_SLOT_MGR_MOVE_ITEM: u64 = 0x0001_4042_1C90;

/// `CInvSlotMgr::SelectSlot` — select an inventory slot
/// Signature: void SelectSlot(CInvSlot* pSlot, bool)
pub const INV_SLOT_MGR_SELECT_SLOT: u64 = 0x0001_4042_3FC0;

/// `CInvSlot::GetItemBase` — resolve the raw item pointer for a slot.
/// Signature: void GetItemBase(PCONTENTS*)
pub const INV_SLOT_GET_ITEM_BASE: u64 = 0x0001_4041_9520;

// ─── CSpellBookWnd function addresses ───
// Source: eqgame.h, client date 20260310

/// `CSpellBookWnd::MemorizeSet` — memorize a set of spells into gem slots
/// Signature: void MemorizeSet(int*, int)
pub const SPELL_BOOK_WND_MEMORIZE_SET: u64 = 0x0001_4050_EFE0;

/// Convert a preferred-base offset to an actual address given the runtime base.
///
/// Returns `None` if `preferred_addr` is below `EQ_PREFERRED_BASE` (would underflow).
#[must_use]
pub fn rebase(preferred_addr: u64, actual_base: u64) -> Option<usize> {
    let offset = preferred_addr.checked_sub(EQ_PREFERRED_BASE)?;
    Some((actual_base + offset) as usize)
}

// ─── eqmain.dll offsets ───
// These are preferred-base addresses within eqmain.dll (loaded into eqgame.exe process).
// eqmain.dll has its own base address; use `eqmain::rebase()` to convert.
// Source: MQ2 AutoLogin / eqmain offsets, client date 20260310

/// Offsets within eqmain.dll (login/server select UI module).
pub mod eqmain {
    /// Preferred base address of eqmain.dll (64-bit)
    pub const EQMAIN_PREFERRED_BASE: u64 = 0x0001_8000_0000;

    // ─── Global pointer addresses (preferred base) ───

    /// Pointer to `CSidlManager` instance
    pub const SIDL_MANAGER: u64 = 0x0001_8038_24C0;

    /// Pointer to `LoginServerAPI` instance
    pub const LOGIN_SERVER_API: u64 = 0x0001_8017_F4D0;

    /// Pointer to `CXWndManager` instance
    pub const CXWND_MANAGER: u64 = 0x0001_8038_24B8;

    /// `LoginServerAPI::JoinServer` function address
    pub const JOIN_SERVER: u64 = 0x0001_8001_8050;

    /// `LoginViewManager` function address
    pub const LOGIN_VIEW_MANAGER: u64 = 0x0001_8001_B0E0;

    // ─── Login pointer addresses (preferred base) ───

    /// Pointer to `LoginClient` instance (`LoginClient`*)
    /// Source: eqmain.h `EQMain__pinstLoginClient_x` (derived from pinstCLoginViewManager - 8)
    pub const PINST_LOGIN_CLIENT: u64 = 0x0001_8017_F4E0;

    /// Pointer to `LoginController` instance
    /// Source: eqmain.h `EQMain__pinstLoginController_x`
    pub const PINST_LOGIN_CONTROLLER: u64 = 0x0001_8017_F4F0;

    /// `LoginController::GiveTime()` — called every frame during eqmain.
    /// Hook target for main-thread execution during login/server select.
    /// Source: eqmain.h `EQMain__LoginController__GiveTime_x`
    pub const LOGIN_CONTROLLER_GIVE_TIME: u64 = 0x0001_8001_6640;

    // ─── LoginClient struct field offsets ───

    /// `LoginClient::pLoginData` (`EQLS::EQLogin`*) at offset 0x010
    pub const LOGINCLIENT_LOGIN_DATA: usize = 0x010;

    /// `LoginClient::ServerList` (`DoublyLinkedList`) at offset 0x178
    pub const LOGINCLIENT_SERVER_LIST: usize = 0x178;

    // ─── EQLogin struct field offsets ───

    /// `EQLogin::hEQWnd` (HWND) at offset 0x408
    pub const EQLOGIN_HWND: usize = 0x408;

    /// `EQLogin::Login` (char\[0x80\] array) at offset 0x414
    pub const EQLOGIN_USERNAME: usize = 0x414;

    /// `EQLogin::PW` (char\[0x80\] array) at offset 0x494
    pub const EQLOGIN_PASSWORD: usize = 0x494;

    /// `EQLogin::Character` (char\[0x40\] array) at offset 0x97C
    pub const EQLOGIN_CHARACTER: usize = 0x97C;

    /// Maximum length of login/password fields (0x80 = 128 bytes, use 0x7F for null terminator)
    pub const EQLOGIN_FIELD_MAX: usize = 0x7F;

    // ─── UI widget field offsets ───

    /// `CEditBaseWnd::InputText` field offset (`CXStr`)
    /// `CXStr` is a single pointer to `CStrRep` (8 bytes).
    pub const CEDITBASEWND_INPUT_TEXT: usize = 0x278;

    /// `XWM_LCLICK` notification code for button clicks
    pub const XWM_LCLICK: u32 = 1;

    /// `CXWnd` vtable offset for `SetWindowText` (virtual void `SetWindowText`(const `CXStr`&))
    /// From MQ2: `CXWnd` vtable layout has `SetWindowText` at /*0x280*/
    pub const CXWND_VTABLE_SET_WINDOW_TEXT: usize = 0x280;

    /// `CXWnd` vtable offset for `WndNotification` (eqmain.dll layout)
    /// Signature: int WndNotification(CXWnd* sender, `uint32_t` message, void* data)
    /// IMPORTANT: eqmain::`CXWnd` has this at 0x110, eqgame's `CXWnd` has it at 0x120!
    /// This offset is for eqmain context (login/server screens).
    pub const CXWND_VTABLE_WND_NOTIFICATION: usize = 0x110;

    // ─── CXWndManager struct offsets ───
    // From MQ2: CXWndManager { /*0x008*/ ArrayClass<CXWnd*> pWindows; ... }
    // ArrayClass<T> = { T* m_array; int m_length; int m_alloc; }

    /// `CXWndManager::pWindows.m_array` (pointer to `CXWnd`* array)
    /// NOTE: eqmain.dll layout differs from eqgame.exe — calibrated from hex dump
    pub const CXWNDMGR_WINDOWS_ARRAY: usize = 0x010;
    /// `CXWndManager::pWindows.m_length` (window count, u32)
    pub const CXWNDMGR_WINDOWS_COUNT: usize = 0x018;
    /// `CXWndManager::FocusWindow` (`CXWnd`*)
    pub const CXWNDMGR_FOCUS_WINDOW: usize = 0x090;

    // ─── CXWnd struct offsets ───

    /// `CXWnd::WindowText` (`CXStr` at +0x078)
    pub const CXWND_WINDOW_TEXT: usize = 0x078;
    /// `CXWnd::XMLIndex` (int at +0x054 in eqmain, varies)
    pub const CXWND_XML_INDEX: usize = 0x054;
    /// `CXWnd::dShow` (bool at +0x06c)
    pub const CXWND_DSHOW: usize = 0x06c;
    /// `CXWnd::FirstNode` (child window, at +0x028)
    pub const CXWND_FIRST_NODE: usize = 0x028;
    /// `CXWnd::Next` (sibling window, at +0x020)
    pub const CXWND_NEXT: usize = 0x020;

    // ─── CStrRep struct offsets ───

    /// `CStrRep::length` (u32 at +0x08)
    pub const CSTRREP_LENGTH: usize = 0x08;
    /// `CStrRep::alloc` (u32 at +0x04)
    pub const CSTRREP_ALLOC: usize = 0x04;
    /// `CStrRep::encoding` (enum at +0x0c, 0=utf8)
    pub const CSTRREP_ENCODING: usize = 0x0c;
    /// `CStrRep::data` (char[] at +0x18)
    pub const CSTRREP_DATA: usize = 0x18;

    /// Convert a preferred-base eqmain.dll offset to an actual address.
    #[must_use]
    pub fn rebase(preferred_addr: u64, actual_base: u64) -> Option<usize> {
        let offset = preferred_addr.checked_sub(EQMAIN_PREFERRED_BASE)?;
        Some((actual_base + offset) as usize)
    }
}

// ─── eqgame.exe CXWndManager offsets ───
// NOTE: These differ from eqmain.dll! eqgame.exe has CXWndManager::pWindows at +0x008,
// while eqmain.dll has it at +0x010 (different struct layout).
/// Offsets within eqgame.exe (in-game UI and window manager).
pub mod eqgame {
    /// `CXWndManager::pWindows.m_length` in eqgame.exe
    /// `ArrayClass` layout: `m_length` at +0x00, `m_array` at +0x08 within the `ArrayClass`
    /// pWindows `ArrayClass` starts at `CXWndManager` +0x008
    pub const CXWNDMGR_WINDOWS_COUNT: usize = 0x008;
    /// `CXWndManager::pWindows.m_array` in eqgame.exe
    pub const CXWNDMGR_WINDOWS_ARRAY: usize = 0x010;

    /// `CXWnd` vtable offset for `WndNotification` in eqgame.exe context.
    /// `eqgame::CXWnd` has this at 0x120 (vs `eqmain::CXWnd` at 0x110).
    pub const CXWND_VTABLE_WND_NOTIFICATION: usize = 0x120;

    /// `CSidlScreenWnd::SidlText` (`CXStr` at +0x270) — the SIDL window name
    /// Used to find windows like "`CharacterListWnd`" by name
    pub const CSIDL_SCREEN_WND_SIDL_TEXT: usize = 0x270;

    // ─── CListWnd offsets (for character list reading) ───
    // Source: third_party/eqlib/include/eqlib/game/UI.h — CListWnd inherits CSidlScreenWnd

    /// `CListWnd::ItemsArray.m_array` — pointer to `SListWndLine` array (at +0x270)
    pub const CLISTWND_ITEMS_ARRAY: usize = 0x270;
    /// `CListWnd::ItemsArray.m_length` — row count (int at +0x278)
    pub const CLISTWND_ITEMS_COUNT: usize = 0x278;

    /// sizeof(SListWndLine) — each row in the list
    pub const SLISTWNDLINE_SIZE: usize = 0x138;
    /// `SListWndLine::Cells.m_length` (`ArrayClass<SListWndCell>` at +0x00)
    pub const SLISTWNDLINE_CELLS_COUNT: usize = 0x000;
    /// `SListWndLine::Cells.m_array` (pointer at +0x08)
    pub const SLISTWNDLINE_CELLS_ARRAY: usize = 0x008;

    /// sizeof(SListWndCell)
    pub const SLISTWNDCELL_SIZE: usize = 0x28;
    /// `SListWndCell::Text` (`CXStr` at +0x08)
    pub const SLISTWNDCELL_TEXT: usize = 0x08;
}

/// Offsets within `CInvSlotMgr`.
/// Source: mq2-eqlib/include/eqlib/game/UI.h
pub mod inv_slot_mgr {
    /// `CInvSlot* SlotArray[4000]`
    pub const SLOT_ARRAY: usize = 0x0008;
    /// `int TotalSlots`
    pub const TOTAL_SLOTS: usize = 0x7d08;
}

/// Offsets within `CInvSlot`.
/// Source: mq2-eqlib/include/eqlib/game/UI.h
pub mod inv_slot {
    /// `CInvSlotWnd* pInvSlotWnd`
    pub const WINDOW: usize = 0x08;
    /// `int Index`
    pub const INDEX: usize = 0x18;
    /// `bool bEnabled`
    pub const ENABLED: usize = 0x1c;
}

/// Offsets within `CInvSlotWnd`.
/// Source: mq2-eqlib/include/eqlib/game/UI.h
pub mod inv_slot_wnd {
    /// `ItemGlobalIndex ItemLocation`
    pub const ITEM_LOCATION: usize = 0x3f8;
    /// `int Quantity`
    pub const QUANTITY: usize = 0x428;
    /// `bool bSelected`
    pub const SELECTED: usize = 0x42c;
    /// `bool bFindSelected`
    pub const FIND_SELECTED: usize = 0x42d;
    /// `int RecastLeft`
    pub const RECAST_LEFT: usize = 0x430;
    /// `bool bHotButton`
    pub const HOT_BUTTON: usize = 0x434;
    /// `bool bInventorySlotLinked`
    pub const LINKED: usize = 0x435;
    /// `CInvSlot* pInvSlot`
    pub const INV_SLOT: usize = 0x438;
}

/// Offsets within `CContainerWnd`.
/// Source: mq2-eqlib/include/eqlib/game/UI.h
pub mod container_wnd {
    /// `ItemPtr Container`
    pub const CONTAINER: usize = 0x2d0;
    /// `ItemGlobalIndex Location`
    pub const LOCATION: usize = 0x2e0;
    /// `VeArray<CInvSlotWnd*> InvSlotWnds`
    pub const INV_SLOT_WNDS: usize = 0x2f0;
    /// `CLabel* ContainerLabel`
    pub const LABEL: usize = 0x348;
    /// `int ContainerType`
    pub const CONTAINER_TYPE: usize = 0x384;
}

/// Offsets within `ItemGlobalIndex`.
/// Source: mq2-eqlib/include/eqlib/game/Items.h
pub mod item_global_index {
    /// `ItemContainerInstance Location`
    pub const LOCATION: usize = 0x00;
    /// `short Slot1`
    pub const SLOT1: usize = 0x04;
    /// `short Slot2`
    pub const SLOT2: usize = 0x06;
    /// `short Slot3`
    pub const SLOT3: usize = 0x08;
    /// `sizeof(ItemGlobalIndex)`
    pub const SIZE: usize = 0x0c;
}

/// Offsets within `ItemBase`.
/// Source: mq2-eqlib/include/eqlib/game/Items.h
pub mod item_base {
    /// `ItemDefinition* ItemDef`
    pub const ITEM_DEF: usize = 0x068;
    /// `int Charges`
    pub const CHARGES: usize = 0x0b0;
    /// `int ID`
    pub const ID: usize = 0x0b4;
    /// `int StackCount`
    pub const STACK_COUNT: usize = 0x0bc;
    /// `int Open`
    pub const OPEN: usize = 0x0d8;
    /// `ItemGlobalIndex GlobalIndex`
    pub const GLOBAL_INDEX: usize = 0x100;
}

/// Offsets within `ItemDefinition`.
/// Source: mq2-eqlib/include/eqlib/game/Items.h
pub mod item_definition {
    /// `char Name[64]`
    pub const NAME: usize = 0x000;
    /// `int ItemNumber`
    pub const ITEM_NUMBER: usize = 0x0b8;
    /// `int IconNumber`
    pub const ICON_NUMBER: usize = 0x0c4;
    /// `uint8_t Size`
    pub const SIZE: usize = 0x0e8;
    /// `uint8_t Type`
    pub const TYPE: usize = 0x0e9;
    /// `uint8_t ItemClass`
    pub const ITEM_CLASS: usize = 0x190;
    /// `uint8_t Slots`
    pub const CONTAINER_SLOTS: usize = 0x53d;
    /// `uint8_t SizeCapacity`
    pub const SIZE_CAPACITY: usize = 0x53e;
    /// `int StackSize`
    pub const STACK_SIZE: usize = 0x590;
}

// ─── Character select offsets (eqgame.exe) ───

/// `CCharacterListWnd::SelectCharacter` function address (preferred base, eqgame.exe)
pub const SELECT_CHARACTER: u64 = 0x0001_400D_5D20;

/// `CCharacterListWnd::EnterWorld` function address (preferred base, eqgame.exe)
pub const ENTER_WORLD: u64 = 0x0001_400D_4B20;

// ─── PlayerClient (SPAWNINFO) field offsets ───
// These are byte offsets within the PlayerClient struct.
// Source: third_party/eqlib/include/eqlib/game/PlayerClient.h

/// Offsets within `PlayerBase` (base class of `PlayerClient`)
pub mod player_base {
    /// `PlayerClient`* — next spawn in linked list (from `TListNode`)
    /// Note: vtable pointer at 0x00 pushes `TListNode` fields down by 8
    pub const NEXT: usize = 0x10;
    /// `PlayerClient`* — previous spawn in linked list (from `TListNode`)
    pub const PREV: usize = 0x08;

    /// float — Y position
    pub const Y: usize = 0x074;
    /// float — X position
    pub const X: usize = 0x078;
    /// float — Z position
    pub const Z: usize = 0x07c;
    /// float — heading/rotation
    pub const HEADING: usize = 0x090;
    /// char\[64\] — internal name (e.g., "`priest_of_discord00`")
    pub const NAME: usize = 0x0b4;
    /// char\[64\] — displayed name (e.g., "Priest of Discord")
    pub const DISPLAYED_NAME: usize = 0x0f4;
    /// float — `SpeedX` (lateral speed component)
    pub const SPEED_CURRENT: usize = 0x084;
    /// float — `SpeedZ` (vertical speed component)
    pub const SPEED_Z: usize = 0x088;
    /// float — `SpeedRun` (actual movement speed, includes modifiers)
    pub const SPEED_RUN: usize = 0x08c;
    /// float — speed heading (direction of movement)
    pub const SPEED_HEADING: usize = 0x09c;
    /// `uint8_t` — spawn type (PC=0, NPC=1, Corpse=2, etc.)
    pub const TYPE: usize = 0x135;
    /// `uint32_t` — unique spawn ID
    pub const SPAWN_ID: usize = 0x168;
    /// char\[32\] — last name
    pub const LASTNAME: usize = 0x048;
}

/// Buff slot constants and `EQ_Affect` field offsets.
/// Source: third_party/eqlib/include/eqlib/game/Spells.h (`EQ_Affect`),
///         third_party/eqlib/include/eqlib/game/Constants.h (slot counts),
///         third_party/eqlib/include/eqlib/game/PcClient.h (access path).
pub mod buff_slots {
    /// Number of long-duration buff slots.
    pub const NUM_LONG_BUFFS: usize = 62;

    /// Number of short-duration (song/temp) buff slots.
    pub const NUM_SHORT_BUFFS: usize = 31;

    /// Total buff slots (long + short).
    pub const MAX_TOTAL_BUFFS: usize = NUM_LONG_BUFFS + NUM_SHORT_BUFFS; // 93

    /// sizeof(`EQ_Affect`) — each buff entry is 0x98 bytes.
    /// Source: Spells.h `EQ_Affect_size = 0x98`.
    pub const EQ_AFFECT_SIZE: usize = 0x98;

    // ── `EQ_Affect` field offsets ──

    /// `EQ_Affect::SpellID` (i32 at +0x6c). -1 or 0 = empty slot.
    pub const SPELL_ID: usize = 0x6c;

    /// `EQ_Affect::Duration` (i32 at +0x70) — remaining ticks (6 sec/tick).
    pub const DURATION: usize = 0x70;

    /// `EQ_Affect::InitialDuration` (i32 at +0x74) — ticks when applied.
    pub const INITIAL_DURATION: usize = 0x74;

    /// `EQ_Affect::HitCount` (i32 at +0x78) — remaining hit count for limited-hit buffs.
    pub const HIT_COUNT: usize = 0x78;

    /// `EQ_Affect::Modifier` (f32 at +0x80) — bard song modifier (1.0 default).
    pub const MODIFIER: usize = 0x80;

    /// `EQ_Affect::Type` (u8 at +0x90) — buff type (2 = standard buff).
    pub const BUFF_TYPE: usize = 0x90;

    /// `EQ_Affect::Level` (u8 at +0x91) — caster level.
    pub const CASTER_LEVEL: usize = 0x91;

    /// EQ tick duration in seconds.
    pub const SECONDS_PER_TICK: f32 = 6.0;

    // ── Convenience aliases for external buff reads ──

    /// Maximum buff slots to iterate (alias for `MAX_TOTAL_BUFFS`).
    pub const MAX_BUFF_SLOTS: usize = MAX_TOTAL_BUFFS;

    /// Offset from PcProfile to the buff array data (`BaseProfile::Buffs`).
    /// This is `profile::BUFFS_ARRAY` — callers must first dereference the
    /// profile pointer chain to reach the PcProfile base.
    pub const BUFF_ARRAY_OFFSET: usize = super::profile::BUFFS_ARRAY;

    /// Size of each buff entry (alias for `EQ_AFFECT_SIZE`).
    pub const BUFF_ENTRY_SIZE: usize = EQ_AFFECT_SIZE;

    /// Remaining duration in ticks (alias for `DURATION`).
    pub const DURATION_TICKS: usize = DURATION;
}

/// Pointer chain from `PINST_LOCAL_PC` → profile → buff array.
/// Source: PcClient.h (`ProfileManager` at 0x2e48, `GetCurrentProfile()`),
///         PcProfile.h (`BaseProfile::Buffs` at 0x0098, `SoeUtil::Array` layout).
pub mod profile {
    /// `PcClient::ProfileManager` offset within PcClient.
    pub const PROFILE_MANAGER: usize = 0x2e48;

    /// `ProfileManager::pFirst` (ProfileList*) at +0x00.
    pub const PROFILE_LIST_PTR: usize = 0x00;

    /// `ProfileList::pFirst` (PcProfile*) at +0x00.
    pub const PROFILE_FIRST: usize = 0x00;

    /// `BaseProfile::Buffs` (`SoeUtil::Array<EQ_Affect>`) at +0x0098.
    pub const BUFFS_ARRAY: usize = 0x0098;
    /// `BaseProfile::SpellBook` (`int[1280]`) at +0x00b0.
    pub const SPELL_BOOK: usize = 0x00b0;
    /// `BaseProfile::MemorizedSpells` (`int[18]`) at +0x14b0.
    pub const MEMORIZED_SPELLS: usize = 0x14b0;
    /// Number of spellbook slots between `SpellBook` and `MemorizedSpells`.
    pub const SPELL_BOOK_SLOT_COUNT: usize = (MEMORIZED_SPELLS - SPELL_BOOK) / 4;
    /// Visible spell-gem slots used by the live client UI.
    pub const MEMORIZED_SPELL_GEM_COUNT: usize = 15;

    /// Alias: total spellbook slots in `BaseProfile::SpellBook`.
    pub const SPELL_BOOK_SLOTS: usize = 1280;

    /// Alias: visible memorized spell gems in `BaseProfile::MemorizedSpells`.
    pub const MEMORIZED_SPELL_GEMS: usize = 15;

    /// `SoeUtil::Array::m_array` (data pointer) at +0x08 within the array.
    pub const ARRAY_DATA_PTR: usize = 0x08;

    /// `SoeUtil::Array::m_size` (i32 element count) at +0x10 within the array.
    pub const ARRAY_SIZE: usize = 0x10;

    /// `PcClient::BuffIDs` — flat array of `i32[62]` spell IDs for long buffs.
    /// Faster than the full profile chain when only spell IDs are needed.
    pub const BUFF_IDS: usize = 0x068;
}

/// Offsets within `CDisplay`.
/// Source: third_party/eqlib/include/eqlib/game/Display.h
pub mod display {
    /// `uint32_t` — EQ's live millisecond timestamp counter.
    pub const TIME_STAMP: usize = 0x016c;
}

/// Offsets within `CharacterZoneClient` as embedded in `PcClient`.
/// Source: third_party/eqlib/include/eqlib/game/PcClient.h
pub mod character_zone {
    /// `PlayerClient*` — local spawn pointer (`CharacterZoneClient::me`).
    pub const ME: usize = 0x2798;
}

/// Offsets within `LaunchSpellData`.
/// Source: third_party/eqlib/include/eqlib/game/PlayerClient.h
pub mod launch_spell_data {
    /// `int` — active spell ID (`-1` when not casting).
    pub const SPELL_ID: usize = 0x00;
    /// `uint32_t` — target spawn ID for the active cast.
    pub const TARGET_ID: usize = 0x04;
    /// `uint32_t` — client timestamp when the cast lands.
    pub const SPELL_ETA: usize = 0x10;
    /// `int` — casting item ID, if any.
    pub const ITEM_ID: usize = 0x14;
    /// `ItemGlobalIndex` — inventory location for item-origin casts.
    pub const ITEM_LOCATION: usize = 0x2c;
    /// `ItemSpellTypes` — which item spell slot is being activated.
    pub const ITEM_CAST_TYPE: usize = 0x38;
    /// `uint8_t` — spell gem slot (`0xFF` when not using a gem).
    pub const SPELL_SLOT: usize = 0x39;

    /// Sentinel spell ID used by EQ when no cast is active.
    pub const NOT_CASTING_SPELL_ID: i32 = -1;
    /// Sentinel spell slot used by EQ when no spell gem is active.
    pub const NOT_CASTING_SPELL_SLOT: u8 = 0xFF;
}

/// Offsets within `ClientSpellManager`.
/// Source: `third_party/eqlib/include/eqlib/game/Spells.h`
pub mod client_spell_manager {
    /// `int` — largest valid spell ID in the loaded spell database.
    pub const MAX_SPELL_ID: usize = 0x0064;
    /// `SoeUtil::HashMap<int, EQ_Spell>` — loaded spell records keyed by spell ID.
    pub const SPELLS: usize = 0x2240;
}

/// Offsets within `EQ_Spell`.
/// Source: `third_party/eqlib/include/eqlib/game/Spells.h`
pub mod eq_spell {
    /// `uint32_t` — base cast time from spell data (does not include live haste/focus modifiers).
    pub const CAST_TIME: usize = 0x0010;
    /// `int` — spell ID inside the record.
    pub const ID: usize = 0x008c;
    /// `char[64]` — spell name.
    pub const NAME: usize = 0x0192;
    /// `sizeof(EQ_Spell)` on the 2026-03-10 live client.
    pub const SIZE: usize = 0x0218;
}

/// Offsets within `SoeUtil::HashMap<int, EQ_Spell>`.
/// Source: `third_party/eqlib/include/eqlib/game/SoeUtil.h` + `EQ_Spell` size above.
pub mod spell_hash_map {
    const fn align_up(value: usize, alignment: usize) -> usize {
        let remainder = value % alignment;
        if remainder == 0 {
            value
        } else {
            value + (alignment - remainder)
        }
    }

    /// `size_t` — number of entries in the map.
    pub const COUNT: usize = 0x08;
    /// `Node*` — head of the linked list of values.
    pub const HEAD: usize = 0x10;
    /// `Node**` — bucket array for hash lookups.
    pub const BUCKETS: usize = 0x20;
    /// `size_t` — bucket count, always a power of two when populated.
    pub const DYNAMIC_SIZE: usize = 0x28;

    /// `int` — hash node key (`spell_id`).
    pub const KEY: usize = 0x000;
    /// `EQ_Spell` — hash node value payload.
    pub const VALUE: usize = 0x004;
    /// `Node*` — next entry within the same hash bucket.
    pub const HASH_NEXT: usize = align_up(VALUE + super::eq_spell::SIZE, 0x08);
    /// `Node*` — next entry in insertion order.
    pub const NEXT: usize = HASH_NEXT + 0x08;
    /// `Node*` — previous entry in insertion order.
    pub const PREV: usize = NEXT + 0x08;
}

/// Offsets within `PlayerZoneClient` (extends `PlayerBase` at 0x01c8)
pub mod player_zone {
    /// `LaunchSpellData` — persistent cast state snapshot for this spawn.
    pub const CASTING_DATA: usize = 0x01d8;
    /// `int64_t` — maximum HP
    pub const HP_MAX: usize = 0x0338;
    /// `int64_t` — current HP
    pub const HP_CURRENT: usize = 0x03a0;
    /// `int32_t` — maximum mana (only valid for local player; other spawns have garbage)
    pub const MANA_MAX: usize = 0x03ac;
    /// `int32_t` — current mana (only valid for local player; other spawns have garbage)
    pub const MANA_CURRENT: usize = 0x03fc;
    /// `uint8_t` — character level
    pub const LEVEL: usize = 0x03ef;
    /// `uint8_t` — standing state (0=standing, 1=frozen, 2=looting, 3=sitting, 4=ducking, 110=feigned, 111=dead)
    /// Source: `PlayerZoneClient` offset 0x0574 in PlayerClient.h
    /// TODO: 0x0574 reads 110 (FD) when character is sitting on March 10, 2026 build.
    /// Needs hex dump calibration scan on a live client to find correct offset.
    /// Old offset 0x0134 always read 0 (Standing). Neither is correct.
    pub const STANDSTATE: usize = 0x0574;
    /// char — GM flag (nonzero = GM). Source: PlayerClient.h offset 0x03ec
    pub const GM: usize = 0x03ec;
    /// `uint8_t` — GM rank. Source: PlayerClient.h offset 0x0368
    pub const GM_RANK: usize = 0x0368;
    /// `uint32_t`\[15\] — per-gem recast timestamps.
    pub const SPELL_GEM_ETA: usize = 0x03b0;
    /// `uint8_t` — character class ID (1=WAR, 2=CLR, ..., 16=BER)
    /// Source: `PlayerZoneClient` offset 0x0420 in PlayerClient.h
    /// This is the direct field — more reliable than the `ActorClient` path (0x0FDC)
    /// which requires traversing through mActorClient at 0x0FC0.
    pub const CHAR_CLASS: usize = 0x0420;
    /// `int32_t` — current endurance
    pub const ENDURANCE_CURRENT: usize = 0x04f8;
    /// `uint32_t` — maximum endurance
    pub const ENDURANCE_MAX: usize = 0x0538;
    /// float — melee range radius
    /// Source: PlayerClient.h offset 0x11D8
    pub const MELEE_RADIUS: usize = 0x11D8;
}

/// Offsets within `ActorClient` (embedded in `PlayerZoneClient` at 0x0FC0)
pub mod actor_client {
    /// `int32_t` — race ID (from `ActorBase` at offset 0x14)
    pub const RACE: usize = 0x0FD4;
    /// `int32_t` — race override (illusions, etc.)
    pub const RACE_OVERRIDE: usize = 0x0FD8;
    /// `int32_t` — character class ID (from `ActorBase` at offset 0x1C)
    /// Source: `ActorClient` at 0x0FC0 + ActorBase.Class at 0x1C = 0x0FDC
    /// Note: Prefer `player_zone::CHAR_CLASS` (0x0420) for spawn reads — it's a
    /// direct field and less likely to break if struct layout shifts.
    pub const CHAR_CLASS: usize = 0x0FDC;
}

/// Group-related offsets
/// Source: third_party/eqlib/include/eqlib/game/PcClient.h
pub mod group {
    /// Offset of `CGroup`* pointer within `PcClient` struct
    /// PcClient.Group at 0x2EB0
    pub const PC_CLIENT_GROUP_PTR: usize = 0x2EB0;

    /// `MAX_GROUP_SIZE` = 6 (including self)
    pub const MAX_GROUP_SIZE: usize = 6;

    // ─── CGroupBase layout (vtable at 0x00) ───
    /// `CGroupMember*` `m_groupMembers`\[6\] — array of 6 member pointers
    pub const GROUP_MEMBERS: usize = 0x08;
    /// `CGroupMember`* `m_groupLeader` — pointer to leader member
    pub const GROUP_LEADER: usize = 0x38;
    /// `uint32_t` `m_id` — group ID
    pub const GROUP_ID: usize = 0x40;

    // ─── CGroupMemberBase layout (vtable at 0x00) ───
    /// `CXStr` Name — member name (`CXStr` = pointer to `CStrRep`)
    pub const MEMBER_NAME_CXSTR: usize = 0x08;
    /// short Type — player type (PC=0, NPC=1, etc.)
    pub const MEMBER_TYPE: usize = 0x10;
    /// `CXStr` `OwnerName` — mercenary owner name
    pub const MEMBER_OWNER_CXSTR: usize = 0x18;
    /// int Level
    pub const MEMBER_LEVEL: usize = 0x20;
    /// bool bIsOffline
    pub const MEMBER_IS_OFFLINE: usize = 0x24;

    // ─── CXStr / CStrRep layout ───
    /// `CXStr` is a single pointer to `CStrRep` (`m_data` at offset 0x00)
    /// CStrRep.utf8 string data starts at offset 0x18
    pub const CXSTR_REP_UTF8: usize = 0x18;
}

/// Zone info offsets (zoneHeader / ZONEINFO struct)
/// Source: third_party/eqlib/include/eqlib/game/EverQuest.h (zoneHeader)
pub mod zone_info {
    /// Address of the zoneHeader struct (instEQZoneInfo).
    /// This is NOT a pointer — it's the struct itself at this address.
    pub const INST_EQ_ZONE_INFO: u64 = 0x0001_40E9_5CD4;

    /// char\[128\] — zone short name (e.g., "qey2hh1")
    pub const SHORT_NAME: usize = 0x000;

    /// char\[128\] — zone long name (e.g., "Queynos Hills")
    pub const LONG_NAME: usize = 0x080;
}

/// Offsets within `SpawnManager` (`PlayerManagerBase`)
pub mod spawn_manager {
    /// `TList`<`PlayerClient`*> — start of the player linked list
    /// The `TList` itself contains `m_pFirstNode` at offset 0x00
    pub const PLAYER_LIST: usize = 0x0010;
}

/// `ZoneGuideManagerClient` / `ZoneGuideZone` struct layout offsets.
/// Source: mq2-eqlib/include/eqlib/game/UI.h, Containers.h
pub mod zone_guide {
    /// Number of zone slots in the fixed array.
    pub const ZONE_COUNT: usize = 888;

    // ─── ZoneGuideManagerBase layout ───
    // vtable at +0x00 (8 bytes), zones array starts at +0x08

    /// Offset of `zones`\[0\] within `ZoneGuideManagerBase`.
    pub const ZONES_OFFSET: usize = 0x0008;

    // ─── ZoneGuideZone layout (0x48 bytes each) ───

    /// sizeof(ZoneGuideZone)
    pub const ZONE_SIZE: usize = 0x48;

    /// `EQZoneIndex` zoneId (int at +0x00)
    pub const ZONE_ID: usize = 0x00;
    /// `CXStr` name (pointer at +0x08)
    pub const ZONE_NAME: usize = 0x08;
    /// int continentIndex (+0x10)
    pub const ZONE_CONTINENT: usize = 0x10;
    /// int minLevel (+0x14)
    pub const ZONE_MIN_LEVEL: usize = 0x14;
    /// int maxLevel (+0x18)
    pub const ZONE_MAX_LEVEL: usize = 0x18;
    /// `ArrayClass<ZoneGuideConnection>` zoneConnections at +0x30
    /// `ArrayClass` layout: `m_length` (int) at +0x00, `m_array` (ptr) at +0x08
    pub const ZONE_CONNECTIONS_COUNT: usize = 0x30;
    /// ArrayClass connections pointer at +0x38
    pub const ZONE_CONNECTIONS_ARRAY: usize = 0x38;

    // ─── ZoneGuideConnection layout (0x14 bytes each) ───

    /// sizeof(ZoneGuideConnection)
    pub const CONNECTION_SIZE: usize = 0x14;

    /// `EQZoneIndex` destZoneId (int at +0x00)
    pub const CONN_DEST_ZONE_ID: usize = 0x00;
    /// int transferTypeIndex (+0x04)
    pub const CONN_TRANSFER_TYPE: usize = 0x04;
    /// bool disabled (+0x10)
    pub const CONN_DISABLED: usize = 0x10;

    // ─── ZoneGuideManagerClient extends ZoneGuideManagerBase ───

    /// `EQZoneIndex` currentZone at +0xFA40
    pub const CURRENT_ZONE: usize = 0xFA40;
    /// bool zoneGuideDataSet at +0xFA48
    pub const DATA_SET: usize = 0xFA48;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebase_normal_case() {
        let actual_base: u64 = 0x7FF600000000;
        let result = rebase(PINST_LOCAL_PLAYER, actual_base);
        let expected_offset = PINST_LOCAL_PLAYER - EQ_PREFERRED_BASE;
        assert_eq!(result, Some((actual_base + expected_offset) as usize));
    }

    #[test]
    fn rebase_underflow_returns_none() {
        // An address below the preferred base should return None
        let result = rebase(0x100, 0x7FF600000000);
        assert_eq!(result, None);
    }

    #[test]
    fn rebase_same_base_returns_original_offset() {
        let result = rebase(PINST_LOCAL_PLAYER, EQ_PREFERRED_BASE);
        assert_eq!(result, Some(PINST_LOCAL_PLAYER as usize));
    }

    #[test]
    fn rebase_preferred_base_itself_returns_actual_base() {
        let actual_base: u64 = 0x7FF600000000;
        let result = rebase(EQ_PREFERRED_BASE, actual_base);
        assert_eq!(result, Some(actual_base as usize));
    }

    #[test]
    fn rebase_zero_actual_base_returns_offset() {
        let result = rebase(PINST_LOCAL_PLAYER, 0);
        let expected_offset = PINST_LOCAL_PLAYER - EQ_PREFERRED_BASE;
        assert_eq!(result, Some(expected_offset as usize));
    }

    #[test]
    fn rebase_all_globals_succeed_with_realistic_base() {
        let actual_base: u64 = 0x7FF600000000;
        let globals = [
            PINST_LOCAL_PLAYER,
            PINST_CONTROLLED_PLAYER,
            PINST_TARGET,
            PINST_SPAWN_MANAGER,
            PINST_LOCAL_PC,
            PINST_SPELL_MANAGER,
            PINST_CDISPLAY,
            PINST_CEVERQUEST,
        ];
        for addr in &globals {
            let result = rebase(*addr, actual_base);
            assert!(result.is_some(), "rebase failed for 0x{:X}", addr);
        }
    }

    #[test]
    fn rebase_all_function_addresses_succeed() {
        let actual_base: u64 = 0x7FF600000000;
        let funcs = [
            CAST_SPELL,
            DO_COMBAT_ABILITY,
            USE_SKILL,
            CAN_USE_ITEM,
            DO_ATTACK,
            EXECUTE_CMD,
            INTERPRET_CMD,
            PROCESS_GAME_EVENTS,
            DSP_CHAT,
            CLICKED_PLAYER,
            ISSUE_PET_COMMAND,
            DO_LOOT,
        ];
        for addr in &funcs {
            let result = rebase(*addr, actual_base);
            assert!(result.is_some(), "rebase failed for func 0x{:X}", addr);
        }
    }

    #[test]
    fn eqmain_rebase_normal_case() {
        let actual_base: u64 = 0x7FFA00000000;
        let result = eqmain::rebase(eqmain::SIDL_MANAGER, actual_base);
        let expected_offset = eqmain::SIDL_MANAGER - eqmain::EQMAIN_PREFERRED_BASE;
        assert_eq!(result, Some((actual_base + expected_offset) as usize));
    }

    #[test]
    fn eqmain_rebase_underflow_returns_none() {
        let result = eqmain::rebase(0x100, 0x7FFA00000000);
        assert_eq!(result, None);
    }

    #[test]
    fn eqmain_rebase_preferred_base_returns_actual_base() {
        let actual_base: u64 = 0x7FFA00000000;
        let result = eqmain::rebase(eqmain::EQMAIN_PREFERRED_BASE, actual_base);
        assert_eq!(result, Some(actual_base as usize));
    }

    #[test]
    fn eqmain_rebase_all_pointers_succeed() {
        let actual_base: u64 = 0x7FFA00000000;
        let ptrs = [
            eqmain::SIDL_MANAGER,
            eqmain::LOGIN_SERVER_API,
            eqmain::CXWND_MANAGER,
            eqmain::JOIN_SERVER,
            eqmain::LOGIN_VIEW_MANAGER,
            eqmain::PINST_LOGIN_CLIENT,
            eqmain::PINST_LOGIN_CONTROLLER,
        ];
        for addr in &ptrs {
            let result = eqmain::rebase(*addr, actual_base);
            assert!(result.is_some(), "eqmain rebase failed for 0x{:X}", addr);
        }
    }

    #[test]
    fn eqmain_and_eqgame_have_different_preferred_bases() {
        assert_ne!(EQ_PREFERRED_BASE, eqmain::EQMAIN_PREFERRED_BASE);
    }

    #[test]
    fn all_globals_above_preferred_base() {
        let globals = [
            PINST_LOCAL_PLAYER,
            PINST_CONTROLLED_PLAYER,
            PINST_TARGET,
            PINST_SPAWN_MANAGER,
            PINST_LOCAL_PC,
            PINST_SPELL_MANAGER,
            PINST_CDISPLAY,
            PINST_CEVERQUEST,
            PINST_CXWND_MANAGER,
            PINST_ACTIVE_CORPSE,
            PINST_CCHAT_WINDOW_MANAGER,
            PINST_CINV_SLOT_MGR,
        ];
        for addr in &globals {
            assert!(
                *addr > EQ_PREFERRED_BASE,
                "Global 0x{:X} should be above preferred base",
                addr
            );
        }
    }

    #[test]
    fn all_function_addresses_above_preferred_base() {
        let funcs = [
            CAST_SPELL,
            DO_COMBAT_ABILITY,
            USE_SKILL,
            CAN_USE_ITEM,
            DO_ATTACK,
            EXECUTE_CMD,
            INTERPRET_CMD,
            PROCESS_GAME_EVENTS,
            REAL_RENDER_WORLD,
            CLICKED_PLAYER,
            ISSUE_PET_COMMAND,
            GET_CON_LEVEL,
            GET_PC_CLIENT,
            DO_LOOT,
            FIX_HEADING,
            GET_BEARING,
            FREE_TARGET_CAST_SPELL,
            CHANGE_HEIGHT,
            ZONE_GUIDE_MANAGER,
            CHAR_LIST_ENTER_WORLD,
            CHAR_LIST_SELECT_CHAR,
            CCHAT_MGR_GET_RGBA,
            CCHAT_MGR_INIT_CONTEXT_MENU,
            CCHAT_MGR_FREE_CHAT_WINDOW,
            CCHAT_MGR_SET_LOCKED_ACTIVE_CHAT,
            CCHAT_MGR_CREATE_CHAT_WINDOW,
            INV_SLOT_MGR_FIND_SLOT,
            INV_SLOT_MGR_MOVE_ITEM,
            INV_SLOT_MGR_SELECT_SLOT,
            SPELL_BOOK_WND_MEMORIZE_SET,
            DSP_CHAT,
        ];
        for addr in &funcs {
            assert!(
                *addr > EQ_PREFERRED_BASE,
                "Function 0x{:X} should be above preferred base",
                addr
            );
        }
    }

    #[test]
    fn eqmain_all_addresses_above_eqmain_base() {
        let addrs = [
            eqmain::SIDL_MANAGER,
            eqmain::LOGIN_SERVER_API,
            eqmain::CXWND_MANAGER,
            eqmain::JOIN_SERVER,
            eqmain::LOGIN_VIEW_MANAGER,
            eqmain::PINST_LOGIN_CLIENT,
            eqmain::PINST_LOGIN_CONTROLLER,
        ];
        for addr in &addrs {
            assert!(
                *addr > eqmain::EQMAIN_PREFERRED_BASE,
                "eqmain 0x{:X} should be above eqmain preferred base",
                addr
            );
        }
    }

    #[test]
    fn player_base_offsets_are_ordered() {
        const _: () = {
            assert!(player_base::NAME < player_base::DISPLAYED_NAME);
            assert!(player_base::Y < player_base::X);
            assert!(player_base::X < player_base::Z);
        };
    }

    #[test]
    fn buff_slots_constants_match_eqlib() {
        assert_eq!(buff_slots::NUM_LONG_BUFFS, 62);
        assert_eq!(buff_slots::NUM_SHORT_BUFFS, 31);
        assert_eq!(buff_slots::MAX_TOTAL_BUFFS, 93);
        assert_eq!(buff_slots::EQ_AFFECT_SIZE, 0x98);
        assert_eq!(buff_slots::SPELL_ID, 0x6c);
        assert_eq!(buff_slots::DURATION, 0x70);
        assert_eq!(buff_slots::INITIAL_DURATION, 0x74);
        assert_eq!(buff_slots::HIT_COUNT, 0x78);
        assert_eq!(buff_slots::CASTER_LEVEL, 0x91);
        assert_eq!(buff_slots::BUFF_TYPE, 0x90);
    }

    #[test]
    fn profile_offsets_consistent() {
        const _: () = assert!(profile::PROFILE_MANAGER > 0);
        assert_eq!(profile::BUFFS_ARRAY, 0x0098);
        assert_eq!(profile::SPELL_BOOK, 0x00b0);
        assert_eq!(profile::MEMORIZED_SPELLS, 0x14b0);
        assert_eq!(profile::SPELL_BOOK_SLOT_COUNT, 1280);
        assert_eq!(profile::MEMORIZED_SPELL_GEM_COUNT, 15);
        assert_eq!(profile::ARRAY_DATA_PTR, 0x08);
        assert_eq!(profile::ARRAY_SIZE, 0x10);
        assert_eq!(profile::BUFF_IDS, 0x068);
    }

    #[test]
    fn display_timestamp_offset_matches_eqlib_live_20260310() {
        assert_eq!(display::TIME_STAMP, 0x016c);
    }

    #[test]
    fn launch_spell_data_offsets_match_eqlib_live_20260310() {
        assert_eq!(launch_spell_data::SPELL_ID, 0x00);
        assert_eq!(launch_spell_data::TARGET_ID, 0x04);
        assert_eq!(launch_spell_data::SPELL_ETA, 0x10);
        assert_eq!(launch_spell_data::ITEM_ID, 0x14);
        assert_eq!(launch_spell_data::ITEM_LOCATION, 0x2c);
        assert_eq!(launch_spell_data::ITEM_CAST_TYPE, 0x38);
        assert_eq!(launch_spell_data::SPELL_SLOT, 0x39);
        assert_eq!(launch_spell_data::NOT_CASTING_SPELL_ID, -1);
        assert_eq!(launch_spell_data::NOT_CASTING_SPELL_SLOT, 0xFF);
    }

    #[test]
    fn player_zone_casting_offsets_match_eqlib_live_20260310() {
        assert_eq!(player_zone::CASTING_DATA, 0x01d8);
        assert_eq!(player_zone::SPELL_GEM_ETA, 0x03b0);
    }

    #[test]
    fn character_zone_me_offset_matches_eqlib_live_20260310() {
        assert_eq!(character_zone::ME, 0x2798);
    }

    #[test]
    fn spell_manager_offsets_match_eqlib_live_20260310() {
        assert_eq!(client_spell_manager::MAX_SPELL_ID, 0x0064);
        assert_eq!(client_spell_manager::SPELLS, 0x2240);
    }

    #[test]
    fn eq_spell_offsets_match_eqlib_live_20260310() {
        assert_eq!(eq_spell::CAST_TIME, 0x0010);
        assert_eq!(eq_spell::ID, 0x008c);
        assert_eq!(eq_spell::NAME, 0x0192);
        assert_eq!(eq_spell::SIZE, 0x0218);
    }

    #[test]
    fn spell_hash_map_offsets_match_eqlib_layout() {
        assert_eq!(spell_hash_map::COUNT, 0x08);
        assert_eq!(spell_hash_map::HEAD, 0x10);
        assert_eq!(spell_hash_map::BUCKETS, 0x20);
        assert_eq!(spell_hash_map::DYNAMIC_SIZE, 0x28);
        assert_eq!(spell_hash_map::KEY, 0x000);
        assert_eq!(spell_hash_map::VALUE, 0x004);
        assert_eq!(spell_hash_map::HASH_NEXT, 0x220);
        assert_eq!(spell_hash_map::NEXT, 0x228);
        assert_eq!(spell_hash_map::PREV, 0x230);
    }

    #[test]
    fn zone_guide_constants_consistent() {
        assert_eq!(zone_guide::ZONE_COUNT, 888);
        const _: () = {
            assert!(zone_guide::ZONE_SIZE > 0);
            assert!(zone_guide::CONNECTION_SIZE > 0);
            assert!(zone_guide::ZONE_CONNECTIONS_COUNT < zone_guide::ZONE_SIZE);
        };
    }

    #[test]
    fn group_constants_valid() {
        assert_eq!(group::MAX_GROUP_SIZE, 6);
        const _: () = {
            assert!(group::PC_CLIENT_GROUP_PTR > 0);
            assert!(group::GROUP_MEMBERS < group::GROUP_LEADER);
        };
    }

    #[test]
    fn eqmain_vtable_offsets_differ_from_eqgame() {
        // eqmain::CXWnd has WndNotification at 0x110, eqgame at 0x120
        assert_ne!(
            eqmain::CXWND_VTABLE_WND_NOTIFICATION,
            eqgame::CXWND_VTABLE_WND_NOTIFICATION
        );
    }

    #[test]
    fn rebase_with_large_offset() {
        // ZONE_GUIDE_MANAGER is a high address
        let actual_base: u64 = 0x7FF600000000;
        let result = rebase(ZONE_GUIDE_MANAGER, actual_base);
        assert!(result.is_some());
        let addr = result.unwrap();
        assert!(addr > actual_base as usize);
    }

    #[test]
    fn duplicate_constants_match() {
        // SELECT_CHARACTER and CHAR_LIST_SELECT_CHAR should be the same
        assert_eq!(SELECT_CHARACTER, CHAR_LIST_SELECT_CHAR);
        assert_eq!(ENTER_WORLD, CHAR_LIST_ENTER_WORLD);
    }

    #[test]
    fn cchat_window_manager_pointer_above_preferred_base() {
        const _: () = assert!(PINST_CCHAT_WINDOW_MANAGER > EQ_PREFERRED_BASE);
    }

    #[test]
    fn cinv_slot_mgr_pointer_above_preferred_base() {
        const _: () = assert!(PINST_CINV_SLOT_MGR > EQ_PREFERRED_BASE);
    }

    #[test]
    fn cchat_mgr_function_addresses_match_eqgame_20260310() {
        assert_eq!(CCHAT_MGR_GET_RGBA, 0x0001_403B_2D40);
        assert_eq!(CCHAT_MGR_INIT_CONTEXT_MENU, 0x0001_403B_2ED0);
        assert_eq!(CCHAT_MGR_FREE_CHAT_WINDOW, 0x0001_403B_1D40);
        assert_eq!(CCHAT_MGR_SET_LOCKED_ACTIVE_CHAT, 0x0001_403B_B240);
        assert_eq!(CCHAT_MGR_CREATE_CHAT_WINDOW, 0x0001_403B_1780);
    }

    #[test]
    fn inv_slot_mgr_function_addresses_match_eqgame_20260310() {
        assert_eq!(INV_SLOT_MGR_FIND_SLOT, 0x0001_4042_1100);
        assert_eq!(INV_SLOT_MGR_MOVE_ITEM, 0x0001_4042_1C90);
        assert_eq!(INV_SLOT_MGR_SELECT_SLOT, 0x0001_4042_3FC0);
        assert_eq!(INV_SLOT_GET_ITEM_BASE, 0x0001_4041_9520);
    }

    #[test]
    fn spell_book_wnd_function_address_matches_eqgame_20260310() {
        assert_eq!(SPELL_BOOK_WND_MEMORIZE_SET, 0x0001_4050_EFE0);
    }

    #[test]
    fn new_function_addresses_all_rebase_successfully() {
        let actual_base: u64 = 0x7FF600000000;
        let funcs = [
            CCHAT_MGR_GET_RGBA,
            CCHAT_MGR_INIT_CONTEXT_MENU,
            CCHAT_MGR_FREE_CHAT_WINDOW,
            CCHAT_MGR_SET_LOCKED_ACTIVE_CHAT,
            CCHAT_MGR_CREATE_CHAT_WINDOW,
            INV_SLOT_MGR_FIND_SLOT,
            INV_SLOT_MGR_MOVE_ITEM,
            INV_SLOT_MGR_SELECT_SLOT,
            SPELL_BOOK_WND_MEMORIZE_SET,
        ];
        for addr in &funcs {
            let result = rebase(*addr, actual_base);
            assert!(result.is_some(), "rebase failed for func 0x{:X}", addr);
        }
    }
}
