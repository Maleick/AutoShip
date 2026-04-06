//! Group builder — dynamic templates, slot assignment, and auto-fill.
//!
//! Data model for composing EverQuest groups from available characters.
//! Supports standard 6-man groups and full raid templates with role-based
//! slot assignment and class-preference matching.

use serde::{Deserialize, Serialize};

/// Combat role within a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Tank,
    Healer,
    DPS,
    Support,
    Puller,
    CC,
}

impl Role {
    /// Classes commonly associated with this role.
    pub fn preferred_classes(self) -> &'static [&'static str] {
        match self {
            Role::Tank => &["Warrior", "Paladin", "Shadow Knight"],
            Role::Healer => &["Cleric", "Druid", "Shaman"],
            Role::DPS => &[
                "Wizard",
                "Magician",
                "Necromancer",
                "Ranger",
                "Rogue",
                "Monk",
                "Berserker",
                "Beastlord",
            ],
            Role::Support => &["Bard", "Enchanter", "Shaman", "Druid"],
            Role::Puller => &["Monk", "Bard", "Ranger", "Shadow Knight"],
            Role::CC => &["Enchanter", "Bard"],
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Tank => write!(f, "Tank"),
            Role::Healer => write!(f, "Healer"),
            Role::DPS => write!(f, "DPS"),
            Role::Support => write!(f, "Support"),
            Role::Puller => write!(f, "Puller"),
            Role::CC => write!(f, "CC"),
        }
    }
}

/// A single slot in a group template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupSlot {
    pub role: Role,
    pub class_preference: Option<String>,
    pub character_name: Option<String>,
    pub locked: bool,
}

impl GroupSlot {
    pub fn new(role: Role) -> Self {
        Self {
            role,
            class_preference: None,
            character_name: None,
            locked: false,
        }
    }

    pub fn with_class_preference(mut self, class: impl Into<String>) -> Self {
        self.class_preference = Some(class.into());
        self
    }

    pub fn is_filled(&self) -> bool {
        self.character_name.is_some()
    }
}

/// A named group composition template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupTemplate {
    pub name: String,
    pub slots: Vec<GroupSlot>,
    pub description: String,
}

impl GroupTemplate {
    /// Standard 6-man group: 1 tank, 1 healer, 1 CC, 3 DPS.
    pub fn standard_group() -> Self {
        Self {
            name: "Standard Group".into(),
            description: "Balanced 6-man group with tank, healer, CC, and DPS.".into(),
            slots: vec![
                GroupSlot::new(Role::Tank),
                GroupSlot::new(Role::Healer),
                GroupSlot::new(Role::CC),
                GroupSlot::new(Role::DPS),
                GroupSlot::new(Role::DPS),
                GroupSlot::new(Role::DPS),
            ],
        }
    }

    /// Full raid template: 6 groups of 6 with balanced roles.
    pub fn raid_group() -> Self {
        let mut slots = Vec::with_capacity(36);
        for i in 0..6 {
            slots.push(GroupSlot::new(Role::Tank));
            slots.push(GroupSlot::new(Role::Healer));
            slots.push(GroupSlot::new(Role::CC));
            if i == 0 {
                slots.push(GroupSlot::new(Role::DPS));
                slots.push(GroupSlot::new(Role::DPS));
                slots.push(GroupSlot::new(Role::Puller));
            } else {
                slots.push(GroupSlot::new(Role::DPS));
                slots.push(GroupSlot::new(Role::DPS));
                slots.push(GroupSlot::new(Role::Support));
            }
        }
        Self {
            name: "Raid (6 groups)".into(),
            description: "36-man raid: 6 groups with tank, healer, CC, DPS, and support.".into(),
            slots,
        }
    }

    /// Count of slots that have a character assigned.
    pub fn filled_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_filled()).count()
    }

    /// Count of slots without a character assigned.
    pub fn empty_count(&self) -> usize {
        self.slots.iter().filter(|s| !s.is_filled()).count()
    }

    /// Assign a character to a slot by index.
    pub fn assign(&mut self, slot_index: usize, character_name: &str) -> Result<(), GroupError> {
        let len = self.slots.len();
        let slot = self
            .slots
            .get_mut(slot_index)
            .ok_or(GroupError::SlotOutOfRange(slot_index, len))?;
        if slot.locked {
            return Err(GroupError::SlotLocked(slot_index));
        }
        slot.character_name = Some(character_name.to_string());
        Ok(())
    }

    /// Remove a character assignment from a slot.
    pub fn unassign(&mut self, slot_index: usize) -> Result<(), GroupError> {
        let len = self.slots.len();
        let slot = self
            .slots
            .get_mut(slot_index)
            .ok_or(GroupError::SlotOutOfRange(slot_index, len))?;
        if slot.locked {
            return Err(GroupError::SlotLocked(slot_index));
        }
        slot.character_name = None;
        Ok(())
    }

    /// Auto-fill empty slots from available characters, matching class to role.
    ///
    /// `available_chars` is a slice of `(character_name, class_name)` pairs.
    /// Returns the number of slots filled.
    pub fn auto_fill(&mut self, available_chars: &[(String, String)]) -> usize {
        let mut filled = 0;
        let mut used: Vec<bool> = vec![false; available_chars.len()];

        // Mark characters already assigned in any slot.
        for slot in self.slots.iter() {
            if let Some(ref name) = slot.character_name {
                for (i, (cname, _)) in available_chars.iter().enumerate() {
                    if cname == name {
                        used[i] = true;
                    }
                }
            }
        }

        for slot_idx in 0..self.slots.len() {
            if self.slots[slot_idx].is_filled() || self.slots[slot_idx].locked {
                continue;
            }

            let preferred = self.slots[slot_idx].role.preferred_classes();
            let class_pref = self.slots[slot_idx].class_preference.clone();

            let candidate = available_chars
                .iter()
                .enumerate()
                .filter(|(i, _)| !used[*i])
                .find(|(_, (_, class))| {
                    if let Some(ref pref) = class_pref {
                        class.eq_ignore_ascii_case(pref)
                    } else {
                        preferred.iter().any(|p| class.eq_ignore_ascii_case(p))
                    }
                });

            if let Some((idx, (name, _))) = candidate {
                self.slots[slot_idx].character_name = Some(name.clone());
                used[idx] = true;
                filled += 1;
            }
        }
        filled
    }
}

/// Errors from group builder operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupError {
    SlotOutOfRange(usize, usize),
    SlotLocked(usize),
}

impl std::fmt::Display for GroupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GroupError::SlotOutOfRange(idx, len) => {
                write!(
                    f,
                    "slot index {idx} out of range (template has {len} slots)"
                )
            }
            GroupError::SlotLocked(idx) => write!(f, "slot {idx} is locked"),
        }
    }
}

impl std::error::Error for GroupError {}

/// UI state for the group builder panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupBuilderState {
    pub templates: Vec<GroupTemplate>,
    pub selected_template: usize,
    pub available_characters: Vec<(String, String)>,
}

impl Default for GroupBuilderState {
    fn default() -> Self {
        Self {
            templates: vec![GroupTemplate::standard_group(), GroupTemplate::raid_group()],
            selected_template: 0,
            available_characters: Vec::new(),
        }
    }
}

impl GroupBuilderState {
    pub fn active_template(&self) -> Option<&GroupTemplate> {
        self.templates.get(self.selected_template)
    }

    pub fn active_template_mut(&mut self) -> Option<&mut GroupTemplate> {
        self.templates.get_mut(self.selected_template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_group_has_6_slots() {
        let g = GroupTemplate::standard_group();
        assert_eq!(g.slots.len(), 6);
        assert_eq!(g.filled_count(), 0);
        assert_eq!(g.empty_count(), 6);
    }

    #[test]
    fn raid_group_has_36_slots() {
        let g = GroupTemplate::raid_group();
        assert_eq!(g.slots.len(), 36);
        assert_eq!(g.filled_count(), 0);
        assert_eq!(g.empty_count(), 36);
    }

    #[test]
    fn standard_group_roles() {
        let g = GroupTemplate::standard_group();
        let roles: Vec<Role> = g.slots.iter().map(|s| s.role).collect();
        assert_eq!(
            roles,
            vec![
                Role::Tank,
                Role::Healer,
                Role::CC,
                Role::DPS,
                Role::DPS,
                Role::DPS
            ]
        );
    }

    #[test]
    fn assign_and_unassign() {
        let mut g = GroupTemplate::standard_group();
        g.assign(0, "Thorin").unwrap();
        assert_eq!(g.slots[0].character_name.as_deref(), Some("Thorin"));
        assert_eq!(g.filled_count(), 1);

        g.unassign(0).unwrap();
        assert!(g.slots[0].character_name.is_none());
        assert_eq!(g.filled_count(), 0);
    }

    #[test]
    fn assign_out_of_range() {
        let mut g = GroupTemplate::standard_group();
        assert_eq!(
            g.assign(99, "Nobody"),
            Err(GroupError::SlotOutOfRange(99, 6))
        );
    }

    #[test]
    fn locked_slot_rejects_assign() {
        let mut g = GroupTemplate::standard_group();
        g.slots[0].locked = true;
        assert_eq!(g.assign(0, "Thorin"), Err(GroupError::SlotLocked(0)));
    }

    #[test]
    fn locked_slot_rejects_unassign() {
        let mut g = GroupTemplate::standard_group();
        g.slots[0].character_name = Some("Thorin".into());
        g.slots[0].locked = true;
        assert_eq!(g.unassign(0), Err(GroupError::SlotLocked(0)));
    }

    #[test]
    fn auto_fill_matches_classes() {
        let mut g = GroupTemplate::standard_group();
        let chars = vec![
            ("Thorin".into(), "Warrior".into()),
            ("Aelara".into(), "Cleric".into()),
            ("Mystik".into(), "Enchanter".into()),
            ("Zappy".into(), "Wizard".into()),
            ("Stabby".into(), "Rogue".into()),
            ("Boomy".into(), "Magician".into()),
            ("Extra".into(), "Monk".into()),
        ];
        let filled = g.auto_fill(&chars);
        assert_eq!(filled, 6);
        assert_eq!(g.filled_count(), 6);
        assert_eq!(g.slots[0].character_name.as_deref(), Some("Thorin"));
        assert_eq!(g.slots[1].character_name.as_deref(), Some("Aelara"));
        assert_eq!(g.slots[2].character_name.as_deref(), Some("Mystik"));
    }

    #[test]
    fn auto_fill_skips_already_assigned() {
        let mut g = GroupTemplate::standard_group();
        g.assign(0, "Thorin").unwrap();

        let chars = vec![
            ("Thorin".into(), "Warrior".into()),
            ("Aelara".into(), "Cleric".into()),
        ];
        let filled = g.auto_fill(&chars);
        assert_eq!(filled, 1);
        assert_eq!(g.slots[1].character_name.as_deref(), Some("Aelara"));
    }

    #[test]
    fn auto_fill_respects_class_preference() {
        let mut g = GroupTemplate::standard_group();
        g.slots[3] = GroupSlot::new(Role::DPS).with_class_preference("Necromancer");

        let chars = vec![
            ("Thorin".into(), "Warrior".into()),
            ("Aelara".into(), "Cleric".into()),
            ("Mystik".into(), "Enchanter".into()),
            ("Bones".into(), "Necromancer".into()),
            ("Zappy".into(), "Wizard".into()),
            ("Stabby".into(), "Rogue".into()),
        ];
        let filled = g.auto_fill(&chars);
        assert_eq!(filled, 6);
        assert_eq!(g.slots[3].character_name.as_deref(), Some("Bones"));
    }

    #[test]
    fn auto_fill_skips_locked_slots() {
        let mut g = GroupTemplate::standard_group();
        g.slots[0].locked = true;

        let chars = vec![("Thorin".into(), "Warrior".into())];
        let filled = g.auto_fill(&chars);
        assert_eq!(filled, 0);
        assert!(g.slots[0].character_name.is_none());
    }

    #[test]
    fn default_builder_state() {
        let state = GroupBuilderState::default();
        assert_eq!(state.templates.len(), 2);
        assert_eq!(state.selected_template, 0);
        assert!(state.available_characters.is_empty());
        assert!(state.active_template().is_some());
    }

    #[test]
    fn role_display() {
        assert_eq!(Role::Tank.to_string(), "Tank");
        assert_eq!(Role::CC.to_string(), "CC");
    }

    #[test]
    fn role_preferred_classes_non_empty() {
        for role in [
            Role::Tank,
            Role::Healer,
            Role::DPS,
            Role::Support,
            Role::Puller,
            Role::CC,
        ] {
            assert!(
                !role.preferred_classes().is_empty(),
                "{role} has no preferred classes"
            );
        }
    }

    #[test]
    fn group_error_display() {
        let e = GroupError::SlotOutOfRange(10, 6);
        assert!(e.to_string().contains("10"));
        assert!(e.to_string().contains("6"));

        let e = GroupError::SlotLocked(2);
        assert!(e.to_string().contains("2"));
    }

    #[test]
    fn serde_round_trip() {
        let mut g = GroupTemplate::standard_group();
        g.assign(0, "Thorin").unwrap();
        let json = serde_json::to_string(&g).unwrap();
        let g2: GroupTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(g, g2);
    }
}
