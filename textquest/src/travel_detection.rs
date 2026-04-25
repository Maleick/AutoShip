//! Travel spell auto-detection and peer broadcast.
//!
//! Detects available travel spells on each character and broadcasts to peers
//! for group travel coordination. Implements rgmercs travel.lua capability.

use std::collections::HashSet;

/// Represents a detected travel spell or ability.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TravelSpell {
    /// Spell or ability name (e.g., "Gate", "Teleport", "Evacuation").
    pub name: String,
    /// Spell ID or gem number if applicable.
    pub id: Option<u32>,
    /// Target zone or destination for this travel spell.
    pub destination: Option<String>,
}

impl TravelSpell {
    /// Create a new travel spell entry.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            id: None,
            destination: None,
        }
    }

    /// Create a travel spell with a destination.
    pub fn with_destination(name: impl Into<String>, destination: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            id: None,
            destination: Some(destination.into()),
        }
    }
}

/// Travel capability for a character.
#[derive(Debug, Clone)]
pub struct TravelCapability {
    /// Character name that has this capability.
    pub character: String,
    /// Set of available travel spells.
    pub spells: HashSet<TravelSpell>,
}

impl TravelCapability {
    /// Create a new travel capability set for a character.
    pub fn new(character: impl Into<String>) -> Self {
        Self {
            character: character.into(),
            spells: HashSet::new(),
        }
    }

    /// Add a travel spell to this capability set.
    pub fn add_spell(&mut self, spell: TravelSpell) {
        self.spells.insert(spell);
    }

    /// Check if this character can travel to a zone.
    pub fn can_travel_to(&self, destination: &str) -> bool {
        self.spells
            .iter()
            .any(|s| s.destination.as_deref() == Some(destination))
    }

    /// Get all travel spell names.
    pub fn spell_names(&self) -> Vec<&str> {
        self.spells.iter().map(|s| s.name.as_str()).collect()
    }
}

/// Broadcast a character's travel capabilities to peers.
pub fn broadcast_travel_capability(capability: &TravelCapability) -> bool {
    let spell_list = capability.spell_names().join(",");
    let message = format!("{}|{}", capability.character, spell_list);

    crate::box_chat::broadcast_channel(
        "travel_detect".to_string(),
        "travel_coordinator".to_string(),
        message,
    )
}

/// Detect travel spells available to a character.
/// This is a placeholder that should be populated with actual spell database queries.
pub fn detect_available_spells(character: &str) -> TravelCapability {
    let mut capability = TravelCapability::new(character);

    // Common travel spells by class — these would normally be queried from the spell database
    // For now, we define known travel spells that should be detected.
    let known_travel_spells = [
        ("Gate", "Qvic"),
        ("Evacuation", "Qvic"),
        ("Teleport", "Plane of Time"),
        ("Call", "Home"),
        ("Summon", "Group"),
    ];

    for (spell_name, destination) in &known_travel_spells {
        capability.add_spell(TravelSpell::with_destination(*spell_name, *destination));
    }

    capability
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn travel_capability_tracks_spells() {
        let mut cap = TravelCapability::new("Cleric1");
        cap.add_spell(TravelSpell::with_destination("Gate", "Qvic"));
        cap.add_spell(TravelSpell::with_destination("Evacuation", "Qvic"));

        assert_eq!(cap.spell_names().len(), 2);
        assert!(cap.can_travel_to("Qvic"));
        assert!(!cap.can_travel_to("Unknown"));
    }

    #[test]
    fn travel_spell_creation() {
        let spell = TravelSpell::with_destination("Gate", "Qvic");
        assert_eq!(spell.name, "Gate");
        assert_eq!(spell.destination, Some("Qvic".to_string()));
    }

    #[test]
    fn detect_available_spells_finds_known_travel() {
        let cap = detect_available_spells("TestChar");
        assert!(!cap.spell_names().is_empty());
        assert!(cap.can_travel_to("Qvic") || cap.can_travel_to("Home"));
    }
}
