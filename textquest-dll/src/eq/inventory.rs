use textquest_common::ipc::{ContainerSlotInfo, ContainerSlotItemInfo, ContainerSlotQuery};

#[cfg(windows)]
use std::collections::HashMap;
#[cfg(windows)]
use std::mem::size_of;
#[cfg(windows)]
use textquest_common::offsets;

const MAX_INV_SLOTS: usize = 4000;
const ITEM_NAME_LEN: usize = 64;

#[derive(Clone)]
struct RawSlotSnapshot {
    location: i32,
    top_slot: i16,
    bag_slot: i16,
    aug_slot: i16,
    manager_slot_index: i32,
    is_selected: bool,
    is_find_selected: bool,
    quantity: i32,
    recast_left: i32,
    is_linked: bool,
    item: Option<ContainerSlotItemInfo>,
}

#[derive(Clone)]
struct ParentContainerInfo {
    name: String,
    id: i32,
}

/// Query open container-window slots from the live EQ client and apply filtering.
pub fn query_open_container_slots(
    eq_base: u64,
    filter: &ContainerSlotQuery,
) -> Vec<ContainerSlotInfo> {
    #[cfg(windows)]
    {
        unsafe { query_open_container_slots_windows(eq_base, filter) }
    }

    #[cfg(not(windows))]
    {
        let _ = (eq_base, filter);
        Vec::new()
    }
}

fn normalize_filter_value(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn container_instance_name(location: i32) -> &'static str {
    match location {
        -1 => "invalid",
        0 => "possessions",
        1 => "bank",
        2 => "sharedbank",
        3 => "trade",
        4 => "world",
        5 => "limbo",
        6 => "tribute",
        7 => "trophytribute",
        8 => "guildtribute",
        9 => "merchant",
        10 => "deleted",
        11 => "corpse",
        12 => "bazaar",
        13 => "inspect",
        14 => "realestate",
        15 => "viewmodpc",
        16 => "viewmodbank",
        17 => "viewmodsharedbank",
        18 => "viewmodlimbo",
        19 => "altstorage",
        20 => "archived",
        21 => "mail",
        22 => "guildtrophytribute",
        23 => "krono",
        24 => "other",
        25 => "mercenaryitems",
        26 => "viewmodmercenaryitems",
        27 => "mountkeyringitems",
        28 => "viewmodmountkeyringitems",
        29 => "illusionkeyringitems",
        30 => "viewmodillusionkeyringitems",
        31 => "familiarkeyringitems",
        32 => "viewmodfamiliarkeyringitems",
        33 => "heroforgekeyringitems",
        34 => "viewmodheroforgekeyringitems",
        35 => "teleportationkeyringitems",
        36 => "viewmodteleportationkeyringitems",
        37 => "activatedkeyringitems",
        38 => "viewmodkeyringitems",
        39 => "equipmentkeyringitems",
        40 => "viewmodequipmentkeyringitems",
        41 => "overflow",
        42 => "dragonhoard",
        43 => "tradeskilldepot",
        44 => "guilddepot",
        45 => "personaequip",
        _ => "unknown",
    }
}

fn matches_filter(slot: &ContainerSlotInfo, filter: &ContainerSlotQuery) -> bool {
    if let Some(location) = &filter.location
        && normalize_filter_value(&slot.location_name) != normalize_filter_value(location)
    {
        return false;
    }

    if let Some(top_slot) = filter.top_slot
        && slot.top_slot != top_slot
    {
        return false;
    }

    if let Some(bag_slot) = filter.bag_slot
        && slot.bag_slot != bag_slot
    {
        return false;
    }

    if !filter.include_empty && slot.is_empty {
        return false;
    }

    if let Some(needle) = &filter.item_name_contains {
        let needle = needle.to_ascii_lowercase();
        let Some(item) = &slot.item else {
            return false;
        };
        if !item.name.to_ascii_lowercase().contains(&needle) {
            return false;
        }
    }

    true
}

struct PreparedContainerSlotQuery {
    normalized_location: Option<String>,
    top_slot: Option<i16>,
    bag_slot: Option<i16>,
    normalized_item_name: Option<String>,
    include_empty: bool,
}

impl From<&ContainerSlotQuery> for PreparedContainerSlotQuery {
    fn from(value: &ContainerSlotQuery) -> Self {
        Self {
            normalized_location: value.location.as_deref().map(normalize_filter_value),
            top_slot: value.top_slot,
            bag_slot: value.bag_slot,
            normalized_item_name: value
                .item_name_contains
                .as_ref()
                .map(|name| name.to_ascii_lowercase()),
            include_empty: value.include_empty,
        }
    }
}

fn matches_prepared_filter(slot: &ContainerSlotInfo, filter: &PreparedContainerSlotQuery) -> bool {
    if let Some(location) = &filter.normalized_location
        && normalize_filter_value(&slot.location_name) != *location
    {
        return false;
    }

    if let Some(top_slot) = filter.top_slot
        && slot.top_slot != top_slot
    {
        return false;
    }

    if let Some(bag_slot) = filter.bag_slot
        && slot.bag_slot != bag_slot
    {
        return false;
    }

    if !filter.include_empty && slot.is_empty {
        return false;
    }

    if let Some(needle) = &filter.normalized_item_name {
        let Some(item) = &slot.item else {
            return false;
        };
        if !item.name.to_ascii_lowercase().contains(needle) {
            return false;
        }
    }

    true
}

#[cfg(windows)]
unsafe fn query_open_container_slots_windows(
    eq_base: u64,
    filter: &ContainerSlotQuery,
) -> Vec<ContainerSlotInfo> {
    type GetItemBaseFn = unsafe extern "C" fn(usize, *mut usize);

    let Some(mgr_addr) = offsets::rebase(offsets::PINST_CINV_SLOT_MGR, eq_base) else {
        return Vec::new();
    };
    let Some(mgr_ptr) = read_value::<usize>(mgr_addr) else {
        return Vec::new();
    };

    let Some(total_slots) = read_value::<i32>(mgr_ptr + offsets::inv_slot_mgr::TOTAL_SLOTS) else {
        return Vec::new();
    };
    let total_slots = total_slots.clamp(0, MAX_INV_SLOTS as i32) as usize;

    let Some(get_item_base_addr) = offsets::rebase(offsets::INV_SLOT_GET_ITEM_BASE, eq_base) else {
        return Vec::new();
    };
    if !crate::eq::validate_fn_ptr(get_item_base_addr, "CInvSlot::GetItemBase") {
        return Vec::new();
    }
    let get_item_base: GetItemBaseFn = unsafe { std::mem::transmute(get_item_base_addr) };

    let mut raw_slots = Vec::new();
    for idx in 0..total_slots {
        let slot_addr = mgr_ptr + offsets::inv_slot_mgr::SLOT_ARRAY + idx * size_of::<usize>();
        let Some(slot_ptr) = read_value::<usize>(slot_addr) else {
            continue;
        };
        if slot_ptr == 0 {
            continue;
        }
        if let Some(snapshot) = read_slot_snapshot(slot_ptr, idx as i32, get_item_base) {
            raw_slots.push(snapshot);
        }
    }

    let parent_slots: HashMap<(i32, i16), ParentContainerInfo> = raw_slots
        .iter()
        .filter(|slot| slot.bag_slot < 0)
        .filter_map(|slot| {
            let item = slot.item.as_ref()?;
            Some((
                (slot.location, slot.top_slot),
                ParentContainerInfo {
                    name: item.name.clone(),
                    id: item.id,
                },
            ))
        })
        .collect();
    let prepared_filter = PreparedContainerSlotQuery::from(filter);

    raw_slots
        .into_iter()
        .filter(|slot| slot.bag_slot >= 0)
        .map(|slot| {
            let parent = parent_slots.get(&(slot.location, slot.top_slot));
            ContainerSlotInfo {
                location: slot.location,
                location_name: container_instance_name(slot.location).to_string(),
                top_slot: slot.top_slot,
                bag_slot: slot.bag_slot,
                aug_slot: slot.aug_slot,
                manager_slot_index: slot.manager_slot_index,
                is_selected: slot.is_selected,
                is_find_selected: slot.is_find_selected,
                quantity: slot.quantity,
                recast_left: slot.recast_left,
                is_linked: slot.is_linked,
                is_empty: slot.item.is_none(),
                container_name: parent.map(|info| info.name.clone()),
                container_item_id: parent.map(|info| info.id),
                item: slot.item,
            }
        })
        .filter(|slot| matches_prepared_filter(slot, &prepared_filter))
        .collect()
}

#[cfg(windows)]
unsafe fn read_slot_snapshot(
    slot_ptr: usize,
    manager_slot_index: i32,
    get_item_base: unsafe extern "C" fn(usize, *mut usize),
) -> Option<RawSlotSnapshot> {
    let wnd_ptr = read_value::<usize>(slot_ptr + offsets::inv_slot::WINDOW)?;
    if wnd_ptr == 0 {
        return None;
    }

    let location = read_item_global_index(wnd_ptr + offsets::inv_slot_wnd::ITEM_LOCATION)?;
    let item = read_slot_item(slot_ptr, get_item_base);

    Some(RawSlotSnapshot {
        location: location.0,
        top_slot: location.1,
        bag_slot: location.2,
        aug_slot: location.3,
        manager_slot_index,
        is_selected: read_value::<bool>(wnd_ptr + offsets::inv_slot_wnd::SELECTED).unwrap_or(false),
        is_find_selected: read_value::<bool>(wnd_ptr + offsets::inv_slot_wnd::FIND_SELECTED)
            .unwrap_or(false),
        quantity: read_value::<i32>(wnd_ptr + offsets::inv_slot_wnd::QUANTITY).unwrap_or(0),
        recast_left: read_value::<i32>(wnd_ptr + offsets::inv_slot_wnd::RECAST_LEFT).unwrap_or(0),
        is_linked: read_value::<bool>(wnd_ptr + offsets::inv_slot_wnd::LINKED).unwrap_or(false),
        item,
    })
}

#[cfg(windows)]
unsafe fn read_slot_item(
    slot_ptr: usize,
    get_item_base: unsafe extern "C" fn(usize, *mut usize),
) -> Option<ContainerSlotItemInfo> {
    let mut item_ptr = 0usize;
    unsafe {
        get_item_base(slot_ptr, &mut item_ptr);
    }
    if item_ptr == 0
        || !crate::hooks::game_loop::is_readable(item_ptr, offsets::item_base::GLOBAL_INDEX)
    {
        return None;
    }

    let item_def_ptr = read_value::<usize>(item_ptr + offsets::item_base::ITEM_DEF)?;
    if item_def_ptr == 0
        || !crate::hooks::game_loop::is_readable(
            item_def_ptr,
            offsets::item_definition::STACK_SIZE + 4,
        )
    {
        return None;
    }

    let id = read_value::<i32>(item_ptr + offsets::item_base::ID)
        .or_else(|| read_value::<i32>(item_def_ptr + offsets::item_definition::ITEM_NUMBER))
        .unwrap_or_default();
    let name = read_fixed_c_string(item_def_ptr + offsets::item_definition::NAME, ITEM_NAME_LEN)
        .filter(|value| !value.is_empty())?;
    let icon_id =
        read_value::<i32>(item_def_ptr + offsets::item_definition::ICON_NUMBER).unwrap_or(0);
    let charges = read_value::<i32>(item_ptr + offsets::item_base::CHARGES).unwrap_or(0);
    let stack_count = read_value::<i32>(item_ptr + offsets::item_base::STACK_COUNT).unwrap_or(0);
    let stack_size =
        read_value::<i32>(item_def_ptr + offsets::item_definition::STACK_SIZE).unwrap_or(0);
    let item_type = read_value::<u8>(item_def_ptr + offsets::item_definition::TYPE).unwrap_or(0);
    let item_class =
        read_value::<u8>(item_def_ptr + offsets::item_definition::ITEM_CLASS).unwrap_or(0);
    let is_container =
        read_value::<u8>(item_def_ptr + offsets::item_definition::CONTAINER_SLOTS).unwrap_or(0) > 0;

    Some(ContainerSlotItemInfo {
        id,
        name,
        icon_id,
        charges,
        stack_count,
        stack_size,
        item_type,
        item_class,
        is_container,
    })
}

#[cfg(windows)]
fn read_item_global_index(addr: usize) -> Option<(i32, i16, i16, i16)> {
    if !crate::hooks::game_loop::is_readable(addr, offsets::item_global_index::SIZE) {
        return None;
    }

    Some((
        read_value::<i32>(addr + offsets::item_global_index::LOCATION)?,
        read_value::<i16>(addr + offsets::item_global_index::SLOT1)?,
        read_value::<i16>(addr + offsets::item_global_index::SLOT2)?,
        read_value::<i16>(addr + offsets::item_global_index::SLOT3)?,
    ))
}

#[cfg(windows)]
fn read_fixed_c_string(addr: usize, max_len: usize) -> Option<String> {
    if !crate::hooks::game_loop::is_readable(addr, max_len) {
        return None;
    }

    let bytes = unsafe { std::slice::from_raw_parts(addr as *const u8, max_len) };
    let len = bytes.iter().position(|b| *b == 0).unwrap_or(max_len);
    std::str::from_utf8(&bytes[..len])
        .ok()
        .map(ToOwned::to_owned)
}

#[cfg(windows)]
fn read_value<T: Copy>(addr: usize) -> Option<T> {
    if !crate::hooks::game_loop::is_readable(addr, size_of::<T>()) {
        return None;
    }
    Some(unsafe { std::ptr::read_unaligned(addr as *const T) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_slot() -> ContainerSlotInfo {
        ContainerSlotInfo {
            location: 0,
            location_name: "possessions".into(),
            top_slot: 22,
            bag_slot: 3,
            aug_slot: -1,
            manager_slot_index: 701,
            is_selected: false,
            is_find_selected: false,
            quantity: 1,
            recast_left: 0,
            is_linked: false,
            is_empty: false,
            container_name: Some("Traveler's Rucksack".into()),
            container_item_id: Some(1729),
            item: Some(ContainerSlotItemInfo {
                id: 1001,
                name: "Cloudy Potion".into(),
                icon_id: 77,
                charges: 5,
                stack_count: 1,
                stack_size: 20,
                item_type: 0,
                item_class: 0,
                is_container: false,
            }),
        }
    }

    #[test]
    fn normalize_filter_value_ignores_spacing_and_punctuation() {
        assert_eq!(normalize_filter_value("Shared Bank"), "sharedbank");
        assert_eq!(normalize_filter_value("shared_bank"), "sharedbank");
        assert_eq!(normalize_filter_value("shared-bank"), "sharedbank");
    }

    #[test]
    fn matches_filter_rejects_empty_slots_by_default() {
        let mut slot = sample_slot();
        slot.is_empty = true;
        slot.item = None;

        assert!(!matches_filter(&slot, &ContainerSlotQuery::default()));
        assert!(matches_filter(
            &slot,
            &ContainerSlotQuery {
                include_empty: true,
                ..Default::default()
            }
        ));
    }

    #[test]
    fn matches_filter_applies_location_slot_and_name_filters() {
        let slot = sample_slot();
        assert!(matches_filter(
            &slot,
            &ContainerSlotQuery {
                location: Some("Possessions".into()),
                top_slot: Some(22),
                bag_slot: Some(3),
                item_name_contains: Some("cloudy".into()),
                include_empty: false,
            }
        ));
        assert!(!matches_filter(
            &slot,
            &ContainerSlotQuery {
                location: Some("bank".into()),
                ..Default::default()
            }
        ));
        assert!(!matches_filter(
            &slot,
            &ContainerSlotQuery {
                item_name_contains: Some("distillate".into()),
                ..Default::default()
            }
        ));
    }
}
