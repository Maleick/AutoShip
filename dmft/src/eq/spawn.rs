use super::structs::{EqClass, GroupInfo, SpawnInfo, SpawnType, StandState};
use crate::process::memory::ProcessHandle;
use anyhow::{Context, Result};
use dmft_common::offsets::{self, actor_client, group, player_base, player_zone, spawn_manager};

/// Read a single spawn's data from the process at the given PlayerClient address.
pub fn read_spawn(proc: &ProcessHandle, addr: usize) -> Result<SpawnInfo> {
    let name = proc
        .read_string(addr + player_base::NAME, 64)
        .unwrap_or_else(|_| String::from("<unreadable>"));
    let displayed_name = proc
        .read_string(addr + player_base::DISPLAYED_NAME, 64)
        .unwrap_or_else(|_| String::from("<unreadable>"));
    let lastname = proc
        .read_string(addr + player_base::LASTNAME, 32)
        .unwrap_or_default();

    let spawn_id = proc.read::<u32>(addr + player_base::SPAWN_ID).unwrap_or(0);
    let spawn_type_id = proc.read::<u8>(addr + player_base::TYPE).unwrap_or(255);

    let y = proc.read::<f32>(addr + player_base::Y).unwrap_or(0.0);
    let x = proc.read::<f32>(addr + player_base::X).unwrap_or(0.0);
    let z = proc.read::<f32>(addr + player_base::Z).unwrap_or(0.0);
    let heading = proc.read::<f32>(addr + player_base::HEADING).unwrap_or(0.0);

    let level = proc.read::<u8>(addr + player_zone::LEVEL).unwrap_or(0);
    // Class is in ActorClient (mActorClient at 0x0FC0 + ActorBase.Class at 0x1C)
    let class_id = proc.read::<u8>(addr + actor_client::CHAR_CLASS).unwrap_or(0);
    let stand_state_id = proc.read::<u8>(addr + player_zone::STANDSTATE).unwrap_or(0);
    let hp_current = proc
        .read::<i64>(addr + player_zone::HP_CURRENT)
        .unwrap_or(0);
    let hp_max = proc.read::<i64>(addr + player_zone::HP_MAX).unwrap_or(0);
    let mana_current = proc
        .read::<i32>(addr + player_zone::MANA_CURRENT)
        .unwrap_or(0);
    let mana_max = proc.read::<i32>(addr + player_zone::MANA_MAX).unwrap_or(0);
    let endurance_current = proc
        .read::<i32>(addr + player_zone::ENDURANCE_CURRENT)
        .unwrap_or(0);
    let endurance_max = proc
        .read::<u32>(addr + player_zone::ENDURANCE_MAX)
        .unwrap_or(0);

    Ok(SpawnInfo {
        name,
        displayed_name,
        lastname,
        spawn_id,
        spawn_type: SpawnType::from_id(spawn_type_id),
        level,
        class_id,
        class: EqClass::from_id(class_id),
        stand_state: StandState::from_id(stand_state_id),
        x,
        y,
        z,
        heading,
        hp_current,
        hp_max,
        mana_current,
        mana_max,
        endurance_current,
        endurance_max,
    })
}

/// Read the local player's spawn info.
pub fn read_local_player(proc: &ProcessHandle, eq_base: u64) -> Result<SpawnInfo> {
    let player_ptr_addr = offsets::rebase(offsets::PINST_LOCAL_PLAYER, eq_base)
        .context("rebase underflow for pinstLocalPlayer")?;
    let player_addr = proc
        .read_ptr(player_ptr_addr)
        .context("Failed to read pinstLocalPlayer pointer")?;

    if player_addr == 0 {
        anyhow::bail!("pinstLocalPlayer is null — not logged in?");
    }

    read_spawn(proc, player_addr).context("Failed to read local player spawn data")
}

/// Read the current target's spawn info, if any.
pub fn read_target(proc: &ProcessHandle, eq_base: u64) -> Result<Option<SpawnInfo>> {
    let target_ptr_addr = offsets::rebase(offsets::PINST_TARGET, eq_base)
        .context("rebase underflow for pinstTarget")?;
    let target_addr = proc
        .read_ptr(target_ptr_addr)
        .context("Failed to read pinstTarget pointer")?;

    if target_addr == 0 {
        return Ok(None);
    }

    let spawn = read_spawn(proc, target_addr).context("Failed to read target spawn data")?;
    Ok(Some(spawn))
}

/// Iterate all spawns in the spawn manager's linked list.
/// Returns up to `max_count` spawns to prevent infinite loops on corrupt data.
pub fn read_all_spawns(
    proc: &ProcessHandle,
    eq_base: u64,
    max_count: usize,
) -> Result<Vec<SpawnInfo>> {
    let mgr_ptr_addr = offsets::rebase(offsets::PINST_SPAWN_MANAGER, eq_base)
        .context("rebase underflow for pinstSpawnManager")?;
    let mgr_addr = proc
        .read_ptr(mgr_ptr_addr)
        .context("Failed to read pinstSpawnManager pointer")?;

    if mgr_addr == 0 {
        anyhow::bail!("pinstSpawnManager is null — not in a zone?");
    }

    // Read first node from TList at offset spawn_manager::PLAYER_LIST
    // TList has m_pFirstNode at offset 0x00 within the TList struct
    let list_addr = mgr_addr + spawn_manager::PLAYER_LIST;
    let mut current = proc
        .read_ptr(list_addr)
        .context("Failed to read first spawn from TList")?;

    let mut spawns = Vec::new();

    while current != 0 && spawns.len() < max_count {
        match read_spawn(proc, current) {
            Ok(spawn) => {
                tracing::trace!(addr = format!("{:#x}", current), name = %spawn.name, "Read spawn OK");
                spawns.push(spawn);
            }
            Err(e) => {
                tracing::warn!(addr = format!("{:#x}", current), error = %e, "Failed to read spawn, stopping iteration");
                break;
            }
        }

        // Follow m_pNext at offset 0x08 (TListNode.m_pNext)
        let next_addr = current + player_base::NEXT;
        match proc.read_ptr(next_addr) {
            Ok(next) => {
                tracing::trace!(
                    current_addr = format!("{:#x}", current),
                    next_ptr_addr = format!("{:#x}", next_addr),
                    next_value = format!("{:#x}", next),
                    "NEXT pointer"
                );
                current = next;
            }
            Err(e) => {
                tracing::warn!(
                    addr = format!("{:#x}", next_addr),
                    error = %e,
                    spawn_count = spawns.len(),
                    "Failed to read NEXT pointer, stopping iteration"
                );
                break;
            }
        }
    }

    Ok(spawns)
}

/// Read a CXStr (EQ's string type) from memory.
/// CXStr is a pointer to CStrRep; the UTF-8 data lives at CStrRep+0x18.
fn read_cxstr(proc: &ProcessHandle, cxstr_addr: usize, max_len: usize) -> Result<String> {
    let rep_ptr = proc
        .read_ptr(cxstr_addr)
        .context("Failed to read CXStr.m_data pointer")?;
    if rep_ptr == 0 {
        return Ok(String::new());
    }
    proc.read_string(rep_ptr + group::CXSTR_REP_UTF8, max_len)
        .context("Failed to read CStrRep.utf8 data")
}

/// Read group membership info from the local PC's CGroup pointer.
///
/// Path: pLocalPC -> +0x2EB0 (CGroup*) -> CGroupBase members array.
/// Each CGroupMember has a CXStr Name at offset 0x08.
pub fn read_group_info(proc: &ProcessHandle, eq_base: u64) -> Result<Option<GroupInfo>> {
    // Read pLocalPC
    let pc_ptr_addr = offsets::rebase(offsets::PINST_LOCAL_PC, eq_base)
        .context("rebase underflow for pinstLocalPC")?;
    let pc_addr = proc
        .read_ptr(pc_ptr_addr)
        .context("Failed to read pinstLocalPC pointer")?;
    if pc_addr == 0 {
        return Ok(None);
    }

    // Read CGroup* from PcClient
    let group_ptr = proc
        .read_ptr(pc_addr + group::PC_CLIENT_GROUP_PTR)
        .context("Failed to read PcClient.Group pointer")?;
    if group_ptr == 0 {
        return Ok(None);
    }

    // Read leader pointer and name
    let leader_ptr = proc
        .read_ptr(group_ptr + group::GROUP_LEADER)
        .unwrap_or(0);
    let leader_name = if leader_ptr != 0 {
        read_cxstr(proc, leader_ptr + group::MEMBER_NAME_CXSTR, 64).unwrap_or_default()
    } else {
        String::new()
    };

    // Read member pointers (6 slots)
    let mut members = Vec::new();
    for i in 0..group::MAX_GROUP_SIZE {
        let member_ptr_addr = group_ptr + group::GROUP_MEMBERS + (i * 8);
        let member_ptr = proc.read_ptr(member_ptr_addr).unwrap_or(0);
        if member_ptr == 0 {
            continue;
        }
        let name = read_cxstr(proc, member_ptr + group::MEMBER_NAME_CXSTR, 64)
            .unwrap_or_default();
        if !name.is_empty() {
            members.push(name);
        }
    }

    if members.is_empty() {
        return Ok(None);
    }

    let member_count = members.len() as u8;
    Ok(Some(GroupInfo {
        leader_name,
        members,
        member_count,
    }))
}

/// Read a range of raw bytes from a spawn's memory for offset calibration.
///
/// Given a spawn address and an offset range, reads `len` bytes starting at
/// `spawn_addr + start_offset`. Returns the bytes as a `Vec<u8>`.
/// Useful for hex-dumping around suspected offsets in the TUI.
pub fn read_spawn_bytes(
    proc: &ProcessHandle,
    spawn_addr: usize,
    start_offset: usize,
    len: usize,
) -> Result<Vec<u8>> {
    let addr = spawn_addr + start_offset;
    proc.read_bytes(addr, len)
        .with_context(|| format!("Failed to read {} bytes at spawn+{:#x}", len, start_offset))
}
