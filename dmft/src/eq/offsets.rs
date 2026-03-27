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

/// Convert a preferred-base offset to an actual address given the runtime base.
pub fn rebase(preferred_addr: u64, actual_base: u64) -> usize {
    let offset = preferred_addr - EQ_PREFERRED_BASE;
    (actual_base + offset) as usize
}

// ─── PlayerClient (SPAWNINFO) field offsets ───
// These are byte offsets within the PlayerClient struct.
// Source: mq2-reference/src/eqlib/include/eqlib/game/PlayerClient.h

/// Offsets within PlayerBase (base class of PlayerClient)
pub mod player_base {
    /// PlayerClient* — next spawn in linked list (from TListNode)
    pub const NEXT: usize = 0x08;
    /// PlayerClient* — previous spawn in linked list (from TListNode)
    pub const PREV: usize = 0x00;

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
    /// int32_t — maximum mana
    pub const MANA_MAX: usize = 0x03ac;
    /// int32_t — current mana
    pub const MANA_CURRENT: usize = 0x03fc;
    /// uint8_t — character level
    pub const LEVEL: usize = 0x03ef;
    /// uint8_t — character class ID
    pub const CHAR_CLASS: usize = 0x0420;
    /// int32_t — current endurance
    pub const ENDURANCE_CURRENT: usize = 0x04f8;
    /// uint32_t — maximum endurance
    pub const ENDURANCE_MAX: usize = 0x0538;
}

/// Offsets within SpawnManager (PlayerManagerBase)
pub mod spawn_manager {
    /// TList<PlayerClient*> — start of the player linked list
    /// The TList itself contains m_pFirstNode at offset 0x00
    pub const PLAYER_LIST: usize = 0x0010;
}
