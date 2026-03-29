/// Complete Heal chain coordinator.
///
/// Manages a rotation of clerics casting Complete Heal on the main tank.
/// Each cleric starts their cast at a fixed interval after the previous one,
/// creating a steady stream of heals landing on the tank.
const TICKS_PER_SECOND: u64 = 20;

pub struct ChChain {
    /// Cleric PIDs in chain order.
    members: Vec<u32>,
    /// Seconds between each cleric starting their CH.
    interval_secs: f32,
    /// Current position in the chain.
    current_index: usize,
    /// Whether the chain is active.
    active: bool,
    /// Tick counter for timing.
    tick_count: u64,
    /// Ticks per interval (computed from interval_secs).
    ticks_per_interval: u64,
}

impl ChChain {
    pub fn new(members: Vec<u32>, interval_secs: f32) -> Self {
        Self {
            members,
            interval_secs,
            current_index: 0,
            active: false,
            tick_count: 0,
            ticks_per_interval: (interval_secs * TICKS_PER_SECOND as f32) as u64,
        }
    }

    pub fn start(&mut self) {
        self.active = true;
        self.tick_count = 0;
        self.current_index = 0;
    }

    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Advance the chain by one tick. Returns the PID that should start
    /// casting CH this tick, if any.
    pub fn tick(&mut self) -> Option<u32> {
        if !self.active || self.members.is_empty() || self.ticks_per_interval == 0 {
            return None;
        }

        let should_fire = self.tick_count % self.ticks_per_interval == 0;
        self.tick_count += 1;

        if should_fire {
            let pid = self.members[self.current_index];
            self.current_index = (self.current_index + 1) % self.members.len();
            Some(pid)
        } else {
            None
        }
    }

    pub fn set_interval(&mut self, secs: f32) {
        self.interval_secs = secs;
        self.ticks_per_interval = (secs * TICKS_PER_SECOND as f32) as u64;
    }

    pub fn add_member(&mut self, pid: u32) {
        if !self.members.contains(&pid) {
            self.members.push(pid);
        }
    }

    pub fn remove_member(&mut self, pid: u32) {
        if let Some(pos) = self.members.iter().position(|&p| p == pid) {
            self.members.remove(pos);
            if self.members.is_empty() {
                self.current_index = 0;
            } else if self.current_index >= self.members.len() {
                self.current_index = 0;
            } else if pos < self.current_index {
                self.current_index -= 1;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_rotation_order() {
        let mut chain = ChChain::new(vec![100, 200, 300, 400], 3.0);
        chain.start();

        // First tick fires immediately (tick 0)
        assert_eq!(chain.tick(), Some(100));

        // Next 59 ticks: nothing until tick 60
        for _ in 1..60 {
            assert_eq!(chain.tick(), None);
        }
        assert_eq!(chain.tick(), Some(200));

        // Skip to tick 120
        for _ in 0..59 {
            assert_eq!(chain.tick(), None);
        }
        assert_eq!(chain.tick(), Some(300));

        // Skip to tick 180
        for _ in 0..59 {
            assert_eq!(chain.tick(), None);
        }
        assert_eq!(chain.tick(), Some(400));
    }

    #[test]
    fn chain_wraps_around() {
        let mut chain = ChChain::new(vec![10, 20], 1.0);
        chain.start();

        // ticks_per_interval = 20
        assert_eq!(chain.tick(), Some(10)); // tick 0
        for _ in 0..19 {
            chain.tick();
        }
        assert_eq!(chain.tick(), Some(20)); // tick 20 — but wait, let me recalc

        // Actually: tick 0 fires (10), ticks 1-19 are None, tick 20 fires (20)
        // We already consumed tick 0 (Some(10)) and 19 more ticks above.
        // So the last tick() above was tick 20, which should be Some(20).
        // Let's just re-verify with a fresh chain.
        let mut chain = ChChain::new(vec![10, 20], 1.0);
        chain.start();

        let mut fired = Vec::new();
        for _ in 0..41 {
            if let Some(pid) = chain.tick() {
                fired.push(pid);
            }
        }
        // Fires at ticks 0, 20, 40 → pids 10, 20, 10 (wrap)
        assert_eq!(fired, vec![10, 20, 10]);
    }

    #[test]
    fn start_stop() {
        let mut chain = ChChain::new(vec![1, 2, 3], 3.0);

        // Not active by default
        assert!(!chain.is_active());
        assert_eq!(chain.tick(), None);

        chain.start();
        assert!(chain.is_active());
        assert_eq!(chain.tick(), Some(1));

        chain.stop();
        assert!(!chain.is_active());
        assert_eq!(chain.tick(), None);
    }

    #[test]
    fn start_resets_position() {
        let mut chain = ChChain::new(vec![1, 2, 3], 3.0);
        chain.start();
        assert_eq!(chain.tick(), Some(1));

        // Restart resets to beginning
        chain.start();
        assert_eq!(chain.tick(), Some(1));
    }

    #[test]
    fn dynamic_add_member() {
        let mut chain = ChChain::new(vec![1, 2], 1.0);
        chain.add_member(3);
        chain.start();

        let mut fired = Vec::new();
        for _ in 0..61 {
            if let Some(pid) = chain.tick() {
                fired.push(pid);
            }
        }
        // Fires at ticks 0, 20, 40, 60 → pids 1, 2, 3, 1
        assert_eq!(fired, vec![1, 2, 3, 1]);
    }

    #[test]
    fn dynamic_remove_member() {
        let mut chain = ChChain::new(vec![1, 2, 3], 1.0);
        chain.start();
        assert_eq!(chain.tick(), Some(1)); // index now 1

        chain.remove_member(2); // remove member at index 1, current_index adjusts
        // members: [1, 3], current_index stays at 1 (pointing to 3)

        for _ in 0..19 {
            chain.tick();
        }
        assert_eq!(chain.tick(), Some(3));
    }

    #[test]
    fn remove_only_member_stops_firing() {
        let mut chain = ChChain::new(vec![1], 1.0);
        chain.start();
        assert_eq!(chain.tick(), Some(1));

        chain.remove_member(1);
        assert_eq!(chain.tick(), None);
    }

    #[test]
    fn add_duplicate_ignored() {
        let mut chain = ChChain::new(vec![1, 2], 1.0);
        chain.add_member(1);
        assert_eq!(chain.members.len(), 2);
    }

    #[test]
    fn set_interval_changes_timing() {
        let mut chain = ChChain::new(vec![1, 2], 3.0);
        chain.start();
        assert_eq!(chain.tick(), Some(1)); // tick 0

        chain.set_interval(1.0); // now 20 ticks per interval

        for _ in 0..19 {
            chain.tick();
        }
        assert_eq!(chain.tick(), Some(2)); // tick 20
    }

    #[test]
    fn empty_members_never_fires() {
        let mut chain = ChChain::new(vec![], 3.0);
        chain.start();
        assert_eq!(chain.tick(), None);
    }
}
