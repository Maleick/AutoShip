//! Dispel and CC mitigation engine.
//!
//! Provides dispel ability database, priority ranking, target selection,
//! and recommendations for removing crowd control and other detrimental effects.

use once_cell::sync::Lazy;
use std::collections::HashMap;

use crate::combat::debuffs::{DebuffType, DispelType, lookup_debuff};
use textquest_common::combat::BuffInfo;

/// A dispel ability that can remove debuffs.
///
/// Contains metadata about an ability that cures or removes debuffs,
/// including what dispel types it handles and its mana/resource cost.
#[derive(Debug, Clone)]
pub struct DispelAbility {
    /// Spell ID of the dispel ability
    pub spell_id: i32,
    /// Human-readable name (e.g., "Cure Poison", "Remove Curse")
    pub name: &'static str,
    /// Which debuff types this ability can cure
    pub dispel_types: &'static [DispelType],
    /// Approximate mana cost
    pub mana_cost: u32,
    /// Approximate recast time in seconds
    pub recast_seconds: u32,
}

impl DispelAbility {
    /// Create a new dispel ability.
    pub const fn new(
        spell_id: i32,
        name: &'static str,
        dispel_types: &'static [DispelType],
        mana_cost: u32,
        recast_seconds: u32,
    ) -> Self {
        DispelAbility {
            spell_id,
            name,
            dispel_types,
            mana_cost,
            recast_seconds,
        }
    }

    /// Check if this ability can cure a specific dispel type.
    #[must_use]
    pub fn can_cure(&self, dispel_type: DispelType) -> bool {
        self.dispel_types.contains(&dispel_type)
    }
}

/// Static database of dispel abilities indexed by spell ID.
static DISPEL_ABILITIES: Lazy<HashMap<i32, DispelAbility>> = Lazy::new(|| {
    let abilities = vec![
        // ─── Poison Cures ──────────────────────────────────────────────────────
        DispelAbility::new(2500, "Cure Poison", &[DispelType::CurePoison], 50, 6),
        DispelAbility::new(2501, "Antidote", &[DispelType::CurePoison], 75, 6),
        // ─── Disease Cures ────────────────────────────────────────────────────────
        DispelAbility::new(2600, "Cure Disease", &[DispelType::CureDisease], 50, 6),
        DispelAbility::new(
            2601,
            "Cure Curse",
            &[DispelType::CureDisease, DispelType::RemoveCurse],
            75,
            6,
        ),
        // ─── Remove Curse ────────────────────────────────────────────────────────
        DispelAbility::new(2700, "Remove Curse", &[DispelType::RemoveCurse], 100, 12),
        // ─── Stun Breakers ────────────────────────────────────────────────────────
        DispelAbility::new(2800, "Stun Breaker", &[DispelType::StunBreaker], 60, 6),
        DispelAbility::new(
            2801,
            "Resilience",
            &[DispelType::StunBreaker, DispelType::MezBreaker],
            80,
            12,
        ),
        // ─── Mez Breakers ────────────────────────────────────────────────────────
        DispelAbility::new(2900, "Awaken", &[DispelType::MezBreaker], 60, 6),
        // ─── Snare Breakers ────────────────────────────────────────────────────────
        DispelAbility::new(3000, "Freedom", &[DispelType::SnareBreaker], 50, 6),
    ];

    let mut map = HashMap::new();
    for ability in abilities {
        map.insert(ability.spell_id, ability);
    }
    map
});

/// Look up a dispel ability by spell ID.
///
/// Returns `Some(ability)` if the spell ID exists in the database,
/// or `None` if unknown.
#[must_use]
pub fn lookup_dispel_ability(spell_id: i32) -> Option<&'static DispelAbility> {
    DISPEL_ABILITIES.get(&spell_id)
}

/// Get all dispel abilities that can cure a specific dispel type.
#[must_use]
pub fn abilities_for_dispel_type(dispel_type: DispelType) -> Vec<&'static DispelAbility> {
    DISPEL_ABILITIES
        .values()
        .filter(|ability| ability.can_cure(dispel_type))
        .collect()
}

/// A debuff with its priority rank.
///
/// Used internally for sorting debuffs by priority.
#[derive(Debug, Clone)]
struct PrioritizedDebuff {
    /// The debuff entry
    debuff: &'static crate::combat::debuffs::DebuffEntry,
    /// Priority score (higher = more urgent)
    priority_score: u32,
}

/// Dispel priority ranking.
///
/// Prioritizes debuffs for removal. Higher scores = higher priority.
/// Priority order: Crowd Control > Curses > Diseases > Poisons
#[derive(Debug, Clone)]
pub struct DispelPriority;

impl DispelPriority {
    /// Rank a debuff entry by priority.
    ///
    /// Returns a score from 0 (lowest priority) to 1000 (highest).
    /// Higher scores indicate more urgent removal.
    #[must_use]
    pub fn rank_debuff(entry: &crate::combat::debuffs::DebuffEntry) -> u32 {
        match entry.debuff_type {
            // Crowd control: highest priority (900+)
            DebuffType::CrowdControl => 950,
            // Curses: high priority (800-899)
            DebuffType::Curse => 850,
            // Diseases: medium priority (600-699)
            DebuffType::Disease => 650,
            // Poisons: lower priority (500-599)
            DebuffType::Poison => 550,
        }
    }

    /// Rank a list of debuffs and return them sorted by priority (highest first).
    #[must_use]
    pub fn rank_debuffs(buffs: &[BuffInfo]) -> Vec<&'static crate::combat::debuffs::DebuffEntry> {
        let mut prioritized: Vec<PrioritizedDebuff> = buffs
            .iter()
            .filter_map(|buff| {
                lookup_debuff(buff.spell_id).map(|debuff| PrioritizedDebuff {
                    debuff,
                    priority_score: Self::rank_debuff(debuff),
                })
            })
            .collect();

        // Sort by priority score descending (highest first)
        prioritized.sort_by(|a, b| b.priority_score.cmp(&a.priority_score));

        prioritized.into_iter().map(|p| p.debuff).collect()
    }
}

/// Target selection for dispel operations.
///
/// Analyzes buff lists to identify which debuffs need removal.
#[derive(Debug, Clone)]
pub struct DispelTargetSelector;

impl DispelTargetSelector {
    /// Count debuffs in a buff list.
    #[must_use]
    pub fn count_debuffs(buffs: &[BuffInfo]) -> usize {
        buffs
            .iter()
            .filter(|buff| lookup_debuff(buff.spell_id).is_some())
            .count()
    }

    /// Count crowd control debuffs in a buff list.
    #[must_use]
    pub fn count_cc_debuffs(buffs: &[BuffInfo]) -> usize {
        buffs
            .iter()
            .filter(|buff| {
                lookup_debuff(buff.spell_id)
                    .map(|entry| entry.debuff_type == DebuffType::CrowdControl)
                    .unwrap_or(false)
            })
            .count()
    }

    /// Check if a buff list contains any debuffs.
    #[must_use]
    pub fn has_debuffs(buffs: &[BuffInfo]) -> bool {
        buffs
            .iter()
            .any(|buff| lookup_debuff(buff.spell_id).is_some())
    }
}

/// Dispel recommendation for a specific debuff.
///
/// Suggests which dispel ability to use for a debuff.
#[derive(Debug, Clone)]
pub struct DispelRecommendation {
    /// Debuff to remove
    pub debuff_name: String,
    /// Recommended dispel ability
    pub ability_name: String,
    /// Urgency level (0-100, where 100 is critical)
    pub urgency: u32,
}

/// Engine for generating dispel recommendations.
#[derive(Debug)]
pub struct DispelRecommendationEngine;

impl DispelRecommendationEngine {
    /// Generate a dispel recommendation for the highest-priority debuff in a buff list.
    ///
    /// Returns the top recommendation, or None if no debuffs found.
    #[must_use]
    pub fn recommend_dispel(buffs: &[BuffInfo]) -> Option<DispelRecommendation> {
        // Find highest-priority debuff
        let priority_ranked = DispelPriority::rank_debuffs(buffs);
        let debuff = priority_ranked.first()?;

        // Find a suitable dispel ability
        let abilities = abilities_for_dispel_type(debuff.dispel_type);
        let ability = abilities.first()?;

        let urgency = match debuff.debuff_type {
            DebuffType::CrowdControl => 95,
            DebuffType::Curse => 80,
            DebuffType::Disease => 60,
            DebuffType::Poison => 40,
        };

        Some(DispelRecommendation {
            debuff_name: debuff.name.to_string(),
            ability_name: ability.name.to_string(),
            urgency,
        })
    }

    /// Generate all dispel recommendations for debuffs in a buff list, sorted by urgency.
    pub fn recommend_all_dispels(buffs: &[BuffInfo]) -> Vec<DispelRecommendation> {
        let mut recommendations = Vec::new();

        let priority_ranked = DispelPriority::rank_debuffs(buffs);

        for debuff in priority_ranked {
            if let Some(ability) = abilities_for_dispel_type(debuff.dispel_type).first() {
                let urgency = match debuff.debuff_type {
                    DebuffType::CrowdControl => 95,
                    DebuffType::Curse => 80,
                    DebuffType::Disease => 60,
                    DebuffType::Poison => 40,
                };

                recommendations.push(DispelRecommendation {
                    debuff_name: debuff.name.to_string(),
                    ability_name: ability.name.to_string(),
                    urgency,
                });
            }
        }

        // Sort by urgency descending
        recommendations.sort_by(|a, b| b.urgency.cmp(&a.urgency));
        recommendations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── DispelAbility Tests ────────────────────────────────────────────────

    #[test]
    fn dispel_ability_new() {
        let ability = DispelAbility::new(2500, "Cure Poison", &[DispelType::CurePoison], 50, 6);
        assert_eq!(ability.spell_id, 2500);
        assert_eq!(ability.name, "Cure Poison");
        assert_eq!(ability.mana_cost, 50);
        assert_eq!(ability.recast_seconds, 6);
    }

    #[test]
    fn dispel_ability_can_cure_matching_type() {
        let ability = DispelAbility::new(2500, "Cure Poison", &[DispelType::CurePoison], 50, 6);
        assert!(ability.can_cure(DispelType::CurePoison));
        assert!(!ability.can_cure(DispelType::CureDisease));
    }

    #[test]
    fn dispel_ability_multiple_dispel_types() {
        let ability = DispelAbility::new(
            2601,
            "Cure Curse",
            &[DispelType::CureDisease, DispelType::RemoveCurse],
            75,
            6,
        );
        assert!(ability.can_cure(DispelType::CureDisease));
        assert!(ability.can_cure(DispelType::RemoveCurse));
        assert!(!ability.can_cure(DispelType::CurePoison));
    }

    // ─── Dispel Ability Database Tests ──────────────────────────────────────

    #[test]
    fn lookup_dispel_ability_finds_cure_poison() {
        let ability = lookup_dispel_ability(2500);
        assert!(ability.is_some());
        let ability = ability.unwrap();
        assert_eq!(ability.name, "Cure Poison");
        assert!(ability.can_cure(DispelType::CurePoison));
    }

    #[test]
    fn lookup_dispel_ability_finds_remove_curse() {
        let ability = lookup_dispel_ability(2700);
        assert!(ability.is_some());
        let ability = ability.unwrap();
        assert_eq!(ability.name, "Remove Curse");
        assert!(ability.can_cure(DispelType::RemoveCurse));
    }

    #[test]
    fn lookup_dispel_ability_unknown_spell_returns_none() {
        let ability = lookup_dispel_ability(99999);
        assert!(ability.is_none());
    }

    #[test]
    fn abilities_for_dispel_type_cure_poison() {
        let abilities = abilities_for_dispel_type(DispelType::CurePoison);
        assert!(!abilities.is_empty());
        assert!(abilities.iter().any(|a| a.spell_id == 2500));
    }

    #[test]
    fn abilities_for_dispel_type_remove_curse() {
        let abilities = abilities_for_dispel_type(DispelType::RemoveCurse);
        assert!(!abilities.is_empty());
        assert!(abilities.iter().any(|a| a.spell_id == 2700));
    }

    // ─── DispelPriority Tests ───────────────────────────────────────────────

    #[test]
    fn dispel_priority_rank_crowd_control_highest() {
        let entry = crate::combat::debuffs::DebuffEntry::new(
            1234,
            "Stun",
            DebuffType::CrowdControl,
            6,
            DispelType::StunBreaker,
        );
        let score = DispelPriority::rank_debuff(&entry);
        assert_eq!(score, 950);
    }

    #[test]
    fn dispel_priority_rank_curse_high() {
        let entry = crate::combat::debuffs::DebuffEntry::new(
            1500,
            "Curse",
            DebuffType::Curse,
            0,
            DispelType::RemoveCurse,
        );
        let score = DispelPriority::rank_debuff(&entry);
        assert_eq!(score, 850);
    }

    #[test]
    fn dispel_priority_rank_disease_medium() {
        let entry = crate::combat::debuffs::DebuffEntry::new(
            1300,
            "Plague",
            DebuffType::Disease,
            120,
            DispelType::CureDisease,
        );
        let score = DispelPriority::rank_debuff(&entry);
        assert_eq!(score, 650);
    }

    #[test]
    fn dispel_priority_rank_poison_low() {
        let entry = crate::combat::debuffs::DebuffEntry::new(
            1400,
            "Poison",
            DebuffType::Poison,
            60,
            DispelType::CurePoison,
        );
        let score = DispelPriority::rank_debuff(&entry);
        assert_eq!(score, 550);
    }

    #[test]
    fn dispel_priority_rank_debuffs_orders_correctly() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![
            BuffInfo {
                spell_id: 1400, // Poison (lowest priority)
                duration_ticks: 360,
                initial_duration: 360,
                hit_count: 0,
                category: BuffCategory::ShortBuff,
                caster_level: 65,
                slot_index: 0,
            },
            BuffInfo {
                spell_id: 1239, // Stun (highest priority)
                duration_ticks: 18,
                initial_duration: 18,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 1,
            },
            BuffInfo {
                spell_id: 1300, // Plague (medium priority)
                duration_ticks: 720,
                initial_duration: 720,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 2,
            },
        ];

        let ranked = DispelPriority::rank_debuffs(&buffs);
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].spell_id, 1239); // Stun first
        assert_eq!(ranked[1].spell_id, 1300); // Plague second
        assert_eq!(ranked[2].spell_id, 1400); // Poison last
    }

    // ─── DispelTargetSelector Tests ────────────────────────────────────────

    #[test]
    fn target_selector_count_debuffs() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![
            BuffInfo {
                spell_id: 1239, // Stun
                duration_ticks: 18,
                initial_duration: 18,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 0,
            },
            BuffInfo {
                spell_id: 1300, // Plague
                duration_ticks: 720,
                initial_duration: 720,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 1,
            },
        ];

        let count = DispelTargetSelector::count_debuffs(&buffs);
        assert_eq!(count, 2);
    }

    #[test]
    fn target_selector_count_cc_debuffs() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![
            BuffInfo {
                spell_id: 1239, // Stun (CC)
                duration_ticks: 18,
                initial_duration: 18,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 0,
            },
            BuffInfo {
                spell_id: 1300, // Plague (not CC)
                duration_ticks: 720,
                initial_duration: 720,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 1,
            },
        ];

        let count = DispelTargetSelector::count_cc_debuffs(&buffs);
        assert_eq!(count, 1);
    }

    #[test]
    fn target_selector_has_debuffs_true() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![BuffInfo {
            spell_id: 1239, // Known debuff
            duration_ticks: 18,
            initial_duration: 18,
            hit_count: 0,
            category: BuffCategory::LongBuff,
            caster_level: 65,
            slot_index: 0,
        }];

        assert!(DispelTargetSelector::has_debuffs(&buffs));
    }

    #[test]
    fn target_selector_has_debuffs_false() {
        let buffs = vec![];
        assert!(!DispelTargetSelector::has_debuffs(&buffs));
    }

    // ─── DispelRecommendationEngine Tests ────────────────────────────────────

    #[test]
    fn recommendation_engine_recommend_dispel_finds_debuff() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![BuffInfo {
            spell_id: 1239, // Stun
            duration_ticks: 18,
            initial_duration: 18,
            hit_count: 0,
            category: BuffCategory::LongBuff,
            caster_level: 65,
            slot_index: 0,
        }];

        let recommendation = DispelRecommendationEngine::recommend_dispel(&buffs);
        assert!(recommendation.is_some());
        let rec = recommendation.unwrap();
        assert_eq!(rec.debuff_name, "Stun");
    }

    #[test]
    fn recommendation_engine_crowdcontrol_high_urgency() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![BuffInfo {
            spell_id: 1239, // Stun (CC)
            duration_ticks: 18,
            initial_duration: 18,
            hit_count: 0,
            category: BuffCategory::LongBuff,
            caster_level: 65,
            slot_index: 0,
        }];

        let recommendation = DispelRecommendationEngine::recommend_dispel(&buffs);
        assert!(recommendation.is_some());
        let rec = recommendation.unwrap();
        assert!(rec.urgency > 90);
    }

    #[test]
    fn recommendation_engine_recommend_all_dispels() {
        use textquest_common::combat::BuffCategory;

        let buffs = vec![
            BuffInfo {
                spell_id: 1239, // Stun
                duration_ticks: 18,
                initial_duration: 18,
                hit_count: 0,
                category: BuffCategory::LongBuff,
                caster_level: 65,
                slot_index: 0,
            },
            BuffInfo {
                spell_id: 1400, // Poison
                duration_ticks: 360,
                initial_duration: 360,
                hit_count: 0,
                category: BuffCategory::ShortBuff,
                caster_level: 65,
                slot_index: 1,
            },
        ];

        let recommendations = DispelRecommendationEngine::recommend_all_dispels(&buffs);
        assert_eq!(recommendations.len(), 2);
        // Should be sorted by urgency (stun > poison)
        assert!(recommendations[0].urgency > recommendations[1].urgency);
    }
}
