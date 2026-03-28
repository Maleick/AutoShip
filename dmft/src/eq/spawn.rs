use anyhow::{Context, Result};
use crate::process::memory::ProcessHandle;
use dmft_common::offsets::{self, player_base, player_zone, spawn_manager};
use super::structs::{SpawnInfo, SpawnType, EqClass};

/// Read a single spawn's data from the process at the given PlayerClient address.
pub fn read_spawn(proc: &ProcessHandle, addr: usize) -> Result<SpawnInfo> {
    let name = proc.read_string(addr + player_base::NAME, 64)
        .unwrap_or_else(|_| String::from("<unreadable>"));
    let displayed_name = proc.read_string(addr + player_base::DISPLAYED_NAME, 64)
        .unwrap_or_else(|_| String::from("<unreadable>"));
    let lastname = proc.read_string(addr + player_base::LASTNAME, 32)
        .unwrap_or_default();

    let spawn_id = proc.read::<u32>(addr + player_base::SPAWN_ID)
        .unwrap_or(0);
    let spawn_type_id = proc.read::<u8>(addr + player_base::TYPE)
        .unwrap_or(255);

    let y = proc.read::<f32>(addr + player_base::Y).unwrap_or(0.0);
    let x = proc.read::<f32>(addr + player_base::X).unwrap_or(0.0);
    let z = proc.read::<f32>(addr + player_base::Z).unwrap_or(0.0);
    let heading = proc.read::<f32>(addr + player_base::HEADING).unwrap_or(0.0);

    let level = proc.read::<u8>(addr + player_zone::LEVEL).unwrap_or(0);
    // Read class as both u8 and try nearby offsets for diagnostics
    let class_id = proc.read::<u8>(addr + player_zone::CHAR_CLASS).unwrap_or(0);
    // If class_id looks wrong, scan nearby for the right value
    if tracing::enabled!(tracing::Level::TRACE) {
        for delta in [-4i32, -3, -2, -1, 0, 1, 2, 3, 4] {
            let probe_offset = (player_zone::CHAR_CLASS as i32 + delta) as usize;
            let val = proc.read::<u8>(addr + probe_offset).unwrap_or(255);
            if val > 0 && val <= 16 {
                tracing::trace!(offset = format!("+{:#x}", probe_offset), value = val, "Possible class_id");
            }
        }
    }
    let hp_current = proc.read::<i64>(addr + player_zone::HP_CURRENT).unwrap_or(0);
    let hp_max = proc.read::<i64>(addr + player_zone::HP_MAX).unwrap_or(0);
    let mana_current = proc.read::<i32>(addr + player_zone::MANA_CURRENT).unwrap_or(0);
    let mana_max = proc.read::<i32>(addr + player_zone::MANA_MAX).unwrap_or(0);
    let endurance_current = proc.read::<i32>(addr + player_zone::ENDURANCE_CURRENT).unwrap_or(0);
    let endurance_max = proc.read::<u32>(addr + player_zone::ENDURANCE_MAX).unwrap_or(0);

    Ok(SpawnInfo {
        name,
        displayed_name,
        lastname,
        spawn_id,
        spawn_type: SpawnType::from_id(spawn_type_id),
        level,
        class_id,
        class: EqClass::from_id(class_id),
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
    let player_addr = proc.read_ptr(player_ptr_addr)
        .context("Failed to read pinstLocalPlayer pointer")?;

    if player_addr == 0 {
        anyhow::bail!("pinstLocalPlayer is null — not logged in?");
    }

    read_spawn(proc, player_addr)
        .context("Failed to read local player spawn data")
}

/// Read the current target's spawn info, if any.
pub fn read_target(proc: &ProcessHandle, eq_base: u64) -> Result<Option<SpawnInfo>> {
    let target_ptr_addr = offsets::rebase(offsets::PINST_TARGET, eq_base)
        .context("rebase underflow for pinstTarget")?;
    let target_addr = proc.read_ptr(target_ptr_addr)
        .context("Failed to read pinstTarget pointer")?;

    if target_addr == 0 {
        return Ok(None);
    }

    let spawn = read_spawn(proc, target_addr)
        .context("Failed to read target spawn data")?;
    Ok(Some(spawn))
}

/// Iterate all spawns in the spawn manager's linked list.
/// Returns up to `max_count` spawns to prevent infinite loops on corrupt data.
pub fn read_all_spawns(proc: &ProcessHandle, eq_base: u64, max_count: usize) -> Result<Vec<SpawnInfo>> {
    let mgr_ptr_addr = offsets::rebase(offsets::PINST_SPAWN_MANAGER, eq_base)
        .context("rebase underflow for pinstSpawnManager")?;
    let mgr_addr = proc.read_ptr(mgr_ptr_addr)
        .context("Failed to read pinstSpawnManager pointer")?;

    if mgr_addr == 0 {
        anyhow::bail!("pinstSpawnManager is null — not in a zone?");
    }

    // Read first node from TList at offset spawn_manager::PLAYER_LIST
    // TList has m_pFirstNode at offset 0x00 within the TList struct
    let list_addr = mgr_addr + spawn_manager::PLAYER_LIST;
    let mut current = proc.read_ptr(list_addr)
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
