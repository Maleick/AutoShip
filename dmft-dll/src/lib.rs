//! DMFT injected DLL payload.
//! This cdylib is loaded into eqgame.exe via CreateRemoteThread + LoadLibrary.
//! It hooks internal EQ functions and communicates with the DMFT orchestrator via IPC.

// All DLL modules are Windows-only at runtime (cdylib loaded into eqgame.exe).
// On macOS they compile with stubs but nothing calls into them, so suppress
// dead_code warnings per-module rather than crate-wide.
#[allow(dead_code)]
mod combat;
#[allow(dead_code)]
mod eq;
#[allow(dead_code)]
mod hooks;
#[allow(dead_code)]
mod ipc;
#[allow(dead_code)]
mod login;
#[allow(dead_code)]
mod nav;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Base address of eqgame.exe in memory. Set during initialization.
/// All EQ offsets are added to this value to compute runtime addresses.
pub static EQ_BASE: AtomicU64 = AtomicU64::new(0);

/// Global flag indicating the DLL is shutting down.
/// Checked by long-running loops (IPC listener, nav ticks) to exit gracefully.
pub static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
mod dll_main {
    use windows::Win32::Foundation::{BOOL, HMODULE, TRUE};
    use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
    use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
    use windows::Win32::System::Threading::CreateThread;

    /// Thread procedure for `CreateThread`. Must match the `LPTHREAD_START_ROUTINE`
    /// signature: `extern "system" fn(*mut c_void) -> u32`.
    unsafe extern "system" fn init_thread(_param: *mut core::ffi::c_void) -> u32 {
        if let Err(e) = super::initialize() {
            tracing::error!("DMFT DLL initialization failed: {}", e);
        }
        0
    }

    /// DLL entry point. Called by Windows when the DLL is loaded/unloaded.
    /// IMPORTANT: DllMain runs under the loader lock — keep work minimal.
    /// We use `CreateThread` (not `thread::spawn`) because `std::thread::spawn`
    /// internally calls `CreateThread` *and* may acquire internal locks that
    /// can deadlock under the loader lock.
    #[unsafe(no_mangle)]
    pub extern "system" fn DllMain(
        module: HMODULE,
        reason: u32,
        _reserved: *mut core::ffi::c_void,
    ) -> BOOL {
        match reason {
            DLL_PROCESS_ATTACH => {
                unsafe {
                    // Suppress DLL_THREAD_ATTACH/DETACH notifications for perf.
                    let _ = DisableThreadLibraryCalls(module);

                    // Use raw CreateThread to avoid std runtime under loader lock.
                    let _ = CreateThread(
                        None,
                        0,
                        Some(init_thread),
                        None,
                        Default::default(),
                        None,
                    );
                }
                TRUE
            }
            DLL_PROCESS_DETACH => {
                super::shutdown();
                TRUE
            }
            _ => TRUE,
        }
    }
}

/// Initialize the DMFT DLL after injection.
/// Called from a spawned thread (NOT under loader lock).
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn initialize() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Set up tracing — write logs to a file since we have no console.
    init_tracing();
    tracing::info!("DMFT DLL initializing (pid={})", std::process::id());

    // 2. Resolve EQ base address.
    let eq_base = resolve_eq_base();
    EQ_BASE.store(eq_base, Ordering::Release);
    tracing::info!(base = format!("{:#x}", eq_base), "EQ base address resolved");

    // 3. Install function hooks (non-fatal if they fail).
    if let Err(e) = install_hooks(eq_base) {
        tracing::warn!("Hook installation failed (continuing without hooks): {}", e);
    }

    // 4. Initialize command jitter RNG for anti-detection.
    hooks::game_loop::init_jitter_rng();

    // 4.5. Initialize login FSM.
    login::init();

    // 5. Start IPC listener.
    let client_id = std::process::id();
    let session_token = generate_session_token(client_id);
    if let Err(e) = ipc::start(client_id, session_token) {
        tracing::warn!("IPC startup failed (continuing without IPC): {}", e);
    }

    tracing::info!("DMFT DLL initialized successfully");
    Ok(())
}

/// Initialize tracing with file output. Falls back silently if setup fails —
/// better to run without logs than crash EQ.
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    use tracing_appender::rolling;

    let log_dir = std::env::temp_dir().join("dmft");
    std::fs::create_dir_all(&log_dir).ok();
    let file_appender = rolling::RollingFileAppender::builder()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix("dmft-dll.log")
        .max_log_files(7) // Keep 1 week of logs
        .build(&log_dir)
        .unwrap_or_else(|_| rolling::daily(&log_dir, "dmft-dll.log"));
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Intentionally leak the guard so it lives for the DLL's lifetime — there is
    // no clean drop point for a cdylib that outlives its init thread.
    // Box::leak is preferred over mem::forget as it makes the intent explicit.
    Box::leak(Box::new(_guard));

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    tracing::info!("DMFT DLL tracing initialized");
}

/// Resolve the base address of eqgame.exe in the current process.
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn resolve_eq_base() -> u64 {
    #[cfg(windows)]
    {
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;

        // GetModuleHandleW(None) returns the base of the hosting exe (eqgame.exe).
        unsafe {
            GetModuleHandleW(None)
                .map(|h| h.0 as u64)
                .unwrap_or(0)
        }
    }

    #[cfg(not(windows))]
    {
        // Preferred base address for eqgame.exe — used for macOS stub builds.
        0x140000000
    }
}

/// Install all function hooks using the resolved EQ base address.
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn install_hooks(eq_base: u64) -> Result<(), Box<dyn std::error::Error>> {
    // Primary: use the offset constant derived from PROCESS_GAME_EVENTS.
    let main_loop_offset = eq::MAIN_LOOP_OFFSET;
    if main_loop_offset == 0 {
        tracing::warn!("Game loop hook SKIPPED — MAIN_LOOP_OFFSET is still 0x0!");
        return Ok(());
    }

    let main_loop_addr = eq_base as usize + main_loop_offset;

    // Cross-check against dmft_common offsets via rebase.
    if let Some(expected) = dmft_common::offsets::rebase(
        dmft_common::offsets::PROCESS_GAME_EVENTS,
        eq_base,
    ) {
        if main_loop_addr != expected {
            tracing::warn!(
                computed = format!("{:#x}", main_loop_addr),
                expected = format!("{:#x}", expected),
                "MAIN_LOOP_OFFSET disagrees with offsets::PROCESS_GAME_EVENTS — using offsets rebase"
            );
            hooks::game_loop::install(expected)?;
        } else {
            hooks::game_loop::install(main_loop_addr)?;
        }
    } else {
        hooks::game_loop::install(main_loop_addr)?;
    }

    // Install render strobe hook -- background clients skip 3D rendering.
    if let Some(render_addr) = dmft_common::offsets::rebase(
        dmft_common::offsets::REAL_RENDER_WORLD,
        eq_base,
    ) {
        if let Err(e) = hooks::render::install(render_addr) {
            tracing::warn!("Render hook failed (continuing without render strobe): {}", e);
        }
    } else {
        tracing::warn!("Could not rebase REAL_RENDER_WORLD -- render strobe disabled");
    }

    Ok(())
}

/// Read the session token injected by the orchestrator.
///
/// The orchestrator writes a 32-byte CSPRNG token to `%TEMP%/dmft/token_{pid}.bin`
/// before injection. The DLL reads it once during init and deletes the file.
/// Falls back to a PID-derived token with a warning if the file is missing (e.g.
/// during development or manual injection).
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn generate_session_token(pid: u32) -> dmft_common::ipc::SessionToken {
    let token_path = std::env::temp_dir()
        .join("dmft")
        .join(format!("token_{}.bin", pid));

    if let Ok(data) = std::fs::read(&token_path) {
        // Clean up — token is single-use
        let _ = std::fs::remove_file(&token_path);

        if data.len() == 32 {
            let mut token = [0u8; 32];
            token.copy_from_slice(&data);
            tracing::info!("Loaded CSPRNG session token from orchestrator");
            return token;
        }
        tracing::warn!(
            len = data.len(),
            "Token file has wrong size — falling back to PID-derived token"
        );
    } else {
        tracing::warn!(
            "No orchestrator token file at {} — generating random fallback token",
            token_path.display()
        );
    }

    // Random fallback: use OS entropy (getrandom) so the token is not predictable
    // even without the orchestrator's token file (e.g. during manual injection).
    let mut token = [0u8; 32];
    if getrandom::getrandom(&mut token).is_err() {
        // getrandom itself failed — extremely unlikely on any supported Windows version.
        // Log prominently and fall back to a PID-mixed value as absolute last resort.
        tracing::error!(
            "getrandom failed — session token entropy is degraded (should never happen)"
        );
        let pid_bytes = pid.to_le_bytes();
        for (i, byte) in token.iter_mut().enumerate() {
            *byte = pid_bytes[i % 4] ^ (i as u8 ^ 0xA5);
        }
    }
    token
}

/// Signal shutdown. Called from `DLL_PROCESS_DETACH` under loader lock, so this
/// must be minimal — just set the flag. Heavy cleanup (hook removal, IPC close)
/// is done by `graceful_shutdown()` via the eject command path BEFORE
/// `DLL_PROCESS_DETACH` fires. Do NOT do I/O or acquire locks here.
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn shutdown() {
    SHUTTING_DOWN.store(true, Ordering::SeqCst);
}

/// Full cleanup — call from the eject command handler, NOT from DLL_PROCESS_DETACH.
/// This runs outside the loader lock so it's safe to do I/O, remove hooks, etc.
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn graceful_shutdown() {
    SHUTTING_DOWN.store(true, Ordering::SeqCst);
    hooks::remove_all();
    ipc::stop();
    tracing::info!("DMFT DLL graceful shutdown complete");
}
