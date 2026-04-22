use anyhow::Result;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::{Terminal, prelude::CrosstermBackend};
use std::{
    collections::{HashMap, HashSet},
    io,
    time::{Duration, Instant},
};

#[cfg(windows)]
use std::sync::LazyLock;

/// RAII guard that restores the terminal on drop, even if a panic unwinds.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
        // show_cursor requires a terminal instance; fall back to raw crossterm command
        let _ = crossterm::execute!(io::stdout(), crossterm::cursor::Show);
    }
}

use super::{
    app::{App, ChChainStatus, NavClientStatus, send_slash_command},
    cast::{CastDisplay, short_cast_label},
    event::handle_events,
    live_cast_capture::{LIVE_CAST_CAPTURE_ENV, live_cast_capture_enabled},
    ui::{
        ch_chain::{CastState as ChPanelCastState, ChainCleric},
        draw,
    },
};
use crate::{eq::structs::SpawnInfo, orchestrator::Orchestrator};
#[cfg(any(windows, test))]
use textquest_common::nav::{NavStatus, PauseReason};

#[cfg(windows)]
use super::live_cast_capture::{
    LiveCastCaptureSnapshot, diff_live_cast_capture, log_live_cast_capture_event,
};
#[cfg(windows)]
use crate::ipc::shared::SharedStateReader;

/// Soul Engine tick interval (5 seconds).
const SOUL_TICK_INTERVAL: Duration = Duration::from_secs(5);

/// How often to scan for new EQ processes (10 seconds).
const PROCESS_SCAN_INTERVAL: Duration = Duration::from_secs(10);

/// How often to poll log watchers (2 seconds).
const LOG_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// How often to poll DLL clients for captured packet events (500ms).
const PACKET_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// How often to poll live memory for the Debug hex dump (500ms).
const MEMORY_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Camp loop tick interval (1 second).
const CAMP_TICK_INTERVAL: Duration = Duration::from_secs(1);

/// Selected-client spawn polling cadence.
#[cfg(any(windows, test))]
const ACTIVE_SPAWN_REFRESH_INTERVAL: Duration = Duration::from_millis(250);

/// Background-client spawn polling cadence.
#[cfg(any(windows, test))]
const BACKGROUND_SPAWN_REFRESH_INTERVAL: Duration = Duration::from_millis(1000);

#[cfg(windows)]
static PERF_TRACE_ENABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var(textquest_common::ipc::PERF_TRACE_ENV)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
});

/// Initialize crossterm, run the TUI loop, and clean up on exit.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_tui(mut app: App, mut orchestrator: Orchestrator) -> Result<()> {
    if live_cast_capture_enabled() {
        tracing::info!(
            target: "textquest::cast_capture",
            env = LIVE_CAST_CAPTURE_ENV,
            log_path = "logs/textquest.log",
            "Live cast capture enabled"
        );
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // RAII guard ensures terminal is restored even if run_loop panics.
    let guard = TerminalGuard;

    let result = run_loop(&mut terminal, &mut app, &mut orchestrator);

    // Normal exit: disarm the guard and do explicit cleanup so we can use
    // the terminal backend directly. Errors are ignored to preserve `result`.
    std::mem::forget(guard);
    let _ = disable_raw_mode();
    let _ = crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    orchestrator: &mut Orchestrator,
) -> Result<()> {
    let refresh_interval = Duration::from_millis(app.refresh_rate_ms);
    let mut last_refresh = Instant::now();
    let mut last_soul_tick = Instant::now();
    let mut last_process_scan = Instant::now();
    let mut last_camp_tick = Instant::now();
    let mut last_log_poll = Instant::now();
    let mut last_packet_poll = Instant::now();
    let mut last_memory_poll = Instant::now();
    let mut process_handles: HashMap<u32, crate::process::memory::ProcessHandle> = HashMap::new();
    #[cfg(windows)]
    let mut shared_state_readers: HashMap<u32, SharedStateReader> = HashMap::new();
    #[cfg(windows)]
    let mut shared_state_reader_retry_at: HashMap<u32, Instant> = HashMap::new();

    while app.running {
        // Draw the UI
        terminal.draw(|frame| draw(frame, app))?;

        // Handle keyboard events (with a short poll timeout so we stay responsive)
        let poll_timeout = Duration::from_millis(50);
        handle_events(app, poll_timeout, orchestrator)?;

        // Periodic scan for new/lost EQ processes
        if last_process_scan.elapsed() >= PROCESS_SCAN_INTERVAL {
            scan_for_clients(app);
            let active_live_pids: HashSet<u32> = app
                .clients
                .iter()
                .filter(|client| !client.is_demo)
                .map(|client| client.pid)
                .collect();
            process_handles.retain(|pid, _| active_live_pids.contains(pid));
            #[cfg(windows)]
            shared_state_readers.retain(|pid, _| active_live_pids.contains(pid));
            #[cfg(windows)]
            shared_state_reader_retry_at.retain(|pid, _| active_live_pids.contains(pid));

            // Sync orchestrator's client list from app
            orchestrator.client_pids = app.clients.iter().map(|c| c.pid).collect();
            orchestrator.client_names = app
                .clients
                .iter()
                .map(|c| (c.pid, c.character_name.clone()))
                .collect();
            orchestrator.routing_scope = app.routing_scope.clone();
            orchestrator.scope_pids = app.focused_pids();
            last_process_scan = Instant::now();
        }

        // Periodic data refresh from EQ process
        if last_refresh.elapsed() >= refresh_interval {
            #[cfg(windows)]
            refresh_eq_data(
                app,
                &mut process_handles,
                &mut shared_state_readers,
                &mut shared_state_reader_retry_at,
            );
            #[cfg(not(windows))]
            refresh_eq_data(app, &mut process_handles);
            app.tick_count += 1;
            app.clear_expired_toast();
            let demo_mode =
                !app.clients.is_empty() && app.clients.iter().all(|client| client.is_demo);
            if demo_mode {
                apply_demo_scenario(app);
            } else {
                app.sync_ch_chain_state(orchestrator);
            }
            app.update_tracked_spawns();
            app.update_spawn_alerts();
            app.update_gm_detection();
            if let Err(error) = app.service_alerts() {
                tracing::warn!(%error, "Failed to service operational alerts");
            }
            last_refresh = Instant::now();
        }

        // Camp loop tick (every 1 second)
        if last_camp_tick.elapsed() >= CAMP_TICK_INTERVAL {
            let dispatched = orchestrator.tick();
            forward_combat_soul_events(app, orchestrator);
            if dispatched > 0 {
                app.status_message = format!(
                    "Camp: {} cmds dispatched | {}",
                    dispatched,
                    orchestrator.camp_status()
                );
            }
            last_camp_tick = Instant::now();
        }

        // Poll log watchers (every 2 seconds)
        if last_log_poll.elapsed() >= LOG_POLL_INTERVAL {
            poll_log_watchers(app);
            last_log_poll = Instant::now();
        }

        // Poll DLL clients for captured packet events.
        if last_packet_poll.elapsed() >= PACKET_POLL_INTERVAL && !app.packet_monitor_state.paused {
            let pids: Vec<u32> = app
                .clients
                .iter()
                .filter(|c| !c.is_demo)
                .map(|c| c.pid)
                .collect();
            for pid in pids {
                let events = orchestrator.poll_packets(pid);
                for evt in events {
                    app.packet_monitor_state
                        .push(crate::tui::state::PacketRecord {
                            client_id: evt.client_id,
                            opcode: evt.opcode,
                            direction: evt.direction,
                            timestamp_ms: evt.timestamp_ms,
                            payload_size: evt.payload_size,
                            payload: evt.payload,
                        });
                }

                let spawn_events = orchestrator.poll_spawn_events(pid);
                app.apply_spawn_events(spawn_events);
            }
            last_packet_poll = Instant::now();
        }

        // Poll live memory for the Debug hex dump.
        if last_memory_poll.elapsed() >= MEMORY_POLL_INTERVAL
            && app.active_screen == super::app::ActiveScreen::Debug
            && app.hex_state.hex_address != 0
        {
            // Use the first live (non-demo) client PID.
            let pid = app.clients.iter().find(|c| !c.is_demo).map(|c| c.pid);
            if let Some(pid) = pid
                && let Some((address, bytes)) =
                    orchestrator.read_memory(pid, app.hex_state.hex_address, 0x200)
                && app.hex_state.hex_address == address
            {
                app.hex_state.hex_data = bytes;
            }
            last_memory_poll = Instant::now();
        }

        // Poll Discord bridge for inbound commands.
        // Collect commands first to avoid borrow conflict with app mutation.
        let discord_cmds: Vec<_> = app
            .discord_bridge
            .as_ref()
            .map(|b| {
                let mut cmds = Vec::new();
                while let Some(cmd) = b.poll() {
                    cmds.push(cmd);
                }
                cmds
            })
            .unwrap_or_default();
        for cmd in discord_cmds {
            tracing::info!(
                sender = %cmd.sender,
                command = %cmd.command,
                "Discord command received"
            );
            let channel_id = cmd.channel_id.clone();
            if !discord_sender_is_authorized(app, &cmd.sender) {
                tracing::warn!(
                    sender = %cmd.sender,
                    "Rejected Discord command from unauthorized sender"
                );
                if let Some(ref bridge) = app.discord_bridge {
                    bridge.respond(crate::discord::bridge::BridgeResponse {
                        message: "Discord command rejected: unauthorized sender".to_string(),
                        channel_id,
                    });
                }
                continue;
            }
            app.cmd_state.command_buffer = cmd.command;
            app.execute_command(orchestrator);
            let response = app.status_message.clone();
            if let Some(ref bridge) = app.discord_bridge {
                bridge.respond(crate::discord::bridge::BridgeResponse {
                    message: response,
                    channel_id,
                });
            }
            app.cmd_state.command_buffer.clear();
        }

        // Soul Engine tick (every 5 seconds)
        if last_soul_tick.elapsed() >= SOUL_TICK_INTERVAL {
            tick_soul_engine(app, orchestrator);
            last_soul_tick = Instant::now();
        }

        // Consume the pending_memory_poll flag set by :addr command.
        // When ReadMemory IPC is available, this is where the live poll is triggered.
        if app.hex_state.pending_memory_poll {
            app.hex_state.pending_memory_poll = false;
            tracing::debug!(
                address = app.hex_state.hex_address,
                "pending_memory_poll consumed — ReadMemory poll would fire here"
            );
        }
    }

    Ok(())
}

fn discord_sender_is_authorized(app: &App, sender: &str) -> bool {
    let allowlist = &app.discord_command_allowed_senders;
    if allowlist.is_empty() {
        return false;
    }
    allowlist.contains(&sender.trim().to_ascii_lowercase())
}

/// Scan for EQ processes and update the client list.
fn scan_for_clients(app: &mut App) {
    #[cfg(windows)]
    {
        scan_for_clients_live(app);
    }

    #[cfg(not(windows))]
    {
        // On macOS/Linux, demo clients are loaded in load_demo_data
        let _ = app;
    }
}

/// Live process scanning — only compiles on Windows.
#[cfg(windows)]
fn scan_for_clients_live(app: &mut App) {
    use super::app::ClientState;
    use crate::process::memory::{ProcessHandle, find_processes_by_name};

    let Ok(pids) = find_processes_by_name("eqgame.exe") else {
        return;
    };
    let window_fields = eq_window_fields_by_pid();

    // Track which PIDs we already have
    let existing_pids: std::collections::HashSet<u32> = app.clients.iter().map(|c| c.pid).collect();

    // Remove clients whose process has gone away (but keep demo clients)
    app.clients.retain(|c| c.is_demo || pids.contains(&c.pid));

    // If we found real EQ processes, clear out any demo clients
    if !pids.is_empty() {
        app.clients.retain(|c| !c.is_demo);
    }

    // Add newly discovered processes
    for &pid in &pids {
        if existing_pids.contains(&pid) {
            continue;
        }

        if let Ok(proc) = ProcessHandle::open(pid)
            && let Ok(base) = crate::get_module_base(&proc)
        {
            let mut client = ClientState::new(pid, base);

            // Read zone name from memory if possible
            if let Ok(zone) = crate::eq::spawn::read_zone_name(&proc, base) {
                client.zone_name = zone;
            } else {
                apply_window_fields(&mut client, window_fields.get(&pid));
            }

            tracing::info!(
                pid,
                base = format!("{:#x}", base),
                "Attached to new EQ client"
            );
            app.clients.push(client);
        }
    }

    // Adjust selected_client if it's now out of bounds
    if app.selected_client >= app.clients.len() && !app.clients.is_empty() {
        app.selected_client = 0;
    }

    // Update status
    let count = app.clients.len();
    if count > 0 {
        app.status_message = format!(
            "{} EQ client{} attached",
            count,
            if count == 1 { "" } else { "s" }
        );
    } else {
        app.status_message = String::from("No EQ process found - scanning...");
    }

    // Apply the deterministic demo script immediately so the first frame is lively.
    apply_demo_scenario(app);
    app.reload_map_for_selected_client();
}

fn apply_demo_scenario(app: &mut App) {
    if app.clients.is_empty() || !app.clients.iter().all(|client| client.is_demo) {
        return;
    }

    let tick_count = app.tick_count;
    let refresh_rate_ms = app.refresh_rate_ms;
    let demo_pids: HashSet<u32> = app
        .clients
        .iter()
        .filter(|client| client.is_demo)
        .map(|client| client.pid)
        .collect();
    app.nav_state
        .nav_statuses
        .retain(|pid, status| !demo_pids.contains(pid) || !status.is_demo_scripted);

    let mut chain_clerics = Vec::new();
    let mut chain_target_id = 0u32;
    let mut chain_target_name = String::new();

    for client in app.clients.iter_mut() {
        let Some(snapshot) = super::demo_data::demo_client_snapshot(
            &client.character_name,
            client.pid,
            tick_count,
            refresh_rate_ms,
        ) else {
            continue;
        };

        if let Some(player) = client.local_player.as_mut() {
            player.stand_state = snapshot.stand_state;
            player.hp_current = snapshot.hp_current;
            player.mana_current = snapshot.mana_current;
            player.x = snapshot.position.0;
            player.y = snapshot.position.1;
            player.z = snapshot.position.2;
            player.heading = snapshot.position.3;
            player.cast_state = Some(super::demo_data::demo_eq_cast_state(snapshot.cast));
        }

        client.client_status = snapshot.status_line.clone();
        client.target = snapshot
            .target_spawn_name
            .and_then(|target_name| demo_spawn_by_name(&client.spawns, target_name))
            .map(|mut spawn| {
                if let Some(label) = snapshot.target_label.clone() {
                    spawn.displayed_name = label;
                }
                spawn
            });

        if let Some(nav) = snapshot.nav.as_ref() {
            let has_manual_nav = app
                .nav_state
                .nav_statuses
                .get(&client.pid)
                .is_some_and(|status| !status.is_demo_scripted);
            if !has_manual_nav {
                app.nav_state.nav_statuses.insert(
                    client.pid,
                    NavClientStatus {
                        destination: nav.destination.clone(),
                        status: nav.status.clone(),
                        eta_secs: None,
                        waypoints: nav.waypoints.clone(),
                        path_exists: true,
                        path_length: None,
                        failure_reason: None,
                        route_state: match &nav.status {
                            textquest_common::nav::NavStatus::Moving { .. } => {
                                String::from("Regroup route")
                            }
                            textquest_common::nav::NavStatus::Paused { .. } => {
                                String::from("Route paused")
                            }
                            textquest_common::nav::NavStatus::Stuck { .. } => {
                                String::from("Recovery route")
                            }
                            textquest_common::nav::NavStatus::Arrived => {
                                String::from("Route complete")
                            }
                            textquest_common::nav::NavStatus::Idle => String::from("Standing by"),
                            textquest_common::nav::NavStatus::Following { leader_name, .. } => {
                                format!("Following {leader_name}")
                            }
                            textquest_common::nav::NavStatus::Sticking { target_id, .. } => {
                                format!("Sticking to #{target_id}")
                            }
                            textquest_common::nav::NavStatus::Circling { radius, .. } => {
                                format!("Circling r={radius:.0}")
                            }
                        },
                        recovery_state: match &nav.status {
                            textquest_common::nav::NavStatus::Stuck { recovery_attempt } => Some(
                                format!("Trying alternate line (attempt {})", recovery_attempt),
                            ),
                            textquest_common::nav::NavStatus::Paused { .. } => {
                                Some(String::from("Waiting for target stability"))
                            }
                            _ => None,
                        },
                        blockers: match &nav.status {
                            textquest_common::nav::NavStatus::Stuck { .. } => vec![format!(
                                "Path to {} is obstructed; waiting for recovery movement.",
                                nav.destination
                            )],
                            textquest_common::nav::NavStatus::Paused { .. } => vec![format!(
                                "Navigation paused near {}; waiting for stable target.",
                                nav.destination
                            )],
                            _ => Vec::new(),
                        },
                        is_demo_scripted: true,
                        blocker_description: None,
                        retry_count: 0,
                        fallback_route: None,
                        progress_pct: 0.0,
                    },
                );
            }
        }

        if let Some(profile) =
            super::demo_data::demo_client_profile(&client.character_name, client.pid)
        {
            match profile.role {
                super::demo_data::DemoRole::MainTank => {
                    if let Some(player) = client.local_player.as_ref() {
                        chain_target_id = player.spawn_id;
                    }
                    chain_target_name = profile.name.to_string();
                }
                super::demo_data::DemoRole::ChainCleric
                | super::demo_data::DemoRole::ChainClericTwo => {
                    let cast_state = match (snapshot.action_state, snapshot.cast) {
                        (super::demo_data::DemoActionState::Casting, Some(cast)) => {
                            ChPanelCastState::Casting(cast.progress)
                        }
                        (super::demo_data::DemoActionState::Sitting, _) => {
                            ChPanelCastState::Completed
                        }
                        (super::demo_data::DemoActionState::Feigned, _) => ChPanelCastState::Missed,
                        _ => ChPanelCastState::Idle,
                    };

                    chain_clerics.push(ChainCleric {
                        name: profile.name.to_string(),
                        pid: client.pid,
                        position: (chain_clerics.len() + 1) as u8,
                        timing_offset_ms: 0,
                        cast_display: snapshot.cast.map(|cast| {
                            CastDisplay::exact_progress(
                                cast.spell_label,
                                short_cast_label(cast.spell_label),
                                cast.progress as f64,
                                cast.total_cast_ms as f32 / 1000.0,
                            )
                        }),
                        cast_state,
                    });
                }
                super::demo_data::DemoRole::Enchanter
                | super::demo_data::DemoRole::Shaman
                | super::demo_data::DemoRole::Druid
                | super::demo_data::DemoRole::Wizard
                | super::demo_data::DemoRole::RecoveryWizard => {}
                _ => {}
            }
        }
    }

    if !chain_clerics.is_empty() {
        app.ch_chain_panel_state.clerics = chain_clerics;
        app.ch_chain_panel_state.selected = 0;
        app.ch_chain_panel_state.target_id = chain_target_id;
        app.ch_chain_panel_state.target_name = chain_target_name;
        app.ch_chain_panel_state.cast_time_secs = 10.0;
        app.ch_chain_panel_state.overlap_buffer_secs = 0.5;
        app.ch_chain_panel_state.chain_delay_secs = 2.5;
        app.ch_chain_panel_state.adaptive = true;
        app.ch_chain_panel_state.stats.total_heals = tick_count as u32;
        app.ch_chain_panel_state.stats.missed_heals = (tick_count as u32 / 48) % 2;
        app.ch_chain_panel_state.stats.late_casts = (tick_count as u32 / 24) % 3;
        app.ch_chain_panel_state.stats.avg_cast_time_ms = 9_980.0;
        app.ch_chain_panel_state.stats.chain_uptime_pct = 97.5;

        app.ch_chain_status = Some(ChChainStatus {
            members: app.ch_chain_panel_state.clerics.len(),
            interval_secs: 2.5,
            is_adaptive: true,
            target_id: chain_target_id,
        });
    } else {
        app.ch_chain_panel_state.clerics.clear();
        app.ch_chain_panel_state.selected = 0;
        app.ch_chain_status = None;
    }

    app.priority_snapshots = super::demo_data::demo_priority_snapshots(tick_count);

    app.status_message = format!(
        "DEMO MODE - {} scripted clients",
        app.clients.iter().filter(|client| client.is_demo).count()
    );

    app.sync_from_selected_client();
}

fn demo_spawn_by_name(spawns: &[SpawnInfo], target_name: &str) -> Option<SpawnInfo> {
    spawns
        .iter()
        .find(|spawn| spawn.name == target_name || spawn.displayed_name == target_name)
        .cloned()
}

/// Parse character name and zone name from the DLL-renamed window title.
/// Format: "[TQ] EQ - `CharName` (`ZoneName`)" or "[TQ] EQ - `CharName`"
/// Falls back to the old EQ format: "`EverQuest` - Character - Zone"
#[cfg(windows)]
fn parse_title_fields(title: &str) -> (String, String) {
    // Strip optional "[TQ] " prefix before parsing.
    let title = title.strip_prefix("[TQ] ").unwrap_or(title);

    // New DLL format: "EQ - CharName (ZoneName)"
    if let Some(rest) = title.strip_prefix("EQ - ") {
        if let Some(paren_start) = rest.rfind('(') {
            let char_name = rest[..paren_start].trim().to_string();
            let zone = rest[paren_start + 1..]
                .trim_end_matches(')')
                .trim()
                .to_string();
            return (char_name, zone);
        }
        // No parentheses — just char name, no zone yet
        return (rest.trim().to_string(), String::new());
    }

    // Old EQ format: "EverQuest - Character - Zone"
    let parts: Vec<&str> = title.splitn(4, " - ").collect();
    let char_name = if parts.len() >= 2 {
        parts[1].trim().to_string()
    } else {
        String::new()
    };
    let zone = if parts.len() >= 3 {
        parts[2].trim().to_string()
    } else {
        String::from("Unknown")
    };
    (char_name, zone)
}

#[cfg(windows)]
fn eq_window_fields_by_pid() -> HashMap<u32, (String, String)> {
    crate::process::window::find_windows_by_title("EverQuest")
        .map(|windows| {
            windows
                .into_iter()
                .map(|window| (window.pid, parse_title_fields(&window.title)))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(windows)]
fn apply_window_fields(
    client: &mut super::app::ClientState,
    window_fields: Option<&(String, String)>,
) {
    let Some((character_name, zone_name)) = window_fields else {
        return;
    };

    if !character_name.is_empty() {
        client.character_name = character_name.clone();
    }
    client.zone_name = if zone_name.is_empty() {
        String::from("Unknown")
    } else {
        zone_name.clone()
    };
}

/// Refresh live EQ data. On non-Windows or when not attached, loads demo data.
fn refresh_eq_data(
    app: &mut App,
    _process_handles: &mut HashMap<u32, crate::process::memory::ProcessHandle>,
    #[cfg(windows)] _shared_state_readers: &mut HashMap<u32, SharedStateReader>,
    #[cfg(windows)] _shared_state_reader_retry_at: &mut HashMap<u32, Instant>,
) {
    #[cfg(windows)]
    {
        refresh_eq_data_live(
            app,
            _process_handles,
            _shared_state_readers,
            _shared_state_reader_retry_at,
        );
        // If no EQ processes found, load demo data so TUI is testable on Windows too
        if app.clients.is_empty() {
            load_demo_data(app);
        }
    }

    #[cfg(not(windows))]
    {
        // On macOS/Linux, load demo data so the TUI is testable
        if app.clients.is_empty() {
            load_demo_data(app);
        }
    }
}

#[cfg(any(windows, test))]
fn spawn_refresh_interval(is_selected: bool) -> Duration {
    if is_selected {
        ACTIVE_SPAWN_REFRESH_INTERVAL
    } else {
        BACKGROUND_SPAWN_REFRESH_INTERVAL
    }
}

#[cfg(any(windows, test))]
fn spawn_refresh_due(last_refresh: Option<Instant>, now: Instant, is_selected: bool) -> bool {
    last_refresh.is_none_or(|last| now.duration_since(last) >= spawn_refresh_interval(is_selected))
}

/// Live EQ memory refresh — only compiles on Windows.
/// Refreshes ALL attached clients.
#[cfg(windows)]
fn refresh_eq_data_live(
    app: &mut App,
    process_handles: &mut HashMap<u32, crate::process::memory::ProcessHandle>,
    shared_state_readers: &mut HashMap<u32, SharedStateReader>,
    shared_state_reader_retry_at: &mut HashMap<u32, Instant>,
) {
    use crate::{eq, process::memory::ProcessHandle};

    let selected_index = app.selected_client;
    let now = Instant::now();
    let mut nav_status_updates = Vec::new();
    let window_fields = eq_window_fields_by_pid();

    for (client_index, client) in app.clients.iter_mut().enumerate() {
        if client.is_demo {
            continue;
        }

        let is_selected = client_index == selected_index;
        let refresh_spawns = spawn_refresh_due(client.last_spawn_refresh, now, is_selected);
        let refresh_zone = refresh_spawns || (is_selected && client.zone_name.is_empty());

        let proc = match process_handles.remove(&client.pid) {
            Some(proc) => proc,
            None => match ProcessHandle::open(client.pid) {
                Ok(handle) => handle,
                Err(e) => {
                    client.client_status = format!("Lost connection: {e}");
                    continue;
                }
            },
        };
        let mut hard_read_failed = false;

        // Read local player
        match eq::spawn::read_local_player(&proc, client.eq_base) {
            Ok(player) => {
                if live_cast_capture_enabled() {
                    let current_capture =
                        LiveCastCaptureSnapshot::from_cast(player.cast_state.as_ref());
                    if let Some(event) = diff_live_cast_capture(
                        client.last_live_cast_capture.as_ref(),
                        current_capture.as_ref(),
                    ) {
                        let character_name = if client.character_name.is_empty() {
                            player.displayed_name.as_str()
                        } else {
                            client.character_name.as_str()
                        };
                        log_live_cast_capture_event(
                            app.tick_count,
                            client.pid,
                            character_name,
                            &event,
                        );
                    }
                    client.last_live_cast_capture = current_capture;
                } else {
                    client.last_live_cast_capture = None;
                }
                client.local_player = Some(player);
            }
            Err(e) => {
                client.last_live_cast_capture = None;
                client.client_status = format!("Player read error: {e}");
                hard_read_failed = true;
            }
        }

        // Read target
        match eq::spawn::read_target(&proc, client.eq_base) {
            Ok(target) => {
                client.target = target;
            }
            Err(e) => {
                client.client_status = format!("Target read error: {e}");
                hard_read_failed = true;
            }
        }
        client.last_fast_refresh = Some(now);

        // Read spawn list on a staged cadence: selected client stays fast, others are
        // throttled.
        if refresh_spawns {
            let perf_start = if *PERF_TRACE_ENABLED {
                Some(Instant::now())
            } else {
                None
            };
            match eq::spawn::read_all_spawns(&proc, client.eq_base, 200) {
                Ok(spawns) => {
                    client.spawns = spawns;
                    client.spawn_revision = client.spawn_revision.wrapping_add(1);
                    client.last_spawn_refresh = Some(now);
                    if let Some(start) = perf_start {
                        tracing::info!(
                            target: "textquest::perf",
                            pid = client.pid,
                            is_selected,
                            spawn_count = client.spawns.len(),
                            spawn_revision = client.spawn_revision,
                            elapsed_ms = start.elapsed().as_secs_f64() * 1000.0,
                            "TUI spawn snapshot refreshed"
                        );
                    }
                }
                Err(e) => {
                    client.client_status = format!("Spawn read error: {e}");
                    hard_read_failed = true;
                }
            }
        }

        // Read zone name on the same cadence as spawn snapshots unless the active
        // client is currently missing zone data.
        if refresh_zone {
            match eq::spawn::read_zone_name(&proc, client.eq_base) {
                Ok(zone) => client.zone_name = zone,
                Err(_) => apply_window_fields(client, window_fields.get(&client.pid)),
            }
        }

        // Read group info from memory
        match eq::spawn::read_group_info(&proc, client.eq_base) {
            Ok(group) => client.group_info = group,
            Err(e) => {
                tracing::trace!(pid = client.pid, error = %e, "Failed to read group info");
            }
        }

        if let Some((zone_name, nav_status)) = read_live_nav_state(
            client.pid,
            shared_state_readers,
            shared_state_reader_retry_at,
            now,
        ) {
            if !zone_name.is_empty() {
                client.zone_name = zone_name;
            }
            nav_status_updates.push((
                client.pid,
                build_live_nav_client_status(&nav_status, &client.zone_name),
            ));
        }

        if !hard_read_failed {
            process_handles.insert(client.pid, proc);
        }
    }

    for (pid, status) in nav_status_updates {
        app.nav_state.nav_statuses.insert(pid, status);
    }

    // Sync selected client data to legacy fields
    app.sync_from_selected_client();

    // Reload map if the selected client's zone changed (or map not yet loaded)
    app.reload_map_for_selected_client();
}

#[cfg(windows)]
fn read_live_nav_state(
    pid: u32,
    shared_state_readers: &mut HashMap<u32, SharedStateReader>,
    shared_state_reader_retry_at: &mut HashMap<u32, Instant>,
    now: Instant,
) -> Option<(String, NavStatus)> {
    if shared_state_reader_retry_at
        .get(&pid)
        .is_some_and(|retry_at| *retry_at > now)
    {
        return None;
    }

    if let std::collections::hash_map::Entry::Vacant(entry) = shared_state_readers.entry(pid) {
        let Some(token) = crate::ipc::load_session_token(pid) else {
            shared_state_reader_retry_at.insert(pid, now + Duration::from_secs(2));
            return None;
        };
        let session_id = textquest_common::ipc::session_id_from_token(&token);
        match SharedStateReader::new(pid, session_id) {
            Ok(reader) => {
                entry.insert(reader);
                shared_state_reader_retry_at.remove(&pid);
            }
            Err(error) => {
                tracing::trace!(pid, %error, "Failed to open TUI shared state reader");
                shared_state_reader_retry_at.insert(pid, now + Duration::from_secs(2));
                return None;
            }
        }
    }

    let reader = shared_state_readers.get_mut(&pid)?;
    let state = reader.read_nav_state()?;
    Some((
        resolve_live_zone_name(state.zone_long_name, state.zone_short_name),
        state.nav_status,
    ))
}

#[cfg(windows)]
fn resolve_live_zone_name(zone_long_name: String, zone_short_name: String) -> String {
    if zone_long_name.is_empty() {
        zone_short_name
    } else {
        zone_long_name
    }
}

#[cfg(any(windows, test))]
fn build_live_nav_client_status(status: &NavStatus, zone_name: &str) -> NavClientStatus {
    NavClientStatus {
        destination: live_nav_destination(status),
        status: status.clone(),
        eta_secs: None,
        waypoints: Vec::new(),
        path_exists: matches!(
            status,
            NavStatus::Moving { .. }
                | NavStatus::Paused { .. }
                | NavStatus::Stuck { .. }
                | NavStatus::Arrived
        ),
        path_length: None,
        failure_reason: None,
        route_state: live_nav_route_state(status),
        recovery_state: live_nav_recovery_state(status),
        blockers: live_nav_blockers(status, zone_name),
        is_demo_scripted: false,
        blocker_description: None,
        retry_count: 0,
        fallback_route: None,
        progress_pct: 0.0,
    }
}

#[cfg(any(windows, test))]
fn live_nav_destination(status: &NavStatus) -> String {
    match status {
        NavStatus::Idle => String::new(),
        NavStatus::Moving { .. }
        | NavStatus::Paused { .. }
        | NavStatus::Stuck { .. }
        | NavStatus::Arrived => String::from("Active route"),
        NavStatus::Following { leader_name, .. } => leader_name.clone(),
        NavStatus::Sticking { target_id, .. } => format!("Target #{target_id}"),
        NavStatus::Circling { radius, .. } => format!("Circle r={radius:.0}"),
    }
}

#[cfg(any(windows, test))]
fn live_nav_route_state(status: &NavStatus) -> String {
    match status {
        NavStatus::Moving { .. } => String::from("Live route"),
        NavStatus::Paused { .. } => String::from("Route paused"),
        NavStatus::Stuck { .. } => String::from("Recovery route"),
        NavStatus::Arrived => String::from("Route complete"),
        NavStatus::Idle => String::from("Standing by"),
        NavStatus::Following { leader_name, .. } => format!("Following {leader_name}"),
        NavStatus::Sticking { target_id, .. } => format!("Sticking to #{target_id}"),
        NavStatus::Circling { radius, .. } => format!("Circling r={radius:.0}"),
    }
}

#[cfg(any(windows, test))]
fn live_nav_recovery_state(status: &NavStatus) -> Option<String> {
    match status {
        NavStatus::Paused { reason, .. } => Some(match reason {
            PauseReason::Warp => String::from("Waiting for warp validation"),
            PauseReason::UserPause => String::from("Paused by operator command"),
            PauseReason::UserInput => String::from("Waiting for manual movement to stop"),
            PauseReason::GmNearby => String::from("Safety hold: GM nearby"),
        }),
        NavStatus::Stuck { recovery_attempt } => Some(format!(
            "Trying alternate line (attempt {})",
            recovery_attempt
        )),
        _ => None,
    }
}

#[cfg(any(windows, test))]
fn live_nav_blockers(status: &NavStatus, zone_name: &str) -> Vec<String> {
    match status {
        NavStatus::Paused { reason, .. } => vec![match reason {
            PauseReason::Warp => format!(
                "Movement paused in {zone_name}; waiting for the client to stabilize after a warp."
            ),
            PauseReason::UserPause => {
                String::from("Movement paused by operator request until /nav resume.")
            }
            PauseReason::UserInput => {
                String::from("Movement paused because local keyboard input is active.")
            }
            PauseReason::GmNearby => {
                String::from("Movement paused because break-on-GM safety is active.")
            }
        }],
        NavStatus::Stuck { recovery_attempt } => vec![format!(
            "Movement validation reported no progress; recovery attempt {} is active in {}.",
            recovery_attempt, zone_name
        )],
        NavStatus::Following {
            leader_name,
            returning,
            ..
        } => {
            if *returning {
                vec![format!("Returning to follow anchor for {leader_name}.")]
            } else {
                Vec::new()
            }
        }
        NavStatus::Sticking { in_range, .. } => {
            if *in_range {
                Vec::new()
            } else {
                vec![String::from(
                    "Closing to stick range before the target can be held in place.",
                )]
            }
        }
        _ => Vec::new(),
    }
}

/// Demo data for testing the TUI without a live EQ process.
fn load_demo_data(app: &mut App) {
    use super::app::ClientState;
    use crate::eq::structs::{SpawnInfo, SpawnType, StandState};

    app.status_message = String::from("DEMO MODE - no EQ process");

    // 18 demo clients across 3 groups, covering all 16 EQ classes.
    // Names use trailing digits (e.g., "Dmft01") so they match group slots
    // via extract_account_number().
    //
    // Format: (name, class_id, level, hp, hp_max, mana, mana_max, stand_state,
    // zone, race_id) Melee classes have mana 0. Caster/hybrid mana is
    // class-appropriate.
    type DemoClient<'a> = (
        &'a str,
        u8,
        u8,
        i64,
        i64,
        i32,
        i32,
        StandState,
        &'a str,
        u32,
    );
    let demo_clients: &[DemoClient<'_>] = &[
        // ── Group 1: Permafrost ──────────────────────────────────────
        // (name, class, lv, hp, hp_max, mana, mana_max, stand, zone, race)
        (
            "Dmft01",
            1,
            60,
            9500,
            10000,
            0,
            0,
            StandState::Standing,
            "Permafrost",
            2,
        ), // WAR Barbarian
        (
            "Iceweaver02",
            2,
            60,
            5300,
            6000,
            5400,
            7500,
            StandState::Standing,
            "Permafrost",
            1,
        ), // CLR Human
        (
            "Coldchain03",
            14,
            60,
            3200,
            4000,
            4200,
            6000,
            StandState::Standing,
            "Permafrost",
            5,
        ), // ENC High Elf
        (
            "Frostsong04",
            8,
            60,
            7200,
            8000,
            0,
            0,
            StandState::Standing,
            "Permafrost",
            7,
        ), // BRD Half Elf
        (
            "Tundrablade05",
            4,
            60,
            6600,
            8000,
            1800,
            2500,
            StandState::Standing,
            "Permafrost",
            4,
        ), // RNG Wood Elf
        (
            "Glacierstrike06",
            12,
            59,
            2800,
            3800,
            5600,
            8000,
            StandState::Sitting,
            "Permafrost",
            3,
        ), // WIZ Erudite
        // ── Group 2: Eastern Wastes ──────────────────────────────────
        (
            "Shadowveil07",
            5,
            58,
            7000,
            10000,
            3200,
            5000,
            StandState::Standing,
            "Eastern Wastes",
            6,
        ), // SK Dark Elf
        (
            "Spiritcaller08",
            10,
            58,
            4400,
            5200,
            4600,
            6000,
            StandState::Standing,
            "Eastern Wastes",
            2,
        ), // SHM Barbarian
        (
            "Verdantleaf09",
            6,
            57,
            3800,
            4200,
            4800,
            7000,
            StandState::Standing,
            "Eastern Wastes",
            4,
        ), // DRU Wood Elf
        (
            "Nightblade10",
            9,
            59,
            5800,
            9000,
            0,
            0,
            StandState::Ducking,
            "Eastern Wastes",
            6,
        ), // ROG Dark Elf
        (
            "Soulreaper11",
            11,
            58,
            3400,
            4000,
            5000,
            7000,
            StandState::Standing,
            "Eastern Wastes",
            12,
        ), // NEC Gnome
        (
            "Petmaster12",
            13,
            57,
            3000,
            3800,
            5200,
            7500,
            StandState::Standing,
            "Eastern Wastes",
            5,
        ), // MAG High Elf
        // ── Group 3: Great Divide ────────────────────────────────────
        (
            "Holyblade13",
            3,
            60,
            7600,
            9000,
            2800,
            4500,
            StandState::Standing,
            "Great Divide",
            1,
        ), // PAL Human
        (
            "Swiftfist14",
            7,
            60,
            6500,
            9000,
            0,
            0,
            StandState::Standing,
            "Great Divide",
            128,
        ), // MNK Iksar
        (
            "Beastkin15",
            15,
            57,
            5600,
            7000,
            2600,
            4000,
            StandState::Standing,
            "Great Divide",
            130,
        ), // BST Vah Shir
        (
            "Ragecleave16",
            16,
            58,
            6200,
            9000,
            0,
            0,
            StandState::Standing,
            "Great Divide",
            10,
        ), // BER Ogre
        (
            "Frostmend17",
            2,
            60,
            5700,
            6000,
            6000,
            7500,
            StandState::Sitting,
            "Great Divide",
            8,
        ), // CLR Dwarf
        (
            "Glacialsurge18",
            12,
            59,
            3100,
            3800,
            5800,
            8000,
            StandState::Feigned,
            "Great Divide",
            1,
        ), // WIZ Human
    ];

    let mut zone_cache = HashMap::new();
    for (i, &(name, class_id, level, hp, hp_max, mana, mana_max, ref stand, zone, race_id)) in
        demo_clients.iter().enumerate()
    {
        let mut client = ClientState::new(1000 + i as u32, 0x0001_4000_0000);
        client.zone_name = zone.to_string();
        let zone_short = zone_to_short_name(zone);
        let (mut x, mut y, z, heading) =
            super::demo_data::demo_player_position(zone, i).unwrap_or((
                1234.5 + (i as f32 * 100.0),
                -567.8 + (i as f32 * 50.0),
                12.0,
                128.0,
            ));
        clamp_demo_xy_to_map_bounds(
            &zone_short,
            &app.map_state.map_dir,
            &mut zone_cache,
            &mut x,
            &mut y,
        );
        client.local_player = Some(SpawnInfo {
            name: name.to_string(),
            displayed_name: name.to_string(),
            lastname: String::new(),
            level,
            class_id,
            class: crate::eq::structs::EqClass::from_id(class_id),
            stand_state: *stand,
            spawn_type: SpawnType::Player,
            hp_current: hp,
            hp_max,
            mana_current: mana,
            mana_max,
            endurance_current: 150,
            endurance_max: 200,
            x,
            y,
            z,
            heading,
            spawn_id: i as u32 + 1,
            is_gm: false,
            race_id,
            buff_slots: Vec::new(),
            spellbook: Vec::new(),
            memorized_spells: Vec::new(),
            cast_state: None,
        });
        client.character_name = name.to_string();
        client.client_status = format!("Connected: {name}");
        client.is_demo = true;
        // Demo: assign lifecycle, launch profile, and session preset per group.
        // Two clients show non-Live states for visual demo variety.
        let lifecycle = match i {
            16 => textquest_common::types::SlotLifecycle::WaitingForLogin,
            17 => textquest_common::types::SlotLifecycle::Recovering,
            _ => textquest_common::types::SlotLifecycle::Live,
        };
        client.slot_lifecycle = lifecycle;
        let (profile, preset) = match i {
            0..=5 => ("frostreaver-main", "Group Alpha"),
            6..=11 => ("frostreaver-main", "Group Beta"),
            _ => ("frostreaver-main", "Group Gamma"),
        };
        client.launch_profile = Some(profile.to_string());
        client.session_preset = Some(preset.to_string());
        app.clients.push(client);
    }

    // ── Spawns for each zone ─────────────────────────────────────────────────
    // Each group's clients share a spawn list appropriate to their zone.
    // Spawn definitions live in demo_data.rs to keep this function focused.
    for client in &mut app.clients {
        let mut spawns = super::demo_data::demo_spawns_for_zone(&client.zone_name);
        if !spawns.is_empty() {
            let zone_short = zone_to_short_name(&client.zone_name);
            for spawn in &mut spawns {
                let spawn_zone = zone_short.as_str();
                clamp_demo_xy_to_map_bounds(
                    spawn_zone,
                    &app.map_state.map_dir,
                    &mut zone_cache,
                    &mut spawn.x,
                    &mut spawn.y,
                );
            }
            client.spawns = spawns;
        }
    }

    // ── Populate live group info so the Groups screen uses dynamic grouping ──
    {
        use crate::eq::structs::GroupInfo;

        let group1_members: Vec<String> = (0..6).map(|i| demo_clients[i].0.to_string()).collect();
        let group2_members: Vec<String> = (6..12).map(|i| demo_clients[i].0.to_string()).collect();
        let group3_members: Vec<String> = (12..18).map(|i| demo_clients[i].0.to_string()).collect();

        for (i, client) in app.clients.iter_mut().enumerate() {
            let (leader, members) = if i < 6 {
                ("Dmft01", &group1_members)
            } else if i < 12 {
                ("Shadowveil07", &group2_members)
            } else {
                ("Holyblade13", &group3_members)
            };
            client.group_info = Some(GroupInfo {
                leader_name: leader.to_string(),
                members: members.clone(),
                member_count: members.len() as u8,
            });
        }
    }

    // Sync selected client to legacy fields
    app.sync_from_selected_client();

    if app.main_tank.is_none() {
        app.main_tank = Some(String::from("Dmft01"));
    }
    if app.main_assist.is_none() {
        app.main_assist = Some(String::from("Iceweaver02"));
    }
    app.operating_mode = crate::camp::hunt::OperatingMode::Hunt;

    // Load zone map for the selected client's zone
    app.reload_map_for_selected_client();
}

#[derive(Clone, Copy)]
struct DemoMapBounds {
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
}

impl DemoMapBounds {
    fn from_zone_name(zone: &str, map_dir: &std::path::Path) -> Option<Self> {
        let map = crate::eq::map_parser::load_zone_map(map_dir, zone).ok()?;
        Some(Self {
            min_x: map.bounds.min_x,
            max_x: map.bounds.max_x,
            min_y: map.bounds.min_y,
            max_y: map.bounds.max_y,
        })
    }

    fn clamp_xy(&self, x: &mut f32, y: &mut f32) {
        let pad_x = (self.max_x - self.min_x).max(220.0) * 0.09;
        let pad_y = (self.max_y - self.min_y).max(220.0) * 0.09;
        let min_x = self.min_x - pad_x;
        let max_x = self.max_x + pad_x;
        let min_y = self.min_y - pad_y;
        let max_y = self.max_y + pad_y;
        *x = (*x).clamp(min_x, max_x);
        *y = (*y).clamp(min_y, max_y);
    }
}

fn clamp_demo_xy_to_map_bounds(
    zone_short: &str,
    map_dir: &std::path::Path,
    cache: &mut HashMap<String, Option<DemoMapBounds>>,
    x: &mut f32,
    y: &mut f32,
) {
    let bounds = cache
        .entry(zone_short.to_string())
        .or_insert_with(|| DemoMapBounds::from_zone_name(zone_short, map_dir));
    if let Some(bounds) = bounds {
        bounds.clamp_xy(x, y);
    }
}

/// Convert a zone display name (long name from zoneHeader) to its EQ short name
/// for Brewall map file lookup. Handles both display names ("West Freeport")
/// and short names that are already correct ("freportw").
pub(super) fn zone_to_short_name(zone_name: &str) -> String {
    let lower = zone_name.to_lowercase();
    match lower.as_str() {
        // Classic zones
        "permafrost" | "permafrost caverns" | "permafrost keep" => "permafrost".to_string(),
        "east commonlands" | "eastern commonlands" => "ecommons".to_string(),
        "west commonlands" | "western commonlands" => "commons".to_string(),
        "west freeport" => "freeportwest".to_string(),
        "east freeport" => "freeporteast".to_string(),
        "north freeport" => "freportn".to_string(),
        "eastern wastes" => "eastwastes".to_string(),
        "western wastes" => "westwastes".to_string(),
        "great divide" | "the great divide" => "greatdivide".to_string(),
        "cobalt scar" => "cobaltscar".to_string(),
        "north karana" | "northern plains of karana" => "northkarana".to_string(),
        "south karana" | "southern plains of karana" => "southkarana".to_string(),
        "east karana" | "eastern plains of karana" => "eastkarana".to_string(),
        "lake rathetear" => "lakerathe".to_string(),
        "north ro" | "northern desert of ro" => "nro".to_string(),
        "south ro" | "southern desert of ro" => "sro".to_string(),
        "ocean of tears" => "oot".to_string(),
        "butcherblock mountains" => "butcher".to_string(),
        "greater faydark" => "gfaydark".to_string(),
        "lesser faydark" => "lfaydark".to_string(),
        "steamfont mountains" => "steamfont".to_string(),
        "misty thicket" => "misty".to_string(),
        "plane of knowledge" => "poknowledge".to_string(),
        "plane of tranquility" => "potranquility".to_string(),
        "plane of hate" => "hateplane".to_string(),
        "plane of fear" => "fearplane".to_string(),
        "plane of air" | "plane of sky" => "airplane".to_string(),
        "the bazaar" => "bazaar".to_string(),
        "the nexus" => "nexus".to_string(),
        "everfrost peaks" => "everfrost".to_string(),
        "lavastorm mountains" => "lavastorm".to_string(),
        "highpass hold" => "highpass".to_string(),
        "high keep" => "highkeep".to_string(),
        "field of bone" => "fieldofbone".to_string(),
        "emerald jungle" => "emeraldjungle".to_string(),
        "burning woods" => "burningwood".to_string(),
        "dreadlands" | "the dreadlands" => "dreadlands".to_string(),
        "lake of ill omen" => "lakeofillomen".to_string(),
        "swamp of no hope" => "swampofnohope".to_string(),
        "frontier mountains" => "frontiermtns".to_string(),
        "kael drakkel" => "kael".to_string(),
        "skyshrine" => "skyshrine".to_string(),
        "velketor's labyrinth" => "velketor".to_string(),
        _ => lower.replace(' ', ""),
    }
}

/// Tick the Soul Engine coordinator (if enabled).
/// Generates soul commands (idle behaviors, chat, emotes) for all registered
/// characters.
fn tick_soul_engine(app: &mut App, orchestrator: &Orchestrator) {
    let Some(coordinator) = app.soul_coordinator.as_mut() else {
        return;
    };

    sync_soul_registrations(coordinator, app, orchestrator);

    let (commands, alerts) = coordinator.tick(&orchestrator.game_states);

    if !commands.is_empty() {
        tracing::debug!(count = commands.len(), "Soul Engine generated commands");
        // Soul commands are logged but not dispatched in TUI demo mode.
        // The Orchestrator handles IPC delivery when live clients are
        // connected.
    }

    for alert in &alerts {
        tracing::warn!(
            character_id = alert.character_id,
            alert_type = ?alert.alert_type,
            severity = ?alert.severity,
            message = %alert.message,
            "Soul Engine anomaly detected"
        );
    }

    app.soul_tick_counter += 1;
}

fn sync_soul_registrations(
    coordinator: &mut textquest_soul::coordinator::SoulCoordinator,
    app: &App,
    orchestrator: &Orchestrator,
) {
    for (&client_id, state) in &orchestrator.game_states {
        let observed_name = state
            .local_player
            .as_ref()
            .map(|player| player.displayed_name.as_str())
            .filter(|name| !name.is_empty())
            .or_else(|| {
                app.clients
                    .iter()
                    .find(|client| client.pid == client_id)
                    .map(|client| client.character_name.as_str())
                    .filter(|name| !name.is_empty())
            });

        if let Some(name) = observed_name {
            coordinator.ensure_character_registered(client_id, name);
        }
    }
}

fn forward_combat_soul_events(app: &mut App, orchestrator: &mut Orchestrator) {
    let Some(coordinator) = app.soul_coordinator.as_mut() else {
        return;
    };

    sync_soul_registrations(coordinator, app, orchestrator);

    for (client_id, event) in orchestrator.combat.drain_soul_events() {
        if let Some(state) = orchestrator.game_states.get(&client_id)
            && let Some(player) = &state.local_player
            && !player.displayed_name.is_empty()
        {
            coordinator.ensure_character_registered(client_id, &player.displayed_name);
        }

        if let Err(error) = coordinator.emit_soul_event(client_id, event) {
            tracing::warn!(client_id, %error, "Failed to forward combat event into soul coordinator");
        }
    }
}

/// Poll all log watchers for new events and merge into the aggregate loot
/// database.
fn poll_log_watchers(app: &mut App) {
    use crate::eq::log_parser::LogEvent;
    let mut events = Vec::new();
    for watcher in &mut app.log_watchers {
        events.extend(watcher.poll());
    }

    for event in events {
        app.loot_database.record(&event);
        if let LogEvent::Chat(chat) = event {
            app.chat_events.push_back(chat.clone());
            if app.chat_events.len() > 200 {
                app.chat_events.pop_front();
            }
            let actions =
                app.chat_pattern_engine
                    .evaluate(&chat.channel, &chat.sender, &chat.message);
            for (_rule_id, action) in actions {
                handle_chat_pattern_action(app, &action);
            }
        }
    }
}

/// Handle a triggered chat pattern rule action.
fn handle_chat_pattern_action(
    app: &mut App,
    action: &textquest_common::chat_pattern_rules::RuleAction,
) {
    use textquest_common::chat_pattern_rules::RuleAction;
    match action {
        RuleAction::ExecuteCommand(cmd) => {
            if let Some(pid) = app
                .clients
                .get(app.selected_client)
                .filter(|client| !client.is_demo)
                .map(|client| client.pid)
            {
                let full_cmd = if cmd.starts_with('/') {
                    cmd.to_string()
                } else {
                    format!("/{}", cmd)
                };
                if let Err(error) = send_slash_command(pid, &full_cmd) {
                    tracing::warn!(
                        pid,
                        cmd = %full_cmd,
                        %error,
                        "Failed to execute pattern rule command"
                    );
                }
            }
        }
        RuleAction::SendIpcCommand(cmd) => {
            tracing::debug!(cmd = %cmd, "Chat pattern rule triggered IPC command");
        }
        RuleAction::TriggerAlert(name) => {
            app.sound_alert_manager.trigger_named_alert(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_live_nav_client_status, discord_sender_is_authorized, forward_combat_soul_events,
        spawn_refresh_due,
    };
    use crate::orchestrator::Orchestrator;
    use crate::tui::app::App;
    use std::path::Path;
    use std::time::{Duration, Instant};
    use textquest_common::combat::CombatStatus;
    use textquest_common::nav::{NavStatus, PauseReason};
    use textquest_common::types::{GameState, SpawnData};
    use textquest_soul::config::{CharacterSoulConfig, SoulConfig};
    use textquest_soul::coordinator::SoulCoordinator;

    #[test]
    fn selected_client_spawn_refresh_uses_fast_interval() {
        let now = Instant::now();

        assert!(!spawn_refresh_due(
            Some(now - Duration::from_millis(249)),
            now,
            true,
        ));
        assert!(spawn_refresh_due(
            Some(now - Duration::from_millis(250)),
            now,
            true,
        ));
    }

    #[test]
    fn background_client_spawn_refresh_uses_slow_interval() {
        let now = Instant::now();

        assert!(!spawn_refresh_due(
            Some(now - Duration::from_millis(999)),
            now,
            false,
        ));
        assert!(spawn_refresh_due(
            Some(now - Duration::from_millis(1000)),
            now,
            false,
        ));
    }

    #[test]
    fn missing_spawn_refresh_timestamp_is_immediately_due() {
        let now = Instant::now();

        assert!(spawn_refresh_due(None, now, true));
        assert!(spawn_refresh_due(None, now, false));
    }

    #[test]
    fn discord_sender_auth_denies_when_allowlist_empty() {
        let app = App::new();
        assert!(!discord_sender_is_authorized(&app, "RaidLead"));
    }

    #[test]
    fn discord_sender_auth_matches_case_insensitively_with_trim() {
        let mut app = App::new();
        app.discord_command_allowed_senders
            .insert("raidlead".to_string());
        assert!(discord_sender_is_authorized(&app, "  RaidLead  "));
    }

    #[test]
    fn live_nav_status_maps_paused_reason_into_operator_surface() {
        let status = build_live_nav_client_status(
            &NavStatus::Paused {
                reason: PauseReason::UserInput,
                waypoint_index: 1,
                waypoint_count: 3,
                distance_remaining: 25.0,
            },
            "Guild Lobby",
        );

        assert_eq!(status.destination, "Active route");
        assert_eq!(status.route_state, "Route paused");
        assert_eq!(
            status.recovery_state.as_deref(),
            Some("Waiting for manual movement to stop")
        );
        assert_eq!(
            status.blockers,
            vec![String::from(
                "Movement paused because local keyboard input is active."
            )]
        );
    }

    #[test]
    fn live_nav_status_maps_following_and_stuck_states() {
        let follow = build_live_nav_client_status(
            &NavStatus::Following {
                leader_name: String::from("Raidlead"),
                distance_to_anchor: 18.0,
                returning: true,
            },
            "poknowledge",
        );
        assert_eq!(follow.destination, "Raidlead");
        assert_eq!(follow.route_state, "Following Raidlead");
        assert_eq!(
            follow.blockers,
            vec![String::from("Returning to follow anchor for Raidlead.")]
        );

        let stuck = build_live_nav_client_status(
            &NavStatus::Stuck {
                recovery_attempt: 2,
            },
            "greatdivide",
        );
        assert_eq!(stuck.route_state, "Recovery route");
        assert_eq!(
            stuck.recovery_state.as_deref(),
            Some("Trying alternate line (attempt 2)")
        );
        assert_eq!(
            stuck.blockers,
            vec![String::from(
                "Movement validation reported no progress; recovery attempt 2 is active in \
                 greatdivide."
            )]
        );
    }

    fn make_soul_game_state(client_id: u32, name: &str, combat_status: CombatStatus) -> GameState {
        GameState {
            client_id,
            local_player: Some(SpawnData {
                spawn_id: client_id,
                name: name.to_string(),
                displayed_name: name.to_string(),
                level: 60,
                hp_current: 100,
                hp_max: 100,
                mana_current: 100,
                mana_max: 100,
                endurance_current: 100,
                endurance_max: 100,
                ..SpawnData::default()
            }),
            target: Some(SpawnData {
                spawn_id: 42,
                name: "a goblin".to_string(),
                displayed_name: "a goblin".to_string(),
                spawn_type: 1,
                hp_current: 75,
                hp_max: 100,
                ..SpawnData::default()
            }),
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status,
            zone_short_name: "crushbone".to_string(),
            zone_long_name: "Crushbone".to_string(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
            is_zone_changing: false,
        }
    }

    #[test]
    fn forward_combat_soul_events_updates_registered_character_mood() {
        let mut app = App::new();
        let mut soul_config = SoulConfig::default();
        soul_config.enabled = true;
        soul_config.character.push(CharacterSoulConfig {
            name: "BattleMage".to_string(),
            traits: textquest_common::soul::PersonalityTraits {
                battle_hunger: 0.95,
                ..Default::default()
            },
            speech: Default::default(),
            edginess: None,
            backstory: String::new(),
            quirks: Vec::new(),
        });
        app.soul_coordinator =
            Some(SoulCoordinator::new(soul_config, Path::new(":memory:")).unwrap());

        let mut orchestrator = Orchestrator::new();
        orchestrator.combat.set_main_tank(1);
        orchestrator.combat.start_camp();

        let mut states = HashMap::new();
        states.insert(
            1,
            make_soul_game_state(1, "BattleMage", CombatStatus::Engaging { target_id: 42 }),
        );
        orchestrator.game_states = states.clone();
        orchestrator.combat.tick(&states);
        orchestrator.combat.drain_soul_events();

        let mut idle_states = HashMap::new();
        idle_states.insert(1, make_soul_game_state(1, "BattleMage", CombatStatus::Idle));
        orchestrator.game_states = idle_states.clone();
        orchestrator.combat.tick(&idle_states);

        forward_combat_soul_events(&mut app, &mut orchestrator);

        let mood = app
            .soul_coordinator
            .as_ref()
            .and_then(|coordinator| coordinator.mood(1));
        assert_eq!(mood, Some(textquest_common::soul::MoodState::Excited));
    }
}
