//! IPC between the injected DLL and the DMFT orchestrator.
//!
//! Provides two channels:
//! - **Shared memory** (`SharedStateWriter`): DLL publishes `GameState` each tick
//! - **Named pipe** (`CommandListener`): orchestrator sends `Command`s, DLL replies
//!
//! The listener runs on a background thread. Received commands are buffered in a
//! channel and drained each game tick via `poll_commands()`.

pub mod pipe;
pub mod shared;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;

use dmft_common::ipc::{Command, Response, SessionToken};
use dmft_common::types::{ClientId, GameState};

use self::pipe::CommandListener;
use self::shared::SharedStateWriter;

/// Shared memory writer, created once at IPC start.
static SHARED_WRITER: OnceLock<Mutex<SharedStateWriter>> = OnceLock::new();

/// Buffered commands received from the orchestrator pipe.
static PENDING_COMMANDS: OnceLock<Mutex<Vec<Command>>> = OnceLock::new();

/// Flag checked by the listener thread to know when to exit.
static IPC_RUNNING: AtomicBool = AtomicBool::new(false);

/// Start IPC: shared memory writer + command listener thread.
///
/// `client_id` identifies this EQ client instance. `token` is the session token
/// generated at injection time; the orchestrator must present it when connecting
/// to the command pipe.
pub fn start(client_id: ClientId, token: SessionToken) -> Result<(), Box<dyn std::error::Error>> {
    if IPC_RUNNING.load(Ordering::SeqCst) {
        tracing::warn!(client_id, "IPC already running, ignoring duplicate start");
        return Ok(());
    }

    // --- Shared memory writer ---
    let writer = match SharedStateWriter::new(client_id) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!(client_id, error = %e, "Failed to create shared memory writer");
            return Err(e.into());
        }
    };
    let _ = SHARED_WRITER.set(Mutex::new(writer));
    let _ = PENDING_COMMANDS.set(Mutex::new(Vec::new()));

    IPC_RUNNING.store(true, Ordering::SeqCst);

    // --- Command listener thread ---
    thread::Builder::new()
        .name(format!("dmft-ipc-{client_id}"))
        .spawn(move || {
            listener_loop(client_id, token);
        })
        .map_err(|e| {
            IPC_RUNNING.store(false, Ordering::SeqCst);
            tracing::error!(client_id, error = %e, "Failed to spawn IPC listener thread");
            e
        })?;

    tracing::info!(client_id, "IPC started");
    Ok(())
}

/// Stop IPC and clean up. Safe to call multiple times.
pub fn stop() {
    if !IPC_RUNNING.swap(false, Ordering::SeqCst) {
        return;
    }

    // The listener thread checks IPC_RUNNING and will exit on its own.
    // SharedStateWriter and CommandListener clean up via Drop.
    tracing::info!("IPC stopped");
}

/// Drain any commands received since the last call. Intended to be called once
/// per game tick from the hook thread.
pub fn poll_commands() -> Vec<Command> {
    let Some(pending) = PENDING_COMMANDS.get() else {
        return Vec::new();
    };
    let Ok(mut queue) = pending.lock() else {
        tracing::error!("IPC command queue mutex poisoned");
        return Vec::new();
    };
    std::mem::take(&mut *queue)
}

/// Publish a game state snapshot to shared memory. Intended to be called once
/// per game tick from the hook thread.
pub fn publish_state(state: &GameState) {
    let Some(writer_lock) = SHARED_WRITER.get() else {
        return;
    };
    let Ok(mut writer) = writer_lock.lock() else {
        tracing::error!("IPC shared memory writer mutex poisoned");
        return;
    };
    if let Err(e) = writer.write(state) {
        tracing::error!(error = %e, "Failed to publish game state");
    }
}

/// Buffered responses to send back to the orchestrator on the next pipe write.
static PENDING_RESPONSES: OnceLock<Mutex<Vec<Response>>> = OnceLock::new();

/// Enqueue a response to be sent to the orchestrator.
/// Called from the game loop thread (e.g., login FSM phase updates).
pub fn send_response(response: Response) {
    let pending = PENDING_RESPONSES.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut queue) = pending.lock() {
        queue.push(response);
    }
}

/// Drain pending responses. Called by the IPC listener thread.
pub fn drain_responses() -> Vec<Response> {
    let Some(pending) = PENDING_RESPONSES.get() else {
        return Vec::new();
    };
    let Ok(mut queue) = pending.lock() else {
        return Vec::new();
    };
    std::mem::take(&mut *queue)
}

/// Handle commands that must work even before the game loop runs
/// (e.g., at the login screen). Returns true if handled.
fn handle_immediate_command(cmd: &Command) -> bool {
    match cmd {
        Command::CalibrateLogin => {
            let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
            let eqmain_base = crate::login::eqmain::find_eqmain();
            tracing::info!(
                eq_base = format!("{:#x}", eq_base),
                eqmain_base = format!("{:#x}", eqmain_base),
                "CalibrateLogin: running calibration dump"
            );
            crate::login::widgets::calibrate_login_dump(eqmain_base);
            true
        }
        Command::StartLogin {
            account_name,
            password,
            server_name,
            character_name,
        } => {
            // Clone password into Zeroizing wrapper so the local copy is wiped
            // from memory when this scope exits — prevents plaintext from
            // lingering on the IPC thread's stack after credential entry.
            let mut password = zeroize::Zeroizing::new(password.clone());

            tracing::info!(
                account = %account_name,
                server = %server_name,
                character = %character_name,
                "StartLogin received — delegating to FSM (password redacted)"
            );

            // Type password via WM_CHAR FIRST (before mem::take moves password).
            // This is the proven working approach — PostMessage directly to EQ's HWND.
            let eqmain_base = crate::login::eqmain::find_eqmain();
            if eqmain_base != 0 {
                tracing::info!("Typing password via WM_CHAR + Enter");
                crate::login::widgets::type_password_wm_char(eqmain_base, &password);
            }

            // Store credentials in the FSM for character select phase.
            crate::login::start_login(
                account_name.to_string(),
                std::mem::take(&mut *password),
                server_name.to_string(),
                character_name.to_string(),
            );

            // Spawn background thread for Phase 2+3 (server select → char select).
            // Polls every 500ms. Uses Enter key to dismiss dialogs.
            // Game loop tick handles Phase 4 (character select → enter world).
            std::thread::Builder::new()
                .name("dmft-login-phase2".into())
                .spawn(move || {
                    login_chain_phase2();
                })
                .ok();

            true
        }
        _ => false,
    }
}

/// Phase 2+3: poll for server select → character select.
/// Uses Enter key (PostMessage) to dismiss dialogs and advance.
/// Runs until eqmain.dll unloads, then game loop tick handles character select.
fn login_chain_phase2() {
    // Phase 2: Wait for authentication, then press Enter/click PLAY EVERQUEST
    tracing::info!("Phase 2: Waiting 5s for authentication...");
    std::thread::sleep(std::time::Duration::from_secs(5));

    // Press Enter to submit login if needed, then poll for PLAY EVERQUEST
    for attempt in 0..60 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let eqmain_base = crate::login::eqmain::find_eqmain();
        if eqmain_base == 0 {
            tracing::info!(attempt, "Phase 2: eqmain.dll gone — already at char select");
            return;
        }

        // Press Enter every 3s to dismiss dialogs or click default button
        if attempt % 6 == 0 {
            tracing::info!(attempt, "Phase 2: Pressing Enter");
            crate::login::widgets::simulate_enter_key(eqmain_base);
        }
    }

    // Phase 3: Poll for eqmain.dll unload (character select)
    tracing::info!("Phase 3: Polling for character select...");
    for attempt in 0..120 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let eqmain_base = crate::login::eqmain::find_eqmain();
        if eqmain_base == 0 {
            tracing::info!(attempt, "Phase 3: eqmain.dll unloaded — at character select");
            // Let the game loop FSM tick handle character selection
            return;
        }
        // Keep pressing Enter to dismiss dialogs
        if attempt % 6 == 3 {
            if eqmain_base != 0 {
                tracing::info!(attempt, "Phase 3: Pressing Enter to dismiss dialog");
                crate::login::widgets::simulate_enter_key(eqmain_base);
            }
        }
    }
    tracing::warn!("Phase 3: Timed out waiting for character select after 60s");
}

/// Background thread: creates a `CommandListener` and loops receiving commands
/// until `IPC_RUNNING` is cleared or the DLL is shutting down.
fn listener_loop(client_id: ClientId, token: SessionToken) {
    let mut listener = match CommandListener::new(client_id, token) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(client_id, error = %e, "Failed to create command listener");
            IPC_RUNNING.store(false, Ordering::SeqCst);
            return;
        }
    };

    tracing::info!(client_id, "IPC listener thread started");

    let mut consecutive_errors: u32 = 0;

    while IPC_RUNNING.load(Ordering::SeqCst) && !crate::SHUTTING_DOWN.load(Ordering::SeqCst) {
        match listener.receive() {
            Ok(cmd) => {
                consecutive_errors = 0;
                tracing::debug!(client_id, ?cmd, "Received command");

                if handle_immediate_command(&cmd) {
                    continue;
                }

                if let Some(pending) = PENDING_COMMANDS.get()
                    && let Ok(mut queue) = pending.lock() {
                        queue.push(cmd);
                    }
            }
            Err(e) => {
                if IPC_RUNNING.load(Ordering::SeqCst) {
                    consecutive_errors += 1;
                    if consecutive_errors <= 3 {
                        tracing::warn!(client_id, error = %e, "Command listener error, resetting");
                    } else if consecutive_errors == 4 {
                        tracing::warn!(client_id, consecutive_errors, "Suppressing repeated pipe errors");
                    }
                    listener.reset_auth();
                    let backoff_ms = std::cmp::min(10 * (1u64 << consecutive_errors.min(9)), 5000);
                    std::thread::sleep(std::time::Duration::from_millis(backoff_ms));
                }
            }
        }
    }

    tracing::info!(client_id, "IPC listener thread exiting");
}
