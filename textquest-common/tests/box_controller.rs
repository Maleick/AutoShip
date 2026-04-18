use textquest_common::box_controller::{
    BoxControllerClientSnapshot, BoxControllerClientState, BoxControllerCommand, BoxControllerMode,
    BoxControllerSnapshot,
};

#[test]
fn pause_and_mode_commands_update_client_state() {
    let mut state = BoxControllerClientState::new("Frostreaver".to_string());

    state.apply_command(&BoxControllerCommand::Pause);
    assert_eq!(state.mode, BoxControllerMode::Paused);
    assert_eq!(state.last_command, BoxControllerCommand::Pause);

    state.apply_command(&BoxControllerCommand::Unpause);
    assert_eq!(state.mode, BoxControllerMode::Automatic);
    assert_eq!(state.last_command, BoxControllerCommand::Unpause);

    state.apply_command(&BoxControllerCommand::Camp);
    assert_eq!(state.mode, BoxControllerMode::Camp);

    state.apply_command(&BoxControllerCommand::Chase);
    assert_eq!(state.mode, BoxControllerMode::Chase);

    state.apply_command(&BoxControllerCommand::Manual);
    assert_eq!(state.mode, BoxControllerMode::Manual);
}

#[test]
fn burn_now_tracks_requests_without_overwriting_mode() {
    let mut state = BoxControllerClientState::new("Aelrindel".to_string());
    state.apply_command(&BoxControllerCommand::Chase);

    state.apply_command(&BoxControllerCommand::BurnNow);
    state.apply_command(&BoxControllerCommand::BurnNow);

    assert_eq!(state.mode, BoxControllerMode::Chase);
    assert_eq!(state.burn_requests, 2);
    assert_eq!(state.last_command, BoxControllerCommand::BurnNow);
}

#[test]
fn raid_assist_num_updates_assist_slot() {
    let mut state = BoxControllerClientState::new("Valerius".to_string());

    state.apply_command(&BoxControllerCommand::RaidAssistNum { assist_num: 3 });

    assert_eq!(state.raid_assist_num, Some(3));
    assert_eq!(
        state.last_command,
        BoxControllerCommand::RaidAssistNum { assist_num: 3 }
    );
}

#[test]
fn command_roundtrip_preserves_raid_assist_num_payload() {
    let command = BoxControllerCommand::RaidAssistNum { assist_num: 2 };

    let json = serde_json::to_string(&command).expect("serialize command");
    let decoded: BoxControllerCommand = serde_json::from_str(&json).expect("deserialize command");

    assert_eq!(decoded, command);
}

#[test]
fn client_state_roundtrip_preserves_mode_and_burn_metadata() {
    let mut state = BoxControllerClientState::new("Mystik".to_string());
    state.apply_command(&BoxControllerCommand::Manual);
    state.apply_command(&BoxControllerCommand::BurnNow);
    state.apply_command(&BoxControllerCommand::RaidAssistNum { assist_num: 1 });

    let json = serde_json::to_string(&state).expect("serialize state");
    let decoded: BoxControllerClientState = serde_json::from_str(&json).expect("deserialize state");

    assert_eq!(decoded.character_name, "Mystik");
    assert_eq!(decoded.mode, BoxControllerMode::Manual);
    assert_eq!(decoded.burn_requests, 1);
    assert_eq!(decoded.raid_assist_num, Some(1));
}

#[test]
fn optional_controller_fields_serialize_as_null() {
    let state = BoxControllerClientState::new("Mystik".to_string());
    let state_json = serde_json::to_value(&state).expect("serialize client state");
    assert!(state_json.get("raidAssistNum").is_some());
    assert!(state_json["raidAssistNum"].is_null());

    let snapshot = BoxControllerSnapshot {
        connected_clients: 1,
        relay_enabled: true,
        last_command: None,
        clients: vec![BoxControllerClientSnapshot {
            character_name: "Mystik".to_string(),
            node_name: "operator-1".to_string(),
            mode: BoxControllerMode::Automatic,
            burn_requests: 0,
            raid_assist_num: None,
        }],
    };
    let snapshot_json = serde_json::to_value(&snapshot).expect("serialize snapshot");

    assert!(snapshot_json.get("lastCommand").is_some());
    assert!(snapshot_json["lastCommand"].is_null());
    assert!(snapshot_json["clients"][0].get("raidAssistNum").is_some());
    assert!(snapshot_json["clients"][0]["raidAssistNum"].is_null());
}
