//! DMFT injected DLL payload.
//! This cdylib is loaded into eqgame.exe via CreateRemoteThread + LoadLibrary.
//! It hooks internal EQ functions and communicates with the DMFT orchestrator via IPC.
#![allow(dead_code)]

mod combat;
mod hooks;
mod ipc;
mod eq;
mod nav;

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
fn initialize() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Set up tracing/logging
    // TODO: Resolve EQ base address
    // TODO: Install function hooks
    // TODO: Start IPC listener
    tracing::info!("DMFT DLL initialized successfully");
    Ok(())
}

use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag indicating the DLL is shutting down.
/// Checked by long-running loops (IPC listener, nav ticks) to exit gracefully.
pub static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

/// Signal shutdown. Called from `DLL_PROCESS_DETACH` under loader lock, so this
/// must be minimal — just set the flag. Actual cleanup (hook removal, IPC close)
/// must happen via the eject command path BEFORE `DLL_PROCESS_DETACH` fires.
fn shutdown() {
    SHUTTING_DOWN.store(true, Ordering::SeqCst);
}
