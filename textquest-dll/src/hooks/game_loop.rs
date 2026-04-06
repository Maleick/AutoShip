//! Game loop hook -- intercepts `CEverQuest::MainLoop`.
//!
//! Uses hardware breakpoint hooking (DR0) instead of detour/trampoline.
//! The VEH handler fires when EQ hits the breakpoint, runs our logic, then
//! resumes the original function with RF set so the BP doesn't re-trigger.

use super::hwbp::{self, HwbpSlot};

const GAME_LOOP_SLOT: HwbpSlot = HwbpSlot::Dr0;

/// HWBP callback for the game loop hook.
///
/// Lives in `.tq` section so it remains executable when stealth::sleep()
/// encrypts `.text` between frames.
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
fn game_loop_callback(_exception_info: *mut ()) -> bool {
    // Wake: decrypt .text + set RX (no-op if stealth disabled).
    crate::stealth::wake();

    on_game_tick();

    // Sleep: set RW + encrypt .text (no-op if stealth disabled).
    crate::stealth::sleep();
    true
}

pub fn install(main_loop_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
    hwbp::register(GAME_LOOP_SLOT, main_loop_addr, game_loop_callback)?;
    tracing::info!(
        addr = format!("{:#x}", main_loop_addr),
        "Game loop HWBP hook installed (DR0)"
    );
    Ok(())
}

pub fn remove() {
    if hwbp::is_active(GAME_LOOP_SLOT) {
        if let Err(e) = hwbp::unregister(GAME_LOOP_SLOT) {
            tracing::warn!("Failed to remove game loop HWBP: {}", e);
        }
    }
    tracing::info!("Game loop hook removed");
}

/// Track whether this window is in the foreground for render skipping.
/// When false, we can tell EQ to skip 3D rendering (near-zero GPU for
/// background clients). Game logic still runs at full speed.
static WINDOW_IS_FOREGROUND: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

/// Track tick count for throttling background checks.
static TICK_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Pending login button click — set by IPC thread, executed on game loop thread.
/// Contains the `CXWnd`* address of the button to click, or 0 if none pending.
static PENDING_BUTTON_CLICK: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Pending Enter World sequence — set by IPC thread, executed on game loop thread.
/// Stage 0 = idle, 1 = `SelectCharacter` pending, 2 = waiting, 3 = `EnterWorld` pending.
static PENDING_ENTER_WORLD_WND: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
/// The rebased `EnterWorld` function address.
static PENDING_ENTER_WORLD_FN: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
/// The rebased `SelectCharacter` function address.
static PENDING_SELECT_CHAR_FN: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
/// Enter World sequence stage (0=idle, 1=select, 2=wait, 3=enter).
static ENTER_WORLD_STAGE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// Tick at which to advance from stage 2→3 (wait before `EnterWorld`).
static ENTER_WORLD_WAIT_UNTIL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Retry counter for stage 3 rescan (abort after 150 ticks / ~5 seconds).
static ENTER_WORLD_RETRIES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// Character name to select (set by IPC thread, read by game loop).
static PENDING_CHAR_NAME: std::sync::OnceLock<std::sync::Mutex<String>> =
    std::sync::OnceLock::new();

/// Cached nearby spawns used by the stick-to-target engine.
///
/// Updated every 30 ticks in `read_and_publish_state`; consumed every tick
/// by `crate::nav::tick()` for target resolution (`hold` / `id` / `always`).
static CACHED_NEARBY_FOR_STICK: std::sync::Mutex<Vec<textquest_common::types::SpawnData>> =
    std::sync::Mutex::new(Vec::new());

/// Set a button widget address to be clicked on the next game loop tick.
/// Called from the IPC thread after writing credentials.
pub fn queue_button_click(button_wnd: usize) {
    PENDING_BUTTON_CLICK.store(button_wnd, std::sync::atomic::Ordering::Release);
}

/// Queue a `SelectCharacter` → EnterWorld() sequence on the game loop thread.
/// Called from the IPC thread during Phase 3 of login chain.
/// The game loop will: (1) find character index by name, (2) call SelectCharacter(index),
/// (3) wait ~90 ticks (~3s), (4) call EnterWorld(). All calls happen on the game loop thread.
pub fn queue_enter_world(char_list_wnd: usize, enter_world_fn: usize, character_name: String) {
    // Also resolve SelectCharacter address
    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    let select_fn =
        textquest_common::offsets::rebase(textquest_common::offsets::SELECT_CHARACTER, eq_base)
            .unwrap_or(0);

    // Store the character name for the game loop to look up.
    let name_lock = PENDING_CHAR_NAME.get_or_init(|| std::sync::Mutex::new(String::new()));
    if let Ok(mut name) = name_lock.lock() {
        *name = character_name;
    }

    // Store function addresses and window handle first (Relaxed is sufficient),
    // then store the stage flag last with Release ordering as the "commit" signal.
    // The reader's Acquire load of ENTER_WORLD_STAGE establishes happens-before
    // for all prior stores, guaranteeing the addresses are visible.
    PENDING_SELECT_CHAR_FN.store(select_fn, std::sync::atomic::Ordering::Relaxed);
    PENDING_ENTER_WORLD_FN.store(enter_world_fn, std::sync::atomic::Ordering::Relaxed);
    PENDING_ENTER_WORLD_WND.store(char_list_wnd, std::sync::atomic::Ordering::Relaxed);
    ENTER_WORLD_STAGE.store(1, std::sync::atomic::Ordering::Release);
}

/// Re-scan `CXWndManager` for `CCharacterListWnd` by `SidlText`.
/// Used in Stage 3 to validate the pointer is still valid before calling `EnterWorld`,
/// and by the login FSM to find the window for initial character selection.
#[cfg(windows)]
pub fn rescan_char_list_wnd() -> Option<usize> {
    use textquest_common::offsets::eqgame as eqg;

    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    let mgr_ptr_addr =
        textquest_common::offsets::rebase(textquest_common::offsets::PINST_CXWND_MANAGER, eq_base)?;

    // SAFETY: mgr_ptr_addr was rebased from PINST_CXWND_MANAGER, a known-valid
    // global pointer in eqgame.exe. Each dereference follows EQ's CXWndManager
    // layout (windows array pointer + count). Null checks and count cap (2000)
    // guard against corrupted data. read_cxstr performs its own pointer validation.
    // Must be called from the game loop thread where CXWndManager is stable.
    unsafe {
        let mgr = *(mgr_ptr_addr as *const usize);
        if mgr == 0 {
            return None;
        }

        let array_ptr = *((mgr + eqg::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
        let count = *((mgr + eqg::CXWNDMGR_WINDOWS_COUNT) as *const u32);
        if array_ptr == 0 || count == 0 || count > 2000 {
            return None;
        }

        for i in 0..count as usize {
            let wnd_ptr = *((array_ptr + i * 8) as *const usize);
            if wnd_ptr == 0 {
                continue;
            }

            if let Some(sidl_text) =
                crate::eq::widgets::read_cxstr(wnd_ptr + eqg::CSIDL_SCREEN_WND_SIDL_TEXT)
            {
                if sidl_text == "CharacterListWnd" {
                    return Some(wnd_ptr);
                }
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub fn rescan_char_list_wnd() -> Option<usize> {
    None
}

/// Find the index of a character by name in the `Character_List` `CListWnd`.
///
/// Walks the `CCharacterListWnd`'s child windows to find "`Character_List`" (a `CListWnd`),
/// then reads each row's column 2 (character name) for a case-insensitive match.
/// Returns the matched index, or 0 as fallback if the name is empty or not found.
#[cfg(windows)]
fn find_character_index(char_list_wnd: usize, character_name: &str) -> i32 {
    const MAX_CHARACTER_LIST_SCAN_ROWS: usize = 64;

    if character_name.is_empty() {
        tracing::info!("Character name empty — defaulting to index 0");
        return 0;
    }

    unsafe {
        // Find the "Character_List" child (CListWnd) inside CCharacterListWnd
        let Some(list_wnd) =
            crate::eq::widgets::find_child_by_sidl_text(char_list_wnd, "Character_List")
        else {
            tracing::warn!("Character_List child not found — defaulting to index 0");
            return 0;
        };

        let row_count = crate::eq::widgets::list_row_count(list_wnd);
        let bounded_row_count = row_count.min(MAX_CHARACTER_LIST_SCAN_ROWS);
        if row_count > bounded_row_count {
            tracing::warn!(
                row_count,
                bounded_row_count,
                "Character_List row count exceeds scan cap; limiting traversal"
            );
        }
        tracing::info!(
            list_wnd = format!("{:#x}", list_wnd),
            row_count,
            bounded_row_count,
            target = character_name,
            "Searching Character_List for character"
        );

        // Column 2 is the character name (MQ2 convention)
        for i in 0..bounded_row_count {
            if let Some(name) = crate::eq::widgets::read_list_item_text(list_wnd, i, 2) {
                tracing::info!(row = i, name = %name, "Character_List row");
                if name.eq_ignore_ascii_case(character_name) {
                    tracing::info!(index = i, name = %name, "Found matching character!");
                    return i as i32;
                }
            }
        }

        tracing::warn!(
            target = character_name,
            row_count,
            "Character not found in list — defaulting to index 0"
        );
    }
    0
}

#[cfg(not(windows))]
fn find_character_index(_char_list_wnd: usize, _character_name: &str) -> i32 {
    0
}

// ─── Command Jitter Queue ───
// Commands are not executed immediately — they sit in a pending queue
// with a random delay of 1-10 ticks to avoid frame-perfect timing patterns.

use std::sync::Mutex;

struct PendingCommand {
    command: textquest_common::ipc::Command,
    execute_at_tick: u64,
}

static PENDING_COMMANDS: Mutex<Vec<PendingCommand>> = Mutex::new(Vec::new());
static JITTER_RNG: Mutex<Option<textquest_common::nav::Xorshift32>> = Mutex::new(None);

/// Initialize the jitter RNG with a seed derived from system time.
/// Using time instead of PID avoids predictable sequences since PIDs
/// are sequential and easily enumerated by anti-cheat.
pub fn init_jitter_rng() {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u32;
    let seed = if seed == 0 { 1 } else { seed };
    if let Ok(mut rng) = JITTER_RNG.lock() {
        *rng = Some(textquest_common::nav::Xorshift32::new(seed));
    }
}

/// Human-like jitter using a triangle distribution (sum of two uniform draws).
/// Peaks at 6 ticks with occasional hesitation spikes simulating distraction.
/// Range: 2-30 ticks (without hesitation: 2-10, with hesitation: 7-30).
fn human_jitter_ticks(rng: &mut textquest_common::nav::Xorshift32) -> u64 {
    // Triangle distribution: sum of two uniform draws (peaks at center)
    let base = (rng.next_u32() % 5 + 1) + (rng.next_u32() % 5 + 1); // 2-10, peaks at 6
    // 5% chance of hesitation spike (simulates distraction)
    let hesitate = if rng.next_u32().is_multiple_of(20) {
        rng.next_u32() % 15 + 5
    } else {
        0
    };
    u64::from(base + hesitate)
}

/// Queue a slash command for execution on the next game loop tick.
/// Safe to call from any thread — the game loop will pick it up.
pub fn queue_slash_command(command: String) {
    if let Ok(mut queue) = PENDING_COMMANDS.lock() {
        queue.push(PendingCommand {
            command: textquest_common::ipc::Command::SlashCommand { command },
            execute_at_tick: 0, // execute immediately on next tick
        });
    }
}

/// Enqueue a command with a human-like jitter delay.
fn enqueue_command(cmd: textquest_common::ipc::Command, current_tick: u64) {
    let delay = JITTER_RNG
        .lock()
        .ok()
        .and_then(|mut guard| guard.as_mut().map(human_jitter_ticks))
        .unwrap_or(5);

    if let Ok(mut queue) = PENDING_COMMANDS.lock() {
        queue.push(PendingCommand {
            command: cmd,
            execute_at_tick: current_tick + delay,
        });
    }
}

/// Drain and execute any commands whose scheduled tick has arrived.
fn process_pending_commands(current_tick: u64) {
    let Ok(mut queue) = PENDING_COMMANDS.lock() else {
        return;
    };
    if queue.is_empty() {
        return;
    }

    let mut ready = Vec::new();
    queue.retain(|pending| {
        if current_tick >= pending.execute_at_tick {
            ready.push(pending.command.clone());
            false
        } else {
            true
        }
    });

    // Drop the lock before dispatching to avoid holding it during command execution.
    drop(queue);

    for cmd in ready {
        dispatch_command(cmd);
    }
}

/// Called every frame (~20/sec) after the original `MainLoop` runs.
/// This is our main entry point for per-frame logic.
fn on_game_tick() {
    let tick = TICK_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    // Check foreground status every 30 frames (~1.5 seconds) to minimize overhead.
    if tick.is_multiple_of(30) {
        update_foreground_status();
    }

    // Auto-accept dialogs every 30 frames (~1.5 seconds).
    if tick % 30 == 15 {
        // SAFETY: check_dialogs reads EQ's CXWndManager and clicks dialog buttons
        // via vtable. Called from the game loop thread where UI state is stable and
        // vtable calls are safe. EQ_BASE and PINST_CXWND_MANAGER are validated
        // internally with null checks before any dereference.
        unsafe {
            crate::dialog::check_dialogs();
        }
    }

    // Rename window every 100 frames (~5 seconds) to "[DMFT] EQ - CharName (ZoneName)".
    if tick % 100 == 5 {
        update_window_title();
    }

    // Enqueue IPC commands with jitter delay for anti-detection.
    for cmd in crate::ipc::poll_commands() {
        enqueue_command(cmd, tick);
    }

    // Execute commands whose scheduled tick has arrived.
    process_pending_commands(tick);

    // Check for pending login button click (queued from IPC thread).
    let button_addr = PENDING_BUTTON_CLICK.swap(0, std::sync::atomic::Ordering::AcqRel);
    if button_addr != 0 {
        if !is_readable(button_addr, 8) {
            tracing::warn!(
                ptr = format!("{:#x}", button_addr),
                "Pending button click target is no longer readable — skipping (window may have been destroyed)"
            );
        } else {
            tracing::info!(
                ptr = format!("{:#x}", button_addr),
                "Clicking login button on game loop thread"
            );
            // SAFETY: button_addr was stored by the IPC thread after resolving a
            // CXWnd pointer from CXWndManager's window array. The click executes
            // WndNotification(XWM_LCLICK) through the CXWnd vtable. Must run on
            // the game loop thread (which we are). The pointer is re-validated via
            // is_readable() above to guard against use-after-free if the window
            // was destroyed between queuing and dispatch.
            unsafe {
                crate::eq::widgets::click_button_via_vtable(button_addr);
            }
        }
    }

    // Process Enter World sequence (SelectCharacter → wait → EnterWorld).
    // Stage 1: Find character by name, call SelectCharacter(index)
    // Stage 2: Wait ~90 ticks (~3 seconds)
    // Stage 3: Call EnterWorld()
    let stage = ENTER_WORLD_STAGE.load(std::sync::atomic::Ordering::Acquire);
    if stage == 1 {
        let wnd = PENDING_ENTER_WORLD_WND.load(std::sync::atomic::Ordering::Acquire);
        let select_fn = PENDING_SELECT_CHAR_FN.load(std::sync::atomic::Ordering::Acquire);
        if wnd != 0 && select_fn != 0 {
            // Look up character index by name (falls back to 0 if not found/empty)
            let char_name = PENDING_CHAR_NAME
                .get()
                .and_then(|m| m.lock().ok())
                .map(|n| n.clone())
                .unwrap_or_default();
            let index = find_character_index(wnd, &char_name);

            tracing::info!(
                wnd = format!("{:#x}", wnd),
                func = format!("{:#x}", select_fn),
                character = %char_name,
                index,
                "Phase 3: Calling SelectCharacter on game loop thread"
            );
            // SAFETY: select_fn was rebased from SELECT_CHARACTER offset against
            // eqgame.exe's base. The transmute converts it to a function pointer
            // matching CCharacterListWnd::SelectCharacter(int). `wnd` is a freshly
            // resolved CCharacterListWnd* from CXWndManager. If either address is
            // stale or the offset is wrong, EQ will crash (no safe fallback exists
            // for calling internal game functions with wrong addresses).
            unsafe {
                // SelectCharacter(int index) — x64: RCX=this, RDX=index
                type SelectCharFn = unsafe extern "C" fn(this: usize, index: i32);
                let func: SelectCharFn = std::mem::transmute(select_fn);
                func(wnd, index);
            }
            tracing::info!(
                index,
                "Phase 3: SelectCharacter called — waiting 3s before EnterWorld"
            );
            ENTER_WORLD_WAIT_UNTIL.store(tick + 90, std::sync::atomic::Ordering::Release);
            ENTER_WORLD_STAGE.store(2, std::sync::atomic::Ordering::Release);
        } else {
            // No SelectCharacter available — skip to EnterWorld directly
            tracing::warn!("Phase 3: SelectCharacter not available — skipping to EnterWorld");
            ENTER_WORLD_WAIT_UNTIL.store(tick + 30, std::sync::atomic::Ordering::Release);
            ENTER_WORLD_STAGE.store(2, std::sync::atomic::Ordering::Release);
        }
    } else if stage == 2 {
        let wait_until = ENTER_WORLD_WAIT_UNTIL.load(std::sync::atomic::Ordering::Acquire);
        if tick >= wait_until {
            ENTER_WORLD_STAGE.store(3, std::sync::atomic::Ordering::Release);
        }
    } else if stage == 3 {
        let enter_fn = PENDING_ENTER_WORLD_FN.load(std::sync::atomic::Ordering::Acquire);
        // Re-scan for CCharacterListWnd fresh — the pointer stored in Stage 1
        // may be stale if the window was destroyed/recreated during the wait.
        let wnd = if let Some(w) = rescan_char_list_wnd() {
            w
        } else {
            // Rescan failed — CXWndManager may be in a transitional state.
            // Do NOT fall back to stored pointer (could be stale/freed).
            // Retry next tick up to ~5 seconds, then abort.
            let stored = PENDING_ENTER_WORLD_WND.load(std::sync::atomic::Ordering::Acquire);
            if stored == 0 {
                tracing::error!("Phase 3: rescan failed and no stored pointer — aborting");
                ENTER_WORLD_STAGE.store(0, std::sync::atomic::Ordering::Release);
                return;
            }
            let retries = ENTER_WORLD_RETRIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if retries >= 150 {
                tracing::error!("Phase 3: rescan failed after 150 retries — aborting");
                ENTER_WORLD_STAGE.store(0, std::sync::atomic::Ordering::Release);
                ENTER_WORLD_RETRIES.store(0, std::sync::atomic::Ordering::Relaxed);
                return;
            }
            tracing::warn!(retries, "Phase 3: rescan failed — will retry next tick");
            return; // Stay in stage 3, retry next tick
        };
        if enter_fn != 0 {
            tracing::info!(
                wnd = format!("{:#x}", wnd),
                func = format!("{:#x}", enter_fn),
                "Phase 3: Calling EnterWorld() on game loop thread (re-validated)"
            );
            // SAFETY: enter_fn was rebased from ENTER_WORLD offset. `wnd` was
            // re-validated via rescan_char_list_wnd() immediately above (not the
            // stale pointer from Stage 1). The transmute converts the address to
            // CCharacterListWnd::EnterWorld(). If the offset is wrong or the
            // window was destroyed between rescan and call, this is UB/crash.
            unsafe {
                type EnterWorldFn = unsafe extern "C" fn(this: usize);
                let func: EnterWorldFn = std::mem::transmute(enter_fn);
                func(wnd);
            }
            tracing::info!("Phase 3: EnterWorld() called — entering world!");
        }
        // Reset all state — clear payload atomics BEFORE clearing the stage flag.
        // If ENTER_WORLD_STAGE were reset to 0 first, the IPC thread could observe
        // stage == 0 and immediately queue a new enter-world, writing new values
        // into PENDING_*; our subsequent stores of 0 would then silently discard
        // that queued request.  Clearing payloads first (Relaxed is fine — they
        // are guarded by the stage flag, not independently synchronized) and
        // publishing the idle stage last (Release) eliminates that window.
        PENDING_ENTER_WORLD_WND.store(0, std::sync::atomic::Ordering::Relaxed);
        PENDING_ENTER_WORLD_FN.store(0, std::sync::atomic::Ordering::Relaxed);
        PENDING_SELECT_CHAR_FN.store(0, std::sync::atomic::Ordering::Relaxed);
        ENTER_WORLD_RETRIES.store(0, std::sync::atomic::Ordering::Relaxed);
        ENTER_WORLD_STAGE.store(0, std::sync::atomic::Ordering::Release);
    }

    // Lazy-init the navigator once we're in-world.
    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);

    {
        static NAV_INITIALIZED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !NAV_INITIALIZED.load(std::sync::atomic::Ordering::Relaxed) {
            if eq_base != 0
                && let Some(player_addr) = textquest_common::offsets::rebase(
                    textquest_common::offsets::PINST_LOCAL_PLAYER,
                    eq_base,
                )
            {
                // SAFETY: player_addr was rebased from PINST_LOCAL_PLAYER — a
                // known global pointer in eqgame.exe. Dereferencing it yields the
                // PlayerClient* (null when not logged in). Checked for non-null
                // immediately after. The address is within committed eqgame memory.
                let player_ptr = unsafe { *(player_addr as *const usize) };
                if player_ptr != 0 {
                    crate::nav::init(player_ptr, std::process::id());
                    NAV_INITIALIZED.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
    }

    // Run navigation state machine.
    // Pass current target and cached nearby spawns for the stick engine,
    // plus a target sample for the warp monitor.
    {
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        let nav_target = if eq_base != 0 {
            read_target_state(eq_base)
        } else {
            None
        };
        let target_sample = nav_target.as_ref().map(|t| crate::nav::warp::TargetSample {
            id: t.spawn_id,
            position: textquest_common::nav::Waypoint::new(t.x, t.y, t.z),
        });
        let nearby_guard = CACHED_NEARBY_FOR_STICK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::nav::tick(nav_target.as_ref(), &nearby_guard, target_sample.as_ref());
    }

    // Run login FSM when not yet in world (local_player is null).
    // The login FSM drives credential entry, server/char selection autonomously.
    // Once login is done (InWorld, Error, or Idle), stop all login automation
    // including the periodic Enter key — otherwise it could dismiss NPC dialogs,
    // close windows, or cause other unintended actions in the live game.
    {
        let login_done = crate::login::is_done();

        if !login_done {
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
            let in_world = if eq_base != 0 {
                textquest_common::offsets::rebase(
                    textquest_common::offsets::PINST_LOCAL_PLAYER,
                    eq_base,
                )
                // SAFETY: addr is rebased PINST_LOCAL_PLAYER — a committed
                // global in eqgame.exe. Reading a usize from it yields the
                // local player pointer (0 = not logged in).
                .is_some_and(|addr| unsafe { *(addr as *const usize) } != 0)
            } else {
                false
            };

            if !in_world {
                if let Some(phase) = crate::login::tick() {
                    crate::ipc::send_response(textquest_common::ipc::Response::LoginPhaseUpdate {
                        phase,
                    });
                }

                // If not in world and game loop is running, we're at character select.
                // Send Enter key every ~3 seconds to click the Enter World button.
                // Guard: skip when the Enter World FSM is active (stage != 0) to
                // avoid dismissing windows the FSM expects to interact with.
                if tick % 90 == 45
                    && ENTER_WORLD_STAGE.load(std::sync::atomic::Ordering::Acquire) == 0
                {
                    send_enter_to_eq();
                }
            }
        }
    }

    // Read game state and publish to shared memory for the orchestrator.
    read_and_publish_state(tick);
}

/// Send Enter key to this EQ process's window via `PostMessage`.
/// Used at character select to click "Enter World".
#[cfg(windows)]
pub fn send_enter_to_eq() {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW,
    };

    let our_pid = std::process::id();
    let mut target_hwnd: isize = 0;

    // SAFETY: This callback is only invoked by EnumWindows below, which passes
    // our `data` pointer as LPARAM. The cast back to (u32, *mut isize) is valid
    // because we control the LPARAM value. HWND is always valid within the callback.
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe extern "system" fn find_eq_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let data = &mut *(lparam.0 as *mut (u32, *mut isize));
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == data.0 && IsWindowVisible(hwnd).as_bool() {
            *data.1 = hwnd.0;
            return BOOL(0); // stop
        }
        BOOL(1)
    }

    let mut data = (our_pid, &mut target_hwnd as *mut isize);
    // SAFETY: EnumWindows calls find_eq_window for each top-level window.
    // `data` lives on the stack and outlives the synchronous EnumWindows call.
    // The LPARAM cast is valid because we cast it back to the same type in the callback.
    unsafe {
        let _ = EnumWindows(Some(find_eq_window), LPARAM(&mut data as *mut _ as isize));
    }

    if target_hwnd != 0 {
        const WM_KEYDOWN: u32 = 0x0100;
        const WM_KEYUP: u32 = 0x0101;
        const VK_RETURN: u16 = 0x0D;
        let hwnd = HWND(target_hwnd);
        // SAFETY: PostMessageW is safe to call with any HWND — if the window was
        // destroyed between EnumWindows and here, PostMessage returns an error
        // (which we ignore with `let _`). The WM_KEYDOWN/WM_KEYUP messages with
        // VK_RETURN are standard Win32 keyboard messages.
        unsafe {
            let _ = PostMessageW(hwnd, WM_KEYDOWN, WPARAM(VK_RETURN as usize), LPARAM(0));
            let _ = PostMessageW(hwnd, WM_KEYUP, WPARAM(VK_RETURN as usize), LPARAM(0));
        }
        tracing::info!("Sent Enter key to eqgame window (character select → enter world)");
    }
}

#[cfg(not(windows))]
pub fn send_enter_to_eq() {}

// ─── Game State Reading ───
// Reads EQ memory directly (we're in-process) and publishes to shared memory.

/// Read game state from EQ memory and publish to shared memory each tick.
/// Local player + target are read every tick (fast — just pointer derefs).
/// Nearby spawns are read every 30 ticks (~1 second) to reduce overhead.
///
/// Uses a cached `GameState` to avoid cloning ~100 `SpawnData` (each with 2 String
/// heap allocations) on the 29/30 ticks where spawns haven't changed. Only the
/// cheap fields (player, target, timestamp, nav/combat status) are updated in place.
fn read_and_publish_state(tick: u64) {
    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    if eq_base == 0 {
        return;
    }

    // Read local player (every tick).
    let local_player = read_local_player_state(eq_base);
    if local_player.is_none() {
        return; // Not logged in — nothing to publish.
    }

    // Read target (every tick).
    let target = read_target_state(eq_base);

    // Cache the entire GameState to avoid cloning the spawn Vec on non-refresh ticks.
    // On refresh ticks (every 30): rebuild spawns + all fields.
    // On other ticks: update only cheap fields in place (no heap allocations for spawns).
    static CACHED_STATE: Mutex<Option<textquest_common::types::GameState>> = Mutex::new(None);
    static SPAWN_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    let Ok(mut cached) = CACHED_STATE.lock() else {
        return;
    };

    let refresh_spawns = tick.is_multiple_of(30) || cached.is_none();

    if refresh_spawns {
        let Some(ref player) = local_player else {
            return;
        };
        let spawns = read_nearby_spawns(eq_base, player.x, player.y, player.z);
        let (zone_short, zone_long) = read_zone_names(eq_base);

        // Update cached nearby spawns for the stick engine (accessed by nav::tick each frame).
        if let Ok(mut cached_nearby) = CACHED_NEARBY_FOR_STICK.lock() {
            cached_nearby.clone_from(&spawns);
        }

        *cached = Some(textquest_common::types::GameState {
            client_id: std::process::id(),
            local_player,
            target,
            nearby_spawns: spawns,
            timestamp_ms: current_time_ms(),
            nav_status: crate::nav::status(),
            combat_status: crate::combat::status(),
            zone_short_name: zone_short,
            zone_long_name: zone_long,
        });
    } else if let Some(ref mut state) = *cached {
        state.local_player = local_player;
        state.target = target;
        state.timestamp_ms = current_time_ms();
        state.nav_status = crate::nav::status();
        state.combat_status = crate::combat::status();
    } else {
        return;
    }

    if let Some(ref state) = *cached {
        let spawn_epoch = if refresh_spawns {
            SPAWN_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
        } else {
            SPAWN_EPOCH.load(std::sync::atomic::Ordering::Relaxed)
        };
        let frame = state.to_shared_frame(spawn_epoch, refresh_spawns);
        crate::ipc::publish_state(&frame);
    }
}

/// Check whether `addr` points to at least `len` bytes of readable committed memory.
///
/// Uses `VirtualQuery` to verify the page is committed and readable before we
/// dereference it. Returns `false` for null, misaligned, or unmapped addresses.
#[cfg(windows)]
pub(crate) fn is_readable(addr: usize, len: usize) -> bool {
    use windows::Win32::System::Memory::{
        MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_GUARD, PAGE_NOACCESS, VirtualQuery,
    };

    if addr == 0 || len == 0 {
        return false;
    }

    let mut mbi = MEMORY_BASIC_INFORMATION::default();
    let ret = unsafe {
        VirtualQuery(
            Some(addr as *const core::ffi::c_void),
            &mut mbi,
            size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };

    if ret == 0 {
        return false;
    }

    // Must be committed (not reserved or free).
    if mbi.State != MEM_COMMIT {
        return false;
    }

    // Reject guard pages and no-access pages.
    let protect = mbi.Protect;
    if protect.contains(PAGE_NOACCESS) || protect.contains(PAGE_GUARD) {
        return false;
    }

    // Verify the entire range falls within this region.
    let region_end = mbi.BaseAddress as usize + mbi.RegionSize;
    let range_end = addr.saturating_add(len);
    range_end <= region_end
}

#[cfg(not(windows))]
pub(crate) fn is_readable(_addr: usize, _len: usize) -> bool {
    // Stub for non-Windows builds (demo mode). Always true since we never
    // dereference real pointers on macOS/Linux.
    true
}

/// Read a null-terminated string from an in-process address into a stack buffer.
///
/// Uses a fixed 128-byte stack buffer (sufficient for EQ name/zone fields) to
/// avoid per-call heap allocations on the hot path. Only allocates a `String`
/// for the final return value.
///
/// # Safety
/// * `addr` must point to readable memory of at least `max_len` bytes.
/// * `max_len` must not exceed 128 (the stack buffer size) AND must not exceed
///   the actual allocated buffer size for the field being read. EQ's fixed-size
///   char arrays (name=64, displayedName=64, zone=128) always meet this.
/// * Returns an empty string safely when `addr == 0` or memory is unreadable.
unsafe fn read_string_at(addr: usize, max_len: usize) -> String {
    if addr == 0 {
        return String::new();
    }

    // Validate pointer before dereferencing.
    if !is_readable(addr, max_len) {
        return String::new();
    }

    // Use a stack buffer to avoid heap allocation for the intermediate copy.
    // 128 bytes covers all EQ string fields (name=64, zone=128).
    const BUF_SIZE: usize = 128;
    let capped = if max_len <= BUF_SIZE {
        max_len
    } else {
        BUF_SIZE
    };

    let mut buf = [0u8; BUF_SIZE];
    // SAFETY: `addr` was validated as readable for `max_len` bytes by
    // is_readable() above (VirtualQuery confirms committed, non-guard pages).
    // `capped` <= `max_len` and <= BUF_SIZE, so both source and dest are in bounds.
    // The copy is non-overlapping because `buf` is a stack allocation and `addr`
    // points into EQ's process memory.
    unsafe {
        core::ptr::copy_nonoverlapping(addr as *const u8, buf.as_mut_ptr(), capped);
    }

    let len = buf[..capped].iter().position(|&b| b == 0).unwrap_or(capped);
    String::from_utf8_lossy(&buf[..len]).into_owned()
}

/// Build a `SpawnData` from a `PlayerClient` pointer (in-process direct read).
///
/// # Safety
/// Caller must ensure `spawn_ptr` is a plausible `PlayerClient` address.
/// This function validates readability before dereferencing and returns
/// `SpawnData::default()` for any invalid pointer.
unsafe fn read_spawn_data(spawn_ptr: usize) -> textquest_common::types::SpawnData {
    use textquest_common::offsets::{player_base, player_zone};

    // Reject null and obviously bad pointers (must be pointer-aligned).
    if spawn_ptr == 0 || !spawn_ptr.is_multiple_of(core::mem::align_of::<usize>()) {
        return textquest_common::types::SpawnData::default();
    }

    // Validate that the spawn_id field region is readable before touching it.
    if !is_readable(spawn_ptr + player_base::SPAWN_ID, size_of::<u32>()) {
        return textquest_common::types::SpawnData::default();
    }

    // Read spawn_id first as a validity canary: id == 0 means the PlayerClient
    // slot is empty or has been freed. Reading further fields from a freed spawn
    // causes an access violation and crashes eqgame.exe.
    // SAFETY: spawn_ptr alignment and readability were validated above via
    // is_multiple_of(align_of::<usize>()) and is_readable(). The SPAWN_ID
    // offset is a known field within the PlayerClient struct.
    let spawn_id = unsafe { *((spawn_ptr + player_base::SPAWN_ID) as *const u32) };
    if spawn_id == 0 {
        return textquest_common::types::SpawnData::default();
    }

    // SAFETY for all field reads below: spawn_ptr points to a live PlayerClient
    // struct validated by is_readable() and a non-zero spawn_id canary above.
    // Each offset is a known field within PlayerClient derived from MQ2 headers.
    // read_string_at performs its own is_readable() check internally. Individual
    // reads of primitives (u8, u32, f32) are naturally aligned within the struct.
    // If the spawn is freed concurrently by EQ (rare race), reads may return
    // garbage but won't segfault because the page is still committed.
    let name = unsafe { read_string_at(spawn_ptr + player_base::NAME, 64) };
    let displayed_name = unsafe { read_string_at(spawn_ptr + player_base::DISPLAYED_NAME, 64) };
    let spawn_type = unsafe { *((spawn_ptr + player_base::TYPE) as *const u8) };
    let x = unsafe { *((spawn_ptr + player_base::X) as *const f32) };
    let y = unsafe { *((spawn_ptr + player_base::Y) as *const f32) };
    let z = unsafe { *((spawn_ptr + player_base::Z) as *const f32) };
    let heading = unsafe { *((spawn_ptr + player_base::HEADING) as *const f32) };

    // Diagnostic: log once if position looks suspicious (near-zero with valid name).
    // Log metadata only (never raw process memory or addresses).
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        static DIAG_TICK: AtomicU64 = AtomicU64::new(0);
        let tick = DIAG_TICK.fetch_add(1, Ordering::Relaxed);
        if x.abs() < 1.0 && y.abs() < 1.0 && tick.is_multiple_of(300) {
            tracing::warn!(
                name = %name,
                spawn_id,
                x, y, z,
                "DLL: position near zero — possible offset mismatch"
            );
        }
    }

    let level = unsafe { *((spawn_ptr + player_zone::LEVEL) as *const u8) };
    let class_id = unsafe { *((spawn_ptr + player_zone::CHAR_CLASS) as *const u8) };
    let hp_current = unsafe { *((spawn_ptr + player_zone::HP_CURRENT) as *const i64) };
    let hp_max = unsafe { *((spawn_ptr + player_zone::HP_MAX) as *const i64) };
    let mana_current = unsafe { *((spawn_ptr + player_zone::MANA_CURRENT) as *const i32) };
    let mana_max = unsafe { *((spawn_ptr + player_zone::MANA_MAX) as *const i32) };
    let endurance_current =
        unsafe { *((spawn_ptr + player_zone::ENDURANCE_CURRENT) as *const i32) };
    let endurance_max = unsafe { *((spawn_ptr + player_zone::ENDURANCE_MAX) as *const u32) };
    let speed_run = unsafe { *((spawn_ptr + player_base::SPEED_RUN) as *const f32) };
    let stand_state = unsafe { *((spawn_ptr + player_zone::STANDSTATE) as *const u8) };

    textquest_common::types::SpawnData {
        spawn_id,
        name,
        displayed_name,
        spawn_type,
        level,
        class_id,
        x,
        y,
        z,
        heading,
        hp_current,
        hp_max,
        mana_current,
        mana_max,
        endurance_current,
        endurance_max,
        speed_run,
        stand_state,
    }
}

/// Read zone short name and long name from zoneHeader struct.
fn read_zone_names(eq_base: u64) -> (String, String) {
    use textquest_common::offsets::zone_info;

    let zone_addr = match textquest_common::offsets::rebase(zone_info::INST_EQ_ZONE_INFO, eq_base) {
        Some(a) => a,
        None => return (String::new(), String::new()),
    };

    let short = unsafe { read_string_at(zone_addr + zone_info::SHORT_NAME, 128) };
    let long = unsafe { read_string_at(zone_addr + zone_info::LONG_NAME, 128) };
    (short, long)
}

/// Read local player state. Returns None if not logged in.
fn read_local_player_state(eq_base: u64) -> Option<textquest_common::types::SpawnData> {
    let player_ptr_addr =
        textquest_common::offsets::rebase(textquest_common::offsets::PINST_LOCAL_PLAYER, eq_base)?;
    // SAFETY: player_ptr_addr is the rebased address of PINST_LOCAL_PLAYER,
    // a global pointer in eqgame.exe's data section. Reading a usize from it
    // yields the PlayerClient* for the local player (null when not logged in).
    // The address is within eqgame.exe's committed memory.
    let player_ptr = unsafe { *(player_ptr_addr as *const usize) };
    if player_ptr == 0 {
        return None;
    }

    // One-time diagnostic log of the pointer chain
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        static LOGGED: AtomicBool = AtomicBool::new(false);
        if !LOGGED.swap(true, Ordering::Relaxed) {
            tracing::info!(
                eq_base = format!("{:#x}", eq_base),
                player_ptr_addr = format!("{:#x}", player_ptr_addr),
                player_ptr = format!("{:#x}", player_ptr),
                "read_local_player_state pointer chain (one-time)"
            );
        }
    }

    Some(unsafe { read_spawn_data(player_ptr) })
}

/// Read current target state. Returns None if no target selected.
fn read_target_state(eq_base: u64) -> Option<textquest_common::types::SpawnData> {
    let target_ptr_addr =
        textquest_common::offsets::rebase(textquest_common::offsets::PINST_TARGET, eq_base)?;
    // SAFETY: target_ptr_addr is the rebased address of PINST_TARGET, a global
    // pointer in eqgame.exe. Reading a usize yields the target's PlayerClient*
    // (null when no target). read_spawn_data validates the pointer internally.
    let target_ptr = unsafe { *(target_ptr_addr as *const usize) };
    if target_ptr == 0 {
        return None;
    }
    Some(unsafe { read_spawn_data(target_ptr) })
}

/// Walk the spawn linked list and collect spawns within `max_distance` units
/// of the given position. Capped at 100 spawns.
fn read_nearby_spawns(
    eq_base: u64,
    player_x: f32,
    player_y: f32,
    player_z: f32,
) -> Vec<textquest_common::types::SpawnData> {
    use textquest_common::offsets::{player_base, spawn_manager};

    const MAX_NEARBY: usize = 100;
    const MAX_DISTANCE_SQ: f32 = 500.0 * 500.0;

    let mgr_ptr_addr = match textquest_common::offsets::rebase(
        textquest_common::offsets::PINST_SPAWN_MANAGER,
        eq_base,
    ) {
        Some(addr) => addr,
        None => return Vec::new(),
    };

    // Validate the manager pointer address is readable.
    if !is_readable(mgr_ptr_addr, size_of::<usize>()) {
        return Vec::new();
    }

    // SAFETY: mgr_ptr_addr was validated as readable above. Reading a usize
    // from PINST_SPAWN_MANAGER yields the SpawnManager* singleton pointer.
    let mgr_ptr = unsafe { *(mgr_ptr_addr as *const usize) };
    if mgr_ptr == 0 {
        return Vec::new();
    }

    // TList at spawn_manager::PLAYER_LIST, first node pointer at offset 0x00.
    let list_addr = mgr_ptr + spawn_manager::PLAYER_LIST;
    if !is_readable(list_addr, size_of::<usize>()) {
        return Vec::new();
    }
    // SAFETY: list_addr was validated as readable immediately above.
    // Reading a usize yields the head pointer of the spawn linked list.
    let mut current = unsafe { *(list_addr as *const usize) };

    let mut spawns = Vec::with_capacity(MAX_NEARBY.min(256));
    let mut walked: usize = 0;
    const MAX_WALK: usize = 2000; // Safety limit to prevent infinite loops.

    while current != 0 && spawns.len() < MAX_NEARBY && walked < MAX_WALK {
        walked += 1;

        // Validate current pointer before any dereference.
        if current % core::mem::align_of::<usize>() != 0 {
            tracing::warn!(
                ptr = format!("{:#x}", current),
                walked,
                "read_nearby_spawns: misaligned spawn pointer, aborting walk"
            );
            break;
        }
        if !is_readable(current + player_base::X, size_of::<f32>() * 3) {
            tracing::warn!(
                ptr = format!("{:#x}", current),
                walked,
                "read_nearby_spawns: unreadable spawn pointer, aborting walk"
            );
            break;
        }

        // SAFETY: `current` was validated as readable for position fields by
        // the is_readable() check above (covers X/Y/Z at known offsets).
        // Quick distance check before building full SpawnData.
        let sx = unsafe { *((current + player_base::X) as *const f32) };
        let sy = unsafe { *((current + player_base::Y) as *const f32) };
        let sz = unsafe { *((current + player_base::Z) as *const f32) };

        let dx = sx - player_x;
        let dy = sy - player_y;
        let dz = sz - player_z;
        let dist_sq = dx * dx + dy * dy + dz * dz;

        if dist_sq <= MAX_DISTANCE_SQ {
            let spawn = unsafe { read_spawn_data(current) };
            spawns.push(spawn);
        }

        // Validate NEXT pointer field is readable before following it.
        let next_addr = current + player_base::NEXT;
        if !is_readable(next_addr, size_of::<usize>()) {
            tracing::warn!(
                ptr = format!("{:#x}", current),
                walked,
                "read_nearby_spawns: NEXT pointer unreadable, aborting walk"
            );
            break;
        }
        // SAFETY: next_addr was validated as readable by is_readable() above.
        // Follow NEXT pointer in linked list.
        current = unsafe { *((next_addr) as *const usize) };
    }

    spawns
}

/// Current time in milliseconds since UNIX epoch.
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Check if our window is the foreground window. Used for render skipping —
/// background clients skip 3D rendering to save GPU/CPU.
fn update_foreground_status() {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

        // SAFETY: GetForegroundWindow and GetWindowThreadProcessId are always
        // safe Win32 calls. GetForegroundWindow returns NULL if no window is
        // focused, which GetWindowThreadProcessId handles gracefully (returns 0).
        let fg: HWND = unsafe { GetForegroundWindow() };
        let our_pid = std::process::id();

        let mut fg_pid: u32 = 0;
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                fg,
                Some(&mut fg_pid),
            );
        }

        let is_fg = fg_pid == our_pid;
        WINDOW_IS_FOREGROUND.store(is_fg, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(not(windows))]
    {
        // Always foreground on non-Windows (dev builds).
    }
}

/// Returns true if this client's window is currently in the foreground.
/// The render hook can use this to skip `CDisplay::RealRender_World` for
/// background clients, saving near-zero GPU usage across 35 bot clients.
pub fn is_foreground() -> bool {
    WINDOW_IS_FOREGROUND.load(std::sync::atomic::Ordering::Relaxed)
}

/// Read character name + zone name from EQ memory and set the window title
/// to "[DMFT] EQ - `CharName` (`ZoneName`)" so the orchestrator can identify clients by PID.
fn update_window_title() {
    #[cfg(windows)]
    {
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        if eq_base == 0 {
            return;
        }

        // Read local player name from PlayerClient->Name (char[64] at offset 0xb4).
        let char_name = match read_char_name(eq_base) {
            Some(n) if !n.is_empty() => n,
            _ => return, // Not logged in yet — skip.
        };

        // Read zone name from zoneHeader struct. Prefer long name (display name like
        // "West Freeport") for readability, fall back to short name ("freportw").
        let zone_name = read_zone_long_name(eq_base)
            .or_else(|| read_zone_short_name(eq_base))
            .unwrap_or_default();

        // Build title: "[DMFT] EQ - CharName (ZoneName)" or "[DMFT] EQ - CharName" if no zone.
        let title = if zone_name.is_empty() {
            tracing::trace!(char_name = %char_name, "Zone name empty — title without zone");
            format!("[DMFT] EQ - {char_name}\0")
        } else {
            format!("[DMFT] EQ - {char_name} ({zone_name})\0")
        };

        // Find our window by enumerating windows for this PID.
        let our_pid = std::process::id();
        set_window_title_for_pid(our_pid, &title);
    }

    #[cfg(not(windows))]
    {
        // No-op on non-Windows.
    }
}

/// Read the local player's Name field (char[64]) directly from EQ memory.
#[cfg(windows)]
fn read_char_name(eq_base: u64) -> Option<String> {
    use textquest_common::offsets::{self, player_base};

    let player_ptr_addr = offsets::rebase(offsets::PINST_LOCAL_PLAYER, eq_base)?;
    // SAFETY: player_ptr_addr is the rebased PINST_LOCAL_PLAYER global.
    // Dereferencing yields PlayerClient* (null when not logged in).
    let player_ptr = unsafe { *(player_ptr_addr as *const usize) };
    if player_ptr == 0 {
        return None;
    }

    let name_addr = player_ptr + player_base::NAME;
    // SAFETY: player_ptr is a live PlayerClient*, validated non-null above.
    // NAME is a char[64] field at a known offset. The 64-byte slice is within
    // the PlayerClient struct's committed memory.
    let name_bytes = unsafe { std::slice::from_raw_parts(name_addr as *const u8, 64) };
    let len = name_bytes.iter().position(|&b| b == 0).unwrap_or(64);
    String::from_utf8(name_bytes[..len].to_vec()).ok()
}

/// Read the zone short name (char[128]) from instEQZoneInfo.
#[cfg(windows)]
fn read_zone_short_name(eq_base: u64) -> Option<String> {
    use textquest_common::offsets::zone_info;

    let zone_addr = textquest_common::offsets::rebase(zone_info::INST_EQ_ZONE_INFO, eq_base)?;
    let short_name_addr = zone_addr + zone_info::SHORT_NAME;
    // SAFETY: zone_addr is the rebased instEQZoneInfo global. SHORT_NAME is a
    // char[128] field at a known offset within the zone header struct.
    let name_bytes = unsafe { std::slice::from_raw_parts(short_name_addr as *const u8, 128) };
    let len = name_bytes.iter().position(|&b| b == 0).unwrap_or(128);
    if len == 0 {
        return None;
    }
    String::from_utf8(name_bytes[..len].to_vec()).ok()
}

/// Read the zone long name (char[128]) from instEQZoneInfo (e.g., "West Freeport").
#[cfg(windows)]
fn read_zone_long_name(eq_base: u64) -> Option<String> {
    use textquest_common::offsets::zone_info;

    let zone_addr = textquest_common::offsets::rebase(zone_info::INST_EQ_ZONE_INFO, eq_base)?;
    let long_name_addr = zone_addr + zone_info::LONG_NAME;
    // SAFETY: zone_addr is the rebased instEQZoneInfo global. LONG_NAME is a
    // char[128] field at a known offset within the zone header struct.
    let name_bytes = unsafe { std::slice::from_raw_parts(long_name_addr as *const u8, 128) };
    let len = name_bytes.iter().position(|&b| b == 0).unwrap_or(128);
    if len == 0 {
        return None;
    }
    String::from_utf8(name_bytes[..len].to_vec()).ok()
}

/// Set the window title for all top-level windows belonging to the given PID.
#[cfg(windows)]
fn set_window_title_for_pid(pid: u32, title: &str) {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetWindowTextA,
    };
    use windows::core::PCSTR;

    // We use a simple callback that captures our PID + title via LPARAM.
    struct CallbackData {
        pid: u32,
        title_ptr: *const u8,
    }

    // SAFETY: Callback invoked by EnumWindows below. The LPARAM is a pointer
    // to our stack-local CallbackData which outlives the synchronous EnumWindows
    // call. The title_ptr points to a null-terminated string also on the stack.
    unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let data = &*(lparam.0 as *const CallbackData);
            let mut wnd_pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut wnd_pid));

            if wnd_pid == data.pid && IsWindowVisible(hwnd).as_bool() {
                let _ = SetWindowTextA(hwnd, PCSTR(data.title_ptr));
            }
            BOOL(1) // continue
        }
    }

    let data = CallbackData {
        pid,
        title_ptr: title.as_ptr(),
    };
    unsafe {
        let _ = EnumWindows(Some(enum_cb), LPARAM(&data as *const _ as isize));
    }
}

/// Dispatch a single IPC command received from the orchestrator.
fn dispatch_command(cmd: textquest_common::ipc::Command) {
    use textquest_common::ipc::Command;

    match cmd {
        Command::SlashCommand { command } => {
            tracing::info!(cmd = %command, "Executing slash command");
            execute_slash_command(&command);
        }
        Command::NavigateTo { waypoints } => {
            crate::nav::handle_command(crate::nav::NavCommand::Navigate(waypoints));
        }
        Command::SetCamp { spot } => {
            crate::nav::handle_command(crate::nav::NavCommand::SetCamp(spot));
        }
        Command::SetCampConfig { config } => {
            crate::nav::handle_command(crate::nav::NavCommand::SetCampConfig(config));
        }
        Command::StopNavigation => {
            crate::nav::handle_command(crate::nav::NavCommand::Stop);
        }
        Command::FollowPlayer {
            config,
            anchor_x,
            anchor_y,
            anchor_z,
        } => {
            tracing::info!(
                leader = %config.leader_name,
                follow_dist = config.follow_distance,
                leash_dist = config.leash_distance,
                "FollowPlayer received"
            );
            let anchor = textquest_common::nav::Waypoint::new(anchor_x, anchor_y, anchor_z);
            crate::nav::handle_command(crate::nav::NavCommand::FollowPlayer { config, anchor });
        }
        Command::UpdateFollowAnchor { x, y, z } => {
            let anchor = textquest_common::nav::Waypoint::new(x, y, z);
            crate::nav::handle_command(crate::nav::NavCommand::UpdateFollowAnchor(anchor));
        }
        Command::StopFollow => {
            tracing::info!("StopFollow received");
            crate::nav::handle_command(crate::nav::NavCommand::StopFollow);
        }
        Command::StickTo { config } => {
            tracing::info!(
                hold = config.hold,
                always = config.always,
                id = config.id,
                distance_mod = config.distance_mod,
                "StickTo received"
            );
            // Read current target ID to support the `hold` modifier — when `hold` is
            // set, we lock onto whichever spawn is targeted at the moment stick starts.
            // We read it unconditionally here (it's a cheap pointer deref) and pass it
            // to the stick engine, which ignores it when an explicit `id` is configured.
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
            let current_target_id = if eq_base != 0 {
                read_target_state(eq_base).map(|t| t.spawn_id)
            } else {
                None
            };
            crate::nav::handle_command(crate::nav::NavCommand::StickTo {
                config,
                current_target_id,
            });
        }
        Command::StickOff => {
            tracing::info!("StickOff received");
            crate::nav::handle_command(crate::nav::NavCommand::StickOff);
        }
        Command::StickMod { delta } => {
            tracing::info!(delta, "StickMod received");
            crate::nav::handle_command(crate::nav::NavCommand::StickMod(delta));
        }
        Command::QueryZoneGraph => {
            tracing::info!("QueryZoneGraph received");
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Relaxed);
            let response = unsafe { crate::nav::zone_graph::read_zone_graph(eq_base) }.map_or_else(
                || textquest_common::ipc::Response::Error {
                    message: "Failed to read zone graph from memory".into(),
                },
                |graph| {
                    let zones = crate::nav::zone_graph::zone_graph_to_ipc(&graph);
                    textquest_common::ipc::Response::ZoneGraph { zones }
                },
            );
            crate::ipc::send_response(response);
        }
        Command::Ping => {
            tracing::info!("Ping received");
        }
        Command::StartLogin {
            account_name,
            password,
            server_name,
            character_name,
        } => {
            // NOTE: Currently unreachable — handle_immediate_command() in ipc/mod.rs
            // intercepts StartLogin before it reaches PENDING_COMMANDS. The immediate
            // handler uses WM_CHAR typing which works at the login screen (before game
            // loop runs). This FSM path is preserved for future server/char selection.
            tracing::info!(
                account = %account_name,
                server = %server_name,
                character = %character_name,
                "StartLogin via game loop FSM (password redacted)"
            );
            crate::login::start_login(account_name, password, server_name, character_name);
        }
        Command::LoginPhaseQuery => {
            let phase = crate::login::phase();
            crate::ipc::send_response(textquest_common::ipc::Response::LoginPhaseUpdate { phase });
        }
        Command::CalibrateLogin => {
            tracing::info!("CalibrateLogin command received — dumping login pointers");
            let eqmain_base = crate::login::eqmain::find_eqmain();
            if eqmain_base == 0 {
                tracing::warn!("CalibrateLogin: eqmain.dll not loaded yet");
            } else {
                crate::login::widgets::calibrate_login_dump(eqmain_base);
            }
        }
        Command::CombatEngage { target_id } => {
            tracing::info!(target_id, "CombatEngage received");
            crate::combat::handle_command(crate::combat::CombatCommand::Engage { target_id });
        }
        Command::CombatDisengage => {
            tracing::info!("CombatDisengage received");
            crate::combat::handle_command(crate::combat::CombatCommand::Disengage);
        }
        Command::CombatSetAssistTarget { spawn_id } => {
            tracing::info!(spawn_id, "CombatSetAssistTarget received");
            crate::combat::handle_command(crate::combat::CombatCommand::SetAssistTarget {
                spawn_id,
            });
        }
        Command::LootCorpse => {
            tracing::info!("LootCorpse received");
            crate::combat::loot::loot_nearest_corpse();
        }
        Command::LootAll => {
            tracing::info!("LootAll received");
            crate::combat::loot::loot_all_items();
        }
        Command::SetAutoAccept { enabled } => {
            tracing::info!(enabled, "SetAutoAccept received");
            crate::dialog::set_enabled(enabled);
        }
        Command::SetRenderMode { mode } => {
            tracing::info!(%mode, "SetRenderMode received");
            crate::hooks::render::set_mode(mode);
            crate::ipc::send_response(textquest_common::ipc::Response::RenderModeChanged { mode });
        }
        Command::Eject => {
            tracing::info!("Eject command received — shutting down");
            crate::graceful_shutdown();
        }
        Command::CastSpell {
            spell_slot,
            target_id,
        } => {
            tracing::info!(spell_slot, ?target_id, "CastSpell received");
            if let Some(tid) = target_id {
                // Save → switch → cast → restore pattern (MQ2Cast style).
                // Queue the target switch, cast, and restore as slash commands
                // so they execute in order on successive game frames.
                queue_slash_command(format!("/target id {tid}"));
                queue_slash_command(format!("/cast {spell_slot}"));
                // Note: target restore after cast completion is the orchestrator's
                // responsibility — it knows who the original target was and can
                // send a follow-up /target command when the cast finishes.
            } else {
                // Cast on current target, no swap needed.
                queue_slash_command(format!("/cast {spell_slot}"));
            }
        }
        Command::InteractTarget => {
            tracing::info!("InteractTarget received — right-clicking current target");
            interact_with_target();
        }
        Command::Relog {
            account_name,
            password,
            server_name,
            character_name,
            config,
        } => {
            tracing::info!(
                account = %account_name,
                server = %server_name,
                character = %character_name,
                "Relog command received (password redacted)"
            );
            crate::login::start_relog(
                account_name,
                password,
                server_name,
                character_name,
                config,
            );
        }
        Command::CancelRelog => {
            tracing::info!("CancelRelog received");
            crate::login::cancel_relog();
        }
        Command::SwitchServer {
            server_name,
            character_name,
            account_name,
            password,
        } => {
            tracing::info!(
                server = %server_name,
                character = %character_name,
                "SwitchServer command received (password redacted)"
            );
            crate::login::switch_server(server_name, character_name, account_name, password);
        }
        Command::SwitchCharacter { character_name } => {
            tracing::info!(character = %character_name, "SwitchCharacter received");
            crate::login::switch_character(character_name);
        }
        other => {
            tracing::debug!(?other, "Unhandled command");
        }
    }
}

/// Call `CEverQuest::RightClickedOnPlayer(target, 0)` to open NPC interaction windows.
fn interact_with_target() {
    #[cfg(windows)]
    {
        use textquest_common::offsets;

        /// Rebase an offset and read the pointer it points to, returning `None`
        /// if the rebase fails or the stored pointer is null.
        unsafe fn rebase_read_ptr(offset: u64, eq_base: u64) -> Option<usize> {
            let addr = offsets::rebase(offset, eq_base)?;
            let ptr = unsafe { std::ptr::read(addr as *const usize) };
            if ptr == 0 { None } else { Some(ptr) }
        }

        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        if eq_base == 0 {
            tracing::warn!("InteractTarget: EQ base not resolved");
            return;
        }

        let Some(eq_inst) = (unsafe { rebase_read_ptr(offsets::PINST_EVERQUEST, eq_base) }) else {
            tracing::warn!("InteractTarget: pinstCEverQuest not available");
            return;
        };
        let Some(target_ptr) = (unsafe { rebase_read_ptr(offsets::PINST_TARGET, eq_base) }) else {
            tracing::warn!("InteractTarget: no target selected");
            return;
        };
        let Some(func_addr) = offsets::rebase(offsets::RIGHT_CLICKED_ON_PLAYER, eq_base) else {
            tracing::warn!("InteractTarget: rebase RIGHT_CLICKED_ON_PLAYER failed");
            return;
        };

        type RightClickFn = unsafe extern "C" fn(this: usize, target: usize, unknown: i32);
        let func: RightClickFn = unsafe { std::mem::transmute(func_addr) };
        unsafe { func(eq_inst, target_ptr, 0) };

        tracing::info!(
            eq_inst = format!("{eq_inst:#x}"),
            target = format!("{target_ptr:#x}"),
            "RightClickedOnPlayer called"
        );
    }
    #[cfg(not(windows))]
    tracing::trace!("InteractTarget (stub)");
}

/// Call EQ's `InterpretCmd` to execute a slash command string.
/// `CEverQuest::InterpretCmd` is a member function:
///   void `CEverQuest::InterpretCmd(PlayerClient`* pChar, const char* szCmd)
/// On x64 Windows: this=RCX (`CEverQuest`*), pChar=RDX, szCmd=R8.
fn execute_slash_command(command: &str) {
    #[cfg(windows)]
    {
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        if eq_base == 0 {
            tracing::error!("Cannot execute slash command — EQ base not resolved");
            return;
        }

        // Get the CEverQuest instance pointer (this).
        let Some(eq_inst_addr) =
            textquest_common::offsets::rebase(textquest_common::offsets::PINST_CEVERQUEST, eq_base)
        else {
            tracing::error!("Failed to rebase PINST_CEVERQUEST");
            return;
        };

        let eq_inst: *mut core::ffi::c_void =
            unsafe { *(eq_inst_addr as *const *mut core::ffi::c_void) };

        if eq_inst.is_null() {
            tracing::error!("CEverQuest instance pointer is null");
            return;
        }

        // Get the local player pointer (pChar).
        let Some(char_spawn_addr) = textquest_common::offsets::rebase(
            textquest_common::offsets::PINST_LOCAL_PLAYER,
            eq_base,
        ) else {
            tracing::error!("Failed to rebase PINST_LOCAL_PLAYER");
            return;
        };

        let player_ptr: *mut core::ffi::c_void =
            unsafe { *(char_spawn_addr as *const *mut core::ffi::c_void) };

        if player_ptr.is_null() {
            tracing::error!("Local player pointer is null — not logged in?");
            return;
        }

        // Get InterpretCmd function address.
        let Some(interpret_addr) =
            textquest_common::offsets::rebase(textquest_common::offsets::INTERPRET_CMD, eq_base)
        else {
            tracing::error!("Failed to rebase INTERPRET_CMD");
            return;
        };

        // Build null-terminated command string.
        let cmd_cstring = match std::ffi::CString::new(command) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "Invalid command string");
                return;
            }
        };

        // CEverQuest::InterpretCmd(this, PlayerClient*, const char*)
        // x64 Windows: this=RCX, pChar=RDX, szCmd=R8
        type InterpretCmdFn = unsafe extern "C" fn(
            *mut core::ffi::c_void, // this (CEverQuest*)
            *mut core::ffi::c_void, // pChar (PlayerClient*)
            *const i8,              // szCmd
        );
        let interpret_cmd: InterpretCmdFn = unsafe { std::mem::transmute(interpret_addr) };

        tracing::info!(
            addr = format!("{:#x}", interpret_addr),
            eq_inst = format!("{:?}", eq_inst),
            player = format!("{:?}", player_ptr),
            cmd = command,
            "Calling InterpretCmd"
        );

        unsafe {
            interpret_cmd(eq_inst, player_ptr, cmd_cstring.as_ptr());
        }

        tracing::info!(cmd = command, "Slash command executed");
    }

    #[cfg(not(windows))]
    {
        let _ = command;
        tracing::warn!("Slash command execution not available on this platform");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_human_jitter_bounds() {
        let mut rng = textquest_common::nav::Xorshift32::new(12345);
        for _ in 0..10_000 {
            let ticks = human_jitter_ticks(&mut rng);
            // Base: 2-10, hesitation adds 5-19 (5% chance)
            // Min = 2, max = 10 + 19 = 29
            assert!(ticks >= 2, "jitter {ticks} below minimum 2");
            assert!(ticks <= 29, "jitter {ticks} above maximum 29");
        }
    }

    #[test]
    fn test_human_jitter_distribution_peaks_at_center() {
        let mut rng = textquest_common::nav::Xorshift32::new(42);
        let mut counts = [0u32; 30]; // indices 0-29

        for _ in 0..100_000 {
            let ticks = human_jitter_ticks(&mut rng) as usize;
            counts[ticks] += 1;
        }

        // The triangle distribution peaks at 6 (most common without hesitation).
        // Verify tick 6 is the most common value in the 2-10 range.
        let peak = counts[2..=10]
            .iter()
            .enumerate()
            .max_by_key(|(_, c)| *c)
            .map(|(i, _)| i + 2)
            .unwrap();
        assert_eq!(peak, 6, "Expected peak at 6 ticks, got {peak}");
    }

    #[test]
    fn test_human_jitter_produces_hesitation_spikes() {
        let mut rng = textquest_common::nav::Xorshift32::new(99);
        let mut saw_spike = false;
        for _ in 0..10_000 {
            let ticks = human_jitter_ticks(&mut rng);
            if ticks > 10 {
                saw_spike = true;
                break;
            }
        }
        assert!(
            saw_spike,
            "Expected at least one hesitation spike > 10 in 10000 draws"
        );
    }
}
