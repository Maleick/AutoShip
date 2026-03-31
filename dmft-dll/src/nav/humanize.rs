//! Movement humanization — per-character speed jitter, heading wobble,
//! and occasional path deviations to avoid bot-like movement patterns.

use dmft_common::nav::Xorshift32;

/// Per-character movement personality. Values are seeded from the client_id
/// so each character consistently moves differently.
pub struct MovementPersonality {
    /// Speed multiplier variance (e.g., 0.93..1.07).
    pub speed_factor: f32,
    /// Max heading wobble in EQ degrees per tick.
    pub heading_wobble: f32,
    /// Chance per waypoint of taking a slight detour (0.0..1.0).
    pub detour_chance: f32,
    /// PRNG for deterministic randomness.
    rng: Xorshift32,
}

impl MovementPersonality {
    /// Create a personality seeded from the client ID.
    pub fn from_client_id(client_id: u32) -> Self {
        let mut rng = Xorshift32::from_client_id(client_id);
        let seed = rng.next_u32();
        let speed_factor = 0.93 + (seed % 140) as f32 / 1000.0; // 0.93..1.07
        let heading_wobble = 1.0 + (seed.wrapping_shr(8) % 30) as f32 / 10.0; // 1.0..4.0
        let detour_chance = (seed.wrapping_shr(16) % 80) as f32 / 1000.0; // 0.00..0.08

        Self {
            speed_factor,
            heading_wobble,
            detour_chance,
            rng,
        }
    }

    /// Apply heading wobble to a base heading. Returns adjusted heading.
    pub fn wobble_heading(&mut self, heading: f32) -> f32 {
        let noise = self.rng.next_f32() * self.heading_wobble * 2.0 - self.heading_wobble;
        (heading + noise + 512.0) % 512.0
    }

    /// Should this character take a detour at the current waypoint?
    pub fn should_detour(&mut self) -> bool {
        self.rng.next_f32() < self.detour_chance
    }

    /// Generate a random stagger delay in ticks (0..max_ticks).
    pub fn stagger_ticks(&mut self, max_ticks: u32) -> u32 {
        (self.rng.next_f32() * max_ticks as f32) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_client_id_creates_personality() {
        let p = MovementPersonality::from_client_id(1);
        assert!(p.speed_factor > 0.0);
        assert!(p.heading_wobble > 0.0);
        assert!(p.detour_chance >= 0.0);
    }

    #[test]
    fn different_client_ids_produce_different_personalities() {
        let a = MovementPersonality::from_client_id(1);
        let b = MovementPersonality::from_client_id(2);
        let differs = (a.speed_factor - b.speed_factor).abs() > f32::EPSILON
            || (a.heading_wobble - b.heading_wobble).abs() > f32::EPSILON
            || (a.detour_chance - b.detour_chance).abs() > f32::EPSILON;
        assert!(
            differs,
            "Different client IDs should produce different values"
        );
    }

    #[test]
    fn speed_factor_in_expected_range() {
        for id in 0..50 {
            let p = MovementPersonality::from_client_id(id);
            assert!(
                p.speed_factor >= 0.93 && p.speed_factor <= 1.07,
                "speed_factor {} out of range for client_id {id}",
                p.speed_factor
            );
        }
    }

    #[test]
    fn heading_wobble_in_expected_range() {
        for id in 0..50 {
            let p = MovementPersonality::from_client_id(id);
            assert!(
                p.heading_wobble >= 1.0 && p.heading_wobble <= 4.0,
                "heading_wobble {} out of range for client_id {id}",
                p.heading_wobble
            );
        }
    }

    #[test]
    fn detour_chance_in_expected_range() {
        for id in 0..50 {
            let p = MovementPersonality::from_client_id(id);
            assert!(
                p.detour_chance >= 0.0 && p.detour_chance <= 0.08,
                "detour_chance {} out of range for client_id {id}",
                p.detour_chance
            );
        }
    }

    #[test]
    fn wobble_heading_stays_in_eq_range() {
        let mut p = MovementPersonality::from_client_id(10);
        for _ in 0..100 {
            let result = p.wobble_heading(256.0);
            assert!(
                (0.0..512.0).contains(&result),
                "wobbled heading {result} out of EQ range 0..512"
            );
        }
    }

    #[test]
    fn wobble_heading_varies() {
        let mut p = MovementPersonality::from_client_id(10);
        let a = p.wobble_heading(256.0);
        let b = p.wobble_heading(256.0);
        // With any non-zero wobble, consecutive calls should sometimes differ
        // (they could theoretically be equal, but with a good RNG it's unlikely)
        let c = p.wobble_heading(256.0);
        let all_same = (a - b).abs() < f32::EPSILON && (b - c).abs() < f32::EPSILON;
        assert!(!all_same, "wobble should produce varying values");
    }

    #[test]
    fn stagger_ticks_in_range() {
        let mut p = MovementPersonality::from_client_id(5);
        for _ in 0..100 {
            let ticks = p.stagger_ticks(20);
            assert!(ticks < 20, "stagger_ticks {ticks} should be < 20");
        }
    }

    #[test]
    fn stagger_ticks_zero_max_returns_zero() {
        let mut p = MovementPersonality::from_client_id(1);
        for _ in 0..10 {
            assert_eq!(p.stagger_ticks(0), 0);
        }
    }

    #[test]
    fn stagger_ticks_one_returns_zero() {
        let mut p = MovementPersonality::from_client_id(1);
        for _ in 0..10 {
            assert_eq!(p.stagger_ticks(1), 0);
        }
    }

    #[test]
    fn should_detour_returns_bool() {
        let mut p = MovementPersonality::from_client_id(7);
        // Just verify it doesn't panic and returns a bool over many calls
        let mut _any_true = false;
        let mut any_false = false;
        for _ in 0..1000 {
            if p.should_detour() {
                _any_true = true;
            } else {
                any_false = true;
            }
        }
        // With detour_chance between 0 and 0.08, we expect mostly false
        assert!(any_false, "should_detour should sometimes return false");
    }

    #[test]
    fn wobble_heading_at_zero() {
        let mut p = MovementPersonality::from_client_id(3);
        let result = p.wobble_heading(0.0);
        assert!(
            (0.0..512.0).contains(&result),
            "wobbled heading at 0 should stay in range"
        );
    }

    #[test]
    fn wobble_heading_at_boundary() {
        let mut p = MovementPersonality::from_client_id(3);
        // Test heading near 512 boundary (wraps to 0)
        let result = p.wobble_heading(511.0);
        assert!(
            (0.0..512.0).contains(&result),
            "wobbled heading near 512 should wrap correctly"
        );
    }
}
