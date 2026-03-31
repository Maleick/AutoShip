/// Tracks the global cooldown (GCD) between spell casts.
/// EQ's GCD is approximately 1.5 seconds. At ~20 frames/sec, that's ~30 frames.
/// (Not to be confused with EQ's 6-second "game tick" for regen/DoTs.)
pub struct GcdTracker {
    remaining_ticks: u32,
    global_gcd_ticks: u32,
}

impl GcdTracker {
    pub fn new(gcd_ticks: u32) -> Self {
        Self {
            remaining_ticks: 0,
            global_gcd_ticks: gcd_ticks,
        }
    }

    pub fn default_gcd() -> Self {
        Self::new(30)
    }

    pub fn is_ready(&self) -> bool {
        self.remaining_ticks == 0
    }

    pub fn consume(&mut self) {
        self.remaining_ticks = self.global_gcd_ticks;
    }

    pub fn tick(&mut self) {
        if self.remaining_ticks > 0 {
            self.remaining_ticks -= 1;
        }
    }

    pub fn remaining(&self) -> u32 {
        self.remaining_ticks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_gcd_creates_tracker() {
        let tracker = GcdTracker::default_gcd();
        assert_eq!(tracker.remaining(), 0);
    }

    #[test]
    fn is_ready_initially() {
        let tracker = GcdTracker::default_gcd();
        assert!(tracker.is_ready());
    }

    #[test]
    fn consume_makes_not_ready() {
        let mut tracker = GcdTracker::default_gcd();
        tracker.consume();
        assert!(!tracker.is_ready());
        assert_eq!(tracker.remaining(), 30);
    }

    #[test]
    fn tick_decrements_remaining() {
        let mut tracker = GcdTracker::new(5);
        tracker.consume();
        assert_eq!(tracker.remaining(), 5);

        tracker.tick();
        assert_eq!(tracker.remaining(), 4);
        assert!(!tracker.is_ready());

        for _ in 0..4 {
            tracker.tick();
        }
        assert_eq!(tracker.remaining(), 0);
        assert!(tracker.is_ready());
    }

    #[test]
    fn tick_does_not_underflow_at_zero() {
        let mut tracker = GcdTracker::default_gcd();
        tracker.tick();
        assert_eq!(tracker.remaining(), 0);
        assert!(tracker.is_ready());
    }

    #[test]
    fn remaining_returns_correct_value_after_partial_ticks() {
        let mut tracker = GcdTracker::new(10);
        tracker.consume();
        for _ in 0..3 {
            tracker.tick();
        }
        assert_eq!(tracker.remaining(), 7);
    }

    #[test]
    fn consume_resets_to_full_gcd() {
        let mut tracker = GcdTracker::new(10);
        tracker.consume();
        for _ in 0..5 {
            tracker.tick();
        }
        assert_eq!(tracker.remaining(), 5);
        tracker.consume(); // re-consume mid-cooldown
        assert_eq!(tracker.remaining(), 10);
    }

    #[test]
    fn zero_gcd_always_ready() {
        let mut tracker = GcdTracker::new(0);
        tracker.consume();
        assert!(tracker.is_ready());
    }

    #[test]
    fn one_tick_gcd() {
        let mut tracker = GcdTracker::new(1);
        tracker.consume();
        assert!(!tracker.is_ready());
        tracker.tick();
        assert!(tracker.is_ready());
    }
}
