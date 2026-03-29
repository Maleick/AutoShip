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
        Command::StartLogin { .. } => {
            if let Command::StartLogin {
                account_name,
                password,
                server_name,
                character_name,
            } = cmd
            {
                let eqmain_base = crate::login::eqmain::find_eqmain();
                if eqmain_base == 0 {
                    tracing::error!("Cannot login: eqmain.dll not found");
                    return true;
                }

                tracing::info!(
                    account = %account_name,
                    eqmain_base = format!("{:#x}", eqmain_base),
                    "Starting full login chain"
                );

                // Phase 1: Write credentials and click Login
                if !crate::login::widgets::type_credentials_to_window(
                    eqmain_base,
                    account_name,
                    password,
                ) {
                    tracing::error!("Failed to write credentials to UI fields");
                    return true;
                }
                tracing::info!("Phase 1 complete: credentials written + Login clicked");

                // Phase 2: Wait for server select screen, then click Play
                // Spawn a thread so we don't block the IPC listener
                let server = server_name.clone();
                let character = character_name.clone();
                std::thread::Builder::new()
                    .name("dmft-login-chain".into())
                    .spawn(move || {
                        login_chain_phase2(server, character);
                    })
                    .ok();
            }
            true
        }
        _ => false,
    }
}

/// Phase 2+3 of the login chain: server select → character select → enter world.
/// Runs on a dedicated thread spawned from the IPC handler.
fn login_chain_phase2(server_name: String, character_name: String) {
    use dmft_common::offsets::eqmain as off;

    // Wait for server select screen to load (~7 seconds after login)
    tracing::info!("Login chain phase 2: waiting 7s for server select...");
    std::thread::sleep(std::time::Duration::from_secs(7));

    let eqmain_base = crate::login::eqmain::find_eqmain();
    if eqmain_base == 0 {
        tracing::error!("Login chain: eqmain.dll not found after login");
        return;
    }

    let Some(cxwnd_mgr) = crate::login::eqmain::resolve_cxwnd_manager(eqmain_base) else {
        tracing::error!("Login chain: CXWndManager not resolved");
        return;
    };

    // Scan for "PLAY EVERQUEST!" button
    unsafe {
        let array_ptr = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
        let count = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_COUNT) as *const u32);

        if array_ptr == 0 || count == 0 || count > 500 {
            tracing::warn!("Login chain: invalid window array");
            return;
        }

        let mut play_button: usize = 0;
        let mut enter_world_button: usize = 0;

        for i in 0..count as usize {
            let wnd_ptr = *((array_ptr + i * 8) as *const usize);
            if wnd_ptr == 0 { continue; }

            if let Some(text) = crate::login::widgets::read_cxstr_pub(wnd_ptr + off::CXWND_WINDOW_TEXT) {
                if text == "PLAY EVERQUEST!" {
                    play_button = wnd_ptr;
                }
                if text == "Enter World" || text == "ENTER WORLD" || text == "Enter" {
                    enter_world_button = wnd_ptr;
                }
            }
        }

        // Click "PLAY EVERQUEST!" to enter the server
        if play_button != 0 {
            tracing::info!(
                ptr = format!("{:#x}", play_button),
                "Phase 2: Clicking PLAY EVERQUEST!"
            );
            crate::login::widgets::click_button_via_vtable(play_button);
            tracing::info!("Phase 2 complete: PLAY EVERQUEST clicked");

            // Phase 3: Wait for character select (~10 seconds), then enter world
            tracing::info!("Login chain phase 3: waiting 10s for character select...");
            std::thread::sleep(std::time::Duration::from_secs(10));

            // Re-scan windows for Enter World button (UI changed after server connect)
            let eqmain_base2 = crate::login::eqmain::find_eqmain();
            if eqmain_base2 != 0 {
                if let Some(mgr2) = crate::login::eqmain::resolve_cxwnd_manager(eqmain_base2) {
                    let array2 = *((mgr2 + off::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
                    let count2 = *((mgr2 + off::CXWNDMGR_WINDOWS_COUNT) as *const u32);

                    if array2 != 0 && count2 > 0 && count2 <= 500 {
                        for i in 0..count2 as usize {
                            let wnd_ptr = *((array2 + i * 8) as *const usize);
                            if wnd_ptr == 0 { continue; }
                            if let Some(text) = crate::login::widgets::read_cxstr_pub(
                                wnd_ptr + off::CXWND_WINDOW_TEXT
                            ) {
                                if text == "Enter World" || text == "ENTER WORLD" || text == "Enter" {
                                    enter_world_button = wnd_ptr;
                                }
                            }
                        }
                    }
                }
            }

            if enter_world_button != 0 {
                tracing::info!(
                    ptr = format!("{:#x}", enter_world_button),
                    "Phase 3: Clicking Enter World"
                );
                crate::login::widgets::click_button_via_vtable(enter_world_button);
                tracing::info!("Phase 3 complete: Enter World clicked — character should be loading");
            } else {
                tracing::warn!("Phase 3: Enter World button not found — may need manual click");
            }
        } else {
            tracing::warn!("Phase 2: PLAY EVERQUEST button not found");
        }
    }
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

    while IPC_RUNNING.load(Ordering::SeqCst) && !crate::SHUTTING_DOWN.load(Ordering::SeqCst) {
        match listener.receive() {
            Ok(cmd) => {
                tracing::debug!(client_id, ?cmd, "Received command");

                // Handle commands that must work even at the login screen
                // (before the game loop hook is running).
                if handle_immediate_command(&cmd) {
                    continue;
                }

                if let Some(pending) = PENDING_COMMANDS.get() {
                    if let Ok(mut queue) = pending.lock() {
                        queue.push(cmd);
                    }
                }
            }
            Err(e) => {
                // On pipe disconnect or error, reset auth and retry unless
                // we are shutting down.
                if IPC_RUNNING.load(Ordering::SeqCst) {
                    tracing::warn!(client_id, error = %e, "Command listener error, resetting");
                    listener.reset_auth();
                }
            }
        }
    }

    tracing::info!(client_id, "IPC listener thread exiting");
}
