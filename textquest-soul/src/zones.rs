use std::{collections::HashMap, path::Path};

use serde::{Deserialize, Serialize};
use textquest_common::soul::{IdleBehaviorType, MoodState};

use crate::idle::PrioritizedBehavior;

/// Relative crowding for a zone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcDensity {
    Low,
    #[default]
    Medium,
    High,
}

impl NpcDensity {
    #[must_use]
    pub const fn is_high(self) -> bool {
        matches!(self, Self::High)
    }
}

/// Environmental metadata used to constrain idle behavior selection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ZoneMetadata {
    /// Canonical short zone name.
    pub name: String,
    /// Safe zones encourage looser ambient behavior.
    pub safe: bool,
    /// Whether fishing is possible here.
    pub has_water: bool,
    /// Whether merchant browsing is possible here.
    pub has_vendors: bool,
    /// Approximate zone activity / population density.
    pub npc_density: NpcDensity,
    /// Active raid zone where idle behavior should be suppressed.
    pub raid_zone: bool,
}

impl ZoneMetadata {
    /// Returns true for social hubs where chatty idle behavior is appropriate.
    #[must_use]
    pub fn is_social_hub(&self) -> bool {
        matches!(
            normalize_zone_name(&self.name).as_str(),
            "pok" | "poknowledge" | "planeofknowledge" | "bazaar" | "thenexus" | "nexus"
        ) || (self.safe && self.has_vendors && self.npc_density.is_high())
    }

    /// Returns true for zones where ambient wandering should be minimized.
    #[must_use]
    pub fn is_dangerous(&self) -> bool {
        self.raid_zone || (!self.safe && self.npc_density.is_high())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ZoneMetadataToml {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    safe: bool,
    #[serde(default)]
    has_water: bool,
    #[serde(default)]
    has_vendors: bool,
    #[serde(default)]
    npc_density: NpcDensity,
    #[serde(default)]
    raid_zone: bool,
}

#[derive(Debug, Deserialize)]
struct ZoneConfigFile {
    #[serde(default)]
    zones: HashMap<String, ZoneMetadataToml>,
}

/// Lookup table mapping normalized zone names to metadata.
#[derive(Debug, Clone, Default)]
pub struct ZoneDatabase {
    zones: HashMap<String, ZoneMetadata>,
}

impl ZoneDatabase {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load zone metadata from a TOML file.
    ///
    /// Unknown zones remain neutral if they are not configured.
    pub fn load_from_toml(&mut self, path: &Path) -> Result<(), anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let file: ZoneConfigFile = toml::from_str(&content)?;
        for (zone_key, metadata) in file.zones {
            let zone_name = metadata.name.unwrap_or(zone_key);
            self.insert(ZoneMetadata {
                name: zone_name,
                safe: metadata.safe,
                has_water: metadata.has_water,
                has_vendors: metadata.has_vendors,
                npc_density: metadata.npc_density,
                raid_zone: metadata.raid_zone,
            });
        }
        Ok(())
    }

    pub fn insert(&mut self, metadata: ZoneMetadata) {
        self.zones
            .insert(normalize_zone_name(&metadata.name), metadata);
    }

    pub fn merge(&mut self, zones: impl IntoIterator<Item = ZoneMetadata>) {
        for zone in zones {
            self.insert(zone);
        }
    }

    /// Look up a zone by name. Unknown zones fall back to neutral metadata.
    #[must_use]
    pub fn lookup(&self, zone: &str) -> ZoneMetadata {
        self.zones
            .get(&normalize_zone_name(zone))
            .cloned()
            .unwrap_or_else(|| ZoneMetadata {
                name: zone.to_string(),
                ..ZoneMetadata::default()
            })
    }
}

/// Apply zone-based behavior constraints to a weight list.
pub fn apply_zone_constraints(
    weights: &mut [PrioritizedBehavior],
    zone: &ZoneMetadata,
    mood: MoodState,
) {
    let social_hub = zone.is_social_hub();
    let dangerous = zone.is_dangerous();

    for pb in weights.iter_mut() {
        match pb.behavior {
            IdleBehaviorType::Fish if !zone.has_water => {
                pb.weight = 0.0;
            }
            IdleBehaviorType::VendorBrowse if !zone.has_vendors => {
                pb.weight = 0.0;
            }
            IdleBehaviorType::Wander => {
                if dangerous {
                    pb.weight = 0.0;
                } else if zone.raid_zone {
                    pb.weight *= 0.25;
                } else if zone.safe {
                    pb.weight *= 1.35;
                }
            }
            IdleBehaviorType::BioBrk if social_hub => {
                pb.weight *= 1.8;
            }
            IdleBehaviorType::LoreChatter => {
                if social_hub {
                    pb.weight *= 1.8;
                } else if zone.npc_density.is_high() {
                    pb.weight *= 0.7;
                }
            }
            IdleBehaviorType::Emote | IdleBehaviorType::RandomJump
                if dangerous || zone.raid_zone =>
            {
                pb.weight *= 0.4;
            }
            IdleBehaviorType::Sit => {
                if dangerous {
                    pb.weight *= 1.6;
                }
                if social_hub && mood == MoodState::Bored {
                    pb.weight *= 0.9;
                }
            }
            IdleBehaviorType::LogOffToSleep if dangerous && mood == MoodState::Anxious => {
                pb.weight *= 1.6;
            }
            _ => {}
        }
    }

    match mood {
        MoodState::Anxious if dangerous => {
            multiply(weights, IdleBehaviorType::Sit, 2.5);
            multiply(weights, IdleBehaviorType::LogOffToSleep, 1.75);
            set_weight(weights, IdleBehaviorType::Wander, 0.0);
        }
        MoodState::Excited if zone.raid_zone => {
            // Ready stance: suppress ambient distractions while remaining staged.
            multiply(weights, IdleBehaviorType::Sit, 1.75);
            multiply(weights, IdleBehaviorType::BioBrk, 0.25);
            multiply(weights, IdleBehaviorType::LoreChatter, 0.35);
            multiply(weights, IdleBehaviorType::Emote, 0.2);
            multiply(weights, IdleBehaviorType::RandomJump, 0.0);
            set_weight(weights, IdleBehaviorType::Wander, 0.0);
        }
        MoodState::Bored if social_hub => {
            multiply(weights, IdleBehaviorType::LoreChatter, 2.0);
            multiply(weights, IdleBehaviorType::Emote, 1.3);
            multiply(weights, IdleBehaviorType::BioBrk, 1.5);
        }
        MoodState::Exhausted => {
            multiply(weights, IdleBehaviorType::Sit, 1.5);
        }
        _ => {}
    }
}

fn normalize_zone_name(zone: &str) -> String {
    zone.trim().to_ascii_lowercase()
}

fn multiply(weights: &mut [PrioritizedBehavior], behavior: IdleBehaviorType, factor: f32) {
    if let Some(item) = weights.iter_mut().find(|item| item.behavior == behavior) {
        item.weight *= factor;
    }
}

fn set_weight(weights: &mut [PrioritizedBehavior], behavior: IdleBehaviorType, weight: f32) {
    if let Some(item) = weights.iter_mut().find(|item| item.behavior == behavior) {
        item.weight = weight;
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn make_weights() -> Vec<PrioritizedBehavior> {
        vec![
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Sit,
                weight: 1.0,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Wander,
                weight: 1.0,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Emote,
                weight: 1.0,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::Fish,
                weight: 1.0,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::VendorBrowse,
                weight: 1.0,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::LoreChatter,
                weight: 1.0,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::BioBrk,
                weight: 1.0,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::LogOffToSleep,
                weight: 1.0,
            },
            PrioritizedBehavior {
                behavior: IdleBehaviorType::RandomJump,
                weight: 1.0,
            },
        ]
    }

    fn weight_of(weights: &[PrioritizedBehavior], target: IdleBehaviorType) -> f32 {
        weights
            .iter()
            .find(|w| w.behavior == target)
            .map(|w| w.weight)
            .unwrap_or_default()
    }

    #[test]
    fn zone_metadata_loads_from_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("soul_zones.toml");
        fs::write(
            &path,
            r#"
[zones.poknowledge]
safe = true
has_water = false
has_vendors = true
npc_density = "high"
raid_zone = false
"#,
        )
        .unwrap();

        let mut db = ZoneDatabase::new();
        db.load_from_toml(&path).unwrap();

        let zone = db.lookup("poknowledge");
        assert_eq!(zone.name, "poknowledge");
        assert!(zone.safe);
        assert!(zone.has_vendors);
        assert_eq!(zone.npc_density, NpcDensity::High);
        assert!(!zone.raid_zone);
    }

    #[test]
    fn unknown_zone_falls_back_to_neutral() {
        let db = ZoneDatabase::new();
        let zone = db.lookup("somewhere");
        assert_eq!(zone.name, "somewhere");
        assert!(!zone.safe);
        assert!(!zone.has_water);
        assert!(!zone.has_vendors);
        assert_eq!(zone.npc_density, NpcDensity::Medium);
        assert!(!zone.raid_zone);
        assert!(!zone.is_dangerous());
    }

    #[test]
    fn dangerous_zone_zeroes_wander_and_boosts_sit_when_anxious() {
        let zone = ZoneMetadata {
            name: "sebilis".into(),
            safe: false,
            has_water: false,
            has_vendors: false,
            npc_density: NpcDensity::High,
            raid_zone: false,
        };
        let mut weights = make_weights();
        apply_zone_constraints(&mut weights, &zone, MoodState::Anxious);
        assert_eq!(weight_of(&weights, IdleBehaviorType::Wander), 0.0);
        assert!(weight_of(&weights, IdleBehaviorType::Sit) > 1.0);
        assert!(weight_of(&weights, IdleBehaviorType::LogOffToSleep) > 1.0);
    }

    #[test]
    fn raid_zone_excited_suppresses_idle_distractions() {
        let zone = ZoneMetadata {
            name: "sleeper".into(),
            safe: false,
            has_water: false,
            has_vendors: false,
            npc_density: NpcDensity::Medium,
            raid_zone: true,
        };
        let mut weights = make_weights();
        apply_zone_constraints(&mut weights, &zone, MoodState::Excited);
        assert_eq!(weight_of(&weights, IdleBehaviorType::Wander), 0.0);
        assert_eq!(weight_of(&weights, IdleBehaviorType::RandomJump), 0.0);
        assert!(weight_of(&weights, IdleBehaviorType::Sit) > 1.0);
    }

    #[test]
    fn social_hub_bored_boosts_chatty_behaviors() {
        let zone = ZoneMetadata {
            name: "poknowledge".into(),
            safe: true,
            has_water: false,
            has_vendors: true,
            npc_density: NpcDensity::High,
            raid_zone: false,
        };
        let mut weights = make_weights();
        apply_zone_constraints(&mut weights, &zone, MoodState::Bored);
        assert!(weight_of(&weights, IdleBehaviorType::LoreChatter) > 1.0);
        assert!(weight_of(&weights, IdleBehaviorType::BioBrk) > 1.0);
    }

    #[test]
    fn water_and_vendor_constraints_are_enforced() {
        let zone = ZoneMetadata {
            name: "neutral".into(),
            safe: false,
            has_water: false,
            has_vendors: false,
            npc_density: NpcDensity::Low,
            raid_zone: false,
        };
        let mut weights = make_weights();
        apply_zone_constraints(&mut weights, &zone, MoodState::Neutral);
        assert_eq!(weight_of(&weights, IdleBehaviorType::Fish), 0.0);
        assert_eq!(weight_of(&weights, IdleBehaviorType::VendorBrowse), 0.0);
    }
}
