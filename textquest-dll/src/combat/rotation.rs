//! Data-driven rotation engine — executes ordered lists of actions with
//! conditions.
//!
//! Modeled after the rgmercs rotation system: each class defines named rotation
//! groups (Downtime, Combat, Emergency, Burn, etc.) with per-entry conditions.
//! The engine iterates groups in priority order, executing entries that pass
//! their conditions, respecting step limits per frame.

use serde::{Deserialize, Serialize};
use textquest_common::combat::{ActionType, BurnState, CombatStateReq, ConditionExpr, TargetSelector};

use super::strategy::CombatContext;

// ---------------------------------------------------------------------------
// Rotation data structures
// ---------------------------------------------------------------------------

/// A single action entry within a rotation group.
///
/// Each entry represents one ability (spell, disc, AA, etc.) with a condition
/// that gates its execution. Entries are evaluated in order; the first entry
/// whose condition passes and whose action is ready gets executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Optional cooldown key used to throttle this entry between attempts.
    ///
    /// When multiple entries share the same key they also share reuse state,
    /// which lets upgraded spell lines or grouped utilities use a single timer.
    pub cooldown_key: Option<String>,
    /// Optional cooldown window in game ticks.
    ///
    /// When set, the combat runtime throttles repeat attempts for this entry.
    /// Spell entries default to target-scoped cooldown tracking so debuffs can
    /// land once per target instead of spamming every frame.
    pub cooldown_ticks: Option<u32>,
    /// Optional shared cooldown key used to model reuse lockouts shared across
    /// multiple actions.
    pub shared_cooldown_key: Option<String>,
}

/// Hook action to execute before or after a rotation entry fires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivationHook {
    /// Stop all movement before casting.
    StopMovement,
    /// Face the current target.
    FaceTarget,
    /// Log a message (for debugging / audit trail).
    Log(String),
    /// Execute multiple hooks in sequence.
    Chain(Vec<ActivationHook>),
    /// Activate a named bandolier set (weapon-loadout swap).
    /// Subject to the BandolierManager cooldown gate.
    Bandolier(String),
}

/// A named group of rotation entries with shared execution conditions.
///
/// Rotation groups are evaluated in the order defined by the class strategy.
/// Each group targets a specific set of entities (self, auto-target, lowest HP
/// member, etc.) and only runs when its combat state requirement is met.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Optional burn duration in combat ticks (only for "Burn" group).
    /// Once the burn group activates, it runs for this duration, then cooldown.
    pub burn_duration_ticks: Option<u32>,
    /// Optional burn cooldown duration in combat ticks (only for "Burn" group).
    /// After burn expires, cooldown prevents re-trigger for this duration.
    pub burn_cooldown_duration_ticks: Option<u32>,
    /// The rotation entries in priority order.
    pub entries: Vec<RotationEntry>,
    /// Current position for round-robin resumption (runtime state, not config).
    #[serde(skip)]
    pub current_step: usize,
}

/// Tracks the runtime state of the burn rotation state machine.
///
/// Manages transitions between Ready → Active → Cooldown states based on:
/// - burnnow manual trigger
/// - burn_duration (how long active phase lasts)
/// - burn_cooldown_duration (how long cooldown phase lasts)
#[derive(Debug, Clone)]
pub struct BurnRotationState {
    /// Current burn phase.
    pub state: BurnState,
    /// Tick when the current phase started (for duration tracking).
    pub phase_start_tick: u32,
    /// Tick when burn group expires (start_tick + burn_duration_ticks).
    pub burn_expiry_tick: Option<u32>,
    /// Tick when cooldown expires (burn_expiry_tick + cooldown_duration_ticks).
    pub cooldown_expiry_tick: Option<u32>,
}

impl Default for BurnRotationState {
    fn default() -> Self {
        Self {
            state: BurnState::Ready,
            phase_start_tick: 0,
            burn_expiry_tick: None,
            cooldown_expiry_tick: None,
        }
    }
}

impl BurnRotationState {
    /// Update burn state machine based on current tick and manual triggers.
    ///
    /// Returns the new BurnState to use in CombatContext.
    pub fn update(
        &mut self,
        current_tick: u32,
        burnnow_triggered: bool,
        group: &RotationGroup,
    ) -> BurnState {
        match self.state {
            BurnState::Ready => {
                if burnnow_triggered {
                    if let Some(duration) = group.burn_duration_ticks {
                        self.phase_start_tick = current_tick;
                        self.burn_expiry_tick = Some(current_tick + duration);
                        self.state = BurnState::Active;
                    }
                }
            }
            BurnState::Active => {
                if let Some(expiry) = self.burn_expiry_tick {
                    if current_tick >= expiry {
                        if let Some(cooldown_duration) = group.burn_cooldown_duration_ticks {
                            self.phase_start_tick = current_tick;
                            self.cooldown_expiry_tick = Some(current_tick + cooldown_duration);
                            self.state = BurnState::Cooldown;
                        } else {
                            self.state = BurnState::Ready;
                            self.burn_expiry_tick = None;
                        }
                    }
                }
            }
            BurnState::Cooldown => {
                if let Some(expiry) = self.cooldown_expiry_tick {
                    if current_tick >= expiry {
                        self.state = BurnState::Ready;
                        self.cooldown_expiry_tick = None;
                    }
                }
            }
        }
        self.state
    }

    /// Calculate remaining ticks for burn duration (only in Active state).
    pub fn burn_remaining_ticks(&self, current_tick: u32) -> u32 {
        if self.state == BurnState::Active {
            self.burn_expiry_tick
                .map(|expiry| expiry.saturating_sub(current_tick))
                .unwrap_or(0)
        } else {
            0
        }
    }

    /// Calculate remaining ticks for cooldown (only in Cooldown state).
    pub fn cooldown_remaining_ticks(&self, current_tick: u32) -> u32 {
        if self.state == BurnState::Cooldown {
            self.cooldown_expiry_tick
                .map(|expiry| expiry.saturating_sub(current_tick))
                .unwrap_or(0)
        } else {
            0
        }
    }
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
        ActivationHook::Bandolier(set_name) => {
            if let Ok(mut mgr) =
                crate::combat::bandolier::BANDOLIER_MANAGER.try_lock()
            {
                if let Some(cmd) = mgr.activate_command(set_name, u64::from(_ctx.tick)) {
                    crate::hooks::game_loop::queue_slash_command(cmd);
                }
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
        ConditionExpr::EnduranceBelow(threshold) => ctx.player.endurance_pct() < *threshold,
        ConditionExpr::TargetHpAbove(threshold) => {
            ctx.target.is_some_and(|t| t.hp_pct() > *threshold)
        }
        ConditionExpr::TargetHpBelow(threshold) => {
            ctx.target.is_some_and(|t| t.hp_pct() < *threshold)
        }
        ConditionExpr::ManaAbove(threshold) => ctx.player.mana_pct() > *threshold,
        ConditionExpr::EnduranceAbove(threshold) => ctx.player.endurance_pct() > *threshold,
        ConditionExpr::AggroOnMe => ctx.extended_targets.map_or_else(
            || ctx.in_combat && ctx.target.is_some_and(|t| t.spawn_type == 1),
            |xt| ctx.target.is_some_and(|t| xt.is_hater(t.spawn_id)),
        ),
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
        ConditionExpr::PlayerLevelAtLeast(level) => ctx.player.level >= *level,
        ConditionExpr::BurnReadyAndTriggered => {
            ctx.burn_state == BurnState::Ready && ctx.burnnow_triggered
        }
        ConditionExpr::BehindTarget => ctx
            .positional
            .is_some_and(|p| p.is_behind_target),
        ConditionExpr::RangedWeaponEquipped => ctx
            .positional
            .is_some_and(|p| p.ranged_weapon_equipped),
        ConditionExpr::PiercerEquipped => ctx
            .positional
            .is_some_and(|p| p.piercer_equipped),
        ConditionExpr::TargetLevelBelow(max_level) => ctx
            .positional
            .is_some_and(|p| p.target_level > 0 && p.target_level < *max_level),
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
    /// Optional shared cooldown key to apply after firing.
    pub cooldown_key: Option<String>,
    /// Optional cooldown window for the selected action.
    pub cooldown_ticks: Option<u32>,
    /// Optional shared cooldown key carried through from the source rotation
    /// entry.
    pub shared_cooldown_key: Option<String>,
}

/// Execute a single rotation group for one frame, returning selected actions.
///
/// The engine iterates entries starting from `group.current_step` (or 0 if
/// `full_rotation`), testing each entry's condition. Entries that pass get
/// added to the result up to `steps_per_frame`. The group's `current_step`
/// is advanced for next frame.
pub fn execute_group(group: &mut RotationGroup, ctx: &CombatContext) -> RotationResult {
    execute_group_with(group, ctx, &mut |_, _| true)
}

/// Execute a single rotation group while letting the caller reject actions
/// that are not ready at runtime (for example, unresolved abilities or active
/// cooldowns).
pub fn execute_group_with<F>(
    group: &mut RotationGroup,
    ctx: &CombatContext,
    is_action_ready: &mut F,
) -> RotationResult
where
    F: FnMut(&RotationEntry, u32) -> bool,
{
    execute_group_inner(group, ctx, None, is_action_ready)
}

fn execute_group_inner<F>(
    group: &mut RotationGroup,
    ctx: &CombatContext,
    strategy_target_id: Option<u32>,
    is_action_ready: &mut F,
) -> RotationResult
where
    F: FnMut(&RotationEntry, u32) -> bool,
{
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
    let Some(target_id) = select_target(&group.target_selector, ctx, strategy_target_id) else {
        return result;
    };

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

        if !is_action_ready(entry, target_id) {
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
            cooldown_key: entry.cooldown_key.clone(),
            cooldown_ticks: entry.cooldown_ticks,
            shared_cooldown_key: entry.shared_cooldown_key.clone(),
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
    execute_rotations_with(groups, ctx, |_, _| true)
}

/// Execute all rotation groups in order while applying a caller-supplied
/// readiness filter.
pub fn execute_rotations_with<F>(
    groups: &mut [RotationGroup],
    ctx: &CombatContext,
    mut is_action_ready: F,
) -> Option<SelectedAction>
where
    F: FnMut(&RotationEntry, u32) -> bool,
{
    execute_rotations_filtered_with_strategy_target(groups, ctx, None, &mut is_action_ready)
}

/// Execute rotations with a specific strategy target (e.g., for enchanters
/// targeting adds for mez/CC while the assist target is different).
pub fn execute_rotations_with_strategy_target(
    groups: &mut [RotationGroup],
    ctx: &CombatContext,
    strategy_target_id: Option<u32>,
) -> Option<SelectedAction> {
    execute_rotations_filtered_with_strategy_target(groups, ctx, strategy_target_id, &mut |_, _| {
        true
    })
}

/// Execute rotations with both a strategy target and a readiness filter.
pub fn execute_rotations_filtered_with_strategy_target<F>(
    groups: &mut [RotationGroup],
    ctx: &CombatContext,
    strategy_target_id: Option<u32>,
    is_ready: &mut F,
) -> Option<SelectedAction>
where
    F: FnMut(&RotationEntry, u32) -> bool,
{
    for group in groups.iter_mut() {
        let result = execute_group_inner(group, ctx, strategy_target_id, is_ready);
        if let Some(action) = result.actions.into_iter().next() {
            return Some(action);
        }
    }
    None
}

/// Resolve target ID based on a `TargetSelector` and current context.
fn select_target(
    selector: &TargetSelector,
    ctx: &CombatContext,
    strategy_target_id: Option<u32>,
) -> Option<u32> {
    match selector {
        TargetSelector::SelfOnly => Some(ctx.player.spawn_id),
        TargetSelector::AutoTarget => ctx.target.map(|t| t.spawn_id),
        TargetSelector::StrategyTarget => strategy_target_id.filter(|target_id| *target_id != 0),
        TargetSelector::AggroTarget => {
            // For now, fall back to auto-target. Full aggro scanning
            // will be implemented in issue #456.
            ctx.target.map(|t| t.spawn_id)
        }
        TargetSelector::LowestHpGroupMember => {
            Some(super::strategy::lowest_hp_member(ctx).map_or(ctx.player.spawn_id, |(id, _)| id))
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
        cooldown_key: None,
        cooldown_ticks: None,
        shared_cooldown_key: None,
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
        cooldown_key: None,
        cooldown_ticks: None,
        shared_cooldown_key: None,
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
        cooldown_key: None,
        cooldown_ticks: None,
        shared_cooldown_key: None,
    }
}

/// Convenience: create an entry with explicit cooldown metadata.
pub fn entry_with_cooldown(
    name: &str,
    action_type: ActionType,
    cooldown_key: &str,
    cooldown_ticks: u32,
) -> RotationEntry {
    let mut entry = entry(name, action_type);
    entry.cooldown_key = Some(cooldown_key.to_string());
    entry.cooldown_ticks = Some(cooldown_ticks);
    entry
}

/// Convenience: create a conditional entry with explicit cooldown metadata.
pub fn entry_if_with_cooldown(
    name: &str,
    action_type: ActionType,
    cond: ConditionExpr,
    cooldown_key: &str,
    cooldown_ticks: u32,
) -> RotationEntry {
    let mut entry = entry_if(name, action_type, cond);
    entry.cooldown_key = Some(cooldown_key.to_string());
    entry.cooldown_ticks = Some(cooldown_ticks);
    entry
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
        burn_duration_ticks: None,
        burn_cooldown_duration_ticks: None,
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
        combat::{
            ActionType, CombatConfig, ConditionExpr, ExtendedTargetList, ExtendedTargetSlot,
            XTargetSlotStatus, XTargetType,
        },
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
            positional: None,
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
    fn execute_rotations_with_skips_entries_rejected_by_readiness_filter() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, true);

        let mut groups = vec![{
            let mut g = group("Combat", TargetSelector::AutoTarget, CombatStateReq::Combat);
            g.entries
                .push(entry("Blocked", ActionType::Disc("Blocked".into())));
            g.entries
                .push(entry("Ready", ActionType::Spell("Ready".into())));
            g
        }];

        let action = execute_rotations_with(&mut groups, &ctx, |entry, _| entry.name == "Ready");
        assert_eq!(action.unwrap().entry_name, "Ready");
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
            positional: None,
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

    #[test]
    fn evaluate_condition_endurance_below() {
        let (mut player, _, config) = make_ctx(true, 80.0, 15.0);
        player.endurance_current = 15;
        player.endurance_max = 100;
        let ctx = build_ctx(&player, None, &config, true);
        assert!(evaluate_condition(
            &ConditionExpr::EnduranceBelow(20.0),
            &ctx
        ));
        assert!(!evaluate_condition(
            &ConditionExpr::EnduranceBelow(10.0),
            &ctx
        ));
    }

    #[test]
    fn evaluate_condition_endurance_above() {
        let (mut player, _, config) = make_ctx(true, 80.0, 15.0);
        player.endurance_current = 55;
        player.endurance_max = 100;
        let ctx = build_ctx(&player, None, &config, true);
        assert!(evaluate_condition(
            &ConditionExpr::EnduranceAbove(40.0),
            &ctx
        ));
        assert!(!evaluate_condition(
            &ConditionExpr::EnduranceAbove(60.0),
            &ctx
        ));
    }

    #[test]
    fn evaluate_condition_endurance_bounds() {
        let (mut player, _, config) = make_ctx(true, 80.0, 80.0);
        player.endurance_current = 200;
        player.endurance_max = 1000;
        let ctx = build_ctx(&player, None, &config, true);
        assert!(evaluate_condition(
            &ConditionExpr::EnduranceBelow(30.0),
            &ctx
        ));
        assert!(evaluate_condition(
            &ConditionExpr::EnduranceAbove(15.0),
            &ctx
        ));
        assert!(!evaluate_condition(
            &ConditionExpr::EnduranceAbove(25.0),
            &ctx
        ));
    }

    #[test]
    fn evaluate_condition_aggro_on_me_prefers_xtarget_signal() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let xtargets = ExtendedTargetList {
            slots: vec![ExtendedTargetSlot {
                slot_type: XTargetType::AutoHater,
                status: XTargetSlotStatus::CurrentZone,
                spawn_id: target.spawn_id,
                name: "TestMob".into(),
                aggro_pct: 0,
            }],
            auto_add_haters: true,
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: Some(&xtargets),
            positional: None,
        };

        assert!(evaluate_condition(&ConditionExpr::AggroOnMe, &ctx));
    }

    #[test]
    fn evaluate_condition_aggro_on_me_false_when_xtarget_omits_target() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let xtargets = ExtendedTargetList {
            slots: vec![ExtendedTargetSlot {
                slot_type: XTargetType::AutoHater,
                status: XTargetSlotStatus::CurrentZone,
                spawn_id: 999,
                name: "OtherMob".into(),
                aggro_pct: 0,
            }],
            auto_add_haters: true,
        };
        let ctx = CombatContext {
            player: &player,
            target: Some(&target),
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: Some(&xtargets),
            positional: None,
        };

        assert!(!evaluate_condition(&ConditionExpr::AggroOnMe, &ctx));
    }

    // -----------------------------------------------------------------------
    // Positional / equipment condition tests
    // -----------------------------------------------------------------------

    use textquest_common::combat::PositionalContext;

    fn build_ctx_positional<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        config: &'a CombatConfig,
        positional: &'a PositionalContext,
    ) -> CombatContext<'a> {
        CombatContext {
            player,
            target,
            nearby_enemies: &[],
            group_members: &[],
            config,
            tick: 0,
            in_combat: true,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
            positional: Some(positional),
        }
    }

    #[test]
    fn behind_target_true_when_positional_set() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let pos = PositionalContext {
            is_behind_target: true,
            ..PositionalContext::default()
        };
        let ctx = build_ctx_positional(&player, Some(&target), &config, &pos);
        assert!(evaluate_condition(&ConditionExpr::BehindTarget, &ctx));
    }

    #[test]
    fn behind_target_false_when_positional_absent() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let ctx = build_ctx(&player, Some(&target), &config, true);
        assert!(!evaluate_condition(&ConditionExpr::BehindTarget, &ctx));
    }

    #[test]
    fn behind_target_false_when_not_behind() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let pos = PositionalContext {
            is_behind_target: false,
            ..PositionalContext::default()
        };
        let ctx = build_ctx_positional(&player, Some(&target), &config, &pos);
        assert!(!evaluate_condition(&ConditionExpr::BehindTarget, &ctx));
    }

    #[test]
    fn piercer_equipped_gates_correctly() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let pos_with = PositionalContext {
            piercer_equipped: true,
            ..PositionalContext::default()
        };
        let pos_without = PositionalContext {
            piercer_equipped: false,
            ..PositionalContext::default()
        };
        let ctx_with = build_ctx_positional(&player, Some(&target), &config, &pos_with);
        let ctx_without = build_ctx_positional(&player, Some(&target), &config, &pos_without);
        assert!(evaluate_condition(&ConditionExpr::PiercerEquipped, &ctx_with));
        assert!(!evaluate_condition(
            &ConditionExpr::PiercerEquipped,
            &ctx_without
        ));
    }

    #[test]
    fn ranged_weapon_equipped_gates_correctly() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let pos = PositionalContext {
            ranged_weapon_equipped: true,
            ..PositionalContext::default()
        };
        let ctx = build_ctx_positional(&player, Some(&target), &config, &pos);
        assert!(evaluate_condition(&ConditionExpr::RangedWeaponEquipped, &ctx));
    }

    #[test]
    fn target_level_below_fires_when_target_low() {
        let (player, _, config) = make_ctx(true, 80.0, 80.0);
        let mut target = SpawnData::default();
        target.spawn_id = 42;
        target.level = 10;
        let ctx = build_ctx(&player, Some(&target), &config, true);
        assert!(evaluate_condition(
            &ConditionExpr::TargetLevelBelow(20),
            &ctx
        ));
        assert!(!evaluate_condition(
            &ConditionExpr::TargetLevelBelow(10),
            &ctx
        ));
    }

    #[test]
    fn target_level_below_false_when_no_target() {
        let (player, _, config) = make_ctx(true, 80.0, 80.0);
        let ctx = build_ctx(&player, None, &config, true);
        assert!(!evaluate_condition(
            &ConditionExpr::TargetLevelBelow(60),
            &ctx
        ));
    }

    #[test]
    fn player_is_stealthed_gates_correctly() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let pos = PositionalContext {
            is_stealthed: true,
            ..PositionalContext::default()
        };
        let ctx = build_ctx_positional(&player, Some(&target), &config, &pos);
        assert!(evaluate_condition(&ConditionExpr::PlayerIsStealthed, &ctx));
    }

    #[test]
    fn and_condition_behind_and_piercer() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let pos = PositionalContext {
            is_behind_target: true,
            piercer_equipped: true,
            ..PositionalContext::default()
        };
        let ctx = build_ctx_positional(&player, Some(&target), &config, &pos);
        let cond = ConditionExpr::And(vec![
            ConditionExpr::BehindTarget,
            ConditionExpr::PiercerEquipped,
        ]);
        assert!(evaluate_condition(&cond, &ctx));
    }

    #[test]
    fn and_condition_behind_without_piercer_fails() {
        let (player, target, config) = make_ctx(true, 80.0, 80.0);
        let pos = PositionalContext {
            is_behind_target: true,
            piercer_equipped: false,
            ..PositionalContext::default()
        };
        let ctx = build_ctx_positional(&player, Some(&target), &config, &pos);
        let cond = ConditionExpr::And(vec![
            ConditionExpr::BehindTarget,
            ConditionExpr::PiercerEquipped,
        ]);
        assert!(!evaluate_condition(&cond, &ctx));
    }
}
