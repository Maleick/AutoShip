//! Vendor/sell cycle — bounded vendor visits with inventory routing and
//! recovery.
//!
//! The controller stays pure and offline-testable:
//! - outer FSM: `Idle` → `Navigating` → `Selling` → `Returning`
//! - inner FSM: `VendorStep` tracks the merchant-window interaction
//! - vendorable-item selection lives in `loot::vendor_cycle`
//! - recovery is observation-driven via navigation status and vendor busy flags

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use textquest_common::nav::NavStatus;

use crate::{
    loot::vendor_cycle::{VendorCyclePlanner, VendorInventoryItem, VendorPlan, VendorReason},
    metrics::events::VendorPlatMetrics,
};

fn normalize_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn vendor_listing_key(
    vendor_name: &str,
    item_name: &str,
    price_copper: Option<u64>,
    quantity: u32,
) -> String {
    format!(
        "{}|{}|{}|{}",
        normalize_name(vendor_name),
        normalize_name(item_name),
        price_copper.map_or_else(|| String::from("unknown"), |price| price.to_string()),
        quantity
    )
}

fn stop_movement_commands(seller_pid: u32) -> Vec<(u32, String)> {
    vec![
        (seller_pid, "/keypress forward".into()),
        (seller_pid, "/nav stop".into()),
    ]
}

fn actionable_vendor_nav_status(nav_status: Option<&NavStatus>) -> Option<&NavStatus> {
    nav_status.filter(|status| {
        matches!(
            status,
            NavStatus::Moving { .. }
                | NavStatus::Paused { .. }
                | NavStatus::Stuck { .. }
                | NavStatus::Arrived
        )
    })
}

/// One watched item entry for vendor browsing alerts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorWatchEntry {
    /// Item name to look for on merchant stock.
    pub item_name: String,
    /// Maximum acceptable vendor price in copper, when known.
    pub max_price_copper: Option<u64>,
}

/// One visible item on the currently opened vendor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorStockItem {
    /// Merchant item name.
    pub item_name: String,
    /// Observed vendor price in copper, when known.
    pub price_copper: Option<u64>,
    /// Quantity or stack size visible on the vendor.
    pub quantity: u32,
}

/// Alert produced when a watched item appears on a vendor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorWatchAlert {
    /// Vendor NPC display name.
    pub vendor_name: String,
    /// Matched item name.
    pub item_name: String,
    /// Configured maximum acceptable vendor price in copper.
    pub expected_max_price_copper: Option<u64>,
    /// Observed vendor price in copper, when available.
    pub actual_price_copper: Option<u64>,
    /// `actual - expected` in copper when both are available.
    pub price_delta_copper: Option<i64>,
    /// Whether the current sighting is within the configured price cap.
    pub within_budget: Option<bool>,
    /// Quantity or stack size visible on the vendor.
    pub quantity: u32,
    /// Tick when the alert was observed.
    pub observed_tick: u64,
}

/// Configuration for the vendor sell cycle.
#[derive(Debug, Clone)]
pub struct VendorConfig {
    /// Name of the vendor NPC to target.
    pub vendor_name: String,
    /// Ticks between sell runs.
    pub sell_interval_ticks: u64,
    /// Items to never sell (quest items, gear, etc.).
    pub keep_items: Vec<String>,
    /// Ticks to wait in `TravelingToVendor` / Returning.
    pub travel_ticks: u64,
    /// Items to sell. If empty, sells everything not in `keep_items`.
    pub sellable_items: Vec<String>,
    /// Ticks to wait between each sell-item command pair (prevents UI race).
    pub sell_step_delay: u64,
    /// Optional gate/origin spell gem for return travel (e.g., "gate" or "5"
    /// for gem 5).
    pub return_spell: Option<String>,
    /// Maximum ticks we allow navigation to run before recovery.
    pub navigation_timeout_ticks: u64,
    /// Delay between vendor-busy retries.
    pub vendor_retry_ticks: u64,
    /// Maximum vendor-busy retries before aborting the sell run.
    pub max_busy_retries: u32,
    /// Items older than this many days are vendored automatically.
    pub backlog_days: i64,
    /// Items to watch for when browsing vendor inventory.
    pub watch_items: Vec<VendorWatchEntry>,
}

/// Sub-steps within the Selling state that drive vendor UI interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VendorStep {
    /// Target the vendor NPC.
    Targeting,
    /// Walk into interaction range and face vendor.
    Approaching,
    /// Right-click vendor to open the merchant window.
    OpeningWindow,
    /// Sell items one at a time. `index` tracks progress through sellable
    /// inventory.
    SellingItems {
        /// Index of the item currently being sold.
        index: usize,
    },
    /// Close the merchant window.
    ClosingWindow,
}

/// Current state of the sell cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SellState {
    /// No sell run needed right now.
    Idle,
    /// En route to the vendor NPC.
    Navigating,
    /// At the vendor, working through the vendor UI sub-FSM.
    Selling {
        /// Current sub-step of the vendor interaction.
        step: VendorStep,
    },
    /// Returning to camp after selling.
    Returning,
}

impl SellState {
    #[allow(non_upper_case_globals)]
    pub const NotNeeded: Self = Self::Idle;
    #[allow(non_upper_case_globals)]
    pub const TravelingToVendor: Self = Self::Navigating;
}

/// Runtime observations that influence vendor-cycle transitions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VendorObservation {
    /// Latest navigation state for the seller, when available.
    pub nav_status: Option<NavStatus>,
    /// Whether the vendor window is currently busy and cannot accept clicks.
    pub vendor_busy: bool,
}

impl VendorObservation {
    #[must_use]
    pub fn arrived() -> Self {
        Self {
            nav_status: Some(NavStatus::Arrived),
            vendor_busy: false,
        }
    }
}

/// Why the controller had to recover instead of finishing a clean sell cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendorFailure {
    NavigationTimeout,
    NavigationStuck,
    VendorBusyTimeout,
}

/// Tracks the sell cycle for one camp group.
pub struct SellCycle {
    /// Vendor configuration.
    pub config: VendorConfig,
    /// Current sell cycle state.
    pub state: SellState,
    /// Tick when the last sell run completed.
    pub last_sell_tick: u64,
    /// Tick when the current state was entered (for delay timing).
    pub state_entered_tick: u64,
    keep_set: HashSet<String>,
    sellable_set: HashSet<String>,
    /// Items queued for selling in the current cycle.
    pub sell_queue: Vec<String>,
    active_vendor_watch_listings: HashSet<String>,
    sale_queue: Vec<crate::loot::vendor_cycle::VendorSaleItem>,
    planner: VendorCyclePlanner,
    session_metrics: VendorPlatMetrics,
    navigation_command_issued: bool,
    return_command_issued: bool,
    vendor_busy_retries: u32,
    /// Last recovery reason, if the previous cycle had to bail out.
    pub last_failure: Option<VendorFailure>,
}

impl SellCycle {
    /// Creates a new sell cycle with the given vendor configuration.
    #[must_use]
    pub fn new(config: VendorConfig) -> Self {
        let keep_set: HashSet<String> = config
            .keep_items
            .iter()
            .map(|item| normalize_name(item))
            .collect();
        let sellable_set: HashSet<String> = config
            .sellable_items
            .iter()
            .map(|item| normalize_name(item))
            .collect();
        let planner = VendorCyclePlanner::new(config.keep_items.clone())
            .with_allowlist(config.sellable_items.clone())
            .with_backlog_days(config.backlog_days);
        Self {
            config,
            state: SellState::Idle,
            last_sell_tick: 0,
            state_entered_tick: 0,
            keep_set,
            sellable_set,
            sell_queue: Vec::new(),
            active_vendor_watch_listings: HashSet::new(),
            sale_queue: Vec::new(),
            planner,
            session_metrics: VendorPlatMetrics::default(),
            navigation_command_issued: false,
            return_command_issued: false,
            vendor_busy_retries: 0,
            last_failure: None,
        }
    }

    /// Check if it's time to sell. Call from camp loop during Idle/Medding.
    #[must_use]
    pub fn needs_sell(&self, current_tick: u64) -> bool {
        self.state == SellState::Idle
            && current_tick.saturating_sub(self.last_sell_tick) >= self.config.sell_interval_ticks
    }

    /// Returns true if the item should be kept (not sold).
    #[must_use]
    pub fn should_keep(&self, item_name: &str) -> bool {
        self.keep_set.contains(&normalize_name(item_name))
    }

    /// Set the list of items to sell this cycle. Call before `start_sell`.
    /// Filters out keep-list items, and applies `sellable_items` as an
    /// allowlist when non-empty.
    pub fn queue_sell_items(&mut self, inventory: &[String]) {
        let use_allowlist = !self.sellable_set.is_empty();
        self.sale_queue = inventory
            .iter()
            .filter(|item| !self.should_keep(item))
            .filter(|item| !use_allowlist || self.sellable_set.contains(&normalize_name(item)))
            .map(|item| crate::loot::vendor_cycle::VendorSaleItem {
                item_id: None,
                item_name: item.clone(),
                stack_count: 1,
                estimated_vendor_value: 0,
                reason: VendorReason::Manual,
            })
            .collect();
        self.sell_queue = self
            .sale_queue
            .iter()
            .map(|item| item.item_name.clone())
            .collect();
    }

    /// Build a sell plan from inventory and store it as the pending queue.
    pub fn prepare_sell_plan(
        &mut self,
        inventory: &[VendorInventoryItem],
        now: DateTime<Utc>,
    ) -> VendorPlan {
        let plan = self.planner.plan(inventory, now);
        self.sale_queue = plan.items.clone();
        self.sell_queue = self
            .sale_queue
            .iter()
            .map(|item| item.item_name.clone())
            .collect();
        plan
    }

    /// Access session-level vendor plat accounting.
    #[must_use]
    pub fn session_metrics(&self) -> &VendorPlatMetrics {
        &self.session_metrics
    }

    /// Scan visible vendor stock for watched items and emit alerts for new
    /// sightings within the current browse session.
    #[must_use]
    pub fn scan_vendor_stock(
        &mut self,
        vendor_name: &str,
        stock: &[VendorStockItem],
        current_tick: u64,
    ) -> Vec<VendorWatchAlert> {
        let mut next_visible = HashSet::new();
        let mut alerts = Vec::new();

        for item in stock {
            let Some(watch) = self
                .config
                .watch_items
                .iter()
                .find(|entry| entry.item_name.eq_ignore_ascii_case(&item.item_name))
            else {
                continue;
            };

            let listing_key = vendor_listing_key(
                vendor_name,
                &item.item_name,
                item.price_copper,
                item.quantity,
            );
            next_visible.insert(listing_key.clone());

            if self.active_vendor_watch_listings.contains(&listing_key) {
                continue;
            }

            let price_delta_copper = match (item.price_copper, watch.max_price_copper) {
                (Some(actual), Some(expected)) => Some(actual as i64 - expected as i64),
                _ => None,
            };
            let within_budget = match (item.price_copper, watch.max_price_copper) {
                (Some(actual), Some(expected)) => Some(actual <= expected),
                _ => None,
            };

            alerts.push(VendorWatchAlert {
                vendor_name: vendor_name.to_string(),
                item_name: item.item_name.clone(),
                expected_max_price_copper: watch.max_price_copper,
                actual_price_copper: item.price_copper,
                price_delta_copper,
                within_budget,
                quantity: item.quantity,
                observed_tick: current_tick,
            });
        }

        self.active_vendor_watch_listings = next_visible;
        alerts
    }

    /// Clear the dedupe cache for the current vendor browse session so the next
    /// interaction can re-alert on matching items.
    pub fn reset_vendor_watch_session(&mut self) {
        self.active_vendor_watch_listings.clear();
    }

    /// Scan visible vendor stock for watched items and emit alerts for new
    /// sightings within the current browse session.
    #[must_use]
    pub fn scan_vendor_stock(
        &mut self,
        vendor_name: &str,
        stock: &[VendorStockItem],
        current_tick: u64,
    ) -> Vec<VendorWatchAlert> {
        let mut next_visible = HashSet::new();
        let mut alerts = Vec::new();

        for item in stock {
            let Some(watch) = self
                .config
                .watch_items
                .iter()
                .find(|entry| entry.item_name.eq_ignore_ascii_case(&item.item_name))
            else {
                continue;
            };

            let listing_key = vendor_listing_key(
                vendor_name,
                &item.item_name,
                item.price_copper,
                item.quantity,
            );
            next_visible.insert(listing_key.clone());

            if self.active_vendor_watch_listings.contains(&listing_key) {
                continue;
            }

            let price_delta_copper = match (item.price_copper, watch.max_price_copper) {
                (Some(actual), Some(expected)) => Some(actual as i64 - expected as i64),
                _ => None,
            };
            let within_budget = match (item.price_copper, watch.max_price_copper) {
                (Some(actual), Some(expected)) => Some(actual <= expected),
                _ => None,
            };

            alerts.push(VendorWatchAlert {
                vendor_name: vendor_name.to_string(),
                item_name: item.item_name.clone(),
                expected_max_price_copper: watch.max_price_copper,
                actual_price_copper: item.price_copper,
                price_delta_copper,
                within_budget,
                quantity: item.quantity,
                observed_tick: current_tick,
            });
        }

        self.active_vendor_watch_listings = next_visible;
        alerts
    }

    /// Clear the dedupe cache for the current vendor browse session so the next
    /// interaction can re-alert on matching items.
    pub fn reset_vendor_watch_session(&mut self) {
        self.active_vendor_watch_listings.clear();
    }

    /// Begin the sell cycle.
    pub fn start_sell(&mut self, current_tick: u64) {
        if self.state == SellState::Idle {
            self.last_failure = None;
            self.navigation_command_issued = false;
            self.return_command_issued = false;
            self.vendor_busy_retries = 0;
            self.state = SellState::Navigating;
            self.state_entered_tick = current_tick;
        }
    }

    /// Advance the sell state machine by one tick. Returns `(pid, command)`
    /// pairs for the designated seller.
    pub fn tick(&mut self, seller_pid: u32, current_tick: u64) -> Vec<(u32, String)> {
        self.tick_with_observation(seller_pid, current_tick, &VendorObservation::default())
    }

    /// Advance the sell state machine using the latest runtime observation.
    pub fn tick_with_observation(
        &mut self,
        seller_pid: u32,
        current_tick: u64,
        observation: &VendorObservation,
    ) -> Vec<(u32, String)> {
        match self.state.clone() {
            SellState::Idle => Vec::new(),
            SellState::Navigating => self.tick_navigation(seller_pid, current_tick, observation),
            SellState::Selling { .. } => {
                self.tick_vendor_step(seller_pid, current_tick, observation)
            }
            SellState::Returning => self.tick_returning(seller_pid, current_tick),
        }
    }

    fn tick_navigation(
        &mut self,
        seller_pid: u32,
        current_tick: u64,
        observation: &VendorObservation,
    ) -> Vec<(u32, String)> {
        let elapsed = current_tick.saturating_sub(self.state_entered_tick);
        let nav_status = actionable_vendor_nav_status(observation.nav_status.as_ref());

        if !self.navigation_command_issued {
            self.navigation_command_issued = true;
            return vec![
                (seller_pid, format!("/target {}", self.config.vendor_name)),
                (seller_pid, "/nav target".into()),
            ];
        }

        if nav_status.is_some_and(|status| status.is_stuck()) {
            self.last_failure = Some(VendorFailure::NavigationStuck);
            self.begin_returning(current_tick);
            return stop_movement_commands(seller_pid);
        }

        if self.navigation_command_issued && elapsed >= self.config.navigation_timeout_ticks {
            self.last_failure = Some(VendorFailure::NavigationTimeout);
            self.begin_returning(current_tick);
            return stop_movement_commands(seller_pid);
        }

        if nav_status.is_some_and(|status| status.is_arrived()) {
            let cmds = vec![
                (seller_pid, format!("/target {}", self.config.vendor_name)),
                (seller_pid, "/face fast".into()),
            ];
            self.state = SellState::Selling {
                step: VendorStep::Targeting,
            };
            self.state_entered_tick = current_tick;
            self.vendor_busy_retries = 0;
            return cmds;
        }

        if nav_status.is_none() && elapsed >= self.config.travel_ticks {
            let cmds = vec![
                (seller_pid, format!("/target {}", self.config.vendor_name)),
                (seller_pid, "/face fast".into()),
            ];
            self.state = SellState::Selling {
                step: VendorStep::Targeting,
            };
            self.state_entered_tick = current_tick;
            self.vendor_busy_retries = 0;
            return cmds;
        }

        Vec::new()
    }

    fn tick_returning(&mut self, seller_pid: u32, current_tick: u64) -> Vec<(u32, String)> {
        if !self.return_command_issued {
            self.return_command_issued = true;
            if let Some(spell) = &self.config.return_spell {
                self.state_entered_tick = current_tick;
                return vec![(seller_pid, format!("/cast {spell}"))];
            }
        }

        if current_tick.saturating_sub(self.state_entered_tick) < self.config.travel_ticks {
            return Vec::new();
        }
        let cmds = vec![(seller_pid, "/stand".into())];
        self.state = SellState::Idle;
        self.last_sell_tick = current_tick;
        self.return_command_issued = false;
        cmds
    }

    /// Drive the vendor interaction sub-FSM within the Selling state.
    fn tick_vendor_step(
        &mut self,
        seller_pid: u32,
        current_tick: u64,
        observation: &VendorObservation,
    ) -> Vec<(u32, String)> {
        let step = match &self.state {
            SellState::Selling { step } => step.clone(),
            _ => return Vec::new(),
        };

        match step {
            VendorStep::Targeting => {
                if current_tick.saturating_sub(self.state_entered_tick)
                    < self.config.sell_step_delay
                {
                    return Vec::new();
                }
                let cmds = vec![
                    (seller_pid, format!("/target {}", self.config.vendor_name)),
                    (seller_pid, "/face fast".into()),
                ];
                self.state = SellState::Selling {
                    step: VendorStep::Approaching,
                };
                self.state_entered_tick = current_tick;
                cmds
            }

            VendorStep::Approaching => {
                if current_tick.saturating_sub(self.state_entered_tick)
                    < self.config.sell_step_delay
                {
                    return Vec::new();
                }
                let cmds = vec![
                    (seller_pid, format!("/target {}", self.config.vendor_name)),
                    (seller_pid, "/face fast".into()),
                    (seller_pid, "/keypress forward hold".into()),
                    (seller_pid, "/nav target".into()),
                ];
                self.state = SellState::Selling {
                    step: VendorStep::OpeningWindow,
                };
                self.state_entered_tick = current_tick;
                cmds
            }

            VendorStep::OpeningWindow => {
                let required_delay = if self.vendor_busy_retries == 0 {
                    self.config.sell_step_delay
                } else {
                    self.config.vendor_retry_ticks
                };
                if current_tick.saturating_sub(self.state_entered_tick) < required_delay {
                    return Vec::new();
                }
                if observation.vendor_busy {
                    self.vendor_busy_retries += 1;
                    self.state_entered_tick = current_tick;
                    if self.vendor_busy_retries >= self.config.max_busy_retries {
                        self.last_failure = Some(VendorFailure::VendorBusyTimeout);
                        self.begin_returning(current_tick);
                        return stop_movement_commands(seller_pid);
                    }
                    return Vec::new();
                }
                let mut cmds = stop_movement_commands(seller_pid);
                cmds.push((seller_pid, "/click right target".into()));
                self.state = SellState::Selling {
                    step: VendorStep::SellingItems { index: 0 },
                };
                self.state_entered_tick = current_tick;
                self.vendor_busy_retries = 0;
                cmds
            }

            VendorStep::SellingItems { index } => {
                if current_tick.saturating_sub(self.state_entered_tick)
                    < self.config.sell_step_delay
                {
                    return Vec::new();
                }
                if index >= self.sale_queue.len() {
                    // All items sold — close window
                    self.state = SellState::Selling {
                        step: VendorStep::ClosingWindow,
                    };
                    self.state_entered_tick = current_tick;
                    return Vec::new();
                }

                let sale = &self.sale_queue[index];
                let item_name = &sale.item_name;
                // EQ vendor sell: /itemnotify <item> leftmouseup to pick up,
                // then /notify MerchantWnd MW_Sell_Button leftmouseup to sell
                let cmds = vec![
                    (
                        seller_pid,
                        format!("/nomodkey /itemnotify \"{item_name}\" leftmouseup"),
                    ),
                    (
                        seller_pid,
                        "/notify MerchantWnd MW_Sell_Button leftmouseup".into(),
                    ),
                ];
                self.session_metrics
                    .record_sale(sale.estimated_vendor_value);

                self.state = SellState::Selling {
                    step: VendorStep::SellingItems { index: index + 1 },
                };
                self.state_entered_tick = current_tick;
                cmds
            }

            VendorStep::ClosingWindow => {
                if current_tick.saturating_sub(self.state_entered_tick)
                    < self.config.sell_step_delay
                {
                    return Vec::new();
                }
                let cmds = vec![(
                    seller_pid,
                    "/notify MerchantWnd MW_Done_Button leftmouseup".into(),
                )];
                self.sell_queue.clear();
                self.reset_vendor_watch_session();
                self.sale_queue.clear();
                self.begin_returning(current_tick);
                cmds
            }
        }
    }

    fn begin_returning(&mut self, current_tick: u64) {
        self.state = SellState::Returning;
        self.state_entered_tick = current_tick;
        self.navigation_command_issued = false;
        self.return_command_issued = false;
        self.vendor_busy_retries = 0;
        self.sell_queue.clear();
        self.sale_queue.clear();
        self.reset_vendor_watch_session();
    }
}

/// Generate slash commands for the current sell state (standalone helper).
#[must_use]
pub fn sell_commands(state: &SellState, vendor_name: &str, seller_pid: u32) -> Vec<(u32, String)> {
    match state {
        SellState::Idle => Vec::new(),
        SellState::Navigating => {
            vec![
                (seller_pid, format!("/target {vendor_name}")),
                (seller_pid, "/nav target".into()),
            ]
        }
        SellState::Selling { .. } => {
            vec![
                (seller_pid, format!("/target {vendor_name}")),
                (seller_pid, "/click right target".into()),
            ]
        }
        SellState::Returning => {
            vec![(seller_pid, "/stand".into())]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn test_vendor_config() -> VendorConfig {
        VendorConfig {
            vendor_name: "Merchant_Leah".into(),
            sell_interval_ticks: 50,
            keep_items: vec!["Fine Steel Dagger".into(), "Bone Chips".into()],
            travel_ticks: 5,
            sellable_items: vec![],
            sell_step_delay: 2,
            return_spell: None,
            navigation_timeout_ticks: 20,
            vendor_retry_ticks: 2,
            max_busy_retries: 3,
            backlog_days: 30,
            watch_items: vec![],
        }
    }

    #[test]
    fn test_starts_not_needed() {
        let cycle = SellCycle::new(test_vendor_config());
        assert_eq!(cycle.state, SellState::NotNeeded);
        assert_eq!(cycle.last_sell_tick, 0);
    }

    #[test]
    fn test_needs_sell_timing() {
        let cycle = SellCycle::new(test_vendor_config());
        assert!(!cycle.needs_sell(30));
        assert!(cycle.needs_sell(50));
        assert!(cycle.needs_sell(100));
    }

    #[test]
    fn test_needs_sell_false_when_already_selling() {
        let mut cycle = SellCycle::new(test_vendor_config());
        cycle.state = SellState::Selling {
            step: VendorStep::Targeting,
        };
        assert!(!cycle.needs_sell(100));
    }

    #[test]
    fn test_should_keep() {
        let cycle = SellCycle::new(test_vendor_config());
        assert!(cycle.should_keep("Fine Steel Dagger"));
        assert!(cycle.should_keep("Bone Chips"));
        assert!(!cycle.should_keep("Cracked Staff"));
    }

    #[test]
    fn test_should_keep_normalizes_configured_names() {
        let cycle = SellCycle::new(test_vendor_config());
        assert!(cycle.should_keep(" fine steel dagger "));
        assert!(cycle.should_keep("bone chips"));
    }

    #[test]
    fn test_queue_sell_items_filters_keep() {
        let mut cycle = SellCycle::new(test_vendor_config());
        cycle.queue_sell_items(&[
            "Cracked Staff".into(),
            "Fine Steel Dagger".into(),
            "Rusty Axe".into(),
            "Bone Chips".into(),
        ]);
        assert_eq!(cycle.sell_queue, vec!["Cracked Staff", "Rusty Axe"]);
    }

    #[test]
    fn test_queue_sell_items_honors_sellable_allowlist() {
        let mut config = test_vendor_config();
        config.sellable_items = vec!["Rusty Axe".into()];
        let mut cycle = SellCycle::new(config);

        cycle.queue_sell_items(&[
            "Cracked Staff".into(),
            "Fine Steel Dagger".into(),
            "Rusty Axe".into(),
            "Bone Chips".into(),
        ]);

        assert_eq!(cycle.sell_queue, vec!["Rusty Axe"]);
    }

    #[test]
    fn test_queue_sell_items_normalizes_sellable_allowlist() {
        let mut config = test_vendor_config();
        config.sellable_items = vec![" rusty axe ".into()];
        let mut cycle = SellCycle::new(config);

        cycle.queue_sell_items(&["Rusty Axe".into(), "Cracked Staff".into()]);

        assert_eq!(cycle.sell_queue, vec!["Rusty Axe"]);
    }

    #[test]
    fn test_full_sell_cycle_with_vendor_steps() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let pid = 104;

        // Queue some items to sell
        cycle.queue_sell_items(&["Cracked Staff".into(), "Rusty Axe".into()]);

        cycle.start_sell(50);
        assert_eq!(cycle.state, SellState::TravelingToVendor);

        // First navigation tick issues movement commands.
        let cmds = cycle.tick(pid, 50);
        assert_eq!(cycle.state, SellState::TravelingToVendor);
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("/nav target")));

        // During travel time — no transition yet
        for t in 51..55 {
            let cmds = cycle.tick(pid, t);
            assert_eq!(cycle.state, SellState::TravelingToVendor);
            assert!(cmds.is_empty());
        }

        // After travel time: traveling -> selling (Targeting step)
        let cmds = cycle.tick(pid, 55);
        assert!(matches!(
            cycle.state,
            SellState::Selling {
                step: VendorStep::Targeting
            }
        ));
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("Merchant_Leah")));

        // Targeting -> Approaching (after sell_step_delay)
        let cmds = cycle.tick(pid, 57);
        assert!(matches!(
            cycle.state,
            SellState::Selling {
                step: VendorStep::Approaching
            }
        ));
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("target")));

        // Approaching -> OpeningWindow
        let cmds = cycle.tick(pid, 59);
        assert!(matches!(
            cycle.state,
            SellState::Selling {
                step: VendorStep::OpeningWindow
            }
        ));
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("forward")));

        // OpeningWindow -> SellingItems { index: 0 }
        let cmds = cycle.tick(pid, 61);
        assert!(matches!(
            cycle.state,
            SellState::Selling {
                step: VendorStep::SellingItems { index: 0 }
            }
        ));
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("right target")));

        // Sell item 0 (Cracked Staff)
        let cmds = cycle.tick(pid, 63);
        assert!(matches!(
            cycle.state,
            SellState::Selling {
                step: VendorStep::SellingItems { index: 1 }
            }
        ));
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("Cracked Staff")));
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("MW_Sell_Button")));

        // Sell item 1 (Rusty Axe)
        let cmds = cycle.tick(pid, 65);
        assert!(matches!(
            cycle.state,
            SellState::Selling {
                step: VendorStep::SellingItems { index: 2 }
            }
        ));
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("Rusty Axe")));

        // No more items -> ClosingWindow (immediate, no commands)
        let cmds = cycle.tick(pid, 67);
        assert!(matches!(
            cycle.state,
            SellState::Selling {
                step: VendorStep::ClosingWindow
            }
        ));
        assert!(cmds.is_empty());

        // ClosingWindow -> Returning
        let cmds = cycle.tick(pid, 69);
        assert_eq!(cycle.state, SellState::Returning);
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("MW_Done_Button")));

        // Wait for return travel
        for t in 69..74 {
            let cmds = cycle.tick(pid, t);
            assert_eq!(cycle.state, SellState::Returning);
            assert!(cmds.is_empty());
        }

        // After return travel: returning -> not needed
        let cmds = cycle.tick(pid, 74);
        assert_eq!(cycle.state, SellState::NotNeeded);
        assert_eq!(cycle.last_sell_tick, 74);
        assert!(!cmds.is_empty());
    }

    #[test]
    fn test_sell_cycle_empty_queue() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let pid = 104;

        // Don't queue any items
        cycle.start_sell(50);

        // First tick enters navigation.
        let _ = cycle.tick(pid, 50);
        // Travel
        let _ = cycle.tick(pid, 55);
        assert!(matches!(cycle.state, SellState::Selling { .. }));

        // Targeting -> Approaching
        let _ = cycle.tick(pid, 57);
        // Approaching -> OpeningWindow
        let _ = cycle.tick(pid, 59);
        // OpeningWindow -> SellingItems { 0 }
        let _ = cycle.tick(pid, 61);
        // SellingItems with empty queue -> ClosingWindow immediately
        let _ = cycle.tick(pid, 63);
        assert!(matches!(
            cycle.state,
            SellState::Selling {
                step: VendorStep::ClosingWindow
            }
        ));
    }

    #[test]
    fn test_start_sell_idempotent() {
        let mut cycle = SellCycle::new(test_vendor_config());
        cycle.state = SellState::Selling {
            step: VendorStep::Targeting,
        };
        cycle.start_sell(100);
        assert!(matches!(cycle.state, SellState::Selling { .. }));
    }

    #[test]
    fn test_sell_commands_not_needed() {
        let cmds = sell_commands(&SellState::NotNeeded, "Vendor", 100);
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_sell_commands_traveling() {
        let cmds = sell_commands(&SellState::TravelingToVendor, "Merchant_Leah", 104);
        assert_eq!(cmds.len(), 2);
        assert!(cmds[0].1.contains("Merchant_Leah"));
        assert_eq!(cmds[1].1, "/nav target");
    }

    #[test]
    fn test_sell_commands_selling() {
        let cmds = sell_commands(
            &SellState::Selling {
                step: VendorStep::Targeting,
            },
            "Merchant_Leah",
            104,
        );
        assert!(cmds.len() >= 2);
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("right target")));
    }

    #[test]
    fn test_sell_commands_returning() {
        let cmds = sell_commands(&SellState::Returning, "Merchant_Leah", 104);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].1, "/stand");
    }

    #[test]
    fn test_travel_delay_blocks_immediate_transition() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let pid = 104;

        cycle.start_sell(100);
        assert_eq!(cycle.state, SellState::TravelingToVendor);

        // First tick issues navigation commands without transitioning.
        let cmds = cycle.tick(pid, 100);
        assert_eq!(cycle.state, SellState::TravelingToVendor);
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("/nav target")));

        // Tick at travel_ticks - 1 should NOT transition
        let cmds = cycle.tick(pid, 104);
        assert_eq!(cycle.state, SellState::TravelingToVendor);
        assert!(cmds.is_empty());

        // Tick at exactly travel_ticks should transition
        let cmds = cycle.tick(pid, 105);
        assert!(matches!(cycle.state, SellState::Selling { .. }));
        assert!(!cmds.is_empty());
    }

    #[test]
    fn test_navigation_waits_for_arrival_when_status_is_available() {
        let mut cycle = SellCycle::new(test_vendor_config());
        cycle.start_sell(100);

        let _ = cycle.tick_with_observation(
            104,
            100,
            &VendorObservation {
                nav_status: Some(textquest_common::nav::NavStatus::Moving {
                    waypoint_index: 0,
                    waypoint_count: 2,
                    distance_remaining: 15.0,
                }),
                ..VendorObservation::default()
            },
        );

        let cmds = cycle.tick_with_observation(
            104,
            105,
            &VendorObservation {
                nav_status: Some(textquest_common::nav::NavStatus::Moving {
                    waypoint_index: 1,
                    waypoint_count: 2,
                    distance_remaining: 5.0,
                }),
                ..VendorObservation::default()
            },
        );

        assert_eq!(cycle.state, SellState::Navigating);
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_navigation_ignores_stale_arrived_before_vendor_nav_is_issued() {
        let mut cycle = SellCycle::new(test_vendor_config());
        cycle.start_sell(100);

        let cmds = cycle.tick_with_observation(104, 100, &VendorObservation::arrived());

        assert_eq!(cycle.state, SellState::Navigating);
        assert!(cycle.navigation_command_issued);
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("/nav target")));
        assert!(!cmds.iter().any(|(_, cmd)| cmd == "/face fast"));
    }

    #[test]
    fn test_idle_nav_status_uses_travel_fallback_after_command_is_issued() {
        let mut cycle = SellCycle::new(test_vendor_config());
        cycle.start_sell(100);

        let _ = cycle.tick_with_observation(
            104,
            100,
            &VendorObservation {
                nav_status: Some(textquest_common::nav::NavStatus::Idle),
                ..VendorObservation::default()
            },
        );

        let cmds = cycle.tick_with_observation(
            104,
            105,
            &VendorObservation {
                nav_status: Some(textquest_common::nav::NavStatus::Idle),
                ..VendorObservation::default()
            },
        );

        assert!(matches!(cycle.state, SellState::Selling { .. }));
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/face fast"));
    }

    #[test]
    fn test_return_delay_blocks_immediate_completion() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let pid = 104;

        cycle.start_sell(100);
        let _ = cycle.tick(pid, 100); // issue navigation
        // Skip past travel
        let _ = cycle.tick(pid, 105);
        assert!(matches!(cycle.state, SellState::Selling { .. }));

        // Run through all vendor steps quickly (sell_step_delay = 2)
        let mut tick = 107;
        while matches!(cycle.state, SellState::Selling { .. }) {
            let _ = cycle.tick(pid, tick);
            tick += 2;
        }
        assert_eq!(cycle.state, SellState::Returning);
        let return_entered = cycle.state_entered_tick;

        // Return travel should take travel_ticks
        let cmds = cycle.tick(pid, return_entered + 2);
        assert_eq!(cycle.state, SellState::Returning);
        assert!(cmds.is_empty());

        let cmds = cycle.tick(pid, return_entered + 5);
        assert_eq!(cycle.state, SellState::NotNeeded);
        assert!(!cmds.is_empty());
    }

    #[test]
    fn test_return_spell_casts_during_returning() {
        let mut config = test_vendor_config();
        config.return_spell = Some("gate".into());
        let mut cycle = SellCycle::new(config);
        cycle.state = SellState::Returning;
        cycle.state_entered_tick = 100;

        let cmds = cycle.tick(104, 101);
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/cast gate"));
        assert_eq!(cycle.state, SellState::Returning);
    }

    #[test]
    fn test_vendor_busy_timeout_after_max_retries() {
        let mut config = test_vendor_config();
        config.max_busy_retries = 2;
        let mut cycle = SellCycle::new(config);
        cycle.queue_sell_items(&["Cracked Staff".into()]);
        cycle.start_sell(0);

        let _ = cycle.tick(104, 0);
        let _ = cycle.tick(104, 5);
        let _ = cycle.tick(104, 7);
        let _ = cycle.tick(104, 9);
        let _ = cycle.tick_with_observation(
            104,
            11,
            &VendorObservation {
                vendor_busy: true,
                ..VendorObservation::default()
            },
        );

        let cmds = cycle.tick_with_observation(
            104,
            13,
            &VendorObservation {
                vendor_busy: true,
                ..VendorObservation::default()
            },
        );

        assert!(cmds.iter().any(|(_, cmd)| cmd == "/nav stop"));
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/keypress forward"));
        assert_eq!(cycle.last_failure, Some(VendorFailure::VendorBusyTimeout));
        assert_eq!(cycle.state, SellState::Returning);
    }

    #[test]
    fn test_recovery_return_clears_pending_sale_queues() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let now = chrono::Utc.with_ymd_and_hms(2026, 4, 17, 12, 0, 0).unwrap();
        cycle.prepare_sell_plan(
            &[crate::loot::vendor_cycle::VendorInventoryItem::trash(
                "Torn Cloth Sandal",
                1,
                7,
            )],
            now,
        );
        cycle.start_sell(0);

        let _ = cycle.tick_with_observation(104, 0, &VendorObservation::default());
        assert!(!cycle.sell_queue.is_empty());
        assert!(!cycle.sale_queue.is_empty());

        let _ = cycle.tick_with_observation(
            104,
            1,
            &VendorObservation {
                nav_status: Some(textquest_common::nav::NavStatus::Stuck {
                    recovery_attempt: 1,
                }),
                ..VendorObservation::default()
            },
        );

        assert_eq!(cycle.state, SellState::Returning);
        assert!(cycle.sell_queue.is_empty());
        assert!(cycle.sale_queue.is_empty());
    }

    #[test]
    fn test_sell_step_delay_enforced() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let pid = 104;
        cycle.queue_sell_items(&["Junk".into()]);

        cycle.start_sell(100);
        let _ = cycle.tick(pid, 100); // issue navigation
        let _ = cycle.tick(pid, 105); // Travel -> Selling(Targeting)

        // Tick immediately — delay not met, should return empty
        let cmds = cycle.tick(pid, 106);
        assert!(cmds.is_empty());

        // After delay (2 ticks), should advance
        let cmds = cycle.tick(pid, 107);
        assert!(!cmds.is_empty());
    }

    fn vendor_config_with_watch_items(watch_items: Vec<VendorWatchEntry>) -> VendorConfig {
        let mut config = test_vendor_config();
        config.watch_items = watch_items;
        config
    }

    #[test]
    fn test_vendor_watch_emits_alert_with_price_delta() {
        let mut cycle = SellCycle::new(vendor_config_with_watch_items(vec![VendorWatchEntry {
            item_name: "Flowing Thought Ring".into(),
            max_price_copper: Some(1500),
        }]));

        let alerts = cycle.scan_vendor_stock(
            "Merchant_Leah",
            &[VendorStockItem {
                item_name: "Flowing Thought Ring".into(),
                price_copper: Some(1200),
                quantity: 1,
            }],
            200,
        );

        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].vendor_name, "Merchant_Leah");
        assert_eq!(alerts[0].item_name, "Flowing Thought Ring");
        assert_eq!(alerts[0].expected_max_price_copper, Some(1500));
        assert_eq!(alerts[0].actual_price_copper, Some(1200));
        assert_eq!(alerts[0].price_delta_copper, Some(-300));
        assert_eq!(alerts[0].within_budget, Some(true));
        assert_eq!(alerts[0].observed_tick, 200);
    }

    #[test]
    fn test_vendor_watch_dedupes_unchanged_listing_until_inventory_changes() {
        let mut cycle = SellCycle::new(vendor_config_with_watch_items(vec![VendorWatchEntry {
            item_name: "Flowing Thought Ring".into(),
            max_price_copper: Some(1500),
        }]));

        let first = cycle.scan_vendor_stock(
            "Merchant_Leah",
            &[VendorStockItem {
                item_name: "Flowing Thought Ring".into(),
                price_copper: Some(1200),
                quantity: 1,
            }],
            200,
        );
        let duplicate = cycle.scan_vendor_stock(
            "Merchant_Leah",
            &[VendorStockItem {
                item_name: "Flowing Thought Ring".into(),
                price_copper: Some(1200),
                quantity: 1,
            }],
            201,
        );
        let changed = cycle.scan_vendor_stock(
            "Merchant_Leah",
            &[VendorStockItem {
                item_name: "Flowing Thought Ring".into(),
                price_copper: Some(1400),
                quantity: 1,
            }],
            202,
        );

        assert_eq!(first.len(), 1);
        assert!(duplicate.is_empty());
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].price_delta_copper, Some(-100));
    }

    #[test]
    fn test_vendor_watch_resets_after_browse_session_ends() {
        let mut cycle = SellCycle::new(vendor_config_with_watch_items(vec![VendorWatchEntry {
            item_name: "Journeyman's Boots".into(),
            max_price_copper: Some(5000),
        }]));

        let initial = cycle.scan_vendor_stock(
            "Merchant_Leah",
            &[VendorStockItem {
                item_name: "Journeyman's Boots".into(),
                price_copper: Some(4200),
                quantity: 1,
            }],
            300,
        );
        cycle.reset_vendor_watch_session();
        let repeated = cycle.scan_vendor_stock(
            "Merchant_Leah",
            &[VendorStockItem {
                item_name: "Journeyman's Boots".into(),
                price_copper: Some(4200),
                quantity: 1,
            }],
            301,
        );

        assert_eq!(initial.len(), 1);
        assert_eq!(repeated.len(), 1);
    }

    #[test]
    fn test_queue_sell_items_all_kept() {
        let mut cycle = SellCycle::new(test_vendor_config());
        cycle.queue_sell_items(&["Fine Steel Dagger".into(), "Bone Chips".into()]);
        assert!(cycle.sell_queue.is_empty());
    }

    #[test]
    fn test_queue_sell_items_empty_inventory() {
        let mut cycle = SellCycle::new(test_vendor_config());
        cycle.queue_sell_items(&[]);
        assert!(cycle.sell_queue.is_empty());
    }

    #[test]
    fn test_needs_sell_false_at_zero_tick() {
        let cycle = SellCycle::new(test_vendor_config());
        assert!(!cycle.needs_sell(0));
    }

    #[test]
    fn test_needs_sell_after_last_sell() {
        let mut cycle = SellCycle::new(test_vendor_config());
        cycle.last_sell_tick = 100;
        assert!(!cycle.needs_sell(120)); // 20 < 50
        assert!(cycle.needs_sell(150)); // 50 >= 50
        assert!(cycle.needs_sell(200)); // 100 >= 50
    }

    #[test]
    fn test_sell_state_debug_format() {
        let state = SellState::Selling {
            step: VendorStep::SellingItems { index: 3 },
        };
        let dbg = format!("{:?}", state);
        assert!(dbg.contains("SellingItems"));
        assert!(dbg.contains("3"));
    }

    #[test]
    fn test_vendor_step_equality() {
        assert_eq!(VendorStep::Targeting, VendorStep::Targeting);
        assert_ne!(VendorStep::Targeting, VendorStep::Approaching);
        assert_eq!(
            VendorStep::SellingItems { index: 0 },
            VendorStep::SellingItems { index: 0 }
        );
        assert_ne!(
            VendorStep::SellingItems { index: 0 },
            VendorStep::SellingItems { index: 1 }
        );
    }

    #[test]
    fn test_sell_commands_pid_passed_through() {
        let cmds = sell_commands(&SellState::Returning, "Vendor", 999);
        assert_eq!(cmds[0].0, 999);
    }

    #[test]
    fn test_not_needed_tick_is_noop() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let cmds = cycle.tick(104, 999);
        assert!(cmds.is_empty());
        assert_eq!(cycle.state, SellState::NotNeeded);
    }

    #[test]
    fn test_vendor_busy_retries_before_selling_items() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let pid = 104;
        cycle.queue_sell_items(&["Cracked Staff".into()]);
        cycle.start_sell(0);

        let _ = cycle.tick(pid, 0);
        let _ = cycle.tick(pid, 5);
        let _ = cycle.tick(pid, 7);
        let _ = cycle.tick(pid, 9);

        let busy = cycle.tick_with_observation(
            pid,
            11,
            &VendorObservation {
                vendor_busy: true,
                ..VendorObservation::default()
            },
        );
        assert!(busy.is_empty(), "busy vendor should not consume the queue");
        assert!(matches!(
            cycle.state,
            SellState::Selling {
                step: VendorStep::OpeningWindow
            }
        ));

        let retry = cycle.tick_with_observation(pid, 13, &VendorObservation::default());
        assert!(retry.iter().any(|(_, cmd)| cmd.contains("right target")));
    }

    #[test]
    fn test_navigation_stuck_triggers_recovery_return() {
        let mut cycle = SellCycle::new(test_vendor_config());
        cycle.start_sell(0);

        let cmds = cycle.tick_with_observation(
            104,
            1,
            &VendorObservation {
                nav_status: Some(textquest_common::nav::NavStatus::Stuck {
                    recovery_attempt: 2,
                }),
                ..VendorObservation::default()
            },
        );

        assert!(cmds.iter().any(|(_, cmd)| cmd == "/nav stop"));
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/keypress forward"));
        assert_eq!(cycle.last_failure, Some(VendorFailure::NavigationStuck));
        assert_eq!(cycle.state, SellState::Returning);
    }

    #[test]
    fn test_opening_window_releases_forward_before_clicking_vendor() {
        let mut cycle = SellCycle::new(test_vendor_config());
        cycle.state = SellState::Selling {
            step: VendorStep::OpeningWindow,
        };
        cycle.state_entered_tick = 0;

        let cmds = cycle.tick_with_observation(104, 2, &VendorObservation::default());

        assert!(cmds.iter().any(|(_, cmd)| cmd == "/keypress forward"));
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/nav stop"));
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/click right target"));
    }

    #[test]
    fn test_vendor_cycle_tracks_sale_metrics() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let now = chrono::Utc.with_ymd_and_hms(2026, 4, 17, 12, 0, 0).unwrap();
        cycle.prepare_sell_plan(
            &[crate::loot::vendor_cycle::VendorInventoryItem::trash(
                "Torn Cloth Sandal",
                1,
                7,
            )],
            now,
        );
        cycle.start_sell(0);

        let _ = cycle.tick_with_observation(104, 0, &VendorObservation::arrived());
        let _ = cycle.tick_with_observation(104, 2, &VendorObservation::default());
        let _ = cycle.tick_with_observation(104, 4, &VendorObservation::default());
        let _ = cycle.tick_with_observation(104, 6, &VendorObservation::default());
        let _ = cycle.tick_with_observation(104, 8, &VendorObservation::default());

        assert_eq!(cycle.session_metrics().gross_plat(), 7);
        assert_eq!(cycle.session_metrics().net_plat(), 7);
        assert_eq!(cycle.session_metrics().items_sold(), 1);
    }
}
