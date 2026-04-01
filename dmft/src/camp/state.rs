//! Camp loop state machine — drives the pull/fight/loot/med cycle.
//!
//! # Integration architecture
//!
//! Two systems cooperate for combat:
//!
//! - **Camp loop** (this module, orchestrator-side): Generates macro-level slash commands
//!   (`/assist`, `/attack`, `/target`) to drive the pull→fight→loot→med cycle. It manages
//!   group-level flow: who pulls, when to engage, when to loot, when to med.
//!
//! - **Combatant FSM** (`dmft-dll/src/combat/state.rs`, DLL-side): Handles micro-level
//!   execution per character — class strategy spell rotations, melee skill firing, GCD
//!   tracking, mana governance, and `HolyShit` emergency overrides.
//!
//! Both are needed: the camp loop orchestrates the group, the combatant executes per-character
//! combat logic. Integration point: `transition_to_fighting()` sends slash commands AND should
//! trigger a `CombatEngage` IPC command so each DLL's Combatant FSM transitions from Idle to
//! Engaging (activating class strategies).
//!
//! Recovery: The `RecoveryTracker` (from `recovery.rs`) detects dead members and generates
//! rez commands. It is checked every tick before the main state match — if recovery is in
//! progress, pulling is paused until all members are alive.

use crate::camp::buffs::{BuffTracker, check_buffs};
use crate::camp::cc::{CcMember, CcTracker};
use crate::camp::class_config::ClassConfig;
use crate::camp::config::CampConfig;
use crate::camp::loot::{CorpseEntry, LootConfig, LootCycle};
use crate::camp::personality::PersonalityProfile;
use crate::camp::recovery::{RecoveryTracker, death_commands_with_roles};
use std::collections::HashMap;

/// Events that can occur during the camp loop, triggering reactive behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CampEvent {
    /// A charm has broken — immediate emergency CC needed.
    CharmBreak { spawn_id: u32 },
    /// A new add has spawned or aggroed within camp radius.
    AddSpawned { spawn_id: u32, name: String },
    /// A CC effect is about to expire on a mob.
    CcExpiring { spawn_id: u32 },
}

/// Real-time game state snapshot for the camp loop.
/// When available, the camp loop uses these values for smarter transitions
/// instead of fixed tick timers.
#[derive(Debug, Clone)]
pub struct CampSnapshot {
    pub healer_mana_pct: f32,
    pub tank_hp_pct: f32,
    pub target_hp_pct: Option<f32>,
    pub target_is_dead: bool,
    /// Spawn ID of the tank's current target (for `CombatEngage` commands).
    pub target_spawn_id: Option<u32>,
    /// Per-member HP values: `(pid, current_hp)`. Used to detect deaths
    /// and trigger recovery (rez commands). Empty when HP data is unavailable.
    pub member_hp: Vec<(u32, i32)>,
}

/// An action the camp loop wants executed on a specific client.
/// Wraps both slash commands and structured IPC commands so the
/// orchestrator can dispatch them appropriately.
#[derive(Debug, Clone)]
pub enum CampAction {
    /// A slash command string (e.g., "/attack", "/assist Tankname").
    Slash(String),
    /// Engage the Combatant FSM against a specific spawn.
    CombatEngage { target_id: u32 },
    /// Disengage the Combatant FSM.
    CombatDisengage,
}

impl CampAction {
    /// Helper to convert a vec of slash command strings into `CampActions`.
    #[must_use]
    pub fn from_slash_vec(cmds: Vec<(u32, String)>) -> Vec<(u32, CampAction)> {
        cmds.into_iter()
            .map(|(pid, cmd)| (pid, CampAction::Slash(cmd)))
            .collect()
    }

    /// Extract the slash command string, if this is a Slash action.
    #[must_use]
    pub fn as_slash(&self) -> Option<&str> {
        match self {
            CampAction::Slash(s) => Some(s),
            _ => None,
        }
    }

    /// Check if this action's slash text contains a substring.
    #[must_use]
    pub fn contains(&self, needle: &str) -> bool {
        self.as_slash().is_some_and(|s| s.contains(needle))
    }

    /// Check if this action's slash text starts with a prefix.
    #[must_use]
    pub fn starts_with(&self, prefix: &str) -> bool {
        self.as_slash().is_some_and(|s| s.starts_with(prefix))
    }
}

impl PartialEq<&str> for CampAction {
    fn eq(&self, other: &&str) -> bool {
        self.as_slash() == Some(*other)
    }
}

impl PartialEq<str> for CampAction {
    fn eq(&self, other: &str) -> bool {
        self.as_slash() == Some(other)
    }
}

/// Current phase of the camp loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CampState {
    Idle,
    Pulling { started_tick: u64 },
    Fighting { started_tick: u64 },
    Looting { started_tick: u64 },
    Medding { started_tick: u64 },
    Buffing { started_tick: u64 },
}

/// Role a group member fills in the camp loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Role {
    Tank,
    Healer,
    CC,
    Dps,
    Puller,
    Bard,
}

/// A single member of the camp group.
#[derive(Debug, Clone)]
pub struct CampMember {
    pub pid: u32,
    pub name: String,
    pub role: Role,
    pub personality: PersonalityProfile,
}

impl CampMember {
    /// Create a new camp member with an auto-generated personality from their name.
    #[must_use]
    pub fn new(pid: u32, name: String, role: Role) -> Self {
        let personality = PersonalityProfile::generate(&name);
        Self {
            pid,
            name,
            role,
            personality,
        }
    }
}

/// Timer durations (in ticks) for each phase.
pub const PULL_DURATION: u64 = 5;
const FIGHT_DURATION: u64 = 15;
const LOOT_DURATION: u64 = 3;
const MED_DURATION: u64 = 10;
const BUFF_DURATION: u64 = 8;

/// Ticks before CC expiry to start re-casting.
const CC_REMEZ_BUFFER: u64 = 3;

/// The camp loop state machine. Each `tick()` call advances state and
/// returns slash commands to send to EQ clients via IPC.
pub struct CampLoop {
    pub config: CampConfig,
    pub state: CampState,
    pub members: Vec<CampMember>,
    pub tick: u64,
    pub last_pull_target: String,
    pub cc_tracker: CcTracker,
    pub cc_members: Vec<CcMember>,
    pub pending_events: Vec<CampEvent>,
    /// Loot configuration for the camp.
    pub loot_config: LootConfig,
    /// Active loot cycle (Some during Looting phase).
    pub loot_cycle: Option<LootCycle>,
    /// Corpses from recent kills, tracked for looting.
    pub pending_corpses: Vec<CorpseEntry>,
    /// Tracks death/recovery state for rez coordination.
    pub recovery: RecoveryTracker,
    /// Spell gem number used for resurrection (e.g., 5 for cleric rez in gem 5).
    pub rez_gem: u8,
    /// Buff duration tracker for rebuff scheduling.
    pub buff_tracker: BuffTracker,
    /// Class configs keyed by role string (e.g., "healer", "cc").
    pub class_configs: HashMap<String, ClassConfig>,
}

impl CampLoop {
    #[must_use]
    pub fn new(config: CampConfig, members: Vec<CampMember>) -> Self {
        let recovery_members: Vec<(u32, String)> =
            members.iter().map(|m| (m.pid, m.name.clone())).collect();
        Self {
            config,
            state: CampState::Idle,
            members,
            tick: 0,
            last_pull_target: String::new(),
            cc_tracker: CcTracker::new(),
            cc_members: Vec::new(),
            pending_events: Vec::new(),
            loot_config: LootConfig::default(),
            loot_cycle: None,
            pending_corpses: Vec::new(),
            recovery: RecoveryTracker::new(&recovery_members),
            rez_gem: 5,
            buff_tracker: BuffTracker::new(),
            class_configs: HashMap::new(),
        }
    }

    /// Record a corpse from a recent kill, to be looted during the Loot phase.
    pub fn record_kill(&mut self, spawn_id: u32, mob_name: String) {
        self.pending_corpses
            .push(CorpseEntry { spawn_id, mob_name });
    }

    /// Push an event to be processed on the next tick.
    pub fn push_event(&mut self, event: CampEvent) {
        self.pending_events.push(event);
    }

    /// Process all pending events, returning commands. Called at the start of tick().
    fn process_events(&mut self) -> Vec<(u32, CampAction)> {
        let mut commands = Vec::new();
        let events: Vec<CampEvent> = self.pending_events.drain(..).collect();

        for event in events {
            commands.extend(self.process_event(event));
        }
        commands
    }

    /// Handle a single camp event.
    fn process_event(&mut self, event: CampEvent) -> Vec<(u32, CampAction)> {
        match event {
            CampEvent::CharmBreak { spawn_id } => CampAction::from_slash_vec(
                self.cc_tracker
                    .charm_break_response(spawn_id, &self.cc_members, self.tick),
            ),
            CampEvent::AddSpawned { spawn_id, name } => {
                self.cc_tracker.add_target(spawn_id, name);
                let mut cmds =
                    self.cc_tracker
                        .debuff_commands(spawn_id, &self.cc_members, self.tick);
                cmds.extend(self.cc_tracker.assign_cc(&mut self.cc_members, self.tick));
                CampAction::from_slash_vec(cmds)
            }
            CampEvent::CcExpiring { spawn_id } => {
                let all_cmds = self
                    .cc_tracker
                    .needs_remez(self.tick, 0, &mut self.cc_members);
                let target_cmd = format!("/target id {spawn_id}");
                let mut result = Vec::new();
                let mut matched = false;
                for (pid, cmd) in all_cmds {
                    if cmd == target_cmd {
                        matched = true;
                        result.push((pid, CampAction::Slash(cmd)));
                    } else if matched && cmd.starts_with("/cast") {
                        result.push((pid, CampAction::Slash(cmd)));
                        matched = false;
                    } else {
                        matched = false;
                    }
                }
                result
            }
        }
    }

    /// Advance the state machine by one tick. Returns `(pid, slash_command)` pairs
    /// to send to EQ clients.
    ///
    /// When `snapshot` is `Some`, real game state drives transitions (target dead,
    /// healer mana ready, tank HP emergency). Falls back to tick timers when `None`.
    pub fn tick(&mut self, snapshot: Option<&CampSnapshot>) -> Vec<(u32, CampAction)> {
        self.tick += 1;
        let mut commands: Vec<(u32, CampAction)> = Vec::new();

        // Cursor stuck watchdog: /autoinventory every 60 ticks as a safety net.
        // This is a no-op if cursor is empty.
        if self.tick.is_multiple_of(60) {
            for member in &self.members {
                commands.push((member.pid, CampAction::Slash("/autoinventory".into())));
            }
        }

        // --- Recovery check: detect deaths and issue rez commands ---
        if let Some(snap) = snapshot
            && !snap.member_hp.is_empty()
        {
            self.recovery.update_hp(&snap.member_hp, self.tick);
        }

        if self.recovery.recovery_in_progress() {
            // Build role map for rez prioritization
            let role_map: Vec<(u32, &str)> = self
                .members
                .iter()
                .map(|m| {
                    let role_str = match m.role {
                        Role::Healer => "Healer",
                        Role::Tank => "Tank",
                        Role::CC => "CC",
                        Role::Puller => "Puller",
                        Role::Dps => "DPS",
                        Role::Bard => "Bard",
                    };
                    (m.pid, role_str)
                })
                .collect();

            let cleric_pid = self.find_by_role(&Role::Healer).map(|m| m.pid);
            let rez_cmds = death_commands_with_roles(
                &mut self.recovery.members,
                cleric_pid,
                self.rez_gem,
                &role_map,
            );
            commands.extend(CampAction::from_slash_vec(rez_cmds));

            // Don't pull or advance the main loop while recovering
            return commands;
        }

        // Process pending events first (charm breaks, adds, etc.)
        commands.extend(self.process_events());

        // Check for CCs about to expire during fighting
        if matches!(self.state, CampState::Fighting { .. }) && !self.cc_members.is_empty() {
            let remez =
                self.cc_tracker
                    .needs_remez(self.tick, CC_REMEZ_BUFFER, &mut self.cc_members);
            commands.extend(CampAction::from_slash_vec(remez));
        }

        match self.state.clone() {
            CampState::Idle => {
                // Only pull if healer has enough mana (when we know).
                // Apply healer's personality jitter to the threshold.
                let pull_threshold = self.find_by_role(&Role::Healer).map_or(
                    f32::from(self.config.pull_mana_pct),
                    |h| {
                        h.personality
                            .adjust_mana_threshold(f32::from(self.config.pull_mana_pct))
                    },
                );
                let healer_ready = snapshot.is_none_or(|s| s.healer_mana_pct >= pull_threshold);
                if healer_ready {
                    self.transition_to_pulling(&mut commands);
                }
            }
            CampState::Pulling { started_tick } => {
                if self.tick - started_tick >= PULL_DURATION {
                    let target_id = snapshot.and_then(|s| s.target_spawn_id);
                    self.transition_to_fighting(&mut commands, target_id);
                }
            }
            CampState::Fighting { started_tick } => {
                // Emergency heal if tank HP < 20%
                if let Some(snap) = snapshot
                    && snap.tank_hp_pct < 20.0
                    && let Some(healer) = self.find_by_role(&Role::Healer)
                {
                    commands.push((healer.pid, CampAction::Slash("/cast 1".into())));
                }

                // Melee characters /face periodically, staggered by personality
                let fight_elapsed = self.tick - started_tick;
                if fight_elapsed > 0 {
                    for member in &self.members {
                        if member.role != Role::Tank && member.role != Role::Dps {
                            continue;
                        }
                        // Each member's clock starts after their phase_offset
                        let personal_elapsed =
                            fight_elapsed.saturating_sub(member.personality.phase_offset);
                        let face_interval = member.personality.adjust_delay(5);
                        if personal_elapsed > 0 && personal_elapsed % face_interval == 0 {
                            commands.push((member.pid, CampAction::Slash("/face".into())));
                        }
                    }
                }

                // Transition to looting: target dead (real data) or timer expired (fallback)
                let target_dead = snapshot.is_some_and(|s| s.target_is_dead);
                let timer_expired = self.tick - started_tick >= FIGHT_DURATION;
                if target_dead || timer_expired {
                    self.transition_to_looting(&mut commands);
                }
            }
            CampState::Looting { started_tick: _ } => {
                // Drive the loot cycle FSM if active.
                // Extract looter info before borrowing loot_cycle mutably.
                let looter = self
                    .find_all_by_role(&Role::Dps)
                    .first()
                    .or(self.members.first().as_ref())
                    .map(|m| (m.pid, m.personality.clone()));

                let cycle_done = if let Some(ref mut cycle) = self.loot_cycle {
                    if let Some((pid, personality)) = looter {
                        commands.extend(CampAction::from_slash_vec(cycle.tick(
                            pid,
                            self.tick,
                            &personality,
                        )));
                    }
                    cycle.is_done()
                } else {
                    true
                };

                if cycle_done {
                    self.loot_cycle = None;
                    self.transition_to_medding(&mut commands);
                }
            }
            CampState::Medding { started_tick } => {
                // Transition when healer mana is above pull threshold (real data) or timer (fallback).
                // Apply healer's personality jitter to the threshold.
                let med_threshold = self.find_by_role(&Role::Healer).map_or(
                    f32::from(self.config.pull_mana_pct),
                    |h| {
                        h.personality
                            .adjust_mana_threshold(f32::from(self.config.pull_mana_pct))
                    },
                );
                let mana_ready = snapshot.is_some_and(|s| s.healer_mana_pct >= med_threshold);
                let timer_expired = self.tick - started_tick >= MED_DURATION;
                if mana_ready || timer_expired {
                    // Check if any buffs need refreshing before going idle
                    let buff_cmds = check_buffs(
                        &self.buff_tracker,
                        &self.members,
                        &self.class_configs,
                        self.tick,
                        &CampState::Idle, // Check as if idle (both Idle and Medding are valid)
                    );
                    if buff_cmds.is_empty() {
                        self.transition_to_idle(&mut commands);
                    } else {
                        // Stand up before buffing (members are seated from medding)
                        for member in &self.members {
                            commands.push((member.pid, CampAction::Slash("/stand".into())));
                        }
                        // Transition to Buffing and emit the buff commands
                        commands.extend(CampAction::from_slash_vec(buff_cmds));
                        self.state = CampState::Buffing {
                            started_tick: self.tick,
                        };
                    }
                }
            }
            CampState::Buffing { started_tick } => {
                if self.tick - started_tick >= BUFF_DURATION {
                    self.transition_to_idle(&mut commands);
                }
            }
        }

        commands
    }

    /// Find the first member with a given role.
    fn find_by_role(&self, role: &Role) -> Option<&CampMember> {
        self.members.iter().find(|m| &m.role == role)
    }

    /// Find all members with a given role.
    fn find_all_by_role(&self, role: &Role) -> Vec<&CampMember> {
        self.members.iter().filter(|m| &m.role == role).collect()
    }

    /// Pick a pull target name. Uses configured mob names or a generic target.
    fn pick_pull_target(&self) -> String {
        if let Some(name) = self.config.pull_mob_names.first() {
            name.clone()
        } else {
            "a_mob".into()
        }
    }

    // -- State transitions --

    fn transition_to_pulling(&mut self, commands: &mut Vec<(u32, CampAction)>) {
        let target = self.pick_pull_target();
        self.last_pull_target = target.clone();

        // Puller targets and attacks
        if let Some(puller) = self.find_by_role(&Role::Puller) {
            commands.push((puller.pid, CampAction::Slash(format!("/target {target}"))));
            commands.push((puller.pid, CampAction::Slash("/attack".into())));
        } else if let Some(tank) = self.find_by_role(&Role::Tank) {
            // Fall back to tank as puller
            commands.push((tank.pid, CampAction::Slash(format!("/target {target}"))));
            commands.push((tank.pid, CampAction::Slash("/attack".into())));
        }

        self.state = CampState::Pulling {
            started_tick: self.tick,
        };
    }

    fn transition_to_fighting(
        &mut self,
        commands: &mut Vec<(u32, CampAction)>,
        target_spawn_id: Option<u32>,
    ) {
        let puller_name = self
            .find_by_role(&Role::Puller)
            .map(|m| m.name.clone())
            .unwrap_or_default();

        let tank_name = self
            .find_by_role(&Role::Tank)
            .map(|m| m.name.clone())
            .unwrap_or_default();

        // Tank assists puller and attacks
        if let Some(tank) = self.find_by_role(&Role::Tank) {
            if !puller_name.is_empty() {
                commands.push((
                    tank.pid,
                    CampAction::Slash(format!("/assist {puller_name}")),
                ));
            }
            commands.push((tank.pid, CampAction::Slash("/attack".into())));
            // Engage the Combatant FSM so class strategies activate
            if let Some(tid) = target_spawn_id {
                commands.push((tank.pid, CampAction::CombatEngage { target_id: tid }));
            }
        }

        // DPS assists tank and attacks
        let assist_name = if tank_name.is_empty() {
            &puller_name
        } else {
            &tank_name
        };

        for dps in self.find_all_by_role(&Role::Dps) {
            if !assist_name.is_empty() {
                commands.push((dps.pid, CampAction::Slash(format!("/assist {assist_name}"))));
            }
            commands.push((dps.pid, CampAction::Slash("/attack".into())));
            if let Some(tid) = target_spawn_id {
                commands.push((dps.pid, CampAction::CombatEngage { target_id: tid }));
            }
        }

        // Healer targets tank (healers don't CombatEngage — they heal)
        if let Some(healer) = self.find_by_role(&Role::Healer) {
            if !tank_name.is_empty() {
                commands.push((
                    healer.pid,
                    CampAction::Slash(format!("/target {tank_name}")),
                ));
            }
            // Healer also gets CombatEngage so healing strategies activate
            if let Some(tid) = target_spawn_id {
                commands.push((healer.pid, CampAction::CombatEngage { target_id: tid }));
            }
        }

        self.state = CampState::Fighting {
            started_tick: self.tick,
        };
    }

    fn transition_to_looting(&mut self, commands: &mut Vec<(u32, CampAction)>) {
        // Everyone stops attacking and disengages the Combatant FSM
        for member in &self.members {
            commands.push((member.pid, CampAction::Slash("/attack off".into())));
            commands.push((member.pid, CampAction::CombatDisengage));
        }

        // Create a loot cycle from pending corpses.
        // If no corpses recorded, fall back to the last pull target as a single corpse.
        let corpses = if self.pending_corpses.is_empty() {
            if self.last_pull_target.is_empty() {
                Vec::new()
            } else {
                vec![CorpseEntry {
                    spawn_id: 0,
                    mob_name: self.last_pull_target.clone(),
                }]
            }
        } else {
            self.pending_corpses.drain(..).collect()
        };

        self.loot_cycle = Some(LootCycle::new(self.loot_config.clone(), corpses));

        self.state = CampState::Looting {
            started_tick: self.tick,
        };
    }

    fn transition_to_medding(&mut self, commands: &mut Vec<(u32, CampAction)>) {
        // Casters sit to med
        for member in &self.members {
            match member.role {
                Role::Healer | Role::CC | Role::Dps => {
                    commands.push((member.pid, CampAction::Slash("/sit".into())));
                }
                _ => {}
            }
        }

        self.state = CampState::Medding {
            started_tick: self.tick,
        };
    }

    fn transition_to_idle(&mut self, commands: &mut Vec<(u32, CampAction)>) {
        // Everyone stand up
        for member in &self.members {
            commands.push((member.pid, CampAction::Slash("/stand".into())));
        }

        self.state = CampState::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CampConfig {
        CampConfig {
            name: "test_camp".into(),
            zone: "crushbone".into(),
            camp_center: [100.0, 200.0, 0.0],
            pull_point: [150.0, 250.0, 0.0],
            pull_radius: 200.0,
            camp_radius: 30.0,
            leash_radius: 100.0,
            rest_mana_pct: 60,
            pull_mana_pct: 30,
            level_range: [5, 12],
            pull_mob_names: vec!["an orc pawn".into()],
            ignore_mob_names: Vec::new(),
            burn_mob_names: Vec::new(),
            next_camp: None,
            prev_camp: None,
        }
    }

    fn test_members() -> Vec<CampMember> {
        vec![
            CampMember::new(100, "Warrior01".into(), Role::Tank),
            CampMember::new(101, "Cleric01".into(), Role::Healer),
            CampMember::new(102, "Enchanter01".into(), Role::CC),
            CampMember::new(103, "Bard01".into(), Role::Puller),
            CampMember::new(104, "Ranger01".into(), Role::Dps),
            CampMember::new(105, "Ranger02".into(), Role::Dps),
        ]
    }

    #[test]
    fn test_starts_idle() {
        let camp = CampLoop::new(test_config(), test_members());
        assert_eq!(camp.state, CampState::Idle);
        assert_eq!(camp.tick, 0);
    }

    #[test]
    fn test_idle_to_pulling() {
        let mut camp = CampLoop::new(test_config(), test_members());
        let cmds = camp.tick(None);

        assert!(matches!(camp.state, CampState::Pulling { .. }));
        // Puller should get /target and /attack
        let puller_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 103).collect();
        assert_eq!(puller_cmds.len(), 2);
        assert!(puller_cmds[0].1.contains("/target"));
        assert_eq!(puller_cmds[1].1, "/attack");
    }

    #[test]
    fn test_pulling_to_fighting() {
        let mut camp = CampLoop::new(test_config(), test_members());
        camp.tick(None); // Idle -> Pulling

        // Advance through pull duration
        for _ in 0..PULL_DURATION - 1 {
            let cmds = camp.tick(None);
            assert!(cmds.is_empty()); // No commands during wait
            assert!(matches!(camp.state, CampState::Pulling { .. }));
        }

        let cmds = camp.tick(None); // Should transition to Fighting
        assert!(matches!(camp.state, CampState::Fighting { .. }));

        // Tank should assist puller
        let tank_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 100).collect();
        assert!(
            tank_cmds
                .iter()
                .any(|(_, cmd)| cmd.contains("/assist Bard01"))
        );
        assert!(tank_cmds.iter().any(|(_, cmd)| cmd == "/attack"));

        // DPS should assist tank
        let dps_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 104).collect();
        assert!(
            dps_cmds
                .iter()
                .any(|(_, cmd)| cmd.contains("/assist Warrior01"))
        );

        // Healer should target tank
        let healer_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 101).collect();
        assert!(
            healer_cmds
                .iter()
                .any(|(_, cmd)| cmd.contains("/target Warrior01"))
        );
    }

    #[test]
    fn test_fighting_to_looting() {
        let mut camp = CampLoop::new(test_config(), test_members());
        camp.tick(None); // -> Pulling
        for _ in 0..PULL_DURATION {
            camp.tick(None);
        }
        assert!(matches!(camp.state, CampState::Fighting { .. }));

        // Advance through fight duration
        for _ in 0..FIGHT_DURATION - 1 {
            camp.tick(None);
        }

        let cmds = camp.tick(None); // -> Looting
        assert!(matches!(camp.state, CampState::Looting { .. }));

        // Everyone should get /attack off
        for member in test_members() {
            assert!(
                cmds.iter()
                    .any(|(pid, cmd)| *pid == member.pid && cmd == "/attack off")
            );
        }

        // LootCycle should have been created with fallback corpse from last_pull_target
        assert!(camp.loot_cycle.is_some());
    }

    #[test]
    fn test_looting_to_medding() {
        let mut camp = CampLoop::new(test_config(), test_members());
        // Use minimal loot delays for fast test
        camp.loot_config = crate::camp::loot::LootConfig {
            item_pickup_delay: 1,
            target_delay: 1,
            approach_delay: 1,
            loot_open_delay: 1,
            close_delay: 1,
            ..Default::default()
        };
        // Fast-forward to Looting
        camp.tick(None); // -> Pulling
        for _ in 0..PULL_DURATION {
            camp.tick(None);
        }
        for _ in 0..FIGHT_DURATION {
            camp.tick(None);
        }
        assert!(matches!(camp.state, CampState::Looting { .. }));

        // Tick through the loot cycle until it finishes and transitions to Medding
        for _ in 0..20 {
            camp.tick(None);
            if matches!(camp.state, CampState::Medding { .. }) {
                break;
            }
        }

        assert!(
            matches!(camp.state, CampState::Medding { .. }),
            "Should transition to Medding after loot cycle completes"
        );
    }

    #[test]
    fn test_medding_to_idle() {
        let mut camp = CampLoop::new(test_config(), test_members());
        // Use minimal loot delays
        camp.loot_config = crate::camp::loot::LootConfig {
            item_pickup_delay: 1,
            target_delay: 1,
            approach_delay: 1,
            loot_open_delay: 1,
            close_delay: 1,
            ..Default::default()
        };
        // Fast-forward to Medding by ticking through Pull -> Fight -> Loot
        camp.tick(None); // -> Pulling
        for _ in 0..PULL_DURATION {
            camp.tick(None);
        }
        for _ in 0..FIGHT_DURATION {
            camp.tick(None);
        }
        // Tick through loot cycle until we reach Medding
        for _ in 0..20 {
            camp.tick(None);
            if matches!(camp.state, CampState::Medding { .. }) {
                break;
            }
        }
        assert!(matches!(camp.state, CampState::Medding { .. }));

        for _ in 0..MED_DURATION - 1 {
            camp.tick(None);
        }

        let cmds = camp.tick(None); // -> Idle
        assert_eq!(camp.state, CampState::Idle);

        // Everyone should /stand
        for member in test_members() {
            assert!(
                cmds.iter()
                    .any(|(pid, cmd)| *pid == member.pid && cmd == "/stand")
            );
        }
    }

    #[test]
    fn test_full_cycle() {
        let mut camp = CampLoop::new(test_config(), test_members());
        // Use minimal loot delays so the cycle completes quickly
        camp.loot_config = crate::camp::loot::LootConfig {
            item_pickup_delay: 1,
            target_delay: 1,
            approach_delay: 1,
            loot_open_delay: 1,
            close_delay: 1,
            ..Default::default()
        };

        // Run through a complete cycle: Idle -> Pull -> Fight -> Loot -> Med -> Idle
        // Generous upper bound since loot FSM timing depends on personality
        for _ in 0..60 {
            camp.tick(None);
            if camp.state == CampState::Idle && camp.tick > 1 {
                break;
            }
        }

        assert_eq!(camp.state, CampState::Idle);

        // Next tick should start a new pull
        camp.tick(None);
        assert!(matches!(camp.state, CampState::Pulling { .. }));
    }

    #[test]
    fn test_no_puller_falls_back_to_tank() {
        let members = vec![
            CampMember::new(100, "Warrior01".into(), Role::Tank),
            CampMember::new(104, "Ranger01".into(), Role::Dps),
        ];
        let mut camp = CampLoop::new(test_config(), members);
        let cmds = camp.tick(None);

        // Tank should pull when no puller exists
        let tank_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 100).collect();
        assert!(tank_cmds.iter().any(|(_, cmd)| cmd.contains("/target")));
        assert!(tank_cmds.iter().any(|(_, cmd)| cmd == "/attack"));
    }

    #[test]
    fn test_empty_pull_mob_names_uses_default() {
        let mut config = test_config();
        config.pull_mob_names.clear();
        let mut camp = CampLoop::new(config, test_members());
        let cmds = camp.tick(None);

        let target_cmd = cmds
            .iter()
            .find(|(_, cmd)| cmd.contains("/target"))
            .unwrap();
        assert!(target_cmd.1.contains("a_mob"));
    }

    #[test]
    fn test_buffing_to_idle() {
        let mut camp = CampLoop::new(test_config(), test_members());
        camp.state = CampState::Buffing { started_tick: 1 };
        camp.tick = 1;

        for _ in 0..BUFF_DURATION - 1 {
            let cmds = camp.tick(None);
            assert!(cmds.is_empty());
            assert!(matches!(camp.state, CampState::Buffing { .. }));
        }

        let cmds = camp.tick(None); // -> Idle
        assert_eq!(camp.state, CampState::Idle);
        assert!(!cmds.is_empty()); // /stand commands
    }

    // -- Snapshot-driven transition tests --

    #[test]
    fn test_fighting_to_looting_on_target_dead() {
        let mut camp = CampLoop::new(test_config(), test_members());
        camp.tick(None); // -> Pulling
        for _ in 0..PULL_DURATION {
            camp.tick(None);
        }
        assert!(matches!(camp.state, CampState::Fighting { .. }));

        // Target dead should trigger immediate transition (no timer wait)
        let snap = CampSnapshot {
            healer_mana_pct: 80.0,
            tank_hp_pct: 90.0,
            target_hp_pct: Some(0.0),
            target_is_dead: true,
            target_spawn_id: None,
            member_hp: vec![],
        };
        let cmds = camp.tick(Some(&snap));
        assert!(matches!(camp.state, CampState::Looting { .. }));
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/attack off"));
    }

    #[test]
    fn test_medding_to_idle_on_mana_ready() {
        let mut camp = CampLoop::new(test_config(), test_members());
        // Fast-forward to Medding
        camp.state = CampState::Medding { started_tick: 1 };
        camp.tick = 1;

        // Healer mana above pull_mana_pct (30) should transition immediately
        let snap = CampSnapshot {
            healer_mana_pct: 50.0,
            tank_hp_pct: 100.0,
            target_hp_pct: None,
            target_is_dead: false,
            target_spawn_id: None,
            member_hp: vec![],
        };
        let cmds = camp.tick(Some(&snap));
        assert_eq!(camp.state, CampState::Idle);
        assert!(!cmds.is_empty()); // /stand commands
    }

    #[test]
    fn test_idle_blocks_pull_on_low_healer_mana() {
        let mut camp = CampLoop::new(test_config(), test_members());
        assert_eq!(camp.state, CampState::Idle);

        // Healer mana below pull_mana_pct (30) should NOT pull
        let snap = CampSnapshot {
            healer_mana_pct: 10.0,
            tank_hp_pct: 100.0,
            target_hp_pct: None,
            target_is_dead: false,
            target_spawn_id: None,
            member_hp: vec![],
        };
        let cmds = camp.tick(Some(&snap));
        assert_eq!(camp.state, CampState::Idle);
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_fighting_emergency_heal_on_low_tank_hp() {
        let mut camp = CampLoop::new(test_config(), test_members());
        camp.state = CampState::Fighting { started_tick: 1 };
        camp.tick = 1;

        let snap = CampSnapshot {
            healer_mana_pct: 80.0,
            tank_hp_pct: 15.0, // Below 20% threshold
            target_hp_pct: Some(50.0),
            target_is_dead: false,
            target_spawn_id: None,
            member_hp: vec![],
        };
        let cmds = camp.tick(Some(&snap));
        // Healer (pid 101) should get emergency /cast 1
        assert!(
            cmds.iter()
                .any(|(pid, cmd)| *pid == 101 && cmd == "/cast 1")
        );
        // Should still be fighting (target alive, timer not expired)
        assert!(matches!(camp.state, CampState::Fighting { .. }));
    }

    // -- Bug #6: CcExpiring must use exact spawn_id match --

    #[test]
    fn test_cc_expiring_event_no_partial_spawn_id_match() {
        use crate::camp::cc::{CcAbility, CcMember, CcType};

        let mut camp = CampLoop::new(test_config(), test_members());
        camp.tick = 10;

        // Set up two CC targets: spawn_id 10 and spawn_id 100
        let spawns = vec![(10, "orc pawn".into()), (100, "orc centurion".into())];
        camp.cc_tracker.update(&spawns, None, 0);
        camp.cc_tracker.targets[0].cc_applied = Some(CcType::Mez);
        camp.cc_tracker.targets[0].cc_expiry_tick = 12; // About to expire
        camp.cc_tracker.targets[0].assigned_to_pid = Some(200);
        camp.cc_tracker.targets[1].cc_applied = Some(CcType::Mez);
        camp.cc_tracker.targets[1].cc_expiry_tick = 999; // Not expiring — far future
        camp.cc_tracker.targets[1].assigned_to_pid = Some(201);

        camp.cc_members = vec![
            CcMember {
                pid: 200,
                name: "Enc01".into(),
                cc_abilities: vec![CcAbility {
                    cc_type: CcType::Mez,
                    command: "/cast 1".into(),
                    cooldown_ticks: 3,
                    duration_ticks: 18,
                    priority: 2,
                }],
                debuff_abilities: vec![],
                last_cast_tick: 0,
            },
            CcMember {
                pid: 201,
                name: "Enc02".into(),
                cc_abilities: vec![CcAbility {
                    cc_type: CcType::Mez,
                    command: "/cast 1".into(),
                    cooldown_ticks: 3,
                    duration_ticks: 18,
                    priority: 2,
                }],
                debuff_abilities: vec![],
                last_cast_tick: 0,
            },
        ];

        // Push event for spawn_id 10 only
        camp.push_event(CampEvent::CcExpiring { spawn_id: 10 });
        // Use Fighting state so tick() doesn't add pull commands
        camp.state = CampState::Fighting {
            started_tick: camp.tick,
        };

        let snap = CampSnapshot {
            healer_mana_pct: 80.0,
            tank_hp_pct: 90.0,
            target_hp_pct: Some(50.0),
            target_is_dead: false,
            target_spawn_id: None,
            member_hp: vec![],
        };
        let cmds = camp.tick(Some(&snap));

        // Filter to only CC-related /target commands (from CcExpiring event processing)
        let cc_target_cmds: Vec<_> = cmds
            .iter()
            .filter(|(_, cmd)| cmd.starts_with("/target id"))
            .collect();
        for (_, cmd) in &cc_target_cmds {
            assert_eq!(
                *cmd, "/target id 10",
                "Should not partially match spawn_id 100"
            );
        }
    }

    // --- Task 3: Buff rebuffing activation ---

    #[test]
    fn test_medding_to_buffing_when_buffs_needed() {
        use crate::camp::class_config::{ClassAbility, ClassConfig};

        let mut camp = CampLoop::new(test_config(), test_members());
        // Put camp in Medding state
        camp.state = CampState::Medding { started_tick: 0 };
        camp.tick = MED_DURATION; // Past the med timer

        // Configure class configs with a buff ability for the healer
        camp.class_configs.insert(
            "healer".into(),
            ClassConfig {
                class_name: "cleric".into(),
                role: "healer".into(),
                combat_abilities: vec![],
                buff_abilities: vec![ClassAbility {
                    name: "Symbol of Naltron".into(),
                    command: "/cast 4".into(),
                    cooldown_secs: 200.0,
                    priority: 1,
                    condition: None,
                    duration_secs: None,
                }],
                emergency_abilities: vec![],
                cc_abilities: vec![],
                debuff_abilities: vec![],
                rest_command: "/sit".into(),
                twist_interval_secs: None,
            },
        );

        // Tick with no snapshot (timer-based fallback)
        let cmds = camp.tick(None);

        // Should transition to Buffing (buffs are needed — never cast)
        assert!(
            matches!(camp.state, CampState::Buffing { .. }),
            "Should transition to Buffing when buffs are needed, got {:?}",
            camp.state
        );
        // Should have buff commands
        assert!(
            cmds.iter().any(|(_, cmd)| cmd.contains("/cast 4")),
            "Should have buff cast command"
        );
    }

    #[test]
    fn test_medding_to_idle_when_no_buffs_needed() {
        let mut camp = CampLoop::new(test_config(), test_members());
        camp.state = CampState::Medding { started_tick: 0 };
        camp.tick = MED_DURATION;

        // No class configs -> no buffs to check -> go straight to Idle
        let _cmds = camp.tick(None);
        assert_eq!(camp.state, CampState::Idle);
    }

    #[test]
    fn test_medding_to_idle_when_buffs_recently_cast() {
        use crate::camp::class_config::{ClassAbility, ClassConfig};

        let mut camp = CampLoop::new(test_config(), test_members());
        camp.state = CampState::Medding { started_tick: 0 };
        camp.tick = MED_DURATION;

        camp.class_configs.insert(
            "healer".into(),
            ClassConfig {
                class_name: "cleric".into(),
                role: "healer".into(),
                combat_abilities: vec![],
                buff_abilities: vec![ClassAbility {
                    name: "Symbol of Naltron".into(),
                    command: "/cast 4".into(),
                    cooldown_secs: 200.0,
                    priority: 1,
                    condition: None,
                    duration_secs: None,
                }],
                emergency_abilities: vec![],
                cc_abilities: vec![],
                debuff_abilities: vec![],
                rest_command: "/sit".into(),
                twist_interval_secs: None,
            },
        );

        // Record all buffs as recently cast for all members
        for member in &camp.members {
            camp.buff_tracker
                .record_cast(member.pid, "Symbol of Naltron", camp.tick);
        }

        let _cmds = camp.tick(None);
        assert_eq!(
            camp.state,
            CampState::Idle,
            "Should go straight to Idle when all buffs are fresh"
        );
    }

    #[test]
    fn test_buffing_to_idle_after_timer() {
        let mut camp = CampLoop::new(test_config(), test_members());
        camp.state = CampState::Buffing { started_tick: 0 };
        camp.tick = BUFF_DURATION;

        let cmds = camp.tick(None);
        assert_eq!(camp.state, CampState::Idle);
        // Should have /stand commands
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/stand"));
    }
}
