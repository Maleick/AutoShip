//! Launch profiles and session preset translation layer for M8 orchestrator.
//!
//! A **launch profile** is the operator-facing specification that defines:
//! - The ordered character list to launch
//! - Per-character roles and group assignments
//! - Resource allocation and launch sequencing parameters
//!
//! A **session preset** is the internal routing state derived from a profile:
//! - Group membership and routing scope configuration
//! - Session resource allocation
//! - Stable routing metadata for command dispatch
//!
//! This module provides the schema and translation layer from profiles → presets,
//! enabling the TUI to specify launch intent and the orchestrator to route commands.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Character role within a group (defines combat behavior and CC responsibilities).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CharacterRole {
    /// Tank — primary hate target, mitigation focus.
    Tank,
    /// Damage dealer — primary damage output.
    Dps,
    /// Crowd control specialist — holds adds, manages enrage.
    Cc,
    /// Healer — primary or secondary healing.
    Healer,
    /// Support — buffs, debuffs, utilities (enchanters, shamans).
    Support,
    /// Puller — initiates encounters, manages adds.
    Puller,
}

impl CharacterRole {
    /// Returns a stable short label for this role (used in routing/display).
    #[must_use]
    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Tank => "tank",
            Self::Dps => "dps",
            Self::Cc => "cc",
            Self::Healer => "healer",
            Self::Support => "support",
            Self::Puller => "puller",
        }
    }

    /// Returns a human-readable name for this role.
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Tank => "Tank",
            Self::Dps => "Damage Dealer",
            Self::Cc => "Crowd Control",
            Self::Healer => "Healer",
            Self::Support => "Support",
            Self::Puller => "Puller",
        }
    }
}

/// Specification for a single character in a launch profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaunchProfileCharacter {
    /// In-game character name.
    pub name: String,
    /// Numeric group ID (1-6) to assign after launch.
    pub group_id: u8,
    /// Character role for combat coordination and assist routing.
    pub role: CharacterRole,
    /// Optional launch delay (milliseconds) after the previous character.
    /// If `None`, use orchestrator default stagger delay.
    #[serde(default)]
    pub launch_delay_ms: Option<u32>,
    /// Whether to wait for this character to reach the zone before continuing
    /// launches.
    #[serde(default = "default_wait_for_zone")]
    pub wait_for_zone: bool,
}

fn default_wait_for_zone() -> bool {
    false
}

impl LaunchProfileCharacter {
    /// Create a new profile character with minimal fields.
    #[must_use]
    pub fn new(name: impl Into<String>, group_id: u8, role: CharacterRole) -> Self {
        Self {
            name: name.into(),
            group_id,
            role,
            launch_delay_ms: None,
            wait_for_zone: false,
        }
    }

    /// Set a custom launch delay for this character (in milliseconds).
    #[must_use]
    pub fn with_launch_delay_ms(mut self, delay_ms: u32) -> Self {
        self.launch_delay_ms = Some(delay_ms);
        self
    }

    /// Mark this character to wait for zone before continuing launches.
    #[must_use]
    pub fn with_wait_for_zone(mut self, wait: bool) -> Self {
        self.wait_for_zone = wait;
        self
    }
}

/// Operator-facing launch specification for a multi-character session.
///
/// A launch profile defines:
/// - Ordered character list with roles and group assignments
/// - Resource allocation and launch sequence preferences
/// - Session metadata (name, description, target server)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LaunchProfile {
    /// Unique profile identifier (e.g., "raid-main", "box-alt").
    pub id: String,
    /// Human-readable profile name for TUI/API display.
    pub name: String,
    /// Optional description of this profile's purpose (e.g., "Main raid group").
    #[serde(default)]
    pub description: Option<String>,
    /// Target EQ server name (e.g., "Firiona Vie", "Teek").
    pub server_name: String,
    /// Ordered list of characters to launch in this session.
    pub characters: Vec<LaunchProfileCharacter>,
    /// Maximum concurrent client launches (0 = no limit).
    #[serde(default)]
    pub max_concurrent_launches: u32,
    /// Minimum stagger delay between sequential launches (milliseconds).
    #[serde(default = "default_stagger_min_ms")]
    pub stagger_min_ms: u32,
    /// Maximum stagger delay between sequential launches (milliseconds).
    #[serde(default = "default_stagger_max_ms")]
    pub stagger_max_ms: u32,
}

fn default_stagger_min_ms() -> u32 {
    2000
}

fn default_stagger_max_ms() -> u32 {
    5000
}

impl LaunchProfile {
    /// Create a new launch profile with minimal fields.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        server_name: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            server_name: server_name.into(),
            characters: vec![],
            max_concurrent_launches: 0,
            stagger_min_ms: default_stagger_min_ms(),
            stagger_max_ms: default_stagger_max_ms(),
        }
    }

    /// Add a character to this profile.
    #[must_use]
    pub fn with_character(mut self, character: LaunchProfileCharacter) -> Self {
        self.characters.push(character);
        self
    }

    /// Set the description for this profile.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set launch concurrency limits.
    #[must_use]
    pub fn with_launch_limits(
        mut self,
        max_concurrent: u32,
        stagger_min_ms: u32,
        stagger_max_ms: u32,
    ) -> Self {
        self.max_concurrent_launches = max_concurrent;
        self.stagger_min_ms = stagger_min_ms;
        self.stagger_max_ms = stagger_max_ms;
        self
    }

    /// Validate this profile for consistency.
    ///
    /// Returns `Err` if:
    /// - No characters are defined
    /// - Any character has group_id == 0 (reserved for broadcast)
    /// - Group IDs exceed 6 (max EQ group size)
    /// - Stagger delays are misconfigured
    pub fn validate(&self) -> Result<(), String> {
        if self.characters.is_empty() {
            return Err("Launch profile must contain at least one character".to_string());
        }

        for (idx, character) in self.characters.iter().enumerate() {
            if character.group_id == 0 {
                return Err(format!(
                    "Character {} has invalid group_id 0 (reserved for broadcast)",
                    idx
                ));
            }
            if character.group_id > 6 {
                return Err(format!(
                    "Character {} has group_id {} exceeding max of 6",
                    idx, character.group_id
                ));
            }
        }

        if self.stagger_min_ms > self.stagger_max_ms {
            return Err("stagger_min_ms must be <= stagger_max_ms".to_string());
        }

        Ok(())
    }

    /// Returns a summary of groups referenced in this profile.
    ///
    /// Groups are reported in ascending order with a count of members per group.
    #[must_use]
    pub fn group_summary(&self) -> HashMap<u8, usize> {
        let mut summary = HashMap::new();
        for character in &self.characters {
            *summary.entry(character.group_id).or_insert(0) += 1;
        }
        summary
    }
}

/// Internal session preset derived from a launch profile.
///
/// Presets represent the stable orchestrator-level routing state after translation
/// from a profile. Each preset corresponds to one group in the profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionPreset {
    /// Unique preset identifier (profile_id + "_g" + group_id).
    pub id: String,
    /// Group ID this preset routes to (1-6).
    pub group_id: u8,
    /// Stable routing scope label for the TUI status bar (e.g., "G1 Main").
    pub routing_label: String,
    /// Set of character names assigned to this group/preset.
    pub character_names: Vec<String>,
    /// Per-character role assignments for combat coordination.
    pub character_roles: HashMap<String, CharacterRole>,
    /// Resource allocation for this group.
    pub resource_allocation: SessionResourceAllocation,
}

impl SessionPreset {
    /// Create a new session preset.
    #[must_use]
    pub fn new(id: impl Into<String>, group_id: u8, routing_label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            group_id,
            routing_label: routing_label.into(),
            character_names: vec![],
            character_roles: HashMap::new(),
            resource_allocation: SessionResourceAllocation::default(),
        }
    }

    /// Add a character to this preset's roster.
    pub fn add_character(&mut self, name: String, role: CharacterRole) {
        if !self.character_names.contains(&name) {
            self.character_names.push(name.clone());
        }
        self.character_roles.insert(name, role);
    }

    /// Returns the count of characters in this preset.
    #[must_use]
    pub fn character_count(&self) -> usize {
        self.character_names.len()
    }

    /// Check if a character is a member of this preset.
    #[must_use]
    pub fn contains_character(&self, name: &str) -> bool {
        self.character_names.iter().any(|c| c == name)
    }

    /// Get the role for a character, if assigned.
    #[must_use]
    pub fn get_character_role(&self, name: &str) -> Option<CharacterRole> {
        self.character_roles.get(name).copied()
    }

    /// Validate this preset for consistency.
    pub fn validate(&self) -> Result<(), String> {
        if self.group_id == 0 || self.group_id > 6 {
            return Err(format!("Invalid group_id: {}", self.group_id));
        }

        if self.character_names.is_empty() {
            return Err("Session preset must contain at least one character".to_string());
        }

        for name in &self.character_names {
            if !self.character_roles.contains_key(name) {
                return Err(format!("Character {} has no role assigned", name));
            }
        }

        Ok(())
    }
}

/// Resource allocation metadata for a session preset (group).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionResourceAllocation {
    /// Maximum physical memory (working set) per client in this group (MB).
    /// `0` = no limit.
    #[serde(default)]
    pub max_working_set_mb: u32,
    /// Whether this group is configured for active combat.
    #[serde(default = "default_combat_enabled")]
    pub combat_enabled: bool,
    /// Whether this group is configured for autonomous farming/hunting.
    #[serde(default)]
    pub autonomous_enabled: bool,
}

fn default_combat_enabled() -> bool {
    true
}

impl Default for SessionResourceAllocation {
    fn default() -> Self {
        Self {
            max_working_set_mb: 0,
            combat_enabled: true,
            autonomous_enabled: false,
        }
    }
}

/// Translate a `LaunchProfile` into a map of `SessionPreset` records,
/// one per unique group ID found in the profile.
///
/// # Errors
///
/// Returns an error if the profile fails validation or contains invalid data.
pub fn translate_profile_to_presets(
    profile: &LaunchProfile,
) -> Result<HashMap<u8, SessionPreset>, String> {
    profile.validate()?;

    let mut presets: HashMap<u8, SessionPreset> = HashMap::new();

    for character in &profile.characters {
        let group_id = character.group_id;

        let preset = presets.entry(group_id).or_insert_with(|| {
            SessionPreset::new(
                format!("{}_{}", profile.id, group_id),
                group_id,
                format!("G{} {}", group_id, profile.name),
            )
        });

        preset.add_character(character.name.clone(), character.role);
    }

    // Validate all generated presets
    for preset in presets.values() {
        preset.validate()?;
    }

    Ok(presets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_role_labels() {
        assert_eq!(CharacterRole::Tank.short_label(), "tank");
        assert_eq!(CharacterRole::Dps.display_name(), "Damage Dealer");
        assert_eq!(CharacterRole::Cc.short_label(), "cc");
        assert_eq!(CharacterRole::Healer.display_name(), "Healer");
        assert_eq!(CharacterRole::Support.display_name(), "Support");
        assert_eq!(CharacterRole::Puller.display_name(), "Puller");
    }

    #[test]
    fn launch_profile_character_builder() {
        let char = LaunchProfileCharacter::new("Warrior", 1, CharacterRole::Tank)
            .with_launch_delay_ms(2000)
            .with_wait_for_zone(true);

        assert_eq!(char.name, "Warrior");
        assert_eq!(char.group_id, 1);
        assert_eq!(char.role, CharacterRole::Tank);
        assert_eq!(char.launch_delay_ms, Some(2000));
        assert!(char.wait_for_zone);
    }

    #[test]
    fn launch_profile_builder() {
        let profile = LaunchProfile::new("raid-main", "Main Raid", "Teek")
            .with_description("Primary raid setup")
            .with_character(LaunchProfileCharacter::new("Tank1", 1, CharacterRole::Tank))
            .with_character(LaunchProfileCharacter::new(
                "Healer1",
                1,
                CharacterRole::Healer,
            ))
            .with_character(LaunchProfileCharacter::new("Dps1", 2, CharacterRole::Dps))
            .with_launch_limits(3, 2000, 4000);

        assert_eq!(profile.id, "raid-main");
        assert_eq!(profile.characters.len(), 3);
        assert_eq!(profile.max_concurrent_launches, 3);
        assert_eq!(profile.stagger_min_ms, 2000);
        assert_eq!(profile.stagger_max_ms, 4000);
    }

    #[test]
    fn launch_profile_validation_succeeds_on_valid() {
        let profile = LaunchProfile::new("test", "Test", "Teek")
            .with_character(LaunchProfileCharacter::new("Char1", 1, CharacterRole::Tank))
            .with_character(LaunchProfileCharacter::new("Char2", 1, CharacterRole::Dps));

        assert!(profile.validate().is_ok());
    }

    #[test]
    fn launch_profile_validation_fails_on_empty_characters() {
        let profile = LaunchProfile::new("test", "Test", "Teek");
        assert!(profile.validate().is_err());
        assert!(
            profile
                .validate()
                .unwrap_err()
                .contains("at least one character")
        );
    }

    #[test]
    fn launch_profile_validation_fails_on_group_id_zero() {
        let profile = LaunchProfile::new("test", "Test", "Teek")
            .with_character(LaunchProfileCharacter::new("Char1", 0, CharacterRole::Tank));

        assert!(profile.validate().is_err());
        assert!(profile.validate().unwrap_err().contains("group_id 0"));
    }

    #[test]
    fn launch_profile_validation_fails_on_group_id_exceeds_six() {
        let profile = LaunchProfile::new("test", "Test", "Teek")
            .with_character(LaunchProfileCharacter::new("Char1", 7, CharacterRole::Tank));

        assert!(profile.validate().is_err());
        assert!(
            profile
                .validate()
                .unwrap_err()
                .contains("exceeding max of 6")
        );
    }

    #[test]
    fn launch_profile_validation_fails_on_stagger_mismatch() {
        let profile = LaunchProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: None,
            server_name: "Teek".to_string(),
            characters: vec![LaunchProfileCharacter::new("Char1", 1, CharacterRole::Tank)],
            max_concurrent_launches: 0,
            stagger_min_ms: 5000,
            stagger_max_ms: 2000,
        };

        assert!(profile.validate().is_err());
        assert!(
            profile
                .validate()
                .unwrap_err()
                .contains("stagger_min_ms must be <= stagger_max_ms")
        );
    }

    #[test]
    fn launch_profile_group_summary() {
        let profile = LaunchProfile::new("test", "Test", "Teek")
            .with_character(LaunchProfileCharacter::new("Char1", 1, CharacterRole::Tank))
            .with_character(LaunchProfileCharacter::new("Char2", 1, CharacterRole::Dps))
            .with_character(LaunchProfileCharacter::new(
                "Char3",
                2,
                CharacterRole::Healer,
            ));

        let summary = profile.group_summary();
        assert_eq!(summary.get(&1), Some(&2));
        assert_eq!(summary.get(&2), Some(&1));
    }

    #[test]
    fn session_preset_builder() {
        let mut preset = SessionPreset::new("test_1", 1, "G1 Main");
        preset.add_character("Tank1".to_string(), CharacterRole::Tank);
        preset.add_character("Healer1".to_string(), CharacterRole::Healer);

        assert_eq!(preset.group_id, 1);
        assert_eq!(preset.character_count(), 2);
        assert!(preset.contains_character("Tank1"));
        assert_eq!(
            preset.get_character_role("Tank1"),
            Some(CharacterRole::Tank)
        );
    }

    #[test]
    fn session_preset_validation_succeeds() {
        let mut preset = SessionPreset::new("test_1", 1, "G1 Main");
        preset.add_character("Char1".to_string(), CharacterRole::Tank);
        assert!(preset.validate().is_ok());
    }

    #[test]
    fn session_preset_validation_fails_on_invalid_group_id() {
        let preset = SessionPreset::new("test_0", 0, "G0 Invalid");
        assert!(preset.validate().is_err());
    }

    #[test]
    fn session_preset_validation_fails_on_empty_characters() {
        let preset = SessionPreset::new("test_1", 1, "G1 Empty");
        assert!(preset.validate().is_err());
    }

    #[test]
    fn session_preset_validation_fails_on_missing_role() {
        let mut preset = SessionPreset::new("test_1", 1, "G1 Incomplete");
        preset.character_names.push("Char1".to_string());
        // No role assigned

        assert!(preset.validate().is_err());
    }

    #[test]
    fn session_resource_allocation_defaults() {
        let allocation = SessionResourceAllocation::default();
        assert_eq!(allocation.max_working_set_mb, 0);
        assert!(allocation.combat_enabled);
        assert!(!allocation.autonomous_enabled);
    }

    #[test]
    fn translate_profile_to_presets_single_group() {
        let profile = LaunchProfile::new("test", "Test", "Teek")
            .with_character(LaunchProfileCharacter::new("Char1", 1, CharacterRole::Tank))
            .with_character(LaunchProfileCharacter::new("Char2", 1, CharacterRole::Dps));

        let presets = translate_profile_to_presets(&profile).unwrap();

        assert_eq!(presets.len(), 1);
        let preset = presets.get(&1).unwrap();
        assert_eq!(preset.character_count(), 2);
        assert!(preset.contains_character("Char1"));
        assert!(preset.contains_character("Char2"));
    }

    #[test]
    fn translate_profile_to_presets_multiple_groups() {
        let profile = LaunchProfile::new("test", "Test", "Teek")
            .with_character(LaunchProfileCharacter::new("Char1", 1, CharacterRole::Tank))
            .with_character(LaunchProfileCharacter::new("Char2", 1, CharacterRole::Dps))
            .with_character(LaunchProfileCharacter::new("Char3", 2, CharacterRole::Tank))
            .with_character(LaunchProfileCharacter::new(
                "Char4",
                2,
                CharacterRole::Healer,
            ));

        let presets = translate_profile_to_presets(&profile).unwrap();

        assert_eq!(presets.len(), 2);

        let preset_1 = presets.get(&1).unwrap();
        assert_eq!(preset_1.character_count(), 2);
        assert_eq!(preset_1.routing_label, "G1 Test");

        let preset_2 = presets.get(&2).unwrap();
        assert_eq!(preset_2.character_count(), 2);
        assert_eq!(preset_2.routing_label, "G2 Test");
    }

    #[test]
    fn translate_profile_to_presets_fails_on_invalid_profile() {
        let profile = LaunchProfile::new("test", "Test", "Teek"); // No characters

        assert!(translate_profile_to_presets(&profile).is_err());
    }

    #[test]
    fn translate_profile_to_presets_preserves_roles() {
        let profile = LaunchProfile::new("test", "Test", "Teek")
            .with_character(LaunchProfileCharacter::new("Tank1", 1, CharacterRole::Tank))
            .with_character(LaunchProfileCharacter::new(
                "Healer1",
                1,
                CharacterRole::Healer,
            ))
            .with_character(LaunchProfileCharacter::new("Dps1", 1, CharacterRole::Dps));

        let presets = translate_profile_to_presets(&profile).unwrap();
        let preset = presets.get(&1).unwrap();

        assert_eq!(
            preset.get_character_role("Tank1"),
            Some(CharacterRole::Tank)
        );
        assert_eq!(
            preset.get_character_role("Healer1"),
            Some(CharacterRole::Healer)
        );
        assert_eq!(preset.get_character_role("Dps1"), Some(CharacterRole::Dps));
    }

    #[test]
    fn session_preset_duplicate_character_add_idempotent() {
        let mut preset = SessionPreset::new("test_1", 1, "G1 Main");
        preset.add_character("Char1".to_string(), CharacterRole::Tank);
        preset.add_character("Char1".to_string(), CharacterRole::Dps); // Duplicate with different role

        // Should still have only one entry; last role wins
        assert_eq!(preset.character_count(), 1);
        assert_eq!(preset.get_character_role("Char1"), Some(CharacterRole::Dps));
    }
}
