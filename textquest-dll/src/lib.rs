//! TextQuest injected DLL payload.
//! This cdylib is loaded into eqgame.exe via reflective injection.
//! Initialization runs on the OS thread pool (PoolParty) — no `CreateThread` /
//! `CreateRemoteThread`. It hooks internal EQ functions and communicates with
//! the TextQuest orchestrator via IPC.

//! Export-table exposure audit:
//! - `Cargo.toml` declares `crate-type = ["cdylib", "rlib"]`, which allows a
//!   native export surface if symbols are emitted by the Rust/LLVM toolchain.
//! - Runtime hardening is applied by unlinking from PEB module lists and
//!   erasing PE headers after startup so scanners that walk in-process exports
//!   do not recover a valid exported symbol table from the loaded image.

// Deeply nested unsafe FFI code with many conditional pointer checks — collapsing
// these ifs reduces readability in practice. Also suppress needless_return for
// early-return patterns in long unsafe blocks.
#![allow(clippy::collapsible_if, clippy::needless_return)]

// All DLL modules are Windows-only at runtime (cdylib loaded into eqgame.exe).
// On macOS they compile with stubs but nothing calls into them, so suppress
// dead_code warnings per-module rather than crate-wide.
#[allow(dead_code)]
mod boxr;
#[allow(dead_code)]
mod combat;
#[allow(dead_code)]
pub mod dannet_tlo;
#[allow(dead_code)]
pub mod commands;
#[allow(dead_code)]
mod debug;
#[allow(dead_code)]
mod dialog;
#[allow(dead_code)]
mod eq;
#[allow(dead_code)]
mod hooks;
#[allow(dead_code)]
pub mod hotkeys;
#[allow(dead_code)]
mod injection;
#[allow(dead_code)]
mod ipc;
#[allow(dead_code)]
mod login;
#[allow(dead_code)]
pub mod mq2;
#[allow(dead_code)]
mod nav;
#[allow(dead_code)]
pub mod overlay;
#[allow(dead_code)]
mod rewards;
#[allow(dead_code)]
mod stealth;
#[allow(dead_code)]
mod syscall;
#[allow(dead_code)]
pub mod timestamp;
#[allow(dead_code)]
mod tradeskill_trophy;

use std::{
    path::{Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use textquest_common::offset_db::OffsetDatabase;

/// Base address of eqgame.exe in memory. Set during initialization.
/// All EQ offsets are added to this value to compute runtime addresses.
pub static EQ_BASE: AtomicU64 = AtomicU64::new(0);

/// Runtime offset database loaded from scan/config data.
/// When populated, resolved offsets use this database before falling back to
/// compile-time rebase logic.
pub static OFFSET_DB: OnceLock<OffsetDatabase> = OnceLock::new();
/// Cached EQ build-date string detected at startup, if available.
static EQ_ACTUAL_VERSION: OnceLock<Option<String>> = OnceLock::new();

/// Global flag indicating the DLL is shutting down.
/// Checked by long-running loops (IPC listener, nav ticks) to exit gracefully.
pub static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

/// Guard against double injection. Set to true on first `DLL_PROCESS_ATTACH`.
/// If a second copy is loaded (randomized DLL names bypass `LoadLibrary`
/// dedup), the init thread exits immediately.
#[cfg(windows)]
static ALREADY_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static HOOK_ROTATION_MANAGER: OnceLock<std::sync::Mutex<hooks::rotation::HookRotationManager>> =
    OnceLock::new();

#[cfg(windows)]
mod dll_main {
    use windows::Win32::{
        Foundation::{BOOL, HMODULE, TRUE},
        System::{
            LibraryLoader::DisableThreadLibraryCalls,
            SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH},
            Threading::{PTP_CALLBACK_INSTANCE, PTP_WORK},
        },
    };

    /// Thread pool callback for PoolParty-style execution. Matches the
    /// `LPTHREAD_START_ROUTINE` for the init thread spawned by DllMain.
    ///
    /// Called on a fresh thread created by `CreateThread`. Matches the
    /// `(LPVOID) -> DWORD` signature required by `CreateThread`.
    unsafe extern "system" fn init_thread_fn(context: *mut core::ffi::c_void) -> u32 {
        // Prevent double initialization if injected twice into the same process.
        if super::ALREADY_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return 0;
        }
        let dll_base = context as *mut u8;
        if let Err(e) = super::initialize(dll_base) {
            tracing::error!("TextQuest DLL initialization failed: {}", e);
        }
        0
    }

    /// PoolParty callback — kept for reference / future stealthier re-enable.
    #[allow(dead_code)]
    unsafe extern "system" fn init_pool_callback(
        _instance: PTP_CALLBACK_INSTANCE,
        context: *mut core::ffi::c_void,
        _work: PTP_WORK,
    ) {
        if super::ALREADY_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let dll_base = context as *mut u8;
        if let Err(e) = super::initialize(dll_base) {
            tracing::error!("TextQuest DLL initialization failed: {}", e);
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
                    use windows::Win32::System::Threading::{CreateThread, THREAD_CREATION_FLAGS};

                    // Suppress DLL_THREAD_ATTACH/DETACH notifications for perf.
                    let _ = DisableThreadLibraryCalls(module);

                    // Spawn init thread. PoolParty (CreateThreadpoolWork) was the
                    // original approach but the process thread pool may be
                    // unavailable or guarded by anti-cheat in EQ. CreateThread is
                    // more detectable but reliably runs our initializer.
                    let context = module.0 as *const core::ffi::c_void;
                    let thread = CreateThread(
                        None,
                        0,
                        Some(init_thread_fn),
                        Some(context),
                        THREAD_CREATION_FLAGS(0),
                        None,
                    );
                    if let Err(e) = thread {
                        // Nothing we can do — tracing not up yet.
                        let _ = e;
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

/// Initialize the TextQuest DLL after injection.
/// Called from a spawned thread (NOT under loader lock).
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn initialize(dll_base: *mut u8) -> Result<(), Box<dyn std::error::Error>> {
    let _dll_base = dll_base;
    // 1. Set up tracing — write logs to a file since we have no console.
    init_tracing();
    tracing::info!("TextQuest DLL initializing (pid={})", std::process::id());

    #[cfg(debug_assertions)]
    {
        let violations = textquest_common::validation::validate_struct_sizes();
        if violations.is_empty() {
            tracing::info!("Struct size validation passed");
        } else {
            tracing::warn!(
                "Struct size validation failures:\n{}",
                violations.join("\n")
            );
        }
    }

    // 2. Resolve EQ base address.
    let eq_base = resolve_eq_base();
    EQ_BASE.store(eq_base, Ordering::Release);
    tracing::info!(base = format!("{:#x}", eq_base), "EQ base address resolved");

    // Version check — read __ActualVersionDate pointer and validate against
    // expected patch.
    let actual_version = eq::check_eq_version(eq_base);
    match &actual_version {
        Some(version) if eq::version_matches_expected(version) => {
            tracing::info!(version = %version, "EQ version check matched expected patch date");
        }
        Some(version) => {
            tracing::warn!(
                version = %version,
                expected = textquest_common::offsets::EXPECTED_VERSION_DATE,
                "EQ version mismatch detected"
            );
        }
        None => {
            tracing::warn!("EQ version check unavailable; proceeding without version validation");
        }
    }
    let _ = EQ_ACTUAL_VERSION.set(actual_version);

    // 2.1. Auto-detect offsets via pattern scanning. Scanning is now default-on;
    // TEXTQUEST_SKIP_SCAN=1 keeps the compiled-offset fallback path available.
    if is_scan_active() {
        scan_offsets();
    }

    // 2.5. Initialize indirect syscall layer (RecycledGate).
    // Must happen early — other stealth modules (HWBP hooks, sleep obfuscation)
    // will use these syscalls for NtSetContextThread, NtProtectVirtualMemory, etc.
    if let Err(e) = syscall::init() {
        tracing::warn!(
            "Indirect syscall init failed (falling back to direct calls): {}",
            e
        );
    }

    let client_id = std::process::id();
    let session_token = generate_session_token(client_id);
    hooks::fingerprint::init(&session_token);
    if let Err(e) = hooks::fingerprint::install_firmware_hooks() {
        tracing::warn!(
            "Firmware table fingerprint hooks failed (continuing with real firmware tables): {}",
            e
        );
    }

    // 3. Install function hooks. If the EQ window isn't available yet (e.g.,
    //    injected at login screen), spawn a background thread that retries until
    //    the window appears and the HWBP can be set on the main thread.
    if let Err(e) = install_hooks(eq_base) {
        tracing::warn!("Hook installation deferred (window not ready): {}", e);
        std::thread::spawn(move || {
            for attempt in 1..=60 {
                std::thread::sleep(std::time::Duration::from_secs(2));
                if crate::SHUTTING_DOWN.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }
                match install_hooks(eq_base) {
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

    // 3.5. Install eqmain GiveTime hook for main-thread login automation.
    // This must be installed while eqmain.dll is loaded (before character select).
    let eqmain_base = login::eqmain::find_eqmain();
    if eqmain_base != 0 {
        if let Err(e) = hooks::eqmain_hook::install(eqmain_base) {
            tracing::warn!(
                "eqmain GiveTime hook failed (login will use fallback): {}",
                e
            );
        }
    } else {
        tracing::info!(
            "eqmain.dll not loaded at init — GiveTime hook skipped (game may already be at char \
             select)"
        );
    }

    // 4. Initialize command jitter RNG for anti-detection.
    hooks::game_loop::init_jitter_rng();

    // 4.5. Initialize login FSM.
    login::init();

    // 5. Initialize sleep obfuscation (used by HWBP callbacks).
    // NOTE: This is temporarily disabled whenever the IPC listener thread is
    // running, because background threads can execute normal `.text` outside
    // HWBP callback wake/sleep transitions.
    if let Err(e) = stealth::init() {
        tracing::warn!("Sleep obfuscation init failed (non-fatal): {}", e);
    }

    // 5.5. Hook integrity self-check — verify HWBP slot state before accepting IPC
    // commands. If any slot is inconsistent (active without address/callback,
    // or stale metadata after removal), enter safe mode: the IPC listener will
    // reject all commands until the DLL is reinjected. This is non-fatal — we
    // log the error and continue so the process can still run without crash;
    // operators see "safe mode" in log and re-inject to recover.
    if let Err(e) = hooks::integrity::verify_hooks_or_safe_mode() {
        tracing::error!(
            error = %e,
            "Hook integrity check failed — DLL entering safe mode (IPC commands will be rejected)"
        );
    }

    // 5.5. Register built-in MQ2 interop commands.
    boxr::register_boxr_command();

    // 6. Start IPC listener.
    match ipc::start(client_id, session_token) {
        Ok(()) => {
            // IPC uses a long-lived background thread that executes regular
            // Rust `.text` code. Keep obfuscation disabled while it runs to
            // avoid re-encrypting executable code out from under that thread.
            stealth::disable();
            tracing::warn!("Sleep obfuscation disabled while IPC listener is active");
        }
        Err(e) => {
            tracing::warn!("IPC startup failed (continuing without IPC): {}", e);
        }
    }

    #[cfg(windows)]
    if !_dll_base.is_null() {
        if let Err(e) = stealth::section_remap::remap_sections(_dll_base) {
            tracing::warn!("Section remap verification failed (non-fatal): {}", e);
        }
        // PEB unlinking + PE header erasure removes module/envelope visibility
        // from conventional in-process enumeration paths (PEB lists / PE exports).
        if let Err(e) = stealth::peb_unlink::unlink_module(_dll_base) {
            tracing::warn!("PEB unlink failed (non-fatal): {}", e);
        }
        if let Err(e) = stealth::pe_erase::erase_pe_headers(_dll_base) {
            tracing::warn!(
                "PE header erasure failed (non-fatal; export parsing may remain possible): {}",
                e
            );
        }
    }

    tracing::info!("TextQuest DLL initialized successfully");
    Ok(())
}

/// Initialize tracing with file output. Falls back silently if setup fails —
/// better to run without logs than crash EQ.
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn init_tracing() {
    use tracing_appender::rolling;
    use tracing_subscriber::{EnvFilter, fmt};

    let log_dir = std::env::temp_dir().join("textquest");
    std::fs::create_dir_all(&log_dir).ok();
    let file_appender = rolling::RollingFileAppender::builder()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix("textquest-dll.log")
        .max_log_files(7) // Keep 1 week of logs
        .build(&log_dir)
        .unwrap_or_else(|_| rolling::daily(&log_dir, "textquest-dll.log"));
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Leak the guard so it lives for the DLL's lifetime — there is no clean
    // drop point for a cdylib that outlives its init thread.
    Box::leak(Box::new(guard));

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    tracing::info!("TextQuest DLL tracing initialized");
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

/// Get the size of the module loaded at `base_addr`.
#[allow(dead_code)]
fn get_module_size(base_addr: u64) -> usize {
    #[cfg(windows)]
    {
        use windows::Win32::{
            Foundation::HMODULE,
            System::{
                ProcessStatus::{GetModuleInformation, MODULEINFO},
                Threading::GetCurrentProcess,
            },
        };

        let mut info = MODULEINFO::default();
        // SAFETY: GetCurrentProcess returns a pseudo-handle that is always valid.
        // The HMODULE is obtained from GetModuleHandle and is valid. We pass a
        // correctly-sized MODULEINFO buffer.
        let ok = unsafe {
            GetModuleInformation(
                GetCurrentProcess(),
                HMODULE(base_addr as isize),
                &mut info,
                std::mem::size_of::<MODULEINFO>() as u32,
            )
        };
        if ok.is_ok() {
            info.SizeOfImage as usize
        } else {
            0
        }
    }

    #[cfg(not(windows))]
    {
        let _ = base_addr;
        // Stub for non-Windows builds — return 0 so module scanning is disabled.
        0
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

    // Cross-check against textquest_common offsets via rebase.
    let install_addr = if let Some(expected) =
        textquest_common::offsets::rebase(textquest_common::offsets::PROCESS_GAME_EVENTS, eq_base)
    {
        if main_loop_addr != expected {
            tracing::warn!(
                computed = format!("{:#x}", main_loop_addr),
                expected = format!("{:#x}", expected),
                "MAIN_LOOP_OFFSET disagrees with offsets::PROCESS_GAME_EVENTS — using offsets \
                 rebase"
            );
            expected
        } else {
            main_loop_addr
        }
    } else {
        main_loop_addr
    };
    hooks::game_loop::install(install_addr)?;
    install_remaining_hooks(eq_base)?;
    Ok(())
}

fn install_remaining_hooks(eq_base: u64) -> Result<(), Box<dyn std::error::Error>> {
    // Install WMI COM hooks before EQ can use WbemLocator for hardware or
    // process inventory that bypasses direct Win32 interception.
    if let Err(e) = hooks::wmi::install() {
        tracing::warn!(
            "WMI evasion hook failed (continuing without WMI filtering): {}",
            e
        );
    }

    // Install render strobe hook -- background clients skip 3D rendering.
    if let Err(e) = hooks::render::install(eq_base) {
        tracing::warn!(
            "Render hook failed (continuing without render strobe): {}",
            e
        );
    }

    // Install chat message hook — intercepts dsp_chat to capture all in-game text.
    if let Some(chat_addr) = resolve_offset("dspChat", textquest_common::offsets::DSP_CHAT, eq_base)
    {
        if let Err(e) = hooks::chat::install(chat_addr as usize) {
            tracing::warn!("Chat hook failed (continuing without chat capture): {}", e);
        }
    } else {
        tracing::warn!("Could not rebase DSP_CHAT -- chat capture disabled");
    }

    // Install CEverQuest state transition hook — keep orchestrator in sync with
    // world/login/loading transitions and allow future hook set rotation.
    if let Some(set_game_state_addr) = textquest_common::offsets::rebase(
        textquest_common::offsets::EVERQUEST_SET_GAME_STATE,
        eq_base,
    ) {
        if let Err(e) = hooks::set_game_state::install(set_game_state_addr) {
            tracing::warn!(
                "SetGameState hook failed (continuing without state notifications): {}",
                e
            );
        }
    } else {
        tracing::warn!("Could not rebase EVERQUEST_SET_GAME_STATE -- SetGameState hook disabled");
    }

    // Install DX11 null device hooks — vtable-hook CreateTexture2D + CreateBuffer
    // so NullRender mode can create 1×1 textures instead of full-size, saving ~500
    // MB.
    if let Err(e) = hooks::dx11_null::install(eq_base) {
        tracing::warn!(
            "DX11 null hooks failed (continuing without texture reduction): {}",
            e
        );
    }
    if let Err(e) = hooks::detours::install_all(eq_base) {
        tracing::warn!("Detour hook manager install_all() failed: {}", e);
    }
    #[cfg(windows)]
    {
        // Trampoline hardening is defensive-only and must never block initialization.
        let trampoline_hardener = stealth::trampoline::TrampolineHardener::new();
        trampoline_hardener.protect_registered();
        trampoline_hardener.harden_private_rwx_allocations();
    }

    Ok(())
}

pub(crate) fn eq_actual_version() -> Option<String> {
    EQ_ACTUAL_VERSION
        .get()
        .and_then(|value: &Option<String>| value.clone())
}

fn is_scan_active() -> bool {
    std::env::var("TEXTQUEST_SKIP_SCAN").map_or(true, |v: String| v != "1")
}

fn current_exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

#[allow(dead_code)]
fn has_scanned_offsets() -> bool {
    OFFSET_DB
        .get()
        .is_some_and(|db: &OffsetDatabase| !db.globals.is_empty() || !db.functions.is_empty())
}

fn scan_offsets() {
    let mut paths = Vec::new();
    if let Some(root) = current_exe_dir() {
        paths.push(root.join("config").join("offsets.json"));
    }
    paths.push(PathBuf::from("config/offsets.json"));
    paths.push(PathBuf::from(r"C:\textquest\config\offsets.json"));
    paths.push(std::env::temp_dir().join("textquest").join("offsets.json"));

    let db = paths.into_iter().find_map(|path| {
        match textquest_common::offset_db::OffsetDatabase::load_from_file(&path) {
            Ok(db) => Some((path, db)),
            Err(err) => {
                tracing::debug!(
                    path = path.display().to_string(),
                    error = %err,
                    "Failed to load scan offsets candidate"
                );
                None
            }
        }
    });

    let Some((resolved_path, db)) = db else {
        tracing::warn!("Scan offsets enabled but no readable offsets source found");
        return;
    };

    install_offset_db(
        db,
        Some(&resolved_path),
        "Loaded scan offsets into OFFSET_DB",
    );
}

fn install_offset_db(db: OffsetDatabase, path: Option<&Path>, message: &'static str) {
    let count = db.functions.len() + db.globals.len();
    textquest_common::bindings::install_fallback_database(db.clone());
    textquest_common::offsets::install_runtime_database(db.clone());
    match OFFSET_DB.set(db) {
        Ok(()) => {
            tracing::info!(
                path = path.map(|p| p.display().to_string()).unwrap_or_default(),
                count,
                message
            );
        }
        Err(_) => {
            tracing::warn!(
                path = path.map(|p| p.display().to_string()).unwrap_or_default(),
                count,
                "OFFSET_DB was already initialized; skipping newly loaded scan offsets"
            );
        }
    }
}

/// Resolve a compile-time offset using scan-updated data first, then fallback
/// to static rebase logic from the common offsets table.
fn resolve_offset(name: &str, compiled_addr: u64, base: u64) -> Option<u64> {
    resolve_offset_with_db(name, compiled_addr, base, OFFSET_DB.get())
}

fn resolve_offset_with_db(
    name: &str,
    compiled_addr: u64,
    base: u64,
    db: Option<&OffsetDatabase>,
) -> Option<u64> {
    if let Some(db) = db {
        if let Some(addr) = db.rebase_by_name(name, base) {
            return Some(addr as u64);
        }
    }

    textquest_common::offsets::rebase(compiled_addr, base).map(|addr| addr as u64)
}

/// Read the session token injected by the orchestrator.
///
/// The orchestrator writes a 32-byte CSPRNG token to
/// `%TEMP%/textquest/token_{pid}.bin` before injection. The DLL reads it once
/// during init and deletes the file. Falls back to a PID-derived token with a
/// warning if the file is missing (e.g. during development or manual
/// injection).
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn generate_session_token(pid: u32) -> textquest_common::ipc::SessionToken {
    let token_path = std::env::temp_dir()
        .join("textquest")
        .join(format!("token_{pid}.bin"));

    if let Ok(mut file) = std::fs::File::open(&token_path) {
        // Read a fixed-size token without ever allocating based on attacker-controlled
        // file size.
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
        // getrandom itself failed — extremely unlikely on any supported Windows
        // version. Log prominently and fall back to a PID-mixed value as
        // absolute last resort.
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

/// Full cleanup — call from the eject command handler, NOT from
/// `DLL_PROCESS_DETACH`. This runs outside the loader lock so it's safe to do
/// I/O, remove hooks, etc.
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn graceful_shutdown() {
    SHUTTING_DOWN.store(true, Ordering::SeqCst);
    stealth::disable();
    hooks::remove_all();
    ipc::stop();
    tracing::info!("TextQuest DLL graceful shutdown complete");
}

/// Activate packet hooks for validation mode.
///
/// This is the explicit entry point for validating the packet capture system
/// without depending on packet hooks being installed during normal DLL startup.
/// Useful for testing the packet monitor on current master or validating
/// packet hook functionality in isolation.
///
/// On Windows: Activates WSASend/WSARecv hooks targeting the current process.
/// On non-Windows: Returns Ok(()) as a no-op stub.
///
/// # Arguments
///
/// * `client_id` — typically the current process ID or a test ID for validation
///
/// # Returns
///
/// - `Ok(())` if hooks are successfully activated or already active
/// - `Err(...)` if hook installation fails (Windows only)
///
/// # Example
///
/// ```ignore
/// // From validation mode handler (not part of normal startup)
/// let pid = std::process::id();
/// let result = activate_packet_validation(pid);
/// if result.is_ok() {
///     tracing::info!("Packet hooks activated for validation");
/// }
/// ```
#[allow(dead_code)] // Called via IPC command handler or operator request
pub fn activate_packet_validation(client_id: u32) -> Result<(), Box<dyn std::error::Error>> {
    hooks::packet_hook::activate_packet_hooks(client_id)
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{Mutex, OnceLock},
    };

    use toml::Value;

    static ENV_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_test_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_TEST_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("environment test mutex poisoned")
    }

    fn set_env(key: &str, value: &str) {
        unsafe { std::env::set_var(key, value) };
    }

    fn clear_env(key: &str) {
        unsafe { std::env::remove_var(key) };
    }

    fn crate_manifest_dir() -> PathBuf {
        let mut dir = std::env::current_dir().expect("test working directory should be readable");

        loop {
            if dir.join("Cargo.toml").exists()
                && dir.file_name().and_then(|name| name.to_str()) == Some("textquest-dll")
            {
                return dir;
            }

            let nested = dir.join("textquest-dll");
            if nested.join("Cargo.toml").exists() {
                return nested;
            }

            if !dir.pop() {
                panic!("could not locate textquest-dll crate root from test working directory");
            }
        }
    }

    #[test]
    fn source_does_not_use_compile_time_manifest_dir() {
        let manifest_key = ["CARGO", "MANIFEST", "DIR"].join("_");
        let forbidden = format!("{}(\"{}\")", "env!", manifest_key);
        let mut stack = vec![crate_manifest_dir().join("src")];
        let mut hits = Vec::new();

        while let Some(path) = stack.pop() {
            let entries = std::fs::read_dir(path)
                .expect("source directories must be readable")
                .collect::<Result<Vec<_>, _>>()
                .expect("source entries must be readable");

            for entry in entries {
                let ty = entry.file_type().expect("entry type should be readable");
                if ty.is_dir() {
                    stack.push(entry.path());
                    continue;
                }

                if entry.path().extension().and_then(|ext| ext.to_str()) != Some("rs") {
                    continue;
                }

                let contents =
                    std::fs::read_to_string(entry.path()).expect("rust source should be readable");
                for (line_idx, line) in contents.lines().enumerate() {
                    if line.contains(&forbidden) {
                        hits.push(format!(
                            "{}:{}",
                            entry.path().display(),
                            line_idx.saturating_add(1)
                        ));
                    }
                }
            }
        }

        assert!(
            hits.is_empty(),
            "compile-time manifest paths leak source locations into textquest-dll: {hits:?}"
        );
    }

    #[test]
    fn manifest_declares_cdylib() {
        let manifest = std::fs::read_to_string(crate_manifest_dir().join("Cargo.toml"))
            .expect("cargo manifest should be readable");
        let manifest: Value = manifest
            .parse()
            .expect("cargo manifest should be valid TOML");
        let crate_types = manifest
            .get("lib")
            .and_then(Value::as_table)
            .and_then(|lib| lib.get("crate-type"))
            .and_then(Value::as_array)
            .expect("[lib].crate-type should be an array in Cargo.toml");

        assert!(
            crate_types
                .iter()
                .any(|value| value.as_str() == Some("cdylib"))
                && crate_types
                    .iter()
                    .any(|value| value.as_str() == Some("rlib")),
            "dll crate should be built as both cdylib and rlib"
        );
    }

    #[test]
    fn no_public_export_symbols_in_textquest_dll_src() {
        let src_root = crate_manifest_dir().join("src");
        let mut stack = vec![src_root];
        let mut no_mangle_attrs = 0usize;
        let mut other_no_mangle_exports = Vec::new();
        let mut public_extern_fns = Vec::new();

        while let Some(path) = stack.pop() {
            let entry = std::fs::read_dir(path)
                .expect("source directories must be readable")
                .collect::<Result<Vec<_>, _>>()
                .expect("source entries must be readable");

            for item in entry {
                let ty = item.file_type().expect("entry type should be readable");
                if ty.is_dir() {
                    stack.push(item.path());
                    continue;
                }
                if item.path().extension().and_then(|ext| ext.to_str()) != Some("rs") {
                    continue;
                }

                let contents =
                    std::fs::read_to_string(item.path()).expect("rust source should be readable");
                let mut saw_no_mangle = false;

                for line in contents.lines() {
                    let line = line.trim();
                    if line.starts_with("#[") && line.contains("no_mangle") {
                        saw_no_mangle = true;
                        no_mangle_attrs += 1;
                        continue;
                    }

                    if saw_no_mangle && line.contains("fn ") {
                        if !line.contains("DllMain") {
                            other_no_mangle_exports.push(format!(
                                "{}: {}",
                                item.path().display(),
                                line
                            ));
                        }
                        saw_no_mangle = false;
                    }

                    if line.contains("pub extern")
                        && line.contains("fn")
                        && !line.contains("DllMain")
                    {
                        public_extern_fns.push(format!("{}: {}", item.path().display(), line));
                    }
                }
            }
        }

        assert!(
            other_no_mangle_exports.is_empty(),
            "unexpected #[no_mangle] export found: {:?}",
            other_no_mangle_exports
        );
        assert_eq!(
            no_mangle_attrs, 1,
            "expected only one #[no_mangle] export (DllMain)"
        );
        assert!(
            public_extern_fns.is_empty(),
            "unexpected pub extern function in Rust source: {:?}",
            public_extern_fns
        );
    }

    #[test]
    fn is_scan_active_defaults_to_enabled() {
        let _guard = env_test_lock();
        clear_env("TEXTQUEST_SCAN_OFFSETS");
        clear_env("TEXTQUEST_SCAN_ACTIVE");
        clear_env("TEXTQUEST_SKIP_SCAN");

        assert!(super::is_scan_active());
    }

    #[test]
    fn is_scan_active_respects_skip_scan_escape_hatch() {
        let _guard = env_test_lock();
        clear_env("TEXTQUEST_SCAN_OFFSETS");
        clear_env("TEXTQUEST_SCAN_ACTIVE");
        set_env("TEXTQUEST_SKIP_SCAN", "1");

        assert!(!super::is_scan_active());

        clear_env("TEXTQUEST_SKIP_SCAN");
    }
}
