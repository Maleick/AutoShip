//! Game loop hook -- intercepts `CEverQuest::MainLoop`.
//!
//! Uses hardware breakpoint hooking (DR0) instead of detour/trampoline.
//! The VEH handler fires when EQ hits the breakpoint, runs our logic, then
//! resumes the original function with RF set so the BP doesn't re-trigger.

use super::hwbp::{self, HwbpSlot};

const GAME_LOOP_SLOT: HwbpSlot = HwbpSlot::Dr0;

/// HWBP callback for the game loop hook.
///
/// Lives in `.tq` section so it remains executable when `.text` is encrypted.
/// wake()/sleep() are handled by the VEH handler that wraps all callbacks,
/// so this function runs with `.text` already decrypted.
#[cfg_attr(windows, unsafe(link_section = ".tq"))]
fn game_loop_callback(_exception_info: *mut ()) -> bool {
    on_game_tick();
    true
}

pub fn install(main_loop_addr: usize) -> Result<(), Box<dyn std::error::Error>> {
    hwbp::register(GAME_LOOP_SLOT, main_loop_addr, game_loop_callback)?;
    tracing::info!(
        addr = format!("{:#x}", main_loop_addr),
        "Game loop HWBP hook installed (DR0)"
    );
    Ok(())
}

pub fn remove() {
    if hwbp::is_active(GAME_LOOP_SLOT) {
        if let Err(e) = hwbp::unregister(GAME_LOOP_SLOT) {
            tracing::warn!("Failed to remove game loop HWBP: {}", e);
        }
    }
    tracing::info!("Game loop hook removed");
}

/// Track whether this window is in the foreground for render skipping.
/// When false, we can tell EQ to skip 3D rendering (near-zero GPU for
/// background clients). Game logic still runs at full speed.
static WINDOW_IS_FOREGROUND: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

/// Track tick count for throttling background checks.
static TICK_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Pending login button click — set by IPC thread, executed on game loop
/// thread. Contains the `CXWnd`* address of the button to click, or 0 if none
/// pending.
static PENDING_BUTTON_CLICK: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Pending Enter World sequence — set by IPC thread, executed on game loop
/// thread. Stage 0 = idle, 1 = `SelectCharacter` pending, 2 = waiting, 3 =
/// `EnterWorld` pending.
static PENDING_ENTER_WORLD_WND: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
/// The rebased `EnterWorld` function address.
static PENDING_ENTER_WORLD_FN: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
/// The rebased `SelectCharacter` function address.
static PENDING_SELECT_CHAR_FN: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
/// Enter World sequence stage (0=idle, 1=select, 2=wait, 3=enter).
static ENTER_WORLD_STAGE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// Tick at which to advance from stage 2→3 (wait before `EnterWorld`).
static ENTER_WORLD_WAIT_UNTIL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Retry counter for stage 3 rescan (abort after 150 ticks / ~5 seconds).
static ENTER_WORLD_RETRIES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// Character name to select (set by IPC thread, read by game loop).
static PENDING_CHAR_NAME: std::sync::OnceLock<std::sync::Mutex<String>> =
    std::sync::OnceLock::new();

/// Cached nearby spawns used by the stick-to-target engine.
///
/// Updated every 30 ticks in `read_and_publish_state`; consumed every tick
/// by `crate::nav::tick()` for target resolution (`hold` / `id` / `always`).
static CACHED_NEARBY_FOR_STICK: std::sync::Mutex<Vec<textquest_common::types::SpawnData>> =
    std::sync::Mutex::new(Vec::new());

/// Previous nearby-spawn snapshot used for delta detection and spawn event
/// emission.
static PREV_NEARBY_SPAWNS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<u32, String>>,
> = std::sync::OnceLock::new();

/// Set a button widget address to be clicked on the next game loop tick.
/// Called from the IPC thread after writing credentials.
pub fn queue_button_click(button_wnd: usize) {
    PENDING_BUTTON_CLICK.store(button_wnd, std::sync::atomic::Ordering::Release);
}

/// Queue a `SelectCharacter` → EnterWorld() sequence on the game loop thread.
/// Called from the IPC thread during Phase 3 of login chain.
/// The game loop will: (1) find character index by name, (2) call
/// SelectCharacter(index), (3) wait ~90 ticks (~3s), (4) call EnterWorld(). All
/// calls happen on the game loop thread.
pub fn queue_enter_world(char_list_wnd: usize, enter_world_fn: usize, character_name: String) {
    // Also resolve SelectCharacter address
    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    let select_fn =
        textquest_common::offsets::rebase(textquest_common::offsets::SELECT_CHARACTER, eq_base)
            .unwrap_or(0);

    // Store the character name for the game loop to look up.
    let name_lock = PENDING_CHAR_NAME.get_or_init(|| std::sync::Mutex::new(String::new()));
    if let Ok(mut name) = name_lock.lock() {
        *name = character_name;
    }

    // Store function addresses and window handle first (Relaxed is sufficient),
    // then store the stage flag last with Release ordering as the "commit" signal.
    // The reader's Acquire load of ENTER_WORLD_STAGE establishes happens-before
    // for all prior stores, guaranteeing the addresses are visible.
    PENDING_SELECT_CHAR_FN.store(select_fn, std::sync::atomic::Ordering::Relaxed);
    PENDING_ENTER_WORLD_FN.store(enter_world_fn, std::sync::atomic::Ordering::Relaxed);
    PENDING_ENTER_WORLD_WND.store(char_list_wnd, std::sync::atomic::Ordering::Relaxed);
    ENTER_WORLD_STAGE.store(1, std::sync::atomic::Ordering::Release);
}

/// Re-scan `CXWndManager` for `CCharacterListWnd` by `SidlText`.
/// Used in Stage 3 to validate the pointer is still valid before calling
/// `EnterWorld`, and by the login FSM to find the window for initial character
/// selection.
#[cfg(windows)]
pub fn rescan_char_list_wnd() -> Option<usize> {
    use textquest_common::offsets::eqgame as eqg;

    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    let mgr_ptr_addr =
        textquest_common::offsets::rebase(textquest_common::offsets::PINST_CXWND_MANAGER, eq_base)?;

    // SAFETY: mgr_ptr_addr was rebased from PINST_CXWND_MANAGER, a known-valid
    // global pointer in eqgame.exe. Each dereference follows EQ's CXWndManager
    // layout (windows array pointer + count). Null checks and count cap (2000)
    // guard against corrupted data. read_cxstr performs its own pointer validation.
    // Must be called from the game loop thread where CXWndManager is stable.
    unsafe {
        let mgr = *(mgr_ptr_addr as *const usize);
        if mgr == 0 {
            return None;
        }

        let array_ptr = *((mgr + eqg::CXWNDMGR_WINDOWS_ARRAY) as *const usize);
        let count = *((mgr + eqg::CXWNDMGR_WINDOWS_COUNT) as *const u32);
        if array_ptr == 0 || count == 0 || count > 2000 {
            return None;
        }

        for i in 0..count as usize {
            let wnd_ptr = *((array_ptr + i * 8) as *const usize);
            if wnd_ptr == 0 {
                continue;
            }

            if let Some(sidl_text) =
                crate::eq::widgets::read_cxstr(wnd_ptr + eqg::CSIDL_SCREEN_WND_SIDL_TEXT)
            {
                if sidl_text == "CharacterListWnd" {
                    return Some(wnd_ptr);
                }
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub fn rescan_char_list_wnd() -> Option<usize> {
    None
}

/// Find the index of a character by name in the `Character_List` `CListWnd`.
///
/// Walks the `CCharacterListWnd`'s child windows to find "`Character_List`" (a
/// `CListWnd`), then reads each row's column 2 (character name) for a
/// case-insensitive match. Returns the matched index, or 0 as fallback if the
/// name is empty or not found.
#[cfg(windows)]
fn find_character_index(char_list_wnd: usize, character_name: &str) -> i32 {
    const MAX_CHARACTER_LIST_SCAN_ROWS: usize = 64;

    if character_name.is_empty() {
        tracing::info!("Character name empty — defaulting to index 0");
        return 0;
    }

    unsafe {
        // Find the "Character_List" child (CListWnd) inside CCharacterListWnd
        let Some(list_wnd) =
            crate::eq::widgets::find_child_by_sidl_text(char_list_wnd, "Character_List")
        else {
            tracing::warn!("Character_List child not found — defaulting to index 0");
            return 0;
        };

        let row_count = crate::eq::widgets::list_row_count(list_wnd);
        let bounded_row_count = row_count.min(MAX_CHARACTER_LIST_SCAN_ROWS);
        if row_count > bounded_row_count {
            tracing::warn!(
                row_count,
                bounded_row_count,
                "Character_List row count exceeds scan cap; limiting traversal"
            );
        }
        tracing::info!(
            list_wnd = format!("{:#x}", list_wnd),
            row_count,
            bounded_row_count,
            target = character_name,
            "Searching Character_List for character"
        );

        // Column 2 is the character name (MQ2 convention)
        for i in 0..bounded_row_count {
            if let Some(name) = crate::eq::widgets::read_list_item_text(list_wnd, i, 2) {
                tracing::info!(row = i, name = %name, "Character_List row");
                if name.eq_ignore_ascii_case(character_name) {
                    tracing::info!(index = i, name = %name, "Found matching character!");
                    return i as i32;
                }
            }
        }

        tracing::warn!(
            target = character_name,
            row_count,
            "Character not found in list — defaulting to index 0"
        );
    }
    0
}

#[cfg(not(windows))]
fn find_character_index(_char_list_wnd: usize, _character_name: &str) -> i32 {
    0
}

// ─── Command Jitter Queue ───
// Commands are not executed immediately — they sit in a pending queue
// with a random delay of 1-10 ticks to avoid frame-perfect timing patterns.

use std::sync::Mutex;

struct PendingCommand {
    command: textquest_common::ipc::Command,
    execute_at_tick: u64,
}

static PENDING_COMMANDS: Mutex<Vec<PendingCommand>> = Mutex::new(Vec::new());
static JITTER_RNG: Mutex<Option<textquest_common::nav::Xorshift32>> = Mutex::new(None);
static ACTIVE_BANDOLIER_SET: Mutex<Option<String>> = Mutex::new(None);
static PENDING_BANDOLIER_RESTORE: Mutex<Option<PendingBandolierRestore>> = Mutex::new(None);

const BANDOLIER_CAST_TIMEOUT_TICKS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingBandolierRestore {
    requested_set: String,
    restore_to: Option<String>,
    deadline_tick: u64,
    saw_casting: bool,
}

/// Initialize the jitter RNG with a seed derived from system time.
/// Using time instead of PID avoids predictable sequences since PIDs
/// are sequential and easily enumerated by anti-cheat.
pub fn init_jitter_rng() {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u32;
    let seed = if seed == 0 { 1 } else { seed };
    if let Ok(mut rng) = JITTER_RNG.lock() {
        *rng = Some(textquest_common::nav::Xorshift32::new(seed));
    }
}

/// Human-like jitter using a triangle distribution (sum of two uniform draws).
/// Peaks at 6 ticks with occasional hesitation spikes simulating distraction.
/// Range: 2-30 ticks (without hesitation: 2-10, with hesitation: 7-30).
fn human_jitter_ticks(rng: &mut textquest_common::nav::Xorshift32) -> u64 {
    // Triangle distribution: sum of two uniform draws (peaks at center)
    let base = (rng.next_u32() % 5 + 1) + (rng.next_u32() % 5 + 1); // 2-10, peaks at 6
    // 5% chance of hesitation spike (simulates distraction)
    let hesitate = if rng.next_u32().is_multiple_of(20) {
        rng.next_u32() % 15 + 5
    } else {
        0
    };
    u64::from(base + hesitate)
}

/// Queue a slash command for execution on the next game loop tick.
/// Safe to call from any thread — the game loop will pick it up.
pub fn queue_slash_command(command: String) {
    if let Ok(mut queue) = PENDING_COMMANDS.lock() {
        queue.push(PendingCommand {
            command: textquest_common::ipc::Command::SlashCommand { command },
            execute_at_tick: 0, // execute immediately on next tick
        });
    }
}

// ─── Casting Loop (kill / recast) ───
//
// When a `CastSpell` command arrives with `kill = true` or `recast > 0`, the
// DLL starts a repeating cast loop driven by the game tick.
//
// `kill` mode:  keep casting until the target's HP drops to zero or the target
//               disappears.  The loop self-cancels on target death or a
//               `CancelCastLoop` command.
//
// `recast` mode: cast N+1 times total with exponential backoff between
//                attempts. The backoff starts at
//                `CAST_LOOP_BASE_BACKOFF_TICKS` and doubles each attempt,
//                capped at `CAST_LOOP_MAX_BACKOFF_TICKS`.

/// Base backoff between recast attempts (~0.4 s at 20 ticks/sec).
const CAST_LOOP_BASE_BACKOFF_TICKS: u64 = 8;

/// Maximum backoff cap in ticks (~1.5 s at 20 ticks/sec).
const CAST_LOOP_MAX_BACKOFF_TICKS: u64 = 30;

#[derive(Debug, Clone)]
enum CastLoopMode {
    /// Keep casting until the target dies.  Stores the target spawn-ID so we
    /// can detect when it changes (e.g. target was cleared between iterations).
    Kill { target_id: u32 },
    /// Cast `remaining` more times (including the current attempt).
    Recast { remaining: u8 },
}

#[derive(Debug)]
struct CastingLoop {
    /// Gem slot to cast (1-based, 1-13).
    spell_slot: u8,
    /// Optional spawn ID to switch target to before each cast.
    target_id: Option<u32>,
    /// Whether the loop is active.
    active: bool,
    /// Control mode.
    mode: CastLoopMode,
    /// Tick at which the next cast attempt should fire.
    next_cast_tick: u64,
    /// Current backoff in ticks (doubles on each recast attempt, capped).
    backoff_ticks: u64,
}

impl CastingLoop {
    fn start_kill(spell_slot: u8, target_id: Option<u32>, current_tick: u64) -> Self {
        let target = target_id.unwrap_or(0);
        Self {
            spell_slot,
            target_id,
            active: true,
            mode: CastLoopMode::Kill { target_id: target },
            next_cast_tick: current_tick,
            backoff_ticks: CAST_LOOP_BASE_BACKOFF_TICKS,
        }
    }

    fn start_recast(spell_slot: u8, target_id: Option<u32>, recast: u8, current_tick: u64) -> Self {
        Self {
            spell_slot,
            target_id,
            active: true,
            mode: CastLoopMode::Recast {
                remaining: recast.saturating_add(1), // include the first cast
            },
            next_cast_tick: current_tick,
            backoff_ticks: CAST_LOOP_BASE_BACKOFF_TICKS,
        }
    }

    fn cancel(&mut self) {
        self.active = false;
    }

    /// Execute one tick of the loop.  Queues a cast slash command if
    /// appropriate. Returns `false` when the loop should be stopped (target
    /// dead, casts exhausted).
    fn tick(&mut self, current_tick: u64, eq_base: u64) -> bool {
        if !self.active {
            return false;
        }
        if current_tick < self.next_cast_tick {
            return true; // waiting for backoff
        }
        if eq_base != 0 && cast_in_progress(eq_base) {
            self.next_cast_tick = current_tick + 1;
            return true;
        }

        match &mut self.mode {
            CastLoopMode::Kill { target_id } => {
                // Check if the target is still alive.  On non-Windows this always
                // returns None (stub), so the loop runs until cancelled.
                if eq_base != 0 {
                    let target = read_target_state(eq_base);
                    match target {
                        None => {
                            tracing::info!(
                                spell_slot = self.spell_slot,
                                "CastLoop(kill): target gone — stopping"
                            );
                            self.active = false;
                            return false;
                        }
                        Some(ref t) if t.spawn_id != *target_id && *target_id != 0 => {
                            tracing::info!(
                                spell_slot = self.spell_slot,
                                old_target = *target_id,
                                new_target = t.spawn_id,
                                "CastLoop(kill): target changed — stopping"
                            );
                            self.active = false;
                            return false;
                        }
                        Some(ref t) if t.hp_current <= 0 => {
                            tracing::info!(
                                spell_slot = self.spell_slot,
                                spawn_id = t.spawn_id,
                                "CastLoop(kill): target HP <= 0 — stopping"
                            );
                            self.active = false;
                            return false;
                        }
                        _ => {}
                    }
                }

                issue_cast(self.spell_slot, self.target_id);
                self.next_cast_tick = current_tick + self.backoff_ticks;
                true
            }
            CastLoopMode::Recast { remaining } => {
                if *remaining == 0 {
                    tracing::info!(
                        spell_slot = self.spell_slot,
                        "CastLoop(recast): all casts complete — stopping"
                    );
                    self.active = false;
                    return false;
                }

                tracing::info!(
                    spell_slot = self.spell_slot,
                    remaining = *remaining,
                    "CastLoop(recast): firing cast"
                );
                issue_cast(self.spell_slot, self.target_id);
                *remaining -= 1;

                let current_delay = self.backoff_ticks;
                self.next_cast_tick = current_tick + current_delay;
                // Exponential backoff, capped.
                self.backoff_ticks = (current_delay * 2).min(CAST_LOOP_MAX_BACKOFF_TICKS);
                true
            }
        }
    }
}

/// Issue the cast slash command(s) for one loop iteration.
fn issue_cast(spell_slot: u8, target_id: Option<u32>) {
    if let Some(tid) = target_id {
        queue_slash_command(format!("/target id {tid}"));
    }
    queue_slash_command(format!("/cast {spell_slot}"));
}

fn cast_in_progress(eq_base: u64) -> bool {
    super::casting::CastingController::new(eq_base)
        .is_casting()
        .unwrap_or(false)
}

static CAST_LOOP: Mutex<Option<CastingLoop>> = Mutex::new(None);

/// Tick the active casting loop (if any) from the game loop thread.
fn tick_cast_loop(current_tick: u64) {
    let Ok(mut guard) = CAST_LOOP.lock() else {
        return;
    };
    let Some(ref mut loop_state) = *guard else {
        return;
    };
    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    if !loop_state.tick(current_tick, eq_base) {
        *guard = None; // loop finished
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CastingAction {
    CastGem(u8),
    UseItem(String),
}

impl CastingAction {
    fn slash_command(&self) -> String {
        match self {
            Self::CastGem(slot) => format!("/cast {slot}"),
            Self::UseItem(selector) => format!("/useitem {selector}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedCastingCommand {
    action: CastingAction,
    target_id: Option<u32>,
    require_not_invisible: bool,
    bandolier_set: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum MovementSlashCommand {
    Stick(StickSlashCommand),
    Follow(FollowSlashCommand),
}

#[derive(Debug, Clone, PartialEq)]
enum StickSlashCommand {
    Start(textquest_common::nav::StickConfig),
    Off,
    Mod(f32),
}

#[derive(Debug, Clone, PartialEq)]
enum FollowSlashCommand {
    Start { leader_name: Option<String> },
    Off,
}

const DEFAULT_FOLLOW_DISTANCE: f32 = 20.0;
const DEFAULT_FOLLOW_LEASH_DISTANCE: f32 = 60.0;

fn parse_casting_command(command: &str) -> Option<Result<ParsedCastingCommand, String>> {
    let tokens = match tokenize_slash_command(command) {
        Ok(tokens) => tokens,
        Err(err) => return Some(Err(err.to_string())),
    };
    let (verb, args) = tokens.split_first()?;
    if !verb.eq_ignore_ascii_case("/casting") {
        return None;
    }

    let Some(subject) = args.first() else {
        return Some(Err("missing spell or item selector".to_string()));
    };

    let mut cast_type: Option<&str> = None;
    let mut target_id = None;
    let mut require_not_invisible = false;
    let mut bandolier_set = None;

    for token in &args[1..] {
        if token.eq_ignore_ascii_case("-invis") {
            require_not_invisible = true;
            continue;
        }

        if let Some(value) = parse_pipe_option(token, "-targetid") {
            let parsed_target = value
                .parse::<u32>()
                .map_err(|_| format!("invalid -targetid value: {value}"));
            match parsed_target {
                Ok(parsed_target) => {
                    target_id = Some(parsed_target);
                    continue;
                }
                Err(err) => return Some(Err(err)),
            }
        }

        if let Some(value) = parse_pipe_option(token, "-bandolier") {
            let value = value.trim();
            if value.is_empty() {
                return Some(Err("missing -bandolier value".to_string()));
            }
            if bandolier_set.replace(value.to_string()).is_some() {
                return Some(Err("multiple -bandolier options specified".to_string()));
            }
            continue;
        }

        if cast_type.replace(token.as_str()).is_some() {
            return Some(Err(format!("multiple cast types specified: {token}")));
        }
    }

    let Some(cast_type) = cast_type else {
        return Some(Err("missing cast type (expected gem#, item, or an item \
                         slot)"
            .to_string()));
    };

    let cast_type_lower = cast_type.to_ascii_lowercase();
    let action = if let Some(slot) = cast_type_lower.strip_prefix("gem") {
        let slot = slot
            .parse::<u8>()
            .map_err(|_| format!("invalid gem selector: {cast_type}"));
        match slot {
            Ok(slot @ 1..=13) => CastingAction::CastGem(slot),
            Ok(slot) => return Some(Err(format!("gem slot out of range: {slot}"))),
            Err(err) => return Some(Err(err)),
        }
    } else if cast_type_lower == "item" {
        CastingAction::UseItem(quote_for_eq(subject))
    } else if is_item_slot_selector(&cast_type_lower) {
        CastingAction::UseItem(cast_type_lower)
    } else {
        return Some(Err(format!("unsupported /casting type: {cast_type}")));
    };

    Some(Ok(ParsedCastingCommand {
        action,
        target_id,
        require_not_invisible,
        bandolier_set,
    }))
}

fn parse_movement_slash_command(command: &str) -> Option<Result<MovementSlashCommand, String>> {
    let tokens = match tokenize_slash_command(command) {
        Ok(tokens) => tokens,
        Err(err) => return Some(Err(err.to_string())),
    };
    let (verb, args) = tokens.split_first()?;
    if verb.eq_ignore_ascii_case("/stick") {
        return Some(parse_stick_slash_command(args).map(MovementSlashCommand::Stick));
    }
    if verb.eq_ignore_ascii_case("/follow") {
        return Some(parse_follow_slash_command(args).map(MovementSlashCommand::Follow));
    }
    None
}

fn parse_stick_slash_command(args: &[String]) -> Result<StickSlashCommand, String> {
    if args.is_empty() {
        return Ok(StickSlashCommand::Start(
            textquest_common::nav::StickConfig::default(),
        ));
    }

    if args.len() == 1 && args[0].eq_ignore_ascii_case("off") {
        return Ok(StickSlashCommand::Off);
    }

    if args[0].eq_ignore_ascii_case("mod") {
        let Some(delta) = args.get(1) else {
            return Err("missing distance delta for /stick mod".to_string());
        };
        let delta = delta
            .parse::<f32>()
            .map_err(|_| format!("invalid /stick mod value: {delta}"))?;
        return Ok(StickSlashCommand::Mod(delta));
    }

    let mut config = textquest_common::nav::StickConfig::default();
    let mut idx = 0usize;
    while let Some(token) = args.get(idx) {
        if token.eq_ignore_ascii_case("hold") {
            config.hold = true;
        } else if token.eq_ignore_ascii_case("always") {
            config.always = true;
        } else if token.eq_ignore_ascii_case("moveback") {
            config.moveback = true;
        } else if token.eq_ignore_ascii_case("healer") {
            config.healer = true;
        } else if token.eq_ignore_ascii_case("autopause") {
            config.autopause = true;
        } else if token.eq_ignore_ascii_case("behind") {
            config.mode = textquest_common::nav::StickMode::Behind;
        } else if token.eq_ignore_ascii_case("!front") || token.eq_ignore_ascii_case("notfront") {
            config.mode = textquest_common::nav::StickMode::NotFront;
        } else if token.eq_ignore_ascii_case("pin") {
            config.mode = textquest_common::nav::StickMode::Pin;
        } else if token.eq_ignore_ascii_case("front") {
            config.mode = textquest_common::nav::StickMode::Front;
        } else if token.eq_ignore_ascii_case("snaproll") {
            config.mode = textquest_common::nav::StickMode::SnapRoll;
        } else if token.eq_ignore_ascii_case("id") {
            let Some(raw_id) = args.get(idx + 1) else {
                return Err("missing spawn id for /stick id".to_string());
            };
            config.id = Some(
                raw_id
                    .parse::<u32>()
                    .map_err(|_| format!("invalid /stick id value: {raw_id}"))?,
            );
            idx += 1;
        } else if token.eq_ignore_ascii_case("behindarc") {
            let Some(raw_arc) = args.get(idx + 1) else {
                return Err("missing arc value for /stick behindarc".to_string());
            };
            config.behind_arc = raw_arc
                .parse::<f32>()
                .map_err(|_| format!("invalid /stick behindarc value: {raw_arc}"))?;
            idx += 1;
        } else if token.eq_ignore_ascii_case("!frontarc")
            || token.eq_ignore_ascii_case("notfrontarc")
        {
            let Some(raw_arc) = args.get(idx + 1) else {
                return Err("missing arc value for /stick !frontarc".to_string());
            };
            config.not_front_arc = raw_arc
                .parse::<f32>()
                .map_err(|_| format!("invalid /stick !frontarc value: {raw_arc}"))?;
            idx += 1;
        } else if token.eq_ignore_ascii_case("backupdist") {
            let Some(raw_dist) = args.get(idx + 1) else {
                return Err("missing distance for /stick backupdist".to_string());
            };
            config.backup_dist = raw_dist
                .parse::<f32>()
                .map_err(|_| format!("invalid /stick backupdist value: {raw_dist}"))?;
            idx += 1;
        } else if let Some(percent) = token.strip_suffix('%') {
            config.distance = textquest_common::nav::StickDistance::Percent(
                percent
                    .parse::<f32>()
                    .map_err(|_| format!("invalid /stick distance percentage: {token}"))?,
            );
        } else if let Ok(distance) = token.parse::<f32>() {
            config.distance = textquest_common::nav::StickDistance::Absolute(distance);
        } else {
            return Err(format!("unsupported /stick token: {token}"));
        }

        idx += 1;
    }

    Ok(StickSlashCommand::Start(config))
}

fn parse_follow_slash_command(args: &[String]) -> Result<FollowSlashCommand, String> {
    if args.is_empty() {
        return Ok(FollowSlashCommand::Start { leader_name: None });
    }
    if args.len() == 1 && args[0].eq_ignore_ascii_case("off") {
        return Ok(FollowSlashCommand::Off);
    }

    let leader_name = args.join(" ").trim().to_string();
    if leader_name.is_empty() {
        return Err("missing leader name for /follow".to_string());
    }
    Ok(FollowSlashCommand::Start {
        leader_name: Some(leader_name),
    })
}

fn parse_pipe_option<'a>(token: &'a str, option: &str) -> Option<&'a str> {
    token
        .split_once('|')
        .filter(|(name, _)| name.eq_ignore_ascii_case(option))
        .map(|(_, value)| value)
}

fn tokenize_slash_command(input: &str) -> Result<Vec<String>, &'static str> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            ch if ch.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if in_quotes {
        return Err("unterminated quote in slash command");
    }
    if escaped {
        current.push('\\');
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

fn quote_for_eq(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn bandolier_activate_command(set_name: &str) -> String {
    let selector = if set_name.chars().all(|ch| ch.is_ascii_digit()) {
        set_name.to_string()
    } else {
        quote_for_eq(set_name)
    };
    format!("/bandolier activate {selector}")
}

fn parse_bandolier_activate_command(command: &str) -> Option<String> {
    let tokens = tokenize_slash_command(command).ok()?;
    match tokens.as_slice() {
        [verb, action, selector, ..]
            if verb.eq_ignore_ascii_case("/bandolier")
                && action.eq_ignore_ascii_case("activate") =>
        {
            let selector = selector.trim();
            if selector.is_empty() {
                None
            } else {
                Some(selector.to_string())
            }
        }
        _ => None,
    }
}

fn schedule_bandolier_swap(requested_set: &str, current_tick: u64) {
    let requested_set = requested_set.trim();
    if requested_set.is_empty() {
        return;
    }

    let current_active = ACTIVE_BANDOLIER_SET
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let should_swap = current_active
        .as_deref()
        .is_none_or(|current| !current.eq_ignore_ascii_case(requested_set));

    let restore_to = if let Ok(mut pending) = PENDING_BANDOLIER_RESTORE.lock() {
        let restore_to = pending
            .as_ref()
            .and_then(|state| state.restore_to.clone())
            .or(current_active.clone())
            .filter(|current| !current.eq_ignore_ascii_case(requested_set));

        *pending = Some(PendingBandolierRestore {
            requested_set: requested_set.to_string(),
            restore_to: restore_to.clone(),
            deadline_tick: current_tick + BANDOLIER_CAST_TIMEOUT_TICKS,
            saw_casting: false,
        });
        restore_to
    } else {
        None
    };

    if should_swap {
        tracing::info!(bandolier = %requested_set, "Queueing /casting bandolier swap");
        queue_slash_command(bandolier_activate_command(requested_set));
    } else if restore_to.is_none() {
        tracing::debug!(bandolier = %requested_set, "Skipping redundant /casting bandolier swap");
    }
}

fn next_bandolier_restore_command(
    pending: &mut Option<PendingBandolierRestore>,
    current_tick: u64,
    observed_casting: Option<bool>,
) -> Option<String> {
    let state = pending.as_mut()?;

    let is_currently_casting = observed_casting == Some(true);
    if is_currently_casting {
        state.saw_casting = true;
        state.deadline_tick = current_tick + BANDOLIER_CAST_TIMEOUT_TICKS;
        return None;
    }

    let should_restore = if state.saw_casting {
        observed_casting == Some(false) || current_tick >= state.deadline_tick
    } else {
        current_tick >= state.deadline_tick
    };

    if !should_restore {
        return None;
    }

    let state = pending.take()?;
    state
        .restore_to
        .filter(|restore_to| !restore_to.eq_ignore_ascii_case(&state.requested_set))
        .map(|restore_to| bandolier_activate_command(&restore_to))
}

fn process_pending_bandolier_restore(current_tick: u64) {
    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    let observed_casting = if eq_base == 0 {
        None
    } else {
        super::casting::CastingController::new(eq_base)
            .is_casting()
            .ok()
    };

    let restore_command = if let Ok(mut pending) = PENDING_BANDOLIER_RESTORE.lock() {
        next_bandolier_restore_command(&mut pending, current_tick, observed_casting)
    } else {
        None
    };

    if let Some(command) = restore_command {
        tracing::info!(cmd = %command, "Restoring bandolier after /casting");
        queue_slash_command(command);
    }
}

fn is_item_slot_selector(selector: &str) -> bool {
    selector.chars().all(|ch| ch.is_ascii_digit())
        || matches!(
            selector,
            "charm"
                | "leftear"
                | "rightear"
                | "head"
                | "face"
                | "neck"
                | "shoulders"
                | "arms"
                | "back"
                | "leftwrist"
                | "rightwrist"
                | "range"
                | "hands"
                | "primary"
                | "secondary"
                | "offhand"
                | "mainhand"
                | "leftfinger"
                | "rightfinger"
                | "chest"
                | "legs"
                | "feet"
                | "waist"
                | "ammo"
                | "powersource"
        )
}

fn spell_name_indicates_invisibility(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    if lower.is_empty() || lower.contains("see invis") || lower.contains("see invisible") {
        return false;
    }
    if lower == "invisibility to animals" {
        return false;
    }

    lower.starts_with("invis")
        || lower.contains(" invisibility")
        || lower.contains("camouflage")
        || lower.contains("shroud of stealth")
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NavWaypointCommand {
    Save(String),
    Recall(String),
    Delete(String),
    List,
}

fn parse_nav_waypoint_command(command: &str) -> Option<Result<NavWaypointCommand, String>> {
    let tokens = match tokenize_slash_command(command) {
        Ok(tokens) => tokens,
        Err(err) => return Some(Err(err.to_string())),
    };

    let (verb, args) = tokens.split_first()?;
    if !verb.eq_ignore_ascii_case("/nav") {
        return None;
    }

    let Some(first) = args.first() else {
        return Some(Err(String::from("Missing waypoint subcommand")));
    };

    // Support MQ2-style aliases: /nav waypoint, /nav wp, /nav recordwaypoint.
    let subcommand = first.to_ascii_lowercase();
    match subcommand.as_str() {
        "waypoint" | "wp" => {
            // /nav waypoint list
            if args.len() == 2 && args[1].eq_ignore_ascii_case("list") {
                return Some(Ok(NavWaypointCommand::List));
            }

            if args.len() >= 2 && args[1].eq_ignore_ascii_case("delete") {
                let name = args.get(2).map(|s| s.as_str()).unwrap_or_default();
                if name.trim().is_empty() {
                    return Some(Err(String::from("Waypoint name is required for delete")));
                }
                return Some(Ok(NavWaypointCommand::Delete(name.to_string())));
            }

            if args.len() >= 2 && args[1].eq_ignore_ascii_case("save") {
                let name = args.get(2).map(|s| s.as_str()).unwrap_or_default();
                if name.trim().is_empty() {
                    return Some(Err(String::from("Waypoint name is required for save")));
                }
                return Some(Ok(NavWaypointCommand::Save(name.to_string())));
            }

            if args.len() >= 2 && args[1].eq_ignore_ascii_case("recall") {
                let name = args.get(2).map(|s| s.as_str()).unwrap_or_default();
                if name.trim().is_empty() {
                    return Some(Err(String::from("Waypoint name is required for recall")));
                }
                return Some(Ok(NavWaypointCommand::Recall(name.to_string())));
            }

            // /nav waypoint recall <name> or /nav waypoint <name>
            let name = args.get(1).map(|s| s.as_str()).unwrap_or_default();
            if name.trim().is_empty() {
                return Some(Err(String::from("Waypoint name is required")));
            }
            Some(Ok(NavWaypointCommand::Recall(name.to_string())))
        }
        "recordwaypoint" => {
            let name = args.get(1).map(|s| s.as_str()).unwrap_or_default();
            if name.trim().is_empty() {
                return Some(Err(String::from("Waypoint name is required for save")));
            }
            Some(Ok(NavWaypointCommand::Save(name.to_string())))
        }
        _ => None,
    }
}

#[cfg(windows)]
fn player_is_invisible(eq_base: u64) -> bool {
    crate::combat::buffs::read_active_buffs(eq_base)
        .into_iter()
        .filter_map(|buff| read_spell_name(eq_base, buff.spell_id))
        .any(|name| spell_name_indicates_invisibility(&name))
}

#[cfg(not(windows))]
fn player_is_invisible(_eq_base: u64) -> bool {
    false
}

#[cfg(windows)]
fn read_spell_name(eq_base: u64, spell_id: i32) -> Option<String> {
    use textquest_common::offsets::{self, client_spell_manager, eq_spell, spell_hash_map};

    let spell_id = u32::try_from(spell_id).ok()?;
    let spell_mgr_ptr_addr = offsets::rebase(offsets::PINST_SPELL_MANAGER, eq_base)?;
    let spell_mgr_addr = read_usize_field(spell_mgr_ptr_addr)?;
    let max_spell_id = read_i32_field(spell_mgr_addr + client_spell_manager::MAX_SPELL_ID)?;
    if max_spell_id <= 0 || spell_id >= max_spell_id as u32 {
        return None;
    }

    let spells_map_addr = spell_mgr_addr + client_spell_manager::SPELLS;
    let buckets_addr = read_usize_field(spells_map_addr + spell_hash_map::BUCKETS)?;
    let dynamic_size = read_u64_field(spells_map_addr + spell_hash_map::DYNAMIC_SIZE)? as usize;
    if dynamic_size == 0 || !dynamic_size.is_power_of_two() {
        return None;
    }

    let bucket_index = spell_id as usize & (dynamic_size - 1);
    let bucket_ptr_addr = buckets_addr + bucket_index * size_of::<usize>();
    let mut node_addr = read_usize_field(bucket_ptr_addr)?;
    let mut hops = 0usize;
    const MAX_HASH_MAP_HOPS: usize = 128;

    while node_addr != 0 && hops < MAX_HASH_MAP_HOPS {
        let node_key = read_i32_field(node_addr + spell_hash_map::KEY)?;
        if node_key == spell_id as i32 {
            let spell_addr = node_addr + spell_hash_map::VALUE;
            let stored_id = read_i32_field(spell_addr + eq_spell::ID)?;
            if stored_id != node_key {
                return None;
            }
            return read_string_field(spell_addr + eq_spell::NAME, 64);
        }

        node_addr = read_usize_field(node_addr + spell_hash_map::HASH_NEXT)?;
        hops += 1;
    }

    None
}

#[cfg(windows)]
fn read_usize_field(addr: usize) -> Option<usize> {
    if !is_readable(addr, size_of::<usize>()) {
        return None;
    }
    Some(unsafe { *(addr as *const usize) })
}

#[cfg(windows)]
fn read_u64_field(addr: usize) -> Option<u64> {
    if !is_readable(addr, size_of::<u64>()) {
        return None;
    }
    Some(unsafe { *(addr as *const u64) })
}

#[cfg(windows)]
fn read_i32_field(addr: usize) -> Option<i32> {
    if !is_readable(addr, size_of::<i32>()) {
        return None;
    }
    Some(unsafe { *(addr as *const i32) })
}

#[cfg(windows)]
fn read_string_field(addr: usize, len: usize) -> Option<String> {
    if !is_readable(addr, len) {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(addr as *const u8, len) };
    let end = bytes.iter().position(|&byte| byte == 0).unwrap_or(len);
    let text = String::from_utf8_lossy(&bytes[..end]).trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn handle_casting_slash_command(command: &str) -> bool {
    match parse_casting_command(command) {
        None => false,
        Some(Err(err)) => {
            tracing::warn!(cmd = %command, %err, "Unsupported /casting command");
            true
        }
        Some(Ok(parsed)) => {
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
            if parsed.require_not_invisible && eq_base != 0 && player_is_invisible(eq_base) {
                tracing::info!(cmd = %command, "Skipping /casting because player is invisible");
                return true;
            }

            if let Some(bandolier_set) = parsed.bandolier_set.as_deref() {
                let current_tick = TICK_COUNT.load(std::sync::atomic::Ordering::Relaxed);
                schedule_bandolier_swap(bandolier_set, current_tick);
            }

            if let Some(target_id) = parsed.target_id {
                queue_slash_command(format!("/target id {target_id}"));
            }
            queue_slash_command(parsed.action.slash_command());
            true
        }
    }
}

fn spawn_matches_name(spawn: &textquest_common::types::SpawnData, leader_name: &str) -> bool {
    spawn.displayed_name.eq_ignore_ascii_case(leader_name)
        || spawn.name.eq_ignore_ascii_case(leader_name)
}

fn resolve_follow_spawn<'a>(
    current_target: Option<&'a textquest_common::types::SpawnData>,
    nearby: &'a [textquest_common::types::SpawnData],
    leader_name: Option<&str>,
) -> Option<&'a textquest_common::types::SpawnData> {
    let leader_name = leader_name.map(str::trim).filter(|value| !value.is_empty());
    match leader_name {
        None => current_target,
        Some(leader_name) => current_target
            .filter(|spawn| spawn_matches_name(spawn, leader_name))
            .or_else(|| {
                nearby
                    .iter()
                    .find(|spawn| spawn_matches_name(spawn, leader_name))
            }),
    }
}

fn handle_movement_slash_command(command: &str) -> bool {
    match parse_movement_slash_command(command) {
        None => false,
        Some(Err(err)) => {
            tracing::warn!(cmd = %command, %err, "Unsupported movement slash command");
            true
        }
        Some(Ok(MovementSlashCommand::Stick(StickSlashCommand::Off))) => {
            crate::nav::handle_command(crate::nav::NavCommand::StickOff);
            true
        }
        Some(Ok(MovementSlashCommand::Stick(StickSlashCommand::Mod(delta)))) => {
            crate::nav::handle_command(crate::nav::NavCommand::StickMod(delta));
            true
        }
        Some(Ok(MovementSlashCommand::Stick(StickSlashCommand::Start(config)))) => {
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
            let current_target_id = if eq_base != 0 {
                read_target_state(eq_base).map(|target| target.spawn_id)
            } else {
                None
            };
            crate::nav::handle_command(crate::nav::NavCommand::StickTo {
                config,
                current_target_id,
            });
            true
        }
        Some(Ok(MovementSlashCommand::Follow(FollowSlashCommand::Off))) => {
            crate::nav::handle_command(crate::nav::NavCommand::StopFollow);
            true
        }
        Some(Ok(MovementSlashCommand::Follow(FollowSlashCommand::Start { leader_name }))) => {
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
            if eq_base == 0 {
                tracing::warn!(cmd = %command, "Ignoring /follow outside the world");
                return true;
            }

            let current_target = read_target_state(eq_base);
            let nearby_guard = CACHED_NEARBY_FOR_STICK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(leader) = resolve_follow_spawn(
                current_target.as_ref(),
                &nearby_guard,
                leader_name.as_deref(),
            ) else {
                tracing::warn!(cmd = %command, "Could not resolve /follow leader");
                return true;
            };

            let leader_display_name = if leader.displayed_name.trim().is_empty() {
                leader.name.clone()
            } else {
                leader.displayed_name.clone()
            };
            let anchor = textquest_common::nav::Waypoint::new(leader.x, leader.y, leader.z);
            drop(nearby_guard);

            crate::nav::handle_command(crate::nav::NavCommand::FollowPlayer {
                config: textquest_common::nav::FollowConfig::new(
                    leader_display_name,
                    DEFAULT_FOLLOW_DISTANCE,
                    DEFAULT_FOLLOW_LEASH_DISTANCE,
                ),
                anchor,
            });
            true
        }
    }
}

/// Enqueue a command with a human-like jitter delay.
fn enqueue_command(cmd: textquest_common::ipc::Command, current_tick: u64) {
    let delay = JITTER_RNG
        .lock()
        .ok()
        .and_then(|mut guard| guard.as_mut().map(human_jitter_ticks))
        .unwrap_or(5);

    if let Ok(mut queue) = PENDING_COMMANDS.lock() {
        queue.push(PendingCommand {
            command: cmd,
            execute_at_tick: current_tick + delay,
        });
    }
}

/// Drain and execute any commands whose scheduled tick has arrived.
fn process_pending_commands(current_tick: u64) {
    let Ok(mut queue) = PENDING_COMMANDS.lock() else {
        return;
    };
    if queue.is_empty() {
        return;
    }

    let mut ready = Vec::new();
    queue.retain(|pending| {
        if current_tick >= pending.execute_at_tick {
            ready.push(pending.command.clone());
            false
        } else {
            true
        }
    });

    // Drop the lock before dispatching to avoid holding it during command
    // execution.
    drop(queue);

    for cmd in ready {
        dispatch_command(cmd);
    }
}

/// Called every frame (~20/sec) after the original `MainLoop` runs.
/// This is our main entry point for per-frame logic.
fn on_game_tick() {
    let tick_start = std::time::Instant::now();
    let tick = TICK_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    // Check foreground status every 30 frames (~1.5 seconds) to minimize overhead.
    if tick.is_multiple_of(30) {
        update_foreground_status();
    }

    // Auto-accept dialogs every 30 frames (~1.5 seconds).
    if tick % 30 == 15 {
        // SAFETY: check_dialogs reads EQ's CXWndManager and clicks dialog buttons
        // via vtable. Called from the game loop thread where UI state is stable and
        // vtable calls are safe. EQ_BASE and PINST_CXWND_MANAGER are validated
        // internally with null checks before any dereference.
        unsafe {
            crate::dialog::check_dialogs();
        }
    }

    // Rename window every 100 frames (~5 seconds) to "[TQ] EQ - CharName
    // (ZoneName)".
    if tick % 100 == 5 {
        update_window_title();
    }

    // Enqueue IPC commands with jitter delay for anti-detection.
    for ipc_cmd in crate::ipc::poll_commands() {
        enqueue_command(ipc_cmd.command, tick);
    }

    // Execute commands whose scheduled tick has arrived.
    process_pending_commands(tick);
    process_pending_bandolier_restore(tick);

    // Tick the active casting loop (kill / recast) every frame.
    tick_cast_loop(tick);

    // Check for pending login button click (queued from IPC thread).
    let button_addr = PENDING_BUTTON_CLICK.swap(0, std::sync::atomic::Ordering::AcqRel);
    if button_addr != 0 {
        if !is_readable(button_addr, 8) {
            tracing::warn!(
                ptr = format!("{:#x}", button_addr),
                "Pending button click target is no longer readable — skipping (window may have \
                 been destroyed)"
            );
        } else {
            tracing::info!(
                ptr = format!("{:#x}", button_addr),
                "Clicking login button on game loop thread"
            );
            // SAFETY: button_addr was stored by the IPC thread after resolving a
            // CXWnd pointer from CXWndManager's window array. The click executes
            // WndNotification(XWM_LCLICK) through the CXWnd vtable. Must run on
            // the game loop thread (which we are). The pointer is re-validated via
            // is_readable() above to guard against use-after-free if the window
            // was destroyed between queuing and dispatch.
            unsafe {
                crate::eq::widgets::click_button_via_vtable(button_addr);
            }
        }
    }

    // Process Enter World sequence (SelectCharacter → wait → EnterWorld).
    // Stage 1: Find character by name, call SelectCharacter(index)
    // Stage 2: Wait ~90 ticks (~3 seconds)
    // Stage 3: Call EnterWorld()
    let stage = ENTER_WORLD_STAGE.load(std::sync::atomic::Ordering::Acquire);
    if stage == 1 {
        let wnd = PENDING_ENTER_WORLD_WND.load(std::sync::atomic::Ordering::Acquire);
        let select_fn = PENDING_SELECT_CHAR_FN.load(std::sync::atomic::Ordering::Acquire);
        if wnd != 0 && select_fn != 0 {
            // Look up character index by name (falls back to 0 if not found/empty)
            let char_name = PENDING_CHAR_NAME
                .get()
                .and_then(|m| m.lock().ok())
                .map(|n| n.clone())
                .unwrap_or_default();
            let index = find_character_index(wnd, &char_name);

            tracing::info!(
                wnd = format!("{:#x}", wnd),
                func = format!("{:#x}", select_fn),
                character = %char_name,
                index,
                "Phase 3: Calling SelectCharacter on game loop thread"
            );
            // SAFETY: select_fn was rebased from SELECT_CHARACTER offset against
            // eqgame.exe's base. The transmute converts it to a function pointer
            // matching CCharacterListWnd::SelectCharacter(int). `wnd` is a freshly
            // resolved CCharacterListWnd* from CXWndManager. If either address is
            // stale or the offset is wrong, EQ will crash (no safe fallback exists
            // for calling internal game functions with wrong addresses).
            unsafe {
                // SelectCharacter(int index) — x64: RCX=this, RDX=index
                type SelectCharFn = unsafe extern "C" fn(this: usize, index: i32);
                let func: SelectCharFn = std::mem::transmute(select_fn);
                func(wnd, index);
            }
            tracing::info!(
                index,
                "Phase 3: SelectCharacter called — waiting 3s before EnterWorld"
            );
            ENTER_WORLD_WAIT_UNTIL.store(tick + 90, std::sync::atomic::Ordering::Release);
            ENTER_WORLD_STAGE.store(2, std::sync::atomic::Ordering::Release);
        } else {
            // No SelectCharacter available — skip to EnterWorld directly
            tracing::warn!("Phase 3: SelectCharacter not available — skipping to EnterWorld");
            ENTER_WORLD_WAIT_UNTIL.store(tick + 30, std::sync::atomic::Ordering::Release);
            ENTER_WORLD_STAGE.store(2, std::sync::atomic::Ordering::Release);
        }
    } else if stage == 2 {
        let wait_until = ENTER_WORLD_WAIT_UNTIL.load(std::sync::atomic::Ordering::Acquire);
        if tick >= wait_until {
            ENTER_WORLD_STAGE.store(3, std::sync::atomic::Ordering::Release);
        }
    } else if stage == 3 {
        let enter_fn = PENDING_ENTER_WORLD_FN.load(std::sync::atomic::Ordering::Acquire);
        // Re-scan for CCharacterListWnd fresh — the pointer stored in Stage 1
        // may be stale if the window was destroyed/recreated during the wait.
        let wnd = if let Some(w) = rescan_char_list_wnd() {
            w
        } else {
            // Rescan failed — CXWndManager may be in a transitional state.
            // Do NOT fall back to stored pointer (could be stale/freed).
            // Retry next tick up to ~5 seconds, then abort.
            let stored = PENDING_ENTER_WORLD_WND.load(std::sync::atomic::Ordering::Acquire);
            if stored == 0 {
                tracing::error!("Phase 3: rescan failed and no stored pointer — aborting");
                ENTER_WORLD_STAGE.store(0, std::sync::atomic::Ordering::Release);
                return;
            }
            let retries = ENTER_WORLD_RETRIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if retries >= 150 {
                tracing::error!("Phase 3: rescan failed after 150 retries — aborting");
                ENTER_WORLD_STAGE.store(0, std::sync::atomic::Ordering::Release);
                ENTER_WORLD_RETRIES.store(0, std::sync::atomic::Ordering::Relaxed);
                return;
            }
            tracing::warn!(retries, "Phase 3: rescan failed — will retry next tick");
            return; // Stay in stage 3, retry next tick
        };
        if enter_fn != 0 {
            tracing::info!(
                wnd = format!("{:#x}", wnd),
                func = format!("{:#x}", enter_fn),
                "Phase 3: Calling EnterWorld() on game loop thread (re-validated)"
            );
            // SAFETY: enter_fn was rebased from ENTER_WORLD offset. `wnd` was
            // re-validated via rescan_char_list_wnd() immediately above (not the
            // stale pointer from Stage 1). The transmute converts the address to
            // CCharacterListWnd::EnterWorld(). If the offset is wrong or the
            // window was destroyed between rescan and call, this is UB/crash.
            unsafe {
                type EnterWorldFn = unsafe extern "C" fn(this: usize);
                let func: EnterWorldFn = std::mem::transmute(enter_fn);
                func(wnd);
            }
            tracing::info!("Phase 3: EnterWorld() called — entering world!");
        }
        // Reset all state — clear payload atomics BEFORE clearing the stage flag.
        // If ENTER_WORLD_STAGE were reset to 0 first, the IPC thread could observe
        // stage == 0 and immediately queue a new enter-world, writing new values
        // into PENDING_*; our subsequent stores of 0 would then silently discard
        // that queued request.  Clearing payloads first (Relaxed is fine — they
        // are guarded by the stage flag, not independently synchronized) and
        // publishing the idle stage last (Release) eliminates that window.
        PENDING_ENTER_WORLD_WND.store(0, std::sync::atomic::Ordering::Relaxed);
        PENDING_ENTER_WORLD_FN.store(0, std::sync::atomic::Ordering::Relaxed);
        PENDING_SELECT_CHAR_FN.store(0, std::sync::atomic::Ordering::Relaxed);
        ENTER_WORLD_RETRIES.store(0, std::sync::atomic::Ordering::Relaxed);
        ENTER_WORLD_STAGE.store(0, std::sync::atomic::Ordering::Release);
    }

    // Lazy-init the navigator once we're in-world.
    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);

    {
        static NAV_INITIALIZED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !NAV_INITIALIZED.load(std::sync::atomic::Ordering::Relaxed) {
            if eq_base != 0
                && let Some(player_addr) = textquest_common::offsets::rebase(
                    textquest_common::offsets::PINST_LOCAL_PLAYER,
                    eq_base,
                )
            {
                // SAFETY: player_addr was rebased from PINST_LOCAL_PLAYER — a
                // known global pointer in eqgame.exe. Dereferencing it yields the
                // PlayerClient* (null when not logged in). Checked for non-null
                // immediately after. The address is within committed eqgame memory.
                let player_ptr = unsafe { *(player_addr as *const usize) };
                if player_ptr != 0 {
                    crate::nav::init(player_ptr, std::process::id());
                    NAV_INITIALIZED.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
        }
    }

    // Run navigation state machine.
    // Pass current target and cached nearby spawns for the stick engine,
    // plus a target sample for the warp monitor.
    {
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        let nav_target = if eq_base != 0 {
            read_target_state(eq_base)
        } else {
            None
        };
        let target_sample = nav_target.as_ref().map(|t| crate::nav::warp::TargetSample {
            id: t.spawn_id,
            position: textquest_common::nav::Waypoint::new(t.x, t.y, t.z),
        });
        let nearby_guard = CACHED_NEARBY_FOR_STICK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::nav::tick(nav_target.as_ref(), &nearby_guard, target_sample.as_ref());
    }

    // Run login FSM when not yet in world (local_player is null).
    // The login FSM drives credential entry, server/char selection autonomously.
    // Once login is done (InWorld, Error, or Idle), stop all login automation
    // including the periodic Enter key — otherwise it could dismiss NPC dialogs,
    // close windows, or cause other unintended actions in the live game.
    {
        let login_done = crate::login::is_done();

        if !login_done {
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
            let in_world = if eq_base != 0 {
                textquest_common::offsets::rebase(
                    textquest_common::offsets::PINST_LOCAL_PLAYER,
                    eq_base,
                )
                // SAFETY: addr is rebased PINST_LOCAL_PLAYER — a committed
                // global in eqgame.exe. Reading a usize from it yields the
                // local player pointer (0 = not logged in).
                .is_some_and(|addr| unsafe { *(addr as *const usize) } != 0)
            } else {
                false
            };

            if !in_world {
                if let Some(phase) = crate::login::tick() {
                    crate::ipc::send_response(textquest_common::ipc::Response::LoginPhaseUpdate {
                        phase,
                    });
                }

                // If not in world and game loop is running, we're at character select.
                // Send Enter key every ~3 seconds to click the Enter World button.
                // Guard: skip when the Enter World FSM is active (stage != 0) to
                // avoid dismissing windows the FSM expects to interact with.
                if tick % 90 == 45
                    && ENTER_WORLD_STAGE.load(std::sync::atomic::Ordering::Acquire) == 0
                {
                    send_enter_to_eq();
                }
            }
        }
    }

    // Read game state and publish to shared memory for the orchestrator.
    read_and_publish_state(tick);

    let overhead = tick_start.elapsed();
    let overhead = tick_start.elapsed();
    crate::hooks::timing::record_game_loop_hook_overhead(overhead);
    #[cfg(debug_assertions)]
    tracing::debug!(
        elapsed_ns = overhead.as_nanos(),
        tick = tick,
        "ProcessGameEvents hook overhead recorded"
    );
}

/// Send Enter key to this EQ process's window via `PostMessage`.
/// Used at character select to click "Enter World".
#[cfg(windows)]
pub fn send_enter_to_eq() {
    use windows::Win32::{
        Foundation::{BOOL, HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW,
        },
    };

    let our_pid = std::process::id();
    let mut target_hwnd: isize = 0;

    // SAFETY: This callback is only invoked by EnumWindows below, which passes
    // our `data` pointer as LPARAM. The cast back to (u32, *mut isize) is valid
    // because we control the LPARAM value. HWND is always valid within the
    // callback.
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe extern "system" fn find_eq_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let data = &mut *(lparam.0 as *mut (u32, *mut isize));
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == data.0 && IsWindowVisible(hwnd).as_bool() {
            *data.1 = hwnd.0;
            return BOOL(0); // stop
        }
        BOOL(1)
    }

    let mut data = (our_pid, &mut target_hwnd as *mut isize);
    // SAFETY: EnumWindows calls find_eq_window for each top-level window.
    // `data` lives on the stack and outlives the synchronous EnumWindows call.
    // The LPARAM cast is valid because we cast it back to the same type in the
    // callback.
    unsafe {
        let _ = EnumWindows(Some(find_eq_window), LPARAM(&mut data as *mut _ as isize));
    }

    if target_hwnd != 0 {
        const WM_KEYDOWN: u32 = 0x0100;
        const WM_KEYUP: u32 = 0x0101;
        const VK_RETURN: u16 = 0x0D;
        let hwnd = HWND(target_hwnd);
        // SAFETY: PostMessageW is safe to call with any HWND — if the window was
        // destroyed between EnumWindows and here, PostMessage returns an error
        // (which we ignore with `let _`). The WM_KEYDOWN/WM_KEYUP messages with
        // VK_RETURN are standard Win32 keyboard messages.
        unsafe {
            let _ = PostMessageW(hwnd, WM_KEYDOWN, WPARAM(VK_RETURN as usize), LPARAM(0));
            let _ = PostMessageW(hwnd, WM_KEYUP, WPARAM(VK_RETURN as usize), LPARAM(0));
        }
        tracing::info!("Sent Enter key to eqgame window (character select → enter world)");
    }
}

#[cfg(not(windows))]
pub fn send_enter_to_eq() {}

// ─── Game State Reading ───
// Reads EQ memory directly (we're in-process) and publishes to shared memory.

/// Read game state from EQ memory and publish to shared memory each tick.
/// Local player + target are read every tick (fast — just pointer derefs).
/// Nearby spawns are read every 30 ticks (~1 second) to reduce overhead.
///
/// Uses a cached `GameState` to avoid cloning ~100 `SpawnData` (each with 2
/// String heap allocations) on the 29/30 ticks where spawns haven't changed.
/// Only the cheap fields (player, target, timestamp, nav/combat status) are
/// updated in place.
fn read_and_publish_state(tick: u64) {
    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    if eq_base == 0 {
        return;
    }

    // Read local player (every tick).
    let local_player = read_local_player_state(eq_base);
    if local_player.is_none() {
        return; // Not logged in — nothing to publish.
    }

    // Read target (every tick).
    let target = read_target_state(eq_base);

    // Cache the entire GameState to avoid cloning the spawn Vec on non-refresh
    // ticks. On refresh ticks (every 30): rebuild spawns + all fields.
    // On other ticks: update only cheap fields in place (no heap allocations for
    // spawns).
    static CACHED_STATE: Mutex<Option<textquest_common::types::GameState>> = Mutex::new(None);
    static SPAWN_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    let Ok(mut cached) = CACHED_STATE.lock() else {
        return;
    };

    let refresh_spawns = tick.is_multiple_of(30) || cached.is_none();

    if refresh_spawns {
        let Some(ref player) = local_player else {
            return;
        };
        let spawns = read_nearby_spawns(eq_base, player.x, player.y, player.z);
        let (zone_short, zone_long) = read_zone_names(eq_base);

        let prev_cache = PREV_NEARBY_SPAWNS
            .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
        if let Ok(mut previous) = prev_cache.lock() {
            let (next_previous, spawn_events) = compute_spawn_delta_events(
                &previous,
                &spawns,
                zone_short.clone(),
                current_time_ms(),
            );
            *previous = next_previous;
            if !spawn_events.is_empty() {
                crate::ipc::send_response(textquest_common::ipc::Response::SpawnEventBatch {
                    events: spawn_events,
                });
            }
        }

        // Update cached nearby spawns for the stick engine (accessed by nav::tick each
        // frame).
        if let Ok(mut cached_nearby) = CACHED_NEARBY_FOR_STICK.lock() {
            cached_nearby.clone_from(&spawns);
        }

        *cached = Some(textquest_common::types::GameState {
            client_id: std::process::id(),
            local_player,
            target,
            nearby_spawns: spawns,
            timestamp_ms: current_time_ms(),
            nav_status: crate::nav::status(),
            combat_status: crate::combat::status(),
            zone_short_name: zone_short,
            zone_long_name: zone_long,
            actual_version: crate::eq_actual_version(),
        });
    } else if let Some(ref mut state) = *cached {
        state.local_player = local_player;
        state.target = target;
        state.timestamp_ms = current_time_ms();
        state.nav_status = crate::nav::status();
        state.combat_status = crate::combat::status();
    } else {
        return;
    }

    if let Some(ref state) = *cached {
        let spawn_epoch = if refresh_spawns {
            SPAWN_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
        } else {
            SPAWN_EPOCH.load(std::sync::atomic::Ordering::Relaxed)
        };
        let frame = state.to_shared_frame(spawn_epoch, refresh_spawns);
        crate::ipc::publish_state(&frame);
    }
}

fn compute_spawn_delta_events(
    previous: &std::collections::HashMap<u32, (String, u8)>,
    current: &[textquest_common::types::SpawnData],
    zone: String,
    timestamp_ms: u64,
) -> (
    std::collections::HashMap<u32, (String, u8)>,
    Vec<textquest_common::ipc::SpawnEvent>,
) {
    let mut next = std::collections::HashMap::new();
    let mut events = Vec::new();
    for spawn in current {
        if spawn.spawn_id == 0 {
            continue;
        }
        next.insert(spawn.spawn_id, (spawn.displayed_name.clone(), spawn.spawn_type));
    }

    if previous.is_empty() {
        return (next, events);
    }

    for (spawn_id, (name, spawn_type)) in &next {
        if !previous.contains_key(spawn_id) {
            events.push(textquest_common::ipc::SpawnEvent {
                client_id: std::process::id(),
                zone: zone.clone(),
                spawn_name: name.clone(),
                spawn_type: *spawn_type,
                kind: textquest_common::ipc::SpawnEventKind::Created,
                timestamp_ms,
            });
        }
    }

    for (spawn_id, (name, spawn_type)) in previous {
        if !next.contains_key(spawn_id) {
            events.push(textquest_common::ipc::SpawnEvent {
                client_id: std::process::id(),
                zone: zone.clone(),
                spawn_name: name.clone(),
                spawn_type: *spawn_type,
                kind: textquest_common::ipc::SpawnEventKind::Destroyed,
                timestamp_ms,
            });
        }
    }

    (next, events)
}

/// Check whether `addr` points to at least `len` bytes of readable committed
/// memory.
///
/// Uses `VirtualQuery` to verify the page is committed and readable before we
/// dereference it. Returns `false` for null, misaligned, or unmapped addresses.
#[cfg(windows)]
pub(crate) fn is_readable(addr: usize, len: usize) -> bool {
    use windows::Win32::System::Memory::{
        MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_GUARD, PAGE_NOACCESS, VirtualQuery,
    };

    if addr == 0 || len == 0 {
        return false;
    }

    let mut mbi = MEMORY_BASIC_INFORMATION::default();
    let ret = unsafe {
        VirtualQuery(
            Some(addr as *const core::ffi::c_void),
            &mut mbi,
            size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };

    if ret == 0 {
        return false;
    }

    // Must be committed (not reserved or free).
    if mbi.State != MEM_COMMIT {
        return false;
    }

    // Reject guard pages and no-access pages.
    let protect = mbi.Protect;
    if protect.contains(PAGE_NOACCESS) || protect.contains(PAGE_GUARD) {
        return false;
    }

    // Verify the entire range falls within this region.
    let region_end = mbi.BaseAddress as usize + mbi.RegionSize;
    let range_end = addr.saturating_add(len);
    range_end <= region_end
}

#[cfg(not(windows))]
pub(crate) fn is_readable(_addr: usize, _len: usize) -> bool {
    // Stub for non-Windows builds (demo mode). Always true since we never
    // dereference real pointers on macOS/Linux.
    true
}

/// Read a null-terminated string from an in-process address into a stack
/// buffer.
///
/// Uses a fixed 128-byte stack buffer (sufficient for EQ name/zone fields) to
/// avoid per-call heap allocations on the hot path. Only allocates a `String`
/// for the final return value.
///
/// # Safety
/// * `addr` must point to readable memory of at least `max_len` bytes.
/// * `max_len` must not exceed 128 (the stack buffer size) AND must not exceed
///   the actual allocated buffer size for the field being read. EQ's fixed-size
///   char arrays (name=64, displayedName=64, zone=128) always meet this.
/// * Returns an empty string safely when `addr == 0` or memory is unreadable.
unsafe fn read_string_at(addr: usize, max_len: usize) -> String {
    if addr == 0 {
        return String::new();
    }

    // Validate pointer before dereferencing.
    if !is_readable(addr, max_len) {
        return String::new();
    }

    // Use a stack buffer to avoid heap allocation for the intermediate copy.
    // 128 bytes covers all EQ string fields (name=64, zone=128).
    const BUF_SIZE: usize = 128;
    let capped = if max_len <= BUF_SIZE {
        max_len
    } else {
        BUF_SIZE
    };

    let mut buf = [0u8; BUF_SIZE];
    // SAFETY: `addr` was validated as readable for `max_len` bytes by
    // is_readable() above (VirtualQuery confirms committed, non-guard pages).
    // `capped` <= `max_len` and <= BUF_SIZE, so both source and dest are in bounds.
    // The copy is non-overlapping because `buf` is a stack allocation and `addr`
    // points into EQ's process memory.
    unsafe {
        core::ptr::copy_nonoverlapping(addr as *const u8, buf.as_mut_ptr(), capped);
    }

    let len = buf[..capped].iter().position(|&b| b == 0).unwrap_or(capped);
    String::from_utf8_lossy(&buf[..len]).into_owned()
}

/// Build a `SpawnData` from a `PlayerClient` pointer (in-process direct read).
///
/// # Safety
/// Caller must ensure `spawn_ptr` is a plausible `PlayerClient` address.
/// This function validates readability before dereferencing and returns
/// `SpawnData::default()` for any invalid pointer.
unsafe fn read_spawn_data(spawn_ptr: usize) -> textquest_common::types::SpawnData {
    use textquest_common::offsets::{player_base, player_zone};

    // Reject null and obviously bad pointers (must be pointer-aligned).
    if spawn_ptr == 0 || !spawn_ptr.is_multiple_of(core::mem::align_of::<usize>()) {
        return textquest_common::types::SpawnData::default();
    }

    // Validate that the spawn_id field region is readable before touching it.
    if !is_readable(spawn_ptr + player_base::SPAWN_ID, size_of::<u32>()) {
        return textquest_common::types::SpawnData::default();
    }

    // Read spawn_id first as a validity canary: id == 0 means the PlayerClient
    // slot is empty or has been freed. Reading further fields from a freed spawn
    // causes an access violation and crashes eqgame.exe.
    // SAFETY: spawn_ptr alignment and readability were validated above via
    // is_multiple_of(align_of::<usize>()) and is_readable(). The SPAWN_ID
    // offset is a known field within the PlayerClient struct.
    let spawn_id = unsafe { *((spawn_ptr + player_base::SPAWN_ID) as *const u32) };
    if spawn_id == 0 {
        return textquest_common::types::SpawnData::default();
    }

    // SAFETY for all field reads below: spawn_ptr points to a live PlayerClient
    // struct validated by is_readable() and a non-zero spawn_id canary above.
    // Each offset is a known field within PlayerClient derived from MQ2 headers.
    // read_string_at performs its own is_readable() check internally. Individual
    // reads of primitives (u8, u32, f32) are naturally aligned within the struct.
    // If the spawn is freed concurrently by EQ (rare race), reads may return
    // garbage but won't segfault because the page is still committed.
    let name = unsafe { read_string_at(spawn_ptr + player_base::NAME, 64) };
    let displayed_name = unsafe { read_string_at(spawn_ptr + player_base::DISPLAYED_NAME, 64) };
    let spawn_type = unsafe { *((spawn_ptr + player_base::TYPE) as *const u8) };
    let x = unsafe { *((spawn_ptr + player_base::X) as *const f32) };
    let y = unsafe { *((spawn_ptr + player_base::Y) as *const f32) };
    let z = unsafe { *((spawn_ptr + player_base::Z) as *const f32) };
    let heading = unsafe { *((spawn_ptr + player_base::HEADING) as *const f32) };

    // Diagnostic: log once if position looks suspicious (near-zero with valid
    // name). Log metadata only (never raw process memory or addresses).
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        static DIAG_TICK: AtomicU64 = AtomicU64::new(0);
        let tick = DIAG_TICK.fetch_add(1, Ordering::Relaxed);
        if x.abs() < 1.0 && y.abs() < 1.0 && tick.is_multiple_of(300) {
            tracing::warn!(
                name = %name,
                spawn_id,
                x, y, z,
                "DLL: position near zero — possible offset mismatch"
            );
        }
    }

    let level = unsafe { *((spawn_ptr + player_zone::LEVEL) as *const u8) };
    let class_id = unsafe { *((spawn_ptr + player_zone::CHAR_CLASS) as *const u8) };
    let hp_current = unsafe { *((spawn_ptr + player_zone::HP_CURRENT) as *const i64) };
    let hp_max = unsafe { *((spawn_ptr + player_zone::HP_MAX) as *const i64) };
    let mana_current = unsafe { *((spawn_ptr + player_zone::MANA_CURRENT) as *const i32) };
    let mana_max = unsafe { *((spawn_ptr + player_zone::MANA_MAX) as *const i32) };
    let endurance_current =
        unsafe { *((spawn_ptr + player_zone::ENDURANCE_CURRENT) as *const i32) };
    let endurance_max = unsafe { *((spawn_ptr + player_zone::ENDURANCE_MAX) as *const u32) };
    let speed_run = unsafe { *((spawn_ptr + player_base::SPEED_RUN) as *const f32) };
    let stand_state = unsafe { *((spawn_ptr + player_zone::STANDSTATE) as *const u8) };
    let is_gm = if is_readable(spawn_ptr + player_zone::GM, size_of::<u8>()) {
        unsafe { *((spawn_ptr + player_zone::GM) as *const u8) != 0 }
    } else {
        false
    };

    textquest_common::types::SpawnData {
        spawn_id,
        name,
        displayed_name,
        spawn_type,
        level,
        class_id,
        x,
        y,
        z,
        heading,
        hp_current,
        hp_max,
        mana_current,
        mana_max,
        endurance_current,
        endurance_max,
        speed_run,
        stand_state,
        is_gm,
    }
}

/// Read zone short name and long name from zoneHeader struct.
fn read_zone_names(eq_base: u64) -> (String, String) {
    use textquest_common::offsets::zone_info;

    let zone_addr = match textquest_common::offsets::rebase(zone_info::INST_EQ_ZONE_INFO, eq_base) {
        Some(a) => a,
        None => return (String::new(), String::new()),
    };

    let short = unsafe { read_string_at(zone_addr + zone_info::SHORT_NAME, 128) };
    let long = unsafe { read_string_at(zone_addr + zone_info::LONG_NAME, 128) };
    (short, long)
}

/// Read local player state. Returns None if not logged in.
fn read_local_player_state(eq_base: u64) -> Option<textquest_common::types::SpawnData> {
    let player_ptr_addr =
        textquest_common::offsets::rebase(textquest_common::offsets::PINST_LOCAL_PLAYER, eq_base)?;
    // SAFETY: player_ptr_addr is the rebased address of PINST_LOCAL_PLAYER,
    // a global pointer in eqgame.exe's data section. Reading a usize from it
    // yields the PlayerClient* for the local player (null when not logged in).
    // The address is within eqgame.exe's committed memory.
    let player_ptr = unsafe { *(player_ptr_addr as *const usize) };
    if player_ptr == 0 {
        return None;
    }

    // One-time diagnostic log of the pointer chain
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        static LOGGED: AtomicBool = AtomicBool::new(false);
        if !LOGGED.swap(true, Ordering::Relaxed) {
            tracing::info!(
                eq_base = format!("{:#x}", eq_base),
                player_ptr_addr = format!("{:#x}", player_ptr_addr),
                player_ptr = format!("{:#x}", player_ptr),
                "read_local_player_state pointer chain (one-time)"
            );
        }
    }

    Some(unsafe { read_spawn_data(player_ptr) })
}

/// Read current target state. Returns None if no target selected.
fn read_target_state(eq_base: u64) -> Option<textquest_common::types::SpawnData> {
    let target_ptr_addr =
        textquest_common::offsets::rebase(textquest_common::offsets::PINST_TARGET, eq_base)?;
    // SAFETY: target_ptr_addr is the rebased address of PINST_TARGET, a global
    // pointer in eqgame.exe. Reading a usize yields the target's PlayerClient*
    // (null when no target). read_spawn_data validates the pointer internally.
    let target_ptr = unsafe { *(target_ptr_addr as *const usize) };
    if target_ptr == 0 {
        return None;
    }
    Some(unsafe { read_spawn_data(target_ptr) })
}

/// Walk the spawn linked list and collect spawns within `max_distance` units
/// of the given position. Capped at 100 spawns.
fn read_nearby_spawns(
    eq_base: u64,
    player_x: f32,
    player_y: f32,
    player_z: f32,
) -> Vec<textquest_common::types::SpawnData> {
    use textquest_common::offsets::{player_base, spawn_manager};

    const MAX_NEARBY: usize = 100;
    const MAX_DISTANCE_SQ: f32 = 500.0 * 500.0;

    let mgr_ptr_addr = match textquest_common::offsets::rebase(
        textquest_common::offsets::PINST_SPAWN_MANAGER,
        eq_base,
    ) {
        Some(addr) => addr,
        None => return Vec::new(),
    };

    // Validate the manager pointer address is readable.
    if !is_readable(mgr_ptr_addr, size_of::<usize>()) {
        return Vec::new();
    }

    // SAFETY: mgr_ptr_addr was validated as readable above. Reading a usize
    // from PINST_SPAWN_MANAGER yields the SpawnManager* singleton pointer.
    let mgr_ptr = unsafe { *(mgr_ptr_addr as *const usize) };
    if mgr_ptr == 0 {
        return Vec::new();
    }

    // TList at spawn_manager::PLAYER_LIST, first node pointer at offset 0x00.
    let list_addr = mgr_ptr + spawn_manager::PLAYER_LIST;
    if !is_readable(list_addr, size_of::<usize>()) {
        return Vec::new();
    }
    // SAFETY: list_addr was validated as readable immediately above.
    // Reading a usize yields the head pointer of the spawn linked list.
    let mut current = unsafe { *(list_addr as *const usize) };

    let mut spawns = Vec::with_capacity(MAX_NEARBY.min(256));
    let mut walked: usize = 0;
    const MAX_WALK: usize = 2000; // Safety limit to prevent infinite loops.

    while current != 0 && spawns.len() < MAX_NEARBY && walked < MAX_WALK {
        walked += 1;

        // Validate current pointer before any dereference.
        if current % core::mem::align_of::<usize>() != 0 {
            tracing::warn!(
                ptr = format!("{:#x}", current),
                walked,
                "read_nearby_spawns: misaligned spawn pointer, aborting walk"
            );
            break;
        }
        if !is_readable(current + player_base::X, size_of::<f32>() * 3) {
            tracing::warn!(
                ptr = format!("{:#x}", current),
                walked,
                "read_nearby_spawns: unreadable spawn pointer, aborting walk"
            );
            break;
        }

        // SAFETY: `current` was validated as readable for position fields by
        // the is_readable() check above (covers X/Y/Z at known offsets).
        // Quick distance check before building full SpawnData.
        let sx = unsafe { *((current + player_base::X) as *const f32) };
        let sy = unsafe { *((current + player_base::Y) as *const f32) };
        let sz = unsafe { *((current + player_base::Z) as *const f32) };

        let dx = sx - player_x;
        let dy = sy - player_y;
        let dz = sz - player_z;
        let dist_sq = dx * dx + dy * dy + dz * dz;

        if dist_sq <= MAX_DISTANCE_SQ {
            let spawn = unsafe { read_spawn_data(current) };
            spawns.push(spawn);
        }

        // Validate NEXT pointer field is readable before following it.
        let next_addr = current + player_base::NEXT;
        if !is_readable(next_addr, size_of::<usize>()) {
            tracing::warn!(
                ptr = format!("{:#x}", current),
                walked,
                "read_nearby_spawns: NEXT pointer unreadable, aborting walk"
            );
            break;
        }
        // SAFETY: next_addr was validated as readable by is_readable() above.
        // Follow NEXT pointer in linked list.
        current = unsafe { *((next_addr) as *const usize) };
    }

    spawns
}

/// Current time in milliseconds since UNIX epoch.
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Check if our window is the foreground window. Used for render skipping —
/// background clients skip 3D rendering to save GPU/CPU.
fn update_foreground_status() {
    #[cfg(windows)]
    {
        use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::GetForegroundWindow};

        // SAFETY: GetForegroundWindow and GetWindowThreadProcessId are always
        // safe Win32 calls. GetForegroundWindow returns NULL if no window is
        // focused, which GetWindowThreadProcessId handles gracefully (returns 0).
        let fg: HWND = unsafe { GetForegroundWindow() };
        let our_pid = std::process::id();

        let mut fg_pid: u32 = 0;
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                fg,
                Some(&mut fg_pid),
            );
        }

        let is_fg = fg_pid == our_pid;
        WINDOW_IS_FOREGROUND.store(is_fg, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(not(windows))]
    {
        // Always foreground on non-Windows (dev builds).
    }
}

/// Returns true if this client's window is currently in the foreground.
/// The render hook can use this to skip `CDisplay::RealRender_World` for
/// background clients, saving near-zero GPU usage across 35 bot clients.
pub fn is_foreground() -> bool {
    WINDOW_IS_FOREGROUND.load(std::sync::atomic::Ordering::Relaxed)
}

/// Read character name + zone name from EQ memory and set the window title
/// to "[TQ] EQ - `CharName` (`ZoneName`)" so the orchestrator can identify
/// clients by PID.
fn update_window_title() {
    #[cfg(windows)]
    {
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        if eq_base == 0 {
            return;
        }

        // Read local player name from PlayerClient->Name (char[64] at offset 0xb4).
        let char_name = match read_char_name(eq_base) {
            Some(n) if !n.is_empty() => n,
            _ => return, // Not logged in yet — skip.
        };

        // Read zone name from zoneHeader struct. Prefer long name (display name like
        // "West Freeport") for readability, fall back to short name ("freportw").
        let zone_name = read_zone_long_name(eq_base)
            .or_else(|| read_zone_short_name(eq_base))
            .unwrap_or_default();

        // Build title: "[TQ] EQ - CharName (ZoneName)" or "[TQ] EQ - CharName" if no
        // zone.
        let title = if zone_name.is_empty() {
            tracing::trace!(char_name = %char_name, "Zone name empty — title without zone");
            format!("[TQ] EQ - {char_name}\0")
        } else {
            format!("[TQ] EQ - {char_name} ({zone_name})\0")
        };

        // Find our window by enumerating windows for this PID.
        let our_pid = std::process::id();
        set_window_title_for_pid(our_pid, &title);
    }

    #[cfg(not(windows))]
    {
        // No-op on non-Windows.
    }
}

/// Read the local player's Name field (char[64]) directly from EQ memory.
#[cfg(windows)]
fn read_char_name(eq_base: u64) -> Option<String> {
    use textquest_common::offsets::{self, player_base};

    let player_ptr_addr = offsets::rebase(offsets::PINST_LOCAL_PLAYER, eq_base)?;
    // SAFETY: player_ptr_addr is the rebased PINST_LOCAL_PLAYER global.
    // Dereferencing yields PlayerClient* (null when not logged in).
    let player_ptr = unsafe { *(player_ptr_addr as *const usize) };
    if player_ptr == 0 {
        return None;
    }

    let name_addr = player_ptr + player_base::NAME;
    // SAFETY: player_ptr is a live PlayerClient*, validated non-null above.
    // NAME is a char[64] field at a known offset. The 64-byte slice is within
    // the PlayerClient struct's committed memory.
    let name_bytes = unsafe { std::slice::from_raw_parts(name_addr as *const u8, 64) };
    let len = name_bytes.iter().position(|&b| b == 0).unwrap_or(64);
    String::from_utf8(name_bytes[..len].to_vec()).ok()
}

/// Read the zone short name (char[128]) from instEQZoneInfo.
#[cfg(windows)]
fn read_zone_short_name(eq_base: u64) -> Option<String> {
    use textquest_common::offsets::zone_info;

    let zone_addr = textquest_common::offsets::rebase(zone_info::INST_EQ_ZONE_INFO, eq_base)?;
    let short_name_addr = zone_addr + zone_info::SHORT_NAME;
    // SAFETY: zone_addr is the rebased instEQZoneInfo global. SHORT_NAME is a
    // char[128] field at a known offset within the zone header struct.
    let name_bytes = unsafe { std::slice::from_raw_parts(short_name_addr as *const u8, 128) };
    let len = name_bytes.iter().position(|&b| b == 0).unwrap_or(128);
    if len == 0 {
        return None;
    }
    String::from_utf8(name_bytes[..len].to_vec()).ok()
}

#[cfg(not(windows))]
fn read_zone_short_name(_eq_base: u64) -> Option<String> {
    None
}

/// Read the zone long name (char[128]) from instEQZoneInfo (e.g., "West
/// Freeport").
#[cfg(windows)]
fn read_zone_long_name(eq_base: u64) -> Option<String> {
    use textquest_common::offsets::zone_info;

    let zone_addr = textquest_common::offsets::rebase(zone_info::INST_EQ_ZONE_INFO, eq_base)?;
    let long_name_addr = zone_addr + zone_info::LONG_NAME;
    // SAFETY: zone_addr is the rebased instEQZoneInfo global. LONG_NAME is a
    // char[128] field at a known offset within the zone header struct.
    let name_bytes = unsafe { std::slice::from_raw_parts(long_name_addr as *const u8, 128) };
    let len = name_bytes.iter().position(|&b| b == 0).unwrap_or(128);
    if len == 0 {
        return None;
    }
    String::from_utf8(name_bytes[..len].to_vec()).ok()
}

#[cfg(not(windows))]
fn read_zone_long_name(_eq_base: u64) -> Option<String> {
    None
}

/// Set the window title for all top-level windows belonging to the given PID.
#[cfg(windows)]
fn set_window_title_for_pid(pid: u32, title: &str) {
    use windows::{
        Win32::{
            Foundation::{BOOL, HWND, LPARAM},
            UI::WindowsAndMessaging::{
                EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetWindowTextA,
            },
        },
        core::PCSTR,
    };

    // We use a simple callback that captures our PID + title via LPARAM.
    struct CallbackData {
        pid: u32,
        title_ptr: *const u8,
    }

    // SAFETY: Callback invoked by EnumWindows below. The LPARAM is a pointer
    // to our stack-local CallbackData which outlives the synchronous EnumWindows
    // call. The title_ptr points to a null-terminated string also on the stack.
    unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let data = &*(lparam.0 as *const CallbackData);
            let mut wnd_pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, Some(&mut wnd_pid));

            if wnd_pid == data.pid && IsWindowVisible(hwnd).as_bool() {
                let _ = SetWindowTextA(hwnd, PCSTR(data.title_ptr));
            }
            BOOL(1) // continue
        }
    }

    let data = CallbackData {
        pid,
        title_ptr: title.as_ptr(),
    };
    unsafe {
        let _ = EnumWindows(Some(enum_cb), LPARAM(&data as *const _ as isize));
    }
}

/// Dispatch a single IPC command received from the orchestrator.
fn dispatch_command(cmd: textquest_common::ipc::Command) {
    use std::borrow::Cow;
    use textquest_common::ipc::Command;

    match cmd {
        Command::SlashCommand { command } => {
            let trimmed = command.trim();
            let slash_command = match intercept_custom_slash_command(trimmed) {
                Some(InterceptedSlashCommand::ClearTarget) => {
                    tracing::info!("Intercepted /cleartarget slash command → ClearTarget");
                    dispatch_command(Command::ClearTarget);
                    return;
                }
                Some(InterceptedSlashCommand::InteractTarget) => {
                    tracing::info!(
                        cmd = trimmed,
                        "Intercepted /click ... target → InteractTarget"
                    );
                    interact_with_target();
                    return;
                }
                Some(InterceptedSlashCommand::LivingShield(target_str)) => {
                    tracing::info!("Intercepted /livingshield slash command");
                    let target_id = if target_str.is_empty() {
                        read_target_state(crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire))
                            .map(|p| p.spawn_id)
                            .unwrap_or(0)
                    } else {
                        target_str.parse::<u32>().unwrap_or_default()
                    };

                    if target_id != 0 {
                        if let Err(e) = crate::eq::send_living_shield(target_id) {
                            tracing::error!("Living shield failed: {}", e);
                        }
                    } else {
                        tracing::warn!("Invalid target for /livingshield");
                    }
                    return;
                }
                Some(InterceptedSlashCommand::Rewrite(rewritten)) => {
                    tracing::info!(from = trimmed, to = %rewritten, "Rewriting custom slash command");
                    Cow::Owned(rewritten)
                }
                None => Cow::Borrowed(trimmed),
            };
            let slash_command = slash_command.as_ref();

            if let Some(nav_waypoint_cmd) = parse_nav_waypoint_command(trimmed) {
                match nav_waypoint_cmd {
                    Ok(NavWaypointCommand::Save(name)) => match save_nav_waypoint(&name) {
                        Ok(saved) => {
                            crate::ipc::send_response(
                                textquest_common::ipc::Response::CommandResult {
                                    success: true,
                                    message: format!(
                                        "Saved waypoint '{}' in {}",
                                        saved.name, saved.zone
                                    ),
                                },
                            );
                        }
                        Err(error) => {
                            crate::ipc::send_response(
                                textquest_common::ipc::Response::CommandResult {
                                    success: false,
                                    message: error,
                                },
                            );
                        }
                    },
                    Ok(NavWaypointCommand::Recall(name)) => match recall_nav_waypoint(&name) {
                        Ok(saved) => {
                            crate::ipc::send_response(
                                textquest_common::ipc::Response::CommandResult {
                                    success: true,
                                    message: format!(
                                        "Navigating to waypoint '{}' in {}",
                                        saved.name, saved.zone
                                    ),
                                },
                            );
                        }
                        Err(error) => {
                            crate::ipc::send_response(
                                textquest_common::ipc::Response::CommandResult {
                                    success: false,
                                    message: error,
                                },
                            );
                        }
                    },
                    Ok(NavWaypointCommand::Delete(name)) => {
                        match crate::nav::waypoint_store::delete(&name) {
                            Ok(true) => {
                                crate::ipc::send_response(
                                    textquest_common::ipc::Response::CommandResult {
                                        success: true,
                                        message: format!("Deleted waypoint '{name}'"),
                                    },
                                );
                            }
                            Ok(false) => {
                                crate::ipc::send_response(
                                    textquest_common::ipc::Response::CommandResult {
                                        success: false,
                                        message: format!("Waypoint '{name}' not found"),
                                    },
                                );
                            }
                            Err(error) => {
                                crate::ipc::send_response(
                                    textquest_common::ipc::Response::CommandResult {
                                        success: false,
                                        message: error,
                                    },
                                );
                            }
                        }
                    }
                    Ok(NavWaypointCommand::List) => {
                        let waypoints = crate::nav::waypoint_store::list();
                        crate::ipc::send_response(
                            textquest_common::ipc::Response::NavWaypointList { waypoints },
                        );
                    }
                    Err(error) => {
                        crate::ipc::send_response(textquest_common::ipc::Response::CommandResult {
                            success: false,
                            message: error,
                        });
                    }
                }
                return;
            }

            if let Some(spell_set) = parse_spell_set_command(slash_command) {
                match handle_spell_set_command(spell_set) {
                    Ok(Some(eq_command)) => {
                        tracing::info!(cmd = %eq_command, "Executing translated spell-set command");
                        execute_slash_command(&eq_command);
                    }
                    Ok(None) => {
                        tracing::info!(cmd = %command, "Completed custom spell-set command");
                    }
                    Err(error) => {
                        tracing::warn!(cmd = %command, error = %error, "Spell-set command failed");
                    }
                }
                return;
            }

            if handle_casting_slash_command(trimmed) {
                tracing::info!(cmd = %command, "Queued translated /casting command");
                return;
            }

            if handle_movement_slash_command(trimmed) {
                tracing::info!(cmd = %command, "Handled movement slash command");
                return;
            }

            // When /target is issued while already targeting, EQ's InterpretCmd
            // may not switch. Clear the current target first so /target reliably
            // acquires a new one.
            if should_preclear_target_for_slash(slash_command) {
                let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
                if eq_base != 0 {
                    let has_target = read_target_state(eq_base).is_some();
                    if has_target {
                        tracing::debug!(
                            cmd = %slash_command,
                            "Pre-clearing target before target-switching slash command"
                        );
                        let controller = super::targeting::TargetingController::new(eq_base);
                        if let Err(e) = controller.clear_target() {
                            tracing::warn!(error = %e, "Failed to pre-clear target");
                        }
                    }
                }
            }

            tracing::info!(cmd = %slash_command, "Executing slash command");
            if let Some(active_bandolier) = parse_bandolier_activate_command(slash_command) {
                if let Ok(mut known_bandolier) = ACTIVE_BANDOLIER_SET.lock() {
                    *known_bandolier = Some(active_bandolier);
                }
            }
            execute_slash_command(slash_command);
        }
        Command::NavigateTo { waypoints } => {
            crate::nav::handle_command(crate::nav::NavCommand::Navigate(waypoints));
        }
        Command::SetCamp { spot } => {
            crate::nav::handle_command(crate::nav::NavCommand::SetCamp(spot));
        }
        Command::SetCampConfig { config } => {
            crate::nav::handle_command(crate::nav::NavCommand::SetCampConfig(config));
        }
        Command::StopNavigation => {
            crate::nav::handle_command(crate::nav::NavCommand::Stop);
        }
        Command::FollowPlayer {
            config,
            anchor_x,
            anchor_y,
            anchor_z,
        } => {
            tracing::info!(
                leader = %config.leader_name,
                follow_dist = config.follow_distance,
                leash_dist = config.leash_distance,
                "FollowPlayer received"
            );
            let anchor = textquest_common::nav::Waypoint::new(anchor_x, anchor_y, anchor_z);
            crate::nav::handle_command(crate::nav::NavCommand::FollowPlayer { config, anchor });
        }
        Command::UpdateFollowAnchor { x, y, z } => {
            let anchor = textquest_common::nav::Waypoint::new(x, y, z);
            crate::nav::handle_command(crate::nav::NavCommand::UpdateFollowAnchor(anchor));
        }
        Command::UpdateFollowConfig { config } => {
            tracing::info!(
                leader = %config.leader_name,
                min_delay_ms = config.min_delay_ms,
                max_delay_ms = config.max_delay_ms,
                return_no_aggro = config.return_no_aggro,
                return_not_looting = config.return_not_looting,
                "UpdateFollowConfig received"
            );
            crate::nav::handle_command(crate::nav::NavCommand::UpdateFollowConfig(config));
        }
        Command::StopFollow => {
            tracing::info!("StopFollow received");
            crate::nav::handle_command(crate::nav::NavCommand::StopFollow);
        }
        Command::MoveToAdvanced { config } => {
            tracing::info!(target_id = config.target_id, "MoveToAdvanced received");
            crate::nav::handle_command(crate::nav::NavCommand::MoveToAdvanced(config));
        }
        Command::SetAutopause { enabled } => {
            tracing::info!(enabled, "SetAutopause received");
            crate::nav::handle_command(crate::nav::NavCommand::SetAutopause(enabled));
        }
        Command::SetBreakOnGm { enabled } => {
            tracing::info!(enabled, "SetBreakOnGm received");
            crate::nav::handle_command(crate::nav::NavCommand::SetBreakOnGm(enabled));
        }
        Command::SetHeadingMode { mode } => {
            tracing::info!(?mode, "SetHeadingMode received");
            crate::nav::handle_command(crate::nav::NavCommand::SetHeadingMode(mode));
        }
        Command::StickTo { config } => {
            tracing::info!(
                hold = config.hold,
                always = config.always,
                id = config.id,
                distance_mod = config.distance_mod,
                "StickTo received"
            );
            // Read current target ID to support the `hold` modifier — when `hold` is
            // set, we lock onto whichever spawn is targeted at the moment stick starts.
            // We read it unconditionally here (it's a cheap pointer deref) and pass it
            // to the stick engine, which ignores it when an explicit `id` is configured.
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
            let current_target_id = if eq_base != 0 {
                read_target_state(eq_base).map(|t| t.spawn_id)
            } else {
                None
            };
            crate::nav::handle_command(crate::nav::NavCommand::StickTo {
                config,
                current_target_id,
            });
        }
        Command::StickOff => {
            tracing::info!("StickOff received");
            crate::nav::handle_command(crate::nav::NavCommand::StickOff);
        }
        Command::StickMod { delta } => {
            tracing::info!(delta, "StickMod received");
            crate::nav::handle_command(crate::nav::NavCommand::StickMod(delta));
        }
        Command::CircleKite { config } => {
            tracing::info!(
                radius = config.radius,
                mode = ?config.mode,
                target_id = config.target_id,
                "CircleKite received"
            );
            // Resolve the center: use config.center when specified, otherwise None
            // (the navigator will read the current player position on start).
            let center = config.center;
            crate::nav::handle_command(crate::nav::NavCommand::CircleKite { config, center });
        }
        Command::CircleOff => {
            tracing::info!("CircleOff received");
            crate::nav::handle_command(crate::nav::NavCommand::CircleOff);
        }
        Command::NavPause => {
            tracing::info!("NavPause received");
            crate::nav::handle_command(crate::nav::NavCommand::Pause);
        }
        Command::NavResume => {
            tracing::info!("NavResume received");
            crate::nav::handle_command(crate::nav::NavCommand::Resume);
        }
        Command::NavLoc { x, y, z } => {
            tracing::info!(x, y, z, "NavLoc received");
            let wp = textquest_common::nav::Waypoint::new(x, y, z);
            crate::nav::handle_command(crate::nav::NavCommand::Navigate(vec![wp]));
        }
        Command::NavTarget => {
            tracing::info!("NavTarget received");
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
            if eq_base != 0 {
                if let Some(target) = read_target_state(eq_base) {
                    let wp = textquest_common::nav::Waypoint::new(target.x, target.y, target.z);
                    crate::nav::handle_command(crate::nav::NavCommand::Navigate(vec![wp]));
                }
            }
        }
        Command::NavDoor => {
            // Navigate to nearest door: use /doortarget to select it, then navigate to
            // target. Full DoorsManager-based position lookup is an M7 feature
            // requiring additional offsets for EQSwitch/DoorsManager memory
            // layout.
            tracing::info!("NavDoor received — queuing /doortarget for nearest door");
            queue_slash_command("/doortarget".to_string());
            // After /doortarget, the door becomes the active door target (not
            // PINST_TARGET), so NavTarget-style coordinate navigation is not
            // directly available here. For now, issue InteractDoor to open the
            // nearest door in place.
            interact_with_door();
        }
        Command::NavItem => {
            // Navigate to nearest ground item — requires ground spawn list traversal.
            // Full implementation requires ground spawn type filtering (M7 feature).
            tracing::info!("NavItem received — clicking nearest ground item");
            click_nearest_object();
        }
        Command::NavReload => {
            tracing::warn!("NavReload: stub — actual mesh loading is an M7 feature");
        }
        Command::NavWaypointSave { name } => {
            tracing::info!(name = %name, "NavWaypointSave received");
            match save_nav_waypoint(&name) {
                Ok(saved) => {
                    crate::ipc::send_response(textquest_common::ipc::Response::CommandResult {
                        success: true,
                        message: format!("Saved waypoint '{}' in {}", saved.name, saved.zone),
                    });
                }
                Err(error) => {
                    crate::ipc::send_response(textquest_common::ipc::Response::CommandResult {
                        success: false,
                        message: error,
                    });
                }
            }
        }
        Command::NavWaypointRecall { name } => {
            tracing::info!(name = %name, "NavWaypointRecall received");
            match recall_nav_waypoint(&name) {
                Ok(saved) => {
                    crate::ipc::send_response(textquest_common::ipc::Response::CommandResult {
                        success: true,
                        message: format!(
                            "Navigating to waypoint '{}' in {}",
                            saved.name, saved.zone
                        ),
                    });
                }
                Err(error) => {
                    crate::ipc::send_response(textquest_common::ipc::Response::CommandResult {
                        success: false,
                        message: error,
                    });
                }
            }
        }
        Command::NavWaypointList => {
            tracing::info!("NavWaypointList received");
            crate::ipc::send_response(textquest_common::ipc::Response::NavWaypointList {
                waypoints: crate::nav::waypoint_store::list(),
            });
        }
        Command::NavWaypointDelete { name } => {
            tracing::info!(name = %name, "NavWaypointDelete received");
            match crate::nav::waypoint_store::delete(&name) {
                Ok(true) => {
                    crate::ipc::send_response(textquest_common::ipc::Response::CommandResult {
                        success: true,
                        message: format!("Deleted waypoint '{name}'"),
                    });
                }
                Ok(false) => {
                    crate::ipc::send_response(textquest_common::ipc::Response::CommandResult {
                        success: false,
                        message: format!("Waypoint '{name}' not found"),
                    });
                }
                Err(error) => {
                    crate::ipc::send_response(textquest_common::ipc::Response::CommandResult {
                        success: false,
                        message: error,
                    });
                }
            }
        }
        Command::NavSignalsQuery => {
            let signals = crate::nav::signals();
            crate::ipc::send_response(textquest_common::ipc::Response::NavSignals { signals });
        }
        Command::NavDiagnosticsQuery => {
            let diagnostics = crate::nav::diagnostics();
            crate::ipc::send_response(textquest_common::ipc::Response::NavDiagnosticsResult {
                diagnostics,
            });
        }
        Command::QueryContainerSlots { filter } => {
            tracing::info!(?filter, "QueryContainerSlots received");
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Relaxed);
            let slots = crate::eq::inventory::query_open_container_slots(eq_base, &filter);
            crate::ipc::send_response(textquest_common::ipc::Response::ContainerSlots { slots });
        }
        Command::QueryBazaarResults { filter } => {
            tracing::info!(?filter, "QueryBazaarResults received");
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Relaxed);
            let windows = crate::eq::bazaar::query_bazaar_results(eq_base, &filter);
            crate::ipc::send_response(textquest_common::ipc::Response::BazaarResults { windows });
        }
        Command::QueryContextMenu => {
            tracing::info!("QueryContextMenu received");
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Relaxed);
            let menus = crate::eq::context_menu::read_context_menus(eq_base);
            crate::ipc::send_response(textquest_common::ipc::Response::ContextMenuState { menus });
        }
        Command::ActivateContextMenuItem {
            menu_index,
            item_index,
        } => {
            tracing::info!(menu_index, item_index, "ActivateContextMenuItem received");
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Relaxed);
            let (success, message) = match crate::eq::context_menu::activate_context_menu_item(
                eq_base, menu_index, item_index,
            ) {
                Ok(()) => (true, "HandleMenu dispatched".into()),
                Err(msg) => (false, msg),
            };
            crate::ipc::send_response(textquest_common::ipc::Response::ContextMenuActivated {
                success,
                message,
            });
        }
        Command::QueryZoneGraph => {
            tracing::info!("QueryZoneGraph received");
            let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Relaxed);
            let response = unsafe { crate::nav::zone_graph::read_zone_graph(eq_base) }.map_or_else(
                || textquest_common::ipc::Response::Error {
                    message: "Failed to read zone graph from memory".into(),
                },
                |graph| {
                    let zones = crate::nav::zone_graph::zone_graph_to_ipc(&graph);
                    textquest_common::ipc::Response::ZoneGraph { zones }
                },
            );
            crate::ipc::send_response(response);
        }
        Command::Ping => {
            tracing::info!("Ping received");
        }
        Command::StartLogin {
            account_name,
            password,
            server_name,
            character_name,
        } => {
            // NOTE: Currently unreachable — handle_immediate_command() in ipc/mod.rs
            // intercepts StartLogin before it reaches PENDING_COMMANDS. The immediate
            // handler uses WM_CHAR typing which works at the login screen (before game
            // loop runs). This FSM path is preserved for future server/char selection.
            tracing::info!(
                account = %account_name,
                server = %server_name,
                character = %character_name,
                "StartLogin via game loop FSM (password redacted)"
            );
            crate::login::start_login(account_name, password, server_name, character_name);
        }
        Command::LoginPhaseQuery => {
            let phase = crate::login::phase();
            crate::ipc::send_response(textquest_common::ipc::Response::LoginPhaseUpdate { phase });
        }
        Command::CalibrateLogin => {
            tracing::info!("CalibrateLogin command received — dumping login pointers");
            let eqmain_base = crate::login::eqmain::find_eqmain();
            if eqmain_base == 0 {
                tracing::warn!("CalibrateLogin: eqmain.dll not loaded yet");
            } else {
                crate::login::widgets::calibrate_login_dump(eqmain_base);
            }
        }
        Command::CombatEngage { target_id } => {
            tracing::info!(target_id, "CombatEngage received");
            crate::combat::handle_command(crate::combat::CombatCommand::Engage { target_id });
        }
        Command::CombatDisengage => {
            tracing::info!("CombatDisengage received");
            crate::combat::handle_command(crate::combat::CombatCommand::Disengage);
        }
        Command::CombatSetAssistTarget { spawn_id } => {
            tracing::info!(spawn_id, "CombatSetAssistTarget received");
            crate::combat::handle_command(crate::combat::CombatCommand::SetAssistTarget {
                spawn_id,
            });
        }
        Command::LootCorpse => {
            tracing::info!("LootCorpse received");
            crate::combat::loot::loot_nearest_corpse();
        }
        Command::LootAll => {
            tracing::info!("LootAll received");
            crate::combat::loot::loot_all_items();
        }
        Command::SetAutoAccept { enabled } => {
            tracing::info!(enabled, "SetAutoAccept received");
            crate::dialog::set_enabled(enabled);
        }
        Command::SetAutoRezConfig { config } => {
            tracing::info!("SetAutoRezConfig received");
            crate::dialog::set_rez_config(config);
        }
        Command::SetRenderMode { mode } => {
            tracing::info!(%mode, "SetRenderMode received");
            crate::hooks::render::set_mode(mode);
            crate::ipc::send_response(textquest_common::ipc::Response::RenderModeChanged { mode });
        }
        Command::CaptureScreenshot => {
            tracing::info!("CaptureScreenshot received");
            // Guard against concurrent captures.
            if crate::hooks::render::is_capture_active() {
                crate::ipc::send_response(textquest_common::ipc::Response::ScreenshotFailed {
                    reason: "A capture is already in progress".into(),
                });
                return;
            }
            crate::hooks::render::request_capture();
            let spawn_result = std::thread::Builder::new()
                .name("textquest-screenshot".into())
                .spawn(|| {
                    for _ in 0..200 {
                        if let Some(result) = crate::hooks::dx11_null::take_capture_result() {
                            match result {
                                Ok(path) => {
                                    crate::ipc::send_response(
                                        textquest_common::ipc::Response::ScreenshotCaptured {
                                            path,
                                        },
                                    );
                                }
                                Err(reason) => {
                                    crate::ipc::send_response(
                                        textquest_common::ipc::Response::ScreenshotFailed {
                                            reason,
                                        },
                                    );
                                }
                            }
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    crate::ipc::send_response(textquest_common::ipc::Response::ScreenshotFailed {
                        reason: "Capture timed out after 2 seconds".into(),
                    });
                });
            if let Err(e) = spawn_result {
                tracing::error!(%e, "Failed to spawn screenshot polling thread");
                crate::ipc::send_response(textquest_common::ipc::Response::ScreenshotFailed {
                    reason: format!("Failed to spawn polling thread: {e}"),
                });
            }
        }
        Command::Eject => {
            tracing::info!("Eject command received — shutting down");
            crate::graceful_shutdown();
        }
        Command::SetTimingCorrection { enabled } => {
            if enabled {
                if let Err(e) = crate::hooks::timing::install() {
                    tracing::warn!("Timing hook install failed: {e}");
                }
            } else {
                crate::hooks::timing::remove();
            }
            crate::hooks::timing::set_enabled(enabled);
            tracing::info!(enabled, "SetTimingCorrection received");
        }
        Command::CastSpell {
            spell_slot,
            target_id,
            kill,
            recast,
        } => {
            tracing::info!(spell_slot, ?target_id, kill, recast, "CastSpell received");
            let tick = TICK_COUNT.load(std::sync::atomic::Ordering::Relaxed);

            if kill {
                // `kill` mode: cast in a loop until the target dies.  A
                // `CancelCastLoop` command or target death stops the loop.
                tracing::info!(spell_slot, ?target_id, "CastSpell: starting kill-loop");
                if let Ok(mut guard) = CAST_LOOP.lock() {
                    *guard = Some(CastingLoop::start_kill(spell_slot, target_id, tick));
                }
            } else if recast > 0 {
                // `recast` mode: cast `recast + 1` times with backoff.
                tracing::info!(
                    spell_slot,
                    ?target_id,
                    total = recast + 1,
                    "CastSpell: starting recast-loop"
                );
                if let Ok(mut guard) = CAST_LOOP.lock() {
                    *guard = Some(CastingLoop::start_recast(
                        spell_slot, target_id, recast, tick,
                    ));
                }
            } else {
                // Plain single cast — original behavior.
                if let Some(tid) = target_id {
                    // Save → switch → cast → restore pattern (MQ2Cast style).
                    // Queue the target switch, cast, and restore as slash commands
                    // so they execute in order on successive game frames.
                    queue_slash_command(format!("/target id {tid}"));
                    queue_slash_command(format!("/cast {spell_slot}"));
                    // Note: target restore after cast completion is the
                    // orchestrator's responsibility — it
                    // knows who the original target was and can
                    // send a follow-up /target command when the cast finishes.
                } else {
                    // Cast on current target, no swap needed.
                    queue_slash_command(format!("/cast {spell_slot}"));
                }
            }
        }
        Command::CancelCastLoop => {
            tracing::info!("CancelCastLoop received — stopping active cast loop");
            if let Ok(mut guard) = CAST_LOOP.lock() {
                if let Some(ref mut loop_state) = *guard {
                    loop_state.cancel();
                }
                *guard = None;
            }
        }
        Command::InteractTarget => {
            tracing::info!("InteractTarget received — right-clicking current target");
            interact_with_target();
        }
        Command::InteractDoor => {
            tracing::info!("InteractDoor received — targeting and opening nearest door");
            interact_with_door();
        }
        Command::ClickObject => {
            tracing::info!("ClickObject received — clicking nearest ground item");
            click_nearest_object();
        }
        Command::Relog {
            account_name,
            password,
            server_name,
            character_name,
            config,
        } => {
            tracing::info!(
                account = %account_name,
                server = %server_name,
                character = %character_name,
                "Relog command received (password redacted)"
            );
            crate::login::start_relog(account_name, password, server_name, character_name, config);
        }
        Command::CancelRelog => {
            tracing::info!("CancelRelog received");
            crate::login::cancel_relog();
        }
        Command::SwitchServer {
            server_name,
            character_name,
            account_name,
            password,
        } => {
            tracing::info!(
                server = %server_name,
                character = %character_name,
                "SwitchServer command received (password redacted)"
            );
            crate::login::switch_server(server_name, character_name, account_name, password);
        }
        Command::SwitchCharacter { character_name } => {
            tracing::info!(character = %character_name, "SwitchCharacter received");
            crate::login::switch_character(character_name);
        }
        Command::ReadMemory { address, size } => {
            let size = size.min(4096);
            let mut buf = vec![0u8; size];
            let bytes_read = {
                #[cfg(windows)]
                {
                    use windows::Win32::System::{
                        Diagnostics::Debug::ReadProcessMemory, Threading::GetCurrentProcess,
                    };
                    let mut bytes_read = 0usize;
                    let _ = unsafe {
                        ReadProcessMemory(
                            GetCurrentProcess(),
                            address as *const core::ffi::c_void,
                            buf.as_mut_ptr() as *mut core::ffi::c_void,
                            size,
                            Some(&mut bytes_read),
                        )
                    };
                    bytes_read
                }
                #[cfg(not(windows))]
                {
                    // Non-Windows stub — return zeros
                    size
                }
            };
            buf.truncate(bytes_read);
            tracing::debug!(
                address = format!("{:#x}", address),
                bytes_read,
                "ReadMemory"
            );
            crate::ipc::send_response(textquest_common::ipc::Response::MemoryData {
                address,
                bytes: buf,
            });
        }
        other => {
            tracing::debug!(?other, "Unhandled command");
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InterceptedSlashCommand {
    ClearTarget,
    InteractTarget,
    Rewrite(String),
    LivingShield(String),
}

fn intercept_custom_slash_command(command: &str) -> Option<InterceptedSlashCommand> {
    if command.eq_ignore_ascii_case("/cleartarget") {
        return Some(InterceptedSlashCommand::ClearTarget);
    }

    if is_click_right_target_command(command) {
        return Some(InterceptedSlashCommand::InteractTarget);
    }

    if let Some(rest) = command
        .strip_prefix("/livingshield")
        .or_else(|| command.strip_prefix("/LivingShield"))
    {
        return Some(InterceptedSlashCommand::LivingShield(
            rest.trim().to_string(),
        ));
    }

    rewrite_door_command(command).map(InterceptedSlashCommand::Rewrite)
}

fn rewrite_door_command(command: &str) -> Option<String> {
    let mut parts = command.splitn(2, char::is_whitespace);
    let head = parts.next().unwrap_or_default();
    if !head.eq_ignore_ascii_case("/door") {
        return None;
    }

    let rest = parts.next().map(str::trim).filter(|rest| !rest.is_empty());
    Some(match rest {
        Some(rest) => format!("/doortarget {rest}"),
        None => "/doortarget".to_string(),
    })
}

fn is_click_right_target_command(command: &str) -> bool {
    let mut parts = command.split_whitespace();
    let Some(head) = parts.next() else {
        return false;
    };
    if !head.eq_ignore_ascii_case("/click") {
        return false;
    }

    let Some(button) = parts.next() else {
        return false;
    };
    if !button.eq_ignore_ascii_case("right") {
        return false;
    }

    let Some(target) = parts.next() else {
        return false;
    };
    target.eq_ignore_ascii_case("target") && parts.next().is_none()
}

fn should_preclear_target_for_slash(command: &str) -> bool {
    let mut parts = command.splitn(2, char::is_whitespace);
    let head = parts.next().unwrap_or_default();

    if head.eq_ignore_ascii_case("/doortarget") {
        return true;
    }

    if !head.eq_ignore_ascii_case("/target") {
        return false;
    }

    parts
        .next()
        .map(str::trim)
        .is_some_and(|rest| !rest.is_empty())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SpellSetCommand {
    Save(String),
    Load(String),
    Delete(String),
}

fn parse_nav_destination_command(
    command: &str,
) -> Option<Result<textquest_common::ipc::Command, String>> {
    use textquest_common::ipc::Command;

    let tokens = match tokenize_slash_command(command) {
        Ok(tokens) => tokens,
        Err(err) => return Some(Err(err.to_string())),
    };
    let (verb, args) = tokens.split_first()?;
    if !verb.eq_ignore_ascii_case("/nav") {
        return None;
    }

    let args = if args
        .first()
        .is_some_and(|token| token.eq_ignore_ascii_case("to"))
    {
        &args[1..]
    } else {
        args
    };
    let (mode, rest) = args.split_first()?;

    if mode.eq_ignore_ascii_case("target") {
        return if rest.is_empty() {
            Some(Ok(Command::NavTarget))
        } else {
            Some(Err(
                "target navigation does not accept extra arguments".to_string()
            ))
        };
    }

    if mode.eq_ignore_ascii_case("loc") {
        if rest.len() != 3 {
            return Some(Err("loc navigation requires coordinates in `/nav loc Y X \
                             Z` order"
                .to_string()));
        }

        let parse_coord = |value: &str, axis: &str| {
            value
                .parse::<f32>()
                .map_err(|_| format!("invalid {axis} coordinate: {value}"))
        };

        let y = match parse_coord(&rest[0], "Y") {
            Ok(value) => value,
            Err(error) => return Some(Err(error)),
        };
        let x = match parse_coord(&rest[1], "X") {
            Ok(value) => value,
            Err(error) => return Some(Err(error)),
        };
        let z = match parse_coord(&rest[2], "Z") {
            Ok(value) => value,
            Err(error) => return Some(Err(error)),
        };

        return Some(Ok(Command::NavLoc { x, y, z }));
    }

    if mode.eq_ignore_ascii_case("door") {
        let valid = rest.is_empty() || (rest.len() == 1 && rest[0].eq_ignore_ascii_case("click"));
        return if valid {
            Some(Ok(Command::NavDoor))
        } else {
            None
        };
    }

    if mode.eq_ignore_ascii_case("item") {
        let valid = rest.is_empty() || (rest.len() == 1 && rest[0].eq_ignore_ascii_case("click"));
        return if valid {
            Some(Ok(Command::NavItem))
        } else {
            None
        };
    }

    None
}

fn parse_spell_set_command(command: &str) -> Option<SpellSetCommand> {
    let trimmed = command.trim();
    let body = trimmed.strip_prefix('/')?.trim_start();
    let (verb, rest) = body.split_once(char::is_whitespace)?;
    let name = rest.trim();
    if name.is_empty() {
        return None;
    }

    match verb.to_ascii_lowercase().as_str() {
        "sss" => Some(SpellSetCommand::Save(name.to_string())),
        "ssl" | "ssm" => Some(SpellSetCommand::Load(name.to_string())),
        "ssd" | "deletespellset" => Some(SpellSetCommand::Delete(name.to_string())),
        _ => None,
    }
}

fn handle_spell_set_command(command: SpellSetCommand) -> Result<Option<String>, String> {
    match command {
        SpellSetCommand::Save(name) => Ok(Some(format!("/savespellset {name}"))),
        SpellSetCommand::Load(name) => Ok(Some(format!("/memspellset {name}"))),
        SpellSetCommand::Delete(name) => delete_spell_set(&name).map(|_| None),
    }
}

/// Parse a `/circle` slash command and return the corresponding IPC `Command`.
///
/// Supported syntax (case-insensitive):
/// ```text
/// /circle on [radius] [cw|ccw|clockwise|counterclockwise|drunken|backward]
/// /circle off
/// /circle loc Y X [radius]
/// ```
fn parse_circle_command(command: &str) -> Option<textquest_common::ipc::Command> {
    use textquest_common::nav::{CircleConfig, CircleMode, Waypoint};

    let body = command.trim().strip_prefix('/')?.trim_start();
    let (verb, rest) = body
        .split_once(char::is_whitespace)
        .map(|(v, r)| (v, r.trim()))
        .unwrap_or((body, ""));

    if !verb.eq_ignore_ascii_case("circle") {
        return None;
    }

    // /circle off
    if rest.eq_ignore_ascii_case("off") || rest.is_empty() && verb.eq_ignore_ascii_case("circle") {
        if rest.eq_ignore_ascii_case("off") {
            return Some(textquest_common::ipc::Command::CircleOff);
        }
    }

    let mut tokens = rest.split_whitespace();
    let first = tokens.next().unwrap_or("");

    // /circle off
    if first.eq_ignore_ascii_case("off") {
        return Some(textquest_common::ipc::Command::CircleOff);
    }

    let mut config = CircleConfig::default();

    // /circle loc Y X [radius]
    if first.eq_ignore_ascii_case("loc") {
        let y: f32 = tokens.next()?.parse().ok()?;
        let x: f32 = tokens.next()?.parse().ok()?;
        if let Some(r_str) = tokens.next() {
            if let Ok(r) = r_str.parse::<f32>() {
                config.radius = r;
            }
        }
        config.center = Some(Waypoint::new(x, y, 0.0));
        return Some(textquest_common::ipc::Command::CircleKite { config });
    }

    // /circle on [radius] [mode]
    if first.eq_ignore_ascii_case("on") {
        // Consume optional radius
        let mut remaining: Vec<&str> = tokens.collect();
        // Check if first remaining token is a number (radius)
        if let Some(&first_r) = remaining.first() {
            if let Ok(r) = first_r.parse::<f32>() {
                config.radius = r;
                remaining.remove(0);
            }
        }
        // Parse mode tokens
        for token in &remaining {
            match token.to_ascii_lowercase().as_str() {
                "cw" | "clockwise" => config.mode = CircleMode::Cw,
                "ccw" | "counterclockwise" => config.mode = CircleMode::Ccw,
                "drunken" => config.mode = CircleMode::Drunken,
                "backward" => config.mode = CircleMode::Backward,
                _ => {}
            }
        }
        return Some(textquest_common::ipc::Command::CircleKite { config });
    }

    None
}

fn delete_spell_set(name: &str) -> Result<usize, String> {
    #[cfg(not(windows))]
    {
        let _ = name;
        Err(String::from(
            "Spell-set deletion is only available on Windows builds",
        ))
    }

    #[cfg(windows)]
    {
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        if eq_base == 0 {
            return Err(String::from("EQ base not resolved"));
        }

        let character_name = read_char_name(eq_base)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| String::from("Local character name unavailable"))?;
        let current_dir = std::env::current_dir()
            .map_err(|error| format!("Failed to resolve EQ working directory: {error}"))?;

        let mut updated_files = 0usize;
        for ini_path in spell_set_ini_candidates(&current_dir, &character_name) {
            let content = match std::fs::read_to_string(&ini_path) {
                Ok(content) => content,
                Err(error) => {
                    tracing::debug!(path = %ini_path.display(), error = %error, "Skipping unreadable spell-set ini");
                    continue;
                }
            };

            let Some(updated) = remove_spell_set_entries(&content, name) else {
                continue;
            };

            std::fs::write(&ini_path, updated).map_err(|error| {
                format!(
                    "Failed to update spell-set file {}: {error}",
                    ini_path.display()
                )
            })?;
            updated_files += 1;
        }

        if updated_files == 0 {
            Err(format!(
                "Spell set '{name}' was not found in any {character_name}_*.ini file"
            ))
        } else {
            Ok(updated_files)
        }
    }
}

fn save_nav_waypoint(name: &str) -> Result<textquest_common::nav::NamedWaypoint, String> {
    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    if eq_base == 0 {
        return Err(String::from(
            "EQ base not resolved; cannot read player position",
        ));
    }

    let Some(player) = read_local_player_state(eq_base) else {
        return Err(String::from(
            "Local player is not available — are you logged in?",
        ));
    };

    let zone = read_zone_short_name(eq_base)
        .ok_or_else(|| String::from("Zone name unavailable for waypoint save"))?;
    let position = textquest_common::nav::Waypoint::new(player.x, player.y, player.z);

    crate::nav::waypoint_store::save(name, position, zone)
}

fn recall_nav_waypoint(name: &str) -> Result<textquest_common::nav::NamedWaypoint, String> {
    let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
    if eq_base == 0 {
        return Err(String::from(
            "EQ base not resolved; cannot read current zone",
        ));
    }

    let current_zone = read_zone_short_name(eq_base)
        .ok_or_else(|| String::from("Zone name unavailable for waypoint recall"))?;

    let Some(saved) = crate::nav::waypoint_store::recall(name) else {
        return Err(format!("Waypoint '{name}' not found"));
    };

    if !saved.zone.eq_ignore_ascii_case(&current_zone) {
        return Err(format!(
            "Waypoint '{name}' is in zone {} (current zone: {})",
            saved.zone, current_zone
        ));
    }

    crate::nav::handle_command(crate::nav::NavCommand::Navigate(vec![saved.position]));
    Ok(saved)
}

fn spell_set_ini_candidates(
    current_dir: &std::path::Path,
    character_name: &str,
) -> Vec<std::path::PathBuf> {
    let prefix = format!("{character_name}_");
    let read_dir = match std::fs::read_dir(current_dir) {
        Ok(entries) => entries,
        Err(error) => {
            tracing::debug!(path = %current_dir.display(), error = %error, "Failed to enumerate spell-set ini candidates");
            return Vec::new();
        }
    };

    let mut matches = read_dir
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.len() > prefix.len() + 4
                        && name
                            .get(..prefix.len())
                            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&prefix))
                        && name
                            .get(name.len() - 4..)
                            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(".ini"))
                })
        })
        .collect::<Vec<_>>();
    matches.sort();
    matches
}

fn remove_spell_set_entries(content: &str, set_name: &str) -> Option<String> {
    use std::collections::HashSet;

    let target = set_name.trim();
    if target.is_empty() {
        return None;
    }

    let mut prefixes = HashSet::new();
    for raw_line in content.split_inclusive('\n') {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !key.starts_with("SpellLoadout") || !key.ends_with(".name") {
            continue;
        }
        if value.trim().eq_ignore_ascii_case(target) {
            prefixes.insert(key.trim_end_matches(".name").to_ascii_lowercase());
        }
    }

    if prefixes.is_empty() {
        return None;
    }

    let mut updated = String::with_capacity(content.len());
    for raw_line in content.split_inclusive('\n') {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        let key = line
            .split_once('=')
            .map(|(key, _)| key.trim())
            .unwrap_or(line.trim());
        let remove = key
            .split_once('.')
            .map(|(prefix, _)| prefixes.contains(&prefix.to_ascii_lowercase()))
            .unwrap_or(false);
        if !remove {
            updated.push_str(raw_line);
        }
    }

    Some(updated)
}

/// Call `CEverQuest::RightClickedOnPlayer(target, 0)` to open NPC interaction
/// windows.
fn interact_with_target() {
    #[cfg(windows)]
    {
        use textquest_common::offsets;

        /// Rebase an offset and read the pointer it points to, returning `None`
        /// if the rebase fails or the stored pointer is null.
        unsafe fn rebase_read_ptr(offset: u64, eq_base: u64) -> Option<usize> {
            let addr = offsets::rebase(offset, eq_base)?;
            let ptr = unsafe { std::ptr::read(addr as *const usize) };
            if ptr == 0 { None } else { Some(ptr) }
        }

        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        if eq_base == 0 {
            tracing::warn!("InteractTarget: EQ base not resolved");
            return;
        }

        let Some(eq_inst) = (unsafe { rebase_read_ptr(offsets::PINST_EVERQUEST, eq_base) }) else {
            tracing::warn!("InteractTarget: pinstCEverQuest not available");
            return;
        };
        let Some(target_ptr) = (unsafe { rebase_read_ptr(offsets::PINST_TARGET, eq_base) }) else {
            tracing::warn!("InteractTarget: no target selected");
            return;
        };
        let Some(func_addr) = offsets::rebase(offsets::RIGHT_CLICKED_ON_PLAYER, eq_base) else {
            tracing::warn!("InteractTarget: rebase RIGHT_CLICKED_ON_PLAYER failed");
            return;
        };

        type RightClickFn = unsafe extern "C" fn(this: usize, target: usize, unknown: i32);
        let func: RightClickFn = unsafe { std::mem::transmute(func_addr) };
        unsafe { func(eq_inst, target_ptr, 0) };

        tracing::info!(
            eq_inst = format!("{eq_inst:#x}"),
            target = format!("{target_ptr:#x}"),
            "RightClickedOnPlayer called"
        );
    }
    #[cfg(not(windows))]
    tracing::trace!("InteractTarget (stub)");
}

/// Interact with the nearest door or switch by queuing `/doortarget` and
/// `/click left door`.
///
/// This replicates MQ2's `/click door` behaviour:
/// 1. `/doortarget` selects the nearest `EQSwitch` in the zone.
/// 2. `/click left door` activates (opens/toggles) the selected door.
///
/// Both commands are queued so they execute on successive game-loop ticks.
fn interact_with_door() {
    tracing::info!("interact_with_door: queuing /doortarget + /click left door");
    queue_slash_command("/doortarget".to_string());
    queue_slash_command("/click left door".to_string());
}

/// Click the nearest ground item or world object by queuing `/click left item`.
///
/// This replicates MQ2's `/click item` behaviour, which activates the nearest
/// ground spawn (loose item lying in the world).
fn click_nearest_object() {
    tracing::info!("click_nearest_object: queuing /click left item");
    queue_slash_command("/click left item".to_string());
}

/// Call EQ's `InterpretCmd` to execute a slash command string.
/// `CEverQuest::InterpretCmd` is a member function:
///   void `CEverQuest::InterpretCmd(PlayerClient`* pChar, const char* szCmd)
/// On x64 Windows: this=RCX (`CEverQuest`*), pChar=RDX, szCmd=R8.
fn execute_slash_command(command: &str) {
    #[cfg(windows)]
    {
        let eq_base = crate::EQ_BASE.load(std::sync::atomic::Ordering::Acquire);
        if eq_base == 0 {
            tracing::error!("Cannot execute slash command — EQ base not resolved");
            return;
        }

        // Get the CEverQuest instance pointer (this).
        let Some(eq_inst_addr) =
            textquest_common::offsets::rebase(textquest_common::offsets::PINST_CEVERQUEST, eq_base)
        else {
            tracing::error!("Failed to rebase PINST_CEVERQUEST");
            return;
        };

        let eq_inst: *mut core::ffi::c_void =
            unsafe { *(eq_inst_addr as *const *mut core::ffi::c_void) };

        if eq_inst.is_null() {
            tracing::error!("CEverQuest instance pointer is null");
            return;
        }

        // Get the local player pointer (pChar).
        let Some(char_spawn_addr) = textquest_common::offsets::rebase(
            textquest_common::offsets::PINST_LOCAL_PLAYER,
            eq_base,
        ) else {
            tracing::error!("Failed to rebase PINST_LOCAL_PLAYER");
            return;
        };

        let player_ptr: *mut core::ffi::c_void =
            unsafe { *(char_spawn_addr as *const *mut core::ffi::c_void) };

        if player_ptr.is_null() {
            tracing::error!("Local player pointer is null — not logged in?");
            return;
        }

        // Get InterpretCmd function address.
        let Some(interpret_addr) =
            textquest_common::offsets::rebase(textquest_common::offsets::INTERPRET_CMD, eq_base)
        else {
            tracing::error!("Failed to rebase INTERPRET_CMD");
            return;
        };

        // Build null-terminated command string.
        let cmd_cstring = match std::ffi::CString::new(command) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "Invalid command string");
                return;
            }
        };

        // CEverQuest::InterpretCmd(this, PlayerClient*, const char*)
        // x64 Windows: this=RCX, pChar=RDX, szCmd=R8
        type InterpretCmdFn = unsafe extern "C" fn(
            *mut core::ffi::c_void, // this (CEverQuest*)
            *mut core::ffi::c_void, // pChar (PlayerClient*)
            *const i8,              // szCmd
        );
        let interpret_cmd: InterpretCmdFn = unsafe { std::mem::transmute(interpret_addr) };

        tracing::info!(
            addr = format!("{:#x}", interpret_addr),
            eq_inst = format!("{:?}", eq_inst),
            player = format!("{:?}", player_ptr),
            cmd = command,
            "Calling InterpretCmd"
        );

        unsafe {
            interpret_cmd(eq_inst, player_ptr, cmd_cstring.as_ptr());
        }

        tracing::info!(cmd = command, "Slash command executed");
    }

    #[cfg(not(windows))]
    {
        let _ = command;
        tracing::warn!("Slash command execution not available on this platform");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn fake_spawn(id: u32, display: &str) -> textquest_common::types::SpawnData {
        textquest_common::types::SpawnData {
            spawn_id: id,
            displayed_name: display.to_string(),
            name: display.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn spawn_delta_events_report_created_and_destroyed() {
        let previous: HashMap<u32, String> = [(1u32, "a_wolf".into()), (2, "a_bear".into())]
            .into_iter()
            .collect();

        let current = vec![fake_spawn(2, "a_bear"), fake_spawn(3, "a_ox")];
        let (next, events) =
            compute_spawn_delta_events(&previous, &current, "freportw".into(), 12345);

        assert_eq!(
            next,
            [(2u32, "a_bear".to_string()), (3u32, "a_ox".to_string())]
                .into_iter()
                .collect()
        );
        assert_eq!(events.len(), 2);
        assert_eq!(
            events.first().unwrap().kind,
            textquest_common::ipc::SpawnEventKind::Created
        );
        assert_eq!(events.first().unwrap().spawn_name, "a_ox");
        assert_eq!(
            events.get(1).unwrap().kind,
            textquest_common::ipc::SpawnEventKind::Destroyed
        );
        assert_eq!(events.get(1).unwrap().spawn_name, "a_wolf");
    }

    #[test]
    fn spawn_delta_events_with_empty_previous_emits_none() {
        let previous: HashMap<u32, String> = HashMap::new();
        let current = vec![fake_spawn(10, "a_goblin")];
        let (next, events) = compute_spawn_delta_events(&previous, &current, "freportw".into(), 1);

        assert_eq!(
            next,
            [(10u32, "a_goblin".to_string())].into_iter().collect()
        );
        assert!(
            events.is_empty(),
            "initial snapshots should be used as baseline without emission"
        );
    }

    #[test]
    fn spell_set_command_parser_supports_shortcuts() {
        assert_eq!(
            parse_spell_set_command("/sss buffs"),
            Some(SpellSetCommand::Save(String::from("buffs")))
        );
        assert_eq!(
            parse_spell_set_command("/ssl burn"),
            Some(SpellSetCommand::Load(String::from("burn")))
        );
        assert_eq!(
            parse_spell_set_command("/ssm heal set"),
            Some(SpellSetCommand::Load(String::from("heal set")))
        );
        assert_eq!(
            parse_spell_set_command("/ssd raid"),
            Some(SpellSetCommand::Delete(String::from("raid")))
        );
        assert_eq!(
            parse_spell_set_command("/deletespellset raid"),
            Some(SpellSetCommand::Delete(String::from("raid")))
        );
        assert_eq!(parse_spell_set_command("/sss   "), None);
        assert_eq!(parse_spell_set_command("/sit"), None);
    }

    #[test]
    fn nav_destination_parser_supports_target_and_object_modes() {
        assert_eq!(
            parse_nav_destination_command("/nav target"),
            Some(Ok(textquest_common::ipc::Command::NavTarget))
        );
        assert_eq!(
            parse_nav_destination_command("/nav to target"),
            Some(Ok(textquest_common::ipc::Command::NavTarget))
        );
        assert_eq!(
            parse_nav_destination_command("/nav door"),
            Some(Ok(textquest_common::ipc::Command::NavDoor))
        );
        assert_eq!(
            parse_nav_destination_command("/nav to door click"),
            Some(Ok(textquest_common::ipc::Command::NavDoor))
        );
        assert_eq!(
            parse_nav_destination_command("/nav item"),
            Some(Ok(textquest_common::ipc::Command::NavItem))
        );
        assert_eq!(
            parse_nav_destination_command("/nav to item click"),
            Some(Ok(textquest_common::ipc::Command::NavItem))
        );
    }

    #[test]
    fn nav_destination_parser_supports_mq2_loc_order() {
        assert_eq!(
            parse_nav_destination_command("/nav loc 200 100 10"),
            Some(Ok(textquest_common::ipc::Command::NavLoc {
                x: 100.0,
                y: 200.0,
                z: 10.0,
            }))
        );
        assert_eq!(
            parse_nav_destination_command("/nav to loc -25.5 13.25 7"),
            Some(Ok(textquest_common::ipc::Command::NavLoc {
                x: 13.25,
                y: -25.5,
                z: 7.0,
            }))
        );
    }

    #[test]
    fn nav_destination_parser_rejects_bad_loc_inputs() {
        assert_eq!(
            parse_nav_destination_command("/nav loc 100 200"),
            Some(Err("loc navigation requires coordinates in `/nav loc Y X \
                      Z` order"
                .to_string()))
        );
        assert_eq!(
            parse_nav_destination_command("/nav to loc 100 nope 30"),
            Some(Err("invalid X coordinate: nope".to_string()))
        );
    }

    #[test]
    fn nav_destination_parser_ignores_non_destination_nav_commands() {
        assert_eq!(parse_nav_destination_command("/nav reload"), None);
        assert_eq!(
            parse_nav_destination_command("/nav waypoint save camp"),
            None
        );
        assert_eq!(parse_nav_destination_command("/nav to guildlobby"), None);
        assert_eq!(parse_nav_destination_command("/follow tank"), None);
    }

    #[test]
    fn spell_set_ini_candidates_match_character_prefix_case_insensitively() {
        let temp = std::env::temp_dir().join(format!(
            "textquest-spellset-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp).expect("create temp dir");
        std::fs::write(temp.join("Cleric01_Teek.ini"), "").expect("write ini");
        std::fs::write(temp.join("cleric01_Test.ini"), "").expect("write ini");
        std::fs::write(temp.join("Wizard01_Teek.ini"), "").expect("write ini");
        std::fs::write(temp.join("Cleric01.txt"), "").expect("write txt");

        let matches = spell_set_ini_candidates(&temp, "cleric01");
        let names: Vec<String> = matches
            .iter()
            .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
            .map(ToOwned::to_owned)
            .collect();

        assert_eq!(names, vec!["Cleric01_Teek.ini", "cleric01_Test.ini"]);
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn spell_set_ini_candidates_ignore_multibyte_prefixes_without_panicking() {
        let temp = std::env::temp_dir().join(format!(
            "textquest-spellset-multibyte-boundary-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp).expect("create temp dir");
        std::fs::write(temp.join("€bad.ini"), "").expect("write multibyte utf8 ini");
        std::fs::write(temp.join("a_good.ini"), "").expect("write ascii ini");

        let matches = spell_set_ini_candidates(&temp, "a");
        let names: Vec<String> = matches
            .iter()
            .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
            .map(ToOwned::to_owned)
            .collect();

        assert_eq!(names, vec!["a_good.ini"]);
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn intercepts_cleartarget_slash_command() {
        assert_eq!(
            intercept_custom_slash_command("/cleartarget"),
            Some(InterceptedSlashCommand::ClearTarget)
        );
        assert_eq!(
            intercept_custom_slash_command("/ClearTarget"),
            Some(InterceptedSlashCommand::ClearTarget)
        );
    }

    #[test]
    fn intercepts_click_right_target_slash_command() {
        assert_eq!(
            intercept_custom_slash_command("/click right target"),
            Some(InterceptedSlashCommand::InteractTarget)
        );
        assert_eq!(
            intercept_custom_slash_command("/CLICK RIGHT TARGET"),
            Some(InterceptedSlashCommand::InteractTarget)
        );
        assert_eq!(intercept_custom_slash_command("/click left target"), None);
        assert_eq!(intercept_custom_slash_command("/click right door"), None);
    }

    #[test]
    fn rewrites_door_slash_command_to_doortarget() {
        assert_eq!(
            intercept_custom_slash_command("/door"),
            Some(InterceptedSlashCommand::Rewrite("/doortarget".into()))
        );
        assert_eq!(
            intercept_custom_slash_command("/door id 7"),
            Some(InterceptedSlashCommand::Rewrite("/doortarget id 7".into()))
        );
        assert_eq!(
            intercept_custom_slash_command("/DOOR   wooden gate"),
            Some(InterceptedSlashCommand::Rewrite(
                "/doortarget wooden gate".into()
            ))
        );
    }

    #[test]
    fn preclears_for_target_and_doortarget_commands() {
        assert!(should_preclear_target_for_slash("/target Emperor Crush"));
        assert!(!should_preclear_target_for_slash("/target"));
        assert!(should_preclear_target_for_slash("/doortarget"));
        assert!(should_preclear_target_for_slash("/doortarget id 5"));
        assert!(!should_preclear_target_for_slash("/nav target"));
        assert!(!should_preclear_target_for_slash("/click right target"));
    }

    #[test]
    fn remove_spell_set_entries_deletes_matching_loadout_block() {
        let content = concat!(
            "[SpellLoadouts]\r\n",
            "SpellLoadout1.inuse=1\r\n",
            "SpellLoadout1.name=Buffs\r\n",
            "SpellLoadout1.slot1=123\r\n",
            "SpellLoadout2.inuse=1\r\n",
            "SpellLoadout2.name=Burn\r\n",
            "SpellLoadout2.slot1=456\r\n",
        );

        let updated = remove_spell_set_entries(content, "buffs").expect("updated content");

        assert!(!updated.contains("SpellLoadout1.inuse=1"));
        assert!(!updated.contains("SpellLoadout1.name=Buffs"));
        assert!(!updated.contains("SpellLoadout1.slot1=123"));
        assert!(updated.contains("SpellLoadout2.name=Burn"));
    }

    #[test]
    fn test_human_jitter_bounds() {
        let mut rng = textquest_common::nav::Xorshift32::new(12345);
        for _ in 0..10_000 {
            let ticks = human_jitter_ticks(&mut rng);
            // Base: 2-10, hesitation adds 5-19 (5% chance)
            // Min = 2, max = 10 + 19 = 29
            assert!(ticks >= 2, "jitter {ticks} below minimum 2");
            assert!(ticks <= 29, "jitter {ticks} above maximum 29");
        }
    }

    #[test]
    fn test_human_jitter_distribution_peaks_at_center() {
        let mut rng = textquest_common::nav::Xorshift32::new(42);
        let mut counts = [0u32; 30]; // indices 0-29

        for _ in 0..100_000 {
            let ticks = human_jitter_ticks(&mut rng) as usize;
            counts[ticks] += 1;
        }

        // The triangle distribution peaks at 6 (most common without hesitation).
        // Verify tick 6 is the most common value in the 2-10 range.
        let peak = counts[2..=10]
            .iter()
            .enumerate()
            .max_by_key(|(_, c)| *c)
            .map(|(i, _)| i + 2)
            .unwrap();
        assert_eq!(peak, 6, "Expected peak at 6 ticks, got {peak}");
    }

    #[test]
    fn test_human_jitter_produces_hesitation_spikes() {
        let mut rng = textquest_common::nav::Xorshift32::new(99);
        let mut saw_spike = false;
        for _ in 0..10_000 {
            let ticks = human_jitter_ticks(&mut rng);
            if ticks > 10 {
                saw_spike = true;
                break;
            }
        }
        assert!(
            saw_spike,
            "Expected at least one hesitation spike > 10 in 10000 draws"
        );
    }

    #[test]
    fn parse_stick_slash_defaults_to_basic_start() {
        let parsed = parse_movement_slash_command("/stick").unwrap().unwrap();
        assert_eq!(
            parsed,
            MovementSlashCommand::Stick(StickSlashCommand::Start(
                textquest_common::nav::StickConfig::default(),
            ))
        );
    }

    #[test]
    fn parse_stick_slash_supports_distance_mode_and_flags() {
        let parsed = parse_movement_slash_command("/stick 18 behind hold moveback")
            .unwrap()
            .unwrap();
        match parsed {
            MovementSlashCommand::Stick(StickSlashCommand::Start(config)) => {
                assert_eq!(
                    config.distance,
                    textquest_common::nav::StickDistance::Absolute(18.0)
                );
                assert_eq!(config.mode, textquest_common::nav::StickMode::Behind);
                assert!(config.hold);
                assert!(config.moveback);
            }
            other => panic!("expected stick start, got {other:?}"),
        }
    }

    #[test]
    fn parse_stick_slash_supports_percent_and_mod() {
        let percent = parse_movement_slash_command("/stick 80% !front")
            .unwrap()
            .unwrap();
        match percent {
            MovementSlashCommand::Stick(StickSlashCommand::Start(config)) => {
                assert_eq!(
                    config.distance,
                    textquest_common::nav::StickDistance::Percent(80.0)
                );
                assert_eq!(config.mode, textquest_common::nav::StickMode::NotFront);
            }
            other => panic!("expected stick start, got {other:?}"),
        }

        let delta = parse_movement_slash_command("/stick mod -5")
            .unwrap()
            .unwrap();
        assert_eq!(
            delta,
            MovementSlashCommand::Stick(StickSlashCommand::Mod(-5.0))
        );
    }

    #[test]
    fn parse_follow_slash_supports_named_and_off_modes() {
        let follow = parse_movement_slash_command(r#"/follow "Main Tank""#)
            .unwrap()
            .unwrap();
        assert_eq!(
            follow,
            MovementSlashCommand::Follow(FollowSlashCommand::Start {
                leader_name: Some("Main Tank".to_string()),
            })
        );

        let off = parse_movement_slash_command("/follow off")
            .unwrap()
            .unwrap();
        assert_eq!(off, MovementSlashCommand::Follow(FollowSlashCommand::Off));
    }

    #[test]
    fn resolve_follow_spawn_prefers_current_target_then_nearby() {
        let current = textquest_common::types::SpawnData {
            spawn_id: 1,
            name: "Camrene".into(),
            displayed_name: "Camrene".into(),
            ..Default::default()
        };
        let nearby = vec![textquest_common::types::SpawnData {
            spawn_id: 2,
            name: "Derakor".into(),
            displayed_name: "Derakor".into(),
            ..Default::default()
        }];

        assert_eq!(
            resolve_follow_spawn(Some(&current), &nearby, None).map(|spawn| spawn.spawn_id),
            Some(1)
        );
        assert_eq!(
            resolve_follow_spawn(Some(&current), &nearby, Some("Derakor"))
                .map(|spawn| spawn.spawn_id),
            Some(2)
        );
    }

    #[test]
    fn parse_casting_spell_with_targetid_and_invis_guard() {
        let parsed = parse_casting_command(r#"/casting "Complete Heal" gem1 -targetid|42 -invis"#)
            .unwrap()
            .unwrap();
        assert_eq!(
            parsed,
            ParsedCastingCommand {
                action: CastingAction::CastGem(1),
                target_id: Some(42),
                require_not_invisible: true,
                bandolier_set: None,
            }
        );
    }

    #[test]
    fn parse_casting_item_by_name() {
        let parsed = parse_casting_command(r#"/casting "Fungi Tunic" item"#)
            .unwrap()
            .unwrap();
        assert_eq!(
            parsed,
            ParsedCastingCommand {
                action: CastingAction::UseItem(r#""Fungi Tunic""#.to_string()),
                target_id: None,
                require_not_invisible: false,
                bandolier_set: None,
            }
        );
    }

    #[test]
    fn parse_casting_item_by_slot_selector() {
        let parsed = parse_casting_command(r#"/casting "Clicky" LeftEar -targetid|99"#)
            .unwrap()
            .unwrap();
        assert_eq!(
            parsed,
            ParsedCastingCommand {
                action: CastingAction::UseItem("leftear".to_string()),
                target_id: Some(99),
                require_not_invisible: false,
                bandolier_set: None,
            }
        );
    }

    #[test]
    fn parse_casting_numeric_slot_selector() {
        let parsed = parse_casting_command(r#"/casting "Clicky" 13"#)
            .unwrap()
            .unwrap();
        assert_eq!(
            parsed,
            ParsedCastingCommand {
                action: CastingAction::UseItem("13".to_string()),
                target_id: None,
                require_not_invisible: false,
                bandolier_set: None,
            }
        );
    }

    #[test]
    fn parse_casting_bandolier_option_with_spaces() {
        let parsed = parse_casting_command(r#"/casting "Fungi Tunic" item -bandolier|"Heal Set""#)
            .unwrap()
            .unwrap();
        assert_eq!(
            parsed,
            ParsedCastingCommand {
                action: CastingAction::UseItem(r#""Fungi Tunic""#.to_string()),
                target_id: None,
                require_not_invisible: false,
                bandolier_set: Some("Heal Set".to_string()),
            }
        );
    }

    #[test]
    fn parse_casting_rejects_invalid_targetid() {
        let err = parse_casting_command(r#"/casting "Clicky" item -targetid|abc"#)
            .unwrap()
            .unwrap_err();
        assert!(err.contains("invalid -targetid value"));
    }

    #[test]
    fn parse_casting_rejects_unsupported_type() {
        let err = parse_casting_command(r#"/casting "Harm Touch" alt"#)
            .unwrap()
            .unwrap_err();
        assert!(err.contains("unsupported /casting type"));
    }

    #[test]
    fn invisibility_matcher_ignores_see_invis_but_matches_camouflage() {
        assert!(!spell_name_indicates_invisibility("See Invisible"));
        assert!(!spell_name_indicates_invisibility(
            "Invisibility to Animals"
        ));
        assert!(spell_name_indicates_invisibility("Camouflage"));
        assert!(spell_name_indicates_invisibility("Improved Invisibility"));
    }

    #[test]
    fn parse_nav_waypoint_supports_save_and_recall() {
        let save = parse_nav_waypoint_command("/nav waypoint save Camp1")
            .unwrap()
            .unwrap();
        assert_eq!(save, NavWaypointCommand::Save(String::from("Camp1")));

        let recall = parse_nav_waypoint_command("/nav waypoint Camp1")
            .unwrap()
            .unwrap();
        assert_eq!(recall, NavWaypointCommand::Recall(String::from("Camp1")));

        let recall_kw = parse_nav_waypoint_command("/nav waypoint recall camp2")
            .unwrap()
            .unwrap();
        assert_eq!(recall_kw, NavWaypointCommand::Recall(String::from("camp2")));

        let record = parse_nav_waypoint_command("/nav recordwaypoint pull_spot tag")
            .unwrap()
            .unwrap();
        assert_eq!(record, NavWaypointCommand::Save(String::from("pull_spot")));
    }

    #[test]
    fn parse_nav_waypoint_handles_list_and_delete() {
        let list = parse_nav_waypoint_command("/nav wp list").unwrap().unwrap();
        assert_eq!(list, NavWaypointCommand::List);

        let delete = parse_nav_waypoint_command("/nav waypoint delete camp1")
            .unwrap()
            .unwrap();
        assert_eq!(delete, NavWaypointCommand::Delete(String::from("camp1")));

        let missing = parse_nav_waypoint_command("/nav waypoint")
            .unwrap()
            .unwrap_err();
        assert!(missing.contains("Waypoint name is required"));
    }

    #[test]
    fn tokenize_slash_command_preserves_escaped_quotes_inside_quotes() {
        let tokens = tokenize_slash_command(r#"/casting "Item \"Name\"" item"#).unwrap();
        assert_eq!(tokens, vec!["/casting", r#"Item "Name""#, "item"]);
    }

    #[test]
    fn parse_bandolier_activate_command_preserves_named_sets() {
        assert_eq!(
            parse_bandolier_activate_command(r#"/bandolier activate "Heal Set""#),
            Some("Heal Set".to_string())
        );
        assert_eq!(
            parse_bandolier_activate_command("/bandolier activate 2"),
            Some("2".to_string())
        );
    }

    #[test]
    fn next_bandolier_restore_command_waits_for_cast_completion() {
        let mut pending = Some(PendingBandolierRestore {
            requested_set: "Heal Set".to_string(),
            restore_to: Some("Melee".to_string()),
            deadline_tick: 30,
            saw_casting: false,
        });

        assert_eq!(
            next_bandolier_restore_command(&mut pending, 5, Some(true)),
            None
        );
        assert!(pending.as_ref().is_some_and(|state| state.saw_casting));
        assert_eq!(
            next_bandolier_restore_command(&mut pending, 6, Some(false)),
            Some(r#"/bandolier activate "Melee""#.to_string())
        );
        assert!(pending.is_none());
    }

    #[test]
    fn next_bandolier_restore_command_rolls_back_after_timeout_without_cast() {
        let mut pending = Some(PendingBandolierRestore {
            requested_set: "Heal Set".to_string(),
            restore_to: Some("Melee".to_string()),
            deadline_tick: 10,
            saw_casting: false,
        });

        assert_eq!(
            next_bandolier_restore_command(&mut pending, 10, None),
            Some(r#"/bandolier activate "Melee""#.to_string())
        );
        assert!(pending.is_none());
    }
}
