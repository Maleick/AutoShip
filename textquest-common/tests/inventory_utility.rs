use textquest_common::inventory_utility::{
    CollectionRoute, CollectionRoutingRule, ConsumableKind, ConsumablePreferences, ConsumableStack,
    CursorAction, CursorRule, RewardClaimDecision, RewardRoutingRule, VendorListing,
    VendorWatchRule, decide_cursor_action, default_inventory_utility_config,
    evaluate_relocation_rules, resolve_reward_claim, route_collection_item, select_consumable,
    vendor_watch_alerts,
};
use textquest_common::nav::{RelocationOptionState, relocation_catalog};

#[test]
fn default_config_covers_every_plugin_in_scope() {
    let config = default_inventory_utility_config();

    let plugins = config
        .plugin_mappings
        .iter()
        .map(|entry| entry.plugin.as_str())
        .collect::<Vec<_>>();

    assert_eq!(plugins.len(), 13);
    assert!(plugins.contains(&"MQ2LinkDB"));
    assert!(plugins.contains(&"MQ2ItemScore"));
    assert!(plugins.contains(&"MQ2Cursor"));
    assert!(plugins.contains(&"MQ2Collections"));
    assert!(plugins.contains(&"MQ2Collectible"));
    assert!(plugins.contains(&"MQ2TributeManager"));
    assert!(plugins.contains(&"MQ2TSTrophy"));
    assert!(plugins.contains(&"MQ2Rewards"));
    assert!(plugins.contains(&"MQ2FeedMe"));
    assert!(plugins.contains(&"MQ2PortalSetter"));
    assert!(plugins.contains(&"MQ2Relocate"));
    assert!(plugins.contains(&"MQ2Vendors"));
    assert!(plugins.contains(&"MQ2AutoClaim"));

    let item_score = config
        .plugin_mappings
        .iter()
        .find(|entry| entry.plugin == "MQ2ItemScore")
        .expect("item-score mapping");
    assert_eq!(item_score.owner, "loot::item_score");
    assert_eq!(item_score.status.as_str(), "native");

    let portal_setter = config
        .plugin_mappings
        .iter()
        .find(|entry| entry.plugin == "MQ2PortalSetter")
        .expect("portal setter mapping");
    assert_eq!(portal_setter.status.as_str(), "adapted");
}

#[test]
fn cursor_rules_apply_keep_limit_before_overflow_action() {
    let rules = vec![CursorRule {
        item_matcher: "Fine Steel Breastplate".into(),
        action: CursorAction::Keep,
        keep_at_or_below: Some(1),
        overflow_action: Some(CursorAction::Destroy),
    }];

    assert_eq!(
        decide_cursor_action("Fine Steel Breastplate", 0, &rules),
        CursorAction::Keep
    );
    assert_eq!(
        decide_cursor_action("Fine Steel Breastplate", 2, &rules),
        CursorAction::Destroy
    );
}

#[test]
fn collection_routing_uses_duplicate_route_for_completed_sets() {
    let rules = vec![CollectionRoutingRule {
        set_matcher: "Frostcrypt".into(),
        incomplete_route: CollectionRoute::Keep,
        completed_route: CollectionRoute::Bank,
        duplicate_route: CollectionRoute::Tribute,
    }];

    assert_eq!(
        route_collection_item("Frostcrypt", false, false, &rules),
        CollectionRoute::Keep
    );
    assert_eq!(
        route_collection_item("Frostcrypt", true, true, &rules),
        CollectionRoute::Tribute
    );
}

#[test]
fn reward_routing_reuses_position_selection_and_claim_toggle() {
    let rules = vec![RewardRoutingRule::by_position("Hero's Mission", 2, true)];
    let rewards = vec![
        "Minor Potion".to_string(),
        "Heroic Augment".to_string(),
        "Ancient Coin".to_string(),
    ];

    assert_eq!(
        resolve_reward_claim("Hero's Mission", &rewards, &rules),
        Some(RewardClaimDecision {
            reward_index: 1,
            auto_claim: true,
        })
    );
}

#[test]
fn consumption_prefers_named_food_and_skips_ignored_items() {
    let inventory = vec![
        ConsumableStack::new("Fish Roll", ConsumableKind::Food),
        ConsumableStack::new("Water Flask", ConsumableKind::Drink),
        ConsumableStack::new("Summoned: Modulation Shard", ConsumableKind::Food),
    ];
    let preferences = ConsumablePreferences {
        enabled: true,
        preferred_food: vec!["Fish Roll".into()],
        preferred_drink: vec!["Water Flask".into()],
        ignored_items: vec!["Summoned: Modulation Shard".into()],
    };

    assert_eq!(
        select_consumable(ConsumableKind::Food, &inventory, &preferences),
        Some("Fish Roll".to_string())
    );
}

#[test]
fn vendor_watch_alerts_respect_price_caps() {
    let listings = vec![
        VendorListing::new("Peridot", 90),
        VendorListing::new("Fine Steel Spear", 250),
    ];
    let rules = vec![
        VendorWatchRule::notify_on("Peridot", Some(100)),
        VendorWatchRule::notify_on("Fine Steel Spear", Some(200)),
    ];

    let alerts = vendor_watch_alerts(&listings, &rules);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].item_name, "Peridot");
}

#[test]
fn relocation_rules_report_when_destination_has_no_ready_option() {
    let option = relocation_catalog()
        .into_iter()
        .find(|entry| entry.id == "throne_of_heroes")
        .expect("throne option");
    let loadout = vec![RelocationOptionState::new(option, true, Some(900))];

    let statuses = evaluate_relocation_rules(
        &default_inventory_utility_config().relocation_rules,
        &loadout,
    );

    let guild_lobby = statuses
        .iter()
        .find(|status| status.destination == "guildlobby")
        .expect("guild lobby status");
    assert!(!guild_lobby.ready);
    assert!(guild_lobby.warning.is_some());
}
