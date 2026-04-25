//! Integration tests for GroupReadinessGate functionality.

#![cfg(windows)]

use textquest::camp::{
    config::{CampConfig, GroupReadinessConfig, RoleReadinessThresholds},
    readiness::{check_group_ready, check_member_ready, ReadinessBlocker},
    state::{CampAction, CampLoop, CampMember, CampSnapshot, CampState, Role},
};
use textquest_common::types::SharedStateFrame;

fn make_frame(
    hp: i32,
    hp_max: i32,
    mana: i32,
    mana_max: i32,
    x: f32,
    y: f32,
) -> SharedStateFrame {
    SharedStateFrame {
        client_id: 1,
        spawn_id: 1,
        name: "Test".to_string(),
        x,
        y,
        z: 0.0,
        x_camp: 0.0,
        y_camp: 0.0,
        hp,
        hp_max,
        mana,
        mana_max,
        level: 50,
        class: "cleric".to_string(),
        buffs: vec![],
        spawn_epoch: 0,
        nearby_spawns: vec![],
    }
}

fn base_config() -> GroupReadinessConfig {
    GroupReadinessConfig {
        healer: RoleReadinessThresholds {
            hp_pct: 95,
            mana_pct: 95,
        },
        tank: RoleReadinessThresholds {
            hp_pct: 100,
            mana_pct: 0,
        },
        dps: RoleReadinessThresholds {
            hp_pct: 80,
            mana_pct: 50,
        },
        position_radius: 30.0,
        timeout_ticks: 300,
        required_buffs: vec![],
    }
}

#[test]
fn test_fixture_all_members_ready() {
    let config = base_config();
    let healer = make_frame(950, 1000, 950, 1000, 0.0, 0.0);
    let tank = make_frame(1000, 1000, 500, 500, 0.0, 0.0);
    let dps1 = make_frame(800, 1000, 500, 1000, 0.0, 0.0);
    let dps2 = make_frame(800, 1000, 600, 1000, 0.0, 0.0);

    let members = vec![
        ("Cleric01".to_string(), Role::Healer, &healer),
        ("Tank01".to_string(), Role::Tank, &tank),
        ("Ranger01".to_string(), Role::Dps, &dps1),
        ("Ranger02".to_string(), Role::Dps, &dps2),
    ];

    let result = check_group_ready(&members, &config);
    assert_eq!(
        result, None,
        "All members at required thresholds should return None"
    );
}

#[test]
fn test_fixture_missing_buff() {
    let mut config = base_config();
    config.required_buffs = vec!["Virtue".to_string(), "Haste".to_string()];

    let healer = make_frame(950, 1000, 950, 1000, 0.0, 0.0);

    let members = vec![("Cleric01".to_string(), Role::Healer, &healer)];

    let result = check_group_ready(&members, &config);
    assert!(
        matches!(result, Some(ReadinessBlocker::MissingBuff { .. })),
        "Missing required buff should block pull: {:?}",
        result
    );
}

#[test]
fn test_fixture_missing_buff_message() {
    let mut config = base_config();
    config.required_buffs = vec!["Virtue".to_string()];

    let healer = make_frame(950, 1000, 950, 1000, 0.0, 0.0);
    let members = vec![("Cleric01".to_string(), Role::Healer, &healer)];

    if let Some(blocker) = check_group_ready(&members, &config) {
        let msg = blocker.message();
        assert!(
            msg.contains("Virtue"),
            "Message should mention the missing buff: {}",
            msg
        );
    } else {
        panic!("Expected a ReadinessBlocker");
    }
}

#[test]
fn test_fsm_idle_transitions_to_group_watch_wait() {
    let config = CampConfig {
        name: "test_camp".into(),
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
        ignore_mob_names: Vec::new(),
        burn_mob_names: Vec::new(),
        return_no_aggro: false,
        next_camp: None,
        prev_camp: None,
        group_readiness: base_config(),
    };

    let members = vec![
        CampMember::new(100, "Tank01".into(), Role::Tank),
        CampMember::new(101, "Healer01".into(), Role::Healer),
        CampMember::new(102, "Puller01".into(), Role::Puller),
    ];

    let mut camp = CampLoop::new(config, members);
    assert_eq!(camp.state, CampState::Idle);

    let cmds = camp.tick(None); // Idle -> GroupWatchWait

    assert!(
        matches!(camp.state, CampState::GroupWatchWait { .. }),
        "Should transition to GroupWatchWait: {:?}",
        camp.state
    );

    let sit_cmds: Vec<_> = cmds
        .iter()
        .filter(|(_, action)| {
            if let CampAction::Slash(s) = action {
                s.contains("/sit")
            } else {
                false
            }
        })
        .collect();

    assert_eq!(
        sit_cmds.len(),
        3,
        "All 3 members should sit during group watch: {:?}",
        cmds
    );
}

#[test]
fn test_fsm_group_watch_wait_times_out() {
    let config = CampConfig {
        name: "test_camp".into(),
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
        ignore_mob_names: Vec::new(),
        burn_mob_names: Vec::new(),
        return_no_aggro: false,
        next_camp: None,
        prev_camp: None,
        group_readiness: GroupReadinessConfig {
            timeout_ticks: 10,
            ..base_config()
        },
    };

    let members = vec![
        CampMember::new(100, "Tank01".into(), Role::Tank),
        CampMember::new(101, "Healer01".into(), Role::Healer),
        CampMember::new(102, "Puller01".into(), Role::Puller),
    ];

    let mut camp = CampLoop::new(config, members);
    camp.tick(None); // Idle -> GroupWatchWait

    assert!(matches!(camp.state, CampState::GroupWatchWait { .. }));

    // Tick through the timeout period without snapshot (group never becomes ready)
    for _ in 0..11 {
        camp.tick(None);
    }

    // After timeout, should transition to Pulling
    assert!(
        matches!(camp.state, CampState::Pulling { .. }),
        "Should timeout and transition to Pulling: {:?}",
        camp.state
    );
}

#[test]
fn test_fsm_group_watch_wait_exits_when_ready() {
    let config = CampConfig {
        name: "test_camp".into(),
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
        ignore_mob_names: Vec::new(),
        burn_mob_names: Vec::new(),
        return_no_aggro: false,
        next_camp: None,
        prev_camp: None,
        group_readiness: base_config(),
    };

    let members = vec![
        CampMember::new(100, "Tank01".into(), Role::Tank),
        CampMember::new(101, "Healer01".into(), Role::Healer),
        CampMember::new(102, "Puller01".into(), Role::Puller),
    ];

    let mut camp = CampLoop::new(config, members);
    camp.tick(None); // Idle -> GroupWatchWait

    // Create snapshot with healer at full mana (ready)
    let snapshot = CampSnapshot {
        healer_mana_pct: 95.0,
        tank_hp_pct: 100.0,
        target_hp_pct: None,
        target_is_dead: false,
        target_spawn_id: None,
        member_hp: vec![],
        member_in_combat: vec![],
    };

    // Tick with snapshot showing group is ready
    camp.tick(Some(&snapshot));

    // Should transition to Pulling
    assert!(
        matches!(camp.state, CampState::Pulling { .. }),
        "Should transition to Pulling when group ready: {:?}",
        camp.state
    );
}
