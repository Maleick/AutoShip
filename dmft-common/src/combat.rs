use serde::{Deserialize, Serialize};

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
    /// The character has aggro from a mob.
    AggroOnMe,
    /// Always true — unconditional trigger.
    Always,
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
    /// Cast was interrupted (took damage, moved, etc.).
    Interrupted,
    /// Spell fizzled (failed skill check).
    Fizzled,
    /// Target was out of range.
    OutOfRange,
    /// Not enough mana to cast.
    OutOfMana,
    /// Target is immune to this spell.
    Immune,
    /// Target resisted the spell.
    Resisted,
    /// Caster was not ready (GCD, already casting, etc.).
    NotReady,
}

impl std::fmt::Display for CastResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "Success"),
            Self::Interrupted => write!(f, "Interrupted"),
            Self::Fizzled => write!(f, "Fizzled"),
            Self::OutOfRange => write!(f, "Out of Range"),
            Self::OutOfMana => write!(f, "Out of Mana"),
            Self::Immune => write!(f, "Immune"),
            Self::Resisted => write!(f, "Resisted"),
            Self::NotReady => write!(f, "Not Ready"),
        }
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
            CastResult::Interrupted,
            CastResult::Fizzled,
            CastResult::OutOfRange,
            CastResult::OutOfMana,
            CastResult::Immune,
            CastResult::Resisted,
            CastResult::NotReady,
        ];
        assert_eq!(variants.len(), 8);
    }

    #[test]
    fn cast_result_display() {
        assert_eq!(CastResult::Success.to_string(), "Success");
        assert_eq!(CastResult::Interrupted.to_string(), "Interrupted");
        assert_eq!(CastResult::Fizzled.to_string(), "Fizzled");
        assert_eq!(CastResult::OutOfRange.to_string(), "Out of Range");
        assert_eq!(CastResult::OutOfMana.to_string(), "Out of Mana");
        assert_eq!(CastResult::Immune.to_string(), "Immune");
        assert_eq!(CastResult::Resisted.to_string(), "Resisted");
        assert_eq!(CastResult::NotReady.to_string(), "Not Ready");
    }

    #[test]
    fn cast_result_serialization_roundtrip() {
        let variants = [
            CastResult::Success,
            CastResult::Interrupted,
            CastResult::Fizzled,
            CastResult::OutOfRange,
            CastResult::OutOfMana,
            CastResult::Immune,
            CastResult::Resisted,
            CastResult::NotReady,
        ];
        for variant in &variants {
            let json = serde_json::to_string(variant).expect("serialize");
            let restored: CastResult = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(*variant, restored);
        }
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
}
