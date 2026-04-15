//! Buff enumeration and duration tracking.
//!
//! Reads the player's active buff array from EQ memory (via the profile
//! pointer chain) and provides query helpers used by combat strategies
//! and the rotation engine.

use textquest_common::combat::BuffInfo;

#[cfg(windows)]
use textquest_common::combat::BuffCategory;
#[cfg(windows)]
use textquest_common::offsets::{buff_slots, profile};

/// Read all active buffs from the local player's `EQ_Affect` array.
///
/// Follows the pointer chain: `PINST_LOCAL_PC` → `ProfileManager` →
/// current `PcProfile` → `BaseProfile::Buffs` (`SoeUtil::Array<EQ_Affect>`).
///
/// Returns an empty vec on non-Windows or if any pointer in the chain is null.
#[must_use]
pub fn read_active_buffs(eq_base: u64) -> Vec<BuffInfo> {
    #[cfg(not(windows))]
    {
        let _ = eq_base;
        Vec::new()
    }

    #[cfg(windows)]
    {
        read_active_buffs_windows(eq_base)
    }
}

/// Windows implementation: reads buffs from live EQ memory.
#[cfg(windows)]
fn read_active_buffs_windows(eq_base: u64) -> Vec<BuffInfo> {
    use textquest_common::offsets;

    let pc_pinst = match offsets::rebase(offsets::PINST_LOCAL_PC, eq_base) {
        Some(a) => a,
        None => return Vec::new(),
    };

    if !crate::hooks::game_loop::is_readable(pc_pinst, size_of::<usize>()) {
        return Vec::new();
    }
    let pc_ptr = unsafe { *(pc_pinst as *const usize) };
    if pc_ptr == 0 {
        return Vec::new();
    }

    // PcClient → ProfileManager → ProfileList* → PcProfile*
    let profile_mgr = pc_ptr + profile::PROFILE_MANAGER;
    let profile_list_ptr = read_ptr_safe(profile_mgr + profile::PROFILE_LIST_PTR);
    let profile_list_ptr = match profile_list_ptr {
        Some(p) if p != 0 => p,
        _ => return Vec::new(),
    };
    let profile_ptr = read_ptr_safe(profile_list_ptr + profile::PROFILE_FIRST);
    let profile_ptr = match profile_ptr {
        Some(p) if p != 0 => p,
        _ => return Vec::new(),
    };

    // BaseProfile → Buffs (SoeUtil::Array<EQ_Affect>)
    let buffs_array_base = profile_ptr + profile::BUFFS_ARRAY;
    let data_ptr = match read_ptr_safe(buffs_array_base + profile::ARRAY_DATA_PTR) {
        Some(p) if p != 0 => p,
        _ => return Vec::new(),
    };
    let count = read_i32_safe(buffs_array_base + profile::ARRAY_SIZE).unwrap_or(0);
    let count = normalize_buff_count(count);
    let span_len = match count.checked_mul(buff_slots::EQ_AFFECT_SIZE) {
        Some(len) => len,
        None => return Vec::new(),
    };
    if !crate::hooks::game_loop::is_readable(data_ptr, span_len) {
        return Vec::new();
    }

    let mut buffs = Vec::new();
    for i in 0..count {
        let entry = data_ptr + i * buff_slots::EQ_AFFECT_SIZE;

        let spell_id = read_i32_safe(entry + buff_slots::SPELL_ID).unwrap_or(-1);
        // Empty slot markers: 0, negative (garbage), or 0xFFFF (EQ sentinel)
        if spell_id <= 0 || spell_id == 0xFFFF {
            continue;
        }

        let duration = read_i32_safe(entry + buff_slots::DURATION).unwrap_or(0);
        let initial_duration = read_i32_safe(entry + buff_slots::INITIAL_DURATION).unwrap_or(0);
        let hit_count = read_i32_safe(entry + buff_slots::HIT_COUNT).unwrap_or(0);
        let caster_level = read_u8_safe(entry + buff_slots::CASTER_LEVEL).unwrap_or(0);

        let category = if i < buff_slots::NUM_LONG_BUFFS {
            BuffCategory::LongBuff
        } else {
            BuffCategory::ShortBuff
        };

        buffs.push(BuffInfo {
            spell_id,
            duration_ticks: duration,
            initial_duration,
            hit_count,
            category,
            caster_level,
            slot_index: i,
        });
    }

    buffs
}

#[cfg(windows)]
fn normalize_buff_count(count: i32) -> usize {
    usize::try_from(count)
        .ok()
        .map_or(0, |n| n.min(buff_slots::MAX_TOTAL_BUFFS))
}

#[cfg(windows)]
fn read_ptr_safe(addr: usize) -> Option<usize> {
    if !crate::hooks::game_loop::is_readable(addr, size_of::<usize>()) {
        return None;
    }
    Some(unsafe { *(addr as *const usize) })
}

#[cfg(windows)]
fn read_i32_safe(addr: usize) -> Option<i32> {
    if !crate::hooks::game_loop::is_readable(addr, size_of::<i32>()) {
        return None;
    }
    Some(unsafe { *(addr as *const i32) })
}

#[cfg(windows)]
fn read_u8_safe(addr: usize) -> Option<u8> {
    if !crate::hooks::game_loop::is_readable(addr, 1) {
        return None;
    }
    Some(unsafe { *(addr as *const u8) })
}

// ── Query helpers ──

/// Check if a specific spell ID is active in the buff list.
#[must_use]
pub fn has_buff(buffs: &[BuffInfo], spell_id: i32) -> bool {
    buffs.iter().any(|b| b.spell_id == spell_id)
}

/// Get the remaining duration in seconds for a specific spell.
/// Returns `None` if the spell is not active, or `Some(0.0)` for permanent
/// buffs.
#[must_use]
pub fn buff_remaining_seconds(buffs: &[BuffInfo], spell_id: i32) -> Option<f32> {
    buffs
        .iter()
        .find(|b| b.spell_id == spell_id)
        .map(|b| b.remaining_seconds())
}

/// Extract just the spell IDs from the buff list (for backward compat
/// with `CombatContext::active_buffs`).
#[must_use]
pub fn active_buff_ids(buffs: &[BuffInfo]) -> Vec<i32> {
    buffs.iter().map(|b| b.spell_id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::combat::BuffCategory;

    fn make_buff(spell_id: i32, duration_ticks: i32, initial_duration: i32) -> BuffInfo {
        BuffInfo {
            spell_id,
            duration_ticks,
            initial_duration,
            hit_count: 0,
            category: BuffCategory::LongBuff,
            caster_level: 65,
            slot_index: 0,
        }
    }

    #[test]
    fn has_buff_finds_active_spell() {
        let buffs = vec![make_buff(1234, 10, 100), make_buff(5678, 5, 50)];
        assert!(has_buff(&buffs, 1234));
        assert!(has_buff(&buffs, 5678));
        assert!(!has_buff(&buffs, 9999));
    }

    #[test]
    fn has_buff_empty_list() {
        assert!(!has_buff(&[], 1234));
    }

    #[test]
    fn buff_remaining_seconds_returns_duration() {
        let buffs = vec![make_buff(1234, 10, 100)];
        let secs = buff_remaining_seconds(&buffs, 1234).unwrap();
        assert!((secs - 60.0).abs() < f32::EPSILON);
    }

    #[test]
    fn buff_remaining_seconds_permanent_buff() {
        let buffs = vec![make_buff(1234, 0, 0)];
        let secs = buff_remaining_seconds(&buffs, 1234).unwrap();
        assert!((secs - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn buff_remaining_seconds_missing_spell() {
        let buffs = vec![make_buff(1234, 10, 100)];
        assert!(buff_remaining_seconds(&buffs, 9999).is_none());
    }

    #[test]
    fn active_buff_ids_extracts_spell_ids() {
        let buffs = vec![make_buff(100, 10, 100), make_buff(200, 5, 50)];
        let ids = active_buff_ids(&buffs);
        assert_eq!(ids, vec![100, 200]);
    }

    #[test]
    fn active_buff_ids_empty() {
        let ids = active_buff_ids(&[]);
        assert!(ids.is_empty());
    }

    #[test]
    fn buff_info_remaining_seconds() {
        let b = make_buff(1, 10, 20);
        assert!((b.remaining_seconds() - 60.0).abs() < f32::EPSILON);
        assert!((b.total_seconds() - 120.0).abs() < f32::EPSILON);
    }

    #[test]
    fn buff_info_expires_within() {
        let b = make_buff(1, 5, 100); // 30 seconds remaining
        assert!(b.expires_within(60.0));
        assert!(b.expires_within(30.0));
        assert!(!b.expires_within(10.0));
    }

    #[test]
    fn buff_info_permanent_never_expires() {
        let b = make_buff(1, 0, 0);
        assert!(!b.expires_within(9999.0));
    }

    #[test]
    fn buff_category_long_vs_short() {
        let long = BuffInfo {
            category: BuffCategory::LongBuff,
            ..make_buff(1, 10, 100)
        };
        let short = BuffInfo {
            category: BuffCategory::ShortBuff,
            ..make_buff(2, 5, 50)
        };
        assert_eq!(long.category, BuffCategory::LongBuff);
        assert_eq!(short.category, BuffCategory::ShortBuff);
    }

    #[test]
    fn read_active_buffs_stub_on_non_windows() {
        let buffs = read_active_buffs(0x140000000);
        assert!(buffs.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn normalize_buff_count_rejects_negative_values() {
        assert_eq!(normalize_buff_count(-1), 0);
    }

    #[cfg(windows)]
    #[test]
    fn normalize_buff_count_caps_to_max_slots() {
        assert_eq!(
            normalize_buff_count(i32::MAX),
            textquest_common::offsets::buff_slots::MAX_TOTAL_BUFFS
        );
    }
}
