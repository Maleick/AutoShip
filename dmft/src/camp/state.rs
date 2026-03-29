//! Camp loop state machine — drives the pull/fight/loot/med cycle.

use crate::camp::config::CampConfig;

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
}

/// Timer durations (in ticks) for each phase.
const PULL_DURATION: u64 = 5;
const FIGHT_DURATION: u64 = 15;
const LOOT_DURATION: u64 = 3;
const MED_DURATION: u64 = 10;
const BUFF_DURATION: u64 = 8;

/// The camp loop state machine. Each `tick()` call advances state and
/// returns slash commands to send to EQ clients via IPC.
pub struct CampLoop {
    pub config: CampConfig,
    pub state: CampState,
    pub members: Vec<CampMember>,
    pub tick: u64,
    pub last_pull_target: String,
}

impl CampLoop {
    pub fn new(config: CampConfig, members: Vec<CampMember>) -> Self {
        Self {
            config,
            state: CampState::Idle,
            members,
            tick: 0,
            last_pull_target: String::new(),
        }
    }

    /// Advance the state machine by one tick. Returns `(pid, slash_command)` pairs
    /// to send to EQ clients.
    pub fn tick(&mut self) -> Vec<(u32, String)> {
        self.tick += 1;
        let mut commands = Vec::new();

        match self.state.clone() {
            CampState::Idle => {
                self.transition_to_pulling(&mut commands);
            }
            CampState::Pulling { started_tick } => {
                if self.tick - started_tick >= PULL_DURATION {
                    self.transition_to_fighting(&mut commands);
                }
            }
            CampState::Fighting { started_tick } => {
                if self.tick - started_tick >= FIGHT_DURATION {
                    self.transition_to_looting(&mut commands);
                }
            }
            CampState::Looting { started_tick } => {
                if self.tick - started_tick >= LOOT_DURATION {
                    self.transition_to_medding(&mut commands);
                }
            }
            CampState::Medding { started_tick } => {
                if self.tick - started_tick >= MED_DURATION {
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
            CampMember { pid: 100, name: "Warrior01".into(), role: Role::Tank },
            CampMember { pid: 101, name: "Cleric01".into(), role: Role::Healer },
            CampMember { pid: 102, name: "Enchanter01".into(), role: Role::CC },
            CampMember { pid: 103, name: "Bard01".into(), role: Role::Puller },
            CampMember { pid: 104, name: "Ranger01".into(), role: Role::DPS },
            CampMember { pid: 105, name: "Ranger02".into(), role: Role::DPS },
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
        let cmds = camp.tick();

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
        camp.tick(); // Idle -> Pulling

        // Advance through pull duration
        for _ in 0..PULL_DURATION - 1 {
            let cmds = camp.tick();
            assert!(cmds.is_empty()); // No commands during wait
            assert!(matches!(camp.state, CampState::Pulling { .. }));
        }

        let cmds = camp.tick(); // Should transition to Fighting
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
        camp.tick(); // -> Pulling
        for _ in 0..PULL_DURATION {
            camp.tick();
        }
        assert!(matches!(camp.state, CampState::Fighting { .. }));

        // Advance through fight duration
        for _ in 0..FIGHT_DURATION - 1 {
            camp.tick();
        }

        let cmds = camp.tick(); // -> Looting
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
        camp.tick(); // -> Pulling
        for _ in 0..PULL_DURATION {
            camp.tick();
        }
        for _ in 0..FIGHT_DURATION {
            camp.tick();
        }
        assert!(matches!(camp.state, CampState::Looting { .. }));

        for _ in 0..LOOT_DURATION - 1 {
            camp.tick();
        }

        let cmds = camp.tick(); // -> Medding
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
        camp.tick(); // -> Pulling
        for _ in 0..PULL_DURATION {
            camp.tick();
        }
        for _ in 0..FIGHT_DURATION {
            camp.tick();
        }
        for _ in 0..LOOT_DURATION {
            camp.tick();
        }
        assert!(matches!(camp.state, CampState::Medding { .. }));

        for _ in 0..MED_DURATION - 1 {
            camp.tick();
        }

        let cmds = camp.tick(); // -> Idle
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
            camp.tick();
        }

        assert_eq!(camp.state, CampState::Idle);

        // Next tick should start a new pull
        camp.tick();
        assert!(matches!(camp.state, CampState::Pulling { .. }));
    }

    #[test]
    fn test_no_puller_falls_back_to_tank() {
        let members = vec![
            CampMember { pid: 100, name: "Warrior01".into(), role: Role::Tank },
            CampMember { pid: 104, name: "Ranger01".into(), role: Role::DPS },
        ];
        let mut camp = CampLoop::new(test_config(), members);
        let cmds = camp.tick();

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
        let cmds = camp.tick();

        let target_cmd = cmds.iter().find(|(_, cmd)| cmd.contains("/target")).unwrap();
        assert!(target_cmd.1.contains("a_mob"));
    }

    #[test]
    fn test_buffing_to_idle() {
        let mut camp = CampLoop::new(test_config(), test_members());
        camp.state = CampState::Buffing { started_tick: 1 };
        camp.tick = 1;

        for _ in 0..BUFF_DURATION - 1 {
            let cmds = camp.tick();
            assert!(cmds.is_empty());
            assert!(matches!(camp.state, CampState::Buffing { .. }));
        }

        let cmds = camp.tick(); // -> Idle
        assert_eq!(camp.state, CampState::Idle);
        assert!(!cmds.is_empty()); // /stand commands
    }
}
