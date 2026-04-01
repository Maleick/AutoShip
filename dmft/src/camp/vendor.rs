//! Vendor/sell cycle — periodically sell loot to a nearby vendor.
//!
//! The sell cycle is a two-level FSM:
//! - Outer: `SellState` — NotNeeded → TravelingToVendor → Selling → Returning
//! - Inner: `VendorStep` — sub-states within `Selling` that drive the actual
//!   vendor UI interaction (target, approach, open window, sell items, close).

use std::collections::HashSet;

/// Configuration for the vendor sell cycle.
#[derive(Debug, Clone)]
pub struct VendorConfig {
    pub vendor_name: String,
    pub sell_interval_ticks: u64,
    pub keep_items: Vec<String>,
    /// Ticks to wait in TravelingToVendor / Returning.
    pub travel_ticks: u64,
    /// Items to sell. If empty, sells everything not in `keep_items`.
    pub sellable_items: Vec<String>,
    /// Ticks to wait between each sell-item command pair (prevents UI race).
    pub sell_step_delay: u64,
    /// Optional gate/origin spell gem for return travel (e.g., "gate" or "5" for gem 5).
    pub return_spell: Option<String>,
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
    /// Sell items one at a time. `index` tracks progress through sellable inventory.
    SellingItems { index: usize },
    /// Close the merchant window.
    ClosingWindow,
}

/// Current state of the sell cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SellState {
    NotNeeded,
    TravelingToVendor,
    Selling { step: VendorStep },
    Returning,
}

/// Tracks the sell cycle for one camp group.
pub struct SellCycle {
    pub config: VendorConfig,
    pub state: SellState,
    pub last_sell_tick: u64,
    pub state_entered_tick: u64,
    keep_set: HashSet<String>,
    /// Items queued for selling in the current cycle.
    sell_queue: Vec<String>,
}

impl SellCycle {
    pub fn new(config: VendorConfig) -> Self {
        let keep_set: HashSet<String> = config.keep_items.iter().cloned().collect();
        Self {
            config,
            state: SellState::NotNeeded,
            last_sell_tick: 0,
            state_entered_tick: 0,
            keep_set,
            sell_queue: Vec::new(),
        }
    }

    /// Check if it's time to sell. Call from camp loop during Idle/Medding.
    pub fn needs_sell(&self, current_tick: u64) -> bool {
        self.state == SellState::NotNeeded
            && current_tick.saturating_sub(self.last_sell_tick) >= self.config.sell_interval_ticks
    }

    /// Returns true if the item should be kept (not sold).
    pub fn should_keep(&self, item_name: &str) -> bool {
        self.keep_set.contains(item_name)
    }

    /// Set the list of items to sell this cycle. Call before `start_sell`.
    /// Filters out any items in the keep list.
    pub fn queue_sell_items(&mut self, inventory: &[String]) {
        self.sell_queue = inventory
            .iter()
            .filter(|item| !self.should_keep(item))
            .cloned()
            .collect();
    }

    /// Begin the sell cycle.
    pub fn start_sell(&mut self, current_tick: u64) {
        if self.state == SellState::NotNeeded {
            self.state = SellState::TravelingToVendor;
            self.state_entered_tick = current_tick;
        }
    }

    /// Advance the sell state machine by one tick. Returns `(pid, command)` pairs
    /// for the designated seller.
    pub fn tick(&mut self, seller_pid: u32, current_tick: u64) -> Vec<(u32, String)> {
        match &self.state {
            SellState::NotNeeded => Vec::new(),

            SellState::TravelingToVendor => {
                if current_tick.saturating_sub(self.state_entered_tick) < self.config.travel_ticks {
                    return Vec::new();
                }
                // If return_spell is set, cast it to travel to vendor bind point
                let mut cmds = Vec::new();
                if let Some(spell) = &self.config.return_spell {
                    cmds.push((seller_pid, format!("/cast {spell}")));
                }
                cmds.push((seller_pid, format!("/target {}", self.config.vendor_name)));
                cmds.push((seller_pid, "/face fast".into()));

                self.state = SellState::Selling {
                    step: VendorStep::Targeting,
                };
                self.state_entered_tick = current_tick;
                cmds
            }

            SellState::Selling { step: _ } => self.tick_vendor_step(seller_pid, current_tick),

            SellState::Returning => {
                if current_tick.saturating_sub(self.state_entered_tick) < self.config.travel_ticks {
                    return Vec::new();
                }
                let cmds = vec![(seller_pid, "/stand".into())];
                self.state = SellState::NotNeeded;
                self.last_sell_tick = current_tick;
                cmds
            }
        }
    }

    /// Drive the vendor interaction sub-FSM within the Selling state.
    fn tick_vendor_step(&mut self, seller_pid: u32, current_tick: u64) -> Vec<(u32, String)> {
        let step = match &self.state {
            SellState::Selling { step } => step.clone(),
            _ => return Vec::new(),
        };

        // Enforce delay between sub-steps
        if current_tick.saturating_sub(self.state_entered_tick) < self.config.sell_step_delay {
            return Vec::new();
        }

        match step {
            VendorStep::Targeting => {
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
                // Move toward vendor — /nav target or walk forward
                let cmds = vec![
                    (seller_pid, format!("/target {}", self.config.vendor_name)),
                    (seller_pid, "/face fast".into()),
                    (seller_pid, "/keypress forward hold".into()),
                ];
                self.state = SellState::Selling {
                    step: VendorStep::OpeningWindow,
                };
                self.state_entered_tick = current_tick;
                cmds
            }

            VendorStep::OpeningWindow => {
                // Stop moving, right-click vendor to open merchant window
                let cmds = vec![
                    (seller_pid, "/keypress forward".into()),
                    (seller_pid, "/click right target".into()),
                ];
                self.state = SellState::Selling {
                    step: VendorStep::SellingItems { index: 0 },
                };
                self.state_entered_tick = current_tick;
                cmds
            }

            VendorStep::SellingItems { index } => {
                if index >= self.sell_queue.len() {
                    // All items sold — close window
                    self.state = SellState::Selling {
                        step: VendorStep::ClosingWindow,
                    };
                    self.state_entered_tick = current_tick;
                    return Vec::new();
                }

                let item_name = &self.sell_queue[index];
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

                self.state = SellState::Selling {
                    step: VendorStep::SellingItems { index: index + 1 },
                };
                self.state_entered_tick = current_tick;
                cmds
            }

            VendorStep::ClosingWindow => {
                let cmds = vec![(
                    seller_pid,
                    "/notify MerchantWnd MW_Done_Button leftmouseup".into(),
                )];
                self.sell_queue.clear();
                self.state = SellState::Returning;
                self.state_entered_tick = current_tick;
                cmds
            }
        }
    }
}

/// Generate slash commands for the current sell state (standalone helper).
pub fn sell_commands(state: &SellState, vendor_name: &str, seller_pid: u32) -> Vec<(u32, String)> {
    match state {
        SellState::NotNeeded => Vec::new(),
        SellState::TravelingToVendor => {
            vec![
                (seller_pid, format!("/target {vendor_name}")),
                (seller_pid, "/face fast".into()),
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

    fn test_vendor_config() -> VendorConfig {
        VendorConfig {
            vendor_name: "Merchant_Leah".into(),
            sell_interval_ticks: 50,
            keep_items: vec!["Fine Steel Dagger".into(), "Bone Chips".into()],
            travel_ticks: 5,
            sellable_items: vec![],
            sell_step_delay: 2,
            return_spell: None,
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
    fn test_full_sell_cycle_with_vendor_steps() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let pid = 104;

        // Queue some items to sell
        cycle.queue_sell_items(&["Cracked Staff".into(), "Rusty Axe".into()]);

        cycle.start_sell(50);
        assert_eq!(cycle.state, SellState::TravelingToVendor);

        // During travel time — no transition yet
        for t in 50..55 {
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
        assert_eq!(cmds[1].1, "/face fast");
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
    fn test_return_delay_blocks_immediate_completion() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let pid = 104;

        cycle.start_sell(100);
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
    fn test_return_spell_included_in_travel_commands() {
        let mut config = test_vendor_config();
        config.return_spell = Some("gate".into());
        let mut cycle = SellCycle::new(config);
        let pid = 104;

        cycle.start_sell(50);
        let cmds = cycle.tick(pid, 55);
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/cast gate"));
    }

    #[test]
    fn test_sell_step_delay_enforced() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let pid = 104;
        cycle.queue_sell_items(&["Junk".into()]);

        cycle.start_sell(100);
        let _ = cycle.tick(pid, 105); // Travel -> Selling(Targeting)

        // Tick immediately — delay not met, should return empty
        let cmds = cycle.tick(pid, 106);
        assert!(cmds.is_empty());

        // After delay (2 ticks), should advance
        let cmds = cycle.tick(pid, 107);
        assert!(!cmds.is_empty());
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
}
