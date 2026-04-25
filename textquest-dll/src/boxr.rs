//! MQ2Boxr interop — implements /boxr command and BoxrPause event.
//!
//! Provides pause/resume automation control compatible with RedGuides
//! ecosystem broadcasting (`/bcaa //boxr Pause/Resume/Status`).

use std::sync::atomic::{AtomicBool, Ordering};

use crate::commands::{CommandResult, register_script_command};
use textquest_common::ipc::Response;

/// Local pause state flag — tracks whether TextQuest automation is paused.
///
/// When paused, the game loop skips command dispatch. This state can be set
/// via `/boxr Pause` or via `Command::Pause` from the orchestrator.
static SESSION_PAUSED: AtomicBool = AtomicBool::new(false);

/// Previous pause state for event firing.
///
/// Tracks the last pause state to detect transitions and fire BoxrPause events.
static PREV_PAUSED_STATE: std::sync::OnceLock<std::sync::Mutex<bool>> = std::sync::OnceLock::new();

/// Register the /boxr command handler.
///
/// Called during DLL initialization to expose pause/resume/status control.
pub fn register_boxr_command() {
    register_script_command("/boxr", boxr_handler);
}

fn boxr_handler(args: &[&str]) -> CommandResult {
    if args.is_empty() {
        return CommandResult::Message(
            "Usage: /boxr Pause|Resume|Status".to_string(),
        );
    }

    match args[0].to_lowercase().as_str() {
        "pause" => handle_pause(),
        "resume" => handle_resume(),
        "status" => handle_status(),
        _ => CommandResult::Message(format!(
            "Unknown /boxr subcommand: {}. Usage: /boxr Pause|Resume|Status",
            args[0]
        )),
    }
}

fn handle_pause() -> CommandResult {
    set_paused(true);
    crate::ipc::send_response(Response::CommandResult {
        success: true,
        message: "Paused by /boxr".to_string(),
    });
    CommandResult::Message("[Boxr] Paused".to_string())
}

fn handle_resume() -> CommandResult {
    set_paused(false);
    crate::ipc::send_response(Response::CommandResult {
        success: true,
        message: "Resumed by /boxr".to_string(),
    });
    CommandResult::Message("[Boxr] Resumed".to_string())
}

fn handle_status() -> CommandResult {
    let paused = is_paused();
    let status = if paused { "PAUSED" } else { "ACTIVE" };
    crate::ipc::send_response(Response::PauseStatus { paused });
    CommandResult::Message(format!("[Boxr] Status: {}", status))
}

/// Set the pause state and fire BoxrPause event if state changed.
///
/// Used by `/boxr Pause/Resume` and orchestrator `Command::Pause/Resume`.
pub fn set_paused(paused: bool) {
    let prev_lock = PREV_PAUSED_STATE.get_or_init(|| std::sync::Mutex::new(false));
    if let Ok(mut prev) = prev_lock.lock() {
        if *prev != paused {
            *prev = paused;
            tracing::info!(paused, "Pause state changed — firing BoxrPause event");
            fire_boxr_pause_event(paused);
        }
    }
    SESSION_PAUSED.store(paused, Ordering::SeqCst);
}

/// Query the current pause state.
///
/// Returns `true` if paused, `false` if active. Used by `/boxr Status` and
/// pause state queries.
pub fn is_paused() -> bool {
    SESSION_PAUSED.load(Ordering::SeqCst)
}

/// Fire the BoxrPause event when pause state changes.
///
/// Signals to MQ2 macros that the pause state has changed via an event.
/// In actual MQ2, this would trigger registered event handlers.
fn fire_boxr_pause_event(paused: bool) {
    let event_name = if paused { "BoxrPaused" } else { "BoxrResumed" };
    crate::ipc::send_response(Response::CommandResult {
        success: true,
        message: format!("Event::{}", event_name),
    });
}
