//! TextQuest injected DLL payload.
//! This cdylib is loaded into eqgame.exe via reflective injection. Initialization
//! runs on the OS thread pool (PoolParty) — no `CreateThread` / `CreateRemoteThread`.
//! It hooks internal EQ functions and communicates with the TextQuest orchestrator via IPC.

//! Export-table exposure audit:
//! - `Cargo.toml` declares `crate-type = ["cdylib", "rlib"]`, which allows a
//!   native export surface if symbols are emitted by the Rust/LLVM toolchain.
//! - Runtime hardening is applied by unlinking from PEB module lists and erasing
//!   PE headers after startup so scanners that walk in-process exports do not
//!   recover a valid exported symbol table from the loaded image.

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
        context: *mut core::ffi::c_void,
        _work: PTP_WORK,
    ) {
        // Prevent double initialization if injected twice into the same process.
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
                    // Suppress DLL_THREAD_ATTACH/DETACH notifications for perf.
                    let _ = DisableThreadLibraryCalls(module);

                    // PoolParty: submit init to the process-default thread pool.
                    // Our callback runs on an existing OS worker thread — no
                    // CreateThread/CreateRemoteThread events across 36 clients.
                    let context = Some(module.0 as *mut core::ffi::c_void);
                    if let Err(e) = super::stealth::thread_pool::submit_to_thread_pool(
                        init_pool_callback,
                        context,
                    ) {
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

/// Initialize the TextQuest DLL after injection.
/// Called from a spawned thread (NOT under loader lock).
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn initialize(dll_base: *mut u8) -> Result<(), Box<dyn std::error::Error>> {
    let _dll_base = dll_base;
    // 1. Set up tracing — write logs to a file since we have no console.
    init_tracing();
    tracing::info!("TextQuest DLL initializing (pid={})", std::process::id());

    // 2. Resolve EQ base address.
    let eq_base = resolve_eq_base();
    EQ_BASE.store(eq_base, Ordering::Release);
    tracing::info!(base = format!("{:#x}", eq_base), "EQ base address resolved");

    // 2.1. Auto-detect offsets via pattern scanning (opt-in shadow mode).
    // Set TEXTQUEST_SCAN_OFFSETS=1 to enable. Results are logged and validated
    // against compiled constants but NOT used for control flow yet. See #746.
    if std::env::var("TEXTQUEST_SCAN_OFFSETS").as_deref() == Ok("1") {
        scan_offsets(eq_base);
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

    // 3. Install function hooks. If the EQ window isn't available yet (e.g.,
    //    injected at login screen), spawn a background thread that retries
    //    until the window appears and the HWBP can be set on the main thread.
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
            "eqmain.dll not loaded at init — GiveTime hook skipped (game may already be at char select)"
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

    // 5.5. Hook integrity self-check — verify HWBP slot state before accepting IPC commands.
    // If any slot is inconsistent (active without address/callback, or stale metadata after
    // removal), enter safe mode: the IPC listener will reject all commands until the DLL
    // is reinjected. This is non-fatal — we log the error and continue so the process can
    // still run without crash; operators see "safe mode" in log and re-inject to recover.
    if let Err(e) = hooks::integrity::verify_hooks_or_safe_mode() {
        tracing::error!(
            error = %e,
            "Hook integrity check failed — DLL entering safe mode (IPC commands will be rejected)"
        );
    }

    // 6. Start IPC listener.
    let client_id = std::process::id();
    let session_token = generate_session_token(client_id);
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

/// Run pattern scanning to auto-detect EQ offsets (shadow mode).
///
/// Scans eqgame.exe memory for known byte patterns and logs results. Does NOT
/// update the active offset table — results are compared against compiled
/// constants for validation only. See #746 (Auto Patch).
#[allow(dead_code)] // Only called when TEXTQUEST_SCAN_OFFSETS is set
fn scan_offsets(eq_base: u64) {
    use textquest_common::pattern_db::{SCAN_ENTRIES, ScanModule};
    use textquest_common::scan_engine;

    tracing::info!("Auto Patch: starting offset scan (shadow mode)");

    // Get module size to bound the scan region.
    let module_size = get_module_size(eq_base);
    if module_size == 0 {
        tracing::warn!("Auto Patch: could not determine eqgame.exe module size — skipping scan");
        return;
    }

    // SAFETY: eq_base is the base address of eqgame.exe obtained via
    // GetModuleHandle, and module_size is from GetModuleInformation.
    // The DLL is loaded inside eqgame.exe's process, so this memory is valid
    // and readable for the lifetime of this function call.
    let data = unsafe { std::slice::from_raw_parts(eq_base as *const u8, module_size) };

    // ── Version detection ──────────────────────────────────────────
    let (client_date, version_matches) = scan_engine::check_version(data);
    match &client_date {
        Some(date) if version_matches => {
            tracing::info!(
                date = %date,
                "Auto Patch: EQ client matches expected version"
            );
        }
        Some(date) => {
            tracing::warn!(
                detected = %date,
                expected = %scan_engine::EXPECTED_CLIENT_DATE,
                "Auto Patch: EQ client version MISMATCH — offsets may be stale!"
            );
        }
        None => {
            tracing::warn!("Auto Patch: could not detect EQ client version");
        }
    }

    // ── Pattern scanning ───────────────────────────────────────────
    let report = scan_engine::scan_module(
        data,
        eq_base,
        textquest_common::offsets::EQ_PREFERRED_BASE,
        ScanModule::EqGame,
        SCAN_ENTRIES,
    );

    // Log summary.
    tracing::info!(
        scanned = report.entries_scanned,
        found = report.entries_found,
        validated = report.entries_validated,
        failed = report.entries_failed.len(),
        skipped_placeholders = report.entries_skipped.len(),
        moved = report.entries_moved.len(),
        "Auto Patch: scan complete"
    );

    // Log placeholder count once (debug level — expected until real patterns exist).
    if !report.entries_skipped.is_empty() {
        tracing::debug!(
            count = report.entries_skipped.len(),
            "Auto Patch: entries with placeholder patterns (awaiting Ghidra export)"
        );
    }

    // Log individual real scan failures (not placeholders).
    for name in &report.entries_failed {
        tracing::warn!(name = %name, "Auto Patch: scan entry failed — pattern not found or resolution failed");
    }

    // Log moved offsets.
    for (name, expected, found) in &report.entries_moved {
        tracing::warn!(
            name = %name,
            expected = format!("{:#x}", expected),
            found = format!("{:#x}", found),
            "Auto Patch: offset moved since last build"
        );
    }
}

/// Get the size of the module loaded at `base_addr`.
#[allow(dead_code)]
fn get_module_size(base_addr: u64) -> usize {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HMODULE;
        use windows::Win32::System::ProcessStatus::{GetModuleInformation, MODULEINFO};
        use windows::Win32::System::Threading::GetCurrentProcess;

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
    if let Err(e) = hooks::detours::install_all(eq_base) {
        tracing::warn!("Inline detour install failed (continuing): {}", e);
    }

    // Primary: use the offset constant derived from PROCESS_GAME_EVENTS.
    let main_loop_offset = eq::MAIN_LOOP_OFFSET;
    if main_loop_offset == 0 {
        tracing::warn!("Game loop hook SKIPPED — MAIN_LOOP_OFFSET is still 0x0!");
        return Ok(());
    }

    let main_loop_addr = eq_base as usize + main_loop_offset;

    // Cross-check against textquest_common offsets via rebase.
    let install_addr =
        if let Some(expected) = textquest_common::offsets::rebase(textquest_common::offsets::PROCESS_GAME_EVENTS, eq_base) {
            if main_loop_addr != expected {
                tracing::warn!(
                    computed = format!("{:#x}", main_loop_addr),
                    expected = format!("{:#x}", expected),
                    "MAIN_LOOP_OFFSET disagrees with offsets::PROCESS_GAME_EVENTS — using offsets rebase"
                );
                expected
            } else {
                main_loop_addr
            }
        } else {
            main_loop_addr
        };
    hooks::game_loop::install(install_addr)?;

    // Install render strobe hook -- background clients skip 3D rendering.
    if let Err(e) = hooks::render::install(eq_base) {
        tracing::warn!(
            "Render hook failed (continuing without render strobe): {}",
            e
        );
    }

    // Install chat message hook — intercepts dsp_chat to capture all in-game text.
    if let Some(chat_addr) =
        textquest_common::offsets::rebase(textquest_common::offsets::DSP_CHAT, eq_base)
    {
        if let Err(e) = hooks::chat::install(chat_addr) {
            tracing::warn!("Chat hook failed (continuing without chat capture): {}", e);
        }
    } else {
        tracing::warn!("Could not rebase DSP_CHAT -- chat capture disabled");
    }

    // Install CEverQuest state transition hook — keep orchestrator in sync with
    // world/login/loading transitions and allow future hook set rotation.
    if let Some(set_game_state_addr) =
        textquest_common::offsets::rebase(textquest_common::offsets::EVERQUEST_SET_GAME_STATE, eq_base)
    {
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
    // so NullRender mode can create 1×1 textures instead of full-size, saving ~500 MB.
    if let Err(e) = hooks::dx11_null::install(eq_base) {
        tracing::warn!(
            "DX11 null hooks failed (continuing without texture reduction): {}",
            e
        );
    }

    Ok(())
}

/// Read the session token injected by the orchestrator.
///
/// The orchestrator writes a 32-byte CSPRNG token to `%TEMP%/textquest/token_{pid}.bin`
/// before injection. The DLL reads it once during init and deletes the file.
/// Falls back to a PID-derived token with a warning if the file is missing (e.g.
/// during development or manual injection).
#[allow(dead_code)] // Only called from #[cfg(windows)] DllMain
fn generate_session_token(pid: u32) -> textquest_common::ipc::SessionToken {
    let token_path = std::env::temp_dir()
        .join("textquest")
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
    tracing::info!("TextQuest DLL graceful shutdown complete");
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use toml::Value;

    #[test]
    fn manifest_declares_cdylib() {
        let manifest = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
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
            crate_types.iter().any(|value| value.as_str() == Some("cdylib"))
                && crate_types.iter().any(|value| value.as_str() == Some("rlib")),
            "dll crate should be built as both cdylib and rlib"
        );
    }

    #[test]
    fn no_public_export_symbols_in_textquest_dll_src() {
        let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
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

                let contents = std::fs::read_to_string(item.path())
                    .expect("rust source should be readable");
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

                    if line.contains("pub extern") && line.contains("fn") && !line.contains("DllMain")
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
        assert_eq!(no_mangle_attrs, 1, "expected only one #[no_mangle] export (DllMain)");
        assert!(
            public_extern_fns.is_empty(),
            "unexpected pub extern function in Rust source: {:?}",
            public_extern_fns
        );
    }
}
