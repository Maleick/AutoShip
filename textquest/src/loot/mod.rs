//! EQ item database, TLP loot tables, wishlists, and loot history.
//!
//! SQLite-backed store for item data imported from Lucy/Allakhazam,
//! Dave's TLP random loot table overlays, per-character gear wishlists,
//! and historical loot tracking with "who needs this?" queries.
//!
//! The [`distributor`] module provides a pure FSM for routing looted items
//! through Reserve → Assign → Execute phases with automatic retry.

pub mod intent;
mod item_score;
pub mod ledger;
pub mod smartloot;
mod store;
pub mod vendor_cycle;
mod wishlist;

pub use intent::{IntentTracker, ItemIntent, WishlistEntry};
pub use item_score::{
    ItemScoreComparison, ItemScoreConfig, ScoreableItem, StatWeights, WeightedStatDelta,
    compare_item_upgrade,
};
pub use ledger::{DaySummary, EconomyLedger, EntrySource, LedgerEntry, TrendReport};
pub use smartloot::{
    LootAndScootSession, LootCandidate, LootFilter, LootFilterSettings, LootLogEntry, LootRarity,
    PickupReason, PlannedLootPickup, ScootPlan, SkipReason, SkippedLoot, SmartLootConfig,
    SmartLootContext, SmartLootPlan, SmartLootPlanner,
};
pub use store::{
    DropRateRow, ImportItem, ItemRow, ItemSearchFilter, LootHistoryRow, LootStore, LootTableRow,
    WishlistRow,
};
pub use vendor_cycle::{
    VendorCyclePlanner, VendorInventoryItem, VendorPlan, VendorReason, VendorSaleItem,
};
pub use wishlist::{
    ReserveRule, RuleConditions, WishlistAction, WishlistManager, WishlistRule, resolve_action,
    resolve_action_with_stats,
};

#[cfg(test)]
mod vendor_cycle_tests {
    use chrono::TimeZone;

    use super::{
        LootHistoryRow,
        vendor_cycle::{VendorCyclePlanner, VendorInventoryItem, VendorReason},
    };

    #[test]
    fn vendor_cycle_marks_trash_duplicates_and_backlog() {
        let planner = VendorCyclePlanner::default();
        let now = chrono::Utc.with_ymd_and_hms(2026, 4, 17, 12, 0, 0).unwrap();

        let items = vec![
            VendorInventoryItem::trash("Torn Cloth Sandal", 1, 3),
            VendorInventoryItem::loot(
                "Fine Steel Long Sword",
                1,
                12,
                Some(chrono::Utc.with_ymd_and_hms(2026, 4, 16, 12, 0, 0).unwrap()),
            ),
            VendorInventoryItem::loot(
                "Fine Steel Long Sword",
                1,
                12,
                Some(chrono::Utc.with_ymd_and_hms(2026, 4, 15, 12, 0, 0).unwrap()),
            ),
            VendorInventoryItem::loot(
                "Ancient Coin",
                1,
                1,
                Some(chrono::Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0).unwrap()),
            ),
        ];

        let plan = planner.plan(&items, now);

        assert_eq!(plan.items.len(), 3);
        assert_eq!(plan.items[0].reason, VendorReason::Trash);
        assert_eq!(plan.items[1].reason, VendorReason::DuplicateLoot);
        assert_eq!(plan.items[2].reason, VendorReason::Backlog);
        assert_eq!(plan.estimated_gross_plat, 16);
    }

    #[test]
    fn vendor_cycle_keeps_protected_items_even_if_old_or_duplicate() {
        let planner = VendorCyclePlanner::new(["Bone Chips"]);
        let now = chrono::Utc.with_ymd_and_hms(2026, 4, 17, 12, 0, 0).unwrap();

        let items = vec![
            VendorInventoryItem::loot(
                "Bone Chips",
                1,
                1,
                Some(chrono::Utc.with_ymd_and_hms(2026, 2, 1, 12, 0, 0).unwrap()),
            ),
            VendorInventoryItem::loot(
                "Bone Chips",
                1,
                1,
                Some(chrono::Utc.with_ymd_and_hms(2026, 2, 2, 12, 0, 0).unwrap()),
            ),
        ];

        let plan = planner.plan(&items, now);
        assert!(plan.items.is_empty());
    }

    #[test]
    fn vendor_cycle_allowlist_filters_out_non_allowlisted_auto_sales() {
        let planner = VendorCyclePlanner::default().with_allowlist(["Rusty Axe"]);
        let now = chrono::Utc.with_ymd_and_hms(2026, 4, 17, 12, 0, 0).unwrap();

        let items = vec![
            VendorInventoryItem::trash("Torn Cloth Sandal", 1, 3),
            VendorInventoryItem::loot("Rusty Axe", 1, 5, None),
            VendorInventoryItem::loot(
                "Ancient Coin",
                1,
                1,
                Some(chrono::Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0).unwrap()),
            ),
        ];

        let plan = planner.plan(&items, now);

        assert_eq!(plan.items.len(), 1);
        assert_eq!(plan.items[0].item_name, "Rusty Axe");
        assert_eq!(plan.items[0].reason, VendorReason::Allowlist);
        assert_eq!(plan.estimated_gross_plat, 5);
    }

    #[test]
    fn vendor_inventory_item_from_loot_history_row_preserves_timestamp() {
        let row = LootHistoryRow {
            id: 1,
            timestamp: "2026-03-01 08:30:00".into(),
            item_id: Some(777),
            item_name: "Ancient Coin".into(),
            recipient: "Cleric01".into(),
            source_mob: None,
            zone: Some("nexus".into()),
            table_name: None,
            quantity: 2,
            assigned_by: None,
        };

        let item = VendorInventoryItem::from_loot_history(&row, 5);

        assert_eq!(item.item_id, Some(777));
        assert_eq!(item.item_name, "Ancient Coin");
        assert_eq!(item.stack_count, 2);
        assert_eq!(
            item.acquired_at,
            Some(chrono::Utc.with_ymd_and_hms(2026, 3, 1, 8, 30, 0).unwrap())
        );
    }
}
