//! Vendor/sell cycle — periodically sell loot to a nearby vendor.

use std::collections::HashSet;

/// Configuration for the vendor sell cycle.
#[derive(Debug, Clone)]
pub struct VendorConfig {
    pub vendor_name: String,
    pub sell_interval_ticks: u64,
    pub keep_items: Vec<String>,
    /// Ticks to wait in TravelingToVendor before transitioning to Selling.
    pub travel_ticks: u64,
}

/// Current state of the sell cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SellState {
    NotNeeded,
    TravelingToVendor,
    Selling,
    Returning,
}

/// Tracks the sell cycle for one camp group.
pub struct SellCycle {
    pub config: VendorConfig,
    pub state: SellState,
    pub last_sell_tick: u64,
    pub state_entered_tick: u64,
    keep_set: HashSet<String>,
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

    /// Advance the sell state machine by one tick. Returns `(pid, command)` pairs
    /// for the designated seller.
    pub fn tick(&mut self, seller_pid: u32, current_tick: u64) -> Vec<(u32, String)> {
        match &self.state {
            SellState::NotNeeded => Vec::new(),
            SellState::TravelingToVendor => {
                // Wait for travel time before transitioning to selling
                if current_tick.saturating_sub(self.state_entered_tick) < self.config.travel_ticks {
                    return Vec::new();
                }
                let cmds = sell_commands(&self.state, &self.config.vendor_name, seller_pid);
                self.state = SellState::Selling;
                self.state_entered_tick = current_tick;
                cmds
            }
            SellState::Selling => {
                let cmds = sell_commands(&self.state, &self.config.vendor_name, seller_pid);
                self.state = SellState::Returning;
                self.state_entered_tick = current_tick;
                cmds
            }
            SellState::Returning => {
                // Wait for travel time before completing the cycle
                if current_tick.saturating_sub(self.state_entered_tick) < self.config.travel_ticks {
                    return Vec::new();
                }
                let cmds = sell_commands(&self.state, &self.config.vendor_name, seller_pid);
                self.state = SellState::NotNeeded;
                self.last_sell_tick = current_tick;
                cmds
            }
        }
    }

    /// Begin the sell cycle.
    pub fn start_sell(&mut self, current_tick: u64) {
        if self.state == SellState::NotNeeded {
            self.state = SellState::TravelingToVendor;
            self.state_entered_tick = current_tick;
        }
    }
}

/// Generate slash commands for the current sell state.
pub fn sell_commands(state: &SellState, vendor_name: &str, seller_pid: u32) -> Vec<(u32, String)> {
    match state {
        SellState::NotNeeded => Vec::new(),
        SellState::TravelingToVendor => {
            vec![
                (seller_pid, format!("/target {vendor_name}")),
                (seller_pid, "/face".into()),
            ]
        }
        SellState::Selling => {
            // V1: generate the commands; actual vendor UI interaction is a TODO.
            vec![
                (seller_pid, format!("/target {vendor_name}")),
                (seller_pid, "/click right target".into()),
                // TODO: vendor window sell logic — for now, placeholder
                (seller_pid, "/say Selling loot".into()),
            ]
        }
        SellState::Returning => {
            // Stand up and return to camp position. NOT /camp which logs out!
            // The orchestrator will handle navigation back to camp_center.
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
        // Interval is 50 ticks, last sell at 0
        assert!(!cycle.needs_sell(30));
        assert!(cycle.needs_sell(50));
        assert!(cycle.needs_sell(100));
    }

    #[test]
    fn test_needs_sell_false_when_already_selling() {
        let mut cycle = SellCycle::new(test_vendor_config());
        cycle.state = SellState::Selling;
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
    fn test_full_sell_cycle() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let pid = 104;

        cycle.start_sell(50);
        assert_eq!(cycle.state, SellState::TravelingToVendor);

        // During travel time (5 ticks) — no transition yet
        for t in 50..55 {
            let cmds = cycle.tick(pid, t);
            assert_eq!(cycle.state, SellState::TravelingToVendor);
            assert!(cmds.is_empty());
        }

        // After travel time: traveling -> selling
        let cmds = cycle.tick(pid, 55);
        assert_eq!(cycle.state, SellState::Selling);
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("Merchant_Leah")));

        // Selling -> returning (immediate, no travel delay for selling itself)
        let cmds = cycle.tick(pid, 56);
        assert_eq!(cycle.state, SellState::Returning);
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("right target")));

        // During return travel time — no transition yet
        for t in 57..61 {
            let cmds = cycle.tick(pid, t);
            assert_eq!(cycle.state, SellState::Returning);
            assert!(cmds.is_empty());
        }

        // After return travel time: returning -> not needed
        let cmds = cycle.tick(pid, 61);
        assert_eq!(cycle.state, SellState::NotNeeded);
        assert_eq!(cycle.last_sell_tick, 61);
        assert!(!cmds.is_empty());
    }

    #[test]
    fn test_start_sell_idempotent() {
        let mut cycle = SellCycle::new(test_vendor_config());
        cycle.state = SellState::Selling;
        cycle.start_sell(100); // Should not reset to TravelingToVendor
        assert_eq!(cycle.state, SellState::Selling);
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
        assert_eq!(cmds[1].1, "/face");
    }

    #[test]
    fn test_sell_commands_selling() {
        let cmds = sell_commands(&SellState::Selling, "Merchant_Leah", 104);
        assert!(cmds.len() >= 2);
        assert!(cmds.iter().any(|(_, cmd)| cmd.contains("right target")));
    }

    #[test]
    fn test_sell_commands_returning() {
        let cmds = sell_commands(&SellState::Returning, "Merchant_Leah", 104);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].1, "/stand");
    }

    // -- Bug #7: travel delay prevents instant cycle completion --

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
        assert_eq!(cycle.state, SellState::Selling);
        assert!(!cmds.is_empty());
    }

    #[test]
    fn test_return_delay_blocks_immediate_completion() {
        let mut cycle = SellCycle::new(test_vendor_config());
        let pid = 104;

        cycle.start_sell(100);
        // Skip past travel
        let _ = cycle.tick(pid, 105);
        assert_eq!(cycle.state, SellState::Selling);
        // Selling -> Returning (immediate)
        let _ = cycle.tick(pid, 106);
        assert_eq!(cycle.state, SellState::Returning);

        // Return travel should also take travel_ticks
        let cmds = cycle.tick(pid, 108);
        assert_eq!(cycle.state, SellState::Returning);
        assert!(cmds.is_empty());

        let cmds = cycle.tick(pid, 111);
        assert_eq!(cycle.state, SellState::NotNeeded);
        assert!(!cmds.is_empty());
    }
}
