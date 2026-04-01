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
        (self.rng.next_u32() % (u32::from(self.assist_jitter_ticks) + 1)) as u8
    }

    pub fn next_cast_delay(&mut self) -> u8 {
        (self.rng.next_u32() % (u32::from(self.cast_start_delay_ticks) + 1)) as u8
    }

    pub fn jitter_threshold(&mut self, base: f32, variance: f32) -> f32 {
        let noise = self.rng.next_f32() * variance * 2.0 - variance;
        base + noise
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_client_id_creates_different_personalities() {
        let a = CombatPersonality::from_client_id(1);
        let b = CombatPersonality::from_client_id(2);
        // At least one field should differ between different client IDs
        let differs = a.assist_jitter_ticks != b.assist_jitter_ticks
            || a.cast_start_delay_ticks != b.cast_start_delay_ticks
            || (a.med_sit_threshold - b.med_sit_threshold).abs() > f32::EPSILON;
        assert!(
            differs,
            "Different client IDs should produce different personalities"
        );
    }

    #[test]
    fn from_client_id_is_deterministic() {
        let a = CombatPersonality::from_client_id(42);
        let b = CombatPersonality::from_client_id(42);
        assert_eq!(a.assist_jitter_ticks, b.assist_jitter_ticks);
        assert_eq!(a.cast_start_delay_ticks, b.cast_start_delay_ticks);
        assert!((a.med_sit_threshold - b.med_sit_threshold).abs() < f32::EPSILON);
    }

    #[test]
    fn next_cast_delay_in_range() {
        let mut personality = CombatPersonality::from_client_id(7);
        for _ in 0..100 {
            let delay = personality.next_cast_delay();
            assert!(
                delay <= personality.cast_start_delay_ticks,
                "cast delay {delay} should be <= max {}",
                personality.cast_start_delay_ticks
            );
        }
    }

    #[test]
    fn next_assist_delay_in_range() {
        let mut personality = CombatPersonality::from_client_id(13);
        for _ in 0..100 {
            let delay = personality.next_assist_delay();
            assert!(
                delay <= personality.assist_jitter_ticks,
                "assist delay {delay} should be <= max {}",
                personality.assist_jitter_ticks
            );
        }
    }

    #[test]
    fn med_sit_threshold_in_expected_range() {
        for id in 0..50 {
            let p = CombatPersonality::from_client_id(id);
            assert!(
                p.med_sit_threshold >= 0.20 && p.med_sit_threshold <= 0.35,
                "med_sit_threshold {} out of range for client_id {id}",
                p.med_sit_threshold
            );
        }
    }

    #[test]
    fn jitter_threshold_stays_near_base() {
        let mut personality = CombatPersonality::from_client_id(99);
        for _ in 0..100 {
            let val = personality.jitter_threshold(50.0, 5.0);
            assert!(
                (45.0..=55.0).contains(&val),
                "jittered value {val} out of range"
            );
        }
    }

    #[test]
    fn jitter_threshold_zero_variance() {
        let mut personality = CombatPersonality::from_client_id(1);
        for _ in 0..20 {
            let val = personality.jitter_threshold(100.0, 0.0);
            assert!((val - 100.0).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn client_id_zero_works() {
        let p = CombatPersonality::from_client_id(0);
        assert!(p.assist_jitter_ticks <= 5);
        assert!(p.cast_start_delay_ticks <= 3);
    }

    #[test]
    fn client_id_u32_max_works() {
        let p = CombatPersonality::from_client_id(u32::MAX);
        assert!(p.assist_jitter_ticks <= 5);
        assert!(p.cast_start_delay_ticks <= 3);
        assert!(p.med_sit_threshold >= 0.20 && p.med_sit_threshold <= 0.35);
    }

    #[test]
    fn next_assist_delay_produces_varied_values() {
        let mut personality = CombatPersonality::from_client_id(42);
        if personality.assist_jitter_ticks > 0 {
            let values: Vec<u8> = (0..50).map(|_| personality.next_assist_delay()).collect();
            let has_variation = values.windows(2).any(|w| w[0] != w[1]);
            assert!(has_variation, "assist delays should vary");
        }
    }
}
