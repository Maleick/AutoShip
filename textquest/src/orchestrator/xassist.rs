//! Cross-group outside-group assist — MQ2XAssist parity.
//!
//! Enables a character to assist a Main Assist who is outside their group or raid.
//! Essential for scenarios where the MA is not in the same group (cross-group
//! raiding, solo boxes assisting a main, raid scenarios with split groups).
//!
//! Unlike [`super::cross_group::CrossGroupCoordinator`] which handles same-zone
//! emergency coordination, this module implements persistent per-character assist
//! targeting based on a configured MA name.

use std::collections::HashMap;

use textquest_common::types::{ClientId, GameState};

/// Priority mode for the cross-group assist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum XAssistPriority {
    /// Always target the cross-group MA's target when `enabled` (default).
    #[default]
    CrossGroupOverride,
    /// Only assist the cross-group MA when no in-group target is active.
    Fallback,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XAssistConfig {
    pub ma_name: Option<String>,
    pub enabled: bool,
    /// How this client should prioritize the cross-group MA vs. in-group assist.
    #[serde(default)]
    pub priority: XAssistPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssistTargetState {
    None,
    Tracking { spawn_id: u32 },
}

pub struct XAssist {
    configs: HashMap<ClientId, XAssistConfig>,
    current_targets: HashMap<ClientId, AssistTargetState>,
}

impl XAssist {
    #[must_use]
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            current_targets: HashMap::new(),
        }
    }

    pub fn set_config(&mut self, client_id: ClientId, config: XAssistConfig) {
        self.configs.insert(client_id, config);
        self.current_targets.remove(&client_id);
    }

    pub fn get_config(&self, client_id: ClientId) -> Option<&XAssistConfig> {
        self.configs.get(&client_id)
    }

    pub fn remove_client(&mut self, client_id: ClientId) {
        self.configs.remove(&client_id);
        self.current_targets.remove(&client_id);
    }

    pub fn tick(
        &mut self,
        game_states: &HashMap<ClientId, GameState>,
    ) -> Vec<(ClientId, AssistCommand)> {
        let mut commands = Vec::new();

        for (&client_id, config) in &self.configs {
            if !config.enabled {
                continue;
            }

            let Some(ma_name) = &config.ma_name else {
                continue;
            };

            let Some(state) = game_states.get(&client_id) else {
                continue;
            };

            let new_target = find_ma_state(game_states, ma_name)
                .and_then(|ma_state| ma_target_for_client(state, ma_state));

            let current = self.current_targets.get(&client_id).copied();

            match new_target {
                None => {
                    self.current_targets
                        .insert(client_id, AssistTargetState::None);
                }
                Some(target) => {
                    let already_tracking = matches!(
                        current,
                        Some(AssistTargetState::Tracking { spawn_id }) if spawn_id == target
                    );
                    if already_tracking {
                        continue;
                    }

                    commands.push((client_id, AssistCommand::Target(target)));
                    self.current_targets
                        .insert(client_id, AssistTargetState::Tracking { spawn_id: target });
                }
            }
        }

        commands
    }

    pub fn clear_all_targets(&mut self) {
        for state in self.current_targets.values_mut() {
            *state = AssistTargetState::None;
        }
    }
}

impl Default for XAssist {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistCommand {
    Target(u32),
}

fn find_spawn_by_name(state: &GameState, name: &str) -> Option<u32> {
    let wanted = normalize_spawn_name(name);
    state
        .nearby_spawns
        .iter()
        .find(|spawn| {
            spawn.spawn_type == 0
                && (normalize_spawn_name(&spawn.name) == wanted
                    || normalize_spawn_name(&spawn.displayed_name) == wanted)
        })
        .map(|spawn| spawn.spawn_id)
}

fn normalize_spawn_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn find_ma_state<'a>(
    game_states: &'a HashMap<ClientId, GameState>,
    ma_name: &str,
) -> Option<&'a GameState> {
    game_states
        .values()
        .find(|state| find_spawn_by_name(state, ma_name).is_some())
}

fn ma_target_for_client(client_state: &GameState, ma_state: &GameState) -> Option<u32> {
    if client_state.zone_short_name != ma_state.zone_short_name {
        return None;
    }

    let target_id = ma_state
        .target
        .as_ref()
        .map(|t| t.spawn_id)
        .filter(|&id| id != 0)?;

    let visible_to_client = client_state
        .target
        .as_ref()
        .is_some_and(|target| target.spawn_id == target_id)
        || client_state
            .nearby_spawns
            .iter()
            .any(|spawn| spawn.spawn_id == target_id);

    visible_to_client.then_some(target_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::{combat::CombatStatus, nav::NavStatus, types::SpawnData};

    fn make_player_spawn(spawn_id: u32, name: &str) -> SpawnData {
        SpawnData {
            spawn_id,
            name: name.into(),
            displayed_name: name.into(),
            spawn_type: 0,
            level: 60,
            class_id: 1,
            race_id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 1000,
            hp_max: 1000,
            mana_current: 500,
            mana_max: 500,
            endurance_current: 100,
            endurance_max: 100,
            speed_run: 0.0,
            stand_state: 0,
            is_gm: false,
            combat_target_id: None,
        }
    }

    fn make_npc_spawn(spawn_id: u32, name: &str) -> SpawnData {
        SpawnData {
            spawn_id,
            name: name.into(),
            displayed_name: name.into(),
            spawn_type: 1,
            level: 50,
            class_id: 1,
            race_id: 1,
            x: 10.0,
            y: 10.0,
            z: 0.0,
            heading: 0.0,
            hp_current: 5000,
            hp_max: 5000,
            mana_current: 0,
            mana_max: 0,
            endurance_current: 100,
            endurance_max: 100,
            speed_run: 0.0,
            stand_state: 0,
            is_gm: false,
            combat_target_id: None,
        }
    }

    fn make_state(
        client_id: u32,
        local_player: SpawnData,
        target: Option<SpawnData>,
        nearby_spawns: Vec<SpawnData>,
    ) -> GameState {
        GameState {
            client_id,
            local_player: Some(local_player),
            target,
            nearby_spawns,
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "test".into(),
            zone_long_name: "Test Zone".into(),
            active_buffs: vec![],
            pet: None,
            actual_version: None,
        }
    }

    #[test]
    fn xassist_disabled_produces_no_commands() {
        let mut xassist = XAssist::new();
        xassist.set_config(
            100,
            XAssistConfig {
                ma_name: Some("MainTank".into()),
                enabled: false,
                priority: XAssistPriority::default(),
            },
        );

        let nearby = vec![
            make_player_spawn(200, "MainTank"),
            make_npc_spawn(300, "an_orc"),
        ];
        let mut target = make_npc_spawn(300, "an_orc");
        target.spawn_id = 300;
        let state = GameState {
            client_id: 100,
            local_player: Some(make_player_spawn(100, "BoxDPS")),
            target: Some(target),
            nearby_spawns: nearby,
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "test".into(),
            zone_long_name: "Test Zone".into(),
            active_buffs: vec![],
            pet: None,
            actual_version: None,
        };

        let commands = xassist.tick(&[(100, state), (200, ma_state)].into_iter().collect());
        assert!(commands.is_empty());
    }

    #[test]
    fn xassist_targets_ma_when_enabled() {
        let mut xassist = XAssist::new();
        xassist.set_config(
            100,
            XAssistConfig {
                ma_name: Some("MainTank".into()),
                enabled: true,
                priority: XAssistPriority::default(),
            },
        );

        let nearby = vec![
            make_player_spawn(200, "MainTank"),
            make_npc_spawn(300, "an_orc"),
            make_npc_spawn(301, "another_orc"),
        ];
        let target = make_npc_spawn(300, "an_orc");
        let state = GameState {
            client_id: 100,
            local_player: Some(make_player_spawn(200, "MainTank")),
            target: Some(target),
            nearby_spawns: nearby,
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "test".into(),
            zone_long_name: "Test Zone".into(),
            active_buffs: vec![],
            pet: None,
            actual_version: None,
        };

        let commands = xassist.tick(&[(100, state)].into_iter().collect());
        assert!(
            !commands.is_empty(),
            "Should have assist commands when targeting MA"
        );
        let state = make_state(100, make_player_spawn(100, "BoxDPS"), None, nearby);

        let commands = xassist.tick(&[(100, state), (200, ma_state)].into_iter().collect());
        assert_eq!(commands, vec![(100, AssistCommand::Target(300))]);
    }

    #[test]
    fn xassist_no_command_when_target_unchanged() {
        let mut xassist = XAssist::new();
        xassist.set_config(
            100,
            XAssistConfig {
                ma_name: Some("MainTank".into()),
                enabled: true,
                priority: XAssistPriority::default(),
            },
        );
        xassist
            .current_targets
            .insert(100, AssistTargetState::Tracking { spawn_id: 300 });

        let nearby = vec![
            make_player_spawn(200, "MainTank"),
            make_npc_spawn(300, "an_orc"),
        ];
        let target = make_npc_spawn(300, "an_orc");
        let state = GameState {
            client_id: 100,
            local_player: Some(make_player_spawn(200, "MainTank")),
            target: Some(target),
            nearby_spawns: nearby,
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "test".into(),
            zone_long_name: "Test Zone".into(),
            active_buffs: vec![],
            pet: None,
            actual_version: None,
        };

        let commands = xassist.tick(&[(100, state), (200, ma_state)].into_iter().collect());
        assert!(
            commands.is_empty(),
            "Should not re-target when already tracking MA's target"
        );
    }

    #[test]
    fn xassist_tracks_new_target_when_ma_switches() {
        let mut xassist = XAssist::new();
        xassist.set_config(
            100,
            XAssistConfig {
                ma_name: Some("MainTank".into()),
                enabled: true,
                priority: XAssistPriority::default(),
            },
        );
        xassist
            .current_targets
            .insert(100, AssistTargetState::Tracking { spawn_id: 300 });

        let nearby = vec![
            make_player_spawn(200, "MainTank"),
            make_npc_spawn(300, "old_orc"),
            make_npc_spawn(301, "new_orc"),
        ];
        let target = make_npc_spawn(301, "new_orc");
        let state = GameState {
            client_id: 100,
            local_player: Some(make_player_spawn(200, "MainTank")),
            target: Some(target),
            nearby_spawns: nearby,
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "test".into(),
            zone_long_name: "Test Zone".into(),
            active_buffs: vec![],
            pet: None,
            actual_version: None,
        };

        let commands = xassist.tick(&[(100, state)].into_iter().collect());
        assert!(
            !commands.is_empty(),
            "Should track MA's new target when targeting MA"
        );
        let state = make_state(100, make_player_spawn(100, "BoxDPS"), None, nearby);

        let commands = xassist.tick(&[(100, state), (200, ma_state)].into_iter().collect());
        assert_eq!(commands, vec![(100, AssistCommand::Target(301))]);
    }

    #[test]
    fn xassist_ma_not_in_spawn_list_produces_no_commands() {
        let mut xassist = XAssist::new();
        xassist.set_config(
            100,
            XAssistConfig {
                ma_name: Some("NonExistentMA".into()),
                enabled: true,
                priority: XAssistPriority::default(),
            },
        );

        let nearby = vec![
            make_player_spawn(200, "SomeOtherPlayer"),
            make_npc_spawn(300, "an_orc"),
        ];
        let state = GameState {
            client_id: 100,
            local_player: Some(make_player_spawn(100, "BoxDPS")),
            target: None,
            nearby_spawns: nearby,
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "test".into(),
            zone_long_name: "Test Zone".into(),
            active_buffs: vec![],
            pet: None,
            actual_version: None,
        };

        let commands = xassist.tick(&[(100, state)].into_iter().collect());
        assert!(
            commands.is_empty(),
            "Should not command when MA not visible"
        );
    }

    #[test]
    fn xassist_name_matching_is_case_insensitive() {
        let mut xassist = XAssist::new();
        xassist.set_config(
            100,
            XAssistConfig {
                ma_name: Some("maInTaNk".into()),
                enabled: true,
                priority: XAssistPriority::default(),
            },
        );

        let nearby = vec![
            make_player_spawn(200, "MainTank"),
            make_npc_spawn(300, "an_orc"),
        ];
        let state = GameState {
            client_id: 100,
            local_player: Some(make_player_spawn(100, "BoxDPS")),
            target: None,
            nearby_spawns: nearby,
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: CombatStatus::Idle,
            zone_short_name: "test".into(),
            zone_long_name: "Test Zone".into(),
            active_buffs: vec![],
            pet: None,
            actual_version: None,
        };

        let state_for_find = make_state(100, make_player_spawn(100, "BoxDPS"), None, nearby);
        assert_eq!(
            find_spawn_by_name(&state_for_find, "  MaInTaNk  "),
            Some(200)
        );
    }

    #[test]
    fn xassist_remove_client_clears_state() {
        let mut xassist = XAssist::new();
        xassist.set_config(
            100,
            XAssistConfig {
                ma_name: Some("MainTank".into()),
                enabled: true,
                priority: XAssistPriority::default(),
            },
        );
        xassist
            .current_targets
            .insert(100, AssistTargetState::Tracking { spawn_id: 300 });

        xassist.remove_client(100);

        assert!(xassist.get_config(100).is_none());
        assert!(!xassist.current_targets.contains_key(&100));
    }
}
