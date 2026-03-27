/// Basic spell info for orchestrator planning decisions.
pub struct SpellInfo {
    pub spell_id: u32,
    pub name: &'static str,
    pub mana_cost: u32,
    pub cast_time_ms: u32,
    pub range: f32,
    pub is_aoe: bool,
}

/// Lookup spell info by ID. Returns None for unknown spells.
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
