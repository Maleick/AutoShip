//! EQ item database, TLP loot tables, wishlists, and loot history.
//!
//! SQLite-backed store for item data imported from Lucy/Allakhazam,
//! Dave's TLP random loot table overlays, per-character gear wishlists,
//! and historical loot tracking with "who needs this?" queries.
//!
//! The [`distributor`] module provides a pure FSM for routing looted items
//! through Reserve → Assign → Execute phases with automatic retry.

pub mod distributor;
mod store;
pub mod wishlist;

pub use distributor::{
    AssignFn, AssignmentTarget, DistributionJob, DistributionPhase, ExecuteFn, ExecuteResult,
    LootDistributor, LootItem, MAX_RETRIES,
};
pub use store::{
    DropRateRow, ImportItem, ItemRow, ItemSearchFilter, LootHistoryRow, LootStore, LootTableRow,
    WishlistRow,
};
pub use wishlist::{
    ReserveRule, RuleConditions, WishlistAction, WishlistManager, WishlistRule, resolve_action,
    resolve_action_with_stats,
};
