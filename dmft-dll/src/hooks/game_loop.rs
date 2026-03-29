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

    // Drain and dispatch IPC commands from the orchestrator.
    for cmd in crate::ipc::poll_commands() {
        dispatch_command(cmd);
    }

    // Run navigation state machine.
    crate::nav::tick();

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
