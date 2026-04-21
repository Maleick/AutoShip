//! TextQuest orchestrator crate — external process for EQ multibox control.
//!
//! This crate provides the TUI dashboard, process reading, IPC, client
//! management, navigation, combat orchestration, camp loop, launcher, and the
//! Soul Engine crate.

#![allow(clippy::new_without_default)]
#![allow(clippy::items_after_test_module)]

/// Operational alerting — persistence, routing, Discord/email delivery.
pub mod alerts;
/// Auto-group config persistence and runtime invite/role orchestration.
pub mod auto_group;
/// EQBC-style cross-machine TCP relay and dispatch manager.
#[cfg(windows)]
pub mod box_chat;
/// Camp loop state machine — pulls, fights, loots, meds, buffs.
#[allow(dead_code)]
pub mod camp;
/// MQ2Log-style per-character chat output logging.
pub mod chat_log;
/// Multi-client session management and self-healing monitor.
#[cfg(windows)]
#[allow(dead_code)]
pub mod client;
/// Combat automation — assist broadcasting, CC assignment, spell database.
#[cfg(windows)]
#[allow(dead_code)]
pub mod combat;
/// Local slash-command dispatch shared by box-chat relay execution and chat/UI
/// entrypoints.
#[cfg(windows)]
pub mod command_dispatch;
/// TOML configuration loading.
pub mod config;
/// Crash reporting and session recovery — per-character context snapshots and
/// recovery commands.
#[cfg(windows)]
pub mod crash_reporter;
/// Encrypted credential store (Argon2id + AES-256-GCM).
#[cfg(windows)]
#[allow(dead_code)]
pub mod credentials;
/// Login automation — per-client FSM, staggered launch, process spawner.
#[cfg(windows)]
#[allow(dead_code)]
pub mod launcher;

/// Discord webhook and bridge integration.
#[cfg_attr(not(windows), allow(dead_code))]
pub mod discord;
/// EverQuest data layer — spawn structs, memory reading, log parsing.
#[cfg(windows)]
#[allow(dead_code)]
pub mod eq;
/// DLL injection and staging.
#[cfg(windows)]
pub mod inject;
/// Named pipe server and shared memory IPC.
#[cfg(windows)]
pub mod ipc;
/// EQ item database, TLP loot tables, wishlists, and loot history.
pub mod loot;
/// Fleet metrics — SQLite-backed storage for events, DPS, loot, lockouts, plat.
#[cfg(windows)]
pub mod metrics;
/// Navigation — waypoint recording, zone routing, navmesh integration.
#[cfg(windows)]
#[allow(dead_code)]
pub mod nav;
/// Orchestrator — wires camp loop state machine to IPC command delivery.
#[cfg(windows)]
pub mod orchestrator;
/// Orchestrator event loop — async tick loop wiring ClientManager,
/// LaunchCoordinator, and Orchestrator.
#[cfg(windows)]
pub mod orchestrator_loop;
/// Shared runtime paths for logs and local state.
pub mod paths;
/// OS-level process interaction — open, read memory, find processes.
#[cfg_attr(not(windows), allow(dead_code))]
pub mod process;
/// Terminal UI — app state, event handling, theme, renderers.
#[cfg(windows)]
pub mod tui;

/// CLI subcommands (dump, inject, navigate, login, etc.).
#[cfg(windows)]
pub mod cli;
/// Testing utilities — scenario harness, metric types, and result types.
#[cfg(windows)]
#[allow(dead_code)]
pub mod testing;

/// Economy system — failure routing and recovery for Krono farm / vendor loops.
#[cfg(windows)]
#[allow(dead_code)]
pub mod economy;

/// Zone transition management — failure codes, recovery actions, and retry
/// logic.
#[cfg(windows)]
pub mod zoning;

/// Operator utilities — clipboard export and persistent scratchpad.
pub mod operator_utils;

/// Timestamp config runtime — loads per-character timestamp settings from disk
/// and dispatches IPC commands to DLL clients.
pub mod timestamp_runtime;
/// Window title config runtime — loads per-character title formats and
/// dispatches IPC commands to DLL clients.
pub mod window_title_runtime;

/// Say channel detection and alerting — MQ2Say parity.
#[cfg(windows)]
pub mod say_detection;
#[cfg(windows)]
use anyhow::Context;
use anyhow::Result;

/// Default path for the soul memory database.
pub const SOUL_DB_PATH: &str = "data/soul_memory.db";

/// Default path for the trade price SQLite cache.
pub const TRADE_PRICE_DB_PATH: &str = "data/trade_prices.db";
/// Default path for the Ghidra analysis SQLite cache.
pub const GHIDRA_DB_PATH: &str = "data/ghidra.db";

/// Path to the opcodes config file imported into the Ghidra DB at startup.
pub const OPCODES_CONFIG_PATH: &str = "config/opcodes.json";

/// Get the base address of eqgame.exe module in the target process.
///
/// # Errors
///
/// Returns an error if the operation fails.
#[cfg(windows)]
pub fn get_module_base(proc: &process::memory::ProcessHandle) -> Result<u64> {
    use windows::Win32::{
        Foundation::CloseHandle,
        System::{
            ProcessStatus::{EnumProcessModulesEx, LIST_MODULES_ALL},
            Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
        },
    };

    let handle =
        unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, proc.pid) }
            .context("Failed to open process for module enumeration")?;

    let mut modules = [windows::Win32::Foundation::HMODULE::default(); 1024];
    let mut bytes_needed: u32 = 0;

    // SAFETY: `handle` is a valid process handle opened with
    // PROCESS_QUERY_INFORMATION and PROCESS_VM_READ. `modules` is a
    // stack-allocated array of sufficient size. `bytes_needed` receives the
    // count of bytes written.
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
    // SAFETY: `handle` is a valid, open handle that we own. Closing it once is
    // correct.
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

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn non_windows_module_base_uses_preferred_base() {
        let handle = process::memory::ProcessHandle::open(42).expect("stub process open");
        let base = get_module_base(&handle).expect("stub module base");

        assert_eq!(base, textquest_common::offsets::EQ_PREFERRED_BASE);
    }
}
