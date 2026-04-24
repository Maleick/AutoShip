//! Hook management -- hardware breakpoint hooks via VEH (DR0-DR3).
//!
//! Hook catalog:
//! [docs/research/hook-detection-surface.md](../docs/research/
//! hook-detection-surface.md).

/// Runtime hook catalog reference for detector-tuning work.
pub const HOOK_CATALOG: &str = "docs/research/hook-detection-surface.md";

pub mod casting;
pub mod chat;
pub mod detours;
pub mod dx11_null;
pub mod eqmain_hook;
pub mod file_integrity_dispatcher;
pub mod fingerprint;
pub mod game_loop;
pub mod hwbp;
pub mod integrity;
pub mod memcheck;
pub mod movement;
pub mod overlay;
pub mod packet_hook;
pub mod render;
pub mod rotation;
pub mod set_game_state;
pub mod slot_manager;
pub mod targeting;
pub mod timing;
pub mod wmi;
pub mod zone_entry_integrity;

use std::sync::{Mutex, OnceLock};

use slot_manager::HookGameState;

static SLOT_MANAGER: OnceLock<Mutex<slot_manager::HookSlotManager>> = OnceLock::new();

fn manager() -> &'static Mutex<slot_manager::HookSlotManager> {
    SLOT_MANAGER.get_or_init(|| Mutex::new(slot_manager::HookSlotManager::new()))
}

/// Update the active game state, rotating HWBP slot assignments if the state
/// changed.
///
/// This records the new planned slot layout via [`HookSlotManager`] and logs
/// the transition. Actual HWBP register writes happen on the Windows game
/// thread and are wired separately in the platform-specific hook installation
/// path.
pub fn set_game_state(game_state: HookGameState) {
    let rotation = {
        let mut guard = manager()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.rotate_hooks(game_state)
    };

    if rotation.changed {
        tracing::info!(
            state = %rotation.state,
            "HWBP slot plan updated for game state transition"
        );
    }
}

pub fn install_all() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

pub fn remove_all() {
    tracing::info!("Removing all hooks...");
    detours::remove_all();
    hwbp::remove_all();
    fingerprint::remove();
    chat::remove();
    memcheck::remove();
    timing::remove();
    set_game_state::remove();
    timing::remove();
    wmi::remove();
    zone_entry_integrity::remove();
    tracing::info!("All hooks removed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use slot_manager::{HookGameState, HookKind};

    #[test]
    fn set_game_state_does_not_panic() {
        // Exercises the OnceLock init + Mutex lock path.
        set_game_state(HookGameState::Login);
        set_game_state(HookGameState::InGame);
        set_game_state(HookGameState::ZoneLoading);
    }

    #[test]
    fn set_game_state_idempotent() {
        // Calling the same state twice should not panic.
        set_game_state(HookGameState::InGame);
        set_game_state(HookGameState::InGame);
    }

    #[test]
    fn install_all_returns_ok() {
        assert!(install_all().is_ok());
    }

    #[test]
    fn remove_all_does_not_panic() {
        remove_all();
    }

    #[test]
    fn manager_returns_same_instance() {
        let a = manager() as *const _;
        let b = manager() as *const _;
        assert_eq!(a, b, "OnceLock should return the same instance");
    }

    #[test]
    fn set_game_state_updates_slot_plan() {
        // Verify the underlying manager state updates
        set_game_state(HookGameState::InGame);
        let guard = manager().lock().unwrap();
        assert_eq!(guard.state(), HookGameState::InGame);
        assert_eq!(guard.assignments()[0], Some(HookKind::ProcessGameEvents));
    }

    #[test]
    fn hook_catalog_constant_is_set() {
        assert!(!HOOK_CATALOG.is_empty());
        assert!(HOOK_CATALOG.contains("hook-detection"));
    }
}
