//! Raid-wide state aggregation and command relay planning.
//!
//! This module is intentionally data-only. The orchestrator owns live IPC
//! delivery; this shared layer builds deterministic raid/camp snapshots that
//! dashboards, relays, and future command coordinators can consume.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    combat::CombatStatus,
    types::{ClientId, GameState},
};

const LOW_HP_THRESHOLD: f32 = 35.0;

/// One live client input for raid aggregation.
#[derive(Debug, Clone, Copy)]
pub struct RaidMemberInput<'a> {
    pub client_id: ClientId,
    pub character_name: Option<&'a str>,
    pub group_id: u8,
    pub state: &'a GameState,
    pub stale_ticks: Option<u64>,
}

/// Aggregates per-client game snapshots into raid/camp views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaidStateAggregator {
    stale_tick_threshold: u64,
}

impl RaidStateAggregator {
    #[must_use]
    pub fn new(stale_tick_threshold: u64) -> Self {
        Self {
            stale_tick_threshold,
        }
    }

    /// Build a deterministic raid snapshot from the current client states.
    #[must_use]
    pub fn aggregate<'a, I>(&self, generated_tick: u64, inputs: I) -> RaidSnapshot
    where
        I: IntoIterator<Item = RaidMemberInput<'a>>,
    {
        let mut members: Vec<_> = inputs
            .into_iter()
            .filter_map(|input| member_from_input(input, self.stale_tick_threshold))
            .collect();

        members.sort_by_key(|member| (member.group_id, member.client_id));
        assign_formation_slots(&mut members);

        let camps = build_camps(&members);
        let events = build_events(&members, &camps);

        RaidSnapshot {
            generated_tick,
            raid_size: members.len(),
            camp_count: camps.len(),
            members,
            camps,
            events,
        }
    }
}

impl Default for RaidStateAggregator {
    fn default() -> Self {
        Self::new(3)
    }
}

/// Raid member liveness used by coordination consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RaidMemberStatus {
    Alive,
    Dead,
    Zoning,
    Stale,
}

/// Raid-wide view of one managed character.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaidMemberState {
    pub client_id: ClientId,
    pub character_name: String,
    pub group_id: u8,
    pub formation_slot: u8,
    pub zone_short_name: String,
    pub hp_pct: f32,
    pub mana_pct: f32,
    pub is_dead: bool,
    pub status: RaidMemberStatus,
    pub target_spawn_id: Option<u32>,
}

/// One group/camp slice inside a raid snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaidCampSnapshot {
    pub group_id: u8,
    pub zone_short_name: String,
    pub member_count: usize,
    pub alive_count: usize,
    pub dead_count: usize,
    pub low_hp_count: usize,
    pub assist_target_spawn_id: Option<u32>,
    pub members: Vec<ClientId>,
}

/// Raid events derived from member state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RaidEvent {
    MemberDown {
        client_id: ClientId,
        character_name: String,
        group_id: u8,
    },
    GroupWipeLikely {
        group_id: u8,
        zone_short_name: String,
    },
}

/// Scope for a planned raid command relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RaidCommandScope {
    Raid,
    Group { group_id: u8 },
    Member { client_id: ClientId },
}

/// One planned command delivery for the orchestrator to dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaidCommandDispatch {
    pub client_id: ClientId,
    pub command: String,
}

/// Deterministic raid-wide snapshot.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RaidSnapshot {
    pub generated_tick: u64,
    pub raid_size: usize,
    pub camp_count: usize,
    pub members: Vec<RaidMemberState>,
    pub camps: Vec<RaidCampSnapshot>,
    pub events: Vec<RaidEvent>,
}

impl RaidSnapshot {
    #[must_use]
    pub fn is_multi_camp(&self) -> bool {
        self.camp_count >= 2
    }

    /// Plan a command relay without performing live IPC.
    #[must_use]
    pub fn relay_plan(
        &self,
        scope: RaidCommandScope,
        command: impl Into<String>,
    ) -> Vec<RaidCommandDispatch> {
        let command = command.into();
        self.members
            .iter()
            .filter(|member| member.status != RaidMemberStatus::Stale)
            .filter(|member| match scope {
                RaidCommandScope::Raid => true,
                RaidCommandScope::Group { group_id } => member.group_id == group_id,
                RaidCommandScope::Member { client_id } => member.client_id == client_id,
            })
            .map(|member| RaidCommandDispatch {
                client_id: member.client_id,
                command: command.clone(),
            })
            .collect()
    }
}

fn member_from_input(
    input: RaidMemberInput<'_>,
    stale_tick_threshold: u64,
) -> Option<RaidMemberState> {
    let player = input.state.local_player.as_ref()?;
    let is_stale = input
        .stale_ticks
        .is_some_and(|ticks| ticks > stale_tick_threshold);
    let is_dead = matches!(input.state.combat_status, CombatStatus::Dead) || player.hp_current <= 0;
    let status = if is_stale {
        RaidMemberStatus::Stale
    } else if is_dead {
        RaidMemberStatus::Dead
    } else if input.state.is_zone_changing {
        RaidMemberStatus::Zoning
    } else {
        RaidMemberStatus::Alive
    };

    Some(RaidMemberState {
        client_id: input.client_id,
        character_name: member_name(input.character_name, player),
        group_id: input.group_id,
        formation_slot: 0,
        zone_short_name: input.state.zone_short_name.clone(),
        hp_pct: player.hp_pct(),
        mana_pct: player.mana_pct(),
        is_dead,
        status,
        target_spawn_id: input.state.target.as_ref().map(|target| target.spawn_id),
    })
}

fn member_name(fallback: Option<&str>, player: &crate::types::SpawnData) -> String {
    if !player.displayed_name.trim().is_empty() {
        player.displayed_name.clone()
    } else if !player.name.trim().is_empty() {
        player.name.clone()
    } else {
        fallback.unwrap_or("Unknown").to_string()
    }
}

fn assign_formation_slots(members: &mut [RaidMemberState]) {
    let mut next_slot = BTreeMap::<u8, u8>::new();
    for member in members {
        let slot = next_slot.entry(member.group_id).or_insert(1);
        member.formation_slot = *slot;
        *slot = slot.saturating_add(1);
    }
}

fn build_camps(members: &[RaidMemberState]) -> Vec<RaidCampSnapshot> {
    let mut grouped = BTreeMap::<u8, Vec<&RaidMemberState>>::new();
    for member in members {
        grouped.entry(member.group_id).or_default().push(member);
    }

    grouped
        .into_iter()
        .map(|(group_id, members)| build_camp(group_id, &members))
        .collect()
}

fn build_camp(group_id: u8, members: &[&RaidMemberState]) -> RaidCampSnapshot {
    let alive_count = members
        .iter()
        .filter(|member| member.status == RaidMemberStatus::Alive)
        .count();
    let dead_count = members.iter().filter(|member| member.is_dead).count();
    let low_hp_count = members
        .iter()
        .filter(|member| {
            member.status == RaidMemberStatus::Alive && member.hp_pct <= LOW_HP_THRESHOLD
        })
        .count();

    RaidCampSnapshot {
        group_id,
        zone_short_name: camp_zone(members),
        member_count: members.len(),
        alive_count,
        dead_count,
        low_hp_count,
        assist_target_spawn_id: camp_assist_target(members),
        members: members.iter().map(|member| member.client_id).collect(),
    }
}

fn camp_zone(members: &[&RaidMemberState]) -> String {
    let Some(first) = members.first() else {
        return String::new();
    };
    if members
        .iter()
        .all(|member| member.zone_short_name == first.zone_short_name)
    {
        first.zone_short_name.clone()
    } else {
        "mixed".to_string()
    }
}

fn camp_assist_target(members: &[&RaidMemberState]) -> Option<u32> {
    let mut counts = BTreeMap::<u32, usize>::new();
    for member in members {
        if member.status == RaidMemberStatus::Alive
            && let Some(target_spawn_id) = member.target_spawn_id
        {
            *counts.entry(target_spawn_id).or_default() += 1;
        }
    }

    counts
        .into_iter()
        .max_by_key(|(target_spawn_id, count)| (*count, std::cmp::Reverse(*target_spawn_id)))
        .map(|(target_spawn_id, _)| target_spawn_id)
}

fn build_events(members: &[RaidMemberState], camps: &[RaidCampSnapshot]) -> Vec<RaidEvent> {
    let mut events = Vec::new();
    for member in members {
        if member.status == RaidMemberStatus::Dead {
            events.push(RaidEvent::MemberDown {
                client_id: member.client_id,
                character_name: member.character_name.clone(),
                group_id: member.group_id,
            });
        }
    }

    for camp in camps {
        if camp.member_count > 0 && camp.alive_count == 0 && camp.dead_count > 0 {
            events.push(RaidEvent::GroupWipeLikely {
                group_id: camp.group_id,
                zone_short_name: camp.zone_short_name.clone(),
            });
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{combat::CombatStatus, nav::NavStatus, types::SpawnData};

    fn spawn(id: u32, name: &str, hp: i64) -> SpawnData {
        SpawnData {
            spawn_id: id,
            name: name.into(),
            displayed_name: name.into(),
            spawn_type: 0,
            level: 60,
            class_id: 1,
            race_id: 1,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            heading: 0.0,
            hp_current: hp,
            hp_max: 1000,
            mana_current: 500,
            mana_max: 1000,
            endurance_current: 100,
            endurance_max: 100,
            speed_run: 0.0,
            stand_state: if hp <= 0 { 111 } else { 0 },
            is_gm: false,
            combat_target_id: None,
        }
    }

    fn state(client_id: ClientId, zone: &str, hp: i64, target: Option<u32>) -> GameState {
        GameState {
            client_id,
            local_player: Some(spawn(client_id, &format!("Char{client_id}"), hp)),
            target: target.map(|id| spawn(id, "a raid target", 1000)),
            nearby_spawns: Vec::new(),
            timestamp_ms: 0,
            nav_status: NavStatus::Idle,
            combat_status: if hp <= 0 {
                CombatStatus::Dead
            } else {
                CombatStatus::Idle
            },
            zone_short_name: zone.into(),
            zone_long_name: zone.into(),
            active_buffs: Vec::new(),
            pet: None,
            actual_version: None,
            is_zone_changing: false,
        }
    }

    #[test]
    fn aggregates_two_camps_and_flags_wipe_event() {
        let states: Vec<_> = (1..=8)
            .map(|client_id| {
                let hp = if client_id >= 5 { 0 } else { 1000 };
                state(client_id, "kael", hp, Some(9000))
            })
            .collect();
        let inputs = states.iter().map(|state| RaidMemberInput {
            client_id: state.client_id,
            character_name: None,
            group_id: if state.client_id <= 4 { 1 } else { 2 },
            state,
            stale_ticks: Some(0),
        });

        let snapshot = RaidStateAggregator::default().aggregate(42, inputs);

        assert_eq!(snapshot.raid_size, 8);
        assert!(snapshot.is_multi_camp());
        assert_eq!(snapshot.camps[0].alive_count, 4);
        assert_eq!(snapshot.camps[1].dead_count, 4);
        assert_eq!(snapshot.members[4].formation_slot, 1);
        assert!(snapshot.events.contains(&RaidEvent::GroupWipeLikely {
            group_id: 2,
            zone_short_name: "kael".into(),
        }));
    }

    #[test]
    fn relay_plan_targets_raid_group_or_single_member() {
        let states = [state(1, "kael", 1000, None), state(2, "kael", 1000, None)];
        let inputs = states.iter().map(|state| RaidMemberInput {
            client_id: state.client_id,
            character_name: None,
            group_id: state.client_id as u8,
            state,
            stale_ticks: Some(0),
        });
        let snapshot = RaidStateAggregator::default().aggregate(1, inputs);

        assert_eq!(
            snapshot
                .relay_plan(RaidCommandScope::Raid, "/assist main")
                .len(),
            2
        );
        assert_eq!(
            snapshot.relay_plan(RaidCommandScope::Group { group_id: 2 }, "/burn"),
            vec![RaidCommandDispatch {
                client_id: 2,
                command: "/burn".into(),
            }]
        );
        assert_eq!(
            snapshot.relay_plan(RaidCommandScope::Member { client_id: 1 }, "/stop"),
            vec![RaidCommandDispatch {
                client_id: 1,
                command: "/stop".into(),
            }]
        );
    }
}
