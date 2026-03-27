//! Movement humanization — per-character speed jitter, heading wobble,
//! and occasional path deviations to avoid bot-like movement patterns.

/// Per-character movement personality. Values are seeded from the client_id
/// so each character consistently moves differently.
pub struct MovementPersonality {
    /// Speed multiplier variance (e.g., 0.93..1.07).
    pub speed_factor: f32,
    /// Max heading wobble in EQ degrees per tick.
    pub heading_wobble: f32,
    /// Chance per waypoint of taking a slight detour (0.0..1.0).
    pub detour_chance: f32,
    /// Simple PRNG state for deterministic randomness.
    rng_state: u32,
}

impl MovementPersonality {
    /// Create a personality seeded from the client ID.
    pub fn from_client_id(client_id: u32) -> Self {
        // Simple hash to spread values across characters.
        let seed = client_id.wrapping_mul(dmft_common::nav::KNUTH_HASH); // Knuth multiplicative hash
        let speed_factor = 0.93 + (seed % 140) as f32 / 1000.0; // 0.93..1.07
        let heading_wobble = 1.0 + (seed.wrapping_shr(8) % 30) as f32 / 10.0; // 1.0..4.0
        let detour_chance = (seed.wrapping_shr(16) % 80) as f32 / 1000.0; // 0.00..0.08

        Self {
            speed_factor,
            heading_wobble,
            detour_chance,
            rng_state: seed,
        }
    }

    /// Apply heading wobble to a base heading. Returns adjusted heading.
    pub fn wobble_heading(&mut self, heading: f32) -> f32 {
        let noise = self.next_f32() * self.heading_wobble * 2.0 - self.heading_wobble;
        (heading + noise + 512.0) % 512.0
    }

    /// Should this character take a detour at the current waypoint?
    pub fn should_detour(&mut self) -> bool {
        self.next_f32() < self.detour_chance
    }

    /// Generate a random stagger delay in ticks (0..max_ticks).
    pub fn stagger_ticks(&mut self, max_ticks: u32) -> u32 {
        (self.next_f32() * max_ticks as f32) as u32
    }

    /// Simple xorshift32 PRNG, returns 0.0..1.0.
    fn next_f32(&mut self) -> f32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        (self.rng_state as f32) / (u32::MAX as f32)
    }
}
