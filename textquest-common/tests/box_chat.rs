use textquest_common::box_chat::{BoxChatConfig, OutboundRoute, parse_slash_route};

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
