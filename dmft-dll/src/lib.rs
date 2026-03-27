//! DMFT injected DLL payload.
//! This cdylib is loaded into eqgame.exe via CreateRemoteThread + LoadLibrary.
//! It hooks internal EQ functions and communicates with the DMFT orchestrator via IPC.

mod hooks;
mod ipc;
mod eq;

#[cfg(windows)]
mod dll_main {
    use windows::Win32::Foundation::{BOOL, HMODULE, TRUE};
    use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
    use std::thread;

    /// DLL entry point. Called by Windows when the DLL is loaded/unloaded.
    /// IMPORTANT: DllMain runs under the loader lock — do NOT do complex work here.
    /// Spawn a thread for initialization instead.
    #[unsafe(no_mangle)]
    pub extern "system" fn DllMain(
        _module: HMODULE,
        reason: u32,
        _reserved: *mut core::ffi::c_void,
    ) -> BOOL {
        match reason {
            DLL_PROCESS_ATTACH => {
                // Spawn init thread to avoid loader lock issues
                thread::spawn(|| {
                    if let Err(e) = super::initialize() {
                        tracing::error!("DMFT DLL initialization failed: {}", e);
                    }
                });
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

/// Clean shutdown — remove hooks and close IPC.
fn shutdown() {
    // TODO: Remove all hooks
    // TODO: Close IPC connections
    tracing::info!("DMFT DLL shutting down");
}
