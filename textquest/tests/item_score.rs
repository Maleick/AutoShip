use std::collections::BTreeMap;

use textquest::{
    camp::loot::{ItemAction, LootRules, classify_item_with_score},
    loot::{
        ImportItem, ItemScoreComparison, ItemScoreConfig, LootStore, ScoreableItem,
        WeightedStatDelta, compare_item_upgrade,
    },
};

fn warrior_config() -> ItemScoreConfig {
    let mut class_weights = BTreeMap::new();
    class_weights.insert(
        "Warrior".to_string(),
        BTreeMap::from([
            ("STR".to_string(), 1.0),
            ("AC".to_string(), 0.5),
            ("HP".to_string(), 0.1),
            ("DAMAGE".to_string(), 2.0),
            ("DELAY".to_string(), -0.2),
        ]),
    );

    ItemScoreConfig {
        min_upgrade_delta: 0.5,
        class_weights,
    }
}

fn scoreable_item(
    name: &str,
    slot: &str,
    classes: &[&str],
    stats: &[(&str, f64)],
) -> ScoreableItem {
    ScoreableItem {
        item_id: None,
        name: name.to_string(),
        slot: Some(slot.to_string()),
        classes: classes.iter().map(|class| (*class).to_string()).collect(),
        stats: stats
            .iter()
            .map(|(stat, value)| (stat.to_string(), *value))
            .collect(),
    }
}

fn sample_item(name: &str, slot: &str, stats_json: &str, classes: &[&str]) -> ImportItem {
    ImportItem {
        name: name.to_string(),
        lucy_id: None,
        slot: Some(slot.to_string()),
        item_type: Some("weapon".to_string()),
        ac: 10,
        hp: 20,
        mana: 0,
        damage: 10,
        delay: 25,
        level_req: 1,
        weight: 2,
        magic: true,
        lore: false,
        nodrop: false,
        expansion: Some("Velious".to_string()),
        effect: None,
        stats_json: Some(stats_json.to_string()),
        source_url: None,
        classes: classes.iter().map(|class| (*class).to_string()).collect(),
    }
}

#[test]
fn compare_item_upgrade_prefers_weighted_candidate_stats() {
    let config = warrior_config();
    let equipped = scoreable_item(
        "Rusty Sword",
        "Primary",
        &["Warrior"],
        &[
            ("STR", 4.0),
            ("AC", 8.0),
            ("HP", 10.0),
            ("DAMAGE", 7.0),
            ("DELAY", 28.0),
        ],
    );
    let candidate = scoreable_item(
        "Blade of Trials",
        "Primary",
        &["WAR"],
        &[
            ("STR", 8.0),
            ("AC", 12.0),
            ("HP", 20.0),
            ("DAMAGE", 10.0),
            ("DELAY", 24.0),
        ],
    );

    let comparison = compare_item_upgrade("warrior", &config, &candidate, Some(&equipped));

    assert!(comparison.is_upgrade);
    assert!(comparison.score_delta > config.min_upgrade_delta);
    assert_eq!(comparison.slot.as_deref(), Some("Primary"));
    assert_eq!(comparison.breakdown[0].stat, "DAMAGE");
}

#[test]
fn compare_item_upgrade_marks_unusable_items_as_non_upgrades() {
    let config = warrior_config();
    let candidate = scoreable_item("Silk Wand", "Primary", &["Wizard"], &[("INT", 10.0)]);

    let comparison = compare_item_upgrade("Warrior", &config, &candidate, None);

    assert!(!comparison.candidate_usable);
    assert!(!comparison.is_upgrade);
    assert!(comparison.score_delta <= 0.0);
}

#[test]
fn compare_item_upgrade_requires_class_weights_to_mark_upgrades() {
    let config = ItemScoreConfig {
        min_upgrade_delta: 0.0,
        class_weights: BTreeMap::new(),
    };
    let candidate = scoreable_item(
        "Training Sword",
        "Primary",
        &["Warrior"],
        &[("DAMAGE", 10.0), ("STR", 5.0)],
    );

    let comparison = compare_item_upgrade("Warrior", &config, &candidate, None);

    assert_eq!(comparison.candidate_score, 0.0);
    assert_eq!(comparison.equipped_score, 0.0);
    assert!(!comparison.is_upgrade);
}

#[test]
fn compare_item_upgrade_accepts_shadow_knight_shd_alias() {
    let mut class_weights = BTreeMap::new();
    class_weights.insert(
        "Shadow Knight".to_string(),
        BTreeMap::from([("STR".to_string(), 1.0), ("AC".to_string(), 0.5)]),
    );
    let config = ItemScoreConfig {
        min_upgrade_delta: 0.0,
        class_weights,
    };
    let candidate = scoreable_item("Runed Blade", "Primary", &["SHD"], &[("STR", 8.0)]);

    let comparison = compare_item_upgrade("shd", &config, &candidate, None);

    assert!(comparison.candidate_usable);
    assert!(comparison.is_upgrade);
    assert_eq!(comparison.class_name, "Shadow Knight");
}

#[test]
fn compare_item_upgrade_uses_raw_class_keys_for_weight_lookup() {
    let mut class_weights = BTreeMap::new();
    class_weights.insert(
        "WAR".to_string(),
        BTreeMap::from([("STR".to_string(), 1.0), ("DAMAGE".to_string(), 2.0)]),
    );
    let config = ItemScoreConfig {
        min_upgrade_delta: 0.0,
        class_weights,
    };
    let candidate = scoreable_item(
        "Jagged Velium Blade",
        "Primary",
        &["Warrior"],
        &[("STR", 8.0), ("DAMAGE", 10.0)],
    );

    let comparison = compare_item_upgrade("WAR", &config, &candidate, None);

    assert!(comparison.is_upgrade);
    assert!(comparison.candidate_score > 0.0);
    assert_eq!(comparison.class_name, "Warrior");
}

#[test]
fn loot_store_compare_item_upgrade_reads_stats_json_and_class_data() {
    let store = LootStore::open_memory().unwrap();
    let equipped_id = store
        .upsert_item(&sample_item(
            "Bronze Longsword",
            "Primary",
            r#"{"STR":4,"HP":5}"#,
            &["Warrior"],
        ))
        .unwrap();
    let candidate_id = store
        .upsert_item(&sample_item(
            "Jagged Velium Blade",
            "Primary",
            r#"{"STR":12,"HP":18,"DAMAGE":13,"DELAY":22}"#,
            &["Warrior", "Paladin"],
        ))
        .unwrap();

    let comparison = store
        .compare_item_upgrade("WAR", &warrior_config(), candidate_id, Some(equipped_id))
        .unwrap()
        .unwrap();

    assert_eq!(comparison.candidate_item_name, "Jagged Velium Blade");
    assert!(comparison.is_upgrade);
    assert!(
        comparison
            .breakdown
            .iter()
            .any(|entry| entry.stat == "STR" && entry.delta > 0.0)
    );
}

#[test]
fn loot_store_compare_item_upgrade_treats_missing_equipped_item_as_empty_slot() {
    let store = LootStore::open_memory().unwrap();
    let candidate_id = store
        .upsert_item(&sample_item(
            "Jagged Velium Blade",
            "Primary",
            r#"{"STR":12,"HP":18,"DAMAGE":13,"DELAY":22}"#,
            &["Warrior", "Paladin"],
        ))
        .unwrap();

    let comparison = store
        .compare_item_upgrade("WAR", &warrior_config(), candidate_id, Some(999_999))
        .unwrap()
        .expect("candidate should still be scored");

    assert_eq!(comparison.equipped_item_name, None);
    assert!(comparison.candidate_score > 0.0);
}

#[test]
fn scored_loot_falls_back_to_sell_for_downgrades_without_explicit_rules() {
    let rules = LootRules {
        loot_all: false,
        ..Default::default()
    };
    let downgrade = ItemScoreComparison {
        class_name: "Warrior".to_string(),
        slot: Some("Primary".to_string()),
        candidate_item_name: "Rusty Dagger".to_string(),
        equipped_item_name: Some("Fine Steel Dagger".to_string()),
        candidate_score: 5.0,
        equipped_score: 15.0,
        score_delta: -10.0,
        is_upgrade: false,
        candidate_usable: true,
        slot_match: true,
        breakdown: vec![WeightedStatDelta {
            stat: "DAMAGE".to_string(),
            candidate: 3.0,
            equipped: 7.0,
            delta: -4.0,
            weight: 2.0,
            weighted_delta: -8.0,
        }],
    };

    assert_eq!(
        classify_item_with_score("Rusty Dagger", &rules, Some(&downgrade)),
        ItemAction::Sell
    );
}

#[test]
fn loot_store_classify_item_for_loot_ignores_unusable_candidates() {
    let store = LootStore::open_memory().unwrap();
    let candidate_id = store
        .upsert_item(&sample_item(
            "Silk Wand",
            "Primary",
            r#"{"INT":10,"MANA":40}"#,
            &["Wizard"],
        ))
        .unwrap();

    let action = store
        .classify_item_for_loot(
            "WAR",
            &LootRules::default(),
            &warrior_config(),
            candidate_id,
            None,
        )
        .unwrap();

    assert_eq!(action, Some(ItemAction::Ignore));
}

#[test]
fn loot_store_classify_item_for_loot_applies_score_fallback() {
    let store = LootStore::open_memory().unwrap();
    let equipped_id = store
        .upsert_item(&sample_item(
            "Bronze Longsword",
            "Primary",
            r#"{"STR":4,"HP":5}"#,
            &["Warrior"],
        ))
        .unwrap();
    let candidate_id = store
        .upsert_item(&sample_item(
            "Jagged Velium Blade",
            "Primary",
            r#"{"STR":12,"HP":18,"DAMAGE":13,"DELAY":22}"#,
            &["Warrior", "Paladin"],
        ))
        .unwrap();

    let action = store
        .classify_item_for_loot(
            "WAR",
            &LootRules::default(),
            &warrior_config(),
            candidate_id,
            Some(equipped_id),
        )
        .unwrap();

    assert_eq!(action, Some(ItemAction::Keep));
}

#[test]
fn loot_store_classify_item_for_loot_ignores_when_class_weights_are_missing() {
    let store = LootStore::open_memory().unwrap();
    let candidate_id = store
        .upsert_item(&sample_item(
            "Jagged Velium Blade",
            "Primary",
            r#"{"STR":12,"HP":18,"DAMAGE":13,"DELAY":22}"#,
            &["Warrior", "Paladin"],
        ))
        .unwrap();

    let action = store
        .classify_item_for_loot(
            "WAR",
            &LootRules::default(),
            &ItemScoreConfig {
                min_upgrade_delta: 0.0,
                class_weights: BTreeMap::new(),
            },
            candidate_id,
            None,
        )
        .unwrap();

    assert_eq!(action, Some(ItemAction::Ignore));
}

// ── Slot matching edge cases ──────────────────────────────────────────────────

#[test]
fn compare_item_upgrade_slot_mismatch_yields_non_upgrade() {
    let config = warrior_config();
    let equipped = scoreable_item(
        "Dragonhide Belt",
        "Waist",
        &["Warrior"],
        &[("AC", 20.0), ("STR", 5.0)],
    );
    let candidate = scoreable_item(
        "Warden Sword",
        "Primary",
        &["Warrior"],
        &[("DAMAGE", 15.0), ("STR", 10.0)],
    );

    let comparison = compare_item_upgrade("WAR", &config, &candidate, Some(&equipped));

    assert!(!comparison.slot_match);
    assert!(!comparison.is_upgrade);
}

#[test]
fn compare_item_upgrade_slot_names_are_case_insensitive_and_normalized() {
    let config = warrior_config();
    let equipped = scoreable_item("Old Blade", "primary", &["Warrior"], &[("DAMAGE", 5.0)]);
    let candidate = scoreable_item("New Blade", "Primary", &["Warrior"], &[("DAMAGE", 12.0)]);

    let comparison = compare_item_upgrade("WAR", &config, &candidate, Some(&equipped));

    assert!(
        comparison.slot_match,
        "slot names should match case-insensitively"
    );
    assert!(comparison.is_upgrade);
}

// ── All-class item usability ─────────────────────────────────────────────────

#[test]
fn compare_item_upgrade_all_class_item_is_usable_by_any_class() {
    let config = warrior_config();
    // Empty `classes` means any class can use it.
    let candidate = scoreable_item("Plain Bag", "Primary", &[], &[("AC", 5.0), ("STR", 3.0)]);

    let comparison = compare_item_upgrade("WAR", &config, &candidate, None);

    assert!(
        comparison.candidate_usable,
        "item with empty class list should be usable by everyone"
    );
    assert!(comparison.is_upgrade);
}

// ── can_drive_loot_fallback ───────────────────────────────────────────────────

#[test]
fn can_drive_loot_fallback_false_when_breakdown_empty() {
    let comparison = ItemScoreComparison {
        class_name: "Warrior".to_string(),
        slot: Some("Primary".to_string()),
        candidate_item_name: "Rusty Dagger".to_string(),
        equipped_item_name: None,
        candidate_score: 0.0,
        equipped_score: 0.0,
        score_delta: 0.0,
        is_upgrade: false,
        candidate_usable: true,
        slot_match: true,
        breakdown: vec![],
    };
    assert!(!comparison.can_drive_loot_fallback());
}

#[test]
fn can_drive_loot_fallback_false_when_slot_does_not_match() {
    let comparison = ItemScoreComparison {
        class_name: "Warrior".to_string(),
        slot: Some("Waist".to_string()),
        candidate_item_name: "Sword".to_string(),
        equipped_item_name: None,
        candidate_score: 10.0,
        equipped_score: 5.0,
        score_delta: 5.0,
        is_upgrade: true,
        candidate_usable: true,
        slot_match: false,
        breakdown: vec![WeightedStatDelta {
            stat: "DAMAGE".to_string(),
            candidate: 10.0,
            equipped: 5.0,
            delta: 5.0,
            weight: 2.0,
            weighted_delta: 10.0,
        }],
    };
    assert!(!comparison.can_drive_loot_fallback());
}

// ── Default class weights completeness ───────────────────────────────────────

#[test]
fn default_item_score_config_has_weights_for_all_sixteen_classes() {
    let config = ItemScoreConfig::default();
    let classes = [
        "Warrior",
        "Cleric",
        "Paladin",
        "Ranger",
        "Shadow Knight",
        "Druid",
        "Monk",
        "Bard",
        "Rogue",
        "Shaman",
        "Necromancer",
        "Wizard",
        "Magician",
        "Enchanter",
        "Beastlord",
        "Berserker",
    ];
    for class in classes {
        assert!(
            config.weights_for_class(class).is_some(),
            "missing weights for {class}"
        );
    }
}

#[test]
fn weights_for_class_accepts_short_alias() {
    let config = ItemScoreConfig::default();
    // "WAR" should resolve to "Warrior" weights via canonical_class_name fallback.
    let weights_long = config.weights_for_class("Warrior");
    let weights_short = config.weights_for_class("WAR");
    assert!(weights_long.is_some());
    assert_eq!(weights_long, weights_short);
}
