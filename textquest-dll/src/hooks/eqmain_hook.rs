//! eqmain.dll main-thread hook — intercepts `LoginController::GiveTime`.
//!
//! `GiveTime()` is called every frame while eqmain.dll is loaded (login screen,
//! server select). Hooking it gives us main-thread execution during the login
//! phase — exactly like MQ2's `OnPulse()` hook.
//!
//! This replaces the background-thread approach for login automation. All UI
//! operations (credential entry, button clicks, dialog handling, server join)
//! now execute on EQ's main thread, matching how MQ2 AutoLogin works.
//!
//! The IPC thread queues commands via atomics; this hook consumes them.

use super::hwbp::{self, HwbpSlot};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;

const EQMAIN_HOOK_SLOT: HwbpSlot = HwbpSlot::Dr1;

// ─── Command queue: IPC thread → main thread ───

/// Pending login action for the main thread to execute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum LoginAction {
    None = 0,
    /// Write credentials to UI fields and click Login.
    SubmitCredentials = 1,
    /// Join server via LoginServerAPI::JoinServer.
    JoinServer = 2,
}

/// Atomic flag for the pending login action.
static PENDING_ACTION: AtomicU8 = AtomicU8::new(LoginAction::None as u8);

/// Whether the hook is installed and active.
static HOOK_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Credentials to submit (set by IPC thread, consumed by main thread).
static PENDING_CREDENTIALS: Mutex<Option<PendingLogin>> = Mutex::new(None);

struct PendingLogin {
    account: String,
    password: zeroize::Zeroizing<String>,
    server_name: String,
    character_name: String,
}

// ─── Public API (called from IPC thread) ───

/// Queue credential submission for the next GiveTime tick.
/// Called from handle_immediate_command on the IPC thread.
pub fn queue_login(
    account: String,
    password: zeroize::Zeroizing<String>,
    server_name: String,
    character_name: String,
) {
    if let Ok(mut creds) = PENDING_CREDENTIALS.lock() {
        *creds = Some(PendingLogin {
            account,
            password,
            server_name,
            character_name,
        });
    }
    PENDING_ACTION.store(LoginAction::SubmitCredentials as u8, Ordering::Release);
    tracing::info!("Queued SubmitCredentials for main thread");
}

/// Queue server join for the next GiveTime tick.
pub fn queue_join_server() {
    PENDING_ACTION.store(LoginAction::JoinServer as u8, Ordering::Release);
    tracing::info!("Queued JoinServer for main thread");
}

/// Check if the eqmain hook is currently active.
pub fn is_active() -> bool {
    HOOK_ACTIVE.load(Ordering::Acquire)
}

// ─── Hook installation ───

/// Install the GiveTime HWBP hook. Called when eqmain.dll is detected.
pub fn install(eqmain_base: u64) -> Result<(), Box<dyn std::error::Error>> {
    use textquest_common::offsets::eqmain as off;

    let give_time_addr = off::rebase(off::LOGIN_CONTROLLER_GIVE_TIME, eqmain_base)
        .ok_or("Failed to rebase LoginController::GiveTime")?;

    hwbp::register(EQMAIN_HOOK_SLOT, give_time_addr, give_time_callback)?;
    HOOK_ACTIVE.store(true, Ordering::Release);

    tracing::info!(
        addr = format!("{:#x}", give_time_addr),
        "eqmain GiveTime HWBP hook installed (DR1)"
    );
    Ok(())
}

/// Remove the GiveTime hook. Called when eqmain.dll unloads.
pub fn remove() {
    if hwbp::is_active(EQMAIN_HOOK_SLOT) {
        if let Err(e) = hwbp::unregister(EQMAIN_HOOK_SLOT) {
            tracing::warn!("Failed to remove eqmain GiveTime HWBP: {}", e);
        }
    }
    HOOK_ACTIVE.store(false, Ordering::Release);
    tracing::info!("eqmain GiveTime hook removed");
}

// ─── Hook callback (runs on EQ's main thread) ───

/// HWBP callback for LoginController::GiveTime.
/// Runs on EQ's main thread every frame during eqmain.
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
fn give_time_callback(_exception_info: *mut ()) -> bool {
    on_eqmain_tick();
    true
}

/// Main-thread tick during eqmain. Processes queued login actions,
/// handles dialogs, and drives the login FSM's eqmain-phase states.
fn on_eqmain_tick() {
    let eqmain_base = crate::login::eqmain::find_eqmain();
    if eqmain_base == 0 {
        // eqmain unloaded — remove this hook, game loop hook takes over.
        remove();
        return;
    }

    // Check for pending actions from IPC thread.
    let action = PENDING_ACTION.load(Ordering::Acquire);
    if action != LoginAction::None as u8 {
        PENDING_ACTION.store(LoginAction::None as u8, Ordering::Release);

        if action == LoginAction::SubmitCredentials as u8 {
            execute_submit_credentials(eqmain_base);
        } else if action == LoginAction::JoinServer as u8 {
            execute_join_server(eqmain_base);
        }
    }

    // Dismiss pre-login prompts (EULA, splash, news) every tick.
    crate::login::widgets::dismiss_splash(eqmain_base);

    // Handle dialogs (already logged in, errors) — FSM drives this.
    // The FSM's tick is called from the game loop for post-eqmain states,
    // but during eqmain we need to drive dialog handling here.
    handle_eqmain_dialogs(eqmain_base);

    // Detect screen transitions and notify the FSM.
    detect_screen_state(eqmain_base);
}

/// Execute credential submission on the main thread.
fn execute_submit_credentials(eqmain_base: u64) {
    let creds = {
        let mut lock = match PENDING_CREDENTIALS.lock() {
            Ok(l) => l,
            Err(_) => return,
        };
        lock.take()
    };

    let Some(creds) = creds else {
        tracing::warn!("SubmitCredentials action but no credentials queued");
        return;
    };

    tracing::info!(
        account = %creds.account,
        "Main thread: writing credentials + clicking Login"
    );

    // Write credentials to CXStr (same as before, but now on main thread).
    let wrote = crate::login::widgets::type_credentials_to_window(
        eqmain_base,
        &creds.account,
        &creds.password,
    );
    tracing::info!(wrote, "Main thread: credentials written to UI fields");

    // Store credentials in FSM for character select phase.
    crate::login::start_login(
        creds.account,
        (*creds.password).clone(),
        creds.server_name,
        creds.character_name,
    );
}

/// Execute JoinServer on the main thread via LoginServerAPI.
fn execute_join_server(eqmain_base: u64) {
    #[cfg(windows)]
    {
        use textquest_common::offsets::eqmain as off;

        // Resolve LoginServerAPI*
        let Some(api_ptr) = crate::login::eqmain::resolve_login_server_api(eqmain_base) else {
            tracing::warn!("JoinServer: LoginServerAPI not resolved — falling back to button click");
            fallback_click_play(eqmain_base);
            return;
        };

        // Read pLoginServerAPI (it's a pointer to the API object)
        let api = unsafe { *(api_ptr as *const usize) };
        if api == 0 {
            tracing::warn!("JoinServer: LoginServerAPI is null — falling back to button click");
            fallback_click_play(eqmain_base);
            return;
        }

        // Resolve JoinServer function address
        let Some(join_fn_addr) = off::rebase(off::JOIN_SERVER, eqmain_base) else {
            tracing::warn!("JoinServer: failed to rebase JOIN_SERVER offset");
            fallback_click_play(eqmain_base);
            return;
        };

        // TODO: Look up server ID from LoginClient::ServerList.
        // For now, use server ID 0 which selects the last/default server.
        // MQ2 iterates g_pLoginClient->ServerList to find by name.
        // We'll implement proper server lookup in a follow-up.
        let server_id: i32 = 0;

        tracing::info!(
            api = format!("{:#x}", api),
            join_fn = format!("{:#x}", join_fn_addr),
            server_id,
            "Main thread: calling LoginServerAPI::JoinServer"
        );

        // SAFETY: api is the LoginServerAPI* from eqmain globals.
        // JoinServer signature: unsigned int JoinServer(int serverID, void* userdata, int timeout)
        // It's a member function: this=RCX, serverID=RDX, userdata=R8, timeout=R9
        type JoinServerFn = unsafe extern "C" fn(this: usize, server_id: i32, userdata: usize, timeout: i32) -> u32;
        let func: JoinServerFn = unsafe { std::mem::transmute(join_fn_addr) };
        let result = unsafe { func(api, server_id, 0, 10) };

        tracing::info!(result, "JoinServer returned");
    }

    #[cfg(not(windows))]
    {
        let _ = eqmain_base;
    }
}

/// Fallback: click PLAY EVERQUEST button if JoinServer can't be called.
fn fallback_click_play(eqmain_base: u64) {
    if let Some(cxwnd_mgr) = crate::login::eqmain::resolve_cxwnd_manager(eqmain_base) {
        if let Some(play_btn) =
            unsafe { crate::eq::widgets::find_window_by_name(cxwnd_mgr, "PLAY EVERQUEST!") }
        {
            // We're on the main thread now, so direct vtable click is safe.
            crate::login::widgets::click_button_for_phase(play_btn, true);
            tracing::info!("Fallback: clicked PLAY EVERQUEST via vtable (main thread)");
        }
    }
}

/// Handle dialogs during eqmain phase on the main thread.
fn handle_eqmain_dialogs(eqmain_base: u64) {
    use crate::login::widgets;

    // YesNo dialog — "already logged in, kick?" → click Yes
    if let Some(dlg) = widgets::find_visible_sidl_window(eqmain_base, widgets::SIDL_YES_NO_DIALOG)
    {
        if widgets::click_yesno_yes(dlg) {
            tracing::info!("Main thread: clicked Yes on YesNo dialog");
        }
    }

    // OK dialog — dismiss errors
    if let Some(dlg) = widgets::find_visible_sidl_window(eqmain_base, widgets::SIDL_OK_DIALOG) {
        widgets::click_ok_dialog(dlg);
        tracing::info!("Main thread: dismissed OK dialog");
    }
}

/// Detect which login screen is active and trigger FSM transitions / actions.
fn detect_screen_state(eqmain_base: u64) {
    use crate::login::widgets;

    // If server select is visible and FSM is waiting, queue JoinServer.
    if widgets::is_sidl_window_visible(eqmain_base, widgets::SIDL_SERVER_SELECT) {
        // Check if we already tried to join
        if PENDING_ACTION.load(Ordering::Acquire) == LoginAction::None as u8 {
            // Only auto-join if FSM is in the right state
            // (the FSM will also handle this, but we provide the main-thread path)
            static JOINED: AtomicBool = AtomicBool::new(false);
            if !JOINED.swap(true, Ordering::AcqRel) {
                tracing::info!("Server select detected — queuing JoinServer");
                queue_join_server();
            }
        }
    }
}
