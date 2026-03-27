use dmft_common::nav::KNUTH_HASH;

pub struct CombatPersonality {
    pub assist_jitter_ticks: u8,
    pub cast_start_delay_ticks: u8,
    pub med_sit_threshold: f32,
    rng_state: u32,
}

impl CombatPersonality {
    pub fn from_client_id(client_id: u32) -> Self {
        let seed = client_id.wrapping_mul(KNUTH_HASH).wrapping_add(7919);
        let assist_jitter = (seed % 6) as u8;
        let cast_delay = ((seed >> 8) % 4) as u8;
        let med_threshold = 0.20 + ((seed >> 16) % 150) as f32 / 1000.0;
        Self {
            assist_jitter_ticks: assist_jitter,
            cast_start_delay_ticks: cast_delay,
            med_sit_threshold: med_threshold,
            rng_state: seed,
        }
    }

    pub fn next_assist_delay(&mut self) -> u8 {
        (self.next_u32() % (self.assist_jitter_ticks as u32 + 1)) as u8
    }

    pub fn next_cast_delay(&mut self) -> u8 {
        (self.next_u32() % (self.cast_start_delay_ticks as u32 + 1)) as u8
    }

    pub fn jitter_threshold(&mut self, base: f32, variance: f32) -> f32 {
        let noise = self.next_f32() * variance * 2.0 - variance;
        base + noise
    }

    fn next_u32(&mut self) -> u32 {
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 17;
        self.rng_state ^= self.rng_state << 5;
        self.rng_state
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32) / (u32::MAX as f32)
    }
}
