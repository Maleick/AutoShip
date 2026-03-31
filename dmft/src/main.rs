// --- Modules wired through orchestrator/TUI, not referenced directly in main ---
#[allow(dead_code)] // M4: camp loop state machine, driven by orchestrator
mod camp;
mod cli;
#[allow(dead_code)] // M2: multi-client sessions, self-healing monitor
mod client;
#[allow(dead_code)] // M4: combat automation, class strategies
mod combat;
mod config;
#[allow(dead_code)] // M2.5: encrypted credential store
mod credentials;
#[allow(dead_code)] // M2.5: login automation, process spawner
mod launcher;

// --- Modules used in main.rs; dead_code on non-Windows from platform stubs ---
#[cfg_attr(not(windows), allow(dead_code))]
mod eq;
#[cfg_attr(not(windows), allow(dead_code))]
mod inject;
#[cfg_attr(not(windows), allow(dead_code))]
mod ipc;
#[cfg_attr(not(windows), allow(dead_code))]
mod nav;
mod orchestrator;
#[cfg_attr(not(windows), allow(dead_code))]
mod process;
#[cfg_attr(not(windows), allow(dead_code))]
mod soul;
mod tui;

use anyhow::{Context, Result};
#[cfg(not(windows))]
use tracing::warn;
use tracing_appender::rolling;
use tracing_subscriber::{EnvFilter, fmt};

/// Default path for the soul memory database.
const SOUL_DB_PATH: &str = "data/soul_memory.db";

fn main() -> Result<()> {
    // Set up file logging — must be done before anything else.
    let log_dir = std::env::current_dir().unwrap_or_default().join("logs");
    std::fs::create_dir_all(&log_dir).ok();
    let file_appender = rolling::RollingFileAppender::builder()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix("dmft.log")
        .max_log_files(7) // Keep 1 week of logs
        .build(&log_dir)
        .unwrap_or_else(|_| rolling::daily(&log_dir, "dmft.log"));
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    tracing::info!("DMFT orchestrator starting");

    let args: Vec<String> = std::env::args().collect();
    let dump_mode = args.iter().any(|a| a == "--dump");
    let inject_mode = args.iter().any(|a| a == "--inject" || a == "inject");
    let inject_pid_mode = args.iter().position(|a| a == "--inject-pid");
    let calibrate_mode = args.iter().any(|a| a == "--calibrate");
    let login_mode = args.iter().position(|a| a == "--login");
    let login_pid_mode = args.iter().position(|a| a == "--login-pid");
    let cmd_mode = args.iter().position(|a| a == "--cmd");
    let nav_mode = args.iter().position(|a| a == "--nav");
    let navpath_mode = args.iter().position(|a| a == "--navpath");
    let navall_mode = args.iter().position(|a| a == "--navall");
    let status_mode = args.iter().position(|a| a == "--status");
    let statusall_mode = args.iter().any(|a| a == "--statusall");
    let zones_mode = args.iter().position(|a| a == "--zones");

    if let Some(pos) = zones_mode {
        let pid: u32 = args
            .get(pos + 1)
            .context("--zones requires: --zones <PID>")?
            .parse()
            .context("PID must be a number")?;
        cli::run_zones_mode(pid)
    } else if statusall_mode {
        cli::run_statusall_mode()
    } else if let Some(pos) = status_mode {
        let pid: u32 = args
            .get(pos + 1)
            .context("--status requires: --status <PID>")?
            .parse()
            .context("PID must be a number")?;
        cli::run_status_mode(pid)
    } else if calibrate_mode {
        cli::run_calibrate_mode()
    } else if let Some(pos) = login_pid_mode {
        // --login-pid <PID> <account> [server] [character]
        let pid: u32 = args
            .get(pos + 1)
            .context(
                "--login-pid requires: --login-pid <PID> <account> [server] [character]",
            )?
            .parse()
            .context("PID must be a number")?;
        let account = args
            .get(pos + 2)
            .context("--login-pid requires: --login-pid <PID> <account>")?
            .clone();
        let password = rpassword::prompt_password("Password: ")
            .context("Failed to read password")?;
        let server = args
            .get(pos + 3)
            .cloned()
            .unwrap_or_else(|| "Firiona Vie".to_string());
        let character = args.get(pos + 4).cloned().unwrap_or_default();
        cli::run_login_pid_mode(pid, &account, &password, &server, &character)
    } else if let Some(pos) = login_mode {
        // --login <account> [server] [character]
        let account = args
            .get(pos + 1)
            .context("--login requires: --login <account> [server] [character]")?
            .clone();
        let password = rpassword::prompt_password("Password: ")
            .context("Failed to read password")?;
        let server = args
            .get(pos + 2)
            .cloned()
            .unwrap_or_else(|| "Firiona Vie".to_string());
        let character = args.get(pos + 3).cloned().unwrap_or_default();
        cli::run_login_mode(&account, &password, &server, &character)
    } else if let Some(pos) = cmd_mode {
        // --cmd <pid> "<slash command>"
        let pid: u32 = args
            .get(pos + 1)
            .context("--cmd requires: --cmd <pid> <command>")?
            .parse()
            .context("PID must be a number")?;
        let command = args
            .get(pos + 2)
            .context("--cmd requires: --cmd <pid> <command>")?
            .clone();
        cli::run_cmd_mode(pid, &command)
    } else if let Some(pos) = nav_mode {
        // --nav <PID> <x> <y> <z> — navigate to coordinates
        let pid: u32 = args
            .get(pos + 1)
            .context("--nav requires: --nav <PID> <x> <y> <z>")?
            .parse()
            .context("PID must be a number")?;
        let x: f32 = args
            .get(pos + 2)
            .context("--nav requires: --nav <PID> <x> <y> <z>")?
            .parse()
            .context("x must be a number")?;
        let y: f32 = args
            .get(pos + 3)
            .context("--nav requires: --nav <PID> <x> <y> <z>")?
            .parse()
            .context("y must be a number")?;
        let z: f32 = args
            .get(pos + 4)
            .context("--nav requires: --nav <PID> <x> <y> <z>")?
            .parse()
            .context("z must be a number")?;
        cli::run_nav_mode(pid, x, y, z)
    } else if let Some(pos) = navall_mode {
        // --navall <x> <y> <z> — navigate all EQ clients to coordinates
        let x: f32 = args
            .get(pos + 1)
            .context("--navall requires: --navall <x> <y> <z>")?
            .parse()
            .context("x must be a number")?;
        let y: f32 = args
            .get(pos + 2)
            .context("--navall requires: --navall <x> <y> <z>")?
            .parse()
            .context("y must be a number")?;
        let z: f32 = args
            .get(pos + 3)
            .context("--navall requires: --navall <x> <y> <z>")?
            .parse()
            .context("z must be a number")?;
        cli::run_navall_mode(x, y, z)
    } else if let Some(pos) = inject_pid_mode {
        // --inject-pid <PID> — inject into a specific process only
        let pid: u32 = args
            .get(pos + 1)
            .context("--inject-pid requires: --inject-pid <PID>")?
            .parse()
            .context("PID must be a number")?;
        cli::run_inject_pid_mode(pid)
    } else if inject_mode {
        cli::run_inject_mode()
    } else if let Some(pos) = navpath_mode {
        // --navpath <zone> <x1> <y1> <z1> <x2> <y2> <z2>
        let zone = args
            .get(pos + 1)
            .context("--navpath requires: --navpath <zone> <x1> <y1> <z1> <x2> <y2> <z2>")?
            .clone();
        let x1: f32 = args
            .get(pos + 2)
            .context("missing x1")?
            .parse()
            .context("x1 not a number")?;
        let y1: f32 = args
            .get(pos + 3)
            .context("missing y1")?
            .parse()
            .context("y1 not a number")?;
        let z1: f32 = args
            .get(pos + 4)
            .context("missing z1")?
            .parse()
            .context("z1 not a number")?;
        let x2: f32 = args
            .get(pos + 5)
            .context("missing x2")?
            .parse()
            .context("x2 not a number")?;
        let y2: f32 = args
            .get(pos + 6)
            .context("missing y2")?
            .parse()
            .context("y2 not a number")?;
        let z2: f32 = args
            .get(pos + 7)
            .context("missing z2")?
            .parse()
            .context("z2 not a number")?;
        cli::run_navpath_mode(&zone, (x1, y1, z1), (x2, y2, z2))
    } else if dump_mode {
        cli::run_dump_mode()
    } else {
        cli::run_tui_mode()
    }
}

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
    let _ = unsafe { CloseHandle(handle) };

    Ok(base)
}

#[cfg(not(windows))]
pub fn get_module_base(_proc: &process::memory::ProcessHandle) -> Result<u64> {
    warn!("Using preferred base address (non-Windows stub)");
    Ok(dmft_common::offsets::EQ_PREFERRED_BASE)
}
