pub mod aggro;
pub mod classes;
pub mod dot_tracker;
pub mod gcd;
pub mod holyshit;
pub mod humanize;
pub mod loot;
pub mod mana;
pub mod mez_queue;
pub mod positioning;
pub mod skill_cooldowns;
pub mod state;
pub mod strategy;

use std::sync::Mutex;

use dmft_common::combat::{CombatConfig, CombatStatus};
use dmft_common::types::SpawnData;

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
    let mut guard = COMBATANT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = Some(combatant);
    tracing::info!(class_id, client_id, "Combat system initialized");
}

/// Advance the combat FSM by one game tick.
/// Called from the game loop hook every frame.
pub fn tick(player: &SpawnData, target: Option<&SpawnData>, nearby: &[SpawnData]) {
    let mut guard = COMBATANT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(combatant) = guard.as_mut() {
        combatant.tick(player, target, nearby);
    }
}

/// Get the current combat status for IPC reporting.
pub fn status() -> CombatStatus {
    let guard = COMBATANT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    match guard.as_ref() {
        Some(combatant) => combatant.status(),
        None => CombatStatus::Idle,
    }
}

/// Handle an incoming combat command.
pub fn handle_command(cmd: CombatCommand) {
    let mut guard = COMBATANT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
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
