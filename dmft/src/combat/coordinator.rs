use dmft_common::combat::CombatStatus;
use dmft_common::ipc::Command;
use dmft_common::types::{ClientId, GameState};
use std::collections::HashMap;

use super::camp_loop::{CampEvent, CampLoop, CampState};
use super::ch_chain::ChChain;

/// Coordinates group combat — assist targeting, CC assignments, and camp loop FSM.
pub struct CombatCoordinator {
    assist_target: Option<u32>,
    main_tank_id: Option<ClientId>,
    cc_assignments: HashMap<u32, ClientId>,
    camp_loop: CampLoop,
    /// Track which clients were in combat last tick for edge detection.
    prev_in_combat: bool,
    /// Track which clients were dead last tick.
    prev_dead: HashMap<ClientId, bool>,
    /// Complete Heal chain coordinator — rotates CH casts across clerics.
    pub ch_chain: Option<ChChain>,
}

impl CombatCoordinator {
    /// Create a new combat coordinator with no assignments.
    #[must_use]
    pub fn new() -> Self {
        Self {
            assist_target: None,
            main_tank_id: None,
            cc_assignments: HashMap::new(),
            camp_loop: CampLoop::new(),
            prev_in_combat: false,
            prev_dead: HashMap::new(),
            ch_chain: None,
        }
    }

    /// Designate a client as the main tank for assist targeting.
    pub fn set_main_tank(&mut self, client_id: ClientId) {
        self.main_tank_id = Some(client_id);
    }

    /// Reference to the inner camp loop FSM.
    #[must_use]
    pub fn camp_loop(&self) -> &CampLoop {
        &self.camp_loop
    }

    /// Start the camp→pull→fight→loot cycle.
    pub fn start_camp(&mut self) {
        self.camp_loop.start();
    }

    /// Stop the camp loop.
    pub fn stop_camp(&mut self) {
        self.camp_loop.stop();
    }

    /// Set the puller for the camp loop.
    pub fn set_puller(&mut self, client_id: ClientId) {
        self.camp_loop.set_puller(client_id);
    }

    /// Called each orchestrator tick with all client states.
    /// Returns commands to send to specific clients.
    pub fn tick(&mut self, states: &HashMap<ClientId, GameState>) -> Vec<(ClientId, Command)> {
        let mut commands = Vec::new();

        // 1. Find MA's target (main tank's current target)
        if let Some(new_assist) = self.decide_assist_target(states)
            && self.assist_target != Some(new_assist)
        {
            self.assist_target = Some(new_assist);
            // Broadcast assist target to all DPS
            for &cid in states.keys() {
                if Some(cid) != self.main_tank_id {
                    commands.push((
                        cid,
                        Command::CombatSetAssistTarget {
                            spawn_id: new_assist,
                        },
                    ));
                }
            }
        }

        // 2. CH chain — feed tank HP for adaptive mode, then tick rotation
        if let Some(ref mut chain) = self.ch_chain {
            // Feed tank HP to adaptive timer
            if chain.is_adaptive()
                && let Some(tank_id) = self.main_tank_id
                && let Some(tank_state) = states.get(&tank_id)
                && let Some(ref lp) = tank_state.local_player
            {
                chain.update_tank_hp(lp.hp_pct());
            }

            if let Some(cleric_pid) = chain.tick() {
                let target_id = chain.target_id();
                let spell_slot = chain.spell_slot();
                tracing::info!(cleric_pid, target_id, spell_slot, "CH chain: firing cleric");
                // Target the tank, then cast CH
                commands.push((
                    cleric_pid,
                    Command::SetTarget {
                        spawn_id: target_id,
                    },
                ));
                commands.push((
                    cleric_pid,
                    Command::CastSpell {
                        spell_slot,
                        target_id: Some(target_id),
                    },
                ));
            }
        }

        // 3. Camp loop integration — detect state changes and feed events
        if self.camp_loop.is_active() {
            let camp_events = self.detect_camp_events(states);
            for event in camp_events {
                let camp_commands = self.camp_loop.process_event(event);
                commands.extend(camp_commands);
            }

            // Check for state timeouts
            if let Some(timeout_event) = self.camp_loop.check_timeout() {
                let camp_commands = self.camp_loop.process_event(timeout_event);
                commands.extend(camp_commands);
            }
        }

        commands
    }

    /// Detect combat state changes from `GameState` and convert to `CampEvents`.
    fn detect_camp_events(&mut self, states: &HashMap<ClientId, GameState>) -> Vec<CampEvent> {
        let mut events = Vec::new();

        let any_in_combat = states.values().any(|gs| {
            matches!(
                gs.combat_status,
                CombatStatus::Engaging { .. }
                    | CombatStatus::Casting { .. }
                    | CombatStatus::OnGcd
                    | CombatStatus::Pulling { .. }
            )
        });

        let any_pulling = states
            .values()
            .any(|gs| matches!(gs.combat_status, CombatStatus::Pulling { .. }));

        // Detect combat start (edge: was not in combat, now is)
        if any_in_combat && !self.prev_in_combat {
            if any_pulling {
                events.push(CampEvent::PullIncoming);
            } else {
                events.push(CampEvent::CombatStarted);
            }
        }

        // Detect combat end (edge: was in combat, now nobody is)
        if !any_in_combat && self.prev_in_combat {
            events.push(CampEvent::CombatEnded);
        }

        // Detect deaths (edge: client was alive, now dead)
        for (&client_id, gs) in states {
            let is_dead = matches!(gs.combat_status, CombatStatus::Dead);
            let was_dead = self.prev_dead.get(&client_id).copied().unwrap_or(false);
            if is_dead && !was_dead {
                events.push(CampEvent::MemberDied { client_id });
            }
        }

        // Check for group wipe (all clients dead)
        let all_dead = !states.is_empty()
            && states
                .values()
                .all(|gs| matches!(gs.combat_status, CombatStatus::Dead));
        if all_dead && self.prev_in_combat {
            events.push(CampEvent::GroupWiped);
        }

        // GroupReady: at camp, nobody in combat, nobody dead
        if !any_in_combat
            && *self.camp_loop.state() == CampState::AtCamp
            && states
                .values()
                .all(|gs| !matches!(gs.combat_status, CombatStatus::Dead))
        {
            events.push(CampEvent::GroupReady);
        }

        // Update tracking state
        self.prev_in_combat = any_in_combat;
        self.prev_dead = states
            .iter()
            .map(|(&cid, gs)| (cid, matches!(gs.combat_status, CombatStatus::Dead)))
            .collect();

        events
    }

    fn decide_assist_target(&self, states: &HashMap<ClientId, GameState>) -> Option<u32> {
        // Read main tank's target
        let tank_id = self.main_tank_id?;
        let tank_state = states.get(&tank_id)?;
        let target = tank_state.target.as_ref()?;
        // Only assist on NPCs (spawn_type 1)
        if target.spawn_type == 1 {
            Some(target.spawn_id)
        } else {
            None
        }
    }

    // --- CH Chain management ---

    /// Start a Complete Heal chain with the given clerics.
    /// `interval_secs` is the time between each cleric's CH start.
    /// `target_id` is the spawn ID of the tank to heal.
    /// `spell_slot` is the gem slot for Complete Heal (1-indexed).
    pub fn start_ch_chain(
        &mut self,
        cleric_pids: Vec<u32>,
        interval_secs: f32,
        target_id: u32,
        spell_slot: u8,
    ) {
        let mut chain = ChChain::new(cleric_pids, interval_secs, target_id, spell_slot);
        chain.start();
        tracing::info!(
            members = chain.members().len(),
            interval_secs,
            target_id,
            spell_slot,
            "CH chain started"
        );
        self.ch_chain = Some(chain);
    }

    /// Stop the active CH chain.
    pub fn stop_ch_chain(&mut self) {
        if let Some(ref mut chain) = self.ch_chain {
            chain.stop();
            tracing::info!("CH chain stopped");
        }
        self.ch_chain = None;
    }

    /// Whether a CH chain is currently active.
    pub fn ch_chain_active(&self) -> bool {
        self.ch_chain
            .as_ref()
            .is_some_and(super::ch_chain::ChChain::is_active)
    }

    /// Add a cleric to the active CH chain.
    pub fn ch_chain_add(&mut self, pid: u32) {
        if let Some(ref mut chain) = self.ch_chain {
            chain.add_member(pid);
        }
    }

    /// Remove a cleric from the active CH chain.
    pub fn ch_chain_remove(&mut self, pid: u32) {
        if let Some(ref mut chain) = self.ch_chain {
            chain.remove_member(pid);
        }
    }

    /// Set the CH chain interval (seconds between each cast).
    pub fn ch_chain_set_interval(&mut self, secs: f32) {
        if let Some(ref mut chain) = self.ch_chain {
            chain.set_interval(secs);
        }
    }

    /// Replace the full CH chain member order.
    pub fn ch_chain_set_members(&mut self, members: Vec<u32>) {
        if let Some(ref mut chain) = self.ch_chain {
            chain.set_members(members);
        }
    }

    /// Set the CH chain target spawn ID.
    pub fn ch_chain_set_target(&mut self, target_id: u32) {
        if let Some(ref mut chain) = self.ch_chain {
            chain.set_target(target_id);
        }
    }

    /// Assign nearby enemies to enchanter CC targets and return commands.
    pub fn decide_cc_assignments(
        &mut self,
        nearby_enemies: &[(u32, String)],
        enchanter_ids: &[ClientId],
    ) -> Vec<(ClientId, Command)> {
        let mut commands = Vec::new();
        self.cc_assignments.clear();

        // Skip the first enemy (that's the main target), mez the rest
        for (i, &(spawn_id, _)) in nearby_enemies.iter().enumerate().skip(1) {
            if let Some(&enc_id) = enchanter_ids.get(i - 1) {
                self.cc_assignments.insert(spawn_id, enc_id);
                commands.push((enc_id, Command::CombatSetAssistTarget { spawn_id }));
            }
        }

        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dmft_common::combat::CombatStatus;
    use dmft_common::nav::NavStatus;
    use dmft_common::types::SpawnData;

    fn make_spawn(spawn_id: u32, spawn_type: u8) -> SpawnData {
        SpawnData {
            spawn_id,
            name: format!("Spawn{spawn_id}"),
            displayed_name: format!("Spawn{spawn_id}"),
            spawn_type,
            level: 50,
            class_id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 10000,
            hp_max: 10000,
            mana_current: 5000,
            mana_max: 5000,
            endurance_current: 100,
            endurance_max: 100,
            speed_run: 0.0,
            stand_state: 0,
        }
    }

    fn make_game_state(client_id: ClientId, target: Option<SpawnData>) -> GameState {
        GameState {
            client_id,
            local_player: Some(make_spawn(client_id, 0)),
            target,
            nearby_spawns: vec![],
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: String::new(),
            zone_long_name: String::new(),
        }
    }

    #[test]
    fn set_main_tank_stores_id() {
        let mut coord = CombatCoordinator::new();
        coord.set_main_tank(1);
        assert_eq!(coord.main_tank_id, Some(1));
    }

    #[test]
    fn tick_with_empty_states_returns_no_commands() {
        let mut coord = CombatCoordinator::new();
        coord.set_main_tank(1);
        let states: HashMap<ClientId, GameState> = HashMap::new();
        let commands = coord.tick(&states);
        assert!(commands.is_empty());
    }

    #[test]
    fn tick_broadcasts_assist_target_when_tank_has_npc_target() {
        let mut coord = CombatCoordinator::new();
        coord.set_main_tank(1);

        let npc = make_spawn(100, 1); // spawn_type 1 = NPC
        let mut states = HashMap::new();
        states.insert(1, make_game_state(1, Some(npc)));
        states.insert(2, make_game_state(2, None));
        states.insert(3, make_game_state(3, None));

        let commands = coord.tick(&states);
        // Should send CombatSetAssistTarget to non-tank clients
        assert!(!commands.is_empty());
        for (cid, cmd) in &commands {
            assert_ne!(*cid, 1, "Tank should not receive assist command");
            match cmd {
                Command::CombatSetAssistTarget { spawn_id } => {
                    assert_eq!(*spawn_id, 100);
                }
                _ => panic!("Expected CombatSetAssistTarget command"),
            }
        }
    }

    #[test]
    fn tick_no_command_when_tank_targets_player() {
        let mut coord = CombatCoordinator::new();
        coord.set_main_tank(1);

        let player_target = make_spawn(50, 0); // spawn_type 0 = player
        let mut states = HashMap::new();
        states.insert(1, make_game_state(1, Some(player_target)));
        states.insert(2, make_game_state(2, None));

        let commands = coord.tick(&states);
        assert!(commands.is_empty(), "Should not assist on player targets");
    }

    #[test]
    fn decide_cc_assignments_assigns_enchanters_to_extra_mobs() {
        let mut coord = CombatCoordinator::new();
        let enemies = vec![
            (100, "MainTarget".to_string()),
            (101, "Add1".to_string()),
            (102, "Add2".to_string()),
        ];
        let enchanters = vec![10u32, 11u32];

        let commands = coord.decide_cc_assignments(&enemies, &enchanters);

        // First enemy (main target) is skipped, enchanters assigned to adds
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].0, 10);
        assert_eq!(commands[1].0, 11);
        match &commands[0].1 {
            Command::CombatSetAssistTarget { spawn_id } => assert_eq!(*spawn_id, 101),
            _ => panic!("Expected CombatSetAssistTarget"),
        }
        match &commands[1].1 {
            Command::CombatSetAssistTarget { spawn_id } => assert_eq!(*spawn_id, 102),
            _ => panic!("Expected CombatSetAssistTarget"),
        }
    }

    #[test]
    fn decide_cc_assignments_handles_more_adds_than_enchanters() {
        let mut coord = CombatCoordinator::new();
        let enemies = vec![
            (100, "MainTarget".to_string()),
            (101, "Add1".to_string()),
            (102, "Add2".to_string()),
            (103, "Add3".to_string()),
        ];
        let enchanters = vec![10u32];

        let commands = coord.decide_cc_assignments(&enemies, &enchanters);
        assert_eq!(commands.len(), 1, "Only 1 enchanter available");
    }

    #[test]
    fn decide_cc_assignments_empty_when_no_adds() {
        let mut coord = CombatCoordinator::new();
        let enemies = vec![(100, "MainTarget".to_string())];
        let enchanters = vec![10u32];

        let commands = coord.decide_cc_assignments(&enemies, &enchanters);
        assert!(commands.is_empty(), "No adds to CC");
    }

    // --- CH Chain tests ---

    #[test]
    fn ch_chain_start_and_stop() {
        let mut coord = CombatCoordinator::new();
        assert!(!coord.ch_chain_active());

        coord.start_ch_chain(vec![10, 20, 30], 3.0, 1, 8);
        assert!(coord.ch_chain_active());

        coord.stop_ch_chain();
        assert!(!coord.ch_chain_active());
    }

    #[test]
    fn ch_chain_fires_cast_commands() {
        let mut coord = CombatCoordinator::new();
        coord.start_ch_chain(vec![10, 20], 1.0, 99, 8);

        let states: HashMap<ClientId, GameState> = HashMap::new();

        // First tick fires cleric 10 (frame 0)
        let cmds = coord.tick(&states);
        assert_eq!(cmds.len(), 2, "Should have SetTarget + CastSpell");
        assert_eq!(cmds[0].0, 10);
        match &cmds[0].1 {
            Command::SetTarget { spawn_id } => assert_eq!(*spawn_id, 99),
            _ => panic!("Expected SetTarget, got {:?}", cmds[0].1),
        }
        match &cmds[1].1 {
            Command::CastSpell {
                spell_slot,
                target_id,
            } => {
                assert_eq!(*spell_slot, 8);
                assert_eq!(*target_id, Some(99));
            }
            _ => panic!("Expected CastSpell, got {:?}", cmds[1].1),
        }

        // Next 19 ticks produce no CH commands
        for _ in 1..20 {
            let cmds = coord.tick(&states);
            assert!(cmds.is_empty());
        }

        // Frame 20 fires cleric 20
        let cmds = coord.tick(&states);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].0, 20);
    }

    #[test]
    fn ch_chain_add_remove_members() {
        let mut coord = CombatCoordinator::new();
        coord.start_ch_chain(vec![10, 20], 1.0, 99, 8);
        coord.ch_chain_add(30);
        coord.ch_chain_remove(10);

        let states: HashMap<ClientId, GameState> = HashMap::new();

        // First tick fires first remaining member (20)
        let cmds = coord.tick(&states);
        assert_eq!(cmds[0].0, 20);
    }
}
