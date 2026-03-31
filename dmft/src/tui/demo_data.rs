//! Demo spawn data for TUI demo mode.
//!
//! Extracted from `run.rs` to reduce inline data duplication. Each zone gets
//! a curated spawn list mixing PCs, NPCs, and corpses appropriate to the zone.

use crate::eq::structs::{EqClass, SpawnInfo, SpawnType, StandState};

/// Spawn definition tuple: (name, level, class_id, spawn_type, hp, hp_max, stand_state).
type SpawnDef<'a> = (&'a str, u8, u8, SpawnType, i64, i64, StandState);

/// Build a `Vec<SpawnInfo>` from a compact tuple list.
fn make_demo_spawns(data: &[SpawnDef<'_>]) -> Vec<SpawnInfo> {
    data.iter()
        .enumerate()
        .map(
            |(i, (name, level, class, stype, hp, hp_max, stand))| SpawnInfo {
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
                x: 1234.5 + (i as f32 * 10.0),
                y: -567.8 + (i as f32 * 5.0),
                z: 12.0,
                heading: 0.0,
                spawn_id: 100 + i as u32,
                is_gm: false,
                race_id: 1,
                buff_slots: Vec::new(),
                cast_state: None,
            },
        )
        .collect()
}

/// Return demo spawns for the given zone display name.
///
/// Recognized zones: "Permafrost", "Eastern Wastes", "Great Divide".
/// Returns an empty vec for unknown zones.
pub fn demo_spawns_for_zone(zone: &str) -> Vec<SpawnInfo> {
    match zone {
        "Permafrost" => make_demo_spawns(&[
            (
                "Frostreaver01",
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
                250000,
                320000,
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
        ]),
        "Eastern Wastes" => make_demo_spawns(&[
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
                200000,
                280000,
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
        ]),
        "Great Divide" => make_demo_spawns(&[
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
                180000,
                220000,
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
        ]),
        _ => Vec::new(),
    }
}
