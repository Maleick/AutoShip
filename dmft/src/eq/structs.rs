use std::fmt;

/// EQ character class IDs.
/// These are the numeric values stored in `CharClass` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EqClass {
    Warrior = 1,
    Cleric = 2,
    Paladin = 3,
    Ranger = 4,
    ShadowKnight = 5,
    Druid = 6,
    Monk = 7,
    Bard = 8,
    Rogue = 9,
    Shaman = 10,
    Necromancer = 11,
    Wizard = 12,
    Magician = 13,
    Enchanter = 14,
    Beastlord = 15,
    Berserker = 16,
}

impl EqClass {
    pub fn from_id(id: u8) -> Option<Self> {
        match id {
            1 => Some(Self::Warrior),
            2 => Some(Self::Cleric),
            3 => Some(Self::Paladin),
            4 => Some(Self::Ranger),
            5 => Some(Self::ShadowKnight),
            6 => Some(Self::Druid),
            7 => Some(Self::Monk),
            8 => Some(Self::Bard),
            9 => Some(Self::Rogue),
            10 => Some(Self::Shaman),
            11 => Some(Self::Necromancer),
            12 => Some(Self::Wizard),
            13 => Some(Self::Magician),
            14 => Some(Self::Enchanter),
            15 => Some(Self::Beastlord),
            16 => Some(Self::Berserker),
            _ => None,
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Warrior => "WAR",
            Self::Cleric => "CLR",
            Self::Paladin => "PAL",
            Self::Ranger => "RNG",
            Self::ShadowKnight => "SK",
            Self::Druid => "DRU",
            Self::Monk => "MNK",
            Self::Bard => "BRD",
            Self::Rogue => "ROG",
            Self::Shaman => "SHM",
            Self::Necromancer => "NEC",
            Self::Wizard => "WIZ",
            Self::Magician => "MAG",
            Self::Enchanter => "ENC",
            Self::Beastlord => "BST",
            Self::Berserker => "BER",
        }
    }
}

impl fmt::Display for EqClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

/// Spawn type values (from PlayerBase.Type field).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnType {
    Player,
    Npc,
    Corpse,
    Unknown(u8),
}

impl SpawnType {
    pub fn from_id(id: u8) -> Self {
        match id {
            0 => Self::Player,
            1 => Self::Npc,
            2 | 3 => Self::Corpse,
            other => Self::Unknown(other),
        }
    }

    /// Return a static string label suitable for display and filtering.
    /// Avoids a heap allocation compared to `to_string()`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Player => "PC",
            Self::Npc => "NPC",
            Self::Corpse => "Corpse",
            Self::Unknown(_) => "Unknown",
        }
    }
}

impl fmt::Display for SpawnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Player => write!(f, "PC"),
            Self::Npc => write!(f, "NPC"),
            Self::Corpse => write!(f, "Corpse"),
            Self::Unknown(id) => write!(f, "Unknown({id})"),
        }
    }
}

/// Standing state values from STANDSTATE offset (0x0574 in `PlayerZoneClient`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandState {
    Standing,
    Frozen,
    Looting,
    Sitting,
    Ducking,
    Feigned,
    Dead,
    Unknown(u8),
}

impl StandState {
    pub fn from_id(id: u8) -> Self {
        match id {
            0 => Self::Standing,
            1 => Self::Frozen,
            2 => Self::Looting,
            3 => Self::Sitting,
            4 => Self::Ducking,
            110 => Self::Feigned,
            111 => Self::Dead,
            other => Self::Unknown(other),
        }
    }

    /// Small ASCII sprite representing the character's current state.
    pub fn sprite(&self) -> &'static str {
        match self {
            Self::Standing => " O \n/|\\\n/ \\",
            Self::Frozen => " O \n/|\\\n | ",
            Self::Looting => " O \n/|\\\n\\ /",
            Self::Sitting => " O \n/|\\\n--'",
            Self::Ducking => " O \n/| \n/ \\",
            Self::Feigned => "____\n-O- \n----",
            Self::Dead => " X \n/|\\\n/ \\",
            Self::Unknown(_) => " ? \n | \n/ \\",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Standing => "Stand",
            Self::Frozen => "Frozen",
            Self::Looting => "Loot",
            Self::Sitting => "Sit",
            Self::Ducking => "Duck",
            Self::Feigned => "FD",
            Self::Dead => "DEAD",
            Self::Unknown(_) => "???",
        }
    }
}

impl fmt::Display for StandState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// A single active buff/song slot from the `CharacterZoneClient` buff array.
#[derive(Debug, Clone)]
pub struct BuffSlot {
    /// Spell ID (0xFFFF = empty).
    pub spell_id: u32,
    /// Remaining ticks (multiply by 6 for seconds). 0 = permanent.
    pub duration_ticks: i32,
    /// Level of the caster who applied the buff.
    pub caster_level: u8,
}

impl BuffSlot {
    pub fn is_empty(&self) -> bool {
        self.spell_id == 0xFFFF || self.spell_id == 0
    }

    /// Duration in seconds.
    pub fn duration_secs(&self) -> i32 {
        self.duration_ticks * 6
    }

    /// Formatted duration "M:SS", "Xs", or "PERM" for permanent buffs.
    pub fn duration_str(&self) -> String {
        if self.duration_ticks <= 0 {
            return "PERM".to_string();
        }
        let secs = self.duration_secs();
        let m = secs / 60;
        let s = secs % 60;
        if m > 0 {
            format!("{m}:{s:02}")
        } else {
            format!("{s}s")
        }
    }
}

/// Active spell cast state for the local player.
/// Read from `CharacterZoneClient` via `PINST_LOCAL_PC`.
#[derive(Debug, Clone)]
pub struct CastState {
    /// Active gem slot (0-based). 0xFF = not currently casting.
    pub spell_slot: u8,
    /// Server timestamp when cast completes (0 = not casting).
    pub spell_eta: u32,
    /// Per-gem recast timestamps (15 entries, 0 = ready).
    pub gem_etas: [u32; 15],
}

impl CastState {
    /// True if actively casting a spell right now.
    pub fn is_casting(&self) -> bool {
        self.spell_slot != 0xFF && self.spell_eta != 0
    }
}

/// Group membership info read from `CGroup` in memory.
#[derive(Debug, Clone)]
pub struct GroupInfo {
    pub leader_name: String,
    pub members: Vec<String>,
    pub member_count: u8,
}

/// Extracted spawn data — not a repr(C) struct, but a high-level view
/// built by reading individual fields at their offsets.
#[derive(Debug, Clone)]
pub struct SpawnInfo {
    pub name: String,
    pub displayed_name: String,
    pub lastname: String,
    pub spawn_id: u32,
    pub spawn_type: SpawnType,
    pub level: u8,
    pub class_id: u8,
    pub class: Option<EqClass>,
    pub stand_state: StandState,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub heading: f32,
    pub hp_current: i64,
    pub hp_max: i64,
    pub mana_current: i32,
    pub mana_max: i32,
    pub endurance_current: i32,
    pub endurance_max: u32,
    pub is_gm: bool,
    /// Race ID from `ActorClient` (e.g., Human=1, Barbarian=2, etc.)
    pub race_id: u32,
    /// Active buff slots (populated only for local player via `read_buff_slots`).
    pub buff_slots: Vec<BuffSlot>,
    /// Cast state (populated only for local player via `read_cast_state`).
    pub cast_state: Option<CastState>,
}

impl SpawnInfo {
    pub fn hp_pct(&self) -> f64 {
        if self.hp_max > 0 {
            (self.hp_current as f64 / self.hp_max as f64) * 100.0
        } else {
            100.0
        }
    }

    pub fn mana_pct(&self) -> f64 {
        if self.mana_max > 0 {
            (self.mana_current as f64 / self.mana_max as f64) * 100.0
        } else {
            100.0
        }
    }

    pub fn class_str(&self) -> String {
        self.class
            .as_ref()
            .map_or(format!("?c{}?", self.class_id), |c| {
                c.short_name().to_string()
            })
    }

    /// Human-readable race name from the numeric race ID.
    pub fn race_name(&self) -> String {
        match self.race_id {
            1 => "Human".to_string(),
            2 => "Barbarian".to_string(),
            3 => "Erudite".to_string(),
            4 => "Wood Elf".to_string(),
            5 => "High Elf".to_string(),
            6 => "Dark Elf".to_string(),
            7 => "Half Elf".to_string(),
            8 => "Dwarf".to_string(),
            9 => "Troll".to_string(),
            10 => "Ogre".to_string(),
            11 => "Halfling".to_string(),
            12 => "Gnome".to_string(),
            128 => "Iksar".to_string(),
            130 => "Vah Shir".to_string(),
            330 => "Froglok".to_string(),
            522 => "Drakkin".to_string(),
            0 => "Unknown".to_string(),
            id => format!("R{id}"),
        }
    }
}

impl fmt::Display for SpawnInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} ({}) Lv{} {} HP:{}/{} ({:.0}%) Mana:{}/{} Pos:({:.1},{:.1},{:.1})",
            self.spawn_type,
            self.displayed_name,
            self.class_str(),
            self.level,
            self.spawn_id,
            self.hp_current,
            self.hp_max,
            self.hp_pct(),
            self.mana_current,
            self.mana_max,
            self.y,
            self.x,
            self.z,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buff_slot_empty_detection() {
        let empty = BuffSlot {
            spell_id: 0xFFFF,
            duration_ticks: 0,
            caster_level: 0,
        };
        assert!(empty.is_empty());
        let zero_id = BuffSlot {
            spell_id: 0,
            duration_ticks: 100,
            caster_level: 60,
        };
        assert!(zero_id.is_empty());
        let active = BuffSlot {
            spell_id: 1,
            duration_ticks: 100,
            caster_level: 60,
        };
        assert!(!active.is_empty());
    }

    #[test]
    fn buff_slot_duration_str_permanent() {
        let perm = BuffSlot {
            spell_id: 1,
            duration_ticks: 0,
            caster_level: 60,
        };
        assert_eq!(perm.duration_str(), "PERM");
    }

    #[test]
    fn buff_slot_duration_str_minutes() {
        // 10 ticks * 6 sec/tick = 60 seconds = 1:00
        let b = BuffSlot {
            spell_id: 1,
            duration_ticks: 10,
            caster_level: 60,
        };
        assert_eq!(b.duration_str(), "1:00");
    }

    #[test]
    fn buff_slot_duration_str_seconds_only() {
        // 3 ticks * 6 = 18 seconds
        let b = BuffSlot {
            spell_id: 1,
            duration_ticks: 3,
            caster_level: 60,
        };
        assert_eq!(b.duration_str(), "18s");
    }

    #[test]
    fn cast_state_is_casting_true() {
        let cs = CastState {
            spell_slot: 0,
            spell_eta: 12345,
            gem_etas: [0; 15],
        };
        assert!(cs.is_casting());
    }

    #[test]
    fn cast_state_not_casting_when_slot_ff() {
        let cs = CastState {
            spell_slot: 0xFF,
            spell_eta: 0,
            gem_etas: [0; 15],
        };
        assert!(!cs.is_casting());
    }

    #[test]
    fn cast_state_not_casting_when_eta_zero() {
        let cs = CastState {
            spell_slot: 0,
            spell_eta: 0,
            gem_etas: [0; 15],
        };
        assert!(!cs.is_casting());
    }

    #[test]
    fn eq_class_all_16_from_id_round_trip() {
        let expected = [
            (1, "WAR"),
            (2, "CLR"),
            (3, "PAL"),
            (4, "RNG"),
            (5, "SK"),
            (6, "DRU"),
            (7, "MNK"),
            (8, "BRD"),
            (9, "ROG"),
            (10, "SHM"),
            (11, "NEC"),
            (12, "WIZ"),
            (13, "MAG"),
            (14, "ENC"),
            (15, "BST"),
            (16, "BER"),
        ];
        for (id, short) in expected {
            let class =
                EqClass::from_id(id).unwrap_or_else(|| panic!("from_id({}) returned None", id));
            assert_eq!(
                class.short_name(),
                short,
                "class id {} short_name mismatch",
                id
            );
        }
    }

    #[test]
    fn eq_class_from_id_invalid_returns_none() {
        assert!(EqClass::from_id(0).is_none());
        assert!(EqClass::from_id(17).is_none());
        assert!(EqClass::from_id(255).is_none());
    }

    #[test]
    fn spawn_type_known_values() {
        assert_eq!(SpawnType::from_id(0), SpawnType::Player);
        assert_eq!(SpawnType::from_id(1), SpawnType::Npc);
        assert_eq!(SpawnType::from_id(2), SpawnType::Corpse);
        assert_eq!(SpawnType::from_id(3), SpawnType::Corpse);
    }

    #[test]
    fn spawn_type_unknown_values() {
        assert_eq!(SpawnType::from_id(4), SpawnType::Unknown(4));
        assert_eq!(SpawnType::from_id(99), SpawnType::Unknown(99));
    }

    #[test]
    fn stand_state_all_known_values() {
        let expected = [
            (0, "Stand"),
            (1, "Frozen"),
            (2, "Loot"),
            (3, "Sit"),
            (4, "Duck"),
            (110, "FD"),
            (111, "DEAD"),
        ];
        for (id, label) in expected {
            let state = StandState::from_id(id);
            assert_eq!(state.label(), label, "StandState id {} label mismatch", id);
            assert!(
                !state.sprite().is_empty(),
                "StandState id {} has empty sprite",
                id
            );
        }
    }

    #[test]
    fn stand_state_unknown() {
        let state = StandState::from_id(50);
        assert_eq!(state, StandState::Unknown(50));
        assert_eq!(state.label(), "???");
        assert!(!state.sprite().is_empty());
    }

    fn make_spawn_info(class_id: u8) -> SpawnInfo {
        SpawnInfo {
            name: "TestPlayer".into(),
            displayed_name: "TestPlayer".into(),
            lastname: String::new(),
            spawn_id: 1,
            spawn_type: SpawnType::Player,
            level: 60,
            class_id,
            class: EqClass::from_id(class_id),
            stand_state: StandState::Standing,
            x: 100.0,
            y: 200.0,
            z: 10.0,
            heading: 0.0,
            hp_current: 7500,
            hp_max: 10000,
            mana_current: 3000,
            mana_max: 5000,
            endurance_current: 100,
            endurance_max: 100,
            is_gm: false,
            race_id: 1,
            buff_slots: Vec::new(),
            cast_state: None,
        }
    }

    #[test]
    fn spawn_info_hp_pct() {
        let s = make_spawn_info(1);
        assert!((s.hp_pct() - 75.0).abs() < 0.01);
    }

    #[test]
    fn spawn_info_hp_pct_zero_max() {
        let mut s = make_spawn_info(1);
        s.hp_max = 0;
        assert!((s.hp_pct() - 100.0).abs() < 0.01);
    }

    #[test]
    fn spawn_info_mana_pct() {
        let s = make_spawn_info(2);
        assert!((s.mana_pct() - 60.0).abs() < 0.01);
    }

    #[test]
    fn spawn_info_mana_pct_zero_max() {
        let mut s = make_spawn_info(1);
        s.mana_max = 0;
        assert!((s.mana_pct() - 100.0).abs() < 0.01);
    }

    #[test]
    fn spawn_info_class_str_known() {
        let s = make_spawn_info(1);
        assert_eq!(s.class_str(), "WAR");
    }

    #[test]
    fn spawn_info_class_str_unknown() {
        let s = make_spawn_info(99);
        assert_eq!(s.class_str(), "?c99?");
    }

    #[test]
    fn spawn_info_race_names() {
        let mut s = make_spawn_info(1);
        s.race_id = 1;
        assert_eq!(s.race_name(), "Human");
        s.race_id = 9;
        assert_eq!(s.race_name(), "Troll");
        s.race_id = 128;
        assert_eq!(s.race_name(), "Iksar");
        s.race_id = 0;
        assert_eq!(s.race_name(), "Unknown");
        s.race_id = 9999;
        assert_eq!(s.race_name(), "R9999");
    }

    #[test]
    fn spawn_info_display() {
        let s = make_spawn_info(2);
        let display = format!("{s}");
        assert!(display.contains("TestPlayer"));
        assert!(display.contains("CLR"));
        assert!(display.contains("Lv60"));
    }

    #[test]
    fn eq_class_display() {
        let c = EqClass::Warrior;
        assert_eq!(format!("{c}"), "WAR");
    }

    #[test]
    fn spawn_type_display() {
        assert_eq!(format!("{}", SpawnType::Player), "PC");
        assert_eq!(format!("{}", SpawnType::Npc), "NPC");
        assert_eq!(format!("{}", SpawnType::Corpse), "Corpse");
        assert_eq!(format!("{}", SpawnType::Unknown(5)), "Unknown(5)");
    }

    #[test]
    fn stand_state_display() {
        assert_eq!(format!("{}", StandState::Standing), "Stand");
        assert_eq!(format!("{}", StandState::Dead), "DEAD");
    }

    #[test]
    fn buff_slot_duration_secs() {
        let b = BuffSlot {
            spell_id: 1,
            duration_ticks: 5,
            caster_level: 60,
        };
        assert_eq!(b.duration_secs(), 30);
    }

    #[test]
    fn buff_slot_negative_duration_is_perm() {
        let b = BuffSlot {
            spell_id: 1,
            duration_ticks: -1,
            caster_level: 60,
        };
        assert_eq!(b.duration_str(), "PERM");
    }

    #[test]
    fn cast_state_slot_nonff_but_eta_zero_not_casting() {
        let cs = CastState {
            spell_slot: 5,
            spell_eta: 0,
            gem_etas: [0; 15],
        };
        assert!(!cs.is_casting());
    }
}
