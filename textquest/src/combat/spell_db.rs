/// Basic spell info for orchestrator planning decisions.
pub struct SpellInfo {
    /// Unique spell identifier.
    pub spell_id: u32,
    /// Display name of the spell.
    pub name: &'static str,
    /// Mana cost to cast.
    pub mana_cost: u32,
    /// Cast time in milliseconds.
    pub cast_time_ms: u32,
    /// Maximum casting range in game units.
    pub range: f32,
    /// Whether this spell is area-of-effect.
    pub is_aoe: bool,
}

/// Lookup spell info by ID. Returns None for unknown spells.
#[must_use]
pub fn get(spell_id: u32) -> Option<&'static SpellInfo> {
    SPELLS.iter().find(|s| s.spell_id == spell_id)
}

static SPELLS: &[SpellInfo] = &[
    // Cleric heals
    SpellInfo {
        spell_id: 201,
        name: "Complete Heal",
        mana_cost: 400,
        cast_time_ms: 10000,
        range: 200.0,
        is_aoe: false,
    },
    SpellInfo {
        spell_id: 202,
        name: "Greater Heal",
        mana_cost: 200,
        cast_time_ms: 4000,
        range: 200.0,
        is_aoe: false,
    },
    SpellInfo {
        spell_id: 203,
        name: "Light Heal",
        mana_cost: 50,
        cast_time_ms: 2500,
        range: 200.0,
        is_aoe: false,
    },
    // Enchanter
    SpellInfo {
        spell_id: 301,
        name: "Mesmerize",
        mana_cost: 100,
        cast_time_ms: 3000,
        range: 200.0,
        is_aoe: false,
    },
    SpellInfo {
        spell_id: 302,
        name: "Color Flux",
        mana_cost: 150,
        cast_time_ms: 2500,
        range: 200.0,
        is_aoe: true,
    },
    // Shaman
    SpellInfo {
        spell_id: 401,
        name: "Slow",
        mana_cost: 175,
        cast_time_ms: 4500,
        range: 200.0,
        is_aoe: false,
    },
    SpellInfo {
        spell_id: 402,
        name: "Haste",
        mana_cost: 200,
        cast_time_ms: 5000,
        range: 200.0,
        is_aoe: false,
    },
    // Common nukes
    SpellInfo {
        spell_id: 501,
        name: "Ice Comet",
        mana_cost: 350,
        cast_time_ms: 5500,
        range: 200.0,
        is_aoe: false,
    },
    SpellInfo {
        spell_id: 502,
        name: "Fire",
        mana_cost: 100,
        cast_time_ms: 3000,
        range: 200.0,
        is_aoe: false,
    },
    // Druid
    SpellInfo {
        spell_id: 601,
        name: "Spirit of Wolf",
        mana_cost: 40,
        cast_time_ms: 3000,
        range: 200.0,
        is_aoe: false,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_known_spell_returns_some() {
        let spell = get(201);
        assert!(spell.is_some());
        let spell = spell.unwrap();
        assert_eq!(spell.name, "Complete Heal");
        assert_eq!(spell.mana_cost, 400);
        assert_eq!(spell.cast_time_ms, 10000);
        assert!(!spell.is_aoe);
    }

    #[test]
    fn get_unknown_spell_returns_none() {
        assert!(get(9999).is_none());
        assert!(get(0).is_none());
    }

    #[test]
    fn mesmerize_is_not_aoe() {
        let spell = get(301).unwrap();
        assert_eq!(spell.name, "Mesmerize");
        assert!(!spell.is_aoe);
    }

    #[test]
    fn color_flux_is_aoe() {
        let spell = get(302).unwrap();
        assert_eq!(spell.name, "Color Flux");
        assert!(spell.is_aoe);
    }

    #[test]
    fn all_spells_have_positive_range() {
        for spell in SPELLS {
            assert!(
                spell.range > 0.0,
                "Spell {} should have positive range",
                spell.name
            );
        }
    }

    #[test]
    fn get_all_known_spells() {
        let known_ids = [201, 202, 203, 301, 302, 401, 402, 501, 502, 601];
        for id in known_ids {
            assert!(get(id).is_some(), "spell_id {id} should be found");
        }
    }

    #[test]
    fn all_spells_have_non_empty_names() {
        for spell in SPELLS {
            assert!(
                !spell.name.is_empty(),
                "spell_id {} has empty name",
                spell.spell_id
            );
        }
    }

    #[test]
    fn all_spells_have_positive_mana_cost() {
        for spell in SPELLS {
            assert!(
                spell.mana_cost > 0,
                "Spell {} should have positive mana cost",
                spell.name
            );
        }
    }

    #[test]
    fn all_spells_have_positive_cast_time() {
        for spell in SPELLS {
            assert!(
                spell.cast_time_ms > 0,
                "Spell {} should have positive cast time",
                spell.name
            );
        }
    }

    #[test]
    fn spirit_of_wolf_is_druid_buff() {
        let spell = get(601).unwrap();
        assert_eq!(spell.name, "Spirit of Wolf");
        assert!(!spell.is_aoe);
        assert_eq!(spell.mana_cost, 40); // cheap buff
    }

    #[test]
    fn all_spell_ids_are_unique() {
        for (i, a) in SPELLS.iter().enumerate() {
            for b in SPELLS.iter().skip(i + 1) {
                assert_ne!(
                    a.spell_id, b.spell_id,
                    "Duplicate spell_id {} for {} and {}",
                    a.spell_id, a.name, b.name
                );
            }
        }
    }
}
