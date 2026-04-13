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
mod tests {
    use super::*;

    #[test]
    fn validate_struct_sizes_smoke_test() {
        let violations = validate_struct_sizes();
        assert!(
            violations.is_empty(),
            "struct validation violations:\n{}",
            violations.join("\n")
        );
    }

    #[test]
    fn check_fields_within_bounds_produces_no_violations() {
        let mut violations = Vec::new();
        check_fields(
            "TestStruct",
            100,
            [("field_a", 0, 4), ("field_b", 96, 4)],
            &mut violations,
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn check_fields_exceeding_size_produces_violation() {
        let mut violations = Vec::new();
        check_fields(
            "TestStruct",
            10,
            [("overflow_field", 8, 4)],
            &mut violations,
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("overflow_field"));
        assert!(violations[0].contains("TestStruct"));
    }

    #[test]
    fn check_fields_at_exact_boundary_is_ok() {
        let mut violations = Vec::new();
        // offset=6, size=4, struct_size=10 → 6+4=10 == struct_size, OK
        check_fields("Exact", 10, [("boundary", 6, 4)], &mut violations);
        assert!(violations.is_empty());
    }

    #[test]
    fn check_fields_one_byte_over_is_violation() {
        let mut violations = Vec::new();
        // offset=7, size=4, struct_size=10 → 7+4=11 > 10
        check_fields("Over", 10, [("one_over", 7, 4)], &mut violations);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn check_fields_multiple_violations_reported() {
        let mut violations = Vec::new();
        check_fields(
            "Multi",
            4,
            [("a", 3, 4), ("b", 0, 2), ("c", 5, 1)],
            &mut violations,
        );
        // a: 3+4=7>4, b: 0+2=2<=4 ok, c: 5+1=6>4
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn check_fields_zero_size_struct_always_violates() {
        let mut violations = Vec::new();
        check_fields("Zero", 0, [("any", 0, 1)], &mut violations);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn check_fields_zero_size_field_always_ok() {
        let mut violations = Vec::new();
        check_fields(
            "TestStruct",
            10,
            [("zero_field", 10, 0)], // 10 + 0 = 10 <= 10 → OK
            &mut violations,
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn check_fields_at_offset_zero() {
        let mut violations = Vec::new();
        check_fields(
            "TestStruct",
            8,
            [("first_field", 0, 8)], // 0 + 8 = 8 <= 8 → OK
            &mut violations,
        );
        assert!(violations.is_empty());
    }

    #[test]
    fn check_fields_empty_fields_no_violations() {
        let mut violations = Vec::new();
        check_fields("TestStruct", 100, std::iter::empty(), &mut violations);
        assert!(violations.is_empty());
    }

    #[test]
    fn check_fields_violation_message_format() {
        let mut violations = Vec::new();
        check_fields(
            "PlayerClient",
            0x100,
            [("NAME", 0x00fe, 4)], // 0xfe + 4 = 0x102 > 0x100
            &mut violations,
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("PlayerClient::NAME"));
        assert!(violations[0].contains("0x00fe"));
        assert!(violations[0].contains("0x0100"));
    }
}
