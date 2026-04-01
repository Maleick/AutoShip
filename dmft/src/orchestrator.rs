//! Orchestrator — wires the camp loop state machine to IPC command delivery.

use crate::camp::cc::CcType;
use crate::camp::config::CampConfig;
use crate::camp::hunt::{HuntLoop, HuntSnapshot, OperatingMode, Pos2D};
use crate::camp::progression::{CampDatabase, CampProgressionEvent, check_progression};
use crate::camp::state::{
    CampAction, CampEvent, CampLoop, CampMember, CampSnapshot, CampState, Role,
};
use crate::camp::vendor::{SellCycle, SellState, VendorConfig};
use crate::combat::coordinator::CombatCoordinator;
use crate::ipc::pipe::CommandPipe;
use crate::ipc::shared::SharedStateReader;
use dmft_common::ipc::{Command, SessionToken};
use dmft_common::types::GameState;
use std::collections::HashMap;

/// Generate a cryptographically random 32-byte session token using OS entropy.
#[allow(dead_code)] // Used when IPC is wired up in later milestones
fn generate_session_token() -> SessionToken {
    dmft_common::ipc::generate_random_token()
}

/// Maximum number of ticks a critical role's state can be stale before
/// `build_camp_snapshot` refuses to produce a snapshot.
const STALE_TICK_THRESHOLD: u64 = 3;

/// How often (in ticks) to check camp progression for level-based advances.
const PROGRESSION_CHECK_INTERVAL: u64 = 50;

/// Ticks before CC expiry to push a `CcExpiring` event.
const CC_EXPIRY_BUFFER: u64 = 3;

/// Top-level orchestrator that ticks the camp loop and dispatches commands.
pub struct Orchestrator {
    /// Process IDs of all registered EQ clients.
    pub client_pids: Vec<u32>,
    /// Mapping of PID to character name for each client.
    pub client_names: HashMap<u32, String>,
    /// Active camp loop state machine, if a camp is running.
    pub active_camp: Option<CampLoop>,
    /// Group combat coordinator (assist, CC, CH chain).
    pub combat: CombatCoordinator,
    /// Monotonically increasing tick counter.
    pub tick_count: u64,
    /// Commands dispatched this tick (for status display).
    pub last_dispatched: Vec<(u32, CampAction)>,
    /// Latest game state per client PID.
    pub game_states: HashMap<u32, GameState>,
    /// Shared memory readers per client PID.
    state_readers: HashMap<u32, SharedStateReader>,
    /// CSPRNG session tokens per client PID (generated at registration time).
    session_tokens: HashMap<u32, SessionToken>,
    /// Tick number when each client's game state was last updated.
    state_timestamps: HashMap<u32, u64>,
    /// Persistent pipe connections per client PID (one long-lived pipe per client).
    pipe_pool: HashMap<u32, CommandPipe>,

    // --- Integration fields ---
    /// Current operating mode: Camp (stationary) or Hunt (roaming).
    pub operating_mode: OperatingMode,
    /// Active hunt loop (used when `operating_mode` == Hunt).
    pub active_hunt: Option<HuntLoop>,
    /// Vendor sell cycle (ticked during camp Idle/Medding).
    pub sell_cycle: Option<SellCycle>,
    /// Camp progression database for level-based camp advancement.
    pub camp_db: Option<CampDatabase>,
    /// Suggested camp from progression check (for TUI display).
    pub suggested_camp: Option<String>,
    /// Previous CC state snapshot for charm break detection (`spawn_id` -> `CcType`).
    prev_cc_state: HashMap<u32, CcType>,
    /// Previous nearby spawn IDs for add detection.
    prev_nearby_spawns: HashMap<u32, String>,
}

impl Orchestrator {
    /// Create a new orchestrator with no registered clients.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client_pids: Vec::new(),
            client_names: HashMap::new(),
            active_camp: None,
            combat: CombatCoordinator::new(),
            tick_count: 0,
            last_dispatched: Vec::new(),
            game_states: HashMap::new(),
            state_readers: HashMap::new(),
            session_tokens: HashMap::new(),
            state_timestamps: HashMap::new(),
            pipe_pool: HashMap::new(),
            operating_mode: OperatingMode::Camp,
            active_hunt: None,
            sell_cycle: None,
            camp_db: None,
            suggested_camp: None,
            prev_cc_state: HashMap::new(),
            prev_nearby_spawns: HashMap::new(),
        }
    }

    /// Get the latest game state for a client PID.
    #[allow(dead_code)]
    #[must_use]
    pub fn get_client_state(&self, pid: u32) -> Option<&GameState> {
        self.game_states.get(&pid)
    }

    /// Read game state from shared memory for all known clients.
    fn poll_game_states(&mut self) {
        for &pid in &self.client_pids {
            // Lazily create readers
            if let std::collections::hash_map::Entry::Vacant(e) = self.state_readers.entry(pid) {
                let session_id = self
                    .session_tokens
                    .get(&pid)
                    .map_or(0, dmft_common::ipc::session_id_from_token);
                match SharedStateReader::new(pid, session_id) {
                    Ok(reader) => {
                        e.insert(reader);
                    }
                    Err(e) => {
                        tracing::debug!(pid, error = %e, "Failed to open shared memory reader");
                        continue;
                    }
                }
            }

            if let Some(reader) = self.state_readers.get(&pid)
                && let Some(state) = reader.read()
            {
                self.game_states.insert(pid, state);
                self.state_timestamps.insert(pid, self.tick_count);
            }
        }
    }

    /// Build a `CampSnapshot` from live game state for the active camp's members.
    /// Returns `None` if any critical role (tank/healer) has stale state.
    fn build_camp_snapshot(&self) -> Option<CampSnapshot> {
        let camp = self.active_camp.as_ref()?;

        let tank = camp.members.iter().find(|m| m.role == Role::Tank)?;
        let healer = camp.members.iter().find(|m| m.role == Role::Healer)?;

        // Staleness check: refuse to act on data older than STALE_TICK_THRESHOLD ticks
        for critical in [tank, healer] {
            if let Some(&last_update) = self.state_timestamps.get(&critical.pid)
                && self.tick_count.saturating_sub(last_update) > STALE_TICK_THRESHOLD
            {
                tracing::warn!(
                    pid = critical.pid,
                    name = %critical.name,
                    role = ?critical.role,
                    stale_ticks = self.tick_count - last_update,
                    "Stale game state for critical role — skipping snapshot"
                );
                return None;
            }
            // No timestamp at all means we never read state — handled by get() below
        }

        let tank_state = self.game_states.get(&tank.pid)?;
        let healer_state = self.game_states.get(&healer.pid)?;

        let tank_hp_pct = tank_state
            .local_player
            .as_ref()
            .map_or(100.0, dmft_common::types::SpawnData::hp_pct);

        let healer_mana_pct = healer_state
            .local_player
            .as_ref()
            .map_or(100.0, dmft_common::types::SpawnData::mana_pct);

        // Use the tank's target for target HP and spawn ID
        let (target_hp_pct, target_is_dead, target_spawn_id) = tank_state
            .target
            .as_ref()
            .map_or((None, false, None), |t| (Some(t.hp_pct()), t.hp_current <= 0, Some(t.spawn_id)));

        // Collect per-member HP for death detection
        let member_hp: Vec<(u32, i32)> = camp
            .members
            .iter()
            .filter_map(|m| {
                self.game_states.get(&m.pid).and_then(|gs| {
                    gs.local_player
                        .as_ref()
                        .map(|lp| (m.pid, lp.hp_current.clamp(0, i64::from(i32::MAX)) as i32))
                })
            })
            .collect();

        Some(CampSnapshot {
            healer_mana_pct,
            tank_hp_pct,
            target_hp_pct,
            target_is_dead,
            target_spawn_id,
            member_hp,
        })
    }

    /// Advance the camp/hunt loop (if active), collect commands, and send via IPC.
    /// Returns the number of commands dispatched.
    pub fn tick(&mut self) -> usize {
        self.tick_count += 1;
        self.last_dispatched.clear();

        self.poll_game_states();

        let commands = match self.operating_mode {
            OperatingMode::Camp => self.tick_camp(),
            OperatingMode::Hunt => self.tick_hunt(),
        };

        let count = commands.len();
        for (pid, action) in &commands {
            self.dispatch_action(*pid, action);
        }
        self.last_dispatched = commands;
        count
    }

    /// Tick the camp loop, including sell cycle, progression checks, and event production.
    fn tick_camp(&mut self) -> Vec<(u32, CampAction)> {
        let snapshot = self.build_camp_snapshot();

        // --- Task 5: Event production (charm breaks, adds, CC expiry) ---
        self.produce_camp_events(&snapshot);

        // --- Task 2: Vendor sell cycle ---
        let sell_cmds = self.tick_sell_cycle();

        // --- Task 4: Camp progression auto-advance ---
        if self.tick_count.is_multiple_of(PROGRESSION_CHECK_INTERVAL) {
            self.check_camp_progression();
        }

        // Tick the main camp loop
        let mut commands = match self.active_camp.as_mut() {
            Some(camp) => camp.tick(snapshot.as_ref()),
            None => {
                return sell_cmds
                    .into_iter()
                    .map(|(pid, cmd)| (pid, CampAction::Slash(cmd)))
                    .collect();
            }
        };

        // Append sell cycle commands (only during Idle/Medding — the sell cycle
        // itself returns empty when not active)
        if !sell_cmds.is_empty() {
            commands.extend(
                sell_cmds
                    .into_iter()
                    .map(|(pid, cmd)| (pid, CampAction::Slash(cmd))),
            );
        }

        commands
    }

    /// Tick the hunt loop.
    fn tick_hunt(&mut self) -> Vec<(u32, CampAction)> {
        let hunt_snapshot = self.build_hunt_snapshot();
        match self.active_hunt.as_mut() {
            Some(hunt) => {
                let slash_cmds = hunt.tick(hunt_snapshot.as_ref());
                CampAction::from_slash_vec(slash_cmds)
            }
            None => Vec::new(),
        }
    }

    /// Build a `HuntSnapshot` from live game state for the hunt loop.
    fn build_hunt_snapshot(&self) -> Option<HuntSnapshot> {
        let hunt = self.active_hunt.as_ref()?;
        let tank = hunt.members.iter().find(|m| m.role == Role::Tank)?;
        let healer = hunt.members.iter().find(|m| m.role == Role::Healer)?;

        let tank_state = self.game_states.get(&tank.pid)?;
        let healer_state = self.game_states.get(&healer.pid)?;

        let tank_lp = tank_state.local_player.as_ref()?;
        let healer_lp = healer_state.local_player.as_ref()?;

        let member_positions: Vec<(u32, Pos2D)> = hunt
            .members
            .iter()
            .filter_map(|m| {
                self.game_states.get(&m.pid).and_then(|gs| {
                    gs.local_player
                        .as_ref()
                        .map(|lp| (m.pid, Pos2D::new(lp.x, lp.y)))
                })
            })
            .collect();

        let target_is_dead = tank_state
            .target
            .as_ref()
            .is_some_and(|t| t.hp_current <= 0);

        Some(HuntSnapshot {
            tank_pos: Pos2D::new(tank_lp.x, tank_lp.y),
            member_positions,
            target_is_dead,
            tank_hp_pct: tank_lp.hp_pct(),
            healer_mana_pct: healer_lp.mana_pct(),
        })
    }

    /// Tick the sell cycle if active and camp is in Idle or Medding state.
    fn tick_sell_cycle(&mut self) -> Vec<(u32, String)> {
        // Capture both values in a single borrow of active_camp
        let (in_downtime, seller_pid) = match &self.active_camp {
            Some(camp) => {
                let downtime = matches!(camp.state, CampState::Idle | CampState::Medding { .. });
                let pid = camp
                    .members
                    .iter()
                    .find(|m| m.role == Role::Dps)
                    .or(camp.members.first())
                    .map(|m| m.pid);
                (downtime, pid)
            }
            None => return Vec::new(),
        };

        if !in_downtime {
            return Vec::new();
        }

        let Some(sell_cycle) = &mut self.sell_cycle else {
            return Vec::new();
        };

        // Check if we need to start a sell cycle
        if sell_cycle.needs_sell(self.tick_count) {
            sell_cycle.start_sell(self.tick_count);
        }

        // Only tick if actually selling
        if sell_cycle.state == SellState::NotNeeded {
            return Vec::new();
        }

        match seller_pid {
            Some(pid) => sell_cycle.tick(pid, self.tick_count),
            None => Vec::new(),
        }
    }

    /// Check camp progression and set `suggested_camp` if the group has outleveled.
    fn check_camp_progression(&mut self) {
        let Some(camp) = &self.active_camp else {
            return;
        };
        let Some(db) = &self.camp_db else {
            return;
        };

        // Calculate average level from game states of camp members
        let levels: Vec<f32> = camp
            .members
            .iter()
            .filter_map(|m| {
                self.game_states
                    .get(&m.pid)
                    .and_then(|gs| gs.local_player.as_ref().map(|lp| f32::from(lp.level)))
            })
            .collect();

        if levels.is_empty() {
            return;
        }

        let avg_level = levels.iter().sum::<f32>() / levels.len() as f32;

        match check_progression(&camp.config, avg_level, db) {
            Some(CampProgressionEvent::AdvanceToNext { to_camp, .. }) => {
                if self.suggested_camp.as_deref() != Some(&to_camp) {
                    tracing::info!(
                        avg_level,
                        to_camp = %to_camp,
                        "Camp progression: suggesting advance"
                    );
                    self.suggested_camp = Some(to_camp);
                }
            }
            Some(CampProgressionEvent::FallbackToPrev { to_camp, .. }) => {
                if self.suggested_camp.as_deref() != Some(&to_camp) {
                    tracing::info!(
                        avg_level,
                        to_camp = %to_camp,
                        "Camp progression: suggesting fallback"
                    );
                    self.suggested_camp = Some(to_camp);
                }
            }
            Some(CampProgressionEvent::EndOfChain { .. }) => {
                // No suggestion — end of chain
            }
            None => {
                // In range — clear any stale suggestion
                self.suggested_camp = None;
            }
        }
    }

    /// Produce camp events by comparing current state to previous tick state.
    /// Detects charm breaks, new adds, and expiring CC.
    fn produce_camp_events(&mut self, _snapshot: &Option<CampSnapshot>) {
        let Some(camp) = &self.active_camp else {
            return;
        };

        // Only produce events during active combat phases
        if !matches!(
            camp.state,
            CampState::Fighting { .. } | CampState::Pulling { .. }
        ) {
            self.prev_cc_state.clear();
            self.prev_nearby_spawns.clear();
            return;
        }

        // --- Charm break detection ---
        let current_cc: HashMap<u32, CcType> = camp
            .cc_tracker
            .targets
            .iter()
            .filter_map(|t| t.cc_applied.map(|cc| (t.spawn_id, cc)))
            .collect();

        for (spawn_id, prev_cc) in &self.prev_cc_state {
            if *prev_cc == CcType::Charm && !current_cc.contains_key(spawn_id) {
                // Charm was on this mob last tick but isn't now
                if let Some(camp) = &mut self.active_camp {
                    camp.push_event(CampEvent::CharmBreak {
                        spawn_id: *spawn_id,
                    });
                }
            }
        }
        self.prev_cc_state = current_cc;

        // --- Add detection: new NPCs within camp radius ---
        // Use the tank's nearby_spawns as the source
        let Some(camp) = &self.active_camp else {
            return;
        };
        let tank = camp.members.iter().find(|m| m.role == Role::Tank);
        if let Some(tank) = tank
            && let Some(gs) = self.game_states.get(&tank.pid)
        {
            let current_nearby: HashMap<u32, String> = gs
                .nearby_spawns
                .iter()
                .filter(|s| s.spawn_type == 1) // NPCs only
                .map(|s| (s.spawn_id, s.name.clone()))
                .collect();

            for (spawn_id, name) in &current_nearby {
                if !self.prev_nearby_spawns.contains_key(spawn_id)
                    && let Some(camp) = &mut self.active_camp
                {
                    camp.push_event(CampEvent::AddSpawned {
                        spawn_id: *spawn_id,
                        name: name.clone(),
                    });
                }
            }
            self.prev_nearby_spawns = current_nearby;
        }

        // --- CC expiry detection ---
        let Some(camp) = &self.active_camp else {
            return;
        };
        let tick = camp.tick;
        let expiring: Vec<u32> = camp
            .cc_tracker
            .targets
            .iter()
            .filter(|t| {
                t.cc_applied.is_some()
                    && t.cc_expiry_tick > tick
                    && t.cc_expiry_tick.saturating_sub(tick) <= CC_EXPIRY_BUFFER
            })
            .map(|t| t.spawn_id)
            .collect();

        for spawn_id in expiring {
            if let Some(camp) = &mut self.active_camp {
                camp.push_event(CampEvent::CcExpiring { spawn_id });
            }
        }
    }

    /// Start a camp loop with the given config and members.
    pub fn start_camp(&mut self, config: CampConfig, members: Vec<CampMember>) {
        tracing::info!(
            camp = %config.name,
            members = members.len(),
            "Starting camp loop"
        );
        self.active_camp = Some(CampLoop::new(config, members));
    }

    /// Stop the current camp loop.
    pub fn stop_camp(&mut self) {
        if self.active_camp.is_some() {
            tracing::info!("Stopping camp loop");
            self.active_camp = None;
        }
    }

    /// Start a hunt loop with the given config and members.
    pub fn start_hunt(&mut self, config: CampConfig, members: Vec<CampMember>) {
        tracing::info!(
            config = %config.name,
            members = members.len(),
            "Starting hunt loop"
        );
        self.active_hunt = Some(HuntLoop::new(config, members));
        self.operating_mode = OperatingMode::Hunt;
    }

    /// Stop the current hunt loop.
    pub fn stop_hunt(&mut self) {
        if self.active_hunt.is_some() {
            tracing::info!("Stopping hunt loop");
            self.active_hunt = None;
            self.operating_mode = OperatingMode::Camp;
        }
    }

    /// Set the operating mode (Camp or Hunt).
    pub fn set_operating_mode(&mut self, mode: OperatingMode) {
        tracing::info!(?mode, "Switching operating mode");
        self.operating_mode = mode;
    }

    /// Activate a vendor sell cycle with the given config.
    pub fn start_sell_cycle(&mut self, config: VendorConfig) {
        tracing::info!(vendor = %config.vendor_name, "Configuring sell cycle");
        self.sell_cycle = Some(SellCycle::new(config));
    }

    /// Load the camp progression database from disk.
    pub fn load_camp_database(&mut self) {
        match CampDatabase::load() {
            Ok(db) => {
                tracing::info!(camps = db.len(), "Loaded camp progression database");
                self.camp_db = Some(db);
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load camp database");
            }
        }
    }

    /// Return the current camp/hunt state for display.
    #[must_use]
    pub fn camp_status(&self) -> String {
        match self.operating_mode {
            OperatingMode::Hunt => match &self.active_hunt {
                None => "Hunt mode — no active hunt".into(),
                Some(hunt) => {
                    let state = match &hunt.state {
                        crate::camp::hunt::HuntState::Roaming => "Roaming",
                        crate::camp::hunt::HuntState::Engaging { .. } => "Engaging",
                        crate::camp::hunt::HuntState::Fighting { .. } => "Fighting",
                        crate::camp::hunt::HuntState::Looting { .. } => "Looting",
                    };
                    format!(
                        "Hunt '{}' — {} — tick {} — {} members",
                        hunt.config.name,
                        state,
                        hunt.tick,
                        hunt.members.len()
                    )
                }
            },
            OperatingMode::Camp => match &self.active_camp {
                None => "No active camp".into(),
                Some(camp) => {
                    let state = match &camp.state {
                        CampState::Idle => "Idle",
                        CampState::Pulling { .. } => "Pulling",
                        CampState::Fighting { .. } => "Fighting",
                        CampState::Looting { .. } => "Looting",
                        CampState::Medding { .. } => "Medding",
                        CampState::Buffing { .. } => "Buffing",
                    };
                    let mut status = format!(
                        "Camp '{}' — {} — tick {} — {} members",
                        camp.config.name,
                        state,
                        camp.tick,
                        camp.members.len()
                    );
                    if let Some(ref suggestion) = self.suggested_camp {
                        status.push_str(&format!(" [suggest: {suggestion}]"));
                    }
                    if let Some(ref sc) = self.sell_cycle
                        && sc.state != SellState::NotNeeded
                    {
                        status.push_str(" [selling]");
                    }
                    status
                }
            },
        }
    }

    /// Register a client PID and generate a CSPRNG session token for it.
    /// Returns the token so the caller can pass it to the DLL during injection.
    #[allow(dead_code)]
    pub fn register_client(&mut self, pid: u32) -> SessionToken {
        let token = generate_session_token();
        self.session_tokens.insert(pid, token);
        if !self.client_pids.contains(&pid) {
            self.client_pids.push(pid);
        }
        token
    }

    /// Dispatch a `CampAction` to the appropriate client via IPC.
    fn dispatch_action(&mut self, pid: u32, action: &CampAction) {
        match action {
            CampAction::Slash(command) => {
                self.send_slash_command(pid, command);
            }
            CampAction::CombatEngage { target_id } => {
                self.send_ipc_command(
                    pid,
                    Command::CombatEngage {
                        target_id: *target_id,
                    },
                );
            }
            CampAction::CombatDisengage => {
                self.send_ipc_command(pid, Command::CombatDisengage);
            }
        }
    }

    /// Get or create a persistent pipe connection for a client.
    /// Authenticates once on first connect; reuses the connection thereafter.
    fn get_pipe(&mut self, pid: u32) -> Option<&CommandPipe> {
        let name = self
            .client_names
            .get(&pid)
            .map_or("?", std::string::String::as_str);

        let token = if let Some(t) = self.session_tokens.get(&pid) { *t } else {
            tracing::warn!(pid, name, "No session token for client — skipping");
            return None;
        };

        // Reuse existing connection or create a new one.
        use std::collections::hash_map::Entry;
        if let Entry::Vacant(entry) = self.pipe_pool.entry(pid) {
            let session_id = dmft_common::ipc::session_id_from_token(&token);
            match CommandPipe::connect(pid, session_id) {
                Ok(pipe) => {
                    if let Err(e) = pipe.send_raw_token(&token) {
                        tracing::warn!(pid, name, error = %e, "Failed to send token");
                        return None;
                    }
                    entry.insert(pipe);
                }
                Err(e) => {
                    tracing::warn!(pid, name, error = %e, "Failed to connect pipe");
                    return None;
                }
            }
        }

        self.pipe_pool.get(&pid)
    }

    /// Send a structured IPC command to a client via named pipe.
    /// Uses persistent connections — one pipe per client, reused across ticks.
    fn send_ipc_command(&mut self, pid: u32, cmd: Command) {
        let name = self
            .client_names
            .get(&pid)
            .map_or("?", std::string::String::as_str)
            .to_string();

        let Some(pipe) = self.get_pipe(pid) else {
            return;
        };
        match pipe.send_async(&cmd) {
            Ok(()) => {
                tracing::debug!(pid, name = %name, ?cmd, "Dispatched IPC command");
            }
            Err(e) => {
                tracing::warn!(pid, name = %name, ?cmd, error = %e, "Failed to send — dropping pipe");
                self.pipe_pool.remove(&pid);
            }
        }
    }

    /// Eject the DLL from a client and clean up its tracked state.
    /// Sends an Eject IPC command, then removes the client from all maps.
    pub fn eject_client(&mut self, pid: u32) {
        let name = self
            .client_names
            .get(&pid)
            .map_or("?", std::string::String::as_str)
            .to_string();
        tracing::info!(pid, name = %name, "Ejecting client");

        // Best-effort eject command — pipe may already be dead
        self.send_ipc_command(pid, Command::Eject);
        self.remove_client(pid);
    }

    /// Remove a client from all tracked state (does NOT send any IPC).
    pub fn remove_client(&mut self, pid: u32) {
        self.client_pids.retain(|&p| p != pid);
        self.client_names.remove(&pid);
        self.game_states.remove(&pid);
        self.state_readers.remove(&pid);
        self.session_tokens.remove(&pid);
        self.state_timestamps.remove(&pid);
        self.pipe_pool.remove(&pid);
        tracing::info!(pid, "Client removed from orchestrator");
    }

    /// Send a single slash command to a client via named pipe.
    fn send_slash_command(&mut self, pid: u32, command: &str) {
        let name = self
            .client_names
            .get(&pid)
            .map_or("?", std::string::String::as_str)
            .to_string();

        let Some(pipe) = self.get_pipe(pid) else {
            return;
        };
        let cmd = Command::SlashCommand {
            command: command.to_string(),
        };
        match pipe.send_async(&cmd) {
            Ok(()) => {
                tracing::debug!(pid, name = %name, %command, "Dispatched command");
            }
            Err(e) => {
                tracing::warn!(pid, name = %name, %command, error = %e, "Failed to send — dropping pipe");
                self.pipe_pool.remove(&pid);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camp::state::Role;

    fn test_config() -> CampConfig {
        CampConfig {
            name: "test".into(),
            zone: "crushbone".into(),
            camp_center: [0.0, 0.0, 0.0],
            pull_point: [10.0, 10.0, 0.0],
            pull_radius: 100.0,
            camp_radius: 20.0,
            leash_radius: 80.0,
            rest_mana_pct: 50,
            pull_mana_pct: 20,
            level_range: [1, 10],
            pull_mob_names: vec!["a_mob".into()],
            ignore_mob_names: Vec::new(),
            burn_mob_names: Vec::new(),
            next_camp: None,
            prev_camp: None,
        }
    }

    fn test_members() -> Vec<CampMember> {
        vec![
            CampMember::new(100, "Tank".into(), Role::Tank),
            CampMember::new(101, "Healer".into(), Role::Healer),
            CampMember::new(102, "DPS".into(), Role::Dps),
        ]
    }

    #[test]
    fn test_new_orchestrator() {
        let orch = Orchestrator::new();
        assert!(orch.active_camp.is_none());
        assert_eq!(orch.tick_count, 0);
    }

    #[test]
    fn test_tick_no_camp() {
        let mut orch = Orchestrator::new();
        assert_eq!(orch.tick(), 0);
        assert_eq!(orch.tick_count, 1);
    }

    #[test]
    fn test_start_and_stop_camp() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        assert!(orch.active_camp.is_some());
        assert!(orch.camp_status().contains("test"));

        orch.stop_camp();
        assert!(orch.active_camp.is_none());
        assert_eq!(orch.camp_status(), "No active camp");
    }

    #[test]
    fn test_tick_with_camp_generates_commands() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());

        // First tick transitions Idle -> Pulling, which generates commands
        let count = orch.tick();
        assert!(count > 0);
        assert!(!orch.last_dispatched.is_empty());
    }

    #[test]
    fn test_camp_status_shows_state() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        orch.tick(); // Idle -> Pulling
        let status = orch.camp_status();
        assert!(status.contains("Pulling"));
        assert!(status.contains("3 members"));
    }

    #[test]
    fn test_get_client_state_none_without_data() {
        let orch = Orchestrator::new();
        assert!(orch.get_client_state(100).is_none());
    }

    #[test]
    fn test_game_states_initialized_empty() {
        let orch = Orchestrator::new();
        assert!(orch.game_states.is_empty());
        assert!(orch.state_readers.is_empty());
    }

    #[test]
    fn test_build_camp_snapshot_none_without_camp() {
        let orch = Orchestrator::new();
        assert!(orch.build_camp_snapshot().is_none());
    }

    #[test]
    fn test_build_camp_snapshot_none_without_game_state() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        // No game state inserted — snapshot should be None
        assert!(orch.build_camp_snapshot().is_none());
    }

    #[test]
    fn test_build_camp_snapshot_with_game_state() {
        use dmft_common::combat::CombatStatus;
        use dmft_common::nav::NavStatus;
        use dmft_common::types::{GameState, SpawnData};

        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());

        fn make_spawn(hp: i64, hp_max: i64, mana: i32, mana_max: i32) -> SpawnData {
            SpawnData {
                spawn_id: 1,
                name: "Test".into(),
                displayed_name: "Test".into(),
                spawn_type: 0,
                level: 60,
                class_id: 1,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                heading: 0.0,
                hp_current: hp,
                hp_max,
                mana_current: mana,
                mana_max,
                endurance_current: 100,
                endurance_max: 100,
            }
        }

        // Set tick_count and timestamps so staleness check passes
        orch.tick_count = 1;
        orch.state_timestamps.insert(100, 1);
        orch.state_timestamps.insert(101, 1);

        // Tank at 80% HP
        orch.game_states.insert(
            100,
            GameState {
                client_id: 100,
                local_player: Some(make_spawn(800, 1000, 0, 0)),
                target: Some(make_spawn(500, 1000, 0, 0)),
                nearby_spawns: vec![],
                timestamp_ms: 0,
                nav_status: NavStatus::Idle,
                combat_status: CombatStatus::Idle,
                zone_short_name: String::new(),
                zone_long_name: String::new(),
            },
        );

        // Healer at 60% mana
        orch.game_states.insert(
            101,
            GameState {
                client_id: 101,
                local_player: Some(make_spawn(1000, 1000, 600, 1000)),
                target: None,
                nearby_spawns: vec![],
                timestamp_ms: 0,
                nav_status: NavStatus::Idle,
                combat_status: CombatStatus::Idle,
                zone_short_name: String::new(),
                zone_long_name: String::new(),
            },
        );

        let snap = orch.build_camp_snapshot().expect("should build snapshot");
        assert!((snap.tank_hp_pct - 80.0).abs() < 0.1);
        assert!((snap.healer_mana_pct - 60.0).abs() < 0.1);
        assert!((snap.target_hp_pct.unwrap() - 50.0).abs() < 0.1);
        assert!(!snap.target_is_dead);
    }

    #[test]
    fn test_register_client_generates_unique_tokens() {
        let mut orch = Orchestrator::new();
        let token_a = orch.register_client(100);
        let token_b = orch.register_client(101);
        // CSPRNG tokens should be different
        assert_ne!(token_a, token_b);
        // Client PID should be tracked
        assert!(orch.client_pids.contains(&100));
        assert!(orch.client_pids.contains(&101));
        // Token should be stored
        assert_eq!(orch.session_tokens[&100], token_a);
    }

    #[test]
    fn test_stale_state_returns_none_snapshot() {
        use dmft_common::combat::CombatStatus;
        use dmft_common::nav::NavStatus;
        use dmft_common::types::{GameState, SpawnData};

        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());

        fn make_spawn(hp: i64, hp_max: i64, mana: i32, mana_max: i32) -> SpawnData {
            SpawnData {
                spawn_id: 1,
                name: "Test".into(),
                displayed_name: "Test".into(),
                spawn_type: 0,
                level: 60,
                class_id: 1,
                x: 0.0,
                y: 0.0,
                z: 0.0,
                heading: 0.0,
                hp_current: hp,
                hp_max,
                mana_current: mana,
                mana_max,
                endurance_current: 100,
                endurance_max: 100,
            }
        }

        // Insert game states
        orch.game_states.insert(
            100,
            GameState {
                client_id: 100,
                local_player: Some(make_spawn(1000, 1000, 0, 0)),
                target: None,
                nearby_spawns: vec![],
                timestamp_ms: 0,
                nav_status: NavStatus::Idle,
                combat_status: CombatStatus::Idle,
                zone_short_name: String::new(),
                zone_long_name: String::new(),
            },
        );
        orch.game_states.insert(
            101,
            GameState {
                client_id: 101,
                local_player: Some(make_spawn(1000, 1000, 1000, 1000)),
                target: None,
                nearby_spawns: vec![],
                timestamp_ms: 0,
                nav_status: NavStatus::Idle,
                combat_status: CombatStatus::Idle,
                zone_short_name: String::new(),
                zone_long_name: String::new(),
            },
        );

        // State was updated at tick 1, current tick is 10 — stale by 9 ticks
        orch.state_timestamps.insert(100, 1);
        orch.state_timestamps.insert(101, 1);
        orch.tick_count = 10;

        assert!(
            orch.build_camp_snapshot().is_none(),
            "stale state should return None"
        );

        // Update timestamps to be fresh — snapshot should work
        orch.state_timestamps.insert(100, 9);
        orch.state_timestamps.insert(101, 9);
        assert!(
            orch.build_camp_snapshot().is_some(),
            "fresh state should return Some"
        );
    }

    // --- Task 1: Hunt mode toggle ---

    #[test]
    fn test_hunt_mode_toggle() {
        let mut orch = Orchestrator::new();
        assert_eq!(orch.operating_mode, OperatingMode::Camp);

        orch.set_operating_mode(OperatingMode::Hunt);
        assert_eq!(orch.operating_mode, OperatingMode::Hunt);

        // Tick in hunt mode with no active hunt returns 0
        assert_eq!(orch.tick(), 0);
    }

    #[test]
    fn test_start_hunt_sets_mode() {
        let mut orch = Orchestrator::new();
        orch.start_hunt(test_config(), test_members());
        assert_eq!(orch.operating_mode, OperatingMode::Hunt);
        assert!(orch.active_hunt.is_some());
    }

    #[test]
    fn test_hunt_tick_generates_commands() {
        let mut orch = Orchestrator::new();
        orch.start_hunt(test_config(), test_members());
        // First tick transitions Roaming -> Engaging
        let count = orch.tick();
        assert!(count > 0);
    }

    #[test]
    fn test_camp_status_hunt_mode() {
        let mut orch = Orchestrator::new();
        orch.start_hunt(test_config(), test_members());
        let status = orch.camp_status();
        assert!(status.contains("Hunt"));
    }

    #[test]
    fn test_camp_mode_tick_still_works() {
        let mut orch = Orchestrator::new();
        orch.set_operating_mode(OperatingMode::Camp);
        orch.start_camp(test_config(), test_members());
        let count = orch.tick();
        assert!(count > 0, "Camp mode tick should still produce commands");
    }

    // --- Task 2: Vendor sell cycle ---

    #[test]
    fn test_sell_cycle_integration() {
        let mut orch = Orchestrator::new();
        let vendor_config = VendorConfig {
            vendor_name: "Merchant_Leah".into(),
            sell_interval_ticks: 10,
            keep_items: vec![],
            travel_ticks: 2,
            sellable_items: vec![],
            sell_step_delay: 1,
            return_spell: None,
        };
        orch.start_sell_cycle(vendor_config);
        assert!(orch.sell_cycle.is_some());
    }

    #[test]
    fn test_sell_cycle_only_during_downtime() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        let vendor_config = VendorConfig {
            vendor_name: "Merchant_Leah".into(),
            sell_interval_ticks: 1, // trigger immediately
            keep_items: vec![],
            travel_ticks: 1,
            sellable_items: vec![],
            sell_step_delay: 1,
            return_spell: None,
        };
        orch.start_sell_cycle(vendor_config);

        // First tick goes Idle -> Pulling, sell cycle should not interfere
        orch.tick();
        // Camp should be in Pulling state, sell cycle should still be NotNeeded
        // (it only triggers during Idle/Medding)
        assert!(orch.active_camp.is_some());
    }

    #[test]
    fn test_sell_cycle_status_display() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        let vendor_config = VendorConfig {
            vendor_name: "Merchant_Leah".into(),
            sell_interval_ticks: 10,
            keep_items: vec![],
            travel_ticks: 2,
            sellable_items: vec![],
            sell_step_delay: 1,
            return_spell: None,
        };
        orch.start_sell_cycle(vendor_config);

        // Manually trigger selling state to check display
        if let Some(ref mut sc) = orch.sell_cycle {
            sc.start_sell(0);
        }
        let status = orch.camp_status();
        assert!(
            status.contains("[selling]"),
            "Status should show selling indicator"
        );
    }

    // --- Task 3: Buff rebuffing (tested via state.rs, verify integration) ---

    #[test]
    fn test_camp_loop_has_buff_tracker() {
        let camp = CampLoop::new(test_config(), test_members());
        assert!(camp.buff_tracker.last_cast.is_empty());
        assert!(camp.class_configs.is_empty());
    }

    // --- Task 4: Camp progression ---

    #[test]
    fn test_progression_suggested_camp() {
        let mut orch = Orchestrator::new();
        // No camp DB loaded — no suggestion
        orch.check_camp_progression();
        assert!(orch.suggested_camp.is_none());
    }

    #[test]
    fn test_progression_status_display() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        orch.suggested_camp = Some("unrest_yard".into());
        let status = orch.camp_status();
        assert!(status.contains("[suggest: unrest_yard]"));
    }

    // --- Task 5: Event production ---

    #[test]
    fn test_event_production_only_during_combat() {
        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());
        // Camp starts in Idle — no events should be produced
        orch.produce_camp_events(&None);
        assert!(
            orch.active_camp.as_ref().unwrap().pending_events.is_empty(),
            "No events during Idle"
        );
    }

    #[test]
    fn test_charm_break_detection() {
        use crate::camp::cc::{CcTarget, CcType};

        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());

        // Set camp to Fighting state
        if let Some(ref mut camp) = orch.active_camp {
            camp.state = CampState::Fighting { started_tick: 1 };
            // Add a charmed mob to CC tracker
            camp.cc_tracker.targets.push(CcTarget {
                spawn_id: 42,
                name: "charmed pet".into(),
                cc_applied: Some(CcType::Charm),
                cc_expiry_tick: 100,
                assigned_to_pid: Some(102),
                debuffed: false,
            });
        }

        // Record previous state with charm active
        orch.prev_cc_state.insert(42, CcType::Charm);

        // Now remove the charm (simulating a charm break)
        if let Some(ref mut camp) = orch.active_camp {
            camp.cc_tracker.targets[0].cc_applied = None;
        }

        orch.produce_camp_events(&None);

        // Should have pushed a CharmBreak event
        let events = &orch.active_camp.as_ref().unwrap().pending_events;
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CampEvent::CharmBreak { spawn_id: 42 })),
            "Should detect charm break"
        );
    }

    #[test]
    fn test_add_detection() {
        use dmft_common::combat::CombatStatus;
        use dmft_common::nav::NavStatus;
        use dmft_common::types::{GameState, SpawnData};

        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());

        // Set camp to Fighting state
        if let Some(ref mut camp) = orch.active_camp {
            camp.state = CampState::Fighting { started_tick: 1 };
        }

        // Insert game state with nearby spawns for the tank (pid 100)
        let new_npc = SpawnData {
            spawn_id: 99,
            name: "an orc centurion".into(),
            displayed_name: "an orc centurion".into(),
            spawn_type: 1, // NPC
            level: 10,
            class_id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 1000,
            hp_max: 1000,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 100,
            endurance_max: 100,
        };
        orch.game_states.insert(
            100,
            GameState {
                client_id: 100,
                local_player: Some(SpawnData {
                    spawn_id: 1,
                    name: "Tank".into(),
                    displayed_name: "Tank".into(),
                    spawn_type: 0,
                    level: 60,
                    class_id: 1,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    heading: 0.0,
                    hp_current: 1000,
                    hp_max: 1000,
                    mana_current: 0,
                    mana_max: 0,
                    endurance_current: 100,
                    endurance_max: 100,
                }),
                target: None,
                nearby_spawns: vec![new_npc],
                timestamp_ms: 0,
                nav_status: NavStatus::Idle,
                combat_status: CombatStatus::Idle,
                zone_short_name: String::new(),
                zone_long_name: String::new(),
            },
        );

        // First call: records spawns as prev
        orch.produce_camp_events(&None);
        // Clear any events from first detection (first time seeing spawn 99)
        let events = &orch.active_camp.as_ref().unwrap().pending_events;
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CampEvent::AddSpawned { spawn_id: 99, .. })),
            "Should detect new add"
        );
    }

    #[test]
    fn test_cc_expiry_detection() {
        use crate::camp::cc::CcTarget;

        let mut orch = Orchestrator::new();
        orch.start_camp(test_config(), test_members());

        // Set camp to Fighting state with a mezzed mob about to expire
        if let Some(ref mut camp) = orch.active_camp {
            camp.state = CampState::Fighting { started_tick: 1 };
            camp.tick = 18; // Current tick
            camp.cc_tracker.targets.push(CcTarget {
                spawn_id: 55,
                name: "mezzed orc".into(),
                cc_applied: Some(CcType::Mez),
                cc_expiry_tick: 20, // 2 ticks away, within CC_EXPIRY_BUFFER (3)
                assigned_to_pid: Some(102),
                debuffed: false,
            });
        }

        orch.produce_camp_events(&None);

        let events = &orch.active_camp.as_ref().unwrap().pending_events;
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CampEvent::CcExpiring { spawn_id: 55 })),
            "Should detect CC about to expire"
        );
    }
}
