//! EQ internal function addresses, signatures, and widget interaction primitives.
//! These are offsets from the eqgame.exe base address.
//! Derived from MQ2 source headers.

pub mod bazaar;
pub mod chat;
pub mod context_menu;
pub mod inventory;
pub mod notification;
pub mod widgets;

/// `CEverQuest::MainLoop` offset from EQ base.
/// Derived from `textquest_common::offsets::PROCESS_GAME_EVENTS` (0x14028E0F0)
/// minus preferred base (0x140000000).
pub const MAIN_LOOP_OFFSET: usize = 0x0028_E0F0;

/// Movement processing function offset.
pub const MOVE_PLAYER_OFFSET: usize = 0x0; // placeholder

/// Spell casting function offset.
pub const CAST_SPELL_OFFSET: usize = 0x0; // placeholder

/// Set target function offset.
pub const SET_TARGET_OFFSET: usize = 0x0; // placeholder

const VERSION_STRING_MAX_LEN: usize = 64;

fn parse_version_string(bytes: &[u8]) -> Option<String> {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let s = String::from_utf8_lossy(&bytes[..end]);
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Read `__ActualVersionDate` from the given EQ base address.
///
/// Returns `None` when the string cannot be read or is empty.
#[cfg(windows)]
pub fn check_eq_version(base: u64) -> Option<String> {
    use textquest_common::offsets::{ACTUAL_VERSION_DATE, rebase};

    let addr: usize = match rebase(ACTUAL_VERSION_DATE, base)?.try_into() {
        Ok(v) => v,
        Err(_) => return None,
    };

    // ACTUAL_VERSION_DATE is a pointer to a string, not the string itself.
    // First dereference the pointer, then read the string bytes.
    if !is_readable(addr, std::mem::size_of::<usize>()) {
        return None;
    }

    let string_addr = unsafe { std::ptr::read_unaligned(addr as *const usize) };
    if string_addr == 0 {
        return None;
    }

    if !is_readable(string_addr, VERSION_STRING_MAX_LEN) {
        return None;
    }

    let mut buffer = [0u8; VERSION_STRING_MAX_LEN];
    unsafe {
        std::ptr::copy_nonoverlapping(
            string_addr as *const u8,
            buffer.as_mut_ptr(),
            VERSION_STRING_MAX_LEN,
        );
    }

    parse_version_string(&buffer)
}

/// Non-Windows stub — always returns `None`.
#[cfg(not(windows))]
pub fn check_eq_version(_base: u64) -> Option<String> {
    None
}

/// Whether the detected version string matches the expected patch marker.
#[must_use]
pub fn version_matches_expected(version: &str) -> bool {
    version == textquest_common::offsets::EXPECTED_VERSION_DATE
}

#[cfg(not(windows))]
fn is_readable(_address: usize, _len: usize) -> bool {
    false
}

#[cfg(windows)]
fn is_readable(address: usize, len: usize) -> bool {
    use windows::Win32::System::Memory::{
        MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_GUARD, PAGE_NOACCESS, VirtualQuery,
    };

    if address == 0 || len == 0 {
        return false;
    }

    let mut mbi = MEMORY_BASIC_INFORMATION::default();
    let result = unsafe {
        VirtualQuery(
            Some(address as *const core::ffi::c_void),
            &mut mbi,
            size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };
    if result == 0 || mbi.State != MEM_COMMIT {
        return false;
    }

    // Use bitflag containment checks, not equality, to handle combined flags.
    if (mbi.Protect.0 & PAGE_NOACCESS.0) != 0 || (mbi.Protect.0 & PAGE_GUARD.0) != 0 {
        return false;
    }

    address.saturating_add(len) <= (mbi.BaseAddress as usize + mbi.RegionSize)
}

// ─── Combat Function Bindings ───
// These call EQ internal functions via transmuted function pointers.
// Pattern: rebase preferred address → transmute to fn pointer → call.
// All gated behind #[cfg(windows)] with non-windows stubs.

use core::ffi::c_void;
use std::sync::atomic::Ordering;

/// Maximum spell gems exposed by the local profile's memorized spell array.
pub const MAX_MEMORIZED_SPELL_GEMS: usize =
    textquest_common::offsets::profile::MEMORIZED_SPELL_GEMS;

/// Resolve the EQ base address from the global atomic.
/// Returns `None` if the base hasn't been set yet.
fn get_eq_base() -> Option<u64> {
    let base = crate::EQ_BASE.load(Ordering::Acquire);
    if base == 0 { None } else { Some(base) }
}

/// Validate that a rebased function pointer address is safe to transmute and call.
///
/// Checks:
/// 1. Address is non-zero
/// 2. Address falls within the EQ module's memory region (base .. base + reasonable size)
/// 3. (Windows only) The memory page is committed and has execute permission
///
/// Returns `true` if the address looks valid, `false` otherwise (with a warning log).
#[cfg(windows)]
pub(crate) fn validate_fn_ptr(addr: usize, name: &str) -> bool {
    use windows::Win32::System::Memory::{
        MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE, PAGE_EXECUTE_READ,
        PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, VirtualQuery,
    };

    // eqgame.exe is typically ~50-80 MB. Use 256 MB as a generous upper bound.
    const MAX_MODULE_SIZE: usize = 256 * 1024 * 1024;

    if addr == 0 {
        tracing::warn!(name, "Function pointer address is null");
        return false;
    }

    let eq_base = crate::EQ_BASE.load(Ordering::Acquire) as usize;
    if eq_base == 0 {
        tracing::warn!(name, "EQ base not set during fn ptr validation");
        return false;
    }

    if addr < eq_base || addr >= eq_base + MAX_MODULE_SIZE {
        tracing::warn!(
            name,
            addr = format!("{:#x}", addr),
            eq_base = format!("{:#x}", eq_base),
            "Function pointer outside EQ module range"
        );
        return false;
    }

    let mut mbi = MEMORY_BASIC_INFORMATION::default();
    let result = unsafe {
        VirtualQuery(
            Some(addr as *const core::ffi::c_void),
            &mut mbi,
            size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };
    if result == 0 {
        tracing::warn!(
            name,
            addr = format!("{:#x}", addr),
            "VirtualQuery failed for function pointer"
        );
        return false;
    }

    if mbi.State != MEM_COMMIT {
        tracing::warn!(
            name,
            addr = format!("{:#x}", addr),
            state = mbi.State.0,
            "Function pointer page not committed"
        );
        return false;
    }

    let protect = mbi.Protect;
    let executable = protect == PAGE_EXECUTE
        || protect == PAGE_EXECUTE_READ
        || protect == PAGE_EXECUTE_READWRITE
        || protect == PAGE_EXECUTE_WRITECOPY;
    if !executable {
        tracing::warn!(
            name,
            addr = format!("{:#x}", addr),
            protect = protect.0,
            "Function pointer page not executable"
        );
        return false;
    }

    true
}

#[cfg(not(windows))]
pub(crate) fn validate_fn_ptr(_addr: usize, _name: &str) -> bool {
    // Non-Windows builds never actually call these function pointers,
    // so validation is a no-op.
    true
}

/// Resolve the local player pointer (`PlayerClient`*).
/// Returns `None` if not logged in.
fn get_local_player(eq_base: u64) -> Option<*mut c_void> {
    let addr =
        textquest_common::offsets::rebase(textquest_common::offsets::PINST_LOCAL_PLAYER, eq_base)?;

    #[cfg(windows)]
    {
        // SAFETY: addr was rebased from PINST_LOCAL_PLAYER, a known global
        // pointer in eqgame.exe's data section. Dereferencing yields the
        // PlayerClient* (null when not logged in). Null-checked below.
        let ptr: *mut c_void = unsafe { *(addr as *const *mut c_void) };
        if ptr.is_null() { None } else { Some(ptr) }
    }

    #[cfg(not(windows))]
    {
        let _ = addr;
        None
    }
}

/// Read the local character's memorized spell IDs from the current profile.
///
/// Returns a fixed-size array of spell IDs indexed by gem slot (0-based).
/// Zero or negative values mean the gem is empty / unavailable.
#[must_use]
pub fn read_memorized_spells() -> [i32; MAX_MEMORIZED_SPELL_GEMS] {
    #[cfg(windows)]
    {
        use textquest_common::offsets::profile;

        let mut spells = [0; MAX_MEMORIZED_SPELL_GEMS];

        let Some(eq_base) = get_eq_base() else {
            return spells;
        };
        let Some(player) = get_local_player(eq_base) else {
            return spells;
        };

        let player_ptr = player as usize;
        if !crate::hooks::game_loop::is_readable(
            player_ptr + profile::PROFILE_MANAGER,
            std::mem::size_of::<usize>(),
        ) {
            return spells;
        }
        let profile_manager = unsafe { *((player_ptr + profile::PROFILE_MANAGER) as *const usize) };
        if profile_manager == 0
            || !crate::hooks::game_loop::is_readable(
                profile_manager + profile::PROFILE_LIST_PTR,
                std::mem::size_of::<usize>(),
            )
        {
            return spells;
        }

        let profile_list =
            unsafe { *((profile_manager + profile::PROFILE_LIST_PTR) as *const usize) };
        if profile_list == 0
            || !crate::hooks::game_loop::is_readable(
                profile_list + profile::PROFILE_FIRST,
                std::mem::size_of::<usize>(),
            )
        {
            return spells;
        }

        let base_profile = unsafe { *((profile_list + profile::PROFILE_FIRST) as *const usize) };
        if base_profile == 0 {
            return spells;
        }

        for (idx, spell_id) in spells.iter_mut().enumerate() {
            let addr = base_profile + profile::MEMORIZED_SPELLS + idx * std::mem::size_of::<i32>();
            if crate::hooks::game_loop::is_readable(addr, std::mem::size_of::<i32>()) {
                *spell_id = unsafe { *(addr as *const i32) };
            }
        }

        spells
    }

    #[cfg(not(windows))]
    {
        [0; MAX_MEMORIZED_SPELL_GEMS]
    }
}

/// Cast a spell by gem index and spell ID.
///
/// `gem_id`: 0-based gem slot index.
/// `spell_id`: the spell's ID number.
///
/// Calls `CharacterZoneClient::CastSpell(gemid`, spellid, `item_ptr=null`, item_guid=0).
pub fn cast_spell(gem_id: u8, spell_id: i32) {
    #[cfg(windows)]
    {
        // CharacterZoneClient::CastSpell(this, gemid, spellid, item_ptr, item_guid)
        // x64: this=RCX, gemid=DL, spellid=R8D, item_ptr=R9, item_guid=[stack]
        type CastSpellFn = unsafe extern "C" fn(
            *mut c_void, // this (CharacterZoneClient*)
            u8,          // gem_id
            i32,         // spell_id
            *mut c_void, // item_ptr (null for normal casts)
            u64,         // item_guid (0 for normal casts)
        );

        let Some(eq_base) = get_eq_base() else {
            tracing::error!("EQ base not set");
            return;
        };
        let Some(player) = get_local_player(eq_base) else {
            tracing::error!("Local player not available");
            return;
        };
        let Some(addr) =
            textquest_common::offsets::rebase(textquest_common::offsets::CAST_SPELL, eq_base)
        else {
            tracing::error!("Failed to rebase CAST_SPELL");
            return;
        };

        if !validate_fn_ptr(addr, "CastSpell") {
            return;
        }
        // SAFETY: addr was rebased from CAST_SPELL — a known function in
        // eqgame.exe. The transmute converts it to CharacterZoneClient::CastSpell's
        // calling convention. `player` is a validated non-null PlayerClient*.
        // If the offset is wrong, this will crash EQ (no way to validate statically).
        // Null item_ptr and zero item_guid indicate a normal (non-item) cast.
        let func: CastSpellFn = unsafe { std::mem::transmute(addr) };

        tracing::info!(
            gem_id,
            spell_id,
            addr = format!("{:#x}", addr),
            "Calling CastSpell"
        );
        unsafe { func(player, gem_id, spell_id, std::ptr::null_mut(), 0) };
    }

    #[cfg(not(windows))]
    {
        let _ = (gem_id, spell_id);
        tracing::warn!("CastSpell not available on this platform");
    }
}

/// Perform a melee attack.
///
/// `attack_type`: attack slot/type byte.
///
/// Calls `PlayerZoneClient::DoAttack(slot`, unknown=null).
pub fn do_attack(attack_type: u8) {
    #[cfg(windows)]
    {
        // PlayerZoneClient::DoAttack(this, slot, unknown)
        type DoAttackFn = unsafe extern "C" fn(
            *mut c_void, // this (PlayerZoneClient*)
            u8,          // attack_type/slot
            *mut c_void, // unknown (null)
        );

        let Some(eq_base) = get_eq_base() else {
            tracing::error!("EQ base not set");
            return;
        };
        let Some(player) = get_local_player(eq_base) else {
            tracing::error!("Local player not available");
            return;
        };
        let Some(addr) =
            textquest_common::offsets::rebase(textquest_common::offsets::DO_ATTACK, eq_base)
        else {
            tracing::error!("Failed to rebase DO_ATTACK");
            return;
        };

        if !validate_fn_ptr(addr, "DoAttack") {
            return;
        }
        // SAFETY: addr was rebased from DO_ATTACK. The transmute converts it
        // to PlayerZoneClient::DoAttack's calling convention. `player` is a
        // validated non-null PlayerClient*. Null unknown arg is the standard
        // calling pattern from MQ2. If the offset is wrong, EQ will crash.
        let func: DoAttackFn = unsafe { std::mem::transmute(addr) };

        tracing::info!(
            attack_type,
            addr = format!("{:#x}", addr),
            "Calling DoAttack"
        );
        unsafe { func(player, attack_type, std::ptr::null_mut()) };
    }

    #[cfg(not(windows))]
    {
        let _ = attack_type;
        tracing::warn!("DoAttack not available on this platform");
    }
}

/// Use a skill on an optional target.
///
/// `skill_id`: the skill index (e.g., kick, bash, etc.).
/// `target`: optional target pointer. Pass `None` to use current target.
///
/// Calls `CharacterZoneClient::UseSkill(skill`, target, bAuto=false).
pub fn use_skill(skill_id: u32, target: Option<*mut c_void>) {
    #[cfg(windows)]
    {
        // CharacterZoneClient::UseSkill(this, skill, target, bAuto)
        type UseSkillFn = unsafe extern "C" fn(
            *mut c_void, // this (CharacterZoneClient*)
            u32,         // skill_id
            *mut c_void, // target (PlayerZoneClient*)
            bool,        // bAuto (false = manual activation)
        );

        let Some(eq_base) = get_eq_base() else {
            tracing::error!("EQ base not set");
            return;
        };
        let Some(player) = get_local_player(eq_base) else {
            tracing::error!("Local player not available");
            return;
        };
        let Some(addr) =
            textquest_common::offsets::rebase(textquest_common::offsets::USE_SKILL, eq_base)
        else {
            tracing::error!("Failed to rebase USE_SKILL");
            return;
        };

        if !validate_fn_ptr(addr, "UseSkill") {
            return;
        }
        // SAFETY: addr was rebased from USE_SKILL. The transmute converts it
        // to CharacterZoneClient::UseSkill's calling convention. `player` is
        // validated non-null. target_ptr may be null (use current target).
        // bAuto=false indicates manual activation. If the offset is wrong, EQ crashes.
        let func: UseSkillFn = unsafe { std::mem::transmute(addr) };

        let target_ptr = target.unwrap_or(std::ptr::null_mut());
        tracing::info!(skill_id, addr = format!("{:#x}", addr), "Calling UseSkill");
        unsafe { func(player, skill_id, target_ptr, false) };
    }

    #[cfg(not(windows))]
    {
        let _ = (skill_id, target);
        tracing::warn!("UseSkill not available on this platform");
    }
}

/// Use a combat ability (discipline, AA, etc.).
///
/// `spell_id`: the ability's spell ID.
/// `allow_lower_rank`: whether to allow using a lower rank if the exact rank is unavailable.
///
/// Calls `PcZoneClient::DoCombatAbility(spellID`, allowLowerRank).
pub fn do_combat_ability(spell_id: i32, allow_lower_rank: bool) {
    #[cfg(windows)]
    {
        // PcZoneClient::DoCombatAbility(this, spellID, allowLowerRank)
        type DoCombatAbilityFn = unsafe extern "C" fn(
            *mut c_void, // this (PcZoneClient*)
            i32,         // spell_id
            bool,        // allow_lower_rank
        );

        let Some(eq_base) = get_eq_base() else {
            tracing::error!("EQ base not set");
            return;
        };
        let Some(player) = get_local_player(eq_base) else {
            tracing::error!("Local player not available");
            return;
        };
        let Some(addr) = textquest_common::offsets::rebase(
            textquest_common::offsets::DO_COMBAT_ABILITY,
            eq_base,
        ) else {
            tracing::error!("Failed to rebase DO_COMBAT_ABILITY");
            return;
        };

        if !validate_fn_ptr(addr, "DoCombatAbility") {
            return;
        }
        // SAFETY: addr was rebased from DO_COMBAT_ABILITY. The transmute
        // converts it to PcZoneClient::DoCombatAbility's calling convention.
        // `player` is validated non-null. If the offset is wrong, EQ crashes.
        let func: DoCombatAbilityFn = unsafe { std::mem::transmute(addr) };

        tracing::info!(
            spell_id,
            allow_lower_rank,
            addr = format!("{:#x}", addr),
            "Calling DoCombatAbility"
        );
        unsafe { func(player, spell_id, allow_lower_rank) };
    }

    #[cfg(not(windows))]
    {
        let _ = (spell_id, allow_lower_rank);
        tracing::warn!("DoCombatAbility not available on this platform");
    }
}

/// Execute an EQ command by command ID.
///
/// `cmd_id`: the command constant (e.g., 0x17 for attack).
/// `active`: 1 to activate, 0 to deactivate.
///
/// Calls __ExecuteCmd(this=null, `cmd_id`, active, unknown=null).
pub fn execute_cmd(cmd_id: u32, active: i32) {
    #[cfg(windows)]
    {
        // __ExecuteCmd(this, cmd_id, active, unknown)
        // __ExecuteCmd is a free function but uses this-call convention with
        // a dummy first arg in the MQ2 source.
        type ExecuteCmdFn = unsafe extern "C" fn(
            *mut c_void, // this (unused, pass null)
            u32,         // cmd_id
            i32,         // active (1=on, 0=off)
            *mut c_void, // unknown (null)
        );

        let Some(eq_base) = get_eq_base() else {
            tracing::error!("EQ base not set");
            return;
        };
        let Some(addr) =
            textquest_common::offsets::rebase(textquest_common::offsets::EXECUTE_CMD, eq_base)
        else {
            tracing::error!("Failed to rebase EXECUTE_CMD");
            return;
        };

        if !validate_fn_ptr(addr, "ExecuteCmd") {
            return;
        }
        // SAFETY: addr was rebased from EXECUTE_CMD. The transmute converts
        // it to __ExecuteCmd's calling convention. The first arg (this) is null
        // per MQ2 convention (free function with dummy this-call). If the offset
        // is wrong, EQ crashes.
        let func: ExecuteCmdFn = unsafe { std::mem::transmute(addr) };

        tracing::info!(
            cmd_id,
            active,
            addr = format!("{:#x}", addr),
            "Calling ExecuteCmd"
        );
        unsafe { func(std::ptr::null_mut(), cmd_id, active, std::ptr::null_mut()) };
    }

    #[cfg(not(windows))]
    {
        let _ = (cmd_id, active);
        tracing::warn!("ExecuteCmd not available on this platform");
    }
}

/// EQ command ID for auto-attack toggle.
pub const CMD_ATTACK: u32 = 0x17;

/// Toggle auto-attack on or off via `ExecuteCmd`.
///
/// `enable`: `true` to turn auto-attack on, `false` to turn it off.
pub fn toggle_auto_attack(enable: bool) {
    let active = i32::from(enable);
    tracing::info!(enable, "Toggling auto-attack");
    execute_cmd(CMD_ATTACK, active);
}

/// Execute an EQ slash command string (e.g., "/face", "/pet attack").
///
/// Calls `CEverQuest::InterpretCmd(this`, pChar, szCmd).
pub fn slash_command(command: &str) {
    #[cfg(windows)]
    {
        // CEverQuest::InterpretCmd(this, PlayerClient*, const char*)
        type InterpretCmdFn = unsafe extern "C" fn(
            *mut c_void, // this (CEverQuest*)
            *mut c_void, // pChar (PlayerClient*)
            *const i8,   // szCmd
        );

        let Some(eq_base) = get_eq_base() else {
            tracing::error!("EQ base not set");
            return;
        };
        let Some(player) = get_local_player(eq_base) else {
            tracing::error!("Local player not available");
            return;
        };

        // Get the CEverQuest instance pointer.
        let Some(eq_inst_addr) =
            textquest_common::offsets::rebase(textquest_common::offsets::PINST_CEVERQUEST, eq_base)
        else {
            tracing::error!("Failed to rebase PINST_CEVERQUEST");
            return;
        };

        // SAFETY: eq_inst_addr is the rebased PINST_CEVERQUEST global pointer.
        // Dereferencing yields the CEverQuest singleton (null if not initialized).
        let eq_inst: *mut c_void = unsafe { *(eq_inst_addr as *const *mut c_void) };
        if eq_inst.is_null() {
            tracing::error!("CEverQuest instance pointer is null");
            return;
        }

        let cmd_cstring = match std::ffi::CString::new(command) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "Invalid command string");
                return;
            }
        };
        let Some(interpret_addr) =
            textquest_common::offsets::rebase(textquest_common::offsets::INTERPRET_CMD, eq_base)
        else {
            tracing::error!("Failed to rebase INTERPRET_CMD");
            return;
        };
        if !validate_fn_ptr(interpret_addr, "InterpretCmd") {
            return;
        }
        // SAFETY: interpret_addr was rebased from INTERPRET_CMD. The transmute
        // converts it to CEverQuest::InterpretCmd's calling convention. eq_inst
        // and player are validated non-null above. cmd_cstring is a valid
        // null-terminated C string (CString guarantees no interior nulls).
        // If the offset is wrong, EQ crashes.
        let func: InterpretCmdFn = unsafe { std::mem::transmute(interpret_addr) };

        tracing::info!(cmd = command, "Executing slash command");
        unsafe { func(eq_inst, player, cmd_cstring.as_ptr()) };
    }

    #[cfg(not(windows))]
    {
        let _ = command;
        tracing::warn!("Slash command not available on this platform");
    }
}

/// Send a raw "Living Shield" packet (Active Hack).
///
/// This path is intentionally disabled until the live-client connection
/// pointer is recalibrated. The command remains wired so callers get a clear
/// runtime error instead of a silent no-op or an unsafe packet write.
pub fn send_living_shield(target_id: u32) -> Result<(), String> {
    #[cfg(windows)]
    {
        let Some(_eq_base) = get_eq_base() else {
            return Err("EQ base not set".to_string());
        };

        tracing::warn!(
            target_id,
            "Living Shield packet injection is disabled until the connection pointer path is recalibrated"
        );
        Err("Living Shield packet injection is not implemented in this build.".to_string())
    }

    #[cfg(not(windows))]
    {
        let _ = target_id;
        Err("Active hacks are only available on Windows".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_attack_constant() {
        assert_eq!(CMD_ATTACK, 23);
    }

    #[test]
    fn test_get_eq_base_returns_none_when_zero() {
        // EQ_BASE defaults to 0, so get_eq_base should return None in tests
        assert!(get_eq_base().is_none());
    }

    #[test]
    fn test_cast_spell_noop_without_eq_base() {
        // Should not panic when EQ_BASE is 0 (non-windows: logs warning)
        cast_spell(0, 100);
    }

    #[test]
    fn test_read_memorized_spells_without_eq_base_returns_empty() {
        assert_eq!(read_memorized_spells(), [0; MAX_MEMORIZED_SPELL_GEMS]);
    }

    #[test]
    fn test_do_attack_noop_without_eq_base() {
        do_attack(0);
    }

    #[test]
    fn test_use_skill_noop_without_eq_base() {
        use_skill(10, None);
    }

    #[test]
    fn test_do_combat_ability_noop_without_eq_base() {
        do_combat_ability(500, false);
    }

    #[test]
    fn test_execute_cmd_noop_without_eq_base() {
        execute_cmd(CMD_ATTACK, 1);
    }

    #[test]
    fn test_toggle_auto_attack_noop_without_eq_base() {
        toggle_auto_attack(true);
        toggle_auto_attack(false);
    }

    #[test]
    fn test_slash_command_noop_without_eq_base() {
        slash_command("/face");
        slash_command("/pet attack");
    }
}
