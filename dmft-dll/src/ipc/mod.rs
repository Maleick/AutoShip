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
                tracing::info!(
                    account = %account_name,
                    "StartLogin received — running inline + delegating to FSM"
                );

                // Store credentials in the FSM for later phases (server/char select)
                // which run in the game loop after eqmain.dll unloads.
                crate::login::start_login(
                    account_name.to_string(),
                    password.to_string(),
                    server_name.to_string(),
                    character_name.to_string(),
                );

                // Run credential entry inline on the IPC thread, because
                // the game loop hook (ProcessGameEvents) doesn't fire during the
                // login screen — eqmain.dll has its own event loop.
                let eqmain_base = crate::login::eqmain::find_eqmain();
                if eqmain_base != 0 {
                    // MQ2 approach: find CEditWnd widgets, call SetWindowText
                    // through the vtable to set both UI display and internal state.
                    use dmft_common::offsets::eqmain as off;

                    let cxwnd_mgr = crate::login::eqmain::resolve_cxwnd_manager(eqmain_base);
                    if let Some(mgr) = cxwnd_mgr {
                        unsafe {
                            let array_ptr = *((mgr + off::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
                            let count = *((mgr + off::CXWNDMGR_WINDOWS_COUNT) as *const u32);

                            if array_ptr != 0 && count > 0 && count <= 500 {
                                let mut username_edit: usize = 0;
                                let mut password_edit: usize = 0;
                                let mut login_button: usize = 0;
                                let mut prev_wnd: usize = 0;
                                let mut prev_prev_wnd: usize = 0;

                                for i in 0..count as usize {
                                    let wnd_ptr = *((array_ptr + i * 8) as *const usize);
                                    if wnd_ptr == 0 { continue; }

                                    if let Some(text) = crate::login::widgets::read_cxstr_pub(
                                        wnd_ptr + off::CXWND_WINDOW_TEXT,
                                    ) {
                                        if text == "USERNAME" && prev_prev_wnd != 0 {
                                            username_edit = prev_prev_wnd;
                                        }
                                        if text == "PASSWORD" && prev_prev_wnd != 0 {
                                            password_edit = prev_prev_wnd;
                                        }
                                        if text == "Login" && login_button == 0 {
                                            login_button = wnd_ptr;
                                        }
                                    }
                                    prev_prev_wnd = prev_wnd;
                                    prev_wnd = wnd_ptr;
                                }

                                // Set username via SetWindowText vtable call
                                if username_edit != 0 {
                                    let ok = crate::login::widgets::set_edit_text_via_vtable(
                                        username_edit, &account_name,
                                    );
                                    tracing::info!(ok, "Inline: SetWindowText username");
                                }

                                // Set password via SetWindowText vtable call
                                if password_edit != 0 {
                                    let ok = crate::login::widgets::set_edit_text_via_vtable(
                                        password_edit, &password,
                                    );
                                    tracing::info!(ok, "Inline: SetWindowText password (redacted)");
                                } else {
                                    tracing::warn!("Inline: password CEditWnd not found");
                                }

                                // Wait for EQ to process the SetWindowText calls.
                                // The vtable call updates internal state but the UI
                                // event loop needs time to read the new values.
                                std::thread::sleep(std::time::Duration::from_secs(1));

                                // Click Login button
                                if login_button != 0 {
                                    crate::login::widgets::click_button_via_vtable(login_button);
                                    tracing::info!("Inline: Login button clicked via vtable");
                                } else {
                                    // Fallback: find by text
                                    let clicked =
                                        crate::login::widgets::click_button(eqmain_base, "Login")
                                        || crate::login::widgets::click_button(eqmain_base, "LOGIN");
                                    tracing::info!(clicked, "Inline: Login button click (fallback)");
                                }
                            }
                        }
                    } else {
                        tracing::warn!("Inline: CXWndManager not resolved");
                    }

                    // Spawn thread for phase 2 (server select)
                    let srv = server_name.clone();
                    let chr = character_name.clone();
                    std::thread::Builder::new()
                        .name("dmft-login-phase2".into())
                        .spawn(move || {
                            login_chain_phase2(srv, chr);
                        })
                        .ok();
                } else {
                    tracing::warn!("Inline: eqmain.dll not loaded — FSM will handle when game loop starts");
                }
            }
            true
        }
        _ => false,
    }
}

/// Phase 2+3 of the login chain: server select → character select → enter world.
/// Uses vtable WndNotification clicks — no foreground focus needed (scales to 36 clients).
/// Called inline from IPC thread since the game loop doesn't run during login.
fn login_chain_phase2(_server_name: String, _character_name: String) {
    use dmft_common::offsets::eqmain as off;

    // Phase 2: Wait for server select, then click PLAY EVERQUEST!
    tracing::info!("Login chain phase 2: waiting 12s for server select...");
    std::thread::sleep(std::time::Duration::from_secs(12));

    let eqmain_base = crate::login::eqmain::find_eqmain();
    if eqmain_base == 0 {
        tracing::error!("Login chain: eqmain.dll not found after login");
        return;
    }

    if let Some(play_btn) = find_button_by_text(eqmain_base, "PLAY EVERQUEST!") {
        tracing::info!(ptr = format!("{:#x}", play_btn), "Phase 2: Clicking PLAY EVERQUEST!");
        unsafe { crate::login::widgets::click_button_via_vtable(play_btn); }
        tracing::info!("Phase 2 complete: PLAY EVERQUEST clicked");
    } else {
        tracing::warn!("Phase 2: PLAY EVERQUEST button not found — trying Enter fallback");
        if let Some(eqm) = Some(eqmain_base).filter(|b| *b != 0) {
            crate::login::widgets::simulate_enter_key(eqm);
        }
    }

    // Phase 3: Wait for character select, then click Enter World
    tracing::info!("Login chain phase 3: waiting 15s for character select...");
    std::thread::sleep(std::time::Duration::from_secs(15));

    let eqmain_base3 = crate::login::eqmain::find_eqmain();
    if eqmain_base3 == 0 {
        // eqmain.dll unloaded means we're already in-world!
        tracing::info!("Phase 3: eqmain.dll unloaded — character is already in-world!");
        return;
    }

    // Scan for Enter World / Enter / Play button
    let enter_candidates = ["Enter World", "ENTER WORLD", "Enter", "Play"];
    let mut found = false;
    for candidate in &enter_candidates {
        if let Some(btn) = find_button_by_text(eqmain_base3, candidate) {
            tracing::info!(
                ptr = format!("{:#x}", btn),
                text = candidate,
                "Phase 3: Clicking enter world button"
            );
            unsafe { crate::login::widgets::click_button_via_vtable(btn); }
            tracing::info!("Phase 3 complete: Enter World clicked");
            found = true;
            break;
        }
    }

    if !found {
        tracing::warn!("Phase 3: Enter World button not found — trying Enter fallback");
        crate::login::widgets::simulate_enter_key(eqmain_base3);
    }
}

/// Find a button widget by its WindowText in the CXWndManager window list.
#[allow(dead_code)]
fn find_button_by_text(eqmain_base: u64, target_text: &str) -> Option<usize> {
    use dmft_common::offsets::eqmain as off;

    let cxwnd_mgr = crate::login::eqmain::resolve_cxwnd_manager(eqmain_base)?;

    unsafe {
        let array_ptr = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
        let count = *((cxwnd_mgr + off::CXWNDMGR_WINDOWS_COUNT) as *const u32);

        if array_ptr == 0 || count == 0 || count > 500 {
            return None;
        }

        for i in 0..count as usize {
            let wnd_ptr = *((array_ptr + i * 8) as *const usize);
            if wnd_ptr == 0 { continue; }

            if let Some(text) = crate::login::widgets::read_cxstr_pub(wnd_ptr + off::CXWND_WINDOW_TEXT) {
                if text == target_text {
                    return Some(wnd_ptr);
                }
            }
        }
    }

    None
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
