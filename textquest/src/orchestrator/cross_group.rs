//! Same-zone cross-group emergency coordination for rez chains and assist
//! calls.
//!
//! The base camp loop is still group-local. This coordinator only watches the
//! shared live roster and issues a small set of cross-group rescue actions when
//! another group in the same zone is collapsing and cannot stabilize itself.

use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};

use crate::{camp::state::CampAction, eq::structs::EqClass};
use textquest_common::types::{GameState, SpawnData};

use super::{STALE_TICK_THRESHOLD, session_control::SessionControl};

const LOW_HP_THRESHOLD: f32 = 35.0;
const RESPONDER_MIN_HP_THRESHOLD: f32 = 50.0;
const REZ_GEM: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RequestKind {
    Rez,
    Assist,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RequestKey {
    distressed_group_id: u8,
    zone: String,
    kind: RequestKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveAssignment {
    responder_group_id: u8,
    responder_pids: Vec<u32>,
    assist_anchor_name: Option<String>,
    rez_target_name: Option<String>,
}

#[derive(Debug, Clone)]
struct MemberSnapshot {
    pid: u32,
    name: String,
    role: MemberRole,
    is_dead: bool,
    hp_pct: f32,
    has_live_target: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemberRole {
    Cleric,
    Tank,
    Other,
}

#[derive(Debug, Clone)]
struct GroupSnapshot {
    group_id: u8,
    zone: String,
    members: Vec<MemberSnapshot>,
}

impl GroupSnapshot {
    fn dead_members(&self) -> Vec<&MemberSnapshot> {
        self.members
            .iter()
            .filter(|member| member.is_dead)
            .collect()
    }

    fn dead_count(&self) -> usize {
        self.members.iter().filter(|member| member.is_dead).count()
    }

    fn low_hp_count(&self) -> usize {
        self.members
            .iter()
            .filter(|member| !member.is_dead && member.hp_pct <= LOW_HP_THRESHOLD)
            .count()
    }

    fn alive_clerics(&self) -> Vec<&MemberSnapshot> {
        self.members
            .iter()
            .filter(|member| !member.is_dead && member.role == MemberRole::Cleric)
            .collect()
    }

    fn available_attackers(&self) -> Vec<&MemberSnapshot> {
        self.members
            .iter()
            .filter(|member| {
                !member.is_dead
                    && member.role != MemberRole::Cleric
                    && member.hp_pct >= RESPONDER_MIN_HP_THRESHOLD
            })
            .collect()
    }

    fn wants_rez_support(&self) -> bool {
        !self.dead_members().is_empty() && self.alive_clerics().is_empty()
    }

    fn wants_assist_support(&self) -> bool {
        self.assist_anchor_name().is_some()
            && (self.low_hp_count() >= 2 || !self.dead_members().is_empty())
    }

    fn is_stable_responder(&self) -> bool {
        self.dead_members().is_empty() && self.low_hp_count() == 0
    }

    fn assist_anchor_name(&self) -> Option<String> {
        self.members
            .iter()
            .find(|member| !member.is_dead && member.has_live_target)
            .or_else(|| self.members.iter().find(|member| !member.is_dead))
            .map(|member| member.name.clone())
    }

    fn rez_target_name(&self) -> Option<String> {
        let mut dead = self.dead_members();
        dead.sort_by(|a, b| {
            role_priority(a.role)
                .cmp(&role_priority(b.role))
                .then(a.pid.cmp(&b.pid))
        });
        dead.first().map(|member| member.name.clone())
    }
}

/// Cross-group same-zone emergency coordinator.
pub struct CrossGroupCoordinator {
    active_requests: HashMap<RequestKey, ActiveAssignment>,
}

impl CrossGroupCoordinator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_requests: HashMap::new(),
        }
    }

    /// Evaluate the live roster and emit new cross-group emergency commands.
    pub fn tick(
        &mut self,
        session_controls: &HashMap<u32, SessionControl>,
        client_names: &HashMap<u32, String>,
        client_class_names: &HashMap<u32, String>,
        game_states: &HashMap<u32, GameState>,
        state_timestamps: &HashMap<u32, u64>,
        tick_count: u64,
    ) -> Vec<(u32, CampAction)> {
        let groups = build_group_snapshots(
            session_controls,
            client_names,
            client_class_names,
            game_states,
            state_timestamps,
            tick_count,
        );
        let desired = self.desired_assignments(&groups);

        let mut commands = Vec::new();
        let mut next_active = HashMap::new();

        for (key, assignment) in desired {
            let is_unchanged = self.active_requests.get(&key) == Some(&assignment);
            if !is_unchanged {
                emit_assignment_commands(&key.kind, &assignment, &mut commands);
            }
            next_active.insert(key, assignment);
        }

        self.active_requests = next_active;
        commands
    }

    fn desired_assignments(&self, groups: &[GroupSnapshot]) -> Vec<(RequestKey, ActiveAssignment)> {
        let mut requests = Vec::new();

        for distressed in groups {
            if distressed.wants_rez_support() {
                requests.push(PendingRequest {
                    distressed,
                    kind: RequestKind::Rez,
                });
            }

            if distressed.wants_assist_support() {
                requests.push(PendingRequest {
                    distressed,
                    kind: RequestKind::Assist,
                });
            }
        }

        requests.sort_by_key(|request| request.priority_key());

        let mut desired = Vec::new();
        let mut reserved_responder_groups = HashSet::new();
        for request in requests {
            let assignment = match request.kind {
                RequestKind::Rez => self.select_rez_assignment(
                    request.distressed,
                    groups,
                    &reserved_responder_groups,
                ),
                RequestKind::Assist => self.select_assist_assignment(
                    request.distressed,
                    groups,
                    &reserved_responder_groups,
                ),
            };

            if let Some((key, assignment)) = assignment {
                reserved_responder_groups.insert(assignment.responder_group_id);
                desired.push((key, assignment));
            }
        }

        desired
    }

    fn select_rez_assignment(
        &self,
        distressed: &GroupSnapshot,
        groups: &[GroupSnapshot],
        reserved_responder_groups: &HashSet<u8>,
    ) -> Option<(RequestKey, ActiveAssignment)> {
        let rez_target_name = distressed.rez_target_name()?;

        let responder = groups
            .iter()
            .filter(|group| {
                group.group_id != distressed.group_id
                    && !reserved_responder_groups.contains(&group.group_id)
                    && group.zone == distressed.zone
                    && group.is_stable_responder()
                    && !group.alive_clerics().is_empty()
            })
            .min_by(|a, b| {
                b.alive_clerics()
                    .len()
                    .cmp(&a.alive_clerics().len())
                    .then(a.group_id.cmp(&b.group_id))
            })?;

        let responder_pid = responder
            .alive_clerics()
            .into_iter()
            .map(|member| member.pid)
            .min()?;

        Some((
            RequestKey {
                distressed_group_id: distressed.group_id,
                zone: distressed.zone.clone(),
                kind: RequestKind::Rez,
            },
            ActiveAssignment {
                responder_group_id: responder.group_id,
                responder_pids: vec![responder_pid],
                assist_anchor_name: None,
                rez_target_name: Some(rez_target_name),
            },
        ))
    }

    fn select_assist_assignment(
        &self,
        distressed: &GroupSnapshot,
        groups: &[GroupSnapshot],
        reserved_responder_groups: &HashSet<u8>,
    ) -> Option<(RequestKey, ActiveAssignment)> {
        let assist_anchor_name = distressed.assist_anchor_name()?;

        let responder = groups
            .iter()
            .filter(|group| {
                group.group_id != distressed.group_id
                    && !reserved_responder_groups.contains(&group.group_id)
                    && group.zone == distressed.zone
                    && group.is_stable_responder()
                    && !group.available_attackers().is_empty()
            })
            .min_by(|a, b| {
                b.available_attackers()
                    .len()
                    .cmp(&a.available_attackers().len())
                    .then(a.group_id.cmp(&b.group_id))
            })?;

        let responder_pids = responder
            .available_attackers()
            .into_iter()
            .map(|member| member.pid)
            .collect::<Vec<_>>();

        Some((
            RequestKey {
                distressed_group_id: distressed.group_id,
                zone: distressed.zone.clone(),
                kind: RequestKind::Assist,
            },
            ActiveAssignment {
                responder_group_id: responder.group_id,
                responder_pids,
                assist_anchor_name: Some(assist_anchor_name),
                rez_target_name: None,
            },
        ))
    }
}

impl Default for CrossGroupCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

struct PendingRequest<'a> {
    distressed: &'a GroupSnapshot,
    kind: RequestKind,
}

impl PendingRequest<'_> {
    fn priority_key(&self) -> (u8, Reverse<usize>, Reverse<usize>, u8) {
        let kind_priority = match self.kind {
            RequestKind::Rez => 0,
            RequestKind::Assist => 1,
        };

        (
            kind_priority,
            Reverse(self.distressed.dead_count()),
            Reverse(self.distressed.low_hp_count()),
            self.distressed.group_id,
        )
    }
}

fn emit_assignment_commands(
    kind: &RequestKind,
    assignment: &ActiveAssignment,
    commands: &mut Vec<(u32, CampAction)>,
) {
    match kind {
        RequestKind::Rez => {
            let Some(responder_pid) = assignment.responder_pids.first().copied() else {
                return;
            };
            let Some(target_name) = assignment.rez_target_name.as_deref() else {
                return;
            };
            commands.push((
                responder_pid,
                CampAction::Slash(format!("/target {target_name}")),
            ));
            commands.push((responder_pid, CampAction::Slash(format!("/cast {REZ_GEM}"))));
        }
        RequestKind::Assist => {
            let Some(anchor_name) = assignment.assist_anchor_name.as_deref() else {
                return;
            };
            for responder_pid in &assignment.responder_pids {
                commands.push((
                    *responder_pid,
                    CampAction::Slash(format!("/assist {anchor_name}")),
                ));
                commands.push((*responder_pid, CampAction::Slash("/attack".to_string())));
            }
        }
    }
}

fn build_group_snapshots(
    session_controls: &HashMap<u32, SessionControl>,
    client_names: &HashMap<u32, String>,
    client_class_names: &HashMap<u32, String>,
    game_states: &HashMap<u32, GameState>,
    state_timestamps: &HashMap<u32, u64>,
    tick_count: u64,
) -> Vec<GroupSnapshot> {
    let mut grouped: HashMap<(u8, String), Vec<MemberSnapshot>> = HashMap::new();

    for (&pid, control) in session_controls {
        if !control.is_active() || control.group_id == 0 {
            continue;
        }

        let Some(state) = game_states.get(&pid) else {
            continue;
        };
        let Some(&last_update) = state_timestamps.get(&pid) else {
            continue;
        };
        if tick_count.saturating_sub(last_update) > STALE_TICK_THRESHOLD {
            continue;
        }
        let Some(local_player) = state.local_player.as_ref() else {
            continue;
        };
        if state.zone_short_name.trim().is_empty() {
            continue;
        }

        let name = client_names
            .get(&pid)
            .cloned()
            .or_else(|| {
                (!local_player.displayed_name.is_empty())
                    .then(|| local_player.displayed_name.clone())
            })
            .unwrap_or_else(|| format!("pid-{pid}"));
        let role = classify_member_role(client_class_names.get(&pid), local_player);
        let member = MemberSnapshot {
            pid,
            name,
            role,
            is_dead: is_dead(local_player),
            hp_pct: local_player.hp_pct(),
            has_live_target: state
                .target
                .as_ref()
                .is_some_and(|target| target.spawn_type == 1 && target.hp_current > 0),
        };

        grouped
            .entry((control.group_id, state.zone_short_name.clone()))
            .or_default()
            .push(member);
    }

    let mut groups = grouped
        .into_iter()
        .map(|((group_id, zone), mut members)| {
            members.sort_by_key(|member| member.pid);
            GroupSnapshot {
                group_id,
                zone,
                members,
            }
        })
        .collect::<Vec<_>>();

    groups.sort_by(|a, b| a.group_id.cmp(&b.group_id).then(a.zone.cmp(&b.zone)));
    groups
}

fn classify_member_role(class_name: Option<&String>, local_player: &SpawnData) -> MemberRole {
    let class_token = class_name
        .map(|value| value.as_str())
        .or_else(|| EqClass::from_id(local_player.class_id).map(|class| class.short_name()))
        .unwrap_or_default();

    if is_cleric_class(class_token) {
        MemberRole::Cleric
    } else if is_tank_class(class_token) {
        MemberRole::Tank
    } else {
        MemberRole::Other
    }
}

fn is_cleric_class(class_token: &str) -> bool {
    class_token.eq_ignore_ascii_case("cleric") || class_token.eq_ignore_ascii_case("clr")
}

fn is_tank_class(class_token: &str) -> bool {
    class_token.eq_ignore_ascii_case("warrior")
        || class_token.eq_ignore_ascii_case("war")
        || class_token.eq_ignore_ascii_case("paladin")
        || class_token.eq_ignore_ascii_case("pal")
        || class_token.eq_ignore_ascii_case("shadowknight")
        || class_token.eq_ignore_ascii_case("sk")
}

fn is_dead(local_player: &SpawnData) -> bool {
    local_player.hp_current <= 0 || local_player.stand_state == 111
}

fn role_priority(role: MemberRole) -> u8 {
    match role {
        MemberRole::Cleric => 0,
        MemberRole::Tank => 1,
        MemberRole::Other => 2,
    }
}
