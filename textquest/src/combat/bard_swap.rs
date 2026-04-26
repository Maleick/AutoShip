//! Bard instrument swap — MQ2BardSwap parity.
//!
//! Before a bard casts a song, the correct instrument must be equipped in the
//! primary or secondary slot to obtain the instrument modifier bonus.  This
//! module tracks per-character instrument-slot state and emits swap/restore
//! commands that the input layer translates into inventory clicks.
//!
//! Coordinates with the Twist/Medley system (#2438) by exposing the swap API
//! without owning the song queue.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use textquest_common::types::ClientId;

/// EQ instrument family — determines which item slot to swap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InstrumentType {
    /// No instrument needed (singing modifier only).
    None,
    /// Stringed instruments — lute, mandolin.
    String,
    /// Brass instruments — horn, trumpet.
    Brass,
    /// Wind instruments — flute, pipes.
    Wind,
    /// Percussion instruments — drum, tambourine.
    Percussion,
}

impl InstrumentType {
    /// True if this type requires a physical item swap.
    #[must_use]
    pub fn requires_swap(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// A named song and the instrument family it requires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongEntry {
    /// Lowercase partial name match — e.g. `"chant of battle"`.
    pub name_pattern: String,
    pub instrument: InstrumentType,
}

/// Per-character bard swap configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BardSwapConfig {
    pub enabled: bool,
    /// Ordered list of songs to instrument mappings.
    pub songs: Vec<SongEntry>,
}

impl Default for BardSwapConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            songs: default_song_map(),
        }
    }
}

/// Reasonable defaults covering classic TLP bard songs.
fn default_song_map() -> Vec<SongEntry> {
    vec![
        // Strings — Lyssa's / Largo's / Selo's
        SongEntry {
            name_pattern: "lyssa".into(),
            instrument: InstrumentType::String,
        },
        SongEntry {
            name_pattern: "largo".into(),
            instrument: InstrumentType::String,
        },
        SongEntry {
            name_pattern: "selo".into(),
            instrument: InstrumentType::String,
        },
        SongEntry {
            name_pattern: "chant of".into(),
            instrument: InstrumentType::String,
        },
        // Brass — Brusco's / Anthem / Jonthan's
        SongEntry {
            name_pattern: "brusco".into(),
            instrument: InstrumentType::Brass,
        },
        SongEntry {
            name_pattern: "anthem".into(),
            instrument: InstrumentType::Brass,
        },
        SongEntry {
            name_pattern: "jonthan".into(),
            instrument: InstrumentType::Brass,
        },
        // Wind — Cassindra's / Hymn of Restoration / Warsong
        SongEntry {
            name_pattern: "cassindra".into(),
            instrument: InstrumentType::Wind,
        },
        SongEntry {
            name_pattern: "hymn of restoration".into(),
            instrument: InstrumentType::Wind,
        },
        SongEntry {
            name_pattern: "warsong".into(),
            instrument: InstrumentType::Wind,
        },
        // Percussion — Angstlich's / Denon's
        SongEntry {
            name_pattern: "angstlich".into(),
            instrument: InstrumentType::Percussion,
        },
        SongEntry {
            name_pattern: "denon".into(),
            instrument: InstrumentType::Percussion,
        },
    ]
}

/// Commands emitted by the swap manager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BardSwapCommand {
    /// Equip the specified instrument type before casting.
    EquipInstrument(InstrumentType),
    /// Restore the previous weapon after the cast completes.
    RestoreWeapon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SwapState {
    Idle,
    Swapped(InstrumentType),
}

pub struct BardSwapManager {
    configs: HashMap<ClientId, BardSwapConfig>,
    states: HashMap<ClientId, SwapState>,
}

impl BardSwapManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            states: HashMap::new(),
        }
    }

    pub fn set_config(&mut self, client_id: ClientId, config: BardSwapConfig) {
        self.configs.insert(client_id, config);
        self.states.insert(client_id, SwapState::Idle);
    }

    pub fn get_config(&self, client_id: ClientId) -> Option<&BardSwapConfig> {
        self.configs.get(&client_id)
    }

    pub fn remove_client(&mut self, client_id: ClientId) {
        self.configs.remove(&client_id);
        self.states.remove(&client_id);
    }

    /// Call before casting `song_name`.  Returns `EquipInstrument` if a swap
    /// is needed, `None` if already equipped or no instrument required.
    pub fn before_cast(&mut self, client_id: ClientId, song_name: &str) -> Option<BardSwapCommand> {
        let config = self.configs.get(&client_id)?;
        if !config.enabled {
            return None;
        }

        let instrument = resolve_instrument(&config.songs, song_name);
        if !instrument.requires_swap() {
            return None;
        }

        let state = self.states.entry(client_id).or_insert(SwapState::Idle);
        if *state == SwapState::Swapped(instrument) {
            return None; // already holding the right instrument
        }

        *state = SwapState::Swapped(instrument);
        Some(BardSwapCommand::EquipInstrument(instrument))
    }

    /// Call after the cast completes or is interrupted.  Returns
    /// `RestoreWeapon` if an instrument was swapped in.
    pub fn after_cast(&mut self, client_id: ClientId) -> Option<BardSwapCommand> {
        let state = self.states.get_mut(&client_id)?;
        if *state == SwapState::Idle {
            return None;
        }
        *state = SwapState::Idle;
        Some(BardSwapCommand::RestoreWeapon)
    }

    /// Lookup only — returns the instrument a song requires without mutating
    /// state.  Used by the Twist scheduler (#2438) to pre-plan swaps.
    pub fn instrument_for_song(&self, client_id: ClientId, song_name: &str) -> InstrumentType {
        self.configs
            .get(&client_id)
            .map_or(InstrumentType::None, |cfg| {
                resolve_instrument(&cfg.songs, song_name)
            })
    }
}

impl Default for BardSwapManager {
    fn default() -> Self {
        Self::new()
    }
}

fn resolve_instrument(songs: &[SongEntry], song_name: &str) -> InstrumentType {
    let lower = song_name.to_ascii_lowercase();
    songs
        .iter()
        .find(|e| lower.contains(e.name_pattern.as_str()))
        .map_or(InstrumentType::None, |e| e.instrument)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_enabled_config() -> BardSwapConfig {
        BardSwapConfig {
            enabled: true,
            ..BardSwapConfig::default()
        }
    }

    #[test]
    fn disabled_config_no_swap() {
        let mut mgr = BardSwapManager::new();
        mgr.set_config(
            1,
            BardSwapConfig {
                enabled: false,
                ..BardSwapConfig::default()
            },
        );
        assert_eq!(mgr.before_cast(1, "Lyssa's Solidarity of Vision"), None);
    }

    #[test]
    fn string_song_equips_string_instrument() {
        let mut mgr = BardSwapManager::new();
        mgr.set_config(1, default_enabled_config());
        assert_eq!(
            mgr.before_cast(1, "Lyssa's Solidarity of Vision"),
            Some(BardSwapCommand::EquipInstrument(InstrumentType::String))
        );
    }

    #[test]
    fn brass_song_equips_brass_instrument() {
        let mut mgr = BardSwapManager::new();
        mgr.set_config(1, default_enabled_config());
        assert_eq!(
            mgr.before_cast(1, "Anthem de Arms"),
            Some(BardSwapCommand::EquipInstrument(InstrumentType::Brass))
        );
    }

    #[test]
    fn wind_song_equips_wind_instrument() {
        let mut mgr = BardSwapManager::new();
        mgr.set_config(1, default_enabled_config());
        assert_eq!(
            mgr.before_cast(1, "Cassindra's Chorus of Clarity"),
            Some(BardSwapCommand::EquipInstrument(InstrumentType::Wind))
        );
    }

    #[test]
    fn percussion_song_equips_percussion() {
        let mut mgr = BardSwapManager::new();
        mgr.set_config(1, default_enabled_config());
        assert_eq!(
            mgr.before_cast(1, "Angstlich's Assonance"),
            Some(BardSwapCommand::EquipInstrument(InstrumentType::Percussion))
        );
    }

    #[test]
    fn unknown_song_no_swap() {
        let mut mgr = BardSwapManager::new();
        mgr.set_config(1, default_enabled_config());
        assert_eq!(mgr.before_cast(1, "Some Unknown Song"), None);
    }

    #[test]
    fn after_cast_restores_weapon() {
        let mut mgr = BardSwapManager::new();
        mgr.set_config(1, default_enabled_config());
        mgr.before_cast(1, "Lyssa's Solidarity of Vision");
        assert_eq!(mgr.after_cast(1), Some(BardSwapCommand::RestoreWeapon));
    }

    #[test]
    fn after_cast_without_swap_no_restore() {
        let mut mgr = BardSwapManager::new();
        mgr.set_config(1, default_enabled_config());
        assert_eq!(mgr.after_cast(1), None);
    }

    #[test]
    fn same_instrument_type_no_re_swap() {
        let mut mgr = BardSwapManager::new();
        mgr.set_config(1, default_enabled_config());
        mgr.before_cast(1, "Lyssa's Solidarity of Vision");
        // Second string song while already holding string instrument — no swap.
        assert_eq!(mgr.before_cast(1, "Largo's Melodic Binding"), None);
    }

    #[test]
    fn instrument_for_song_lookup_only() {
        let mgr = BardSwapManager::new();
        // No config set — returns None variant.
        assert_eq!(
            mgr.instrument_for_song(99, "Anthem de Arms"),
            InstrumentType::None
        );
    }
}
