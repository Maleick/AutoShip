use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use textquest_common::ghidra_db::GhidraDatabase;
use tracing::{error, info, warn};
use zeroize::Zeroizing;

use crate::config;
use crate::eq;
use crate::inject;
use crate::ipc;
use crate::nav;
use crate::orchestrator;
use crate::paths;
use crate::process;
use crate::soul;
use crate::tui;

use crate::{GHIDRA_DB_PATH, OPCODES_CONFIG_PATH, SOUL_DB_PATH, get_module_base};

fn read_shared_state_with_retry(
    reader: &mut ipc::shared::SharedStateReader,
    timeout: Duration,
) -> Option<textquest_common::types::GameState> {
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

fn load_pid_session(pid: u32) -> Result<(textquest_common::ipc::SessionToken, u64)> {
    let token = ipc::load_session_token(pid).ok_or_else(|| {
        anyhow::anyhow!(
            "No session token for PID {pid}. Inject the DLL first to create authenticated IPC state."
        )
    })?;
    let session_id = textquest_common::ipc::session_id_from_token(&token);
    Ok((token, session_id))
}

fn send_timing_correction_command(pid: u32, enabled: bool) -> Result<()> {
    use textquest_common::ipc::Command;

    let deadline = Instant::now() + Duration::from_secs(8);
    let mut last_error = None;

    while Instant::now() < deadline {
        match connect_authenticated_pipe(pid) {
            Ok(pipe) => {
                if let Err(e) = pipe.send(&Command::SetTimingCorrection { enabled }) {
                    last_error = Some(e);
                } else {
                    return Ok(());
                }
            }
            Err(e) => {
                last_error = Some(e);
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    if let Some(err) = last_error {
        Err(err).with_context(|| {
            format!("Failed to send timing correction command to PID {pid} after 8 seconds")
        })
    } else {
        Err(anyhow::anyhow!(
            "Failed to send timing correction command to PID {pid}: unknown error"
        ))
    }
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

fn resolve_navmesh_zone(zone: Option<&str>, pid: Option<u32>) -> Result<String> {
    if let Some(zone) = zone {
        return Ok(zone.to_string());
    }

    let pid = pid.ok_or_else(|| anyhow::anyhow!("Provide a zone name or --pid to resolve it"))?;
    let mut reader = shared_state_reader_for_pid(pid)?;
    let state = read_shared_state_with_retry(&mut reader, Duration::from_millis(1200))
        .ok_or_else(|| anyhow::anyhow!("No shared memory data available for PID {pid}"))?;
    if state.zone_short_name.is_empty() {
        anyhow::bail!("PID {pid} is not in a zone yet; no zone short name is available");
    }
    Ok(state.zone_short_name)
}

fn query_nav_signals(pid: u32) -> Result<textquest_common::nav::NavStateSignals> {
    use textquest_common::ipc::{Command, Response};

    let pipe = connect_authenticated_pipe(pid)?;
    match pipe.send(&Command::NavSignalsQuery)? {
        Response::NavSignals { signals } => Ok(signals),
        Response::Error { message } => anyhow::bail!("DLL returned error: {message}"),
        other => anyhow::bail!("Unexpected nav signals response: {other:?}"),
    }
}

fn query_nav_diagnostics(pid: u32) -> Result<textquest_common::nav::NavDiagnostics> {
    use textquest_common::ipc::{Command, Response};

    let pipe = connect_authenticated_pipe(pid)?;
    match pipe.send(&Command::NavDiagnosticsQuery)? {
        Response::NavDiagnosticsResult { diagnostics } => Ok(diagnostics),
        Response::Error { message } => anyhow::bail!("DLL returned error: {message}"),
        other => anyhow::bail!("Unexpected nav diagnostics response: {other:?}"),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct NavSignalDisplay {
    active: &'static str,
    mesh_loaded: &'static str,
    path_exists: &'static str,
    path_length: String,
    velocity: String,
}

fn format_nav_signal_flag(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "yes",
        Some(false) => "no",
        None => "n/a",
    }
}

fn format_nav_signal_metric(value: Option<f32>) -> String {
    value.map_or_else(|| String::from("n/a"), |metric| format!("{metric:.1}"))
}

fn nav_signal_display(
    signals: Option<&textquest_common::nav::NavStateSignals>,
) -> NavSignalDisplay {
    NavSignalDisplay {
        active: format_nav_signal_flag(signals.map(|signals| signals.active)),
        mesh_loaded: format_nav_signal_flag(signals.map(|signals| signals.mesh_loaded)),
        path_exists: format_nav_signal_flag(signals.map(|signals| signals.path_exists)),
        path_length: format_nav_signal_metric(signals.and_then(|signals| signals.path_length)),
        velocity: format_nav_signal_metric(signals.map(|signals| signals.velocity)),
    }
}

fn dump_guidance_message() -> String {
    format!(
        "Use `{}` for a one-shot snapshot in files matching {}",
        paths::dump_command_label(),
        paths::dump_log_path().display()
    )
}

fn calibration_dump_guidance_message() -> String {
    format!(
        "For a local process snapshot, run `{}` and check files matching {}",
        paths::dump_command_label(),
        paths::dump_log_path().display()
    )
}

fn resolve_built_dll_path() -> Result<PathBuf> {
    let exe_dir = std::env::current_exe()
        .context("Failed to resolve current executable path")?
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow::anyhow!("Current executable has no parent directory"))?;

    let dll_candidates = [
        exe_dir.join("textquest_dll.dll"),
        exe_dir.join("target/release/textquest_dll.dll"),
        exe_dir.join("target/debug/textquest_dll.dll"),
    ];

    dll_candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Cannot find textquest_dll.dll near the textquest executable. Run `cargo build --release` first."
            )
        })
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

    // Apply spawn watch config
    if config.spawn_watch.enabled {
        app.spawn_watch_named = config.spawn_watch.alert_named;
        app.spawn_alert_feed =
            crate::eq::spawn_alert::SpawnAlertFeed::new(config.spawn_watch.max_feed_entries);
        for pattern in &config.spawn_watch.watch_names {
            app.spawn_alert_feed.add_watch(pattern);
        }
    } else {
        app.spawn_watch_named = false;
    }

    // Initialize Ghidra DB and import opcodes from config/opcodes.json if present.
    {
        let db_path = Path::new(GHIDRA_DB_PATH);
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        match GhidraDatabase::open(db_path) {
            Ok(db) => {
                let opcodes_path = Path::new(OPCODES_CONFIG_PATH);
                match db.import_opcodes_from_file(opcodes_path) {
                    Ok(0) => {
                        info!(
                            "Ghidra DB opened (no opcodes config found at {})",
                            OPCODES_CONFIG_PATH
                        );
                    }
                    Ok(n) => {
                        info!(
                            count = n,
                            "Ghidra DB: imported {} opcodes from {}", n, OPCODES_CONFIG_PATH
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Ghidra DB: failed to import opcodes from {}: {}",
                            OPCODES_CONFIG_PATH, e
                        );
                    }
                }
                app.ghidra_db = Some(db);
            }
            Err(e) => {
                warn!("Ghidra DB: failed to open {}: {}", GHIDRA_DB_PATH, e);
            }
        }
    }

    let orchestrator = orchestrator::Orchestrator::new();
    tui::run::run_tui(app, orchestrator)
}

/// Inject mode (--inject) — find eqgame.exe processes and inject `textquest_dll.dll` into each.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_inject_mode() -> Result<()> {
    info!("TextQuest inject mode — finding EQ processes...");

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

    // Locate the DLL relative to the executable, not the current working directory.
    let source_dll = resolve_built_dll_path()?;

    println!("Using DLL: {}", source_dll.display());

    // Stage the DLL (copies with randomized name).
    // TODO: Switch to reflective loader once d3d11.dll cross-process import
    // resolution is fixed (addresses differ per-process due to ASLR).
    let staged_dll = inject::dll_prep::prepare_dll_locked(&source_dll)?;
    println!("Staged DLL: {}", staged_dll.path().display());

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
        match inject::loader::inject_dll(pid, staged_dll.path()) {
            Ok(()) => {
                println!("OK");
                info!(pid, "Injection succeeded");
                if config.timing_correction {
                    if let Err(e) = send_timing_correction_command(pid, true) {
                        println!("  FAILED to send timing correction setting: {e:#}");
                        error!(pid, error = %e, "Failed to send timing correction command");
                    }
                }
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
    println!(
        "  Orchestrator: files matching {}",
        paths::orchestrator_log_path().display()
    );
    println!(
        "  Dump mode:    files matching {}",
        paths::dump_log_path().display()
    );
    println!("  DLL (injected): {}", paths::dll_log_path().display());
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
    use textquest_common::ipc::{Command, Response};
    use textquest_common::nav::ZoneGraph;

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
                    .map(
                        |(dest, tt, disabled)| textquest_common::nav::ZoneConnection {
                            dest_zone_id: *dest,
                            transfer_type: *tt,
                            disabled: *disabled,
                        },
                    )
                    .collect();
                graph.zones.insert(
                    *zone_id,
                    textquest_common::nav::ZoneNode {
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

/// Navmesh reload mode — discard the cached zone mesh, redownload it, and verify it loads.
pub fn run_navmesh_reload_mode(zone: Option<&str>, pid: Option<u32>) -> Result<()> {
    let zone = resolve_navmesh_zone(zone, pid)?;
    println!("Reloading navmesh for zone '{zone}'...");

    let reload = nav::mesh::reload_zone_mesh(&zone)?;
    println!(
        "Reloaded navmesh for '{}': {}",
        reload.zone_short_name,
        if reload.replaced_cached_file {
            "replaced cached file"
        } else {
            "downloaded fresh cache"
        }
    );
    println!("Cache path: {}", reload.cache_path.display());
    println!("Bytes: {}", reload.cache_bytes);
    println!("Overlay segments: {}", reload.overlay_segment_count);

    Ok(())
}

/// Navmesh diagnostics mode — inspect the cached zone mesh and, optionally, the live DLL nav state.
pub fn run_navmesh_diagnostics_mode(zone: Option<&str>, pid: Option<u32>) -> Result<()> {
    let zone = resolve_navmesh_zone(zone, pid)?;
    let diagnostics = nav::mesh::cached_zone_mesh_diagnostics(&zone)?;

    println!(
        "Navmesh diagnostics for zone '{}':",
        diagnostics.zone_short_name
    );
    println!("  Cache path: {}", diagnostics.cache_path.display());
    println!(
        "  Cache file: {}",
        if diagnostics.cache_exists {
            "present"
        } else {
            "missing"
        }
    );
    println!(
        "  Cache bytes: {}",
        diagnostics
            .cache_bytes
            .map_or_else(|| String::from("n/a"), |bytes| bytes.to_string())
    );
    println!(
        "  Load status: {}",
        diagnostics
            .load_error
            .as_deref()
            .map_or("ok", |error| error)
    );
    println!(
        "  Overlay segments: {}",
        diagnostics
            .overlay_segment_count
            .map_or_else(|| String::from("n/a"), |count| count.to_string())
    );

    if let Some(pid) = pid {
        let signals = query_nav_signals(pid)?;
        let nav_diagnostics = query_nav_diagnostics(pid)?;
        println!();
        println!("Live navigator diagnostics for PID {pid}:");
        println!("  State: {}", nav_diagnostics.state);
        println!("  Active: {}", signals.active);
        println!("  Paused: {}", signals.paused);
        println!("  Mesh loaded: {}", signals.mesh_loaded);
        println!("  Path exists: {}", nav_diagnostics.path_exists);
        println!(
            "  Path length: {}",
            nav_diagnostics
                .path_length
                .map_or_else(|| String::from("n/a"), |length| format!("{length:.1}"))
        );
        println!("  Velocity: {:.1}", nav_diagnostics.velocity);
        println!(
            "  Waypoints: {}/{}",
            nav_diagnostics.waypoint_index, nav_diagnostics.waypoint_count
        );
        println!(
            "  Distance remaining: {:.1}",
            nav_diagnostics.distance_remaining
        );
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
    let nav_signals = query_nav_signals(pid).ok();
    let nav_display = nav_signal_display(nav_signals.as_ref());

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
                println!("Nav active: {}", nav_display.active);
                println!("Nav mesh loaded: {}", nav_display.mesh_loaded);
                println!("Nav path exists: {}", nav_display.path_exists);
                println!("Nav path length: {}", nav_display.path_length);
                println!("Nav velocity: {}", nav_display.velocity);
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
        "{:<7}{:<14}{:<18}{:<24}{:<6}{:<4}{:<16}{:<8}{:<12}{:<12}{:<12}{:<10}{:<7}",
        "PID",
        "Character",
        "Zone",
        "Position",
        "HP%",
        "Lv",
        "Nav",
        "Active",
        "MeshLoaded",
        "PathExists",
        "PathLength",
        "Velocity",
        "Spawns"
    );

    for &pid in &pids {
        let nav_signals = query_nav_signals(pid).ok();
        let nav_display = nav_signal_display(nav_signals.as_ref());
        match shared_state_reader_for_pid(pid) {
            Ok(mut reader) => {
                match read_shared_state_with_retry(&mut reader, Duration::from_millis(1200)) {
                    Some(state) => {
                        if let Some(ref player) = state.local_player {
                            let pos =
                                format!("({:.0}, {:.0}, {:.0})", player.x, player.y, player.z);
                            let hp = format!("{:.0}%", player.hp_pct());
                            let nav = match &state.nav_status {
                                textquest_common::nav::NavStatus::Idle => "Idle".to_string(),
                                textquest_common::nav::NavStatus::Moving {
                                    waypoint_index,
                                    waypoint_count,
                                    ..
                                } => {
                                    format!("{waypoint_index}/{waypoint_count}")
                                }
                                textquest_common::nav::NavStatus::Paused { reason, .. } => {
                                    format!("Paused({reason:?})")
                                }
                                textquest_common::nav::NavStatus::Stuck { .. } => {
                                    "Stuck".to_string()
                                }
                                textquest_common::nav::NavStatus::Arrived => "Done".to_string(),
                                textquest_common::nav::NavStatus::Following {
                                    leader_name, ..
                                } => {
                                    format!("Follow:{leader_name}")
                                }
                                textquest_common::nav::NavStatus::Sticking {
                                    target_id, ..
                                } => {
                                    format!("Sticking #{target_id}")
                                }
                                textquest_common::nav::NavStatus::Circling { radius, .. } => {
                                    format!("Circling r={radius:.0}")
                                }
                            };
                            let zone = if state.zone_short_name.is_empty() {
                                "(unknown)".to_string()
                            } else {
                                state.zone_short_name.clone()
                            };
                            println!(
                                "{:<7}{:<14}{:<18}{:<24}{:<6}{:<4}{:<16}{:<8}{:<12}{:<12}{:<12}{:<10}{:<7}",
                                pid,
                                player.name,
                                zone,
                                pos,
                                hp,
                                player.level,
                                nav,
                                nav_display.active,
                                nav_display.mesh_loaded,
                                nav_display.path_exists,
                                nav_display.path_length,
                                nav_display.velocity,
                                state.nearby_spawns.len()
                            );
                        } else {
                            println!(
                                "{:<7}{:<14}{:<18}{:<24}{:<6}{:<4}{:<16}{:<8}{:<12}{:<12}{:<12}{:<10}{:<7}",
                                pid,
                                "(no player)",
                                "(not in world)",
                                "-",
                                "-",
                                "-",
                                "-",
                                nav_display.active,
                                nav_display.mesh_loaded,
                                nav_display.path_exists,
                                nav_display.path_length,
                                nav_display.velocity,
                                "-"
                            );
                        }
                    }
                    None => {
                        println!(
                            "{:<7}{:<14}{:<18}{:<24}{:<6}{:<4}{:<16}{:<8}{:<12}{:<12}{:<12}{:<10}{:<7}",
                            pid,
                            "(no data)",
                            "-",
                            "-",
                            "-",
                            "-",
                            "-",
                            nav_display.active,
                            nav_display.mesh_loaded,
                            nav_display.path_exists,
                            nav_display.path_length,
                            nav_display.velocity,
                            "-"
                        );
                    }
                }
            }
            Err(_) => {
                println!(
                    "{:<7}{:<14}{:<18}{:<24}{:<6}{:<4}{:<16}{:<8}{:<12}{:<12}{:<12}{:<10}{:<7}",
                    pid,
                    "(no shm)",
                    "-",
                    "-",
                    "-",
                    "-",
                    "-",
                    nav_display.active,
                    nav_display.mesh_loaded,
                    nav_display.path_exists,
                    nav_display.path_length,
                    nav_display.velocity,
                    "-"
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
    use textquest_common::ipc::Command;
    use textquest_common::nav::Waypoint;

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
    use textquest_common::ipc::Command;
    use textquest_common::nav::Waypoint;

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
    info!(pid, "TextQuest inject-pid mode — targeting single process");

    let source_dll = resolve_built_dll_path()?;

    // Write session token file BEFORE injection so DLL can read it during init.
    ipc::write_session_token_file(pid)?;

    // TODO: Switch to reflective loader once cross-process import resolution is fixed.
    let staged_dll = inject::dll_prep::prepare_dll_locked(&source_dll)?;
    println!("Injecting into PID {pid}...");

    inject::loader::inject_dll(pid, staged_dll.path())?;
    let config = load_config()?;
    if config.timing_correction {
        send_timing_correction_command(pid, true)?;
    }
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
    mut password: Zeroizing<String>,
    server: &str,
    character: &str,
) -> Result<()> {
    use textquest_common::ipc::Command;

    println!("Sending StartLogin to PID {pid} (account: {account}, server: {server})...");

    let pipe = connect_authenticated_pipe(pid)?;
    let password = std::mem::take(&mut *password);
    let cmd = Command::StartLogin {
        account_name: account.to_string(),
        password,
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
pub fn run_login_mode(
    account: &str,
    password: Zeroizing<String>,
    server: &str,
    character: &str,
) -> Result<()> {
    use textquest_common::ipc::Command;

    let config = load_config()?;
    let pids = process::memory::find_processes_by_name(&config.process_name)?;

    if pids.is_empty() {
        println!("No EQ processes found. Launch EQ first, then inject, then login.");
        return Ok(());
    }

    if pids.len() == 1 {
        let result = run_login_pid_mode(pids[0], account, password, server, character);
        println!("\nLogin commands sent. Check DLL log for progress.");
        return result;
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

/// End-to-end autologin: find/spawn EQ processes → inject DLL → send StartLogin.
///
/// Reads `config/accounts.toml` for the account roster. Per-account passwords
/// come from the encrypted credential store (`data/credentials.db`) when a
/// master password is supplied. Falls back to:
/// 1. `--password` CLI flag (shared for all)
/// 2. `TEXTQUEST_PASSWORD` environment variable (shared for all)
/// 3. Interactive prompt (shared for all)
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_autologin_mode(
    filter_account: Option<String>,
    filter_group: Option<u32>,
    password_flag: Option<String>,
    master_password_flag: Option<String>,
    spawn_new: bool,
    inject_delay_secs: u64,
) -> Result<()> {
    use textquest_common::ipc::Command;

    let config = load_config()?;

    // 1. Load accounts
    let accounts_path = Path::new("config/accounts.toml");
    let accounts_config = config::AccountsConfig::load(accounts_path)
        .context("Failed to load config/accounts.toml — create it first")?;

    let mut targets: Vec<&config::AccountEntry> = accounts_config.accounts.iter().collect();

    // Filter by account name
    if let Some(ref name) = filter_account {
        targets.retain(|a| a.name.eq_ignore_ascii_case(name));
        if targets.is_empty() {
            anyhow::bail!("Account '{name}' not found in accounts.toml");
        }
    }

    // Filter by group
    if let Some(gid) = filter_group {
        targets.retain(|a| a.group == gid);
        if targets.is_empty() {
            anyhow::bail!("No accounts in group {gid} in accounts.toml");
        }
    }

    if targets.is_empty() {
        println!("No accounts configured in accounts.toml.");
        return Ok(());
    }

    println!("Autologin: {} account(s) to process", targets.len());

    // 2. Load per-account passwords from encrypted store (if master password provided)
    let master_password = master_password_flag.map(Zeroizing::new).or_else(|| {
        std::env::var("TEXTQUEST_MASTER_PASSWORD")
            .ok()
            .map(Zeroizing::new)
    });
    let credentials_map = load_credentials_store(master_password.as_ref().map(|pw| pw.as_str()))?;
    let has_per_account = !credentials_map.is_empty();

    if has_per_account {
        println!(
            "Loaded {} per-account password(s) from encrypted credential store",
            credentials_map.len()
        );
    }

    // Shared password fallback (for accounts not in the encrypted credential store)
    let shared_password = if !has_per_account {
        // Only prompt if we have no per-account passwords
        if let Some(pw) = password_flag {
            Some(Zeroizing::new(pw))
        } else if let Ok(pw) = std::env::var("TEXTQUEST_PASSWORD") {
            Some(Zeroizing::new(pw))
        } else {
            Some(
                crate::credentials::prompt::prompt_password(
                    "EQ Password (shared for all accounts): ",
                )
                .context("Failed to read password")?,
            )
        }
    } else {
        // Per-account passwords available; only use shared as fallback if provided
        password_flag
            .map(Zeroizing::new)
            .or_else(|| std::env::var("TEXTQUEST_PASSWORD").ok().map(Zeroizing::new))
    };

    // 3. Find existing EQ processes
    let mut pids = process::memory::find_processes_by_name(&config.process_name)?;
    println!("Found {} existing eqgame.exe process(es)", pids.len());

    // 4. Spawn new processes if requested
    if spawn_new {
        let eq_path = PathBuf::from(&config.launch.eq_path);
        if !eq_path.exists() {
            anyhow::bail!(
                "EQ path not found: {}. Set launch.eq_path in config/textquest.toml",
                eq_path.display()
            );
        }

        for target in &targets {
            println!("Spawning EQ for account {}...", target.name);
            match crate::launcher::spawner::spawn_eq_client(
                &eq_path,
                &target.name,
                &target.server,
                &config.launch.launch_args,
            ) {
                Ok(spawned) => {
                    println!("  PID {} spawned for {}", spawned.pid, target.name);
                    pids.push(spawned.pid);
                }
                Err(e) => {
                    error!(account = %target.name, %e, "Failed to spawn EQ");
                    println!("  FAILED to spawn for {}: {e}", target.name);
                }
            }
            // Stagger spawns
            std::thread::sleep(Duration::from_secs(3));
        }

        // Wait for spawned processes to initialize
        println!(
            "Waiting {}s for processes to initialize...",
            inject_delay_secs
        );
        std::thread::sleep(Duration::from_secs(inject_delay_secs));

        // Merge spawned PIDs with any additional processes found by re-scan.
        // Keep the spawned PIDs even if re-scan fails (OpenProcess may be denied
        // for freshly created processes that haven't loaded their main module yet).
        let rescanned = process::memory::find_processes_by_name(&config.process_name)?;
        for &pid in &rescanned {
            if !pids.contains(&pid) {
                pids.push(pid);
            }
        }
        println!("Now have {} eqgame.exe process(es)", pids.len());
    }

    if pids.is_empty() {
        println!("No EQ processes found. Use --spawn to launch them, or start EQ manually.");
        return Ok(());
    }

    // 5. Inject + Login for each process
    let source_dll = resolve_built_dll_path()?;
    let mut success_count = 0u32;
    let mut fail_count = 0u32;

    // Pair PIDs with accounts: if we spawned, they're 1:1 in order.
    // If using existing processes, we inject all and send the same login to each,
    // or match by index if we have the same count.
    let pairs: Vec<(u32, &config::AccountEntry)> = if pids.len() == targets.len() {
        // 1:1 pairing (e.g., spawned, or user has exactly N processes for N accounts)
        pids.iter().copied().zip(targets.iter().copied()).collect()
    } else if targets.len() == 1 {
        // Single account → send to all processes
        pids.iter().map(|&pid| (pid, targets[0])).collect()
    } else {
        // Multiple accounts, mismatched process count — send first account to all
        // (user should use --spawn for proper 1:1 mapping)
        println!(
            "Warning: {} accounts but {} processes. Sending first account to all.",
            targets.len(),
            pids.len()
        );
        pids.iter().map(|&pid| (pid, targets[0])).collect()
    };

    for (pid, account) in &pairs {
        println!("\n─── PID {} (account: {}) ───", pid, account.name);

        // 5a. Check if already injected by trying IPC connection
        let already_injected = connect_authenticated_pipe(*pid).is_ok();

        if already_injected {
            println!("  DLL already injected — skipping injection");
        } else {
            // 5b. Write session token + inject
            println!("  Writing session token...");
            if let Err(e) = ipc::write_session_token_file(*pid) {
                println!("  FAILED: token write: {e}");
                fail_count += 1;
                continue;
            }

            println!("  Injecting DLL...");
            let staged_dll = match inject::dll_prep::prepare_dll_locked(&source_dll) {
                Ok(staged) => staged,
                Err(e) => {
                    println!("  FAILED: staging DLL: {e}");
                    fail_count += 1;
                    continue;
                }
            };
            match inject::loader::inject_dll(*pid, staged_dll.path()) {
                Ok(()) => println!("  DLL injected successfully"),
                Err(e) => {
                    println!("  FAILED: injection: {e}");
                    fail_count += 1;
                    continue;
                }
            }

            // 5c. Wait for DLL to initialize IPC
            println!("  Waiting {}s for DLL initialization...", inject_delay_secs);
            std::thread::sleep(Duration::from_secs(inject_delay_secs));
        }

        // 5d. Resolve password for this account (stays Zeroizing until consumed)
        let acct_password: Option<Zeroizing<String>> = credentials_map
            .get(&account.name)
            .cloned()
            .or_else(|| shared_password.clone());

        let Some(acct_pw) = acct_password else {
            println!(
                "  SKIPPED: no password for '{}' (not in credential store, no shared password)",
                account.name
            );
            fail_count += 1;
            continue;
        };

        // 5e. Connect and send StartLogin (retry up to 10s for pipe to be ready)
        let pipe_result = {
            let mut last_err = None;
            let mut connected = None;
            for attempt in 0..10 {
                match connect_authenticated_pipe(*pid) {
                    Ok(pipe) => {
                        connected = Some(pipe);
                        break;
                    }
                    Err(e) => {
                        if attempt < 9 {
                            std::thread::sleep(Duration::from_secs(1));
                        }
                        last_err = Some(e);
                    }
                }
            }
            connected.ok_or_else(|| last_err.unwrap())
        };
        match pipe_result {
            Ok(pipe) => {
                let cmd = Command::StartLogin {
                    account_name: account.name.clone(),
                    password: (*acct_pw).clone(),
                    server_name: account.server.clone(),
                    character_name: account.character.clone(),
                };
                match pipe.send_async(&cmd) {
                    Ok(()) => {
                        println!("  StartLogin sent — FSM will drive UI automation");
                        success_count += 1;
                    }
                    Err(e) => {
                        println!("  FAILED to send StartLogin: {e}");
                        fail_count += 1;
                    }
                }
            }
            Err(e) => {
                println!("  FAILED to connect IPC after injection: {e}");
                fail_count += 1;
            }
        }
    }

    println!("\n═══ Autologin Summary ═══");
    println!("  Success: {success_count}");
    println!("  Failed:  {fail_count}");
    println!("\nThe DLL login FSM handles all UI steps autonomously.");
    println!(
        "Check DLL logs for progress: {}",
        paths::dll_log_path().display()
    );
    println!("{}", dump_guidance_message());

    Ok(())
}

/// Calibrate mode (--calibrate) — find all EQ processes and send `calibrate_login` to each.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_calibrate_mode() -> Result<()> {
    use textquest_common::ipc::Command;

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

    println!(
        "\nCalibration complete. Check DLL log at {}",
        paths::dll_log_path().display()
    );
    println!("{}", calibration_dump_guidance_message());
    Ok(())
}

/// Command mode (`--cmd pid command`) — send a slash command to an injected client.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_cmd_mode(pid: u32, command: &str) -> Result<()> {
    use textquest_common::ipc::Command;

    println!("Sending command to PID {pid}: {command}");

    if let Some(message) = nav::try_handle_local_slash_command(pid, command)? {
        println!("{message}");
        return Ok(());
    }

    let pipe = connect_authenticated_pipe(pid)?;
    // Send the slash command (fire-and-forget — DLL disconnects pipe after read).
    let cmd = Command::SlashCommand {
        command: command.to_string(),
    };

    pipe.send_async(&cmd).context("Failed to send command")?;

    println!("Command sent successfully.");

    Ok(())
}

/// Interact mode — send `InteractTarget` to right-click the current target.
pub fn run_interact_mode(pid: u32) -> Result<()> {
    use textquest_common::ipc::Command;

    println!("Sending InteractTarget to PID {pid}...");

    let pipe = connect_authenticated_pipe(pid)?;
    let cmd = Command::InteractTarget;
    pipe.send_async(&cmd)
        .context("Failed to send InteractTarget")?;

    println!("InteractTarget sent — the current target should be interacted with if valid.");
    Ok(())
}

/// Parse a render mode string ("normal", "strobe", "null") into a `RenderMode`.
fn parse_render_mode(s: &str) -> Result<textquest_common::ipc::RenderMode> {
    match s.to_lowercase().as_str() {
        "normal" | "n" => Ok(textquest_common::ipc::RenderMode::Normal),
        "strobe" | "s" => Ok(textquest_common::ipc::RenderMode::Strobe),
        "null" | "off" | "none" => Ok(textquest_common::ipc::RenderMode::NullRender),
        _ => anyhow::bail!("Unknown render mode '{s}'. Expected: normal, strobe, or null"),
    }
}

/// Set render mode for a single client.
///
/// # Errors
///
/// Returns an error if the pipe connection or command send fails.
pub fn run_render_mode(pid: u32, mode_str: &str) -> Result<()> {
    use textquest_common::ipc::Command;

    let mode = parse_render_mode(mode_str)?;
    println!("Setting render mode for PID {pid}: {mode}");

    let pipe = connect_authenticated_pipe(pid)?;
    let cmd = Command::SetRenderMode { mode };
    pipe.send_async(&cmd).context("Failed to send command")?;

    println!("Render mode set to '{mode}' for PID {pid}.");
    Ok(())
}

/// Set render mode for ALL injected EQ clients.
///
/// # Errors
///
/// Returns an error if no processes are found or config loading fails.
pub fn run_renderall_mode(mode_str: &str) -> Result<()> {
    use textquest_common::ipc::Command;

    let mode = parse_render_mode(mode_str)?;
    let config = load_config()?;
    let pids = process::memory::find_processes_by_name(&config.process_name)?;

    if pids.is_empty() {
        println!("No {} processes found.", config.process_name);
        return Ok(());
    }

    println!(
        "Setting render mode to '{mode}' for {} client(s)...",
        pids.len()
    );

    let mut ok = 0u32;
    let mut fail = 0u32;

    for &pid in &pids {
        match connect_authenticated_pipe(pid) {
            Ok(pipe) => {
                let cmd = Command::SetRenderMode { mode };
                if pipe.send_async(&cmd).is_ok() {
                    println!("  PID {pid}: {mode}");
                    ok += 1;
                } else {
                    println!("  PID {pid}: send failed");
                    fail += 1;
                }
            }
            Err(_) => {
                println!("  PID {pid}: not connected (not injected?)");
                fail += 1;
            }
        }
    }

    println!("Done: {ok} set, {fail} failed.");
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
        "TextQuest v{} — EQ Memory Reader (dump mode)",
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

// ─── Orchestration ──────────────────────────────────────────────────────────

/// Run the orchestrator event loop — health checks, launch coordinator, camp loop.
///
/// Blocks until Ctrl+C is pressed.
pub fn run_orchestrate_mode() -> Result<()> {
    let config = load_config()?;

    let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
    rt.block_on(async {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        // Catch Ctrl+C for graceful shutdown
        let tx = shutdown_tx.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                let _ = tx.send(true);
            }
        });

        let mut oloop =
            crate::orchestrator_loop::OrchestratorLoop::from_config(&config, shutdown_rx);

        info!("Orchestrator loop starting — press Ctrl+C to stop");
        eprintln!("Orchestrator loop running. Press Ctrl+C to stop.");

        let events = oloop.run().await;
        info!(events = events.len(), "Orchestrator loop stopped");
        eprintln!("Orchestrator loop stopped ({} events).", events.len());
    });

    Ok(())
}

// ─── Daemon lifecycle ───────────────────────────────────────────────────────

const PIDFILE_PATH: &str = "textquest.pid";

/// Start the TextQuest daemon — launches TUI + background services.
///
/// In foreground mode, runs the TUI directly. When daemonized (future),
/// writes a PID file and runs headless.
pub fn run_start_mode(foreground: bool) -> Result<()> {
    if let Ok(contents) = std::fs::read_to_string(PIDFILE_PATH)
        && let Ok(pid) = contents.trim().parse::<u32>()
        && process_is_alive(pid)
    {
        eprintln!("TextQuest daemon is already running (PID {pid}).");
        eprintln!("Use `textquest stop` to shut it down first.");
        return Ok(());
    }
    // Clean up any stale PID file
    let _ = std::fs::remove_file(PIDFILE_PATH);

    // Write our PID file
    let pid = std::process::id();
    std::fs::write(PIDFILE_PATH, pid.to_string()).context("Failed to write PID file")?;
    info!(pid, foreground, "TextQuest daemon starting");

    if foreground {
        eprintln!("TextQuest daemon starting in foreground (PID {pid})...");
        let result = run_tui_mode();
        let _ = std::fs::remove_file(PIDFILE_PATH);
        result
    } else {
        // For now, foreground is the only mode — true daemonization requires
        // platform-specific fork/setsid on Unix or service registration on Windows.
        eprintln!("TextQuest daemon starting (PID {pid})...");
        eprintln!("(Background mode not yet implemented — running in foreground)");
        let result = run_tui_mode();
        let _ = std::fs::remove_file(PIDFILE_PATH);
        result
    }
}

/// Stop a running TextQuest daemon by sending it a termination signal.
pub fn run_stop_mode() -> Result<()> {
    let pidfile = Path::new(PIDFILE_PATH);
    if !pidfile.exists() {
        eprintln!("No TextQuest daemon is running (no PID file found).");
        return Ok(());
    }

    let contents = std::fs::read_to_string(pidfile).context("Failed to read PID file")?;
    let pid: u32 = contents.trim().parse().context("Invalid PID in PID file")?;

    if !process_is_alive(pid) {
        eprintln!("TextQuest daemon (PID {pid}) is not running. Cleaning up stale PID file.");
        let _ = std::fs::remove_file(pidfile);
        return Ok(());
    }

    eprintln!("Stopping TextQuest daemon (PID {pid})...");
    send_terminate(pid)?;

    // Wait up to 5 seconds for graceful shutdown
    for _ in 0..50 {
        if !process_is_alive(pid) {
            let _ = std::fs::remove_file(pidfile);
            eprintln!("TextQuest daemon stopped.");
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    eprintln!("Daemon did not stop within 5 seconds. PID file retained.");
    Ok(())
}

/// Show the running daemon's status.
pub fn run_daemon_status_mode() -> Result<()> {
    let pidfile = Path::new(PIDFILE_PATH);
    if !pidfile.exists() {
        eprintln!("TextQuest is not running (no PID file).");
        return Ok(());
    }

    let contents = std::fs::read_to_string(pidfile).context("Failed to read PID file")?;
    let pid: u32 = contents.trim().parse().context("Invalid PID in PID file")?;

    if process_is_alive(pid) {
        eprintln!("TextQuest daemon is running (PID {pid}).");
    } else {
        eprintln!("TextQuest daemon is NOT running (stale PID file for PID {pid}).");
        let _ = std::fs::remove_file(pidfile);
    }

    // Also show connected EQ clients
    let config = load_config()?;
    match process::memory::find_processes_by_name(&config.process_name) {
        Ok(pids) if !pids.is_empty() => {
            eprintln!("Connected EQ clients: {} ({:?})", pids.len(), pids);
        }
        _ => {
            eprintln!("No EQ clients detected.");
        }
    }

    Ok(())
}

// ─── Dashboard ──────────────────────────────────────────────────────────────

/// Launch the web dashboard.
pub fn run_dashboard_mode(port: u16, open: bool) -> Result<()> {
    eprintln!("Starting TextQuest web dashboard on http://127.0.0.1:{port}");
    info!(port, "Web dashboard starting");

    if open {
        // Best-effort browser open
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open")
            .arg(format!("http://127.0.0.1:{port}"))
            .spawn();
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", &format!("http://127.0.0.1:{port}")])
            .spawn();
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open")
            .arg(format!("http://127.0.0.1:{port}"))
            .spawn();
    }

    // The actual Axum server lives in textquest-web. For now we just exec it.
    // Once integrated, this will spawn the server as a tokio task.
    eprintln!("Dashboard server not yet integrated — run `cargo run -p textquest-web` separately.");
    eprintln!("Integration planned for M6 milestone.");
    Ok(())
}

// ─── Configuration ─────────────────────────────────���────────────────────────

/// Validate the configuration file.
pub fn run_config_check_mode(path: Option<&str>) -> Result<()> {
    let config_path = path.unwrap_or("config/textquest.toml");
    let p = Path::new(config_path);

    if !p.exists() {
        // Try the legacy path
        let legacy = Path::new("config/frostreaver.toml");
        if legacy.exists() {
            eprintln!(
                "Config file not found at {config_path}, using legacy path: config/frostreaver.toml"
            );
            let cfg = config::AppConfig::load(legacy).context("Failed to parse configuration")?;
            eprintln!("Configuration is valid.");
            eprintln!("  Process name: {}", cfg.process_name);
            eprintln!("  Max spawns: {}", cfg.max_spawns);
            return Ok(());
        }
        eprintln!("No configuration file found at {config_path}.");
        eprintln!("Using built-in defaults.");
        let cfg = config::AppConfig::default_config();
        eprintln!("  Process name: {}", cfg.process_name);
        eprintln!("  Max spawns: {}", cfg.max_spawns);
        return Ok(());
    }

    let cfg = config::AppConfig::load(p).context("Configuration validation failed")?;
    eprintln!("Configuration is valid: {config_path}");
    eprintln!("  Process name: {}", cfg.process_name);
    eprintln!("  Max spawns: {}", cfg.max_spawns);
    Ok(())
}

/// Print the resolved configuration.
pub fn run_config_show_mode() -> Result<()> {
    let cfg = load_config()?;
    eprintln!("Resolved configuration:");
    eprintln!("  Process name: {}", cfg.process_name);
    eprintln!("  Max spawns: {}", cfg.max_spawns);
    // Additional fields can be printed as the config struct grows
    Ok(())
}

// ─── Credential management ──────────────────────────────────────────────────

const CREDENTIAL_DB_PATH: &str = "data/credentials.db";
const CREDENTIAL_META_TABLE: &str = "credential_store_meta";

/// Load decrypted account passwords from the encrypted credential store.
///
/// Returns an empty map if no master password is provided.
fn load_credentials_store(
    master_password: Option<&str>,
) -> Result<std::collections::HashMap<String, Zeroizing<String>>> {
    let mut map = std::collections::HashMap::new();

    let Some(master_password) = master_password else {
        return Ok(map);
    };

    let store = open_credential_store(master_password)?;
    for account in store.list_accounts()? {
        let password = store
            .get_password(&account)
            .with_context(|| format!("Failed to load credential for account '{account}'"))?;
        map.insert(account, password);
    }

    Ok(map)
}

/// Add or update an account credential.
pub fn run_credential_add_mode(
    account: &str,
    master_password: zeroize::Zeroizing<String>,
) -> Result<()> {
    let store = open_credential_store(&master_password)?;

    let account_password = crate::credentials::prompt::prompt_password("Account password: ")
        .context("Failed to read account password")?;

    store.add_account(account, account_password.as_str())?;
    eprintln!("Account '{account}' added/updated.");
    Ok(())
}

/// List all stored account names.
pub fn run_credential_list_mode(master_password: zeroize::Zeroizing<String>) -> Result<()> {
    let store = open_credential_store(&master_password)?;
    let accounts = store.list_accounts()?;

    if accounts.is_empty() {
        eprintln!("No accounts stored.");
    } else {
        eprintln!("{} account(s):", accounts.len());
        for name in &accounts {
            eprintln!("  - {name}");
        }
    }
    Ok(())
}

/// Remove an account credential.
pub fn run_credential_remove_mode(
    account: &str,
    master_password: zeroize::Zeroizing<String>,
) -> Result<()> {
    let store = open_credential_store(&master_password)?;
    store.remove_account(account)?;
    eprintln!("Account '{account}' removed.");
    Ok(())
}

pub fn open_credential_store(
    master_password: &str,
) -> Result<crate::credentials::store::CredentialStore> {
    let db_path = std::path::PathBuf::from(CREDENTIAL_DB_PATH);
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let salt = load_or_create_master_salt(&db_path)?;
    let master_key = crate::credentials::crypto::derive_key(master_password, &salt)?;
    crate::credentials::store::CredentialStore::open(&db_path, master_key)
}

fn load_or_create_master_salt(db_path: &std::path::Path) -> Result<[u8; 32]> {
    use rusqlite::OptionalExtension;

    let conn = rusqlite::Connection::open(db_path).with_context(|| {
        format!(
            "Failed to open credential metadata DB at {}",
            db_path.display()
        )
    })?;
    conn.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS {CREDENTIAL_META_TABLE} (key TEXT PRIMARY KEY, value BLOB NOT NULL)"
        ),
        [],
    )
    .context("Failed to initialize credential metadata table")?;

    let salt_blob: Option<Vec<u8>> = conn
        .query_row(
            &format!("SELECT value FROM {CREDENTIAL_META_TABLE} WHERE key = 'master_salt'"),
            [],
            |row| row.get(0),
        )
        .optional()
        .context("Failed to query credential master salt")?;

    if let Some(salt_blob) = salt_blob {
        return salt_blob
            .try_into()
            .map_err(|_| anyhow::anyhow!("Stored credential master salt has invalid length"));
    }

    let salt = crate::credentials::crypto::generate_salt();
    conn.execute(
        &format!("INSERT INTO {CREDENTIAL_META_TABLE} (key, value) VALUES ('master_salt', ?1)"),
        [&salt[..]],
    )
    .context("Failed to persist credential master salt")?;
    Ok(salt)
}

// ─── Platform helpers ───────────────────────────────────────────────────────

/// Check if a process with the given PID is still alive.
fn process_is_alive(_pid: u32) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, _pid).is_ok() }
    }
    #[cfg(not(windows))]
    {
        // Use `kill -0` to check process existence without sending a signal
        std::process::Command::new("kill")
            .args(["-0", &_pid.to_string()])
            .output()
            .is_ok_and(|o| o.status.success())
    }
}

/// Send a terminate signal to a process.
fn send_terminate(_pid: u32) -> Result<()> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, false, _pid)
                .context("Failed to open process for termination")?;
            TerminateProcess(handle, 0).context("Failed to terminate process")?;
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        // Send SIGTERM via kill command
        std::process::Command::new("kill")
            .args(["-TERM", &_pid.to_string()])
            .output()
            .context("Failed to send SIGTERM")?;
        Ok(())
    }
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
    use crate::process::memory::is_probably_valid_process_ptr;
    use textquest_common::offsets::{self, spawn_manager};

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

    if !is_probably_valid_process_ptr(mgr_addr) {
        error!("SpawnManager pointer is invalid: {mgr_addr:#x}");
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
                    let looks_like_ptr = is_probably_valid_process_ptr(val as usize);
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
                    let looks_like_ptr = is_probably_valid_process_ptr(val as usize);
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
    match proc.read_string(
        first_node + textquest_common::offsets::player_base::NAME,
        64,
    ) {
        Ok(name) => info!(
            "  Name at +{:#x}: \"{}\"",
            textquest_common::offsets::player_base::NAME,
            name
        ),
        Err(e) => error!("  Failed to read name: {:#}", e),
    }

    // Step 7: Probe offsets +0x00 through +0x38 for valid pointers to other spawns
    info!("--- Probing offsets +0x00..+0x38 on first spawn for valid pointers ---");
    for offset in [0x00usize, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38] {
        match proc.read_ptr(first_node + offset) {
            Ok(val) => {
                let looks_like_ptr = is_probably_valid_process_ptr(val);
                if looks_like_ptr {
                    // Try reading a name to confirm it points to another PlayerClient
                    let name_check = proc
                        .read_string(val + textquest_common::offsets::player_base::NAME, 64)
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
                    proc.read_string(alt + textquest_common::offsets::player_base::NAME, 64)
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
                    proc.read_string(alt + textquest_common::offsets::player_base::NAME, 64)
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
    let config_path = Path::new("config/textquest.toml");
    if config_path.exists() {
        config::AppConfig::load(config_path).context("Failed to load configuration")
    } else {
        Ok(config::AppConfig::default_config())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        calibration_dump_guidance_message, dump_guidance_message, load_pid_session,
        nav_signal_display, resolve_navmesh_zone,
    };
    use textquest_common::nav::NavStateSignals;

    #[test]
    fn load_pid_session_errors_without_token_file() {
        let pid = 4_242_421;
        let token_dir = std::env::temp_dir().join("textquest");
        let _ = std::fs::remove_file(token_dir.join(format!("token_{}.bin", pid)));
        let _ = std::fs::remove_file(token_dir.join(format!("login_token_{}.bin", pid)));

        let err = load_pid_session(pid).expect_err("missing token should error");
        assert!(err.to_string().contains("No session token"));
    }

    #[test]
    fn load_pid_session_reads_written_token() {
        let pid = 4_242_422;
        let token_dir = std::env::temp_dir().join("textquest");
        let token_path = token_dir.join(format!("token_{}.bin", pid));
        let login_token_path = token_dir.join(format!("login_token_{}.bin", pid));

        textquest_common::ipc::write_session_token_file(pid).expect("token file should be created");

        let (token, session_id) = load_pid_session(pid).expect("token should be readable");
        assert_ne!(token, [0u8; 32]);
        assert_eq!(
            session_id,
            textquest_common::ipc::session_id_from_token(&token)
        );

        let _ = std::fs::remove_file(token_path);
        let _ = std::fs::remove_file(login_token_path);
    }

    #[test]
    fn resolve_navmesh_zone_returns_explicit_zone_without_pid() {
        let zone = resolve_navmesh_zone(Some("gfaydark"), None).expect("explicit zone");
        assert_eq!(zone, "gfaydark");
    }

    #[test]
    fn nav_signal_display_formats_available_values() {
        let signals = NavStateSignals {
            active: true,
            mesh_loaded: false,
            path_exists: true,
            path_length: Some(123.4),
            velocity: 8.75,
            paused: false,
        };

        let display = nav_signal_display(Some(&signals));
        assert_eq!(display.active, "yes");
        assert_eq!(display.mesh_loaded, "no");
        assert_eq!(display.path_exists, "yes");
        assert_eq!(display.path_length, "123.4");
        assert_eq!(display.velocity, "8.8");
    }

    #[test]
    fn nav_signal_display_formats_unavailable_values() {
        let display = nav_signal_display(None);
        assert_eq!(display.active, "n/a");
        assert_eq!(display.mesh_loaded, "n/a");
        assert_eq!(display.path_exists, "n/a");
        assert_eq!(display.path_length, "n/a");
        assert_eq!(display.velocity, "n/a");
    }

    #[test]
    fn dump_guidance_message_reports_resolved_path() {
        let message = dump_guidance_message();
        assert!(message.contains("textquest-dump.log.*"));
        assert!(message.contains("`textquest"));
    }

    #[test]
    fn calibration_guidance_message_reports_resolved_path() {
        let message = calibration_dump_guidance_message();
        assert!(message.contains("textquest-dump.log.*"));
        assert!(message.contains("check "));
    }
}
