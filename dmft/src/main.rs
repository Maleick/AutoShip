#![allow(dead_code)]

mod client;
mod config;
mod eq;
mod inject;
mod ipc;
mod process;
mod tui;

use anyhow::{Context, Result};
use std::path::Path;
use tracing::{info, warn, error};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let dump_mode = args.iter().any(|a| a == "--dump");

    if dump_mode {
        run_dump_mode()
    } else {
        run_tui_mode()
    }
}

/// TUI mode — the default. Shows ShowEQ-inspired live dashboard.
fn run_tui_mode() -> Result<()> {
    let mut app = tui::app::App::new();

    // Try to attach to an EQ process before launching TUI
    #[cfg(windows)]
    {
        let config = load_config()?;
        if let Ok(pids) = process::memory::find_processes_by_name(&config.process_name) {
            if let Some(&pid) = pids.first() {
                if let Ok(proc) = process::memory::ProcessHandle::open(pid) {
                    if let Ok(base) = get_module_base(&proc) {
                        app.attached_pid = Some(pid);
                        app.eq_base = base;
                        app.status_message = format!("Attached to PID {} (base {:#x})", pid, base);
                    }
                }
            }
        }
        if app.attached_pid.is_none() {
            app.status_message = String::from("No EQ process found — waiting...");
        }
    }

    #[cfg(not(windows))]
    {
        app.status_message = String::from("DEMO MODE — macOS build (no EQ process)");
    }

    tui::run::run_tui(app)
}

/// Dump mode (--dump) — one-shot CLI output, the original M1 behavior.
fn run_dump_mode() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    info!("Frostreaver v{} — EQ Memory Reader", env!("CARGO_PKG_VERSION"));

    let config = load_config()?;

    info!(process = %config.process_name, "Looking for EQ processes...");

    let pids = process::memory::find_processes_by_name(&config.process_name)?;

    if pids.is_empty() {
        warn!("No {} processes found.", config.process_name);
        warn!("(On non-Windows, this tool only works as a build/structure check.)");
        return Ok(());
    }

    info!(count = pids.len(), "Found EQ processes: {:?}", pids);

    let pid = pids[0];
    info!(pid, "Attaching to first EQ process...");
    let proc = process::memory::ProcessHandle::open(pid)
        .context("Failed to open EQ process")?;

    let eq_base = get_module_base(&proc)?;
    info!(base = format!("{:#x}", eq_base), "eqgame.exe base address");

    // Read local player
    info!("═══════════════════════════════════════");
    info!("LOCAL PLAYER");
    info!("═══════════════════════════════════════");
    match eq::spawn::read_local_player(&proc, eq_base) {
        Ok(player) => info!("{}", player),
        Err(e) => error!("Failed to read local player: {:#}", e),
    }

    // Read current target
    info!("═══════════════════════════════════════");
    info!("CURRENT TARGET");
    info!("═══════════════════════════════════════");
    match eq::spawn::read_target(&proc, eq_base) {
        Ok(Some(target)) => info!("{}", target),
        Ok(None) => info!("No target selected"),
        Err(e) => error!("Failed to read target: {:#}", e),
    }

    // Read nearby spawns
    info!("═══════════════════════════════════════");
    info!("SPAWN LIST (first 50)");
    info!("═══════════════════════════════════════");
    match eq::spawn::read_all_spawns(&proc, eq_base, config.max_spawns) {
        Ok(spawns) => {
            info!("Total spawns in zone: {}", spawns.len());
            for (i, spawn) in spawns.iter().take(50).enumerate() {
                info!("  [{:3}] {}", i, spawn);
            }
        }
        Err(e) => error!("Failed to read spawn list: {:#}", e),
    }

    info!("Done.");
    Ok(())
}

fn load_config() -> Result<config::AppConfig> {
    let config_path = Path::new("config/frostreaver.toml");
    if config_path.exists() {
        config::AppConfig::load(config_path).context("Failed to load configuration")
    } else {
        Ok(config::AppConfig::default_config())
    }
}

/// Get the base address of eqgame.exe module in the target process.
#[cfg(windows)]
fn get_module_base(proc: &process::memory::ProcessHandle) -> Result<u64> {
    use windows::Win32::System::ProcessStatus::{EnumProcessModulesEx, LIST_MODULES_ALL};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
    use windows::Win32::Foundation::CloseHandle;

    let handle = unsafe {
        OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, proc.pid)
    }.context("Failed to open process for module enumeration")?;

    let mut modules = [windows::Win32::Foundation::HMODULE::default(); 1024];
    let mut bytes_needed: u32 = 0;

    unsafe {
        EnumProcessModulesEx(
            handle,
            modules.as_mut_ptr(),
            std::mem::size_of_val(&modules) as u32,
            &mut bytes_needed,
            LIST_MODULES_ALL,
        )
    }.context("EnumProcessModulesEx failed")?;

    let base = modules[0].0 as u64;
    let _ = unsafe { CloseHandle(handle) };

    Ok(base)
}

#[cfg(not(windows))]
fn get_module_base(_proc: &process::memory::ProcessHandle) -> Result<u64> {
    warn!("Using preferred base address (non-Windows stub)");
    Ok(eq::offsets::EQ_PREFERRED_BASE)
}
