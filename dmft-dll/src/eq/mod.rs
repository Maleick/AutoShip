//! EQ internal function addresses and signatures.
//! These are offsets from the eqgame.exe base address.
//! Derived from MQ2 source headers.

/// CEverQuest::MainLoop offset from EQ base.
/// Derived from dmft_common::offsets::PROCESS_GAME_EVENTS (0x14028E0F0)
/// minus preferred base (0x140000000).
pub const MAIN_LOOP_OFFSET: usize = 0x28E0F0;

/// Movement processing function offset.
pub const MOVE_PLAYER_OFFSET: usize = 0x0; // placeholder

/// Spell casting function offset.
pub const CAST_SPELL_OFFSET: usize = 0x0; // placeholder

/// Set target function offset.
pub const SET_TARGET_OFFSET: usize = 0x0; // placeholder

// ─── Combat Function Bindings ───
// These call EQ internal functions via transmuted function pointers.
// Pattern: rebase preferred address → transmute to fn pointer → call.
// All gated behind #[cfg(windows)] with non-windows stubs.

use core::ffi::c_void;
use std::sync::atomic::Ordering;

/// Resolve the EQ base address from the global atomic.
/// Returns `None` if the base hasn't been set yet.
fn get_eq_base() -> Option<u64> {
    let base = crate::EQ_BASE.load(Ordering::Acquire);
    if base == 0 { None } else { Some(base) }
}

/// Resolve the local player pointer (PlayerClient*).
/// Returns `None` if not logged in.
fn get_local_player(eq_base: u64) -> Option<*mut c_void> {
    let addr = dmft_common::offsets::rebase(
        dmft_common::offsets::PINST_LOCAL_PLAYER,
        eq_base,
    )?;

    #[cfg(windows)]
    {
        let ptr: *mut c_void = unsafe { *(addr as *const *mut c_void) };
        if ptr.is_null() { None } else { Some(ptr) }
    }

    #[cfg(not(windows))]
    {
        let _ = addr;
        None
    }
}

/// Cast a spell by gem index and spell ID.
///
/// `gem_id`: 0-based gem slot index.
/// `spell_id`: the spell's ID number.
///
/// Calls CharacterZoneClient::CastSpell(gemid, spellid, item_ptr=null, item_guid=0).
pub fn cast_spell(gem_id: u8, spell_id: i32) {
    #[cfg(windows)]
    {
        let Some(eq_base) = get_eq_base() else {
            tracing::error!("EQ base not set");
            return;
        };
        let Some(player) = get_local_player(eq_base) else {
            tracing::error!("Local player not available");
            return;
        };
        let Some(addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::CAST_SPELL,
            eq_base,
        ) else {
            tracing::error!("Failed to rebase CAST_SPELL");
            return;
        };

        // CharacterZoneClient::CastSpell(this, gemid, spellid, item_ptr, item_guid)
        // x64: this=RCX, gemid=DL, spellid=R8D, item_ptr=R9, item_guid=[stack]
        type CastSpellFn = unsafe extern "C" fn(
            *mut c_void, // this (CharacterZoneClient*)
            u8,          // gem_id
            i32,         // spell_id
            *mut c_void, // item_ptr (null for normal casts)
            u64,         // item_guid (0 for normal casts)
        );
        let func: CastSpellFn = unsafe { std::mem::transmute(addr) };

        tracing::info!(gem_id, spell_id, addr = format!("{:#x}", addr), "Calling CastSpell");
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
/// Calls PlayerZoneClient::DoAttack(slot, unknown=null).
pub fn do_attack(attack_type: u8) {
    #[cfg(windows)]
    {
        let Some(eq_base) = get_eq_base() else {
            tracing::error!("EQ base not set");
            return;
        };
        let Some(player) = get_local_player(eq_base) else {
            tracing::error!("Local player not available");
            return;
        };
        let Some(addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::DO_ATTACK,
            eq_base,
        ) else {
            tracing::error!("Failed to rebase DO_ATTACK");
            return;
        };

        // PlayerZoneClient::DoAttack(this, slot, unknown)
        type DoAttackFn = unsafe extern "C" fn(
            *mut c_void, // this (PlayerZoneClient*)
            u8,          // attack_type/slot
            *mut c_void, // unknown (null)
        );
        let func: DoAttackFn = unsafe { std::mem::transmute(addr) };

        tracing::info!(attack_type, addr = format!("{:#x}", addr), "Calling DoAttack");
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
/// Calls CharacterZoneClient::UseSkill(skill, target, bAuto=false).
pub fn use_skill(skill_id: u32, target: Option<*mut c_void>) {
    #[cfg(windows)]
    {
        let Some(eq_base) = get_eq_base() else {
            tracing::error!("EQ base not set");
            return;
        };
        let Some(player) = get_local_player(eq_base) else {
            tracing::error!("Local player not available");
            return;
        };
        let Some(addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::USE_SKILL,
            eq_base,
        ) else {
            tracing::error!("Failed to rebase USE_SKILL");
            return;
        };

        // CharacterZoneClient::UseSkill(this, skill, target, bAuto)
        type UseSkillFn = unsafe extern "C" fn(
            *mut c_void, // this (CharacterZoneClient*)
            u32,         // skill_id
            *mut c_void, // target (PlayerZoneClient*)
        );
        let func: UseSkillFn = unsafe { std::mem::transmute(addr) };

        let target_ptr = target.unwrap_or(std::ptr::null_mut());
        tracing::info!(skill_id, addr = format!("{:#x}", addr), "Calling UseSkill");
        unsafe { func(player, skill_id, target_ptr) };
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
/// Calls PcZoneClient::DoCombatAbility(spellID, allowLowerRank).
pub fn do_combat_ability(spell_id: i32, allow_lower_rank: bool) {
    #[cfg(windows)]
    {
        let Some(eq_base) = get_eq_base() else {
            tracing::error!("EQ base not set");
            return;
        };
        let Some(player) = get_local_player(eq_base) else {
            tracing::error!("Local player not available");
            return;
        };
        let Some(addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::DO_COMBAT_ABILITY,
            eq_base,
        ) else {
            tracing::error!("Failed to rebase DO_COMBAT_ABILITY");
            return;
        };

        // PcZoneClient::DoCombatAbility(this, spellID, allowLowerRank)
        type DoCombatAbilityFn = unsafe extern "C" fn(
            *mut c_void, // this (PcZoneClient*)
            i32,         // spell_id
            bool,        // allow_lower_rank
        );
        let func: DoCombatAbilityFn = unsafe { std::mem::transmute(addr) };

        tracing::info!(spell_id, allow_lower_rank, addr = format!("{:#x}", addr), "Calling DoCombatAbility");
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
/// Calls __ExecuteCmd(this=null, cmd_id, active, unknown=null).
pub fn execute_cmd(cmd_id: u32, active: i32) {
    #[cfg(windows)]
    {
        let Some(eq_base) = get_eq_base() else {
            tracing::error!("EQ base not set");
            return;
        };
        let Some(addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::EXECUTE_CMD,
            eq_base,
        ) else {
            tracing::error!("Failed to rebase EXECUTE_CMD");
            return;
        };

        // __ExecuteCmd(this, cmd_id, active, unknown)
        // __ExecuteCmd is a free function but uses this-call convention with
        // a dummy first arg in the MQ2 source.
        type ExecuteCmdFn = unsafe extern "C" fn(
            *mut c_void, // this (unused, pass null)
            u32,         // cmd_id
            i32,         // active (1=on, 0=off)
            *mut c_void, // unknown (null)
        );
        let func: ExecuteCmdFn = unsafe { std::mem::transmute(addr) };

        tracing::info!(cmd_id, active, addr = format!("{:#x}", addr), "Calling ExecuteCmd");
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

/// Toggle auto-attack on or off via ExecuteCmd.
///
/// `enable`: `true` to turn auto-attack on, `false` to turn it off.
pub fn toggle_auto_attack(enable: bool) {
    let active = if enable { 1 } else { 0 };
    tracing::info!(enable, "Toggling auto-attack");
    execute_cmd(CMD_ATTACK, active);
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
}
