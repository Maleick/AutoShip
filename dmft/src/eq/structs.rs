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

/// A single active buff/song slot from the CharacterZoneClient buff array.
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
            format!("{}:{:02}", m, s)
        } else {
            format!("{}s", s)
        }
    }
}

/// Active spell cast state for the local player.
/// Read from CharacterZoneClient via PINST_LOCAL_PC.
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

/// Group membership info read from CGroup in memory.
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
    /// Active buff slots (populated only for local player via read_buff_slots).
    pub buff_slots: Vec<BuffSlot>,
    /// Cast state (populated only for local player via read_cast_state).
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
}
