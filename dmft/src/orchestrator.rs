//! Orchestrator — wires the camp loop state machine to IPC command delivery.

use crate::camp::config::CampConfig;
use crate::camp::state::{CampLoop, CampMember, CampSnapshot, CampState, Role};
use crate::ipc::pipe::CommandPipe;
use crate::ipc::shared::SharedStateReader;
use dmft_common::ipc::Command;
use dmft_common::types::GameState;
use std::collections::HashMap;

/// Generate a PID-derived session token for IPC auth.
fn generate_session_token(pid: u32) -> [u8; 32] {
    let pid_bytes = pid.to_le_bytes();
    let mut token = [0u8; 32];
    for (i, byte) in token.iter_mut().enumerate() {
        *byte = pid_bytes[i % 4] ^ (i as u8);
    }
    token
}

/// Top-level orchestrator that ticks the camp loop and dispatches commands.
pub struct Orchestrator {
    pub client_pids: Vec<u32>,
    pub client_names: HashMap<u32, String>,
    pub active_camp: Option<CampLoop>,
    pub tick_count: u64,
    /// Commands dispatched this tick (for status display).
    pub last_dispatched: Vec<(u32, String)>,
    /// Latest game state per client PID.
    pub game_states: HashMap<u32, GameState>,
    /// Shared memory readers per client PID.
    state_readers: HashMap<u32, SharedStateReader>,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            client_pids: Vec::new(),
            client_names: HashMap::new(),
            active_camp: None,
            tick_count: 0,
            last_dispatched: Vec::new(),
            game_states: HashMap::new(),
            state_readers: HashMap::new(),
        }
    }

    /// Get the latest game state for a client PID.
    pub fn get_client_state(&self, pid: u32) -> Option<&GameState> {
        self.game_states.get(&pid)
    }

    /// Read game state from shared memory for all known clients.
    fn poll_game_states(&mut self) {
        for &pid in &self.client_pids {
            // Lazily create readers
            if !self.state_readers.contains_key(&pid) {
                match SharedStateReader::new(pid) {
                    Ok(reader) => {
                        self.state_readers.insert(pid, reader);
                    }
                    Err(e) => {
                        tracing::debug!(pid, error = %e, "Failed to open shared memory reader");
                        continue;
                    }
                }
            }

            if let Some(reader) = self.state_readers.get(&pid) {
                if let Some(state) = reader.read() {
                    self.game_states.insert(pid, state);
                }
            }
        }
    }

    /// Build a `CampSnapshot` from live game state for the active camp's members.
    fn build_camp_snapshot(&self) -> Option<CampSnapshot> {
        let camp = self.active_camp.as_ref()?;

        let tank = camp.members.iter().find(|m| m.role == Role::Tank)?;
        let healer = camp.members.iter().find(|m| m.role == Role::Healer)?;

        let tank_state = self.game_states.get(&tank.pid)?;
        let healer_state = self.game_states.get(&healer.pid)?;

        let tank_hp_pct = tank_state
            .local_player
            .as_ref()
            .map(|p| p.hp_pct())
            .unwrap_or(100.0);

        let healer_mana_pct = healer_state
            .local_player
            .as_ref()
            .map(|p| p.mana_pct())
            .unwrap_or(100.0);

        // Use the tank's target for target HP info
        let (target_hp_pct, target_is_dead) = tank_state
            .target
            .as_ref()
            .map(|t| (Some(t.hp_pct()), t.hp_current <= 0))
            .unwrap_or((None, false));

        Some(CampSnapshot {
            healer_mana_pct,
            tank_hp_pct,
            target_hp_pct,
            target_is_dead,
        })
    }

    /// Advance the camp loop (if active), collect commands, and send via IPC.
    /// Returns the number of commands dispatched.
    pub fn tick(&mut self) -> usize {
        self.tick_count += 1;
        self.last_dispatched.clear();

        self.poll_game_states();
        let snapshot = self.build_camp_snapshot();

        let commands = match self.active_camp.as_mut() {
            Some(camp) => camp.tick(snapshot.as_ref()),
            None => return 0,
        };

        let count = commands.len();
        for (pid, cmd) in &commands {
            self.send_command(*pid, cmd);
        }
        self.last_dispatched = commands;
        count
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

    /// Return the current camp state for display.
    pub fn camp_status(&self) -> String {
        match &self.active_camp {
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
                format!(
                    "Camp '{}' — {} — tick {} — {} members",
                    camp.config.name,
                    state,
                    camp.tick,
                    camp.members.len()
                )
            }
        }
    }

    /// Send a single slash command to a client via named pipe.
    fn send_command(&self, pid: u32, command: &str) {
        let name = self
            .client_names
            .get(&pid)
            .map(|s| s.as_str())
            .unwrap_or("?");

        match CommandPipe::connect(pid) {
            Ok(pipe) => {
                let token = generate_session_token(pid);
                if let Err(e) = pipe.send_raw_token(&token) {
                    tracing::warn!(pid, name, error = %e, "Failed to send token");
                    return;
                }
                let cmd = Command::SlashCommand {
                    command: command.to_string(),
                };
                if let Err(e) = pipe.send_async(&cmd) {
                    tracing::warn!(pid, name, %command, error = %e, "Failed to send command");
                } else {
                    tracing::debug!(pid, name, %command, "Dispatched command");
                }
            }
            Err(e) => {
                tracing::warn!(pid, name, error = %e, "Failed to connect pipe");
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
        }
    }

    fn test_members() -> Vec<CampMember> {
        vec![
            CampMember { pid: 100, name: "Tank".into(), role: Role::Tank },
            CampMember { pid: 101, name: "Healer".into(), role: Role::Healer },
            CampMember { pid: 102, name: "DPS".into(), role: Role::DPS },
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
        use dmft_common::types::{GameState, SpawnData};
        use dmft_common::nav::NavStatus;
        use dmft_common::combat::CombatStatus;

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
                x: 0.0, y: 0.0, z: 0.0, heading: 0.0,
                hp_current: hp,
                hp_max,
                mana_current: mana,
                mana_max,
                endurance_current: 100,
                endurance_max: 100,
            }
        }

        // Tank at 80% HP
        orch.game_states.insert(100, GameState {
            client_id: 100,
            local_player: Some(make_spawn(800, 1000, 0, 0)),
            target: Some(make_spawn(500, 1000, 0, 0)),
            nearby_spawns: vec![],
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
        });

        // Healer at 60% mana
        orch.game_states.insert(101, GameState {
            client_id: 101,
            local_player: Some(make_spawn(1000, 1000, 600, 1000)),
            target: None,
            nearby_spawns: vec![],
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
        });

        let snap = orch.build_camp_snapshot().expect("should build snapshot");
        assert!((snap.tank_hp_pct - 80.0).abs() < 0.1);
        assert!((snap.healer_mana_pct - 60.0).abs() < 0.1);
        assert!((snap.target_hp_pct.unwrap() - 50.0).abs() < 0.1);
        assert!(!snap.target_is_dead);
    }
}
