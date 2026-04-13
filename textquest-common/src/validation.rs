//! Compile-time-like struct range validations for known EQ layouts.

use std::mem::size_of;

use crate::offsets;

#[cfg(any(test, debug_assertions))]
pub fn validate_struct_sizes() -> Vec<String> {
    let mut violations = Vec::new();

    check_fields(
        "PlayerClient::PLAYER_CLIENT_SIZE",
        offsets::PLAYER_CLIENT_SIZE,
        [("SPAWN_INFO_MAX_OFFSET", offsets::SPAWN_INFO_MAX_OFFSET, 4)],
        &mut violations,
    );

    check_fields(
        "PlayerClient::player_base",
        offsets::PLAYER_CLIENT_SIZE,
        [
            ("NEXT", offsets::player_base::NEXT, size_of::<usize>()),
            ("PREV", offsets::player_base::PREV, size_of::<usize>()),
            ("X", offsets::player_base::X, size_of::<f32>()),
            ("Y", offsets::player_base::Y, size_of::<f32>()),
            ("Z", offsets::player_base::Z, size_of::<f32>()),
            ("HEADING", offsets::player_base::HEADING, size_of::<f32>()),
            ("NAME", offsets::player_base::NAME, 64),
            ("DISPLAYED_NAME", offsets::player_base::DISPLAYED_NAME, 64),
            ("LASTNAME", offsets::player_base::LASTNAME, 32),
            ("SPAWN_ID", offsets::player_base::SPAWN_ID, size_of::<u32>()),
            ("TYPE", offsets::player_base::TYPE, size_of::<u8>()),
        ],
        &mut violations,
    );

    check_fields(
        "PlayerClient::player_zone",
        offsets::PLAYER_CLIENT_SIZE,
        [
            (
                "CASTING_DATA",
                offsets::player_zone::CASTING_DATA,
                size_of::<u32>(),
            ),
            ("HP_MAX", offsets::player_zone::HP_MAX, size_of::<u64>()),
            (
                "HP_CURRENT",
                offsets::player_zone::HP_CURRENT,
                size_of::<u64>(),
            ),
            (
                "MANA_CURRENT",
                offsets::player_zone::MANA_CURRENT,
                size_of::<u32>(),
            ),
            ("MANA_MAX", offsets::player_zone::MANA_MAX, size_of::<u32>()),
            ("LEVEL", offsets::player_zone::LEVEL, size_of::<u8>()),
            (
                "MELEE_RADIUS",
                offsets::player_zone::MELEE_RADIUS,
                size_of::<f32>(),
            ),
            (
                "SPELL_GEM_ETA",
                offsets::player_zone::SPELL_GEM_ETA,
                size_of::<u32>() * 15,
            ),
            (
                "CHAR_CLASS",
                offsets::player_zone::CHAR_CLASS,
                size_of::<u8>(),
            ),
            (
                "ENDURANCE_CURRENT",
                offsets::player_zone::ENDURANCE_CURRENT,
                size_of::<i32>(),
            ),
            (
                "ENDURANCE_MAX",
                offsets::player_zone::ENDURANCE_MAX,
                size_of::<u32>(),
            ),
        ],
        &mut violations,
    );

    check_fields(
        "EQ_Spell",
        offsets::EQ_SPELL_SIZE,
        [
            ("CAST_TIME", offsets::eq_spell::CAST_TIME, size_of::<u32>()),
            ("ID", offsets::eq_spell::ID, size_of::<u32>()),
            ("NAME", offsets::eq_spell::NAME, 64),
        ],
        &mut violations,
    );

    check_fields(
        "ZoneGuideZone",
        offsets::SPAWN_MANAGER_ZONE_ZONE_SIZE,
        [
            (
                "ZONE_CONNECTIONS_ARRAY",
                offsets::zone_guide::ZONE_CONNECTIONS_ARRAY,
                size_of::<usize>(),
            ),
            (
                "ZONE_CONNECTIONS_COUNT",
                offsets::zone_guide::ZONE_CONNECTIONS_COUNT,
                size_of::<u32>(),
            ),
        ],
        &mut violations,
    );

    violations
}

fn check_fields(
    struct_name: &str,
    struct_size: usize,
    fields: impl IntoIterator<Item = (&'static str, usize, usize)>,
    violations: &mut Vec<String>,
) {
    for (field_name, offset, size) in fields {
        if offset + size > struct_size {
            violations.push(format!(
                "{struct_name}::{field_name} at 0x{offset:04x} exceeds size 0x{struct_size:04x}",
            ));
        }
    }
}

#[cfg(test)]
#[test]
fn validate_struct_sizes_smoke_test() {
    let violations = validate_struct_sizes();
    assert!(
        violations.is_empty(),
        "struct validation violations:\n{}",
        violations.join("\n")
    );
}
