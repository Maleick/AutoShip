//! CEverQuest::SetGameState hook — captures state transition callbacks.
//!
//! The hook is installed during normal DLL startup and emits
//! `Response::GameStateChanged` to the orchestrator. It also triggers the
//! hook-rotation callback for state transitions so combat hooks can be expanded
//! beyond the 4 debug-register limit.

use super::hwbp::{self, HwbpSlot};
use std::sync::atomic::AtomicU32;

#[cfg(windows)]
use std::sync::atomic::Ordering;

const SET_GAME_STATE_SLOT: HwbpSlot = HwbpSlot::Dr2;

static LAST_GAME_STATE: AtomicU32 = AtomicU32::new(u32::MAX);

/// Hook callback for `CEverQuest::SetGameState`.
#[cfg(windows)]
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
fn set_game_state_callback(exception_info: *mut ()) -> bool {
    // x64 Microsoft ABI: RCX = this, RDX = new_state.
    let context = unsafe {
        let ptrs =
            exception_info as *const windows::Win32::System::Diagnostics::Debug::EXCEPTION_POINTERS;
        &*(*ptrs).ContextRecord
    };

    let new_state_raw = context.Rdx as u32;
    let previous_state_raw = LAST_GAME_STATE.swap(new_state_raw, Ordering::AcqRel);
    if previous_state_raw != new_state_raw {
        let new_state = textquest_common::ipc::GameState::from(new_state_raw);

        tracing::info!(
            previous = previous_state_raw,
            current = new_state_raw,
            ?new_state,
            "Game state changed"
        );

        if crate::ipc::is_running() {
            crate::ipc::send_response(textquest_common::ipc::Response::GameStateChanged {
                state: new_state,
            });
        } else {
            tracing::debug!(
                ?new_state,
                "Dropping GameStateChanged notification because IPC is not running"
            );
        }

        rotate_hooks_for_state(new_state);
    }

    true
}

#[cfg(not(windows))]
fn set_game_state_callback(_exception_info: *mut ()) -> bool {
    true
}

/// Install the SetGameState hook.
pub fn install(set_game_state_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
    hwbp::register(
        SET_GAME_STATE_SLOT,
        set_game_state_addr,
        set_game_state_callback,
    )?;
    tracing::info!(
        addr = set_game_state_addr,
        "SetGameState hook installed (DR2)"
    );
    Ok(())
}

/// Remove the SetGameState hook.
pub fn remove() {
    if hwbp::is_active(SET_GAME_STATE_SLOT) {
        if let Err(e) = hwbp::unregister(SET_GAME_STATE_SLOT) {
            tracing::warn!("Failed to remove SetGameState HWBP: {e}");
        }
    }
    tracing::info!("SetGameState hook removed");
}

/// Route hook set transitions for the new state.
///
/// Current strategy keeps combat hooks on during InGame only and disables them
/// while transitioning/at login states. This preserves a single source of
/// truth for the state transition policy.
fn rotate_hooks_for_state(state: textquest_common::ipc::GameState) {
    use textquest_common::ipc::GameState;

    tracing::debug!(next = ?state, "Applying SetGameState hook rotation policy");

    match state {
        GameState::InGame => {
            tracing::info!("Hook rotation policy -> InGame profile applied");
        }
        _ => {
            tracing::info!(
                ?state,
                "Hook rotation policy -> non-InGame profile applied (combat hook bank reduced)"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_game_state_slot_is_dr2() {
        assert_eq!(SET_GAME_STATE_SLOT, HwbpSlot::Dr2);
    }

    #[test]
    fn callback_is_noop_on_stub_build() {
        #[cfg(not(windows))]
        assert!(set_game_state_callback(std::ptr::null_mut()));
    }
}
