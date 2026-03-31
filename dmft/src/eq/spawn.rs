use super::structs::{BuffSlot, CastState, EqClass, GroupInfo, SpawnInfo, SpawnType, StandState};
use crate::process::memory::ProcessHandle;
use anyhow::{Context, Result};
use dmft_common::offsets::{
    self, actor_client, group, player_base, player_zone, spawn_manager, zone_info,
};

/// Read a single spawn's data from the process at the given PlayerClient address.
pub fn read_spawn(proc: &ProcessHandle, addr: usize) -> Result<SpawnInfo> {
    // Critical fields — hard fail if any are unreadable (corrupt memory → skip spawn)
    let name = proc
        .read_string(addr + player_base::NAME, 64)
        .context("critical field: name")?;
    let spawn_type_id = proc
        .read::<u8>(addr + player_base::TYPE)
        .context("critical field: spawn_type")?;
    let y = proc
        .read::<f32>(addr + player_base::Y)
        .context("critical field: y")?;
    let x = proc
        .read::<f32>(addr + player_base::X)
        .context("critical field: x")?;
    let z = proc
        .read::<f32>(addr + player_base::Z)
        .context("critical field: z")?;

    // Non-critical fields — degrade gracefully with defaults
    let displayed_name = proc
        .read_string(addr + player_base::DISPLAYED_NAME, 64)
        .unwrap_or_else(|_| String::from("<unreadable>"));
    let lastname = proc
        .read_string(addr + player_base::LASTNAME, 32)
        .unwrap_or_default();

    let spawn_id = proc.read::<u32>(addr + player_base::SPAWN_ID).unwrap_or(0);
    let heading = proc.read::<f32>(addr + player_base::HEADING).unwrap_or(0.0);

    // Diagnostic: if position looks suspicious (all near-zero) but name is valid,
    // hex-dump the region around the position offsets so we can verify them.
    if x.abs() < 1.0
        && y.abs() < 1.0
        && !name.is_empty()
        && name != "<unreadable>"
        && let Ok(bytes) = proc.read_bytes(addr + 0x060, 0x50)
    {
        tracing::warn!(
            spawn_addr = format!("{:#x}", addr),
            name = %name,
            spawn_id,
            x, y, z,
            hex_0x060_to_0x0b0 = format!("{:02x?}", bytes),
            "Position near zero — hex dump of PlayerBase 0x060..0x0b0 for offset verification"
        );
    }

    let level = proc
        .read::<u8>(addr + player_zone::LEVEL)
        .context("critical field: level")?;
    // Class is a direct uint8_t field in PlayerZoneClient at 0x0420
    let class_id = proc.read::<u8>(addr + player_zone::CHAR_CLASS).unwrap_or(0);
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

    let gm_flag = proc.read::<u8>(addr + player_zone::GM).unwrap_or(0);
    let race_id = proc.read::<i32>(addr + actor_client::RACE).unwrap_or(0) as u32;

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
        is_gm: gm_flag != 0,
        race_id,
        buff_slots: Vec::new(),
        cast_state: None,
    })
}

/// Read the local player's spawn info, including buff slots and cast state.
pub fn read_local_player(proc: &ProcessHandle, eq_base: u64) -> Result<SpawnInfo> {
    let player_ptr_addr = offsets::rebase(offsets::PINST_LOCAL_PLAYER, eq_base)
        .context("rebase underflow for pinstLocalPlayer")?;
    let player_addr = proc
        .read_ptr(player_ptr_addr)
        .context("Failed to read pinstLocalPlayer pointer")?;

    if player_addr == 0 {
        anyhow::bail!("pinstLocalPlayer is null — not logged in?");
    }

    tracing::debug!(
        player_ptr_addr = format!("{:#x}", player_ptr_addr),
        player_addr = format!("{:#x}", player_addr),
        eq_base = format!("{:#x}", eq_base),
        "read_local_player pointer chain"
    );

    let mut spawn =
        read_spawn(proc, player_addr).context("Failed to read local player spawn data")?;
    spawn.buff_slots = read_buff_slots(proc, eq_base);
    spawn.cast_state = read_cast_state(proc, eq_base);
    Ok(spawn)
}

/// Read buff slots for the local player via PINST_LOCAL_PC.
/// On non-Windows builds returns an empty vec (stub).
pub fn read_buff_slots(proc: &ProcessHandle, eq_base: u64) -> Vec<BuffSlot> {
    #[cfg(not(windows))]
    {
        let _ = (proc, eq_base);
        Vec::new()
    }
    #[cfg(windows)]
    {
        use dmft_common::offsets::buff_slots as bs;
        let pc_ptr_addr = match offsets::rebase(offsets::PINST_LOCAL_PC, eq_base) {
            Some(a) => a,
            None => return Vec::new(),
        };
        let pc_addr = match proc.read_ptr(pc_ptr_addr) {
            Ok(a) if a != 0 => a,
            _ => return Vec::new(),
        };
        let mut slots = Vec::new();
        for i in 0..bs::MAX_BUFF_SLOTS {
            let slot_addr = pc_addr + bs::BUFF_ARRAY_OFFSET + i * bs::BUFF_ENTRY_SIZE;
            let spell_id = proc.read::<u32>(slot_addr + bs::SPELL_ID).unwrap_or(0xFFFF);
            let duration_ticks = proc
                .read::<i32>(slot_addr + bs::DURATION_TICKS)
                .unwrap_or(0);
            let caster_level = proc.read::<u8>(slot_addr + bs::CASTER_LEVEL).unwrap_or(0);
            slots.push(BuffSlot {
                spell_id,
                duration_ticks,
                caster_level,
            });
        }
        slots
    }
}

/// Read cast state for the local player via PINST_LOCAL_PC.
/// On non-Windows builds returns None (stub).
pub fn read_cast_state(proc: &ProcessHandle, eq_base: u64) -> Option<CastState> {
    #[cfg(not(windows))]
    {
        let _ = (proc, eq_base);
        None
    }
    #[cfg(windows)]
    {
        use dmft_common::offsets::character_zone;
        let pc_ptr_addr = offsets::rebase(offsets::PINST_LOCAL_PC, eq_base)?;
        let pc_addr = proc.read_ptr(pc_ptr_addr).ok().filter(|&a| a != 0)?;
        let spell_slot = proc
            .read::<u8>(pc_addr + character_zone::SPELL_SLOT)
            .unwrap_or(0xFF);
        let spell_eta = proc
            .read::<u32>(pc_addr + character_zone::SPELL_ETA)
            .unwrap_or(0);
        let mut gem_etas = [0u32; 15];
        for (i, eta) in gem_etas.iter_mut().enumerate() {
            *eta = proc
                .read::<u32>(pc_addr + character_zone::SPELL_GEM_ETA + i * 4)
                .unwrap_or(0);
        }
        Some(CastState {
            spell_slot,
            spell_eta,
            gem_etas,
        })
    }
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
                tracing::warn!(addr = format!("{:#x}", current), error = %e, "Skipping spawn with unreadable critical fields");
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
    let leader_ptr = proc.read_ptr(group_ptr + group::GROUP_LEADER).unwrap_or(0);
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
        let name = read_cxstr(proc, member_ptr + group::MEMBER_NAME_CXSTR, 64).unwrap_or_default();
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

/// Read the current zone's long name from the zoneHeader struct in memory.
/// Returns the display name (e.g., "Queynos Hills") or an error if not zoned in.
pub fn read_zone_name(proc: &ProcessHandle, eq_base: u64) -> Result<String> {
    let zone_addr = offsets::rebase(zone_info::INST_EQ_ZONE_INFO, eq_base)
        .context("rebase underflow for instEQZoneInfo")?;

    let long_name = proc
        .read_string(zone_addr + zone_info::LONG_NAME, 128)
        .context("Failed to read zone long name")?;

    if long_name.is_empty() {
        anyhow::bail!("Zone long name is empty — not zoned in?");
    }

    Ok(long_name)
}

/// Read the current zone's short name from the zoneHeader struct in memory.
/// Returns the internal name (e.g., "qey2hh1") or an error if not zoned in.
pub fn read_zone_short_name(proc: &ProcessHandle, eq_base: u64) -> Result<String> {
    let zone_addr = offsets::rebase(zone_info::INST_EQ_ZONE_INFO, eq_base)
        .context("rebase underflow for instEQZoneInfo")?;

    let short_name = proc
        .read_string(zone_addr + zone_info::SHORT_NAME, 128)
        .context("Failed to read zone short name")?;

    if short_name.is_empty() {
        anyhow::bail!("Zone short name is empty — not zoned in?");
    }

    Ok(short_name)
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
