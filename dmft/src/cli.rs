use anyhow::{Context, Result};
use std::path::Path;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

use crate::config;
use crate::eq;
use crate::inject;
use crate::ipc;
use crate::nav;
use crate::orchestrator;
use crate::process;
use crate::soul;
use crate::tui;

use crate::{SOUL_DB_PATH, get_module_base};

fn read_shared_state_with_retry(
    reader: &mut ipc::shared::SharedStateReader,
    timeout: Duration,
) -> Option<dmft_common::types::GameState> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(state) = reader.read() {
            return Some(state);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn load_pid_session(pid: u32) -> Result<(dmft_common::ipc::SessionToken, u64)> {
    let token = ipc::load_session_token(pid).ok_or_else(|| {
        anyhow::anyhow!(
            "No session token for PID {pid}. Inject the DLL first to create authenticated IPC state."
        )
    })?;
    let session_id = dmft_common::ipc::session_id_from_token(&token);
    Ok((token, session_id))
}

fn connect_authenticated_pipe(pid: u32) -> Result<ipc::pipe::CommandPipe> {
    let (token, session_id) = load_pid_session(pid)?;
    let pipe = ipc::pipe::CommandPipe::connect(pid, session_id)
        .with_context(|| format!("Failed to connect to PID {pid}. Is the DLL injected?"))?;
    pipe.send_raw_token(&token)
        .with_context(|| format!("Failed to authenticate with PID {pid}"))?;
    Ok(pipe)
}

fn shared_state_reader_for_pid(pid: u32) -> Result<ipc::shared::SharedStateReader> {
    let (_, session_id) = load_pid_session(pid)?;
    ipc::shared::SharedStateReader::new(pid, session_id)
        .with_context(|| format!("Cannot open shared memory for PID {pid} — is the DLL injected?"))
}

/// TUI mode — the default. Shows ShowEQ-inspired live dashboard.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_tui_mode() -> Result<()> {
    let mut app = tui::app::App::new();
    let config = load_config()?;

    // Set server name and launch path from config
    app.server_name = config.server.name.clone();
    if !config.launch.eq_path.is_empty() {
        app.launch_eq_path = config.launch.eq_path.clone();
    }

    // Try to attach to ALL EQ processes before launching TUI
    #[cfg(windows)]
    {
        if let Ok(pids) = process::memory::find_processes_by_name(&config.process_name) {
            for &pid in &pids {
                if let Ok(proc) = process::memory::ProcessHandle::open(pid)
                    && let Ok(base) = get_module_base(&proc)
                {
                    let client = tui::app::ClientState::new(pid, base);
                    info!(pid, base = format!("{:#x}", base), "Attached to EQ client");
                    app.clients.push(client);
                }
            }
        }
        let count = app.clients.len();
        if count > 0 {
            app.status_message = format!(
                "{} EQ client{} attached",
                count,
                if count == 1 { "" } else { "s" }
            );
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

    // Initialize Discord integration if webhook URL is configured
    app.init_discord(&config.discord);

    let orchestrator = orchestrator::Orchestrator::new();
    tui::run::run_tui(app, orchestrator)
}

/// Inject mode (--inject) — find eqgame.exe processes and inject `dmft_dll.dll` into each.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_inject_mode() -> Result<()> {
    info!("DMFT inject mode — finding EQ processes...");

    let config = load_config()?;
    let pids = process::memory::find_processes_by_name(&config.process_name)?;

    if pids.is_empty() {
        println!(
            "No {} processes found. Launch EQ first.",
            config.process_name
        );
        return Ok(());
    }

    println!("Found {} EQ process(es): {:?}", pids.len(), pids);

    // Locate the DLL — check release first, then debug
    let project_dir = std::env::current_dir().unwrap_or_default();
    let dll_candidates = [
        project_dir.join("target/release/dmft_dll.dll"),
        project_dir.join("target/debug/dmft_dll.dll"),
    ];

    let source_dll = dll_candidates.iter().find(|p| p.exists()).ok_or_else(|| {
        anyhow::anyhow!("Cannot find dmft_dll.dll. Run `cargo build --release` first.")
    })?;

    println!("Using DLL: {}", source_dll.display());

    // Stage the DLL (copies with randomized name)
    let staged_dll = inject::dll_prep::prepare_dll(source_dll)?;
    println!("Staged DLL: {}", staged_dll.display());

    let mut success = 0u32;
    let mut failed = 0u32;

    for &pid in &pids {
        print!("Injecting into PID {pid}... ");
        // Write session token before injection so DLL can read it during init.
        if let Err(e) = ipc::write_session_token_file(pid) {
            println!("FAILED: token write failed: {e:#}");
            error!(pid, error = %e, "Session token staging failed");
            failed += 1;
            continue;
        }
        match inject::loader::inject_dll(pid, &staged_dll) {
            Ok(()) => {
                println!("OK");
                info!(pid, "Injection succeeded");
                success += 1;
            }
            Err(e) => {
                println!("FAILED: {e:#}");
                error!(pid, error = %e, "Injection failed");
                failed += 1;
            }
        }
    }

    println!();
    println!("Results: {success} succeeded, {failed} failed");
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

/// Zones mode (`--zones PID`) — query the zone adjacency graph from an injected client.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_zones_mode(pid: u32) -> Result<()> {
    use dmft_common::ipc::{Command, Response};
    use dmft_common::nav::ZoneGraph;

    println!("Querying zone graph from PID {pid}...");

    let pipe = connect_authenticated_pipe(pid)?;
    let response = pipe
        .send(&Command::QueryZoneGraph)
        .context("Failed to query zone graph")?;

    match response {
        Response::ZoneGraph { zones } => {
            // Convert wire format back to ZoneGraph for display
            let mut graph = ZoneGraph::default();
            for (zone_id, name, min_level, max_level, conns) in &zones {
                let connections = conns
                    .iter()
                    .map(|(dest, tt, disabled)| dmft_common::nav::ZoneConnection {
                        dest_zone_id: *dest,
                        transfer_type: *tt,
                        disabled: *disabled,
                    })
                    .collect();
                graph.zones.insert(
                    *zone_id,
                    dmft_common::nav::ZoneNode {
                        zone_id: *zone_id,
                        name: name.clone(),
                        min_level: *min_level,
                        max_level: *max_level,
                        connections,
                    },
                );
            }

            println!("Zone graph: {} zones", graph.zones.len());
            println!();

            // Print zones sorted by ID
            let transfer_names = ["Zone Line", "Door", "Book", "Translocator", "Spell"];
            for (zone_id, name, min_level, max_level, conns) in &zones {
                let level_str = if *max_level > 0 {
                    format!(" (lv {min_level}-{max_level})")
                } else {
                    String::new()
                };
                println!("[{zone_id:>3}] {name}{level_str}");
                for (dest_id, tt, disabled) in conns {
                    let tt_name = transfer_names.get(*tt as usize).unwrap_or(&"Unknown");
                    let dest_name = zones
                        .iter()
                        .find(|(id, _, _, _, _)| *id == *dest_id)
                        .map_or("???", |(_, n, _, _, _)| n.as_str());
                    let disabled_str = if *disabled { " [DISABLED]" } else { "" };
                    println!("      -> [{dest_id:>3}] {dest_name} via {tt_name}{disabled_str}");
                }
            }
        }
        Response::Error { message } => {
            anyhow::bail!("DLL returned error: {message}");
        }
        other => {
            anyhow::bail!("Unexpected response: {other:?}");
        }
    }

    Ok(())
}

/// Status mode (`--status PID`) — read shared memory and print player state.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_status_mode(pid: u32) -> Result<()> {
    let mut reader = shared_state_reader_for_pid(pid)?;

    match read_shared_state_with_retry(&mut reader, Duration::from_millis(1200)) {
        Some(state) => {
            if !state.zone_short_name.is_empty() {
                println!("Zone: {} ({})", state.zone_long_name, state.zone_short_name);
            }
            if let Some(ref player) = state.local_player {
                println!("Player: {} (ID: {})", player.name, player.spawn_id);
                println!(
                    "Position: x={:.1}, y={:.1}, z={:.1} heading={:.1}",
                    player.x, player.y, player.z, player.heading
                );
                println!(
                    "HP: {}/{} ({:.0}%)",
                    player.hp_current,
                    player.hp_max,
                    player.hp_pct()
                );
                println!("Mana: {}/{}", player.mana_current, player.mana_max);
                println!("Level: {} Class: {}", player.level, player.class_id);
                println!("Nav: {:?}", state.nav_status);
            } else {
                println!("No player data (not in world?)");
            }
            if let Some(ref target) = state.target {
                println!(
                    "Target: {} (ID: {}) HP: {:.0}%",
                    target.name,
                    target.spawn_id,
                    target.hp_pct()
                );
            }
            println!("Nearby spawns: {}", state.nearby_spawns.len());
        }
        None => {
            println!("No data from shared memory (DLL hasn't written yet or write in progress)");
        }
    }
    Ok(())
}

/// Status-all mode (--statusall) — read shared memory for all EQ clients and print a summary table.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_statusall_mode() -> Result<()> {
    let pids = process::memory::find_processes_by_name("eqgame.exe")?;

    if pids.is_empty() {
        println!("No eqgame.exe processes found.");
        return Ok(());
    }

    // Header
    println!(
        "{:<7}{:<14}{:<18}{:<24}{:<6}{:<4}{:<7}{:<7}",
        "PID", "Character", "Zone", "Position", "HP%", "Lv", "Nav", "Spawns"
    );

    for &pid in &pids {
        match shared_state_reader_for_pid(pid) {
            Ok(mut reader) => {
                match read_shared_state_with_retry(&mut reader, Duration::from_millis(1200)) {
                    Some(state) => {
                        if let Some(ref player) = state.local_player {
                            let pos =
                                format!("({:.0}, {:.0}, {:.0})", player.x, player.y, player.z);
                            let hp = format!("{:.0}%", player.hp_pct());
                            let nav = match &state.nav_status {
                                dmft_common::nav::NavStatus::Idle => "Idle".to_string(),
                                dmft_common::nav::NavStatus::Moving {
                                    waypoint_index,
                                    waypoint_count,
                                    ..
                                } => {
                                    format!("{waypoint_index}/{waypoint_count}")
                                }
                                dmft_common::nav::NavStatus::Stuck { .. } => "Stuck".to_string(),
                                dmft_common::nav::NavStatus::Arrived => "Done".to_string(),
                            };
                            let zone = if state.zone_short_name.is_empty() {
                                "(unknown)".to_string()
                            } else {
                                state.zone_short_name.clone()
                            };
                            println!(
                                "{:<7}{:<14}{:<18}{:<24}{:<6}{:<4}{:<7}{:<7}",
                                pid,
                                player.name,
                                zone,
                                pos,
                                hp,
                                player.level,
                                nav,
                                state.nearby_spawns.len()
                            );
                        } else {
                            println!(
                                "{:<7}{:<14}{:<18}{:<24}{:<6}{:<4}{:<7}{:<7}",
                                pid, "(no player)", "(not in world)", "-", "-", "-", "-", "-"
                            );
                        }
                    }
                    None => {
                        println!(
                            "{:<7}{:<14}{:<18}{:<24}{:<6}{:<4}{:<7}{:<7}",
                            pid, "(no data)", "-", "-", "-", "-", "-", "-"
                        );
                    }
                }
            }
            Err(_) => {
                println!(
                    "{:<7}{:<14}{:<18}{:<24}{:<6}{:<4}{:<7}{:<7}",
                    pid, "(no shm)", "-", "-", "-", "-", "-", "-"
                );
            }
        }
    }

    Ok(())
}

/// Navigate mode (`--nav PID x y z`) — send `NavigateTo` to a specific client.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_nav_mode(pid: u32, x: f32, y: f32, z: f32) -> Result<()> {
    use dmft_common::ipc::Command;
    use dmft_common::nav::Waypoint;

    println!("Navigating PID {pid} -> ({x}, {y}, {z})");

    // 1. Read shared memory to get current position and zone
    let waypoints = match shared_state_reader_for_pid(pid) {
        Ok(mut reader) => {
            match read_shared_state_with_retry(&mut reader, Duration::from_millis(1200)) {
                Some(state) if !state.zone_short_name.is_empty() => {
                    let player = state
                        .local_player
                        .as_ref()
                        .context("No player data in shared memory — character not in world?")?;
                    let from = (player.x, player.y, player.z);
                    let zone = &state.zone_short_name;
                    println!(
                        "Player at ({:.1}, {:.1}, {:.1}) in zone '{}'",
                        from.0, from.1, from.2, zone
                    );

                    // 2. Try navmesh pathfinding
                    match nav::mesh::load_zone(zone) {
                        Ok(loaded) => match nav::mesh::find_path(&loaded, from, (x, y, z)) {
                            Ok(path) => {
                                println!("Navmesh path found ({} waypoints):", path.len());
                                for (i, (wx, wy, wz)) in path.iter().enumerate() {
                                    println!("  [{i:>3}] ({wx:.2}, {wy:.2}, {wz:.2})");
                                }
                                path.iter()
                                    .map(|&(wx, wy, wz)| Waypoint::new(wx, wy, wz))
                                    .collect()
                            }
                            Err(e) => {
                                warn!(
                                    "Navmesh path query failed: {:#} — falling back to straight line",
                                    e
                                );
                                println!("Navmesh path failed: {e} — using straight line");
                                vec![Waypoint::new(x, y, z)]
                            }
                        },
                        Err(e) => {
                            warn!(
                                "Cannot load navmesh for zone '{}': {:#} — falling back to straight line",
                                zone, e
                            );
                            println!("No navmesh for '{zone}': {e} — using straight line");
                            vec![Waypoint::new(x, y, z)]
                        }
                    }
                }
                _ => {
                    println!("No shared memory data — using straight line (no navmesh)");
                    vec![Waypoint::new(x, y, z)]
                }
            }
        }
        Err(e) => {
            println!("Cannot read shared memory for PID {pid}: {e} — using straight line");
            vec![Waypoint::new(x, y, z)]
        }
    };

    // 3. Send waypoints via IPC pipe
    let pipe = connect_authenticated_pipe(pid)?;
    let cmd = Command::NavigateTo { waypoints };
    pipe.send_async(&cmd)
        .context(format!("Failed to send NavigateTo to PID {pid}"))?;

    println!("NavigateTo sent — character should start moving.");
    Ok(())
}

/// Navigate ALL EQ clients to a destination using navmesh pathfinding.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_navall_mode(x: f32, y: f32, z: f32) -> Result<()> {
    use dmft_common::ipc::Command;
    use dmft_common::nav::Waypoint;

    let config = load_config()?;
    let pids = process::memory::find_processes_by_name(&config.process_name)?;

    if pids.is_empty() {
        println!("No {} processes found.", config.process_name);
        return Ok(());
    }

    println!(
        "Found {} EQ client(s). Navigating all to ({}, {}, {})...",
        pids.len(),
        x,
        y,
        z
    );

    let mut success_count = 0u32;
    let mut fail_count = 0u32;

    for &pid in &pids {
        // Read shared memory for position + zone
        let waypoints = if let Ok(mut reader) = shared_state_reader_for_pid(pid) {
            match read_shared_state_with_retry(&mut reader, Duration::from_millis(1200)) {
                Some(state) if !state.zone_short_name.is_empty() => {
                    let player = if let Some(p) = state.local_player.as_ref() {
                        p
                    } else {
                        println!("  PID {pid}: no player data — skipping");
                        fail_count += 1;
                        continue;
                    };
                    let from = (player.x, player.y, player.z);
                    let zone = &state.zone_short_name;

                    match nav::mesh::load_zone(zone) {
                        Ok(loaded) => match nav::mesh::find_path(&loaded, from, (x, y, z)) {
                            Ok(path) => {
                                println!(
                                    "  PID {} ({}): navmesh path, {} waypoints",
                                    pid,
                                    player.name,
                                    path.len()
                                );
                                path.iter()
                                    .map(|&(wx, wy, wz)| Waypoint::new(wx, wy, wz))
                                    .collect()
                            }
                            Err(e) => {
                                println!(
                                    "  PID {} ({}): navmesh failed ({}), straight line",
                                    pid, player.name, e
                                );
                                vec![Waypoint::new(x, y, z)]
                            }
                        },
                        Err(e) => {
                            println!(
                                "  PID {} ({}): no mesh for '{}' ({}), straight line",
                                pid, player.name, zone, e
                            );
                            vec![Waypoint::new(x, y, z)]
                        }
                    }
                }
                _ => {
                    println!("  PID {pid}: no shared memory data — straight line");
                    vec![Waypoint::new(x, y, z)]
                }
            }
        } else {
            println!("  PID {pid}: cannot read shared memory — skipping");
            fail_count += 1;
            continue;
        };

        // Send via IPC
        match connect_authenticated_pipe(pid) {
            Ok(pipe) => {
                let cmd = Command::NavigateTo { waypoints };
                if let Err(e) = pipe.send_async(&cmd) {
                    println!("  PID {pid}: send failed: {e} — skipping");
                    fail_count += 1;
                } else {
                    success_count += 1;
                }
            }
            Err(e) => {
                println!("  PID {pid}: cannot connect ({e})");
                fail_count += 1;
            }
        }
    }

    println!("Done: {success_count} navigating, {fail_count} failed");
    Ok(())
}

/// Inject mode targeting a specific PID (`--inject-pid PID`).
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_inject_pid_mode(pid: u32) -> Result<()> {
    info!(pid, "DMFT inject-pid mode — targeting single process");

    let project_dir = std::env::current_dir().unwrap_or_default();
    let dll_candidates = [
        project_dir.join("target/release/dmft_dll.dll"),
        project_dir.join("target/debug/dmft_dll.dll"),
    ];

    let source_dll = dll_candidates.iter().find(|p| p.exists()).ok_or_else(|| {
        anyhow::anyhow!("Cannot find dmft_dll.dll. Run `cargo build --release` first.")
    })?;

    // Write session token file BEFORE injection so DLL can read it during init.
    ipc::write_session_token_file(pid)?;

    let staged_dll = inject::dll_prep::prepare_dll(source_dll)?;
    println!("Injecting into PID {pid}...");

    inject::loader::inject_dll(pid, &staged_dll)?;
    println!("OK — DLL injected into PID {pid}");
    Ok(())
}

/// Login mode targeting a specific PID (`--login-pid PID account password [server] [character]`).
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_login_pid_mode(
    pid: u32,
    account: &str,
    password: &str,
    server: &str,
    character: &str,
) -> Result<()> {
    use dmft_common::ipc::Command;

    println!("Sending StartLogin to PID {pid} (account: {account}, server: {server})...");

    let pipe = connect_authenticated_pipe(pid)?;
    let cmd = Command::StartLogin {
        account_name: account.to_string(),
        password: password.to_string(),
        server_name: server.to_string(),
        character_name: character.to_string(),
    };
    pipe.send_async(&cmd)
        .context(format!("Failed to send StartLogin to PID {pid}"))?;

    println!("StartLogin sent to PID {pid}");
    Ok(())
}

/// Login mode (`--login account password [server] [character]`) — send `StartLogin` to all injected EQ clients.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_login_mode(account: &str, password: &str, server: &str, character: &str) -> Result<()> {
    use dmft_common::ipc::Command;

    let config = load_config()?;
    let pids = process::memory::find_processes_by_name(&config.process_name)?;

    if pids.is_empty() {
        println!("No EQ processes found. Launch EQ first, then inject, then login.");
        return Ok(());
    }

    for &pid in &pids {
        println!("Sending StartLogin to PID {pid} (account: {account}, server: {server})...");

        match connect_authenticated_pipe(pid) {
            Ok(pipe) => {
                let cmd = Command::StartLogin {
                    account_name: account.to_string(),
                    password: password.to_string(),
                    server_name: server.to_string(),
                    character_name: character.to_string(),
                };
                if pipe.send_async(&cmd).is_ok() {
                    println!("  StartLogin sent to PID {pid}");
                } else {
                    println!("  Failed to send StartLogin to PID {pid}");
                }
            }
            Err(_) => {
                println!("  Cannot connect to PID {pid} — is the DLL injected?");
            }
        }
    }

    println!("\nLogin commands sent. Check DLL log for progress.");
    Ok(())
}

/// Calibrate mode (--calibrate) — find all EQ processes and send `calibrate_login` to each.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_calibrate_mode() -> Result<()> {
    use dmft_common::ipc::Command;

    let config = load_config()?;
    let pids = process::memory::find_processes_by_name(&config.process_name)?;

    if pids.is_empty() {
        println!("No EQ processes found. Launch EQ first, then inject, then calibrate.");
        return Ok(());
    }

    for &pid in &pids {
        println!("Sending calibrate_login to PID {pid}...");

        match connect_authenticated_pipe(pid) {
            Ok(pipe) => {
                let cmd = Command::CalibrateLogin;
                if pipe.send_async(&cmd).is_ok() {
                    println!("  Calibration sent to PID {pid}");
                } else {
                    println!("  Failed to send calibration to PID {pid}");
                }
            }
            Err(_) => {
                println!("  Cannot connect to PID {pid} — is the DLL injected?");
            }
        }
    }

    println!("\nCalibration complete. Check DLL log at %TEMP%\\dmft\\dmft-dll.log");
    Ok(())
}

/// Command mode (`--cmd pid command`) — send a slash command to an injected client.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_cmd_mode(pid: u32, command: &str) -> Result<()> {
    use dmft_common::ipc::Command;

    println!("Sending command to PID {pid}: {command}");

    let pipe = connect_authenticated_pipe(pid)?;
    // Send the slash command (fire-and-forget — DLL disconnects pipe after read).
    let cmd = Command::SlashCommand {
        command: command.to_string(),
    };

    pipe.send_async(&cmd).context("Failed to send command")?;

    println!("Command sent successfully.");

    Ok(())
}

/// Navpath mode (--navpath) — download zone navmesh and query a path between two points.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_navpath_mode(zone: &str, from: (f32, f32, f32), to: (f32, f32, f32)) -> Result<()> {
    info!("Navpath mode: zone={zone} from={from:?} to={to:?}");
    println!("Loading navmesh for zone '{zone}'...");

    // Parse first to show mesh params for debugging
    let data = nav::mesh::download_zone_mesh(zone)?;
    let proto = nav::mesh::parse_navmesh(&data)?;
    if let Some(ts) = &proto.tile_set {
        if let Some(p) = &ts.mesh_params {
            let o = p
                .origin
                .as_ref()
                .map(|v| (v.x, v.y, v.z))
                .unwrap_or_default();
            println!(
                "  Mesh params: origin=({:.1}, {:.1}, {:.1}) tile={}x{} tiles={} polys={}",
                o.0, o.1, o.2, p.tile_width, p.tile_height, p.max_tiles, p.max_polys
            );
        }
        println!("  Tiles: {}", ts.tiles.len());
    }
    let loaded = nav::mesh::load_navmesh(&proto)?;
    println!("Navmesh loaded. Finding path...");

    let waypoints = nav::mesh::find_path(&loaded, from, to)?;
    println!("Path found ({} waypoints):", waypoints.len());
    for (i, (x, y, z)) in waypoints.iter().enumerate() {
        println!("  [{i:>3}] ({x:.2}, {y:.2}, {z:.2})");
    }
    Ok(())
}

/// Dump mode (--dump) — one-shot CLI output, the original M1 behavior.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_dump_mode() -> Result<()> {
    info!(
        "DMFT v{} — EQ Memory Reader (dump mode)",
        env!("CARGO_PKG_VERSION")
    );

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
    let proc = process::memory::ProcessHandle::open(pid).context("Failed to open EQ process")?;

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

// ─── Utility functions ──────────────────────────────────────────────────────

/// Format a byte buffer as a hex dump with offset labels.
fn format_hex_dump(base_addr: usize, bytes: &[u8]) -> String {
    let mut lines = Vec::new();
    for (i, chunk) in bytes.chunks(16).enumerate() {
        let offset = i * 16;
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
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
        lines.push(format!(
            "  {:#010x} (+{:#04x}): {}  |{}|",
            base_addr + offset,
            offset,
            hex_str,
            ascii
        ));
    }
    lines.join("\n")
}

/// Diagnostic hex dump of `SpawnManager`, the `TList`, and the first spawn node.
/// Helps debug why the NEXT pointer reads as 0x0 after the first spawn.
#[allow(unused_variables)]
fn dump_spawn_list_diagnostic(proc: &process::memory::ProcessHandle, eq_base: u64) {
    use dmft_common::offsets::{self, spawn_manager};

    info!("===================================================");
    info!("SPAWN LIST DIAGNOSTIC HEX DUMP");
    info!("===================================================");

    // Step 1: Read SpawnManager pointer
    let mgr_ptr_addr = if let Some(addr) = offsets::rebase(offsets::PINST_SPAWN_MANAGER, eq_base) {
        addr
    } else {
        error!("Failed to rebase pinstSpawnManager");
        return;
    };
    let mgr_addr = match proc.read_ptr(mgr_ptr_addr) {
        Ok(addr) => addr,
        Err(e) => {
            error!("Failed to read pinstSpawnManager: {:#}", e);
            return;
        }
    };
    info!(
        "pinstSpawnManager ptr at {:#x} -> SpawnManager at {:#x}",
        mgr_ptr_addr, mgr_addr
    );

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
                if let Some(slice) = bytes.get(off..off + 8)
                    && let Ok(arr) = <[u8; 8]>::try_from(slice)
                {
                    let val = u64::from_le_bytes(arr);
                    let looks_like_ptr = val > 0x10000 && val < 0x7FFF_FFFF_FFFF;
                    info!(
                        "  SpawnManager+{:#04x}: {:#018x} {}",
                        off,
                        val,
                        if looks_like_ptr {
                            "<-- looks like a pointer"
                        } else {
                            ""
                        }
                    );
                }
            }
        }
        Err(e) => error!("Failed to read SpawnManager bytes: {:#}", e),
    }

    // Step 3: Read the TList at SpawnManager+PLAYER_LIST (0x10)
    let list_addr = mgr_addr + spawn_manager::PLAYER_LIST;
    info!(
        "--- TList at SpawnManager+{:#x} = {:#x} ---",
        spawn_manager::PLAYER_LIST,
        list_addr
    );
    match proc.read_bytes(list_addr, 16) {
        Ok(bytes) => {
            info!("\n{}", format_hex_dump(list_addr, &bytes));
            if let (Some(first_slice), Some(last_slice)) = (bytes.get(0..8), bytes.get(8..16)) {
                let first_node =
                    u64::from_le_bytes(<[u8; 8]>::try_from(first_slice).unwrap_or_default());
                let last_node =
                    u64::from_le_bytes(<[u8; 8]>::try_from(last_slice).unwrap_or_default());
                info!("  TList.m_pFirstNode: {:#x}", first_node);
                info!("  TList.m_pLastNode:  {:#x}", last_node);
            }
        }
        Err(e) => error!("Failed to read TList bytes: {:#}", e),
    }

    // Step 4: Read the first node pointer from TList
    let first_node = match proc.read_ptr(list_addr) {
        Ok(addr) => addr,
        Err(e) => {
            error!("Failed to read first node: {:#}", e);
            return;
        }
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
                if let Some(slice) = bytes.get(off..off + 8) {
                    let arr = <[u8; 8]>::try_from(slice).unwrap_or_default();
                    let val = u64::from_le_bytes(arr);
                    let looks_like_ptr = val > 0x10000 && val < 0x7FFF_FFFF_FFFF;
                    info!(
                        "  +{:#04x} ({}): {:#018x} {}",
                        off,
                        label,
                        val,
                        if looks_like_ptr {
                            "<-- valid pointer"
                        } else if val == 0 {
                            "<-- NULL"
                        } else {
                            ""
                        }
                    );
                }
            }
        }
        Err(e) => error!("Failed to read first spawn bytes: {:#}", e),
    }

    // Step 6: Verify this IS a PlayerClient by reading the name at known offset
    match proc.read_string(first_node + dmft_common::offsets::player_base::NAME, 64) {
        Ok(name) => info!(
            "  Name at +{:#x}: \"{}\"",
            dmft_common::offsets::player_base::NAME,
            name
        ),
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
                            !n.is_empty() && n.chars().all(|c| c.is_ascii_graphic() || c == ' ')
                        })
                        .map(|n| format!(" -> name=\"{n}\""))
                        .unwrap_or_default();
                    info!(
                        "  +{:#04x}: {:#018x} <-- VALID PTR{}",
                        offset, val, name_check
                    );
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
        && next_at_08 == 0
    {
        info!("--- NEXT at +0x08 is NULL. Checking alternative SpawnManager members ---");

        // Try SpawnManager+0x00 (might be a different list or vtable)
        match proc.read_ptr(mgr_addr) {
            Ok(alt) if alt != 0 && alt != first_node => {
                info!(
                    "SpawnManager+0x00 -> {:#x} (DIFFERENT from PLAYER_LIST head!)",
                    alt
                );
                match proc.read_bytes(alt, 32) {
                    Ok(bytes) => info!("\n{}", format_hex_dump(alt, &bytes)),
                    Err(e) => error!("  Failed to read: {:#}", e),
                }
                if let Ok(name) =
                    proc.read_string(alt + dmft_common::offsets::player_base::NAME, 64)
                {
                    info!("  Name: \"{}\"", name)
                } else {
                    info!("  (name unreadable)")
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
                if let Ok(name) =
                    proc.read_string(alt + dmft_common::offsets::player_base::NAME, 64)
                {
                    info!("  Name: \"{}\"", name)
                } else {
                    info!("  (name unreadable)")
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

                let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
                let ascii: String = chunk
                    .iter()
                    .map(|&b| {
                        if (0x20..=0x7e).contains(&b) {
                            b as char
                        } else {
                            '.'
                        }
                    })
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
        Err(e) => error!(
            "  Failed to read {} bytes at base+{:#x}: {:#}",
            count, start_offset, e
        ),
    }
}
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn load_config() -> Result<config::AppConfig> {
    let config_path = Path::new("config/dmft.toml");
    if config_path.exists() {
        config::AppConfig::load(config_path).context("Failed to load configuration")
    } else {
        Ok(config::AppConfig::default_config())
    }
}

#[cfg(test)]
mod tests {
    use super::load_pid_session;

    #[test]
    fn load_pid_session_errors_without_token_file() {
        let pid = 4_242_421;
        let token_dir = std::env::temp_dir().join("dmft");
        let _ = std::fs::remove_file(token_dir.join(format!("token_{}.bin", pid)));
        let _ = std::fs::remove_file(token_dir.join(format!("login_token_{}.bin", pid)));

        let err = load_pid_session(pid).expect_err("missing token should error");
        assert!(err.to_string().contains("No session token"));
    }

    #[test]
    fn load_pid_session_reads_written_token() {
        let pid = 4_242_422;
        let token_dir = std::env::temp_dir().join("dmft");
        let token_path = token_dir.join(format!("token_{}.bin", pid));
        let login_token_path = token_dir.join(format!("login_token_{}.bin", pid));

        dmft_common::ipc::write_session_token_file(pid).expect("token file should be created");

        let (token, session_id) = load_pid_session(pid).expect("token should be readable");
        assert_ne!(token, [0u8; 32]);
        assert_eq!(session_id, dmft_common::ipc::session_id_from_token(&token));

        let _ = std::fs::remove_file(token_path);
        let _ = std::fs::remove_file(login_token_path);
    }
}
