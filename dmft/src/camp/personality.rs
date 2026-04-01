//! Per-character personality profiles for anti-synchronicity.
//!
//! Each character gets a deterministic personality derived from their name hash.
//! This ensures the same character always behaves the same way, but different
//! characters have visibly different timing patterns to avoid synchronized
//! bot-like behavior.

use dmft_common::nav::{KNUTH_HASH, Xorshift32};

/// Per-character behavioral profile that shapes timing and thresholds.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersonalityProfile {
    /// Character name this profile belongs to.
    pub name: String,
    /// Multiplier on tick delays (0.7 = fast reactor, 1.5 = slow/distracted).
    pub reaction_speed: f32,
    /// Affects pull timing aggressiveness (0.8 = cautious, 1.2 = eager).
    pub aggression: f32,
    /// How closely the character follows its rotation (0.9 = sloppy, 1.0 = precise).
    pub discipline: f32,
    /// Variance applied to mana sit/stand thresholds (e.g. +/- 5%).
    pub med_threshold_jitter: f32,
    /// Tick offset so characters don't all act on the same tick (0-30).
    pub phase_offset: u64,
}

impl PersonalityProfile {
    /// Generate a deterministic personality from a character name.
    /// The same name always produces the same profile.
    #[must_use]
    pub fn generate(character_name: &str) -> Self {
        let hash = name_hash(character_name);
        let mut rng = Xorshift32::new(if hash == 0 { 1 } else { hash });

        // reaction_speed: 0.7 - 1.5
        let reaction_speed = 0.7 + rng.next_f32() * 0.8;
        // aggression: 0.8 - 1.2
        let aggression = 0.8 + rng.next_f32() * 0.4;
        // discipline: 0.9 - 1.0
        let discipline = 0.9 + rng.next_f32() * 0.1;
        // med_threshold_jitter: -5.0 to +5.0
        let med_threshold_jitter = (rng.next_f32() - 0.5) * 10.0;
        // phase_offset: 0-30
        let phase_offset = u64::from(rng.next_u32() % 31);

        Self {
            name: character_name.to_string(),
            reaction_speed,
            aggression,
            discipline,
            med_threshold_jitter,
            phase_offset,
        }
    }

    /// Apply `reaction_speed` multiplier to a tick delay.
    #[must_use]
    pub fn adjust_delay(&self, base_ticks: u64) -> u64 {
        let adjusted = (base_ticks as f32 * self.reaction_speed).round() as u64;
        adjusted.max(1) // never zero
    }

    /// Apply `med_threshold_jitter` to a mana percentage threshold.
    #[must_use]
    pub fn adjust_mana_threshold(&self, base_pct: f32) -> f32 {
        (base_pct + self.med_threshold_jitter).clamp(0.0, 100.0)
    }
}

/// Hash a character name to a u32 seed using the Knuth multiplicative hash.
fn name_hash(name: &str) -> u32 {
    let mut h: u32 = 0;
    for byte in name.bytes() {
        h = h.wrapping_add(u32::from(byte)).wrapping_mul(KNUTH_HASH);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_generation() {
        let p1 = PersonalityProfile::generate("Warrior01");
        let p2 = PersonalityProfile::generate("Warrior01");
        assert_eq!(p1.reaction_speed, p2.reaction_speed);
        assert_eq!(p1.aggression, p2.aggression);
        assert_eq!(p1.discipline, p2.discipline);
        assert_eq!(p1.med_threshold_jitter, p2.med_threshold_jitter);
        assert_eq!(p1.phase_offset, p2.phase_offset);
    }

    #[test]
    fn test_different_names_different_profiles() {
        let p1 = PersonalityProfile::generate("Warrior01");
        let p2 = PersonalityProfile::generate("Cleric01");
        // Extremely unlikely (but not impossible) for all fields to match
        let all_same = p1.reaction_speed == p2.reaction_speed
            && p1.aggression == p2.aggression
            && p1.phase_offset == p2.phase_offset;
        assert!(
            !all_same,
            "Different names should produce different profiles"
        );
    }

    #[test]
    fn test_reaction_speed_bounds() {
        for name in &[
            "Warrior01",
            "Cleric01",
            "Enchanter01",
            "Bard01",
            "Ranger01",
            "Ranger02",
        ] {
            let p = PersonalityProfile::generate(name);
            assert!(
                (0.7..=1.5).contains(&p.reaction_speed),
                "{}: reaction_speed {} out of [0.7, 1.5]",
                name,
                p.reaction_speed
            );
        }
    }

    #[test]
    fn test_aggression_bounds() {
        for name in &["Warrior01", "Cleric01", "Enchanter01", "Bard01"] {
            let p = PersonalityProfile::generate(name);
            assert!(
                (0.8..=1.2).contains(&p.aggression),
                "{}: aggression {} out of [0.8, 1.2]",
                name,
                p.aggression
            );
        }
    }

    #[test]
    fn test_discipline_bounds() {
        for name in &["Warrior01", "Cleric01", "Enchanter01", "Bard01"] {
            let p = PersonalityProfile::generate(name);
            assert!(
                (0.9..=1.0).contains(&p.discipline),
                "{}: discipline {} out of [0.9, 1.0]",
                name,
                p.discipline
            );
        }
    }

    #[test]
    fn test_med_threshold_jitter_bounds() {
        for name in &["Warrior01", "Cleric01", "Enchanter01", "Bard01"] {
            let p = PersonalityProfile::generate(name);
            assert!(
                (-5.0..=5.0).contains(&p.med_threshold_jitter),
                "{}: med_threshold_jitter {} out of [-5.0, 5.0]",
                name,
                p.med_threshold_jitter
            );
        }
    }

    #[test]
    fn test_phase_offset_bounds() {
        for name in &[
            "Warrior01",
            "Cleric01",
            "Enchanter01",
            "Bard01",
            "Ranger01",
            "Ranger02",
        ] {
            let p = PersonalityProfile::generate(name);
            assert!(
                p.phase_offset <= 30,
                "{}: phase_offset {} exceeds 30",
                name,
                p.phase_offset
            );
        }
    }

    #[test]
    fn test_adjust_delay() {
        let mut p = PersonalityProfile::generate("Warrior01");
        p.reaction_speed = 1.5;
        assert_eq!(p.adjust_delay(10), 15);

        p.reaction_speed = 0.7;
        assert_eq!(p.adjust_delay(10), 7);

        // Never returns zero
        p.reaction_speed = 0.7;
        assert_eq!(p.adjust_delay(1), 1);
    }

    #[test]
    fn test_adjust_mana_threshold() {
        let mut p = PersonalityProfile::generate("Cleric01");
        p.med_threshold_jitter = 5.0;
        assert!((p.adjust_mana_threshold(30.0) - 35.0).abs() < f32::EPSILON);

        p.med_threshold_jitter = -5.0;
        assert!((p.adjust_mana_threshold(30.0) - 25.0).abs() < f32::EPSILON);

        // Clamped to valid range
        p.med_threshold_jitter = -5.0;
        assert!((p.adjust_mana_threshold(3.0) - 0.0).abs() < f32::EPSILON);

        p.med_threshold_jitter = 5.0;
        assert!((p.adjust_mana_threshold(98.0) - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_empty_name_does_not_panic() {
        let p = PersonalityProfile::generate("");
        assert!((0.7..=1.5).contains(&p.reaction_speed));
    }

    #[test]
    fn test_name_stored_correctly() {
        let p = PersonalityProfile::generate("Warrior01");
        assert_eq!(p.name, "Warrior01");
    }

    #[test]
    fn test_adjust_delay_never_zero() {
        let mut p = PersonalityProfile::generate("Test");
        p.reaction_speed = 0.01; // extremely fast
        assert_eq!(p.adjust_delay(1), 1); // clamped to 1
    }

    #[test]
    fn test_adjust_delay_rounding() {
        let mut p = PersonalityProfile::generate("Test");
        p.reaction_speed = 1.0;
        assert_eq!(p.adjust_delay(10), 10);

        p.reaction_speed = 1.25;
        assert_eq!(p.adjust_delay(10), 13); // 12.5 rounds to 13
    }

    #[test]
    fn test_serialization_roundtrip() {
        let p = PersonalityProfile::generate("Cleric01");
        let json = serde_json::to_string(&p).unwrap();
        let restored: PersonalityProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, "Cleric01");
        assert!((restored.reaction_speed - p.reaction_speed).abs() < f32::EPSILON);
        assert_eq!(restored.phase_offset, p.phase_offset);
    }

    #[test]
    fn test_name_hash_deterministic() {
        let a = name_hash("Warrior01");
        let b = name_hash("Warrior01");
        assert_eq!(a, b);
    }

    #[test]
    fn test_name_hash_differs() {
        let a = name_hash("Warrior01");
        let b = name_hash("Cleric01");
        assert_ne!(a, b);
    }

    #[test]
    fn test_many_names_varied_phase_offsets() {
        let offsets: Vec<u64> = (0..20)
            .map(|i| PersonalityProfile::generate(&format!("Char{i}")).phase_offset)
            .collect();
        // With 20 names over 31 possible offsets, we should get some variation
        let unique: std::collections::HashSet<_> = offsets.iter().collect();
        assert!(unique.len() > 3, "Expected varied phase offsets");
    }
}
