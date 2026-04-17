use serde::{Deserialize, Serialize};

use crate::{
    character_config::{
        RewardAutomationConfig, RewardPreference, TaskRewardPreference, resolve_reward_index,
    },
    nav::{RelocationOptionState, select_best_relocation_option},
};

fn normalize_matcher(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PluginCoverageStatus {
    #[default]
    Native,
    Adapted,
    Deferred,
}

impl PluginCoverageStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Adapted => "adapted",
            Self::Deferred => "deferred",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMapping {
    pub plugin: String,
    pub owner: String,
    pub status: PluginCoverageStatus,
    pub config_surface: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LegacyAdapterWarning {
    pub plugin: String,
    pub source_reference: String,
    pub adapted_into: String,
    #[serde(default)]
    pub unsupported_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemKnowledgeConfig {
    #[serde(default)]
    pub link_sources: Vec<String>,
    pub show_provenance: bool,
    pub show_unsupported_fields: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CursorAction {
    Keep,
    Sell,
    Destroy,
    Consume,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CursorRule {
    pub item_matcher: String,
    pub action: CursorAction,
    pub keep_at_or_below: Option<u32>,
    pub overflow_action: Option<CursorAction>,
}

#[must_use]
pub fn decide_cursor_action(
    item_name: &str,
    owned_count: u32,
    rules: &[CursorRule],
) -> CursorAction {
    let normalized_item = normalize_matcher(item_name);

    let Some(rule) = rules
        .iter()
        .find(|rule| normalize_matcher(&rule.item_matcher) == normalized_item)
    else {
        return CursorAction::Keep;
    };

    match (rule.keep_at_or_below, rule.overflow_action) {
        (Some(limit), Some(overflow)) if owned_count > limit => overflow,
        _ => rule.action,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CollectionRoute {
    Keep,
    Bank,
    Tribute,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CollectionRoutingRule {
    pub set_matcher: String,
    pub incomplete_route: CollectionRoute,
    pub completed_route: CollectionRoute,
    pub duplicate_route: CollectionRoute,
}

#[must_use]
pub fn route_collection_item(
    set_name: &str,
    duplicate: bool,
    completed: bool,
    rules: &[CollectionRoutingRule],
) -> CollectionRoute {
    let normalized_set = normalize_matcher(set_name);
    let Some(rule) = rules
        .iter()
        .find(|rule| normalize_matcher(&rule.set_matcher) == normalized_set)
    else {
        return if duplicate || completed {
            CollectionRoute::Bank
        } else {
            CollectionRoute::Keep
        };
    };

    if duplicate {
        rule.duplicate_route
    } else if completed {
        rule.completed_route
    } else {
        rule.incomplete_route
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RewardRoutingRule {
    pub task_matcher: String,
    pub preference: RewardPreference,
    pub auto_claim: bool,
}

impl RewardRoutingRule {
    #[must_use]
    pub fn by_position(
        task_matcher: impl Into<String>,
        reward_position: usize,
        auto_claim: bool,
    ) -> Self {
        Self {
            task_matcher: task_matcher.into(),
            preference: RewardPreference::ByPosition { reward_position },
            auto_claim,
        }
    }

    #[must_use]
    pub fn by_name(
        task_matcher: impl Into<String>,
        reward_name: impl Into<String>,
        auto_claim: bool,
    ) -> Self {
        Self {
            task_matcher: task_matcher.into(),
            preference: RewardPreference::ByName {
                reward_name: reward_name.into(),
            },
            auto_claim,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewardClaimDecision {
    pub reward_index: usize,
    pub auto_claim: bool,
}

#[must_use]
pub fn resolve_reward_claim(
    task_name: &str,
    reward_names: &[String],
    rules: &[RewardRoutingRule],
) -> Option<RewardClaimDecision> {
    if reward_names.is_empty() {
        return None;
    }

    let config = RewardAutomationConfig {
        rules: rules
            .iter()
            .map(|rule| TaskRewardPreference {
                task_matcher: rule.task_matcher.clone(),
                preference: rule.preference.clone(),
            })
            .collect(),
    };

    let reward_index = resolve_reward_index(task_name, reward_names, &config)?;
    let normalized_task = normalize_matcher(task_name);
    let auto_claim = rules
        .iter()
        .find(|rule| normalize_matcher(&rule.task_matcher) == normalized_task)
        .or_else(|| {
            rules.iter().find(|rule| {
                let matcher = normalize_matcher(&rule.task_matcher);
                matcher.is_empty() || matcher == "*"
            })
        })
        .map(|rule| rule.auto_claim)
        .unwrap_or(false);

    Some(RewardClaimDecision {
        reward_index,
        auto_claim,
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsumableKind {
    Food,
    Drink,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumableStack {
    pub item_name: String,
    pub kind: ConsumableKind,
}

impl ConsumableStack {
    #[must_use]
    pub fn new(item_name: impl Into<String>, kind: ConsumableKind) -> Self {
        Self {
            item_name: item_name.into(),
            kind,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsumablePreferences {
    pub enabled: bool,
    #[serde(default)]
    pub preferred_food: Vec<String>,
    #[serde(default)]
    pub preferred_drink: Vec<String>,
    #[serde(default)]
    pub ignored_items: Vec<String>,
}

#[must_use]
pub fn select_consumable(
    kind: ConsumableKind,
    inventory: &[ConsumableStack],
    preferences: &ConsumablePreferences,
) -> Option<String> {
    if !preferences.enabled {
        return None;
    }

    let ignored = preferences
        .ignored_items
        .iter()
        .map(|item| normalize_matcher(item))
        .collect::<Vec<_>>();
    let preferred = match kind {
        ConsumableKind::Food => &preferences.preferred_food,
        ConsumableKind::Drink => &preferences.preferred_drink,
    };

    let available = inventory
        .iter()
        .filter(|item| item.kind == kind)
        .filter(|item| !ignored.contains(&normalize_matcher(&item.item_name)))
        .collect::<Vec<_>>();

    preferred
        .iter()
        .find_map(|preferred_item| {
            available
                .iter()
                .find(|candidate| candidate.item_name.eq_ignore_ascii_case(preferred_item))
                .map(|candidate| candidate.item_name.clone())
        })
        .or_else(|| {
            available
                .first()
                .map(|candidate| candidate.item_name.clone())
        })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VendorWatchRule {
    pub item_name: String,
    pub max_price_pp: Option<u32>,
    pub notify: bool,
}

impl VendorWatchRule {
    #[must_use]
    pub fn notify_on(item_name: impl Into<String>, max_price_pp: Option<u32>) -> Self {
        Self {
            item_name: item_name.into(),
            max_price_pp,
            notify: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VendorListing {
    pub item_name: String,
    pub price_pp: u32,
}

impl VendorListing {
    #[must_use]
    pub fn new(item_name: impl Into<String>, price_pp: u32) -> Self {
        Self {
            item_name: item_name.into(),
            price_pp,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VendorWatchAlert {
    pub item_name: String,
    pub price_pp: u32,
}

#[must_use]
pub fn vendor_watch_alerts(
    listings: &[VendorListing],
    rules: &[VendorWatchRule],
) -> Vec<VendorWatchAlert> {
    listings
        .iter()
        .filter_map(|listing| {
            rules
                .iter()
                .find(|rule| {
                    rule.notify
                        && rule.item_name.eq_ignore_ascii_case(&listing.item_name)
                        && rule.max_price_pp.is_none_or(|max| listing.price_pp <= max)
                })
                .map(|_| VendorWatchAlert {
                    item_name: listing.item_name.clone(),
                    price_pp: listing.price_pp,
                })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelocationRule {
    pub destination: String,
    pub required_option_id: Option<String>,
    pub keep_on_hand: u32,
    pub notify_if_unavailable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelocationRuleStatus {
    pub destination: String,
    pub selected_option_id: Option<String>,
    pub ready: bool,
    pub keep_on_hand: u32,
    pub warning: Option<String>,
}

#[must_use]
pub fn evaluate_relocation_rules(
    rules: &[RelocationRule],
    loadout: &[RelocationOptionState],
) -> Vec<RelocationRuleStatus> {
    rules
        .iter()
        .map(|rule| {
            let destination = normalize_matcher(&rule.destination);
            let candidates = loadout
                .iter()
                .filter(|entry| entry.option.zone_name == destination)
                .filter(|entry| {
                    rule.required_option_id
                        .as_ref()
                        .is_none_or(|required| entry.option.id.eq_ignore_ascii_case(required))
                })
                .cloned()
                .collect::<Vec<_>>();
            let selected = select_best_relocation_option(&candidates);
            let ready = selected.is_some_and(|entry| entry.ready);
            let selected_option_id = selected.map(|entry| entry.option.id.clone());
            let warning = if ready || !rule.notify_if_unavailable {
                None
            } else if let Some(required) = &rule.required_option_id {
                Some(format!(
                    "Required relocation option '{}' is unavailable for {}.",
                    required, rule.destination
                ))
            } else {
                Some(format!(
                    "No ready relocation item or AA is available for {}.",
                    rule.destination
                ))
            };

            RelocationRuleStatus {
                destination,
                selected_option_id,
                ready,
                keep_on_hand: rule.keep_on_hand,
                warning,
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrophyPreferences {
    pub enabled: bool,
    pub auto_equip: bool,
    pub restore_after_craft: bool,
    #[serde(default)]
    pub trophy_items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AutoClaimPreferences {
    pub enabled: bool,
    pub claim_membership_grants: bool,
    pub claim_task_windows: bool,
    pub once_per_session: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryUtilityConfig {
    #[serde(default)]
    pub plugin_mappings: Vec<PluginMapping>,
    #[serde(default)]
    pub legacy_adapters: Vec<LegacyAdapterWarning>,
    pub item_knowledge: ItemKnowledgeConfig,
    #[serde(default)]
    pub cursor_rules: Vec<CursorRule>,
    #[serde(default)]
    pub collection_routing: Vec<CollectionRoutingRule>,
    #[serde(default)]
    pub reward_routing: Vec<RewardRoutingRule>,
    pub consumption: ConsumablePreferences,
    #[serde(default)]
    pub vendor_watch: Vec<VendorWatchRule>,
    #[serde(default)]
    pub relocation_rules: Vec<RelocationRule>,
    pub trophy_preferences: TrophyPreferences,
    pub auto_claim: AutoClaimPreferences,
}

impl Default for InventoryUtilityConfig {
    fn default() -> Self {
        default_inventory_utility_config()
    }
}

#[must_use]
pub fn default_inventory_utility_config() -> InventoryUtilityConfig {
    InventoryUtilityConfig {
        plugin_mappings: vec![
            PluginMapping {
                plugin: "MQ2LinkDB".into(),
                owner: "inventory_utility.item_knowledge".into(),
                status: PluginCoverageStatus::Adapted,
                config_surface: "Loot Config > Inventory Utilities".into(),
                notes: "Uses TextQuest item-knowledge sources plus provenance instead of direct in-game link generation.".into(),
            },
            PluginMapping {
                plugin: "MQ2ItemScore".into(),
                owner: "loot::item_score".into(),
                status: PluginCoverageStatus::Native,
                config_surface: "Loot Config > Item Score".into(),
                notes: "Native weighted upgrade scoring drives keep or sell fallback decisions.".into(),
            },
            PluginMapping {
                plugin: "MQ2Cursor".into(),
                owner: "inventory_utility.cursor_rules".into(),
                status: PluginCoverageStatus::Adapted,
                config_surface: "Loot Config > Inventory Utilities".into(),
                notes: "Cursor actions map onto keep, sell, destroy, and consume rules with quantity caps.".into(),
            },
            PluginMapping {
                plugin: "MQ2Collections".into(),
                owner: "inventory_utility.collection_routing".into(),
                status: PluginCoverageStatus::Adapted,
                config_surface: "Loot Config > Inventory Utilities".into(),
                notes: "Set routing decides whether collectible duplicates are kept, banked, sold, or tributed.".into(),
            },
            PluginMapping {
                plugin: "MQ2Collectible".into(),
                owner: "camp::collectibles".into(),
                status: PluginCoverageStatus::Native,
                config_surface: "Loot Config > Inventory Utilities".into(),
                notes: "Native collection tracking owns set progress and duplicate handling.".into(),
            },
            PluginMapping {
                plugin: "MQ2TributeManager".into(),
                owner: "camp::collectibles::TributeAutomationController".into(),
                status: PluginCoverageStatus::Native,
                config_surface: "Tuning Panel > Tribute Automation".into(),
                notes: "Tribute expiry and activation are handled natively, with inventory utility docs tracking the parity status.".into(),
            },
            PluginMapping {
                plugin: "MQ2TSTrophy".into(),
                owner: "inventory_utility.trophy_preferences".into(),
                status: PluginCoverageStatus::Deferred,
                config_surface: "Loot Config > Inventory Utilities".into(),
                notes: "Operator preferences are tracked now; full trophy equip automation is explicitly deferred.".into(),
            },
            PluginMapping {
                plugin: "MQ2Rewards".into(),
                owner: "inventory_utility.reward_routing".into(),
                status: PluginCoverageStatus::Native,
                config_surface: "Loot Config > Inventory Utilities".into(),
                notes: "Task reward selection and claim intent route through normalized reward rules.".into(),
            },
            PluginMapping {
                plugin: "MQ2FeedMe".into(),
                owner: "inventory_utility.consumption".into(),
                status: PluginCoverageStatus::Adapted,
                config_surface: "Loot Config > Inventory Utilities".into(),
                notes: "Food and drink policies are normalized into preferred consumables and ignore lists.".into(),
            },
            PluginMapping {
                plugin: "MQ2PortalSetter".into(),
                owner: "inventory_utility.relocation_rules".into(),
                status: PluginCoverageStatus::Adapted,
                config_surface: "Loot Config > Inventory Utilities".into(),
                notes: "Portal presets are modeled as relocation destinations backed by clickies or AAs.".into(),
            },
            PluginMapping {
                plugin: "MQ2Relocate".into(),
                owner: "nav::relocate".into(),
                status: PluginCoverageStatus::Native,
                config_surface: "Loot Config > Inventory Utilities".into(),
                notes: "Travel routing already selects the best ready relocation option per destination.".into(),
            },
            PluginMapping {
                plugin: "MQ2Vendors".into(),
                owner: "inventory_utility.vendor_watch".into(),
                status: PluginCoverageStatus::Adapted,
                config_surface: "Loot Config > Inventory Utilities".into(),
                notes: "Vendor watch rules complement the existing sell-cycle controller with watched-item alerts.".into(),
            },
            PluginMapping {
                plugin: "MQ2AutoClaim".into(),
                owner: "inventory_utility.auto_claim".into(),
                status: PluginCoverageStatus::Adapted,
                config_surface: "Loot Config > Inventory Utilities".into(),
                notes: "Claim policies are tracked in the shared inventory surface while popup automation remains adapter-backed.".into(),
            },
        ],
        legacy_adapters: vec![
            LegacyAdapterWarning {
                plugin: "MQ2LinkDB".into(),
                source_reference: "legacy_ini:itemdb".into(),
                adapted_into: "item_knowledge".into(),
                unsupported_fields: vec![
                    "custom link color formatting".into(),
                    "chat-channel output templates".into(),
                ],
            },
            LegacyAdapterWarning {
                plugin: "MQ2Cursor".into(),
                source_reference: "legacy_ini:cursor".into(),
                adapted_into: "cursor_rules".into(),
                unsupported_fields: vec!["drop-on-ground overflow actions".into()],
            },
            LegacyAdapterWarning {
                plugin: "MQ2PortalSetter".into(),
                source_reference: "legacy_ini:portalsetter".into(),
                adapted_into: "relocation_rules".into(),
                unsupported_fields: vec!["campfire placement macros".into()],
            },
        ],
        item_knowledge: ItemKnowledgeConfig {
            link_sources: vec!["item-db".into(), "loot-history".into(), "reward-rules".into()],
            show_provenance: true,
            show_unsupported_fields: true,
        },
        cursor_rules: vec![
            CursorRule {
                item_matcher: "Fine Steel Breastplate".into(),
                action: CursorAction::Keep,
                keep_at_or_below: Some(1),
                overflow_action: Some(CursorAction::Destroy),
            },
            CursorRule {
                item_matcher: "Fish Roll".into(),
                action: CursorAction::Consume,
                keep_at_or_below: Some(40),
                overflow_action: Some(CursorAction::Sell),
            },
        ],
        collection_routing: vec![CollectionRoutingRule {
            set_matcher: "Frostcrypt".into(),
            incomplete_route: CollectionRoute::Keep,
            completed_route: CollectionRoute::Bank,
            duplicate_route: CollectionRoute::Tribute,
        }],
        reward_routing: vec![
            RewardRoutingRule::by_name("Hero's Mission", "Heroic Augment", true),
            RewardRoutingRule::by_position("*", 1, false),
        ],
        consumption: ConsumablePreferences {
            enabled: true,
            preferred_food: vec!["Fish Roll".into(), "Misty Thicket Picnic".into()],
            preferred_drink: vec!["Water Flask".into(), "Kaladim Constitutional".into()],
            ignored_items: vec!["Summoned: Modulation Shard".into()],
        },
        vendor_watch: vec![VendorWatchRule::notify_on("Peridot", Some(100))],
        relocation_rules: vec![
            RelocationRule {
                destination: "guildlobby".into(),
                required_option_id: None,
                keep_on_hand: 1,
                notify_if_unavailable: true,
            },
            RelocationRule {
                destination: "guildhall".into(),
                required_option_id: Some("secondary_anchor".into()),
                keep_on_hand: 1,
                notify_if_unavailable: true,
            },
        ],
        trophy_preferences: TrophyPreferences {
            enabled: false,
            auto_equip: true,
            restore_after_craft: true,
            trophy_items: vec!["Blacksmithing Trophy".into()],
        },
        auto_claim: AutoClaimPreferences {
            enabled: false,
            claim_membership_grants: true,
            claim_task_windows: true,
            once_per_session: true,
        },
    }
}
