use dmft_common::nav::{KNUTH_HASH, Xorshift32};

pub struct CombatPersonality {
    pub assist_jitter_ticks: u8,
    pub cast_start_delay_ticks: u8,
    pub med_sit_threshold: f32,
    rng: Xorshift32,
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
            rng: Xorshift32::new(seed),
        }
    }

    pub fn next_assist_delay(&mut self) -> u8 {
        (self.rng.next_u32() % (self.assist_jitter_ticks as u32 + 1)) as u8
    }

    pub fn next_cast_delay(&mut self) -> u8 {
        (self.rng.next_u32() % (self.cast_start_delay_ticks as u32 + 1)) as u8
    }

    pub fn jitter_threshold(&mut self, base: f32, variance: f32) -> f32 {
        let noise = self.rng.next_f32() * variance * 2.0 - variance;
        base + noise
    }
}
