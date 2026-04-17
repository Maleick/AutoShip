//! EQ item database, TLP loot tables, wishlists, and loot history.
//!
//! SQLite-backed store for item data imported from Lucy/Allakhazam,
//! Dave's TLP random loot table overlays, per-character gear wishlists,
//! and historical loot tracking with "who needs this?" queries.
//!
//! The [`distributor`] module provides a pure FSM for routing looted items
//! through Reserve → Assign → Execute phases with automatic retry.

mod item_score;
pub mod ledger;
mod store;
mod wishlist;

pub use item_score::{
    ItemScoreComparison, ItemScoreConfig, ScoreableItem, StatWeights, WeightedStatDelta,
    compare_item_upgrade,
};
pub use ledger::{DaySummary, EconomyLedger, EntrySource, LedgerEntry, TrendReport};
pub use store::{
    DropRateRow, ImportItem, ItemRow, ItemSearchFilter, LootHistoryRow, LootStore, LootTableRow,
    WishlistRow,
};
pub use wishlist::{
    ReserveRule, RuleConditions, WishlistAction, WishlistManager, WishlistRule, resolve_action,
    resolve_action_with_stats,
};
