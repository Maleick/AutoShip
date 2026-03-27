/// Tracks the global cooldown (GCD) between spell casts.
/// EQ's GCD is approximately 1.5 seconds. At ~20 ticks/sec, that's ~30 ticks.
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
