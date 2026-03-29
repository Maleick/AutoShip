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
                // Clone password into Zeroizing wrapper so the local copy is wiped
                // from memory when this scope exits — prevents plaintext from
                // lingering on the IPC thread's stack after credential entry.
                let password = zeroize::Zeroizing::new(password.clone());

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
                    // Direct InputText property write (MQ2's actual approach):
                    // Find CEditWnd widgets, write to CXStr at +0x278 (InputText),
                    // clone CStrRep for empty password field, click Login.
                    // type_credentials_to_window handles all of this including
                    // hex dumps for debugging and readback verification.
                    let wrote = crate::login::widgets::type_credentials_to_window(
                        eqmain_base, account_name, &password,
                    );
                    tracing::info!(wrote, "Inline: type_credentials_to_window");

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
    // Phase 2: Poll for PLAY EVERQUEST button (replaces hard 8s sleep).
    // Polls every 500ms for up to 30s.
    tracing::info!("Login chain phase 2: polling for server select...");
    let mut play_btn_found = false;
    for attempt in 0..60 {
        std::thread::sleep(std::time::Duration::from_millis(500));

        let eqmain_base = crate::login::eqmain::find_eqmain();
        if eqmain_base == 0 {
            if attempt > 10 {
                tracing::warn!("Phase 2: eqmain.dll not found after {} attempts", attempt);
            }
            continue;
        }

        if let Some(play_btn) = find_button_by_text(eqmain_base, "PLAY EVERQUEST!") {
            tracing::info!(
                ptr = format!("{:#x}", play_btn),
                attempt,
                "Phase 2: Found PLAY EVERQUEST! button"
            );
            unsafe { crate::eq::widgets::click_button_via_vtable(play_btn); }
            tracing::info!("Phase 2 complete: PLAY EVERQUEST clicked");
            play_btn_found = true;
            break;
        }
    }

    if !play_btn_found {
        tracing::warn!("Phase 2: PLAY EVERQUEST button not found after 30s — trying Enter fallback");
        let eqmain_base = crate::login::eqmain::find_eqmain();
        if eqmain_base != 0 {
            crate::login::widgets::simulate_enter_key(eqmain_base);
        }
    }

    // Phase 3: Poll for eqmain.dll unload (character select ready).
    // eqmain.dll unloads when transitioning from server select to character select.
    // Polls every 500ms for up to 60s.
    tracing::info!("Login chain phase 3: polling for character select...");
    let mut char_select_ready = false;
    for attempt in 0..120 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let eqmain_check = crate::login::eqmain::find_eqmain();
        if eqmain_check == 0 {
            tracing::info!(attempt, "Phase 3: eqmain.dll unloaded — at character select");
            char_select_ready = true;
            // Wait a bit more for UI to settle
            std::thread::sleep(std::time::Duration::from_secs(2));
            break;
        }
    }

    if !char_select_ready {
        tracing::warn!("Phase 3: Timed out waiting for character select after 60s");
    }

    let eqmain_base3 = crate::login::eqmain::find_eqmain();
    if eqmain_base3 == 0 {
        // eqmain.dll unloaded — we're at character select (eqgame.exe).
        phase3_enter_world();
        return;
    }

    // eqmain.dll still loaded — try button click as fallback (shouldn't happen normally)
    let enter_candidates = ["Enter World", "ENTER WORLD", "Enter", "Play"];
    let mut found = false;
    for candidate in &enter_candidates {
        if let Some(btn) = find_button_by_text(eqmain_base3, candidate) {
            tracing::info!(
                ptr = format!("{:#x}", btn),
                text = candidate,
                "Phase 3: Clicking enter world button (eqmain still loaded)"
            );
            unsafe { crate::eq::widgets::click_button_via_vtable(btn); }
            found = true;
            break;
        }
    }

    if !found {
        tracing::warn!("Phase 3: Enter World not found — trying /enterworld command");
        crate::hooks::game_loop::queue_slash_command("/enterworld".to_string());
    }
}

/// Phase 3 implementation: Find CCharacterListWnd by SidlText and call EnterWorld().
///
/// Uses eqgame.exe CXWndManager offsets (NOT eqmain.dll — they differ!).
/// MQ2 approach: walk window array, match SidlText == "CharacterListWnd",
/// then call CCharacterListWnd::EnterWorld() as a direct function call.
fn phase3_enter_world() {
    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    if eq_base == 0 {
        tracing::error!("Phase 3: EQ base not resolved");
        return;
    }

    // Resolve pinstCXWndManager in eqgame.exe
    let Some(mgr_ptr_addr) = dmft_common::offsets::rebase(
        dmft_common::offsets::PINST_CXWND_MANAGER, eq_base,
    ) else {
        tracing::warn!("Phase 3: Could not rebase pinstCXWndManager");
        return;
    };

    // Use eqgame.exe offsets (NOT eqmain.dll — different CXWndManager layout!)
    use dmft_common::offsets::eqgame as eqg;

    unsafe {
        let mgr = *(mgr_ptr_addr as *const usize);
        if mgr == 0 {
            tracing::warn!("Phase 3: CXWndManager is null");
            return;
        }

        // Read window array using eqgame.exe offsets (+0x008/+0x010, not +0x010/+0x018)
        let array_ptr = *((mgr + eqg::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
        let count = *((mgr + eqg::CXWNDMGR_WINDOWS_COUNT) as *const u32);

        tracing::info!(
            mgr = format!("{:#x}", mgr),
            array = format!("{:#x}", array_ptr),
            count,
            "Phase 3: CXWndManager resolved (eqgame.exe offsets)"
        );

        if array_ptr == 0 || count == 0 || count > 2000 {
            tracing::warn!("Phase 3: Invalid CXWndManager window array");
            return;
        }

        // Walk the window array — find CCharacterListWnd by SidlText at +0x270
        let mut char_list_wnd: usize = 0;
        let mut logged = 0u32;

        for i in 0..count as usize {
            let wnd_ptr = *((array_ptr + i * 8) as *const usize);
            if wnd_ptr == 0 { continue; }

            // Read SidlText (CSidlScreenWnd::SidlText at +0x270)
            if let Some(sidl_text) = crate::eq::widgets::read_cxstr(
                wnd_ptr + eqg::CSIDL_SCREEN_WND_SIDL_TEXT,
            ) {
                if logged < 30 && !sidl_text.is_empty() {
                    tracing::info!(
                        idx = i,
                        ptr = format!("{:#x}", wnd_ptr),
                        sidl = %sidl_text,
                        "Phase 3 eqgame window"
                    );
                    logged += 1;
                }

                if sidl_text == "CharacterListWnd" {
                    char_list_wnd = wnd_ptr;
                    tracing::info!(
                        ptr = format!("{:#x}", wnd_ptr),
                        "Phase 3: Found CCharacterListWnd!"
                    );
                    break;
                }
            }

            // Also check WindowText for calibration logging
            if logged < 30
                && let Some(wnd_text) = crate::eq::widgets::read_cxstr(
                    wnd_ptr + dmft_common::offsets::eqmain::CXWND_WINDOW_TEXT,
                )
                    && !wnd_text.is_empty() {
                        tracing::info!(
                            idx = i,
                            ptr = format!("{:#x}", wnd_ptr),
                            text = %wnd_text,
                            "Phase 3 eqgame window (WindowText)"
                        );
                    }
        }

        if char_list_wnd == 0 {
            tracing::warn!(count, "Phase 3: CCharacterListWnd not found in {} windows", count);
            // Fallback: try /enterworld slash command
            crate::hooks::game_loop::queue_slash_command("/enterworld".to_string());
            return;
        }

        // Call CCharacterListWnd::EnterWorld() directly — the MQ2 approach.
        // Signature: void EnterWorld() — __thiscall, just `this` pointer (RCX on x64)
        let Some(enter_world_addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::ENTER_WORLD, eq_base,
        ) else {
            tracing::warn!("Phase 3: Could not rebase ENTER_WORLD");
            return;
        };

        tracing::info!(
            char_list_wnd = format!("{:#x}", char_list_wnd),
            enter_world = format!("{:#x}", enter_world_addr),
            "Phase 3: Queuing EnterWorld() to game loop thread"
        );

        // Queue to game loop thread — UI/game-state mutation must happen on main thread.
        // The game loop hook picks this up on the next tick via PENDING_ENTER_WORLD.
        crate::hooks::game_loop::queue_enter_world(char_list_wnd, enter_world_addr);

        tracing::info!("Phase 3: EnterWorld() queued — will execute on next game tick");
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

            if let Some(text) = crate::eq::widgets::read_cxstr(wnd_ptr + off::CXWND_WINDOW_TEXT)
                && text == target_text {
                    return Some(wnd_ptr);
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

                if let Some(pending) = PENDING_COMMANDS.get()
                    && let Ok(mut queue) = pending.lock() {
                        queue.push(cmd);
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
