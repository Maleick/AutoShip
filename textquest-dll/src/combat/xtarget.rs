//! Extended Target list reader — reads EQ's XTarget window from memory.
//!
//! The extended target list is accessed via PcClient->pExtendedTargetList,
//! which contains an ArrayClass<ExtendedTargetSlot> with up to ~23 slots.
//! Each slot has a type (AutoHater, GroupTank, etc.), status, spawn ID, and
//! name.

use textquest_common::combat::{
    ExtendedTargetList, ExtendedTargetSlot, XTargetSlotStatus, XTargetType,
};
#[cfg(windows)]
use textquest_common::offsets;

/// Maximum number of extended target slots we'll read (safety bound).
const MAX_XTARGET_SLOTS: i32 = 32;

/// Maximum name length in an ExtendedTargetSlot (EQ_MAX_NAME = 64).
const EQ_MAX_NAME: usize = 64;

/// Read the extended target list from EQ memory.
///
/// # Safety
/// This reads raw process memory via pointer dereference. Must only be called
/// from the game loop thread where EQ pointers are valid.
#[cfg(windows)]
pub unsafe fn read_extended_targets(eq_base: u64) -> Option<ExtendedTargetList> {
    if eq_base == 0 {
        return None;
    }

    let pc_pinst = offsets::rebase(offsets::PINST_LOCAL_PC, eq_base)?;
    let pc_ptr = unsafe { read_ptr(pc_pinst)? };
    if pc_ptr == 0 {
        return None;
    }

    let xtarget_list_ptr_addr = pc_ptr + offsets::PCCLIENT_EXTENDED_TARGET_LIST as usize;
    let xtarget_list_ptr = unsafe { read_ptr(xtarget_list_ptr_addr)? };
    if xtarget_list_ptr == 0 {
        return None;
    }

    let array_base = xtarget_list_ptr + offsets::XTARGET_LIST_SLOTS_OFFSET as usize;
    let slot_count = unsafe { read_i32(array_base + offsets::ARRAY_CLASS_LENGTH as usize)? };
    if !(0..=MAX_XTARGET_SLOTS).contains(&slot_count) {
        tracing::trace!(slot_count, "XTarget slot count out of range");
        return Some(ExtendedTargetList {
            slots: Vec::new(),
            auto_add_haters: false,
        });
    }

    let array_ptr = unsafe { read_ptr(array_base + offsets::ARRAY_CLASS_ARRAY_PTR as usize)? };
    if array_ptr == 0 {
        return None;
    }

    let auto_add_haters_raw =
        unsafe { read_u8(xtarget_list_ptr + offsets::XTARGET_LIST_AUTO_ADD_HATERS as usize)? };
    let auto_add_haters = auto_add_haters_raw != 0;

    let mut slots = Vec::with_capacity(slot_count as usize);
    for i in 0..slot_count {
        let slot_base = array_ptr + (i as u64 * offsets::XTARGET_SLOT_SIZE) as usize;

        let raw_type = unsafe { read_u32(slot_base + offsets::XTARGET_SLOT_TYPE as usize)? };
        let raw_status = unsafe { read_u32(slot_base + offsets::XTARGET_SLOT_STATUS as usize)? };
        let spawn_id = unsafe { read_u32(slot_base + offsets::XTARGET_SLOT_SPAWN_ID as usize)? };

        let name_ptr = (slot_base + offsets::XTARGET_SLOT_NAME as usize) as *const u8;
        let name = unsafe { read_c_string(name_ptr, EQ_MAX_NAME) };

        let aggro_raw =
            unsafe { read_i32(slot_base + offsets::XTARGET_SLOT_AGGRO_PCT as usize)? };
        let aggro_pct = aggro_raw.clamp(0, 100) as u8;

        slots.push(ExtendedTargetSlot {
            slot_type: XTargetType::from_raw(raw_type).unwrap_or(XTargetType::Empty),
            status: XTargetSlotStatus::from_raw(raw_status),
            spawn_id,
            name,
            aggro_pct,
        });
    }

    Some(ExtendedTargetList {
        slots,
        auto_add_haters,
    })
}

#[cfg(windows)]
unsafe fn read_ptr(addr: usize) -> Option<usize> {
    if !crate::hooks::game_loop::is_readable(addr, size_of::<usize>()) {
        return None;
    }
    Some(unsafe { std::ptr::read(addr as *const usize) })
}

#[cfg(windows)]
unsafe fn read_i32(addr: usize) -> Option<i32> {
    if !crate::hooks::game_loop::is_readable(addr, size_of::<i32>()) {
        return None;
    }
    Some(unsafe { std::ptr::read(addr as *const i32) })
}

#[cfg(windows)]
unsafe fn read_u32(addr: usize) -> Option<u32> {
    if !crate::hooks::game_loop::is_readable(addr, size_of::<u32>()) {
        return None;
    }
    Some(unsafe { std::ptr::read(addr as *const u32) })
}

#[cfg(windows)]
unsafe fn read_u8(addr: usize) -> Option<u8> {
    if !crate::hooks::game_loop::is_readable(addr, size_of::<u8>()) {
        return None;
    }
    Some(unsafe { std::ptr::read(addr as *const u8) })
}

#[cfg(windows)]
unsafe fn read_c_string(ptr: *const u8, max_len: usize) -> String {
    let mut bytes = Vec::with_capacity(max_len);
    for i in 0..max_len {
        let addr = ptr.wrapping_add(i) as usize;
        let Some(b) = (unsafe { read_u8(addr) }) else {
            break;
        };
        if b == 0 {
            break;
        }
        bytes.push(b);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Stub for non-Windows platforms (macOS dev builds).
#[cfg(not(windows))]
pub unsafe fn read_extended_targets(_eq_base: u64) -> Option<ExtendedTargetList> {
    Some(ExtendedTargetList {
        slots: vec![
            ExtendedTargetSlot {
                slot_type: XTargetType::AutoHater,
                status: XTargetSlotStatus::CurrentZone,
                spawn_id: 1001,
                name: "a_fire_beetle".into(),
                aggro_pct: 75,
            },
            ExtendedTargetSlot {
                slot_type: XTargetType::AutoHater,
                status: XTargetSlotStatus::CurrentZone,
                spawn_id: 1002,
                name: "a_fire_beetle".into(),
                aggro_pct: 25,
            },
            ExtendedTargetSlot {
                slot_type: XTargetType::GroupAssistTarget,
                status: XTargetSlotStatus::CurrentZone,
                spawn_id: 1001,
                name: "a_fire_beetle".into(),
                aggro_pct: 0,
            },
        ],
        auto_add_haters: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn stub_returns_demo_data() {
        let result = unsafe { read_extended_targets(0) };
        assert!(result.is_some());
        let list = result.unwrap();
        assert!(!list.slots.is_empty());
        assert!(list.auto_add_haters);
    }

    #[cfg(windows)]
    #[test]
    fn read_extended_targets_null_eq_base_returns_none() {
        let result = unsafe { read_extended_targets(0) };
        assert!(result.is_none());
    }

    #[test]
    fn hater_spawn_ids_filters_correctly() {
        let list = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 100,
                    name: "mob_a".into(),
                    aggro_pct: 0,
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::GroupTank,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 200,
                    name: "tank".into(),
                    aggro_pct: 0,
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::DifferentZone,
                    spawn_id: 300,
                    name: "mob_b".into(),
                    aggro_pct: 0,
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 400,
                    name: "mob_c".into(),
                    aggro_pct: 0,
                },
            ],
            auto_add_haters: true,
        };
        let haters = list.hater_spawn_ids();
        assert_eq!(haters, vec![100, 400]);
        assert!(list.is_hater(100));
        assert!(!list.is_hater(200));
        assert!(!list.is_hater(300));
        assert_eq!(list.hater_count(), 2);
    }

    #[test]
    fn get_by_type_finds_active_slot() {
        let list = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::GroupAssist,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 500,
                    name: "assist_target".into(),
                    aggro_pct: 0,
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::GroupTank,
                    status: XTargetSlotStatus::Empty,
                    spawn_id: 0,
                    name: String::new(),
                    aggro_pct: 0,
                },
            ],
            auto_add_haters: false,
        };
        assert_eq!(
            list.get_by_type(XTargetType::GroupAssist).unwrap().spawn_id,
            500
        );
        assert!(list.get_by_type(XTargetType::GroupTank).is_none());
    }

    #[test]
    fn active_slots_excludes_empty() {
        let list = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 10,
                    name: "mob".into(),
                    aggro_pct: 0,
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::Empty,
                    status: XTargetSlotStatus::Empty,
                    spawn_id: 0,
                    name: String::new(),
                    aggro_pct: 0,
                },
            ],
            auto_add_haters: true,
        };
        assert_eq!(list.active_slots().len(), 1);
    }

    #[test]
    fn empty_list_has_no_haters() {
        let list = ExtendedTargetList::default();
        assert_eq!(list.hater_count(), 0);
        assert!(list.hater_spawn_ids().is_empty());
        assert!(!list.is_hater(1));
    }

    /// Verify aggro_pct is stored and readable per slot.
    ///
    /// This tests the fixture-level representation of `ExtendedTargetSlot.aggro_pct`.
    /// The field is populated in the Windows reader from `XTARGET_SLOT_AGGRO_PCT`
    /// (offset 0x4c, `int nAggroPct`) and clamped to [0, 100] as `u8`.
    #[test]
    fn aggro_pct_stored_and_readable_per_slot() {
        // Fixture XTarget bytes: two slots with known aggro% values.
        let list = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 111,
                    name: "a_goblin".into(),
                    aggro_pct: 100,
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 222,
                    name: "a_skeleton".into(),
                    aggro_pct: 42,
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::GroupTank,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 333,
                    name: "tank_pc".into(),
                    aggro_pct: 0,
                },
            ],
            auto_add_haters: true,
        };

        assert_eq!(list.slots[0].aggro_pct, 100);
        assert_eq!(list.slots[1].aggro_pct, 42);
        assert_eq!(list.slots[2].aggro_pct, 0);

        // Verify aggro_pct is 0-100 range (u8 bounds enforce max 255,
        // reader clamps EQ's int to [0,100]).
        for slot in &list.slots {
            assert!(slot.aggro_pct <= 100);
        }
    }

    /// Verify aggro_pct clamp behaviour mirrors the reader logic.
    #[test]
    fn aggro_pct_clamp_to_valid_range() {
        // Simulate what the Windows reader does: raw_int.clamp(0, 100) as u8
        let raw_values: &[(i32, u8)] = &[
            (-1, 0),   // negative raw → clamped to 0
            (0, 0),
            (50, 50),
            (100, 100),
            (101, 100), // over 100 → clamped to 100
            (255, 100),
        ];
        for &(raw, expected) in raw_values {
            let clamped = raw.clamp(0, 100) as u8;
            assert_eq!(
                clamped, expected,
                "clamp({raw}) should be {expected}, got {clamped}"
            );
        }
    }
}
