use std::fmt;

/// EQ character class IDs.
/// These are the numeric values stored in CharClass field.
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
}

impl fmt::Display for SpawnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Player => write!(f, "PC"),
            Self::Npc => write!(f, "NPC"),
            Self::Corpse => write!(f, "Corpse"),
            Self::Unknown(id) => write!(f, "Unknown({})", id),
        }
    }
}

/// Standing state values from STANDSTATE offset (0x0574 in PlayerZoneClient).
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
