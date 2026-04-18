use textquest_common::inventory_utility::{
    InventoryUtilityConfig, RewardRoutingRule, RewardClaimDecision, resolve_reward_claim,
};

#[test]
fn default_config_explicitly_maps_every_redguides_plugin_in_scope() {
    let config = InventoryUtilityConfig::default();
    let plugins = config
        .plugin_mappings
        .iter()
        .map(|mapping| mapping.plugin.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    for expected in [
        "MQ2LinkDB",
        "MQ2ItemScore",
        "MQ2Cursor",
        "MQ2Collections",
        "MQ2Collectible",
        "MQ2TributeManager",
        "MQ2TSTrophy",
        "MQ2Rewards",
        "MQ2FeedMe",
        "MQ2PortalSetter",
        "MQ2Relocate",
        "MQ2Vendors",
        "MQ2AutoClaim",
    ] {
        assert!(plugins.contains(expected), "missing mapping for {expected}");
    }

    assert!(config
        .plugin_mappings
        .iter()
        .all(|mapping| !mapping.owner.trim().is_empty()));
}

#[test]
fn reward_routes_prefer_exact_task_then_wildcard() {
    let rewards = vec![
        "Ancient Coin".to_string(),
        "Heroic Augment".to_string(),
    ];

    let rules = vec![
        RewardRoutingRule::by_name("*", "Ancient Coin", false),
        RewardRoutingRule::by_name("Artifact Recovery", "Heroic Augment", true),
    ];

    // Exact task match selects Heroic Augment at index 1 with auto_claim=true
    let decision = resolve_reward_claim("Artifact Recovery", &rewards, &rules);
    assert_eq!(
        decision,
        Some(RewardClaimDecision { reward_index: 1, auto_claim: true }),
    );

    // Wildcard match selects Ancient Coin at index 0 with auto_claim=false
    let decision = resolve_reward_claim("Different Task", &rewards, &rules);
    assert_eq!(
        decision,
        Some(RewardClaimDecision { reward_index: 0, auto_claim: false }),
    );
}
