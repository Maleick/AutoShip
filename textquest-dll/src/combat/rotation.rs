//! Data-driven rotation engine — executes ordered lists of actions with
//! conditions.
//!
//! Modeled after the rgmercs rotation system: each class defines named rotation
//! groups (Downtime, Combat, Emergency, Burn, etc.) with per-entry conditions.
//! The engine iterates groups in priority order, executing entries that pass
//! their conditions, respecting step limits per frame.

use textquest_common::combat::{ActionType, CombatStateReq, ConditionExpr, TargetSelector};

use super::strategy::CombatContext;

// ---------------------------------------------------------------------------
// Rotation data structures
// ---------------------------------------------------------------------------

/// A single action entry within a rotation group.
///
/// Each entry represents one ability (spell, disc, AA, etc.) with a condition
/// that gates its execution. Entries are evaluated in order; the first entry
/// whose condition passes and whose action is ready gets executed.
#[derive(Debug, Clone)]
pub struct RotationEntry {
    /// Human-readable name (matches an AbilitySet name or literal ability
    /// name).
    pub name: String,
    /// What type of action to perform.
    pub action_type: ActionType,
    /// Condition that must be true for this entry to fire.
    /// `None` means unconditional (always attempt).
    pub condition: Option<ConditionExpr>,
    /// Check if this effect is already active (prevents re-casting).
    /// If set and returns true, the entry is skipped.
    pub active_condition: Option<ConditionExpr>,
    /// Hook to run before action executes (e.g., stop movement, face target).
    pub pre_activate: Option<ActivationHook>,
    /// Hook to run after action completes (e.g., log result, update tracker).
    pub post_activate: Option<ActivationHook>,
    /// Whether the user has enabled this entry (togglable at runtime).
    pub enabled: bool,
}

/// Hook action to execute before or after a rotation entry fires.
#[derive(Debug, Clone)]
pub enum ActivationHook {
    /// Stop all movement before casting.
    StopMovement,
    /// Face the current target.
    FaceTarget,
    /// Log a message (for debugging / audit trail).
    Log(String),
    /// Execute multiple hooks in sequence.
    Chain(Vec<ActivationHook>),
}

/// A named group of rotation entries with shared execution conditions.
///
/// Rotation groups are evaluated in the order defined by the class strategy.
/// Each group targets a specific set of entities (self, auto-target, lowest HP
/// member, etc.) and only runs when its combat state requirement is met.
#[derive(Debug, Clone)]
pub struct RotationGroup {
    /// Name of this rotation group (e.g., "Downtime", "Combat", "Emergency").
    pub name: String,
    /// How to select targets for entries in this group.
    pub target_selector: TargetSelector,
    /// What combat state is required for this group to run.
    pub combat_state_req: CombatStateReq,
    /// Maximum entries to execute from this group per frame.
    /// 0 means unlimited (execute all passing entries).
    pub steps_per_frame: u8,
    /// If true, always restart from entry 0 each frame (used by bard weaving).
    /// If false, resume from where the last frame left off (round-robin).
    pub full_rotation: bool,
    /// Optional HP threshold — group only runs when player HP is below this.
    /// Used for emergency rotations. `None` means no HP gate.
    pub hp_threshold: Option<f32>,
    /// The rotation entries in priority order.
    pub entries: Vec<RotationEntry>,
    /// Current position for round-robin resumption (runtime state, not config).
    pub current_step: usize,
}

// ---------------------------------------------------------------------------
// Hook execution
// ---------------------------------------------------------------------------

/// Execute an activation hook. Hooks are fire-and-forget side effects.
fn run_hook(hook: &ActivationHook, _ctx: &CombatContext) {
    match hook {
        ActivationHook::StopMovement => {
            tracing::debug!("Hook: stop movement");
            // Will call nav::stop() when wired to the nav FSM.
        }
        ActivationHook::FaceTarget => {
            tracing::debug!("Hook: face target");
            // Will call movement::face_target() when wired.
        }
        ActivationHook::Log(msg) => {
            tracing::info!("Rotation hook: {}", msg);
        }
        ActivationHook::Chain(hooks) => {
            for h in hooks {
                run_hook(h, _ctx);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Condition evaluation
// ---------------------------------------------------------------------------

/// Evaluate a `ConditionExpr` against the current combat context.
pub fn evaluate_condition(expr: &ConditionExpr, ctx: &CombatContext) -> bool {
    match expr {
        ConditionExpr::Always => true,
        ConditionExpr::HpBelow(threshold) => ctx.player.hp_pct() < *threshold,
        ConditionExpr::ManaBelow(threshold) => ctx.player.mana_pct() < *threshold,
        ConditionExpr::TargetHpAbove(threshold) => {
            ctx.target.is_some_and(|t| t.hp_pct() > *threshold)
        }
        ConditionExpr::TargetHpBelow(threshold) => {
            ctx.target.is_some_and(|t| t.hp_pct() < *threshold)
        }
        ConditionExpr::ManaAbove(threshold) => ctx.player.mana_pct() > *threshold,
        ConditionExpr::AggroOnMe => {
            // Check if we're in combat with an NPC target (spawn_type == 1).
            ctx.in_combat && ctx.target.is_some_and(|t| t.spawn_type == 1)
        }
        ConditionExpr::InCombat => ctx.in_combat,
        ConditionExpr::OutOfCombat => !ctx.in_combat,
        ConditionExpr::BuffActive(spell_id) => ctx.active_buffs.contains(spell_id),
        ConditionExpr::BuffMissing(spell_id) => !ctx.active_buffs.contains(spell_id),
        ConditionExpr::BuffExpiringSoon(spell_id, threshold_secs) => ctx
            .buff_info
            .iter()
            .any(|b| b.spell_id == *spell_id && b.expires_within(*threshold_secs)),
        ConditionExpr::TargetDistanceBelow(range) => ctx.target.is_some_and(|t| {
            let dx = t.x - ctx.player.x;
            let dy = t.y - ctx.player.y;
            let dz = t.z - ctx.player.z;
            (dx * dx + dy * dy + dz * dz).sqrt() < *range
        }),
        ConditionExpr::EnemyCountAbove(count) => ctx.nearby_enemies.len() as u32 >= *count,
        ConditionExpr::TargetMezzed => {
            // Check if the target has a mez buff active.
            // Uses target_debuffs from CombatContext.
            ctx.target_is_mezzed
        }
        ConditionExpr::TargetNotMezzed => !ctx.target_is_mezzed,
        ConditionExpr::Not(inner) => !evaluate_condition(inner, ctx),
        ConditionExpr::And(exprs) => exprs.iter().all(|e| evaluate_condition(e, ctx)),
        ConditionExpr::Or(exprs) => exprs.iter().any(|e| evaluate_condition(e, ctx)),
        ConditionExpr::XTargetHaterCountAbove(count) => ctx
            .extended_targets
            .is_some_and(|xt| xt.hater_count() as u32 >= *count),
        ConditionExpr::HasXTargetAggro => {
            ctx.extended_targets.is_some_and(|xt| xt.hater_count() > 0)
        }
    }
}

// ---------------------------------------------------------------------------
// Rotation execution engine
// ---------------------------------------------------------------------------

/// Result of executing one rotation group for one frame.
#[derive(Debug, Clone)]
pub struct RotationResult {
    /// Actions selected for execution this frame (name + action type).
    pub actions: Vec<SelectedAction>,
}

/// A single action selected by the rotation engine for execution.
#[derive(Debug, Clone)]
pub struct SelectedAction {
    /// Name of the rotation entry that produced this action.
    pub entry_name: String,
    /// The action to perform.
    pub action_type: ActionType,
    /// Target spawn ID (0 = self).
    pub target_id: u32,
}

/// Execute a single rotation group for one frame, returning selected actions.
///
/// The engine iterates entries starting from `group.current_step` (or 0 if
/// `full_rotation`), testing each entry's condition. Entries that pass get
/// added to the result up to `steps_per_frame`. The group's `current_step`
/// is advanced for next frame.
pub fn execute_group(group: &mut RotationGroup, ctx: &CombatContext) -> RotationResult {
    let mut result = RotationResult {
        actions: Vec::new(),
    };

    if group.entries.is_empty() {
        return result;
    }

    // Check combat state requirement
    let in_combat = ctx.in_combat;
    match group.combat_state_req {
        CombatStateReq::Combat => {
            if !in_combat {
                return result;
            }
        }
        CombatStateReq::Downtime => {
            if in_combat {
                return result;
            }
        }
        CombatStateReq::Any => {}
    }

    // Check HP threshold
    if let Some(hp_thresh) = group.hp_threshold {
        if ctx.player.hp_pct() > hp_thresh {
            return result;
        }
    }

    // Determine target ID based on selector
    let target_id = select_target(&group.target_selector, ctx);

    // Determine start position
    let start = if group.full_rotation {
        0
    } else {
        group.current_step
    };

    let max_steps = if group.steps_per_frame == 0 {
        group.entries.len()
    } else {
        group.steps_per_frame as usize
    };

    let entry_count = group.entries.len();
    let mut steps_taken = 0;
    let mut last_idx = start;

    for offset in 0..entry_count {
        if steps_taken >= max_steps {
            break;
        }

        let idx = (start + offset) % entry_count;
        last_idx = idx;

        let entry = &group.entries[idx];

        // Skip disabled entries
        if !entry.enabled {
            continue;
        }

        // Evaluate main condition
        let pass = entry
            .condition
            .as_ref()
            .is_none_or(|c| evaluate_condition(c, ctx));

        if !pass {
            continue;
        }

        // Check active_condition — skip if effect is already active
        let already_active = entry
            .active_condition
            .as_ref()
            .is_some_and(|c| evaluate_condition(c, ctx));

        if already_active {
            continue;
        }

        // Run pre-activation hook
        if let Some(ref hook) = entry.pre_activate {
            run_hook(hook, ctx);
        }

        result.actions.push(SelectedAction {
            entry_name: entry.name.clone(),
            action_type: entry.action_type.clone(),
            target_id,
        });

        // Run post-activation hook
        if let Some(ref hook) = entry.post_activate {
            run_hook(hook, ctx);
        }

        steps_taken += 1;
    }

    // Advance current_step for next frame (skip past what we just processed)
    if !group.full_rotation {
        group.current_step = (last_idx + 1) % entry_count;
    }

    result
}

/// Execute all rotation groups in order, returning the first non-empty result.
///
/// This is the main entry point for the rotation system. Groups are evaluated
/// in priority order. The first group that produces at least one action wins.
/// This matches rgmercs behavior where higher-priority rotations (emergency,
/// hate tools) preempt lower-priority ones (combat, burn).
pub fn execute_rotations(
    groups: &mut [RotationGroup],
    ctx: &CombatContext,
) -> Option<SelectedAction> {
    for group in groups.iter_mut() {
        let result = execute_group(group, ctx);
        if let Some(action) = result.actions.into_iter().next() {
            return Some(action);
        }
    }
    None
}

/// Resolve target ID based on a `TargetSelector` and current context.
fn select_target(selector: &TargetSelector, ctx: &CombatContext) -> u32 {
    match selector {
        TargetSelector::SelfOnly => ctx.player.spawn_id,
        TargetSelector::AutoTarget => ctx.target.map_or(0, |t| t.spawn_id),
        TargetSelector::AggroTarget => {
            // For now, fall back to auto-target. Full aggro scanning
            // will be implemented in issue #456.
            ctx.target.map_or(0, |t| t.spawn_id)
        }
        TargetSelector::LowestHpGroupMember => {
            super::strategy::lowest_hp_member(ctx).map_or(ctx.player.spawn_id, |(id, _)| id)
        }
    }
}

// ---------------------------------------------------------------------------
// Builder helpers for class strategies
// ---------------------------------------------------------------------------

/// Convenience builder for rotation entries.
pub fn entry(name: &str, action_type: ActionType) -> RotationEntry {
    RotationEntry {
        name: name.to_string(),
        action_type,
        condition: None,
        active_condition: None,
        pre_activate: None,
        post_activate: None,
        enabled: true,
    }
}

/// Convenience: create an entry with a condition.
pub fn entry_if(name: &str, action_type: ActionType, cond: ConditionExpr) -> RotationEntry {
    RotationEntry {
        name: name.to_string(),
        action_type,
        condition: Some(cond),
        active_condition: None,
        pre_activate: None,
        post_activate: None,
        enabled: true,
    }
}

/// Convenience: create an entry that skips when a buff is already active.
pub fn entry_unless_active(
    name: &str,
    action_type: ActionType,
    active_check: ConditionExpr,
) -> RotationEntry {
    RotationEntry {
        name: name.to_string(),
        action_type,
        condition: None,
        active_condition: Some(active_check),
        pre_activate: None,
        post_activate: None,
        enabled: true,
    }
}

/// Convenience builder for rotation groups.
pub fn group(name: &str, target: TargetSelector, state: CombatStateReq) -> RotationGroup {
    RotationGroup {
        name: name.to_string(),
        target_selector: target,
        combat_state_req: state,
        steps_per_frame: 1,
        full_rotation: false,
        hp_threshold: None,
        entries: Vec::new(),
        current_step: 0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use textquest_common::{
        combat::{ActionType, CombatConfig, ConditionExpr},
        types::SpawnData,
    };

    fn make_ctx(
        _in_combat: bool,
        player_hp: f32,
        player_mana: f32,
    ) -> (SpawnData, SpawnData, CombatConfig) {
        let mut player = SpawnData::default();
        player.hp_current = (player_hp * 100.0) as i64;
        player.hp_max = 10000;
        player.mana_current = (player_mana * 100.0) as i32;
        player.mana_max = 10000;
        let target = SpawnData {
            spawn_id: 42,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        (player, target, config)
    }

    fn build_ctx<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        config: &'a CombatConfig,
        in_combat: bool,
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: &[],
            group_members: &[],
            config,
            tick: 0,
            in_combat,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        }
    }

    #[test]
    fn empty_group_returns_no_actions() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let mut g = group("Empty", TargetSelector::AutoTarget, CombatStateReq::Combat);
        let result = execute_group(&mut g, &ctx);
        assert!(result.actions.is_empty());
    }

    #[test]
    fn unconditional_entry_fires() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let mut g = group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
        g.entries
            .push(entry("Taunt", ActionType::Ability("Taunt".into())));
        let result = execute_group(&mut g, &ctx);
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].entry_name, "Taunt");
        assert_eq!(result.actions[0].target_id, 42);
    }

    #[test]
    fn combat_state_req_filters_downtime() {
        let (player, target, config) = make_ctx(false, 80.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, false);
        let mut g = group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
        g.entries
            .push(entry("Nuke", ActionType::Spell("Nuke".into())));
        let result = execute_group(&mut g, &ctx);
        assert!(
            result.actions.is_empty(),
            "Combat rotation should not run during downtime"
        );
    }

    #[test]
    fn downtime_state_req_runs_out_of_combat() {
        let (player, _, config) = make_ctx(false, 80.0, 80.0);
        let ctx = build_ctx(&player, None, &config, false);
        let mut g = group(
            "Downtime",
            TargetSelector::SelfOnly,
            CombatStateReq::Downtime,
        );
        g.entries
            .push(entry("SelfBuff", ActionType::Spell("Shield".into())));
        let result = execute_group(&mut g, &ctx);
        assert_eq!(result.actions.len(), 1);
    }

    #[test]
    fn hp_threshold_gates_group() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let mut g = group(
            "Emergency",
            TargetSelector::AutoTarget,
            CombatStateReq::Combat,
        );
        g.hp_threshold = Some(30.0); // only run when HP < 30%
        g.entries
            .push(entry("Defensive", ActionType::Disc("Defensive".into())));
        let result = execute_group(&mut g, &ctx);
        assert!(
            result.actions.is_empty(),
            "Emergency should not fire at 80% HP"
        );
    }

    #[test]
    fn hp_threshold_fires_when_low() {
        let (player, target, config) = make_ctx(true, 20.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let mut g = group(
            "Emergency",
            TargetSelector::AutoTarget,
            CombatStateReq::Combat,
        );
        g.hp_threshold = Some(30.0);
        g.entries
            .push(entry("Defensive", ActionType::Disc("Defensive".into())));
        let result = execute_group(&mut g, &ctx);
        assert_eq!(result.actions.len(), 1);
    }

    #[test]
    fn condition_gates_entry() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let mut g = group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
        g.steps_per_frame = 2;
        g.entries.push(entry_if(
            "LowHpHeal",
            ActionType::Spell("Heal".into()),
            ConditionExpr::HpBelow(30.0), // won't pass at 80% HP
        ));
        g.entries
            .push(entry("Nuke", ActionType::Spell("Nuke".into())));
        let result = execute_group(&mut g, &ctx);
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].entry_name, "Nuke");
    }

    #[test]
    fn steps_per_frame_limits_actions() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let mut g = group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
        g.steps_per_frame = 1;
        g.entries
            .push(entry("Ability1", ActionType::Ability("Kick".into())));
        g.entries
            .push(entry("Ability2", ActionType::Ability("Bash".into())));
        let result = execute_group(&mut g, &ctx);
        assert_eq!(
            result.actions.len(),
            1,
            "Should only execute 1 step per frame"
        );
    }

    #[test]
    fn round_robin_resumes_from_last_position() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);

        let mut g = group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
        g.steps_per_frame = 1;
        g.entries.push(entry("A", ActionType::Ability("A".into())));
        g.entries.push(entry("B", ActionType::Ability("B".into())));
        g.entries.push(entry("C", ActionType::Ability("C".into())));

        // Frame 1: should pick A
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let result = execute_group(&mut g, &ctx);
        assert_eq!(result.actions[0].entry_name, "A");

        // Frame 2: should pick B (resumed from position 1)
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let result = execute_group(&mut g, &ctx);
        assert_eq!(result.actions[0].entry_name, "B");

        // Frame 3: should pick C
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let result = execute_group(&mut g, &ctx);
        assert_eq!(result.actions[0].entry_name, "C");

        // Frame 4: wraps around to A
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let result = execute_group(&mut g, &ctx);
        assert_eq!(result.actions[0].entry_name, "A");
    }

    #[test]
    fn full_rotation_restarts_each_frame() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);

        let mut g = group("Songs", TargetSelector::AutoTarget, CombatStateReq::Combat);
        g.steps_per_frame = 1;
        g.full_rotation = true;
        g.entries
            .push(entry("Song1", ActionType::Song("WarMarch".into())));
        g.entries
            .push(entry("Song2", ActionType::Song("Aria".into())));

        // Frame 1: picks Song1
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let result = execute_group(&mut g, &ctx);
        assert_eq!(result.actions[0].entry_name, "Song1");

        // Frame 2: full_rotation=true means restart from 0, picks Song1 again
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let result = execute_group(&mut g, &ctx);
        assert_eq!(result.actions[0].entry_name, "Song1");
    }

    #[test]
    fn disabled_entries_skipped() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let mut g = group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
        g.steps_per_frame = 2;
        let mut disabled = entry("Disabled", ActionType::Ability("X".into()));
        disabled.enabled = false;
        g.entries.push(disabled);
        g.entries
            .push(entry("Enabled", ActionType::Ability("Y".into())));
        let result = execute_group(&mut g, &ctx);
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].entry_name, "Enabled");
    }

    #[test]
    fn execute_rotations_returns_first_non_empty() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, true);

        let mut groups = vec![
            {
                // Emergency — won't fire (HP too high)
                let mut g = group(
                    "Emergency",
                    TargetSelector::AutoTarget,
                    CombatStateReq::Combat,
                );
                g.hp_threshold = Some(30.0);
                g.entries
                    .push(entry("Defensive", ActionType::Disc("Def".into())));
                g
            },
            {
                // Combat — will fire
                let mut g = group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
                g.entries
                    .push(entry("Nuke", ActionType::Spell("Nuke".into())));
                g
            },
        ];

        let action = execute_rotations(&mut groups, &ctx);
        assert!(action.is_some());
        assert_eq!(action.unwrap().entry_name, "Nuke");
    }

    #[test]
    fn execute_rotations_prefers_earlier_group() {
        let (player, target, config) = make_ctx(true, 20.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, true);

        let mut groups = vec![
            {
                // Emergency — will fire (HP low)
                let mut g = group(
                    "Emergency",
                    TargetSelector::AutoTarget,
                    CombatStateReq::Combat,
                );
                g.hp_threshold = Some(30.0);
                g.entries
                    .push(entry("Defensive", ActionType::Disc("Def".into())));
                g
            },
            {
                // Combat — would fire but emergency takes priority
                let mut g = group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
                g.entries
                    .push(entry("Nuke", ActionType::Spell("Nuke".into())));
                g
            },
        ];

        let action = execute_rotations(&mut groups, &ctx);
        assert_eq!(action.unwrap().entry_name, "Defensive");
    }

    #[test]
    fn self_target_selector() {
        let (player, target, config) = make_ctx(false, 80.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, false);
        let mut g = group("Buffs", TargetSelector::SelfOnly, CombatStateReq::Downtime);
        g.entries
            .push(entry("SelfBuff", ActionType::Spell("Aegolism".into())));
        let result = execute_group(&mut g, &ctx);
        assert_eq!(result.actions[0].target_id, player.spawn_id);
    }

    #[test]
    fn lowest_hp_target_selector() {
        use super::super::strategy::GroupMemberState;
        let (player, _, config) = make_ctx(true, 80.0, 80.0);
        let members = vec![
            GroupMemberState {
                spawn_id: 100,
                hp_pct: 90.0,
                mana_pct: 100.0,
                class_id: 1,
                is_dead: false,
                name: "Tank".into(),
                has_detrimental: false,
            },
            GroupMemberState {
                spawn_id: 101,
                hp_pct: 30.0,
                mana_pct: 100.0,
                class_id: 7,
                is_dead: false,
                name: "Monk".into(),
                has_detrimental: false,
            },
        ];
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        let mut g = group(
            "Heals",
            TargetSelector::LowestHpGroupMember,
            CombatStateReq::Combat,
        );
        g.entries
            .push(entry("Heal", ActionType::Spell("CompleteHeal".into())));
        let result = execute_group(&mut g, &ctx);
        assert_eq!(
            result.actions[0].target_id, 101,
            "Should target lowest HP member"
        );
    }

    #[test]
    fn and_condition_both_must_pass() {
        let (player, _, config) = make_ctx(true, 20.0, 80.0);
        // Target with 10% HP — TargetHpAbove(50.0) should fail
        let mut target = SpawnData::default();
        target.spawn_id = 42;
        target.hp_current = 1000;
        target.hp_max = 10000;
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let mut g = group("Test", TargetSelector::AutoTarget, CombatStateReq::Combat);
        g.entries.push(entry_if(
            "BothCheck",
            ActionType::Disc("Defensive".into()),
            ConditionExpr::And(vec![
                ConditionExpr::HpBelow(30.0),       // passes at 20% HP
                ConditionExpr::TargetHpAbove(50.0), // fails — target is at 10%
            ]),
        ));
        let result = execute_group(&mut g, &ctx);
        assert!(
            result.actions.is_empty(),
            "And with one failing arm should not fire"
        );
    }

    #[test]
    fn or_condition_either_passes() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, true);
        let mut g = group("Test", TargetSelector::AutoTarget, CombatStateReq::Combat);
        g.entries.push(entry_if(
            "EitherCheck",
            ActionType::Ability("Taunt".into()),
            ConditionExpr::Or(vec![
                ConditionExpr::HpBelow(30.0), // fails at 80%
                ConditionExpr::Always,        // passes
            ]),
        ));
        let result = execute_group(&mut g, &ctx);
        assert_eq!(result.actions.len(), 1);
    }

    #[test]
    fn evaluate_condition_always() {
        let (player, _, config) = make_ctx(true, 80.0, 80.0);
        let ctx = build_ctx(&player, None, &config, true);
        assert!(evaluate_condition(&ConditionExpr::Always, &ctx));
    }

    #[test]
    fn evaluate_condition_hp_below() {
        let (player, _, config) = make_ctx(true, 20.0, 80.0);
        let ctx = build_ctx(&player, None, &config, true);
        assert!(evaluate_condition(&ConditionExpr::HpBelow(30.0), &ctx));
        assert!(!evaluate_condition(&ConditionExpr::HpBelow(10.0), &ctx));
    }

    #[test]
    fn evaluate_condition_mana_below() {
        let (player, _, config) = make_ctx(true, 80.0, 15.0);
        let ctx = build_ctx(&player, None, &config, true);
        assert!(evaluate_condition(&ConditionExpr::ManaBelow(20.0), &ctx));
        assert!(!evaluate_condition(&ConditionExpr::ManaBelow(10.0), &ctx));
    }
}
