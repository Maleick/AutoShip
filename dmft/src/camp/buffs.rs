//! Buff maintenance — track buff durations and queue rebuffs during downtime.

use std::collections::HashMap;

use crate::camp::class_config::ClassConfig;
use crate::camp::state::{CampMember, CampState, Role};

/// Priority ordering for buff types (lower = higher priority).
const PRIORITY_HASTE: u8 = 0;
const PRIORITY_HP: u8 = 1;
const PRIORITY_MANA_REGEN: u8 = 2;
const PRIORITY_STAT: u8 = 3;

/// Tracks when each buff was last cast per member.
pub struct BuffTracker {
    /// (pid, buff_name) -> last_cast_tick
    pub last_cast: HashMap<(u32, String), u64>,
}

impl BuffTracker {
    pub fn new() -> Self {
        Self {
            last_cast: HashMap::new(),
        }
    }

    /// Record that a buff was cast on a member.
    pub fn record_cast(&mut self, pid: u32, buff_name: &str, tick: u64) {
        self.last_cast.insert((pid, buff_name.to_string()), tick);
    }

    /// Check if a buff has expired for a member.
    pub fn is_expired(
        &self,
        pid: u32,
        buff_name: &str,
        duration_ticks: u64,
        current_tick: u64,
    ) -> bool {
        match self.last_cast.get(&(pid, buff_name.to_string())) {
            Some(&last) => current_tick.saturating_sub(last) >= duration_ticks,
            None => true, // Never cast = expired
        }
    }

    /// Ticks remaining on a buff, or 0 if expired.
    pub fn remaining(
        &self,
        pid: u32,
        buff_name: &str,
        duration_ticks: u64,
        current_tick: u64,
    ) -> u64 {
        match self.last_cast.get(&(pid, buff_name.to_string())) {
            Some(&last) => duration_ticks.saturating_sub(current_tick.saturating_sub(last)),
            None => 0,
        }
    }
}

/// A buff that needs to be cast, with its priority.
#[derive(Debug, Clone)]
struct PendingBuff {
    caster_pid: u32,
    target_pid: u32,
    buff_name: String,
    command: String,
    priority: u8,
}

/// Classify a buff name into a priority category.
fn buff_priority(buff_name: &str) -> u8 {
    let lower = buff_name.to_lowercase();
    if lower.contains("haste") || lower.contains("speed") || lower.contains("alacrity") {
        PRIORITY_HASTE
    } else if lower.contains("hp")
        || lower.contains("health")
        || lower.contains("symbol")
        || lower.contains("aegolism")
    {
        PRIORITY_HP
    } else if lower.contains("mana") || lower.contains("clarity") || lower.contains("kei") {
        PRIORITY_MANA_REGEN
    } else {
        PRIORITY_STAT
    }
}

/// Check all members for expired buffs and return `(caster_pid, command)` pairs.
///
/// Only returns rebuff commands when the camp is in Idle or Medding state.
/// Buffs are prioritized: haste > HP buff > mana regen > stat buffs.
pub fn check_buffs(
    tracker: &BuffTracker,
    members: &[CampMember],
    class_configs: &HashMap<String, ClassConfig>,
    current_tick: u64,
    camp_state: &CampState,
) -> Vec<(u32, String)> {
    // Only rebuff during downtime
    if !matches!(camp_state, CampState::Idle | CampState::Medding { .. }) {
        return Vec::new();
    }

    let mut pending: Vec<PendingBuff> = Vec::new();

    // For each member that has buff_abilities in their class config, check targets
    for member in members {
        let role_str = match member.role {
            Role::Tank => "tank",
            Role::Healer => "healer",
            Role::CC => "cc",
            Role::Dps => "dps",
            Role::Puller => "puller",
            Role::Bard => "bard",
        };

        if let Some(config) = class_configs.get(role_str) {
            for ability in &config.buff_abilities {
                // Each buff caster buffs all members
                // Use explicit buff duration if set, else fall back to cooldown
                let duration_ticks = ability.effective_duration_secs() as u64;

                for target in members {
                    if tracker.is_expired(target.pid, &ability.name, duration_ticks, current_tick) {
                        pending.push(PendingBuff {
                            caster_pid: member.pid,
                            target_pid: target.pid,
                            buff_name: ability.name.clone(),
                            command: ability.command.clone(),
                            priority: buff_priority(&ability.name),
                        });
                    }
                }
            }
        }
    }

    // Sort by priority (lower = higher priority)
    pending.sort_by_key(|p| p.priority);

    // Generate commands: target then cast
    let mut commands = Vec::new();
    for buff in &pending {
        // Find target name
        if let Some(target) = members.iter().find(|m| m.pid == buff.target_pid) {
            commands.push((buff.caster_pid, format!("/target {}", target.name)));
            commands.push((buff.caster_pid, buff.command.clone()));
        }
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camp::class_config::ClassAbility;

    fn test_members() -> Vec<CampMember> {
        vec![
            CampMember::new(100, "Warrior01".into(), Role::Tank),
            CampMember::new(101, "Cleric01".into(), Role::Healer),
            CampMember::new(102, "Enchanter01".into(), Role::CC),
        ]
    }

    fn test_class_configs() -> HashMap<String, ClassConfig> {
        let mut configs = HashMap::new();
        configs.insert(
            "healer".into(),
            ClassConfig {
                class_name: "cleric".into(),
                role: "healer".into(),
                combat_abilities: vec![],
                buff_abilities: vec![ClassAbility {
                    name: "Symbol of Naltron".into(),
                    command: "/cast 4".into(),
                    cooldown_secs: 100.0, // used as duration in ticks
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
        configs.insert(
            "cc".into(),
            ClassConfig {
                class_name: "enchanter".into(),
                role: "cc".into(),
                combat_abilities: vec![],
                buff_abilities: vec![
                    ClassAbility {
                        name: "Haste".into(),
                        command: "/cast 6".into(),
                        cooldown_secs: 120.0,
                        priority: 1,
                        condition: None,
                        duration_secs: None,
                    },
                    ClassAbility {
                        name: "Clarity".into(),
                        command: "/cast 7".into(),
                        cooldown_secs: 80.0,
                        priority: 2,
                        condition: None,
                        duration_secs: None,
                    },
                ],
                emergency_abilities: vec![],
                cc_abilities: vec![],
                debuff_abilities: vec![],
                rest_command: "/sit".into(),
                twist_interval_secs: None,
            },
        );
        configs
    }

    #[test]
    fn test_tracker_new_empty() {
        let tracker = BuffTracker::new();
        assert!(tracker.last_cast.is_empty());
    }

    #[test]
    fn test_record_and_check_expired() {
        let mut tracker = BuffTracker::new();
        tracker.record_cast(100, "Haste", 10);

        // Not expired at tick 50 (duration 100)
        assert!(!tracker.is_expired(100, "Haste", 100, 50));
        // Expired at tick 110
        assert!(tracker.is_expired(100, "Haste", 100, 110));
    }

    #[test]
    fn test_never_cast_is_expired() {
        let tracker = BuffTracker::new();
        assert!(tracker.is_expired(100, "Haste", 100, 0));
    }

    #[test]
    fn test_remaining() {
        let mut tracker = BuffTracker::new();
        tracker.record_cast(100, "Haste", 10);

        assert_eq!(tracker.remaining(100, "Haste", 100, 50), 60);
        assert_eq!(tracker.remaining(100, "Haste", 100, 110), 0);
    }

    #[test]
    fn test_remaining_never_cast() {
        let tracker = BuffTracker::new();
        assert_eq!(tracker.remaining(100, "Haste", 100, 50), 0);
    }

    #[test]
    fn test_check_buffs_idle_generates_commands() {
        let tracker = BuffTracker::new(); // All expired (never cast)
        let members = test_members();
        let configs = test_class_configs();

        let cmds = check_buffs(&tracker, &members, &configs, 0, &CampState::Idle);
        // Should have commands for expired buffs
        assert!(!cmds.is_empty());
    }

    #[test]
    fn test_check_buffs_fighting_returns_empty() {
        let tracker = BuffTracker::new();
        let members = test_members();
        let configs = test_class_configs();

        let cmds = check_buffs(
            &tracker,
            &members,
            &configs,
            0,
            &CampState::Fighting { started_tick: 0 },
        );
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_check_buffs_medding_generates_commands() {
        let tracker = BuffTracker::new();
        let members = test_members();
        let configs = test_class_configs();

        let cmds = check_buffs(
            &tracker,
            &members,
            &configs,
            0,
            &CampState::Medding { started_tick: 0 },
        );
        assert!(!cmds.is_empty());
    }

    #[test]
    fn test_check_buffs_not_expired_no_commands() {
        let mut tracker = BuffTracker::new();
        let members = test_members();
        let configs = test_class_configs();

        // Record all buffs as recently cast
        for member in &members {
            tracker.record_cast(member.pid, "Symbol of Naltron", 0);
            tracker.record_cast(member.pid, "Haste", 0);
            tracker.record_cast(member.pid, "Clarity", 0);
        }

        // Check at tick 10 — nothing should be expired yet
        let cmds = check_buffs(&tracker, &members, &configs, 10, &CampState::Idle);
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_buff_priority_ordering() {
        assert!(buff_priority("Haste") < buff_priority("Symbol of Naltron"));
        assert!(buff_priority("Symbol of Naltron") < buff_priority("Clarity"));
        assert!(buff_priority("Clarity") < buff_priority("Strength"));
    }

    #[test]
    fn test_haste_buffed_before_hp() {
        let tracker = BuffTracker::new(); // All expired
        let members = test_members();
        let configs = test_class_configs();

        let cmds = check_buffs(&tracker, &members, &configs, 0, &CampState::Idle);

        // Find first haste command and first symbol command
        let first_haste = cmds.iter().position(|(_, cmd)| cmd == "/cast 6");
        let first_symbol = cmds.iter().position(|(_, cmd)| cmd == "/cast 4");

        if let (Some(h), Some(s)) = (first_haste, first_symbol) {
            assert!(h < s, "Haste should come before HP buff");
        }
    }

    #[test]
    fn test_check_buffs_pulling_returns_empty() {
        let tracker = BuffTracker::new();
        let members = test_members();
        let configs = test_class_configs();

        let cmds = check_buffs(
            &tracker,
            &members,
            &configs,
            0,
            &CampState::Pulling { started_tick: 0 },
        );
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_record_cast_overwrites_previous() {
        let mut tracker = BuffTracker::new();
        tracker.record_cast(100, "Haste", 10);
        tracker.record_cast(100, "Haste", 50);
        // Should use the later cast time
        assert!(!tracker.is_expired(100, "Haste", 100, 100));
        assert_eq!(tracker.remaining(100, "Haste", 100, 100), 50);
    }

    #[test]
    fn test_different_members_tracked_separately() {
        let mut tracker = BuffTracker::new();
        tracker.record_cast(100, "Haste", 10);
        tracker.record_cast(101, "Haste", 50);
        assert_eq!(tracker.remaining(100, "Haste", 100, 60), 50);
        assert_eq!(tracker.remaining(101, "Haste", 100, 60), 90);
    }

    #[test]
    fn test_different_buffs_tracked_separately() {
        let mut tracker = BuffTracker::new();
        tracker.record_cast(100, "Haste", 10);
        tracker.record_cast(100, "Clarity", 50);
        assert_eq!(tracker.remaining(100, "Haste", 100, 60), 50);
        assert_eq!(tracker.remaining(100, "Clarity", 100, 60), 90);
    }

    #[test]
    fn test_buff_priority_alacrity_is_haste() {
        assert_eq!(buff_priority("Alacrity"), PRIORITY_HASTE);
    }

    #[test]
    fn test_buff_priority_aegolism_is_hp() {
        assert_eq!(buff_priority("Aegolism"), PRIORITY_HP);
    }

    #[test]
    fn test_buff_priority_kei_is_mana_regen() {
        assert_eq!(buff_priority("KEI"), PRIORITY_MANA_REGEN);
    }

    #[test]
    fn test_buff_priority_unknown_is_stat() {
        assert_eq!(buff_priority("Shield of the Magi"), PRIORITY_STAT);
    }

    #[test]
    fn test_expired_at_exact_duration_boundary() {
        let mut tracker = BuffTracker::new();
        tracker.record_cast(100, "Haste", 0);
        // At exactly duration ticks, should be expired
        assert!(tracker.is_expired(100, "Haste", 100, 100));
        // One tick before, not expired
        assert!(!tracker.is_expired(100, "Haste", 100, 99));
    }

    #[test]
    fn test_remaining_at_exact_cast_time() {
        let mut tracker = BuffTracker::new();
        tracker.record_cast(100, "Haste", 50);
        assert_eq!(tracker.remaining(100, "Haste", 100, 50), 100);
    }

    #[test]
    fn test_check_buffs_no_class_config_for_role() {
        let tracker = BuffTracker::new();
        let members = vec![CampMember::new(100, "Warrior01".into(), Role::Tank)];
        let configs = HashMap::new(); // No configs at all
        let cmds = check_buffs(&tracker, &members, &configs, 0, &CampState::Idle);
        assert!(cmds.is_empty());
    }

    #[test]
    fn test_check_buffs_empty_members() {
        let tracker = BuffTracker::new();
        let configs = test_class_configs();
        let cmds = check_buffs(&tracker, &[], &configs, 0, &CampState::Idle);
        assert!(cmds.is_empty());
    }
}
