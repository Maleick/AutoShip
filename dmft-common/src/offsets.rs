// EQ global pointer addresses from eqlib/offsets/eqgame.h
// These are PREFERRED 64-bit addresses (base 0x140000000).
// At runtime, subtract the preferred base and add the actual base
// (obtained via GetModuleInformation or EnumProcessModules).
//
// Source: mq2-reference/src/eqlib/include/eqlib/offsets/eqgame.h
// Client date: 20260310 (March 10, 2026)

/// Preferred base address of eqgame.exe (64-bit)
pub const EQ_PREFERRED_BASE: u64 = 0x140000000;

/// Pointer to local player (PlayerClient*)
pub const PINST_LOCAL_PLAYER: u64 = 0x140E8E380;

/// Pointer to controlled player (PlayerClient*)
pub const PINST_CONTROLLED_PLAYER: u64 = 0x140E8E430;

/// Pointer to current target (PlayerClient*)
pub const PINST_TARGET: u64 = 0x140E8E428;

/// Pointer to spawn manager (PlayerManagerClient*)
pub const PINST_SPAWN_MANAGER: u64 = 0x140F0CD90;

/// Pointer to local PC data (PcClient*)
pub const PINST_LOCAL_PC: u64 = 0x140E909A8;

/// Pointer to spell manager
pub const PINST_SPELL_MANAGER: u64 = 0x140F0E6F0;

/// Pointer to CDisplay
pub const PINST_CDISPLAY: u64 = 0x140E8E450;

/// Pointer to CEverQuest
pub const PINST_CEVERQUEST: u64 = 0x140F11758;

// ─── EQ Internal Function Addresses ───
// These are preferred-base addresses for EQ's internal functions.
// Used for calling game functions directly from the injected DLL.
// Source: macroquest/eqlib live branch, client date 20260310
// Calling convention: x64 MSVC (this in RCX for member functions)

/// CharacterZoneClient::CastSpell — cast a spell by gem ID
/// Signature: unsigned char CastSpell(unsigned char gemid, int spellid, ...)
pub const CAST_SPELL: u64 = 0x1400D9F20;

/// PcZoneClient::DoCombatAbility — use a combat ability
/// Signature: bool DoCombatAbility(int spellID, bool allowLowerRank)
pub const DO_COMBAT_ABILITY: u64 = 0x1402ED490;

/// CharacterZoneClient::UseSkill — use a skill on a target
/// Signature: void UseSkill(unsigned char skill, PlayerZoneClient* Target, bool bAuto)
pub const USE_SKILL: u64 = 0x1401052A0;

/// CharacterZoneClient::CanUseItem — check if an item is usable
pub const CAN_USE_ITEM: u64 = 0x1400EDDB0;

/// PlayerZoneClient::DoAttack — perform a melee attack
/// Signature: bool DoAttack(BYTE slot, BYTE skill, PlayerZoneClient* Target, ...)
pub const DO_ATTACK: u64 = 0x14031B890;

/// __ExecuteCmd — execute any EQ command by command ID (most versatile)
/// Can do: follow, stopcast, face, sit, stand, attack, etc.
pub const EXECUTE_CMD: u64 = 0x1402235B0;

/// CEverQuest::InterpretCmd — interpret a slash command string
/// Signature: void InterpretCmd(PlayerClient*, const char*)
pub const INTERPRET_CMD: u64 = 0x140283FB0;

/// pinstCXWndManager — eqgame.exe's UI window manager (not eqmain.dll's)
pub const PINST_CXWND_MANAGER: u64 = 0x140F37B28;

/// CCharacterListWnd::EnterWorld — enter world from character select
/// Signature: void EnterWorld() (member function, takes this only)
pub const CHAR_LIST_ENTER_WORLD: u64 = 0x1400D4B20;

/// CCharacterListWnd::SelectCharacter — select a character by index
/// Signature: void SelectCharacter(int index)
pub const CHAR_LIST_SELECT_CHAR: u64 = 0x1400D5D20;

/// CEverQuest::ClickedPlayer — click-target a player
pub const CLICKED_PLAYER: u64 = 0x1402724F0;

/// CEverQuest::IssuePetCommand — issue a pet command
/// Signature: void IssuePetCommand(ePetCommandType, int TargetID, bool bQuiet, ...)
pub const ISSUE_PET_COMMAND: u64 = 0x1402856A0;

/// PcClient::GetConLevel — get consider level of target
pub const GET_CON_LEVEL: u64 = 0x1402E3C10;

/// PlayerClient::GetPcClient — get PcClient from PlayerClient
pub const GET_PC_CLIENT: u64 = 0x140307970;

/// __ProcessGameEvents — game event processing (hook point for game loop)
pub const PROCESS_GAME_EVENTS: u64 = 0x14028E0F0;

/// CDisplay::RealRender_World — render loop (alternative hook point)
pub const REAL_RENDER_WORLD: u64 = 0x1401A4320;

/// __FixHeading — normalize heading value
pub const FIX_HEADING: u64 = 0x140661520;

/// __get_bearing — calculate bearing between two points
pub const GET_BEARING: u64 = 0x140258850;

/// FreeTargetTracker::CastSpell — ground-targeted spell casting
pub const FREE_TARGET_CAST_SPELL: u64 = 0x1402B5740;

/// PlayerZoneClient::ChangeHeight — change character height
pub const CHANGE_HEIGHT: u64 = 0x14031AB80;

/// Convert a preferred-base offset to an actual address given the runtime base.
///
/// Returns `None` if `preferred_addr` is below `EQ_PREFERRED_BASE` (would underflow).
pub fn rebase(preferred_addr: u64, actual_base: u64) -> Option<usize> {
    let offset = preferred_addr.checked_sub(EQ_PREFERRED_BASE)?;
    Some((actual_base + offset) as usize)
}

// ─── eqmain.dll offsets ───
// These are preferred-base addresses within eqmain.dll (loaded into eqgame.exe process).
// eqmain.dll has its own base address; use `eqmain::rebase()` to convert.
// Source: MQ2 AutoLogin / eqmain offsets, client date 20260310

pub mod eqmain {
    /// Preferred base address of eqmain.dll (64-bit)
    pub const EQMAIN_PREFERRED_BASE: u64 = 0x180000000;

    // ─── Global pointer addresses (preferred base) ───

    /// Pointer to CSidlManager instance
    pub const SIDL_MANAGER: u64 = 0x1803824C0;

    /// Pointer to LoginServerAPI instance
    pub const LOGIN_SERVER_API: u64 = 0x18017F4D0;

    /// Pointer to CXWndManager instance
    pub const CXWND_MANAGER: u64 = 0x1803824B8;

    /// LoginServerAPI::JoinServer function address
    pub const JOIN_SERVER: u64 = 0x180018050;

    /// LoginViewManager function address
    pub const LOGIN_VIEW_MANAGER: u64 = 0x18001B0E0;

    // ─── Login pointer addresses (preferred base) ───

    /// Pointer to LoginClient instance (LoginClient*)
    /// Source: eqmain.h EQMain__pinstLoginClient_x (derived from pinstCLoginViewManager - 8)
    pub const PINST_LOGIN_CLIENT: u64 = 0x18017F4E0;

    /// Pointer to LoginController instance
    /// Source: eqmain.h EQMain__pinstLoginController_x
    pub const PINST_LOGIN_CONTROLLER: u64 = 0x18017F4F0;

    // ─── LoginClient struct field offsets ───

    /// LoginClient::pLoginData (EQLS::EQLogin*) at offset 0x010
    pub const LOGINCLIENT_LOGIN_DATA: usize = 0x010;

    /// LoginClient::ServerList (DoublyLinkedList) at offset 0x178
    pub const LOGINCLIENT_SERVER_LIST: usize = 0x178;

    // ─── EQLogin struct field offsets ───

    /// EQLogin::hEQWnd (HWND) at offset 0x408
    pub const EQLOGIN_HWND: usize = 0x408;

    /// EQLogin::Login[0x80] (char array) at offset 0x414
    pub const EQLOGIN_USERNAME: usize = 0x414;

    /// EQLogin::PW[0x80] (char array) at offset 0x494
    pub const EQLOGIN_PASSWORD: usize = 0x494;

    /// EQLogin::Character[0x40] (char array) at offset 0x97C
    pub const EQLOGIN_CHARACTER: usize = 0x97C;

    /// Maximum length of login/password fields (0x80 = 128 bytes, use 0x7F for null terminator)
    pub const EQLOGIN_FIELD_MAX: usize = 0x7F;

    // ─── UI widget field offsets ───

    /// CEditBaseWnd::InputText field offset (CXStr)
    /// CXStr is a single pointer to CStrRep (8 bytes).
    pub const CEDITBASEWND_INPUT_TEXT: usize = 0x278;

    /// XWM_LCLICK notification code for button clicks
    pub const XWM_LCLICK: u32 = 1;

    /// CXWnd vtable offset for SetWindowText (virtual void SetWindowText(const CXStr&))
    /// From MQ2: CXWnd vtable layout has SetWindowText at /*0x280*/
    pub const CXWND_VTABLE_SET_WINDOW_TEXT: usize = 0x280;

    /// CXWnd vtable offset for WndNotification (eqmain.dll layout)
    /// Signature: int WndNotification(CXWnd* sender, uint32_t message, void* data)
    pub const CXWND_VTABLE_WND_NOTIFICATION: usize = 0x110;

    // ─── CXWndManager struct offsets ───
    // From MQ2: CXWndManager { /*0x008*/ ArrayClass<CXWnd*> pWindows; ... }
    // ArrayClass<T> = { T* m_array; int m_length; int m_alloc; }

    /// CXWndManager::pWindows.m_array (pointer to CXWnd* array)
    /// NOTE: eqmain.dll layout differs from eqgame.exe — calibrated from hex dump
    pub const CXWNDMGR_WINDOWS_ARRAY: usize = 0x010;
    /// CXWndManager::pWindows.m_length (window count, u32)
    pub const CXWNDMGR_WINDOWS_COUNT: usize = 0x018;
    /// CXWndManager::FocusWindow (CXWnd*)
    pub const CXWNDMGR_FOCUS_WINDOW: usize = 0x090;

    // ─── CXWnd struct offsets ───

    /// CXWnd::WindowText (CXStr at +0x078)
    pub const CXWND_WINDOW_TEXT: usize = 0x078;
    /// CXWnd::XMLIndex (int at +0x054 in eqmain, varies)
    pub const CXWND_XML_INDEX: usize = 0x054;
    /// CXWnd::dShow (bool at +0x06c)
    pub const CXWND_DSHOW: usize = 0x06c;
    /// CXWnd::FirstNode (child window, at +0x028)
    pub const CXWND_FIRST_NODE: usize = 0x028;
    /// CXWnd::Next (sibling window, at +0x020)
    pub const CXWND_NEXT: usize = 0x020;

    // ─── CStrRep struct offsets ───

    /// CStrRep::length (u32 at +0x08)
    pub const CSTRREP_LENGTH: usize = 0x08;
    /// CStrRep::alloc (u32 at +0x04)
    pub const CSTRREP_ALLOC: usize = 0x04;
    /// CStrRep::encoding (enum at +0x0c, 0=utf8)
    pub const CSTRREP_ENCODING: usize = 0x0c;
    /// CStrRep::data (char[] at +0x18)
    pub const CSTRREP_DATA: usize = 0x18;

    /// Convert a preferred-base eqmain.dll offset to an actual address.
    pub fn rebase(preferred_addr: u64, actual_base: u64) -> Option<usize> {
        let offset = preferred_addr.checked_sub(EQMAIN_PREFERRED_BASE)?;
        Some((actual_base + offset) as usize)
    }
}

// ─── eqgame.exe CXWndManager offsets ───
// NOTE: These differ from eqmain.dll! eqgame.exe has CXWndManager::pWindows at +0x008,
// while eqmain.dll has it at +0x010 (different struct layout).
pub mod eqgame {
    /// CXWndManager::pWindows.m_array in eqgame.exe (ArrayClass at +0x008)
    pub const CXWNDMGR_WINDOWS_ARRAY: usize = 0x008;
    /// CXWndManager::pWindows.m_length in eqgame.exe
    pub const CXWNDMGR_WINDOWS_COUNT: usize = 0x010;

    /// CSidlScreenWnd::SidlText (CXStr at +0x270) — the SIDL window name
    /// Used to find windows like "CharacterListWnd" by name
    pub const CSIDL_SCREEN_WND_SIDL_TEXT: usize = 0x270;
}

// ─── Character select offsets (eqgame.exe) ───

/// CCharacterListWnd::SelectCharacter function address (preferred base, eqgame.exe)
pub const SELECT_CHARACTER: u64 = 0x1400D5D20;

/// CCharacterListWnd::EnterWorld function address (preferred base, eqgame.exe)
pub const ENTER_WORLD: u64 = 0x1400D4B20;

// ─── PlayerClient (SPAWNINFO) field offsets ───
// These are byte offsets within the PlayerClient struct.
// Source: mq2-reference/src/eqlib/include/eqlib/game/PlayerClient.h

/// Offsets within PlayerBase (base class of PlayerClient)
pub mod player_base {
    /// PlayerClient* — next spawn in linked list (from TListNode)
    /// Note: vtable pointer at 0x00 pushes TListNode fields down by 8
    pub const NEXT: usize = 0x10;
    /// PlayerClient* — previous spawn in linked list (from TListNode)
    pub const PREV: usize = 0x08;

    /// float — Y position
    pub const Y: usize = 0x074;
    /// float — X position
    pub const X: usize = 0x078;
    /// float — Z position
    pub const Z: usize = 0x07c;
    /// float — heading/rotation
    pub const HEADING: usize = 0x090;
    /// char[64] — internal name (e.g., "priest_of_discord00")
    pub const NAME: usize = 0x0b4;
    /// char[64] — displayed name (e.g., "Priest of Discord")
    pub const DISPLAYED_NAME: usize = 0x0f4;
    /// float — SpeedX (lateral speed component)
    pub const SPEED_CURRENT: usize = 0x084;
    /// float — SpeedZ (vertical speed component)
    pub const SPEED_Z: usize = 0x088;
    /// float — SpeedRun (actual movement speed, includes modifiers)
    pub const SPEED_RUN: usize = 0x08c;
    /// float — speed heading (direction of movement)
    pub const SPEED_HEADING: usize = 0x09c;
    /// uint8_t — spawn type (PC=0, NPC=1, Corpse=2, etc.)
    pub const TYPE: usize = 0x135;
    /// uint32_t — unique spawn ID
    pub const SPAWN_ID: usize = 0x168;
    /// char[32] — last name
    pub const LASTNAME: usize = 0x048;
}

/// Offsets within CharacterZoneClient (casting state)
pub mod character_zone {
    /// uint32_t — cast completion ETA (server timestamp when spell finishes)
    /// Source: PlayerClient.h offset 0x010 (CharacterZoneClient::SpellETA)
    pub const SPELL_ETA: usize = 0x010;
    /// uint8_t — active spell gem slot (0xFF = not casting)
    /// Source: PlayerClient.h offset 0x039 (CharacterZoneClient::SpellSlot)
    pub const SPELL_SLOT: usize = 0x039;
    /// uint32_t[15] — per-gem recast timestamp array
    /// Source: PlayerClient.h offset 0x3B0 (CharacterZoneClient::SpellGemETA)
    pub const SPELL_GEM_ETA: usize = 0x3B0;
}

/// Offsets within PlayerZoneClient (extends PlayerBase at 0x01c8)
pub mod player_zone {
    /// int64_t — maximum HP
    pub const HP_MAX: usize = 0x0338;
    /// int64_t — current HP
    pub const HP_CURRENT: usize = 0x03a0;
    /// int32_t — maximum mana (only valid for local player; other spawns have garbage)
    pub const MANA_MAX: usize = 0x03ac;
    /// int32_t — current mana (only valid for local player; other spawns have garbage)
    pub const MANA_CURRENT: usize = 0x03fc;
    /// uint8_t — character level
    pub const LEVEL: usize = 0x03ef;
    /// uint8_t — standing state (0=standing, 1=frozen, 2=looting, 3=sitting, 4=ducking, 110=feigned, 111=dead)
    /// Source: PlayerZoneClient offset 0x0574 in PlayerClient.h
    /// TODO: 0x0574 reads 110 (FD) when character is sitting on March 10, 2026 build.
    /// Needs hex dump calibration scan on frostreaver to find correct offset.
    /// Old offset 0x0134 always read 0 (Standing). Neither is correct.
    pub const STANDSTATE: usize = 0x0574;
    /// char — GM flag (nonzero = GM). Source: PlayerClient.h offset 0x03ec
    pub const GM: usize = 0x03ec;
    /// uint8_t — GM rank. Source: PlayerClient.h offset 0x0368
    pub const GM_RANK: usize = 0x0368;
    /// uint8_t — character class ID (1=WAR, 2=CLR, ..., 16=BER)
    /// Source: PlayerZoneClient offset 0x0420 in PlayerClient.h
    /// This is the direct field — more reliable than the ActorClient path (0x0FDC)
    /// which requires traversing through mActorClient at 0x0FC0.
    pub const CHAR_CLASS: usize = 0x0420;
    /// int32_t — current endurance
    pub const ENDURANCE_CURRENT: usize = 0x04f8;
    /// uint32_t — maximum endurance
    pub const ENDURANCE_MAX: usize = 0x0538;
    /// float — melee range radius
    /// Source: PlayerClient.h offset 0x11D8
    pub const MELEE_RADIUS: usize = 0x11D8;
}

/// Offsets within ActorClient (embedded in PlayerZoneClient at 0x0FC0)
pub mod actor_client {
    /// int32_t — race ID (from ActorBase at offset 0x14)
    pub const RACE: usize = 0x0FD4;
    /// int32_t — race override (illusions, etc.)
    pub const RACE_OVERRIDE: usize = 0x0FD8;
    /// int32_t — character class ID (from ActorBase at offset 0x1C)
    /// Source: ActorClient at 0x0FC0 + ActorBase.Class at 0x1C = 0x0FDC
    /// Note: Prefer player_zone::CHAR_CLASS (0x0420) for spawn reads — it's a
    /// direct field and less likely to break if struct layout shifts.
    pub const CHAR_CLASS: usize = 0x0FDC;
}

/// Group-related offsets
/// Source: mq2-reference/src/eqlib/include/eqlib/game/PcClient.h
pub mod group {
    /// Offset of CGroup* pointer within PcClient struct
    /// PcClient.Group at 0x2EB0
    pub const PC_CLIENT_GROUP_PTR: usize = 0x2EB0;

    /// MAX_GROUP_SIZE = 6 (including self)
    pub const MAX_GROUP_SIZE: usize = 6;

    // ─── CGroupBase layout (vtable at 0x00) ───
    /// CGroupMember* m_groupMembers[6] — array of 6 member pointers
    pub const GROUP_MEMBERS: usize = 0x08;
    /// CGroupMember* m_groupLeader — pointer to leader member
    pub const GROUP_LEADER: usize = 0x38;
    /// uint32_t m_id — group ID
    pub const GROUP_ID: usize = 0x40;

    // ─── CGroupMemberBase layout (vtable at 0x00) ───
    /// CXStr Name — member name (CXStr = pointer to CStrRep)
    pub const MEMBER_NAME_CXSTR: usize = 0x08;
    /// short Type — player type (PC=0, NPC=1, etc.)
    pub const MEMBER_TYPE: usize = 0x10;
    /// CXStr OwnerName — mercenary owner name
    pub const MEMBER_OWNER_CXSTR: usize = 0x18;
    /// int Level
    pub const MEMBER_LEVEL: usize = 0x20;
    /// bool bIsOffline
    pub const MEMBER_IS_OFFLINE: usize = 0x24;

    // ─── CXStr / CStrRep layout ───
    /// CXStr is a single pointer to CStrRep (m_data at offset 0x00)
    /// CStrRep.utf8 string data starts at offset 0x18
    pub const CXSTR_REP_UTF8: usize = 0x18;
}

/// Zone info offsets (zoneHeader / ZONEINFO struct)
/// Source: mq2-reference/src/eqlib/include/eqlib/game/EverQuest.h (zoneHeader)
pub mod zone_info {
    /// Address of the zoneHeader struct (instEQZoneInfo).
    /// This is NOT a pointer — it's the struct itself at this address.
    pub const INST_EQ_ZONE_INFO: u64 = 0x140E95CD4;

    /// char[128] — zone short name (e.g., "qey2hh1")
    pub const SHORT_NAME: usize = 0x000;

    /// char[128] — zone long name (e.g., "Queynos Hills")
    pub const LONG_NAME: usize = 0x080;
}

/// Offsets within SpawnManager (PlayerManagerBase)
pub mod spawn_manager {
    /// TList<PlayerClient*> — start of the player linked list
    /// The TList itself contains m_pFirstNode at offset 0x00
    pub const PLAYER_LIST: usize = 0x0010;
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
}
