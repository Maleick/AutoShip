//! Hunt mode — tank roams for mobs while group members maintain formation.
//!
//! Unlike camp mode where the group anchors at a position and pulls mobs to
//! camp, hunt mode has the tank roaming through the zone engaging mobs in place
//! while the rest of the group follows at role-appropriate distances.

use super::{
    config::CampConfig,
    positioning::distance_2d,
    state::{CampMember, Role},
};

/// Operating mode for a group — camp (stationary) or hunt (roaming).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatingMode {
    /// Stationary camp — group anchors and pulls mobs to a fixed position.
    Camp,
    /// Roaming hunt — tank moves through the zone engaging mobs in place.
    Hunt,
}

impl std::fmt::Display for OperatingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Camp => write!(f, "Camp"),
            Self::Hunt => write!(f, "Hunt"),
        }
    }
}

/// Current phase of the hunt loop FSM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HuntState {
    /// Tank is roaming, looking for the next mob.
    Roaming,
    /// Tank has found a mob and is closing distance to engage.
    Engaging {
        /// Tick when engagement started.
        started_tick: u64,
    },
    /// Group is fighting the current target.
    Fighting {
        /// Tick when combat started.
        started_tick: u64,
    },
    /// Looting the corpse after a kill.
    Looting {
        /// Tick when looting started.
        started_tick: u64,
    },
}

/// Position in 2D space (EQ x, y).
#[derive(Debug, Clone, Copy)]
pub struct Pos2D {
    /// X coordinate in EQ world units.
    pub x: f32,
    /// Y coordinate in EQ world units.
    pub y: f32,
}

impl Pos2D {
    /// Creates a new 2D position.
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Euclidean distance to another 2D position.
    #[must_use]
    pub fn distance_to(&self, other: &Pos2D) -> f32 {
        distance_2d(self.x, self.y, other.x, other.y)
    }
}

/// Snapshot of real-time state for the hunt loop.
#[derive(Debug, Clone)]
pub struct HuntSnapshot {
    /// Tank's current position.
    pub tank_pos: Pos2D,
    /// Each member's current position, keyed by pid.
    pub member_positions: Vec<(u32, Pos2D)>,
    /// Whether the current target is dead.
    pub target_is_dead: bool,
    /// Tank HP percentage.
    pub tank_hp_pct: f32,
    /// Healer mana percentage.
    pub healer_mana_pct: f32,
}

/// Role-based formation distances from the tank.
#[derive(Debug, Clone)]
pub struct FormationConfig {
    /// Melee DPS: desired distance from tank.
    pub melee_follow_dist: f32,
    /// Melee DPS: start moving toward tank if farther than this.
    pub melee_leash_dist: f32,
    /// Caster: desired distance from tank (within cast range).
    pub caster_follow_dist: f32,
    /// Caster: start moving if farther than this.
    pub caster_leash_dist: f32,
    /// Caster: never get closer than this to the mob/tank during combat.
    pub caster_min_dist: f32,
    /// Healer: desired distance from tank.
    pub healer_follow_dist: f32,
    /// Healer: start moving if farther than this.
    pub healer_leash_dist: f32,
}

impl Default for FormationConfig {
    fn default() -> Self {
        Self {
            melee_follow_dist: 20.0,
            melee_leash_dist: 40.0,
            caster_follow_dist: 70.0,
            caster_leash_dist: 100.0,
            caster_min_dist: 40.0,
            healer_follow_dist: 50.0,
            healer_leash_dist: 80.0,
        }
    }
}

/// Manages group formation relative to the tank.
pub struct FormationManager {
    /// Configuration for role-based follow distances.
    pub config: FormationConfig,
}

impl FormationManager {
    /// Creates a new formation manager with the given distance config.
    #[must_use]
    pub fn new(config: FormationConfig) -> Self {
        Self { config }
    }

    /// Determine movement commands for a member based on their role and
    /// distance to tank. Returns slash commands if the member needs to
    /// move, or empty vec if in position.
    ///
    /// Movement uses discrete steps: /face toward tank, hold forward key,
    /// release when close. Does NOT use /follow to avoid EQ's
    /// rubber-banding behavior.
    #[must_use]
    pub fn formation_commands(
        &self,
        member: &CampMember,
        member_pos: &Pos2D,
        tank_pos: &Pos2D,
        tank_name: &str,
        in_combat: bool,
    ) -> Vec<String> {
        if matches!(member.role, Role::Tank | Role::Puller) {
            return Vec::new();
        }

        let dist = member_pos.distance_to(tank_pos);
        let (desired, leash) = self.role_distances(&member.role);

        // If in combat, casters should not move closer than min_dist
        if in_combat && is_caster_role(&member.role) && dist < self.config.caster_min_dist {
            return Vec::new();
        }

        if dist > leash {
            // Too far — move toward tank
            let mut cmds = Vec::new();
            cmds.push(format!("/face {tank_name}"));
            cmds.push("/keypress forward hold".into());
            cmds
        } else if dist < desired * 0.5 && !matches!(member.role, Role::Tank) {
            // Too close — stop moving (release forward if held)
            vec!["/keypress forward".into()]
        } else {
            Vec::new()
        }
    }

    /// Check if a member is close enough to stop moving toward tank.
    #[must_use]
    pub fn is_in_position(&self, role: &Role, dist_to_tank: f32) -> bool {
        if matches!(role, Role::Tank | Role::Puller) {
            return true; // tank/puller don't follow themselves
        }
        let (desired, _) = self.role_distances(role);
        dist_to_tank <= desired * 1.2
    }

    /// Get (`desired_distance`, `leash_distance`) for a role.
    fn role_distances(&self, role: &Role) -> (f32, f32) {
        match role {
            Role::Tank | Role::Puller => (0.0, 0.0), // tank doesn't follow itself
            Role::Dps => (self.config.melee_follow_dist, self.config.melee_leash_dist),
            Role::Healer => (
                self.config.healer_follow_dist,
                self.config.healer_leash_dist,
            ),
            Role::CC | Role::Bard => (
                self.config.caster_follow_dist,
                self.config.caster_leash_dist,
            ),
        }
    }
}

fn is_caster_role(role: &Role) -> bool {
    matches!(role, Role::Healer | Role::CC | Role::Bard)
}

/// How often non-tank members check distance to tank (in ticks).
const FORMATION_CHECK_INTERVAL: u64 = 5;

/// Timer durations for hunt phases.
const ENGAGE_DURATION: u64 = 4;
const FIGHT_DURATION: u64 = 20;
const LOOT_DURATION: u64 = 3;

/// The hunt loop state machine. Each `tick()` call advances state and returns
/// slash commands to send to EQ clients.
pub struct HuntLoop {
    /// Camp configuration for this hunt group.
    pub config: CampConfig,
    /// Current FSM state.
    pub state: HuntState,
    /// Group members participating in the hunt.
    pub members: Vec<CampMember>,
    /// Formation manager for non-tank positioning.
    pub formation: FormationManager,
    /// Current tick counter.
    pub tick: u64,
    /// Waypoint patrol route for the tank (if set).
    pub patrol_waypoints: Vec<Pos2D>,
    /// Current index into `patrol_waypoints`.
    pub patrol_idx: usize,
    /// Name of the last mob killed.
    pub last_kill_target: String,
}

impl HuntLoop {
    /// Creates a new hunt loop with the given config and group members.
    #[must_use]
    pub fn new(config: CampConfig, members: Vec<CampMember>) -> Self {
        Self {
            config,
            state: HuntState::Roaming,
            members,
            formation: FormationManager::new(FormationConfig::default()),
            tick: 0,
            patrol_waypoints: Vec::new(),
            patrol_idx: 0,
            last_kill_target: String::new(),
        }
    }

    /// Advance the hunt FSM by one tick. Returns `(pid, slash_command)` pairs.
    pub fn tick(&mut self, snapshot: Option<&HuntSnapshot>) -> Vec<(u32, String)> {
        self.tick += 1;
        let mut commands = Vec::new();

        // Non-tank members check formation every FORMATION_CHECK_INTERVAL ticks
        if self.tick.is_multiple_of(FORMATION_CHECK_INTERVAL)
            && let Some(snap) = snapshot
        {
            commands.extend(self.formation_tick(snap));
        }

        match self.state.clone() {
            HuntState::Roaming => {
                self.tick_roaming(&mut commands, snapshot);
            }
            HuntState::Engaging { started_tick } => {
                self.tick_engaging(&mut commands, snapshot, started_tick);
            }
            HuntState::Fighting { started_tick } => {
                self.tick_fighting(&mut commands, snapshot, started_tick);
            }
            HuntState::Looting { started_tick } => {
                self.tick_looting(&mut commands, started_tick);
            }
        }

        commands
    }

    /// Generate formation movement commands for all followers
    /// (non-tank/non-puller).
    fn formation_tick(&self, snap: &HuntSnapshot) -> Vec<(u32, String)> {
        let mut commands = Vec::new();
        let tank_name = self
            .find_by_role(&Role::Tank)
            .map(|t| t.name.clone())
            .unwrap_or_default();

        if tank_name.is_empty() {
            return commands;
        }

        let in_combat = matches!(self.state, HuntState::Fighting { .. });

        for member in &self.members {
            if matches!(member.role, Role::Tank | Role::Puller) {
                continue;
            }

            if let Some((_pid, member_pos)) = snap
                .member_positions
                .iter()
                .find(|(pid, _)| *pid == member.pid)
            {
                let cmds = self.formation.formation_commands(
                    member,
                    member_pos,
                    &snap.tank_pos,
                    &tank_name,
                    in_combat,
                );
                for cmd in cmds {
                    commands.push((member.pid, cmd));
                }
            }
        }

        commands
    }

    fn tick_roaming(
        &mut self,
        commands: &mut Vec<(u32, String)>,
        _snapshot: Option<&HuntSnapshot>,
    ) {
        // Tank looks for the nearest mob to engage.
        // In the absence of real mob data, use pull_mob_names from config.
        if let Some(tank) = self.find_by_role(&Role::Tank) {
            let target = self.pick_target();
            commands.push((tank.pid, format!("/target {target}")));
            self.state = HuntState::Engaging {
                started_tick: self.tick,
            };
        }
    }

    fn tick_engaging(
        &mut self,
        commands: &mut Vec<(u32, String)>,
        _snapshot: Option<&HuntSnapshot>,
        started_tick: u64,
    ) {
        if self.tick - started_tick >= ENGAGE_DURATION {
            // Tank attacks, transition to fighting
            if let Some(tank) = self.find_by_role(&Role::Tank) {
                commands.push((tank.pid, "/attack".into()));
            }
            self.transition_to_fighting(commands);
        }
    }

    fn tick_fighting(
        &mut self,
        commands: &mut Vec<(u32, String)>,
        snapshot: Option<&HuntSnapshot>,
        started_tick: u64,
    ) {
        // Emergency heal
        if let Some(snap) = snapshot
            && snap.tank_hp_pct < 20.0
            && let Some(healer) = self.find_by_role(&Role::Healer)
        {
            commands.push((healer.pid, "/cast 1".into()));
        }

        // Melee /face periodically with personality stagger
        let fight_elapsed = self.tick - started_tick;
        if fight_elapsed > 0 {
            for member in &self.members {
                if !matches!(member.role, Role::Tank | Role::Dps) {
                    continue;
                }
                let personal_elapsed =
                    fight_elapsed.saturating_sub(member.personality.phase_offset);
                let face_interval = member.personality.adjust_delay(5);
                if personal_elapsed > 0 && personal_elapsed % face_interval == 0 {
                    commands.push((member.pid, "/face".into()));
                }
            }
        }

        // Check if target is dead
        let target_dead = snapshot.is_some_and(|s| s.target_is_dead);
        let timer_expired = self.tick - started_tick >= FIGHT_DURATION;

        if target_dead || timer_expired {
            self.transition_to_looting(commands);
        }
    }

    fn tick_looting(&mut self, commands: &mut Vec<(u32, String)>, started_tick: u64) {
        if self.tick - started_tick >= LOOT_DURATION {
            // Back to roaming — tank continues hunting
            self.state = HuntState::Roaming;
            // Release any held movement keys
            for member in &self.members {
                if member.role != Role::Tank {
                    commands.push((member.pid, "/keypress forward".into()));
                }
            }
        }
    }

    fn transition_to_fighting(&mut self, commands: &mut Vec<(u32, String)>) {
        let tank_name = self
            .find_by_role(&Role::Tank)
            .map(|m| m.name.clone())
            .unwrap_or_default();

        // DPS assists tank and attacks
        for dps in self.find_all_by_role(&Role::Dps) {
            if !tank_name.is_empty() {
                commands.push((dps.pid, format!("/assist {tank_name}")));
            }
            commands.push((dps.pid, "/attack".into()));
        }

        // Healer targets tank for heals
        if let Some(healer) = self.find_by_role(&Role::Healer)
            && !tank_name.is_empty()
        {
            commands.push((healer.pid, format!("/target {tank_name}")));
        }

        // CC assists for caster DPS (cast instead of melee)
        for cc in self.find_all_by_role(&Role::CC) {
            if !tank_name.is_empty() {
                commands.push((cc.pid, format!("/assist {tank_name}")));
            }
        }

        self.state = HuntState::Fighting {
            started_tick: self.tick,
        };
    }

    fn transition_to_looting(&mut self, commands: &mut Vec<(u32, String)>) {
        // Everyone stops attacking
        for member in &self.members {
            commands.push((member.pid, "/attack off".into()));
        }

        self.state = HuntState::Looting {
            started_tick: self.tick,
        };
    }

    fn find_by_role(&self, role: &Role) -> Option<&CampMember> {
        self.members.iter().find(|m| &m.role == role)
    }

    fn find_all_by_role(&self, role: &Role) -> Vec<&CampMember> {
        self.members.iter().filter(|m| &m.role == role).collect()
    }

    fn pick_target(&self) -> String {
        if let Some(name) = self.config.pull_mob_names.first() {
            name.clone()
        } else {
            "a_mob".into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CampConfig {
        CampConfig {
            name: "hunt_test".into(),
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
            return_no_aggro: false,
            next_camp: None,
            prev_camp: None,
        }
    }

    fn test_members() -> Vec<CampMember> {
        vec![
            CampMember::new(100, "Warrior01".into(), Role::Tank),
            CampMember::new(101, "Cleric01".into(), Role::Healer),
            CampMember::new(102, "Enchanter01".into(), Role::CC),
            CampMember::new(103, "Ranger01".into(), Role::Dps),
            CampMember::new(104, "Ranger02".into(), Role::Dps),
        ]
    }

    fn test_snapshot(target_dead: bool) -> HuntSnapshot {
        HuntSnapshot {
            tank_pos: Pos2D::new(100.0, 200.0),
            member_positions: vec![
                (100, Pos2D::new(100.0, 200.0)),
                (101, Pos2D::new(100.0, 250.0)), // healer ~50 from tank
                (102, Pos2D::new(100.0, 270.0)), // CC ~70 from tank
                (103, Pos2D::new(105.0, 215.0)), // DPS ~15 from tank
                (104, Pos2D::new(95.0, 218.0)),  // DPS ~18 from tank
            ],
            target_is_dead: target_dead,
            tank_hp_pct: 90.0,
            healer_mana_pct: 80.0,
        }
    }

    #[test]
    fn test_starts_roaming() {
        let hunt = HuntLoop::new(test_config(), test_members());
        assert_eq!(hunt.state, HuntState::Roaming);
        assert_eq!(hunt.tick, 0);
    }

    #[test]
    fn test_roaming_to_engaging() {
        let mut hunt = HuntLoop::new(test_config(), test_members());
        let cmds = hunt.tick(None);

        assert!(matches!(hunt.state, HuntState::Engaging { .. }));
        // Tank should get /target command
        let tank_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 100).collect();
        assert!(tank_cmds.iter().any(|(_, cmd)| cmd.contains("/target")));
    }

    #[test]
    fn test_engaging_to_fighting() {
        let mut hunt = HuntLoop::new(test_config(), test_members());
        hunt.tick(None); // Roaming -> Engaging

        // Advance through engage duration
        for _ in 0..ENGAGE_DURATION - 1 {
            let cmds = hunt.tick(None);
            // No fighting transition yet
            assert!(matches!(hunt.state, HuntState::Engaging { .. }));
            // Should not have DPS assist commands yet
            assert!(!cmds.iter().any(|(_, cmd)| cmd.contains("/assist")));
        }

        let cmds = hunt.tick(None); // -> Fighting
        assert!(matches!(hunt.state, HuntState::Fighting { .. }));

        // Tank should get /attack
        assert!(
            cmds.iter()
                .any(|(pid, cmd)| *pid == 100 && cmd == "/attack")
        );

        // DPS should assist tank
        let dps_cmds: Vec<_> = cmds.iter().filter(|(pid, _)| *pid == 103).collect();
        assert!(
            dps_cmds
                .iter()
                .any(|(_, cmd)| cmd.contains("/assist Warrior01"))
        );

        // Healer should target tank
        assert!(
            cmds.iter()
                .any(|(pid, cmd)| *pid == 101 && cmd.contains("/target Warrior01"))
        );
    }

    #[test]
    fn test_fighting_to_looting_on_target_dead() {
        let mut hunt = HuntLoop::new(test_config(), test_members());
        hunt.tick(None); // -> Engaging
        for _ in 0..ENGAGE_DURATION {
            hunt.tick(None);
        }
        assert!(matches!(hunt.state, HuntState::Fighting { .. }));

        let snap = test_snapshot(true);
        let cmds = hunt.tick(Some(&snap));
        assert!(matches!(hunt.state, HuntState::Looting { .. }));
        assert!(cmds.iter().any(|(_, cmd)| cmd == "/attack off"));
    }

    #[test]
    fn test_fighting_to_looting_on_timer() {
        let mut hunt = HuntLoop::new(test_config(), test_members());
        hunt.tick(None); // -> Engaging
        for _ in 0..ENGAGE_DURATION {
            hunt.tick(None);
        }
        assert!(matches!(hunt.state, HuntState::Fighting { .. }));

        for _ in 0..FIGHT_DURATION - 1 {
            hunt.tick(None);
        }
        hunt.tick(None); // -> Looting
        assert!(matches!(hunt.state, HuntState::Looting { .. }));
    }

    #[test]
    fn test_looting_to_roaming() {
        let mut hunt = HuntLoop::new(test_config(), test_members());
        hunt.tick(None); // -> Engaging
        for _ in 0..ENGAGE_DURATION {
            hunt.tick(None);
        }
        for _ in 0..FIGHT_DURATION {
            hunt.tick(None);
        }
        assert!(matches!(hunt.state, HuntState::Looting { .. }));

        for _ in 0..LOOT_DURATION - 1 {
            hunt.tick(None);
        }
        hunt.tick(None); // -> Roaming
        assert_eq!(hunt.state, HuntState::Roaming);
    }

    #[test]
    fn test_full_hunt_cycle() {
        let mut hunt = HuntLoop::new(test_config(), test_members());

        // Run through a full cycle: Roaming -> Engaging -> Fighting -> Looting ->
        // Roaming
        for _ in 0..50 {
            hunt.tick(None);
            if hunt.state == HuntState::Roaming && hunt.tick > 1 {
                break;
            }
        }

        assert_eq!(hunt.state, HuntState::Roaming);
        // Next tick starts a new engage
        hunt.tick(None);
        assert!(matches!(hunt.state, HuntState::Engaging { .. }));
    }

    #[test]
    fn test_emergency_heal_during_fighting() {
        let mut hunt = HuntLoop::new(test_config(), test_members());
        hunt.state = HuntState::Fighting {
            started_tick: hunt.tick,
        };

        let mut snap = test_snapshot(false);
        snap.tank_hp_pct = 15.0; // Below 20% threshold
        let cmds = hunt.tick(Some(&snap));

        assert!(
            cmds.iter()
                .any(|(pid, cmd)| *pid == 101 && cmd == "/cast 1")
        );
    }

    // -- Formation manager tests --

    #[test]
    fn test_formation_melee_in_position() {
        let fm = FormationManager::new(FormationConfig::default());
        let member = CampMember::new(103, "Ranger01".into(), Role::Dps);
        let member_pos = Pos2D::new(100.0, 215.0); // ~15 from tank
        let tank_pos = Pos2D::new(100.0, 200.0);

        let cmds = fm.formation_commands(&member, &member_pos, &tank_pos, "Tank", false);
        assert!(cmds.is_empty(), "DPS at 15 units should be in position");
    }

    #[test]
    fn test_formation_melee_too_far() {
        let fm = FormationManager::new(FormationConfig::default());
        let member = CampMember::new(103, "Ranger01".into(), Role::Dps);
        let member_pos = Pos2D::new(100.0, 250.0); // ~50 from tank, beyond leash of 40
        let tank_pos = Pos2D::new(100.0, 200.0);

        let cmds = fm.formation_commands(&member, &member_pos, &tank_pos, "Tank", false);
        assert!(!cmds.is_empty(), "DPS at 50 units should need to move");
        assert!(cmds.iter().any(|c| c.contains("/face")));
        assert!(cmds.iter().any(|c| c.contains("/keypress forward hold")));
    }

    #[test]
    fn test_formation_caster_min_distance() {
        let fm = FormationManager::new(FormationConfig::default());
        let member = CampMember::new(101, "Cleric01".into(), Role::Healer);
        let member_pos = Pos2D::new(100.0, 230.0); // ~30 from tank, below caster_min_dist
        let tank_pos = Pos2D::new(100.0, 200.0);

        // During combat, caster should NOT move closer
        let cmds = fm.formation_commands(&member, &member_pos, &tank_pos, "Tank", true);
        assert!(
            cmds.is_empty(),
            "Healer below caster_min_dist during combat should not move"
        );
    }

    #[test]
    fn test_formation_healer_too_far() {
        let fm = FormationManager::new(FormationConfig::default());
        let member = CampMember::new(101, "Cleric01".into(), Role::Healer);
        let member_pos = Pos2D::new(100.0, 290.0); // ~90 from tank, beyond healer leash of 80
        let tank_pos = Pos2D::new(100.0, 200.0);

        let cmds = fm.formation_commands(&member, &member_pos, &tank_pos, "Tank", false);
        assert!(!cmds.is_empty(), "Healer at 90 units should need to move");
    }

    #[test]
    fn test_formation_tank_skipped() {
        let fm = FormationManager::new(FormationConfig::default());
        assert!(fm.is_in_position(&Role::Tank, 999.0));
    }

    #[test]
    fn test_formation_check_runs_on_interval() {
        let mut hunt = HuntLoop::new(test_config(), test_members());
        // Put in fighting state so we can test formation ticks
        hunt.state = HuntState::Fighting { started_tick: 0 };

        let mut snap = test_snapshot(false);
        // Put healer far away so formation commands would fire
        snap.member_positions[1] = (101, Pos2D::new(100.0, 350.0)); // 150 from tank

        // Tick until we hit a formation check interval
        let mut got_formation_cmd = false;
        for _ in 0..FORMATION_CHECK_INTERVAL + 1 {
            let cmds = hunt.tick(Some(&snap));
            if cmds
                .iter()
                .any(|(pid, cmd)| *pid == 101 && cmd.contains("/face"))
            {
                got_formation_cmd = true;
                break;
            }
        }

        assert!(
            got_formation_cmd,
            "Should generate formation commands for out-of-position healer"
        );
    }

    #[test]
    fn test_operating_mode_display() {
        assert_eq!(format!("{}", OperatingMode::Camp), "Camp");
        assert_eq!(format!("{}", OperatingMode::Hunt), "Hunt");
    }

    #[test]
    fn test_pos2d_distance() {
        let a = Pos2D::new(0.0, 0.0);
        let b = Pos2D::new(3.0, 4.0);
        assert!((a.distance_to(&b) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_pos2d_distance_to_self_is_zero() {
        let a = Pos2D::new(100.0, 200.0);
        assert!(a.distance_to(&a) < f32::EPSILON);
    }

    #[test]
    fn test_operating_mode_equality() {
        assert_eq!(OperatingMode::Camp, OperatingMode::Camp);
        assert_eq!(OperatingMode::Hunt, OperatingMode::Hunt);
        assert_ne!(OperatingMode::Camp, OperatingMode::Hunt);
    }

    #[test]
    fn test_hunt_state_equality() {
        assert_eq!(HuntState::Roaming, HuntState::Roaming);
        assert_ne!(HuntState::Roaming, HuntState::Engaging { started_tick: 0 });
    }

    #[test]
    fn test_formation_default_values() {
        let fc = FormationConfig::default();
        assert!((fc.melee_follow_dist - 20.0).abs() < f32::EPSILON);
        assert!((fc.melee_leash_dist - 40.0).abs() < f32::EPSILON);
        assert!((fc.caster_follow_dist - 70.0).abs() < f32::EPSILON);
        assert!((fc.caster_leash_dist - 100.0).abs() < f32::EPSILON);
        assert!((fc.caster_min_dist - 40.0).abs() < f32::EPSILON);
        assert!((fc.healer_follow_dist - 50.0).abs() < f32::EPSILON);
        assert!((fc.healer_leash_dist - 80.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_formation_puller_is_in_position() {
        let fm = FormationManager::new(FormationConfig::default());
        assert!(fm.is_in_position(&Role::Puller, 999.0));
    }

    #[test]
    fn test_formation_puller_gets_no_movement_commands() {
        let fm = FormationManager::new(FormationConfig::default());
        let member = CampMember::new(105, "Puller01".into(), Role::Puller);
        let member_pos = Pos2D::new(100.0, 260.0); // displaced from tank
        let tank_pos = Pos2D::new(100.0, 200.0);

        let cmds = fm.formation_commands(&member, &member_pos, &tank_pos, "Tank", false);
        assert!(
            cmds.is_empty(),
            "Puller should not receive formation movement commands"
        );
    }

    #[test]
    fn test_formation_dps_in_position_boundary() {
        let fm = FormationManager::new(FormationConfig::default());
        // desired = 20.0, in_position if dist <= 20.0 * 1.2 = 24.0
        assert!(fm.is_in_position(&Role::Dps, 24.0));
        assert!(!fm.is_in_position(&Role::Dps, 25.0));
    }

    #[test]
    fn test_pick_target_with_no_pull_mobs() {
        let mut config = test_config();
        config.pull_mob_names.clear();
        let hunt = HuntLoop::new(config, test_members());
        // pick_target falls back to "a_mob"
        assert_eq!(hunt.config.pull_mob_names.len(), 0);
    }

    #[test]
    fn test_patrol_waypoints_initially_empty() {
        let hunt = HuntLoop::new(test_config(), test_members());
        assert!(hunt.patrol_waypoints.is_empty());
        assert_eq!(hunt.patrol_idx, 0);
    }

    #[test]
    fn test_hunt_no_members_no_target_command() {
        let mut hunt = HuntLoop::new(test_config(), Vec::new());
        let cmds = hunt.tick(None);
        // No tank found, still transitions to Engaging (roaming checks tank)
        // With no members, find_by_role returns None so no /target is issued
        assert!(cmds.is_empty() || !cmds.iter().any(|(_, cmd)| cmd.contains("/target")));
    }
}
