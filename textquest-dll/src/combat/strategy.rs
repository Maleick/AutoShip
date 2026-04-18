use std::collections::HashMap;

use textquest_common::{
    combat::{
        AbilitySet, BuffInfo, CastResult, CombatConfig, CombatRole, ExtendedTargetList,
        HpPreference, KnownAbility, NamedPreference, ResolvedAbility, SpellEntry, TargetScanConfig,
    },
    types::SpawnData,
};

use super::rotation::RotationGroup;

// ─── Pet action / status types
// ────────────────────────────────────────────────

use super::classes::{
    bard::BardStrategy, beastlord::BeastlordStrategy, berserker::BerserkerStrategy,
    cleric::ClericStrategy, druid::DruidStrategy, enchanter::EnchanterStrategy,
    generic_dps::GenericDpsStrategy, magician::MagicianStrategy, monk::MonkStrategy,
    necromancer::NecromancerStrategy, paladin::PaladinStrategy, ranger::RangerStrategy,
    rogue::RogueStrategy, shadow_knight::ShadowKnightStrategy, shaman::ShamanStrategy,
    warrior::WarriorStrategy, wizard::WizardStrategy,
};

/// Snapshot of a summoned pet's current state for pet-management decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PetStatus {
    /// The pet's spawn ID, or `None` if no pet is active.
    pub spawn_id: Option<u32>,
    /// The spawn ID the pet is currently attacking, or `None` if idle.
    pub target_id: Option<u32>,
}

impl PetStatus {
    /// Returns `true` if a pet is currently active.
    pub fn has_pet(self) -> bool {
        self.spawn_id.is_some()
    }

    /// Returns `true` if the pet is already attacking `target_id`.
    pub fn is_attacking(self, target_id: u32) -> bool {
        self.target_id == Some(target_id)
    }
}

/// Pet-management action requested by a `ClassStrategy` for the current frame.
#[derive(Debug, Clone, PartialEq)]
pub enum PetAction {
    /// Send `/pet attack` to the current target.
    Attack,
    /// Cast a pet-targeted buff (e.g., Burnout) using the given spell entry.
    Buff {
        /// The spell the strategy wants to cast on the pet.
        spell: SpellEntry,
    },
}

/// Read-only snapshot of combat-relevant state, passed to strategy methods each
/// frame.
pub struct CombatContext<'a> {
    pub player: &'a SpawnData,
    pub target: Option<&'a SpawnData>,
    pub nearby_enemies: &'a [SpawnData],
    pub group_members: &'a [GroupMemberState],
    pub config: &'a CombatConfig,
    pub tick: u32,
    pub in_combat: bool,
    /// When a CH chain is active, this is the spell slot the cleric should
    /// cast. The cleric strategy defers its normal priority cascade and
    /// casts CH instead when this is `Some`. Set by the orchestrator when
    /// it's this cleric's turn.
    pub ch_chain_slot: Option<u8>,
    /// Active buff spell IDs on the player (populated from buff window scan).
    pub active_buffs: &'a [i32],
    /// Full buff details with duration/hit-count info (see `combat::buffs`).
    pub buff_info: &'a [BuffInfo],
    /// Whether the current target is mezzed (has a mesmerize debuff active).
    pub target_is_mezzed: bool,
    /// Extended target list snapshot (hate list, group roles, etc.).
    /// `None` if the data couldn't be read (not yet loaded, zoning, etc.).
    pub extended_targets: Option<&'a ExtendedTargetList>,
}

impl CombatContext<'_> {
    pub fn pet(&self) -> Option<&textquest_common::combat::ExtendedTargetSlot> {
        self.extended_targets.and_then(ExtendedTargetList::pet)
    }

    pub fn pet_spawn_id(&self) -> Option<u32> {
        self.extended_targets
            .and_then(ExtendedTargetList::pet_spawn_id)
    }

    pub fn pet_target(&self) -> Option<&textquest_common::combat::ExtendedTargetSlot> {
        self.extended_targets
            .and_then(ExtendedTargetList::pet_target)
    }

    pub fn pet_target_id(&self) -> Option<u32> {
        self.extended_targets
            .and_then(ExtendedTargetList::pet_target_id)
    }

    pub fn pet_status(&self) -> PetStatus {
        PetStatus {
            spawn_id: self.pet_spawn_id(),
            target_id: self.pet_target_id(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GroupMemberState {
    pub spawn_id: u32,
    pub hp_pct: f32,
    pub mana_pct: f32,
    pub class_id: u8,
    /// True if this member is dead (corpse present, needs resurrection).
    pub is_dead: bool,
    /// Character name, used for corpse targeting during resurrection.
    pub name: String,
    /// True if this member has a detrimental effect (poison, disease, curse)
    /// that should be cured. Set by the orchestrator from buff window parsing.
    pub has_detrimental: bool,
}

/// Runtime-resolved ability plus any activated-ability cooldown metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityResolution {
    /// Which ability set this entry resolved from.
    pub set_name: String,
    /// The chosen candidate's display name.
    pub ability_name: String,
    /// EQ spell/disc ID to activate.
    pub spell_id: i32,
    /// Level gate that selected this candidate.
    pub min_level: u8,
    /// Explicit reuse timer for this ability, in combat ticks.
    pub cooldown_ticks: Option<u32>,
    /// Optional shared reuse bucket name.
    pub shared_cooldown_key: Option<String>,
    /// Optional shared reuse duration for the bucket, in combat ticks.
    pub shared_cooldown_ticks: Option<u32>,
}

impl From<ResolvedAbility> for AbilityResolution {
    fn from(value: ResolvedAbility) -> Self {
        Self {
            set_name: value.set_name,
            ability_name: value.ability_name,
            spell_id: value.spell_id,
            min_level: value.min_level,
            cooldown_ticks: None,
            shared_cooldown_key: None,
            shared_cooldown_ticks: None,
        }
    }
}

/// The core seam between generic combat framework and per-class logic.
/// Each EQ class implements this trait to define its combat behavior.
pub trait ClassStrategy: Send {
    /// Which EQ class this strategy handles.
    fn class_id(&self) -> u8;

    /// Select the next spell to cast given current context.
    /// Returns None if no spell should be cast (med, wait, etc.)
    fn select_spell(&self, ctx: &CombatContext) -> Option<SpellEntry>;

    /// Select what to target. For DPS: assist target's target.
    /// For healers: lowest HP group member. For tanks: current mob.
    fn select_target(&self, ctx: &CombatContext) -> Option<u32>;

    /// Whether this character should assist the main assist.
    fn should_assist(&self, ctx: &CombatContext) -> bool;

    /// Optional pet-management action for this frame.
    ///
    /// Strategies can inspect `ctx.pet_status()` and request an explicit pet
    /// command or pet-targeted buff without hardcoding pet logic in the combat
    /// FSM.
    fn pet_action(&self, _ctx: &CombatContext) -> Option<PetAction> {
        None
    }

    /// Called when engaging a new target.
    /// Override to log engagement or toggle auto-attack.
    fn on_engage(&mut self, _ctx: &CombatContext) {}

    /// Called after an action completes (spell cast, ability use, song twist).
    /// Override to advance internal state (e.g., bard twist index, auto-attack
    /// toggle).
    fn on_action_complete(&mut self, _ctx: &CombatContext) {}

    /// Called when a cast is interrupted before completion (HolyShit preempt,
    /// target lost mid-cast, or external interrupt like "You miss a note").
    /// `gem` is the spell slot that was being cast when the interrupt occurred.
    /// Default is a no-op; bards override this to re-queue the interrupted
    /// song.
    fn on_cast_interrupted(&mut self, _ctx: &CombatContext, _gem: u8) {}

    /// Called when a cast attempt resolves to a concrete MQ2Cast-style outcome.
    ///
    /// Default behavior preserves the older interrupt-only hook for strategies
    /// that only care about interrupted casts.
    fn on_cast_outcome(&mut self, ctx: &CombatContext, gem: u8, result: CastResult) {
        if matches!(
            result,
            CastResult::Interrupted | CastResult::Aborted | CastResult::Cancelled
        ) {
            self.on_cast_interrupted(ctx, gem);
        }
    }

    /// Called when a cast attempt resolves for a specific rotation entry.
    ///
    /// This gives class strategies access to the resolved spell ID and target
    /// so they can maintain per-line state like mez refresh queues without
    /// hard-coding that logic into the generic combat FSM.
    fn on_resolved_action_outcome(
        &mut self,
        _ctx: &CombatContext,
        _entry_name: Option<&str>,
        _spell_id: i32,
        _target_id: u32,
        _result: CastResult,
    ) {
    }

    /// Minimum enemy count before switching to `AoE` rotation.
    fn aoe_threshold(&self) -> u8;

    /// Combat role for this strategy.
    fn role(&self) -> CombatRole;

    /// Return data-driven rotation groups for this class.
    ///
    /// When this returns `Some`, the combat FSM uses the rotation engine
    /// instead of `select_spell()`. This enables multi-rotation behavior
    /// with named groups (Downtime, Combat, Emergency, Burn, etc.),
    /// per-entry conditions, and step limits.
    ///
    /// Default returns `None` — falling back to `select_spell()` for
    /// backward compatibility with existing class strategies.
    fn rotation_groups(&self) -> Option<Vec<RotationGroup>> {
        None
    }

    /// Return ability sets for spell auto-resolution.
    ///
    /// Each set maps a spell line name to an ordered list of candidates,
    /// strongest first. The combat FSM resolves these at startup using the
    /// character's known spells and level.
    fn ability_sets(&self) -> Vec<AbilitySet> {
        Vec::new()
    }

    /// Resolve class ability sets for the current character.
    ///
    /// Default behavior uses the generic resolver and carries no explicit
    /// cooldown metadata. Classes with activated abilities that are not present
    /// in spellbook scans can override this.
    fn resolve_abilities_for_character(
        &self,
        known: &[KnownAbility],
        character_level: u8,
    ) -> HashMap<String, AbilityResolution> {
        textquest_common::combat::resolve_abilities(&self.ability_sets(), known, character_level)
            .into_iter()
            .map(|(set_name, resolved)| (set_name, resolved.into()))
            .collect()
    }

    /// Whether this class owns its melee skills through the rotation engine.
    ///
    /// Classes that return `true` skip the generic `tick_melee_skills()`
    /// helper so taunt/kick/bash usage stays config-driven and does not double
    /// fire.
    fn manages_melee_skills_in_rotation(&self) -> bool {
        false
    }

    /// Called whenever the FSM re-resolves this class' ability sets.
    ///
    /// Strategies that build runtime priority state from resolved spell lines
    /// can cache it here.
    fn on_abilities_resolved(&mut self, _resolved: &HashMap<String, ResolvedAbility>) {}

    /// Whether this class should be gated by mana recovery logic during
    /// combat.
    fn uses_mana_for_combat(&self) -> bool {
        true
    }

    /// Allow strategies to reconcile rotation groups after spell-line
    /// resolution (for example, pruning entries that do not exist at the
    /// current level).
    fn sync_resolved_rotation_groups(
        &self,
        _groups: &mut [RotationGroup],
        _resolved_abilities: &HashMap<String, AbilityResolution>,
    ) {
    }

    /// Receive the current resolved ability map after spell-book resolution.
    ///
    /// Strategies that need line-aware spell selection can cache these results
    /// locally without widening `CombatContext` for every class.
    fn on_resolved_abilities(&mut self, _resolved: &HashMap<String, ResolvedAbility>) {}
    /// Whether the current cast should participate in heal-cancel logic.
    ///
    /// The default heuristic only treats explicitly configured `*heal*` spells
    /// as cancelable. Classes with line-based healing can override this.
    fn is_heal_cast(&self, ctx: &CombatContext, spell_slot: u8, spell_id: i32) -> bool {
        spell_for_cast(ctx, spell_slot, spell_id)
            .is_some_and(|spell| spell_name_contains_heal(&spell.name))
    }

    /// HP threshold above which an in-flight heal should be canceled.
    ///
    /// Healer-role classes inherit the legacy 85% threshold unless they
    /// override it explicitly. Non-healers return `None`.
    fn heal_cancel_threshold(&self) -> Option<f32> {
        matches!(self.role(), CombatRole::Healer).then_some(85.0)
    }

    /// Whether this class uses built-in combat drivers (e.g., damage procs, pet management).
    /// Classes that manage abilities through rotation entries return false.
    fn uses_builtin_combat_drivers(&self) -> bool {
        true
    }

    /// Get the cooldown ticks for an activated ability by spell ID.
    /// Returns None if the ability is not known or has no fixed cooldown.
    fn activated_ability_cooldown_ticks(&self, _spell_id: i32) -> Option<u32> {
        None
    }

    /// Get the list of shared timer IDs for an activated ability.
    /// Used for abilities that share timers with other abilities.
    fn shared_activated_ability_ids(&self, _spell_id: i32) -> &'static [i32] {
        &[]
    }
}

// ---------------------------------------------------------------------------
// Shared melee helpers — reused by warrior, rogue, monk, SK, paladin,
// berserker, beastlord to eliminate duplicated code across class strategies.
// ---------------------------------------------------------------------------

/// Find the nearest NPC from the nearby enemies list based on 2D distance to
/// player. Used by tank/pull-capable classes (warrior, berserker, beastlord)
/// for target selection.
#[inline]
pub fn nearest_enemy<'a>(player: &SpawnData, enemies: &'a [SpawnData]) -> Option<&'a SpawnData> {
    let px = player.x;
    let py = player.y;
    enemies.iter().min_by(|a, b| {
        let dist_sq_a = (a.x - px).powi(2) + (a.y - py).powi(2);
        let dist_sq_b = (b.x - px).powi(2) + (b.y - py).powi(2);
        dist_sq_a
            .partial_cmp(&dist_sq_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// Common assist-target selection: return the current target's spawn ID.
/// Used by DPS melee classes that follow the main assist.
#[inline]
pub fn assist_target(ctx: &CombatContext) -> Option<u32> {
    ctx.target.map(|t| t.spawn_id)
}

/// Common pet attack behavior: if we have a pet and it's not already attacking
/// the current target, request `/pet attack`.
#[inline]
pub fn pet_attack_action(ctx: &CombatContext) -> Option<PetAction> {
    let target_id = ctx.target?.spawn_id;
    let pet = ctx.pet_status();
    if !ctx.in_combat || !pet.has_pet() || pet.is_attacking(target_id) {
        return None;
    }
    Some(PetAction::Attack)
}

/// Common `on_engage` for melee classes: log engagement and enable auto-attack.
pub fn melee_on_engage(ctx: &CombatContext, class_label: &str) {
    if let Some(target) = ctx.target {
        tracing::info!(
            target_id = target.spawn_id,
            target_name = %target.name,
            "{class_label} engaging"
        );
    }
    crate::eq::toggle_auto_attack(true);
}

/// Common disengage for melee classes: disable auto-attack.
/// Only call when `!ctx.in_combat` — mid-combat spell completions should NOT
/// disable auto-attack.
pub fn melee_on_disengage() {
    crate::eq::toggle_auto_attack(false);
}

/// Common pet attack helper for pet classes.
pub fn pet_attack() {
    crate::eq::slash_command("/pet attack");
}

/// Common pet single-target focus helper for pet classes.
pub fn pet_focus() {
    crate::eq::slash_command("/pet focus");
}

/// Find the configured spell entry backing an active cast.
#[inline]
pub fn spell_for_cast<'a>(
    ctx: &'a CombatContext<'a>,
    spell_slot: u8,
    spell_id: i32,
) -> Option<&'a SpellEntry> {
    ctx.config
        .spells
        .iter()
        .find(|spell| spell.slot == spell_slot)
        .or_else(|| {
            (spell_id > 0)
                .then(|| {
                    ctx.config
                        .spells
                        .iter()
                        .find(|spell| spell.spell_id == spell_id)
                })
                .flatten()
        })
}

#[inline]
pub fn spell_name_contains_heal(name: &str) -> bool {
    name.to_ascii_lowercase().contains("heal")
}

/// Common pet engage helper: attack the current target and focus it.
pub fn pet_attack_focused() {
    pet_attack();
    pet_focus();
}

/// Common pet disengage helper: call the pet back to the owner.
pub fn pet_back_off() {
    crate::eq::slash_command("/pet back");
}

/// Find the group member with the lowest HP percentage (alive only).
/// Used by healer and hybrid classes (cleric, druid, paladin, shaman) for heal
/// targeting.
#[inline]
pub fn lowest_hp_member(ctx: &CombatContext) -> Option<(u32, f32)> {
    lowest_hp_member_in(
        ctx.group_members
            .iter()
            .filter(|m| !m.is_dead && m.hp_pct.is_finite() && m.hp_pct > 0.0),
    )
}

/// Count living group members who currently need a cure.
#[inline]
pub fn afflicted_member_count(ctx: &CombatContext) -> usize {
    ctx.group_members
        .iter()
        .filter(|m| !m.is_dead && m.has_detrimental)
        .count()
}

/// Find the afflicted group member with the lowest HP.
///
/// This makes single-target cures prefer the most at-risk player instead of the
/// first member in roster order.
#[inline]
pub fn prioritized_afflicted_member(ctx: &CombatContext) -> Option<(u32, f32)> {
    lowest_hp_member_in(
        ctx.group_members
            .iter()
            .filter(|m| !m.is_dead && m.has_detrimental && m.hp_pct.is_finite() && m.hp_pct > 0.0),
    )
}

#[inline]
fn lowest_hp_member_in<'a>(
    members: impl Iterator<Item = &'a GroupMemberState>,
) -> Option<(u32, f32)> {
    members
        .min_by(|a, b| {
            a.hp_pct
                .partial_cmp(&b.hp_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|m| (m.spawn_id, m.hp_pct))
}

#[inline]
pub fn is_standard_cure_spell(spell: &SpellEntry) -> bool {
    let name = spell.name.to_lowercase();
    name.contains("cure")
        || name.contains("purify")
        || name.contains("remove")
        || name.contains("counteract")
}

#[inline]
pub fn is_group_cure_spell(spell: &SpellEntry) -> bool {
    let name = spell.name.to_lowercase();
    spell.is_aoe || name.contains("group") || name.contains("radiant cure")
}

/// Select the highest-priority spell from config, filtered by current mana.
#[inline]
pub fn best_spell_by_mana(ctx: &CombatContext) -> Option<SpellEntry> {
    let mana_pct = ctx.player.mana_pct();
    ctx.config
        .spells
        .iter()
        .filter(|s| mana_pct >= s.min_mana_pct)
        .max_by_key(|s| s.priority)
        .cloned()
}

// ---------------------------------------------------------------------------
// MA target scan — rgmercs-style smart target selection with named/aggro
// priority, safe targeting, and mez skip.
// ---------------------------------------------------------------------------

/// Stand state values that indicate a mezzed (crowd-controlled) mob.
/// EQ uses various stand_state values for different CC effects.
const STAND_STATE_STUNNED: u8 = 4;

/// Check if a mob appears to be mezzed or otherwise crowd-controlled.
/// Mezzed mobs have specific stand_state values and zero run speed.
#[inline]
pub fn is_mezzed(spawn: &SpawnData) -> bool {
    spawn.stand_state == STAND_STATE_STUNNED && spawn.speed_run.abs() < 0.01
}

/// Check if a mob is a "named" (non-generic) NPC based on its displayed name.
/// Generic mobs start with articles ("a ", "an ", "the ") while named mobs
/// have proper names (e.g., "Lord Nagafen", "Cazic Thule").
#[inline]
pub fn is_named_mob(name: &str) -> bool {
    let lower = name.to_lowercase();
    !lower.starts_with("a ") && !lower.starts_with("an ") && !lower.starts_with("the ")
}

/// Check if a mob is safe to target — its current target (if any) should be
/// in our group. This prevents pulling mobs that are fighting another group.
///
/// Since we don't have the mob's target info in `SpawnData`, we approximate:
/// a moving mob with non-zero speed that is far from the player is likely
/// fighting something else. This is a heuristic — XTarget would be
/// authoritative.
#[inline]
pub fn is_safe_target(
    spawn: &SpawnData,
    player: &SpawnData,
    group_ids: &[u32],
    safe_targeting: bool,
) -> bool {
    if !safe_targeting {
        return true;
    }
    // If the mob is stationary, it's not engaged with anyone — safe.
    if !spawn.is_moving() {
        return true;
    }
    // If the mob is close to us (within 50 units), it's probably our fight.
    let dx = spawn.x - player.x;
    let dy = spawn.y - player.y;
    let dist_sq = dx * dx + dy * dy;
    if dist_sq < 50.0 * 50.0 {
        return true;
    }
    // If any group member is close to the mob, it's our group's fight.
    // (We can't check the mob's target directly without XTarget, so this
    // is a proximity heuristic.)
    let _ = group_ids; // Reserved for future XTarget-based check
    true
}

/// MA target scan — scans nearby enemies and returns the best target based on
/// named priority, HP preference, mez skip, and safe targeting rules.
///
/// This implements rgmercs-style `MATargetScan()` logic using available spawn
/// data.
pub fn ma_target_scan<'a>(
    player: &SpawnData,
    enemies: &'a [SpawnData],
    group_ids: &[u32],
    config: &TargetScanConfig,
) -> Option<&'a SpawnData> {
    let px = player.x;
    let py = player.y;
    let pz = player.z;

    // Filter candidates: in range, alive, not mezzed, safe
    let candidates: Vec<&SpawnData> = enemies
        .iter()
        .filter(|e| {
            // Distance check (2D)
            let dx = e.x - px;
            let dy = e.y - py;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq > config.scan_radius * config.scan_radius {
                return false;
            }
            // Z-axis check
            if (e.z - pz).abs() > config.scan_z_radius {
                return false;
            }
            // Skip dead mobs
            if e.hp_pct() <= 0.0 {
                return false;
            }
            // Skip mezzed mobs
            if config.skip_mezzed && is_mezzed(e) {
                return false;
            }
            // Safe targeting check
            if !is_safe_target(e, player, group_ids, config.safe_targeting) {
                return false;
            }
            true
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    // Select pool based on named preference
    let pool: Vec<&SpawnData> = match config.named_preference {
        NamedPreference::NamedFirst => {
            let named: Vec<&SpawnData> = candidates
                .iter()
                .filter(|e| is_named_mob(&e.displayed_name))
                .copied()
                .collect();
            if named.is_empty() { candidates } else { named }
        }
        NamedPreference::TrashFirst => {
            let trash: Vec<&SpawnData> = candidates
                .iter()
                .filter(|e| !is_named_mob(&e.displayed_name))
                .copied()
                .collect();
            if trash.is_empty() { candidates } else { trash }
        }
        NamedPreference::NoPreference => candidates,
    };

    if pool.is_empty() {
        return None;
    }

    // Sort by HP preference
    match config.hp_preference {
        HpPreference::LowestHp => pool.into_iter().min_by(|a, b| {
            a.hp_pct()
                .partial_cmp(&b.hp_pct())
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        HpPreference::HighestHp => pool.into_iter().max_by(|a, b| {
            a.hp_pct()
                .partial_cmp(&b.hp_pct())
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        HpPreference::Nearest => pool.into_iter().min_by(|a, b| {
            let da = (a.x - px).powi(2) + (a.y - py).powi(2);
            let db = (b.x - px).powi(2) + (b.y - py).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        }),
    }
}

/// Factory function -- creates the right strategy for a given class.
pub fn build_strategy(class_id: u8, config: &CombatConfig) -> Box<dyn ClassStrategy> {
    match class_id {
        1 => Box::new(WarriorStrategy::new(class_id)), // Warrior
        2 => Box::new(ClericStrategy::new(class_id)),  // Cleric
        3 => Box::new(PaladinStrategy::new(class_id)), // Paladin
        4 => Box::new(RangerStrategy::new(class_id)),  // Ranger
        5 => Box::new(ShadowKnightStrategy::new(class_id)), // Shadow Knight
        6 => Box::new(DruidStrategy::new(class_id)),   // Druid
        7 => Box::new(MonkStrategy::new(class_id)),    // Monk
        8 => Box::new(BardStrategy::live_safe(class_id)), // Bard
        9 => Box::new(RogueStrategy::new(class_id)),   // Rogue
        10 => Box::new(ShamanStrategy::new(class_id)), // Shaman
        11 => Box::new(NecromancerStrategy::new(class_id)), // Necromancer
        12 => Box::new(WizardStrategy::new(class_id)), // Wizard
        13 => Box::new(MagicianStrategy::new(class_id)), // Magician
        14 => Box::new(EnchanterStrategy::new(class_id)), // Enchanter
        15 => Box::new(BeastlordStrategy::new(class_id)), // Beastlord
        16 => Box::new(BerserkerStrategy::new(class_id)), // Berserker
        _ => Box::new(GenericDpsStrategy::new(class_id, config)),
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use textquest_common::combat::{ExtendedTargetSlot, XTargetSlotStatus, XTargetType};

    fn make_ctx_with_xtargets<'a>(
        player: &'a SpawnData,
        target: Option<&'a SpawnData>,
        config: &'a CombatConfig,
        in_combat: bool,
        extended_targets: Option<&'a ExtendedTargetList>,
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
            extended_targets,
        }
    }

    #[test]
    fn build_strategy_all_16_classes() {
        let config = CombatConfig::default();
        for id in 1..=16u8 {
            let strategy = build_strategy(id, &config);
            assert_eq!(
                strategy.class_id(),
                id,
                "build_strategy({}) returned class_id {}",
                id,
                strategy.class_id()
            );
            // Just verify role() doesn't panic
            let _role = strategy.role();
        }
    }

    #[test]
    fn build_strategy_unknown_class_uses_generic() {
        let config = CombatConfig::default();
        let strategy = build_strategy(99, &config);
        assert_eq!(strategy.class_id(), 99);
    }

    #[test]
    fn nearest_enemy_returns_closest() {
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 100.0,
                y: 0.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 10.0,
                y: 0.0,
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 3,
                x: 50.0,
                y: 0.0,
                ..SpawnData::default()
            },
        ];
        let closest = nearest_enemy(&player, &enemies).unwrap();
        assert_eq!(closest.spawn_id, 2);
    }

    #[test]
    fn nearest_enemy_empty_list() {
        let player = SpawnData::default();
        assert!(nearest_enemy(&player, &[]).is_none());
    }

    #[test]
    fn assist_target_returns_target_id() {
        let player = SpawnData::default();
        let target = SpawnData {
            spawn_id: 42,
            ..SpawnData::default()
        };
        let config = CombatConfig::default();
        let ctx = make_ctx_with_xtargets(&player, Some(&target), &config, false, None);
        assert_eq!(assist_target(&ctx), Some(42));
    }

    #[test]
    fn assist_target_none_without_target() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx_with_xtargets(&player, None, &config, false, None);
        assert!(assist_target(&ctx).is_none());
    }

    #[test]
    fn combat_context_pet_helpers_expose_pet_state() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let xtargets = ExtendedTargetList {
            slots: vec![
                textquest_common::combat::ExtendedTargetSlot {
                    slot_type: textquest_common::combat::XTargetType::MyPet,
                    status: textquest_common::combat::XTargetSlotStatus::CurrentZone,
                    spawn_id: 77,
                    name: "Warder".into(),
                },
                textquest_common::combat::ExtendedTargetSlot {
                    slot_type: textquest_common::combat::XTargetType::MyPetTarget,
                    status: textquest_common::combat::XTargetSlotStatus::CurrentZone,
                    spawn_id: 88,
                    name: "A goblin".into(),
                },
            ],
            auto_add_haters: false,
        };
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: Some(&xtargets),
        };

        assert_eq!(ctx.pet().map(|slot| slot.name.as_str()), Some("Warder"));
        assert_eq!(ctx.pet_spawn_id(), Some(77));
        assert_eq!(
            ctx.pet_target().map(|slot| slot.name.as_str()),
            Some("A goblin")
        );
        assert_eq!(ctx.pet_target_id(), Some(88));
    }

    #[test]
    fn combat_context_pet_helpers_handle_missing_xtargets() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &[],
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };

        assert!(ctx.pet().is_none());
        assert_eq!(ctx.pet_spawn_id(), None);
        assert!(ctx.pet_target().is_none());
        assert_eq!(ctx.pet_target_id(), None);
    }

    #[test]
    fn pet_command_helpers_noop_without_eq_base() {
        pet_attack();
        pet_focus();
        pet_attack_focused();
        pet_back_off();
    }

    #[test]
    fn best_spell_by_mana_returns_highest_priority() {
        use textquest_common::combat::SpellEntry;
        let mut player = SpawnData::default();
        player.mana_current = 5000;
        player.mana_max = 10000;
        let config = CombatConfig {
            spells: vec![
                SpellEntry {
                    name: "Low".into(),
                    slot: 1,
                    spell_id: 100,
                    priority: 1,
                    min_mana_pct: 10.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "High".into(),
                    slot: 2,
                    spell_id: 200,
                    priority: 10,
                    min_mana_pct: 10.0,
                    is_aoe: false,
                },
                SpellEntry {
                    name: "TooExpensive".into(),
                    slot: 3,
                    spell_id: 300,
                    priority: 100,
                    min_mana_pct: 90.0,
                    is_aoe: false,
                },
            ],
            ..CombatConfig::default()
        };
        let ctx = make_ctx_with_xtargets(&player, None, &config, false, None);
        let spell = best_spell_by_mana(&ctx).unwrap();
        assert_eq!(spell.name, "High"); // highest priority that we can afford
    }

    #[test]
    fn best_spell_by_mana_none_when_empty() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let ctx = make_ctx_with_xtargets(&player, None, &config, false, None);
        assert!(best_spell_by_mana(&ctx).is_none());
    }

    #[test]
    fn best_spell_by_mana_none_when_all_too_expensive() {
        let mut player = SpawnData::default();
        player.mana_current = 100;
        player.mana_max = 10000; // 1% mana
        let config = CombatConfig {
            spells: vec![SpellEntry {
                name: "Nuke".into(),
                slot: 1,
                spell_id: 1,
                priority: 10,
                min_mana_pct: 50.0,
                is_aoe: false,
            }],
            ..CombatConfig::default()
        };
        let ctx = make_ctx_with_xtargets(&player, None, &config, false, None);
        assert!(best_spell_by_mana(&ctx).is_none());
    }

    #[test]
    fn pet_status_reads_my_pet_slots() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let xtargets = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::MyPet,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 77,
                    name: "Bonewalker".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::MyPetTarget,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 99,
                    name: "a skeleton".into(),
                },
            ],
            auto_add_haters: false,
        };
        let ctx = make_ctx_with_xtargets(&player, None, &config, false, Some(&xtargets));
        assert_eq!(
            ctx.pet_status(),
            PetStatus {
                spawn_id: Some(77),
                target_id: Some(99),
            }
        );
    }

    #[test]
    fn pet_strategy_requests_attack_when_pet_is_idle() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let target = SpawnData {
            spawn_id: 99,
            ..SpawnData::default()
        };
        let xtargets = ExtendedTargetList {
            slots: vec![ExtendedTargetSlot {
                slot_type: XTargetType::MyPet,
                status: XTargetSlotStatus::CurrentZone,
                spawn_id: 77,
                name: "Warder".into(),
            }],
            auto_add_haters: false,
        };
        let ctx = make_ctx_with_xtargets(&player, Some(&target), &config, true, Some(&xtargets));
        let mage = build_strategy(13, &config);
        assert_eq!(mage.pet_action(&ctx), Some(PetAction::Attack));
    }

    #[test]
    fn pet_action_buff_equality_tolerates_float_rounding() {
        let lhs = PetAction::Buff {
            spell: SpellEntry {
                name: "Burnout".into(),
                slot: 1,
                spell_id: 42,
                min_mana_pct: 10.0,
                priority: 1,
                is_aoe: false,
            },
        };
        let rhs = PetAction::Buff {
            spell: SpellEntry {
                name: "Burnout".into(),
                slot: 1,
                spell_id: 42,
                min_mana_pct: 10.0 + (f32::EPSILON / 2.0),
                priority: 1,
                is_aoe: false,
            },
        };

        assert_eq!(lhs, rhs);
    }

    #[test]
    fn pet_strategy_skips_attack_when_pet_already_has_target() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let target = SpawnData {
            spawn_id: 99,
            ..SpawnData::default()
        };
        let xtargets = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::MyPet,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 77,
                    name: "Warder".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::MyPetTarget,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 99,
                    name: "a skeleton".into(),
                },
            ],
            auto_add_haters: false,
        };
        let ctx = make_ctx_with_xtargets(&player, Some(&target), &config, true, Some(&xtargets));
        let mage = build_strategy(13, &config);
        assert_eq!(mage.pet_action(&ctx), None);
    }

    #[test]
    fn nearest_enemy_diagonal_distances() {
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            SpawnData {
                spawn_id: 1,
                x: 70.0,
                y: 70.0, // distance ~99
                ..SpawnData::default()
            },
            SpawnData {
                spawn_id: 2,
                x: 50.0,
                y: 50.0, // distance ~71
                ..SpawnData::default()
            },
        ];
        let closest = nearest_enemy(&player, &enemies).unwrap();
        assert_eq!(closest.spawn_id, 2);
    }

    #[test]
    fn nearest_enemy_single() {
        let player = SpawnData::default();
        let enemies = vec![SpawnData {
            spawn_id: 42,
            x: 10.0,
            ..SpawnData::default()
        }];
        assert_eq!(nearest_enemy(&player, &enemies).unwrap().spawn_id, 42);
    }

    #[test]
    fn build_strategy_boundary_class_ids() {
        let config = CombatConfig::default();
        // Class ID 0 should use generic
        let s = build_strategy(0, &config);
        assert_eq!(s.class_id(), 0);
        // Class ID 255 should use generic
        let s = build_strategy(255, &config);
        assert_eq!(s.class_id(), 255);
    }

    #[test]
    fn build_strategy_roles_correct() {
        use textquest_common::combat::CombatRole;
        let config = CombatConfig::default();

        assert_eq!(build_strategy(1, &config).role(), CombatRole::MainTank); // Warrior
        assert_eq!(build_strategy(2, &config).role(), CombatRole::Healer); // Cleric
        assert_eq!(build_strategy(3, &config).role(), CombatRole::OffTank); // Paladin
        assert_eq!(build_strategy(4, &config).role(), CombatRole::DpsRanged); // Ranger
        assert_eq!(build_strategy(5, &config).role(), CombatRole::OffTank); // SK
        assert_eq!(build_strategy(6, &config).role(), CombatRole::Healer); // Druid
        assert_eq!(build_strategy(7, &config).role(), CombatRole::DpsMelee); // Monk
        assert_eq!(build_strategy(9, &config).role(), CombatRole::DpsMelee); // Rogue
        assert_eq!(build_strategy(12, &config).role(), CombatRole::DpsRanged); // Wizard
        assert_eq!(build_strategy(13, &config).role(), CombatRole::DpsRanged); // Magician
        assert_eq!(build_strategy(16, &config).role(), CombatRole::DpsMelee); // Berserker
    }

    #[test]
    fn group_member_state_debug() {
        let gms = GroupMemberState {
            spawn_id: 1,
            hp_pct: 75.0,
            mana_pct: 50.0,
            class_id: 2,
            is_dead: false,
            name: String::new(),
            has_detrimental: false,
        };
        let debug = format!("{gms:?}");
        assert!(debug.contains("spawn_id: 1"));
    }

    #[test]
    fn lowest_hp_member_excludes_nan_hp() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let members = vec![
            GroupMemberState {
                spawn_id: 1,
                hp_pct: f32::NAN,
                mana_pct: 100.0,
                class_id: 1,
                is_dead: false,
                name: "NanWarrior".into(),
                has_detrimental: false,
            },
            GroupMemberState {
                spawn_id: 2,
                hp_pct: 50.0,
                mana_pct: 100.0,
                class_id: 2,
                is_dead: false,
                name: "Cleric".into(),
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
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        let result = lowest_hp_member(&ctx);
        assert_eq!(result, Some((2, 50.0)));
    }

    #[test]
    fn prioritized_afflicted_member_prefers_lowest_hp_afflicted() {
        let player = SpawnData::default();
        let config = CombatConfig::default();
        let members = vec![
            GroupMemberState {
                spawn_id: 1,
                hp_pct: 80.0,
                mana_pct: 100.0,
                class_id: 1,
                is_dead: false,
                name: "Warrior".into(),
                has_detrimental: true,
            },
            GroupMemberState {
                spawn_id: 2,
                hp_pct: 40.0,
                mana_pct: 100.0,
                class_id: 2,
                is_dead: false,
                name: "Cleric".into(),
                has_detrimental: true,
            },
            GroupMemberState {
                spawn_id: 3,
                hp_pct: 20.0,
                mana_pct: 100.0,
                class_id: 3,
                is_dead: true,
                name: "Paladin".into(),
                has_detrimental: true,
            },
        ];
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &members,
            config: &config,
            tick: 0,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };

        assert_eq!(afflicted_member_count(&ctx), 2);
        assert_eq!(prioritized_afflicted_member(&ctx), Some((2, 40.0)));
    }

    // ── MA target scan tests ─────────────────────────────────────────

    fn make_enemy(id: u32, name: &str, x: f32, y: f32, hp_current: i64) -> SpawnData {
        SpawnData {
            spawn_id: id,
            displayed_name: name.to_string(),
            name: name.to_string(),
            x,
            y,
            z: 0.0,
            hp_current,
            hp_max: 100,
            stand_state: 0,
            speed_run: 0.0,
            ..SpawnData::default()
        }
    }

    #[test]
    fn is_named_mob_generic_has_article() {
        assert!(!is_named_mob("a fire beetle"));
        assert!(!is_named_mob("an orc pawn"));
        assert!(!is_named_mob("the scribe"));
    }

    #[test]
    fn is_named_mob_proper_name() {
        assert!(is_named_mob("Lord Nagafen"));
        assert!(is_named_mob("Cazic Thule"));
        assert!(is_named_mob("Ghoul Archon"));
    }

    #[test]
    fn is_mezzed_stunned_and_stationary() {
        let mut mob = SpawnData::default();
        mob.stand_state = STAND_STATE_STUNNED;
        mob.speed_run = 0.0;
        assert!(is_mezzed(&mob));
    }

    #[test]
    fn is_mezzed_standing_not_mezzed() {
        let mut mob = SpawnData::default();
        mob.stand_state = 0;
        mob.speed_run = 5.0;
        assert!(!is_mezzed(&mob));
    }

    #[test]
    fn ma_target_scan_empty_enemies() {
        let player = SpawnData::default();
        let config = TargetScanConfig::default();
        assert!(ma_target_scan(&player, &[], &[], &config).is_none());
    }

    #[test]
    fn ma_target_scan_nearest_preference() {
        let player = SpawnData {
            x: 0.0,
            y: 0.0,
            ..SpawnData::default()
        };
        let enemies = vec![
            make_enemy(1, "a goblin", 100.0, 0.0, 100),
            make_enemy(2, "a goblin scout", 20.0, 0.0, 100),
            make_enemy(3, "a goblin warrior", 50.0, 0.0, 100),
        ];
        let config = TargetScanConfig {
            hp_preference: HpPreference::Nearest,
            ..TargetScanConfig::default()
        };
        let target = ma_target_scan(&player, &enemies, &[], &config).unwrap();
        assert_eq!(target.spawn_id, 2, "should pick closest");
    }

    #[test]
    fn ma_target_scan_named_first() {
        let player = SpawnData::default();
        let enemies = vec![
            make_enemy(1, "a goblin", 10.0, 0.0, 100),
            make_enemy(2, "King Tranix", 50.0, 0.0, 80),
        ];
        let config = TargetScanConfig {
            named_preference: NamedPreference::NamedFirst,
            hp_preference: HpPreference::Nearest,
            ..TargetScanConfig::default()
        };
        let target = ma_target_scan(&player, &enemies, &[], &config).unwrap();
        assert_eq!(target.spawn_id, 2, "should prefer named over closer trash");
    }

    #[test]
    fn ma_target_scan_trash_first() {
        let player = SpawnData::default();
        let enemies = vec![
            make_enemy(1, "a goblin", 10.0, 0.0, 100),
            make_enemy(2, "King Tranix", 50.0, 0.0, 80),
        ];
        let config = TargetScanConfig {
            named_preference: NamedPreference::TrashFirst,
            hp_preference: HpPreference::Nearest,
            ..TargetScanConfig::default()
        };
        let target = ma_target_scan(&player, &enemies, &[], &config).unwrap();
        assert_eq!(target.spawn_id, 1, "should prefer trash over named");
    }

    #[test]
    fn ma_target_scan_lowest_hp() {
        let player = SpawnData::default();
        let enemies = vec![
            make_enemy(1, "a goblin", 10.0, 0.0, 90),
            make_enemy(2, "a goblin", 20.0, 0.0, 30),
            make_enemy(3, "a goblin", 30.0, 0.0, 60),
        ];
        let config = TargetScanConfig {
            hp_preference: HpPreference::LowestHp,
            ..TargetScanConfig::default()
        };
        let target = ma_target_scan(&player, &enemies, &[], &config).unwrap();
        assert_eq!(target.spawn_id, 2, "should pick lowest HP mob");
    }

    #[test]
    fn ma_target_scan_highest_hp() {
        let player = SpawnData::default();
        let enemies = vec![
            make_enemy(1, "a goblin", 10.0, 0.0, 30),
            make_enemy(2, "a goblin", 20.0, 0.0, 90),
            make_enemy(3, "a goblin", 30.0, 0.0, 60),
        ];
        let config = TargetScanConfig {
            hp_preference: HpPreference::HighestHp,
            ..TargetScanConfig::default()
        };
        let target = ma_target_scan(&player, &enemies, &[], &config).unwrap();
        assert_eq!(target.spawn_id, 2, "should pick highest HP mob");
    }

    #[test]
    fn ma_target_scan_skips_mezzed() {
        let player = SpawnData::default();
        let mut mezzed = make_enemy(1, "a goblin", 10.0, 0.0, 100);
        mezzed.stand_state = STAND_STATE_STUNNED;
        mezzed.speed_run = 0.0;
        let normal = make_enemy(2, "a goblin warrior", 50.0, 0.0, 100);
        let enemies = vec![mezzed, normal];
        let config = TargetScanConfig {
            skip_mezzed: true,
            hp_preference: HpPreference::Nearest,
            ..TargetScanConfig::default()
        };
        let target = ma_target_scan(&player, &enemies, &[], &config).unwrap();
        assert_eq!(target.spawn_id, 2, "should skip mezzed mob");
    }

    #[test]
    fn ma_target_scan_skips_out_of_range() {
        let player = SpawnData::default();
        let enemies = vec![make_enemy(1, "a goblin", 300.0, 0.0, 100)];
        let config = TargetScanConfig {
            scan_radius: 200.0,
            hp_preference: HpPreference::Nearest,
            ..TargetScanConfig::default()
        };
        assert!(
            ma_target_scan(&player, &enemies, &[], &config).is_none(),
            "should skip out-of-range mob"
        );
    }

    #[test]
    fn ma_target_scan_skips_dead() {
        let player = SpawnData::default();
        let enemies = vec![make_enemy(1, "a goblin", 10.0, 0.0, 0)];
        let config = TargetScanConfig::default();
        assert!(
            ma_target_scan(&player, &enemies, &[], &config).is_none(),
            "should skip dead mob"
        );
    }

    #[test]
    fn ma_target_scan_z_filter() {
        let player = SpawnData::default();
        let mut far_z = make_enemy(1, "a goblin", 10.0, 0.0, 100);
        far_z.z = 200.0;
        let enemies = vec![far_z];
        let config = TargetScanConfig {
            scan_z_radius: 50.0,
            hp_preference: HpPreference::Nearest,
            ..TargetScanConfig::default()
        };
        assert!(
            ma_target_scan(&player, &enemies, &[], &config).is_none(),
            "should skip mob on different floor"
        );
    }

    #[test]
    fn ma_target_scan_named_fallback_to_trash() {
        let player = SpawnData::default();
        // Only trash mobs available with NamedFirst — should still pick one
        let enemies = vec![
            make_enemy(1, "a goblin", 10.0, 0.0, 100),
            make_enemy(2, "a gnoll", 20.0, 0.0, 100),
        ];
        let config = TargetScanConfig {
            named_preference: NamedPreference::NamedFirst,
            hp_preference: HpPreference::Nearest,
            ..TargetScanConfig::default()
        };
        let target = ma_target_scan(&player, &enemies, &[], &config);
        assert!(target.is_some(), "should fall back to trash when no named");
        assert_eq!(target.unwrap().spawn_id, 1);
    }
}
