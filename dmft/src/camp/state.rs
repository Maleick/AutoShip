//! Camp loop state machine — drives the pull/fight/loot/med cycle.

use crate::camp::cc::{CcMember, CcTracker};
use crate::camp::config::CampConfig;
use crate::camp::personality::PersonalityProfile;

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
    DPS,
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
const PULL_DURATION: u64 = 5;
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
}

impl CampLoop {
    pub fn new(config: CampConfig, members: Vec<CampMember>) -> Self {
        Self {
            config,
            state: CampState::Idle,
            members,
            tick: 0,
            last_pull_target: String::new(),
            cc_tracker: CcTracker::new(),
            cc_members: Vec::new(),
            pending_events: Vec::new(),
        }
    }

    /// Push an event to be processed on the next tick.
    pub fn push_event(&mut self, event: CampEvent) {
        self.pending_events.push(event);
    }

    /// Process all pending events, returning commands. Called at the start of tick().
    fn process_events(&mut self) -> Vec<(u32, String)> {
        let mut commands = Vec::new();
        let events: Vec<CampEvent> = self.pending_events.drain(..).collect();

        for event in events {
            commands.extend(self.process_event(event));
        }
        commands
    }

    /// Handle a single camp event.
    fn process_event(&mut self, event: CampEvent) -> Vec<(u32, String)> {
        match event {
            CampEvent::CharmBreak { spawn_id } => {
                self.cc_tracker
                    .charm_break_response(spawn_id, &self.cc_members, self.tick)
            }
            CampEvent::AddSpawned { spawn_id, name } => {
                // Update tracker with the new add, then assign CC
                let spawns = vec![(spawn_id, name)];
                self.cc_tracker.update(&spawns, None, self.tick);
                // Debuff first, then CC
                let mut cmds = self.cc_tracker.debuff_commands(
                    spawn_id,
                    &self.cc_members,
                    self.tick,
                );
                cmds.extend(self.cc_tracker.assign_cc(&mut self.cc_members, self.tick));
                cmds
            }
            CampEvent::CcExpiring { spawn_id } => {
                // Find the assigned member and re-CC (exact spawn_id match to avoid partial ID hits)
                let all_cmds = self.cc_tracker.needs_remez(self.tick, 0, &mut self.cc_members);
                let target_cmd = format!("/target id {spawn_id}");
                let mut result = Vec::new();
                let mut matched = false;
                for (pid, cmd) in all_cmds {
                    if cmd == target_cmd {
                        matched = true;
                        result.push((pid, cmd));
                    } else if matched && cmd.starts_with("/cast") {
                        result.push((pid, cmd));
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
    pub fn tick(&mut self, snapshot: Option<&CampSnapshot>) -> Vec<(u32, String)> {
        self.tick += 1;
        let mut commands = Vec::new();

        // Process pending events first (charm breaks, adds, etc.)
        commands.extend(self.process_events());

        // Check for CCs about to expire during fighting
        if matches!(self.state, CampState::Fighting { .. }) && !self.cc_members.is_empty() {
            let remez = self
                .cc_tracker
                .needs_remez(self.tick, CC_REMEZ_BUFFER, &mut self.cc_members);
            commands.extend(remez);
        }

        match self.state.clone() {
            CampState::Idle => {
                // Only pull if healer has enough mana (when we know).
                // Apply healer's personality jitter to the threshold.
                let pull_threshold = self
                    .find_by_role(&Role::Healer)
                    .map(|h| h.personality.adjust_mana_threshold(self.config.pull_mana_pct as f32))
                    .unwrap_or(self.config.pull_mana_pct as f32);
                let healer_ready = snapshot
                    .map(|s| s.healer_mana_pct >= pull_threshold)
                    .unwrap_or(true);
                if healer_ready {
                    self.transition_to_pulling(&mut commands);
                }
            }
            CampState::Pulling { started_tick } => {
                if self.tick - started_tick >= PULL_DURATION {
                    self.transition_to_fighting(&mut commands);
                }
            }
            CampState::Fighting { started_tick } => {
                // Emergency heal if tank HP < 20%
                if let Some(snap) = snapshot {
                    if snap.tank_hp_pct < 20.0 {
                        if let Some(healer) = self.find_by_role(&Role::Healer) {
                            commands.push((healer.pid, "/cast 1".into()));
                        }
                    }
                }

                // Melee characters /face periodically, staggered by personality
                let fight_elapsed = self.tick - started_tick;
                if fight_elapsed > 0 {
                    for member in &self.members {
                        if member.role != Role::Tank && member.role != Role::DPS {
                            continue;
                        }
                        // Each member's clock starts after their phase_offset
                        let personal_elapsed =
                            fight_elapsed.saturating_sub(member.personality.phase_offset);
                        let face_interval = member.personality.adjust_delay(5);
                        if personal_elapsed > 0 && personal_elapsed % face_interval == 0 {
                            commands.push((member.pid, "/face".into()));
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
            CampState::Looting { started_tick } => {
                if self.tick - started_tick >= LOOT_DURATION {
                    self.transition_to_medding(&mut commands);
                }
            }
            CampState::Medding { started_tick } => {
                // Transition when healer mana is above pull threshold (real data) or timer (fallback).
                // Apply healer's personality jitter to the threshold.
                let med_threshold = self
                    .find_by_role(&Role::Healer)
                    .map(|h| h.personality.adjust_mana_threshold(self.config.pull_mana_pct as f32))
                    .unwrap_or(self.config.pull_mana_pct as f32);
                let mana_ready = snapshot
                    .map(|s| s.healer_mana_pct >= med_threshold)
                    .unwrap_or(false);
                let timer_expired = self.tick - started_tick >= MED_DURATION;
                if mana_ready || timer_expired {
                    self.transition_to_idle(&mut commands);
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

    fn transition_to_pulling(&mut self, commands: &mut Vec<(u32, String)>) {
        let target = self.pick_pull_target();
        self.last_pull_target = target.clone();

        // Puller targets and attacks
        if let Some(puller) = self.find_by_role(&Role::Puller) {
            commands.push((puller.pid, format!("/target {target}")));
            commands.push((puller.pid, "/attack".into()));
        } else if let Some(tank) = self.find_by_role(&Role::Tank) {
            // Fall back to tank as puller
            commands.push((tank.pid, format!("/target {target}")));
            commands.push((tank.pid, "/attack".into()));
        }

        self.state = CampState::Pulling {
            started_tick: self.tick,
        };
    }

    fn transition_to_fighting(&mut self, commands: &mut Vec<(u32, String)>) {
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
                commands.push((tank.pid, format!("/assist {puller_name}")));
            }
            commands.push((tank.pid, "/attack".into()));
        }

        // DPS assists tank and attacks
        let assist_name = if !tank_name.is_empty() {
            &tank_name
        } else {
            &puller_name
        };

        for dps in self.find_all_by_role(&Role::DPS) {
            if !assist_name.is_empty() {
                commands.push((dps.pid, format!("/assist {assist_name}")));
            }
            commands.push((dps.pid, "/attack".into()));
        }

        // Healer targets tank
        if let Some(healer) = self.find_by_role(&Role::Healer) {
            if !tank_name.is_empty() {
                commands.push((healer.pid, format!("/target {tank_name}")));
            }
        }

        self.state = CampState::Fighting {
            started_tick: self.tick,
        };
    }

    fn transition_to_looting(&mut self, commands: &mut Vec<(u32, String)>) {
        // Everyone stops attacking
        for member in &self.members {
            commands.push((member.pid, "/attack off".into()));
        }

        // First available member loots (prefer DPS so tank holds position)
        if let Some(looter) = self
            .find_all_by_role(&Role::DPS)
            .first()
            .or(self.members.first().as_ref())
        {
            commands.push((looter.pid, "/loot".into()));
        }

        self.state = CampState::Looting {
            started_tick: self.tick,
        };
    }

    fn transition_to_medding(&mut self, commands: &mut Vec<(u32, String)>) {
        // Casters sit to med
        for member in &self.members {
            match member.role {
                Role::Healer | Role::CC | Role::DPS => {
                    commands.push((member.pid, "/sit".into()));
                }
                _ => {}
            }
        }

        self.state = CampState::Medding {
            started_tick: self.tick,
        };
    }

    fn transition_to_idle(&mut self, commands: &mut Vec<(u32, String)>) {
        // Everyone stand up
        for member in &self.members {
            commands.push((member.pid, "/stand".into()));
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
        }
    }

    fn test_members() -> Vec<CampMember> {
        vec![
            CampMember::new(100, "Warrior01".into(), Role::Tank),
            CampMember::new(101, "Cleric01".into(), Role::Healer),
            CampMember::new(102, "Enchanter01".into(), Role::CC),
            CampMember::new(103, "Bard01".into(), Role::Puller),
            CampMember::new(104, "Ranger01".into(), Role::DPS),
            CampMember::new(105, "Ranger02".into(), Role::DPS),
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
        assert!(tank_cmds.iter().any(|(_, cmd)| cmd.contains("/assist Bard01")));
        assert!(tank_cmds.iter().any(|(_, cmd)| cmd == "/attack"));

        // DPS should assist tank
        let dps_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 104).collect();
        assert!(dps_cmds.iter().any(|(_, cmd)| cmd.contains("/assist Warrior01")));

        // Healer should target tank
        let healer_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 101).collect();
        assert!(healer_cmds.iter().any(|(_, cmd)| cmd.contains("/target Warrior01")));
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
            assert!(cmds.iter().any(|(pid, cmd)| *pid == member.pid && cmd == "/attack off"));
        }

        // Someone should /loot
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/loot"));
    }

    #[test]
    fn test_looting_to_medding() {
        let mut camp = CampLoop::new(test_config(), test_members());
        // Fast-forward to Looting
        camp.tick(None); // -> Pulling
        for _ in 0..PULL_DURATION {
            camp.tick(None);
        }
        for _ in 0..FIGHT_DURATION {
            camp.tick(None);
        }
        assert!(matches!(camp.state, CampState::Looting { .. }));

        for _ in 0..LOOT_DURATION - 1 {
            camp.tick(None);
        }

        let cmds = camp.tick(None); // -> Medding
        assert!(matches!(camp.state, CampState::Medding { .. }));

        // Casters should /sit
        let healer_sit = cmds.iter().any(|(pid, cmd)| *pid == 101 && cmd == "/sit");
        let cc_sit = cmds.iter().any(|(pid, cmd)| *pid == 102 && cmd == "/sit");
        assert!(healer_sit, "Healer should /sit");
        assert!(cc_sit, "CC should /sit");

        // Tank should NOT /sit
        let tank_sit = cmds.iter().any(|(pid, cmd)| *pid == 100 && cmd == "/sit");
        assert!(!tank_sit, "Tank should not /sit");
    }

    #[test]
    fn test_medding_to_idle() {
        let mut camp = CampLoop::new(test_config(), test_members());
        // Fast-forward to Medding
        camp.tick(None); // -> Pulling
        for _ in 0..PULL_DURATION {
            camp.tick(None);
        }
        for _ in 0..FIGHT_DURATION {
            camp.tick(None);
        }
        for _ in 0..LOOT_DURATION {
            camp.tick(None);
        }
        assert!(matches!(camp.state, CampState::Medding { .. }));

        for _ in 0..MED_DURATION - 1 {
            camp.tick(None);
        }

        let cmds = camp.tick(None); // -> Idle
        assert_eq!(camp.state, CampState::Idle);

        // Everyone should /stand
        for member in test_members() {
            assert!(cmds.iter().any(|(pid, cmd)| *pid == member.pid && cmd == "/stand"));
        }
    }

    #[test]
    fn test_full_cycle() {
        let mut camp = CampLoop::new(test_config(), test_members());

        // Run through a complete cycle: Idle -> Pull -> Fight -> Loot -> Med -> Idle
        let total_ticks = 1 + PULL_DURATION + FIGHT_DURATION + LOOT_DURATION + MED_DURATION;
        for _ in 0..total_ticks {
            camp.tick(None);
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
            CampMember::new(104, "Ranger01".into(), Role::DPS),
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

        let target_cmd = cmds.iter().find(|(_, cmd)| cmd.contains("/target")).unwrap();
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
        };
        let cmds = camp.tick(Some(&snap));
        // Healer (pid 101) should get emergency /cast 1
        assert!(cmds.iter().any(|(pid, cmd)| *pid == 101 && cmd == "/cast 1"));
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
        camp.state = CampState::Fighting { started_tick: camp.tick };

        let snap = CampSnapshot {
            healer_mana_pct: 80.0,
            tank_hp_pct: 90.0,
            target_hp_pct: Some(50.0),
            target_is_dead: false,
        };
        let cmds = camp.tick(Some(&snap));

        // Filter to only CC-related /target commands (from CcExpiring event processing)
        let cc_target_cmds: Vec<_> = cmds
            .iter()
            .filter(|(_, cmd)| cmd.starts_with("/target id"))
            .collect();
        for (_, cmd) in &cc_target_cmds {
            assert_eq!(*cmd, "/target id 10", "Should not partially match spawn_id 100");
        }
    }
}
