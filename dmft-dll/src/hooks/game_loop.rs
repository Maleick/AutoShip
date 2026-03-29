//! Game loop hook -- intercepts CEverQuest::MainLoop.
//! Runs our logic every game tick after the original function completes.

#[cfg(windows)]
mod inner {
    use retour::static_detour;

    // CEverQuest::MainLoop function signature (void, no params, thiscall).
    // On x86_64 Windows thiscall is the default convention, so we use
    // "system" which resolves to stdcall on x86 and the MS x64 ABI on
    // x86_64 -- both compatible with thiscall for single-pointer-arg
    // member functions.
    type MainLoopFn = unsafe extern "system" fn(*mut core::ffi::c_void);

    static_detour! {
        static MainLoopHook: unsafe extern "system" fn(*mut core::ffi::c_void);
    }

    /// The detour function -- called instead of the original MainLoop.
    fn main_loop_detour(this: *mut core::ffi::c_void) {
        // Call original first -- let EQ process normally.
        unsafe {
            MainLoopHook.call(this);
        }

        // Now run our per-tick logic.
        super::on_game_tick();
    }

    /// Install the game loop hook.
    pub fn install(main_loop_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            let target: MainLoopFn = std::mem::transmute(main_loop_addr);
            MainLoopHook.initialize(target, main_loop_detour)?;
            MainLoopHook.enable()?;
        }
        tracing::info!(addr = format!("{:#x}", main_loop_addr), "Game loop hook installed");
        Ok(())
    }

    /// Remove the game loop hook.
    pub fn remove() {
        unsafe {
            if MainLoopHook.is_enabled() {
                let _ = MainLoopHook.disable();
            }
        }
        tracing::info!("Game loop hook removed");
    }
}

#[cfg(not(windows))]
mod inner {
    /// Stub -- hooks are only functional on Windows.
    pub fn install(_main_loop_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
        tracing::warn!("Game loop hook not available on this platform (stub)");
        Ok(())
    }

    /// Stub -- nothing to remove on non-Windows platforms.
    pub fn remove() {
        tracing::warn!("Game loop hook removal not available (stub)");
    }
}

pub use inner::{install, remove};

/// Track whether this window is in the foreground for render skipping.
/// When false, we can tell EQ to skip 3D rendering (near-zero GPU for
/// background clients). Game logic still runs at full speed.
static WINDOW_IS_FOREGROUND: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

/// Track tick count for throttling background checks.
static TICK_COUNT: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Pending login button click — set by IPC thread, executed on game loop thread.
/// Contains the CXWnd* address of the button to click, or 0 if none pending.
static PENDING_BUTTON_CLICK: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Pending Enter World sequence — set by IPC thread, executed on game loop thread.
/// Stage 0 = idle, 1 = SelectCharacter pending, 2 = waiting, 3 = EnterWorld pending.
static PENDING_ENTER_WORLD_WND: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
/// The rebased EnterWorld function address.
static PENDING_ENTER_WORLD_FN: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
/// The rebased SelectCharacter function address.
static PENDING_SELECT_CHAR_FN: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
/// Enter World sequence stage (0=idle, 1=select, 2=wait, 3=enter).
static ENTER_WORLD_STAGE: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);
/// Tick at which to advance from stage 2→3 (wait before EnterWorld).
static ENTER_WORLD_WAIT_UNTIL: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Set a button widget address to be clicked on the next game loop tick.
/// Called from the IPC thread after writing credentials.
pub fn queue_button_click(button_wnd: usize) {
    PENDING_BUTTON_CLICK.store(button_wnd, std::sync::atomic::Ordering::Release);
}

/// Queue a SelectCharacter(0) → EnterWorld() sequence on the game loop thread.
/// Called from the IPC thread during Phase 3 of login chain.
/// The game loop will: (1) call SelectCharacter(0), (2) wait ~90 ticks (~3s),
/// (3) call EnterWorld(). Both calls happen on the game loop thread.
pub fn queue_enter_world(char_list_wnd: usize, enter_world_fn: usize) {
    // Also resolve SelectCharacter address
    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    let select_fn = dmft_common::offsets::rebase(
        dmft_common::offsets::SELECT_CHARACTER, eq_base,
    ).unwrap_or(0);

    // Store function addresses and window handle first (Relaxed is sufficient),
    // then store the stage flag last with Release ordering as the "commit" signal.
    // The reader's Acquire load of ENTER_WORLD_STAGE establishes happens-before
    // for all prior stores, guaranteeing the addresses are visible.
    PENDING_SELECT_CHAR_FN.store(select_fn, std::sync::atomic::Ordering::Relaxed);
    PENDING_ENTER_WORLD_FN.store(enter_world_fn, std::sync::atomic::Ordering::Relaxed);
    PENDING_ENTER_WORLD_WND.store(char_list_wnd, std::sync::atomic::Ordering::Relaxed);
    ENTER_WORLD_STAGE.store(1, std::sync::atomic::Ordering::Release);
}

/// Re-scan CXWndManager for CCharacterListWnd by SidlText.
/// Used in Stage 3 to validate the pointer is still valid before calling EnterWorld.
#[cfg(windows)]
fn rescan_char_list_wnd() -> Option<usize> {
    use dmft_common::offsets::eqgame as eqg;

    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    let mgr_ptr_addr = dmft_common::offsets::rebase(
        dmft_common::offsets::PINST_CXWND_MANAGER, eq_base,
    )?;

    unsafe {
        let mgr = *(mgr_ptr_addr as *const usize);
        if mgr == 0 { return None; }

        let array_ptr = *((mgr + eqg::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
        let count = *((mgr + eqg::CXWNDMGR_WINDOWS_COUNT) as *const u32);
        if array_ptr == 0 || count == 0 || count > 2000 { return None; }

        for i in 0..count as usize {
            let wnd_ptr = *((array_ptr + i * 8) as *const usize);
            if wnd_ptr == 0 { continue; }

            if let Some(sidl_text) = crate::eq::widgets::read_cxstr(
                wnd_ptr + eqg::CSIDL_SCREEN_WND_SIDL_TEXT,
            ) {
                if sidl_text == "CharacterListWnd" {
                    return Some(wnd_ptr);
                }
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn rescan_char_list_wnd() -> Option<usize> { None }

// ─── Command Jitter Queue ───
// Commands are not executed immediately — they sit in a pending queue
// with a random delay of 1-10 ticks to avoid frame-perfect timing patterns.

use std::sync::Mutex;

struct PendingCommand {
    command: dmft_common::ipc::Command,
    execute_at_tick: u64,
}

static PENDING_COMMANDS: Mutex<Vec<PendingCommand>> = Mutex::new(Vec::new());
static JITTER_RNG: Mutex<Option<dmft_common::nav::Xorshift32>> = Mutex::new(None);

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
        *rng = Some(dmft_common::nav::Xorshift32::new(seed));
    }
}

/// Human-like jitter using a triangle distribution (sum of two uniform draws).
/// Peaks at 6 ticks with occasional hesitation spikes simulating distraction.
/// Range: 2-30 ticks (without hesitation: 2-10, with hesitation: 7-30).
fn human_jitter_ticks(rng: &mut dmft_common::nav::Xorshift32) -> u64 {
    // Triangle distribution: sum of two uniform draws (peaks at center)
    let base = (rng.next_u32() % 5 + 1) + (rng.next_u32() % 5 + 1); // 2-10, peaks at 6
    // 5% chance of hesitation spike (simulates distraction)
    let hesitate = if rng.next_u32().is_multiple_of(20) {
        rng.next_u32() % 15 + 5
    } else {
        0
    };
    (base + hesitate) as u64
}

/// Queue a slash command for execution on the next game loop tick.
/// Safe to call from any thread — the game loop will pick it up.
pub fn queue_slash_command(command: String) {
    if let Ok(mut queue) = PENDING_COMMANDS.lock() {
        queue.push(PendingCommand {
            command: dmft_common::ipc::Command::SlashCommand { command },
            execute_at_tick: 0, // execute immediately on next tick
        });
    }
}

/// Enqueue a command with a human-like jitter delay.
fn enqueue_command(cmd: dmft_common::ipc::Command, current_tick: u64) {
    let delay = if let Ok(mut rng) = JITTER_RNG.lock() {
        if let Some(ref mut r) = *rng {
            human_jitter_ticks(r)
        } else {
            5 // fallback: middle of range
        }
    } else {
        5
    };

    if let Ok(mut queue) = PENDING_COMMANDS.lock() {
        queue.push(PendingCommand {
            command: cmd,
            execute_at_tick: current_tick + delay,
        });
    }
}

/// Drain and execute any commands whose scheduled tick has arrived.
fn process_pending_commands(current_tick: u64) {
    let ready: Vec<dmft_common::ipc::Command> = if let Ok(mut queue) = PENDING_COMMANDS.lock() {
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
        ready
    } else {
        return;
    };

    for cmd in ready {
        dispatch_command(cmd);
    }
}

/// Called every game tick after the original MainLoop runs.
/// This is our main entry point for per-tick logic.
fn on_game_tick() {
    let tick = TICK_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    // Check foreground status every 30 ticks (~1 second) to minimize overhead.
    if tick.is_multiple_of(30) {
        update_foreground_status();
    }

    // Rename window every 100 ticks (~3 seconds) to "[DMFT] EQ - CharName (ZoneName)".
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
        tracing::info!(
            ptr = format!("{:#x}", button_addr),
            "Clicking login button on game loop thread"
        );
        unsafe {
            crate::eq::widgets::click_button_via_vtable(button_addr);
        }
    }

    // Process Enter World sequence (SelectCharacter → wait → EnterWorld).
    // Stage 1: Call SelectCharacter(0)
    // Stage 2: Wait ~90 ticks (~3 seconds)
    // Stage 3: Call EnterWorld()
    let stage = ENTER_WORLD_STAGE.load(std::sync::atomic::Ordering::Acquire);
    if stage == 1 {
        let wnd = PENDING_ENTER_WORLD_WND.load(std::sync::atomic::Ordering::Acquire);
        let select_fn = PENDING_SELECT_CHAR_FN.load(std::sync::atomic::Ordering::Acquire);
        if wnd != 0 && select_fn != 0 {
            tracing::info!(
                wnd = format!("{:#x}", wnd),
                func = format!("{:#x}", select_fn),
                "Phase 3: Calling SelectCharacter(0) on game loop thread"
            );
            unsafe {
                // SelectCharacter(int index) — x64: RCX=this, RDX=index
                type SelectCharFn = unsafe extern "C" fn(this: usize, index: i32);
                let func: SelectCharFn = std::mem::transmute(select_fn);
                func(wnd, 0); // Select first character
            }
            tracing::info!("Phase 3: SelectCharacter(0) called — waiting 3s before EnterWorld");
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
        let wnd = rescan_char_list_wnd().unwrap_or_else(|| {
            // Fall back to stored pointer if re-scan fails
            PENDING_ENTER_WORLD_WND.load(std::sync::atomic::Ordering::Acquire)
        });
        if wnd != 0 && enter_fn != 0 {
            tracing::info!(
                wnd = format!("{:#x}", wnd),
                func = format!("{:#x}", enter_fn),
                "Phase 3: Calling EnterWorld() on game loop thread (re-validated)"
            );
            unsafe {
                type EnterWorldFn = unsafe extern "C" fn(this: usize);
                let func: EnterWorldFn = std::mem::transmute(enter_fn);
                func(wnd);
            }
            tracing::info!("Phase 3: EnterWorld() called — entering world!");
        }
        // Reset all state
        ENTER_WORLD_STAGE.store(0, std::sync::atomic::Ordering::Release);
        PENDING_ENTER_WORLD_WND.store(0, std::sync::atomic::Ordering::Release);
        PENDING_ENTER_WORLD_FN.store(0, std::sync::atomic::Ordering::Release);
        PENDING_SELECT_CHAR_FN.store(0, std::sync::atomic::Ordering::Release);
    }

    // Run navigation state machine.
    crate::nav::tick();

    // Run login FSM when not yet in world (local_player is null).
    // The login FSM drives credential entry, server/char selection autonomously.
    {
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        let in_world = if eq_base != 0 {
            dmft_common::offsets::rebase(dmft_common::offsets::PINST_LOCAL_PLAYER, eq_base)
                .map(|addr| unsafe { *(addr as *const usize) } != 0)
                .unwrap_or(false)
        } else {
            false
        };

        if !in_world
            && let Some(phase) = crate::login::tick()
        {
            crate::ipc::send_response(dmft_common::ipc::Response::LoginPhaseUpdate { phase });
        }
    }

    // Read game state and publish to shared memory for the orchestrator.
    read_and_publish_state(tick);
}

// ─── Game State Reading ───
// Reads EQ memory directly (we're in-process) and publishes to shared memory.

/// Read game state from EQ memory and publish to shared memory each tick.
/// Local player + target are read every tick (fast — just pointer derefs).
/// Nearby spawns are read every 30 ticks (~1 second) to reduce overhead.
///
/// Uses a cached GameState to avoid cloning ~100 SpawnData (each with 2 String
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
    static CACHED_STATE: Mutex<Option<dmft_common::types::GameState>> = Mutex::new(None);

    let Ok(mut cached) = CACHED_STATE.lock() else {
        return;
    };

    let refresh_spawns = tick.is_multiple_of(30) || cached.is_none();

    if refresh_spawns {
        let player = local_player.as_ref().unwrap();
        let spawns = read_nearby_spawns(eq_base, player.x, player.y, player.z);
        *cached = Some(dmft_common::types::GameState {
            client_id: std::process::id(),
            local_player,
            target,
            nearby_spawns: spawns,
            timestamp_ms: current_time_ms(),
            nav_status: crate::nav::status(),
            combat_status: crate::combat::status(),
        });
    } else {
        let state = cached.as_mut().unwrap();
        state.local_player = local_player;
        state.target = target;
        state.timestamp_ms = current_time_ms();
        state.nav_status = crate::nav::status();
        state.combat_status = crate::combat::status();
    }

    crate::ipc::publish_state(cached.as_ref().unwrap());
}

/// Read a null-terminated string from an in-process address. Max `max_len` bytes.
///
/// # Safety
/// Caller must ensure `addr` points to readable memory of at least `max_len` bytes.
unsafe fn read_string_at(addr: usize, max_len: usize) -> String {
    if addr == 0 {
        return String::new();
    }
    let bytes = unsafe { std::slice::from_raw_parts(addr as *const u8, max_len) };
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(max_len);
    String::from_utf8_lossy(&bytes[..len]).into_owned()
}

/// Build a `SpawnData` from a PlayerClient pointer (in-process direct read).
///
/// # Safety
/// Caller must ensure `spawn_ptr` is a valid PlayerClient address.
unsafe fn read_spawn_data(spawn_ptr: usize) -> dmft_common::types::SpawnData {
    use dmft_common::offsets::{player_base, player_zone};

    let name = unsafe { read_string_at(spawn_ptr + player_base::NAME, 64) };
    let displayed_name = unsafe { read_string_at(spawn_ptr + player_base::DISPLAYED_NAME, 64) };
    let spawn_id = unsafe { *((spawn_ptr + player_base::SPAWN_ID) as *const u32) };
    let spawn_type = unsafe { *((spawn_ptr + player_base::TYPE) as *const u8) };
    let x = unsafe { *((spawn_ptr + player_base::X) as *const f32) };
    let y = unsafe { *((spawn_ptr + player_base::Y) as *const f32) };
    let z = unsafe { *((spawn_ptr + player_base::Z) as *const f32) };
    let heading = unsafe { *((spawn_ptr + player_base::HEADING) as *const f32) };
    let level = unsafe { *((spawn_ptr + player_zone::LEVEL) as *const u8) };
    let class_id = unsafe { *((spawn_ptr + player_zone::CHAR_CLASS) as *const u8) };
    let hp_current = unsafe { *((spawn_ptr + player_zone::HP_CURRENT) as *const i64) };
    let hp_max = unsafe { *((spawn_ptr + player_zone::HP_MAX) as *const i64) };
    let mana_current = unsafe { *((spawn_ptr + player_zone::MANA_CURRENT) as *const i32) };
    let mana_max = unsafe { *((spawn_ptr + player_zone::MANA_MAX) as *const i32) };
    let endurance_current = unsafe { *((spawn_ptr + player_zone::ENDURANCE_CURRENT) as *const i32) };
    let endurance_max = unsafe { *((spawn_ptr + player_zone::ENDURANCE_MAX) as *const u32) };

    dmft_common::types::SpawnData {
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
    }
}

/// Read local player state. Returns None if not logged in.
fn read_local_player_state(eq_base: u64) -> Option<dmft_common::types::SpawnData> {
    let player_ptr_addr = dmft_common::offsets::rebase(
        dmft_common::offsets::PINST_LOCAL_PLAYER,
        eq_base,
    )?;
    let player_ptr = unsafe { *(player_ptr_addr as *const usize) };
    if player_ptr == 0 {
        return None;
    }
    Some(unsafe { read_spawn_data(player_ptr) })
}

/// Read current target state. Returns None if no target selected.
fn read_target_state(eq_base: u64) -> Option<dmft_common::types::SpawnData> {
    let target_ptr_addr = dmft_common::offsets::rebase(
        dmft_common::offsets::PINST_TARGET,
        eq_base,
    )?;
    let target_ptr = unsafe { *(target_ptr_addr as *const usize) };
    if target_ptr == 0 {
        return None;
    }
    Some(unsafe { read_spawn_data(target_ptr) })
}

/// Walk the spawn linked list and collect spawns within `max_distance` units
/// of the given position. Capped at 100 spawns.
fn read_nearby_spawns(eq_base: u64, player_x: f32, player_y: f32, player_z: f32) -> Vec<dmft_common::types::SpawnData> {
    use dmft_common::offsets::{player_base, spawn_manager};

    const MAX_NEARBY: usize = 100;
    const MAX_DISTANCE_SQ: f32 = 500.0 * 500.0;

    let mgr_ptr_addr = match dmft_common::offsets::rebase(
        dmft_common::offsets::PINST_SPAWN_MANAGER,
        eq_base,
    ) {
        Some(addr) => addr,
        None => return Vec::new(),
    };

    let mgr_ptr = unsafe { *(mgr_ptr_addr as *const usize) };
    if mgr_ptr == 0 {
        return Vec::new();
    }

    // TList at spawn_manager::PLAYER_LIST, first node pointer at offset 0x00.
    let list_addr = mgr_ptr + spawn_manager::PLAYER_LIST;
    let mut current = unsafe { *(list_addr as *const usize) };

    let mut spawns = Vec::new();
    let mut walked: usize = 0;
    const MAX_WALK: usize = 2000; // Safety limit to prevent infinite loops.

    while current != 0 && spawns.len() < MAX_NEARBY && walked < MAX_WALK {
        walked += 1;

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

        // Follow NEXT pointer in linked list.
        current = unsafe { *((current + player_base::NEXT) as *const usize) };
    }

    spawns
}

/// Current time in milliseconds since UNIX epoch.
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Check if our window is the foreground window. Used for render skipping —
/// background clients skip 3D rendering to save GPU/CPU.
fn update_foreground_status() {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
        use windows::Win32::Foundation::HWND;

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
/// The render hook can use this to skip CDisplay::RealRender_World for
/// background clients, saving near-zero GPU usage across 35 bot clients.
pub fn is_foreground() -> bool {
    WINDOW_IS_FOREGROUND.load(std::sync::atomic::Ordering::Relaxed)
}

/// Read character name + zone name from EQ memory and set the window title
/// to "[DMFT] EQ - CharName (ZoneName)" so the orchestrator can identify clients by PID.
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
            format!("[DMFT] EQ - {}\0", char_name)
        } else {
            format!("[DMFT] EQ - {} ({})\0", char_name, zone_name)
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
    use dmft_common::offsets::{self, player_base};

    let player_ptr_addr = offsets::rebase(offsets::PINST_LOCAL_PLAYER, eq_base)?;
    let player_ptr = unsafe { *(player_ptr_addr as *const usize) };
    if player_ptr == 0 {
        return None;
    }

    let name_addr = player_ptr + player_base::NAME;
    let name_bytes = unsafe { std::slice::from_raw_parts(name_addr as *const u8, 64) };
    let len = name_bytes.iter().position(|&b| b == 0).unwrap_or(64);
    String::from_utf8(name_bytes[..len].to_vec()).ok()
}

/// Read the zone short name (char[128]) from instEQZoneInfo.
#[cfg(windows)]
fn read_zone_short_name(eq_base: u64) -> Option<String> {
    use dmft_common::offsets::zone_info;

    let zone_addr = dmft_common::offsets::rebase(zone_info::INST_EQ_ZONE_INFO, eq_base)?;
    let short_name_addr = zone_addr + zone_info::SHORT_NAME;
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
    use dmft_common::offsets::zone_info;

    let zone_addr = dmft_common::offsets::rebase(zone_info::INST_EQ_ZONE_INFO, eq_base)?;
    let long_name_addr = zone_addr + zone_info::LONG_NAME;
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
fn dispatch_command(cmd: dmft_common::ipc::Command) {
    use dmft_common::ipc::Command;

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
        Command::StopNavigation => {
            crate::nav::handle_command(crate::nav::NavCommand::Stop);
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
            crate::ipc::send_response(dmft_common::ipc::Response::LoginPhaseUpdate { phase });
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
        Command::Eject => {
            tracing::info!("Eject command received — shutting down");
            crate::graceful_shutdown();
        }
        other => {
            tracing::debug!(?other, "Unhandled command");
        }
    }
}

/// Call EQ's InterpretCmd to execute a slash command string.
/// CEverQuest::InterpretCmd is a member function:
///   void CEverQuest::InterpretCmd(PlayerClient* pChar, const char* szCmd)
/// On x64 Windows: this=RCX (CEverQuest*), pChar=RDX, szCmd=R8.
fn execute_slash_command(command: &str) {
    #[cfg(windows)]
    {
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        if eq_base == 0 {
            tracing::error!("Cannot execute slash command — EQ base not resolved");
            return;
        }

        // Get the CEverQuest instance pointer (this).
        let Some(eq_inst_addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::PINST_CEVERQUEST,
            eq_base,
        ) else {
            tracing::error!("Failed to rebase PINST_CEVERQUEST");
            return;
        };

        let eq_inst: *mut core::ffi::c_void = unsafe {
            *(eq_inst_addr as *const *mut core::ffi::c_void)
        };

        if eq_inst.is_null() {
            tracing::error!("CEverQuest instance pointer is null");
            return;
        }

        // Get the local player pointer (pChar).
        let Some(char_spawn_addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::PINST_LOCAL_PLAYER,
            eq_base,
        ) else {
            tracing::error!("Failed to rebase PINST_LOCAL_PLAYER");
            return;
        };

        let player_ptr: *mut core::ffi::c_void = unsafe {
            *(char_spawn_addr as *const *mut core::ffi::c_void)
        };

        if player_ptr.is_null() {
            tracing::error!("Local player pointer is null — not logged in?");
            return;
        }

        // Get InterpretCmd function address.
        let Some(interpret_addr) = dmft_common::offsets::rebase(
            dmft_common::offsets::INTERPRET_CMD,
            eq_base,
        ) else {
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
        let mut rng = dmft_common::nav::Xorshift32::new(12345);
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
        let mut rng = dmft_common::nav::Xorshift32::new(42);
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
        let mut rng = dmft_common::nav::Xorshift32::new(99);
        let mut saw_spike = false;
        for _ in 0..10_000 {
            let ticks = human_jitter_ticks(&mut rng);
            if ticks > 10 {
                saw_spike = true;
                break;
            }
        }
        assert!(saw_spike, "Expected at least one hesitation spike > 10 in 10000 draws");
    }
}
