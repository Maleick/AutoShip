use std::fmt;

use textquest_common::offsets::launch_spell_data;

/// EQ character class IDs.
/// These are the numeric values stored in `CharClass` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EqClass {
    /// Plate tank, high aggro and mitigation.
    Warrior = 1,
    /// Primary healer, Complete Heal chains.
    Cleric = 2,
    /// Holy knight, heals and tanking.
    Paladin = 3,
    /// Bow DPS with tracking and utility heals.
    Ranger = 4,
    /// Dark knight, lifetaps and tanking.
    ShadowKnight = 5,
    /// Nature healer with ports and DoTs.
    Druid = 6,
    /// Melee DPS with feign death and mend.
    Monk = 7,
    /// Songs, crowd control, and run speed.
    Bard = 8,
    /// Stealth melee DPS with backstab.
    Rogue = 9,
    /// Slow, heals, and buffs.
    Shaman = 10,
    /// Pet and DoT caster with feign death.
    Necromancer = 11,
    /// Nuke DPS caster.
    Wizard = 12,
    /// Pet class with direct damage.
    Magician = 13,
    /// Crowd control, haste, and mana regen.
    Enchanter = 14,
    /// Pet melee hybrid with slow.
    Beastlord = 15,
    /// Two-handed melee DPS with frenzy.
    Berserker = 16,
}

impl EqClass {
    /// Converts a numeric class ID to an `EqClass` variant, if valid.
    #[must_use]
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

    /// Returns the 3-letter abbreviation (e.g., WAR, CLR, PAL).
    #[must_use]
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
    /// A player character (type 0).
    Player,
    /// A non-player character (type 1).
    Npc,
    /// A corpse (types 2-3).
    Corpse,
    /// An unrecognized spawn type.
    Unknown(u8),
}

impl SpawnType {
    /// Converts a numeric type ID to a `SpawnType` variant.
    #[must_use]
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
    #[must_use]
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
    /// Normal upright stance (0).
    Standing,
    /// Frozen in place (1).
    Frozen,
    /// Looting a corpse (2).
    Looting,
    /// Sitting for mana/health regen (3).
    Sitting,
    /// Ducking stance (4).
    Ducking,
    /// Feign death (110).
    Feigned,
    /// Dead (111).
    Dead,
    /// Unrecognized standing state.
    Unknown(u8),
}

impl StandState {
    /// Converts a numeric state ID to a `StandState` variant.
    #[must_use]
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
    #[must_use]
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

    /// Returns a short display label for this standing state.
    #[must_use]
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
    /// Returns `true` if this buff slot is empty (no active buff).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spell_id == 0xFFFF || self.spell_id == 0
    }

    /// Duration in seconds.
    #[must_use]
    pub fn duration_secs(&self) -> i32 {
        self.duration_ticks * 6
    }

    /// Formatted duration "M:SS", "Xs", or "PERM" for permanent buffs.
    #[must_use]
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

/// Active spell cast state for a spawn.
/// Backed by `PlayerZoneClient::CastingData`; local spawns may also include gem timers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastDurationSource {
    /// No trustworthy total cast duration is currently available.
    Unknown,
    /// Base spell data from `EQ_Spell::CastTime`.
    ///
    /// This is useful for labels and rough progress, but it does not include
    /// live casting-speed modifiers such as haste, AAs, or focus effects.
    SpellDataBase,
    /// Exact runtime duration captured from a verified live source.
    ExactRuntime,
}

impl CastDurationSource {
    /// Returns `true` when the total cast duration is exact for the live cast in progress.
    #[must_use]
    pub fn is_exact(self) -> bool {
        matches!(self, Self::ExactRuntime)
    }
}

#[derive(Debug, Clone)]
pub struct CastState {
    /// Active spell ID (`-1` = not currently casting).
    pub spell_id: i32,
    /// Spell name resolved from the live spell database, if available.
    pub spell_name: Option<String>,
    /// Target spawn ID for the current cast.
    pub target_id: u32,
    /// Server timestamp when cast completes (0 = not casting).
    pub spell_eta: u32,
    /// Casting item ID, if the spell came from an item click.
    pub item_id: i32,
    /// Active gem slot (0-based). `0xFF` = not currently using a spell gem.
    pub spell_slot: u8,
    /// Remaining cast time in milliseconds, if the display timestamp was available.
    pub remaining_ms: Option<u32>,
    /// Total cast duration in milliseconds, when the backend can determine one.
    pub total_cast_ms: Option<u32>,
    /// Where `total_cast_ms` came from. This lets UI code avoid false precision.
    pub duration_source: CastDurationSource,
    /// Per-gem recast timestamps (15 entries, 0 = ready) for the local player only.
    pub gem_etas: Option<[u32; 15]>,
}

impl CastState {
    /// True if actively casting a spell right now.
    #[must_use]
    pub fn is_casting(&self) -> bool {
        self.spell_id != launch_spell_data::NOT_CASTING_SPELL_ID
    }

    /// Active spell gem number (1-based), if the cast is coming from a memorized gem.
    #[must_use]
    pub fn spell_gem(&self) -> Option<u8> {
        if self.is_casting() && self.spell_slot != launch_spell_data::NOT_CASTING_SPELL_SLOT {
            Some(self.spell_slot + 1)
        } else {
            None
        }
    }

    /// Remaining cast time in milliseconds, if known.
    #[must_use]
    pub fn cast_time_remaining_ms(&self) -> Option<u32> {
        if self.is_casting() {
            self.remaining_ms
        } else {
            None
        }
    }

    /// Total cast duration in milliseconds, if known.
    #[must_use]
    pub fn cast_time_total_ms(&self) -> Option<u32> {
        if self.is_casting() {
            self.total_cast_ms
        } else {
            None
        }
    }

    /// Elapsed cast time in milliseconds, if both total and remaining durations are known.
    #[must_use]
    pub fn cast_time_elapsed_ms(&self) -> Option<u32> {
        let total_ms = self.cast_time_total_ms()?;
        let remaining_ms = self.cast_time_remaining_ms()?;
        Some(total_ms.saturating_sub(remaining_ms))
    }

    /// Normalized cast progress from `0.0` to `1.0`, if both total and remaining are known.
    #[must_use]
    pub fn cast_progress(&self) -> Option<f64> {
        let total_ms = self.cast_time_total_ms()?;
        if total_ms == 0 {
            return None;
        }

        let elapsed_ms = self.cast_time_elapsed_ms()?;
        Some((f64::from(elapsed_ms) / f64::from(total_ms)).clamp(0.0, 1.0))
    }

    /// Returns `true` when `total_cast_ms` is exact for the current live cast.
    #[must_use]
    pub fn has_exact_total_cast_time(&self) -> bool {
        self.is_casting() && self.duration_source.is_exact() && self.total_cast_ms.is_some()
    }

    /// Human-readable timing precision label for UI consumers.
    #[must_use]
    pub fn timing_precision_label(&self) -> &'static str {
        if self.has_exact_total_cast_time() {
            "exact"
        } else {
            "est"
        }
    }
}

/// Group membership info read from `CGroup` in memory.
#[derive(Debug, Clone)]
pub struct GroupInfo {
    /// Name of the group leader.
    pub leader_name: String,
    /// Names of all group members (including leader).
    pub members: Vec<String>,
    /// Number of members in the group.
    pub member_count: u8,
}

/// Extracted spawn data — not a repr(C) struct, but a high-level view
/// built by reading individual fields at their offsets.
#[derive(Debug, Clone)]
pub struct SpawnInfo {
    /// Internal spawn name (may differ from display name).
    pub name: String,
    /// Name shown in-game (with title/suffix applied).
    pub displayed_name: String,
    /// Character surname, if any.
    pub lastname: String,
    /// Unique spawn identifier within the zone.
    pub spawn_id: u32,
    /// Whether this is a PC, NPC, or corpse.
    pub spawn_type: SpawnType,
    /// Character level (1-255).
    pub level: u8,
    /// Raw numeric class identifier.
    pub class_id: u8,
    /// Parsed EQ class, if recognized.
    pub class: Option<EqClass>,
    /// Current standing/sitting/feigned state.
    pub stand_state: StandState,
    /// X coordinate in the zone.
    pub x: f32,
    /// Y coordinate in the zone.
    pub y: f32,
    /// Z coordinate (altitude) in the zone.
    pub z: f32,
    /// Facing direction in degrees.
    pub heading: f32,
    /// Current hit points.
    pub hp_current: i64,
    /// Maximum hit points.
    pub hp_max: i64,
    /// Current mana.
    pub mana_current: i32,
    /// Maximum mana.
    pub mana_max: i32,
    /// Current endurance.
    pub endurance_current: i32,
    /// Maximum endurance.
    pub endurance_max: u32,
    /// Whether this spawn is a GM.
    pub is_gm: bool,
    /// Race ID from `ActorClient` (e.g., Human=1, Barbarian=2, etc.)
    pub race_id: u32,
    /// Active buff slots (populated only for local player via `read_buff_slots`).
    pub buff_slots: Vec<BuffSlot>,
    /// Cast state for this spawn. Local player snapshots also include gem recast timers.
    pub cast_state: Option<CastState>,
}

impl SpawnInfo {
    /// Returns the current HP as a percentage (0.0-100.0).
    #[must_use]
    pub fn hp_pct(&self) -> f64 {
        if self.hp_max > 0 {
            (self.hp_current as f64 / self.hp_max as f64) * 100.0
        } else {
            100.0
        }
    }

    /// Returns the current mana as a percentage (0.0-100.0).
    #[must_use]
    pub fn mana_pct(&self) -> f64 {
        if self.mana_max > 0 {
            (f64::from(self.mana_current) / f64::from(self.mana_max)) * 100.0
        } else {
            100.0
        }
    }

    /// Returns the short class name string (e.g., "WAR"), or "?cN?" if unknown.
    #[must_use]
    pub fn class_str(&self) -> String {
        self.class
            .as_ref()
            .map_or(format!("?c{}?", self.class_id), |c| {
                c.short_name().to_string()
            })
    }

    /// Human-readable race name from the numeric race ID.
    #[must_use]
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

    fn make_cast_state() -> CastState {
        CastState {
            spell_id: 123,
            spell_name: Some("Test Spell".to_string()),
            target_id: 456,
            spell_eta: 1_234,
            item_id: 0,
            spell_slot: 0,
            remaining_ms: Some(900),
            total_cast_ms: Some(1_200),
            duration_source: CastDurationSource::SpellDataBase,
            gem_etas: Some([0; 15]),
        }
    }

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
        let cs = make_cast_state();
        assert!(cs.is_casting());
        assert_eq!(cs.spell_gem(), Some(1));
    }

    #[test]
    fn cast_state_not_casting_when_spell_id_minus_one() {
        let cs = CastState {
            spell_id: -1,
            spell_name: None,
            target_id: 0,
            spell_eta: 0,
            item_id: 0,
            spell_slot: 0xFF,
            remaining_ms: None,
            total_cast_ms: None,
            duration_source: CastDurationSource::Unknown,
            gem_etas: Some([0; 15]),
        };
        assert!(!cs.is_casting());
    }

    #[test]
    fn cast_state_item_click_can_cast_without_spell_slot() {
        let cs = CastState {
            spell_id: 444,
            spell_name: Some("Wand of Test".to_string()),
            target_id: 77,
            spell_eta: 0,
            item_id: 999,
            spell_slot: 0xFF,
            remaining_ms: Some(0),
            total_cast_ms: None,
            duration_source: CastDurationSource::Unknown,
            gem_etas: None,
        };
        assert!(cs.is_casting());
        assert_eq!(cs.spell_gem(), None);
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
    fn cast_state_remaining_ms_none_when_unknown() {
        let cs = CastState {
            spell_id: 55,
            spell_name: Some("Unknown Timing".to_string()),
            target_id: 0,
            spell_eta: 1000,
            item_id: 0,
            spell_slot: 5,
            remaining_ms: None,
            total_cast_ms: Some(2_500),
            duration_source: CastDurationSource::SpellDataBase,
            gem_etas: None,
        };
        assert_eq!(cs.cast_time_remaining_ms(), None);
    }

    // --- SpawnType::as_str tests ---

    #[test]
    fn spawn_type_as_str_player() {
        assert_eq!(SpawnType::Player.as_str(), "PC");
    }

    #[test]
    fn spawn_type_as_str_npc() {
        assert_eq!(SpawnType::Npc.as_str(), "NPC");
    }

    #[test]
    fn spawn_type_as_str_corpse() {
        assert_eq!(SpawnType::Corpse.as_str(), "Corpse");
    }

    #[test]
    fn spawn_type_as_str_unknown() {
        assert_eq!(SpawnType::Unknown(42).as_str(), "Unknown");
        assert_eq!(SpawnType::Unknown(0).as_str(), "Unknown");
    }

    // --- race_name full coverage ---

    #[test]
    fn spawn_info_race_name_all_known() {
        let mut s = make_spawn_info(1);
        let expected = [
            (1, "Human"),
            (2, "Barbarian"),
            (3, "Erudite"),
            (4, "Wood Elf"),
            (5, "High Elf"),
            (6, "Dark Elf"),
            (7, "Half Elf"),
            (8, "Dwarf"),
            (9, "Troll"),
            (10, "Ogre"),
            (11, "Halfling"),
            (12, "Gnome"),
            (128, "Iksar"),
            (130, "Vah Shir"),
            (330, "Froglok"),
            (522, "Drakkin"),
        ];
        for (id, name) in expected {
            s.race_id = id;
            assert_eq!(s.race_name(), name, "race_id {} should be {}", id, name);
        }
    }

    #[test]
    fn spawn_info_race_name_unknown_formats_with_id() {
        let mut s = make_spawn_info(1);
        s.race_id = 255;
        assert_eq!(s.race_name(), "R255");
        s.race_id = 65535;
        assert_eq!(s.race_name(), "R65535");
    }

    // --- GroupInfo construction ---

    #[test]
    fn group_info_construction() {
        let g = GroupInfo {
            leader_name: "Camrene".to_string(),
            members: vec!["Camrene".into(), "Zisdarenu".into()],
            member_count: 2,
        };
        assert_eq!(g.leader_name, "Camrene");
        assert_eq!(g.member_count, 2);
        assert_eq!(g.members.len(), 2);
    }

    // --- BuffSlot edge cases ---

    #[test]
    fn buff_slot_large_duration_formats_correctly() {
        // 100 ticks * 6 = 600s = 10:00
        let b = BuffSlot {
            spell_id: 42,
            duration_ticks: 100,
            caster_level: 60,
        };
        assert_eq!(b.duration_str(), "10:00");
        assert_eq!(b.duration_secs(), 600);
    }

    #[test]
    fn buff_slot_one_tick_formats_as_seconds() {
        let b = BuffSlot {
            spell_id: 42,
            duration_ticks: 1,
            caster_level: 60,
        };
        assert_eq!(b.duration_str(), "6s");
        assert_eq!(b.duration_secs(), 6);
    }

    #[test]
    fn buff_slot_is_empty_boundary() {
        // 0xFFFE is NOT empty
        let b = BuffSlot {
            spell_id: 0xFFFE,
            duration_ticks: 0,
            caster_level: 0,
        };
        assert!(!b.is_empty());
    }

    // --- CastState edge cases ---

    #[test]
    fn cast_state_last_gem_slot() {
        let cs = CastState {
            spell_id: 999,
            spell_name: Some("Last Gem".to_string()),
            target_id: 999,
            spell_eta: 99999,
            item_id: 0,
            spell_slot: 14,
            remaining_ms: Some(5000),
            total_cast_ms: Some(10_000),
            duration_source: CastDurationSource::SpellDataBase,
            gem_etas: Some([0; 15]),
        };
        assert!(cs.is_casting());
        assert_eq!(cs.spell_gem(), Some(15));
    }

    #[test]
    fn cast_state_slot_0xfe_still_reports_a_gem() {
        let cs = CastState {
            spell_id: 1,
            spell_name: Some("Odd Slot".to_string()),
            target_id: 1,
            spell_eta: 100,
            item_id: 0,
            spell_slot: 0xFE,
            remaining_ms: Some(50),
            total_cast_ms: Some(100),
            duration_source: CastDurationSource::SpellDataBase,
            gem_etas: Some([0; 15]),
        };
        assert!(cs.is_casting());
        assert_eq!(cs.spell_gem(), Some(0xFF));
    }

    #[test]
    fn cast_state_reports_remaining_ms_when_known() {
        let cs = CastState {
            spell_id: 11,
            spell_name: Some("Remaining".to_string()),
            target_id: 123,
            spell_eta: 1200,
            item_id: 0,
            spell_slot: 2,
            remaining_ms: Some(250),
            total_cast_ms: Some(3_000),
            duration_source: CastDurationSource::SpellDataBase,
            gem_etas: None,
        };
        assert_eq!(cs.cast_time_remaining_ms(), Some(250));
    }

    #[test]
    fn cast_state_reports_total_elapsed_and_progress_when_known() {
        let cs = make_cast_state();
        assert_eq!(cs.cast_time_total_ms(), Some(1_200));
        assert_eq!(cs.cast_time_elapsed_ms(), Some(300));
        assert_eq!(cs.cast_progress(), Some(0.25));
        assert!(!cs.has_exact_total_cast_time());
    }

    #[test]
    fn cast_duration_source_exact_runtime_is_exact() {
        assert!(CastDurationSource::ExactRuntime.is_exact());
        assert!(!CastDurationSource::Unknown.is_exact());
        assert!(!CastDurationSource::SpellDataBase.is_exact());
    }

    #[test]
    fn cast_state_precision_label_matches_duration_source() {
        let mut exact = make_cast_state();
        exact.duration_source = CastDurationSource::ExactRuntime;
        assert_eq!(exact.timing_precision_label(), "exact");

        let estimated = make_cast_state();
        assert_eq!(estimated.timing_precision_label(), "est");
    }

    // --- SpawnInfo display edge cases ---

    #[test]
    fn spawn_info_display_corpse_type() {
        let mut s = make_spawn_info(1);
        s.spawn_type = SpawnType::Corpse;
        let display = format!("{s}");
        assert!(display.contains("Corpse"));
    }

    #[test]
    fn spawn_info_hp_pct_negative_current() {
        let mut s = make_spawn_info(1);
        s.hp_current = -500;
        s.hp_max = 10000;
        // Negative current HP should give negative percentage
        assert!(s.hp_pct() < 0.0);
    }

    // --- StandState edge cases ---

    #[test]
    fn stand_state_from_id_all_known_ids() {
        // Ensure specific IDs map to the correct variants
        assert_eq!(StandState::from_id(0), StandState::Standing);
        assert_eq!(StandState::from_id(1), StandState::Frozen);
        assert_eq!(StandState::from_id(2), StandState::Looting);
        assert_eq!(StandState::from_id(3), StandState::Sitting);
        assert_eq!(StandState::from_id(4), StandState::Ducking);
        assert_eq!(StandState::from_id(110), StandState::Feigned);
        assert_eq!(StandState::from_id(111), StandState::Dead);
    }

    #[test]
    fn stand_state_display_all_known() {
        assert_eq!(format!("{}", StandState::Frozen), "Frozen");
        assert_eq!(format!("{}", StandState::Looting), "Loot");
        assert_eq!(format!("{}", StandState::Sitting), "Sit");
        assert_eq!(format!("{}", StandState::Ducking), "Duck");
        assert_eq!(format!("{}", StandState::Feigned), "FD");
        assert_eq!(format!("{}", StandState::Unknown(42)), "???");
    }

    // --- EqClass Display ---

    #[test]
    fn eq_class_display_all_classes() {
        let pairs = [
            (EqClass::Cleric, "CLR"),
            (EqClass::Paladin, "PAL"),
            (EqClass::Ranger, "RNG"),
            (EqClass::ShadowKnight, "SK"),
            (EqClass::Druid, "DRU"),
            (EqClass::Monk, "MNK"),
            (EqClass::Bard, "BRD"),
            (EqClass::Rogue, "ROG"),
            (EqClass::Shaman, "SHM"),
            (EqClass::Necromancer, "NEC"),
            (EqClass::Wizard, "WIZ"),
            (EqClass::Magician, "MAG"),
            (EqClass::Enchanter, "ENC"),
            (EqClass::Beastlord, "BST"),
            (EqClass::Berserker, "BER"),
        ];
        for (class, expected) in pairs {
            assert_eq!(format!("{}", class), expected);
        }
    }
}
