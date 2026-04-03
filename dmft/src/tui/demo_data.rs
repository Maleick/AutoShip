//! Demo spawn data for TUI demo mode.
//!
//! Extracted from `run.rs` to reduce inline data duplication. Each zone gets
//! a curated spawn list mixing PCs, NPCs, and corpses appropriate to the zone.

use crate::eq::named_tracker::is_named;
use crate::eq::structs::{
    CastDurationSource, CastState as EqCastState, EqClass, SpawnInfo, SpawnType, StandState,
};
use dmft_common::nav::{NavStatus, Waypoint};
use dmft_common::offsets::launch_spell_data;

/// Spawn definition tuple: (name, level, `class_id`, `spawn_type`, hp, `hp_max`, `stand_state`).
type SpawnDef<'a> = (&'a str, u8, u8, SpawnType, i64, i64, StandState);

#[derive(Clone, Copy)]
struct DemoAnchor {
    x: f32,
    y: f32,
    z: f32,
    heading: f32,
}

/// High-level action states used by the deterministic demo scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoActionState {
    Idle,
    Casting,
    Fighting,
    Looting,
    Sitting,
    Ducking,
    Feigned,
    Moving,
    Arrived,
    Stuck,
    Buffing,
    Medding,
}

impl DemoActionState {
    /// Human-readable label suitable for status bars and demo tooltips.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Casting => "casting",
            Self::Fighting => "fighting",
            Self::Looting => "looting",
            Self::Sitting => "sitting",
            Self::Ducking => "ducking",
            Self::Feigned => "feigned",
            Self::Moving => "moving",
            Self::Arrived => "arrived",
            Self::Stuck => "stuck",
            Self::Buffing => "buffing",
            Self::Medding => "medding",
        }
    }
}

/// A deterministic demo cast snapshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DemoCastInfo {
    /// Spell gem slot used for the cast (0-based, matches EQ `CastState`).
    pub spell_slot: u8,
    /// Exact spell label for demo rendering.
    pub spell_label: &'static str,
    /// Full cast duration in milliseconds.
    pub total_cast_ms: u32,
    /// Elapsed cast time in milliseconds.
    pub elapsed_ms: u32,
    /// Remaining cast time in milliseconds.
    pub remaining_ms: u32,
    /// Normalized progress ratio in the range 0.0..=1.0.
    pub progress: f32,
}

impl DemoCastInfo {
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.remaining_ms > 0
    }
}

/// Deterministic navigation state for the demo scheduler.
#[derive(Debug, Clone, PartialEq)]
pub struct DemoNavInfo {
    /// Current navigation status.
    pub status: NavStatus,
    /// Human-readable destination label.
    pub destination: String,
    /// Waypoints used for the mini-map overlay.
    pub waypoints: Vec<Waypoint>,
}

/// A full demo snapshot for one client at a given tick.
#[derive(Debug, Clone, PartialEq)]
pub struct DemoClientSnapshot {
    /// Current action state.
    pub action_state: DemoActionState,
    /// Human-readable action label.
    pub action_label: String,
    /// Stand state to apply to the local player.
    pub stand_state: StandState,
    /// Optional deterministic cast snapshot.
    pub cast: Option<DemoCastInfo>,
    /// Optional deterministic nav snapshot.
    pub nav: Option<DemoNavInfo>,
    /// Preferred target spawn name for fight/assist states.
    pub target_spawn_name: Option<&'static str>,
    /// Optional explicit target label.
    pub target_label: Option<String>,
    /// Current HP value to apply.
    pub hp_current: i64,
    /// Current mana value to apply.
    pub mana_current: i32,
    /// Position in world coordinates: (x, y, z, heading).
    pub position: (f32, f32, f32, f32),
    /// Status line suitable for the demo status bar.
    pub status_line: String,
}

#[derive(Debug, Clone, PartialEq)]
struct DemoActionFrame {
    action_state: DemoActionState,
    cast: Option<DemoCastInfo>,
    nav: Option<DemoNavInfo>,
    action_label: String,
}

impl DemoActionFrame {
    fn new(
        action_state: DemoActionState,
        cast: Option<DemoCastInfo>,
        nav: Option<DemoNavInfo>,
        action_label: String,
    ) -> Self {
        Self {
            action_state,
            cast,
            nav,
            action_label,
        }
    }
}

/// Static profile information for a demo client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemoClientProfile {
    /// Demo roster index.
    pub index: usize,
    /// Demo process PID assigned in `load_demo_data`.
    pub pid: u32,
    /// Demo character name.
    pub name: &'static str,
    /// Demo zone name.
    pub zone: &'static str,
    /// Role used to drive the action script.
    pub role: DemoRole,
    /// Primary target label for the role, if any.
    pub target_spawn_name: Option<&'static str>,
    /// Navigation destination label, if any.
    pub nav_destination: Option<&'static str>,
}

/// Named role buckets used by the demo script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoRole {
    MainTank,
    ChainCleric,
    Enchanter,
    Bard,
    Ranger,
    Wizard,
    ShadowKnight,
    Shaman,
    Druid,
    Rogue,
    Necromancer,
    Magician,
    Paladin,
    Monk,
    Beastlord,
    Berserker,
    ChainClericTwo,
    RecoveryWizard,
}

const DEMO_CLIENT_PROFILES: &[DemoClientProfile] = &[
    DemoClientProfile {
        index: 0,
        pid: 1000,
        name: "Dmft01",
        zone: "Permafrost",
        role: DemoRole::MainTank,
        target_spawn_name: Some("a frost giant"),
        nav_destination: Some("Main camp ridge"),
    },
    DemoClientProfile {
        index: 1,
        pid: 1001,
        name: "Iceweaver02",
        zone: "Permafrost",
        role: DemoRole::ChainCleric,
        target_spawn_name: Some("Dmft01"),
        nav_destination: None,
    },
    DemoClientProfile {
        index: 2,
        pid: 1002,
        name: "Coldchain03",
        zone: "Permafrost",
        role: DemoRole::Enchanter,
        target_spawn_name: Some("a frost giant"),
        nav_destination: None,
    },
    DemoClientProfile {
        index: 3,
        pid: 1003,
        name: "Frostsong04",
        zone: "Permafrost",
        role: DemoRole::Bard,
        target_spawn_name: None,
        nav_destination: None,
    },
    DemoClientProfile {
        index: 4,
        pid: 1004,
        name: "Tundrablade05",
        zone: "Permafrost",
        role: DemoRole::Ranger,
        target_spawn_name: Some("a frost giant"),
        nav_destination: Some("Permafrost ridge"),
    },
    DemoClientProfile {
        index: 5,
        pid: 1005,
        name: "Glacierstrike06",
        zone: "Permafrost",
        role: DemoRole::Wizard,
        target_spawn_name: Some("a frost giant"),
        nav_destination: None,
    },
    DemoClientProfile {
        index: 6,
        pid: 1006,
        name: "Shadowveil07",
        zone: "Eastern Wastes",
        role: DemoRole::ShadowKnight,
        target_spawn_name: Some("a Kael warrior"),
        nav_destination: None,
    },
    DemoClientProfile {
        index: 7,
        pid: 1007,
        name: "Spiritcaller08",
        zone: "Eastern Wastes",
        role: DemoRole::Shaman,
        target_spawn_name: Some("a Kael warrior"),
        nav_destination: None,
    },
    DemoClientProfile {
        index: 8,
        pid: 1008,
        name: "Verdantleaf09",
        zone: "Eastern Wastes",
        role: DemoRole::Druid,
        target_spawn_name: Some("a tundra kodiak"),
        nav_destination: None,
    },
    DemoClientProfile {
        index: 9,
        pid: 1009,
        name: "Nightblade10",
        zone: "Eastern Wastes",
        role: DemoRole::Rogue,
        target_spawn_name: Some("a dire wolf"),
        nav_destination: None,
    },
    DemoClientProfile {
        index: 10,
        pid: 1010,
        name: "Soulreaper11",
        zone: "Eastern Wastes",
        role: DemoRole::Necromancer,
        target_spawn_name: Some("a dire wolf"),
        nav_destination: None,
    },
    DemoClientProfile {
        index: 11,
        pid: 1011,
        name: "Petmaster12",
        zone: "Eastern Wastes",
        role: DemoRole::Magician,
        target_spawn_name: Some("Wuoshi"),
        nav_destination: None,
    },
    DemoClientProfile {
        index: 12,
        pid: 1012,
        name: "Holyblade13",
        zone: "Great Divide",
        role: DemoRole::Paladin,
        target_spawn_name: Some("a frost giant scout"),
        nav_destination: None,
    },
    DemoClientProfile {
        index: 13,
        pid: 1013,
        name: "Swiftfist14",
        zone: "Great Divide",
        role: DemoRole::Monk,
        target_spawn_name: Some("a Coldain warrior"),
        nav_destination: Some("Stuck on the spires path"),
    },
    DemoClientProfile {
        index: 14,
        pid: 1014,
        name: "Beastkin15",
        zone: "Great Divide",
        role: DemoRole::Beastlord,
        target_spawn_name: Some("a frost giant scout"),
        nav_destination: Some("Great Divide camp lane"),
    },
    DemoClientProfile {
        index: 15,
        pid: 1015,
        name: "Ragecleave16",
        zone: "Great Divide",
        role: DemoRole::Berserker,
        target_spawn_name: Some("a Coldain warrior"),
        nav_destination: Some("Great Divide camp lane"),
    },
    DemoClientProfile {
        index: 16,
        pid: 1016,
        name: "Frostmend17",
        zone: "Great Divide",
        role: DemoRole::ChainClericTwo,
        target_spawn_name: Some("Holyblade13"),
        nav_destination: None,
    },
    DemoClientProfile {
        index: 17,
        pid: 1017,
        name: "Glacialsurge18",
        zone: "Great Divide",
        role: DemoRole::RecoveryWizard,
        target_spawn_name: Some("Garudon"),
        nav_destination: Some("Great Divide overlook"),
    },
];

fn demo_cycle_phase(tick_count: u64, phase_offset: u64, cycle_ticks: u64) -> u64 {
    if cycle_ticks == 0 {
        return 0;
    }
    (tick_count + phase_offset) % cycle_ticks
}

fn demo_index_from_name_or_pid(name: &str, pid: u32) -> Option<usize> {
    let pid_idx = pid
        .checked_sub(1000)
        .filter(|idx| *idx < 18)
        .map(|idx| idx as usize);
    if pid_idx.is_some() {
        return pid_idx;
    }

    let digits: String = name
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let parsed = digits.parse::<usize>().ok()?.checked_sub(1)?;
    (parsed < 18).then_some(parsed)
}

/// Look up the deterministic demo profile for a client by name or PID.
#[must_use]
pub fn demo_client_profile(name: &str, pid: u32) -> Option<&'static DemoClientProfile> {
    let index = demo_index_from_name_or_pid(name, pid)?;
    DEMO_CLIENT_PROFILES.get(index)
}

/// Determine the demo action state for a given client at a specific tick.
#[must_use]
pub fn demo_client_action_state(name: &str, pid: u32, tick_count: u64) -> Option<DemoActionState> {
    let profile = demo_client_profile(name, pid)?;
    Some(demo_action_for_profile(profile, tick_count).action_state)
}

/// Determine whether the demo client is actively casting at a specific tick.
#[must_use]
pub fn demo_client_cast_active(name: &str, pid: u32, tick_count: u64) -> Option<bool> {
    let profile = demo_client_profile(name, pid)?;
    Some(demo_action_for_profile(profile, tick_count).cast.is_some())
}

/// Convert an optional demo cast snapshot into an EQ-style cast state.
#[must_use]
pub fn demo_eq_cast_state(cast: Option<DemoCastInfo>) -> EqCastState {
    match cast {
        Some(cast) => EqCastState {
            spell_id: demo_spell_id(cast.spell_label),
            spell_name: Some(cast.spell_label.to_string()),
            target_id: 0,
            spell_slot: cast.spell_slot,
            spell_eta: cast.remaining_ms.max(1),
            item_id: 0,
            remaining_ms: Some(cast.remaining_ms.max(1)),
            total_cast_ms: Some(cast.total_cast_ms.max(1)),
            duration_source: CastDurationSource::ExactRuntime,
            gem_etas: Some([0; 15]),
        },
        None => EqCastState {
            spell_id: launch_spell_data::NOT_CASTING_SPELL_ID,
            spell_name: None,
            target_id: 0,
            spell_slot: launch_spell_data::NOT_CASTING_SPELL_SLOT,
            spell_eta: 0,
            item_id: 0,
            remaining_ms: None,
            total_cast_ms: None,
            duration_source: CastDurationSource::Unknown,
            gem_etas: Some([0; 15]),
        },
    }
}

#[must_use]
fn demo_spell_id(spell_label: &str) -> i32 {
    match spell_label {
        "Complete Heal" => 201,
        "Greater Heal" => 202,
        "Light Heal" => 203,
        "Mesmerize" => 301,
        "Color Flux" => 302,
        "Slow" => 401,
        "Haste" => 402,
        "Ice Comet" => 501,
        "Fire" => 502,
        "Spirit of Wolf" => 601,
        _ => 1,
    }
}

/// Determine the active demo cast, if any, for a given client and tick.
#[must_use]
pub fn demo_client_cast_info(
    name: &str,
    pid: u32,
    tick_count: u64,
    refresh_rate_ms: u64,
) -> Option<DemoCastInfo> {
    let profile = demo_client_profile(name, pid)?;
    demo_action_for_profile(profile, tick_count)
        .cast
        .map(|cast| {
            // Recompute the progress with the caller's cadence so tests can assert exact values.
            demo_cast_info(
                cast.spell_slot,
                cast.spell_label,
                cast.total_cast_ms,
                tick_count,
                profile.index as u64,
                refresh_rate_ms,
                false,
            )
        })
}

/// Determine the demo navigation scripting for a given client and tick.
#[must_use]
pub fn demo_client_nav_info(name: &str, pid: u32, tick_count: u64) -> Option<DemoNavInfo> {
    let profile = demo_client_profile(name, pid)?;
    demo_action_for_profile(profile, tick_count).nav
}

/// Full deterministic demo snapshot for a client.
#[must_use]
pub fn demo_client_snapshot(
    name: &str,
    pid: u32,
    tick_count: u64,
    _refresh_rate_ms: u64,
) -> Option<DemoClientSnapshot> {
    let profile = demo_client_profile(name, pid)?;
    let frame = demo_action_for_profile(profile, tick_count);
    let DemoActionFrame {
        action_state,
        cast,
        nav,
        action_label,
    } = frame;
    let (x, y, z, heading) =
        demo_player_position(profile.zone, profile.index).unwrap_or((0.0, 0.0, 0.0, 0.0));
    let (hp_current, mana_current) = demo_resource_values(profile.role, action_state, cast);
    let target_label = profile
        .target_spawn_name
        .map(std::string::ToString::to_string);
    let status_line = format!("Demo: {action_label}");

    Some(DemoClientSnapshot {
        action_state,
        action_label,
        stand_state: demo_stand_state_for_action(action_state),
        cast,
        nav,
        target_spawn_name: profile.target_spawn_name,
        target_label,
        hp_current,
        mana_current,
        position: (x, y, z, heading),
        status_line,
    })
}

fn demo_action_for_profile(profile: &DemoClientProfile, tick_count: u64) -> DemoActionFrame {
    let refresh_rate_ms: u64 = 250;
    let tick_ms = refresh_rate_ms.max(1);

    match profile.role {
        DemoRole::MainTank => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 24);
            let target = profile.target_spawn_name.unwrap_or("a frost giant");
            if phase < 6 {
                let nav = demo_nav_from_profile(
                    profile,
                    tick_count,
                    NavStatus::Moving {
                        waypoint_index: 0,
                        waypoint_count: 3,
                        distance_remaining: 120.0,
                    },
                );
                DemoActionFrame::new(
                    DemoActionState::Moving,
                    None,
                    Some(nav),
                    match phase {
                        0..=5 => format!("moving toward {target}"),
                        _ => unreachable!(),
                    },
                )
            } else if phase < 14 {
                DemoActionFrame::new(
                    DemoActionState::Fighting,
                    None,
                    None,
                    format!("fighting {target}"),
                )
            } else if phase < 18 {
                DemoActionFrame::new(
                    DemoActionState::Looting,
                    None,
                    None,
                    format!("looting {target}"),
                )
            } else if phase < 21 {
                DemoActionFrame::new(
                    DemoActionState::Sitting,
                    None,
                    None,
                    String::from("sitting to recover"),
                )
            } else {
                DemoActionFrame::new(
                    DemoActionState::Idle,
                    None,
                    None,
                    String::from("standing by the camp"),
                )
            }
        }
        DemoRole::ChainCleric | DemoRole::ChainClericTwo => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 48);
            let total_cast_ms: u32 = 10_000;
            let cast_ticks = total_cast_ms.div_ceil(tick_ms as u32).max(1) as u64;
            if phase < cast_ticks {
                let cast = demo_cast_info(
                    7,
                    "Complete Heal",
                    total_cast_ms,
                    tick_count,
                    profile.index as u64,
                    refresh_rate_ms,
                    false,
                );
                DemoActionFrame::new(
                    DemoActionState::Casting,
                    Some(cast),
                    None,
                    format!(
                        "casting {} on {}",
                        cast.spell_label,
                        profile.target_spawn_name.unwrap_or("the tank")
                    ),
                )
            } else if phase < cast_ticks + 4 {
                DemoActionFrame::new(
                    DemoActionState::Sitting,
                    None,
                    None,
                    String::from("sitting to med"),
                )
            } else {
                DemoActionFrame::new(
                    DemoActionState::Idle,
                    None,
                    None,
                    String::from("waiting for next heal"),
                )
            }
        }
        DemoRole::Enchanter => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 24);
            if phase < 5 {
                let spell = if ((tick_count / 24) & 1) == 0 {
                    "Mesmerize"
                } else {
                    "Color Flux"
                };
                let total = if spell == "Mesmerize" { 3_000 } else { 2_500 };
                let slot = if spell == "Mesmerize" { 2 } else { 1 };
                let cast = demo_cast_info(
                    slot,
                    spell,
                    total,
                    tick_count,
                    profile.index as u64,
                    refresh_rate_ms,
                    false,
                );
                DemoActionFrame::new(
                    DemoActionState::Casting,
                    Some(cast),
                    None,
                    format!(
                        "casting {spell} on {}",
                        profile.target_spawn_name.unwrap_or("the mob")
                    ),
                )
            } else if phase < 8 {
                DemoActionFrame::new(
                    DemoActionState::Ducking,
                    None,
                    None,
                    String::from("ducking between mez casts"),
                )
            } else if phase < 14 {
                DemoActionFrame::new(
                    DemoActionState::Fighting,
                    None,
                    None,
                    String::from("debuffing the pull"),
                )
            } else if phase < 18 {
                DemoActionFrame::new(
                    DemoActionState::Sitting,
                    None,
                    None,
                    String::from("medding"),
                )
            } else {
                DemoActionFrame::new(
                    DemoActionState::Buffing,
                    None,
                    None,
                    String::from("buffing the group"),
                )
            }
        }
        DemoRole::Bard => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 16);
            if phase < 5 {
                let cast = demo_cast_info(
                    3,
                    "Haste",
                    5_000,
                    tick_count,
                    profile.index as u64,
                    refresh_rate_ms,
                    false,
                );
                DemoActionFrame::new(
                    DemoActionState::Buffing,
                    Some(cast),
                    None,
                    String::from("singing a haste tune"),
                )
            } else if phase < 10 {
                DemoActionFrame::new(
                    DemoActionState::Fighting,
                    None,
                    None,
                    String::from("twisting songs in combat"),
                )
            } else {
                DemoActionFrame::new(
                    DemoActionState::Sitting,
                    None,
                    None,
                    String::from("sitting to recover"),
                )
            }
        }
        DemoRole::Ranger => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 20);
            if phase < 9 {
                let nav = demo_nav_from_profile(
                    profile,
                    tick_count,
                    NavStatus::Moving {
                        waypoint_index: 1,
                        waypoint_count: 3,
                        distance_remaining: 150.0,
                    },
                );
                DemoActionFrame::new(
                    DemoActionState::Moving,
                    None,
                    Some(nav),
                    String::from("moving toward the ridge"),
                )
            } else if phase < 13 {
                let nav = demo_nav_from_profile(profile, tick_count, NavStatus::Arrived);
                DemoActionFrame::new(
                    DemoActionState::Arrived,
                    None,
                    Some(nav),
                    String::from("arrived at the overlook"),
                )
            } else if phase < 16 {
                DemoActionFrame::new(
                    DemoActionState::Idle,
                    None,
                    None,
                    String::from("holding position"),
                )
            } else {
                DemoActionFrame::new(
                    DemoActionState::Moving,
                    None,
                    None,
                    String::from("repositioning"),
                )
            }
        }
        DemoRole::Wizard => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 20);
            if phase < 5 {
                DemoActionFrame::new(
                    DemoActionState::Medding,
                    None,
                    None,
                    String::from("medding"),
                )
            } else if phase < 11 {
                let cast = demo_cast_info(
                    7,
                    "Ice Comet",
                    5_500,
                    tick_count,
                    profile.index as u64,
                    refresh_rate_ms,
                    false,
                );
                DemoActionFrame::new(
                    DemoActionState::Casting,
                    Some(cast),
                    None,
                    format!(
                        "casting Ice Comet on {}",
                        profile.target_spawn_name.unwrap_or("the mob")
                    ),
                )
            } else if phase < 14 {
                DemoActionFrame::new(
                    DemoActionState::Fighting,
                    None,
                    None,
                    String::from("nuking"),
                )
            } else if phase < 16 {
                DemoActionFrame::new(
                    DemoActionState::Looting,
                    None,
                    None,
                    String::from("checking the corpse"),
                )
            } else {
                DemoActionFrame::new(DemoActionState::Idle, None, None, String::from("ready"))
            }
        }
        DemoRole::ShadowKnight => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 18);
            if phase < 7 {
                DemoActionFrame::new(
                    DemoActionState::Fighting,
                    None,
                    None,
                    format!(
                        "fighting {}",
                        profile.target_spawn_name.unwrap_or("the mob")
                    ),
                )
            } else if phase < 9 {
                DemoActionFrame::new(
                    DemoActionState::Ducking,
                    None,
                    None,
                    String::from("ducking"),
                )
            } else if phase < 12 {
                DemoActionFrame::new(
                    DemoActionState::Sitting,
                    None,
                    None,
                    String::from("sitting to recover"),
                )
            } else {
                DemoActionFrame::new(
                    DemoActionState::Idle,
                    None,
                    None,
                    String::from("holding aggro"),
                )
            }
        }
        DemoRole::Shaman => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 24);
            if phase < 6 {
                DemoActionFrame::new(
                    DemoActionState::Medding,
                    None,
                    None,
                    String::from("medding"),
                )
            } else if phase < 11 {
                let cast = demo_cast_info(
                    3,
                    "Slow",
                    4_500,
                    tick_count,
                    profile.index as u64,
                    refresh_rate_ms,
                    false,
                );
                DemoActionFrame::new(
                    DemoActionState::Casting,
                    Some(cast),
                    None,
                    format!(
                        "casting Slow on {}",
                        profile.target_spawn_name.unwrap_or("the mob")
                    ),
                )
            } else if phase < 16 {
                let cast = demo_cast_info(
                    4,
                    "Haste",
                    5_000,
                    tick_count,
                    profile.index as u64,
                    refresh_rate_ms,
                    false,
                );
                DemoActionFrame::new(
                    DemoActionState::Buffing,
                    Some(cast),
                    None,
                    String::from("buffing the group"),
                )
            } else {
                DemoActionFrame::new(
                    DemoActionState::Fighting,
                    None,
                    None,
                    String::from("slowing the pull"),
                )
            }
        }
        DemoRole::Druid => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 20);
            if phase < 5 {
                DemoActionFrame::new(
                    DemoActionState::Sitting,
                    None,
                    None,
                    String::from("medding"),
                )
            } else if phase < 9 {
                let spell = if ((tick_count / 20) & 1) == 0 {
                    "Spirit of Wolf"
                } else {
                    "Greater Heal"
                };
                let total = if spell == "Spirit of Wolf" {
                    3_000
                } else {
                    4_000
                };
                let slot = if spell == "Spirit of Wolf" { 5 } else { 2 };
                let cast = demo_cast_info(
                    slot,
                    spell,
                    total,
                    tick_count,
                    profile.index as u64,
                    refresh_rate_ms,
                    false,
                );
                DemoActionFrame::new(
                    DemoActionState::Casting,
                    Some(cast),
                    None,
                    format!("casting {spell}"),
                )
            } else if phase < 13 {
                DemoActionFrame::new(
                    DemoActionState::Fighting,
                    None,
                    None,
                    String::from("healing and pulling"),
                )
            } else {
                DemoActionFrame::new(
                    DemoActionState::Buffing,
                    None,
                    None,
                    String::from("buffing"),
                )
            }
        }
        DemoRole::Rogue => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 16);
            if phase < 8 {
                DemoActionFrame::new(
                    DemoActionState::Fighting,
                    None,
                    None,
                    format!(
                        "backstabbing {}",
                        profile.target_spawn_name.unwrap_or("the mob")
                    ),
                )
            } else if phase < 12 {
                DemoActionFrame::new(
                    DemoActionState::Looting,
                    None,
                    None,
                    String::from("looting"),
                )
            } else if phase < 14 {
                DemoActionFrame::new(
                    DemoActionState::Sitting,
                    None,
                    None,
                    String::from("sitting"),
                )
            } else {
                let nav = demo_nav_from_profile(profile, tick_count, NavStatus::Arrived);
                DemoActionFrame::new(
                    DemoActionState::Arrived,
                    None,
                    Some(nav),
                    String::from("arrived at the next pull point"),
                )
            }
        }
        DemoRole::Necromancer => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 24);
            if phase < 6 {
                DemoActionFrame::new(
                    DemoActionState::Sitting,
                    None,
                    None,
                    String::from("medding"),
                )
            } else if phase < 11 {
                let cast = demo_cast_info(
                    5,
                    "Fire",
                    3_000,
                    tick_count,
                    profile.index as u64,
                    refresh_rate_ms,
                    false,
                );
                DemoActionFrame::new(
                    DemoActionState::Casting,
                    Some(cast),
                    None,
                    format!(
                        "casting Fire on {}",
                        profile.target_spawn_name.unwrap_or("the mob")
                    ),
                )
            } else if phase < 15 {
                DemoActionFrame::new(
                    DemoActionState::Fighting,
                    None,
                    None,
                    String::from("dotting the mob"),
                )
            } else if phase < 18 {
                DemoActionFrame::new(
                    DemoActionState::Ducking,
                    None,
                    None,
                    String::from("ducking"),
                )
            } else {
                DemoActionFrame::new(
                    DemoActionState::Looting,
                    None,
                    None,
                    String::from("checking drops"),
                )
            }
        }
        DemoRole::Magician => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 22);
            if phase < 7 {
                let spell = if ((tick_count / 22) & 1) == 0 {
                    "Fire"
                } else {
                    "Ice Comet"
                };
                let total = if spell == "Fire" { 3_000 } else { 5_500 };
                let slot = if spell == "Fire" { 6 } else { 7 };
                let cast = demo_cast_info(
                    slot,
                    spell,
                    total,
                    tick_count,
                    profile.index as u64,
                    refresh_rate_ms,
                    false,
                );
                DemoActionFrame::new(
                    DemoActionState::Casting,
                    Some(cast),
                    None,
                    format!("casting {spell}"),
                )
            } else if phase < 11 {
                DemoActionFrame::new(
                    DemoActionState::Fighting,
                    None,
                    None,
                    String::from("burning the target"),
                )
            } else if phase < 15 {
                DemoActionFrame::new(
                    DemoActionState::Sitting,
                    None,
                    None,
                    String::from("medding"),
                )
            } else {
                DemoActionFrame::new(
                    DemoActionState::Looting,
                    None,
                    None,
                    String::from("looting"),
                )
            }
        }
        DemoRole::Paladin => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 20);
            if phase < 5 {
                let spell = if ((tick_count / 20) & 1) == 0 {
                    "Greater Heal"
                } else {
                    "Light Heal"
                };
                let total = if spell == "Greater Heal" {
                    4_000
                } else {
                    2_500
                };
                let slot = if spell == "Greater Heal" { 1 } else { 2 };
                let cast = demo_cast_info(
                    slot,
                    spell,
                    total,
                    tick_count,
                    profile.index as u64,
                    refresh_rate_ms,
                    false,
                );
                DemoActionFrame::new(
                    DemoActionState::Casting,
                    Some(cast),
                    None,
                    format!("casting {spell}"),
                )
            } else if phase < 10 {
                DemoActionFrame::new(
                    DemoActionState::Fighting,
                    None,
                    None,
                    String::from("tanking and healing"),
                )
            } else if phase < 14 {
                DemoActionFrame::new(
                    DemoActionState::Sitting,
                    None,
                    None,
                    String::from("medding"),
                )
            } else {
                DemoActionFrame::new(DemoActionState::Idle, None, None, String::from("ready"))
            }
        }
        DemoRole::Monk => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 24);
            if phase < 6 {
                DemoActionFrame::new(
                    DemoActionState::Feigned,
                    None,
                    None,
                    String::from("feigned"),
                )
            } else if phase < 10 {
                DemoActionFrame::new(
                    DemoActionState::Idle,
                    None,
                    None,
                    String::from("recovering"),
                )
            } else if phase < 16 {
                let nav = demo_nav_from_profile(
                    profile,
                    tick_count,
                    NavStatus::Moving {
                        waypoint_index: 1,
                        waypoint_count: 3,
                        distance_remaining: 80.0,
                    },
                );
                DemoActionFrame::new(
                    DemoActionState::Moving,
                    None,
                    Some(nav),
                    String::from("moving to the next waypoint"),
                )
            } else if phase < 20 {
                let nav = demo_nav_from_profile(
                    profile,
                    tick_count,
                    NavStatus::Stuck {
                        recovery_attempt: 2,
                    },
                );
                DemoActionFrame::new(
                    DemoActionState::Stuck,
                    None,
                    Some(nav),
                    String::from("stuck and recovering"),
                )
            } else {
                DemoActionFrame::new(
                    DemoActionState::Arrived,
                    None,
                    None,
                    String::from("arrived"),
                )
            }
        }
        DemoRole::Beastlord => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 18);
            if phase < 7 {
                DemoActionFrame::new(
                    DemoActionState::Fighting,
                    None,
                    None,
                    format!(
                        "fighting {}",
                        profile.target_spawn_name.unwrap_or("the mob")
                    ),
                )
            } else if phase < 11 {
                let nav = demo_nav_from_profile(
                    profile,
                    tick_count,
                    NavStatus::Moving {
                        waypoint_index: 2,
                        waypoint_count: 4,
                        distance_remaining: 110.0,
                    },
                );
                DemoActionFrame::new(
                    DemoActionState::Moving,
                    None,
                    Some(nav),
                    String::from("moving"),
                )
            } else if phase < 14 {
                DemoActionFrame::new(
                    DemoActionState::Sitting,
                    None,
                    None,
                    String::from("medding"),
                )
            } else {
                DemoActionFrame::new(DemoActionState::Idle, None, None, String::from("ready"))
            }
        }
        DemoRole::Berserker => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 20);
            if phase < 10 {
                DemoActionFrame::new(
                    DemoActionState::Fighting,
                    None,
                    None,
                    format!(
                        "whirling on {}",
                        profile.target_spawn_name.unwrap_or("the mob")
                    ),
                )
            } else if phase < 14 {
                DemoActionFrame::new(
                    DemoActionState::Looting,
                    None,
                    None,
                    String::from("looting"),
                )
            } else if phase < 18 {
                let nav = demo_nav_from_profile(profile, tick_count, NavStatus::Arrived);
                DemoActionFrame::new(
                    DemoActionState::Arrived,
                    None,
                    Some(nav),
                    String::from("arrived"),
                )
            } else {
                DemoActionFrame::new(DemoActionState::Idle, None, None, String::from("ready"))
            }
        }
        DemoRole::RecoveryWizard => {
            let phase = demo_cycle_phase(tick_count, profile.index as u64, 24);
            if phase < 6 {
                let nav = demo_nav_from_profile(
                    profile,
                    tick_count,
                    NavStatus::Moving {
                        waypoint_index: 1,
                        waypoint_count: 4,
                        distance_remaining: 160.0,
                    },
                );
                DemoActionFrame::new(
                    DemoActionState::Moving,
                    None,
                    Some(nav),
                    String::from("moving"),
                )
            } else if phase < 12 {
                let cast = demo_cast_info(
                    7,
                    "Ice Comet",
                    5_500,
                    tick_count,
                    profile.index as u64,
                    refresh_rate_ms,
                    false,
                );
                DemoActionFrame::new(
                    DemoActionState::Casting,
                    Some(cast),
                    None,
                    format!(
                        "casting Ice Comet on {}",
                        profile.target_spawn_name.unwrap_or("the mob")
                    ),
                )
            } else if phase < 16 {
                let nav = demo_nav_from_profile(profile, tick_count, NavStatus::Arrived);
                DemoActionFrame::new(
                    DemoActionState::Arrived,
                    None,
                    Some(nav),
                    String::from("arrived"),
                )
            } else if phase < 20 {
                let nav = demo_nav_from_profile(
                    profile,
                    tick_count,
                    NavStatus::Stuck {
                        recovery_attempt: 1,
                    },
                );
                DemoActionFrame::new(
                    DemoActionState::Stuck,
                    None,
                    Some(nav),
                    String::from("stuck"),
                )
            } else {
                DemoActionFrame::new(
                    DemoActionState::Sitting,
                    None,
                    None,
                    String::from("medding"),
                )
            }
        }
    }
}

fn demo_cast_info(
    spell_slot: u8,
    spell_label: &'static str,
    total_cast_ms: u32,
    tick_count: u64,
    phase_offset: u64,
    refresh_rate_ms: u64,
    force_complete: bool,
) -> DemoCastInfo {
    let refresh_rate_ms = refresh_rate_ms.max(1);
    let cast_ticks = total_cast_ms
        .max(refresh_rate_ms as u32)
        .div_ceil(refresh_rate_ms as u32)
        .max(1) as u64;
    let phase = demo_cycle_phase(tick_count, phase_offset, cast_ticks);
    let elapsed_ticks = phase.min(cast_ticks.saturating_sub(1));
    let elapsed_ms = (elapsed_ticks * refresh_rate_ms).min(total_cast_ms as u64) as u32;
    let remaining_ms = total_cast_ms.saturating_sub(elapsed_ms);
    let progress = if force_complete || total_cast_ms == 0 {
        1.0
    } else {
        (elapsed_ms as f32 / total_cast_ms as f32).clamp(0.0, 1.0)
    };

    DemoCastInfo {
        spell_slot,
        spell_label,
        total_cast_ms,
        elapsed_ms,
        remaining_ms,
        progress,
    }
}

fn demo_nav_from_profile(
    profile: &DemoClientProfile,
    tick_count: u64,
    status: NavStatus,
) -> DemoNavInfo {
    let (x, y, z, _) =
        demo_player_position(profile.zone, profile.index).unwrap_or((0.0, 0.0, 0.0, 0.0));
    let destination = profile
        .nav_destination
        .map_or_else(|| String::from("Demo waypoint"), ToString::to_string);
    let waypoints = match status {
        NavStatus::Moving {
            waypoint_index: _,
            waypoint_count,
            distance_remaining: _,
        } => {
            let step = 12.0 + (profile.index as f32 * 0.5);
            let offset =
                demo_cycle_phase(tick_count, profile.index as u64, waypoint_count as u64) as f32;
            vec![
                Waypoint::new(x, y, z),
                Waypoint::new(x + step + offset, y + step * 0.5, z),
                Waypoint::new(x + step * 2.0, y + step, z),
            ]
        }
        NavStatus::Paused { waypoint_count, .. } => {
            let step = 10.0 + (profile.index as f32 * 0.5);
            let offset =
                demo_cycle_phase(tick_count, profile.index as u64, waypoint_count as u64) as f32;
            vec![
                Waypoint::new(x, y, z),
                Waypoint::new(x + step + offset, y + step * 0.25, z),
                Waypoint::new(x + step * 1.5, y + step * 0.75, z),
            ]
        }
        NavStatus::Stuck { .. } => vec![Waypoint::new(x, y, z), Waypoint::new(x + 2.0, y + 1.0, z)],
        NavStatus::Arrived => vec![Waypoint::new(x, y, z)],
        NavStatus::Idle => Vec::new(),
    };

    DemoNavInfo {
        status,
        destination,
        waypoints,
    }
}

fn demo_stand_state_for_action(action_state: DemoActionState) -> StandState {
    match action_state {
        DemoActionState::Sitting | DemoActionState::Medding => StandState::Sitting,
        DemoActionState::Ducking => StandState::Ducking,
        DemoActionState::Feigned => StandState::Feigned,
        DemoActionState::Looting => StandState::Looting,
        DemoActionState::Moving
        | DemoActionState::Arrived
        | DemoActionState::Stuck
        | DemoActionState::Casting
        | DemoActionState::Fighting
        | DemoActionState::Buffing
        | DemoActionState::Idle => StandState::Standing,
    }
}

fn demo_resource_values(
    role: DemoRole,
    action_state: DemoActionState,
    cast: Option<DemoCastInfo>,
) -> (i64, i32) {
    let (base_hp, base_mana) = match role {
        DemoRole::MainTank => (9_700, 0),
        DemoRole::ChainCleric | DemoRole::ChainClericTwo => (5_400, 7_200),
        DemoRole::Enchanter => (3_300, 5_700),
        DemoRole::Bard => (7_400, 0),
        DemoRole::Ranger => (6_900, 2_000),
        DemoRole::Wizard | DemoRole::RecoveryWizard => (3_000, 7_900),
        DemoRole::ShadowKnight => (7_200, 3_600),
        DemoRole::Shaman => (4_500, 6_000),
        DemoRole::Druid => (4_000, 7_200),
        DemoRole::Rogue => (5_900, 0),
        DemoRole::Necromancer => (3_700, 7_000),
        DemoRole::Magician => (3_300, 7_400),
        DemoRole::Paladin => (7_700, 4_600),
        DemoRole::Monk => (6_700, 0),
        DemoRole::Beastlord => (5_900, 4_000),
        DemoRole::Berserker => (6_400, 0),
    };

    let hp_delta = match action_state {
        DemoActionState::Fighting => -200,
        DemoActionState::Casting => -75,
        DemoActionState::Moving => -50,
        DemoActionState::Looting => -25,
        DemoActionState::Feigned => -150,
        _ => 0,
    };
    let mana_delta = if let Some(cast) = cast {
        -((cast.total_cast_ms / 50) as i32)
    } else {
        match action_state {
            DemoActionState::Sitting | DemoActionState::Medding => 250,
            DemoActionState::Buffing => -75,
            DemoActionState::Fighting => -100,
            _ => 0,
        }
    };

    ((base_hp + hp_delta).max(0), (base_mana + mana_delta).max(0))
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
    fn demo_scheduler_covers_key_action_states() {
        assert_eq!(
            demo_client_action_state("Iceweaver02", 1001, 0),
            Some(DemoActionState::Casting)
        );
        assert_eq!(
            demo_client_action_state("Coldchain03", 1002, 3),
            Some(DemoActionState::Ducking)
        );
        assert_eq!(
            demo_client_action_state("Tundrablade05", 1004, 0),
            Some(DemoActionState::Moving)
        );
        assert_eq!(
            demo_client_action_state("Holyblade13", 1012, 0),
            Some(DemoActionState::Sitting)
        );
        assert_eq!(
            demo_client_action_state("Swiftfist14", 1013, 16),
            Some(DemoActionState::Feigned)
        );
        assert_eq!(
            demo_client_action_state("Ragecleave16", 1015, 0),
            Some(DemoActionState::Arrived)
        );
        assert_eq!(
            demo_client_action_state("Glacialsurge18", 1017, 0),
            Some(DemoActionState::Stuck)
        );
    }

    #[test]
    fn demo_ch_snapshot_exposes_exact_cast_info() {
        let snapshot = demo_client_snapshot("Iceweaver02", 1001, 0, 250).unwrap();
        let cast = snapshot.cast.expect("CH cleric should be casting");

        assert_eq!(snapshot.action_state, DemoActionState::Casting);
        assert_eq!(cast.spell_label, "Complete Heal");
        assert_eq!(cast.total_cast_ms, 10_000);
        assert!(cast.is_active());
        assert!(cast.progress > 0.0 && cast.progress < 1.0);
        assert_eq!(snapshot.action_label, "casting Complete Heal on Dmft01");
        assert!(snapshot.status_line.contains("Complete Heal"));
        assert!(demo_client_cast_active("Iceweaver02", 1001, 0).unwrap());
        assert!(demo_eq_cast_state(snapshot.cast).is_casting());
    }

    #[test]
    fn demo_nav_scripting_covers_moving_and_stuck_states() {
        let moving = demo_client_nav_info("Tundrablade05", 1004, 0).expect("moving nav");
        assert!(matches!(moving.status, NavStatus::Moving { .. }));
        assert_eq!(moving.destination, "Permafrost ridge");
        assert!(!moving.waypoints.is_empty());

        let arrived = demo_client_nav_info("Ragecleave16", 1015, 0).expect("arrived nav");
        assert_eq!(arrived.status, NavStatus::Arrived);
        assert_eq!(arrived.destination, "Great Divide camp lane");

        let stuck = demo_client_nav_info("Glacialsurge18", 1017, 0).expect("stuck nav");
        assert!(matches!(
            stuck.status,
            NavStatus::Stuck {
                recovery_attempt: 1
            }
        ));
        assert_eq!(stuck.destination, "Great Divide overlook");
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
