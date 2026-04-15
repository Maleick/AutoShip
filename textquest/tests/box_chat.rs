use textquest::config::AppConfig;
use textquest_common::box_chat::{BoxChatConfig, OutboundRoute, parse_slash_route};

#[test]
fn app_config_defaults_box_chat_to_disabled() {
    let cfg = AppConfig::default_config();

    assert_eq!(cfg.box_chat, BoxChatConfig::default());
    assert!(!cfg.box_chat.enabled);
    assert_eq!(cfg.box_chat.host, "127.0.0.1");
    assert_eq!(cfg.box_chat.port, 2112);
    assert!(!cfg.box_chat.auto_connect);
}

#[test]
fn app_config_parses_box_chat_section() {
    let cfg: AppConfig = toml::from_str(
        r#"
[box_chat]
enabled = true
host = "192.168.1.25"
port = 3002
auto_connect = true
"#,
    )
    .expect("box chat config should parse");

    assert!(cfg.box_chat.enabled);
    assert_eq!(cfg.box_chat.host, "192.168.1.25");
    assert_eq!(cfg.box_chat.port, 3002);
    assert!(cfg.box_chat.auto_connect);
}

#[test]
fn parse_bc_route_accepts_slash_or_mq2_double_slash_payloads() {
    assert_eq!(
        parse_slash_route("/bc /assist MainTank"),
        Some(Ok(OutboundRoute::Broadcast {
            command: "/assist MainTank".to_string(),
        }))
    );

    assert_eq!(
        parse_slash_route("/bca //follow MainTank"),
        Some(Ok(OutboundRoute::Broadcast {
            command: "/follow MainTank".to_string(),
        }))
    );
}

#[test]
fn parse_bct_route_targets_specific_character() {
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
