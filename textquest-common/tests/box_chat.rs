use textquest_common::box_chat::{BoxChatConfig, OutboundRoute, WireMessage, parse_slash_route};

#[test]
fn defaults_disable_box_chat_and_use_expected_endpoint() {
    let cfg = BoxChatConfig::default();

    assert!(!cfg.enabled);
    assert_eq!(cfg.host, "127.0.0.1");
    assert_eq!(cfg.port, 2112);
    assert!(!cfg.auto_connect);
}

#[test]
fn parse_bc_and_bct_routes() {
    assert_eq!(
        parse_slash_route("/bc /assist MainTank"),
        Some(Ok(OutboundRoute::Broadcast {
            command: "/assist MainTank".to_string(),
        }))
    );
    assert_eq!(
        parse_slash_route("/bct Cleric01 //cast 1"),
        Some(Ok(OutboundRoute::Target {
            character: "Cleric01".to_string(),
            command: "/cast 1".to_string(),
        }))
    );
}

#[test]
fn parse_slash_route_rejects_missing_or_non_command_payloads() {
    let missing = parse_slash_route("/bc").expect("box chat command should be recognized");
    assert!(missing.is_err());

    let text = parse_slash_route("/bct Cleric01 heal me now")
        .expect("box chat command should be recognized");
    assert!(text.is_err());

    assert_eq!(parse_slash_route("/assist MainTank"), None);
}

// ── /bca and /bcaa ───────────────────────────────────────────────────────────

#[test]
fn parse_bca_broadcasts() {
    let result = parse_slash_route("/bca /gate");
    assert_eq!(
        result,
        Some(Ok(OutboundRoute::Broadcast {
            command: "/gate".to_string(),
        }))
    );
}

#[test]
fn parse_bcaa_broadcasts() {
    let result = parse_slash_route("/bcaa //sit");
    assert_eq!(
        result,
        Some(Ok(OutboundRoute::Broadcast {
            command: "/sit".to_string(),
        }))
    );
}

// ── // prefix normalisation ──────────────────────────────────────────────────

#[test]
fn double_slash_payload_is_normalised_to_single_slash() {
    let result = parse_slash_route("/bc //cast 3");
    assert_eq!(
        result,
        Some(Ok(OutboundRoute::Broadcast {
            command: "/cast 3".to_string(),
        }))
    );
}

#[test]
fn bct_double_slash_payload() {
    let result = parse_slash_route("/bct Warrior //assist");
    assert_eq!(
        result,
        Some(Ok(OutboundRoute::Target {
            character: "Warrior".to_string(),
            command: "/assist".to_string(),
        }))
    );
}

// ── Error cases ──────────────────────────────────────────────────────────────

#[test]
fn parse_bct_missing_character_is_error() {
    let result = parse_slash_route("/bct //cast 1")
        .expect("bct should be recognized");
    // "/bct //cast 1" → character = "//cast", payload = "1" which doesn't start with /
    // actually this splits on whitespace: character="//cast", rest="1"
    // normalize_payload("1") → Err (no leading / or //)
    assert!(result.is_err(), "expected error but got: {:?}", result);
}

#[test]
fn parse_bc_empty_payload_is_error() {
    let result = parse_slash_route("/bc   ")
        .expect("bc should be recognized");
    assert!(result.is_err());
}

#[test]
fn parse_double_slash_only_is_error() {
    // "/bc //" → normalize_payload("//") → stripped = "", which is empty → Err
    let result = parse_slash_route("/bc //")
        .expect("bc should be recognized");
    assert!(result.is_err());
}

#[test]
fn non_slash_input_returns_none() {
    assert_eq!(parse_slash_route("bc /assist"), None);
    assert_eq!(parse_slash_route(""), None);
    assert_eq!(parse_slash_route("   "), None);
}

#[test]
fn unrelated_slash_command_returns_none() {
    assert_eq!(parse_slash_route("/say hello"), None);
    assert_eq!(parse_slash_route("/camp"), None);
}

// ── WireMessage serialisation ────────────────────────────────────────────────

#[test]
fn wire_message_hello_roundtrip() {
    let msg = WireMessage::Hello {
        node_name: "raid-host".to_string(),
        characters: vec!["Tankor".to_string(), "Healer".to_string()],
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let decoded: WireMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, decoded);
}

#[test]
fn wire_message_update_characters_roundtrip() {
    let msg = WireMessage::UpdateCharacters {
        characters: vec!["Bard01".to_string()],
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let decoded: WireMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, decoded);
}

#[test]
fn wire_message_broadcast_roundtrip() {
    let msg = WireMessage::Broadcast {
        command: "/assist MainTank".to_string(),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let decoded: WireMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, decoded);
}

#[test]
fn wire_message_target_roundtrip() {
    let msg = WireMessage::Target {
        character: "Cleric01".to_string(),
        command: "/cast 1".to_string(),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let decoded: WireMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, decoded);
}

#[test]
fn wire_message_execute_broadcast_roundtrip() {
    let msg = WireMessage::ExecuteBroadcast {
        command: "/gate".to_string(),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let decoded: WireMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, decoded);
}

#[test]
fn wire_message_execute_target_roundtrip() {
    let msg = WireMessage::ExecuteTarget {
        character: "Warrior".to_string(),
        command: "/follow".to_string(),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    let decoded: WireMessage = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(msg, decoded);
}

#[test]
fn wire_message_type_tag_is_snake_case() {
    let msg = WireMessage::ExecuteBroadcast {
        command: "/gate".to_string(),
    };
    let json = serde_json::to_string(&msg).expect("serialize");
    assert!(json.contains("\"type\":\"execute_broadcast\""));
}

// ── BoxChatConfig serialisation ──────────────────────────────────────────────

#[test]
fn box_chat_config_roundtrip() {
    let cfg = BoxChatConfig {
        enabled: true,
        host: "192.168.1.50".to_string(),
        port: 9000,
        auto_connect: true,
    };
    let json = serde_json::to_string(&cfg).expect("serialize");
    let decoded: BoxChatConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(cfg, decoded);
}

#[test]
fn box_chat_config_default_roundtrip() {
    let cfg = BoxChatConfig::default();
    let json = serde_json::to_string(&cfg).expect("serialize");
    let decoded: BoxChatConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(cfg, decoded);
}
