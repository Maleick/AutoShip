use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Extended Target (XTarget) types — read from EQ's ExtendedTargetList
// ---------------------------------------------------------------------------

/// Role assigned to an extended target slot.
/// Source: eqlib EQData.h `XTargetTypes` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum XTargetType {
    Empty = 0,
    AutoHater = 1,
    SpecificPc = 2,
    SpecificNpc = 3,
    TargetsTarget = 4,
    GroupTank = 5,
    GroupTanksTarget = 6,
    GroupAssist = 7,
    GroupAssistTarget = 8,
    GroupPuller = 9,
    GroupPullerTarget = 10,
    GroupMark1 = 11,
    GroupMark2 = 12,
    GroupMark3 = 13,
    RaidAssist1 = 14,
    RaidAssist2 = 15,
    RaidAssist3 = 16,
    RaidAssist1Target = 17,
    RaidAssist2Target = 18,
    RaidAssist3Target = 19,
    RaidMark1 = 20,
    RaidMark2 = 21,
    RaidMark3 = 22,
    MyPet = 23,
    MyPetTarget = 24,
    MyMercenary = 25,
    MyMercenaryTarget = 26,
}

impl XTargetType {
    pub fn from_raw(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Empty),
            1 => Some(Self::AutoHater),
            2 => Some(Self::SpecificPc),
            3 => Some(Self::SpecificNpc),
            4 => Some(Self::TargetsTarget),
            5 => Some(Self::GroupTank),
            6 => Some(Self::GroupTanksTarget),
            7 => Some(Self::GroupAssist),
            8 => Some(Self::GroupAssistTarget),
            9 => Some(Self::GroupPuller),
            10 => Some(Self::GroupPullerTarget),
            11 => Some(Self::GroupMark1),
            12 => Some(Self::GroupMark2),
            13 => Some(Self::GroupMark3),
            14 => Some(Self::RaidAssist1),
            15 => Some(Self::RaidAssist2),
            16 => Some(Self::RaidAssist3),
            17 => Some(Self::RaidAssist1Target),
            18 => Some(Self::RaidAssist2Target),
            19 => Some(Self::RaidAssist3Target),
            20 => Some(Self::RaidMark1),
            21 => Some(Self::RaidMark2),
            22 => Some(Self::RaidMark3),
            23 => Some(Self::MyPet),
            24 => Some(Self::MyPetTarget),
            25 => Some(Self::MyMercenary),
            26 => Some(Self::MyMercenaryTarget),
            _ => None,
        }
    }

    pub fn is_auto_hater(self) -> bool {
        self == Self::AutoHater
    }
}

/// Slot status in the extended target window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum XTargetSlotStatus {
    Empty = 0,
    CurrentZone = 1,
    DifferentZone = 2,
    Unknown = 3,
}

impl XTargetSlotStatus {
    pub fn from_raw(v: u32) -> Self {
        match v {
            0 => Self::Empty,
            1 => Self::CurrentZone,
            2 => Self::DifferentZone,
            _ => Self::Unknown,
        }
    }
}

/// A single slot from the EQ Extended Target window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtendedTargetSlot {
    pub slot_type: XTargetType,
    pub status: XTargetSlotStatus,
    pub spawn_id: u32,
    pub name: String,
}

impl ExtendedTargetSlot {
    pub fn is_active(&self) -> bool {
        self.status == XTargetSlotStatus::CurrentZone
            && self.spawn_id != 0
            && self.slot_type != XTargetType::Empty
    }
}

/// Snapshot of the full extended target list, read from EQ memory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtendedTargetList {
    pub slots: Vec<ExtendedTargetSlot>,
    pub auto_add_haters: bool,
}

impl ExtendedTargetList {
    pub fn hater_spawn_ids(&self) -> Vec<u32> {
        self.slots
            .iter()
            .filter(|s| s.is_active() && s.slot_type.is_auto_hater())
            .map(|s| s.spawn_id)
            .collect()
    }

    pub fn is_hater(&self, spawn_id: u32) -> bool {
        self.slots
            .iter()
            .any(|s| s.is_active() && s.slot_type.is_auto_hater() && s.spawn_id == spawn_id)
    }

    pub fn hater_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| s.is_active() && s.slot_type.is_auto_hater())
            .count()
    }

    /// Return active extended-target spawn IDs that represent the group's
    /// current primary kill target(s), not CC adds.
    pub fn primary_target_spawn_ids(&self) -> Vec<u32> {
        self.slots
            .iter()
            .filter(|s| {
                s.is_active()
                    && matches!(
                        s.slot_type,
                        XTargetType::GroupTanksTarget
                            | XTargetType::GroupAssistTarget
                            | XTargetType::GroupPullerTarget
                            | XTargetType::RaidAssist1Target
                            | XTargetType::RaidAssist2Target
                            | XTargetType::RaidAssist3Target
                    )
            })
            .map(|s| s.spawn_id)
            .collect()
    }

    /// Return active extended-target spawn IDs that look like CC adds.
    ///
    /// These are auto-haters that are not also referenced by the group's
    /// primary-target slots, which keeps CC helpers from selecting the mob the
    /// group is already burning down.
    pub fn cc_add_spawn_ids(&self) -> Vec<u32> {
        let primary_targets = self.primary_target_spawn_ids();
        self.slots
            .iter()
            .filter(|s| {
                s.is_active()
                    && s.slot_type.is_auto_hater()
                    && !primary_targets.contains(&s.spawn_id)
            })
            .map(|s| s.spawn_id)
            .collect()
    }

    pub fn first_cc_add_spawn_id(&self) -> Option<u32> {
        self.cc_add_spawn_ids().into_iter().next()
    }

    pub fn get_by_type(&self, slot_type: XTargetType) -> Option<&ExtendedTargetSlot> {
        self.slots
            .iter()
            .find(|s| s.is_active() && s.slot_type == slot_type)
    }

    pub fn pet(&self) -> Option<&ExtendedTargetSlot> {
        self.get_by_type(XTargetType::MyPet)
    }

    pub fn pet_spawn_id(&self) -> Option<u32> {
        self.pet().map(|slot| slot.spawn_id)
    }

    pub fn pet_target(&self) -> Option<&ExtendedTargetSlot> {
        self.get_by_type(XTargetType::MyPetTarget)
    }

    pub fn pet_target_id(&self) -> Option<u32> {
        self.pet_target().map(|slot| slot.spawn_id)
    }

    pub fn active_slots(&self) -> Vec<&ExtendedTargetSlot> {
        self.slots.iter().filter(|s| s.is_active()).collect()
    }
}

// ---------------------------------------------------------------------------
// Hate-target categories — classify off-target adds for CC / kiting decisions
// ---------------------------------------------------------------------------

/// Classifies an off-target mob for CC assignment and kiting priority decisions.
///
/// The category drives two independent priority axes:
/// * **CC priority** — which add should be controlled first.
/// * **Kite priority** — which add should be kited away from the group first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum HateTargetCategory {
    /// Mob is on the XTarget auto-hater list — actively attacking a group member.
    /// Highest default CC priority; must be controlled before it deals damage.
    #[default]
    ActiveHater,
    /// Mob is a spellcaster (nuker, healer, or debuffer).
    /// Snare/root preferred so it cannot kite away or continue casting.
    CasterAdd,
    /// Mob is a melee attacker that has joined the fight.
    /// Mez/stun preferred for full lockdown.
    MeleeAdd,
    /// Mob is moving toward the group but has not yet engaged.
    /// Good kite candidate before it fully joins the fight.
    Approaching,
    /// Mob is in the area but has not aggroed.
    /// Lowest priority — handle only when all engaged adds are controlled.
    Roamer,
}

impl HateTargetCategory {
    /// CC assignment priority (lower = higher priority).
    ///
    /// Order: `ActiveHater` → `CasterAdd` → `Approaching` → `MeleeAdd` → `Roamer`.
    #[must_use]
    pub fn cc_priority(self) -> u8 {
        match self {
            Self::ActiveHater => 1,
            Self::CasterAdd => 2,
            Self::Approaching => 3,
            Self::MeleeAdd => 4,
            Self::Roamer => 5,
        }
    }

    /// Kiting priority (lower = more urgent to kite away).
    ///
    /// Casters are kited first (prevent ranged damage), followed by approaching
    /// mobs (intercept before melee contact), then active melee, then roamers.
    #[must_use]
    pub fn kite_priority(self) -> u8 {
        match self {
            Self::CasterAdd => 1,
            Self::Approaching => 2,
            Self::ActiveHater => 3,
            Self::MeleeAdd => 4,
            Self::Roamer => 5,
        }
    }

    /// Returns `true` if this category should be handled with CC rather than kiting.
    ///
    /// [`CasterAdd`] appears in both `prefers_cc` and [`prefers_kite`] because casters
    /// are dangerous at range: the group wants them either silenced via CC (ideal) or
    /// kited far enough away that their spells land out of range (fallback when no CC
    /// is available). Callers should prefer CC when a CC member is ready; fall back to
    /// kiting only when no CC ability is off cooldown.
    ///
    /// [`CasterAdd`]: Self::CasterAdd
    /// [`prefers_kite`]: Self::prefers_kite
    #[must_use]
    pub fn prefers_cc(self) -> bool {
        matches!(self, Self::ActiveHater | Self::MeleeAdd | Self::CasterAdd)
    }

    /// Returns `true` if this category is a better kite candidate than CC target.
    ///
    /// [`CasterAdd`] is included here as a fallback strategy: when no CC ability is
    /// available, kiting a caster is preferable to leaving it free-casting in melee
    /// range. See [`prefers_cc`] for the primary strategy.
    ///
    /// [`CasterAdd`]: Self::CasterAdd
    /// [`prefers_cc`]: Self::prefers_cc
    #[must_use]
    pub fn prefers_kite(self) -> bool {
        matches!(self, Self::Approaching | Self::CasterAdd)
    }
}

// ---------------------------------------------------------------------------
// Buff tracking
// ---------------------------------------------------------------------------

/// Which buff window category a buff belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuffCategory {
    /// Long-duration buff (slots 0–61).
    LongBuff,
    /// Short-duration / song / combat discipline (slots 62–92).
    ShortBuff,
}

/// A single active buff read from the `EQ_Affect` array.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuffInfo {
    /// EQ spell ID for this buff.
    pub spell_id: i32,
    /// Remaining duration in ticks (6 s/tick). 0 = permanent / no timer.
    pub duration_ticks: i32,
    /// Duration when the buff was first applied (ticks).
    pub initial_duration: i32,
    /// Remaining hit count for limited-hit buffs. 0 = unlimited.
    pub hit_count: i32,
    /// Buff category (long vs short/song).
    pub category: BuffCategory,
    /// Level of the caster who applied the buff.
    pub caster_level: u8,
    /// Slot index in the buff array (0-based).
    pub slot_index: usize,
}

impl BuffInfo {
    /// Remaining duration in seconds (ticks × 6). Returns 0 for permanent buffs.
    #[must_use]
    pub fn remaining_seconds(&self) -> f32 {
        if self.duration_ticks <= 0 {
            return 0.0;
        }
        self.duration_ticks as f32 * crate::offsets::buff_slots::SECONDS_PER_TICK
    }

    /// Total initial duration in seconds.
    #[must_use]
    pub fn total_seconds(&self) -> f32 {
        if self.initial_duration <= 0 {
            return 0.0;
        }
        self.initial_duration as f32 * crate::offsets::buff_slots::SECONDS_PER_TICK
    }

    /// Returns true if this buff will expire within `threshold_secs` seconds.
    #[must_use]
    pub fn expires_within(&self, threshold_secs: f32) -> bool {
        self.duration_ticks > 0 && self.remaining_seconds() <= threshold_secs
    }
}

/// Current state of a character in the combat FSM.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CombatStatus {
    /// Not in combat, no pending actions.
    Idle,
    /// Actively attacking a target.
    Engaging {
        /// Spawn ID of the mob being engaged.
        target_id: u32,
    },
    /// Casting a spell on a target.
    Casting {
        /// Memorized spell slot (0-indexed).
        spell_slot: u8,
        /// Spawn ID of the cast target.
        target_id: u32,
    },
    /// Waiting for the global cooldown to expire.
    OnGcd,
    /// Pulling a mob back to camp.
    Pulling {
        /// Spawn ID of the mob being pulled.
        target_id: u32,
    },
    /// Post-combat recovery (regen, med, rebuff).
    Recovering,
    /// Running away from danger.
    Fleeing,
    /// Character is dead, awaiting resurrect or respawn.
    Dead,
}

/// Role a character fills in the group combat formation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CombatRole {
    /// Primary tank — holds aggro on the main target.
    MainTank,
    /// Secondary tank — picks up adds or swap targets.
    OffTank,
    /// Healer — keeps the group alive.
    Healer,
    /// Puller — brings mobs to camp.
    Puller,
    /// Melee DPS — fights in melee range.
    DpsMelee,
    /// Ranged DPS — nukes or uses ranged attacks.
    DpsRanged,
    /// Crowd control — mezzes, roots, or charms adds.
    CrowdControl,
    /// Support — buffs, debuffs, utility (bard, shaman, etc.).
    Support,
}

/// How the puller initiates a pull.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PullMethod {
    /// Pull with a spell from the given memorized slot.
    SpellPull {
        /// Memorized spell slot to cast.
        spell_slot: u8,
    },
    /// Pull with a bow/ranged attack.
    BowPull,
    /// Walk close enough for the mob to aggro.
    ProximityPull,
}

/// How multiple DPS characters divide their targets.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AssistMode {
    /// Everyone assists the main assist target.
    AssistTrain,
    /// DPS split across multiple targets.
    SplitDps,
    /// Each character picks its own target.
    Solo,
}

/// A memorized spell available for the combat rotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpellEntry {
    /// Memorized spell slot (0-indexed gem number).
    pub slot: u8,
    /// EQ spell ID — used by `CastSpell` FFI. 0 = use whatever is memorized in slot.
    #[serde(default)]
    pub spell_id: i32,
    /// Human-readable spell name for logging/config.
    pub name: String,
    /// Minimum mana % required to cast this spell.
    pub min_mana_pct: f32,
    /// Priority in the rotation (lower = higher priority).
    pub priority: u8,
    /// Whether this spell is area-of-effect.
    pub is_aoe: bool,
}

/// Boolean expression tree for evaluating combat conditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionExpr {
    /// All sub-conditions must be true.
    And(Vec<ConditionExpr>),
    /// At least one sub-condition must be true.
    Or(Vec<ConditionExpr>),
    /// Character HP is below the given percentage.
    HpBelow(f32),
    /// Character mana is below the given percentage.
    ManaBelow(f32),
    /// Current target HP is above the given percentage.
    TargetHpAbove(f32),
    /// Current target HP is below the given percentage.
    TargetHpBelow(f32),
    /// Character mana is above the given percentage.
    ManaAbove(f32),
    /// The character has aggro from a mob.
    AggroOnMe,
    /// Character is in combat.
    InCombat,
    /// Character is out of combat (downtime).
    OutOfCombat,
    /// A specific buff/spell ID is active on the player.
    BuffActive(i32),
    /// A specific buff/spell ID is NOT active on the player.
    BuffMissing(i32),
    /// A buff will expire within the given number of seconds.
    /// (spell_id, threshold_seconds)
    BuffExpiringSoon(i32, f32),
    /// Target distance is below the given range (melee check).
    TargetDistanceBelow(f32),
    /// Number of nearby enemies is at or above the given count (AE threshold).
    EnemyCountAbove(u32),
    /// Target is currently mezzed (prevent mez break).
    TargetMezzed,
    /// Target is NOT mezzed.
    TargetNotMezzed,
    /// Negation of a sub-condition.
    Not(Box<ConditionExpr>),
    /// Always true — unconditional trigger.
    Always,
    /// Number of mobs on the extended target hate list is at or above the given count.
    XTargetHaterCountAbove(u32),
    /// We have aggro from at least one mob (checked via the XTarget auto-hater list).
    HasXTargetAggro,
}

/// An emergency reaction rule that fires when conditions are met.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolyShitCondition {
    /// Evaluation priority (lower = checked first).
    pub priority: u8,
    /// Boolean condition tree that triggers this rule.
    pub condition: ConditionExpr,
    /// Action to take when the condition is true.
    pub action: HolyShitAction,
}

/// Emergency action to execute when a HolyShit condition fires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HolyShitAction {
    /// Cast a spell from the given memorized slot.
    CastSpell(u8),
    /// Use a combat ability by ID.
    UseAbility(u32),
    /// Use an inventory item by ID.
    UseItem(u32),
    /// Run away from combat.
    Flee,
}

/// A warrior/monk/etc discipline (activated combat ability with a reuse timer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisciplineEntry {
    /// Human-readable discipline name.
    pub name: String,
    /// EQ spell ID for the discipline.
    pub spell_id: i32,
    /// Priority relative to other disciplines (lower = higher priority).
    pub priority: u8,
    /// Cooldown in game ticks (~20 ticks/sec). Disciplines have long reuse timers.
    pub cooldown_ticks: u32,
    /// Minimum HP % to use this disc (e.g., Defensive only when < 50% HP)
    pub min_hp_pct: f32,
    /// Maximum HP % (e.g., don't waste Defensive at full HP)
    pub max_hp_pct: f32,
    /// Minimum endurance % required
    pub min_endurance_pct: f32,
}

/// Preference for which mob type to prioritize during target scanning.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum NamedPreference {
    /// Prioritize named mobs over trash.
    NamedFirst,
    /// Prioritize trash mobs (e.g., to clear a camp before engaging named).
    TrashFirst,
    /// No preference — pick by distance/HP.
    #[default]
    NoPreference,
}

/// Preference for how to sort candidate targets by HP.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum HpPreference {
    /// Target the lowest HP mob (finish off wounded targets).
    #[default]
    LowestHp,
    /// Target the highest HP mob (fresh targets).
    HighestHp,
    /// No HP preference — use distance.
    Nearest,
}

/// Configuration for the MA (Main Assist) target scan system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetScanConfig {
    /// Maximum distance to scan for targets (2D).
    pub scan_radius: f32,
    /// Maximum Z-axis distance to consider (vertical filter).
    pub scan_z_radius: f32,
    /// Named mob priority preference.
    pub named_preference: NamedPreference,
    /// HP-based target sorting preference.
    pub hp_preference: HpPreference,
    /// HP % threshold — only assist when MA target is below this HP.
    pub assist_hp_threshold: f32,
    /// Skip mezzed mobs (stand_state animation check).
    pub skip_mezzed: bool,
    /// Skip mobs whose target is not in our group (safe targeting).
    pub safe_targeting: bool,
}

impl Default for TargetScanConfig {
    fn default() -> Self {
        Self {
            scan_radius: 200.0,
            scan_z_radius: 50.0,
            named_preference: NamedPreference::default(),
            hp_preference: HpPreference::default(),
            assist_hp_threshold: 98.0,
            skip_mezzed: true,
            safe_targeting: true,
        }
    }
}

/// Per-character combat configuration (role, spells, rules).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatConfig {
    /// This character's combat role.
    pub role: CombatRole,
    /// How this character pulls (if puller role).
    pub pull_method: Option<PullMethod>,
    /// How DPS targets are divided.
    pub assist_mode: AssistMode,
    /// Spell rotation entries.
    pub spells: Vec<SpellEntry>,
    /// Discipline rotation entries.
    pub disciplines: Vec<DisciplineEntry>,
    /// Emergency reaction rules evaluated each tick.
    pub holyshit_rules: Vec<HolyShitCondition>,
    /// Mana % floor — stop casting below this.
    pub mana_floor: f32,
    /// Minimum mob count to trigger AoE spells.
    pub aoe_threshold: u8,
    /// MA target scan settings for smart target selection.
    #[serde(default)]
    pub target_scan: TargetScanConfig,
}

impl Default for CombatConfig {
    fn default() -> Self {
        Self {
            role: CombatRole::DpsMelee,
            pull_method: None,
            assist_mode: AssistMode::AssistTrain,
            spells: Vec::new(),
            disciplines: Vec::new(),
            holyshit_rules: Vec::new(),
            mana_floor: 20.0,
            aoe_threshold: 3,
            target_scan: TargetScanConfig::default(),
        }
    }
}

/// Whether a character is ready to cast a spell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CastReadiness {
    /// Ready to cast — no cooldowns or active casts.
    Ready,
    /// Currently casting a spell.
    Casting,
    /// Waiting for the global cooldown to expire.
    GlobalCooldown,
    /// Post-cast recovery window.
    Recovering,
    /// Spell is not memorized in any gem slot.
    NotMemorized,
}

impl std::fmt::Display for CastReadiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ready => write!(f, "Ready"),
            Self::Casting => write!(f, "Casting"),
            Self::GlobalCooldown => write!(f, "GCD"),
            Self::Recovering => write!(f, "Recovering"),
            Self::NotMemorized => write!(f, "Not Memorized"),
        }
    }
}

/// Outcome of a cast attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CastResult {
    /// Spell landed successfully.
    Success,
    /// Spell fizzled (failed skill check).
    Fizzled,
    /// Spell or gate collapsed before completion.
    Collapsed,
    /// Target resisted the spell.
    Resisted,
    /// Target is immune to this spell.
    Immune,
    /// Cast was interrupted (took damage, moved, etc.).
    Interrupted,
    /// Cast was manually aborted by the player/automation.
    Aborted,
    /// Spell or gem is not ready yet.
    NotReady,
    /// Not enough mana to cast.
    OutOfMana,
    /// Target was out of range.
    OutOfRange,
    /// No line of sight to the target.
    CannotSee,
    /// No valid target is selected.
    NoTarget,
    /// Must stand before casting.
    Standing,
    /// Character is stunned and cannot cast.
    Stunned,
    /// Required spell components are missing.
    Components,
    /// Cast was cancelled before it completed.
    Cancelled,
    /// Casting was disrupted by distraction.
    Distracted,
    /// Character is invisible and cannot cast.
    Invisible,
    /// Spell can only be cast outdoors.
    Outdoors,
    /// A cast is already pending.
    Pending,
    /// Character is still in post-cast recovery.
    Recovering,
    /// Spell completed but did not take hold on the target.
    TakeHold,
    /// Outcome could not be classified.
    Unknown,
}

impl std::fmt::Display for CastResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "Success"),
            Self::Fizzled => write!(f, "Fizzled"),
            Self::Collapsed => write!(f, "Collapsed"),
            Self::Resisted => write!(f, "Resisted"),
            Self::Immune => write!(f, "Immune"),
            Self::Interrupted => write!(f, "Interrupted"),
            Self::Aborted => write!(f, "Aborted"),
            Self::NotReady => write!(f, "Not Ready"),
            Self::OutOfMana => write!(f, "Out of Mana"),
            Self::OutOfRange => write!(f, "Out of Range"),
            Self::CannotSee => write!(f, "Cannot See"),
            Self::NoTarget => write!(f, "No Target"),
            Self::Standing => write!(f, "Standing"),
            Self::Stunned => write!(f, "Stunned"),
            Self::Components => write!(f, "Components"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Distracted => write!(f, "Distracted"),
            Self::Invisible => write!(f, "Invisible"),
            Self::Outdoors => write!(f, "Outdoors"),
            Self::Pending => write!(f, "Pending"),
            Self::Recovering => write!(f, "Recovering"),
            Self::TakeHold => write!(f, "Take Hold"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

impl CastResult {
    /// Parse an EQ feedback/chat line into an MQ2Cast-style cast result.
    pub fn from_feedback_message(text: &str) -> Option<Self> {
        let normalized = text.trim().to_ascii_lowercase();

        if normalized.is_empty() {
            return None;
        }

        if normalized.contains("spell fizzles") {
            return Some(Self::Fizzled);
        }
        if normalized.contains("collapses") {
            return Some(Self::Collapsed);
        }
        if normalized.contains("resisted") {
            return Some(Self::Resisted);
        }
        if normalized.contains("immune") {
            return Some(Self::Immune);
        }
        if normalized.contains("interrupted") || normalized.contains("miss a note") {
            return Some(Self::Interrupted);
        }
        if normalized.contains("aborted") {
            return Some(Self::Aborted);
        }
        if normalized.contains("not ready") || normalized.contains("spell is not ready") {
            return Some(Self::NotReady);
        }
        if normalized.contains("not enough mana") || normalized.contains("don't have enough mana") {
            return Some(Self::OutOfMana);
        }
        if normalized.contains("out of range") {
            return Some(Self::OutOfRange);
        }
        if normalized.contains("cannot see your target")
            || normalized.contains("can't see your target")
        {
            return Some(Self::CannotSee);
        }
        if normalized.contains("must first select a target")
            || normalized.contains("you need a target")
        {
            return Some(Self::NoTarget);
        }
        if normalized.contains("stand up first") {
            return Some(Self::Standing);
        }
        if normalized.contains("stunned") {
            return Some(Self::Stunned);
        }
        if normalized.contains("required components")
            || normalized.contains("missing some components")
        {
            return Some(Self::Components);
        }
        if normalized.contains("spell is canceled")
            || normalized.contains("spell has been cancelled")
        {
            return Some(Self::Cancelled);
        }
        if normalized.contains("distracted from your casting") {
            return Some(Self::Distracted);
        }
        if normalized.contains("cannot cast while invisible") {
            return Some(Self::Invisible);
        }
        if normalized.contains("only cast this spell in the outdoors") {
            return Some(Self::Outdoors);
        }
        if normalized.contains("already have a spell pending") {
            return Some(Self::Pending);
        }
        if normalized.contains("haven't recovered yet")
            || normalized.contains("must wait to perform another action")
        {
            return Some(Self::Recovering);
        }
        if normalized.contains("did not take hold") {
            return Some(Self::TakeHold);
        }
        if normalized.contains("spell cast") || normalized.contains("you begin casting") {
            return Some(Self::Success);
        }

        None
    }

    /// Whether the cast finished in a way that should generally be retried.
    pub fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::Fizzled
                | Self::Collapsed
                | Self::Interrupted
                | Self::Distracted
                | Self::Pending
                | Self::Recovering
                | Self::NotReady
        )
    }

    /// Whether the spell actually landed on the target.
    pub fn landed(self) -> bool {
        matches!(self, Self::Success)
    }
}

/// Telemetry snapshot for a single cast attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CastTelemetry {
    /// Outcome of the cast.
    pub result: CastResult,
    /// Wall-clock duration of the cast in milliseconds.
    pub cast_duration_ms: u64,
    /// EQ spell ID that was cast.
    pub spell_id: i32,
    /// Spawn ID of the cast target (0 = self/none).
    pub target_id: u32,
    /// Unix timestamp (milliseconds) when the cast completed.
    pub timestamp_ms: u64,
}

impl std::fmt::Display for CastTelemetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "spell={} target={} result={} duration={}ms",
            self.spell_id, self.target_id, self.result, self.cast_duration_ms
        )
    }
}

// ---------------------------------------------------------------------------
// Rotation system — data-driven combat action tables (rgmercs-inspired)
// ---------------------------------------------------------------------------

/// The type of action a rotation entry performs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionType {
    /// Cast a spell (references an ability set name or direct spell name).
    Spell(String),
    /// Activate a discipline.
    Disc(String),
    /// Fire an alternate advancement ability.
    AA(String),
    /// Use a combat ability (kick, bash, taunt, etc.).
    Ability(String),
    /// Use an item (clicky, epic, etc.).
    Item(String),
    /// Sing a bard song.
    Song(String),
}

/// How to select targets for a rotation group.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TargetSelector {
    /// Target self (for self-buffs, downtime actions).
    SelfOnly,
    /// Target the current auto-target (main assist target).
    AutoTarget,
    /// Target the mob with lowest aggro on the tank (aggro scan).
    AggroTarget,
    /// Target the group member with the lowest HP (for heals).
    LowestHpGroupMember,
}

/// What combat state is required for a rotation group to run.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CombatStateReq {
    /// Only run during active combat.
    Combat,
    /// Only run during downtime (no enemies on hatelist).
    Downtime,
    /// Run in any state.
    Any,
}

// ── Ability Resolution (AbilitySets) ────────────────────────────────────────

/// A single candidate in an ability set — one rank of a spell line.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AbilityCandidate {
    /// Human-readable spell/disc/AA name (e.g., "Ice Comet").
    pub name: String,
    /// Minimum character level required to use this candidate.
    pub min_level: u8,
    /// EQ spell ID. -1 means "look up at runtime from the spell book."
    pub spell_id: i32,
}

/// An ordered set of candidates for one spell line (e.g., "Nuke", "Heal").
/// Candidates are ordered strongest-first; the resolver picks the first one
/// the character knows and meets the level requirement for.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AbilitySet {
    /// Spell line name (used as the key in rotation entries via ActionType).
    pub name: String,
    /// Candidates, ordered strongest → weakest.
    pub candidates: Vec<AbilityCandidate>,
}

/// The result of resolving an AbilitySet for a specific character.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedAbility {
    /// Which AbilitySet this was resolved from.
    pub set_name: String,
    /// The chosen candidate's name.
    pub ability_name: String,
    /// The chosen candidate's spell ID.
    pub spell_id: i32,
    /// The chosen candidate's minimum level.
    pub min_level: u8,
}

/// A spell/disc/AA that the character actually knows (from spell book scan).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnownAbility {
    /// Spell name as it appears in the spell book.
    pub name: String,
    /// EQ spell ID.
    pub spell_id: i32,
    /// Level at which the character learned it.
    pub level: u8,
}

/// Resolve each AbilitySet to the best candidate the character knows.
///
/// For each set, iterates candidates strongest-first and picks the first one
/// where: (a) the character's level >= candidate's min_level, and (b) the
/// candidate appears in the `known` list (matched case-insensitively by name,
/// or by spell_id if the candidate has a non-negative spell_id).
pub fn resolve_abilities(
    sets: &[AbilitySet],
    known: &[KnownAbility],
    character_level: u8,
) -> HashMap<String, ResolvedAbility> {
    let mut resolved = HashMap::new();
    for set in sets {
        for candidate in &set.candidates {
            if character_level < candidate.min_level {
                continue;
            }
            let is_known = known.iter().any(|k| {
                if candidate.spell_id >= 0 && k.spell_id == candidate.spell_id {
                    return true;
                }
                k.name.eq_ignore_ascii_case(&candidate.name)
            });
            if is_known {
                resolved.insert(
                    set.name.clone(),
                    ResolvedAbility {
                        set_name: set.name.clone(),
                        ability_name: candidate.name.clone(),
                        spell_id: candidate.spell_id,
                        min_level: candidate.min_level,
                    },
                );
                break;
            }
        }
    }
    resolved
}

/// Priority level for buff maintenance — determines rebuff urgency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BuffPriority {
    /// Must never drop — e.g., cleric Aegolism, enchanter haste.
    Critical,
    /// Important but brief gaps are tolerable — e.g., stat buffs.
    High,
    /// Standard maintenance buffs — e.g., Symbol, skin line.
    Normal,
    /// Nice-to-have — e.g., see invis, levitate.
    Low,
}

/// Tracks a single active buff's timing for proactive rebuffing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuffTimer {
    /// EQ spell ID of the buff.
    pub spell_id: i32,
    /// Human-readable spell name for logging/config.
    pub spell_name: String,
    /// Spawn ID of the character who cast the buff.
    pub caster_id: u32,
    /// Spawn ID of the buff recipient.
    pub target_id: u32,
    /// Game tick when the buff was applied.
    pub applied_at: u64,
    /// Buff duration in game ticks.
    pub duration_ticks: u64,
    /// Rebuff priority — higher priority buffs are refreshed first.
    pub priority: BuffPriority,
}

impl BuffTimer {
    /// Returns the game tick when this buff expires.
    pub fn expires_at(&self) -> u64 {
        self.applied_at.saturating_add(self.duration_ticks)
    }

    /// Returns the remaining ticks before expiry, or 0 if already expired.
    pub fn time_remaining(&self, current_tick: u64) -> u64 {
        self.expires_at().saturating_sub(current_tick)
    }

    /// Returns true if the buff has expired at the given tick.
    pub fn is_expired(&self, current_tick: u64) -> bool {
        current_tick >= self.expires_at()
    }

    /// Returns true if the buff will expire within `threshold_ticks` from now.
    pub fn is_expiring_soon(&self, current_tick: u64, threshold_ticks: u64) -> bool {
        self.time_remaining(current_tick) <= threshold_ticks
    }
}

/// Collection of active buff timers with expiry management.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuffTimerSet {
    /// Active buff timers.
    pub timers: Vec<BuffTimer>,
}

impl BuffTimerSet {
    /// Creates an empty buff timer set.
    pub fn new() -> Self {
        Self { timers: Vec::new() }
    }

    /// Adds a buff timer. If a buff with the same spell_id and target_id
    /// already exists, it is replaced (rebuff resets the timer).
    pub fn add_buff(&mut self, timer: BuffTimer) {
        self.timers
            .retain(|t| !(t.spell_id == timer.spell_id && t.target_id == timer.target_id));
        self.timers.push(timer);
    }

    /// Removes all buff timers matching the given spell_id and target_id.
    pub fn remove_buff(&mut self, spell_id: i32, target_id: u32) {
        self.timers
            .retain(|t| !(t.spell_id == spell_id && t.target_id == target_id));
    }

    /// Returns all buffs that have expired at the given tick.
    pub fn get_expired(&self, current_tick: u64) -> Vec<&BuffTimer> {
        self.timers
            .iter()
            .filter(|t| t.is_expired(current_tick))
            .collect()
    }

    /// Returns buffs expiring within `threshold_ticks`, sorted by priority
    /// (Critical first) then by time remaining (soonest first).
    pub fn get_expiring_soon(&self, current_tick: u64, threshold_ticks: u64) -> Vec<&BuffTimer> {
        let mut expiring: Vec<&BuffTimer> = self
            .timers
            .iter()
            .filter(|t| {
                !t.is_expired(current_tick) && t.is_expiring_soon(current_tick, threshold_ticks)
            })
            .collect();
        expiring.sort_by(|a, b| {
            a.priority.cmp(&b.priority).then_with(|| {
                a.time_remaining(current_tick)
                    .cmp(&b.time_remaining(current_tick))
            })
        });
        expiring
    }

    /// Removes all expired buffs and returns how many were purged.
    pub fn purge_expired(&mut self, current_tick: u64) -> usize {
        let before = self.timers.len();
        self.timers.retain(|t| !t.is_expired(current_tick));
        before - self.timers.len()
    }

    /// Returns the number of active (non-expired) buffs at the given tick.
    pub fn active_count(&self, current_tick: u64) -> usize {
        self.timers
            .iter()
            .filter(|t| !t.is_expired(current_tick))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_status_idle_variant() {
        let status = CombatStatus::Idle;
        assert!(matches!(status, CombatStatus::Idle));
    }

    #[test]
    fn primary_target_spawn_ids_filters_group_target_slots() {
        let list = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 100,
                    name: "add".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::GroupAssistTarget,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 200,
                    name: "main".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::RaidAssist2Target,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 300,
                    name: "raid_main".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::SpecificNpc,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 400,
                    name: "manual".into(),
                },
            ],
            auto_add_haters: true,
        };

        assert_eq!(list.primary_target_spawn_ids(), vec![200, 300]);
    }

    #[test]
    fn cc_add_spawn_ids_exclude_primary_target_duplicates() {
        let list = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 100,
                    name: "main_dup".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::GroupAssistTarget,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 100,
                    name: "main_dup".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 200,
                    name: "add_1".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 300,
                    name: "add_2".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::AutoHater,
                    status: XTargetSlotStatus::DifferentZone,
                    spawn_id: 400,
                    name: "remote".into(),
                },
            ],
            auto_add_haters: true,
        };

        assert_eq!(list.cc_add_spawn_ids(), vec![200, 300]);
        assert_eq!(list.first_cc_add_spawn_id(), Some(200));
    }

    #[test]
    fn combat_status_engaging_carries_target() {
        let status = CombatStatus::Engaging { target_id: 42 };
        if let CombatStatus::Engaging { target_id } = status {
            assert_eq!(target_id, 42);
        } else {
            panic!("expected Engaging");
        }
    }

    #[test]
    fn combat_role_equality() {
        assert_eq!(CombatRole::MainTank, CombatRole::MainTank);
        assert_ne!(CombatRole::MainTank, CombatRole::Healer);
        assert_ne!(CombatRole::Puller, CombatRole::DpsMelee);
    }

    #[test]
    fn combat_config_default() {
        let config = CombatConfig::default();
        assert_eq!(config.role, CombatRole::DpsMelee);
        assert!(config.pull_method.is_none());
        assert!(matches!(config.assist_mode, AssistMode::AssistTrain));
        assert!(config.spells.is_empty());
        assert!(config.disciplines.is_empty());
        assert!(config.holyshit_rules.is_empty());
        assert!((config.mana_floor - 20.0).abs() < f32::EPSILON);
        assert_eq!(config.aoe_threshold, 3);
    }

    #[test]
    fn combat_status_all_variants_constructible() {
        let _idle = CombatStatus::Idle;
        let _engaging = CombatStatus::Engaging { target_id: 1 };
        let _casting = CombatStatus::Casting {
            spell_slot: 0,
            target_id: 1,
        };
        let _gcd = CombatStatus::OnGcd;
        let _pulling = CombatStatus::Pulling { target_id: 1 };
        let _recovering = CombatStatus::Recovering;
        let _fleeing = CombatStatus::Fleeing;
        let _dead = CombatStatus::Dead;
    }

    #[test]
    fn combat_role_all_variants_constructible() {
        let _mt = CombatRole::MainTank;
        let _ot = CombatRole::OffTank;
        let _healer = CombatRole::Healer;
        let _puller = CombatRole::Puller;
        let _melee = CombatRole::DpsMelee;
        let _ranged = CombatRole::DpsRanged;
        let _cc = CombatRole::CrowdControl;
        let _support = CombatRole::Support;
    }

    #[test]
    fn condition_expr_all_variants_constructible() {
        let _and = ConditionExpr::And(vec![ConditionExpr::Always]);
        let _or = ConditionExpr::Or(vec![ConditionExpr::AggroOnMe]);
        let _hp = ConditionExpr::HpBelow(50.0);
        let _mana = ConditionExpr::ManaBelow(20.0);
        let _thp_above = ConditionExpr::TargetHpAbove(90.0);
        let _thp_below = ConditionExpr::TargetHpBelow(10.0);
        let _aggro = ConditionExpr::AggroOnMe;
        let _always = ConditionExpr::Always;
    }

    #[test]
    fn condition_expr_nested_and_or() {
        let expr = ConditionExpr::And(vec![
            ConditionExpr::HpBelow(30.0),
            ConditionExpr::Or(vec![
                ConditionExpr::AggroOnMe,
                ConditionExpr::TargetHpBelow(20.0),
            ]),
        ]);
        // Just verify construction and debug formatting
        let debug = format!("{:?}", expr);
        assert!(debug.contains("And"));
        assert!(debug.contains("Or"));
    }

    #[test]
    fn extended_target_list_pet_helpers_use_pet_slots() {
        let xtargets = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::MyPet,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 1001,
                    name: "Fluffy".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::MyPetTarget,
                    status: XTargetSlotStatus::CurrentZone,
                    spawn_id: 2002,
                    name: "A fire beetle".into(),
                },
            ],
            auto_add_haters: false,
        };

        assert_eq!(
            xtargets.pet().map(|slot| slot.name.as_str()),
            Some("Fluffy")
        );
        assert_eq!(xtargets.pet_spawn_id(), Some(1001));
        assert_eq!(
            xtargets.pet_target().map(|slot| slot.name.as_str()),
            Some("A fire beetle")
        );
        assert_eq!(xtargets.pet_target_id(), Some(2002));
    }

    #[test]
    fn extended_target_list_pet_helpers_ignore_inactive_pet_slots() {
        let xtargets = ExtendedTargetList {
            slots: vec![
                ExtendedTargetSlot {
                    slot_type: XTargetType::MyPet,
                    status: XTargetSlotStatus::DifferentZone,
                    spawn_id: 1001,
                    name: "Fluffy".into(),
                },
                ExtendedTargetSlot {
                    slot_type: XTargetType::MyPetTarget,
                    status: XTargetSlotStatus::Empty,
                    spawn_id: 2002,
                    name: "A fire beetle".into(),
                },
            ],
            auto_add_haters: false,
        };

        assert!(xtargets.pet().is_none());
        assert_eq!(xtargets.pet_spawn_id(), None);
        assert!(xtargets.pet_target().is_none());
        assert_eq!(xtargets.pet_target_id(), None);
    }

    #[test]
    fn holyshit_condition_construction() {
        let cond = HolyShitCondition {
            priority: 1,
            condition: ConditionExpr::HpBelow(20.0),
            action: HolyShitAction::Flee,
        };
        assert_eq!(cond.priority, 1);
        assert!(matches!(cond.action, HolyShitAction::Flee));
    }

    #[test]
    fn holyshit_action_all_variants() {
        let actions = [
            HolyShitAction::CastSpell(3),
            HolyShitAction::UseAbility(100),
            HolyShitAction::UseItem(5001),
            HolyShitAction::Flee,
        ];
        assert_eq!(actions.len(), 4);
    }

    #[test]
    fn pull_method_all_variants() {
        let _spell = PullMethod::SpellPull { spell_slot: 1 };
        let _bow = PullMethod::BowPull;
        let _prox = PullMethod::ProximityPull;
    }

    #[test]
    fn assist_mode_all_variants() {
        let _train = AssistMode::AssistTrain;
        let _split = AssistMode::SplitDps;
        let _solo = AssistMode::Solo;
    }

    #[test]
    fn spell_entry_construction() {
        let entry = SpellEntry {
            slot: 1,
            spell_id: 12345,
            name: "Complete Heal".into(),
            min_mana_pct: 30.0,
            priority: 1,
            is_aoe: false,
        };
        assert_eq!(entry.slot, 1);
        assert_eq!(entry.spell_id, 12345);
        assert_eq!(entry.name, "Complete Heal");
        assert!(!entry.is_aoe);
    }

    #[test]
    fn spell_entry_default_spell_id() {
        // spell_id has #[serde(default)] so should deserialize as 0 when missing
        let json = r#"{"slot":1,"name":"Test","min_mana_pct":10.0,"priority":1,"is_aoe":false}"#;
        let entry: SpellEntry = serde_json::from_str(json).expect("deserialize");
        assert_eq!(entry.spell_id, 0);
    }

    #[test]
    fn discipline_entry_construction() {
        let disc = DisciplineEntry {
            name: "Defensive".into(),
            spell_id: 9999,
            priority: 1,
            cooldown_ticks: 6000,
            min_hp_pct: 0.0,
            max_hp_pct: 50.0,
            min_endurance_pct: 10.0,
        };
        assert_eq!(disc.name, "Defensive");
        assert_eq!(disc.cooldown_ticks, 6000);
        assert!((disc.max_hp_pct - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn combat_config_serialization_roundtrip() {
        let config = CombatConfig {
            role: CombatRole::MainTank,
            pull_method: Some(PullMethod::BowPull),
            assist_mode: AssistMode::AssistTrain,
            spells: vec![SpellEntry {
                slot: 1,
                spell_id: 100,
                name: "Taunt".into(),
                min_mana_pct: 0.0,
                priority: 1,
                is_aoe: false,
            }],
            disciplines: vec![],
            holyshit_rules: vec![HolyShitCondition {
                priority: 1,
                condition: ConditionExpr::HpBelow(20.0),
                action: HolyShitAction::Flee,
            }],
            mana_floor: 15.0,
            aoe_threshold: 5,
            target_scan: TargetScanConfig::default(),
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let restored: CombatConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.role, CombatRole::MainTank);
        assert!(restored.pull_method.is_some());
        assert_eq!(restored.spells.len(), 1);
        assert_eq!(restored.holyshit_rules.len(), 1);
        assert_eq!(restored.aoe_threshold, 5);
    }

    #[test]
    fn combat_status_serialization_roundtrip() {
        let statuses = [
            CombatStatus::Idle,
            CombatStatus::Engaging { target_id: 42 },
            CombatStatus::Casting {
                spell_slot: 3,
                target_id: 100,
            },
            CombatStatus::OnGcd,
            CombatStatus::Pulling { target_id: 7 },
            CombatStatus::Recovering,
            CombatStatus::Fleeing,
            CombatStatus::Dead,
        ];
        for status in &statuses {
            let json = serde_json::to_string(status).expect("serialize");
            let restored: CombatStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*status, restored);
        }
    }

    #[test]
    fn cast_readiness_all_variants_constructible() {
        let variants = [
            CastReadiness::Ready,
            CastReadiness::Casting,
            CastReadiness::GlobalCooldown,
            CastReadiness::Recovering,
            CastReadiness::NotMemorized,
        ];
        assert_eq!(variants.len(), 5);
    }

    #[test]
    fn cast_readiness_display() {
        assert_eq!(CastReadiness::Ready.to_string(), "Ready");
        assert_eq!(CastReadiness::Casting.to_string(), "Casting");
        assert_eq!(CastReadiness::GlobalCooldown.to_string(), "GCD");
        assert_eq!(CastReadiness::Recovering.to_string(), "Recovering");
        assert_eq!(CastReadiness::NotMemorized.to_string(), "Not Memorized");
    }

    #[test]
    fn cast_readiness_serialization_roundtrip() {
        let variants = [
            CastReadiness::Ready,
            CastReadiness::Casting,
            CastReadiness::GlobalCooldown,
            CastReadiness::Recovering,
            CastReadiness::NotMemorized,
        ];
        for variant in &variants {
            let json = serde_json::to_string(variant).expect("serialize");
            let restored: CastReadiness = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*variant, restored);
        }
    }

    #[test]
    fn cast_result_all_variants_constructible() {
        let variants = [
            CastResult::Success,
            CastResult::Fizzled,
            CastResult::Collapsed,
            CastResult::Resisted,
            CastResult::Immune,
            CastResult::Interrupted,
            CastResult::Aborted,
            CastResult::NotReady,
            CastResult::OutOfMana,
            CastResult::OutOfRange,
            CastResult::CannotSee,
            CastResult::NoTarget,
            CastResult::Standing,
            CastResult::Stunned,
            CastResult::Components,
            CastResult::Cancelled,
            CastResult::Distracted,
            CastResult::Invisible,
            CastResult::Outdoors,
            CastResult::Pending,
            CastResult::Recovering,
            CastResult::TakeHold,
            CastResult::Unknown,
        ];
        assert_eq!(variants.len(), 23);
    }

    #[test]
    fn cast_result_display() {
        assert_eq!(CastResult::Success.to_string(), "Success");
        assert_eq!(CastResult::Fizzled.to_string(), "Fizzled");
        assert_eq!(CastResult::Collapsed.to_string(), "Collapsed");
        assert_eq!(CastResult::Resisted.to_string(), "Resisted");
        assert_eq!(CastResult::Immune.to_string(), "Immune");
        assert_eq!(CastResult::Interrupted.to_string(), "Interrupted");
        assert_eq!(CastResult::Aborted.to_string(), "Aborted");
        assert_eq!(CastResult::NotReady.to_string(), "Not Ready");
        assert_eq!(CastResult::OutOfMana.to_string(), "Out of Mana");
        assert_eq!(CastResult::OutOfRange.to_string(), "Out of Range");
        assert_eq!(CastResult::CannotSee.to_string(), "Cannot See");
        assert_eq!(CastResult::NoTarget.to_string(), "No Target");
        assert_eq!(CastResult::Standing.to_string(), "Standing");
        assert_eq!(CastResult::Stunned.to_string(), "Stunned");
        assert_eq!(CastResult::Components.to_string(), "Components");
        assert_eq!(CastResult::Cancelled.to_string(), "Cancelled");
        assert_eq!(CastResult::Distracted.to_string(), "Distracted");
        assert_eq!(CastResult::Invisible.to_string(), "Invisible");
        assert_eq!(CastResult::Outdoors.to_string(), "Outdoors");
        assert_eq!(CastResult::Pending.to_string(), "Pending");
        assert_eq!(CastResult::Recovering.to_string(), "Recovering");
        assert_eq!(CastResult::TakeHold.to_string(), "Take Hold");
        assert_eq!(CastResult::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn cast_result_serialization_roundtrip() {
        let variants = [
            CastResult::Success,
            CastResult::Fizzled,
            CastResult::Collapsed,
            CastResult::Resisted,
            CastResult::Immune,
            CastResult::Interrupted,
            CastResult::Aborted,
            CastResult::NotReady,
            CastResult::OutOfMana,
            CastResult::OutOfRange,
            CastResult::CannotSee,
            CastResult::NoTarget,
            CastResult::Standing,
            CastResult::Stunned,
            CastResult::Components,
            CastResult::Cancelled,
            CastResult::Distracted,
            CastResult::Invisible,
            CastResult::Outdoors,
            CastResult::Pending,
            CastResult::Recovering,
            CastResult::TakeHold,
            CastResult::Unknown,
        ];
        for variant in &variants {
            let json = serde_json::to_string(variant).expect("serialize");
            let restored: CastResult = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*variant, restored);
        }
    }

    #[test]
    fn cast_result_feedback_message_parsing() {
        let cases = [
            ("Your spell fizzles!", Some(CastResult::Fizzled)),
            (
                "Your gate is too unstable, and collapses.",
                Some(CastResult::Collapsed),
            ),
            (
                "Your target resisted the Ensnare spell.",
                Some(CastResult::Resisted),
            ),
            (
                "Your target is immune to changes in its attack speed.",
                Some(CastResult::Immune),
            ),
            ("Your spell is interrupted.", Some(CastResult::Interrupted)),
            ("Your spell is aborted.", Some(CastResult::Aborted)),
            (
                "Spell is not ready yet, please wait.",
                Some(CastResult::NotReady),
            ),
            (
                "You don't have enough mana to cast this spell.",
                Some(CastResult::OutOfMana),
            ),
            (
                "Your target is out of range, get closer!",
                Some(CastResult::OutOfRange),
            ),
            ("You cannot see your target.", Some(CastResult::CannotSee)),
            (
                "You must first select a target for this spell!",
                Some(CastResult::NoTarget),
            ),
            ("Stand up first!", Some(CastResult::Standing)),
            ("You are stunned!", Some(CastResult::Stunned)),
            (
                "You are missing some components for this spell.",
                Some(CastResult::Components),
            ),
            (
                "Your spell has been cancelled.",
                Some(CastResult::Cancelled),
            ),
            (
                "You have been distracted from your casting.",
                Some(CastResult::Distracted),
            ),
            (
                "You cannot cast while invisible.",
                Some(CastResult::Invisible),
            ),
            (
                "You can only cast this spell in the outdoors.",
                Some(CastResult::Outdoors),
            ),
            (
                "You already have a spell pending.",
                Some(CastResult::Pending),
            ),
            ("You haven't recovered yet...", Some(CastResult::Recovering)),
            (
                "Your spell did not take hold on your target.",
                Some(CastResult::TakeHold),
            ),
            ("Completely unrelated text", None),
        ];

        for (message, expected) in cases {
            assert_eq!(CastResult::from_feedback_message(message), expected);
        }
    }

    #[test]
    fn cast_result_retryable_and_landed_helpers() {
        assert!(CastResult::Fizzled.is_retryable());
        assert!(CastResult::Collapsed.is_retryable());
        assert!(CastResult::Interrupted.is_retryable());
        assert!(CastResult::NotReady.is_retryable());
        assert!(!CastResult::Resisted.is_retryable());
        assert!(!CastResult::OutOfMana.is_retryable());
        assert!(CastResult::Success.landed());
        assert!(!CastResult::TakeHold.landed());
    }

    #[test]
    fn cast_telemetry_construction_and_display() {
        let telemetry = CastTelemetry {
            result: CastResult::Success,
            cast_duration_ms: 2500,
            spell_id: 12345,
            target_id: 42,
            timestamp_ms: 1700000000000,
        };
        assert_eq!(telemetry.result, CastResult::Success);
        assert_eq!(telemetry.cast_duration_ms, 2500);
        assert_eq!(telemetry.spell_id, 12345);
        assert_eq!(telemetry.target_id, 42);
        let display = telemetry.to_string();
        assert!(display.contains("spell=12345"));
        assert!(display.contains("target=42"));
        assert!(display.contains("result=Success"));
        assert!(display.contains("duration=2500ms"));
    }

    #[test]
    fn cast_telemetry_serialization_roundtrip() {
        let telemetry = CastTelemetry {
            result: CastResult::Interrupted,
            cast_duration_ms: 1200,
            spell_id: 9999,
            target_id: 100,
            timestamp_ms: 1700000001000,
        };
        let json = serde_json::to_string(&telemetry).expect("serialize");
        let restored: CastTelemetry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(telemetry, restored);
    }

    #[test]
    fn cast_readiness_equality_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(CastReadiness::Ready);
        set.insert(CastReadiness::Ready);
        set.insert(CastReadiness::Casting);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn cast_result_equality_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(CastResult::Success);
        set.insert(CastResult::Success);
        set.insert(CastResult::Fizzled);
        assert_eq!(set.len(), 2);
    }

    // ── Buff timer tests ──────────────────────────────────────────────

    fn make_timer(
        spell_id: i32,
        target_id: u32,
        applied_at: u64,
        duration: u64,
        priority: BuffPriority,
    ) -> BuffTimer {
        BuffTimer {
            spell_id,
            spell_name: format!("Spell{spell_id}"),
            caster_id: 1,
            target_id,
            applied_at,
            duration_ticks: duration,
            priority,
        }
    }

    #[test]
    fn buff_timer_expires_at() {
        let t = make_timer(100, 1, 1000, 500, BuffPriority::Normal);
        assert_eq!(t.expires_at(), 1500);
    }

    #[test]
    fn buff_timer_time_remaining() {
        let t = make_timer(100, 1, 1000, 500, BuffPriority::Normal);
        assert_eq!(t.time_remaining(1200), 300);
        assert_eq!(t.time_remaining(1500), 0);
        assert_eq!(t.time_remaining(1600), 0);
    }

    #[test]
    fn buff_timer_is_expired() {
        let t = make_timer(100, 1, 1000, 500, BuffPriority::Normal);
        assert!(!t.is_expired(1499));
        assert!(t.is_expired(1500));
        assert!(t.is_expired(2000));
    }

    #[test]
    fn buff_timer_expiring_soon() {
        let t = make_timer(100, 1, 1000, 500, BuffPriority::Normal);
        assert!(t.is_expiring_soon(1300, 300));
        assert!(!t.is_expiring_soon(1100, 300));
        assert!(t.is_expiring_soon(1600, 300));
    }

    #[test]
    fn buff_timer_saturating_math() {
        let t = make_timer(100, 1, u64::MAX - 10, 20, BuffPriority::Normal);
        assert_eq!(t.expires_at(), u64::MAX);
    }

    #[test]
    fn buff_timer_set_add_and_count() {
        let mut set = BuffTimerSet::new();
        set.add_buff(make_timer(100, 1, 0, 1000, BuffPriority::Normal));
        set.add_buff(make_timer(200, 1, 0, 1000, BuffPriority::High));
        assert_eq!(set.active_count(0), 2);
    }

    #[test]
    fn buff_timer_set_add_replaces_duplicate() {
        let mut set = BuffTimerSet::new();
        set.add_buff(make_timer(100, 1, 0, 500, BuffPriority::Normal));
        set.add_buff(make_timer(100, 1, 400, 500, BuffPriority::Normal));
        assert_eq!(set.timers.len(), 1);
        assert_eq!(set.timers[0].applied_at, 400);
    }

    #[test]
    fn buff_timer_set_add_same_spell_different_targets() {
        let mut set = BuffTimerSet::new();
        set.add_buff(make_timer(100, 1, 0, 500, BuffPriority::Normal));
        set.add_buff(make_timer(100, 2, 0, 500, BuffPriority::Normal));
        assert_eq!(set.timers.len(), 2);
    }

    #[test]
    fn buff_timer_set_remove() {
        let mut set = BuffTimerSet::new();
        set.add_buff(make_timer(100, 1, 0, 1000, BuffPriority::Normal));
        set.add_buff(make_timer(200, 1, 0, 1000, BuffPriority::High));
        set.remove_buff(100, 1);
        assert_eq!(set.timers.len(), 1);
        assert_eq!(set.timers[0].spell_id, 200);
    }

    #[test]
    fn buff_timer_set_remove_nonexistent_is_noop() {
        let mut set = BuffTimerSet::new();
        set.add_buff(make_timer(100, 1, 0, 1000, BuffPriority::Normal));
        set.remove_buff(999, 1);
        assert_eq!(set.timers.len(), 1);
    }

    #[test]
    fn buff_timer_set_get_expired() {
        let mut set = BuffTimerSet::new();
        set.add_buff(make_timer(100, 1, 0, 500, BuffPriority::Normal));
        set.add_buff(make_timer(200, 1, 0, 1000, BuffPriority::High));
        let expired = set.get_expired(600);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].spell_id, 100);
    }

    #[test]
    fn buff_timer_set_expiring_soon_sorted() {
        let mut set = BuffTimerSet::new();
        set.add_buff(make_timer(100, 1, 0, 1000, BuffPriority::Low));
        set.add_buff(make_timer(200, 1, 0, 1200, BuffPriority::Critical));
        set.add_buff(make_timer(300, 1, 0, 900, BuffPriority::Normal));
        let expiring = set.get_expiring_soon(700, 600);
        assert_eq!(expiring.len(), 3);
        assert_eq!(expiring[0].priority, BuffPriority::Critical);
        assert_eq!(expiring[1].priority, BuffPriority::Normal);
        assert_eq!(expiring[2].priority, BuffPriority::Low);
    }

    #[test]
    fn buff_timer_set_expiring_soon_excludes_expired() {
        let mut set = BuffTimerSet::new();
        set.add_buff(make_timer(100, 1, 0, 500, BuffPriority::Normal));
        set.add_buff(make_timer(200, 1, 0, 1000, BuffPriority::High));
        let expiring = set.get_expiring_soon(600, 500);
        assert_eq!(expiring.len(), 1);
        assert_eq!(expiring[0].spell_id, 200);
    }

    #[test]
    fn buff_timer_set_purge_expired() {
        let mut set = BuffTimerSet::new();
        set.add_buff(make_timer(100, 1, 0, 500, BuffPriority::Normal));
        set.add_buff(make_timer(200, 1, 0, 1000, BuffPriority::High));
        set.add_buff(make_timer(300, 1, 0, 300, BuffPriority::Low));
        let purged = set.purge_expired(600);
        assert_eq!(purged, 2);
        assert_eq!(set.timers.len(), 1);
        assert_eq!(set.timers[0].spell_id, 200);
    }

    #[test]
    fn buff_timer_set_active_count_excludes_expired() {
        let mut set = BuffTimerSet::new();
        set.add_buff(make_timer(100, 1, 0, 500, BuffPriority::Normal));
        set.add_buff(make_timer(200, 1, 0, 1000, BuffPriority::High));
        assert_eq!(set.active_count(600), 1);
    }

    #[test]
    fn buff_priority_ordering() {
        assert!(BuffPriority::Critical < BuffPriority::High);
        assert!(BuffPriority::High < BuffPriority::Normal);
        assert!(BuffPriority::Normal < BuffPriority::Low);
    }

    #[test]
    fn buff_timer_serialization_roundtrip() {
        let timer = make_timer(100, 42, 5000, 18000, BuffPriority::Critical);
        let json = serde_json::to_string(&timer).expect("serialize");
        let restored: BuffTimer = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.spell_id, 100);
        assert_eq!(restored.target_id, 42);
        assert_eq!(restored.applied_at, 5000);
        assert_eq!(restored.duration_ticks, 18000);
        assert!(matches!(restored.priority, BuffPriority::Critical));
    }

    #[test]
    fn buff_timer_set_serialization_roundtrip() {
        let mut set = BuffTimerSet::new();
        set.add_buff(make_timer(100, 1, 0, 1000, BuffPriority::Normal));
        set.add_buff(make_timer(200, 2, 100, 2000, BuffPriority::Critical));
        let json = serde_json::to_string(&set).expect("serialize");
        let restored: BuffTimerSet = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.timers.len(), 2);
    }

    #[test]
    fn buff_timer_set_default_is_empty() {
        let set = BuffTimerSet::default();
        assert!(set.timers.is_empty());
    }

    // ── AbilitySet / resolve_abilities tests ────────────────────────────

    fn nuke_set() -> AbilitySet {
        AbilitySet {
            name: "Nuke".into(),
            candidates: vec![
                AbilityCandidate {
                    name: "Ice Comet".into(),
                    min_level: 60,
                    spell_id: 1500,
                },
                AbilityCandidate {
                    name: "Frost".into(),
                    min_level: 52,
                    spell_id: 1200,
                },
                AbilityCandidate {
                    name: "Chill Sight".into(),
                    min_level: 44,
                    spell_id: 900,
                },
            ],
        }
    }

    fn known_spells() -> Vec<KnownAbility> {
        vec![
            KnownAbility {
                name: "Ice Comet".into(),
                spell_id: 1500,
                level: 60,
            },
            KnownAbility {
                name: "Frost".into(),
                spell_id: 1200,
                level: 52,
            },
            KnownAbility {
                name: "Chill Sight".into(),
                spell_id: 900,
                level: 44,
            },
        ]
    }

    #[test]
    fn resolve_picks_highest_known() {
        let sets = vec![nuke_set()];
        let result = resolve_abilities(&sets, &known_spells(), 65);
        let nuke = result.get("Nuke").expect("should resolve Nuke");
        assert_eq!(nuke.ability_name, "Ice Comet");
        assert_eq!(nuke.spell_id, 1500);
    }

    #[test]
    fn resolve_skips_too_high_level() {
        let sets = vec![nuke_set()];
        let result = resolve_abilities(&sets, &known_spells(), 55);
        let nuke = result.get("Nuke").expect("should resolve Nuke");
        assert_eq!(nuke.ability_name, "Frost");
    }

    #[test]
    fn resolve_empty_when_nothing_known() {
        let sets = vec![nuke_set()];
        let result = resolve_abilities(&sets, &[], 65);
        assert!(result.is_empty());
    }

    #[test]
    fn resolve_empty_when_under_all_levels() {
        let sets = vec![nuke_set()];
        let result = resolve_abilities(&sets, &known_spells(), 30);
        assert!(result.is_empty());
    }

    #[test]
    fn resolve_multiple_sets() {
        let heal_set = AbilitySet {
            name: "Heal".into(),
            candidates: vec![AbilityCandidate {
                name: "Complete Heal".into(),
                min_level: 39,
                spell_id: 13,
            }],
        };
        let known = vec![
            KnownAbility {
                name: "Ice Comet".into(),
                spell_id: 1500,
                level: 60,
            },
            KnownAbility {
                name: "Complete Heal".into(),
                spell_id: 13,
                level: 39,
            },
        ];
        let result = resolve_abilities(&[nuke_set(), heal_set], &known, 65);
        assert_eq!(result.len(), 2);
        assert!(result.contains_key("Nuke"));
        assert!(result.contains_key("Heal"));
    }

    #[test]
    fn resolve_case_insensitive_name_match() {
        let sets = vec![nuke_set()];
        let known = vec![KnownAbility {
            name: "ice comet".into(),
            spell_id: -1,
            level: 60,
        }];
        let result = resolve_abilities(&sets, &known, 65);
        assert!(result.contains_key("Nuke"));
    }

    #[test]
    fn resolve_matches_by_spell_id() {
        let sets = vec![nuke_set()];
        let known = vec![KnownAbility {
            name: "Renamed Spell".into(),
            spell_id: 1500,
            level: 60,
        }];
        let result = resolve_abilities(&sets, &known, 65);
        let nuke = result.get("Nuke").expect("should match by spell_id");
        assert_eq!(nuke.ability_name, "Ice Comet");
    }

    #[test]
    fn resolve_first_known_candidate_wins() {
        let sets = vec![nuke_set()];
        let known = vec![
            KnownAbility {
                name: "Frost".into(),
                spell_id: 1200,
                level: 52,
            },
            KnownAbility {
                name: "Chill Sight".into(),
                spell_id: 900,
                level: 44,
            },
        ];
        let result = resolve_abilities(&sets, &known, 55);
        let nuke = result.get("Nuke").expect("should resolve Nuke");
        assert_eq!(nuke.ability_name, "Frost");
    }

    // -- HateTargetCategory tests --

    #[test]
    fn hate_target_category_default_is_active_hater() {
        assert_eq!(
            HateTargetCategory::default(),
            HateTargetCategory::ActiveHater
        );
    }

    #[test]
    fn hate_target_category_all_variants_constructible() {
        let _ah = HateTargetCategory::ActiveHater;
        let _ca = HateTargetCategory::CasterAdd;
        let _ma = HateTargetCategory::MeleeAdd;
        let _ap = HateTargetCategory::Approaching;
        let _ro = HateTargetCategory::Roamer;
    }

    #[test]
    fn hate_target_category_cc_priority_unique_and_ordered() {
        let mut priorities: Vec<u8> = vec![
            HateTargetCategory::ActiveHater.cc_priority(),
            HateTargetCategory::CasterAdd.cc_priority(),
            HateTargetCategory::Approaching.cc_priority(),
            HateTargetCategory::MeleeAdd.cc_priority(),
            HateTargetCategory::Roamer.cc_priority(),
        ];
        // All priorities distinct
        priorities.sort_unstable();
        let before_dedup = priorities.len();
        priorities.dedup();
        assert_eq!(
            priorities.len(),
            before_dedup,
            "CC priorities should be unique"
        );
        // Monotonically increasing after sort
        for i in 1..priorities.len() {
            assert!(priorities[i] > priorities[i - 1]);
        }
    }

    #[test]
    fn hate_target_category_kite_priority_unique_and_ordered() {
        let mut priorities: Vec<u8> = vec![
            HateTargetCategory::CasterAdd.kite_priority(),
            HateTargetCategory::Approaching.kite_priority(),
            HateTargetCategory::ActiveHater.kite_priority(),
            HateTargetCategory::MeleeAdd.kite_priority(),
            HateTargetCategory::Roamer.kite_priority(),
        ];
        priorities.sort_unstable();
        let before_dedup = priorities.len();
        priorities.dedup();
        assert_eq!(
            priorities.len(),
            before_dedup,
            "Kite priorities should be unique"
        );
        for i in 1..priorities.len() {
            assert!(priorities[i] > priorities[i - 1]);
        }
    }

    #[test]
    fn hate_target_category_prefers_cc_and_kite_consistent() {
        use HateTargetCategory::*;
        // prefers_cc and prefers_kite are not mutually exclusive (CasterAdd is both)
        assert!(CasterAdd.prefers_cc());
        assert!(CasterAdd.prefers_kite());
        // Roamer prefers neither
        assert!(!Roamer.prefers_cc());
        assert!(!Roamer.prefers_kite());
    }

    #[test]
    fn hate_target_category_serialization_roundtrip() {
        let cats = [
            HateTargetCategory::ActiveHater,
            HateTargetCategory::CasterAdd,
            HateTargetCategory::MeleeAdd,
            HateTargetCategory::Approaching,
            HateTargetCategory::Roamer,
        ];
        for cat in &cats {
            let json = serde_json::to_string(cat).expect("serialize");
            let back: HateTargetCategory = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, *cat);
        }
    }
}
