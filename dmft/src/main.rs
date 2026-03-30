// Modules for future milestones or Windows-only functionality.
// Suppressed per-module instead of crate-wide to keep active code checked.
#[allow(dead_code)]
mod camp;
#[allow(dead_code)]
mod client;
#[allow(dead_code)]
mod combat;
mod config;
#[allow(dead_code)]
mod credentials;
#[allow(dead_code)]
mod eq;
#[allow(dead_code)]
mod inject;
#[allow(dead_code)]
mod ipc;
#[allow(dead_code)]
mod launcher;
#[allow(dead_code)]
mod nav;
mod orchestrator;
#[allow(dead_code)]
mod process;
#[allow(dead_code)]
mod soul;
mod tui;

use anyhow::{Context, Result};
use std::path::Path;
use tracing::{info, warn, error};
use tracing_subscriber::{fmt, EnvFilter};
use tracing_appender::rolling;

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

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

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
    let status_mode = args.iter().position(|a| a == "--status");

    if let Some(pos) = status_mode {
        let pid: u32 = args.get(pos + 1)
            .context("--status requires: --status <PID>")?
            .parse()
            .context("PID must be a number")?;
        return run_status_mode(pid);
    } else if calibrate_mode {
        run_calibrate_mode()
    } else if let Some(pos) = login_pid_mode {
        // --login-pid <PID> <account> <password> [server] [character]
        let pid: u32 = args.get(pos + 1)
            .context("--login-pid requires: --login-pid <PID> <account> <password> [server] [character]")?
            .parse()
            .context("PID must be a number")?;
        let account = args.get(pos + 2)
            .context("--login-pid requires: --login-pid <PID> <account> <password>")?
            .clone();
        let password = args.get(pos + 3)
            .context("--login-pid requires: --login-pid <PID> <account> <password>")?
            .clone();
        let server = args.get(pos + 4)
            .cloned()
            .unwrap_or_else(|| "Firiona Vie".to_string());
        let character = args.get(pos + 5)
            .cloned()
            .unwrap_or_default();
        run_login_pid_mode(pid, &account, &password, &server, &character)
    } else if let Some(pos) = login_mode {
        // --login <account> <password> [server] [character]
        let account = args.get(pos + 1)
            .context("--login requires: --login <account> <password> [server] [character]")?
            .clone();
        let password = args.get(pos + 2)
            .context("--login requires: --login <account> <password>")?
            .clone();
        let server = args.get(pos + 3)
            .cloned()
            .unwrap_or_else(|| "Firiona Vie".to_string());
        let character = args.get(pos + 4)
            .cloned()
            .unwrap_or_default();
        run_login_mode(&account, &password, &server, &character)
    } else if let Some(pos) = cmd_mode {
        // --cmd <pid> "<slash command>"
        let pid: u32 = args.get(pos + 1)
            .context("--cmd requires: --cmd <pid> <command>")?
            .parse()
            .context("PID must be a number")?;
        let command = args.get(pos + 2)
            .context("--cmd requires: --cmd <pid> <command>")?
            .clone();
        run_cmd_mode(pid, &command)
    } else if let Some(pos) = nav_mode {
        // --nav <PID> <x> <y> <z> — navigate to coordinates
        let pid: u32 = args.get(pos + 1)
            .context("--nav requires: --nav <PID> <x> <y> <z>")?
            .parse()
            .context("PID must be a number")?;
        let x: f32 = args.get(pos + 2)
            .context("--nav requires: --nav <PID> <x> <y> <z>")?
            .parse()
            .context("x must be a number")?;
        let y: f32 = args.get(pos + 3)
            .context("--nav requires: --nav <PID> <x> <y> <z>")?
            .parse()
            .context("y must be a number")?;
        let z: f32 = args.get(pos + 4)
            .context("--nav requires: --nav <PID> <x> <y> <z>")?
            .parse()
            .context("z must be a number")?;
        run_nav_mode(pid, x, y, z)
    } else if let Some(pos) = inject_pid_mode {
        // --inject-pid <PID> — inject into a specific process only
        let pid: u32 = args.get(pos + 1)
            .context("--inject-pid requires: --inject-pid <PID>")?
            .parse()
            .context("PID must be a number")?;
        run_inject_pid_mode(pid)
    } else if inject_mode {
        run_inject_mode()
    } else if dump_mode {
        run_dump_mode()
    } else {
        run_tui_mode()
    }
}

/// TUI mode — the default. Shows ShowEQ-inspired live dashboard.
fn run_tui_mode() -> Result<()> {
    let mut app = tui::app::App::new();
    let config = load_config()?;

    // Set server name from config
    app.server_name = config.server.name.clone();

    // Try to attach to ALL EQ processes before launching TUI
    #[cfg(windows)]
    {
        if let Ok(pids) = process::memory::find_processes_by_name(&config.process_name) {
            for &pid in &pids {
                if let Ok(proc) = process::memory::ProcessHandle::open(pid) {
                    if let Ok(base) = get_module_base(&proc) {
                        let client = tui::app::ClientState::new(pid, base);
                        info!(pid, base = format!("{:#x}", base), "Attached to EQ client");
                        app.clients.push(client);
                    }
                }
            }
        }
        let count = app.clients.len();
        if count > 0 {
            app.status_message = format!("{} EQ client{} attached", count, if count == 1 { "" } else { "s" });
            app.sync_from_selected_client();
        } else {
            app.status_message = String::from("No EQ process found — scanning...");
        }
    }

    #[cfg(not(windows))]
    {
        app.status_message = String::from("DEMO MODE — macOS build (no EQ process)");
    }

    // Initialize Soul Engine if enabled
    if config.soul.enabled {
        let db_path = Path::new(SOUL_DB_PATH);
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        match soul::coordinator::SoulCoordinator::new(config.soul, db_path) {
            Ok(coordinator) => {
                info!("Soul Engine initialized");
                app.soul_coordinator = Some(coordinator);
            }
            Err(e) => {
                warn!("Soul Engine failed to initialize: {}", e);
            }
        }
    }

    let orchestrator = orchestrator::Orchestrator::new();
    tui::run::run_tui(app, orchestrator)
}

/// Inject mode (--inject) — find eqgame.exe processes and inject dmft_dll.dll into each.
fn run_inject_mode() -> Result<()> {
    info!("DMFT inject mode — finding EQ processes...");

    let config = load_config()?;
    let pids = process::memory::find_processes_by_name(&config.process_name)?;

    if pids.is_empty() {
        println!("No {} processes found. Launch EQ first.", config.process_name);
        return Ok(());
    }

    println!("Found {} EQ process(es): {:?}", pids.len(), pids);

    // Locate the DLL — check release first, then debug
    let project_dir = std::env::current_dir().unwrap_or_default();
    let dll_candidates = [
        project_dir.join("target/release/dmft_dll.dll"),
        project_dir.join("target/debug/dmft_dll.dll"),
    ];

    let source_dll = dll_candidates
        .iter()
        .find(|p| p.exists())
        .ok_or_else(|| anyhow::anyhow!(
            "Cannot find dmft_dll.dll. Run `cargo build --release` first."
        ))?;

    println!("Using DLL: {}", source_dll.display());

    // Stage the DLL (copies with randomized name)
    let staged_dll = inject::dll_prep::prepare_dll(source_dll)?;
    println!("Staged DLL: {}", staged_dll.display());

    let mut success = 0u32;
    let mut failed = 0u32;

    for &pid in &pids {
        print!("Injecting into PID {}... ", pid);
        // Write session token before injection so DLL can read it during init.
        if let Err(e) = write_session_token_file(pid) {
            println!("WARN: token write failed: {:#}", e);
        }
        match inject::loader::inject_dll(pid, &staged_dll) {
            Ok(()) => {
                println!("OK");
                info!(pid, "Injection succeeded");
                success += 1;
            }
            Err(e) => {
                println!("FAILED: {:#}", e);
                error!(pid, error = %e, "Injection failed");
                failed += 1;
            }
        }
    }

    println!();
    println!("Results: {} succeeded, {} failed", success, failed);
    println!();
    println!("Log locations:");
    println!("  Orchestrator: logs/dmft.log");
    #[cfg(windows)]
    {
        let temp = std::env::temp_dir();
        println!("  DLL (injected): {}\\dmft\\dmft-dll.log", temp.display());
    }
    #[cfg(not(windows))]
    {
        println!("  DLL (injected): $TMPDIR/dmft/dmft-dll.log");
    }
    println!();
    println!("Run scripts\\verify_injection.bat to check injection status.");

    Ok(())
}

/// Status mode (--status <PID>) — read shared memory and print player state.
fn run_status_mode(pid: u32) -> Result<()> {
    let reader = ipc::shared::SharedStateReader::new(pid)
        .context(format!("Cannot open shared memory for PID {} — is the DLL injected?", pid))?;

    match reader.read() {
        Some(state) => {
            if let Some(ref player) = state.local_player {
                println!("Player: {} (ID: {})", player.name, player.spawn_id);
                println!("Position: x={:.1}, y={:.1}, z={:.1} heading={:.1}", player.x, player.y, player.z, player.heading);
                println!("HP: {}/{} ({:.0}%)", player.hp_current, player.hp_max, player.hp_pct());
                println!("Mana: {}/{}", player.mana_current, player.mana_max);
                println!("Level: {} Class: {}", player.level, player.class_id);
                println!("Nav: {:?}", state.nav_status);
            } else {
                println!("No player data (not in world?)");
            }
            if let Some(ref target) = state.target {
                println!("Target: {} (ID: {}) HP: {:.0}%", target.name, target.spawn_id, target.hp_pct());
            }
            println!("Nearby spawns: {}", state.nearby_spawns.len());
        }
        None => {
            println!("No data from shared memory (DLL hasn't written yet or write in progress)");
        }
    }
    Ok(())
}

/// Navigate mode (--nav <PID> <x> <y> <z>) — send NavigateTo to a specific client.
fn run_nav_mode(pid: u32, x: f32, y: f32, z: f32) -> Result<()> {
    use dmft_common::ipc::Command;
    use dmft_common::nav::Waypoint;

    println!("Sending NavigateTo PID {} -> ({}, {}, {})", pid, x, y, z);

    let pipe = ipc::pipe::CommandPipe::connect(pid)
        .context(format!("Cannot connect to PID {} — is the DLL injected?", pid))?;

    let token = generate_session_token(pid);
    pipe.send_raw_token(&token)
        .context(format!("Failed to auth with PID {}", pid))?;

    let cmd = Command::NavigateTo {
        waypoints: vec![Waypoint::new(x, y, z)],
    };
    pipe.send_async(&cmd)
        .context(format!("Failed to send NavigateTo to PID {}", pid))?;

    println!("NavigateTo sent — character should start moving.");
    Ok(())
}

/// Inject mode targeting a specific PID (--inject-pid <PID>).
fn run_inject_pid_mode(pid: u32) -> Result<()> {
    info!(pid, "DMFT inject-pid mode — targeting single process");

    let project_dir = std::env::current_dir().unwrap_or_default();
    let dll_candidates = [
        project_dir.join("target/release/dmft_dll.dll"),
        project_dir.join("target/debug/dmft_dll.dll"),
    ];

    let source_dll = dll_candidates
        .iter()
        .find(|p| p.exists())
        .ok_or_else(|| anyhow::anyhow!(
            "Cannot find dmft_dll.dll. Run `cargo build --release` first."
        ))?;

    // Write session token file BEFORE injection so DLL can read it during init.
    write_session_token_file(pid)?;

    let staged_dll = inject::dll_prep::prepare_dll(source_dll)?;
    println!("Injecting into PID {}...", pid);

    inject::loader::inject_dll(pid, &staged_dll)?;
    println!("OK — DLL injected into PID {}", pid);
    Ok(())
}

/// Login mode targeting a specific PID (--login-pid <PID> <account> <password> [server] [character]).
fn run_login_pid_mode(pid: u32, account: &str, password: &str, server: &str, character: &str) -> Result<()> {
    use dmft_common::ipc::Command;

    println!("Sending StartLogin to PID {} (account: {}, server: {})...", pid, account, server);

    let pipe = ipc::pipe::CommandPipe::connect(pid)
        .context(format!("Cannot connect to PID {} — is the DLL injected?", pid))?;

    let token = generate_session_token(pid);
    pipe.send_raw_token(&token)
        .context(format!("Failed to auth with PID {}", pid))?;

    let cmd = Command::StartLogin {
        account_name: account.to_string(),
        password: password.to_string(),
        server_name: server.to_string(),
        character_name: character.to_string(),
    };
    pipe.send_async(&cmd)
        .context(format!("Failed to send StartLogin to PID {}", pid))?;

    println!("StartLogin sent to PID {}", pid);
    Ok(())
}

/// Login mode (--login <account> <password> [server] [character]) — send StartLogin to all injected EQ clients.
fn run_login_mode(account: &str, password: &str, server: &str, character: &str) -> Result<()> {
    use dmft_common::ipc::Command;

    let config = load_config()?;
    let pids = process::memory::find_processes_by_name(&config.process_name)?;

    if pids.is_empty() {
        println!("No EQ processes found. Launch EQ first, then inject, then login.");
        return Ok(());
    }

    for &pid in &pids {
        println!("Sending StartLogin to PID {} (account: {}, server: {})...", pid, account, server);

        match ipc::pipe::CommandPipe::connect(pid) {
            Ok(pipe) => {
                let token = generate_session_token(pid);
                if pipe.send_raw_token(&token).is_err() {
                    println!("  Failed to auth with PID {}", pid);
                    continue;
                }

                let cmd = Command::StartLogin {
                    account_name: account.to_string(),
                    password: password.to_string(),
                    server_name: server.to_string(),
                    character_name: character.to_string(),
                };
                if pipe.send_async(&cmd).is_ok() {
                    println!("  StartLogin sent to PID {}", pid);
                } else {
                    println!("  Failed to send StartLogin to PID {}", pid);
                }
            }
            Err(_) => {
                println!("  Cannot connect to PID {} — is the DLL injected?", pid);
            }
        }
    }

    println!("\nLogin commands sent. Check DLL log for progress.");
    Ok(())
}

/// Calibrate mode (--calibrate) — find all EQ processes and send calibrate_login to each.
fn run_calibrate_mode() -> Result<()> {
    use dmft_common::ipc::Command;

    let config = load_config()?;
    let pids = process::memory::find_processes_by_name(&config.process_name)?;

    if pids.is_empty() {
        println!("No EQ processes found. Launch EQ first, then inject, then calibrate.");
        return Ok(());
    }

    for &pid in &pids {
        println!("Sending calibrate_login to PID {}...", pid);

        match ipc::pipe::CommandPipe::connect(pid) {
            Ok(pipe) => {
                let token = generate_session_token(pid);
                if pipe.send_raw_token(&token).is_err() {
                    println!("  Failed to auth with PID {}", pid);
                    continue;
                }

                let cmd = Command::CalibrateLogin;
                if pipe.send_async(&cmd).is_ok() {
                    println!("  Calibration sent to PID {}", pid);
                } else {
                    println!("  Failed to send calibration to PID {}", pid);
                }
            }
            Err(_) => {
                println!("  Cannot connect to PID {} — is the DLL injected?", pid);
            }
        }
    }

    println!("\nCalibration complete. Check DLL log at %TEMP%\\dmft\\dmft-dll.log");
    Ok(())
}

/// Command mode (--cmd <pid> <command>) — send a slash command to an injected client.
fn run_cmd_mode(pid: u32, command: &str) -> Result<()> {
    use dmft_common::ipc::Command;

    println!("Sending command to PID {}: {}", pid, command);

    let pipe = ipc::pipe::CommandPipe::connect(pid)
        .with_context(|| format!("Failed to connect to PID {}. Is the DLL injected?", pid))?;

    // Send the session token first (handshake).
    // For now, use the same PID-derived token the DLL generates.
    let token = generate_session_token(pid);
    pipe.send_raw_token(&token)
        .context("Failed to send session token")?;

    // Send the slash command (fire-and-forget — DLL disconnects pipe after read).
    let cmd = Command::SlashCommand {
        command: command.to_string(),
    };

    pipe.send_async(&cmd)
        .context("Failed to send command")?;

    println!("Command sent successfully.");

    Ok(())
}

/// Generate the same session token the DLL uses (PID-derived, for testing).
/// Write a CSPRNG session token file for the given PID. The DLL reads this during init.
/// Must be called BEFORE injection.
fn write_session_token_file(pid: u32) -> Result<()> {
    let token_dir = std::env::temp_dir().join("dmft");
    std::fs::create_dir_all(&token_dir)?;
    let token_path = token_dir.join(format!("token_{}.bin", pid));

    use rand::RngCore;
    let mut token = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut token);

    std::fs::write(&token_path, &token)?;
    // Also cache in memory for later --login-pid calls in the same process
    // (not needed — separate process invocations. Write a second copy for login to read.)
    let login_token_path = token_dir.join(format!("login_token_{}.bin", pid));
    std::fs::write(&login_token_path, &token)?;

    info!(pid, "Session token written to {}", token_path.display());
    Ok(())
}

/// Read the session token for authenticating with an already-injected DLL.
/// The token was written by --inject-pid before injection.
fn generate_session_token(pid: u32) -> [u8; 32] {
    let token_path = std::env::temp_dir()
        .join("dmft")
        .join(format!("login_token_{}.bin", pid));

    if let Ok(data) = std::fs::read(&token_path) {
        if data.len() == 32 {
            let mut token = [0u8; 32];
            token.copy_from_slice(&data);
            return token;
        }
    }

    // Fallback: PID-derived (won't match DLL's random token — will fail auth)
    warn!(pid, "No login token file found — auth will likely fail");
    let pid_bytes = pid.to_le_bytes();
    let mut token = [0u8; 32];
    for (i, byte) in token.iter_mut().enumerate() {
        *byte = pid_bytes[i % 4] ^ (i as u8);
    }
    token
}

/// Dump mode (--dump) — one-shot CLI output, the original M1 behavior.
fn run_dump_mode() -> Result<()> {
    info!("Frostreaver v{} — EQ Memory Reader (dump mode)", env!("CARGO_PKG_VERSION"));

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

    // Diagnostic hex dump of SpawnManager and spawn list structure
    dump_spawn_list_diagnostic(&proc, eq_base);

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

/// Format a byte buffer as a hex dump with offset labels.
fn format_hex_dump(base_addr: usize, bytes: &[u8]) -> String {
    let mut lines = Vec::new();
    for (i, chunk) in bytes.chunks(16).enumerate() {
        let offset = i * 16;
        let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
        let ascii: String = chunk.iter().map(|&b| {
            if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' }
        }).collect();
        let hex_str = if hex.len() < 16 {
            let mut s = hex.join(" ");
            for _ in hex.len()..16 { s.push_str("   "); }
            s
        } else {
            hex.join(" ")
        };
        lines.push(format!("  {:#010x} (+{:#04x}): {}  |{}|",
            base_addr + offset, offset, hex_str, ascii));
    }
    lines.join("\n")
}

/// Diagnostic hex dump of SpawnManager, the TList, and the first spawn node.
/// Helps debug why the NEXT pointer reads as 0x0 after the first spawn.
#[allow(unused_variables)]
fn dump_spawn_list_diagnostic(proc: &process::memory::ProcessHandle, eq_base: u64) {
    use dmft_common::offsets::{self, spawn_manager};

    info!("===================================================");
    info!("SPAWN LIST DIAGNOSTIC HEX DUMP");
    info!("===================================================");

    // Step 1: Read SpawnManager pointer
    let mgr_ptr_addr = match offsets::rebase(offsets::PINST_SPAWN_MANAGER, eq_base) {
        Some(addr) => addr,
        None => { error!("Failed to rebase pinstSpawnManager"); return; }
    };
    let mgr_addr = match proc.read_ptr(mgr_ptr_addr) {
        Ok(addr) => addr,
        Err(e) => { error!("Failed to read pinstSpawnManager: {:#}", e); return; }
    };
    info!("pinstSpawnManager ptr at {:#x} -> SpawnManager at {:#x}", mgr_ptr_addr, mgr_addr);

    if mgr_addr == 0 {
        error!("SpawnManager is null -- not in a zone?");
        return;
    }

    // Step 2: Dump first 64 bytes of SpawnManager to find all list pointers
    info!("--- SpawnManager first 64 bytes ---");
    match proc.read_bytes(mgr_addr, 64) {
        Ok(bytes) => {
            info!("\n{}", format_hex_dump(mgr_addr, &bytes));
            // Interpret as 8 sequential u64 values
            for i in 0..8 {
                let off = i * 8;
                if off + 8 <= bytes.len() {
                    let val = u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
                    let looks_like_ptr = val > 0x10000 && val < 0x7FFF_FFFF_FFFF;
                    info!("  SpawnManager+{:#04x}: {:#018x} {}",
                        off, val,
                        if looks_like_ptr { "<-- looks like a pointer" } else { "" });
                }
            }
        }
        Err(e) => error!("Failed to read SpawnManager bytes: {:#}", e),
    }

    // Step 3: Read the TList at SpawnManager+PLAYER_LIST (0x10)
    let list_addr = mgr_addr + spawn_manager::PLAYER_LIST;
    info!("--- TList at SpawnManager+{:#x} = {:#x} ---",
        spawn_manager::PLAYER_LIST, list_addr);
    match proc.read_bytes(list_addr, 16) {
        Ok(bytes) => {
            info!("\n{}", format_hex_dump(list_addr, &bytes));
            if bytes.len() >= 16 {
                let first_node = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
                let last_node = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
                info!("  TList.m_pFirstNode: {:#x}", first_node);
                info!("  TList.m_pLastNode:  {:#x}", last_node);
            }
        }
        Err(e) => error!("Failed to read TList bytes: {:#}", e),
    }

    // Step 4: Read the first node pointer from TList
    let first_node = match proc.read_ptr(list_addr) {
        Ok(addr) => addr,
        Err(e) => { error!("Failed to read first node: {:#}", e); return; }
    };

    if first_node == 0 {
        error!("First node is null -- spawn list empty?");
        return;
    }

    info!("First spawn node at: {:#x}", first_node);

    // Step 5: Dump first 64 bytes of the first spawn (covers TListNode + vtable area)
    info!("--- First spawn: first 64 bytes (TListNode region + beyond) ---");
    match proc.read_bytes(first_node, 64) {
        Ok(bytes) => {
            info!("\n{}", format_hex_dump(first_node, &bytes));
            for &(off, label) in &[
                (0usize, "m_pPrev / PREV"),
                (8, "m_pNext / NEXT"),
                (16, "m_pList"),
                (24, "+0x18 unknown"),
            ] {
                if off + 8 <= bytes.len() {
                    let val = u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
                    let looks_like_ptr = val > 0x10000 && val < 0x7FFF_FFFF_FFFF;
                    info!("  +{:#04x} ({}): {:#018x} {}",
                        off, label, val,
                        if looks_like_ptr { "<-- valid pointer" }
                        else if val == 0 { "<-- NULL" }
                        else { "" });
                }
            }
        }
        Err(e) => error!("Failed to read first spawn bytes: {:#}", e),
    }

    // Step 6: Verify this IS a PlayerClient by reading the name at known offset
    match proc.read_string(
        first_node + dmft_common::offsets::player_base::NAME, 64,
    ) {
        Ok(name) => info!("  Name at +{:#x}: \"{}\"",
            dmft_common::offsets::player_base::NAME, name),
        Err(e) => error!("  Failed to read name: {:#}", e),
    }

    // Step 7: Probe offsets +0x00 through +0x38 for valid pointers to other spawns
    info!("--- Probing offsets +0x00..+0x38 on first spawn for valid pointers ---");
    for offset in [0x00usize, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38] {
        match proc.read_ptr(first_node + offset) {
            Ok(val) => {
                let looks_like_ptr = val > 0x10000 && val < 0x7FFF_FFFF_FFFF;
                if looks_like_ptr {
                    // Try reading a name to confirm it points to another PlayerClient
                    let name_check = proc
                        .read_string(val + dmft_common::offsets::player_base::NAME, 64)
                        .ok()
                        .filter(|n| {
                            !n.is_empty()
                                && n.chars().all(|c| c.is_ascii_graphic() || c == ' ')
                        })
                        .map(|n| format!(" -> name=\"{}\"", n))
                        .unwrap_or_default();
                    info!("  +{:#04x}: {:#018x} <-- VALID PTR{}", offset, val, name_check);
                } else if val == 0 {
                    info!("  +{:#04x}: NULL", offset);
                } else {
                    info!("  +{:#04x}: {:#018x}", offset, val);
                }
            }
            Err(e) => info!("  +{:#04x}: read failed: {:#}", offset, e),
        }
    }

    // Step 8: If +0x08 is null, try alternative list heads in SpawnManager
    if let Ok(next_at_08) = proc.read_ptr(first_node + 0x08)
        && next_at_08 == 0 {
            info!("--- NEXT at +0x08 is NULL. Checking alternative SpawnManager members ---");

            // Try SpawnManager+0x00 (might be a different list or vtable)
            match proc.read_ptr(mgr_addr) {
                Ok(alt) if alt != 0 && alt != first_node => {
                    info!("SpawnManager+0x00 -> {:#x} (DIFFERENT from PLAYER_LIST head!)", alt);
                    match proc.read_bytes(alt, 32) {
                        Ok(bytes) => info!("\n{}", format_hex_dump(alt, &bytes)),
                        Err(e) => error!("  Failed to read: {:#}", e),
                    }
                    match proc.read_string(
                        alt + dmft_common::offsets::player_base::NAME, 64,
                    ) {
                        Ok(name) => info!("  Name: \"{}\"", name),
                        Err(_) => info!("  (name unreadable)"),
                    }
                }
                Ok(alt) if alt == first_node => {
                    info!("SpawnManager+0x00 -> same node as PLAYER_LIST ({:#x})", alt);
                }
                Ok(_) => info!("SpawnManager+0x00 -> NULL"),
                Err(e) => error!("Failed to read SpawnManager+0x00: {:#}", e),
            }

            // Try SpawnManager+0x08
            match proc.read_ptr(mgr_addr + 0x08) {
                Ok(alt) if alt != 0 => {
                    info!("SpawnManager+0x08 -> {:#x}", alt);
                    match proc.read_string(
                        alt + dmft_common::offsets::player_base::NAME, 64,
                    ) {
                        Ok(name) => info!("  Name: \"{}\"", name),
                        Err(_) => info!("  (name unreadable)"),
                    }
                }
                _ => info!("SpawnManager+0x08 -> NULL or unreadable"),
            }
        }

    info!("===================================================");
}

/// Helper: read and log a hex dump of `count` bytes starting at `base_addr + start_offset`.
#[allow(dead_code)]
fn dump_hex_region(
    proc: &process::memory::ProcessHandle,
    base_addr: usize,
    start_offset: usize,
    count: usize,
    label: &str,
) {
    info!("--- {} ---", label);
    match proc.read_bytes(base_addr + start_offset, count) {
        Ok(bytes) => {
            for chunk_start in (0..bytes.len()).step_by(16) {
                let chunk_end = (chunk_start + 16).min(bytes.len());
                let chunk = &bytes[chunk_start..chunk_end];
                let offset = start_offset + chunk_start;

                let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
                let ascii: String = chunk
                    .iter()
                    .map(|&b| if (0x20..=0x7e).contains(&b) { b as char } else { '.' })
                    .collect();

                let hex_str = if hex.len() < 16 {
                    let mut s = hex.join(" ");
                    for _ in hex.len()..16 {
                        s.push_str("   ");
                    }
                    s
                } else {
                    hex.join(" ")
                };

                info!("  {:#06x}: {}  |{}|", offset, hex_str, ascii);
            }
        }
        Err(e) => error!("  Failed to read {} bytes at base+{:#x}: {:#}", count, start_offset, e),
    }
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
pub fn get_module_base(proc: &process::memory::ProcessHandle) -> Result<u64> {
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
pub fn get_module_base(_proc: &process::memory::ProcessHandle) -> Result<u64> {
    warn!("Using preferred base address (non-Windows stub)");
    Ok(dmft_common::offsets::EQ_PREFERRED_BASE)
}
