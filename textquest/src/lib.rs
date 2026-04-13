//! TextQuest orchestrator crate — external process for EQ multibox control.
//!
//! This crate provides the TUI dashboard, process reading, IPC, client management,
//! navigation, combat orchestration, camp loop, launcher, and Soul Engine modules.

#![allow(clippy::new_without_default)]

/// Camp loop state machine — pulls, fights, loots, meds, buffs.
#[allow(dead_code)]
pub mod camp;
/// Multi-client session management and self-healing monitor.
#[allow(dead_code)]
pub mod client;
/// Combat automation — assist broadcasting, CC assignment, spell database.
#[allow(dead_code)]
pub mod combat;
/// TOML configuration loading.
pub mod config;
/// Encrypted credential store (Argon2id + AES-256-GCM).
#[allow(dead_code)]
pub mod credentials;
/// Login automation — per-client FSM, staggered launch, process spawner.
#[allow(dead_code)]
pub mod launcher;

/// Discord webhook and bridge integration.
#[allow(dead_code)]
pub mod discord;
/// EverQuest data layer — spawn structs, memory reading, log parsing.
#[allow(dead_code)]
pub mod eq;
/// DLL injection and staging.
#[cfg_attr(not(windows), allow(dead_code))]
pub mod inject;
/// Named pipe server and shared memory IPC.
#[cfg_attr(not(windows), allow(dead_code))]
pub mod ipc;
/// EQ item database, TLP loot tables, wishlists, and loot history.
pub mod loot;
/// Fleet metrics — SQLite-backed storage for events, DPS, loot, lockouts, plat.
pub mod metrics;
/// Navigation — waypoint recording, zone routing, navmesh integration.
#[allow(dead_code)]
pub mod nav;
/// Orchestrator — wires camp loop state machine to IPC command delivery.
pub mod orchestrator;
/// Orchestrator event loop — async tick loop wiring ClientManager, LaunchCoordinator, and Orchestrator.
pub mod orchestrator_loop;
/// Shared runtime paths for logs and local state.
pub mod paths;
/// OS-level process interaction — open, read memory, find processes.
#[cfg_attr(not(windows), allow(dead_code))]
pub mod process;
/// Soul Engine — LLM-driven character personalities, persistent memory.
#[allow(dead_code)]
pub mod soul;
/// Terminal UI — app state, event handling, theme, renderers.
pub mod tui;

/// CLI subcommands (dump, inject, navigate, login, etc.).
pub mod cli;

/// Economy system — failure routing and recovery for Krono farm / vendor loops.
#[allow(dead_code)]
pub mod economy;

#[cfg(windows)]
use anyhow::Context;
use anyhow::Result;

/// Default path for the soul memory database.
pub const SOUL_DB_PATH: &str = "data/soul_memory.db";

/// Get the base address of eqgame.exe module in the target process.
///
/// # Errors
///
/// Returns an error if the operation fails.
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

/// Non-Windows stub: returns the preferred base address.
///
/// # Errors
///
/// This stub always succeeds.
#[cfg(not(windows))]
pub fn get_module_base(_proc: &process::memory::ProcessHandle) -> Result<u64> {
    tracing::warn!("Using preferred base address (non-Windows stub)");
    Ok(textquest_common::offsets::EQ_PREFERRED_BASE)
}
