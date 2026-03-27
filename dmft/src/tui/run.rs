use anyhow::Result;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::app::App;
use super::event::handle_events;
use super::ui::draw;

/// Soul Engine tick interval (5 seconds).
const SOUL_TICK_INTERVAL: Duration = Duration::from_secs(5);

/// Initialize crossterm, run the TUI loop, and clean up on exit.
pub fn run_tui(mut app: App) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app);

    // Restore terminal — always runs even if loop panicked
    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    let refresh_interval = Duration::from_millis(app.refresh_rate_ms);
    let mut last_refresh = Instant::now();
    let mut last_soul_tick = Instant::now();

    while app.running {
        // Draw the UI
        terminal.draw(|frame| draw(frame, app))?;

        // Handle keyboard events (with a short poll timeout so we stay responsive)
        let poll_timeout = Duration::from_millis(50);
        handle_events(app, poll_timeout)?;

        // Periodic data refresh from EQ process
        if last_refresh.elapsed() >= refresh_interval {
            refresh_eq_data(app);
            app.tick_count += 1;
            last_refresh = Instant::now();
        }

        // Soul Engine tick (every 5 seconds)
        if last_soul_tick.elapsed() >= SOUL_TICK_INTERVAL {
            tick_soul_engine(app);
            last_soul_tick = Instant::now();
        }
    }

    Ok(())
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
        if app.spawns.is_empty() {
            load_demo_data(app);
        }
    }
}

/// Live EQ memory refresh — only compiles on Windows.
#[cfg(windows)]
fn refresh_eq_data_live(app: &mut App) {
    use crate::eq;
    use crate::process::memory::ProcessHandle;

    let pid = match app.attached_pid {
        Some(pid) => pid,
        None => return,
    };

    let proc = match ProcessHandle::open(pid) {
        Ok(p) => p,
        Err(e) => {
            app.status_message = format!("Lost connection: {}", e);
            app.attached_pid = None;
            return;
        }
    };

    // Read local player
    match eq::spawn::read_local_player(&proc, app.eq_base) {
        Ok(player) => app.local_player = Some(player),
        Err(e) => app.status_message = format!("Player read error: {}", e),
    }

    // Read target
    match eq::spawn::read_target(&proc, app.eq_base) {
        Ok(target) => app.target = target,
        Err(e) => app.status_message = format!("Target read error: {}", e),
    }

    // Read spawn list
    match eq::spawn::read_all_spawns(&proc, app.eq_base, 200) {
        Ok(spawns) => app.spawns = spawns,
        Err(e) => app.status_message = format!("Spawn read error: {}", e),
    }
}

/// Demo data for testing the TUI on macOS without a live EQ process.
#[cfg(not(windows))]
fn load_demo_data(app: &mut App) {
    use crate::eq::structs::{SpawnInfo, SpawnType};

    app.status_message = String::from("DEMO MODE — no EQ process");

    app.local_player = Some(SpawnInfo {
        name: String::from("Frostreaver"),
        displayed_name: String::from("Frostreaver"),
        lastname: String::new(),
        level: 60,
        class_id: 1,
        class: Some(crate::eq::structs::EqClass::Warrior),
        spawn_type: SpawnType::Player,
        hp_current: 8500,
        hp_max: 10000,
        mana_current: 0,
        mana_max: 0,
        endurance_current: 150,
        endurance_max: 200,
        x: 1234.5,
        y: -567.8,
        z: 12.0,
        heading: 128.0,
        spawn_id: 1,
    });

    let demo_spawns = vec![
        ("Frostreaver", 60, 1, SpawnType::Player, 8500, 10000),
        ("Iceweaver", 60, 14, SpawnType::Player, 3200, 4000),
        ("Coldchain", 60, 2, SpawnType::Player, 5500, 6000),
        ("a frost giant", 55, 0, SpawnType::Npc, 12000, 15000),
        ("a snow griffin", 52, 0, SpawnType::Npc, 8000, 8000),
        ("Lady Vox", 60, 0, SpawnType::Npc, 250000, 320000),
        ("a frost giant's corpse", 55, 0, SpawnType::Corpse, 0, 15000),
        ("Trader Mikhail", 45, 0, SpawnType::Npc, 5000, 5000),
        ("a dire wolf", 48, 0, SpawnType::Npc, 6000, 7200),
        ("Velketor", 60, 0, SpawnType::Npc, 180000, 200000),
    ];

    app.spawns = demo_spawns
        .into_iter()
        .enumerate()
        .map(|(i, (name, level, class, stype, hp, hp_max))| SpawnInfo {
            name: name.to_string(),
            displayed_name: name.to_string(),
            lastname: String::new(),
            level,
            class_id: class,
            class: crate::eq::structs::EqClass::from_id(class),
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
        })
        .collect();
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
