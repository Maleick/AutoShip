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

use textquest_common::ipc::{Command, Response, SessionToken};
use textquest_common::types::{ClientId, SharedStateFrame};

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
    let session_id = textquest_common::ipc::session_id_from_token(&token);
    let writer = match SharedStateWriter::new(client_id, session_id) {
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
        .name(format!("textquest-ipc-{client_id}"))
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
pub fn publish_state(frame: &SharedStateFrame) {
    let Some(writer_lock) = SHARED_WRITER.get() else {
        return;
    };
    let Ok(mut writer) = writer_lock.lock() else {
        tracing::error!("IPC shared memory writer mutex poisoned");
        return;
    };
    if let Err(e) = writer.write(frame) {
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
    const MAX_LOGIN_FIELD_CHARS: usize = 128;

    match cmd {
        Command::CalibrateLogin => {
            let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
            let eqmain_base = crate::login::eqmain::find_eqmain();
            tracing::info!(
                eq_base = format!("{:#x}", eq_base),
                eqmain_base = format!("{:#x}", eqmain_base),
                "CalibrateLogin: running calibration dump"
            );
            if eqmain_base == 0 {
                tracing::warn!("CalibrateLogin: eqmain.dll not loaded yet; skipping dump");
            } else {
                crate::login::widgets::calibrate_login_dump(eqmain_base);
            }
            true
        }
        Command::StartLogin {
            account_name,
            password,
            server_name,
            character_name,
        } => {
            let account_name: String = account_name.chars().take(MAX_LOGIN_FIELD_CHARS).collect();
            let password: String = password.chars().take(MAX_LOGIN_FIELD_CHARS).collect();

            // Clone password into Zeroizing wrapper so the local copy is wiped
            // from memory when this scope exits — prevents plaintext from
            // lingering on the IPC thread's stack after credential entry.
            let mut password = zeroize::Zeroizing::new(password);

            tracing::info!(
                account = %account_name,
                server = %server_name,
                character = %character_name,
                "StartLogin received — delegating to FSM (password redacted)"
            );

            // Two-phase credential entry:
            // 1. Write username/password to CXStr memory (EQ's internal state)
            // 2. Type password via WM_CHAR + Enter (actual form submission)
            // CXStr writes alone don't trigger EQ's login handler — WM_CHAR is required.
            let eqmain_base = crate::login::eqmain::find_eqmain();
            if eqmain_base != 0 {
                crate::login::widgets::type_credentials_to_window(
                    eqmain_base,
                    &account_name,
                    &password,
                );
                crate::login::widgets::type_password_wm_char(eqmain_base, &password);
            }

            // Store credentials in the FSM for character select phase.
            crate::login::start_login(
                account_name,
                std::mem::take(&mut *password),
                server_name.to_string(),
                character_name.to_string(),
            );

            // Spawn background thread for Phase 2+3 (server select → char select).
            // Polls every 500ms. Uses Enter key to dismiss dialogs.
            // Game loop tick handles Phase 4 (character select → enter world).
            std::thread::Builder::new()
                .name("textquest-login-phase2".into())
                .spawn(login_chain_phase2)
                .ok();

            true
        }
        _ => false,
    }
}

/// Find a button by `WindowText` in eqmain's `CXWndManager`.
fn find_button_by_text(eqmain_base: u64, target_text: &str) -> Option<usize> {
    let cxwnd_mgr = crate::login::eqmain::resolve_cxwnd_manager(eqmain_base)?;
    unsafe { crate::eq::widgets::find_window_by_name(cxwnd_mgr, target_text) }
}

/// Phase 2+3: server select → character select.
/// Detects server select via SIDL window name (not "PLAY EVERQUEST!" text which
/// exists on the login screen too). Uses direct vtable click — `queue_button_click()`
/// won't work because ProcessGameEvents is NOT hooked during eqmain.
/// Polls for eqmain.dll unload. Game loop tick handles character select.
fn login_chain_phase2() {
    use crate::login::widgets;

    tracing::info!("Phase 2: Waiting for server select screen...");

    let mut found = false;
    for attempt in 0..60 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let eqmain_base = crate::login::eqmain::find_eqmain();
        if eqmain_base == 0 {
            tracing::info!(attempt, "Phase 2: eqmain.dll gone — already at char select");
            return;
        }

        // Detect server select by SIDL name — not "PLAY EVERQUEST!" text which
        // also exists on the login screen as a branding label.
        if widgets::is_sidl_window_visible(eqmain_base, widgets::SIDL_SERVER_SELECT) {
            tracing::info!(attempt, "Phase 2: Server select screen detected (SIDL)");

            // Direct vtable click (eqmain context — game loop hook not active).
            if let Some(play_btn) = find_button_by_text(eqmain_base, "PLAY EVERQUEST!") {
                tracing::info!(ptr = format!("{:#x}", play_btn), "Phase 2: Clicking PLAY EVERQUEST");
                std::thread::sleep(std::time::Duration::from_millis(150));
                widgets::click_button_for_phase(play_btn, true);
                std::thread::sleep(std::time::Duration::from_millis(200));
                widgets::simulate_enter_key(eqmain_base);
            } else {
                tracing::info!("Phase 2: PLAY EVERQUEST button not found, sending Enter");
                widgets::simulate_enter_key(eqmain_base);
            }
            found = true;
            break;
        }

        // Dismiss "already logged in" dialog during authentication
        if attempt % 2 == 0 {
            if let Some(dlg) = widgets::find_visible_sidl_window(
                eqmain_base,
                widgets::SIDL_YES_NO_DIALOG,
            ) {
                if widgets::click_yesno_yes(dlg) {
                    tracing::info!(attempt, "Phase 2: Clicked Yes on dialog during auth");
                }
            }
        }

        // Screen-state trace every 5s for diagnostics
        if attempt % 10 == 0 {
            log_screen_state(eqmain_base, "Phase 2", attempt);
        }

        // Press Enter every 5s to dismiss blocking dialogs (EULA, notices)
        if attempt % 10 == 5 {
            widgets::simulate_enter_key(eqmain_base);
        }
    }
    if !found {
        tracing::warn!("Phase 2: Server select not detected after 30s");
    }

    // Phase 3: Poll for eqmain.dll unload (character select).
    // Also handle "already logged in" Yes/No dialog during this phase.
    tracing::info!("Phase 3: Polling for character select...");
    for attempt in 0..120 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let eqmain_base = crate::login::eqmain::find_eqmain();
        if eqmain_base == 0 {
            tracing::info!(attempt, "Phase 3: eqmain.dll unloaded — at character select");
            return;
        }

        if attempt % 2 == 0 {
            if let Some(dlg) = widgets::find_visible_sidl_window(
                eqmain_base,
                widgets::SIDL_YES_NO_DIALOG,
            ) {
                if widgets::click_yesno_yes(dlg) {
                    tracing::info!(attempt, "Phase 3: Clicked Yes on 'already logged in' dialog");
                }
            }
        }

        // Press Enter every 3s to dismiss other dialogs
        if attempt % 6 == 3 {
            widgets::simulate_enter_key(eqmain_base);
        }

        // Screen-state trace every 10s for diagnostics
        if attempt % 20 == 0 {
            log_screen_state(eqmain_base, "Phase 3", attempt);
        }
    }
    tracing::warn!("Phase 3: Timed out after 60s");
}

/// Log which SIDL windows are visible — shared by Phase 2 and Phase 3 polling.
fn log_screen_state(eqmain_base: u64, phase: &str, attempt: u32) {
    use crate::login::widgets;

    let connect = widgets::is_sidl_window_visible(eqmain_base, widgets::SIDL_CONNECT);
    let server = widgets::is_sidl_window_visible(eqmain_base, widgets::SIDL_SERVER_SELECT);
    let yesno = widgets::is_sidl_window_visible(eqmain_base, widgets::SIDL_YES_NO_DIALOG);
    let ok_dlg = widgets::is_sidl_window_visible(eqmain_base, widgets::SIDL_OK_DIALOG);
    tracing::info!(
        attempt,
        connect,
        server,
        yesno,
        ok_dlg,
        eqmain = format!("{:#x}", eqmain_base),
        "{phase}: screen state"
    );
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
                    // Immediate commands get an Ack response.
                    let _ = listener.respond(&Response::CommandResult {
                        success: true,
                        message: "handled".into(),
                    });
                    listener.disconnect();
                    continue;
                }

                // Respond to Ping inline — no need to queue.
                if matches!(&cmd, Command::Ping) {
                    let _ = listener.respond(&Response::Pong {
                        client_id,
                        timestamp_ms: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_or(0, |d| d.as_millis() as u64),
                    });
                    listener.disconnect();
                    continue;
                }

                // Respond to PollPackets inline — drain accumulated packet events.
                if matches!(&cmd, Command::PollPackets) {
                    let pending = drain_responses();
                    let events: Vec<textquest_common::ipc::PacketEventInfo> = pending
                        .into_iter()
                        .filter_map(|r| match r {
                            Response::PacketEvent {
                                client_id,
                                opcode,
                                direction,
                                timestamp_ms,
                                payload_size,
                            } => Some(textquest_common::ipc::PacketEventInfo {
                                client_id,
                                opcode,
                                direction,
                                timestamp_ms,
                                payload_size,
                            }),
                            _ => None,
                        })
                        .collect();
                    let _ = listener.respond(&Response::PacketBatch { events });
                    listener.disconnect();
                    continue;
                }

                // Queue for game loop processing (bounded to prevent OOM
                // if the game loop stalls during loading screens).
                const MAX_PENDING: usize = 256;
                let queued = if let Some(pending) = PENDING_COMMANDS.get()
                    && let Ok(mut queue) = pending.lock()
                {
                    if queue.len() < MAX_PENDING {
                        queue.push(cmd);
                        true
                    } else {
                        tracing::warn!(
                            client_id,
                            "Command queue full ({MAX_PENDING}), dropping command"
                        );
                        false
                    }
                } else {
                    false
                };

                // Send Ack so the orchestrator isn't left waiting.
                let _ = listener.respond(&Response::CommandResult {
                    success: queued,
                    message: if queued { "queued" } else { "queue full" }.into(),
                });

                // Reset pipe for next connection. The orchestrator uses
                // fire-and-forget (connect → token → command → close), so the
                // server must DisconnectNamedPipe after each command to accept
                // the next client via ConnectNamedPipe.
                listener.disconnect();
            }
            Err(e) => {
                if IPC_RUNNING.load(Ordering::SeqCst) {
                    consecutive_errors += 1;
                    if consecutive_errors <= 3 {
                        tracing::warn!(client_id, error = %e, "Command listener error, resetting");
                    } else if consecutive_errors == 4 {
                        tracing::warn!(
                            client_id,
                            consecutive_errors,
                            "Suppressing repeated pipe errors"
                        );
                    }
                    // Connection dropped — disconnect will make next receive() wait
                    // for a new connection.
                    listener.disconnect();
                    let backoff_ms = std::cmp::min(10 * (1u64 << consecutive_errors.min(9)), 5000);
                    std::thread::sleep(std::time::Duration::from_millis(backoff_ms));
                }
            }
        }
    }

    tracing::info!(client_id, "IPC listener thread exiting");
}
