//! Debuff detection and classification.
//!
//! Provides debuff type classification, duration tracking, and dispel requirements
//! for crowd control, diseases, poisons, and curses commonly found in EverQuest.
//!
//! The debuff database is a static collection of known EQ debuffs categorized by type,
//! used by the combat system to determine cure priorities and per-class handling.

use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Debuff type classification.
///
/// Debuffs are categorized by their primary effect and removal requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DebuffType {
    /// Crowd control: stun, root, snare, mez (mesmerization), charm
    CrowdControl,
    /// Disease type debuff
    Disease,
    /// Poison type debuff
    Poison,
    /// Curse type debuff
    Curse,
}

impl DebuffType {
    /// Human-readable name for the debuff type.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            DebuffType::CrowdControl => "Crowd Control",
            DebuffType::Disease => "Disease",
            DebuffType::Poison => "Poison",
            DebuffType::Curse => "Curse",
        }
    }
}

/// Dispel type specifies which cure spells can remove a debuff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DispelType {
    /// Curable by cure poison/antidote spells
    CurePoison,
    /// Curable by cure disease/cure curse spells
    CureDisease,
    /// Curable by remove curse spells
    RemoveCurse,
    /// Curable by break-stun or stun-related cures
    StunBreaker,
    /// Curable by mez-breaker or wake spells
    MezBreaker,
    /// Curable by snare-break spells
    SnareBreaker,
    /// No known cure available in-game
    Incurable,
}

impl DispelType {
    /// Human-readable name for the dispel type.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            DispelType::CurePoison => "Cure Poison",
            DispelType::CureDisease => "Cure Disease",
            DispelType::RemoveCurse => "Remove Curse",
            DispelType::StunBreaker => "Stun Breaker",
            DispelType::MezBreaker => "Mez Breaker",
            DispelType::SnareBreaker => "Snare Breaker",
            DispelType::Incurable => "Incurable",
        }
    }
}

/// A debuff entry from the static database.
///
/// Contains metadata about a known EQ debuff: its name, type, typical duration,
/// and what spells/abilities can cure it.
#[derive(Debug, Clone)]
pub struct DebuffEntry {
    /// Spell ID of the debuff
    pub spell_id: i32,
    /// Human-readable name (e.g., "Hamstring", "Plague")
    pub name: &'static str,
    /// Debuff category
    pub debuff_type: DebuffType,
    /// Typical duration in seconds (0 = permanent/indefinite)
    pub duration_seconds: u32,
    /// How to remove/cure the debuff
    pub dispel_type: DispelType,
}

impl DebuffEntry {
    /// Create a new debuff entry.
    pub const fn new(
        spell_id: i32,
        name: &'static str,
        debuff_type: DebuffType,
        duration_seconds: u32,
        dispel_type: DispelType,
    ) -> Self {
        DebuffEntry {
            spell_id,
            name,
            debuff_type,
            duration_seconds,
            dispel_type,
        }
    }

    /// Whether this debuff is permanent or very long-lasting.
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        self.duration_seconds == 0
    }

    /// Whether this debuff requires immediate removal for combat effectiveness.
    #[must_use]
    pub fn is_high_priority(&self) -> bool {
        matches!(
            self.debuff_type,
            DebuffType::CrowdControl | DebuffType::Curse
        )
    }
}

/// Static debuff database indexed by spell ID.
static DEBUFF_DATABASE: Lazy<HashMap<i32, DebuffEntry>> = Lazy::new(|| {
    let entries = vec![
        // ─── Crowd Control: Stun/Root/Snare ────────────────────────────────
        DebuffEntry::new(
            1234, // Example stun spell
            "Paralyzing Bite",
            DebuffType::CrowdControl,
            6,
            DispelType::StunBreaker,
        ),
        DebuffEntry::new(
            1235,
            "Hamstring",
            DebuffType::CrowdControl,
            18,
            DispelType::SnareBreaker,
        ),
        DebuffEntry::new(
            1236,
            "Entangle",
            DebuffType::CrowdControl,
            24,
            DispelType::SnareBreaker,
        ),
        DebuffEntry::new(
            1237,
            "Ensnare",
            DebuffType::CrowdControl,
            30,
            DispelType::SnareBreaker,
        ),
        DebuffEntry::new(
            1238,
            "Root",
            DebuffType::CrowdControl,
            0, // typically permanent until dispelled
            DispelType::Incurable,
        ),
        DebuffEntry::new(
            1239,
            "Stun",
            DebuffType::CrowdControl,
            3,
            DispelType::StunBreaker,
        ),
        // ─── Diseases ──────────────────────────────────────────────────────
        DebuffEntry::new(
            1300,
            "Plague",
            DebuffType::Disease,
            120,
            DispelType::CureDisease,
        ),
        DebuffEntry::new(
            1301,
            "Plague of Insects",
            DebuffType::Disease,
            60,
            DispelType::CureDisease,
        ),
        DebuffEntry::new(
            1302,
            "Rotting Flesh",
            DebuffType::Disease,
            90,
            DispelType::CureDisease,
        ),
        // ─── Poisons ──────────────────────────────────────────────────────
        DebuffEntry::new(
            1400,
            "Poison",
            DebuffType::Poison,
            60,
            DispelType::CurePoison,
        ),
        DebuffEntry::new(
            1401,
            "Venom",
            DebuffType::Poison,
            90,
            DispelType::CurePoison,
        ),
        DebuffEntry::new(
            1402,
            "Noxious Poison",
            DebuffType::Poison,
            120,
            DispelType::CurePoison,
        ),
        // ─── Curses ────────────────────────────────────────────────────────
        DebuffEntry::new(
            1500,
            "Curse",
            DebuffType::Curse,
            0, // permanent until removed
            DispelType::RemoveCurse,
        ),
        DebuffEntry::new(
            1501,
            "Curse of Magi",
            DebuffType::Curse,
            0,
            DispelType::RemoveCurse,
        ),
        DebuffEntry::new(
            1502,
            "Symbol of Corruption",
            DebuffType::Curse,
            0,
            DispelType::RemoveCurse,
        ),
    ];

    let mut map = HashMap::new();
    for entry in entries {
        map.insert(entry.spell_id, entry);
    }
    map
});

/// Look up a debuff entry by spell ID.
///
/// Returns `Some(entry)` if the spell ID exists in the database,
/// or `None` if unknown.
#[must_use]
pub fn lookup_debuff(spell_id: i32) -> Option<&'static DebuffEntry> {
    DEBUFF_DATABASE.get(&spell_id)
}

/// Classify all active debuffs from a buff list.
///
/// This is a utility function that examines each buff and determines
/// if it is a known debuff. Used to prioritize enemy crowd control
/// and status effects for dispel/cure operations.
///
/// # Arguments
/// * `buffs` - List of active BuffInfo from the player or NPC
///
/// # Returns
/// Vector of matching DebuffEntry references for known debuffs.
pub fn classify_debuffs(buffs: &[textquest_common::combat::BuffInfo]) -> Vec<&'static DebuffEntry> {
    buffs
        .iter()
        .filter_map(|buff| lookup_debuff(buff.spell_id))
        .collect()
}

/// Get all high-priority debuffs (CC and Curses) from a list.
#[must_use]
pub fn high_priority_debuffs(
    buffs: &[textquest_common::combat::BuffInfo],
) -> Vec<&'static DebuffEntry> {
    classify_debuffs(buffs)
        .into_iter()
        .filter(|entry| entry.is_high_priority())
        .collect()
}

/// Count debuffs by type.
#[must_use]
pub fn count_by_type(buffs: &[textquest_common::combat::BuffInfo]) -> HashMap<DebuffType, usize> {
    let mut counts = HashMap::new();
    for entry in classify_debuffs(buffs) {
        *counts.entry(entry.debuff_type).or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── DebuffType Tests ──────────────────────────────────────────────────

    #[test]
    fn debuff_type_crowd_control_as_str() {
        assert_eq!(DebuffType::CrowdControl.as_str(), "Crowd Control");
    }

    #[test]
    fn debuff_type_disease_as_str() {
        assert_eq!(DebuffType::Disease.as_str(), "Disease");
    }

    #[test]
    fn debuff_type_poison_as_str() {
        assert_eq!(DebuffType::Poison.as_str(), "Poison");
    }

    #[test]
    fn debuff_type_curse_as_str() {
        assert_eq!(DebuffType::Curse.as_str(), "Curse");
    }

    // ─── DispelType Tests ──────────────────────────────────────────────────

    #[test]
    fn dispel_type_cure_poison_as_str() {
        assert_eq!(DispelType::CurePoison.as_str(), "Cure Poison");
    }

    #[test]
    fn dispel_type_cure_disease_as_str() {
        assert_eq!(DispelType::CureDisease.as_str(), "Cure Disease");
    }

    #[test]
    fn dispel_type_remove_curse_as_str() {
        assert_eq!(DispelType::RemoveCurse.as_str(), "Remove Curse");
    }

    #[test]
    fn dispel_type_stun_breaker_as_str() {
        assert_eq!(DispelType::StunBreaker.as_str(), "Stun Breaker");
    }

    #[test]
    fn dispel_type_mez_breaker_as_str() {
        assert_eq!(DispelType::MezBreaker.as_str(), "Mez Breaker");
    }

    #[test]
    fn dispel_type_snare_breaker_as_str() {
        assert_eq!(DispelType::SnareBreaker.as_str(), "Snare Breaker");
    }

    #[test]
    fn dispel_type_incurable_as_str() {
        assert_eq!(DispelType::Incurable.as_str(), "Incurable");
    }

    // ─── DebuffEntry Tests ────────────────────────────────────────────────

    #[test]
    fn debuff_entry_new() {
        let entry = DebuffEntry::new(
            1234,
            "Test Stun",
            DebuffType::CrowdControl,
            6,
            DispelType::StunBreaker,
        );
        assert_eq!(entry.spell_id, 1234);
        assert_eq!(entry.name, "Test Stun");
        assert_eq!(entry.debuff_type, DebuffType::CrowdControl);
        assert_eq!(entry.duration_seconds, 6);
        assert_eq!(entry.dispel_type, DispelType::StunBreaker);
    }

    #[test]
    fn debuff_entry_is_permanent_true() {
        let entry = DebuffEntry::new(
            1500,
            "Permanent Curse",
            DebuffType::Curse,
            0,
            DispelType::RemoveCurse,
        );
        assert!(entry.is_permanent());
    }

    #[test]
    fn debuff_entry_is_permanent_false() {
        let entry = DebuffEntry::new(
            1234,
            "Temporary Stun",
            DebuffType::CrowdControl,
            6,
            DispelType::StunBreaker,
        );
        assert!(!entry.is_permanent());
    }

    #[test]
    fn debuff_entry_is_high_priority_for_crowd_control() {
        let entry = DebuffEntry::new(
            1234,
            "Stun",
            DebuffType::CrowdControl,
            3,
            DispelType::StunBreaker,
        );
        assert!(entry.is_high_priority());
    }

    #[test]
    fn debuff_entry_is_high_priority_for_curse() {
        let entry = DebuffEntry::new(1500, "Curse", DebuffType::Curse, 0, DispelType::RemoveCurse);
        assert!(entry.is_high_priority());
    }

    #[test]
    fn debuff_entry_is_high_priority_false_for_disease() {
        let entry = DebuffEntry::new(
            1300,
            "Plague",
            DebuffType::Disease,
            120,
            DispelType::CureDisease,
        );
        assert!(!entry.is_high_priority());
    }

    #[test]
    fn debuff_entry_is_high_priority_false_for_poison() {
        let entry = DebuffEntry::new(
            1400,
            "Poison",
            DebuffType::Poison,
            60,
            DispelType::CurePoison,
        );
        assert!(!entry.is_high_priority());
    }

    // ─── Database Tests ────────────────────────────────────────────────────

    #[test]
    fn lookup_debuff_finds_stun() {
        let entry = lookup_debuff(1239);
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.name, "Stun");
        assert_eq!(entry.debuff_type, DebuffType::CrowdControl);
    }

    #[test]
    fn lookup_debuff_finds_plague() {
        let entry = lookup_debuff(1300);
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.name, "Plague");
        assert_eq!(entry.debuff_type, DebuffType::Disease);
    }

    #[test]
    fn lookup_debuff_finds_poison() {
        let entry = lookup_debuff(1400);
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.name, "Poison");
        assert_eq!(entry.debuff_type, DebuffType::Poison);
    }

    #[test]
    fn lookup_debuff_finds_curse() {
        let entry = lookup_debuff(1500);
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.name, "Curse");
        assert_eq!(entry.debuff_type, DebuffType::Curse);
    }

    #[test]
    fn lookup_debuff_unknown_spell_returns_none() {
        let entry = lookup_debuff(99999);
        assert!(entry.is_none());
    }

    #[test]
    fn lookup_debuff_negative_spell_id_returns_none() {
        let entry = lookup_debuff(-1);
        assert!(entry.is_none());
    }

    // ─── Classification Tests ──────────────────────────────────────────────

    #[test]
    fn classify_debuffs_empty_list() {
        let buffs = vec![];
        let debuffs = classify_debuffs(&buffs);
        assert!(debuffs.is_empty());
    }

    #[test]
    fn classify_debuffs_with_known_debuff() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![textquest_common::combat::BuffInfo {
            spell_id: 1239, // Stun
            duration_ticks: 18,
            initial_duration: 18,
            hit_count: 0,
            category: BuffCategory::LongBuff,
            caster_level: 65,
            slot_index: 0,
        }];

        let debuffs = classify_debuffs(&buffs);
        assert_eq!(debuffs.len(), 1);
        assert_eq!(debuffs[0].name, "Stun");
    }

    #[test]
    fn classify_debuffs_mixed_known_and_unknown() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![
            textquest_common::combat::BuffInfo {
                spell_id: 1239, // Stun (known)
                duration_ticks: 18,
                initial_duration: 18,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 0,
            },
            textquest_common::combat::BuffInfo {
                spell_id: 99999, // Unknown
                duration_ticks: 10,
                initial_duration: 10,
                hit_count: 0,
                category: BuffCategory::ShortBuff,
                caster_level: 65,
                slot_index: 1,
            },
        ];

        let debuffs = classify_debuffs(&buffs);
        assert_eq!(debuffs.len(), 1);
        assert_eq!(debuffs[0].spell_id, 1239);
    }

    #[test]
    fn high_priority_debuffs_filters_correctly() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![
            textquest_common::combat::BuffInfo {
                spell_id: 1239, // Stun (high priority)
                duration_ticks: 18,
                initial_duration: 18,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 0,
            },
            textquest_common::combat::BuffInfo {
                spell_id: 1300, // Plague (not high priority)
                duration_ticks: 720,
                initial_duration: 720,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 1,
            },
            textquest_common::combat::BuffInfo {
                spell_id: 1500, // Curse (high priority)
                duration_ticks: 0,
                initial_duration: 0,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 2,
            },
        ];

        let high_priority = high_priority_debuffs(&buffs);
        assert_eq!(high_priority.len(), 2);
        assert!(high_priority.iter().any(|e| e.spell_id == 1239));
        assert!(high_priority.iter().any(|e| e.spell_id == 1500));
    }

    #[test]
    fn count_by_type_single_debuff() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![textquest_common::combat::BuffInfo {
            spell_id: 1300, // Plague (Disease)
            duration_ticks: 720,
            initial_duration: 720,
            hit_count: 0,
            category: BuffCategory::LongBuff,
            caster_level: 65,
            slot_index: 0,
        }];

        let counts = count_by_type(&buffs);
        assert_eq!(counts.get(&DebuffType::Disease), Some(&1));
        assert_eq!(counts.get(&DebuffType::Poison), None);
    }

    #[test]
    fn count_by_type_multiple_same_type() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![
            textquest_common::combat::BuffInfo {
                spell_id: 1300, // Plague (Disease)
                duration_ticks: 720,
                initial_duration: 720,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 0,
            },
            textquest_common::combat::BuffInfo {
                spell_id: 1301, // Plague of Insects (Disease)
                duration_ticks: 360,
                initial_duration: 360,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 1,
            },
        ];

        let counts = count_by_type(&buffs);
        assert_eq!(counts.get(&DebuffType::Disease), Some(&2));
    }

    #[test]
    fn count_by_type_mixed_types() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![
            textquest_common::combat::BuffInfo {
                spell_id: 1239, // Stun (CC)
                duration_ticks: 18,
                initial_duration: 18,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 0,
            },
            textquest_common::combat::BuffInfo {
                spell_id: 1300, // Plague (Disease)
                duration_ticks: 720,
                initial_duration: 720,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 1,
            },
            textquest_common::combat::BuffInfo {
                spell_id: 1400, // Poison
                duration_ticks: 360,
                initial_duration: 360,
                hit_count: 0,
                category: BuffCategory::ShortBuff,
                caster_level: 65,
                slot_index: 2,
            },
        ];

        let counts = count_by_type(&buffs);
        assert_eq!(counts.get(&DebuffType::CrowdControl), Some(&1));
        assert_eq!(counts.get(&DebuffType::Disease), Some(&1));
        assert_eq!(counts.get(&DebuffType::Poison), Some(&1));
    }

    #[test]
    fn count_by_type_with_unknown_debuffs() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![
            textquest_common::combat::BuffInfo {
                spell_id: 1239, // Stun (known)
                duration_ticks: 18,
                initial_duration: 18,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 0,
            },
            textquest_common::combat::BuffInfo {
                spell_id: 99999, // Unknown
                duration_ticks: 10,
                initial_duration: 10,
                hit_count: 0,
                category: BuffCategory::ShortBuff,
                caster_level: 65,
                slot_index: 1,
            },
        ];

        let counts = count_by_type(&buffs);
        assert_eq!(counts.get(&DebuffType::CrowdControl), Some(&1));
        assert_eq!(counts.len(), 1); // Only 1 type counted (unknown ignored)
    }
}
