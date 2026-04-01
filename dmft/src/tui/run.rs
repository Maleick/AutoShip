use anyhow::Result;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::prelude::CrosstermBackend;
use std::collections::HashMap;
use std::io;
use std::time::{Duration, Instant};

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
///
/// # Errors
///
/// Returns an error if the operation fails.
pub fn run_tui(mut app: App, mut orchestrator: Orchestrator) -> Result<()> {
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

    let pids = match find_processes_by_name("eqgame.exe") {
        Ok(p) => p,
        Err(_) => return,
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
        app.status_message = String::from("No EQ process found — scanning...");
    }

    // Sync legacy fields and reload map for current client
    app.sync_from_selected_client();
    app.reload_map_for_selected_client();
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
fn refresh_eq_data(app: &mut App) {
    #[cfg(windows)]
    {
        refresh_eq_data_live(app);
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
                client.client_status = format!("Lost connection: {e}");
                continue;
            }
        };

        // Read local player
        match eq::spawn::read_local_player(&proc, client.eq_base) {
            Ok(player) => client.local_player = Some(player),
            Err(e) => client.client_status = format!("Player read error: {e}"),
        }

        // Read target
        match eq::spawn::read_target(&proc, client.eq_base) {
            Ok(target) => client.target = target,
            Err(e) => client.client_status = format!("Target read error: {e}"),
        }

        // Read spawn list
        match eq::spawn::read_all_spawns(&proc, client.eq_base, 200) {
            Ok(spawns) => client.spawns = spawns,
            Err(e) => client.client_status = format!("Spawn read error: {e}"),
        }

        // Read zone name from memory (preferred) or fall back to window title
        match eq::spawn::read_zone_name(&proc, client.eq_base) {
            Ok(zone) => client.zone_name = zone,
            Err(_) => {
                // Fallback: parse from window title
                if let Ok(windows) = crate::process::window::find_windows_by_title("EverQuest") {
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

        // Read group info from memory
        match eq::spawn::read_group_info(&proc, client.eq_base) {
            Ok(group) => client.group_info = group,
            Err(e) => {
                tracing::trace!(pid = client.pid, error = %e, "Failed to read group info");
            }
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

    app.status_message = String::from("DEMO MODE — no EQ process");

    // 18 demo clients across 3 groups, covering all 16 EQ classes.
    // Names use trailing digits (e.g., "Frostreaver01") so they match group slots
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
            "Frostreaver01",
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

    for (i, &(name, class_id, level, hp, hp_max, mana, mana_max, ref stand, zone, race_id)) in
        demo_clients.iter().enumerate()
    {
        let mut client = ClientState::new(1000 + i as u32, 0x0001_4000_0000);
        client.zone_name = zone.to_string();
        let (x, y, z, heading) = super::demo_data::demo_player_position(zone, i).unwrap_or((
            1234.5 + (i as f32 * 100.0),
            -567.8 + (i as f32 * 50.0),
            12.0,
            128.0,
        ));
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
        app.clients.push(client);
    }

    // ── Spawns for each zone ─────────────────────────────────────────────────
    // Each group's clients share a spawn list appropriate to their zone.
    // Spawn definitions live in demo_data.rs to keep this function focused.
    for client in &mut app.clients {
        let spawns = super::demo_data::demo_spawns_for_zone(&client.zone_name);
        if !spawns.is_empty() {
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
                ("Frostreaver01", &group1_members)
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

    // Load zone map for the selected client's zone
    app.reload_map_for_selected_client();
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
