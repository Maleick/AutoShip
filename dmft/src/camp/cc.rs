//! Crowd control subsystem — mez, stun, charm, snare, root tracking and assignment.
//!
//! CC priority (highest to lowest):
//! 1. Stun (instant, highest priority)
//! 2. Mez (longer duration control)
//! 3. Snare (slows, doesn't stop)
//! 4. Charm (permanent fix via re-charm)
//! 5. Root (last resort — makes re-charm harder)

use std::collections::HashMap;

/// Types of crowd control, ordered by priority (lower discriminant = higher priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CcType {
    Stun,
    Mez,
    Snare,
    Charm,
    Root,
}

impl CcType {
    /// Lower value = higher priority. Matches the CC priority list.
    pub fn priority(self) -> u8 {
        match self {
            CcType::Stun => 1,
            CcType::Mez => 2,
            CcType::Snare => 3,
            CcType::Charm => 4,
            CcType::Root => 5,
        }
    }
}

/// A CC ability available to a group member.
#[derive(Debug, Clone)]
pub struct CcAbility {
    pub cc_type: CcType,
    pub command: String,
    pub cooldown_ticks: u64,
    pub duration_ticks: u64,
    pub priority: u8,
}

/// A debuff ability (Tash, Malo) that should land before CC.
#[derive(Debug, Clone)]
pub struct DebuffAbility {
    pub name: String,
    pub command: String,
    pub cooldown_ticks: u64,
    /// Lower = lands first (Tash before Malo).
    pub order: u8,
}

/// A mob being tracked for CC.
#[derive(Debug, Clone)]
pub struct CcTarget {
    pub spawn_id: u32,
    pub name: String,
    pub cc_applied: Option<CcType>,
    pub cc_expiry_tick: u64,
    pub assigned_to_pid: Option<u32>,
    pub debuffed: bool,
}

/// A group member with CC capabilities.
#[derive(Debug, Clone)]
pub struct CcMember {
    pub pid: u32,
    pub name: String,
    pub cc_abilities: Vec<CcAbility>,
    pub debuff_abilities: Vec<DebuffAbility>,
    pub last_cast_tick: u64,
}

/// Tracks all CC targets and assignments for a camp group.
pub struct CcTracker {
    pub targets: Vec<CcTarget>,
}

impl CcTracker {
    pub fn new() -> Self {
        Self {
            targets: Vec::new(),
        }
    }

    /// Update tracked CC targets based on current spawns near camp.
    ///
    /// - `current_spawn_ids`: all mob spawn IDs currently in camp radius
    /// - `assist_target_id`: the mob being killed — do NOT CC this one
    /// - `tick`: current game tick
    ///   Add a single new CC target without pruning existing targets.
    ///   Used when an add spawns mid-fight — we don't want to lose
    ///   existing CC state on other mobs.
    pub fn add_target(&mut self, spawn_id: u32, name: String) {
        if !self.targets.iter().any(|t| t.spawn_id == spawn_id) {
            self.targets.push(CcTarget {
                spawn_id,
                name,
                cc_applied: None,
                cc_expiry_tick: 0,
                assigned_to_pid: None,
                debuffed: false,
            });
        }
    }

    pub fn update(
        &mut self,
        current_spawns: &[(u32, String)],
        assist_target_id: Option<u32>,
        tick: u64,
    ) {
        let active_ids: Vec<u32> = current_spawns.iter().map(|(id, _)| *id).collect();

        // Remove targets that are no longer present (dead or despawned)
        self.targets.retain(|t| active_ids.contains(&t.spawn_id));

        // Add new spawns that aren't the assist target and aren't already tracked
        for (id, name) in current_spawns {
            if assist_target_id == Some(*id) {
                continue;
            }
            if !self.targets.iter().any(|t| t.spawn_id == *id) {
                self.targets.push(CcTarget {
                    spawn_id: *id,
                    name: name.clone(),
                    cc_applied: None,
                    cc_expiry_tick: 0,
                    assigned_to_pid: None,
                    debuffed: false,
                });
            }
        }

        // Expire CC that has worn off
        for target in &mut self.targets {
            if target.cc_applied.is_some() && tick >= target.cc_expiry_tick {
                target.cc_applied = None;
                target.assigned_to_pid = None;
            }
        }
    }

    /// Assign CC duties to group members. Returns `(pid, command)` pairs.
    ///
    /// Each uncontrolled target gets assigned to the member with the
    /// highest-priority CC ability that isn't on cooldown.
    pub fn assign_cc(
        &mut self,
        members: &mut [CcMember],
        tick: u64,
    ) -> Vec<(u32, String)> {
        let mut commands = Vec::new();

        // Track how many targets each member is already assigned to
        let mut assignment_count: HashMap<u32, usize> = HashMap::new();
        for target in &self.targets {
            if let Some(pid) = target.assigned_to_pid {
                *assignment_count.entry(pid).or_insert(0) += 1;
            }
        }

        // Sort uncontrolled targets — unassigned first, then by spawn_id for determinism
        let uncontrolled: Vec<usize> = self
            .targets
            .iter()
            .enumerate()
            .filter(|(_, t)| t.cc_applied.is_none())
            .map(|(i, _)| i)
            .collect();

        for idx in uncontrolled {
            // Find best available member: lowest CC priority value, fewest existing assignments, off cooldown
            let best = members
                .iter()
                .filter(|m| !m.cc_abilities.is_empty())
                .filter(|m| {
                    // Must be off cooldown (simple: last_cast_tick + min cooldown <= tick)
                    m.cc_abilities.iter().any(|a| {
                        m.last_cast_tick + a.cooldown_ticks <= tick
                    })
                })
                .min_by_key(|m| {
                    let best_priority = m
                        .cc_abilities
                        .iter()
                        .map(|a| a.priority)
                        .min()
                        .unwrap_or(255);
                    let count = assignment_count.get(&m.pid).copied().unwrap_or(0);
                    (best_priority, count)
                });

            if let Some(member) = best {
                // Pick the highest-priority ability that's off cooldown
                if let Some(ability) = member
                    .cc_abilities
                    .iter()
                    .filter(|a| member.last_cast_tick + a.cooldown_ticks <= tick)
                    .min_by_key(|a| a.priority)
                {
                    let target = &self.targets[idx];
                    let member_pid = member.pid;
                    // Target the mob, then cast CC
                    commands.push((member_pid, format!("/target id {}", target.spawn_id)));
                    commands.push((member_pid, ability.command.clone()));

                    // Mark assignment
                    self.targets[idx].cc_applied = Some(ability.cc_type);
                    self.targets[idx].cc_expiry_tick = tick + ability.duration_ticks;
                    self.targets[idx].assigned_to_pid = Some(member_pid);
                    *assignment_count.entry(member_pid).or_insert(0) += 1;

                    // Update member's cooldown tracking
                    if let Some(m) = members.iter_mut().find(|m| m.pid == member_pid) {
                        m.last_cast_tick = tick;
                    }
                }
            }
        }

        commands
    }

    /// Emergency response to a charm break. Returns immediate stun/mez commands.
    ///
    /// Priority: stun first (instant), then mez as backup.
    pub fn charm_break_response(
        &mut self,
        charm_broken_spawn_id: u32,
        members: &[CcMember],
        tick: u64,
    ) -> Vec<(u32, String)> {
        let mut commands = Vec::new();

        // Mark the target as no longer charmed
        if let Some(target) = self.targets.iter_mut().find(|t| t.spawn_id == charm_broken_spawn_id) {
            target.cc_applied = None;
            target.assigned_to_pid = None;
        }

        // Collect all stun abilities first, then mez abilities
        let mut responders: Vec<(&CcMember, &CcAbility)> = Vec::new();
        for member in members {
            for ability in &member.cc_abilities {
                if matches!(ability.cc_type, CcType::Stun | CcType::Mez)
                    && member.last_cast_tick + ability.cooldown_ticks <= tick
                {
                    responders.push((member, ability));
                }
            }
        }

        // Sort by CC priority (stun first)
        responders.sort_by_key(|(_, a)| a.cc_type.priority());

        // Send the first available responder
        if let Some((member, ability)) = responders.first() {
            commands.push((member.pid, format!("/target id {charm_broken_spawn_id}")));
            commands.push((member.pid, ability.command.clone()));

            // Update tracker
            if let Some(target) = self.targets.iter_mut().find(|t| t.spawn_id == charm_broken_spawn_id) {
                target.cc_applied = Some(ability.cc_type);
                target.cc_expiry_tick = tick + ability.duration_ticks;
                target.assigned_to_pid = Some(member.pid);
            }
        }

        commands
    }

    /// Check for CCs about to expire and return re-mez/re-CC commands.
    ///
    /// `buffer_ticks`: how many ticks before expiry to start re-casting (default: 3).
    pub fn needs_remez(&mut self, tick: u64, buffer_ticks: u64, members: &mut [CcMember]) -> Vec<(u32, String)> {
        let mut commands = Vec::new();

        for target in &mut self.targets {
            // Only re-CC targets that have active CC about to expire
            let Some(cc_type) = target.cc_applied else {
                continue;
            };
            if tick + buffer_ticks < target.cc_expiry_tick {
                continue;
            }

            // Find the assigned member (or any member with matching CC ability)
            let member = target
                .assigned_to_pid
                .and_then(|pid| members.iter().find(|m| m.pid == pid))
                .or_else(|| {
                    members.iter().find(|m| {
                        m.cc_abilities.iter().any(|a| a.cc_type == cc_type)
                    })
                });

            if let Some(member) = member
                && let Some(ability) = member
                    .cc_abilities
                    .iter()
                    .find(|a| a.cc_type == cc_type && member.last_cast_tick + a.cooldown_ticks <= tick)
            {
                let member_pid = member.pid;
                let duration = ability.duration_ticks;
                commands.push((member_pid, format!("/target id {}", target.spawn_id)));
                commands.push((member_pid, ability.command.clone()));

                // Extend CC expiry so we don't spam re-mez every tick
                target.cc_expiry_tick = tick + duration;

                // Update member's cooldown tracking
                if let Some(m) = members.iter_mut().find(|m| m.pid == member_pid) {
                    m.last_cast_tick = tick;
                }
            }
        }

        commands
    }

    /// Generate debuff commands (Tash/Malo) that should land before CC.
    ///
    /// Returns commands ordered by debuff priority (Tash before Malo).
    pub fn debuff_commands(
        &mut self,
        target_id: u32,
        members: &[CcMember],
        tick: u64,
    ) -> Vec<(u32, String)> {
        let mut commands = Vec::new();

        // Check if this target still needs debuffs
        let needs_debuff = self
            .targets
            .iter()
            .any(|t| t.spawn_id == target_id && !t.debuffed);

        if !needs_debuff {
            return commands;
        }

        // Collect all debuff abilities from all members, sorted by order
        let mut debuffers: Vec<(&CcMember, &DebuffAbility)> = Vec::new();
        for member in members {
            for debuff in &member.debuff_abilities {
                if member.last_cast_tick + debuff.cooldown_ticks <= tick {
                    debuffers.push((member, debuff));
                }
            }
        }
        debuffers.sort_by_key(|(_, d)| d.order);

        for (member, debuff) in &debuffers {
            commands.push((member.pid, format!("/target id {target_id}")));
            commands.push((member.pid, debuff.command.clone()));
        }

        // Mark as debuffed (debuffs sent — actual landing is async)
        if !debuffers.is_empty() && let Some(target) = self.targets.iter_mut().find(|t| t.spawn_id == target_id) {
            target.debuffed = true;
        }

        commands
    }

    /// Count of mobs currently without any CC applied.
    pub fn uncontrolled_count(&self) -> usize {
        self.targets.iter().filter(|t| t.cc_applied.is_none()).count()
    }

    /// Count of mobs currently under CC.
    pub fn controlled_count(&self) -> usize {
        self.targets.iter().filter(|t| t.cc_applied.is_some()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_enchanter(pid: u32) -> CcMember {
        CcMember {
            pid,
            name: format!("Enchanter{pid:02}"),
            cc_abilities: vec![
                CcAbility {
                    cc_type: CcType::Mez,
                    command: "/cast 1".into(),
                    cooldown_ticks: 3,
                    duration_ticks: 18,
                    priority: 2,
                },
                CcAbility {
                    cc_type: CcType::Stun,
                    command: "/cast 8".into(),
                    cooldown_ticks: 6,
                    duration_ticks: 4,
                    priority: 1,
                },
                CcAbility {
                    cc_type: CcType::Charm,
                    command: "/cast 3".into(),
                    cooldown_ticks: 3,
                    duration_ticks: 60,
                    priority: 4,
                },
            ],
            debuff_abilities: vec![DebuffAbility {
                name: "Tash".into(),
                command: "/cast 2".into(),
                cooldown_ticks: 3,
                order: 1,
            }],
            last_cast_tick: 0,
        }
    }

    fn make_paladin(pid: u32) -> CcMember {
        CcMember {
            pid,
            name: format!("Paladin{pid:02}"),
            cc_abilities: vec![CcAbility {
                cc_type: CcType::Stun,
                command: "/cast 1".into(),
                cooldown_ticks: 6,
                duration_ticks: 4,
                priority: 1,
            }],
            debuff_abilities: vec![],
            last_cast_tick: 0,
        }
    }

    fn make_shaman(pid: u32) -> CcMember {
        CcMember {
            pid,
            name: format!("Shaman{pid:02}"),
            cc_abilities: vec![],
            debuff_abilities: vec![DebuffAbility {
                name: "Malo".into(),
                command: "/cast 3".into(),
                cooldown_ticks: 3,
                order: 2,
            }],
            last_cast_tick: 0,
        }
    }

    fn make_druid(pid: u32) -> CcMember {
        CcMember {
            pid,
            name: format!("Druid{pid:02}"),
            cc_abilities: vec![
                CcAbility {
                    cc_type: CcType::Snare,
                    command: "/cast 2".into(),
                    cooldown_ticks: 3,
                    duration_ticks: 24,
                    priority: 3,
                },
                CcAbility {
                    cc_type: CcType::Root,
                    command: "/cast 7".into(),
                    cooldown_ticks: 3,
                    duration_ticks: 30,
                    priority: 5,
                },
            ],
            debuff_abilities: vec![],
            last_cast_tick: 0,
        }
    }

    // -- CcType priority tests --

    #[test]
    fn test_cc_priority_ordering() {
        assert!(CcType::Stun.priority() < CcType::Mez.priority());
        assert!(CcType::Mez.priority() < CcType::Snare.priority());
        assert!(CcType::Snare.priority() < CcType::Charm.priority());
        assert!(CcType::Charm.priority() < CcType::Root.priority());
    }

    // -- CcTracker::update tests --

    #[test]
    fn test_update_adds_new_targets() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into()), (11, "orc centurion".into())];
        tracker.update(&spawns, None, 0);
        assert_eq!(tracker.targets.len(), 2);
    }

    #[test]
    fn test_update_skips_assist_target() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into()), (11, "orc centurion".into())];
        tracker.update(&spawns, Some(10), 0);
        assert_eq!(tracker.targets.len(), 1);
        assert_eq!(tracker.targets[0].spawn_id, 11);
    }

    #[test]
    fn test_update_removes_dead_spawns() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into()), (11, "orc centurion".into())];
        tracker.update(&spawns, None, 0);
        assert_eq!(tracker.targets.len(), 2);

        // Only one mob remains
        let spawns = vec![(11, "orc centurion".into())];
        tracker.update(&spawns, None, 1);
        assert_eq!(tracker.targets.len(), 1);
        assert_eq!(tracker.targets[0].spawn_id, 11);
    }

    #[test]
    fn test_update_expires_cc() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, None, 0);
        tracker.targets[0].cc_applied = Some(CcType::Mez);
        tracker.targets[0].cc_expiry_tick = 10;
        tracker.targets[0].assigned_to_pid = Some(100);

        // Before expiry — still controlled
        tracker.update(&spawns, None, 9);
        assert!(tracker.targets[0].cc_applied.is_some());

        // At expiry — CC drops
        tracker.update(&spawns, None, 10);
        assert!(tracker.targets[0].cc_applied.is_none());
        assert!(tracker.targets[0].assigned_to_pid.is_none());
    }

    #[test]
    fn test_update_does_not_duplicate() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, None, 0);
        tracker.update(&spawns, None, 1);
        assert_eq!(tracker.targets.len(), 1);
    }

    // -- CcTracker::assign_cc tests --

    #[test]
    fn test_assign_cc_basic() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, None, 0);

        let mut members = vec![make_enchanter(100)];
        let cmds = tracker.assign_cc(&mut members, 10);

        // Should target then cast (stun is highest priority for enchanter)
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0], (100, "/target id 10".into()));
        assert_eq!(cmds[1], (100, "/cast 8".into())); // stun
    }

    #[test]
    fn test_assign_cc_multiple_adds() {
        let mut tracker = CcTracker::new();
        let spawns = vec![
            (10, "orc pawn".into()),
            (11, "orc centurion".into()),
        ];
        tracker.update(&spawns, None, 0);

        let mut members = vec![make_enchanter(100), make_paladin(101)];
        let cmds = tracker.assign_cc(&mut members, 10);

        // Both mobs should get CC'd
        assert_eq!(cmds.len(), 4); // 2 commands per target
        let pids: Vec<u32> = cmds.iter().map(|(pid, _)| *pid).collect();
        assert!(pids.contains(&100));
        assert!(pids.contains(&101));
    }

    #[test]
    fn test_assign_cc_already_controlled_skip() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, None, 0);
        tracker.targets[0].cc_applied = Some(CcType::Mez);
        tracker.targets[0].cc_expiry_tick = 100;

        let mut members = vec![make_enchanter(100)];
        let cmds = tracker.assign_cc(&mut members, 10);
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_assign_cc_no_members_with_abilities() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, None, 0);

        let mut members = vec![make_shaman(100)]; // shaman has no CC abilities
        let cmds = tracker.assign_cc(&mut members, 10);
        assert!(cmds.is_empty());
    }

    // -- charm_break_response tests --

    #[test]
    fn test_charm_break_response_stun_first() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "a_charmed_mob".into())];
        tracker.update(&spawns, None, 0);
        tracker.targets[0].cc_applied = Some(CcType::Charm);
        tracker.targets[0].cc_expiry_tick = 100;

        let members = vec![make_enchanter(100), make_paladin(101)];
        let cmds = tracker.charm_break_response(10, &members, 10);

        // Should pick stun (priority 1) over mez (priority 2)
        assert_eq!(cmds.len(), 2);
        assert!(cmds[1].1.contains("/cast")); // stun command
        // The target should now be marked as stunned
        assert_eq!(tracker.targets[0].cc_applied, Some(CcType::Stun));
    }

    #[test]
    fn test_charm_break_clears_charm() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "a_charmed_mob".into())];
        tracker.update(&spawns, None, 0);
        tracker.targets[0].cc_applied = Some(CcType::Charm);
        tracker.targets[0].cc_expiry_tick = 100;
        tracker.targets[0].assigned_to_pid = Some(100);

        let members: Vec<CcMember> = vec![];
        let cmds = tracker.charm_break_response(10, &members, 10);

        // No responders available, but charm should still be cleared
        assert!(cmds.is_empty());
        assert!(tracker.targets[0].cc_applied.is_none());
        assert!(tracker.targets[0].assigned_to_pid.is_none());
    }

    // -- needs_remez tests --

    #[test]
    fn test_needs_remez_within_buffer() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, None, 0);
        tracker.targets[0].cc_applied = Some(CcType::Mez);
        tracker.targets[0].cc_expiry_tick = 20;
        tracker.targets[0].assigned_to_pid = Some(100);

        let mut members = vec![make_enchanter(100)];
        // tick 18, buffer 3 => 18+3=21 >= 20, needs remez
        let cmds = tracker.needs_remez(18, 3, &mut members);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0], (100, "/target id 10".into()));
        assert_eq!(cmds[1], (100, "/cast 1".into())); // mez command
    }

    #[test]
    fn test_needs_remez_not_yet() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, None, 0);
        tracker.targets[0].cc_applied = Some(CcType::Mez);
        tracker.targets[0].cc_expiry_tick = 20;

        let mut members = vec![make_enchanter(100)];
        // tick 10, buffer 3 => 10+3=13 < 20, no remez yet
        let cmds = tracker.needs_remez(10, 3, &mut members);
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_needs_remez_no_cc_applied() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, None, 0);

        let mut members = vec![make_enchanter(100)];
        let cmds = tracker.needs_remez(18, 3, &mut members);
        assert!(cmds.is_empty());
    }

    // -- debuff_commands tests --

    #[test]
    fn test_debuff_tash_before_malo() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, None, 0);

        let members = vec![make_enchanter(100), make_shaman(101)];
        let cmds = tracker.debuff_commands(10, &members, 10);

        // Should have 4 commands: target+tash, target+malo
        assert_eq!(cmds.len(), 4);
        // Tash (order 1) before Malo (order 2)
        assert_eq!(cmds[0], (100, "/target id 10".into()));
        assert_eq!(cmds[1], (100, "/cast 2".into())); // Tash
        assert_eq!(cmds[2], (101, "/target id 10".into()));
        assert_eq!(cmds[3], (101, "/cast 3".into())); // Malo

        // Target should be marked debuffed
        assert!(tracker.targets[0].debuffed);
    }

    #[test]
    fn test_debuff_skip_already_debuffed() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, None, 0);
        tracker.targets[0].debuffed = true;

        let members = vec![make_enchanter(100), make_shaman(101)];
        let cmds = tracker.debuff_commands(10, &members, 10);
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_debuff_unknown_target_no_crash() {
        let mut tracker = CcTracker::new();
        let members = vec![make_enchanter(100)];
        let cmds = tracker.debuff_commands(999, &members, 10);
        assert!(cmds.is_empty());
    }

    // -- Counter tests --

    #[test]
    fn test_controlled_uncontrolled_counts() {
        let mut tracker = CcTracker::new();
        let spawns = vec![
            (10, "orc pawn".into()),
            (11, "orc centurion".into()),
            (12, "orc legionnaire".into()),
        ];
        tracker.update(&spawns, None, 0);

        assert_eq!(tracker.uncontrolled_count(), 3);
        assert_eq!(tracker.controlled_count(), 0);

        tracker.targets[0].cc_applied = Some(CcType::Mez);
        tracker.targets[1].cc_applied = Some(CcType::Stun);

        assert_eq!(tracker.uncontrolled_count(), 1);
        assert_eq!(tracker.controlled_count(), 2);
    }

    // -- Integration-style tests --

    #[test]
    fn test_full_cc_workflow() {
        let mut tracker = CcTracker::new();
        let tick = 10u64;

        // Adds show up in camp
        let spawns = vec![
            (10, "orc pawn".into()),
            (11, "orc centurion".into()),
        ];
        tracker.update(&spawns, Some(10), tick); // 10 is assist target

        // Only mob 11 should be tracked (10 is being killed)
        assert_eq!(tracker.targets.len(), 1);
        assert_eq!(tracker.targets[0].spawn_id, 11);

        // Debuff before CC
        let mut members = vec![make_enchanter(100), make_shaman(101)];
        let debuff_cmds = tracker.debuff_commands(11, &members, tick);
        assert!(!debuff_cmds.is_empty());
        assert!(tracker.targets[0].debuffed);

        // Assign CC
        let cc_cmds = tracker.assign_cc(&mut members, tick);
        assert_eq!(cc_cmds.len(), 2); // target + cast
        assert!(tracker.targets[0].cc_applied.is_some());

        // Later: check for remez
        let _remez_cmds = tracker.needs_remez(tick + 16, 3, &mut members);
        // If cc_expiry is tick+4 (stun duration), and we're at tick+16, it already expired
        // so needs_remez won't fire (cc_applied would have been cleared by update)
        // Let's manually set a mez instead for this test
        tracker.targets[0].cc_applied = Some(CcType::Mez);
        tracker.targets[0].cc_expiry_tick = tick + 33;
        tracker.targets[0].assigned_to_pid = Some(100);

        // tick + 31 is past the mez cooldown (3 ticks) since last cast was updated at tick+16
        let remez_cmds = tracker.needs_remez(tick + 31, 3, &mut members);
        assert!(!remez_cmds.is_empty());

        // Mob dies, gets removed on next update
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, Some(10), tick + 20);
        assert!(tracker.targets.is_empty());
    }

    #[test]
    fn test_druid_snare_and_root_as_last_resort() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "fleeing mob".into())];
        tracker.update(&spawns, None, 0);

        // Druid only — should use snare (priority 3) over root (priority 5)
        let mut members = vec![make_druid(100)];
        let cmds = tracker.assign_cc(&mut members, 10);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[1], (100, "/cast 2".into())); // snare, not root
        assert_eq!(tracker.targets[0].cc_applied, Some(CcType::Snare));
    }

    // -- Bug #3: assign_cc must update last_cast_tick --

    #[test]
    fn test_assign_cc_updates_last_cast_tick() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, None, 0);

        let mut members = vec![make_enchanter(100)];
        assert_eq!(members[0].last_cast_tick, 0);

        tracker.assign_cc(&mut members, 10);
        assert_eq!(members[0].last_cast_tick, 10);
    }

    #[test]
    fn test_assign_cc_cooldown_prevents_double_cast() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into()), (11, "orc centurion".into())];
        tracker.update(&spawns, None, 0);

        // Single enchanter with stun (cooldown 6 ticks)
        let mut members = vec![make_enchanter(100)];
        let cmds = tracker.assign_cc(&mut members, 10);
        // First mob gets CC'd
        assert_eq!(cmds.len(), 2);
        // Member is now on cooldown at tick 10, so second mob should NOT get CC'd
        // (stun cooldown is 6, so next available at tick 16; mez cooldown is 3, so available at tick 13)
        // Actually, the second uncontrolled mob is still there — but the member's
        // last_cast_tick was updated to 10 during the first assignment
        assert_eq!(members[0].last_cast_tick, 10);
    }

    // -- Bug #4: needs_remez must extend cc_expiry_tick --

    #[test]
    fn test_needs_remez_extends_expiry() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, None, 0);
        tracker.targets[0].cc_applied = Some(CcType::Mez);
        tracker.targets[0].cc_expiry_tick = 20;
        tracker.targets[0].assigned_to_pid = Some(100);

        let mut members = vec![make_enchanter(100)];
        // tick 18, buffer 3 => needs remez
        let cmds = tracker.needs_remez(18, 3, &mut members);
        assert_eq!(cmds.len(), 2);

        // cc_expiry_tick should be extended (18 + mez duration 18 = 36)
        assert_eq!(tracker.targets[0].cc_expiry_tick, 36);

        // Calling again at same tick should NOT produce commands (expiry is now 36)
        let cmds = tracker.needs_remez(18, 3, &mut members);
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_needs_remez_updates_member_cooldown() {
        let mut tracker = CcTracker::new();
        let spawns = vec![(10, "orc pawn".into())];
        tracker.update(&spawns, None, 0);
        tracker.targets[0].cc_applied = Some(CcType::Mez);
        tracker.targets[0].cc_expiry_tick = 20;
        tracker.targets[0].assigned_to_pid = Some(100);

        let mut members = vec![make_enchanter(100)];
        assert_eq!(members[0].last_cast_tick, 0);

        tracker.needs_remez(18, 3, &mut members);
        assert_eq!(members[0].last_cast_tick, 18);
    }
}
