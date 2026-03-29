use anyhow::Result;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use std::collections::HashMap;
use std::io;
use std::time::{Duration, Instant};

use super::app::App;
use super::event::handle_events;
use super::ui::draw;
use crate::orchestrator::Orchestrator;

/// Soul Engine tick interval (5 seconds).
const SOUL_TICK_INTERVAL: Duration = Duration::from_secs(5);

/// How often to scan for new EQ processes (10 seconds).
const PROCESS_SCAN_INTERVAL: Duration = Duration::from_secs(10);

/// How often to poll log watchers (2 seconds).
const LOG_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Camp loop tick interval (1 second).
const CAMP_TICK_INTERVAL: Duration = Duration::from_secs(1);

/// Initialize crossterm, run the TUI loop, and clean up on exit.
pub fn run_tui(mut app: App, mut orchestrator: Orchestrator) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app, &mut orchestrator);

    // Restore terminal — always runs even if loop panicked
    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

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

    while app.running {
        // Draw the UI
        terminal.draw(|frame| draw(frame, app))?;

        // Handle keyboard events (with a short poll timeout so we stay responsive)
        let poll_timeout = Duration::from_millis(50);
        handle_events(app, poll_timeout, orchestrator)?;

        // Periodic scan for new/lost EQ processes
        if last_process_scan.elapsed() >= PROCESS_SCAN_INTERVAL {
            scan_for_clients(app);
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
            refresh_eq_data(app);
            app.tick_count += 1;
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
    use crate::process::memory::{find_processes_by_name, ProcessHandle};

    let pids = match find_processes_by_name("eqgame.exe") {
        Ok(p) => p,
        Err(_) => return,
    };

    // Track which PIDs we already have
    let existing_pids: std::collections::HashSet<u32> = app.clients.iter().map(|c| c.pid).collect();

    // Remove clients whose process has gone away
    app.clients.retain(|c| pids.contains(&c.pid));

    // Add newly discovered processes
    for &pid in &pids {
        if existing_pids.contains(&pid) {
            continue;
        }

        if let Ok(proc) = ProcessHandle::open(pid) {
            if let Ok(base) = crate::get_module_base(&proc) {
                let mut client = ClientState::new(pid, base);

                // Try to extract zone name from window title
                if let Ok(windows) = crate::process::window::find_windows_by_title("EverQuest") {
                    for w in &windows {
                        if w.pid == pid {
                            let (char_name, zone) = parse_title_fields(&w.title);
                            if !char_name.is_empty() {
                                client.character_name = char_name;
                            }
                            client.zone_name = if zone.is_empty() { String::from("Unknown") } else { zone };
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
        app.status_message = String::from("No EQ process found — scanning...");
    }

    // Sync legacy fields
    app.sync_from_selected_client();
}

/// Parse character name and zone name from the DLL-renamed window title.
/// Format: "[DMFT] EQ - CharName (ZoneName)" or "[DMFT] EQ - CharName"
/// Falls back to the old EQ format: "EverQuest - Character - Zone"
#[cfg(windows)]
fn parse_title_fields(title: &str) -> (String, String) {
    // Strip optional "[DMFT] " prefix before parsing.
    let title = title.strip_prefix("[DMFT] ").unwrap_or(title);

    // New DLL format: "EQ - CharName (ZoneName)"
    if title.starts_with("EQ - ") {
        let rest = &title[5..]; // after "EQ - "
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
fn refresh_eq_data(app: &mut App) {
    #[cfg(windows)]
    {
        refresh_eq_data_live(app);
    }

    #[cfg(not(windows))]
    {
        // On macOS/Linux, load demo data so the TUI is testable
        if app.clients.is_empty() {
            load_demo_data(app);
        }
    }
}

/// Live EQ memory refresh — only compiles on Windows.
/// Refreshes ALL attached clients.
#[cfg(windows)]
fn refresh_eq_data_live(app: &mut App) {
    use crate::eq;
    use crate::process::memory::ProcessHandle;

    for client in app.clients.iter_mut() {
        let proc = match ProcessHandle::open(client.pid) {
            Ok(p) => p,
            Err(e) => {
                client.client_status = format!("Lost connection: {}", e);
                continue;
            }
        };

        // Read local player
        match eq::spawn::read_local_player(&proc, client.eq_base) {
            Ok(player) => client.local_player = Some(player),
            Err(e) => client.client_status = format!("Player read error: {}", e),
        }

        // Read target
        match eq::spawn::read_target(&proc, client.eq_base) {
            Ok(target) => client.target = target,
            Err(e) => client.client_status = format!("Target read error: {}", e),
        }

        // Read spawn list
        match eq::spawn::read_all_spawns(&proc, client.eq_base, 200) {
            Ok(spawns) => client.spawns = spawns,
            Err(e) => client.client_status = format!("Spawn read error: {}", e),
        }

        // Refresh zone name from window title
        if let Ok(windows) = crate::process::window::find_windows_by_title("EverQuest") {
            for w in &windows {
                if w.pid == client.pid {
                    let (char_name, zone) = parse_title_fields(&w.title);
                            if !char_name.is_empty() {
                                client.character_name = char_name;
                            }
                            client.zone_name = if zone.is_empty() { String::from("Unknown") } else { zone };
                    break;
                }
            }
        }
    }

    // Sync selected client data to legacy fields
    app.sync_from_selected_client();
}

/// Demo data for testing the TUI on macOS without a live EQ process.
#[cfg(not(windows))]
fn load_demo_data(app: &mut App) {
    use super::app::ClientState;
    use crate::eq::structs::{SpawnInfo, SpawnType, StandState};

    app.status_message = String::from("DEMO MODE — no EQ process");

    // Create multiple demo clients to showcase multi-client TUI
    let demo_clients = vec![
        (
            "Frostreaver",
            1,
            "WAR",
            60,
            8500,
            10000,
            0,
            0,
            StandState::Standing,
            "Permafrost",
        ),
        (
            "Iceweaver",
            14,
            "ENC",
            60,
            3200,
            4000,
            3800,
            4000,
            StandState::Standing,
            "Permafrost",
        ),
        (
            "Coldchain",
            2,
            "CLR",
            60,
            5500,
            6000,
            3500,
            4500,
            StandState::Sitting,
            "Permafrost",
        ),
        (
            "Glacialmend",
            10,
            "SHM",
            58,
            4800,
            5200,
            2800,
            3600,
            StandState::Standing,
            "Eastern Wastes",
        ),
        (
            "Frostbolt",
            12,
            "WIZ",
            59,
            3000,
            3800,
            4000,
            5000,
            StandState::Standing,
            "Eastern Wastes",
        ),
        (
            "Tundrastalker",
            4,
            "RNG",
            57,
            5000,
            5800,
            2000,
            2500,
            StandState::Ducking,
            "Eastern Wastes",
        ),
    ];

    for (i, (name, class_id, _class_str, level, hp, hp_max, mana, mana_max, stand, zone)) in
        demo_clients.iter().enumerate()
    {
        let mut client = ClientState::new(1000 + i as u32, 0x140000000);
        client.zone_name = zone.to_string();
        client.local_player = Some(SpawnInfo {
            name: name.to_string(),
            displayed_name: name.to_string(),
            lastname: String::new(),
            level: *level,
            class_id: *class_id,
            class: crate::eq::structs::EqClass::from_id(*class_id),
            stand_state: *stand,
            spawn_type: SpawnType::Player,
            hp_current: *hp,
            hp_max: *hp_max,
            mana_current: *mana,
            mana_max: *mana_max,
            endurance_current: 150,
            endurance_max: 200,
            x: 1234.5 + (i as f32 * 100.0),
            y: -567.8 + (i as f32 * 50.0),
            z: 12.0,
            heading: 128.0,
            spawn_id: i as u32 + 1,
            is_gm: false,
        });
        client.client_status = format!("Demo client: {}", name);
        app.clients.push(client);
    }

    // Build spawns for first client (Frostreaver in Permafrost)
    let demo_spawns = vec![
        (
            "Frostreaver",
            60,
            1,
            SpawnType::Player,
            8500,
            10000,
            StandState::Standing,
        ),
        (
            "Iceweaver",
            60,
            14,
            SpawnType::Player,
            3200,
            4000,
            StandState::Standing,
        ),
        (
            "Coldchain",
            60,
            2,
            SpawnType::Player,
            5500,
            6000,
            StandState::Sitting,
        ),
        (
            "a frost giant",
            55,
            0,
            SpawnType::Npc,
            12000,
            15000,
            StandState::Standing,
        ),
        (
            "a snow griffin",
            52,
            0,
            SpawnType::Npc,
            8000,
            8000,
            StandState::Standing,
        ),
        (
            "Lady Vox",
            60,
            0,
            SpawnType::Npc,
            250000,
            320000,
            StandState::Standing,
        ),
        (
            "a frost giant's corpse",
            55,
            0,
            SpawnType::Corpse,
            0,
            15000,
            StandState::Dead,
        ),
        (
            "Trader Mikhail",
            45,
            0,
            SpawnType::Npc,
            5000,
            5000,
            StandState::Standing,
        ),
        (
            "a dire wolf",
            48,
            0,
            SpawnType::Npc,
            6000,
            7200,
            StandState::Standing,
        ),
        (
            "Velketor",
            60,
            0,
            SpawnType::Npc,
            180000,
            200000,
            StandState::Standing,
        ),
    ];

    let spawns: Vec<SpawnInfo> = demo_spawns
        .into_iter()
        .enumerate()
        .map(
            |(i, (name, level, class, stype, hp, hp_max, stand))| SpawnInfo {
                name: name.to_string(),
                displayed_name: name.to_string(),
                lastname: String::new(),
                level,
                class_id: class,
                class: crate::eq::structs::EqClass::from_id(class),
                stand_state: stand,
                spawn_type: stype,
                hp_current: hp,
                hp_max,
                mana_current: if class > 0 { 3000 } else { 0 },
                mana_max: if class > 0 { 4000 } else { 0 },
                endurance_current: 150,
                endurance_max: 200,
                x: 1234.5 + (i as f32 * 10.0),
                y: -567.8 + (i as f32 * 5.0),
                z: 12.0,
                heading: 0.0,
                spawn_id: i as u32 + 1,
                is_gm: false,
            },
        )
        .collect();

    // Assign spawns to the first client
    if !app.clients.is_empty() {
        app.clients[0].spawns = spawns;
    }

    // Sync selected client to legacy fields
    app.sync_from_selected_client();

    // Load zone map for the first client's zone
    if !app.clients.is_empty() {
        let zone = zone_to_short_name(&app.clients[0].zone_name);
        app.load_zone_map(&zone);
    }
}

/// Convert a zone display name (long name from zoneHeader) to its EQ short name
/// for Brewall map file lookup. Handles both display names ("West Freeport") and
/// short names that are already correct ("freportw").
fn zone_to_short_name(zone_name: &str) -> String {
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
        "highpass hold" | "high keep" => "highkeep".to_string(),
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
    let coordinator = match app.soul_coordinator.as_mut() {
        Some(c) => c,
        None => return,
    };

    // Build game states from current app data
    // In the full orchestrator, this comes from shared memory per client.
    // For now, use an empty map (no clients registered yet = no commands generated).
    let states: HashMap<dmft_common::types::ClientId, dmft_common::types::GameState> =
        HashMap::new();

    let commands = coordinator.tick(&states);

    if !commands.is_empty() {
        tracing::debug!(count = commands.len(), "Soul Engine generated commands");
        // TODO: dispatch commands to clients via IPC pipe
        // For now, commands are generated but not sent (no live clients in TUI demo mode)
    }

    app.soul_tick_counter += 1;
}

/// Poll all log watchers for new events and merge into the aggregate loot database.
fn poll_log_watchers(app: &mut App) {
    for watcher in &mut app.log_watchers {
        let events = watcher.poll();
        for event in &events {
            app.loot_database.record(event);
        }
    }
}
