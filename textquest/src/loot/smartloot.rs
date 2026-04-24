//! Smart loot filtering, loot-and-scoot planning, and inventory management.
//!
//! This module keeps filtering and timing decisions pure so the camp loop can
//! batch pickup commands without re-evaluating every item on every DLL tick.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use textquest_common::types::ClientId;

use super::{WishlistAction, WishlistManager, resolve_action_with_stats};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LootRarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Legendary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartLootConfig {
    #[serde(default = "default_true")]
    pub auto_loot: bool,
    #[serde(default = "default_vendor_trash_threshold")]
    pub vendor_trash_threshold: u32,
    #[serde(default = "default_loot_timeout")]
    pub loot_timeout: u64,
    #[serde(default = "default_scoot_threshold")]
    pub scoot_threshold: u64,
    #[serde(default = "default_vendor_trash_threshold")]
    pub value_threshold: u32,
    #[serde(default)]
    pub rarity_threshold: LootRarity,
    #[serde(default = "default_max_vendor_trash_weight")]
    pub max_vendor_trash_weight: f32,
}

impl Default for SmartLootConfig {
    fn default() -> Self {
        Self {
            auto_loot: true,
            vendor_trash_threshold: default_vendor_trash_threshold(),
            loot_timeout: default_loot_timeout(),
            scoot_threshold: default_scoot_threshold(),
            value_threshold: default_vendor_trash_threshold(),
            rarity_threshold: LootRarity::Common,
            max_vendor_trash_weight: default_max_vendor_trash_weight(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LootFilterSettings {
    #[serde(default = "default_true")]
    pub wishlist: bool,
    #[serde(default = "default_true")]
    pub vendor_trash: bool,
    #[serde(default = "default_true")]
    pub value: bool,
    #[serde(default = "default_true")]
    pub rarity: bool,
    #[serde(default = "default_true")]
    pub weight: bool,
}

impl Default for LootFilterSettings {
    fn default() -> Self {
        Self {
            wishlist: true,
            vendor_trash: true,
            value: true,
            rarity: true,
            weight: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootFilter {
    Wishlist,
    VendorTrash,
    Value,
    Rarity,
    Weight,
}

impl LootFilter {
    #[must_use]
    pub fn from_api_name(name: &str) -> Option<Self> {
        match name {
            "wishlist" | "wishlists" => Some(Self::Wishlist),
            "vendor_trash" | "vendor-trash" => Some(Self::VendorTrash),
            "value" | "value_threshold" => Some(Self::Value),
            "rarity" | "rarity_threshold" => Some(Self::Rarity),
            "weight" | "weight_rejection" => Some(Self::Weight),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LootCandidate {
    pub item_id: Option<u32>,
    pub item_name: String,
    #[serde(default = "default_quantity")]
    pub quantity: u32,
    #[serde(default)]
    pub value_platinum: u32,
    #[serde(default)]
    pub weight: f32,
    #[serde(default)]
    pub rarity: LootRarity,
}

impl LootCandidate {
    #[must_use]
    pub fn new(item_id: Option<u32>, item_name: impl Into<String>) -> Self {
        Self {
            item_id,
            item_name: item_name.into(),
            quantity: default_quantity(),
            value_platinum: 0,
            weight: 0.0,
            rarity: LootRarity::Common,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagedInventoryItem {
    pub item_id: Option<u32>,
    pub item_name: String,
    #[serde(default = "default_quantity")]
    pub quantity: u32,
    #[serde(default)]
    pub value_platinum: u32,
    #[serde(default)]
    pub weight: f32,
    #[serde(default)]
    pub rarity: LootRarity,
    #[serde(default)]
    pub equipped: bool,
}

impl ManagedInventoryItem {
    #[must_use]
    pub fn from_loot_candidate(item: LootCandidate) -> Self {
        Self {
            item_id: item.item_id,
            item_name: item.item_name,
            quantity: item.quantity,
            value_platinum: item.value_platinum,
            weight: item.weight,
            rarity: item.rarity,
            equipped: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentSet {
    pub name: String,
    #[serde(default)]
    pub item_ids: Vec<u32>,
    #[serde(default)]
    pub item_names: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InventoryManagementContext {
    #[serde(default)]
    pub inventory: Vec<ManagedInventoryItem>,
    #[serde(default)]
    pub cursor_item: Option<ManagedInventoryItem>,
    #[serde(default)]
    pub equipment_sets: Vec<EquipmentSet>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventorySortPlacement {
    pub target_index: usize,
    pub item: ManagedInventoryItem,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorItemAction {
    #[default]
    None,
    StowInInventory,
    ManualReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentSetStatus {
    pub name: String,
    pub present_count: usize,
    pub missing_item_names: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InventoryManagementPlan {
    pub sorted_items: Vec<InventorySortPlacement>,
    pub cursor_action: CursorItemAction,
    pub equipment_sets: Vec<EquipmentSetStatus>,
}

pub trait AutoLootInventoryManagement {
    fn plan_inventory_management(
        &self,
        context: &InventoryManagementContext,
    ) -> InventoryManagementPlan;
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmartLootContext<'a> {
    pub looter: ClientId,
    pub level: u8,
    pub class_bit: u8,
    pub wishlists: &'a WishlistManager,
    pub loot_started_at: u64,
    pub now: u64,
    pub session: Option<&'a LootAndScootSession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootAndScootSession {
    pub next_zone: String,
    pub started_at: u64,
    pub timeout: u64,
}

impl LootAndScootSession {
    #[must_use]
    pub fn new(next_zone: impl Into<String>, started_at: u64, timeout: u64) -> Self {
        Self {
            next_zone: next_zone.into(),
            started_at,
            timeout,
        }
    }

    #[must_use]
    pub fn elapsed(&self, now: u64) -> u64 {
        now.saturating_sub(self.started_at)
    }

    #[must_use]
    pub fn is_elapsed(&self, now: u64) -> bool {
        self.elapsed(now) >= self.timeout
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickupReason {
    Wishlist(WishlistAction),
    VendorTrash,
    ValueThreshold,
    RarityThreshold,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    AutoLootDisabled,
    LootTimedOut,
    HeavyVendorTrash,
    LowValueTrash,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedLootPickup {
    pub item: LootCandidate,
    pub reason: PickupReason,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkippedLoot {
    pub item: LootCandidate,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScootPlan {
    pub next_zone: String,
    pub elapsed: u64,
    pub timeout: u64,
    pub summon_group: bool,
    pub navigate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootLogEntry {
    pub item_name: String,
    pub quantity: u32,
    pub reason: PickupReason,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SmartLootPlan {
    pub pickups: Vec<PlannedLootPickup>,
    pub skipped: Vec<SkippedLoot>,
    pub loot_timed_out: bool,
    pub summon_group: bool,
    pub scoot: Option<ScootPlan>,
    pub loot_log: Vec<LootLogEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartLootPlanner {
    pub config: SmartLootConfig,
    pub filters: LootFilterSettings,
}

impl Default for SmartLootPlanner {
    fn default() -> Self {
        Self::new(SmartLootConfig::default())
    }
}

impl SmartLootPlanner {
    #[must_use]
    pub fn new(config: SmartLootConfig) -> Self {
        Self {
            config,
            filters: LootFilterSettings::default(),
        }
    }

    #[must_use]
    pub fn start_loot_and_scoot(
        &self,
        next_zone: impl Into<String>,
        now: u64,
        timeout: Option<u64>,
    ) -> LootAndScootSession {
        LootAndScootSession::new(
            next_zone,
            now,
            timeout.unwrap_or(self.config.scoot_threshold),
        )
    }

    pub fn set_loot_filter(&mut self, filter_name: &str, enabled: bool) -> bool {
        let Some(filter) = LootFilter::from_api_name(filter_name) else {
            return false;
        };

        match filter {
            LootFilter::Wishlist => self.filters.wishlist = enabled,
            LootFilter::VendorTrash => self.filters.vendor_trash = enabled,
            LootFilter::Value => self.filters.value = enabled,
            LootFilter::Rarity => self.filters.rarity = enabled,
            LootFilter::Weight => self.filters.weight = enabled,
        }
        true
    }

    #[must_use]
    pub fn plan_collection(
        &self,
        items: &[LootCandidate],
        context: SmartLootContext<'_>,
    ) -> SmartLootPlan {
        let loot_timed_out =
            context.now.saturating_sub(context.loot_started_at) >= self.config.loot_timeout;
        let scoot = self.plan_scoot(context.session, context.now);
        let summon_group = loot_timed_out || scoot.is_some();

        let mut plan = SmartLootPlan {
            loot_timed_out,
            summon_group,
            scoot,
            ..SmartLootPlan::default()
        };

        for item in items {
            if !self.config.auto_loot {
                plan.skipped.push(SkippedLoot {
                    item: item.clone(),
                    reason: SkipReason::AutoLootDisabled,
                });
                continue;
            }

            if loot_timed_out {
                plan.skipped.push(SkippedLoot {
                    item: item.clone(),
                    reason: SkipReason::LootTimedOut,
                });
                continue;
            }

            match self.pickup_reason(item, &context) {
                Some(reason) => {
                    plan.loot_log.push(LootLogEntry {
                        item_name: item.item_name.clone(),
                        quantity: item.quantity,
                        reason: reason.clone(),
                    });
                    plan.pickups.push(PlannedLootPickup {
                        item: item.clone(),
                        reason,
                    });
                }
                None => plan.skipped.push(SkippedLoot {
                    item: item.clone(),
                    reason: self.skip_reason(item),
                }),
            }
        }

        plan
    }

    fn pickup_reason(
        &self,
        item: &LootCandidate,
        context: &SmartLootContext<'_>,
    ) -> Option<PickupReason> {
        if self.filters.wishlist
            && let Some(item_id) = item.item_id
        {
            let action = resolve_action_with_stats(
                item_id,
                context.looter,
                context.level,
                context.class_bit,
                context.wishlists,
            );
            if !matches!(action, WishlistAction::Vendor) {
                return Some(PickupReason::Wishlist(action));
            }
        }

        if self.is_heavy_vendor_trash(item) {
            return None;
        }

        if self.filters.value && item.value_platinum >= self.config.value_threshold {
            return Some(PickupReason::ValueThreshold);
        }

        if self.filters.rarity && item.rarity >= self.config.rarity_threshold {
            return Some(PickupReason::RarityThreshold);
        }

        if self.filters.vendor_trash && item.value_platinum >= self.config.vendor_trash_threshold {
            return Some(PickupReason::VendorTrash);
        }

        None
    }

    fn skip_reason(&self, item: &LootCandidate) -> SkipReason {
        if self.is_heavy_vendor_trash(item) {
            SkipReason::HeavyVendorTrash
        } else {
            SkipReason::LowValueTrash
        }
    }

    fn is_heavy_vendor_trash(&self, item: &LootCandidate) -> bool {
        self.filters.weight
            && item.value_platinum <= self.config.vendor_trash_threshold
            && item.weight > self.config.max_vendor_trash_weight
    }

    fn plan_scoot(&self, session: Option<&LootAndScootSession>, now: u64) -> Option<ScootPlan> {
        let session = session?;
        let elapsed = session.elapsed(now);
        let timeout = session.timeout.min(self.config.scoot_threshold);
        if elapsed < timeout {
            return None;
        }

        Some(ScootPlan {
            next_zone: session.next_zone.clone(),
            elapsed,
            timeout,
            summon_group: true,
            navigate: true,
        })
    }
}

impl AutoLootInventoryManagement for SmartLootPlanner {
    fn plan_inventory_management(
        &self,
        context: &InventoryManagementContext,
    ) -> InventoryManagementPlan {
        let mut sorted_inventory = context.inventory.clone();
        sorted_inventory
            .sort_by(|left, right| compare_inventory_items(left, right, &context.equipment_sets));

        InventoryManagementPlan {
            sorted_items: sorted_inventory
                .into_iter()
                .enumerate()
                .map(|(target_index, item)| InventorySortPlacement { target_index, item })
                .collect(),
            cursor_action: context
                .cursor_item
                .as_ref()
                .map(|item| self.cursor_action_for(item))
                .unwrap_or_default(),
            equipment_sets: context
                .equipment_sets
                .iter()
                .map(|set| equipment_set_status(set, context))
                .collect(),
        }
    }
}

impl SmartLootPlanner {
    fn cursor_action_for(&self, item: &ManagedInventoryItem) -> CursorItemAction {
        if !self.config.auto_loot || self.is_managed_item_heavy_vendor_trash(item) {
            CursorItemAction::ManualReview
        } else {
            CursorItemAction::StowInInventory
        }
    }

    fn is_managed_item_heavy_vendor_trash(&self, item: &ManagedInventoryItem) -> bool {
        self.filters.weight
            && item.value_platinum <= self.config.vendor_trash_threshold
            && item.weight > self.config.max_vendor_trash_weight
    }
}

fn compare_inventory_items(
    left: &ManagedInventoryItem,
    right: &ManagedInventoryItem,
    equipment_sets: &[EquipmentSet],
) -> Ordering {
    let left_equipment_set_item = belongs_to_equipment_set(left, equipment_sets);
    let right_equipment_set_item = belongs_to_equipment_set(right, equipment_sets);

    right_equipment_set_item
        .cmp(&left_equipment_set_item)
        .then_with(|| right.equipped.cmp(&left.equipped))
        .then_with(|| right.rarity.cmp(&left.rarity))
        .then_with(|| right.value_platinum.cmp(&left.value_platinum))
        .then_with(|| {
            left.weight
                .partial_cmp(&right.weight)
                .unwrap_or(Ordering::Equal)
        })
        .then_with(|| left.item_name.cmp(&right.item_name))
        .then_with(|| left.item_id.cmp(&right.item_id))
}

fn belongs_to_equipment_set(item: &ManagedInventoryItem, equipment_sets: &[EquipmentSet]) -> bool {
    equipment_sets.iter().any(|set| item_matches_set(item, set))
}

fn equipment_set_status(
    set: &EquipmentSet,
    context: &InventoryManagementContext,
) -> EquipmentSetStatus {
    let mut present_count = 0;
    let mut missing_item_names = Vec::new();

    for item_name in &set.item_names {
        if context
            .inventory
            .iter()
            .chain(context.cursor_item.iter())
            .any(|item| normalized_item_name(&item.item_name) == normalized_item_name(item_name))
        {
            present_count += 1;
        } else {
            missing_item_names.push(item_name.clone());
        }
    }

    present_count += set
        .item_ids
        .iter()
        .filter(|item_id| {
            context
                .inventory
                .iter()
                .chain(context.cursor_item.iter())
                .any(|item| item.item_id == Some(**item_id))
        })
        .count();

    EquipmentSetStatus {
        name: set.name.clone(),
        present_count,
        missing_item_names,
    }
}

fn item_matches_set(item: &ManagedInventoryItem, set: &EquipmentSet) -> bool {
    item.item_id
        .is_some_and(|item_id| set.item_ids.contains(&item_id))
        || set
            .item_names
            .iter()
            .any(|name| normalized_item_name(name) == normalized_item_name(&item.item_name))
}

fn normalized_item_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn default_true() -> bool {
    true
}

fn default_vendor_trash_threshold() -> u32 {
    10
}

fn default_loot_timeout() -> u64 {
    30
}

fn default_scoot_threshold() -> u64 {
    300
}

fn default_max_vendor_trash_weight() -> f32 {
    5.0
}

fn default_quantity() -> u32 {
    1
}
