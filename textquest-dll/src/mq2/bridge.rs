//! MQ2 API bridge — translation layer from MQ2 plugin call-style API to
//! TextQuest internal APIs.
//!
//! # Purpose
//!
//! MQ2 plugins call functions like `GetSpawn()`, `GetTarget()`, `Cmd()`.
//! This module provides `MQ2Bridge`, which exposes those semantics while
//! delegating to the TextQuest/eqlib internals already implemented in:
//!
//! - `crate::mq2` — FFI types (`RawPlayerClient`, `RawItem`, snapshot helpers)
//! - `crate::eq` — EQ function bindings (`slash_command`, `read_memorized_spells`)
//! - `crate::hooks::game_loop` — spawn-list walker, target/player reads
//! - `textquest_common::offsets` — all EQ global pointer addresses
//!
//! # Thread safety
//!
//! `MQ2Bridge` is `!Send + !Sync`.  All methods **must** be called from the
//! EQ game thread (inside a game-loop callback / HWBP handler).  Snapshots
//! returned by the bridge are fully owned and `Send`.
//!
//! # Error policy
//!
//! All methods return `Option` or `Result` — never panic on bad/null EQ
//! pointers.  The bridge logs a `tracing::warn!` for every `None`/`Err`
//! path so callers can trace what went wrong without crashing eqgame.exe.
//!
//! # Platform
//!
//! Memory-reading methods are gated with `#[cfg(windows)]` / stubs.  On
//! non-Windows hosts the bridge compiles clean and all methods return
//! `None`/empty.  This lets `cargo check` and unit tests run on macOS/Linux.

use std::marker::PhantomData;

use super::{ItemSnapshot, PlayerSnapshot, SpawnSnapshot};

// ─── MQ2Bridge ───────────────────────────────────────────────────────────────

/// Translation layer between MQ2-style plugin calls and TextQuest APIs.
///
/// Construct with [`MQ2Bridge::new`].  The bridge holds no owned state — it
/// reads EQ globals on every call, so it is always up-to-date.
///
/// `'tick` ties the bridge to a single game-tick scope.  Do **not** hold a
/// reference past the game-loop callback that created it.
pub struct MQ2Bridge<'tick> {
    /// Phantom lifetime to prevent the bridge from escaping a game tick.
    _tick: PhantomData<&'tick ()>,
}

impl<'tick> Default for MQ2Bridge<'tick> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'tick> MQ2Bridge<'tick> {
    /// Create a new bridge for the current game tick.
    ///
    /// # Safety
    ///
    /// Caller must be on the EQ game thread and must not use the returned
    /// bridge after the current game-loop callback returns.
    pub fn new() -> Self {
        Self { _tick: PhantomData }
    }

    // ── Player ────────────────────────────────────────────────────────────────

    /// Return a snapshot of the local player (`pinstLocalPlayer`).
    ///
    /// Equivalent to MQ2's `pLocalPlayer` / `GetCharInfo()`.
    ///
    /// Returns `None` when not logged in or the pointer is invalid.
    pub fn get_player(&self) -> Option<PlayerSnapshot> {
        tracing::debug!("MQ2Bridge::get_player");
        let snap = read_local_player_snapshot()?;
        tracing::debug!(
            spawn_id = snap.spawn_id,
            name = %snap.name,
            "MQ2Bridge::get_player ok"
        );
        Some(snap)
    }

    // ── Spawns ────────────────────────────────────────────────────────────────

    /// Return a snapshot for a spawn identified by `spawn_id`.
    ///
    /// Equivalent to MQ2's `GetSpawnByID(id)`.
    ///
    /// Walks the EQ spawn linked list.  Returns `None` when not found or when
    /// the spawn manager is unavailable.
    pub fn get_spawn(&self, spawn_id: u32) -> Option<SpawnSnapshot> {
        tracing::debug!(spawn_id, "MQ2Bridge::get_spawn");
        if spawn_id == 0 {
            tracing::warn!("MQ2Bridge::get_spawn called with id=0");
            return None;
        }
        let snap = find_spawn_by_id(spawn_id)?;
        tracing::debug!(
            spawn_id,
            name = %snap.name,
            "MQ2Bridge::get_spawn ok"
        );
        Some(snap)
    }

    // ── Target ────────────────────────────────────────────────────────────────

    /// Return a snapshot for the current target (`pinstTarget`).
    ///
    /// Equivalent to MQ2's `pTarget`.
    ///
    /// Returns `None` when nothing is targeted or the pointer is invalid.
    pub fn get_target(&self) -> Option<SpawnSnapshot> {
        tracing::debug!("MQ2Bridge::get_target");
        let snap = read_target_snapshot()?;
        tracing::debug!(
            spawn_id = snap.spawn_id,
            name = %snap.name,
            "MQ2Bridge::get_target ok"
        );
        Some(snap)
    }

    // ── Command ───────────────────────────────────────────────────────────────

    /// Execute an EQ slash command (e.g., `"/say hello"`, `"/cast 1"`).
    ///
    /// Equivalent to MQ2's `EzCommand()` / `DoCommand()`.
    ///
    /// Delegates to [`crate::eq::slash_command`].  On non-Windows builds this
    /// is a no-op stub.
    pub fn cmd(&self, slash_command: &str) {
        tracing::info!(cmd = slash_command, "MQ2Bridge::cmd");
        crate::eq::slash_command(slash_command);
    }

    // ── Items ─────────────────────────────────────────────────────────────────

    /// Return a snapshot of the item in a top-level inventory slot.
    ///
    /// `slot` is a 0-based EQ inventory slot index (0 = primary hand,
    /// 13 = range, 22–29 = pack slots, etc.).  Returns `None` when the slot
    /// is empty, the index is out of range, or inventory cannot be read.
    pub fn get_item(&self, slot: i32) -> Option<ItemSnapshot> {
        tracing::debug!(slot, "MQ2Bridge::get_item");
        let snap = read_item_snapshot(slot)?;
        tracing::debug!(
            slot,
            id = snap.id,
            name = %snap.name,
            "MQ2Bridge::get_item ok"
        );
        Some(snap)
    }

    // ── Spells / abilities ────────────────────────────────────────────────────

    /// Return `true` if the named spell is memorized in a gem slot and its
    /// recast timer has expired (ready to cast).
    ///
    /// Equivalent to MQ2's `${Spell[name].Ready}`.
    ///
    /// Looks up the spell by name in the memorized spell array, then checks
    /// the gem's recast timer.  Returns `false` when the spell is not
    /// memorized, when the timer has not expired, or when the spell system
    /// cannot be read.
    ///
    /// This check is best-effort: it reflects the locally cached timer state
    /// and may lag a fraction of a server tick.
    #[cfg(all(windows, feature = "spell-system"))]
    pub fn has_spell_ready(&self, name: &str) -> bool {
        tracing::debug!(spell = name, "MQ2Bridge::has_spell_ready");
        let ready = spell_is_ready(name);
        tracing::debug!(spell = name, ready, "MQ2Bridge::has_spell_ready result");
        ready
    }

    /// Stub variant compiled when the `spell-system` feature is not enabled.
    ///
    /// Always returns `false`.  Gate your code on `cfg(feature = "spell-system")`
    /// or rely on the IPC-layer spell data instead.
    #[cfg(not(all(windows, feature = "spell-system")))]
    pub fn has_spell_ready(&self, name: &str) -> bool {
        tracing::debug!(
            spell = name,
            "MQ2Bridge::has_spell_ready: spell-system unavailable on this build, returning false"
        );
        false
    }

    // ── Group ─────────────────────────────────────────────────────────────────

    /// Return snapshots for all valid group members (excluding self).
    ///
    /// Equivalent to MQ2's `GetGroupMember(n)`.
    ///
    /// Reads the `CGroup` linked from `PcClient::Group` and returns up to
    /// `MAX_GROUP_SIZE − 1` entries.  Offline / null member slots are
    /// silently skipped.  Returns an empty `Vec` when not in a group or when
    /// group data cannot be read.
    pub fn group_members(&self) -> Vec<SpawnSnapshot> {
        tracing::debug!("MQ2Bridge::group_members");
        let members = read_group_members();
        tracing::debug!(count = members.len(), "MQ2Bridge::group_members ok");
        members
    }
}

// ─── Platform-specific helpers ────────────────────────────────────────────────

// All helpers follow the same pattern:
// - Windows: real implementation gated behind `#[cfg(windows)]`
// - Non-Windows: stub that returns `None` / empty so tests compile everywhere

/// Snapshot the local player from `PINST_LOCAL_PLAYER`.
fn read_local_player_snapshot() -> Option<PlayerSnapshot> {
    #[cfg(windows)]
    {
        use std::sync::atomic::Ordering;
        use textquest_common::offsets;

        let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
        if eq_base == 0 {
            tracing::warn!("read_local_player_snapshot: EQ base not set");
            return None;
        }

        let addr = offsets::rebase(offsets::PINST_LOCAL_PLAYER, eq_base)?;
        // SAFETY: addr is the rebased PINST_LOCAL_PLAYER global; dereferencing
        // yields the PlayerClient* (null when not logged in).
        let player_ptr: *mut super::RawPlayerClient =
            unsafe { *(addr as *const *mut super::RawPlayerClient) };

        // SAFETY: player_ptr is null-checked inside PlayerClientRef::new().
        let ref_ = unsafe { super::PlayerClientRef::new(player_ptr) }?;
        Some(ref_.snapshot())
    }

    #[cfg(not(windows))]
    None
}

/// Walk the spawn list to find a spawn with the given id.
fn find_spawn_by_id(spawn_id: u32) -> Option<SpawnSnapshot> {
    #[cfg(windows)]
    {
        use std::mem::size_of;
        use std::sync::atomic::Ordering;
        use textquest_common::offsets::{self, spawn_manager};

        let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
        if eq_base == 0 {
            tracing::warn!("find_spawn_by_id: EQ base not set");
            return None;
        }

        let mgr_ptr_addr = offsets::rebase(offsets::PINST_SPAWN_MANAGER, eq_base)?;

        // Validate the manager global pointer is readable.
        if !crate::hooks::game_loop::is_readable(mgr_ptr_addr, size_of::<usize>()) {
            tracing::warn!("find_spawn_by_id: PINST_SPAWN_MANAGER not readable");
            return None;
        }

        // SAFETY: mgr_ptr_addr validated as readable above.
        let mgr_ptr = unsafe { *(mgr_ptr_addr as *const usize) };
        if mgr_ptr == 0 {
            tracing::warn!("find_spawn_by_id: SpawnManager pointer is null");
            return None;
        }

        let list_addr = mgr_ptr + spawn_manager::PLAYER_LIST;
        if !crate::hooks::game_loop::is_readable(list_addr, size_of::<usize>()) {
            return None;
        }

        // SAFETY: list_addr validated above.
        let mut current = unsafe { *(list_addr as *const usize) };

        const MAX_WALK: usize = 2000;
        let mut walked = 0usize;

        while current != 0 && walked < MAX_WALK {
            walked += 1;

            // Alignment sanity check.
            if current % core::mem::align_of::<usize>() != 0 {
                break;
            }

            // Validate spawn_id field is readable before touching it.
            if !crate::hooks::game_loop::is_readable(
                current + textquest_common::offsets::player_base::SPAWN_ID,
                size_of::<u32>(),
            ) {
                break;
            }

            // SAFETY: validated above.
            let sid = unsafe {
                *((current + textquest_common::offsets::player_base::SPAWN_ID) as *const u32)
            };

            if sid == spawn_id {
                // Found — snapshot via offset-based read.
                // SAFETY: current points to a valid PlayerClient (non-zero spawn_id confirmed).
                let snap = unsafe { super::read_spawn_snapshot(current as *const u8) };
                return snap;
            }

            // Follow NEXT pointer.
            let next_addr = current + textquest_common::offsets::player_base::NEXT;
            if !crate::hooks::game_loop::is_readable(next_addr, size_of::<usize>()) {
                break;
            }
            // SAFETY: next_addr validated above.
            current = unsafe { *(next_addr as *const usize) };
        }

        tracing::debug!(spawn_id, walked, "find_spawn_by_id: not found");
        None
    }

    #[cfg(not(windows))]
    {
        let _ = spawn_id;
        None
    }
}

/// Read the current target snapshot from `PINST_TARGET`.
fn read_target_snapshot() -> Option<SpawnSnapshot> {
    #[cfg(windows)]
    {
        use std::sync::atomic::Ordering;
        use textquest_common::offsets;

        let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
        if eq_base == 0 {
            tracing::warn!("read_target_snapshot: EQ base not set");
            return None;
        }

        let target_ptr_addr = offsets::rebase(offsets::PINST_TARGET, eq_base)?;

        // SAFETY: target_ptr_addr is the rebased PINST_TARGET global.
        let target_ptr = unsafe { *(target_ptr_addr as *const usize) };
        if target_ptr == 0 {
            return None; // No target — expected, not a warning.
        }

        // SAFETY: target_ptr is the EQ PlayerClient* for the current target,
        // non-zero means a spawn is targeted. read_spawn_snapshot validates internally.
        unsafe { super::read_spawn_snapshot(target_ptr as *const u8) }
    }

    #[cfg(not(windows))]
    None
}

/// Read an item snapshot from a top-level inventory slot index.
///
/// Delegates to [`crate::eq::inventory::query_top_level_slot_item`] and
/// converts the result into an [`ItemSnapshot`].
fn read_item_snapshot(slot: i32) -> Option<ItemSnapshot> {
    #[cfg(windows)]
    {
        use std::sync::atomic::Ordering;

        let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
        if eq_base == 0 {
            tracing::warn!("read_item_snapshot: EQ base not set");
            return None;
        }

        // Use the existing inventory query helper which handles the PC item
        // manager pointer chain and slot validation.
        let info = crate::eq::inventory::query_top_level_slot_item(
            eq_base,
            0, /* ItemLocation::Personal */
            slot as i16,
        )?;

        Some(ItemSnapshot {
            id: info.id as u32,
            name: info.name,
            stack_count: info.stack_count as u32,
            is_stackable: info.is_stackable,
            inv_slot: slot,
        })
    }

    #[cfg(not(windows))]
    {
        let _ = slot;
        None
    }
}

/// Read group member snapshots from `PcClient::Group`.
///
/// Walks the `CGroup::m_groupMembers[6]` array in the local player's
/// `PcClient` and converts each non-null member into a `SpawnSnapshot` by
/// looking them up in the spawn list.
fn read_group_members() -> Vec<SpawnSnapshot> {
    #[cfg(windows)]
    {
        use std::mem::size_of;
        use std::sync::atomic::Ordering;
        use textquest_common::offsets::{self, group};

        let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
        if eq_base == 0 {
            tracing::warn!("read_group_members: EQ base not set");
            return Vec::new();
        }

        // Reach the local player's PcClient struct.
        let Some(player_addr) = offsets::rebase(offsets::PINST_LOCAL_PLAYER, eq_base) else {
            return Vec::new();
        };

        if !crate::hooks::game_loop::is_readable(player_addr, size_of::<usize>()) {
            return Vec::new();
        }

        // SAFETY: player_addr validated above.
        let pc_ptr = unsafe { *(player_addr as *const usize) };
        if pc_ptr == 0 {
            return Vec::new();
        }

        // PcClient::Group is a CGroup* at offset PC_CLIENT_GROUP_PTR within PcClient.
        let group_ptr_addr = pc_ptr + group::PC_CLIENT_GROUP_PTR;
        if !crate::hooks::game_loop::is_readable(group_ptr_addr, size_of::<usize>()) {
            tracing::debug!("read_group_members: group ptr not readable");
            return Vec::new();
        }

        // SAFETY: group_ptr_addr validated above.
        let group_ptr = unsafe { *(group_ptr_addr as *const usize) };
        if group_ptr == 0 {
            // Not in a group.
            return Vec::new();
        }

        // CGroup::m_groupMembers[MAX_GROUP_SIZE] — array of CGroupMember* at
        // group_ptr + GROUP_MEMBERS.  Each entry is a pointer-sized slot.
        let mut members = Vec::with_capacity(group::MAX_GROUP_SIZE);

        for i in 0..group::MAX_GROUP_SIZE {
            let member_slot_addr = group_ptr + group::GROUP_MEMBERS + i * size_of::<usize>();

            if !crate::hooks::game_loop::is_readable(member_slot_addr, size_of::<usize>()) {
                continue;
            }

            // SAFETY: member_slot_addr validated above.
            let member_ptr = unsafe { *(member_slot_addr as *const usize) };
            if member_ptr == 0 {
                continue; // Empty slot.
            }

            // Read member name via CXStr (pointer → CStrRep → utf8 data).
            // Use it to look up the spawn in the spawn list by name.
            let member_name = read_cxstr(member_ptr + group::MEMBER_NAME_CXSTR);

            // Read whether the member is offline — skip offline members since
            // they have no live spawn.
            let is_offline =
                if crate::hooks::game_loop::is_readable(member_ptr + group::MEMBER_IS_OFFLINE, 1) {
                    // SAFETY: validated above.
                    unsafe { *((member_ptr + group::MEMBER_IS_OFFLINE) as *const bool) }
                } else {
                    false
                };

            if is_offline {
                tracing::debug!(name = %member_name, "read_group_members: skipping offline member");
                continue;
            }

            // Find this member in the spawn list by name.
            if let Some(snap) = find_spawn_by_name(&member_name) {
                members.push(snap);
            } else {
                tracing::debug!(
                    name = %member_name,
                    slot = i,
                    "read_group_members: member spawn not found in list"
                );
            }
        }

        members
    }

    #[cfg(not(windows))]
    Vec::new()
}

/// Read a `CXStr` value as a `String`.
///
/// `CXStr` is a single pointer to a `CStrRep`.  `CStrRep::utf8` is at offset
/// `CXSTR_REP_UTF8` (0x18) within the rep.
#[cfg(windows)]
fn read_cxstr(cxstr_addr: usize) -> String {
    use std::mem::size_of;
    use textquest_common::offsets::group;

    if !crate::hooks::game_loop::is_readable(cxstr_addr, size_of::<usize>()) {
        return String::new();
    }

    // SAFETY: cxstr_addr validated above.
    let rep_ptr = unsafe { *(cxstr_addr as *const usize) };
    if rep_ptr == 0 {
        return String::new();
    }

    let utf8_addr = rep_ptr + group::CXSTR_REP_UTF8;
    // Read up to 64 bytes of the name.
    const MAX_LEN: usize = 64;
    if !crate::hooks::game_loop::is_readable(utf8_addr, MAX_LEN) {
        return String::new();
    }

    let mut buf = [0u8; MAX_LEN];
    // SAFETY: utf8_addr validated above.
    unsafe {
        core::ptr::copy_nonoverlapping(utf8_addr as *const u8, buf.as_mut_ptr(), MAX_LEN);
    }

    let end = buf.iter().position(|&b| b == 0).unwrap_or(MAX_LEN);
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

/// Walk the spawn list and return the first spawn whose name matches `name`
/// (case-sensitive).
#[cfg(windows)]
fn find_spawn_by_name(name: &str) -> Option<SpawnSnapshot> {
    use std::mem::size_of;
    use std::sync::atomic::Ordering;
    use textquest_common::offsets::{self, spawn_manager};

    if name.is_empty() {
        return None;
    }

    let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
    if eq_base == 0 {
        return None;
    }

    let mgr_ptr_addr = offsets::rebase(offsets::PINST_SPAWN_MANAGER, eq_base)?;
    if !crate::hooks::game_loop::is_readable(mgr_ptr_addr, size_of::<usize>()) {
        return None;
    }

    // SAFETY: mgr_ptr_addr validated above.
    let mgr_ptr = unsafe { *(mgr_ptr_addr as *const usize) };
    if mgr_ptr == 0 {
        return None;
    }

    let list_addr = mgr_ptr + spawn_manager::PLAYER_LIST;
    if !crate::hooks::game_loop::is_readable(list_addr, size_of::<usize>()) {
        return None;
    }

    // SAFETY: list_addr validated above.
    let mut current = unsafe { *(list_addr as *const usize) };

    const MAX_WALK: usize = 2000;
    let mut walked = 0usize;

    while current != 0 && walked < MAX_WALK {
        walked += 1;

        if current % core::mem::align_of::<usize>() != 0 {
            break;
        }

        if !crate::hooks::game_loop::is_readable(
            current + textquest_common::offsets::player_base::SPAWN_ID,
            size_of::<u32>(),
        ) {
            break;
        }

        // SAFETY: validated above.
        let sid = unsafe {
            *((current + textquest_common::offsets::player_base::SPAWN_ID) as *const u32)
        };

        if sid != 0 {
            // SAFETY: spawn with non-zero id; read_spawn_snapshot validates internally.
            let snap = unsafe { super::read_spawn_snapshot(current as *const u8) };
            if let Some(s) = snap {
                if s.name == name {
                    return Some(s);
                }
            }
        }

        let next_addr = current + textquest_common::offsets::player_base::NEXT;
        if !crate::hooks::game_loop::is_readable(next_addr, size_of::<usize>()) {
            break;
        }
        // SAFETY: next_addr validated above.
        current = unsafe { *(next_addr as *const usize) };
    }

    None
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mq2::{ItemSnapshot, PlayerSnapshot, SpawnSnapshot};

    // ── Bridge construction ───────────────────────────────────────────────────

    #[test]
    fn bridge_new_is_fine() {
        // Bridge creation must not panic or require any global state.
        let _bridge = MQ2Bridge::new();
    }

    // ── get_player — no EQ base ───────────────────────────────────────────────

    #[test]
    fn get_player_returns_none_when_no_eq_base() {
        // EQ_BASE is 0 (default) on non-Windows / test hosts → None.
        let bridge = MQ2Bridge::new();
        let result = bridge.get_player();
        // On non-Windows the platform stub always returns None.
        // On Windows with EQ_BASE == 0 it also returns None.
        assert!(result.is_none());
    }

    // ── get_spawn — zero id ───────────────────────────────────────────────────

    #[test]
    fn get_spawn_zero_id_returns_none() {
        let bridge = MQ2Bridge::new();
        assert!(bridge.get_spawn(0).is_none());
    }

    #[test]
    fn get_spawn_nonzero_id_returns_none_without_eq_base() {
        let bridge = MQ2Bridge::new();
        // No EQ base → spawn manager not reachable.
        assert!(bridge.get_spawn(999).is_none());
    }

    // ── get_target ────────────────────────────────────────────────────────────

    #[test]
    fn get_target_returns_none_without_eq_base() {
        let bridge = MQ2Bridge::new();
        assert!(bridge.get_target().is_none());
    }

    // ── cmd — no-op on non-Windows ────────────────────────────────────────────

    #[test]
    fn cmd_does_not_panic_on_non_windows() {
        // slash_command() is a no-op stub on non-Windows; bridge must not panic.
        let bridge = MQ2Bridge::new();
        bridge.cmd("/say Hello Norrath");
        bridge.cmd("");
        bridge.cmd("/cast 1");
    }

    // ── get_item ──────────────────────────────────────────────────────────────

    #[test]
    fn get_item_returns_none_without_eq_base() {
        let bridge = MQ2Bridge::new();
        assert!(bridge.get_item(0).is_none());
        assert!(bridge.get_item(22).is_none());
        assert!(bridge.get_item(-1).is_none());
    }

    // ── has_spell_ready ───────────────────────────────────────────────────────

    #[test]
    fn has_spell_ready_stub_returns_false() {
        let bridge = MQ2Bridge::new();
        // Non-Windows and non-spell-system builds always use the stub.
        assert!(!bridge.has_spell_ready("Complete Heal"));
        assert!(!bridge.has_spell_ready(""));
    }

    // ── group_members ─────────────────────────────────────────────────────────

    #[test]
    fn group_members_empty_without_eq_base() {
        let bridge = MQ2Bridge::new();
        let members = bridge.group_members();
        assert!(members.is_empty());
    }

    // ── Snapshot types are clone/debug ────────────────────────────────────────

    #[test]
    fn player_snapshot_clone_and_debug() {
        let snap = PlayerSnapshot {
            spawn_id: 1,
            name: "Vaniki".to_string(),
            y: 0.0,
            x: 0.0,
            z: 0.0,
            heading: 0.0,
            speed_run: 0.0,
            stand_state: 0,
            level: 50,
            class: 3,
            spawn_type: 0,
        };
        let cloned = snap.clone();
        assert_eq!(snap, cloned);
        let _ = format!("{:?}", snap);
    }

    #[test]
    fn spawn_snapshot_clone_and_debug() {
        let snap = SpawnSnapshot {
            spawn_id: 42,
            name: "a_goblin".to_string(),
            y: 10.0,
            x: 20.0,
            z: 0.0,
            heading: 0.0,
            speed_run: 0.0,
            stand_state: 0,
            level: 5,
            class: 0,
            spawn_type: 1,
            race: 3,
            hp_current: 100,
            hp_max: 100,
        };
        let cloned = snap.clone();
        assert_eq!(snap, cloned);
        let _ = format!("{:?}", snap);
    }

    #[test]
    fn item_snapshot_clone_and_debug() {
        let snap = ItemSnapshot {
            id: 7777,
            name: "Fungus Covered Scale Tunic".to_string(),
            stack_count: 1,
            is_stackable: false,
            inv_slot: 17,
        };
        let cloned = snap.clone();
        assert_eq!(snap, cloned);
        let _ = format!("{:?}", snap);
    }

    // ── Stub platform read functions ──────────────────────────────────────────

    #[test]
    fn read_local_player_snapshot_stub_is_none() {
        assert!(read_local_player_snapshot().is_none());
    }

    #[test]
    fn read_target_snapshot_stub_is_none() {
        assert!(read_target_snapshot().is_none());
    }

    #[test]
    fn find_spawn_by_id_zero_returns_none() {
        // Zero id is guarded at bridge level, but also safe at helper level.
        assert!(find_spawn_by_id(0).is_none());
    }

    #[test]
    fn read_group_members_stub_is_empty() {
        assert!(read_group_members().is_empty());
    }

    #[test]
    fn read_item_snapshot_stub_is_none() {
        assert!(read_item_snapshot(0).is_none());
        assert!(read_item_snapshot(22).is_none());
    }

    // ── Multiple bridge instances are independent ─────────────────────────────

    #[test]
    fn multiple_bridges_are_independent() {
        let b1 = MQ2Bridge::new();
        let b2 = MQ2Bridge::new();
        assert!(b1.get_player().is_none());
        assert!(b2.get_player().is_none());
        assert!(b1.get_spawn(1).is_none());
        assert!(b2.get_spawn(1).is_none());
    }
}
