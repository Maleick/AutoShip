//! Charm and pet management primitives for combat automation.
//!
//! This is the orchestration-side state layer: it tracks controlled pets,
//! selects preferred charm candidates, and emits the first pet-control commands
//! without coupling the core combat coordinator to a concrete UI workflow.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use textquest_common::{ipc::Command, types::ClientId};

const DEFAULT_RECAST_THRESHOLD_SECS: u64 = 10;

fn default_recast_threshold() -> u64 {
    DEFAULT_RECAST_THRESHOLD_SECS
}

fn default_target_priority() -> Vec<CharmTargetKind> {
    vec![CharmTargetKind::BeastlordPet, CharmTargetKind::RegularMob]
}

/// Configuration for charm and controlled-pet automation.
///
/// Matches the intended TOML shape:
///
/// ```toml
/// [charm]
/// recast_threshold = 10
/// prefer_humanoid = true
/// target_priority = ["beastlord_pet", "regular_mob"]
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharmConfig {
    /// Seconds before expiration when a charm should be refreshed.
    #[serde(default = "default_recast_threshold")]
    pub recast_threshold: u64,
    /// Prefer humanoid charm targets when priorities otherwise tie.
    #[serde(default)]
    pub prefer_humanoid: bool,
    /// Target classes in descending preference order.
    #[serde(default = "default_target_priority")]
    pub target_priority: Vec<CharmTargetKind>,
}

impl Default for CharmConfig {
    fn default() -> Self {
        Self {
            recast_threshold: DEFAULT_RECAST_THRESHOLD_SECS,
            prefer_humanoid: false,
            target_priority: default_target_priority(),
        }
    }
}

impl CharmConfig {
    fn recast_threshold_duration(&self) -> Duration {
        Duration::from_secs(self.recast_threshold)
    }

    fn target_priority_rank(&self, kind: CharmTargetKind) -> usize {
        self.target_priority
            .iter()
            .position(|candidate| *candidate == kind)
            .unwrap_or(usize::MAX)
    }
}

/// Coarse target classes used by charm affinity rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharmTargetKind {
    /// Existing beastlord pet that can be preferred over nearby trash.
    BeastlordPet,
    /// Normal NPC target.
    RegularMob,
    /// Summoned creature controlled by the client.
    SummonedPet,
    /// Unknown or unclassified target.
    Unknown,
}

/// A possible target for a future charm cast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharmCandidate {
    /// Spawn ID of the candidate.
    pub spawn_id: u32,
    /// Display name used for operator diagnostics.
    pub name: String,
    /// Coarse class for affinity matching.
    pub kind: CharmTargetKind,
    /// Whether this target is humanoid.
    pub is_humanoid: bool,
}

/// A currently controlled pet.
#[derive(Debug, Clone)]
pub struct ManagedPet {
    /// Owning client ID.
    pub owner_id: ClientId,
    /// Pet spawn ID.
    pub spawn_id: u32,
    /// Pet display name.
    pub name: String,
    /// Pet source.
    pub pet_type: ManagedPetType,
    /// When charm expires, if this pet is charm-controlled.
    pub charm_expires_at: Option<Instant>,
}

/// Pet source tracked by the manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedPetType {
    /// Charmed NPC.
    Charmed,
    /// Summoned pet.
    Summoned,
}

/// Next charm lifecycle action the coordinator should perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CharmAction {
    /// Recast charm before the current control window expires.
    RecastCharm {
        /// Client that owns the current charm.
        owner_id: ClientId,
        /// Current charmed target spawn ID.
        target_id: u32,
    },
}

/// Tracks charm targets and controlled pets for the combat coordinator.
#[derive(Debug, Default)]
pub struct CharmManager {
    config: CharmConfig,
    pets: HashMap<ClientId, Vec<ManagedPet>>,
}

impl CharmManager {
    /// Create a manager with default charm configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(CharmConfig::default())
    }

    /// Create a manager with explicit charm configuration.
    #[must_use]
    pub fn with_config(config: CharmConfig) -> Self {
        Self {
            config,
            pets: HashMap::new(),
        }
    }

    /// Return the active configuration.
    #[must_use]
    pub fn config(&self) -> &CharmConfig {
        &self.config
    }

    /// Register or refresh a charmed pet for `owner_id`.
    pub fn track_charm(
        &mut self,
        owner_id: ClientId,
        spawn_id: u32,
        name: impl Into<String>,
        expires_at: Instant,
    ) {
        self.remove_pet(owner_id, spawn_id);
        self.pets.entry(owner_id).or_default().push(ManagedPet {
            owner_id,
            spawn_id,
            name: name.into(),
            pet_type: ManagedPetType::Charmed,
            charm_expires_at: Some(expires_at),
        });
    }

    /// Register or refresh a summoned pet for `owner_id`.
    pub fn track_summoned_pet(
        &mut self,
        owner_id: ClientId,
        spawn_id: u32,
        name: impl Into<String>,
    ) {
        self.remove_pet(owner_id, spawn_id);
        self.pets.entry(owner_id).or_default().push(ManagedPet {
            owner_id,
            spawn_id,
            name: name.into(),
            pet_type: ManagedPetType::Summoned,
            charm_expires_at: None,
        });
    }

    /// Stop tracking a specific controlled pet.
    pub fn remove_pet(&mut self, owner_id: ClientId, spawn_id: u32) {
        if let Some(pets) = self.pets.get_mut(&owner_id) {
            pets.retain(|pet| pet.spawn_id != spawn_id);
            if pets.is_empty() {
                self.pets.remove(&owner_id);
            }
        }
    }

    /// Select the best available charm candidate according to configured
    /// affinity preferences.
    #[must_use]
    pub fn best_candidate<'a>(
        &self,
        candidates: &'a [CharmCandidate],
    ) -> Option<&'a CharmCandidate> {
        candidates.iter().min_by_key(|candidate| {
            (
                self.config.target_priority_rank(candidate.kind),
                !self.config.prefer_humanoid || !candidate.is_humanoid,
                candidate.spawn_id,
            )
        })
    }

    /// Return the next charm action needed at `now`, if any.
    #[must_use]
    pub fn next_charm_action(&self, now: Instant) -> Option<CharmAction> {
        let threshold = self.config.recast_threshold_duration();

        self.pets
            .values()
            .flatten()
            .filter(|pet| pet.pet_type == ManagedPetType::Charmed)
            .filter_map(|pet| {
                let expires_at = pet.charm_expires_at?;
                let remaining = expires_at.saturating_duration_since(now);
                (remaining <= threshold).then_some((expires_at, pet))
            })
            .min_by_key(|(expires_at, _)| *expires_at)
            .map(|(_, pet)| CharmAction::RecastCharm {
                owner_id: pet.owner_id,
                target_id: pet.spawn_id,
            })
    }

    /// Generate the first pet-control command: put the owner's active pet into
    /// follow mode.
    #[must_use]
    pub fn follow_command(&self, owner_id: ClientId) -> Option<(ClientId, Command)> {
        self.pets
            .get(&owner_id)
            .filter(|pets| !pets.is_empty())
            .map(|_| {
                (
                    owner_id,
                    Command::SlashCommand {
                        command: "/pet follow".to_string(),
                    },
                )
            })
    }

    /// Read the controlled pets for an owner.
    #[must_use]
    pub fn pets_for(&self, owner_id: ClientId) -> &[ManagedPet] {
        self.pets.get(&owner_id).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recharm_action_fires_inside_threshold() {
        let mut manager = CharmManager::new();
        let now = Instant::now();

        manager.track_charm(7, 42, "a_sarnak", now + Duration::from_secs(5));

        assert_eq!(
            manager.next_charm_action(now),
            Some(CharmAction::RecastCharm {
                owner_id: 7,
                target_id: 42
            })
        );
    }

    #[test]
    fn best_candidate_uses_priority_then_humanoid_preference() {
        let manager = CharmManager::with_config(CharmConfig {
            recast_threshold: 10,
            prefer_humanoid: true,
            target_priority: vec![CharmTargetKind::RegularMob],
        });
        let candidates = vec![
            CharmCandidate {
                spawn_id: 2,
                name: "a_wolf".into(),
                kind: CharmTargetKind::RegularMob,
                is_humanoid: false,
            },
            CharmCandidate {
                spawn_id: 1,
                name: "a_goblin".into(),
                kind: CharmTargetKind::RegularMob,
                is_humanoid: true,
            },
        ];

        assert_eq!(
            manager.best_candidate(&candidates).map(|c| c.spawn_id),
            Some(1)
        );
    }

    #[test]
    fn follow_command_requires_tracked_pet() {
        let mut manager = CharmManager::new();
        assert!(manager.follow_command(1).is_none());

        manager.track_summoned_pet(1, 99, "warder");

        assert_eq!(
            manager.follow_command(1),
            Some((
                1,
                Command::SlashCommand {
                    command: "/pet follow".into()
                }
            ))
        );
    }
}
