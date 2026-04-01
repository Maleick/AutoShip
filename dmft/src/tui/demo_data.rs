//! Demo spawn data for TUI demo mode.
//!
//! Extracted from `run.rs` to reduce inline data duplication. Each zone gets
//! a curated spawn list mixing PCs, NPCs, and corpses appropriate to the zone.

use crate::eq::named_tracker::is_named;
use crate::eq::structs::{EqClass, SpawnInfo, SpawnType, StandState};

/// Spawn definition tuple: (name, level, `class_id`, `spawn_type`, hp, `hp_max`, `stand_state`).
type SpawnDef<'a> = (&'a str, u8, u8, SpawnType, i64, i64, StandState);

#[derive(Clone, Copy)]
struct DemoAnchor {
    x: f32,
    y: f32,
    z: f32,
    heading: f32,
}

const PLAYER_OFFSETS: &[(f32, f32)] = &[
    (-10.0, 8.0),
    (6.0, -12.0),
    (18.0, 10.0),
    (-22.0, -16.0),
    (28.0, 14.0),
    (-30.0, 22.0),
];

const SPAWN_OFFSETS: &[(f32, f32, f32)] = &[
    (-12.0, 8.0, 0.0),
    (10.0, -10.0, 0.0),
    (24.0, 6.0, 0.0),
    (-26.0, -18.0, 2.0),
    (34.0, 18.0, 4.0),
    (-38.0, 24.0, -2.0),
    (44.0, -20.0, 3.0),
    (12.0, 34.0, 0.0),
    (-16.0, -36.0, -1.0),
];

fn zone_anchor(zone: &str) -> Option<DemoAnchor> {
    match zone {
        "Permafrost" => Some(DemoAnchor {
            x: -340.0,
            y: -260.0,
            z: -38.0,
            heading: 128.0,
        }),
        // Near Ry'Gorr Keep area — matches EQ Atlas Eastern Wastes coordinates
        "Eastern Wastes" => Some(DemoAnchor {
            x: -4_500.0,
            y: -3_780.0,
            z: 400.0,
            heading: 96.0,
        }),
        // Near Wizard Spires — matches EQ Atlas Great Divide coordinates
        "Great Divide" => Some(DemoAnchor {
            x: -1_910.0,
            y: -2_710.0,
            z: -300.0,
            heading: 144.0,
        }),
        _ => None,
    }
}

/// Generate a demo player position for a given zone and group slot index.
#[must_use]
pub fn demo_player_position(zone: &str, slot: usize) -> Option<(f32, f32, f32, f32)> {
    let anchor = zone_anchor(zone)?;
    let (dx, dy) = PLAYER_OFFSETS[slot % PLAYER_OFFSETS.len()];
    let ring = (slot / PLAYER_OFFSETS.len()) as f32;
    Some((
        anchor.x + dx + ring * 14.0,
        anchor.y + dy - ring * 12.0,
        anchor.z + (slot % 2) as f32,
        anchor.heading,
    ))
}

fn demo_spawn_position(
    zone: &str,
    index: usize,
    spawn_type: SpawnType,
    name: &str,
) -> (f32, f32, f32, f32) {
    let anchor = zone_anchor(zone).unwrap_or(DemoAnchor {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        heading: 0.0,
    });
    let (dx, dy, dz) = SPAWN_OFFSETS[index % SPAWN_OFFSETS.len()];
    let ring = (index / SPAWN_OFFSETS.len()) as f32;
    let scale = match spawn_type {
        SpawnType::Player => 0.55,
        SpawnType::Corpse => 1.15,
        SpawnType::Npc if is_named(name) => 1.6,
        _ => 1.0,
    };

    (
        anchor.x + dx * scale + ring * 26.0,
        anchor.y + dy * scale - ring * 18.0,
        anchor.z + dz,
        anchor.heading,
    )
}

/// Build a `Vec<SpawnInfo>` from a compact tuple list.
fn make_demo_spawns(zone: &str, data: &[SpawnDef<'_>]) -> Vec<SpawnInfo> {
    data.iter()
        .enumerate()
        .map(|(i, (name, level, class, stype, hp, hp_max, stand))| {
            let (x, y, z, heading) = demo_spawn_position(zone, i, *stype, name);
            SpawnInfo {
                name: name.to_string(),
                displayed_name: name.to_string(),
                lastname: String::new(),
                level: *level,
                class_id: *class,
                class: EqClass::from_id(*class),
                stand_state: *stand,
                spawn_type: *stype,
                hp_current: *hp,
                hp_max: *hp_max,
                mana_current: if *class > 0 { 3000 } else { 0 },
                mana_max: if *class > 0 { 4000 } else { 0 },
                endurance_current: 150,
                endurance_max: 200,
                x,
                y,
                z,
                heading,
                spawn_id: 100 + i as u32,
                is_gm: false,
                race_id: 1,
                buff_slots: Vec::new(),
                cast_state: None,
            }
        })
        .collect()
}

/// Return demo spawns for the given zone display name.
///
/// Recognized zones: "Permafrost", "Eastern Wastes", "Great Divide".
/// Returns an empty vec for unknown zones.
#[must_use]
pub fn demo_spawns_for_zone(zone: &str) -> Vec<SpawnInfo> {
    match zone {
        "Permafrost" => make_demo_spawns(
            zone,
            &[
                (
                    "Dmft01",
                    60,
                    1,
                    SpawnType::Player,
                    9500,
                    10000,
                    StandState::Standing,
                ),
                (
                    "Iceweaver02",
                    60,
                    2,
                    SpawnType::Player,
                    5300,
                    6000,
                    StandState::Standing,
                ),
                (
                    "Coldchain03",
                    60,
                    14,
                    SpawnType::Player,
                    3200,
                    4000,
                    StandState::Standing,
                ),
                (
                    "Lady Vox",
                    60,
                    0,
                    SpawnType::Npc,
                    250_000,
                    320_000,
                    StandState::Standing,
                ),
                (
                    "a frost giant",
                    55,
                    0,
                    SpawnType::Npc,
                    12000,
                    15000,
                    StandState::Standing,
                ),
                (
                    "a snow griffin",
                    52,
                    0,
                    SpawnType::Npc,
                    8000,
                    8000,
                    StandState::Standing,
                ),
                (
                    "an ice bone skeleton",
                    48,
                    0,
                    SpawnType::Npc,
                    4200,
                    5000,
                    StandState::Standing,
                ),
                (
                    "Trader Mikhail",
                    45,
                    0,
                    SpawnType::Npc,
                    5000,
                    5000,
                    StandState::Standing,
                ),
                (
                    "a frost giant's corpse",
                    55,
                    0,
                    SpawnType::Corpse,
                    0,
                    15000,
                    StandState::Dead,
                ),
            ],
        ),
        "Eastern Wastes" => make_demo_spawns(
            zone,
            &[
                (
                    "Shadowveil07",
                    58,
                    5,
                    SpawnType::Player,
                    7000,
                    10000,
                    StandState::Standing,
                ),
                (
                    "Spiritcaller08",
                    58,
                    10,
                    SpawnType::Player,
                    4400,
                    5200,
                    StandState::Standing,
                ),
                (
                    "Nightblade10",
                    59,
                    9,
                    SpawnType::Player,
                    5800,
                    9000,
                    StandState::Ducking,
                ),
                (
                    "a Kael warrior",
                    56,
                    0,
                    SpawnType::Npc,
                    14000,
                    18000,
                    StandState::Standing,
                ),
                (
                    "a tundra kodiak",
                    50,
                    0,
                    SpawnType::Npc,
                    6000,
                    7000,
                    StandState::Standing,
                ),
                (
                    "a dire wolf",
                    48,
                    0,
                    SpawnType::Npc,
                    5000,
                    6200,
                    StandState::Standing,
                ),
                (
                    "Wuoshi",
                    60,
                    0,
                    SpawnType::Npc,
                    200_000,
                    280_000,
                    StandState::Standing,
                ),
                (
                    "a walrus's corpse",
                    45,
                    0,
                    SpawnType::Corpse,
                    0,
                    4000,
                    StandState::Dead,
                ),
            ],
        ),
        "Great Divide" => make_demo_spawns(
            zone,
            &[
                (
                    "Holyblade13",
                    60,
                    3,
                    SpawnType::Player,
                    7600,
                    9000,
                    StandState::Standing,
                ),
                (
                    "Swiftfist14",
                    60,
                    7,
                    SpawnType::Player,
                    6500,
                    9000,
                    StandState::Standing,
                ),
                (
                    "Ragecleave16",
                    58,
                    16,
                    SpawnType::Player,
                    6200,
                    9000,
                    StandState::Standing,
                ),
                (
                    "a Coldain warrior",
                    52,
                    0,
                    SpawnType::Npc,
                    8000,
                    9500,
                    StandState::Standing,
                ),
                (
                    "a frost giant scout",
                    54,
                    0,
                    SpawnType::Npc,
                    11000,
                    14000,
                    StandState::Standing,
                ),
                (
                    "a velium hound",
                    46,
                    0,
                    SpawnType::Npc,
                    4500,
                    5000,
                    StandState::Standing,
                ),
                (
                    "Garudon",
                    60,
                    0,
                    SpawnType::Npc,
                    180_000,
                    220_000,
                    StandState::Standing,
                ),
                (
                    "a Coldain's corpse",
                    50,
                    0,
                    SpawnType::Corpse,
                    0,
                    8000,
                    StandState::Dead,
                ),
            ],
        ),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eq::map_parser;
    use std::path::Path;

    fn map_name_for_zone(zone: &str) -> &'static str {
        match zone {
            "Permafrost" => "permafrost",
            "Eastern Wastes" => "eastwastes",
            "Great Divide" => "greatdivide",
            _ => "unknown",
        }
    }

    fn in_demo_map_bounds(map_name: &str, spawn: &crate::eq::structs::SpawnInfo) -> bool {
        let map_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../config/maps");
        let Ok(map) = map_parser::load_zone_map(&map_dir, map_name) else {
            return false;
        };
        let margin_x = map.bounds.width().max(120.0);
        let margin_y = map.bounds.height().max(120.0);
        spawn.x >= map.bounds.min_x - margin_x * 0.10
            && spawn.x <= map.bounds.max_x + margin_x * 0.10
            && spawn.y >= map.bounds.min_y - margin_y * 0.10
            && spawn.y <= map.bounds.max_y + margin_y * 0.10
    }

    #[test]
    fn player_positions_match_zone_space() {
        let (x, y, z, _) = demo_player_position("Permafrost", 0).unwrap();
        assert!(x < 0.0);
        assert!(y < 0.0);
        assert!(z < 0.0);

        // Eastern Wastes anchor is near Great Span: (-4500, -3780, 400)
        let (x, y, z, _) = demo_player_position("Eastern Wastes", 0).unwrap();
        assert!(x < -4_000.0, "EW x should be near -4500, got {x}");
        assert!(y < -3_000.0, "EW y should be near -3780, got {y}");
        assert!(z > 100.0, "EW z should be ~400, got {z}");
    }

    #[test]
    fn demo_spawns_use_zone_anchor() {
        // Great Divide anchor is near Wizard Spires: (-1910, -2710, -300)
        let spawns = demo_spawns_for_zone("Great Divide");
        assert!(!spawns.is_empty());
        assert!(spawns.iter().all(|spawn| spawn.x < -1_800.0));
        assert!(spawns.iter().all(|spawn| spawn.y < -2_600.0));
    }

    #[test]
    fn demo_spawns_fall_within_render_bounds() {
        for zone in ["Permafrost", "Eastern Wastes", "Great Divide"] {
            let map_name = map_name_for_zone(zone);
            let spawns = demo_spawns_for_zone(zone);

            for spawn in spawns {
                assert!(
                    in_demo_map_bounds(map_name, &spawn),
                    "spawn {zone}::{name} ({x}, {y}) is out of bounds",
                    name = spawn.name,
                    x = spawn.x,
                    y = spawn.y,
                );
            }
        }
    }
}
