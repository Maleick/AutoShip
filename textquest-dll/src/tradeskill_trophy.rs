//! Tradeskill trophy automation.
//!
//! Mirrors MQ2TSTrophy's crafting-container detection and slot selection, but
//! keeps the state machine in `textquest-common` so the orchestration logic is
//! testable outside the injected DLL.

use std::sync::{LazyLock, Mutex};

use textquest_common::tradeskill_trophy::{
    TradeskillTrophyManager, TradeskillTrophySettings, TradeskillTrophyStatus,
};
#[cfg(windows)]
use textquest_common::{
    ipc::ContainerSlotQuery,
    tradeskill_trophy::{
        TradeskillTrophyObservation, TrophyEquipSlot, detect_tradeskill_container_type,
        preferred_trophy_slot,
    },
};

const POSSESSIONS_LOCATION: i32 = 0;
const WORLD_LOCATION: &str = "world";
const MAINHAND_TOP_SLOT: i16 = 13;
const AMMO_TOP_SLOT: i16 = 22;
// MacroQuest slot names use 35 for the cursor slot, but older EQ inventory
// tables report 33. Probe both so the observer tolerates either layout.
const CURSOR_TOP_SLOT_CANDIDATES: &[i16] = &[35, 33];

static TROPHY_MANAGER: LazyLock<Mutex<TradeskillTrophyManager>> =
    LazyLock::new(|| Mutex::new(TradeskillTrophyManager::default()));

pub fn set_settings(settings: TradeskillTrophySettings) {
    let enabled = settings.enabled;
    let trophy_item_name = settings.trophy_item_name.trim().to_string();
    let mut manager = TROPHY_MANAGER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    manager.update_settings(settings);
    tracing::info!(
        enabled,
        trophy_item_name,
        "Tradeskill trophy settings updated"
    );
}

#[must_use]
pub fn status() -> TradeskillTrophyStatus {
    TROPHY_MANAGER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .status()
}

#[cfg(windows)]
pub fn check() {
    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    if eq_base == 0 {
        return;
    }

    let Some(local_player_addr) =
        textquest_common::offsets::rebase(textquest_common::offsets::PINST_LOCAL_PLAYER, eq_base)
    else {
        return;
    };
    let local_player = unsafe { *(local_player_addr as *const usize) };
    if local_player == 0 {
        return;
    }

    let (settings, previous_status, active_trophy_item_name) = {
        let manager = TROPHY_MANAGER
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (
            manager.settings().clone(),
            manager.status(),
            manager.active_trophy_item_name().map(str::to_string),
        )
    };
    if !settings.is_configured() && !previous_status.active {
        return;
    }

    let observation = observe(
        eq_base,
        &settings,
        &previous_status,
        active_trophy_item_name.as_deref(),
    );
    let command = {
        let mut manager = TROPHY_MANAGER
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        manager.tick(&observation)
    };

    if let Some(command) = command {
        tracing::info!(command, "Tradeskill trophy automation queued slash command");
        crate::hooks::game_loop::queue_slash_command(command);
    }
}

#[cfg(not(windows))]
pub fn check() {}

#[cfg(windows)]
fn observe(
    eq_base: u64,
    settings: &TradeskillTrophySettings,
    previous_status: &TradeskillTrophyStatus,
    active_trophy_item_name: Option<&str>,
) -> TradeskillTrophyObservation {
    let tracked_trophy_item_name = active_trophy_item_name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(settings.trophy_item_name.as_str());
    let open_container_name = current_world_container_name(eq_base);
    let container_type = open_container_name
        .as_deref()
        .and_then(detect_tradeskill_container_type);
    let target_slot =
        observed_target_slot(tracked_trophy_item_name, previous_status, container_type);
    let equipped_item = target_slot.and_then(|slot| current_equipped_item(eq_base, slot));
    let cursor_item = current_cursor_item(eq_base);

    let trophy_equipped = equipped_item
        .as_ref()
        .is_some_and(|item| item_name_matches(&item.name, tracked_trophy_item_name));
    let trophy_charges = if trophy_equipped {
        equipped_item.as_ref().map(|item| item.charges)
    } else if cursor_item
        .as_ref()
        .is_some_and(|item| item_name_matches(&item.name, tracked_trophy_item_name))
    {
        cursor_item.as_ref().map(|item| item.charges)
    } else {
        None
    };

    TradeskillTrophyObservation {
        open_container_name,
        cursor_item_name: cursor_item.map(|item| item.name),
        trophy_equipped,
        trophy_charges,
    }
}

#[cfg(windows)]
fn current_world_container_name(eq_base: u64) -> Option<String> {
    crate::eq::inventory::query_open_container_slots(
        eq_base,
        &ContainerSlotQuery {
            location: Some(WORLD_LOCATION.to_string()),
            include_empty: true,
            ..Default::default()
        },
    )
    .into_iter()
    .find_map(|slot| slot.container_name)
}

#[cfg(windows)]
fn current_equipped_item(
    eq_base: u64,
    slot: TrophyEquipSlot,
) -> Option<textquest_common::ipc::ContainerSlotItemInfo> {
    crate::eq::inventory::query_top_level_slot_item(
        eq_base,
        POSSESSIONS_LOCATION,
        match slot {
            TrophyEquipSlot::Ammo => AMMO_TOP_SLOT,
            TrophyEquipSlot::Mainhand => MAINHAND_TOP_SLOT,
        },
    )
}

#[cfg(windows)]
fn current_cursor_item(eq_base: u64) -> Option<textquest_common::ipc::ContainerSlotItemInfo> {
    CURSOR_TOP_SLOT_CANDIDATES.iter().find_map(|slot| {
        crate::eq::inventory::query_top_level_slot_item(eq_base, POSSESSIONS_LOCATION, *slot)
    })
}

#[cfg(windows)]
fn item_name_matches(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
}

fn observed_target_slot(
    tracked_trophy_item_name: &str,
    previous_status: &TradeskillTrophyStatus,
    container_type: Option<textquest_common::tradeskill_trophy::TradeskillContainerType>,
) -> Option<textquest_common::tradeskill_trophy::TrophyEquipSlot> {
    container_type
        .map(|kind| {
            textquest_common::tradeskill_trophy::preferred_trophy_slot(
                tracked_trophy_item_name,
                kind,
            )
        })
        .or(previous_status.target_slot)
}

#[cfg(test)]
mod tests {
    use super::observed_target_slot;
    use textquest_common::tradeskill_trophy::{
        TradeskillContainerType, TradeskillTrophyStatus, TrophyEquipSlot,
    };

    #[test]
    fn current_container_slot_overrides_previous_status_slot() {
        let previous_status = TradeskillTrophyStatus {
            target_slot: Some(TrophyEquipSlot::Ammo),
            ..Default::default()
        };

        assert_eq!(
            observed_target_slot(
                "Blessed Akhevan Shadow Shears",
                &previous_status,
                Some(TradeskillContainerType::Tailoring)
            ),
            Some(TrophyEquipSlot::Mainhand)
        );
    }

    #[test]
    fn closed_container_reuses_previous_status_slot() {
        let previous_status = TradeskillTrophyStatus {
            target_slot: Some(TrophyEquipSlot::Ammo),
            ..Default::default()
        };

        assert_eq!(
            observed_target_slot("Any Trophy", &previous_status, None),
            Some(TrophyEquipSlot::Ammo)
        );
    }
}
