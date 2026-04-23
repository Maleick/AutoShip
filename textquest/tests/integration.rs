#![cfg(windows)]

//! Platform-independent integration tests for the Login -> Enter World ->
//! Navigate pipeline. Uses the pure FSM APIs directly — no live EQ process, no
//! mocks, no trait abstractions.

use std::collections::HashMap;

use textquest::{
    camp::{
        config::CampConfig,
        state::{CampAction, CampLoop, CampMember, CampSnapshot, CampState, PULL_DURATION, Role},
    },
    config::{LaunchConfig, RetryConfig, ServerConfig},
    launcher::{
        coordinator::LaunchCoordinator,
        login_sm::{LoginAction, LoginEvent, LoginStateMachine},
        post_login::{PostLoginEvent, PostLoginSequencer},
    },
    nav::router::{GroupRouter, TravelPlan, TravelStep, generate_zone_staggers, plan_group_travel},
};
use textquest_common::{
    ipc::Command,
    login::{AccountInfo, LoginError, LoginPhase},
    nav::Waypoint,
    types::GameState,
};

// ============================================================================
// Helpers
// ============================================================================

fn make_account(name: &str, character: &str, class: &str) -> AccountInfo {
    AccountInfo {
        account_name: name.to_string(),
        character_name: character.to_string(),
        class_name: class.to_string(),
        level: 60,
        group_id: 1,
        server_name: "TestServer".to_string(),
    }
}

fn make_game_state() -> GameState {
    GameState {
        client_id: 1,
        local_player: None,
        target: None,
        nearby_spawns: Vec::new(),
        timestamp_ms: 0,
        nav_status: textquest_common::nav::NavStatus::Idle,
        combat_status: textquest_common::combat::CombatStatus::Idle,
        zone_short_name: String::new(),
        zone_long_name: String::new(),
        active_buffs: Vec::new(),
        pet: None,
        actual_version: None,
    }
}

fn test_camp_config() -> CampConfig {
    CampConfig {
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
    }
}

fn test_camp_members() -> Vec<CampMember> {
    vec![
        CampMember::new(100, "Warrior01".into(), Role::Tank),
        CampMember::new(101, "Cleric01".into(), Role::Healer),
        CampMember::new(102, "Enchanter01".into(), Role::CC),
        CampMember::new(103, "Bard01".into(), Role::Puller),
        CampMember::new(104, "Ranger01".into(), Role::Dps),
        CampMember::new(105, "Ranger02".into(), Role::Dps),
    ]
}

fn test_configs() -> (LaunchConfig, RetryConfig, ServerConfig) {
    let launch = LaunchConfig {
        eq_path: "/tmp/fake_eq".to_string(),
        stagger_min_secs: 0,
        stagger_max_secs: 0,
        max_concurrent_launches: 3,
        launch_args: Vec::new(),
        max_working_set_mb: 800,
    };
    let retry = RetryConfig {
        max_retries: 3,
        base_backoff_secs: 1,
        max_backoff_secs: 60,
        backoff_multiplier: 2.0,
        backoff_jitter: 0.0,
        mass_failure_threshold: 5,
        mass_failure_window_secs: 60,
    };
    let server = ServerConfig {
        name: "TestServer".to_string(),
        status_url: None,
        status_check_timeout_secs: 10,
    };
    (launch, retry, server)
}

/// Drive a LoginStateMachine through the full happy path and return it in Ready
/// state.
fn drive_login_to_ready(sm: &mut LoginStateMachine) {
    sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
    sm.advance(LoginEvent::LoginScreenDetected);
    sm.advance(LoginEvent::CredentialsSent);
    sm.advance(LoginEvent::ServerSelected);
    sm.advance(LoginEvent::CharacterSelected);
    sm.advance(LoginEvent::ZoneInComplete);
    sm.advance(LoginEvent::PlayerDataConfirmed {
        name: sm.account_info.character_name.clone(),
        class_name: sm.account_info.class_name.clone(),
    });
}

// ============================================================================
// Test 1: Login -> Enter World -> Navigate (end-to-end single client)
// ============================================================================

#[test]
fn login_enter_world_navigate_pipeline() {
    // -- Phase 1: Login FSM --
    let account = make_account("acct1", "Frostreaver", "Warrior");
    let mut login = LoginStateMachine::new(1, account);

    // Drive through every login phase, verifying transitions
    let action = login.advance(LoginEvent::ProcessStarted { pid: 1234 });
    assert!(matches!(action, LoginAction::None));
    assert!(matches!(login.phase, LoginPhase::ProcessLaunching));

    let action = login.advance(LoginEvent::LoginScreenDetected);
    assert!(matches!(action, LoginAction::SendCredentials));
    assert!(matches!(login.phase, LoginPhase::AtLoginScreen));

    let action = login.advance(LoginEvent::CredentialsSent);
    match &action {
        LoginAction::SelectServer { name } => assert_eq!(name, "TestServer"),
        other => panic!(
            "expected SelectServer, got {:?}",
            std::mem::discriminant(other)
        ),
    }

    let action = login.advance(LoginEvent::ServerSelected);
    match &action {
        LoginAction::SelectCharacter { name } => assert_eq!(name, "Frostreaver"),
        other => panic!(
            "expected SelectCharacter, got {:?}",
            std::mem::discriminant(other)
        ),
    }

    let action = login.advance(LoginEvent::CharacterSelected);
    assert!(matches!(action, LoginAction::WaitForZone));

    let action = login.advance(LoginEvent::ZoneInComplete);
    assert!(matches!(action, LoginAction::BeginPostLogin));
    assert!(matches!(login.phase, LoginPhase::InWorld));

    let action = login.advance(LoginEvent::PlayerDataConfirmed {
        name: "Frostreaver".to_string(),
        class_name: "Warrior".to_string(),
    });
    assert!(matches!(action, LoginAction::None));
    assert!(matches!(login.phase, LoginPhase::Ready));
    assert!(login.is_terminal());

    // -- Phase 2: Post-login sequencer --
    let waypoints = vec![
        Waypoint::new(100.0, 200.0, 0.0),
        Waypoint::new(150.0, 250.0, 5.0),
    ];
    let mut seq = PostLoginSequencer::new(1, 1, waypoints);
    let state = make_game_state();

    // Initial state: should generate JoinGroup command
    let cmd = seq.next_command(&state);
    assert!(matches!(cmd, Some(Command::JoinGroup { group_id: 1 })));

    // Advance: group joined -> buffing
    seq.advance(PostLoginEvent::GroupJoined);
    assert!(!seq.is_ready());

    // Buffing phase with waypoints -> should generate NavigateTo
    let cmd = seq.next_command(&state);
    assert!(matches!(cmd, Some(Command::NavigateTo { .. })));

    // Advance: buffs applied -> navigating to camp
    seq.advance(PostLoginEvent::BuffsApplied);
    assert!(!seq.is_ready());

    // Navigation phase generates no command (nav system is autonomous)
    let cmd = seq.next_command(&state);
    assert!(cmd.is_none());

    // Camp reached -> ready
    seq.advance(PostLoginEvent::CampReached);
    assert!(seq.is_ready());

    // -- Phase 3: Travel plan verification --
    let steps = vec![
        TravelStep::StaggerWait {
            min_secs: 5,
            max_secs: 5,
        },
        TravelStep::WalkTo {
            waypoints: vec![Waypoint::new(100.0, 200.0, 0.0)],
        },
        TravelStep::ZoneTo {
            zone_name: "gfay".to_string(),
            zone_line_pos: Waypoint::new(500.0, 600.0, 0.0),
        },
    ];
    let mut plan = TravelPlan::new(1, steps);

    // Verify traversal through each step
    assert!(!plan.is_complete());
    assert!(matches!(
        plan.current(),
        Some(TravelStep::StaggerWait { .. })
    ));

    assert!(plan.advance());
    assert!(matches!(plan.current(), Some(TravelStep::WalkTo { .. })));

    assert!(plan.advance());
    assert!(matches!(plan.current(), Some(TravelStep::ZoneTo { .. })));
    if let Some(TravelStep::ZoneTo { zone_name, .. }) = plan.current() {
        assert_eq!(zone_name, "gfay");
    }
}

// ============================================================================
// Test 2: Multi-client launch coordination
// ============================================================================

#[test]
fn multi_client_launch_coordination() {
    let (launch, retry, server) = test_configs();
    let mut coord = LaunchCoordinator::new(launch, retry, server);

    // Enqueue 3 clients
    coord.enqueue(1, make_account("acct1", "Warrior01", "Warrior"));
    coord.enqueue(2, make_account("acct2", "Cleric01", "Cleric"));
    coord.enqueue(3, make_account("acct3", "Wizard01", "Wizard"));

    assert_eq!(coord.pending_count(), 3);
    assert_eq!(coord.active_count(), 0);
    assert!(!coord.is_paused());
}

#[test]
fn coordinator_tick_attempts_launch_from_queue() {
    let (launch, retry, server) = test_configs();
    let mut coord = LaunchCoordinator::new(launch, retry, server);

    coord.enqueue(1, make_account("acct1", "Warrior01", "Warrior"));
    coord.enqueue(2, make_account("acct2", "Cleric01", "Cleric"));

    // On non-Windows, spawn_eq_client fails, producing ClientFailed events.
    // On Windows without a real EQ install, it also fails. Either way, the
    // coordinator dequeues and attempts to launch.
    let events = coord.tick();
    assert!(
        !events.is_empty(),
        "tick should produce events when clients are queued"
    );
    assert_eq!(
        coord.pending_count(),
        1,
        "exactly one client should be dequeued per tick"
    );
    // Verify the event is a launch attempt (ClientLaunched or ClientFailed)
    assert!(
        events.iter().any(|e| matches!(
            e,
            textquest::launcher::coordinator::CoordinatorEvent::ClientFailed { .. }
                | textquest::launcher::coordinator::CoordinatorEvent::ClientLaunched { .. }
        )),
        "tick should produce a launch-related event"
    );
}

#[test]
fn coordinator_paused_state() {
    let (launch, retry, server) = test_configs();
    let mut coord = LaunchCoordinator::new(launch, retry, server);
    coord.enqueue(1, make_account("acct1", "Warrior01", "Warrior"));

    assert!(!coord.is_paused());
    coord.resume(); // no-op on unpaused
    assert!(!coord.is_paused());
}

// ============================================================================
// Test 3: Login FSM error handling
// ============================================================================

#[test]
fn login_wrong_password_is_fatal() {
    let account = make_account("acct1", "Frostreaver", "Warrior");
    let mut sm = LoginStateMachine::new(1, account);
    sm.advance(LoginEvent::ProcessStarted { pid: 1234 });

    let action = sm.advance(LoginEvent::ErrorDetected {
        error: LoginError::WrongPassword,
    });
    assert!(matches!(action, LoginAction::Abort { .. }));
    assert!(matches!(sm.phase, LoginPhase::Failed { .. }));
    assert!(sm.is_terminal());
}

#[test]
fn login_server_full_retries_then_aborts() {
    let account = make_account("acct1", "Frostreaver", "Warrior");
    let mut sm = LoginStateMachine::new(1, account);
    sm.advance(LoginEvent::ProcessStarted { pid: 1234 });

    // First two attempts: retry
    let action = sm.advance(LoginEvent::ErrorDetected {
        error: LoginError::ServerFull,
    });
    assert!(matches!(action, LoginAction::Retry { .. }));

    let action = sm.advance(LoginEvent::ErrorDetected {
        error: LoginError::ServerFull,
    });
    assert!(matches!(action, LoginAction::Retry { .. }));

    // Third attempt: abort (MAX_ATTEMPTS = 3)
    let action = sm.advance(LoginEvent::ErrorDetected {
        error: LoginError::ServerFull,
    });
    assert!(matches!(action, LoginAction::Abort { .. }));
    assert!(sm.is_terminal());
}

#[test]
fn login_character_mismatch_aborts() {
    let account = make_account("acct1", "Frostreaver", "Warrior");
    let mut sm = LoginStateMachine::new(1, account);
    drive_login_to_ready(&mut sm);

    // Reset to test mismatch path (create new SM)
    let account = make_account("acct1", "Frostreaver", "Warrior");
    let mut sm = LoginStateMachine::new(1, account);
    sm.advance(LoginEvent::ProcessStarted { pid: 1234 });
    sm.advance(LoginEvent::LoginScreenDetected);
    sm.advance(LoginEvent::CredentialsSent);
    sm.advance(LoginEvent::ServerSelected);
    sm.advance(LoginEvent::CharacterSelected);
    sm.advance(LoginEvent::ZoneInComplete);

    let action = sm.advance(LoginEvent::PlayerDataConfirmed {
        name: "WrongCharacter".to_string(),
        class_name: "Warrior".to_string(),
    });
    assert!(matches!(action, LoginAction::Abort { .. }));
    assert!(matches!(sm.phase, LoginPhase::Failed { .. }));
}

#[test]
fn login_mass_failure_triggers_pause_all() {
    let account = make_account("acct1", "Frostreaver", "Warrior");
    let mut sm = LoginStateMachine::new(1, account);
    sm.advance(LoginEvent::ProcessStarted { pid: 1234 });

    let action = sm.advance(LoginEvent::ErrorDetected {
        error: LoginError::MassFailure,
    });
    assert!(matches!(action, LoginAction::PauseAll));
    assert!(sm.is_terminal());
}

// ============================================================================
// Test 4: Post-login sequencer variations
// ============================================================================

#[test]
fn post_login_no_waypoints_skips_navigation() {
    let mut seq = PostLoginSequencer::new(1, 42, vec![]);
    let state = make_game_state();

    // Should start with JoinGroup
    match seq.next_command(&state) {
        Some(Command::JoinGroup { group_id }) => assert_eq!(group_id, 42),
        other => panic!("expected JoinGroup, got {other:?}"),
    }

    seq.advance(PostLoginEvent::GroupJoined);

    // Buffing phase with no waypoints -> ReportReady
    match seq.next_command(&state) {
        Some(Command::ReportReady) => {}
        other => panic!("expected ReportReady, got {other:?}"),
    }

    seq.advance(PostLoginEvent::BuffsApplied);
    assert!(seq.is_ready(), "no waypoints means skip straight to ready");
}

#[test]
fn post_login_with_waypoints_generates_navigate() {
    let waypoints = vec![Waypoint::new(500.0, 600.0, 10.0)];
    let mut seq = PostLoginSequencer::new(1, 1, waypoints);
    let state = make_game_state();

    seq.advance(PostLoginEvent::GroupJoined);

    // Buffing phase with waypoints -> NavigateTo
    let cmd = seq.next_command(&state);
    match cmd {
        Some(Command::NavigateTo { waypoints }) => {
            assert_eq!(waypoints.len(), 1);
            assert_eq!(waypoints[0].x, 500.0);
        }
        other => panic!("expected NavigateTo, got {other:?}"),
    }

    seq.advance(PostLoginEvent::BuffsApplied);
    assert!(!seq.is_ready(), "should be navigating, not ready");

    seq.advance(PostLoginEvent::CampReached);
    assert!(seq.is_ready());
}

#[test]
fn post_login_joining_group_phase_produces_apply_buffs() {
    let mut seq = PostLoginSequencer::new(1, 42, vec![]);
    let state = make_game_state();

    // NotStarted -> next_command returns JoinGroup
    let cmd = seq.next_command(&state);
    assert!(
        matches!(cmd, Some(Command::JoinGroup { group_id: 42 })),
        "NotStarted phase should produce JoinGroup, got {cmd:?}"
    );

    // Mark the JoinGroup command as dispatched -> transitions to JoiningGroup
    seq.mark_dispatched();
    assert!(
        matches!(
            seq.phase(),
            textquest::client::session::PostLoginPhase::JoiningGroup
        ),
        "phase should be JoiningGroup after dispatch, got {:?}",
        seq.phase()
    );

    // JoiningGroup -> next_command returns ApplyBuffs (waiting for confirmation)
    let cmd = seq.next_command(&state);
    assert!(
        matches!(cmd, Some(Command::ApplyBuffs)),
        "JoiningGroup phase should produce ApplyBuffs, got {cmd:?}"
    );

    // GroupJoined event -> transitions to Buffing
    seq.advance(PostLoginEvent::GroupJoined);
    assert!(
        matches!(
            seq.phase(),
            textquest::client::session::PostLoginPhase::Buffing
        ),
        "phase should be Buffing after GroupJoined event, got {:?}",
        seq.phase()
    );

    // Buffing with no waypoints -> ReportReady
    let cmd = seq.next_command(&state);
    assert!(
        matches!(cmd, Some(Command::ReportReady)),
        "Buffing phase with no waypoints should produce ReportReady, got {cmd:?}"
    );
}

// ============================================================================
// Test 5: Camp loop — Idle -> Pulling -> Fighting -> Looting -> Medding -> Idle
// ============================================================================

#[test]
fn camp_loop_full_cycle_with_snapshot() {
    let mut camp = CampLoop::new(test_camp_config(), test_camp_members());

    // Tick 1: Idle -> Pulling (healer mana unknown, defaults to allow pull)
    let cmds = camp.tick(None);
    assert!(matches!(camp.state, CampState::Pulling { .. }));
    // Puller (pid 103) should get /target and /attack
    let puller_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 103).collect();
    assert!(puller_cmds.len() >= 2);
    assert!(puller_cmds.iter().any(|(_, cmd)| cmd.contains("/target")));
    assert!(puller_cmds.iter().any(|(_, cmd)| cmd == "/attack"));

    // Advance through pull duration (PULL_DURATION - 1 remaining ticks)
    for _ in 0..PULL_DURATION - 1 {
        let cmds = camp.tick(None);
        assert!(matches!(camp.state, CampState::Pulling { .. }));
        // No transition commands during pull wait
        assert!(cmds.iter().all(|(_, cmd)| cmd.contains("/autoinventory")));
    }

    // Final pull tick: Pulling -> Fighting
    let cmds = camp.tick(None);
    assert!(matches!(camp.state, CampState::Fighting { .. }));
    // Tank should get /assist and /attack
    let tank_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 100).collect();
    assert!(
        tank_cmds
            .iter()
            .any(|(_, cmd)| cmd.contains("/assist Bard01"))
    );
    assert!(tank_cmds.iter().any(|(_, cmd)| cmd == "/attack"));
    // DPS should assist tank
    let dps_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 104).collect();
    assert!(
        dps_cmds
            .iter()
            .any(|(_, cmd)| cmd.contains("/assist Warrior01"))
    );
    // Healer should target tank
    let healer_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 101).collect();
    assert!(
        healer_cmds
            .iter()
            .any(|(_, cmd)| cmd.contains("/target Warrior01"))
    );

    // Use snapshot to signal target dead -> immediate transition to Looting
    let dead_snapshot = CampSnapshot {
        healer_mana_pct: 80.0,
        tank_hp_pct: 100.0,
        target_hp_pct: Some(0.0),
        target_is_dead: true,
        target_spawn_id: Some(9999),
        member_hp: vec![],
        member_in_combat: vec![],
    };
    let cmds = camp.tick(Some(&dead_snapshot));
    assert!(matches!(camp.state, CampState::Looting { .. }));
    // Everyone should get /attack off and CombatDisengage
    assert!(
        cmds.iter()
            .any(|(pid, cmd)| *pid == 100 && cmd == "/attack off")
    );
    assert!(
        cmds.iter()
            .any(|(_, cmd)| matches!(cmd, CampAction::CombatDisengage))
    );

    // Looting -> Medding: loot cycle has multi-tick delays per corpse.
    // Tick until the looting phase completes and we transition to Medding.
    let mut medding_cmds = Vec::new();
    for _ in 0..50 {
        let cmds = camp.tick(None);
        if matches!(camp.state, CampState::Medding { .. }) {
            medding_cmds = cmds;
            break;
        }
    }
    assert!(
        matches!(camp.state, CampState::Medding { .. }),
        "camp should transition to Medding after looting, got {:?}",
        camp.state
    );
    // Casters should /sit on medding transition
    let healer_cmds: Vec<_> = medding_cmds.iter().filter(|(pid, _)| *pid == 101).collect();
    assert!(healer_cmds.iter().any(|(_, cmd)| cmd == "/sit"));

    // Medding -> Idle when healer mana is above threshold
    let mana_ready_snapshot = CampSnapshot {
        healer_mana_pct: 80.0,
        tank_hp_pct: 100.0,
        target_hp_pct: None,
        target_is_dead: false,
        target_spawn_id: None,
        member_hp: vec![],
        member_in_combat: vec![],
    };
    let cmds = camp.tick(Some(&mana_ready_snapshot));
    assert!(matches!(camp.state, CampState::Idle));
    // Everyone should /stand
    assert!(cmds.iter().any(|(pid, cmd)| *pid == 100 && cmd == "/stand"));
}

#[test]
fn camp_loop_emergency_heal_on_low_tank_hp() {
    let mut camp = CampLoop::new(test_camp_config(), test_camp_members());

    // Drive to Fighting state
    camp.tick(None); // Idle -> Pulling
    for _ in 0..PULL_DURATION - 1 {
        camp.tick(None);
    }
    camp.tick(None); // Pulling -> Fighting
    assert!(matches!(camp.state, CampState::Fighting { .. }));

    // Tick with tank at low HP
    let low_tank_snapshot = CampSnapshot {
        healer_mana_pct: 50.0,
        tank_hp_pct: 15.0,
        target_hp_pct: Some(50.0),
        target_is_dead: false,
        target_spawn_id: Some(9999),
        member_hp: vec![],
        member_in_combat: vec![],
    };
    let cmds = camp.tick(Some(&low_tank_snapshot));
    // Healer (pid 101) should get emergency /cast 1
    let healer_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 101).collect();
    assert!(
        healer_cmds.iter().any(|(_, cmd)| cmd == "/cast 1"),
        "healer should cast emergency heal when tank HP < 20%"
    );
}

#[test]
fn camp_idle_respects_healer_mana_threshold() {
    let mut camp = CampLoop::new(test_camp_config(), test_camp_members());

    // Provide snapshot with low healer mana — camp should NOT pull
    let low_mana = CampSnapshot {
        healer_mana_pct: 10.0,
        tank_hp_pct: 100.0,
        target_hp_pct: None,
        target_is_dead: false,
        target_spawn_id: None,
        member_hp: vec![],
        member_in_combat: vec![],
    };
    let cmds = camp.tick(Some(&low_mana));
    // Should stay Idle since healer mana is below pull_mana_pct (30)
    assert!(
        matches!(camp.state, CampState::Idle),
        "camp should not pull when healer mana is low"
    );
    assert!(cmds.iter().all(|(_, cmd)| cmd.contains("/autoinventory")));
}

// ============================================================================
// Test 6: Zone routing
// ============================================================================

#[test]
fn zone_routing_generates_staggered_travel_plans() {
    let client_ids = vec![1, 2, 3, 4, 5];
    let class_map: HashMap<u32, u8> = client_ids.iter().map(|&id| (id, 1u8)).collect();

    let plans = plan_group_travel(&client_ids, &class_map, "ecommons", "gfay");
    assert_eq!(plans.len(), 5, "one plan per client");

    for plan in &plans {
        assert!(!plan.is_complete());
        assert!(plan.current().is_some());
        // Each plan should start with a StaggerWait
        assert!(
            matches!(plan.current(), Some(TravelStep::StaggerWait { .. })),
            "travel should begin with stagger delay"
        );
    }
}

#[test]
fn zone_stagger_delays_are_within_range() {
    let ids: Vec<u32> = (1..=36).collect(); // full 36-box
    let staggers = generate_zone_staggers(&ids, 5, 60, 42);

    assert_eq!(staggers.len(), 36);
    for (&id, &delay) in &staggers {
        assert!(
            (5..=60).contains(&delay),
            "client {id} stagger {delay} outside [5, 60]"
        );
    }
}

#[test]
fn zone_stagger_delays_are_deterministic() {
    let ids = vec![1, 2, 3, 4, 5];
    let a = generate_zone_staggers(&ids, 5, 60, 42);
    let b = generate_zone_staggers(&ids, 5, 60, 42);
    assert_eq!(a, b, "same seed must produce identical staggers");
}

#[test]
fn travel_plan_with_zone_transitions() {
    let steps = vec![
        TravelStep::StaggerWait {
            min_secs: 3,
            max_secs: 10,
        },
        TravelStep::WalkTo {
            waypoints: vec![Waypoint::new(0.0, 0.0, 0.0), Waypoint::new(100.0, 0.0, 0.0)],
        },
        TravelStep::ZoneTo {
            zone_name: "nro".to_string(),
            zone_line_pos: Waypoint::new(200.0, 0.0, 0.0),
        },
        TravelStep::StaggerWait {
            min_secs: 5,
            max_secs: 15,
        },
        TravelStep::WalkTo {
            waypoints: vec![Waypoint::new(0.0, 50.0, 0.0)],
        },
        TravelStep::ZoneTo {
            zone_name: "sro".to_string(),
            zone_line_pos: Waypoint::new(300.0, 100.0, 0.0),
        },
    ];

    let mut plan = TravelPlan::new(1, steps);

    // Walk through each step, counting zone transitions
    let mut zone_transitions = 0;
    let mut stagger_waits = 0;
    loop {
        match plan.current() {
            Some(TravelStep::ZoneTo { zone_name, .. }) => {
                zone_transitions += 1;
                assert!(!zone_name.is_empty(), "zone name should not be empty");
            }
            Some(TravelStep::StaggerWait { min_secs, max_secs }) => {
                stagger_waits += 1;
                assert!(min_secs <= max_secs, "min should be <= max");
            }
            Some(TravelStep::WalkTo { waypoints }) => {
                assert!(!waypoints.is_empty(), "walk step should have waypoints");
            }
            _ => break,
        }
        if !plan.advance() {
            break;
        }
    }

    assert_eq!(zone_transitions, 2, "plan should have 2 zone transitions");
    assert_eq!(stagger_waits, 2, "plan should have 2 stagger waits");
}

#[test]
fn group_router_with_porters() {
    let mut router = GroupRouter::new();
    assert!(!router.has_porters());

    // Register a druid and wizard as porters
    router.set_porters(vec![10, 20]);
    assert!(router.has_porters());

    let client_ids = vec![1, 2, 3, 10, 20];
    let class_map: HashMap<u32, u8> = vec![(1, 1), (2, 2), (3, 1), (10, 6), (20, 12)]
        .into_iter()
        .collect();

    let plans = router.plan_travel(&client_ids, &class_map, "ecommons", "wakening");
    assert_eq!(plans.len(), 5);

    // Each plan should have at least one step
    for plan in &plans {
        assert!(plan.current().is_some());
    }
}

// ============================================================================
// Test 7: Login FSM DLL-reported phase transitions
// ============================================================================

#[test]
fn dll_reported_in_world_triggers_post_login() {
    let account = make_account("acct1", "Frostreaver", "Warrior");
    let mut sm = LoginStateMachine::new(1, account);
    sm.advance(LoginEvent::ProcessStarted { pid: 1234 });

    let action = sm.advance(LoginEvent::DllReported {
        phase: LoginPhase::InWorld,
    });
    assert!(matches!(sm.phase, LoginPhase::InWorld));
    assert!(matches!(action, LoginAction::BeginPostLogin));
}

#[test]
fn dll_reported_ready_sets_terminal() {
    let account = make_account("acct1", "Frostreaver", "Warrior");
    let mut sm = LoginStateMachine::new(1, account);
    sm.advance(LoginEvent::ProcessStarted { pid: 1234 });

    let action = sm.advance(LoginEvent::DllReported {
        phase: LoginPhase::Ready,
    });
    assert!(matches!(sm.phase, LoginPhase::Ready));
    assert!(matches!(action, LoginAction::None));
    assert!(sm.is_terminal());
}

// ============================================================================
// Test 8: Camp snapshot-driven transitions
// ============================================================================

#[test]
fn camp_snapshot_driven_fight_to_loot_on_target_death() {
    let mut camp = CampLoop::new(test_camp_config(), test_camp_members());

    // Drive to fighting state
    camp.tick(None); // Idle -> Pulling
    for _ in 0..PULL_DURATION - 1 {
        camp.tick(None);
    }
    camp.tick(None); // Pulling -> Fighting
    assert!(matches!(camp.state, CampState::Fighting { .. }));

    // Target still alive — should remain fighting
    let alive_snapshot = CampSnapshot {
        healer_mana_pct: 80.0,
        tank_hp_pct: 90.0,
        target_hp_pct: Some(50.0),
        target_is_dead: false,
        target_spawn_id: Some(9999),
        member_hp: vec![],
        member_in_combat: vec![],
    };
    camp.tick(Some(&alive_snapshot));
    assert!(matches!(camp.state, CampState::Fighting { .. }));

    // Target dies -> Looting
    let dead_snapshot = CampSnapshot {
        healer_mana_pct: 80.0,
        tank_hp_pct: 90.0,
        target_hp_pct: Some(0.0),
        target_is_dead: true,
        target_spawn_id: Some(9999),
        member_hp: vec![],
        member_in_combat: vec![],
    };
    camp.tick(Some(&dead_snapshot));
    assert!(matches!(camp.state, CampState::Looting { .. }));
}

// ============================================================================
// Test 10: Travel System — group travel, straggler handling, zone FSM
// ============================================================================

#[test]
fn group_travel_all_clients_receive_plans() {
    let client_ids: Vec<u32> = (1..=6).collect();
    let class_map: HashMap<u32, u8> = client_ids.iter().map(|&id| (id, 1u8)).collect();
    let plans = plan_group_travel(&client_ids, &class_map, "qeynos", "highkeep");
    assert_eq!(plans.len(), 6, "every client must get a travel plan");
    for plan in &plans {
        assert!(
            plan.current().is_some(),
            "each plan must have at least one step"
        );
    }
}

#[test]
fn group_travel_stagger_step_is_first() {
    let client_ids = vec![1u32, 2, 3];
    let class_map: HashMap<u32, u8> = client_ids.iter().map(|&id| (id, 1u8)).collect();
    let plans = plan_group_travel(&client_ids, &class_map, "ecommons", "nro");
    for plan in &plans {
        assert!(
            matches!(plan.current(), Some(TravelStep::StaggerWait { .. })),
            "first step should be a stagger wait to avoid simultaneous zone"
        );
    }
}

#[test]
fn stagger_delays_spread_across_clients() {
    // With a wide window (5-60s) and 10 clients, we expect non-uniform delays.
    let client_ids: Vec<u32> = (1..=10).collect();
    let staggers = generate_zone_staggers(&client_ids, 5, 60, 7);
    let mut unique_delays: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for &delay in staggers.values() {
        assert!(
            (5..=60).contains(&delay),
            "delay {delay} out of [5, 60] range"
        );
        unique_delays.insert(delay);
    }
    // At least 2 different delays expected across 10 clients.
    assert!(
        unique_delays.len() >= 2,
        "stagger should produce varied delays, got {unique_delays:?}"
    );
}

#[test]
fn straggler_single_client_receives_solo_plan() {
    // A "straggler" is a client that didn't make the group zone.
    // The router should produce a valid plan for even a single client.
    let client_ids = vec![42u32];
    let class_map: HashMap<u32, u8> = [(42, 5u8)].into_iter().collect();
    let plans = plan_group_travel(&client_ids, &class_map, "oasis", "sro");
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].client_id, 42);
    assert!(plans[0].current().is_some());
}

#[test]
fn travel_plan_sequential_step_advancement() {
    // A multi-step plan: stagger → walk → zone → stagger → walk → zone.
    let steps = vec![
        TravelStep::StaggerWait {
            min_secs: 10,
            max_secs: 10,
        },
        TravelStep::WalkTo {
            waypoints: vec![
                Waypoint::new(50.0, 0.0, 0.0),
                Waypoint::new(100.0, 0.0, 0.0),
            ],
        },
        TravelStep::ZoneTo {
            zone_name: "ecommons".to_string(),
            zone_line_pos: Waypoint::new(100.0, 0.0, 0.0),
        },
        TravelStep::StaggerWait {
            min_secs: 5,
            max_secs: 15,
        },
        TravelStep::ZoneTo {
            zone_name: "nro".to_string(),
            zone_line_pos: Waypoint::new(200.0, 0.0, 0.0),
        },
    ];
    let mut plan = TravelPlan::new(99, steps);
    assert!(!plan.is_complete());

    // Advance through all steps and verify step types in order.
    let expected_kinds = ["StaggerWait", "WalkTo", "ZoneTo", "StaggerWait", "ZoneTo"];
    for expected in &expected_kinds {
        let current = plan.current().expect("step should exist");
        let kind = match current {
            TravelStep::StaggerWait { .. } => "StaggerWait",
            TravelStep::WalkTo { .. } => "WalkTo",
            TravelStep::ZoneTo { .. } => "ZoneTo",
            TravelStep::PortTo { .. } => "PortTo",
            TravelStep::RelocateTo { .. } => "RelocateTo",
        };
        assert_eq!(kind, *expected, "unexpected step kind");
        plan.advance();
    }
}

#[test]
fn group_router_with_porters_produces_plans_for_all() {
    let mut router = GroupRouter::new();
    // Druid (6) and Wizard (12) are porter classes.
    router.set_porters(vec![10u32, 20]);

    let client_ids = vec![1u32, 2, 3, 10, 20];
    let class_map: HashMap<u32, u8> = vec![(1, 1), (2, 2), (3, 1), (10, 6), (20, 12)]
        .into_iter()
        .collect();

    let plans = router.plan_travel(&client_ids, &class_map, "gfay", "wakening");
    assert_eq!(
        plans.len(),
        5,
        "each client must get a plan even with porters registered"
    );
    for plan in &plans {
        assert!(
            plan.current().is_some(),
            "porter-aware plan must have at least one step"
        );
    }
}

#[test]
fn zone_failure_stagger_retry_recovery_action() {
    use textquest::zoning::{RecoveryAction, ZoneFailureCode, ZoneFailureState};

    // GeneralFailure → RetryZone.
    let state = ZoneFailureState::new(ZoneFailureCode::GeneralFailure, 3);
    assert_eq!(
        state.recovery_action,
        RecoveryAction::RetryZone,
        "GeneralFailure should produce RetryZone recovery"
    );
    assert_eq!(state.retry_count, 0, "fresh failure starts with 0 retries");
    assert_eq!(state.max_retries, 3);
}

#[test]
fn zone_failure_abandon_codes_map_correctly() {
    use textquest::zoning::{RecoveryAction, ZoneFailureCode, ZoneFailureState};

    for abandon_code in [
        ZoneFailureCode::LevelTooLow,
        ZoneFailureCode::LevelTooHigh,
        ZoneFailureCode::RaidLockoutActive,
        ZoneFailureCode::GuildHallUnavailable,
        ZoneFailureCode::WrongType,
    ] {
        let state = ZoneFailureState::new(abandon_code, 3);
        assert_eq!(
            state.recovery_action,
            RecoveryAction::Abandon,
            "{abandon_code:?} should map to Abandon recovery"
        );
    }
}

#[test]
fn zone_failure_retryable_codes_map_correctly() {
    use textquest::zoning::{RecoveryAction, ZoneFailureCode, ZoneFailureState};

    for retry_code in [
        ZoneFailureCode::SpellResisted,
        ZoneFailureCode::AlreadyZoning,
        ZoneFailureCode::ZoneLoadTimeout,
        ZoneFailureCode::PortSpellExpired,
    ] {
        let state = ZoneFailureState::new(retry_code, 3);
        assert_eq!(
            state.recovery_action,
            RecoveryAction::RetryZone,
            "{retry_code:?} should map to RetryZone"
        );
    }
}

#[test]
fn zone_failure_combat_maps_to_wait_out_of_combat() {
    use textquest::zoning::{RecoveryAction, ZoneFailureCode, ZoneFailureState};

    let state = ZoneFailureState::new(ZoneFailureCode::PlayerInCombat, 3);
    assert_eq!(state.recovery_action, RecoveryAction::WaitOutOfCombat);
}

#[test]
fn zone_failure_mana_maps_to_wait_mana_regen() {
    use textquest::zoning::{RecoveryAction, ZoneFailureCode, ZoneFailureState};

    let state = ZoneFailureState::new(ZoneFailureCode::InsufficientMana, 3);
    assert_eq!(state.recovery_action, RecoveryAction::WaitManaRegen);
}

#[test]
fn zone_nav_fsm_starts_idle() {
    use textquest::nav::zone_transition::ZoneTransitionFsm;

    let fsm = ZoneTransitionFsm::new(1);
    assert!(fsm.is_idle(), "freshly created FSM must be idle");
    assert_eq!(fsm.client_id, 1);
}

#[test]
fn zone_nav_fsm_walk_to_transition() {
    use textquest::nav::zone_transition::{TransitionKind, ZoneTransitionFsm};

    let mut fsm = ZoneTransitionFsm::new(2);
    fsm.start(TransitionKind::WalkTo {
        destination: Waypoint::new(100.0, 50.0, 0.0),
    });
    assert!(!fsm.is_idle(), "FSM should leave idle after start()");
}

#[test]
fn zone_nav_fsm_zone_to_transition() {
    use textquest::nav::zone_transition::{TransitionKind, ZoneTransitionFsm};

    let mut fsm = ZoneTransitionFsm::new(3);
    fsm.start(TransitionKind::ZoneTo {
        zone_name: "highkeep".to_string(),
        zone_line_pos: Waypoint::new(0.0, 0.0, 0.0),
    });
    assert!(!fsm.is_idle());
}

#[test]
fn zone_nav_fsm_port_to_transition() {
    use textquest::nav::zone_transition::{TransitionKind, ZoneTransitionFsm};

    let mut fsm = ZoneTransitionFsm::new(4);
    fsm.start(TransitionKind::PortTo {
        zone_name: "poknowledge".to_string(),
        caster_id: 99,
    });
    assert!(!fsm.is_idle());
}

#[test]
fn zone_nav_fsm_restarting_replaces_transition() {
    use textquest::nav::zone_transition::{TransitionKind, ZoneTransitionFsm};

    let mut fsm = ZoneTransitionFsm::new(5);
    fsm.start(TransitionKind::WalkTo {
        destination: Waypoint::new(10.0, 0.0, 0.0),
    });
    // Replace with a different transition.
    fsm.start(TransitionKind::ZoneTo {
        zone_name: "nro".to_string(),
        zone_line_pos: Waypoint::new(500.0, 0.0, 0.0),
    });
    assert!(!fsm.is_idle(), "restarted FSM should still be in-progress");
}

// ============================================================================
// Test 11: Integration Scenario Tests
// ============================================================================

mod scenarios;

#[test]
fn scenario_solo_farming_loop() {
    use scenarios::{ScenarioRunner, SoloFarmingScenario};

    let scenario = SoloFarmingScenario;
    let result = ScenarioRunner::run(&scenario);

    assert!(
        result.passed,
        "Solo farming scenario failed: {}",
        result.reason.unwrap_or_default()
    );
    assert!(result.total_ticks > 0, "Scenario should execute ticks");
}

#[test]
fn scenario_group_healing() {
    use scenarios::{GroupHealScenario, ScenarioRunner};

    let scenario = GroupHealScenario;
    let result = ScenarioRunner::run(&scenario);

    assert!(
        result.passed,
        "Group healing scenario failed: {}",
        result.reason.unwrap_or_default()
    );
    assert!(result.total_ticks > 0, "Scenario should execute ticks");
}

#[test]
fn scenario_zone_recovery() {
    use scenarios::{ScenarioRunner, ZoneRecoveryScenario};

    let scenario = ZoneRecoveryScenario;
    let result = ScenarioRunner::run(&scenario);

    assert!(
        result.passed,
        "Zone recovery scenario failed: {}",
        result.reason.unwrap_or_default()
    );
    assert!(result.total_ticks > 0, "Scenario should execute ticks");
}
