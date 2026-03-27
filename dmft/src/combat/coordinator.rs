use dmft_common::ipc::Command;
use dmft_common::types::{ClientId, GameState};
use std::collections::HashMap;

pub struct CombatCoordinator {
    assist_target: Option<u32>,
    main_tank_id: Option<ClientId>,
    cc_assignments: HashMap<u32, ClientId>,
}

impl CombatCoordinator {
    pub fn new() -> Self {
        Self {
            assist_target: None,
            main_tank_id: None,
            cc_assignments: HashMap::new(),
        }
    }

    pub fn set_main_tank(&mut self, client_id: ClientId) {
        self.main_tank_id = Some(client_id);
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

        commands
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
                commands.push((
                    enc_id,
                    Command::CombatSetAssistTarget { spawn_id },
                ));
            }
        }

        commands
    }
}
