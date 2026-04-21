use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use textquest_common::soul::{IdleBehaviorType, MoodState};

use crate::idle::PrioritizedBehavior;

/// Environmental metadata for an EQ zone that drives idle behavior selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZoneClassification {
    /// Zone is generally safe for players (guards, low danger).
    pub safe: bool,
    /// Zone has significant mob danger — wandering, patrols, or aggressive NPCs.
    pub dangerous: bool,
    /// Zone has fishing spots (water bodies).
    pub has_water: bool,
    /// Zone has merchant NPCs players can browse.
    pub has_vendors: bool,
    /// Relative NPC/player density: 0.0 = empty, 1.0 = extremely busy.
    pub npc_density: f32,
    /// Zone supports player-vs-player combat.
    pub is_pvp: bool,
    /// High-traffic social zone (PoK, Bazaar, EC Tunnel).
    pub is_social_hub: bool,
    /// Zone is an active raid area (boss encounter expected).
    pub is_raid_zone: bool,
    /// Zone has tradeskill facilities (forge, loom, etc.).
    pub has_tradeskill: bool,
}

impl Default for ZoneClassification {
    fn default() -> Self {
        Self {
            safe: false,
            dangerous: true,
            has_water: false,
            has_vendors: false,
            npc_density: 0.5,
            is_pvp: false,
            is_social_hub: false,
            is_raid_zone: false,
            has_tradeskill: false,
        }
    }
}

/// TOML-loadable outer wrapper for `config/soul/zones.toml`.
#[derive(Debug, Deserialize)]
pub struct ZoneConfigFile {
    #[serde(default)]
    pub zones: HashMap<String, ZoneClassification>,
}

/// Lookup table mapping zone short names to their classification.
///
/// Falls back to `ZoneClassification::default()` for unknown zones.
pub struct ZoneDatabase {
    zones: HashMap<String, ZoneClassification>,
}

impl ZoneDatabase {
    /// Build a database from inline defaults covering common EQ zones.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut db = Self {
            zones: HashMap::new(),
        };
        db.load_defaults();
        db
    }

    /// Load additional zone classifications from a TOML config file.
    /// Unknown zone keys are inserted; existing keys are overwritten.
    pub fn load_from_toml(&mut self, path: &std::path::Path) -> Result<(), anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let file: ZoneConfigFile = toml::from_str(&content)?;
        for (zone, classification) in file.zones {
            self.zones.insert(zone, classification);
        }
        Ok(())
    }

    /// Merge additional zone entries. Overwrites any existing key.
    pub fn merge(&mut self, zones: HashMap<String, ZoneClassification>) {
        self.zones.extend(zones);
    }

    /// Look up a zone by its short name. Returns default for unknown zones.
    #[must_use]
    pub fn lookup(&self, zone: &str) -> &ZoneClassification {
        self.zones.get(zone).unwrap_or(&DEFAULT_ZONE)
    }

    fn load_defaults(&mut self) {
        let entries: &[(&str, ZoneClassification)] = &[
            // ─── Social hubs ───
            (
                "pok",
                ZoneClassification {
                    safe: true,
                    dangerous: false,
                    has_water: false,
                    has_vendors: true,
                    npc_density: 0.9,
                    is_pvp: false,
                    is_social_hub: true,
                    is_raid_zone: false,
                    has_tradeskill: true,
                },
            ),
            (
                "bazaar",
                ZoneClassification {
                    safe: true,
                    dangerous: false,
                    has_water: false,
                    has_vendors: true,
                    npc_density: 1.0,
                    is_pvp: false,
                    is_social_hub: true,
                    is_raid_zone: false,
                    has_tradeskill: false,
                },
            ),
            (
                "nexus",
                ZoneClassification {
                    safe: true,
                    dangerous: false,
                    has_water: false,
                    has_vendors: true,
                    npc_density: 0.8,
                    is_pvp: false,
                    is_social_hub: true,
                    is_raid_zone: false,
                    has_tradeskill: false,
                },
            ),
            (
                "gfaydark",
                ZoneClassification {
                    safe: false,
                    dangerous: false,
                    has_water: true,
                    has_vendors: false,
                    npc_density: 0.4,
                    is_pvp: false,
                    is_social_hub: false,
                    is_raid_zone: false,
                    has_tradeskill: false,
                },
            ),
            // ─── Outdoor/travel zones ───
            (
                "eastkarana",
                ZoneClassification {
                    safe: false,
                    dangerous: false,
                    has_water: true,
                    has_vendors: false,
                    npc_density: 0.2,
                    is_pvp: false,
                    is_social_hub: false,
                    is_raid_zone: false,
                    has_tradeskill: false,
                },
            ),
            (
                "northkarana",
                ZoneClassification {
                    safe: false,
                    dangerous: false,
                    has_water: true,
                    has_vendors: false,
                    npc_density: 0.2,
                    is_pvp: false,
                    is_social_hub: false,
                    is_raid_zone: false,
                    has_tradeskill: false,
                },
            ),
            // ─── Dangerous dungeons ───
            (
                "sebilis",
                ZoneClassification {
                    safe: false,
                    dangerous: true,
                    has_water: false,
                    has_vendors: false,
                    npc_density: 0.8,
                    is_pvp: false,
                    is_social_hub: false,
                    is_raid_zone: false,
                    has_tradeskill: false,
                },
            ),
            (
                "lguk",
                ZoneClassification {
                    safe: false,
                    dangerous: true,
                    has_water: false,
                    has_vendors: false,
                    npc_density: 0.7,
                    is_pvp: false,
                    is_social_hub: false,
                    is_raid_zone: false,
                    has_tradeskill: false,
                },
            ),
            (
                "velks",
                ZoneClassification {
                    safe: false,
                    dangerous: true,
                    has_water: false,
                    has_vendors: false,
                    npc_density: 0.7,
                    is_pvp: false,
                    is_social_hub: false,
                    is_raid_zone: false,
                    has_tradeskill: false,
                },
            ),
            (
                "crushbone",
                ZoneClassification {
                    safe: false,
                    dangerous: true,
                    has_water: false,
                    has_vendors: false,
                    npc_density: 0.6,
                    is_pvp: false,
                    is_social_hub: false,
                    is_raid_zone: false,
                    has_tradeskill: false,
                },
            ),
            // ─── Raid zones ───
            (
                "sleeper",
                ZoneClassification {
                    safe: false,
                    dangerous: true,
                    has_water: false,
                    has_vendors: false,
                    npc_density: 0.3,
                    is_pvp: false,
                    is_social_hub: false,
                    is_raid_zone: true,
                    has_tradeskill: false,
                },
            ),
            (
                "ntov",
                ZoneClassification {
                    safe: false,
                    dangerous: true,
                    has_water: false,
                    has_vendors: false,
                    npc_density: 0.4,
                    is_pvp: false,
                    is_social_hub: false,
                    is_raid_zone: true,
                    has_tradeskill: false,
                },
            ),
        ];

        for (name, classification) in entries {
            self.zones
                .insert((*name).to_string(), classification.clone());
        }
    }
}

static DEFAULT_ZONE: ZoneClassification = ZoneClassification {
    safe: false,
    dangerous: true,
    has_water: false,
    has_vendors: false,
    npc_density: 0.5,
    is_pvp: false,
    is_social_hub: false,
    is_raid_zone: false,
    has_tradeskill: false,
};

/// Apply zone-based behavior constraints to a weight list.
///
/// - Behaviors impossible in this zone are zeroed out.
/// - Dangerous zones suppress wandering and emotes.
/// - Social hubs boost bio-breaks and lore chatter.
/// - PvP zones suppress emotes entirely.
/// - Raid zones suppress all idle except Sit.
pub fn apply_zone_constraints(
    weights: &mut [PrioritizedBehavior],
    zone: &ZoneClassification,
    mood: MoodState,
) {
    for pb in weights.iter_mut() {
        match pb.behavior {
            IdleBehaviorType::Fish if !zone.has_water => {
                pb.weight = 0.0;
            }
            IdleBehaviorType::VendorBrowse if !zone.has_vendors => {
                pb.weight = 0.0;
            }
            IdleBehaviorType::Craft if !zone.has_tradeskill => {
                pb.weight = 0.0;
            }
            IdleBehaviorType::Wander => {
                if zone.dangerous {
                    // Anxious in dangerous zone → near zero
                    let factor = if mood == MoodState::Anxious {
                        0.05
                    } else {
                        0.3
                    };
                    pb.weight *= factor;
                } else if zone.safe {
                    pb.weight *= 1.5;
                }
            }
            IdleBehaviorType::Emote | IdleBehaviorType::RandomJump => {
                if zone.is_pvp {
                    pb.weight = 0.0;
                } else if zone.dangerous {
                    pb.weight *= 0.4;
                }
            }
            IdleBehaviorType::LoreChatter => {
                // High density → less chat (scrutiny pressure)
                if zone.npc_density > 0.7 && !zone.is_social_hub {
                    pb.weight *= 0.5;
                }
                // Social hub boosts lore
                if zone.is_social_hub {
                    pb.weight *= 1.4;
                }
            }
            IdleBehaviorType::BioBrk if zone.is_social_hub => {
                pb.weight *= 1.5;
            }
            IdleBehaviorType::Sit => {
                // Anxious in dangerous zone → strongly prefer sitting
                if zone.dangerous && mood == MoodState::Anxious {
                    pb.weight *= 2.5;
                }
                // Raid zone → maximize sitting while waiting
                if zone.is_raid_zone {
                    pb.weight *= 2.0;
                }
            }
            _ => {}
        }
    }

    // In raid zones, suppress everything except Sit (not in combat but staged)
    if zone.is_raid_zone {
        for pb in weights.iter_mut() {
            if pb.behavior != IdleBehaviorType::Sit && pb.behavior != IdleBehaviorType::BioBrk {
                pb.weight *= 0.1;
            }
        }
    }

    // Excited mood in raid zone → keep Sit weight high (ready stance)
    if zone.is_raid_zone && mood == MoodState::Excited {
        for pb in weights.iter_mut() {
            if pb.behavior == IdleBehaviorType::Sit {
                pb.weight *= 1.5;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use textquest_common::soul::IdleBehaviorType;

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
                behavior: IdleBehaviorType::Craft,
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
                behavior: IdleBehaviorType::RandomJump,
                weight: 1.0,
            },
        ]
    }

    fn weight_of(weights: &[PrioritizedBehavior], target: &IdleBehaviorType) -> f32 {
        weights
            .iter()
            .find(|w| &w.behavior == target)
            .map(|w| w.weight)
            .unwrap_or(0.0)
    }

    #[test]
    fn fish_zeroed_in_zone_without_water() {
        let zone = ZoneClassification {
            has_water: false,
            safe: true,
            dangerous: false,
            ..Default::default()
        };
        let mut weights = make_weights();
        apply_zone_constraints(&mut weights, &zone, MoodState::Neutral);
        assert_eq!(weight_of(&weights, &IdleBehaviorType::Fish), 0.0);
    }

    #[test]
    fn fish_nonzero_in_zone_with_water() {
        let zone = ZoneClassification {
            has_water: true,
            safe: true,
            dangerous: false,
            ..Default::default()
        };
        let mut weights = make_weights();
        apply_zone_constraints(&mut weights, &zone, MoodState::Neutral);
        assert!(weight_of(&weights, &IdleBehaviorType::Fish) > 0.0);
    }

    #[test]
    fn vendor_browse_zeroed_without_vendors() {
        let zone = ZoneClassification {
            has_vendors: false,
            safe: true,
            dangerous: false,
            ..Default::default()
        };
        let mut weights = make_weights();
        apply_zone_constraints(&mut weights, &zone, MoodState::Neutral);
        assert_eq!(weight_of(&weights, &IdleBehaviorType::VendorBrowse), 0.0);
    }

    #[test]
    fn craft_zeroed_without_tradeskill() {
        let zone = ZoneClassification {
            has_tradeskill: false,
            ..Default::default()
        };
        let mut weights = make_weights();
        apply_zone_constraints(&mut weights, &zone, MoodState::Neutral);
        assert_eq!(weight_of(&weights, &IdleBehaviorType::Craft), 0.0);
    }

    #[test]
    fn wander_suppressed_in_dangerous_zone() {
        let dangerous = ZoneClassification {
            dangerous: true,
            safe: false,
            ..Default::default()
        };
        let safe = ZoneClassification {
            dangerous: false,
            safe: true,
            ..Default::default()
        };
        let mut dangerous_w = make_weights();
        let mut safe_w = make_weights();
        apply_zone_constraints(&mut dangerous_w, &dangerous, MoodState::Neutral);
        apply_zone_constraints(&mut safe_w, &safe, MoodState::Neutral);
        assert!(
            weight_of(&dangerous_w, &IdleBehaviorType::Wander)
                < weight_of(&safe_w, &IdleBehaviorType::Wander)
        );
    }

    #[test]
    fn anxious_in_dangerous_zone_nearly_zero_wander() {
        let zone = ZoneClassification {
            dangerous: true,
            safe: false,
            ..Default::default()
        };
        let mut weights = make_weights();
        apply_zone_constraints(&mut weights, &zone, MoodState::Anxious);
        assert!(weight_of(&weights, &IdleBehaviorType::Wander) < 0.1);
    }

    #[test]
    fn emote_zeroed_in_pvp_zone() {
        let zone = ZoneClassification {
            is_pvp: true,
            ..Default::default()
        };
        let mut weights = make_weights();
        apply_zone_constraints(&mut weights, &zone, MoodState::Neutral);
        assert_eq!(weight_of(&weights, &IdleBehaviorType::Emote), 0.0);
        assert_eq!(weight_of(&weights, &IdleBehaviorType::RandomJump), 0.0);
    }

    #[test]
    fn social_hub_boosts_bio_brk_and_lore() {
        let hub = ZoneClassification {
            is_social_hub: true,
            safe: true,
            dangerous: false,
            npc_density: 0.9,
            ..Default::default()
        };
        let normal = ZoneClassification {
            is_social_hub: false,
            safe: true,
            dangerous: false,
            npc_density: 0.9,
            ..Default::default()
        };
        let mut hub_w = make_weights();
        let mut normal_w = make_weights();
        apply_zone_constraints(&mut hub_w, &hub, MoodState::Neutral);
        apply_zone_constraints(&mut normal_w, &normal, MoodState::Neutral);
        assert!(
            weight_of(&hub_w, &IdleBehaviorType::BioBrk)
                > weight_of(&normal_w, &IdleBehaviorType::BioBrk)
        );
        assert!(
            weight_of(&hub_w, &IdleBehaviorType::LoreChatter)
                > weight_of(&normal_w, &IdleBehaviorType::LoreChatter)
        );
    }

    #[test]
    fn raid_zone_suppresses_all_except_sit() {
        let zone = ZoneClassification {
            is_raid_zone: true,
            dangerous: true,
            ..Default::default()
        };
        let mut weights = make_weights();
        apply_zone_constraints(&mut weights, &zone, MoodState::Neutral);
        let sit_w = weight_of(&weights, &IdleBehaviorType::Sit);
        let wander_w = weight_of(&weights, &IdleBehaviorType::Wander);
        assert!(sit_w > wander_w);
    }

    #[test]
    fn zone_database_lookup_defaults_for_unknown() {
        let db = ZoneDatabase::with_defaults();
        let cls = db.lookup("somerandomunknownzone");
        assert!(cls.dangerous);
    }

    #[test]
    fn zone_database_pok_is_safe_social_hub() {
        let db = ZoneDatabase::with_defaults();
        let pok = db.lookup("pok");
        assert!(pok.safe);
        assert!(!pok.dangerous);
        assert!(pok.is_social_hub);
        assert!(pok.has_vendors);
    }

    #[test]
    fn zone_database_bazaar_has_vendors() {
        let db = ZoneDatabase::with_defaults();
        let bazaar = db.lookup("bazaar");
        assert!(bazaar.has_vendors);
        assert!(bazaar.is_social_hub);
    }

    #[test]
    fn zone_database_sebilis_is_dangerous() {
        let db = ZoneDatabase::with_defaults();
        let sebilis = db.lookup("sebilis");
        assert!(sebilis.dangerous);
        assert!(!sebilis.safe);
        assert!(!sebilis.has_water);
    }

    #[test]
    fn zone_database_eastkarana_has_water() {
        let db = ZoneDatabase::with_defaults();
        let zone = db.lookup("eastkarana");
        assert!(zone.has_water);
    }

    #[test]
    fn zone_database_merge_overwrites() {
        let mut db = ZoneDatabase::with_defaults();
        let mut custom = HashMap::new();
        custom.insert(
            "pok".to_string(),
            ZoneClassification {
                safe: false,
                dangerous: true,
                ..Default::default()
            },
        );
        db.merge(custom);
        let pok = db.lookup("pok");
        assert!(!pok.safe);
        assert!(pok.dangerous);
    }

    #[test]
    fn zone_classification_default_is_dangerous() {
        let z = ZoneClassification::default();
        assert!(z.dangerous);
        assert!(!z.safe);
        assert!(!z.has_water);
        assert!(!z.has_vendors);
    }

    #[test]
    fn anxious_in_dangerous_zone_boosts_sit() {
        let zone = ZoneClassification {
            dangerous: true,
            safe: false,
            ..Default::default()
        };
        let mut anxious_w = make_weights();
        let mut neutral_w = make_weights();
        apply_zone_constraints(&mut anxious_w, &zone, MoodState::Anxious);
        apply_zone_constraints(&mut neutral_w, &zone, MoodState::Neutral);
        assert!(
            weight_of(&anxious_w, &IdleBehaviorType::Sit)
                > weight_of(&neutral_w, &IdleBehaviorType::Sit)
        );
    }
}
