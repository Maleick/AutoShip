//! Equipment set management — focus effects, item sets, gear swaps.

use std::collections::HashMap;
use std::sync::LazyLock;

static EMPTY_ITEMS: LazyLock<HashMap<EquipmentSlot, EquipmentItem>> =
    LazyLock::new(HashMap::new);

/// Equipment slots matching EQ inventory positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquipmentSlot {
    Primary,
    Secondary,
    Range,
    Ammo,
    Head,
    Face,
    Ear,
    Neck,
    Shoulder,
    Arms,
    Wrist,
    Hands,
    Fingers,
    Chest,
    Back,
    Legs,
    Feet,
    Waist,
    Charm,
    PowerSource,
}

/// A single equipped item with optional focus effects and augments.
#[derive(Debug, Clone)]
pub struct EquipmentItem {
    pub item_id: u32,
    pub name: String,
    pub slot: EquipmentSlot,
    pub focus_effects: Vec<String>,
    pub augments: Vec<u32>,
}

/// A named set of equipment (e.g. "raid_tank", "xp_grind").
#[derive(Debug, Clone)]
pub struct EquipmentSet {
    pub name: String,
    pub items: HashMap<EquipmentSlot, EquipmentItem>,
}

/// Manages multiple equipment sets and tracks which is active.
#[derive(Debug)]
pub struct EquipmentManager {
    pub sets: HashMap<String, EquipmentSet>,
    pub active_set: Option<String>,
}

impl EquipmentManager {
    /// Creates an empty equipment manager with no sets.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sets: HashMap::new(),
            active_set: None,
        }
    }

    /// Save (or overwrite) a named equipment set.
    pub fn save_set(&mut self, name: &str, items: HashMap<EquipmentSlot, EquipmentItem>) {
        self.sets.insert(
            name.to_string(),
            EquipmentSet {
                name: name.to_string(),
                items,
            },
        );
    }

    /// Activate a named equipment set. Returns `false` if the set doesn't exist.
    pub fn activate_set(&mut self, name: &str) -> bool {
        if self.sets.contains_key(name) {
            self.active_set = Some(name.to_string());
            true
        } else {
            false
        }
    }

    /// Returns the items in the active set, or an empty map if none is active.
    #[must_use]
    pub fn active_items(&self) -> &HashMap<EquipmentSlot, EquipmentItem> {
        self.active_set
            .as_ref()
            .and_then(|name| self.sets.get(name))
            .map_or(&*EMPTY_ITEMS, |set| &set.items)
    }

    /// Returns all unique focus effects across the active equipment set.
    #[must_use]
    pub fn focus_effects_active(&self) -> Vec<String> {
        let mut effects: Vec<String> = self
            .active_items()
            .values()
            .flat_map(|item| item.focus_effects.iter().cloned())
            .collect();
        effects.sort();
        effects.dedup();
        effects
    }
}

impl Default for EquipmentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(id: u32, name: &str, slot: EquipmentSlot, effects: &[&str]) -> EquipmentItem {
        EquipmentItem {
            item_id: id,
            name: name.to_string(),
            slot,
            focus_effects: effects.iter().map(|s| (*s).to_string()).collect(),
            augments: vec![],
        }
    }

    #[test]
    fn test_new_manager_empty() {
        let mgr = EquipmentManager::new();
        assert!(mgr.sets.is_empty());
        assert!(mgr.active_set.is_none());
    }

    #[test]
    fn test_save_set() {
        let mut mgr = EquipmentManager::new();
        let mut items = HashMap::new();
        items.insert(
            EquipmentSlot::Primary,
            make_item(1001, "Blade of Carnage", EquipmentSlot::Primary, &[]),
        );
        mgr.save_set("raid", items);
        assert_eq!(mgr.sets.len(), 1);
        assert!(mgr.sets.contains_key("raid"));
    }

    #[test]
    fn test_save_set_overwrites() {
        let mut mgr = EquipmentManager::new();
        let mut items1 = HashMap::new();
        items1.insert(
            EquipmentSlot::Head,
            make_item(100, "Old Helm", EquipmentSlot::Head, &[]),
        );
        mgr.save_set("tank", items1);

        let mut items2 = HashMap::new();
        items2.insert(
            EquipmentSlot::Head,
            make_item(200, "New Helm", EquipmentSlot::Head, &[]),
        );
        mgr.save_set("tank", items2);

        assert_eq!(mgr.sets.len(), 1);
        assert_eq!(
            mgr.sets["tank"].items[&EquipmentSlot::Head].item_id,
            200
        );
    }

    #[test]
    fn test_activate_existing_set() {
        let mut mgr = EquipmentManager::new();
        mgr.save_set("xp", HashMap::new());
        assert!(mgr.activate_set("xp"));
        assert_eq!(mgr.active_set.as_deref(), Some("xp"));
    }

    #[test]
    fn test_activate_nonexistent_set() {
        let mut mgr = EquipmentManager::new();
        assert!(!mgr.activate_set("missing"));
        assert!(mgr.active_set.is_none());
    }

    #[test]
    fn test_active_items_no_active_set() {
        let mgr = EquipmentManager::new();
        assert!(mgr.active_items().is_empty());
    }

    #[test]
    fn test_active_items_returns_correct_set() {
        let mut mgr = EquipmentManager::new();
        let mut items = HashMap::new();
        items.insert(
            EquipmentSlot::Chest,
            make_item(500, "Breastplate of Ro", EquipmentSlot::Chest, &[]),
        );
        mgr.save_set("raid", items);
        mgr.activate_set("raid");
        assert_eq!(mgr.active_items().len(), 1);
        assert!(mgr.active_items().contains_key(&EquipmentSlot::Chest));
    }

    #[test]
    fn test_focus_effects_empty_when_no_active() {
        let mgr = EquipmentManager::new();
        assert!(mgr.focus_effects_active().is_empty());
    }

    #[test]
    fn test_focus_effects_collected() {
        let mut mgr = EquipmentManager::new();
        let mut items = HashMap::new();
        items.insert(
            EquipmentSlot::Head,
            make_item(
                10,
                "Crown of Focus",
                EquipmentSlot::Head,
                &["Extended Enhancement", "Improved Healing"],
            ),
        );
        items.insert(
            EquipmentSlot::Hands,
            make_item(
                11,
                "Gloves of Mending",
                EquipmentSlot::Hands,
                &["Improved Healing", "Quickened Casting"],
            ),
        );
        mgr.save_set("heal", items);
        mgr.activate_set("heal");

        let effects = mgr.focus_effects_active();
        assert_eq!(effects.len(), 3);
        assert!(effects.contains(&"Extended Enhancement".to_string()));
        assert!(effects.contains(&"Improved Healing".to_string()));
        assert!(effects.contains(&"Quickened Casting".to_string()));
    }

    #[test]
    fn test_focus_effects_deduplication() {
        let mut mgr = EquipmentManager::new();
        let mut items = HashMap::new();
        items.insert(
            EquipmentSlot::Ear,
            make_item(20, "Earring A", EquipmentSlot::Ear, &["Mana Pres"]),
        );
        items.insert(
            EquipmentSlot::Neck,
            make_item(21, "Necklace B", EquipmentSlot::Neck, &["Mana Pres"]),
        );
        mgr.save_set("dup_test", items);
        mgr.activate_set("dup_test");

        let effects = mgr.focus_effects_active();
        assert_eq!(effects.len(), 1);
    }

    #[test]
    fn test_item_with_augments() {
        let item = EquipmentItem {
            item_id: 999,
            name: "Augmented Sword".to_string(),
            slot: EquipmentSlot::Primary,
            focus_effects: vec![],
            augments: vec![5001, 5002, 5003],
        };
        assert_eq!(item.augments.len(), 3);
    }

    #[test]
    fn test_default_trait() {
        let mgr = EquipmentManager::default();
        assert!(mgr.sets.is_empty());
        assert!(mgr.active_set.is_none());
    }

    #[test]
    fn test_multiple_sets_independent() {
        let mut mgr = EquipmentManager::new();
        let mut raid_items = HashMap::new();
        raid_items.insert(
            EquipmentSlot::Primary,
            make_item(1, "Raid Sword", EquipmentSlot::Primary, &["Ferocity"]),
        );
        let mut xp_items = HashMap::new();
        xp_items.insert(
            EquipmentSlot::Primary,
            make_item(2, "XP Sword", EquipmentSlot::Primary, &["Experience"]),
        );

        mgr.save_set("raid", raid_items);
        mgr.save_set("xp", xp_items);

        mgr.activate_set("raid");
        assert_eq!(mgr.focus_effects_active(), vec!["Ferocity".to_string()]);

        mgr.activate_set("xp");
        assert_eq!(mgr.focus_effects_active(), vec!["Experience".to_string()]);
    }

    #[test]
    fn test_all_equipment_slots() {
        let slots = [
            EquipmentSlot::Primary,
            EquipmentSlot::Secondary,
            EquipmentSlot::Range,
            EquipmentSlot::Ammo,
            EquipmentSlot::Head,
            EquipmentSlot::Face,
            EquipmentSlot::Ear,
            EquipmentSlot::Neck,
            EquipmentSlot::Shoulder,
            EquipmentSlot::Arms,
            EquipmentSlot::Wrist,
            EquipmentSlot::Hands,
            EquipmentSlot::Fingers,
            EquipmentSlot::Chest,
            EquipmentSlot::Back,
            EquipmentSlot::Legs,
            EquipmentSlot::Feet,
            EquipmentSlot::Waist,
            EquipmentSlot::Charm,
            EquipmentSlot::PowerSource,
        ];
        let mut items = HashMap::new();
        for (i, slot) in slots.iter().enumerate() {
            items.insert(
                *slot,
                make_item(i as u32, &format!("Item {i}"), *slot, &[]),
            );
        }
        assert_eq!(items.len(), 20);
    }
}
