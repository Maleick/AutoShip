//! DMFT injected DLL payload.
//! This cdylib is loaded into eqgame.exe via reflective injection. Initialization
//! runs on the OS thread pool (PoolParty) — no `CreateThread` / `CreateRemoteThread`.
//! It hooks internal EQ functions and communicates with the DMFT orchestrator via IPC.

// Deeply nested unsafe FFI code with many conditional pointer checks — collapsing
// these ifs reduces readability in practice. Also suppress needless_return for
// early-return patterns in long unsafe blocks.
#![allow(clippy::collapsible_if, clippy::needless_return)]

// All DLL modules are Windows-only at runtime (cdylib loaded into eqgame.exe).
// On macOS they compile with stubs but nothing calls into them, so suppress
// dead_code warnings per-module rather than crate-wide.
#[allow(dead_code)]
mod combat;
#[allow(dead_code)]
mod dialog;
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
#[allow(dead_code)]
mod stealth;
#[allow(dead_code)]
mod syscall;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Base address of eqgame.exe in memory. Set during initialization.
/// All EQ offsets are added to this value to compute runtime addresses.
pub static EQ_BASE: AtomicU64 = AtomicU64::new(0);

/// Global flag indicating the DLL is shutting down.
/// Checked by long-running loops (IPC listener, nav ticks) to exit gracefully.
pub static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

/// Guard against double injection. Set to true on first `DLL_PROCESS_ATTACH`.
/// If a second copy is loaded (randomized DLL names bypass `LoadLibrary` dedup),
/// the init thread exits immediately.
#[cfg(windows)]
static ALREADY_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
mod dll_main {
    use windows::Win32::Foundation::{BOOL, HMODULE, TRUE};
    use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
    use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
    use windows::Win32::System::Threading::{PTP_CALLBACK_INSTANCE, PTP_WORK};

    /// Thread pool callback for PoolParty-style execution. Matches the
    /// `PTP_WORK_CALLBACK` signature used by `CreateThreadpoolWork`.
    ///
    /// Runs on a pre-existing OS worker thread — no `CreateThread` or
    /// `CreateRemoteThread` events are generated. Indistinguishable from
    /// normal application thread pool activity across 36 clients.
    unsafe extern "system" fn init_pool_callback(
        _instance: PTP_CALLBACK_INSTANCE,
        _context: *mut core::ffi::c_void,
        _work: PTP_WORK,
    ) {
        // Prevent double initialization if injected twice into the same process.
        if super::ALREADY_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        if let Err(e) = super::initialize() {
            tracing::error!("DMFT DLL initialization failed: {}", e);
        }
    }

    /// DLL entry point. Called by Windows when the DLL is loaded/unloaded.
    /// IMPORTANT: `DllMain` runs under the loader lock — keep work minimal.
    /// We submit init to the OS thread pool (PoolParty) instead of calling
    /// `CreateThread`, which generates suspicious thread creation events.
    #[unsafe(no_mangle)]
    pub extern "system" fn DllMain(
        module: HMODULE,
        reason: u32,
        _reserved: *mut core::ffi::c_void,
    ) -> BOOL {
        match reason {
            DLL_PROCESS_ATTACH => {
                // SAFETY: Called from DllMain under the loader lock with a valid HMODULE.
                // DisableThreadLibraryCalls requires a valid module handle (guaranteed by
                // the OS calling DllMain). Thread pool submission via CreateThreadpoolWork
                // + SubmitThreadpoolWork is safe under the loader lock — it only queues
                // a work item to existing threads without creating new ones.
                unsafe {
                    // Suppress DLL_THREAD_ATTACH/DETACH notifications for perf.
                    let _ = DisableThreadLibraryCalls(module);

                    // PoolParty: submit init to the process-default thread pool.
                    // Our callback runs on an existing OS worker thread — no
                    // CreateThread/CreateRemoteThread events across 36 clients.
                    if let Err(e) =
                        super::stealth::thread_pool::submit_to_thread_pool(init_pool_callback, None)
                    {
                        // Thread pool submission failed — this should be extremely rare.
                        // Log will only appear if tracing is somehow already initialized.
                        tracing::error!("PoolParty thread pool submission failed: {}", e);
                    }
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

    // 2.5. Initialize indirect syscall layer (RecycledGate).
    // Must happen early — other stealth modules (HWBP hooks, sleep obfuscation)
    // will use these syscalls for NtSetContextThread, NtProtectVirtualMemory, etc.
    if let Err(e) = syscall::init() {
        tracing::warn!(
            "Indirect syscall init failed (falling back to direct calls): {}",
            e
        );
    }

    // 3. Install function hooks. If the EQ window isn't available yet (e.g.,
    //    injected at login screen), spawn a background thread that retries
    //    until the window appears and the HWBP can be set on the main thread.
    if let Err(e) = install_hooks(eq_base) {
        tracing::warn!("Hook installation deferred (window not ready): {}", e);
        let eq_base_copy = eq_base;
        std::thread::spawn(move || {
            for attempt in 1..=60 {
                std::thread::sleep(std::time::Duration::from_secs(2));
                if crate::SHUTTING_DOWN.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }
                match install_hooks(eq_base_copy) {
                    Ok(()) => {
                        tracing::info!(attempt, "Deferred hook installation succeeded");
                        return;
                    }
                    Err(e) => {
                        if attempt % 10 == 0 {
                            tracing::debug!(attempt, error = %e, "Deferred hook install retry");
                        }
                    }
                }
            }
            tracing::error!("Deferred hook installation gave up after 60 attempts (2 minutes)");
        });
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

    // 6. Initialize per-frame sleep obfuscation (Gargoyle-style).
    if let Err(e) = stealth::init() {
        tracing::warn!("Sleep obfuscation init failed (non-fatal): {}", e);
    }

    tracing::info!("DMFT DLL initialized successfully");
    Ok(())
}

/// Initialize tracing with file output. Falls back silently if setup fails —
/// better to run without logs than crash EQ.
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn init_tracing() {
    use tracing_appender::rolling;
    use tracing_subscriber::{EnvFilter, fmt};

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

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

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

        // SAFETY: GetModuleHandleW(None) is always safe to call — it returns the
        // base address of the hosting executable (eqgame.exe). The handle is used
        // only as an integer base address, not as a loadable module reference.
        unsafe { GetModuleHandleW(None).map_or(0, |h| h.0 as u64) }
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
    if let Some(expected) =
        dmft_common::offsets::rebase(dmft_common::offsets::PROCESS_GAME_EVENTS, eq_base)
    {
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
    if let Some(render_addr) =
        dmft_common::offsets::rebase(dmft_common::offsets::REAL_RENDER_WORLD, eq_base)
    {
        if let Err(e) = hooks::render::install(render_addr) {
            tracing::warn!(
                "Render hook failed (continuing without render strobe): {}",
                e
            );
        }
    } else {
        tracing::warn!("Could not rebase REAL_RENDER_WORLD -- render strobe disabled");
    }

    // Install DX11 null device hooks — vtable-hook CreateTexture2D + CreateBuffer
    // so NullRender mode can create 1×1 textures instead of full-size, saving ~500 MB.
    if let Err(e) = hooks::dx11_null::install(eq_base) {
        tracing::warn!("DX11 null hooks failed (continuing without texture reduction): {}", e);
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
        .join(format!("token_{pid}.bin"));

    if let Ok(mut file) = std::fs::File::open(&token_path) {
        // Read a fixed-size token without ever allocating based on attacker-controlled file size.
        let mut token = [0u8; 32];
        let read_ok = std::io::Read::read_exact(&mut file, &mut token).is_ok();
        let mut extra = [0u8; 1];
        let has_extra = std::io::Read::read(&mut file, &mut extra)
            .map(|n| n > 0)
            .unwrap_or(false);

        // Clean up — token is single-use
        let _ = std::fs::remove_file(&token_path);

        if read_ok && !has_extra {
            tracing::info!("Loaded CSPRNG session token from orchestrator");
            return token;
        }

        tracing::warn!("Token file has wrong size — falling back to PID-derived token");
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

/// Full cleanup — call from the eject command handler, NOT from `DLL_PROCESS_DETACH`.
/// This runs outside the loader lock so it's safe to do I/O, remove hooks, etc.
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn graceful_shutdown() {
    SHUTTING_DOWN.store(true, Ordering::SeqCst);
    stealth::disable();
    hooks::remove_all();
    ipc::stop();
    tracing::info!("DMFT DLL graceful shutdown complete");
}
