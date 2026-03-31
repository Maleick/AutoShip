// --- Modules wired through orchestrator/TUI, not referenced directly in main ---
#[allow(dead_code)] // M4: camp loop state machine, driven by orchestrator
pub mod camp;
#[allow(dead_code)] // M2: multi-client sessions, self-healing monitor
pub mod client;
#[allow(dead_code)] // M4: combat automation, class strategies
pub mod combat;
pub mod config;
#[allow(dead_code)] // M2.5: encrypted credential store
pub mod credentials;
#[allow(dead_code)] // M2.5: login automation, process spawner
pub mod launcher;

// --- Modules used in main.rs; dead_code on non-Windows from platform stubs ---
#[allow(dead_code)] // M1: EQ data layer — some fields/functions are scaffolding for future features
pub mod eq;
#[cfg_attr(not(windows), allow(dead_code))]
pub mod inject;
#[cfg_attr(not(windows), allow(dead_code))]
pub mod ipc;
#[allow(dead_code)] // M3+: nav routing, camp management, waypoint recording — scaffolding
pub mod nav;
pub mod orchestrator;
#[cfg_attr(not(windows), allow(dead_code))]
pub mod process;
#[allow(dead_code)] // M5/M6: Soul Engine — scaffolding for LLM personalities, social graph
pub mod soul;
pub mod discord;
pub mod tui;

pub mod cli;

use anyhow::{Context, Result};

/// Default path for the soul memory database.
pub const SOUL_DB_PATH: &str = "data/soul_memory.db";

/// Get the base address of eqgame.exe module in the target process.
#[cfg(windows)]
pub fn get_module_base(proc: &process::memory::ProcessHandle) -> Result<u64> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::ProcessStatus::{EnumProcessModulesEx, LIST_MODULES_ALL};
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    let handle =
        unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, proc.pid) }
            .context("Failed to open process for module enumeration")?;

    let mut modules = [windows::Win32::Foundation::HMODULE::default(); 1024];
    let mut bytes_needed: u32 = 0;

    // SAFETY: `handle` is a valid process handle opened with PROCESS_QUERY_INFORMATION
    // and PROCESS_VM_READ. `modules` is a stack-allocated array of sufficient size.
    // `bytes_needed` receives the count of bytes written.
    unsafe {
        EnumProcessModulesEx(
            handle,
            modules.as_mut_ptr(),
            std::mem::size_of_val(&modules) as u32,
            &mut bytes_needed,
            LIST_MODULES_ALL,
        )
    }
    .context("EnumProcessModulesEx failed")?;

    let base = modules[0].0 as u64;
    // SAFETY: `handle` is a valid, open handle that we own. Closing it once is correct.
    let _ = unsafe { CloseHandle(handle) };

    Ok(base)
}

#[cfg(not(windows))]
pub fn get_module_base(_proc: &process::memory::ProcessHandle) -> Result<u64> {
    tracing::warn!("Using preferred base address (non-Windows stub)");
    Ok(dmft_common::offsets::EQ_PREFERRED_BASE)
}
