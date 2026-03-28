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

/// Called every game tick after the original MainLoop runs.
/// This is our main entry point for per-tick logic.
fn on_game_tick() {
    let tick = TICK_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    // Check foreground status every 30 ticks (~1 second) to minimize overhead.
    if tick % 30 == 0 {
        update_foreground_status();
    }

    // Run navigation state machine.
    crate::nav::tick();

    // TODO: Read game state from EQ memory (local player, target, spawns)
    // TODO: Publish state to shared memory via IPC
    // TODO: Check for and execute pending commands from the orchestrator

    // Combat FSM tick — runs after nav, before IPC publish.
    // Uncomment when we have a real game state snapshot:
    //
    //   if let Some(ref player) = game_state.local_player {
    //       crate::combat::tick(player, game_state.target.as_ref(), &game_state.nearby_spawns);
    //   }
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
