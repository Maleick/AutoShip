// Integration tests for TextQuest Web SDK client
// These tests verify type deserialization and client construction

use textquest_web_sdk::{Client, models::*};

#[test]
fn test_client_creation() {
    let _client = Client::new("http://localhost:3000");
}

#[test]
fn test_client_with_token() {
    let _client = Client::with_token("http://localhost:3000", Some("test-token".to_string()));
}

#[test]
fn test_health_response_serialization() {
    let json = r#"{
        "status": "ok",
        "version": "0.1.0"
    }"#;

    let health: HealthResponse = serde_json::from_str(json)
        .expect("Failed to deserialize HealthResponse");

    assert_eq!(health.status, "ok");
    assert_eq!(health.version, "0.1.0");
}

#[test]
fn test_error_response_serialization() {
    let json = r#"{"error": "API error message"}"#;

    let error: ErrorResponse = serde_json::from_str(json)
        .expect("Failed to deserialize ErrorResponse");

    assert_eq!(error.error, "API error message");
}

#[test]
fn test_session_info_serialization() {
    let json = r#"{
        "client_id": 1,
        "character_name": "Maleick",
        "zone": "Kaladim",
        "level": 65,
        "hp_pct": 100.0,
        "mana_pct": 95.5,
        "endurance_pct": 88.0,
        "status": "idle",
        "buff_count": 3,
        "target_name": null,
        "target_hp_pct": null,
        "pet_name": null
    }"#;

    let session: SessionInfo = serde_json::from_str(json)
        .expect("Failed to deserialize SessionInfo");

    assert_eq!(session.client_id, 1);
    assert_eq!(session.character_name, "Maleick");
    assert_eq!(session.level, 65);
    assert_eq!(session.status, "idle");
}

#[test]
fn test_character_config_serialization() {
    let json = r#"{
        "character_name": "TestChar",
        "class": "Wizard",
        "role": "DPS",
        "heal_at_pct": 50,
        "mana_sit_pct": 20,
        "nuke_at_pct": 80,
        "rotation": [],
        "class_params": {},
        "group_override": false,
        "group_name": null
    }"#;

    let config: CharacterConfig = serde_json::from_str(json)
        .expect("Failed to deserialize CharacterConfig");

    assert_eq!(config.character_name, "TestChar");
    assert_eq!(config.class, "Wizard");
    assert_eq!(config.role, "DPS");
    assert!(!config.group_override);
}

#[test]
fn test_economy_settings_serialization() {
    let json = r#"{
        "krono": {
            "target_rate_per_day": 5,
            "min_sell_price": 900,
            "max_buy_price": 850,
            "restock_threshold": 3,
            "enabled": true
        },
        "banking_rules": [],
        "tradeskill_supplies": []
    }"#;

    let settings: EconomySettings = serde_json::from_str(json)
        .expect("Failed to deserialize EconomySettings");

    assert!(settings.krono.enabled);
    assert_eq!(settings.krono.target_rate_per_day, 5);
    assert_eq!(settings.banking_rules.len(), 0);
}

#[test]
fn test_loot_action_enum() {
    let json_keep = r#""keep""#;
    let action: LootAction = serde_json::from_str(json_keep)
        .expect("Failed to deserialize LootAction");

    match action {
        LootAction::Keep => {}
        _ => panic!("Expected Keep variant"),
    }

    let json_vendor = r#""vendor""#;
    let action: LootAction = serde_json::from_str(json_vendor)
        .expect("Failed to deserialize LootAction");

    match action {
        LootAction::Vendor => {}
        _ => panic!("Expected Vendor variant"),
    }
}

#[test]
fn test_vendor_route_serialization() {
    let json = r#"{
        "id": "route-1",
        "zone": "East Commonlands",
        "npc_name": "Merchant Targin",
        "path_notes": "Near the stone",
        "item_categories": ["Tradeskill"],
        "enabled": true
    }"#;

    let route: VendorRoute = serde_json::from_str(json)
        .expect("Failed to deserialize VendorRoute");

    assert_eq!(route.id, "route-1");
    assert_eq!(route.zone, "East Commonlands");
    assert!(route.enabled);
}

#[test]
fn test_spawn_alert_serialization() {
    let json = r#"{
        "id": "alert-1",
        "pattern": "Venril Sathir",
        "location": "Sepulcher",
        "timestamp": 1713350400,
        "level": 65
    }"#;

    let alert: SpawnAlert = serde_json::from_str(json)
        .expect("Failed to deserialize SpawnAlert");

    assert_eq!(alert.pattern, "Venril Sathir");
    assert_eq!(alert.location, "Sepulcher");
    assert_eq!(alert.level, 65);
}

#[test]
fn test_loot_rules_serialization() {
    let json = r#"{
        "rules": [],
        "master_looter": "MainChar",
        "auto_loot_enabled": true
    }"#;

    let rules: LootRules = serde_json::from_str(json)
        .expect("Failed to deserialize LootRules");

    assert_eq!(rules.master_looter, "MainChar");
    assert!(rules.auto_loot_enabled);
    assert_eq!(rules.rules.len(), 0);
}

#[test]
fn test_soul_state_serialization() {
    let json = r#"{
        "character_id": "char-123",
        "memory_usage": 1024000,
        "processed_events": 1500,
        "last_action_time": 1713350400,
        "status": "active"
    }"#;

    let state: SoulState = serde_json::from_str(json)
        .expect("Failed to deserialize SoulState");

    assert_eq!(state.character_id, "char-123");
    assert_eq!(state.memory_usage, 1024000);
    assert_eq!(state.status, "active");
}

#[test]
fn test_chat_pattern_rule_serialization() {
    let json = r#"{
        "id": "rule-1",
        "pattern": "looking for group",
        "action": "respond",
        "enabled": true,
        "cooldown_seconds": 60
    }"#;

    let rule: ChatPatternRule = serde_json::from_str(json)
        .expect("Failed to deserialize ChatPatternRule");

    assert_eq!(rule.id, "rule-1");
    assert_eq!(rule.pattern, "looking for group");
    assert!(rule.enabled);
    assert_eq!(rule.cooldown_seconds, 60);
}

#[test]
fn test_xassist_config_serialization() {
    let json = r#"{
        "character": "Warrior1",
        "enabled": true,
        "assist_target": "MainTank",
        "auto_attack": true
    }"#;

    let config: XAssistConfig = serde_json::from_str(json)
        .expect("Failed to deserialize XAssistConfig");

    assert_eq!(config.character, "Warrior1");
    assert!(config.enabled);
    assert_eq!(config.assist_target, "MainTank");
    assert!(config.auto_attack);
}

#[test]
fn test_gm_alert_serialization() {
    let json = r#"{
        "zone": "Plane of Sky",
        "detected": false,
        "last_seen_timestamp": 1713350400
    }"#;

    let alert: GmAlert = serde_json::from_str(json)
        .expect("Failed to deserialize GmAlert");

    assert_eq!(alert.zone, "Plane of Sky");
    assert!(!alert.detected);
}

#[test]
fn test_kill_tracker_entry_serialization() {
    let json = r#"{
        "timestamp": 1713350400,
        "character": "Warrior1",
        "target": "Giant Spider",
        "zone": "Undead Cavern",
        "level": 25
    }"#;

    let entry: KillTrackerEntry = serde_json::from_str(json)
        .expect("Failed to deserialize KillTrackerEntry");

    assert_eq!(entry.character, "Warrior1");
    assert_eq!(entry.target, "Giant Spider");
    assert_eq!(entry.level, 25);
}

#[test]
fn test_alert_entry_serialization() {
    let json = r#"{
        "id": "alert-001",
        "alert_type": "spawn",
        "message": "Raid target spawned",
        "timestamp": 1713350400,
        "acknowledged": false
    }"#;

    let entry: AlertEntry = serde_json::from_str(json)
        .expect("Failed to deserialize AlertEntry");

    assert_eq!(entry.id, "alert-001");
    assert_eq!(entry.alert_type, "spawn");
    assert!(!entry.acknowledged);
}

#[test]
fn test_timestamp_config_serialization() {
    let json = r#"{
        "enabled": true,
        "format": "time24"
    }"#;

    let config: TimestampConfig = serde_json::from_str(json)
        .expect("Failed to deserialize TimestampConfig");

    assert!(config.enabled);
    match config.format {
        TimestampFormat::Time24 => {}
        _ => panic!("Expected Time24 format variant"),
    }
}

#[test]
fn test_player_watch_config_serialization() {
    let json = r#"{
        "filter_mode": "friends_only",
        "sound_on_zone_in": true,
        "friends": ["Maleick", "Frostreaver"]
    }"#;

    let config: PlayerWatchConfig = serde_json::from_str(json)
        .expect("Failed to deserialize PlayerWatchConfig");

    assert!(config.sound_on_zone_in);
    assert_eq!(config.friends.len(), 2);
    match config.filter_mode {
        PlayerFilterMode::FriendsOnly => {}
        _ => panic!("Expected FriendsOnly filter mode"),
    }
}

#[test]
fn test_wealth_history_serialization() {
    let json = r#"{
        "current": {
            "timestamp": "2024-04-17T12:00:00Z",
            "plat": 50000,
            "krono": 3,
            "item_value_estimate": 120000
        },
        "snapshots": []
    }"#;

    let history: WealthHistory = serde_json::from_str(json)
        .expect("Failed to deserialize WealthHistory");

    assert_eq!(history.current.plat, 50000);
    assert_eq!(history.current.krono, 3);
    assert_eq!(history.snapshots.len(), 0);
}

#[test]
fn test_auto_accept_settings_serialization() {
    let json = r#"{
        "enabled": true,
        "accept_group_invites": true,
        "accept_trades": false,
        "accept_task_adds": true,
        "accept_dz_adds": true,
        "accept_translocates": false,
        "accept_anchors": true,
        "trust_mode": "anyone",
        "trusted_players": []
    }"#;

    let settings: AutoAcceptSettings = serde_json::from_str(json)
        .expect("Failed to deserialize AutoAcceptSettings");

    assert!(settings.enabled);
    assert!(settings.accept_group_invites);
    assert!(!settings.accept_trades);
    assert!(settings.trusted_players.is_empty());
}

