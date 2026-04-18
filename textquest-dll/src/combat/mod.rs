pub mod ability_cooldowns;
pub mod aggro;
pub mod buffs;
pub mod classes;
pub mod debuffs;
pub mod dot_tracker;
pub mod gcd;
pub mod holyshit;
pub mod humanize;
pub mod loot;
pub mod mana;
pub mod melee;
pub mod mez_queue;
pub mod positioning;
pub mod rotation;
pub mod skill_cooldowns;
pub mod state;
pub mod strategy;
pub mod toon_config;
pub mod twist;
pub mod xtarget;

use std::sync::Mutex;

use textquest_common::{
    combat::{CastResult, CombatConfig, CombatStatus},
    shared_client_state::SharedClientState,
    types::SpawnData,
};

use state::Combatant;

/// Global combatant singleton — one per injected DLL (one per EQ client).
static COMBATANT: Mutex<Option<Combatant>> = Mutex::new(None);

/// Commands that can be sent to the combat FSM from IPC or other subsystems.
#[derive(Clone, Copy)]
pub enum CombatCommand {
    /// Start combat against a specific spawn.
    Engage { target_id: u32 },
    /// Stop combat and return to idle.
    Disengage,
    /// Set the main-assist target for assist-train behavior.
    SetAssistTarget { spawn_id: u32 },
}

/// Initialize the combat system for this client.
/// Called once during DLL setup after we know our class and client ID.
pub fn init(class_id: u8, client_id: u32, config: CombatConfig) {
    let combatant = Combatant::new(class_id, client_id, config);
    let mut guard = COMBATANT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = Some(combatant);
    tracing::info!(class_id, client_id, "Combat system initialized");
}

/// Advance the combat FSM by one game tick.
/// Called from the game loop hook every frame.
pub fn tick(player: &SpawnData, target: Option<&SpawnData>, nearby: &[SpawnData]) {
    let mut guard = COMBATANT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(combatant) = guard.as_mut() {
        combatant.tick(player, target, nearby);
    }
}

/// Feed a chat/system line into the combat FSM so active casts can resolve
/// against real in-game feedback instead of timing out purely by duration.
pub fn observe_chat_message(text: &str) -> Option<CastResult> {
    let mut guard = COMBATANT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.as_mut()?.observe_chat_message(text)
}

/// Get the current combat status for IPC reporting.
pub fn status() -> CombatStatus {
    let guard = COMBATANT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match guard.as_ref() {
        Some(combatant) => combatant.status(),
        None => CombatStatus::Idle,
    }
}

/// Handle an incoming combat command.
pub fn handle_command(cmd: CombatCommand) {
    let mut guard = COMBATANT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(combatant) = guard.as_mut() else {
        tracing::warn!("Combat command received but combatant not initialized");
        return;
    };

    match cmd {
        CombatCommand::Engage { target_id } => combatant.engage(target_id),
        CombatCommand::Disengage => combatant.disengage(),
        CombatCommand::SetAssistTarget { spawn_id } => combatant.set_assist_target(spawn_id),
    }
}

/// Replace the full shared client roster received from the orchestrator.
pub fn set_shared_client_states(states: Vec<SharedClientState>) {
    let mut guard = COMBATANT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(combatant) = guard.as_mut() else {
        tracing::warn!("Shared client roster received but combatant not initialized");
        return;
    };
    combatant.set_shared_client_states(states);
}

#[cfg(test)]
mod tests {
    use textquest_common::combat::CombatStatus;

    use super::*;

    // ─── CombatCommand enum ────────────────────────────────────────────────────

    #[test]
    fn combat_command_engage_stores_target_id() {
        let cmd = CombatCommand::Engage { target_id: 42 };
        if let CombatCommand::Engage { target_id } = cmd {
            assert_eq!(target_id, 42);
        } else {
            panic!("expected Engage");
        }
    }

    #[test]
    fn combat_command_disengage_is_constructible() {
        let cmd = CombatCommand::Disengage;
        assert!(matches!(cmd, CombatCommand::Disengage));
    }

    #[test]
    fn combat_command_set_assist_target_stores_spawn_id() {
        let cmd = CombatCommand::SetAssistTarget { spawn_id: 999 };
        if let CombatCommand::SetAssistTarget { spawn_id } = cmd {
            assert_eq!(spawn_id, 999);
        } else {
            panic!("expected SetAssistTarget");
        }
    }

    #[test]
    fn combat_command_engage_accepts_zero_target() {
        let cmd = CombatCommand::Engage { target_id: 0 };
        if let CombatCommand::Engage { target_id } = cmd {
            assert_eq!(target_id, 0);
        } else {
            panic!("expected Engage");
        }
    }

    #[test]
    fn combat_command_engage_accepts_max_target() {
        let cmd = CombatCommand::Engage {
            target_id: u32::MAX,
        };
        if let CombatCommand::Engage { target_id } = cmd {
            assert_eq!(target_id, u32::MAX);
        } else {
            panic!("expected Engage");
        }
    }

    // ─── Module-level functions: pre-init behavior ─────────────────────────────
    //
    // The COMBATANT global starts as None. Tests here validate that each public
    // function is safe to call before `init()` is invoked.

    #[test]
    fn status_returns_idle_when_no_combatant() {
        // Override the global to None for this check. Because other tests in the
        // binary may have already called init(), we lock the mutex and temporarily
        // clear the state, then restore it.
        let mut guard = COMBATANT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = guard.take();
        drop(guard);

        let s = status();
        assert!(
            matches!(s, CombatStatus::Idle),
            "expected Idle before init, got {s:?}"
        );

        // Restore whatever was there before (important in a shared test binary).
        let mut guard = COMBATANT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = previous;
    }

    #[test]
    fn tick_is_safe_with_no_combatant() {
        let player = textquest_common::types::SpawnData::default();
        let mut guard = COMBATANT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = guard.take();
        drop(guard);

        // Should not panic.
        tick(&player, None, &[]);

        let mut guard = COMBATANT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = previous;
    }

    #[test]
    fn observe_chat_message_returns_none_with_no_combatant() {
        let mut guard = COMBATANT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = guard.take();
        drop(guard);

        let result = observe_chat_message("Your spell is interrupted.");
        assert!(
            result.is_none(),
            "expected None before init, got {result:?}"
        );

        let mut guard = COMBATANT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = previous;
    }

    #[test]
    fn handle_command_is_safe_with_no_combatant() {
        let mut guard = COMBATANT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = guard.take();
        drop(guard);

        // Should not panic — just logs a warning.
        handle_command(CombatCommand::Disengage);

        let mut guard = COMBATANT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = previous;
    }

    #[test]
    fn handle_engage_command_is_safe_with_no_combatant() {
        let mut guard = COMBATANT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = guard.take();
        drop(guard);

        handle_command(CombatCommand::Engage { target_id: 1 });

        let mut guard = COMBATANT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = previous;
    }

    #[test]
    fn handle_set_assist_command_is_safe_with_no_combatant() {
        let mut guard = COMBATANT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = guard.take();
        drop(guard);

        handle_command(CombatCommand::SetAssistTarget { spawn_id: 7 });

        let mut guard = COMBATANT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = previous;
    }
}
