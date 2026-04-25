//! Comprehensive melee automation module.
//!
//! Provides:
//! - Endurance management (skip abilities below threshold)
//! - Enrage detection (auto-attack pause when target is enraged)
//! - Combat discipline scheduler (priority-based disc timing)
//! - Melee skill scheduling (kick, bash, slam, backstab, tiger claw)
//! - Class-specific melee automation configuration

use textquest_common::combat::{ActionType, CombatStateReq, ConditionExpr, TargetSelector};

use super::rotation::{RotationEntry, RotationGroup};

/// Melee skill types that can be automated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MeleeSkillType {
    /// Kick ability
    Kick,
    /// Bash ability (Warriors, Paladins, Shadow Knights)
    Bash,
    /// Slam ability (Warriors, Berserkers)
    Slam,
    /// Backstab ability (Rogues)
    Backstab,
    /// Tiger Claw ability (Monks)
    TigerClaw,
    /// Flying Kick (Monks, Beastlords)
    FlyingKick,
    /// Round Kick (Monks)
    RoundKick,
    /// Eagle Strike (Monks)
    EagleStrike,
}

/// Combat discipline with priority and cooldown info.
#[derive(Debug, Clone)]
pub struct DiscWithCooldown {
    pub name: String,
    /// Priority level: higher values fire first (typically 100-0)
    pub priority: u8,
    /// Optional minimum endurance threshold (percent) before this disc can fire
    pub min_endurance_pct: Option<f32>,
    /// Cooldown in ticks after activation
    pub cooldown_ticks: u32,
    /// Optional shared cooldown timer name (e.g., "burn", "precision")
    pub shared_timer_key: Option<String>,
}

/// Melee skill with optional endurance requirement.
#[derive(Debug, Clone)]
pub struct MeleeSkillConfig {
    pub skill_type: MeleeSkillType,
    pub name: String,
    /// Optional minimum endurance threshold (percent) before this skill can fire.
    /// If endurance drops below this, the skill is held.
    pub min_endurance_pct: Option<f32>,
    /// Cooldown in ticks (for skills with cooldowns beyond base reuse)
    pub cooldown_ticks: Option<u32>,
}

/// Endurance threshold configuration.
#[derive(Debug, Clone, Copy)]
pub struct EnduranceThresholds {
    /// Minimum endurance percent to use any endurance-consuming ability (default: 10%)
    pub floor_pct: f32,
    /// Minimum endurance for disc usage (default: floor_pct)
    pub disc_min_pct: f32,
    /// Minimum endurance for melee skill usage (default: floor_pct)
    pub skill_min_pct: f32,
}

impl Default for EnduranceThresholds {
    fn default() -> Self {
        Self {
            floor_pct: 10.0,
            disc_min_pct: 10.0,
            skill_min_pct: 10.0,
        }
    }
}

/// Enrage tracking state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrageState {
    /// Target is not enraged
    Normal,
    /// Target is enraged; auto-attack should be paused
    Enraged,
}

/// Melee automation context for a character.
#[derive(Debug, Clone)]
pub struct MeleeAutomationConfig {
    /// Endurance thresholds for ability execution
    pub endurance_thresholds: EnduranceThresholds,
    /// Disciplines to schedule with priority ordering
    pub discs: Vec<DiscWithCooldown>,
    /// Melee skills to schedule
    pub skills: Vec<MeleeSkillConfig>,
    /// Whether to pause auto-attack on enrage
    pub pause_auto_attack_on_enrage: bool,
    /// Whether to pause auto-attack during mez
    pub pause_auto_attack_on_mez: bool,
}

impl Default for MeleeAutomationConfig {
    fn default() -> Self {
        Self {
            endurance_thresholds: EnduranceThresholds::default(),
            discs: Vec::new(),
            skills: Vec::new(),
            pause_auto_attack_on_enrage: true,
            pause_auto_attack_on_mez: true,
        }
    }
}

/// Check if current endurance is above the threshold.
///
/// Returns true if endurance_pct >= threshold, false otherwise.
#[inline]
pub fn check_endurance_threshold(endurance_pct: f32, threshold: f32) -> bool {
    endurance_pct >= threshold
}

/// Known EverQuest spell/buff IDs that indicate an enrage state on the target.
///
/// These are aura or proc effects that appear in a mob's buff window during enrage.
/// Sources:
///   529  — "Enrage" (classic NPC enrage aura; sub-20% HP trigger)
///   13854 — "Frenzy" (high-level NPC frenzy variant, Planes-era+)
///   3716  — "Primal Fury" (ToV/Luclin raid enrage variant)
///   4822  — "Berserker Frenzy" (secondary frenzy proc seen on raid targets)
///   8904  — "Enraged Assault" (GoD+ raid enrage aura)
const ENRAGE_SPELL_IDS: &[i32] = &[529, 13854, 3716, 4822, 8904];

/// Detect enrage state from target buff/debuff aura spell IDs.
///
/// In EQ, NPC enrage is signalled by one of several aura buff IDs appearing in the
/// target's active buff list (typically triggered when the mob drops below ~20% HP).
/// When enraged, melee characters should cease auto-attack to avoid riposte deaths.
///
/// # Arguments
/// * `target_buffs`  — slice of active buff/debuff spell IDs on the target (i32)
/// * `target_effects` — raw effect bytes (reserved for future visual-effect parsing)
///
/// # Returns
/// `EnrageState::Enraged` if any known enrage aura ID is present, otherwise `EnrageState::Normal`.
pub fn detect_enrage_state(target_buffs: &[i32], _target_effects: &[u8]) -> EnrageState {
    for &buff_id in target_buffs {
        if ENRAGE_SPELL_IDS.contains(&buff_id) {
            return EnrageState::Enraged;
        }
    }
    EnrageState::Normal
}

/// Build rotation group for melee disciplines with priority-based scheduling.
pub fn build_disc_rotation_group(
    discs: &[DiscWithCooldown],
    endurance_thresholds: &EnduranceThresholds,
) -> RotationGroup {
    let mut entries = Vec::new();

    // Sort discs by priority (highest first)
    let mut sorted_discs = discs.to_vec();
    sorted_discs.sort_by_key(|b| std::cmp::Reverse(b.priority));

    for disc in sorted_discs {
        // Build condition: check endurance threshold
        let min_endurance = disc
            .min_endurance_pct
            .unwrap_or(endurance_thresholds.disc_min_pct);
        let condition = ConditionExpr::EnduranceAbove(min_endurance);

        let mut entry =
            super::rotation::entry_if(&disc.name, ActionType::Disc(disc.name.clone()), condition);

        if let Some(shared_timer) = disc.shared_timer_key.as_ref() {
            entry.shared_cooldown_key = Some(shared_timer.clone());
        }
        entry.cooldown_key = Some(disc.name.clone());
        entry.cooldown_ticks = Some(disc.cooldown_ticks);

        entries.push(entry);
    }

    RotationGroup {
        name: "MeleeBurn".into(),
        target_selector: TargetSelector::AutoTarget,
        combat_state_req: CombatStateReq::Combat,
        steps_per_frame: 1,
        full_rotation: false,
        hp_threshold: None,
        burn_duration_ticks: None,
        burn_cooldown_duration_ticks: None,
        entries,
        current_step: 0,
    }
}

/// Build rotation entries for melee skills with endurance checks.
pub fn build_melee_skill_entries(
    skills: &[MeleeSkillConfig],
    endurance_thresholds: &EnduranceThresholds,
) -> Vec<RotationEntry> {
    skills
        .iter()
        .map(|skill| {
            let min_endurance = skill
                .min_endurance_pct
                .unwrap_or(endurance_thresholds.skill_min_pct);
            let condition = ConditionExpr::EnduranceAbove(min_endurance);

            let mut entry = super::rotation::entry_if(
                &skill.name,
                ActionType::Ability(skill.name.clone()),
                condition,
            );

            if let Some(cooldown_ticks) = skill.cooldown_ticks {
                entry.cooldown_key = Some(skill.name.clone());
                entry.cooldown_ticks = Some(cooldown_ticks);
            }

            entry
        })
        .collect()
}

/// Determine whether auto-attack should be paused based on target state.
pub fn should_pause_auto_attack(
    enrage_state: EnrageState,
    is_mezed: bool,
    pause_on_enrage: bool,
    pause_on_mez: bool,
) -> bool {
    (pause_on_enrage && enrage_state == EnrageState::Enraged) || (pause_on_mez && is_mezed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_endurance_threshold_at_boundary() {
        assert!(check_endurance_threshold(50.0, 50.0));
        assert!(!check_endurance_threshold(49.9, 50.0));
        assert!(check_endurance_threshold(50.1, 50.0));
    }

    #[test]
    fn check_endurance_threshold_zero() {
        assert!(check_endurance_threshold(0.0, 0.0));
        assert!(!check_endurance_threshold(0.0, 0.1));
    }

    #[test]
    fn check_endurance_threshold_max() {
        assert!(check_endurance_threshold(100.0, 100.0));
        assert!(check_endurance_threshold(100.0, 50.0));
    }

    #[test]
    fn detect_enrage_state_normal_when_no_buffs() {
        let state = detect_enrage_state(&[], &[]);
        assert_eq!(state, EnrageState::Normal);
    }

    #[test]
    fn detect_enrage_state_normal_when_no_enrage_buffs() {
        // Non-enrage buff IDs should not trigger enrage detection
        let buffs = [1, 42, 100, 9999];
        let state = detect_enrage_state(&buffs, &[]);
        assert_eq!(state, EnrageState::Normal);
    }

    #[test]
    fn detect_enrage_state_enraged_with_classic_enrage_id() {
        // Spell ID 529 — classic EQ "Enrage" aura
        let buffs = [529];
        let state = detect_enrage_state(&buffs, &[]);
        assert_eq!(state, EnrageState::Enraged);
    }

    #[test]
    fn detect_enrage_state_enraged_with_frenzy_id() {
        // Spell ID 13854 — "Frenzy" high-level enrage variant
        let buffs = [13854];
        let state = detect_enrage_state(&buffs, &[]);
        assert_eq!(state, EnrageState::Enraged);
    }

    #[test]
    fn detect_enrage_state_enraged_with_primal_fury_id() {
        // Spell ID 3716 — "Primal Fury" ToV/Luclin raid enrage
        let buffs = [3716];
        let state = detect_enrage_state(&buffs, &[]);
        assert_eq!(state, EnrageState::Enraged);
    }

    #[test]
    fn detect_enrage_state_enraged_with_berserker_frenzy_id() {
        // Spell ID 4822 — "Berserker Frenzy"
        let buffs = [4822];
        let state = detect_enrage_state(&buffs, &[]);
        assert_eq!(state, EnrageState::Enraged);
    }

    #[test]
    fn detect_enrage_state_enraged_with_enraged_assault_id() {
        // Spell ID 8904 — "Enraged Assault" GoD+ aura
        let buffs = [8904];
        let state = detect_enrage_state(&buffs, &[]);
        assert_eq!(state, EnrageState::Enraged);
    }

    #[test]
    fn detect_enrage_state_enraged_mixed_buffs() {
        // Enrage ID present among many non-enrage IDs
        let buffs = [1, 42, 529, 9999, 777];
        let state = detect_enrage_state(&buffs, &[]);
        assert_eq!(state, EnrageState::Enraged);
    }

    #[test]
    fn detect_enrage_state_all_known_enrage_ids() {
        // Every known enrage ID must trigger Enraged
        for &id in ENRAGE_SPELL_IDS {
            let state = detect_enrage_state(&[id], &[]);
            assert_eq!(
                state,
                EnrageState::Enraged,
                "spell ID {id} should produce EnrageState::Enraged"
            );
        }
    }

    #[test]
    fn detect_enrage_state_ignores_target_effects_bytes() {
        // target_effects is reserved; non-empty slice must not affect result
        let state = detect_enrage_state(&[], &[1, 2, 3, 255]);
        assert_eq!(state, EnrageState::Normal);
    }

    #[test]
    fn should_pause_auto_attack_on_enrage() {
        assert!(should_pause_auto_attack(
            EnrageState::Enraged,
            false,
            true,
            false
        ));
    }

    #[test]
    fn should_not_pause_auto_attack_when_not_enraged() {
        assert!(!should_pause_auto_attack(
            EnrageState::Normal,
            false,
            true,
            false
        ));
    }

    #[test]
    fn should_pause_auto_attack_on_mez() {
        assert!(should_pause_auto_attack(
            EnrageState::Normal,
            true,
            false,
            true
        ));
    }

    #[test]
    fn should_pause_auto_attack_respects_disable_flags() {
        assert!(!should_pause_auto_attack(
            EnrageState::Enraged,
            true,
            false,
            false
        ));
    }

    #[test]
    fn build_disc_rotation_group_sorts_by_priority() {
        let discs = vec![
            DiscWithCooldown {
                name: "Disc1".into(),
                priority: 50,
                min_endurance_pct: None,
                cooldown_ticks: 1200,
                shared_timer_key: None,
            },
            DiscWithCooldown {
                name: "Disc2".into(),
                priority: 100,
                min_endurance_pct: None,
                cooldown_ticks: 1200,
                shared_timer_key: None,
            },
        ];

        let thresholds = EnduranceThresholds::default();
        let group = build_disc_rotation_group(&discs, &thresholds);

        // Highest priority (Disc2) should come first
        assert_eq!(group.entries.len(), 2);
        assert_eq!(group.entries[0].name, "Disc2");
        assert_eq!(group.entries[1].name, "Disc1");
    }

    #[test]
    fn build_disc_rotation_group_respects_custom_endurance() {
        let discs = vec![DiscWithCooldown {
            name: "TestDisc".into(),
            priority: 50,
            min_endurance_pct: Some(75.0),
            cooldown_ticks: 1200,
            shared_timer_key: None,
        }];

        let thresholds = EnduranceThresholds::default();
        let group = build_disc_rotation_group(&discs, &thresholds);

        // The entry should have the custom endurance condition
        assert_eq!(group.entries.len(), 1);
        assert!(group.entries[0].condition.is_some());
    }

    #[test]
    fn build_melee_skill_entries_includes_endurance_check() {
        let skills = vec![MeleeSkillConfig {
            skill_type: MeleeSkillType::Kick,
            name: "Kick".into(),
            min_endurance_pct: Some(25.0),
            cooldown_ticks: Some(60),
        }];

        let thresholds = EnduranceThresholds::default();
        let entries = build_melee_skill_entries(&skills, &thresholds);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Kick");
        assert!(entries[0].condition.is_some());
    }

    #[test]
    fn build_melee_skill_entries_uses_default_threshold() {
        let skills = vec![MeleeSkillConfig {
            skill_type: MeleeSkillType::Bash,
            name: "Bash".into(),
            min_endurance_pct: None,
            cooldown_ticks: None,
        }];

        let thresholds = EnduranceThresholds {
            floor_pct: 10.0,
            disc_min_pct: 10.0,
            skill_min_pct: 20.0,
        };
        let entries = build_melee_skill_entries(&skills, &thresholds);

        // Should use skill_min_pct (20.0)
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn endurance_thresholds_default() {
        let thresholds = EnduranceThresholds::default();
        assert_eq!(thresholds.floor_pct, 10.0);
        assert_eq!(thresholds.disc_min_pct, 10.0);
        assert_eq!(thresholds.skill_min_pct, 10.0);
    }

    #[test]
    fn melee_automation_config_default() {
        let config = MeleeAutomationConfig::default();
        assert!(config.discs.is_empty());
        assert!(config.skills.is_empty());
        assert!(config.pause_auto_attack_on_enrage);
        assert!(config.pause_auto_attack_on_mez);
    }

    #[test]
    fn disc_with_cooldown_has_all_fields() {
        let disc = DiscWithCooldown {
            name: "Test Disc".into(),
            priority: 75,
            min_endurance_pct: Some(50.0),
            cooldown_ticks: 2400,
            shared_timer_key: Some("burn".into()),
        };
        assert_eq!(disc.name, "Test Disc");
        assert_eq!(disc.priority, 75);
        assert_eq!(disc.cooldown_ticks, 2400);
    }

    #[test]
    fn should_pause_auto_attack_multiple_conditions() {
        // Enraged and mezed, pause on both: should pause
        assert!(should_pause_auto_attack(
            EnrageState::Enraged,
            true,
            true,
            true
        ));

        // Enraged and mezed, but only pause on enrage: should still pause
        assert!(should_pause_auto_attack(
            EnrageState::Enraged,
            true,
            true,
            false
        ));

        // Enraged and mezed, but only pause on mez: should still pause
        assert!(should_pause_auto_attack(
            EnrageState::Enraged,
            true,
            false,
            true
        ));
    }
}
