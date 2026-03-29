//! Death recovery — detect dead members, request resurrections, resume camp loop.

/// Death state for a single group member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeathState {
    Alive,
    Dead { died_at_tick: u64 },
    WaitingForRez,
    Rebuffing,
}

/// Tracks death/recovery state for the entire group.
pub struct RecoveryTracker {
    /// (pid, character_name, death_state)
    pub members: Vec<(u32, String, DeathState)>,
}

impl RecoveryTracker {
    pub fn new(members: &[(u32, String)]) -> Self {
        Self {
            members: members
                .iter()
                .map(|(pid, name)| (*pid, name.clone(), DeathState::Alive))
                .collect(),
        }
    }

    /// Update member state based on HP values. HP <= 0 means dead.
    pub fn update_hp(&mut self, hp_map: &[(u32, i32)], current_tick: u64) {
        for (pid, hp) in hp_map {
            if let Some(member) = self.members.iter_mut().find(|(p, _, _)| p == pid) {
                if *hp <= 0 && member.2 == DeathState::Alive {
                    member.2 = DeathState::Dead {
                        died_at_tick: current_tick,
                    };
                }
            }
        }
    }

    /// Mark a member as having received a rez (waiting to accept/stand).
    pub fn mark_rezzed(&mut self, pid: u32) {
        if let Some(member) = self.members.iter_mut().find(|(p, _, _)| *p == pid) {
            if matches!(member.2, DeathState::Dead { .. } | DeathState::WaitingForRez) {
                member.2 = DeathState::Rebuffing;
            }
        }
    }

    /// Mark a member as fully recovered.
    pub fn mark_alive(&mut self, pid: u32) {
        if let Some(member) = self.members.iter_mut().find(|(p, _, _)| *p == pid) {
            member.2 = DeathState::Alive;
        }
    }

    /// Returns true if any member is not Alive (camp loop should pause).
    pub fn recovery_in_progress(&self) -> bool {
        self.members.iter().any(|(_, _, state)| *state != DeathState::Alive)
    }

    /// Returns true when all members are alive (safe to resume camp loop).
    pub fn all_alive(&self) -> bool {
        self.members.iter().all(|(_, _, state)| *state == DeathState::Alive)
    }

    /// Get list of dead member pids.
    pub fn dead_members(&self) -> Vec<u32> {
        self.members
            .iter()
            .filter(|(_, _, state)| matches!(state, DeathState::Dead { .. }))
            .map(|(pid, _, _)| *pid)
            .collect()
    }
}

/// Generate recovery commands based on current member states.
///
/// - If cleric is alive and members are dead: target dead char + cast rez
/// - Dead char after rez: /stand
/// - Returns `(pid, command)` pairs
pub fn death_commands(
    members_state: &mut [(u32, String, DeathState)],
    cleric_pid: Option<u32>,
    rez_gem: u8,
) -> Vec<(u32, String)> {
    let mut commands = Vec::new();

    // Find the first dead member's pid and name
    let first_dead = members_state
        .iter()
        .find(|(_, _, state)| matches!(state, DeathState::Dead { .. }))
        .map(|(pid, name, _)| (*pid, name.clone()));

    // Find members waiting to rebuff (just got rezzed — stand up)
    let rebuffing_pids: Vec<u32> = members_state
        .iter()
        .filter(|(_, _, state)| *state == DeathState::Rebuffing)
        .map(|(pid, _, _)| *pid)
        .collect();

    // Cleric rezzes the first dead member
    if let Some(cleric) = cleric_pid {
        // Only rez if cleric is alive
        let cleric_alive = members_state
            .iter()
            .any(|(pid, _, state)| *pid == cleric && *state == DeathState::Alive);

        if cleric_alive {
            if let Some((dead_pid, dead_name)) = first_dead {
                commands.push((cleric, format!("/target {dead_name}")));
                commands.push((cleric, format!("/cast {rez_gem}")));

                // Mark as WaitingForRez so we don't spam rez every tick
                if let Some(member) = members_state.iter_mut().find(|(p, _, _)| *p == dead_pid) {
                    member.2 = DeathState::WaitingForRez;
                }
            }
        }
    }

    // Rebuffing members stand up
    for pid in &rebuffing_pids {
        commands.push((*pid, "/stand".into()));
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_tracker_all_alive() {
        let tracker = RecoveryTracker::new(&[(100, "Warrior01".into()), (101, "Cleric01".into())]);
        assert!(tracker.all_alive());
        assert!(!tracker.recovery_in_progress());
    }

    #[test]
    fn test_update_hp_marks_dead() {
        let mut tracker =
            RecoveryTracker::new(&[(100, "Warrior01".into()), (101, "Cleric01".into())]);
        tracker.update_hp(&[(100, 0)], 10);

        assert!(!tracker.all_alive());
        assert!(tracker.recovery_in_progress());
        assert_eq!(tracker.dead_members(), vec![100]);

        // Cleric still alive
        assert!(tracker
            .members
            .iter()
            .any(|(pid, _, state)| *pid == 101 && *state == DeathState::Alive));
    }

    #[test]
    fn test_update_hp_negative_is_dead() {
        let mut tracker = RecoveryTracker::new(&[(100, "Warrior01".into())]);
        tracker.update_hp(&[(100, -50)], 5);
        assert_eq!(tracker.dead_members(), vec![100]);
    }

    #[test]
    fn test_update_hp_positive_stays_alive() {
        let mut tracker = RecoveryTracker::new(&[(100, "Warrior01".into())]);
        tracker.update_hp(&[(100, 500)], 5);
        assert!(tracker.all_alive());
    }

    #[test]
    fn test_mark_rezzed() {
        let mut tracker = RecoveryTracker::new(&[(100, "Warrior01".into())]);
        tracker.update_hp(&[(100, 0)], 10);
        assert!(matches!(
            tracker.members[0].2,
            DeathState::Dead { died_at_tick: 10 }
        ));

        tracker.mark_rezzed(100);
        assert_eq!(tracker.members[0].2, DeathState::Rebuffing);
    }

    #[test]
    fn test_mark_alive() {
        let mut tracker = RecoveryTracker::new(&[(100, "Warrior01".into())]);
        tracker.update_hp(&[(100, 0)], 10);
        tracker.mark_rezzed(100);
        tracker.mark_alive(100);
        assert!(tracker.all_alive());
    }

    #[test]
    fn test_death_commands_cleric_rezzes_dead() {
        let mut members = vec![
            (100, "Warrior01".into(), DeathState::Dead { died_at_tick: 5 }),
            (101, "Cleric01".into(), DeathState::Alive),
        ];
        let cmds = death_commands(&mut members, Some(101), 5);

        assert!(cmds.iter().any(|(pid, cmd)| *pid == 101 && cmd.contains("/target Warrior01")));
        assert!(cmds.iter().any(|(pid, cmd)| *pid == 101 && cmd == "/cast 5"));
    }

    #[test]
    fn test_death_commands_dead_cleric_no_rez() {
        let mut members = vec![
            (100, "Warrior01".into(), DeathState::Dead { died_at_tick: 5 }),
            (
                101,
                "Cleric01".into(),
                DeathState::Dead { died_at_tick: 5 },
            ),
        ];
        let cmds = death_commands(&mut members, Some(101), 5);
        // Dead cleric can't cast
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_death_commands_rebuffing_stands() {
        let mut members = vec![
            (100, "Warrior01".into(), DeathState::Rebuffing),
            (101, "Cleric01".into(), DeathState::Alive),
        ];
        let cmds = death_commands(&mut members, Some(101), 5);
        assert!(cmds.iter().any(|(pid, cmd)| *pid == 100 && cmd == "/stand"));
    }

    #[test]
    fn test_death_commands_no_cleric() {
        let mut members = vec![(100, "Warrior01".into(), DeathState::Dead { died_at_tick: 5 })];
        let cmds = death_commands(&mut members, None, 5);
        // No cleric, no rez commands
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_death_commands_all_alive_no_commands() {
        let mut members = vec![
            (100, "Warrior01".into(), DeathState::Alive),
            (101, "Cleric01".into(), DeathState::Alive),
        ];
        let cmds = death_commands(&mut members, Some(101), 5);
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_multiple_dead_rezzes_first() {
        let mut members = vec![
            (100, "Warrior01".into(), DeathState::Dead { died_at_tick: 5 }),
            (101, "Cleric01".into(), DeathState::Alive),
            (102, "Ranger01".into(), DeathState::Dead { died_at_tick: 6 }),
        ];
        let cmds = death_commands(&mut members, Some(101), 5);
        // Should only target the first dead member
        let target_cmds: Vec<_> = cmds
            .iter()
            .filter(|(pid, cmd)| *pid == 101 && cmd.contains("/target"))
            .collect();
        assert_eq!(target_cmds.len(), 1);
        assert!(target_cmds[0].1.contains("Warrior01"));
    }

    #[test]
    fn test_dead_not_re_killed_on_update() {
        let mut tracker = RecoveryTracker::new(&[(100, "Warrior01".into())]);
        tracker.update_hp(&[(100, 0)], 10);
        // Second update with 0 HP should not change died_at_tick
        tracker.update_hp(&[(100, 0)], 20);
        assert!(matches!(
            tracker.members[0].2,
            DeathState::Dead { died_at_tick: 10 }
        ));
    }

    // -- Bug #5: rez must not spam every tick --

    #[test]
    fn test_death_commands_marks_waiting_for_rez() {
        let mut members = vec![
            (100, "Warrior01".into(), DeathState::Dead { died_at_tick: 5 }),
            (101, "Cleric01".into(), DeathState::Alive),
        ];
        let cmds = death_commands(&mut members, Some(101), 5);
        assert!(!cmds.is_empty());

        // After issuing rez, dead member should be WaitingForRez
        assert_eq!(members[0].2, DeathState::WaitingForRez);
    }

    #[test]
    fn test_death_commands_no_double_rez() {
        let mut members = vec![
            (100, "Warrior01".into(), DeathState::Dead { died_at_tick: 5 }),
            (101, "Cleric01".into(), DeathState::Alive),
        ];

        // First call: issues rez and transitions to WaitingForRez
        let cmds1 = death_commands(&mut members, Some(101), 5);
        assert!(cmds1.iter().any(|(pid, cmd)| *pid == 101 && cmd.contains("/cast")));

        // Second call: member is now WaitingForRez, no rez should be issued
        let cmds2 = death_commands(&mut members, Some(101), 5);
        assert!(!cmds2.iter().any(|(pid, cmd)| *pid == 101 && cmd.contains("/cast")));
    }
}
