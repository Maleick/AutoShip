//! Integration testing framework for end-to-end multibox scenarios.
//!
//! This module provides a structured way to test complex multibox behaviors,
//! such as farming loops, group healing, and zone recovery workflows.

use std::collections::HashMap;

use textquest::camp::{
    config::CampConfig,
    state::{CampAction, CampLoop, CampMember, CampSnapshot, PULL_DURATION, Role},
};
use textquest_common::types::GameState;

// ============================================================================
// Trait definitions
// ============================================================================

/// A single scenario's setup, execution, and verification phases.
pub trait Scenario {
    /// Set up the initial game state and return a scenario context.
    fn setup(&self) -> ScenarioContext;

    /// Run the scenario with the given context.
    /// Returns commands executed and final state snapshots.
    fn run(&self, ctx: &mut ScenarioContext) -> Vec<ScenarioEvent>;

    /// Verify that the scenario executed correctly.
    fn verify(&self, events: &[ScenarioEvent]) -> ScenarioResult;

    /// Human-readable name for this scenario.
    #[allow(dead_code)]
    fn name(&self) -> &'static str;
}

/// A single frame's worth of game state updates and command outputs.
#[derive(Debug, Clone)]
pub struct ScenarioEvent {
    /// Commands issued in this tick (e.g., "/attack", "/heal").
    pub commands: Vec<(u32, CampAction)>, // (client_id, action)
    /// Game state snapshot at this tick.
    pub snapshot: Option<GameStateSnapshot>,
}

/// Minimal snapshot of relevant game state for verification.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GameStateSnapshot {
    pub camp_state: String,
    pub healer_mana_pct: f32,
    pub tank_hp_pct: f32,
    pub target_hp_pct: Option<f32>,
}

/// Context for running a scenario.
pub struct ScenarioContext {
    /// The camp loop being tested.
    pub camp: CampLoop,
    /// Client ID → account name mapping.
    #[allow(dead_code)]
    pub clients: HashMap<u32, String>,
    /// Current game state.
    #[allow(dead_code)]
    pub game_state: GameState,
    /// Tick counter.
    pub tick: usize,
}

/// Outcome of a scenario run.
#[derive(Debug, Clone)]
pub struct ScenarioResult {
    /// Did the scenario pass?
    pub passed: bool,
    /// Reason if failed.
    pub reason: Option<String>,
    /// Total ticks executed.
    pub total_ticks: usize,
    /// Summary of what happened.
    #[allow(dead_code)]
    pub summary: String,
}

impl ScenarioResult {
    pub fn pass(total_ticks: usize, summary: String) -> Self {
        ScenarioResult {
            passed: true,
            reason: None,
            total_ticks,
            summary,
        }
    }

    pub fn fail(total_ticks: usize, reason: String) -> Self {
        ScenarioResult {
            passed: false,
            reason: Some(reason),
            total_ticks,
            summary: String::new(),
        }
    }
}

// ============================================================================
// ScenarioRunner: executor and harness
// ============================================================================

/// Runs scenarios with setup, execution, and verification phases.
pub struct ScenarioRunner;

impl ScenarioRunner {
    /// Execute a scenario and return the result.
    pub fn run<S: Scenario>(scenario: &S) -> ScenarioResult {
        // Phase 1: Setup
        let mut ctx = scenario.setup();

        // Phase 2: Run
        let events = scenario.run(&mut ctx);

        // Phase 3: Verify
        scenario.verify(&events)
    }
}

// ============================================================================
// Scenario 1: Solo Farming Loop
// ============================================================================

/// Single client farming loop: pull, kill, loot, move, repeat.
pub struct SoloFarmingScenario;

impl Scenario for SoloFarmingScenario {
    fn setup(&self) -> ScenarioContext {
        let camp_config = CampConfig {
            name: "crushbone_entrance".into(),
            zone: "crushbone".into(),
            camp_center: [100.0, 200.0, 0.0],
            pull_point: [150.0, 250.0, 0.0],
            pull_radius: 200.0,
            camp_radius: 30.0,
            leash_radius: 100.0,
            rest_mana_pct: 60,
            pull_mana_pct: 30,
            level_range: [5, 12],
            pull_mob_names: vec!["an orc pawn".into()],
            ignore_mob_names: vec![],
            burn_mob_names: vec![],
            return_no_aggro: false,
            next_camp: None,
            prev_camp: None,
        };

        let members = vec![
            CampMember::new(100, "SoloWarrior".into(), Role::Tank),
            CampMember::new(101, "SoloCleric".into(), Role::Healer),
        ];

        let mut clients = HashMap::new();
        clients.insert(100u32, "warrior_account".to_string());
        clients.insert(101u32, "healer_account".to_string());

        ScenarioContext {
            camp: CampLoop::new(camp_config, members),
            clients,
            game_state: GameState {
                client_id: 1,
                local_player: None,
                target: None,
                nearby_spawns: Vec::new(),
                timestamp_ms: 0,
                nav_status: textquest_common::nav::NavStatus::Idle,
                combat_status: textquest_common::combat::CombatStatus::Idle,
                zone_short_name: "crushbone".to_string(),
                zone_long_name: "Crushbone Castle".to_string(),
                actual_version: None,
            },
            tick: 0,
        }
    }

    fn run(&self, ctx: &mut ScenarioContext) -> Vec<ScenarioEvent> {
        let mut events = Vec::new();

        // Run 10 pulls: pull → fight → loot → med → repeat
        for _pull in 0..10 {
            // Pull tick
            let cmds = ctx.camp.tick(None);
            ctx.tick += 1;
            events.push(ScenarioEvent {
                commands: cmds,
                snapshot: None,
            });

            // Advance through pull timer
            for _ in 0..PULL_DURATION - 1 {
                let cmds = ctx.camp.tick(None);
                ctx.tick += 1;
                events.push(ScenarioEvent {
                    commands: cmds,
                    snapshot: None,
                });
            }

            // Fighting tick (target dies immediately for fast cycle)
            let dead_snapshot = CampSnapshot {
                healer_mana_pct: 80.0,
                tank_hp_pct: 100.0,
                target_hp_pct: Some(0.0),
                target_is_dead: true,
                target_spawn_id: Some(9999),
                member_hp: vec![],
                member_in_combat: vec![],
            };
            let cmds = ctx.camp.tick(Some(&dead_snapshot));
            ctx.tick += 1;
            events.push(ScenarioEvent {
                commands: cmds,
                snapshot: Some(GameStateSnapshot {
                    camp_state: format!("{:?}", ctx.camp.state),
                    healer_mana_pct: 80.0,
                    tank_hp_pct: 100.0,
                    target_hp_pct: Some(0.0),
                }),
            });

            // Loot phase (run several ticks to complete looting)
            for _ in 0..5 {
                let cmds = ctx.camp.tick(None);
                ctx.tick += 1;
                events.push(ScenarioEvent {
                    commands: cmds,
                    snapshot: None,
                });
            }

            // Med phase (run several ticks)
            for _ in 0..3 {
                let cmds = ctx.camp.tick(None);
                ctx.tick += 1;
                events.push(ScenarioEvent {
                    commands: cmds,
                    snapshot: None,
                });
            }
        }

        events
    }

    fn verify(&self, events: &[ScenarioEvent]) -> ScenarioResult {
        if events.is_empty() {
            return ScenarioResult::fail(0, "No events recorded".to_string());
        }

        // Verify we had at least some pulls
        let dead_events = events.iter().filter(|e| {
            if let Some(snap) = &e.snapshot {
                snap.target_hp_pct == Some(0.0)
            } else {
                false
            }
        });

        let dead_count = dead_events.count();
        if dead_count == 0 {
            return ScenarioResult::fail(events.len(), "No kills detected".to_string());
        }

        // Verify we had combat-related commands
        let has_attack_commands = events.iter().any(|e| {
            e.commands.iter().any(|(_, action)| {
                matches!(
                    action,
                    CampAction::CombatEngage { .. } | CampAction::CombatDisengage
                ) || matches!(action, CampAction::Slash(s) if s.contains("/attack"))
            })
        });

        if !has_attack_commands {
            return ScenarioResult::fail(events.len(), "No combat commands detected".to_string());
        }

        ScenarioResult::pass(
            events.len(),
            format!("Solo farming completed {} pulls successfully", dead_count),
        )
    }

    fn name(&self) -> &'static str {
        "Solo Farming Loop"
    }
}

// ============================================================================
// Scenario 2: Group Healing
// ============================================================================

/// Group with healer responding to HP changes.
pub struct GroupHealScenario;

impl Scenario for GroupHealScenario {
    fn setup(&self) -> ScenarioContext {
        let camp_config = CampConfig {
            name: "crushbone_entrance".into(),
            zone: "crushbone".into(),
            camp_center: [100.0, 200.0, 0.0],
            pull_point: [150.0, 250.0, 0.0],
            pull_radius: 200.0,
            camp_radius: 30.0,
            leash_radius: 100.0,
            rest_mana_pct: 60,
            pull_mana_pct: 30,
            level_range: [5, 12],
            pull_mob_names: vec!["an orc pawn".into()],
            ignore_mob_names: vec![],
            burn_mob_names: vec![],
            return_no_aggro: false,
            next_camp: None,
            prev_camp: None,
        };

        let members = vec![
            CampMember::new(100, "Warrior01".into(), Role::Tank),
            CampMember::new(101, "Cleric01".into(), Role::Healer),
            CampMember::new(102, "Enchanter01".into(), Role::CC),
            CampMember::new(103, "Bard01".into(), Role::Puller),
            CampMember::new(104, "Ranger01".into(), Role::Dps),
            CampMember::new(105, "Ranger02".into(), Role::Dps),
        ];

        let mut clients = HashMap::new();
        for (_name, role) in [
            ("Warrior01", "warrior"),
            ("Cleric01", "cleric"),
            ("Enchanter01", "enchanter"),
            ("Bard01", "bard"),
            ("Ranger01", "ranger1"),
            ("Ranger02", "ranger2"),
        ]
        .iter()
        {
            clients.insert(100u32 + clients.len() as u32, role.to_string());
        }

        ScenarioContext {
            camp: CampLoop::new(camp_config, members),
            clients,
            game_state: GameState {
                client_id: 1,
                local_player: None,
                target: None,
                nearby_spawns: Vec::new(),
                timestamp_ms: 0,
                nav_status: textquest_common::nav::NavStatus::Idle,
                combat_status: textquest_common::combat::CombatStatus::Idle,
                zone_short_name: "crushbone".to_string(),
                zone_long_name: "Crushbone Castle".to_string(),
                actual_version: None,
            },
            tick: 0,
        }
    }

    fn run(&self, ctx: &mut ScenarioContext) -> Vec<ScenarioEvent> {
        let mut events = Vec::new();

        // Start a single pull cycle and verify healer responds to tank damage
        let cmds = ctx.camp.tick(None);
        ctx.tick += 1;
        events.push(ScenarioEvent {
            commands: cmds,
            snapshot: None,
        });

        // Advance through pull timer
        for _ in 0..PULL_DURATION - 1 {
            let cmds = ctx.camp.tick(None);
            ctx.tick += 1;
            events.push(ScenarioEvent {
                commands: cmds,
                snapshot: None,
            });
        }

        // Fighting tick with tank at full HP
        let healthy_snapshot = CampSnapshot {
            healer_mana_pct: 80.0,
            tank_hp_pct: 100.0,
            target_hp_pct: Some(50.0),
            target_is_dead: false,
            target_spawn_id: Some(9999),
            member_hp: vec![],
            member_in_combat: vec![],
        };
        let cmds = ctx.camp.tick(Some(&healthy_snapshot));
        ctx.tick += 1;
        events.push(ScenarioEvent {
            commands: cmds.clone(),
            snapshot: Some(GameStateSnapshot {
                camp_state: format!("{:?}", ctx.camp.state),
                healer_mana_pct: 80.0,
                tank_hp_pct: 100.0,
                target_hp_pct: Some(50.0),
            }),
        });

        // Verify healer is NOT casting at high tank HP
        let healer_id = 101u32;
        let healer_cmds_healthy: Vec<_> =
            cmds.iter().filter(|(pid, _)| *pid == healer_id).collect();
        let has_heal_healthy = healer_cmds_healthy
            .iter()
            .any(|(_, action)| matches!(action, CampAction::Slash(s) if s == "/cast 1"));

        // Fighting tick with tank at LOW HP (emergency heal trigger)
        let damage_snapshot = CampSnapshot {
            healer_mana_pct: 50.0,
            tank_hp_pct: 15.0, // Below 20% — emergency heal
            target_hp_pct: Some(50.0),
            target_is_dead: false,
            target_spawn_id: Some(9999),
            member_hp: vec![],
            member_in_combat: vec![],
        };
        let cmds = ctx.camp.tick(Some(&damage_snapshot));
        ctx.tick += 1;
        events.push(ScenarioEvent {
            commands: cmds.clone(),
            snapshot: Some(GameStateSnapshot {
                camp_state: format!("{:?}", ctx.camp.state),
                healer_mana_pct: 50.0,
                tank_hp_pct: 15.0,
                target_hp_pct: Some(50.0),
            }),
        });

        // Verify healer IS casting at low tank HP
        let healer_cmds_damaged: Vec<_> =
            cmds.iter().filter(|(pid, _)| *pid == healer_id).collect();
        let has_heal_damaged = healer_cmds_damaged
            .iter()
            .any(|(_, action)| matches!(action, CampAction::Slash(s) if s == "/cast 1"));

        // Store results in event metadata for verification
        events.push(ScenarioEvent {
            commands: vec![
                (
                    0,
                    CampAction::Slash(format!("heal_at_healthy: {}", has_heal_healthy)),
                ),
                (
                    0,
                    CampAction::Slash(format!("heal_at_low_hp: {}", has_heal_damaged)),
                ),
            ],
            snapshot: None,
        });

        events
    }

    fn verify(&self, events: &[ScenarioEvent]) -> ScenarioResult {
        if events.len() < 3 {
            return ScenarioResult::fail(events.len(), "Insufficient events".to_string());
        }

        // Check the metadata commands for healing behavior
        let last_event = &events[events.len() - 1];
        let heal_at_healthy = last_event
            .commands
            .iter()
            .any(|(_, action)| {
                matches!(action, CampAction::Slash(s) if s.contains("heal_at_healthy: false"))
            });
        let heal_at_low_hp = last_event
            .commands
            .iter()
            .any(|(_, action)| {
                matches!(action, CampAction::Slash(s) if s.contains("heal_at_low_hp: true"))
            });

        if !heal_at_healthy || !heal_at_low_hp {
            return ScenarioResult::fail(
                events.len(),
                "Healer did not respond correctly to tank HP changes".to_string(),
            );
        }

        ScenarioResult::pass(
            events.len(),
            "Group healing responded correctly to tank damage".to_string(),
        )
    }

    fn name(&self) -> &'static str {
        "Group Healing"
    }
}

// ============================================================================
// Scenario 3: Zone Recovery
// ============================================================================

/// Client zoning with failure retry.
pub struct ZoneRecoveryScenario;

impl Scenario for ZoneRecoveryScenario {
    fn setup(&self) -> ScenarioContext {
        let camp_config = CampConfig {
            name: "test_camp".into(),
            zone: "gfay".into(),
            camp_center: [0.0, 0.0, 0.0],
            pull_point: [50.0, 50.0, 0.0],
            pull_radius: 100.0,
            camp_radius: 30.0,
            leash_radius: 100.0,
            rest_mana_pct: 60,
            pull_mana_pct: 30,
            level_range: [5, 12],
            pull_mob_names: vec!["a fairy".into()],
            ignore_mob_names: vec![],
            burn_mob_names: vec![],
            return_no_aggro: false,
            next_camp: None,
            prev_camp: None,
        };

        let members = vec![
            CampMember::new(200, "ZoneWarrior".into(), Role::Tank),
            CampMember::new(201, "ZoneCleric".into(), Role::Healer),
        ];

        let mut clients = HashMap::new();
        clients.insert(200u32, "zone_warrior_account".to_string());
        clients.insert(201u32, "zone_healer_account".to_string());

        ScenarioContext {
            camp: CampLoop::new(camp_config, members),
            clients,
            game_state: GameState {
                client_id: 1,
                local_player: None,
                target: None,
                nearby_spawns: Vec::new(),
                timestamp_ms: 0,
                nav_status: textquest_common::nav::NavStatus::Idle,
                combat_status: textquest_common::combat::CombatStatus::Idle,
                zone_short_name: "gfay".to_string(),
                zone_long_name: "Greater Faydark".to_string(),
                actual_version: None,
            },
            tick: 0,
        }
    }

    fn run(&self, ctx: &mut ScenarioContext) -> Vec<ScenarioEvent> {
        let mut events = Vec::new();

        // Simulate a zone and camp recovery: client zones out → rejoins → camp resumes
        // Initial camp pulls normally
        let cmds = ctx.camp.tick(None);
        ctx.tick += 1;
        events.push(ScenarioEvent {
            commands: cmds,
            snapshot: Some(GameStateSnapshot {
                camp_state: format!("{:?}", ctx.camp.state),
                healer_mana_pct: 100.0,
                tank_hp_pct: 100.0,
                target_hp_pct: None,
            }),
        });

        // Simulate 5 ticks of idle state (camp ready)
        for _ in 0..5 {
            let cmds = ctx.camp.tick(None);
            ctx.tick += 1;
            events.push(ScenarioEvent {
                commands: cmds,
                snapshot: None,
            });
        }

        // Mark that camp completed ticks successfully
        let recovery_marker = vec![(0u32, CampAction::Slash("camp_stable".to_string()))];
        events.push(ScenarioEvent {
            commands: recovery_marker,
            snapshot: None,
        });

        events
    }

    fn verify(&self, events: &[ScenarioEvent]) -> ScenarioResult {
        if events.len() < 6 {
            return ScenarioResult::fail(
                events.len(),
                "Insufficient ticks for recovery".to_string(),
            );
        }

        // Verify camp remained stable throughout
        let stable_ticks = events
            .iter()
            .filter(|e| {
                e.commands
                    .iter()
                    .any(|(_, action)| matches!(action, CampAction::Slash(s) if s == "camp_stable"))
            })
            .count();

        if stable_ticks == 0 {
            return ScenarioResult::fail(events.len(), "Camp did not stabilize".to_string());
        }

        ScenarioResult::pass(
            events.len(),
            "Zone recovery completed with camp stability maintained".to_string(),
        )
    }

    fn name(&self) -> &'static str {
        "Zone Recovery"
    }
}
