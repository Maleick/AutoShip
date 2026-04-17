//! Combatant FSM — the per-character combat state machine.
//!
//! Each injected DLL runs one `Combatant` that drives a single EQ character
//! through the Idle → Engaging → Casting → `OnGcd` → Engaging loop, with
//! `HolyShit` emergency overrides evaluated every tick before the normal
//! rotation.

use std::{collections::HashMap, sync::atomic::Ordering};

use textquest_common::{
    combat::{
        ActionType, CastResult, CombatConfig, CombatRole, CombatStatus, HolyShitAction,
        ResolvedAbility, SpellEntry,
    },
    nav::Waypoint,
    shared_client_state::SharedClientState,
    types::SpawnData,
};

use super::{
    ability_cooldowns::AbilityCooldownTracker,
    dot_tracker::DotTracker,
    gcd::GcdTracker,
    holyshit::HolyShitEvaluator,
    humanize::CombatPersonality,
    mana::ManaGovernor,
    rotation::{self, RotationGroup},
    skill_cooldowns::{SkillCooldownTracker, default_cooldown},
    strategy::{
        AbilityResolution, ClassStrategy, CombatContext, GroupMemberState, PetAction, PetStatus,
        build_strategy, pet_attack_focused, pet_back_off,
    },
    toon_config,
};

/// Maximum spell range in EQ units. Spells beyond this distance will not fire.
const MAX_SPELL_RANGE: f32 = 200.0;

/// Pet classes that should issue `/pet attack` on engage.
const PET_CLASSES: &[u8] = &[5, 10, 11, 13, 15]; // SK, Shaman, Necro, Mage, Beastlord

/// Internal FSM states — not exposed outside this module.
/// The public-facing status uses `CombatStatus` from textquest-common.
enum CombatState {
    Idle,
    Engaging {
        target_id: u32,
    },
    Casting {
        spell_slot: u8,
        /// Original spell ID passed to `cast_spell` — retained so retries can
        /// re-issue the same spell even when the cast source was
        /// `SpellCastSource::SpellIdDirect` (gem_id 0 + explicit spell_id).
        spell_id: i32,
        target_id: u32,
        ticks_remaining: u32,
        /// Number of times this spell slot has been attempted (incremented on
        /// retryable failures before the retry fires).
        retry_count: u8,
        /// Number of idle ticks to wait before the next retry attempt.
        backoff_ticks: u32,
    },
    OnGcd,
    Recovering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpellCastSource {
    PreferredGem,
    FallbackGem,
    SpellIdDirect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlannedSpellCast {
    gem_id: u8,
    spell_id: i32,
    source: SpellCastSource,
}

/// Normalize a user/config spell slot into a safe EQ gem index.
///
/// Canonical FFI gem IDs are 0-based (0-12). For backward compatibility,
/// slot 13 is treated as 1-based and normalized to gem 12.
fn normalize_gem_id(slot: u8) -> Option<u8> {
    match slot {
        0..=12 => Some(slot),
        13 => {
            tracing::warn!(
                slot,
                "Normalizing legacy 1-based spell slot to 0-based gem index"
            );
            Some(12)
        }
        _ => None,
    }
}

fn plan_spell_cast(
    preferred_slot: Option<u8>,
    spell_id: i32,
    memorized_spells: &[i32],
) -> Option<PlannedSpellCast> {
    if let Some(slot) = preferred_slot {
        let gem_id = normalize_gem_id(slot)?;
        if spell_id <= 0 {
            return Some(PlannedSpellCast {
                gem_id,
                spell_id,
                source: SpellCastSource::PreferredGem,
            });
        }
        if memorized_spells.get(gem_id as usize).copied() == Some(spell_id) {
            return Some(PlannedSpellCast {
                gem_id,
                spell_id,
                source: SpellCastSource::PreferredGem,
            });
        }
    } else if spell_id <= 0 {
        return None;
    }

    if spell_id > 0
        && let Some((gem_id, _)) = memorized_spells
            .iter()
            .enumerate()
            .find(|(_, memorized_spell_id)| **memorized_spell_id == spell_id)
    {
        return Some(PlannedSpellCast {
            gem_id: gem_id as u8,
            spell_id,
            source: SpellCastSource::FallbackGem,
        });
    }

    Some(PlannedSpellCast {
        gem_id: 0,
        spell_id,
        source: SpellCastSource::SpellIdDirect,
    })
}

fn preferred_spell_slot(spell: &SpellEntry, memorized_spells: &[i32]) -> Option<u8> {
    // Strategy-generated spells sometimes use slot 0 as a placeholder when no
    // preferred gem is known. If gem 0 does not actually hold the spell, drop
    // the preference and let the planner search memorized gems normally.
    if spell.spell_id > 0
        && spell.slot == 0
        && memorized_spells.first().copied() != Some(spell.spell_id)
    {
        return None;
    }

    Some(spell.slot)
}

/// Build a safe `/useitem` slash command for an item name.
///
/// EQ item names commonly contain spaces, so they are quoted. Quotes and
/// control characters are stripped to avoid malformed commands or command
/// injection.
fn use_item_command(item_name: &str) -> Option<String> {
    let sanitized = item_name
        .chars()
        .filter(|ch| !ch.is_control() && *ch != '"')
        .collect::<String>();
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(format!("/useitem \"{trimmed}\""))
    }
}

/// Stable key for tracking item-action retry windows in the ability cooldown
/// map.
///
/// `ActionType::Item` currently carries only a display string, so the runtime
/// needs a deterministic surrogate key to reuse the existing integer-keyed
/// cooldown tracker. FNV-1a is tiny, stable across runs, and sufficient for
/// best-effort retry throttling; a collision would only cause two clickies to
/// share a retry window, which is acceptable until real item IDs are wired in.
/// The sign bit is masked off so the result always fits the positive `i32`
/// keyspace used elsewhere by the ability tracker.
fn item_action_key(item_name: &str) -> i32 {
    let mut hash = 0x811C_9DC5u32;
    for byte in item_name.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    (hash & 0x7FFF_FFFF) as i32
}

/// Stable key for spell/song rotation cooldown tracking.
///
/// Spell entries default to target-scoped cooldowns so debuffs like Tash and
/// Slow can fire once per mob instead of spamming every frame. Self-targeted
/// actions resolve to the player spawn ID, so their cooldown keys remain stable
/// for the owning character without leaking across targets.
fn rotation_spell_key(entry_name: &str, target_id: u32) -> i32 {
    let scope = if target_id == 0 {
        entry_name.to_string()
    } else {
        format!("{entry_name}:{target_id}")
    };

    let mut hash = 0x811C_9DC5u32;
    for byte in scope.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    (hash & 0x7FFF_FFFF) as i32
}

fn normalize_action_name(action_name: &str) -> String {
    action_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn item_cooldown_ticks(item_name: &str) -> Option<u32> {
    match normalize_action_name(item_name).as_str() {
        "rodofmysticaltransvergence" => Some(300 * 20),
        _ => None,
    }
}

fn rotation_action_target<'a>(ctx: &'a CombatContext<'_>, target_id: u32) -> Option<&'a SpawnData> {
    if target_id == 0 {
        return None;
    }

    ctx.target
        .filter(|target| target.spawn_id == target_id)
        .or_else(|| {
            ctx.nearby_enemies
                .iter()
                .find(|spawn| spawn.spawn_id == target_id)
        })
}

fn should_retarget_spell_target(
    target_id: u32,
    current_target_id: Option<u32>,
    player_id: u32,
) -> bool {
    target_id != 0 && target_id != player_id && current_target_id != Some(target_id)
}

struct RotationActionRuntime<'a> {
    resolved_abilities: &'a HashMap<String, AbilityResolution>,
    ability_cooldowns: &'a mut AbilityCooldownTracker,
    skill_cooldowns: &'a mut SkillCooldownTracker,
    gcd: &'a mut GcdTracker,
    personality: &'a mut CombatPersonality,
    active_cast_entry: &'a mut Option<String>,
    active_cast_cooldown_ticks: &'a mut Option<u32>,
    tick_count: u32,
    state: &'a mut CombatState,
}

fn execute_rotation_action(
    groups: &mut Option<Vec<RotationGroup>>,
    ctx: &CombatContext,
    selected_spell_target: Option<u32>,
    runtime: &mut RotationActionRuntime<'_>,
) -> bool {
    let Some(groups) = groups.as_mut() else {
        return false;
    };
    let ability_cooldowns = &*runtime.ability_cooldowns;
    let tick_count = runtime.tick_count;
    let mut rotation_entry_ready =
        |entry: &rotation::RotationEntry, target_id: u32| match entry.action_type {
            ActionType::Spell(_) | ActionType::Song(_) => entry.cooldown_ticks.is_none_or(|_| {
                ability_cooldowns.can_use(
                    rotation_spell_key(&entry.name, target_id),
                    None,
                    tick_count,
                )
            }),
            _ => true,
        };
    let Some(action) = rotation::execute_rotations_filtered_with_strategy_target(
        groups,
        ctx,
        selected_spell_target,
        &mut rotation_entry_ready,
    ) else {
        return false;
    };

    let (spell_id, resolved_name, cooldown_ticks, shared_cooldown_key, shared_cooldown_ticks) =
        if let Some(resolved) = runtime.resolved_abilities.get(&action.entry_name) {
            (
                resolved.spell_id,
                resolved.ability_name.as_str(),
                resolved.cooldown_ticks,
                resolved.shared_cooldown_key.as_deref(),
                resolved.shared_cooldown_ticks,
            )
        } else {
            (0, action.entry_name.as_str(), None, None, None)
        };

    tracing::debug!(
        entry = %action.entry_name,
        resolved = %resolved_name,
        spell_id,
        target = action.target_id,
        "Rotation engine selected action"
    );

    match &action.action_type {
        ActionType::Spell(_) | ActionType::Song(_) => {
            if let Some(target_spawn) = rotation_action_target(ctx, action.target_id) {
                let dist = Waypoint::new(ctx.player.x, ctx.player.y, ctx.player.z).distance_3d(
                    &Waypoint::new(target_spawn.x, target_spawn.y, target_spawn.z),
                );
                if dist > MAX_SPELL_RANGE {
                    tracing::debug!(
                        entry = %action.entry_name,
                        target_id = action.target_id,
                        dist,
                        "Rotation spell target out of range"
                    );
                    return true;
                }
            }
            if should_retarget_spell_target(
                action.target_id,
                ctx.target.map(|target| target.spawn_id),
                ctx.player.spawn_id,
            ) {
                tracing::debug!(
                    entry = %action.entry_name,
                    target_id = action.target_id,
                    "Retargeting for rotation spell"
                );
                crate::eq::slash_command(&format!("/target id {}", action.target_id));
            }
            if spell_id <= 0 {
                tracing::warn!(
                    entry = %action.entry_name,
                    action = ?action.action_type,
                    "Skipping unresolved spell/song from rotation"
                );
                return true;
            }
            let memorized_spells = crate::eq::read_memorized_spells();
            let Some(cast_plan) = plan_spell_cast(None, spell_id, &memorized_spells) else {
                tracing::warn!(
                    entry = %action.entry_name,
                    spell_id,
                    action = ?action.action_type,
                    "Skipping spell/song from rotation because no valid cast plan was found"
                );
                return true;
            };
            crate::eq::cast_spell(cast_plan.gem_id, cast_plan.spell_id);
            let cast_delay = u32::from(runtime.personality.next_cast_delay());
            runtime.gcd.consume();
            *runtime.active_cast_entry = Some(action.entry_name.clone());
            *runtime.active_cast_cooldown_ticks = action.cooldown_ticks;
            *runtime.state = CombatState::Casting {
                spell_slot: cast_plan.gem_id,
                spell_id: cast_plan.spell_id,
                target_id: action.target_id,
                ticks_remaining: 20 + cast_delay,
                backoff_ticks: 0,
                retry_count: 0,
            };
        }
        ActionType::Disc(_) | ActionType::AA(_) => {
            if spell_id <= 0 {
                tracing::warn!(
                    entry = %action.entry_name,
                    action = ?action.action_type,
                    "Skipping unresolved activated ability from rotation"
                );
                return true;
            }
            if !runtime
                .ability_cooldowns
                .can_use(spell_id, shared_cooldown_key, runtime.tick_count)
            {
                tracing::debug!(
                    entry = %action.entry_name,
                    spell_id,
                    "Activated rotation ability blocked by cooldown metadata"
                );
                return true;
            }
            crate::eq::do_combat_ability(spell_id, true);
            runtime.ability_cooldowns.consume(
                spell_id,
                cooldown_ticks,
                shared_cooldown_key,
                shared_cooldown_ticks,
                runtime.tick_count,
            );
            runtime.gcd.consume();
            *runtime.active_cast_entry = None;
            *runtime.active_cast_cooldown_ticks = None;
            *runtime.state = CombatState::OnGcd;
        }
        ActionType::Ability(ability_name) => {
            let Some(skill_id) = combat_skill_id(ability_name) else {
                tracing::warn!(
                    ability = %ability_name,
                    entry = %action.entry_name,
                    "Skipping unknown combat skill from rotation"
                );
                return true;
            };
            if !runtime.skill_cooldowns.is_ready(skill_id) {
                tracing::debug!(
                    skill_id,
                    ability = %ability_name,
                    "Rotation skill blocked by cooldown"
                );
                return true;
            }
            crate::eq::use_skill(skill_id, None);
            if let Some(cooldown) = default_cooldown(skill_id) {
                runtime.skill_cooldowns.consume(skill_id, cooldown);
            }
            runtime.gcd.consume();
            *runtime.active_cast_entry = None;
            *runtime.active_cast_cooldown_ticks = None;
            *runtime.state = CombatState::OnGcd;
        }
        ActionType::Item(item_name) => {
            let Some(command) = use_item_command(item_name) else {
                tracing::warn!(
                    entry = %action.entry_name,
                    "Skipping item rotation with empty sanitized name"
                );
                return true;
            };
            let item_key = item_action_key(item_name);
            if !runtime
                .ability_cooldowns
                .can_use(item_key, None, runtime.tick_count)
            {
                tracing::debug!(
                    entry = %action.entry_name,
                    item = %item_name,
                    "Rotation item blocked by cooldown metadata"
                );
                return true;
            }
            crate::eq::slash_command(&command);
            runtime.ability_cooldowns.consume(
                item_key,
                item_cooldown_ticks(item_name),
                None,
                None,
                runtime.tick_count,
            );
            runtime.gcd.consume();
            *runtime.active_cast_entry = None;
            *runtime.active_cast_cooldown_ticks = None;
            *runtime.state = CombatState::OnGcd;
        }
    }

    true
}

const COMBAT_SKILL_ID_TAUNT: u32 = 73;
const COMBAT_SKILL_ID_BASH: u32 = 10;
const COMBAT_SKILL_ID_KICK: u32 = 30;
const COMBAT_SKILL_ID_FLYING_KICK: u32 = 26;
const COMBAT_SKILL_ID_ROUND_KICK: u32 = 38;
const COMBAT_SKILL_ID_TIGER_CLAW: u32 = 52;
const COMBAT_SKILL_ID_EAGLE_STRIKE: u32 = 23;
const COMBAT_SKILL_ID_BACKSTAB: u32 = 8;

const COMBAT_SKILL_IDS: &[(&str, u32)] = &[
    ("taunt", COMBAT_SKILL_ID_TAUNT),
    ("bash", COMBAT_SKILL_ID_BASH),
    ("kick", COMBAT_SKILL_ID_KICK),
    ("flyingkick", COMBAT_SKILL_ID_FLYING_KICK),
    ("roundkick", COMBAT_SKILL_ID_ROUND_KICK),
    ("tigerclaw", COMBAT_SKILL_ID_TIGER_CLAW),
    ("eaglestrike", COMBAT_SKILL_ID_EAGLE_STRIKE),
    ("backstab", COMBAT_SKILL_ID_BACKSTAB),
];

const WARRIOR_MELEE_SKILLS: &[u32] = &[COMBAT_SKILL_ID_TAUNT, COMBAT_SKILL_ID_KICK];
const BARD_MELEE_SKILLS: &[u32] = &[COMBAT_SKILL_ID_KICK];
const PALADIN_MELEE_SKILLS: &[u32] = &[
    COMBAT_SKILL_ID_TAUNT,
    COMBAT_SKILL_ID_BASH,
    COMBAT_SKILL_ID_KICK,
];
const SHADOW_KNIGHT_MELEE_SKILLS: &[u32] = &[
    COMBAT_SKILL_ID_TAUNT,
    COMBAT_SKILL_ID_BASH,
    COMBAT_SKILL_ID_KICK,
];
const MONK_MELEE_SKILLS: &[u32] = &[
    COMBAT_SKILL_ID_FLYING_KICK,
    COMBAT_SKILL_ID_ROUND_KICK,
    COMBAT_SKILL_ID_TIGER_CLAW,
    COMBAT_SKILL_ID_EAGLE_STRIKE,
];
const ROGUE_MELEE_SKILLS: &[u32] = &[COMBAT_SKILL_ID_BACKSTAB];
const BEASTLORD_MELEE_SKILLS: &[u32] = &[COMBAT_SKILL_ID_KICK, COMBAT_SKILL_ID_FLYING_KICK];
const DEFAULT_MELEE_SKILLS: &[u32] = &[COMBAT_SKILL_ID_KICK];

fn lookup_combat_skill_id(normalized_action_name: &str) -> Option<u32> {
    COMBAT_SKILL_IDS
        .iter()
        .find_map(|(name, id)| (*name == normalized_action_name).then_some(*id))
}

fn combat_skill_id(action_name: &str) -> Option<u32> {
    let normalized_action_name = normalize_action_name(action_name);
    lookup_combat_skill_id(&normalized_action_name)
}

/// The main combat state machine for a single EQ character.
pub struct Combatant {
    state: CombatState,
    strategy: Box<dyn ClassStrategy>,
    personality: CombatPersonality,
    gcd: GcdTracker,
    mana_governor: ManaGovernor,
    holyshit: HolyShitEvaluator,
    assist_target: Option<u32>,
    /// Set when a `HolyShit` Flee action fires. The orchestrator checks this
    /// via `status()` (which returns `CombatStatus::Fleeing`) to know it
    /// should send a flee waypoint to the navigator.
    flee_requested: bool,
    /// True when we just entered Engaging state — triggers `on_engage`
    /// callback.
    needs_on_engage: bool,
    /// Group member snapshots, populated by the orchestrator via IPC.
    /// Required for healer strategies (cleric, druid, shaman) to select
    /// heal targets. Empty until the orchestrator sends group state updates.
    group_members: Vec<GroupMemberState>,
    /// Full shared roster snapshot received from the orchestrator.
    shared_client_states: Vec<SharedClientState>,
    skill_cooldowns: SkillCooldownTracker,
    /// Cooldown state for disciplines and AA-like activations.
    ability_cooldowns: AbilityCooldownTracker,
    dot_tracker: DotTracker,
    /// Data-driven rotation groups from the class strategy.
    /// When `Some`, the rotation engine is used instead of `select_spell()`.
    rotation_groups: Option<Vec<RotationGroup>>,
    /// Resolved ability sets — maps spell line names to the best available
    /// spell for this character's level.
    resolved_abilities: HashMap<String, AbilityResolution>,
    tick_count: u32,
    client_id: u32,
    config: CombatConfig,
    pending_cast_result: Option<CastResult>,
    last_cast_result: Option<CastResult>,
    active_cast_entry: Option<String>,
    active_cast_cooldown_ticks: Option<u32>,
    toon_actions_loaded: bool,
}

impl Combatant {
    pub fn new(class_id: u8, client_id: u32, mut config: CombatConfig) -> Self {
        let is_healer = matches!(config.role, CombatRole::Healer);
        let strategy = build_strategy(class_id, &config);
        let personality = CombatPersonality::from_client_id(client_id);
        let gcd = GcdTracker::default_gcd();
        let mana_governor = ManaGovernor::new(config.mana_floor, is_healer);
        let holyshit = HolyShitEvaluator::new(config.holyshit_rules.clone());

        tracing::info!(
            class_id,
            client_id,
            holyshit_rules = holyshit.rule_count(),
            "Combatant created"
        );

        // Pre-sort disciplines by priority so tick_disciplines can iterate
        // without cloning or sorting every frame.
        config.disciplines.sort_by_key(|d| d.priority);

        // Initialize data-driven rotation groups from the class strategy.
        let rotation_groups = strategy.rotation_groups();
        if rotation_groups.is_some() {
            tracing::info!(class_id, "Using data-driven rotation engine");
        }

        Self {
            state: CombatState::Idle,
            strategy,
            personality,
            gcd,
            mana_governor,
            holyshit,
            assist_target: None,
            flee_requested: false,
            needs_on_engage: false,
            group_members: Vec::new(),
            shared_client_states: Vec::new(),
            skill_cooldowns: SkillCooldownTracker::new(),
            ability_cooldowns: AbilityCooldownTracker::new(),
            dot_tracker: DotTracker::new(),
            rotation_groups,
            resolved_abilities: HashMap::new(),
            tick_count: 0,
            client_id,
            config,
            pending_cast_result: None,
            last_cast_result: None,
            active_cast_entry: None,
            active_cast_cooldown_ticks: None,
            toon_actions_loaded: false,
        }
    }

    fn try_load_toon_actions(&mut self, player: &SpawnData) {
        if self.toon_actions_loaded {
            return;
        }

        let toon_name = if player.name.trim().is_empty() {
            player.displayed_name.trim()
        } else {
            player.name.trim()
        };

        if toon_name.is_empty() {
            return;
        }

        self.toon_actions_loaded = true;

        match toon_config::load_for_toon(toon_name) {
            Ok(Some((path, toon_config))) => {
                toon_config.apply_to(&mut self.config, &mut self.rotation_groups);
                self.config
                    .disciplines
                    .sort_by_key(|discipline| discipline.priority);
                self.holyshit = HolyShitEvaluator::new(self.config.holyshit_rules.clone());
                tracing::info!(
                    toon = toon_name,
                    path = %path.display(),
                    spells = self.config.spells.len(),
                    disciplines = self.config.disciplines.len(),
                    holyshit_rules = self.config.holyshit_rules.len(),
                    rotation_groups = self.rotation_groups.as_ref().map_or(0, Vec::len),
                    "Loaded per-toon combat actions"
                );
            }
            Ok(None) => {
                tracing::debug!(toon = toon_name, "No per-toon combat action config found");
            }
            Err(error) => {
                tracing::warn!(toon = toon_name, error = %error, "Failed to load per-toon combat action config");
            }
        }
    }

    /// Resolve ability sets for this character's known spells and level.
    ///
    /// Called by the orchestrator when the spell book scan completes
    /// (post-login or on level-up).
    pub fn resolve_abilities(
        &mut self,
        known: &[textquest_common::combat::KnownAbility],
        character_level: u8,
    ) {
        let sets = self.strategy.ability_sets();
        if sets.is_empty() {
            return;
        }
        self.resolved_abilities = self
            .strategy
            .resolve_abilities_for_character(known, character_level);
        let resolved_for_hooks: HashMap<String, ResolvedAbility> = self
            .resolved_abilities
            .iter()
            .map(|(name, resolved)| {
                (
                    name.clone(),
                    ResolvedAbility {
                        set_name: resolved.set_name.clone(),
                        ability_name: resolved.ability_name.clone(),
                        spell_id: resolved.spell_id,
                        min_level: resolved.min_level,
                    },
                )
            })
            .collect();
        self.strategy.on_abilities_resolved(&resolved_for_hooks);
        tracing::info!(
            resolved = self.resolved_abilities.len(),
            total_sets = sets.len(),
            level = character_level,
            "Ability sets resolved"
        );
        for (name, resolved) in &self.resolved_abilities {
            tracing::debug!(
                set = %name,
                ability = %resolved.ability_name,
                spell_id = resolved.spell_id,
                "Resolved ability"
            );
        }
    }

    /// Advance the combat FSM by one frame (~50ms, ~20/sec).
    /// Note: an EQ "game tick" is 6 seconds (~120 frames); this runs every
    /// frame.
    pub fn tick(&mut self, player: &SpawnData, target: Option<&SpawnData>, nearby: &[SpawnData]) {
        self.try_load_toon_actions(player);
        self.tick_count += 1;
        self.gcd.tick();
        self.skill_cooldowns.tick();
        self.ability_cooldowns.tick(self.tick_count);

        // Periodically prune expired DoT entries to prevent unbounded growth.
        // Every 120 ticks (~6 seconds at 20 ticks/sec).
        if self
            .tick_count
            .wrapping_add(self.client_id)
            .is_multiple_of(120)
        {
            self.dot_tracker.prune_expired(self.tick_count);
        }

        // --- Zone/disconnect safety guard ---
        // If we're in an active combat state but our target has vanished (zoned,
        // despawned, server disconnect back to char select), auto-disengage to
        // prevent the FSM from getting stuck in Engaging/Casting forever.
        if matches!(
            self.state,
            CombatState::Engaging { .. } | CombatState::Casting { .. } | CombatState::OnGcd
        ) && target.is_none()
        {
            tracing::warn!("Combat target lost (zone/despawn/disconnect) — auto-disengaging");
            // If we were mid-cast, notify the strategy this was an interrupt (not
            // completion)
            if let CombatState::Casting {
                spell_slot,
                spell_id,
                target_id,
                ..
            } = &self.state
            {
                self.finish_cast(
                    player,
                    None,
                    nearby,
                    false,
                    *spell_slot,
                    *spell_id,
                    *target_id,
                    0,
                    CastResult::Interrupted,
                );
            } else {
                let group_members = std::mem::take(&mut self.group_members);
                let cleanup_ctx = CombatContext {
                    player,
                    target: None,
                    nearby_enemies: nearby,
                    group_members: &group_members,
                    config: &self.config,
                    tick: self.tick_count,
                    in_combat: false,
                    ch_chain_slot: None,
                    active_buffs: &[],
                    buff_info: &[],
                    target_is_mezzed: false,
                    extended_targets: None,
                };
                self.strategy.on_action_complete(&cleanup_ctx);
                self.group_members = group_members;
            }
            crate::eq::toggle_auto_attack(false);
            // Clear DoT tracking — target is gone (zone/despawn/disconnect).
            if let CombatState::Engaging { target_id } = &self.state {
                self.dot_tracker.clear_target(*target_id);
            }
            self.dot_tracker.prune_expired(self.tick_count);
            self.assist_target = None;
            self.flee_requested = false;
            self.state = CombatState::Idle;
            return;
        }

        // Fire melee skills when engaging (independent of GCD/spell casting)
        if matches!(self.state, CombatState::Engaging { .. }) {
            let class_id = self.strategy.class_id();
            if !self.strategy.manages_melee_skills_in_rotation() {
                self.tick_melee_skills(class_id, player);
            }
            self.tick_disciplines(player);
        }

        let eq_base = crate::EQ_BASE.load(Ordering::Acquire);
        let extended_targets = if eq_base == 0 {
            None
        } else {
            unsafe { super::xtarget::read_extended_targets(eq_base) }
        };

        let (current_target_id, pet_status, pet_action) = {
            // Build context snapshot for this tick.
            let ctx = CombatContext {
                player,
                target,
                nearby_enemies: nearby,
                group_members: &self.group_members,
                config: &self.config,
                tick: self.tick_count,
                in_combat: !matches!(self.state, CombatState::Idle | CombatState::Recovering),
                ch_chain_slot: None,
                active_buffs: &[],
                buff_info: &[],
                target_is_mezzed: target.is_some_and(super::strategy::is_mezzed),
                extended_targets: extended_targets.as_ref(),
            };

            // --- Call on_engage when first entering Engaging state ---
            if self.needs_on_engage {
                self.needs_on_engage = false;
                self.strategy.on_engage(&ctx);
            }

            (
                ctx.target.map(|current| current.spawn_id),
                ctx.pet_status(),
                self.strategy.pet_action(&ctx),
            )
        };
        if let Some(action) = pet_action {
            if self.execute_pet_action(current_target_id, pet_status, action) {
                return;
            }
        }

        let ctx = CombatContext {
            player,
            target,
            nearby_enemies: nearby,
            group_members: &self.group_members,
            config: &self.config,
            tick: self.tick_count,
            in_combat: !matches!(self.state, CombatState::Idle | CombatState::Recovering),
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: target.is_some_and(super::strategy::is_mezzed),
            extended_targets: extended_targets.as_ref(),
        };

        // --- HolyShit evaluation (always runs first) ---
        if let Some(action) = self.holyshit.evaluate(&ctx).cloned() {
            // If HolyShit fires while we're mid-cast, notify the strategy that
            // the current cast was interrupted so class-specific state gets cleaned
            // up (e.g., bard melody index, cleric rez_pending).
            if let CombatState::Casting {
                spell_slot,
                spell_id,
                target_id,
                ..
            } = &self.state
            {
                self.finish_cast(
                    player,
                    target,
                    nearby,
                    true,
                    *spell_slot,
                    *spell_id,
                    *target_id,
                    0,
                    CastResult::Interrupted,
                );
            }

            match action {
                HolyShitAction::CastSpell(slot) => {
                    let Some(gem_id) = normalize_gem_id(slot) else {
                        tracing::warn!(slot, "HolyShit: skipping cast with invalid spell slot");
                        return;
                    };

                    tracing::warn!(slot, gem_id, "HolyShit: casting emergency spell");
                    crate::eq::cast_spell(gem_id, 0); // spell_id 0 = use whatever is in the gem
                    self.gcd.consume();
                    self.active_cast_entry = None;
                    self.state = CombatState::Casting {
                        spell_slot: gem_id,
                        spell_id: 0,
                        target_id: target.map(|t| t.spawn_id).unwrap_or(0),
                        ticks_remaining: 20,
                        retry_count: 0,
                        backoff_ticks: 0,
                    };
                    return;
                }
                HolyShitAction::UseAbility(ability_id) => {
                    if !self
                        .ability_cooldowns
                        .can_use(ability_id as i32, None, self.tick_count)
                    {
                        tracing::debug!(
                            ability_id,
                            "HolyShit: ability blocked by cooldown/metadata"
                        );
                        return;
                    }
                    tracing::warn!(ability_id, "HolyShit: using emergency ability");
                    crate::eq::do_combat_ability(ability_id as i32, true);
                    self.ability_cooldowns.consume(
                        ability_id as i32,
                        None,
                        None,
                        None,
                        self.tick_count,
                    );
                    self.gcd.consume();
                    self.state = CombatState::OnGcd;
                    return;
                }
                HolyShitAction::UseItem(item_id) => {
                    tracing::warn!(item_id, "HolyShit: using emergency item");
                    crate::eq::slash_command(&format!("/useitem {item_id}"));
                    self.gcd.consume();
                    self.state = CombatState::OnGcd;
                    return;
                }
                HolyShitAction::Flee => {
                    tracing::warn!("HolyShit: FLEE — disengaging and requesting flee movement");
                    // Notify strategy of combat end so class-specific cleanup runs
                    // (e.g., bard stops /melody).
                    let flee_ctx = CombatContext {
                        player,
                        target,
                        nearby_enemies: nearby,
                        group_members: &self.group_members,
                        config: &self.config,
                        tick: self.tick_count,
                        in_combat: false,
                        ch_chain_slot: None,
                        active_buffs: &[],
                        buff_info: &[],
                        target_is_mezzed: target.is_some_and(super::strategy::is_mezzed),
                        extended_targets: None,
                    };
                    self.strategy.on_action_complete(&flee_ctx);
                    self.assist_target = None;
                    self.flee_requested = true;
                    self.state = CombatState::Idle;
                    return;
                }
            }
        }

        let mut selected_spell_target = None;
        if matches!(self.state, CombatState::Engaging { .. }) {
            if !self.gcd.is_ready() {
                return;
            }

            selected_spell_target = self.strategy.select_target(&ctx);
            let selected_spell_target_spawn = selected_spell_target
                .and_then(|spell_target| rotation_action_target(&ctx, spell_target));

            let range_check_target = if selected_spell_target.is_some() {
                selected_spell_target_spawn
            } else {
                target
            };

            if let Some(t) = range_check_target {
                let dist = Waypoint::new(player.x, player.y, player.z)
                    .distance_3d(&Waypoint::new(t.x, t.y, t.z));
                if dist > MAX_SPELL_RANGE {
                    tracing::debug!(dist, "Target out of spell range, waiting");
                    return;
                }
            }

            let mana_pct = player.mana_pct();
            if !self.mana_governor.can_cast(mana_pct) {
                tracing::debug!(mana_pct, "Mana too low, transitioning to Recovering");
                self.state = CombatState::Recovering;
                return;
            }

            let mut runtime = RotationActionRuntime {
                resolved_abilities: &self.resolved_abilities,
                ability_cooldowns: &mut self.ability_cooldowns,
                skill_cooldowns: &mut self.skill_cooldowns,
                gcd: &mut self.gcd,
                personality: &mut self.personality,
                active_cast_entry: &mut self.active_cast_entry,
                active_cast_cooldown_ticks: &mut self.active_cast_cooldown_ticks,
                tick_count: self.tick_count,
                state: &mut self.state,
            };
            if execute_rotation_action(
                &mut self.rotation_groups,
                &ctx,
                selected_spell_target,
                &mut runtime,
            ) {
                return;
            }
        }

        if self.gcd.is_ready() && matches!(self.state, CombatState::Idle | CombatState::Recovering)
        {
            let mut runtime = RotationActionRuntime {
                resolved_abilities: &self.resolved_abilities,
                ability_cooldowns: &mut self.ability_cooldowns,
                skill_cooldowns: &mut self.skill_cooldowns,
                gcd: &mut self.gcd,
                personality: &mut self.personality,
                active_cast_entry: &mut self.active_cast_entry,
                active_cast_cooldown_ticks: &mut self.active_cast_cooldown_ticks,
                tick_count: self.tick_count,
                state: &mut self.state,
            };
            if execute_rotation_action(&mut self.rotation_groups, &ctx, None, &mut runtime) {
                return;
            }
        }

        // --- Normal state machine ---
        match &mut self.state {
            CombatState::Idle => {
                // Wait for an explicit engage command — do nothing.
            }

            CombatState::Engaging { .. } => {
                // --- Legacy select_spell() path ---
                // Ask strategy for next spell
                if let Some(spell) = self.strategy.select_spell(&ctx) {
                    tracing::debug!(
                        slot = spell.slot,
                        name = %spell.name,
                        "Strategy selected spell"
                    );

                    let memorized_spells = crate::eq::read_memorized_spells();
                    let Some(cast_plan) = plan_spell_cast(
                        preferred_spell_slot(&spell, &memorized_spells),
                        spell.spell_id,
                        &memorized_spells,
                    ) else {
                        tracing::warn!(slot = spell.slot, "Skipping cast with invalid spell slot");
                        return;
                    };

                    if spell.spell_id > 0 {
                        match cast_plan.source {
                            SpellCastSource::PreferredGem => {}
                            SpellCastSource::FallbackGem => {
                                tracing::info!(
                                    requested_slot = spell.slot,
                                    fallback_gem = cast_plan.gem_id,
                                    spell_id = spell.spell_id,
                                    name = %spell.name,
                                    "Configured gem missing spell, falling back to memorized gem"
                                );
                            }
                            SpellCastSource::SpellIdDirect => {
                                tracing::info!(
                                    spell_id = spell.spell_id,
                                    name = %spell.name,
                                    "Spell not memorized in any gem, casting by spell_id for EQ auto-memorization"
                                );
                            }
                        }
                    }

                    if let Some(target_id) = selected_spell_target.filter(|target_id| {
                        should_retarget_spell_target(
                            *target_id,
                            target.map(|current| current.spawn_id),
                            player.spawn_id,
                        )
                    }) {
                        tracing::debug!(target_id, "Retargeting for legacy spell");
                        crate::eq::slash_command(&format!("/target id {target_id}"));
                    }

                    // Call the real EQ CastSpell function via FFI
                    crate::eq::cast_spell(cast_plan.gem_id, cast_plan.spell_id);

                    // Apply humanization delay (cast_start_delay absorbed into cast time)
                    let cast_delay = u32::from(self.personality.next_cast_delay());
                    self.gcd.consume();
                    self.active_cast_entry = None;
                    self.active_cast_cooldown_ticks = None;
                    self.state = CombatState::Casting {
                        spell_slot: cast_plan.gem_id,
                        spell_id: cast_plan.spell_id,
                        target_id: selected_spell_target
                            .or_else(|| target.map(|t| t.spawn_id))
                            .unwrap_or(0),
                        ticks_remaining: 20 + cast_delay, // base ~1s + jitter
                        retry_count: 0,
                        backoff_ticks: 0,
                    };
                }
                // If strategy returns None, stay in Engaging and try next tick.
            }

            CombatState::Casting {
                spell_slot,
                spell_id,
                target_id,
                ticks_remaining,
                retry_count,
                backoff_ticks,
            } => {
                // Consume backoff ticks before resuming normal cast countdown.
                if *backoff_ticks > 0 {
                    *backoff_ticks -= 1;
                    return;
                }

                // Retry casts are scheduled by `finish_cast` after a retryable
                // result. Re-issue the cast once when the retry backoff has
                // completed so the retry actually occurs.
                if *retry_count > 0 && *ticks_remaining == 20 && self.pending_cast_result.is_none()
                {
                    tracing::debug!(
                        spell_slot = *spell_slot,
                        spell_id = *spell_id,
                        target_id = *target_id,
                        retry_count = *retry_count,
                        "Issuing retry cast"
                    );
                    crate::eq::cast_spell(*spell_slot, *spell_id);
                    self.gcd.consume();
                }

                // Healer heal-cancel: if lowest HP member recovered above 85%,
                // duck to interrupt the heal and save mana.
                if matches!(self.config.role, CombatRole::Healer) && *ticks_remaining > 5 {
                    let all_healthy = ctx
                        .group_members
                        .iter()
                        .filter(|m| m.hp_pct > 0.0)
                        .all(|m| m.hp_pct >= 85.0);
                    if all_healthy && !ctx.group_members.is_empty() {
                        tracing::info!("Healer: canceling heal — group HP recovered above 85%");
                        // Duck to interrupt cast (write STANDSTATE=4 briefly)
                        crate::eq::slash_command("/duck");
                        let (ss, sid, tid) = (*spell_slot, *spell_id, *target_id);
                        self.finish_cast(
                            player,
                            target,
                            nearby,
                            true,
                            ss,
                            sid,
                            tid,
                            0,
                            CastResult::Aborted,
                        );
                        return;
                    }
                }

                if let Some(result) = self.pending_cast_result.take() {
                    let (ss, sid, tid, rc) = (*spell_slot, *spell_id, *target_id, *retry_count);
                    self.finish_cast(player, target, nearby, true, ss, sid, tid, rc, result);
                    return;
                }

                if *ticks_remaining == 0 {
                    let (ss, sid, tid) = (*spell_slot, *spell_id, *target_id);
                    self.finish_cast(
                        player,
                        target,
                        nearby,
                        true,
                        ss,
                        sid,
                        tid,
                        0,
                        CastResult::Success,
                    );
                    return;
                }

                *ticks_remaining -= 1;
            }

            CombatState::OnGcd => {
                if self.gcd.is_ready() {
                    // Re-check if we still have an assist target or engagement
                    if self.assist_target.is_some() || target.is_some() {
                        let tid = self
                            .assist_target
                            .or_else(|| target.map(|t| t.spawn_id))
                            .unwrap_or(0);
                        self.state = CombatState::Engaging { target_id: tid };
                    } else {
                        self.state = CombatState::Idle;
                    }
                }
            }

            CombatState::Recovering => {
                let mana_pct = player.mana_pct();

                // Recover until we're above the floor
                if !self.mana_governor.should_med(mana_pct, false) {
                    tracing::debug!(mana_pct, "Mana recovered, transitioning to Idle");
                    self.state = CombatState::Idle;
                }
            }
        }
    }

    /// Return the public-facing combat status for IPC reporting.
    pub fn status(&self) -> CombatStatus {
        if self.flee_requested {
            return CombatStatus::Fleeing;
        }

        match &self.state {
            CombatState::Idle => CombatStatus::Idle,
            CombatState::Engaging { target_id } => CombatStatus::Engaging {
                target_id: *target_id,
            },
            CombatState::Casting {
                spell_slot,
                target_id,
                ..
            } => CombatStatus::Casting {
                spell_slot: *spell_slot,
                target_id: *target_id,
            },
            CombatState::OnGcd => CombatStatus::OnGcd,
            CombatState::Recovering => CombatStatus::Recovering,
        }
    }

    /// Set the main-assist target that this combatant should attack.
    pub fn set_assist_target(&mut self, spawn_id: u32) {
        tracing::info!(spawn_id, "Assist target set");
        self.assist_target = Some(spawn_id);
    }

    /// Begin combat against a specific target.
    /// Issues `/face` to turn toward the target (melee misses without facing),
    /// `/pet attack` for pet classes, and immediate taunt for tanks without
    /// aggro.
    pub fn engage(&mut self, target_id: u32) {
        tracing::info!(target_id, "Engaging target");

        // Face the target so melee attacks connect
        crate::eq::slash_command("/face");

        // Pet classes: send pet to attack with /pet focus for single-target
        let class_id = self.strategy.class_id();
        if PET_CLASSES.contains(&class_id) {
            pet_attack_focused();
            tracing::info!(class_id, "Sent /pet attack + /pet focus");
        }

        // Tanks: immediate taunt to establish aggro on engage
        let role = self.strategy.role();
        if matches!(role, CombatRole::MainTank | CombatRole::OffTank) {
            crate::eq::use_skill(COMBAT_SKILL_ID_TAUNT, None);
            self.skill_cooldowns.consume(
                COMBAT_SKILL_ID_TAUNT,
                super::skill_cooldowns::skill_timers::TAUNT.1,
            );
            tracing::info!("Tank: immediate taunt on engage");
        }

        crate::eq::toggle_auto_attack(true);
        self.needs_on_engage = true;
        self.state = CombatState::Engaging { target_id };
    }

    /// Stop combat — return to idle.
    pub fn disengage(&mut self) {
        tracing::info!("Disengaging from combat");

        // Clear DoT tracking for the current target (target died or we're done).
        if let CombatState::Engaging { target_id } = &self.state {
            self.dot_tracker.clear_target(*target_id);
        }

        // Notify strategy of kill/disengage for state cleanup
        let player = SpawnData::default();
        let ctx = CombatContext {
            player: &player,
            target: None,
            nearby_enemies: &[],
            group_members: &self.group_members,
            config: &self.config,
            tick: self.tick_count,
            in_combat: false,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: false,
            extended_targets: None,
        };
        self.strategy.on_action_complete(&ctx);

        crate::eq::toggle_auto_attack(false);

        // Pet classes: call pet back on disengage so it doesn't pull adds
        let class_id = self.strategy.class_id();
        if PET_CLASSES.contains(&class_id) {
            pet_back_off();
            tracing::info!(class_id, "Sent /pet back on disengage");
        }

        self.assist_target = None;
        self.state = CombatState::Idle;
    }

    /// Update group member snapshots (called when the orchestrator sends group
    /// state). Required for healer strategies to function — without this,
    /// healers have no targets to evaluate.
    pub fn set_group_members(&mut self, members: Vec<GroupMemberState>) {
        self.group_members = members;
    }

    /// Update the full shared client roster, deriving healer-facing member rows
    /// from it.
    pub fn set_shared_client_states(&mut self, states: Vec<SharedClientState>) {
        self.group_members = states
            .iter()
            .filter(|state| state.spawn_id != 0)
            .map(|state| GroupMemberState {
                spawn_id: state.spawn_id,
                hp_pct: state.hp_pct,
                mana_pct: state.mana_pct,
                class_id: state.class_id,
                is_dead: state.is_dead,
                name: state.character_name.clone(),
                has_detrimental: false,
            })
            .collect();
        self.shared_client_states = states;
    }

    /// Whether a `HolyShit` Flee was triggered and not yet acknowledged.
    pub fn flee_requested(&self) -> bool {
        self.flee_requested
    }

    /// Clear the flee flag after the orchestrator has dispatched a flee
    /// waypoint.
    pub fn clear_flee_requested(&mut self) {
        self.flee_requested = false;
    }

    /// Queue a cast outcome parsed from chat/system feedback for the current
    /// cast.
    pub fn observe_chat_message(&mut self, text: &str) -> Option<CastResult> {
        let result = CastResult::from_feedback_message(text)?;
        if matches!(self.state, CombatState::Casting { .. }) {
            self.pending_cast_result = Some(result);
        }
        Some(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_cast(
        &mut self,
        player: &SpawnData,
        target: Option<&SpawnData>,
        nearby: &[SpawnData],
        in_combat: bool,
        spell_slot: u8,
        spell_id: i32,
        target_id: u32,
        retry_count: u8,
        result: CastResult,
    ) {
        let group_members = std::mem::take(&mut self.group_members);
        let ctx = CombatContext {
            player,
            target,
            nearby_enemies: nearby,
            group_members: &group_members,
            config: &self.config,
            tick: self.tick_count,
            in_combat,
            ch_chain_slot: None,
            active_buffs: &[],
            buff_info: &[],
            target_is_mezzed: target.is_some_and(super::strategy::is_mezzed),
            extended_targets: None,
        };

        self.pending_cast_result = None;
        self.last_cast_result = Some(result);
        self.strategy.on_cast_outcome(&ctx, spell_slot, result);
        if result.landed()
            && let (Some(entry_name), Some(cooldown_ticks)) = (
                self.active_cast_entry.as_deref(),
                self.active_cast_cooldown_ticks,
            )
        {
            self.ability_cooldowns.consume(
                rotation_spell_key(entry_name, target_id),
                Some(cooldown_ticks),
                None,
                None,
                self.tick_count,
            );
        }
        self.strategy.on_resolved_action_outcome(
            &ctx,
            self.active_cast_entry.as_deref(),
            spell_id,
            target_id,
            result,
        );

        // Attempts so far = retry_count + 1 (the initial cast).
        let attempts_so_far = retry_count.saturating_add(1);
        let policy = self.config.cast_retry_policy;

        if result.is_retryable() && policy.retry_allowed(attempts_so_far) {
            let new_retry_count = attempts_so_far;
            let backoff = policy.backoff_ticks(new_retry_count);
            tracing::debug!(
                spell_slot,
                spell_id,
                target_id,
                result = %result,
                retry_count = new_retry_count,
                backoff_ticks = backoff,
                "Retryable cast result — scheduling retry"
            );
            self.state = CombatState::Casting {
                spell_slot,
                spell_id,
                target_id,
                ticks_remaining: 20,
                retry_count: new_retry_count,
                backoff_ticks: backoff,
            };
            self.group_members = group_members;
            return;
        }

        self.active_cast_entry = None;
        self.active_cast_cooldown_ticks = None;
        self.strategy.on_action_complete(&ctx);

        if result.is_retryable() {
            tracing::debug!(
                spell_slot,
                target_id,
                result = %result,
                attempts = attempts_so_far,
                max_tries = ?policy.max_tries,
                "Retry limit reached — giving up on this cast"
            );
        } else {
            tracing::debug!(spell_slot, target_id, result = %result, "Cast resolved");
        }

        self.state = match result {
            CastResult::OutOfMana => CombatState::Recovering,
            CastResult::NotReady | CastResult::Pending | CastResult::Recovering => {
                CombatState::Engaging { target_id }
            }
            _ => CombatState::OnGcd,
        };
        self.group_members = group_members;
    }

    /// Fire class-appropriate melee skills (kick, bash, taunt, backstab, etc.)
    /// Called every tick while Engaging. Each skill fires independently as soon
    /// as its individual cooldown expires.
    fn tick_melee_skills(&mut self, class_id: u8, player: &SpawnData) {
        // Endurance check — melee skills cost endurance, don't fire if too low
        let end_pct = if player.endurance_max > 0 {
            (player.endurance_current as f32 / player.endurance_max as f32) * 100.0
        } else {
            100.0
        };
        if end_pct < 10.0 {
            return; // conserve endurance
        }

        // Build the skill list for this class
        let skills: &[u32] = match class_id {
            1 => WARRIOR_MELEE_SKILLS,       // Warrior: taunt, kick
            8 => BARD_MELEE_SKILLS,          // Bard: kick
            3 => PALADIN_MELEE_SKILLS,       // Paladin: taunt, bash, kick
            5 => SHADOW_KNIGHT_MELEE_SKILLS, // Shadow Knight: taunt, bash, kick
            7 => MONK_MELEE_SKILLS,          /* Monk: flying kick, round kick, tiger claw, eagle
            * strike */
            9 => ROGUE_MELEE_SKILLS,      // Rogue: backstab
            15 => BEASTLORD_MELEE_SKILLS, // Beastlord: kick, flying kick
            _ => DEFAULT_MELEE_SKILLS,    // Berserker / Generic: kick
        };

        // Fire each skill independently when its cooldown is ready
        for &skill_id in skills {
            if self.skill_cooldowns.is_ready(skill_id) {
                crate::eq::use_skill(skill_id, None);
                if let Some(cd) = default_cooldown(skill_id) {
                    self.skill_cooldowns.consume(skill_id, cd);
                }
            }
        }
    }

    /// Fire disciplines (combat abilities) when conditions are met.
    /// Called every tick while Engaging. Only one discipline fires per tick
    /// since they share the GCD. Disciplines are evaluated in priority order
    /// (lower priority number = higher priority).
    fn tick_disciplines(&mut self, player: &SpawnData) {
        if self.config.disciplines.is_empty() {
            return;
        }

        let hp_pct = player.hp_pct();
        let end_pct = if player.endurance_max > 0 {
            (player.endurance_current as f32 / player.endurance_max as f32) * 100.0
        } else {
            100.0
        };

        // Disciplines are pre-sorted by priority in new(), iterate directly.
        for disc in &self.config.disciplines {
            let availability = self
                .ability_cooldowns
                .availability(disc.spell_id, self.tick_count);
            if !availability.is_ready_at(self.tick_count) {
                tracing::trace!(
                    name = %disc.name,
                    spell_id = disc.spell_id,
                    ?availability,
                    "Discipline not ready"
                );
                continue;
            }

            // Skip if HP outside valid range
            if hp_pct < disc.min_hp_pct || hp_pct > disc.max_hp_pct {
                continue;
            }

            // Skip if endurance too low
            if end_pct < disc.min_endurance_pct {
                continue;
            }

            tracing::debug!(
                name = %disc.name,
                spell_id = disc.spell_id,
                "Firing discipline"
            );
            let cooldown = if disc.cooldown_ticks > 0 {
                Some(disc.cooldown_ticks)
            } else {
                tracing::debug!(
                    name = %disc.name,
                    spell_id = disc.spell_id,
                    "Discipline cooldown unknown; scheduling fallback retry"
                );
                None
            };
            crate::eq::do_combat_ability(disc.spell_id, true);
            self.ability_cooldowns
                .consume(disc.spell_id, cooldown, None, None, self.tick_count);

            // Only one disc per tick
            return;
        }
    }

    fn execute_pet_action(
        &mut self,
        current_target_id: Option<u32>,
        pet_status: PetStatus,
        action: PetAction,
    ) -> bool {
        match action {
            PetAction::Attack => {
                let Some(target_id) = current_target_id else {
                    return false;
                };
                crate::eq::slash_command("/pet attack");
                crate::eq::slash_command("/pet focus");
                tracing::info!(target_id, "Sent /pet attack + /pet focus");
                false
            }
            PetAction::Buff { spell } => {
                let Some(pet_id) = pet_status.spawn_id else {
                    return false;
                };
                let memorized_spells = crate::eq::read_memorized_spells();
                let Some(cast_plan) = plan_spell_cast(
                    preferred_spell_slot(&spell, &memorized_spells),
                    spell.spell_id,
                    &memorized_spells,
                ) else {
                    tracing::warn!(
                        slot = spell.slot,
                        "Skipping pet buff without a valid cast plan"
                    );
                    return false;
                };

                let original_target = current_target_id;
                if original_target != Some(pet_id) {
                    crate::eq::slash_command(&format!("/target id {pet_id}"));
                }
                crate::eq::cast_spell(cast_plan.gem_id, cast_plan.spell_id);
                if let Some(original_target) = original_target
                    && original_target != pet_id
                {
                    crate::eq::slash_command(&format!("/target id {original_target}"));
                }

                let cast_delay = u32::from(self.personality.next_cast_delay());
                self.gcd.consume();
                self.active_cast_entry = None;
                self.active_cast_cooldown_ticks = None;
                self.state = CombatState::Casting {
                    spell_slot: cast_plan.gem_id,
                    spell_id: cast_plan.spell_id,
                    target_id: pet_id,
                    ticks_remaining: 20 + cast_delay,
                    retry_count: 0,
                    backoff_ticks: 0,
                };
                true
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use crate::combat::{ability_cooldowns::AbilityAvailability, rotation};
    use textquest_common::combat::{
        ActionType, CastRetryPolicy, CombatConfig, CombatRole, SpellEntry,
    };

    struct AllyTargetStrategy {
        spell: SpellEntry,
        target_id: u32,
    }

    impl ClassStrategy for AllyTargetStrategy {
        fn class_id(&self) -> u8 {
            2
        }

        fn select_spell(&self, _ctx: &CombatContext) -> Option<SpellEntry> {
            Some(self.spell.clone())
        }

        fn select_target(&self, _ctx: &CombatContext) -> Option<u32> {
            Some(self.target_id)
        }

        fn should_assist(&self, _ctx: &CombatContext) -> bool {
            false
        }

        fn aoe_threshold(&self) -> u8 {
            u8::MAX
        }

        fn role(&self) -> CombatRole {
            CombatRole::Healer
        }
    }

    fn test_config() -> CombatConfig {
        CombatConfig {
            cast_retry_policy: textquest_common::combat::CastRetryPolicy {
                max_tries: Some(1),
                base_backoff_ticks: 0,
            },
            ..CombatConfig::default()
        }
    }

    fn test_player() -> SpawnData {
        let mut p = SpawnData::default();
        p.name = "TestPlayer".into();
        p.spawn_id = 1;
        p
    }

    fn test_target() -> SpawnData {
        let mut t = SpawnData::default();
        t.name = "TestMob".into();
        t.spawn_id = 100;
        t
    }

    fn item_rotation_group_with(
        item_name: &str,
        target_selector: textquest_common::combat::TargetSelector,
        combat_state_req: textquest_common::combat::CombatStateReq,
    ) -> RotationGroup {
        RotationGroup {
            name: "ItemTest".into(),
            target_selector,
            combat_state_req,
            steps_per_frame: 1,
            full_rotation: false,
            hp_threshold: None,
            entries: vec![rotation::entry(
                "UseClicky",
                ActionType::Item(item_name.to_string()),
            )],
            current_step: 0,
        }
    }

    fn item_rotation_group(item_name: &str) -> RotationGroup {
        item_rotation_group_with(
            item_name,
            textquest_common::combat::TargetSelector::SelfOnly,
            textquest_common::combat::CombatStateReq::Combat,
        )
    }

    fn spell_rotation_group_with_cooldown(
        entry_name: &str,
        cooldown_ticks: u32,
        target_selector: textquest_common::combat::TargetSelector,
    ) -> RotationGroup {
        RotationGroup {
            name: "SpellTest".into(),
            target_selector,
            combat_state_req: textquest_common::combat::CombatStateReq::Combat,
            steps_per_frame: 1,
            full_rotation: false,
            hp_threshold: None,
            entries: vec![rotation::entry_with_cooldown(
                entry_name,
                ActionType::Spell(entry_name.to_string()),
                cooldown_ticks,
            )],
            current_step: 0,
        }
    }

    #[test]
    fn new_combatant_starts_idle() {
        let c = Combatant::new(1, 0, test_config());
        assert!(matches!(c.status(), CombatStatus::Idle));
    }

    #[test]
    fn zone_disconnect_auto_disengages() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = test_player();

        // Force into Engaging state
        c.state = CombatState::Engaging { target_id: 100 };
        c.assist_target = Some(100);

        // Tick with no target (simulates zone/disconnect)
        c.tick(&player, None, &[]);

        // Should have auto-disengaged back to Idle
        assert!(matches!(c.status(), CombatStatus::Idle));
        assert!(c.assist_target.is_none());
    }

    #[test]
    fn zone_disconnect_during_casting_auto_disengages() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = test_player();

        // Force into Casting state
        c.state = CombatState::Casting {
            spell_slot: 1,
            spell_id: 0,
            target_id: 100,
            ticks_remaining: 10,
            retry_count: 0,
            backoff_ticks: 0,
        };

        // Tick with no target
        c.tick(&player, None, &[]);

        assert!(matches!(c.status(), CombatStatus::Idle));
    }

    #[test]
    fn idle_with_no_target_stays_idle() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = test_player();

        // Idle + no target should NOT trigger zone guard (already safe)
        c.tick(&player, None, &[]);

        assert!(matches!(c.status(), CombatStatus::Idle));
    }

    #[test]
    fn engaging_with_target_stays_engaging() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = test_player();
        let target = test_target();

        c.state = CombatState::Engaging { target_id: 100 };

        // Tick WITH target — should stay in combat
        c.tick(&player, Some(&target), &[]);

        assert!(!matches!(c.status(), CombatStatus::Idle));
    }

    #[test]
    fn zone_guard_clears_flee_requested() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = test_player();

        // Simulate: flee was requested, then zone happens while engaging
        c.state = CombatState::Engaging { target_id: 100 };
        c.assist_target = Some(100);
        c.flee_requested = true;

        // Zone/disconnect — target vanishes
        c.tick(&player, None, &[]);

        // flee_requested must be cleared so status() doesn't stick on Fleeing
        assert!(matches!(c.status(), CombatStatus::Idle));
        assert!(!c.flee_requested());
    }

    #[test]
    fn distance_3d_basic() {
        let mut a = SpawnData::default();
        a.x = 0.0;
        a.y = 0.0;
        a.z = 0.0;
        let mut b = SpawnData::default();
        b.x = 3.0;
        b.y = 4.0;
        b.z = 0.0;
        assert!(
            (Waypoint::new(a.x, a.y, a.z).distance_3d(&Waypoint::new(b.x, b.y, b.z)) - 5.0).abs()
                < 0.01
        );
    }

    // --- Discipline tests ---

    use textquest_common::combat::DisciplineEntry;

    fn make_disc(name: &str, spell_id: i32, priority: u8, cooldown: u32) -> DisciplineEntry {
        DisciplineEntry {
            name: name.to_string(),
            spell_id,
            priority,
            cooldown_ticks: cooldown,
            min_hp_pct: 0.0,
            max_hp_pct: 100.0,
            min_endurance_pct: 0.0,
        }
    }

    fn config_with_discs(discs: Vec<DisciplineEntry>) -> CombatConfig {
        let mut cfg = CombatConfig::default();
        cfg.disciplines = discs;
        cfg
    }

    fn player_with_hp_end(hp: i64, hp_max: i64, end: i32, end_max: u32) -> SpawnData {
        let mut p = SpawnData::default();
        p.name = "TestPlayer".into();
        p.spawn_id = 1;
        p.hp_current = hp;
        p.hp_max = hp_max;
        p.endurance_current = end;
        p.endurance_max = end_max;
        p
    }

    #[test]
    fn discipline_fires_when_conditions_met() {
        let cfg = config_with_discs(vec![make_disc("Mighty Strike", 1001, 1, 100)]);
        let mut c = Combatant::new(1, 0, cfg);
        let player = player_with_hp_end(1000, 1000, 500, 500);

        c.state = CombatState::Engaging { target_id: 100 };
        let target = test_target();
        c.tick(&player, Some(&target), &[]);

        // Disc should be on cooldown now (meaning it fired)
        assert_eq!(
            c.ability_cooldowns.availability(1001, c.tick_count),
            AbilityAvailability::CoolingDown(100)
        );
    }

    #[test]
    fn discipline_respects_cooldown() {
        let cfg = config_with_discs(vec![make_disc("Mighty Strike", 1001, 1, 100)]);
        let mut c = Combatant::new(1, 0, cfg);
        let player = player_with_hp_end(1000, 1000, 500, 500);
        let target = test_target();

        c.state = CombatState::Engaging { target_id: 100 };

        // First tick fires the disc
        c.tick(&player, Some(&target), &[]);
        assert_eq!(
            c.ability_cooldowns.availability(1001, c.tick_count),
            AbilityAvailability::CoolingDown(100)
        );

        // Second tick should NOT re-fire (still on cooldown).
        c.state = CombatState::Engaging { target_id: 100 };
        c.tick(&player, Some(&target), &[]);

        // Cooldown should be decremented, not reset to 100
        assert_eq!(
            c.ability_cooldowns.availability(1001, c.tick_count),
            AbilityAvailability::CoolingDown(99)
        );
    }

    #[test]
    fn discipline_respects_hp_range() {
        let mut disc = make_disc("Defensive", 2001, 1, 200);
        disc.min_hp_pct = 20.0;
        disc.max_hp_pct = 50.0;
        let cfg = config_with_discs(vec![disc]);

        // Player at full HP -- should NOT fire (hp_pct = 100%, outside [20, 50])
        let mut c = Combatant::new(1, 0, cfg.clone());
        let player_full = player_with_hp_end(1000, 1000, 500, 500);
        let target = test_target();
        c.state = CombatState::Engaging { target_id: 100 };
        c.tick(&player_full, Some(&target), &[]);
        assert!(
            matches!(
                c.ability_cooldowns.availability(2001, c.tick_count),
                AbilityAvailability::Ready
            ),
            "Should not fire at full HP"
        );

        // Player at 40% HP -- should fire (inside [20, 50])
        let mut c2 = Combatant::new(1, 0, cfg);
        let player_low = player_with_hp_end(400, 1000, 500, 500);
        c2.state = CombatState::Engaging { target_id: 100 };
        c2.tick(&player_low, Some(&target), &[]);
        assert!(
            matches!(
                c2.ability_cooldowns.availability(2001, c2.tick_count),
                AbilityAvailability::CoolingDown(_)
            ),
            "Should fire at 40% HP"
        );
    }

    #[test]
    fn only_one_discipline_fires_per_tick() {
        let cfg = config_with_discs(vec![
            make_disc("Mighty Strike", 1001, 1, 100),
            make_disc("Fellstrike", 1002, 2, 100),
        ]);
        let mut c = Combatant::new(1, 0, cfg);
        let player = player_with_hp_end(1000, 1000, 500, 500);
        let target = test_target();

        c.state = CombatState::Engaging { target_id: 100 };
        c.tick(&player, Some(&target), &[]);

        // Only the higher-priority (lower number) disc should have fired
        assert!(
            matches!(
                c.ability_cooldowns.availability(1001, c.tick_count),
                AbilityAvailability::CoolingDown(_)
            ),
            "Priority 1 disc should fire"
        );
        assert!(
            matches!(
                c.ability_cooldowns.availability(1002, c.tick_count),
                AbilityAvailability::Ready
            ),
            "Priority 2 disc should NOT fire on same tick"
        );
    }

    #[test]
    fn discipline_respects_endurance_minimum() {
        let mut disc = make_disc("Mighty Strike", 1001, 1, 100);
        disc.min_endurance_pct = 50.0;
        let cfg = config_with_discs(vec![disc]);

        // Player with only 10% endurance -- should NOT fire
        let mut c = Combatant::new(1, 0, cfg);
        let player_low_end = player_with_hp_end(1000, 1000, 50, 500);
        let target = test_target();
        c.state = CombatState::Engaging { target_id: 100 };
        c.tick(&player_low_end, Some(&target), &[]);
        assert!(
            matches!(
                c.ability_cooldowns.availability(1001, c.tick_count),
                AbilityAvailability::Ready
            ),
            "Should not fire with low endurance"
        );
    }

    #[test]
    fn bard_melee_skill_enters_shared_kick_cooldown() {
        let mut c = Combatant::new(8, 0, CombatConfig::default());
        let mut player = player_with_hp_end(1000, 1000, 100, 100);
        player.mana_current = 100;
        player.mana_max = 100;
        let target = test_target();

        c.state = CombatState::Engaging { target_id: 100 };
        c.tick(&player, Some(&target), &[]);

        assert!(
            !c.skill_cooldowns.is_ready(COMBAT_SKILL_ID_KICK),
            "Bard kick should enter the shared melee cooldown tracker"
        );
    }

    #[test]
    fn bard_melee_skill_respects_endurance_floor() {
        let mut c = Combatant::new(8, 0, CombatConfig::default());
        let mut player = player_with_hp_end(1000, 1000, 5, 100);
        player.mana_current = 100;
        player.mana_max = 100;
        let target = test_target();

        c.state = CombatState::Engaging { target_id: 100 };
        c.tick(&player, Some(&target), &[]);

        assert!(
            c.skill_cooldowns.is_ready(COMBAT_SKILL_ID_KICK),
            "Bard kick should not fire below the endurance floor"
        );
    }

    #[test]
    fn warrior_rotation_owns_melee_skills() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = player_with_hp_end(1000, 1000, 100, 100);
        let target = test_target();

        c.state = CombatState::Engaging {
            target_id: target.spawn_id,
        };
        c.tick(&player, Some(&target), &[]);

        assert!(
            !c.skill_cooldowns.is_ready(COMBAT_SKILL_ID_TAUNT),
            "Warrior rotation should spend Taunt on the first combat tick"
        );
        assert!(
            c.skill_cooldowns.is_ready(COMBAT_SKILL_ID_KICK),
            "Generic melee skill ticking should not also fire Kick for Warrior"
        );
    }

    #[test]
    fn rotation_disc_consumes_shared_cooldown_metadata() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = player_with_hp_end(1000, 1000, 100, 100);
        let target = test_target();

        c.rotation_groups = Some(vec![RotationGroup {
            name: "Burn".into(),
            target_selector: textquest_common::combat::TargetSelector::AutoTarget,
            combat_state_req: textquest_common::combat::CombatStateReq::Combat,
            steps_per_frame: 1,
            full_rotation: false,
            hp_threshold: None,
            entries: vec![
                rotation::entry("BurnPrimary", ActionType::Disc("BurnPrimary".into())),
                rotation::entry("PrecisionLine", ActionType::Disc("PrecisionLine".into())),
            ],
            current_step: 0,
        }]);
        c.resolved_abilities = std::collections::HashMap::from([
            (
                "BurnPrimary".into(),
                AbilityResolution {
                    set_name: "BurnPrimary".into(),
                    ability_name: "Fellstrike Discipline".into(),
                    spell_id: 4001,
                    min_level: 65,
                    cooldown_ticks: Some(20),
                    shared_cooldown_key: Some("warrior-offense".into()),
                    shared_cooldown_ticks: Some(30),
                },
            ),
            (
                "PrecisionLine".into(),
                AbilityResolution {
                    set_name: "PrecisionLine".into(),
                    ability_name: "Precision Discipline".into(),
                    spell_id: 4002,
                    min_level: 60,
                    cooldown_ticks: Some(20),
                    shared_cooldown_key: Some("warrior-offense".into()),
                    shared_cooldown_ticks: Some(30),
                },
            ),
        ]);

        c.state = CombatState::Engaging {
            target_id: target.spawn_id,
        };
        c.tick(&player, Some(&target), &[]);

        assert_eq!(
            c.ability_cooldowns.availability(4001, c.tick_count),
            AbilityAvailability::CoolingDown(20)
        );
        assert!(
            !c.ability_cooldowns
                .can_use(4002, Some("warrior-offense"), c.tick_count),
            "Shared timer metadata should block the second offensive disc"
        );
    }

    #[test]
    fn status_reflects_fleeing() {
        let mut c = Combatant::new(1, 0, test_config());
        c.flee_requested = true;
        assert!(matches!(c.status(), CombatStatus::Fleeing));
    }

    #[test]
    fn plan_spell_cast_prefers_configured_gem_when_spell_is_loaded() {
        let plan = plan_spell_cast(Some(3), 1500, &[0, 0, 0, 1500, 0]);
        assert_eq!(
            plan,
            Some(PlannedSpellCast {
                gem_id: 3,
                spell_id: 1500,
                source: SpellCastSource::PreferredGem,
            })
        );
    }

    #[test]
    fn plan_spell_cast_falls_back_to_other_matching_gem() {
        let plan = plan_spell_cast(Some(3), 1500, &[0, 1500, 0, 0, 0]);
        assert_eq!(
            plan,
            Some(PlannedSpellCast {
                gem_id: 1,
                spell_id: 1500,
                source: SpellCastSource::FallbackGem,
            })
        );
    }

    #[test]
    fn plan_spell_cast_uses_spell_id_when_not_memorized_anywhere() {
        let plan = plan_spell_cast(Some(3), 1500, &[0, 0, 0, 0, 0]);
        assert_eq!(
            plan,
            Some(PlannedSpellCast {
                gem_id: 0,
                spell_id: 1500,
                source: SpellCastSource::SpellIdDirect,
            })
        );
    }

    #[test]
    fn plan_spell_cast_allows_rotation_spell_without_preferred_slot() {
        let plan = plan_spell_cast(None, 1500, &[0, 0, 1500, 0, 0]);
        assert_eq!(
            plan,
            Some(PlannedSpellCast {
                gem_id: 2,
                spell_id: 1500,
                source: SpellCastSource::FallbackGem,
            })
        );
    }

    #[test]
    fn plan_spell_cast_rejects_invalid_preferred_slot() {
        assert_eq!(plan_spell_cast(Some(99), 1500, &[0, 0, 0]), None);
    }

    #[test]
    fn preferred_spell_slot_ignores_placeholder_zero_when_spell_is_elsewhere() {
        let spell = SpellEntry {
            slot: 0,
            spell_id: 1500,
            name: "StrategySpell".into(),
            min_mana_pct: 0.0,
            priority: 1,
            is_aoe: false,
        };

        assert_eq!(preferred_spell_slot(&spell, &[0, 0, 1500]), None);
        assert_eq!(preferred_spell_slot(&spell, &[1500, 0, 0]), Some(0));
    }

    #[test]
    fn rotation_group_absence_of_action_falls_back_to_magician_spell_selection() {
        let mut c = Combatant::new(13, 0, test_config());
        c.resolve_abilities(
            &[
                textquest_common::combat::KnownAbility {
                    name: "Seeking Flame of Seukor".into(),
                    spell_id: 1715,
                    level: 59,
                },
                textquest_common::combat::KnownAbility {
                    name: "Shock of Steel".into(),
                    spell_id: 1716,
                    level: 60,
                },
                textquest_common::combat::KnownAbility {
                    name: "Mala".into(),
                    spell_id: 1717,
                    level: 60,
                },
            ],
            60,
        );

        let mut player = test_player();
        player.level = 60;
        player.mana_current = 9000;
        player.mana_max = 10000;
        let mut target = test_target();
        target.hp_current = 9900;
        target.hp_max = 10000;

        c.state = CombatState::Engaging {
            target_id: target.spawn_id,
        };
        c.tick(&player, Some(&target), &[]);

        assert!(
            matches!(c.state, CombatState::Casting { spell_id: 1717, .. }),
            "Magician should fall back to spell selection when no downtime item action fires"
        );
    }

    #[test]
    fn item_cooldown_ticks_tracks_magician_mod_rod_reuse() {
        assert_eq!(
            item_cooldown_ticks("Rod of Mystical Transvergence"),
            Some(6000)
        );
        assert_eq!(item_cooldown_ticks("Unknown Clicky"), None);
    }

    #[test]
    fn rotation_spell_cooldown_commits_on_success_not_cast_start() {
        let mut c = Combatant::new(1, 0, test_config());
        c.rotation_groups = Some(vec![spell_rotation_group_with_cooldown(
            "TestDebuff",
            77,
            textquest_common::combat::TargetSelector::AutoTarget,
        )]);
        c.resolved_abilities.insert(
            "TestDebuff".into(),
            ResolvedAbility {
                set_name: "TestSet".into(),
                ability_name: "Test Debuff".into(),
                spell_id: 4242,
                min_level: 1,
            }
            .into(),
        );

        let mut player = test_player();
        player.mana_current = 1000;
        player.mana_max = 1000;
        let target = test_target();

        c.state = CombatState::Engaging {
            target_id: target.spawn_id,
        };
        c.tick(&player, Some(&target), std::slice::from_ref(&target));

        let cooldown_key = rotation_spell_key("TestDebuff", target.spawn_id);
        assert!(
            c.ability_cooldowns.can_use(cooldown_key, None, c.tick_count),
            "rotation cooldown should remain ready until the cast lands"
        );
        assert_eq!(c.active_cast_entry.as_deref(), Some("TestDebuff"));
        assert_eq!(c.active_cast_cooldown_ticks, Some(77));

        let (spell_slot, spell_id, target_id) = match c.state {
            CombatState::Casting {
                spell_slot,
                spell_id,
                target_id,
                ..
            } => (spell_slot, spell_id, target_id),
            _ => panic!("expected rotation spell to enter Casting state"),
        };

        c.finish_cast(
            &player,
            Some(&target),
            std::slice::from_ref(&target),
            true,
            spell_slot,
            spell_id,
            target_id,
            0,
            CastResult::Success,
        );

        assert_eq!(
            c.ability_cooldowns.availability(cooldown_key, c.tick_count),
            AbilityAvailability::CoolingDown(77)
        );
        assert_eq!(c.active_cast_entry, None);
        assert_eq!(c.active_cast_cooldown_ticks, None);
    }

    #[test]
    fn retryable_rotation_cast_preserves_active_entry_until_terminal_result() {
        let mut c = Combatant::new(1, 0, config_with_retry(3, 0));
        let player = test_player();
        let target = test_target();

        c.active_cast_entry = Some("Mez".into());
        c.active_cast_cooldown_ticks = Some(55);
        c.state = CombatState::Casting {
            spell_slot: 2,
            spell_id: 5150,
            target_id: target.spawn_id,
            ticks_remaining: 10,
            retry_count: 0,
            backoff_ticks: 0,
        };

        c.finish_cast(
            &player,
            Some(&target),
            std::slice::from_ref(&target),
            true,
            2,
            5150,
            target.spawn_id,
            0,
            CastResult::Fizzled,
        );

        assert!(
            matches!(c.state, CombatState::Casting { retry_count: 1, .. }),
            "retryable failures should schedule a retry"
        );
        assert_eq!(c.active_cast_entry.as_deref(), Some("Mez"));
        assert_eq!(c.active_cast_cooldown_ticks, Some(55));
        assert!(
            c.ability_cooldowns
                .can_use(rotation_spell_key("Mez", target.spawn_id), None, c.tick_count),
            "retryable failures should not commit the rotation cooldown"
        );
    }

    #[test]
    fn downtime_rotation_groups_execute_while_idle() {
        let mut c = Combatant::new(1, 0, test_config());
        c.rotation_groups = Some(vec![item_rotation_group_with(
            "Rod of Mystical Transvergence",
            textquest_common::combat::TargetSelector::SelfOnly,
            textquest_common::combat::CombatStateReq::Downtime,
        )]);

        c.tick(&test_player(), None, &[]);

        assert!(matches!(c.state, CombatState::OnGcd));
        assert!(
            !c.ability_cooldowns.can_use(
                item_action_key("Rod of Mystical Transvergence"),
                None,
                c.tick_count
            ),
            "Downtime rotation should consume the mod rod cooldown"
        );
    }

    #[test]
    fn executed_rotation_action_does_not_fall_through_to_legacy_spell_selection() {
        let mut c = Combatant::new(13, 0, test_config());
        c.rotation_groups = Some(vec![item_rotation_group("Rod of Mystical Transvergence")]);
        c.config.spells = vec![SpellEntry {
            slot: 1,
            spell_id: 1717,
            name: "Mala".into(),
            min_mana_pct: 0.0,
            priority: 1,
            is_aoe: false,
        }];

        let player = test_player();
        let target = test_target();
        c.state = CombatState::Engaging {
            target_id: target.spawn_id,
        };

        c.tick(&player, Some(&target), &[]);

        assert!(
            matches!(c.state, CombatState::OnGcd),
            "A successful rotation action should end the tick before legacy spell selection runs"
        );
    }

    #[test]
    fn legacy_spell_targeting_does_not_use_far_enemy_for_ally_selected_casts() {
        let mut c = Combatant::new(2, 0, test_config());
        c.strategy = Box::new(AllyTargetStrategy {
            spell: SpellEntry {
                slot: 1,
                spell_id: 5150,
                name: "Test Heal".into(),
                min_mana_pct: 0.0,
                priority: 1,
                is_aoe: false,
            },
            target_id: 7,
        });

        let player = test_player();
        let mut far_enemy = test_target();
        far_enemy.x = MAX_SPELL_RANGE + 50.0;

        c.state = CombatState::Engaging {
            target_id: far_enemy.spawn_id,
        };
        c.tick(&player, Some(&far_enemy), &[]);

        assert!(
            matches!(
                c.state,
                CombatState::Casting {
                    spell_id: 5150,
                    target_id: 7,
                    ..
                }
            ),
            "ally-targeted legacy casts should proceed even when the current enemy target is out of range"
        );
    }

    #[test]
    fn combat_skill_id_maps_known_rotation_skills() {
        assert_eq!(combat_skill_id("Taunt"), Some(73));
        assert_eq!(combat_skill_id("Kick"), Some(30));
        assert_eq!(combat_skill_id("Flying Kick"), Some(26));
        assert_eq!(combat_skill_id("Backstab"), Some(8));
    }

    #[test]
    fn combat_skill_id_rejects_unknown_rotation_skills() {
        assert_eq!(combat_skill_id("Mystery Skill"), None);
    }

    #[test]
    fn clear_flee_requested_restores_idle() {
        let mut c = Combatant::new(1, 0, test_config());
        c.flee_requested = true;
        c.clear_flee_requested();
        assert!(!c.flee_requested());
        assert!(matches!(c.status(), CombatStatus::Idle));
    }

    #[test]
    fn set_assist_target_stores_id() {
        let mut c = Combatant::new(1, 0, test_config());
        c.set_assist_target(42);
        assert_eq!(c.assist_target, Some(42));
    }

    #[test]
    fn set_group_members_populates_state() {
        let mut c = Combatant::new(2, 0, test_config());
        let members = vec![
            GroupMemberState {
                spawn_id: 1,
                hp_pct: 80.0,
                mana_pct: 100.0,
                class_id: 1,
                is_dead: false,
                name: String::new(),
                has_detrimental: false,
            },
            GroupMemberState {
                spawn_id: 2,
                hp_pct: 60.0,
                mana_pct: 50.0,
                class_id: 6,
                is_dead: false,
                name: String::new(),
                has_detrimental: false,
            },
        ];
        c.set_group_members(members);
        assert_eq!(c.group_members.len(), 2);
        assert_eq!(c.group_members[0].spawn_id, 1);
    }

    #[test]
    fn set_shared_client_states_derives_group_members() {
        let mut c = Combatant::new(2, 0, test_config());
        c.set_shared_client_states(vec![SharedClientState {
            client_id: 10,
            spawn_id: 42,
            character_name: "Frostreaver".into(),
            class_id: 2,
            level: 60,
            zone_short_name: "kael".into(),
            zone_long_name: "Kael Drakkel".into(),
            hp_pct: 65.0,
            mana_pct: 80.0,
            endurance_pct: 50.0,
            is_dead: false,
            status: "active".into(),
            target: None,
            buffs: Vec::new(),
            pet: None,
        }]);

        assert_eq!(c.shared_client_states.len(), 1);
        assert_eq!(c.group_members.len(), 1);
        assert_eq!(c.group_members[0].spawn_id, 42);
        assert_eq!(c.group_members[0].name, "Frostreaver");
        assert!(!c.group_members[0].is_dead);
    }

    #[test]
    fn distance_3d_with_z_component() {
        let mut a = SpawnData::default();
        a.x = 0.0;
        a.y = 0.0;
        a.z = 0.0;
        let mut b = SpawnData::default();
        b.x = 0.0;
        b.y = 0.0;
        b.z = 10.0;
        assert!(
            (Waypoint::new(a.x, a.y, a.z).distance_3d(&Waypoint::new(b.x, b.y, b.z)) - 10.0).abs()
                < 0.01
        );
    }

    #[test]
    fn distance_3d_same_position() {
        let a = SpawnData::default();
        assert!(
            (Waypoint::new(a.x, a.y, a.z).distance_3d(&Waypoint::new(a.x, a.y, a.z))).abs() < 0.01
        );
    }

    #[test]
    fn pet_classes_list_is_correct() {
        // SK=5, Shaman=10, Necro=11, Mage=13, Beastlord=15
        assert!(PET_CLASSES.contains(&5));
        assert!(PET_CLASSES.contains(&10));
        assert!(PET_CLASSES.contains(&11));
        assert!(PET_CLASSES.contains(&13));
        assert!(PET_CLASSES.contains(&15));
        // Warrior, Cleric, Wizard should NOT be pet classes
        assert!(!PET_CLASSES.contains(&1));
        assert!(!PET_CLASSES.contains(&2));
        assert!(!PET_CLASSES.contains(&12));
    }

    #[test]
    fn status_engaging_carries_target_id() {
        let mut c = Combatant::new(1, 0, test_config());
        c.state = CombatState::Engaging { target_id: 42 };
        if let CombatStatus::Engaging { target_id } = c.status() {
            assert_eq!(target_id, 42);
        } else {
            panic!("expected Engaging status");
        }
    }

    #[test]
    fn status_casting_carries_spell_slot() {
        let mut c = Combatant::new(1, 0, test_config());
        c.state = CombatState::Casting {
            spell_slot: 3,
            spell_id: 0,
            target_id: 42,
            ticks_remaining: 10,
            retry_count: 0,
            backoff_ticks: 0,
        };
        if let CombatStatus::Casting { spell_slot, .. } = c.status() {
            assert_eq!(spell_slot, 3);
        } else {
            panic!("expected Casting status");
        }
    }

    #[test]
    fn chat_feedback_resolves_fizzle_cast_outcome() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = test_player();
        let target = test_target();

        c.state = CombatState::Casting {
            spell_slot: 2,
            spell_id: 0,
            target_id: target.spawn_id,
            ticks_remaining: 10,
            retry_count: 0,
            backoff_ticks: 0,
        };
        assert_eq!(
            c.observe_chat_message("Your spell fizzles!"),
            Some(CastResult::Fizzled)
        );

        c.tick(&player, Some(&target), &[]);

        assert_eq!(c.last_cast_result, Some(CastResult::Fizzled));
        assert!(matches!(c.status(), CombatStatus::OnGcd));
    }

    #[test]
    fn chat_feedback_out_of_mana_moves_to_recovering() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = test_player();
        let target = test_target();

        c.state = CombatState::Casting {
            spell_slot: 2,
            spell_id: 0,
            target_id: target.spawn_id,
            ticks_remaining: 10,
            retry_count: 0,
            backoff_ticks: 0,
        };
        c.observe_chat_message("You don't have enough mana to cast this spell.");

        c.tick(&player, Some(&target), &[]);

        assert_eq!(c.last_cast_result, Some(CastResult::OutOfMana));
        assert!(matches!(c.status(), CombatStatus::Recovering));
    }

    #[test]
    fn chat_feedback_not_ready_returns_to_engaging() {
        let mut c = Combatant::new(1, 0, test_config());
        let player = test_player();
        let target = test_target();

        c.state = CombatState::Casting {
            spell_slot: 4,
            spell_id: 0,
            target_id: target.spawn_id,
            ticks_remaining: 10,
            retry_count: 0,
            backoff_ticks: 0,
        };
        c.observe_chat_message("Spell is not ready yet, please wait.");

        c.tick(&player, Some(&target), &[]);

        assert_eq!(c.last_cast_result, Some(CastResult::NotReady));
        assert!(matches!(
            c.status(),
            CombatStatus::Engaging { target_id: 100 }
        ));
    }

    #[test]
    fn status_on_gcd() {
        let mut c = Combatant::new(1, 0, test_config());
        c.state = CombatState::OnGcd;
        assert!(matches!(c.status(), CombatStatus::OnGcd));
    }

    #[test]
    fn status_recovering() {
        let mut c = Combatant::new(1, 0, test_config());
        c.state = CombatState::Recovering;
        assert!(matches!(c.status(), CombatStatus::Recovering));
    }

    #[test]
    fn disc_cooldown_expires_and_disc_refires() {
        let cfg = config_with_discs(vec![make_disc("Quick Disc", 3001, 1, 3)]);
        let mut c = Combatant::new(1, 0, cfg);
        let player = player_with_hp_end(1000, 1000, 500, 500);
        let target = test_target();

        // Fire the disc
        c.state = CombatState::Engaging { target_id: 100 };
        c.tick(&player, Some(&target), &[]);
        assert_eq!(
            c.ability_cooldowns.availability(3001, c.tick_count),
            AbilityAvailability::CoolingDown(3)
        );

        // Tick 3 more times (cooldown=3). Each tick() decrements at the start.
        for _ in 0..3 {
            c.state = CombatState::Engaging { target_id: 100 };
            c.tick(&player, Some(&target), &[]);
        }

        // After 3 ticks the cooldown expired and the disc re-fired,
        // so it should be back on cooldown with the full duration.
        assert!(
            matches!(
                c.ability_cooldowns.availability(3001, c.tick_count),
                AbilityAvailability::CoolingDown(3)
            ),
            "Disc should re-fire after cooldown expires"
        );
    }

    fn config_with_retry(max_tries: u8, base_backoff_ticks: u32) -> CombatConfig {
        CombatConfig {
            cast_retry_policy: CastRetryPolicy {
                max_tries: Some(max_tries),
                base_backoff_ticks,
            },
            ..CombatConfig::default()
        }
    }

    #[test]
    fn recast_loop_retries_on_fizzle_within_max_tries() {
        // max_tries=2 means 1 initial attempt + 1 retry allowed.
        let mut c = Combatant::new(1, 0, config_with_retry(2, 0));
        let player = test_player();
        let target = test_target();

        c.state = CombatState::Casting {
            spell_slot: 2,
            spell_id: 0,
            target_id: target.spawn_id,
            ticks_remaining: 10,
            retry_count: 0,
            backoff_ticks: 0,
        };
        c.observe_chat_message("Your spell fizzles!");
        c.tick(&player, Some(&target), &[]);

        // Should still be Casting (retrying), not OnGcd.
        assert!(
            matches!(c.status(), CombatStatus::Casting { .. }),
            "Expected Casting (retry), got {:?}",
            c.status()
        );
        // retry_count should be incremented to 1.
        if let CombatState::Casting { retry_count, .. } = c.state {
            assert_eq!(retry_count, 1);
        } else {
            panic!("expected CombatState::Casting");
        }
    }

    #[test]
    fn recast_loop_gives_up_after_max_tries_exhausted() {
        // max_tries=1 means no retries: first failure ends the cast.
        let mut c = Combatant::new(1, 0, config_with_retry(1, 0));
        let player = test_player();
        let target = test_target();

        c.state = CombatState::Casting {
            spell_slot: 2,
            spell_id: 0,
            target_id: target.spawn_id,
            ticks_remaining: 10,
            retry_count: 0,
            backoff_ticks: 0,
        };
        c.observe_chat_message("Your spell fizzles!");
        c.tick(&player, Some(&target), &[]);

        // Should move to OnGcd after exhausting max_tries.
        assert!(
            matches!(c.status(), CombatStatus::OnGcd),
            "Expected OnGcd after exhausting retries, got {:?}",
            c.status()
        );
    }

    #[test]
    fn recast_loop_uses_base_backoff_before_exponential_growth() {
        // max_tries=3 with base_backoff_ticks=2: first retry backs off 2 ticks.
        let mut c = Combatant::new(1, 0, config_with_retry(3, 2));
        let player = test_player();
        let target = test_target();

        c.state = CombatState::Casting {
            spell_slot: 1,
            spell_id: 0,
            target_id: target.spawn_id,
            ticks_remaining: 10,
            retry_count: 0,
            backoff_ticks: 0,
        };
        c.observe_chat_message("Your spell fizzles!");
        c.tick(&player, Some(&target), &[]);

        // After the first fizzle: should be Casting with backoff_ticks = 2*1 = 2.
        if let CombatState::Casting {
            retry_count,
            backoff_ticks,
            ..
        } = c.state
        {
            assert_eq!(retry_count, 1, "retry_count should be 1");
            assert_eq!(
                backoff_ticks, 2,
                "backoff_ticks should be 2 (base * attempt)"
            );
        } else {
            panic!("expected CombatState::Casting");
        }

        // Tick once — backoff decrements but stays in Casting.
        c.tick(&player, Some(&target), &[]);
        if let CombatState::Casting { backoff_ticks, .. } = c.state {
            assert_eq!(backoff_ticks, 1, "backoff_ticks should decrement to 1");
        }

        // Tick again — backoff reaches 0, cast resumes.
        c.tick(&player, Some(&target), &[]);
        if let CombatState::Casting { backoff_ticks, .. } = c.state {
            assert_eq!(
                backoff_ticks, 0,
                "backoff_ticks should be 0 after burn-down"
            );
        }
    }

    #[test]
    fn recast_unlimited_retries_when_max_tries_is_none() {
        let mut c = Combatant::new(
            1,
            0,
            CombatConfig {
                cast_retry_policy: CastRetryPolicy {
                    max_tries: None,
                    base_backoff_ticks: 0,
                },
                ..CombatConfig::default()
            },
        );
        let player = test_player();
        let target = test_target();

        // Simulate 10 consecutive fizzles — each should produce a retry.
        for i in 0..10u8 {
            c.state = CombatState::Casting {
                spell_slot: 1,
                spell_id: 0,
                target_id: target.spawn_id,
                ticks_remaining: 10,
                retry_count: i,
                backoff_ticks: 0,
            };
            c.observe_chat_message("Your spell fizzles!");
            c.tick(&player, Some(&target), &[]);
            assert!(
                matches!(c.status(), CombatStatus::Casting { .. }),
                "Attempt {i}: expected Casting (unlimited retries)"
            );
        }
    }

    #[test]
    fn recast_non_retryable_result_skips_retry() {
        // Resisted is non-retryable — should not trigger a retry regardless of policy.
        let mut c = Combatant::new(1, 0, config_with_retry(5, 0));
        let player = test_player();
        let target = test_target();

        c.state = CombatState::Casting {
            spell_slot: 2,
            spell_id: 0,
            target_id: target.spawn_id,
            ticks_remaining: 10,
            retry_count: 0,
            backoff_ticks: 0,
        };
        c.observe_chat_message("Your target resisted your spell!");
        c.tick(&player, Some(&target), &[]);

        assert!(
            matches!(c.status(), CombatStatus::OnGcd),
            "Resisted cast should not retry, expected OnGcd"
        );
    }

    /// Verify that a retry attempt (retry_count > 0, ticks_remaining == 20,
    /// no pending result) consumes the GCD — confirming the re-issue path runs.
    /// Also verifies that the spell_id stored in Casting state is preserved on
    /// retry.
    #[test]
    fn recast_retry_reissue_consumes_gcd_and_preserves_spell_id() {
        // max_tries=3 so retry is allowed, no backoff needed.
        let mut c = Combatant::new(1, 0, config_with_retry(3, 0));
        let player = test_player();
        let target = test_target();

        // Place the FSM into a retry-pending state: retry_count=1, backoff already
        // exhausted (backoff_ticks=0), ticks_remaining=20 (fresh retry window),
        // and a non-zero spell_id to confirm it is forwarded to cast_spell.
        const SPELL_ID: i32 = 9999;
        c.state = CombatState::Casting {
            spell_slot: 2,
            spell_id: SPELL_ID,
            target_id: target.spawn_id,
            ticks_remaining: 20,
            retry_count: 1,
            backoff_ticks: 0,
        };

        // GCD must be idle before the tick so we can observe consume().
        assert!(c.gcd.is_ready(), "GCD should be ready before retry tick");

        c.tick(&player, Some(&target), &[]);

        // The retry re-issue path must consume the GCD.
        assert!(
            !c.gcd.is_ready(),
            "GCD should be consumed after retry-cast reissue"
        );

        // spell_id must be preserved in the Casting state after the tick.
        if let CombatState::Casting { spell_id, .. } = c.state {
            assert_eq!(
                spell_id, SPELL_ID,
                "spell_id must be preserved across retry tick"
            );
        } else {
            panic!("Expected CombatState::Casting after retry tick");
        }
    }
}
