use super::structs::{
    BuffSlot, CastDurationSource, CastState, EqClass, GroupInfo, SpawnInfo, SpawnType, StandState,
};
use crate::process::memory::ProcessHandle;
use anyhow::{Context, Result};
use dmft_common::offsets::{
    self, actor_client, character_zone, client_spell_manager, display, eq_spell, group,
    launch_spell_data, player_base, player_zone, spawn_manager, spell_hash_map, zone_info,
};
use std::mem::size_of;

fn sanitize_terminal_text(input: String) -> String {
    input.chars().filter(|ch| !ch.is_control()).collect()
}

/// Read a single spawn's data from the process at the given `PlayerClient` address.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn read_spawn(
    proc: &ProcessHandle,
    addr: usize,
    eq_base: u64,
    display_timestamp: Option<u32>,
) -> Result<SpawnInfo> {
    // Critical fields — hard fail if any are unreadable (corrupt memory → skip spawn)
    let name = proc
        .read_string(addr + player_base::NAME, 64)
        .context("critical field: name")?;
    let name = sanitize_terminal_text(name);
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
    let displayed_name = sanitize_terminal_text(displayed_name);
    let lastname = proc
        .read_string(addr + player_base::LASTNAME, 32)
        .unwrap_or_default();
    let lastname = sanitize_terminal_text(lastname);

    let spawn_id = proc.read::<u32>(addr + player_base::SPAWN_ID).unwrap_or(0);
    let heading = proc.read::<f32>(addr + player_base::HEADING).unwrap_or(0.0);

    // Diagnostic: if position looks suspicious (all near-zero) but name is valid,
    // log metadata only (never raw process memory or addresses).
    if x.abs() < 1.0 && y.abs() < 1.0 && !name.is_empty() && name != "<unreadable>" {
        tracing::warn!(
            name = %name,
            spawn_id,
            x, y, z,
            "Position near zero — possible offset mismatch"
        );
    }

    let level = proc
        .read::<u8>(addr + player_zone::LEVEL)
        .context("critical field: level")?;
    // Use ActorClient::Class (int32_t at 0x0FDC) — the reliable field for all spawn types.
    // PlayerZoneClient::CharClass (uint8_t at 0x0420) is often zero for NPCs/mercs/pets.
    let class_id = proc
        .read::<i32>(addr + actor_client::CHAR_CLASS)
        .unwrap_or(0) as u8;
    let stand_state_id = proc.read::<u8>(addr + player_zone::STANDSTATE).unwrap_or(0);
    let hp_current = proc
        .read::<i64>(addr + player_zone::HP_CURRENT)
        .unwrap_or(0);
    let hp_max = proc.read::<i64>(addr + player_zone::HP_MAX).unwrap_or(0);
    // Mana fields are only valid for the local player — other spawns have garbage
    // at these offsets. Clamp obviously invalid values to zero.
    let raw_mana_current = proc
        .read::<i32>(addr + player_zone::MANA_CURRENT)
        .unwrap_or(0);
    let raw_mana_max = proc.read::<i32>(addr + player_zone::MANA_MAX).unwrap_or(0);
    let (mana_current, mana_max) = if raw_mana_max > 0 && raw_mana_max < 1_000_000 {
        (raw_mana_current.max(0), raw_mana_max)
    } else {
        (0, 0)
    };
    let endurance_current = proc
        .read::<i32>(addr + player_zone::ENDURANCE_CURRENT)
        .unwrap_or(0);
    let endurance_max = proc
        .read::<u32>(addr + player_zone::ENDURANCE_MAX)
        .unwrap_or(0);

    let gm_flag = proc.read::<u8>(addr + player_zone::GM).unwrap_or(0);
    let race_id = proc.read::<i32>(addr + actor_client::RACE).unwrap_or(0) as u32;
    let cast_state = read_spawn_cast_state(proc, addr, eq_base, display_timestamp, None);

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
        cast_state,
    })
}

/// Read the local player's spawn info, including buff slots and cast state.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn read_local_player(proc: &ProcessHandle, eq_base: u64) -> Result<SpawnInfo> {
    let display_timestamp = read_display_timestamp(proc, eq_base);
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

    let mut spawn = read_spawn(proc, player_addr, eq_base, display_timestamp)
        .context("Failed to read local player spawn data")?;
    spawn.buff_slots = read_buff_slots(proc, eq_base);
    if let Some(local_cast_state) = read_cast_state(proc, eq_base) {
        spawn.cast_state = Some(local_cast_state);
    }
    Ok(spawn)
}

/// Read buff slots for the local player via `PINST_LOCAL_PC`.
/// On non-Windows builds returns an empty vec (stub).
#[must_use]
pub fn read_buff_slots(proc: &ProcessHandle, eq_base: u64) -> Vec<BuffSlot> {
    #[cfg(not(windows))]
    {
        let _ = (proc, eq_base);
        Vec::new()
    }
    #[cfg(windows)]
    {
        use dmft_common::offsets::buff_slots as bs;
        let Some(pc_ptr_addr) = offsets::rebase(offsets::PINST_LOCAL_PC, eq_base) else {
            return Vec::new();
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

/// Read cast state for the local player via `PINST_LOCAL_PC -> CharacterZoneClient::me`.
/// On non-Windows builds returns None (stub).
#[must_use]
pub fn read_cast_state(proc: &ProcessHandle, eq_base: u64) -> Option<CastState> {
    #[cfg(not(windows))]
    {
        let _ = (proc, eq_base);
        None
    }
    #[cfg(windows)]
    {
        let player_addr = read_local_player_addr_from_pc(proc, eq_base)?;
        let display_timestamp = read_display_timestamp(proc, eq_base);
        let gem_etas = read_spell_gem_etas(proc, player_addr);
        read_spawn_cast_state(
            proc,
            player_addr,
            eq_base,
            display_timestamp,
            Some(gem_etas),
        )
    }
}

/// Read the current target's spawn info, if any.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn read_target(proc: &ProcessHandle, eq_base: u64) -> Result<Option<SpawnInfo>> {
    let display_timestamp = read_display_timestamp(proc, eq_base);
    let target_ptr_addr = offsets::rebase(offsets::PINST_TARGET, eq_base)
        .context("rebase underflow for pinstTarget")?;
    let target_addr = proc
        .read_ptr(target_ptr_addr)
        .context("Failed to read pinstTarget pointer")?;

    if target_addr == 0 {
        return Ok(None);
    }

    let spawn = read_spawn(proc, target_addr, eq_base, display_timestamp)
        .context("Failed to read target spawn data")?;
    Ok(Some(spawn))
}

/// Iterate all spawns in the spawn manager's linked list.
/// Returns up to `max_count` spawns to prevent infinite loops on corrupt data.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn read_all_spawns(
    proc: &ProcessHandle,
    eq_base: u64,
    max_count: usize,
) -> Result<Vec<SpawnInfo>> {
    let display_timestamp = read_display_timestamp(proc, eq_base);
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
        match read_spawn(proc, current, eq_base, display_timestamp) {
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

fn read_display_timestamp(proc: &ProcessHandle, eq_base: u64) -> Option<u32> {
    let display_ptr_addr = offsets::rebase(offsets::PINST_CDISPLAY, eq_base)?;
    let display_addr = proc.read_ptr(display_ptr_addr).ok().filter(|&a| a != 0)?;
    proc.read::<u32>(display_addr + display::TIME_STAMP).ok()
}

fn read_local_player_addr_from_pc(proc: &ProcessHandle, eq_base: u64) -> Option<usize> {
    let pc_ptr_addr = offsets::rebase(offsets::PINST_LOCAL_PC, eq_base)?;
    let pc_addr = proc.read_ptr(pc_ptr_addr).ok().filter(|&a| a != 0)?;
    proc.read_ptr(pc_addr + character_zone::ME)
        .ok()
        .filter(|&a| a != 0)
        .or_else(|| {
            let player_ptr_addr = offsets::rebase(offsets::PINST_LOCAL_PLAYER, eq_base)?;
            proc.read_ptr(player_ptr_addr).ok().filter(|&a| a != 0)
        })
}

fn read_spell_gem_etas(proc: &ProcessHandle, player_addr: usize) -> [u32; 15] {
    let mut gem_etas = [0u32; 15];
    for (i, eta) in gem_etas.iter_mut().enumerate() {
        *eta = proc
            .read::<u32>(player_addr + player_zone::SPELL_GEM_ETA + i * 4)
            .unwrap_or(0);
    }
    gem_etas
}

#[derive(Debug, Clone, Copy)]
struct LaunchSpellSnapshot {
    spell_id: i32,
    target_id: u32,
    spell_eta: u32,
    item_id: i32,
    spell_slot: u8,
}

#[derive(Debug, Clone)]
struct SpellCastMetadata {
    spell_name: Option<String>,
    base_cast_ms: Option<u32>,
}

fn read_launch_spell_snapshot(
    proc: &ProcessHandle,
    cast_addr: usize,
) -> Option<LaunchSpellSnapshot> {
    Some(LaunchSpellSnapshot {
        spell_id: proc
            .read::<i32>(cast_addr + launch_spell_data::SPELL_ID)
            .ok()?,
        target_id: proc
            .read::<u32>(cast_addr + launch_spell_data::TARGET_ID)
            .unwrap_or(0),
        spell_eta: proc
            .read::<u32>(cast_addr + launch_spell_data::SPELL_ETA)
            .unwrap_or(0),
        item_id: proc
            .read::<i32>(cast_addr + launch_spell_data::ITEM_ID)
            .unwrap_or(0),
        spell_slot: proc
            .read::<u8>(cast_addr + launch_spell_data::SPELL_SLOT)
            .unwrap_or(launch_spell_data::NOT_CASTING_SPELL_SLOT),
    })
}

fn remaining_cast_ms(snapshot: LaunchSpellSnapshot, display_timestamp: Option<u32>) -> Option<u32> {
    if snapshot.spell_id == launch_spell_data::NOT_CASTING_SPELL_ID {
        return None;
    }

    display_timestamp.map(|timestamp| snapshot.spell_eta.saturating_sub(timestamp))
}

fn build_cast_state(
    snapshot: LaunchSpellSnapshot,
    display_timestamp: Option<u32>,
    gem_etas: Option<[u32; 15]>,
    metadata: Option<SpellCastMetadata>,
) -> Option<CastState> {
    let is_casting = snapshot.spell_id != launch_spell_data::NOT_CASTING_SPELL_ID;
    let remaining_ms = remaining_cast_ms(snapshot, display_timestamp);

    let spell_name = metadata.as_ref().and_then(|entry| entry.spell_name.clone());
    let (total_cast_ms, duration_source) = if !is_casting {
        (None, CastDurationSource::Unknown)
    } else if snapshot.item_id > 0 {
        // Item clicks can override the spell's base cast time, so do not claim a
        // duration unless we have item-definition data for the specific click.
        (None, CastDurationSource::Unknown)
    } else if let Some(base_cast_ms) = metadata.and_then(|entry| entry.base_cast_ms) {
        (Some(base_cast_ms), CastDurationSource::SpellDataBase)
    } else {
        (None, CastDurationSource::Unknown)
    };

    let cast_state = CastState {
        spell_id: snapshot.spell_id,
        spell_name,
        target_id: snapshot.target_id,
        spell_eta: snapshot.spell_eta,
        item_id: snapshot.item_id,
        spell_slot: snapshot.spell_slot,
        remaining_ms,
        total_cast_ms,
        duration_source,
        gem_etas,
    };

    if cast_state.gem_etas.is_some() || cast_state.is_casting() {
        Some(cast_state)
    } else {
        None
    }
}

fn read_spell_cast_metadata(
    proc: &ProcessHandle,
    eq_base: u64,
    spell_id: i32,
) -> Option<SpellCastMetadata> {
    let spell_id = u32::try_from(spell_id).ok().filter(|&id| id > 0)?;
    let spell_mgr_ptr_addr = offsets::rebase(offsets::PINST_SPELL_MANAGER, eq_base)?;
    let spell_mgr_addr = proc.read_ptr(spell_mgr_ptr_addr).ok().filter(|&a| a != 0)?;
    let max_spell_id = proc
        .read::<i32>(spell_mgr_addr + client_spell_manager::MAX_SPELL_ID)
        .ok()
        .filter(|&max_id| max_id > 0)?;
    if spell_id >= max_spell_id as u32 {
        return None;
    }

    let spells_map_addr = spell_mgr_addr + client_spell_manager::SPELLS;
    let buckets_addr = proc
        .read_ptr(spells_map_addr + spell_hash_map::BUCKETS)
        .ok()
        .filter(|&a| a != 0)?;
    let dynamic_size = proc
        .read::<u64>(spells_map_addr + spell_hash_map::DYNAMIC_SIZE)
        .ok()? as usize;
    if dynamic_size == 0 || !dynamic_size.is_power_of_two() {
        return None;
    }

    let bucket_index = spell_id as usize & (dynamic_size - 1);
    let bucket_ptr_addr = buckets_addr + bucket_index * size_of::<usize>();
    let mut node_addr = proc.read_ptr(bucket_ptr_addr).ok().unwrap_or(0);
    const MAX_HASH_MAP_HOPS: usize = 128;
    let mut hops = 0usize;

    while node_addr != 0 && hops < MAX_HASH_MAP_HOPS {
        let node_key = proc.read::<i32>(node_addr + spell_hash_map::KEY).ok()?;
        if node_key == spell_id as i32 {
            let spell_addr = node_addr + spell_hash_map::VALUE;
            let stored_id = proc
                .read::<i32>(spell_addr + eq_spell::ID)
                .unwrap_or(node_key);
            if stored_id != node_key {
                return None;
            }

            let spell_name = proc
                .read_string(spell_addr + eq_spell::NAME, 64)
                .ok()
                .filter(|name| !name.is_empty());
            let base_cast_ms = proc
                .read::<u32>(spell_addr + eq_spell::CAST_TIME)
                .ok()
                .filter(|&ms| ms > 0);

            return Some(SpellCastMetadata {
                spell_name,
                base_cast_ms,
            });
        }

        node_addr = proc
            .read_ptr(node_addr + spell_hash_map::HASH_NEXT)
            .ok()
            .unwrap_or(0);
        hops += 1;
    }

    None
}

fn read_spawn_cast_state(
    proc: &ProcessHandle,
    player_addr: usize,
    eq_base: u64,
    display_timestamp: Option<u32>,
    gem_etas: Option<[u32; 15]>,
) -> Option<CastState> {
    let cast_addr = player_addr + player_zone::CASTING_DATA;
    let snapshot = read_launch_spell_snapshot(proc, cast_addr)?;
    let metadata = read_spell_cast_metadata(proc, eq_base, snapshot.spell_id);
    build_cast_state(snapshot, display_timestamp, gem_etas, metadata)
}

/// Read a `CXStr` (EQ's string type) from memory.
/// `CXStr` is a pointer to `CStrRep`; the UTF-8 data lives at `CStrRep`+0x18.
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

/// Read group membership info from the local PC's `CGroup` pointer.
///
/// Path: pLocalPC -> +0x2EB0 (`CGroup`*) -> `CGroupBase` members array.
/// Each `CGroupMember` has a `CXStr` Name at offset 0x08.
///
/// # Errors
///
/// Returns an error if the operation fails.
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
///
/// # Errors
///
/// Returns an error if the operation fails.
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
///
/// # Errors
///
/// Returns an error if the operation fails.
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
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn read_spawn_bytes(
    proc: &ProcessHandle,
    spawn_addr: usize,
    start_offset: usize,
    len: usize,
) -> Result<Vec<u8>> {
    let addr = spawn_addr + start_offset;
    proc.read_bytes(addr, len)
        .with_context(|| format!("Failed to read {len} bytes at spawn+{start_offset:#x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(
        spell_id: i32,
        spell_eta: u32,
        item_id: i32,
        spell_slot: u8,
    ) -> LaunchSpellSnapshot {
        LaunchSpellSnapshot {
            spell_id,
            target_id: 42,
            spell_eta,
            item_id,
            spell_slot,
        }
    }

    fn metadata(name: &str, base_cast_ms: u32) -> SpellCastMetadata {
        SpellCastMetadata {
            spell_name: Some(name.to_string()),
            base_cast_ms: Some(base_cast_ms),
        }
    }

    #[test]
    fn sanitize_terminal_text_removes_ansi_control_bytes() {
        let raw = "Guard\u{1b}[31mHACK\u{1b}[0m\u{7}".to_string();
        assert_eq!(sanitize_terminal_text(raw), "Guard[31mHACK[0m");
    }

    #[test]
    fn remaining_cast_ms_uses_display_timestamp_for_active_casts() {
        let snapshot = snapshot(123, 1_500, 0, 2);
        assert_eq!(remaining_cast_ms(snapshot, Some(1_000)), Some(500));
    }

    #[test]
    fn remaining_cast_ms_is_none_when_not_casting() {
        let snapshot = snapshot(launch_spell_data::NOT_CASTING_SPELL_ID, 1_500, 0, 2);
        assert_eq!(remaining_cast_ms(snapshot, Some(1_000)), None);
    }

    #[test]
    fn build_cast_state_uses_spell_data_base_duration_for_memmed_spells() {
        let cast_state = build_cast_state(
            snapshot(123, 2_500, 0, 4),
            Some(1_000),
            None,
            Some(metadata("Complete Heal", 10_000)),
        )
        .expect("cast state");

        assert_eq!(cast_state.spell_name.as_deref(), Some("Complete Heal"));
        assert_eq!(cast_state.remaining_ms, Some(1_500));
        assert_eq!(cast_state.total_cast_ms, Some(10_000));
        assert_eq!(
            cast_state.duration_source,
            CastDurationSource::SpellDataBase
        );
    }

    #[test]
    fn build_cast_state_keeps_item_cast_duration_unknown_without_item_definition() {
        let cast_state = build_cast_state(
            snapshot(321, 2_000, 9_999, launch_spell_data::NOT_CASTING_SPELL_SLOT),
            Some(1_000),
            None,
            Some(metadata("Clicky Gate", 8_000)),
        )
        .expect("cast state");

        assert_eq!(cast_state.spell_name.as_deref(), Some("Clicky Gate"));
        assert_eq!(cast_state.remaining_ms, Some(1_000));
        assert_eq!(cast_state.total_cast_ms, None);
        assert_eq!(cast_state.duration_source, CastDurationSource::Unknown);
    }

    #[test]
    fn build_cast_state_preserves_gem_timers_when_idle() {
        let cast_state = build_cast_state(
            snapshot(
                launch_spell_data::NOT_CASTING_SPELL_ID,
                0,
                0,
                launch_spell_data::NOT_CASTING_SPELL_SLOT,
            ),
            Some(1_000),
            Some([7; 15]),
            None,
        )
        .expect("idle state with gem timers");

        assert!(!cast_state.is_casting());
        assert_eq!(cast_state.gem_etas, Some([7; 15]));
    }

    #[test]
    fn build_cast_state_clamps_remaining_ms_when_eta_has_elapsed() {
        let cast_state = build_cast_state(
            snapshot(123, 800, 0, 1),
            Some(1_000),
            None,
            Some(metadata("Late Cast", 2_000)),
        )
        .expect("cast state");

        assert_eq!(cast_state.remaining_ms, Some(0));
        assert_eq!(cast_state.cast_progress(), Some(1.0));
    }
}
