use anyhow::Result;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::prelude::CrosstermBackend;
use std::collections::{HashMap, HashSet};
use std::io;
use std::time::{Duration, Instant};

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

use super::app::App;
use super::app::{ChChainStatus, NavClientStatus};
use super::cast::{CastDisplay, short_cast_label};
use super::event::handle_events;
use super::live_cast_capture::{LIVE_CAST_CAPTURE_ENV, live_cast_capture_enabled};
use super::ui::ch_chain::{CastState as ChPanelCastState, ChainCleric};
use super::ui::draw;
use crate::eq::structs::SpawnInfo;
use crate::orchestrator::Orchestrator;

#[cfg(windows)]
use super::live_cast_capture::{
    LiveCastCaptureSnapshot, diff_live_cast_capture, log_live_cast_capture_event,
};

/// Soul Engine tick interval (5 seconds).
const SOUL_TICK_INTERVAL: Duration = Duration::from_secs(5);

/// How often to scan for new EQ processes (10 seconds).
const PROCESS_SCAN_INTERVAL: Duration = Duration::from_secs(10);

/// How often to poll log watchers (2 seconds).
const LOG_POLL_INTERVAL: Duration = Duration::from_secs(2);

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
    std::env::var(dmft_common::ipc::PERF_TRACE_ENV)
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
            target: "dmft::cast_capture",
            env = LIVE_CAST_CAPTURE_ENV,
            log_path = "logs/dmft.log",
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
    let mut process_handles: HashMap<u32, crate::process::memory::ProcessHandle> = HashMap::new();

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

            // Sync orchestrator's client list from app
            orchestrator.client_pids = app.clients.iter().map(|c| c.pid).collect();
            orchestrator.client_names = app
                .clients
                .iter()
                .map(|c| (c.pid, c.character_name.clone()))
                .collect();
            last_process_scan = Instant::now();
        }

        // Periodic data refresh from EQ process
        if last_refresh.elapsed() >= refresh_interval {
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
            last_refresh = Instant::now();
        }

        // Camp loop tick (every 1 second)
        if last_camp_tick.elapsed() >= CAMP_TICK_INTERVAL {
            let dispatched = orchestrator.tick();
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
            tick_soul_engine(app);
            last_soul_tick = Instant::now();
        }
    }

    Ok(())
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
            } else if let Ok(windows) = crate::process::window::find_windows_by_title("EverQuest") {
                for w in &windows {
                    if w.pid == pid {
                        let (char_name, zone) = parse_title_fields(&w.title);
                        if !char_name.is_empty() {
                            client.character_name = char_name;
                        }
                        client.zone_name = if zone.is_empty() {
                            String::from("Unknown")
                        } else {
                            zone
                        };
                        break;
                    }
                }
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
                        route_state: match &nav.status {
                            dmft_common::nav::NavStatus::Moving { .. } => {
                                String::from("Regroup route")
                            }
                            dmft_common::nav::NavStatus::Paused { .. } => {
                                String::from("Route paused")
                            }
                            dmft_common::nav::NavStatus::Stuck { .. } => {
                                String::from("Recovery route")
                            }
                            dmft_common::nav::NavStatus::Arrived => String::from("Route complete"),
                            dmft_common::nav::NavStatus::Idle => String::from("Standing by"),
                            dmft_common::nav::NavStatus::Sticking { target_id, .. } => {
                                format!("Sticking to #{target_id}")
                            }
                        },
                        recovery_state: match &nav.status {
                            dmft_common::nav::NavStatus::Stuck { recovery_attempt } => Some(
                                format!("Trying alternate line (attempt {})", recovery_attempt),
                            ),
                            dmft_common::nav::NavStatus::Paused { .. } => {
                                Some(String::from("Waiting for target stability"))
                            }
                            _ => None,
                        },
                        blockers: match &nav.status {
                            dmft_common::nav::NavStatus::Stuck { .. } => vec![format!(
                                "Path to {} is obstructed; waiting for recovery movement.",
                                nav.destination
                            )],
                            dmft_common::nav::NavStatus::Paused { .. } => vec![format!(
                                "Navigation paused near {}; waiting for stable target.",
                                nav.destination
                            )],
                            _ => Vec::new(),
                        },
                        is_demo_scripted: true,
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
/// Format: "[DMFT] EQ - `CharName` (`ZoneName`)" or "[DMFT] EQ - `CharName`"
/// Falls back to the old EQ format: "`EverQuest` - Character - Zone"
#[cfg(windows)]
fn parse_title_fields(title: &str) -> (String, String) {
    // Strip optional "[DMFT] " prefix before parsing.
    let title = title.strip_prefix("[DMFT] ").unwrap_or(title);

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

/// Refresh live EQ data. On non-Windows or when not attached, loads demo data.
fn refresh_eq_data(
    app: &mut App,
    _process_handles: &mut HashMap<u32, crate::process::memory::ProcessHandle>,
) {
    #[cfg(windows)]
    {
        refresh_eq_data_live(app, _process_handles);
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
) {
    use crate::eq;
    use crate::process::memory::ProcessHandle;

    let selected_index = app.selected_client;
    let now = Instant::now();

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

        // Read spawn list on a staged cadence: selected client stays fast, others are throttled.
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
                            target: "dmft::perf",
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

        // Read zone name on the same cadence as spawn snapshots unless the active client
        // is currently missing zone data.
        if refresh_zone {
            match eq::spawn::read_zone_name(&proc, client.eq_base) {
                Ok(zone) => client.zone_name = zone,
                Err(_) => {
                    // Fallback: parse from window title
                    if let Ok(windows) = crate::process::window::find_windows_by_title("EverQuest")
                    {
                        for w in &windows {
                            if w.pid == client.pid {
                                let (char_name, zone) = parse_title_fields(&w.title);
                                if !char_name.is_empty() {
                                    client.character_name = char_name;
                                }
                                client.zone_name = if zone.is_empty() {
                                    String::from("Unknown")
                                } else {
                                    zone
                                };
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Read group info from memory
        match eq::spawn::read_group_info(&proc, client.eq_base) {
            Ok(group) => client.group_info = group,
            Err(e) => {
                tracing::trace!(pid = client.pid, error = %e, "Failed to read group info");
            }
        }

        if !hard_read_failed {
            process_handles.insert(client.pid, proc);
        }
    }

    // Sync selected client data to legacy fields
    app.sync_from_selected_client();

    // Reload map if the selected client's zone changed (or map not yet loaded)
    app.reload_map_for_selected_client();
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
    // Format: (name, class_id, level, hp, hp_max, mana, mana_max, stand_state, zone, race_id)
    // Melee classes have mana 0. Caster/hybrid mana is class-appropriate.
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
            cast_state: None,
        });
        client.character_name = name.to_string();
        client.client_status = format!("Connected: {name}");
        client.is_demo = true;
        // Demo: assign lifecycle, launch profile, and session preset per group.
        // Two clients show non-Live states for visual demo variety.
        let lifecycle = match i {
            16 => dmft_common::types::SlotLifecycle::WaitingForLogin,
            17 => dmft_common::types::SlotLifecycle::Recovering,
            _ => dmft_common::types::SlotLifecycle::Live,
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
/// for Brewall map file lookup. Handles both display names ("West Freeport") and
/// short names that are already correct ("freportw").
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
/// Generates soul commands (idle behaviors, chat, emotes) for all registered characters.
fn tick_soul_engine(app: &mut App) {
    let Some(coordinator) = app.soul_coordinator.as_mut() else {
        return;
    };

    // Build game states from current app data
    // In the full orchestrator, this comes from shared memory per client.
    // For now, use an empty map (no clients registered yet = no commands generated).
    let states: HashMap<dmft_common::types::ClientId, dmft_common::types::GameState> =
        HashMap::new();

    let commands = coordinator.tick(&states);

    if !commands.is_empty() {
        tracing::debug!(count = commands.len(), "Soul Engine generated commands");
        // Soul commands are logged but not dispatched in TUI demo mode.
        // The Orchestrator handles IPC delivery when live clients are connected.
    }

    app.soul_tick_counter += 1;
}

/// Poll all log watchers for new events and merge into the aggregate loot database.
fn poll_log_watchers(app: &mut App) {
    use crate::eq::log_parser::LogEvent;
    for watcher in &mut app.log_watchers {
        let events = watcher.poll();
        for event in &events {
            app.loot_database.record(event);
            if let LogEvent::Chat(chat) = event {
                app.chat_events.push_back(chat.clone());
                if app.chat_events.len() > 200 {
                    app.chat_events.pop_front();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::spawn_refresh_due;
    use std::time::{Duration, Instant};

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
}
