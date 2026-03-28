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
    pub const STANDSTATE: usize = 0x0574;
    /// int32_t — current endurance
    pub const ENDURANCE_CURRENT: usize = 0x04f8;
    /// uint32_t — maximum endurance
    pub const ENDURANCE_MAX: usize = 0x0538;
}

/// Offsets within ActorClient (embedded in PlayerZoneClient at 0x0FC0)
pub mod actor_client {
    /// int32_t — race ID (from ActorBase at offset 0x14)
    pub const RACE: usize = 0x0FD4;
    /// int32_t — race override (illusions, etc.)
    pub const RACE_OVERRIDE: usize = 0x0FD8;
    /// int32_t — character class ID (from ActorBase at offset 0x1C)
    /// Source: ActorClient at 0x0FC0 + ActorBase.Class at 0x1C = 0x0FDC
    /// Read as u8 for EqClass::from_id() compatibility (valid range 1-16)
    pub const CHAR_CLASS: usize = 0x0FDC;
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
