//! Buff maintenance — track buff durations and queue rebuffs during downtime.

use std::collections::{HashMap, HashSet};

use textquest_common::combat::{BuffCategory, BuffInfo};

use crate::camp::{
    class_config::ClassConfig,
    state::{CampMember, CampState, Role},
};

/// Priority ordering for buff types (lower = higher priority).
const PRIORITY_HASTE: u8 = 0;
const PRIORITY_HP: u8 = 1;
const PRIORITY_MANA_REGEN: u8 = 2;
const PRIORITY_STAT: u8 = 3;

/// Tracks when each buff was last cast per member.
pub struct BuffTracker {
    /// (pid, `buff_name`) -> `last_cast_tick`
    pub last_cast: HashMap<(u32, String), u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffCheckResult {
    /// Buff is not present and can be cast.
    ShouldCast,
    /// The requested buff is already present on the target.
    AlreadyActive,
    /// A per-character blocked-buff rule skipped this cast.
    Blocked,
    /// A normal slot-spell conflict failed the cast.
    StacksFailed,
    /// A triggered-spell-slot conflict failed the cast.
    TriggerStacksFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuffSlot {
    category: BuffCategory,
    slot_index: usize,
}

#[derive(Debug, Clone, Copy)]
struct ParsedBuffCheck {
    stack_spells: &'static [i32],
    trigger_spells: &'static [i32],
}

/// Parse a buffcheck condition expression such as:
/// - `buffcheck(1234)`
/// - `buffcheck(1234,5678)`
/// - `buffcheck(1234,5678|9012)`
fn parse_buffcheck(condition: &str) -> Option<ParsedBuffCheck> {
    let trimmed = condition.trim();
    let lowered = trimmed.to_ascii_lowercase();
    let prefix = "buffcheck(";

    if !lowered.starts_with(prefix) || !trimmed.ends_with(')') {
        return None;
    }

    let Some(inner) = trimmed
        .get(prefix.len()..trimmed.len().saturating_sub(1))
    else {
        return None;
    };

    let parse_list = |raw: &str| -> Option<Vec<i32>> {
        if raw.trim().is_empty() {
            return Some(Vec::new());
        }

        raw.split(',')
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(|token| token.parse().ok())
            .collect::<Option<Vec<i32>>>()
            .map(|mut ids| {
                ids.sort_unstable();
                ids.dedup();
                ids
            })
    };

    let (stack_text, trigger_text) = inner
        .split_once('|')
        .map_or((inner, ""), |(stack_text, trigger_text)| {
            (stack_text, trigger_text)
        });

    let stack_spells = parse_list(stack_text)?;
    let trigger_spells = parse_list(trigger_text)?;

    if stack_spells.is_empty() && trigger_spells.is_empty() {
        return None;
    }

    Some(ParsedBuffCheck {
        stack_spells: Box::leak(stack_spells.into_boxed_slice()),
        trigger_spells: Box::leak(trigger_spells.into_boxed_slice()),
    })
}

fn buff_slots(active_buffs: &[BuffInfo]) -> HashMap<i32, BuffSlot> {
    let mut slots = HashMap::new();
    for buff in active_buffs {
        slots.entry(buff.spell_id).or_insert(BuffSlot {
            category: buff.category,
            slot_index: buff.slot_index,
        });
    }
    slots
}

fn buff_slots_conflict(active_buffs: &[BuffInfo], slot_map: &HashMap<i32, BuffSlot>, checks: &[i32]) -> bool {
    for active in active_buffs {
        for check_spell in checks {
            let Some(check_slot) = slot_map.get(check_spell) else {
                continue;
            };

            if active.spell_id == *check_spell {
                continue;
            }

            if active.category == check_slot.category && active.slot_index == check_slot.slot_index {
                return true;
            }
        }
    }
    false
}

fn choose_candidates(spell_id: i32, stack_spells: &[i32]) -> Vec<i32> {
    if stack_spells.is_empty() {
        vec![spell_id]
    } else {
        stack_spells.to_vec()
    }
}

fn check_buffs_with_slots(
    spell_id: i32,
    stack_spells: &[i32],
    trigger_spells: &[i32],
    active_buffs: &[BuffInfo],
    blocked_buffs: &[i32],
) -> BuffCheckResult {
    let blocked: HashSet<i32> = blocked_buffs.iter().copied().collect();
    let candidates = choose_candidates(spell_id, stack_spells);

    if candidates.iter().any(|id| blocked.contains(id)) {
        return BuffCheckResult::Blocked;
    }

    if candidates.iter().any(|id| active_buffs.iter().any(|buff| buff.spell_id == *id)) {
        return BuffCheckResult::AlreadyActive;
    }

    let slot_map = buff_slots(active_buffs);
    if buff_slots_conflict(active_buffs, &slot_map, &candidates) {
        return BuffCheckResult::StacksFailed;
    }

    if buff_slots_conflict(active_buffs, &slot_map, trigger_spells) {
        return BuffCheckResult::TriggerStacksFailed;
    }

    BuffCheckResult::ShouldCast
}

/// Evaluate a local-target buffcheck against live buff data.
#[must_use]
pub fn local_buff_check(
    spell_id: i32,
    active_buffs: &[BuffInfo],
    blocked_buffs: &[i32],
    stack_spells: &[i32],
    trigger_spells: &[i32],
) -> BuffCheckResult {
    check_buffs_with_slots(spell_id, stack_spells, trigger_spells, active_buffs, blocked_buffs)
}

/// Evaluate a pet-target buffcheck against live pet buff data.
#[must_use]
pub fn local_pet_buff_check(
    spell_id: i32,
    active_buffs: &[BuffInfo],
    blocked_buffs: &[i32],
    stack_spells: &[i32],
    trigger_spells: &[i32],
) -> BuffCheckResult {
    check_buffs_with_slots(spell_id, stack_spells, trigger_spells, active_buffs, blocked_buffs)
}

/// Backwards-compatible alias for self-buff checks.
#[must_use]
pub fn self_buff_check(
    spell_id: i32,
    active_buffs: &[BuffInfo],
    blocked_buffs: &[i32],
    stack_spells: &[i32],
    trigger_spells: &[i32],
) -> BuffCheckResult {
    local_buff_check(spell_id, active_buffs, blocked_buffs, stack_spells, trigger_spells)
}

/// Backwards-compatible alias for group-member checks.
#[must_use]
pub fn group_buff_check(
    spell_id: i32,
    active_buffs: &[BuffInfo],
    blocked_buffs: &[i32],
    stack_spells: &[i32],
    trigger_spells: &[i32],
) -> BuffCheckResult {
    local_buff_check(spell_id, active_buffs, blocked_buffs, stack_spells, trigger_spells)
}

impl BuffTracker {
    /// Creates an empty buff tracker with no recorded casts.
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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

/// Check all members for expired buffs and return `(caster_pid, command)`
/// pairs.
///
/// Only returns rebuff commands when the camp is in Idle or Medding state.
/// Buffs are prioritized: haste > HP buff > mana regen > stat buffs.
#[must_use]
pub fn check_buffs(
    tracker: &BuffTracker,
    members: &[CampMember],
    class_configs: &HashMap<String, ClassConfig>,
    current_tick: u64,
    camp_state: &CampState,
) -> Vec<(u32, String)> {
    check_buffs_with_context(
        tracker,
        members,
        class_configs,
        current_tick,
        camp_state,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
    )
}

/// Like [`check_buffs`], but uses live member and pet buffs for stack-aware
/// checks and blocked lists keyed by character name.
pub fn check_buffs_with_context(
    tracker: &BuffTracker,
    members: &[CampMember],
    class_configs: &HashMap<String, ClassConfig>,
    current_tick: u64,
    camp_state: &CampState,
    live_buffs_by_member: &HashMap<u32, Vec<BuffInfo>>,
    _live_pet_buffs_by_member: &HashMap<u32, Vec<BuffInfo>>,
    blocked_buffs_by_member: &HashMap<String, Vec<i32>>,
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
            let profile = config.profile_for_level(member.level);

            for ability in &profile.buff_abilities {
                // Each buff caster buffs all members
                // Use explicit buff duration if set, else fall back to cooldown
            let duration_ticks = ability.effective_duration_secs() as u64;
            let condition = ability.condition.as_deref();
            let buff_check = condition
                .and_then(parse_buffcheck)
                .filter(|_| live_buffs_by_member.contains_key(&target.pid));

            for target in members {
                let can_cast = match &buff_check {
                    Some(parsed) => {
                        let active_buffs = live_buffs_by_member.get(&target.pid).map_or(&[][..], |v| v.as_slice());
                        let blocked_buffs = blocked_buffs_by_member
                            .get(&target.name)
                            .map_or(&[][..], |v| v.as_slice());

                        matches!(
                            group_buff_check(
                                ability.name.len() as i32,
                                active_buffs,
                                blocked_buffs,
                                parsed.stack_spells,
                                parsed.trigger_spells
                            ),
                            BuffCheckResult::ShouldCast
                        )
                    }
                    None => tracker.is_expired(target.pid, &ability.name, duration_ticks, current_tick),
                };

                if can_cast {
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
    use crate::camp::class_config::{AbilityProfileOverride, ClassAbility, ClassConfig};

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
                level_overrides: Vec::new(),
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
                level_overrides: Vec::new(),
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

    #[test]
    fn test_check_buffs_uses_level_override_profile() {
        let tracker = BuffTracker::new();
        let members = vec![
            CampMember::new(100, "Warrior01".into(), Role::Tank),
            CampMember::new(101, "Cleric01".into(), Role::Healer).with_level(15),
        ];

        let base_buff = ClassAbility {
            name: "Base Buff".into(),
            command: "/cast 1".into(),
            cooldown_secs: 120.0,
            priority: 1,
            condition: None,
            duration_secs: None,
        };

        let override_buff = ClassAbility {
            name: "Low Level Buff".into(),
            command: "/cast 2".into(),
            cooldown_secs: 90.0,
            priority: 1,
            condition: None,
            duration_secs: None,
        };

        let mut configs = HashMap::new();
        configs.insert(
            "healer".into(),
            ClassConfig {
                class_name: "cleric".into(),
                role: "healer".into(),
                level_overrides: vec![AbilityProfileOverride {
                    name: "low-range".into(),
                    min_level: Some(1),
                    max_level: Some(20),
                    buff_abilities: Some(vec![override_buff.clone()]),
                    ..AbilityProfileOverride::default()
                }],
                combat_abilities: vec![],
                buff_abilities: vec![base_buff],
                emergency_abilities: vec![],
                cc_abilities: vec![],
                debuff_abilities: vec![],
                rest_command: "/sit".into(),
                twist_interval_secs: None,
            },
        );

        let commands = check_buffs(&tracker, &members, &configs, 0, &CampState::Idle);
        assert!(
            commands.iter().any(|(_, cmd)| cmd == "/cast 2"),
            "override buff should be used when level matches"
        );
        assert!(
            !commands.iter().any(|(_, cmd)| cmd == "/cast 1"),
            "base buff should not be used when override applies"
        );
    }
}
