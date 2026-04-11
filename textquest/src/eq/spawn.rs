use super::structs::{
    BuffSlot, CastDurationSource, CastState, EqClass, GroupInfo, SpawnInfo, SpawnType, SpellSlot,
    StandState,
};
use crate::process::memory::{ProcessHandle, is_probably_valid_process_ptr};
use anyhow::{Context, Result};
use std::{collections::HashSet, mem::size_of};
use textquest_common::offsets::{
    self, actor_client, character_zone, client_spell_manager, display, eq_spell, group,
    launch_spell_data, player_base, player_zone, spawn_manager, spell_hash_map, zone_info,
};

fn sanitize_terminal_text(input: String) -> String {
    input.chars().filter(|ch| !ch.is_control()).collect()
}

const EMPTY_PROCESS_PTR_SENTINEL: usize = 0xFFFF;
const SPAWN_LIST_HOP_MULTIPLIER: usize = 4;

fn valid_process_ptr(addr: usize) -> Option<usize> {
    (addr != EMPTY_PROCESS_PTR_SENTINEL && is_probably_valid_process_ptr(addr)).then_some(addr)
}

fn spawn_list_hop_limit(max_count: usize) -> usize {
    max_count.saturating_mul(SPAWN_LIST_HOP_MULTIPLIER).max(1)
}

fn checked_process_ptr_offset(base: usize, offset: usize) -> Option<usize> {
    base.checked_add(offset)
}

fn ensure_valid_process_ptr(addr: usize, label: &str) -> Result<usize> {
    if let Some(addr) = valid_process_ptr(addr) {
        Ok(addr)
    } else {
        anyhow::bail!("{label} is invalid ({addr:#x}) — client not ready?")
    }
}

fn read_local_player_addr(proc: &ProcessHandle, eq_base: u64) -> Result<usize> {
    let player_ptr_addr = offsets::rebase(offsets::PINST_LOCAL_PLAYER, eq_base)
        .context("rebase underflow for pinstLocalPlayer")?;
    let player_addr = proc
        .read_ptr(player_ptr_addr)
        .context("Failed to read pinstLocalPlayer pointer")?;

    if player_addr == 0 {
        anyhow::bail!("pinstLocalPlayer is null — not logged in?");
    }

    ensure_valid_process_ptr(player_addr, "pinstLocalPlayer")
}

fn first_spawn_from_local_player_links(proc: &ProcessHandle, eq_base: u64) -> Result<usize> {
    const MAX_BACKTRACK_STEPS: usize = 4096;

    let mut current = read_local_player_addr(proc, eq_base)?;
    let mut visited = HashSet::new();

    for _ in 0..MAX_BACKTRACK_STEPS {
        if !visited.insert(current) {
            break;
        }

        let prev = proc.read_ptr(current + player_base::PREV).unwrap_or(0);
        let Some(prev) = valid_process_ptr(prev) else {
            break;
        };
        current = prev;
    }

    Ok(current)
}

fn read_spawn_list_from_first(
    proc: &ProcessHandle,
    first_spawn: usize,
    eq_base: u64,
    display_timestamp: Option<u32>,
    max_count: usize,
) -> Result<Vec<SpawnInfo>> {
    let mut current = ensure_valid_process_ptr(first_spawn, "first spawn pointer")?;
    let mut spawns = Vec::new();
    let mut visited = HashSet::new();
    let mut hops = 0usize;
    let max_hops = spawn_list_hop_limit(max_count);

    while spawns.len() < max_count && visited.insert(current) && hops < max_hops {
        hops += 1;
        match read_spawn(proc, current, eq_base, display_timestamp) {
            Ok(spawn) => {
                tracing::trace!(addr = format!("{:#x}", current), name = %spawn.name, "Read spawn OK");
                spawns.push(spawn);
            }
            Err(e) => {
                tracing::warn!(addr = format!("{:#x}", current), error = %e, "Skipping spawn with unreadable critical fields");
            }
        }

        let Some(next_addr) = checked_process_ptr_offset(current, player_base::NEXT) else {
            tracing::warn!(
                current_addr = format!("{:#x}", current),
                next_offset = player_base::NEXT,
                spawn_count = spawns.len(),
                "Overflow calculating NEXT pointer address, stopping iteration"
            );
            break;
        };
        match proc.read_ptr(next_addr) {
            Ok(next) => match valid_process_ptr(next) {
                Some(next) => {
                    tracing::trace!(
                        current_addr = format!("{:#x}", current),
                        next_ptr_addr = format!("{:#x}", next_addr),
                        next_value = format!("{:#x}", next),
                        "NEXT pointer"
                    );
                    current = next;
                }
                None => break,
            },
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

    if hops >= max_hops {
        tracing::warn!(
            max_hops,
            max_count,
            readable_spawns = spawns.len(),
            "Stopping spawn traversal after reaching hop limit"
        );
    }

    Ok(spawns)
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
    ensure_valid_process_ptr(addr, "spawn pointer")?;

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
        spellbook: Vec::new(),
        memorized_spells: Vec::new(),
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
    let player_addr = read_local_player_addr(proc, eq_base)?;

    tracing::debug!(
        player_addr = format!("{:#x}", player_addr),
        eq_base = format!("{:#x}", eq_base),
        "read_local_player pointer chain"
    );

    let mut spawn = read_spawn(proc, player_addr, eq_base, display_timestamp)
        .context("Failed to read local player spawn data")?;
    spawn.buff_slots = read_buff_slots(proc, eq_base);
    spawn.spellbook = read_spellbook(proc, eq_base);
    spawn.memorized_spells = read_memorized_spells(proc, eq_base);
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
        use textquest_common::offsets::{buff_slots as bs, profile};
        let Some(profile_ptr) = read_local_profile_addr(proc, eq_base) else {
            return Vec::new();
        };

        // BaseProfile → Buffs (SoeUtil::Array<EQ_Affect>)
        let buffs_array_base = profile_ptr + profile::BUFFS_ARRAY;
        let data_ptr = match proc.read_ptr(buffs_array_base + profile::ARRAY_DATA_PTR) {
            Ok(p) if p != 0 => p,
            _ => return Vec::new(),
        };
        let count = proc
            .read::<i32>(buffs_array_base + profile::ARRAY_SIZE)
            .unwrap_or(0);
        let count = (count as usize).min(bs::MAX_TOTAL_BUFFS);

        let mut slots = Vec::new();
        for i in 0..count {
            let entry = data_ptr + i * bs::EQ_AFFECT_SIZE;
            let spell_id = proc.read::<u32>(entry + bs::SPELL_ID).unwrap_or(0xFFFF);
            if spell_id == 0 || spell_id == 0xFFFF {
                continue;
            }
            let duration_ticks = proc.read::<i32>(entry + bs::DURATION).unwrap_or(0);
            let caster_level = proc.read::<u8>(entry + bs::CASTER_LEVEL).unwrap_or(0);
            slots.push(BuffSlot {
                spell_id,
                duration_ticks,
                caster_level,
            });
        }
        slots
    }
}

/// Read spellbook entries for the local player.
/// On non-Windows builds returns an empty vec (stub).
#[must_use]
pub fn read_spellbook(proc: &ProcessHandle, eq_base: u64) -> Vec<SpellSlot> {
    #[cfg(not(windows))]
    {
        let _ = (proc, eq_base);
        Vec::new()
    }
    #[cfg(windows)]
    {
        use textquest_common::offsets::profile;
        let Some(profile_ptr) = read_local_profile_addr(proc, eq_base) else {
            return Vec::new();
        };

        build_spell_slots(
            read_spell_slot_ids(
                proc,
                profile_ptr + profile::SPELL_BOOK,
                profile::SPELL_BOOK_SLOT_COUNT,
            ),
            |_| None,
        )
    }
}

/// Read the current memorized spell gems for the local player.
/// On non-Windows builds returns an empty vec (stub).
#[must_use]
pub fn read_memorized_spells(proc: &ProcessHandle, eq_base: u64) -> Vec<SpellSlot> {
    #[cfg(not(windows))]
    {
        let _ = (proc, eq_base);
        Vec::new()
    }
    #[cfg(windows)]
    {
        use textquest_common::offsets::profile;
        let Some(profile_ptr) = read_local_profile_addr(proc, eq_base) else {
            return Vec::new();
        };

        build_spell_slots(
            read_spell_slot_ids(
                proc,
                profile_ptr + profile::MEMORIZED_SPELLS,
                profile::MEMORIZED_SPELL_GEM_COUNT,
            ),
            |spell_id| resolve_spell_name(proc, eq_base, spell_id),
        )
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

    let Some(target_addr) = valid_process_ptr(target_addr) else {
        tracing::debug!(
            target_addr = format!("{:#x}", target_addr),
            "Ignoring invalid pinstTarget pointer"
        );
        return Ok(None);
    };

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
    let raw_mgr_addr = proc
        .read_ptr(mgr_ptr_addr)
        .context("Failed to read pinstSpawnManager pointer")?;
    let first_spawn = 'get_first: {
        if let Some(mgr_addr) = valid_process_ptr(raw_mgr_addr) {
            let Some(list_addr) = checked_process_ptr_offset(mgr_addr, spawn_manager::PLAYER_LIST)
            else {
                tracing::debug!(
                    mgr_addr = format!("{:#x}", mgr_addr),
                    offset = format!("{:#x}", spawn_manager::PLAYER_LIST),
                    "SpawnManager player list address overflowed, falling back to local-player links"
                );
                break 'get_first first_spawn_from_local_player_links(proc, eq_base)?;
            };
            match proc.read_ptr(list_addr) {
                Ok(first) => {
                    if let Some(valid_first) = valid_process_ptr(first) {
                        break 'get_first valid_first;
                    }
                    tracing::debug!(
                        mgr_addr = format!("{:#x}", mgr_addr),
                        first = format!("{:#x}", first),
                        "SpawnManager list head unavailable, falling back to local-player links"
                    );
                }
                Err(e) => {
                    tracing::debug!(
                        mgr_addr = format!("{:#x}", mgr_addr),
                        error = %e,
                        "Failed to read SpawnManager list head, falling back to local-player links"
                    );
                }
            }
        } else {
            tracing::debug!(
                mgr_addr = format!("{:#x}", raw_mgr_addr),
                "SpawnManager pointer unavailable, falling back to local-player links"
            );
        }
        first_spawn_from_local_player_links(proc, eq_base)?
    };

    read_spawn_list_from_first(proc, first_spawn, eq_base, display_timestamp, max_count)
}

fn read_display_timestamp(proc: &ProcessHandle, eq_base: u64) -> Option<u32> {
    let display_ptr_addr = offsets::rebase(offsets::PINST_CDISPLAY, eq_base)?;
    let display_addr = proc.read_ptr(display_ptr_addr).ok().filter(|&a| a != 0)?;
    proc.read::<u32>(display_addr + display::TIME_STAMP).ok()
}

fn read_local_profile_addr(proc: &ProcessHandle, eq_base: u64) -> Option<usize> {
    let pc_ptr_addr = offsets::rebase(offsets::PINST_LOCAL_PC, eq_base)?;
    let pc_addr = proc
        .read_ptr(pc_ptr_addr)
        .ok()
        .and_then(valid_process_ptr)?;

    use textquest_common::offsets::profile;
    let profile_mgr = pc_addr + profile::PROFILE_MANAGER;
    let profile_list_ptr = proc
        .read_ptr(profile_mgr + profile::PROFILE_LIST_PTR)
        .ok()
        .and_then(valid_process_ptr)?;
    proc.read_ptr(profile_list_ptr + profile::PROFILE_FIRST)
        .ok()
        .and_then(valid_process_ptr)
}

fn read_local_player_addr_from_pc(proc: &ProcessHandle, eq_base: u64) -> Option<usize> {
    let pc_ptr_addr = offsets::rebase(offsets::PINST_LOCAL_PC, eq_base)?;
    let pc_addr = proc
        .read_ptr(pc_ptr_addr)
        .ok()
        .and_then(valid_process_ptr)?;
    proc.read_ptr(pc_addr + character_zone::ME)
        .ok()
        .and_then(valid_process_ptr)
        .or_else(|| read_local_player_addr(proc, eq_base).ok())
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

fn build_spell_slots<I, F>(slots: I, mut resolve_name: F) -> Vec<SpellSlot>
where
    I: IntoIterator<Item = (usize, i32)>,
    F: FnMut(u32) -> Option<String>,
{
    slots
        .into_iter()
        .filter_map(|(slot, spell_id)| {
            let spell_id = u32::try_from(spell_id).ok().filter(|&id| id > 0)?;
            Some(SpellSlot {
                slot,
                spell_id,
                spell_name: resolve_name(spell_id),
            })
        })
        .collect()
}

fn read_spell_slot_ids(
    proc: &ProcessHandle,
    base_addr: usize,
    slot_count: usize,
) -> Vec<(usize, i32)> {
    let Ok(bytes) = proc.read_bytes(base_addr, slot_count * size_of::<i32>()) else {
        return Vec::new();
    };

    bytes
        .chunks_exact(size_of::<i32>())
        .enumerate()
        .map(|(slot, chunk)| {
            (
                slot,
                i32::from_le_bytes(chunk.try_into().expect("4-byte chunk from chunks_exact")),
            )
        })
        .collect()
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

fn resolve_spell_name(proc: &ProcessHandle, eq_base: u64, spell_id: u32) -> Option<String> {
    read_spell_cast_metadata(proc, eq_base, i32::try_from(spell_id).ok()?)
        .and_then(|entry| entry.spell_name)
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
    let rep_ptr = ensure_valid_process_ptr(rep_ptr, "CXStr.m_data")?;
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
    let Some(pc_addr) = valid_process_ptr(pc_addr) else {
        return Ok(None);
    };

    // Read CGroup* from PcClient
    let group_ptr = proc
        .read_ptr(pc_addr + group::PC_CLIENT_GROUP_PTR)
        .context("Failed to read PcClient.Group pointer")?;
    if group_ptr == 0 {
        return Ok(None);
    }
    let Some(group_ptr) = valid_process_ptr(group_ptr) else {
        return Ok(None);
    };

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
    ensure_valid_process_ptr(spawn_addr, "spawn pointer")?;
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
    fn valid_process_ptr_rejects_common_sentinel_values() {
        assert_eq!(valid_process_ptr(0), None);
        assert_eq!(valid_process_ptr(0xFFFF), None);
        assert_eq!(valid_process_ptr(usize::MAX), None);
        assert!(valid_process_ptr(0x0000_1234_5678).is_some());
    }

    #[test]
    fn spawn_list_hop_limit_scales_with_requested_count() {
        assert_eq!(spawn_list_hop_limit(0), 1);
        assert_eq!(spawn_list_hop_limit(1), 4);
        assert_eq!(spawn_list_hop_limit(25), 100);
    }

    #[test]
    fn checked_process_ptr_offset_returns_none_on_overflow() {
        assert_eq!(checked_process_ptr_offset(usize::MAX, 1), None);
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

    #[test]
    fn build_spell_slots_skips_empty_and_invalid_entries() {
        let slots = build_spell_slots([(0, 123), (1, 0), (2, -1), (3, 456)], |_| None);

        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].slot, 0);
        assert_eq!(slots[0].spell_id, 123);
        assert_eq!(slots[1].slot, 3);
        assert_eq!(slots[1].spell_id, 456);
    }

    #[test]
    fn build_spell_slots_preserves_slot_numbers_and_resolved_names() {
        let slots = build_spell_slots([(4, 123), (7, 456)], |spell_id| match spell_id {
            123 => Some("Complete Heal".to_string()),
            456 => Some("Celestial Remedy".to_string()),
            _ => None,
        });

        assert_eq!(slots[0].slot, 4);
        assert_eq!(slots[0].spell_name.as_deref(), Some("Complete Heal"));
        assert_eq!(slots[1].slot, 7);
        assert_eq!(slots[1].spell_name.as_deref(), Some("Celestial Remedy"));
    }

    #[test]
    fn read_spell_slot_ids_decodes_little_endian_i32_values() {
        fn parse(bytes: &[u8]) -> Vec<(usize, i32)> {
            bytes
                .chunks_exact(size_of::<i32>())
                .enumerate()
                .map(|(slot, chunk)| {
                    (
                        slot,
                        i32::from_le_bytes(
                            chunk.try_into().expect("4-byte chunk from chunks_exact"),
                        ),
                    )
                })
                .collect()
        }

        let bytes = [
            123_i32.to_le_bytes(),
            0_i32.to_le_bytes(),
            (-1_i32).to_le_bytes(),
            456_i32.to_le_bytes(),
        ]
        .concat();

        assert_eq!(parse(&bytes), vec![(0, 123), (1, 0), (2, -1), (3, 456)]);
    }
}
