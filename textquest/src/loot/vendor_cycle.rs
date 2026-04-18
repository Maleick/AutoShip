//! Pure vendorable-item planning for the camp vendor controller.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use textquest_common::ipc::ContainerSlotInfo;

use super::LootHistoryRow;

const SQLITE_TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// Why an item was selected for vendor sale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VendorReason {
    /// Explicit trash/junk item.
    Trash,
    /// Extra copy of loot that should not be kept.
    DuplicateLoot,
    /// Loot that has sat in inventory beyond the backlog threshold.
    Backlog,
    /// Operator override from an explicit sell allowlist.
    Allowlist,
    /// Legacy/manual queue item.
    Manual,
}

/// One vendorable item selected by the planner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorSaleItem {
    pub item_id: Option<i64>,
    pub item_name: String,
    pub stack_count: i64,
    /// Estimated total plat value for this queue entry.
    pub estimated_vendor_value: i64,
    pub reason: VendorReason,
}

/// Full vendor plan for one cycle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VendorPlan {
    pub items: Vec<VendorSaleItem>,
    pub estimated_gross_plat: i64,
}

/// Inventory item candidate used by the vendoring planner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorInventoryItem {
    pub item_id: Option<i64>,
    pub item_name: String,
    pub stack_count: i64,
    /// Estimated total plat value for the stack/entry.
    pub estimated_vendor_value: i64,
    pub acquired_at: Option<DateTime<Utc>>,
    pub is_trash: bool,
    pub protected: bool,
}

impl VendorInventoryItem {
    /// Construct a standard loot item candidate.
    #[must_use]
    pub fn loot(
        item_name: &str,
        stack_count: i64,
        estimated_vendor_value: i64,
        acquired_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            item_id: None,
            item_name: item_name.to_string(),
            stack_count,
            estimated_vendor_value,
            acquired_at,
            is_trash: false,
            protected: false,
        }
    }

    /// Construct a trash/junk item candidate.
    #[must_use]
    pub fn trash(item_name: &str, stack_count: i64, estimated_vendor_value: i64) -> Self {
        Self {
            item_id: None,
            item_name: item_name.to_string(),
            stack_count,
            estimated_vendor_value,
            acquired_at: None,
            is_trash: true,
            protected: false,
        }
    }

    /// Mark the item as protected from vendoring.
    #[must_use]
    pub fn protected(mut self) -> Self {
        self.protected = true;
        self
    }

    /// Convert loot history into an inventory candidate.
    #[must_use]
    pub fn from_loot_history(row: &LootHistoryRow, estimated_vendor_value: i64) -> Self {
        Self {
            item_id: row.item_id,
            item_name: row.item_name.clone(),
            stack_count: row.quantity.max(1),
            estimated_vendor_value,
            acquired_at: parse_sqlite_timestamp(&row.timestamp),
            is_trash: false,
            protected: false,
        }
    }
}

impl TryFrom<&ContainerSlotInfo> for VendorInventoryItem {
    type Error = &'static str;

    fn try_from(slot: &ContainerSlotInfo) -> Result<Self, Self::Error> {
        let item = slot.item.as_ref().ok_or("container slot has no item")?;
        Ok(Self {
            item_id: Some(i64::from(item.id)),
            item_name: item.name.clone(),
            stack_count: i64::from(item.stack_count.max(1)),
            estimated_vendor_value: 0,
            acquired_at: None,
            is_trash: false,
            protected: false,
        })
    }
}

/// Builds vendorable plans from inventory snapshots.
#[derive(Debug, Clone)]
pub struct VendorCyclePlanner {
    keep_names: HashSet<String>,
    allowlist: HashSet<String>,
    backlog_days: i64,
}

impl Default for VendorCyclePlanner {
    fn default() -> Self {
        Self {
            keep_names: HashSet::new(),
            allowlist: HashSet::new(),
            backlog_days: 30,
        }
    }
}

impl VendorCyclePlanner {
    /// Create a planner that protects the provided keep-list item names.
    #[must_use]
    pub fn new<I, S>(keep_items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            keep_names: keep_items
                .into_iter()
                .map(|item| normalize_name(item.as_ref()))
                .collect(),
            ..Self::default()
        }
    }

    /// Add an explicit always-sell allowlist.
    #[must_use]
    pub fn with_allowlist<I, S>(mut self, sell_items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.allowlist = sell_items
            .into_iter()
            .map(|item| normalize_name(item.as_ref()))
            .collect();
        self
    }

    /// Override the backlog age threshold in days.
    #[must_use]
    pub fn with_backlog_days(mut self, backlog_days: i64) -> Self {
        self.backlog_days = backlog_days.max(0);
        self
    }

    /// Produce a vendor plan for the current inventory snapshot.
    #[must_use]
    pub fn plan(&self, inventory: &[VendorInventoryItem], now: DateTime<Utc>) -> VendorPlan {
        let duplicate_indexes = self.duplicate_indexes(inventory);
        let backlog_cutoff = now - Duration::days(self.backlog_days);
        let mut items = Vec::new();

        for (idx, item) in inventory.iter().enumerate() {
            if self.is_protected(item) {
                continue;
            }

            let normalized = normalize_name(&item.item_name);
            let use_allowlist = !self.allowlist.is_empty();
            if use_allowlist && !self.allowlist.contains(&normalized) {
                continue;
            }

            let reason = if item.is_trash {
                Some(VendorReason::Trash)
            } else if item
                .acquired_at
                .is_some_and(|acquired_at| acquired_at <= backlog_cutoff)
            {
                Some(VendorReason::Backlog)
            } else if duplicate_indexes.contains(&idx) {
                Some(VendorReason::DuplicateLoot)
            } else if use_allowlist {
                Some(VendorReason::Allowlist)
            } else {
                None
            };

            if let Some(reason) = reason {
                items.push(VendorSaleItem {
                    item_id: item.item_id,
                    item_name: item.item_name.clone(),
                    stack_count: item.stack_count.max(1),
                    estimated_vendor_value: item.estimated_vendor_value.max(0),
                    reason,
                });
            }
        }

        let estimated_gross_plat = items
            .iter()
            .map(|item| item.estimated_vendor_value.max(0))
            .sum();

        VendorPlan {
            items,
            estimated_gross_plat,
        }
    }

    fn duplicate_indexes(&self, inventory: &[VendorInventoryItem]) -> HashSet<usize> {
        type IndexedAcquisition = (usize, Option<DateTime<Utc>>);
        type DuplicateGroups = HashMap<String, Vec<IndexedAcquisition>>;

        let mut groups: DuplicateGroups = HashMap::new();

        for (idx, item) in inventory.iter().enumerate() {
            if self.is_protected(item) {
                continue;
            }
            groups
                .entry(normalize_name(&item.item_name))
                .or_default()
                .push((idx, item.acquired_at));
        }

        let mut duplicates = HashSet::new();
        for group in groups.values_mut() {
            group.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
            for (idx, _) in group.iter().skip(1) {
                duplicates.insert(*idx);
            }
        }
        duplicates
    }

    fn is_protected(&self, item: &VendorInventoryItem) -> bool {
        item.protected || self.keep_names.contains(&normalize_name(&item.item_name))
    }
}

fn normalize_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn parse_sqlite_timestamp(timestamp: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(timestamp, SQLITE_TIMESTAMP_FORMAT)
        .ok()
        .map(|value| value.and_utc())
}
